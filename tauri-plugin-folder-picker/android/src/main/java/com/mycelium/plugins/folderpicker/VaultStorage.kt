package com.mycelium.plugins.folderpicker

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.provider.DocumentsContract
import java.io.FileNotFoundException

/**
 * The vault, as reached through the Storage Access Framework.
 *
 * Android does not hand out a path for a folder the user picks — it hands out a
 * `content://` tree URI, and since Android 11 there is no supported way to turn
 * one into something `open(2)` will accept. Everything above this class still
 * speaks in vault-relative paths like `daily/2026-08-21.org`; mapping those onto
 * document URIs is this class's job and nobody else's, which is what keeps the
 * Rust indexer and the editor identical on both platforms.
 *
 * Document IDs are treated as opaque. They are *usually* readable strings like
 * `primary:Notes/inbox.org`, and it is tempting to build child IDs by string
 * concatenation, but that is a property of one provider rather than of the
 * framework. Instead the tree is enumerated once and the resulting
 * path-to-document-ID map is cached, which is also what makes indexing fast.
 */
class VaultStorage(private val context: Context) {

    data class OrgFile(val path: String, val lastModified: Long)

    private val resolver: ContentResolver get() = context.contentResolver

    private var treeUri: Uri? = null

    /** Vault-relative path to document ID, filled by [listOrgFiles]. */
    private val documentIds = mutableMapOf<String, String>()

    /** Directories only, so a write can find or create its parent. */
    private val directoryIds = mutableMapOf<String, String>()

    // ── Access ────────────────────────────────────────────────────────────

    /**
     * Record the folder the user picked and hold onto it across restarts.
     *
     * Without [ContentResolver.takePersistableUriPermission] the grant dies with
     * the process and the vault is unreachable on next launch.
     */
    fun persist(uri: Uri) {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        runCatching { resolver.takePersistableUriPermission(uri, flags) }
        prefs().edit().putString(KEY_TREE, uri.toString()).apply()
        adopt(uri)
    }

    /**
     * The folder from a previous run, or null.
     *
     * The stored grant is re-checked rather than trusted: the user can revoke it
     * in system settings, and the folder itself can be deleted or unmounted.
     */
    fun restore(): Uri? {
        val stored = prefs().getString(KEY_TREE, null) ?: return null
        val uri = runCatching { Uri.parse(stored) }.getOrNull() ?: return null

        val held = resolver.persistedUriPermissions.any {
            it.uri == uri && it.isReadPermission && it.isWritePermission
        }
        if (!held) {
            forget()
            return null
        }
        adopt(uri)
        // Still granted is not the same as still there.
        return if (rootDocumentId() != null) uri else { forget(); null }
    }

    fun forget() {
        prefs().edit().remove(KEY_TREE).apply()
        treeUri = null
        documentIds.clear()
        directoryIds.clear()
    }

    private fun adopt(uri: Uri) {
        treeUri = uri
        documentIds.clear()
        directoryIds.clear()
    }

    private fun prefs() = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    private fun rootDocumentId(): String? {
        val tree = treeUri ?: return null
        return runCatching { DocumentsContract.getTreeDocumentId(tree) }.getOrNull()
    }

    private fun documentUri(documentId: String): Uri? {
        val tree = treeUri ?: return null
        return DocumentsContract.buildDocumentUriUsingTree(tree, documentId)
    }

    // ── Enumeration ───────────────────────────────────────────────────────

    /**
     * Every `.org` file in the vault, with its modification time.
     *
     * Queries each directory once for all of its children, rather than using
     * `DocumentFile.listFiles()` which issues a query per entry — on a vault of
     * any size that difference is the difference between usable and not.
     *
     * Populates the path caches as a side effect, so the writes and reads that
     * follow an index pass resolve without touching the provider again.
     */
    fun listOrgFiles(): List<OrgFile> {
        val root = rootDocumentId() ?: return emptyList()
        documentIds.clear()
        directoryIds.clear()
        directoryIds[""] = root

        val found = mutableListOf<OrgFile>()
        walk(root, "", found, depth = 0)
        return found
    }

    private fun walk(parentId: String, parentPath: String, out: MutableList<OrgFile>, depth: Int) {
        // A pathological tree should not take the app down with it.
        if (depth > MAX_DEPTH) return
        val tree = treeUri ?: return
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)

        val cursor: Cursor = runCatching {
            resolver.query(children, CHILD_COLUMNS, null, null, null)
        }.getOrNull() ?: return

        cursor.use {
            while (it.moveToNext()) {
                val id = it.getString(0) ?: continue
                val name = it.getString(1) ?: continue
                val mime = it.getString(2)
                val modified = if (it.isNull(3)) 0L else it.getLong(3)
                val path = if (parentPath.isEmpty()) name else "$parentPath/$name"

                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    // The same directories the indexer skips, for the same reasons:
                    // a .git holds far more files than the notes beside it.
                    if (IGNORED_DIRS.contains(name)) continue
                    directoryIds[path] = id
                    walk(id, path, out, depth + 1)
                } else if (name.endsWith(ORG_SUFFIX)) {
                    documentIds[path] = id
                    out.add(OrgFile(path, modified))
                }
            }
        }
    }

    // ── Files ─────────────────────────────────────────────────────────────

    fun exists(path: String): Boolean = resolveDocument(path) != null

    fun lastModified(path: String): Long? {
        val id = resolveDocument(path) ?: return null
        val uri = documentUri(id) ?: return null
        return queryLong(uri, DocumentsContract.Document.COLUMN_LAST_MODIFIED)
    }

    fun read(path: String): ByteArray {
        val id = resolveDocument(path) ?: throw FileNotFoundException(path)
        val uri = documentUri(id) ?: throw FileNotFoundException(path)
        return resolver.openInputStream(uri)?.use { it.readBytes() }
            ?: throw FileNotFoundException(path)
    }

    fun delete(path: String): Boolean {
        val id = resolveDocument(path) ?: return false
        val uri = documentUri(id) ?: return false
        val deleted = runCatching { DocumentsContract.deleteDocument(resolver, uri) }
            .getOrDefault(false)
        if (deleted) documentIds.remove(path)
        return deleted
    }

    /**
     * Replace a file's contents, in the three phases `db::atomic` defines.
     *
     * SAF has no rename-into-place, so the sequence is: write `.part`, rename it
     * to `.ready` to record that it is complete, delete the target, then rename
     * `.ready` into its place. A crash therefore leaves either a `.part`, which
     * is incomplete and always safe to discard, or a `.ready`, which holds the
     * whole note — and the vault sweep on next open acts on that difference.
     *
     * Providers are not obliged to support renaming. Where the flag is absent
     * the content is written straight to the target, which is what SAF would
     * have done anyway; the guarantee is weaker and there is no way around it.
     */
    fun write(path: String, bytes: ByteArray, id: String) {
        val name = path.substringAfterLast('/')
        val parentPath = if (path.contains('/')) path.substringBeforeLast('/') else ""
        val parentId = resolveDirectory(parentPath, create = true)
            ?: throw FileNotFoundException("no parent for $path")

        if (!supportsRename(parentId)) {
            writeDirect(path, parentId, name, bytes)
            return
        }

        val partName = ".$name.$id.part"
        val partUri = createDocument(parentId, partName)
            ?: throw FileNotFoundException("could not create $partName")
        writeBytes(partUri, bytes)

        val readyName = ".$name.$id.ready"
        val readyUri = runCatching { DocumentsContract.renameDocument(resolver, partUri, readyName) }
            .getOrNull()
        if (readyUri == null) {
            // Could not mark it complete, so it must not be left looking complete.
            runCatching { DocumentsContract.deleteDocument(resolver, partUri) }
            throw FileNotFoundException("could not stage $path")
        }

        delete(path)
        val published = runCatching { DocumentsContract.renameDocument(resolver, readyUri, name) }
            .getOrNull()
            ?: throw FileNotFoundException("could not publish $path")

        documentIds[path] = DocumentsContract.getDocumentId(published)
    }

    private fun writeDirect(path: String, parentId: String, name: String, bytes: ByteArray) {
        val existing = resolveDocument(path)?.let { documentUri(it) }
        val target = existing
            ?: createDocument(parentId, name)
            ?: throw FileNotFoundException("could not create $path")
        writeBytes(target, bytes)
        documentIds[path] = DocumentsContract.getDocumentId(target)
    }

    private fun writeBytes(uri: Uri, bytes: ByteArray) {
        // "wt" truncates first; without it a shorter note leaves the tail of the
        // longer one it replaced.
        resolver.openOutputStream(uri, "wt")?.use { it.write(bytes) }
            ?: throw FileNotFoundException(uri.toString())
    }

    fun mkdirs(path: String) {
        resolveDirectory(path, create = true)
    }

    // ── Resolution ────────────────────────────────────────────────────────

    private fun resolveDocument(path: String): String? {
        documentIds[path]?.let { return it }
        val parentPath = if (path.contains('/')) path.substringBeforeLast('/') else ""
        val parentId = resolveDirectory(parentPath, create = false) ?: return null
        val name = path.substringAfterLast('/')
        val id = findChild(parentId, name, wantDirectory = false) ?: return null
        documentIds[path] = id
        return id
    }

    /** Walk (and optionally create) a directory chain one segment at a time. */
    private fun resolveDirectory(path: String, create: Boolean): String? {
        if (path.isEmpty()) return directoryIds[""] ?: rootDocumentId()?.also { directoryIds[""] = it }
        directoryIds[path]?.let { return it }

        var currentPath = ""
        var currentId = directoryIds[""] ?: rootDocumentId() ?: return null
        for (segment in path.split('/')) {
            if (segment.isEmpty()) continue
            currentPath = if (currentPath.isEmpty()) segment else "$currentPath/$segment"
            val cached = directoryIds[currentPath]
            if (cached != null) {
                currentId = cached
                continue
            }
            var found = findChild(currentId, segment, wantDirectory = true)
            if (found == null && create) {
                found = createDirectory(currentId, segment)
            }
            if (found == null) return null
            directoryIds[currentPath] = found
            currentId = found
        }
        return currentId
    }

    private fun findChild(parentId: String, name: String, wantDirectory: Boolean): String? {
        val tree = treeUri ?: return null
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)
        val cursor = runCatching {
            resolver.query(children, CHILD_COLUMNS, null, null, null)
        }.getOrNull() ?: return null

        cursor.use {
            while (it.moveToNext()) {
                if (it.getString(1) != name) continue
                val isDir = it.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR
                if (isDir == wantDirectory) return it.getString(0)
            }
        }
        return null
    }

    private fun createDocument(parentId: String, name: String): Uri? {
        val parent = documentUri(parentId) ?: return null
        // Org files are plain text; the provider decides what it does with the
        // hint, and some append an extension if the name has none.
        return runCatching {
            DocumentsContract.createDocument(resolver, parent, MIME_TEXT, name)
        }.getOrNull()
    }

    private fun createDirectory(parentId: String, name: String): String? {
        val parent = documentUri(parentId) ?: return null
        val created = runCatching {
            DocumentsContract.createDocument(
                resolver, parent, DocumentsContract.Document.MIME_TYPE_DIR, name
            )
        }.getOrNull() ?: return null
        return DocumentsContract.getDocumentId(created)
    }

    private fun supportsRename(documentId: String): Boolean {
        val uri = documentUri(documentId) ?: return false
        val flags = queryLong(uri, DocumentsContract.Document.COLUMN_FLAGS) ?: return false
        return flags and DocumentsContract.Document.FLAG_SUPPORTS_RENAME.toLong() != 0L
    }

    private fun queryLong(uri: Uri, column: String): Long? {
        val cursor = runCatching {
            resolver.query(uri, arrayOf(column), null, null, null)
        }.getOrNull() ?: return null
        return cursor.use { if (it.moveToFirst() && !it.isNull(0)) it.getLong(0) else null }
    }

    private companion object {
        const val PREFS = "mycelium_vault"
        const val KEY_TREE = "vault_tree_uri"
        const val ORG_SUFFIX = ".org"
        const val MIME_TEXT = "text/plain"
        const val MAX_DEPTH = 32

        val CHILD_COLUMNS = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        )

        /** Mirrors `db::sync::IGNORED_DIRS`. */
        val IGNORED_DIRS = setOf(
            ".git", ".hg", ".svn", ".jj",
            ".stversions", ".stfolder",
            ".trash", ".Trash",
            "node_modules",
        )
    }
}

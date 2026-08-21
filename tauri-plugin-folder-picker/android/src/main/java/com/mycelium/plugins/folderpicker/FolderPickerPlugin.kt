package com.mycelium.plugins.folderpicker

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import android.util.Base64

@InvokeArg
class PathArgs {
    lateinit var path: String
}

@InvokeArg
class WriteArgs {
    lateinit var path: String
    /** Base64, so a pasted image survives the trip as faithfully as a note. */
    lateinit var contents: String
    lateinit var id: String
}

/**
 * Vault access and the editing toolbar, exposed to the Rust side.
 *
 * The plugin is deliberately thin. Reaching the vault lives in [VaultStorage]
 * and the accessory bar in [KeyboardToolbar]; what remains here is argument
 * parsing, threading and turning results into JSON.
 *
 * Every file command runs on [Dispatchers.IO]. The Storage Access Framework
 * talks to another process over Binder, so doing any of this on the main thread
 * would block the UI for as long as the provider takes to answer.
 */
@TauriPlugin
class FolderPickerPlugin(private val activity: Activity) : Plugin(activity) {

    private val storage by lazy { VaultStorage(activity) }
    private val toolbar by lazy { KeyboardToolbar(activity) }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var webView: WebView? = null

    override fun load(webView: WebView) {
        this.webView = webView
    }

    // ── Choosing a vault ──────────────────────────────────────────────────

    @Command
    fun pickFolder(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                    Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
            )
        }
        startActivityForResult(invoke, intent, "folderPicked")
    }

    @ActivityCallback
    fun folderPicked(invoke: Invoke, result: ActivityResult) {
        val uri: Uri? = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            // A cancelled picker is an ordinary outcome, not a failure.
            invoke.resolve(JSObject().apply { put("path", null as String?) })
            return
        }
        storage.persist(uri)
        invoke.resolve(JSObject().apply { put("path", uri.toString()) })
    }

    @Command
    fun restoreAccess(invoke: Invoke) {
        scope.launch {
            val uri = storage.restore()
            invoke.resolve(JSObject().apply { put("path", uri?.toString()) })
        }
    }

    @Command
    fun forgetVault(invoke: Invoke) {
        storage.forget()
        invoke.resolve(JSObject())
    }

    // ── Files ─────────────────────────────────────────────────────────────

    @Command
    fun listOrgFiles(invoke: Invoke) {
        io(invoke) {
            val files = JSArray()
            for (file in storage.listOrgFiles()) {
                files.put(JSObject().apply {
                    put("path", file.path)
                    // Compared for equality only, never parsed.
                    put("mtime", file.lastModified.toString())
                })
            }
            JSObject().apply { put("files", files) }
        }
    }

    @Command
    fun readFile(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        io(invoke) {
            val bytes = storage.read(args.path)
            JSObject().apply { put("contents", Base64.encodeToString(bytes, Base64.NO_WRAP)) }
        }
    }

    @Command
    fun writeFile(invoke: Invoke) {
        val args = invoke.parseArgs(WriteArgs::class.java)
        io(invoke) {
            storage.write(args.path, Base64.decode(args.contents, Base64.DEFAULT), args.id)
            JSObject()
        }
    }

    @Command
    fun deleteFile(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        io(invoke) { JSObject().apply { put("deleted", storage.delete(args.path)) } }
    }

    @Command
    fun fileExists(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        io(invoke) { JSObject().apply { put("exists", storage.exists(args.path)) } }
    }

    @Command
    fun fileModified(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        io(invoke) {
            JSObject().apply { put("mtime", storage.lastModified(args.path)?.toString()) }
        }
    }

    @Command
    fun createDirectory(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        io(invoke) {
            storage.mkdirs(args.path)
            JSObject()
        }
    }

    // ── Editing toolbar ───────────────────────────────────────────────────

    @Command
    fun setupToolbar(invoke: Invoke) {
        val wv = webView
        if (wv == null) {
            invoke.resolve(JSObject().apply { put("installed", false) })
            return
        }
        activity.runOnUiThread {
            toolbar.install(wv)
            invoke.resolve(JSObject().apply { put("installed", true) })
        }
    }

    /**
     * Run provider work off the main thread and answer the caller either way.
     *
     * A command that neither resolves nor rejects leaves the Rust side waiting
     * forever, so the failure path is as important as the success one.
     */
    private fun io(invoke: Invoke, block: () -> JSObject) {
        scope.launch {
            try {
                invoke.resolve(block())
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.javaClass.simpleName)
            }
        }
    }
}

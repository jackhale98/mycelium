/// Durable in-place file replacement, in three phases.
///
/// A single temporary file cannot be recovered from, because a leftover is
/// ambiguous: it may hold a complete note whose publish never happened, or the
/// first half of one whose write was interrupted. They look identical on disk,
/// so the only safe response is to delete it — throwing away an edit whenever it
/// was the former.
///
/// Naming the phases removes the ambiguity:
///
/// 1. content is written to `.<name>.<id>.part` and flushed
/// 2. `.part` is renamed to `.ready`, which is the record that it is complete
/// 3. `.ready` is renamed over the target, publishing it
///
/// A crash therefore leaves either a `.part`, which is incomplete and always
/// safe to discard, or a `.ready`, which holds the whole note and can be put
/// back. [`crate::sweep`] acts on that distinction when a vault opens.
///
/// Renaming within a directory is atomic on a native filesystem, so a reader
/// never sees a half-written note; steps 1 and 2 cost one extra rename to make
/// the interrupted case recoverable. The Storage Access Framework has no
/// rename-into-place, so the Android implementation runs the same three phases
/// with a delete between steps 2 and 3 — see `VaultFs::write`.
use std::io::Write;
use std::path::{Path, PathBuf};

/// Suffix of a temporary whose content is still being written.
pub const PART: &str = "part";
/// Suffix of a temporary that holds a complete note, not yet published.
pub const READY: &str = "ready";

/// `.<name>.<id>.<phase>`, beside the target so the rename stays within one
/// directory. The leading dot keeps it out of the way; the sweep recognises the
/// shape, and the README asks users to gitignore it.
pub fn temp_path(target: &Path, id: &str, phase: &str) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!(".{name}.{id}.{phase}"))
}

/// Replace a file's contents, leaving a recoverable temporary if interrupted.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", dir.display()))?;

    let id = uuid::Uuid::new_v4().simple().to_string();
    let part = temp_path(path, &id, PART);
    let ready = temp_path(path, &id, READY);

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&part)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Failed to write file: {e}"));
    }

    // Carry the target's permissions so a publish never changes its mode, which
    // would otherwise show up as a spurious change in git.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.permissions().mode();
            let _ = std::fs::set_permissions(&part, std::fs::Permissions::from_mode(mode));
        }
    }

    // The content is on disk and flushed; this rename is what marks it complete.
    if let Err(e) = std::fs::rename(&part, &ready) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Failed to write file: {e}"));
    }

    if let Err(e) = std::fs::rename(&ready, path) {
        // Leave the .ready in place: it holds the complete note, and the sweep
        // can publish it later. Removing it here would discard the edit.
        return Err(format!("Failed to write file: {e}"));
    }

    #[cfg(unix)]
    {
        if let Ok(dir_handle) = std::fs::File::open(dir) {
            let _ = dir_handle.sync_all();
        }
    }

    Ok(())
}

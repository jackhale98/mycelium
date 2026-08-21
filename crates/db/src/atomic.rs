/// Durable in-place file replacement.
///
/// A note is written to a temporary file in the same directory, flushed, and
/// renamed over the target, so a concurrent reader — the file watcher, a git
/// client, another device — never observes a half-written file. The temporary
/// name is a dotfile carrying the target's name and a UUID, which the vault
/// sweep recognises and clears if a write is interrupted.
///
/// This lives beside `VaultFs` because it is the native answer to
/// `VaultFs::write`. The Storage Access Framework offers no rename-into-place,
/// so the Android implementation cannot reproduce the guarantee.
use std::io::Write;
use std::path::Path;

pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;

    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", dir.display()))?;

    let tmp = dir.join(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Failed to write file: {e}"));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.permissions().mode();
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
        }
    }

    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
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

//! Vault access on Android, over the Storage Access Framework.
//!
//! Android hands the app a `content://` tree URI for the folder the user picks,
//! not a path, and since Android 11 there is no supported way to turn one into
//! something `std::fs` will open. This implements the same [`db::VaultFs`] the
//! rest of the app is written against by calling into the Kotlin plugin, which
//! owns the mapping from vault-relative paths onto document URIs.
//!
//! The one behavioural difference is documented at [`db::VaultFs::write`]: the
//! native write renames a temporary over its target, which is atomic, while the
//! Storage Access Framework has no such operation. The Kotlin side runs the
//! same three phases so an interrupted write stays recoverable, but a reader
//! can still catch the moment between the delete and the publish.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use db::{VaultDirEntry, VaultEntry, VaultFs, VaultFsError};
use tauri::{AppHandle, Runtime};
use tauri_plugin_folder_picker::FolderPickerExt;

pub struct AndroidFs<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AndroidFs<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

fn failed(path: &str, error: impl std::fmt::Display) -> VaultFsError {
    VaultFsError::Io(format!("{path}: {error}"))
}

impl<R: Runtime> VaultFs for AndroidFs<R> {
    /// `root` is ignored: the plugin already holds the tree the user granted,
    /// and there is no second vault it could be asked about.
    fn list_org_files(&self, _root: &str) -> Result<Vec<VaultEntry>, VaultFsError> {
        let response = self
            .app
            .folder_picker()
            .list_org_files()
            .map_err(|e| VaultFsError::Io(e.to_string()))?;
        Ok(response
            .files
            .into_iter()
            .map(|f| VaultEntry {
                path: f.path,
                mtime: f.mtime,
            })
            .collect())
    }

    fn read_to_string(&self, path: &str) -> Result<String, VaultFsError> {
        let response = self
            .app
            .folder_picker()
            .read_file(path)
            .map_err(|e| failed(path, e))?;
        let bytes = BASE64
            .decode(response.contents)
            .map_err(|e| failed(path, e))?;
        String::from_utf8(bytes).map_err(|e| failed(path, e))
    }

    fn write(&self, path: &str, content: &str) -> Result<(), VaultFsError> {
        self.write_bytes(path, content.as_bytes())
    }

    fn write_bytes(&self, path: &str, bytes: &[u8]) -> Result<(), VaultFsError> {
        // The id pairs this write's `.part` with its `.ready`, so an interrupted
        // save can be told apart from a complete one that never published.
        let id = uuid::Uuid::new_v4().simple().to_string();
        self.app
            .folder_picker()
            .write_file(path, BASE64.encode(bytes), &id)
            .map(|_| ())
            .map_err(|e| failed(path, e))
    }

    fn modified(&self, path: &str) -> Result<String, VaultFsError> {
        let response = self
            .app
            .folder_picker()
            .file_modified(path)
            .map_err(|e| failed(path, e))?;
        response
            .mtime
            .ok_or_else(|| VaultFsError::NotFound(path.to_string()))
    }

    fn exists(&self, path: &str) -> bool {
        self.app
            .folder_picker()
            .file_exists(path)
            .map(|r| r.exists)
            .unwrap_or(false)
    }

    fn remove_file(&self, path: &str) -> Result<(), VaultFsError> {
        let response = self
            .app
            .folder_picker()
            .delete_file(path)
            .map_err(|e| failed(path, e))?;
        if response.deleted {
            Ok(())
        } else {
            Err(VaultFsError::NotFound(path.to_string()))
        }
    }

    fn create_dir_all(&self, path: &str) -> Result<(), VaultFsError> {
        self.app
            .folder_picker()
            .create_directory(path)
            .map(|_| ())
            .map_err(|e| failed(path, e))
    }

    /// Not offered. Directory listing exists so the vault sweep can find stray
    /// temporaries, and on Android that work belongs to the plugin's own walk —
    /// exposing a second, slower path through here would invite its use.
    fn read_dir(&self, path: &str) -> Result<Vec<VaultDirEntry>, VaultFsError> {
        Err(VaultFsError::Io(format!(
            "directory listing is not available through the Android bridge: {path}"
        )))
    }
}

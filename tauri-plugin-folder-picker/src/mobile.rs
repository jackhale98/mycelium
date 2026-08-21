use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_folder_picker);

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.mycelium.plugins.folderpicker";

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<FolderPicker<R>> {
    #[cfg(target_os = "ios")]
    {
        let handle = api.register_ios_plugin(init_plugin_folder_picker)?;
        Ok(FolderPicker(Some(handle)))
    }
    #[cfg(target_os = "android")]
    {
        let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "FolderPickerPlugin")?;
        Ok(FolderPicker(Some(handle)))
    }
}

pub struct FolderPicker<R: Runtime>(Option<PluginHandle<R>>);

impl<R: Runtime> FolderPicker<R> {
    pub fn pick_folder(&self, _payload: PickFolderRequest) -> crate::Result<PickFolderResponse> {
        match &self.0 {
            Some(handle) => handle
                .run_mobile_plugin("pickFolder", ())
                .map_err(Into::into),
            None => Ok(PickFolderResponse { path: None }),
        }
    }

    pub fn restore_access(&self) -> crate::Result<PickFolderResponse> {
        match &self.0 {
            Some(handle) => handle
                .run_mobile_plugin("restoreAccess", ())
                .map_err(Into::into),
            None => Ok(PickFolderResponse { path: None }),
        }
    }

    // ── Vault files ───────────────────────────────────────────────────
    //
    // Android only. On desktop and iOS the vault is a real directory and
    // `db::NativeFs` answers these directly, so the handle is never consulted.

    pub fn list_org_files(&self) -> crate::Result<ListFilesResponse> {
        self.call("listOrgFiles", ())
    }

    pub fn read_file(&self, path: &str) -> crate::Result<ReadResponse> {
        self.call("readFile", PathRequest { path: path.to_string() })
    }

    pub fn write_file(&self, path: &str, contents: String, id: &str) -> crate::Result<EmptyResponse> {
        self.call(
            "writeFile",
            WriteRequest { path: path.to_string(), contents, id: id.to_string() },
        )
    }

    pub fn delete_file(&self, path: &str) -> crate::Result<DeletedResponse> {
        self.call("deleteFile", PathRequest { path: path.to_string() })
    }

    pub fn file_exists(&self, path: &str) -> crate::Result<ExistsResponse> {
        self.call("fileExists", PathRequest { path: path.to_string() })
    }

    pub fn file_modified(&self, path: &str) -> crate::Result<ModifiedResponse> {
        self.call("fileModified", PathRequest { path: path.to_string() })
    }

    pub fn create_directory(&self, path: &str) -> crate::Result<EmptyResponse> {
        self.call("createDirectory", PathRequest { path: path.to_string() })
    }

    fn call<T, P>(&self, command: &str, payload: P) -> crate::Result<T>
    where
        T: DeserializeOwned,
        P: serde::Serialize,
    {
        let handle = self.0.as_ref().ok_or(crate::Error::Unavailable)?;
        handle.run_mobile_plugin(command, payload).map_err(Into::into)
    }

    pub fn setup_toolbar(&self) -> crate::Result<PickFolderResponse> {
        match &self.0 {
            Some(handle) => handle
                .run_mobile_plugin("setupToolbar", ())
                .map_err(Into::into),
            None => Ok(PickFolderResponse { path: None }),
        }
    }
}

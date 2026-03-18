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

    pub fn setup_toolbar(&self) -> crate::Result<PickFolderResponse> {
        match &self.0 {
            Some(handle) => handle
                .run_mobile_plugin("setupToolbar", ())
                .map_err(Into::into),
            None => Ok(PickFolderResponse { path: None }),
        }
    }
}

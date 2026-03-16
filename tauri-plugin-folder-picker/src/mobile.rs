use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_folder_picker);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<FolderPicker<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("", "FolderPickerPlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_folder_picker)?;
    Ok(FolderPicker(handle))
}

pub struct FolderPicker<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> FolderPicker<R> {
    pub fn pick_folder(&self, _payload: PickFolderRequest) -> crate::Result<PickFolderResponse> {
        self.0
            .run_mobile_plugin("pickFolder", ())
            .map_err(Into::into)
    }
}

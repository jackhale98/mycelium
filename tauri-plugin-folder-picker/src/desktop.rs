use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<FolderPicker<R>> {
    Ok(FolderPicker(app.clone()))
}

pub struct FolderPicker<R: Runtime>(AppHandle<R>);

impl<R: Runtime> FolderPicker<R> {
    pub fn pick_folder(&self, _payload: PickFolderRequest) -> crate::Result<PickFolderResponse> {
        // On desktop, return None — the frontend uses tauri-plugin-dialog instead
        Ok(PickFolderResponse { path: None })
    }

    pub fn restore_access(&self) -> crate::Result<PickFolderResponse> {
        // On desktop, no security-scoped access needed
        Ok(PickFolderResponse { path: None })
    }
}

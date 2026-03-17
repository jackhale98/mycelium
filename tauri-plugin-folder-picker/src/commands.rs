use tauri::{AppHandle, command, Runtime};

use crate::models::*;
use crate::Result;
use crate::FolderPickerExt;

#[command]
pub(crate) async fn pick_folder<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PickFolderResponse> {
    app.folder_picker().pick_folder(PickFolderRequest {})
}

#[command]
pub(crate) async fn restore_access<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PickFolderResponse> {
    app.folder_picker().restore_access()
}

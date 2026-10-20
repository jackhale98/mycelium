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

#[command]
pub(crate) async fn setup_toolbar<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PickFolderResponse> {
    app.folder_picker().setup_toolbar()
}

#[command]
pub(crate) async fn set_editor_focused<R: Runtime>(
    app: AppHandle<R>,
    focused: bool,
) -> Result<()> {
    app.folder_picker().set_editor_focused(focused)
}

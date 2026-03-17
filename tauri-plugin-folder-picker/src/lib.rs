use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::FolderPicker;
#[cfg(mobile)]
use mobile::FolderPicker;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the folder-picker APIs.
pub trait FolderPickerExt<R: Runtime> {
  fn folder_picker(&self) -> &FolderPicker<R>;
}

impl<R: Runtime, T: Manager<R>> crate::FolderPickerExt<R> for T {
  fn folder_picker(&self) -> &FolderPicker<R> {
    self.state::<FolderPicker<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("folder-picker")
    .invoke_handler(tauri::generate_handler![commands::pick_folder, commands::restore_access])
    .setup(|app, api| {
      #[cfg(mobile)]
      let folder_picker = mobile::init(app, api)?;
      #[cfg(desktop)]
      let folder_picker = desktop::init(app, api)?;
      app.manage(folder_picker);
      Ok(())
    })
    .build()
}

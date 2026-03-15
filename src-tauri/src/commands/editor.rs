use crate::state::AppState;
use db::index;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

/// Read the contents of an org file
#[tauri::command]
pub async fn read_file(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;

    // Ensure the file is within the vault
    let full_path = if PathBuf::from(&file_path).is_absolute() {
        PathBuf::from(&file_path)
    } else {
        vault_path.join(&file_path)
    };

    if !full_path.starts_with(&vault_path) {
        return Err("File path is outside the vault directory.".to_string());
    }

    std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {e}"))
}

/// Save file contents, re-index, and emit db-updated event
#[tauri::command]
pub async fn save_file(
    app: AppHandle,
    file_path: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault_path = state.vault_path()?;

    let full_path = if PathBuf::from(&file_path).is_absolute() {
        PathBuf::from(&file_path)
    } else {
        vault_path.join(&file_path)
    };

    if !full_path.starts_with(&vault_path) {
        return Err("File path is outside the vault directory.".to_string());
    }

    // Write file
    std::fs::write(&full_path, &content)
        .map_err(|e| format!("Failed to write file: {e}"))?;

    // Re-index the file
    let file_path_str = full_path.to_string_lossy().to_string();
    state.with_db(|conn| {
        index::index_file(conn, &file_path_str, &content)
            .map_err(|e| format!("Failed to index file: {e}"))
    })?;

    // Emit db-updated event so frontend can refresh
    let _ = app.emit("db-updated", ());

    Ok(())
}

/// Create a new org file with a UUID node (file-level property drawer)
#[tauri::command]
pub async fn create_file(
    app: AppHandle,
    title: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let id = uuid::Uuid::new_v4().to_string();

    // Create filename from title
    let filename = sanitize_filename(&title);
    let file_path = vault_path.join(format!("{filename}.org"));

    if file_path.exists() {
        return Err(format!("File already exists: {}", file_path.display()));
    }

    let content = format!(
        ":PROPERTIES:\n:ID: {id}\n:END:\n#+TITLE: {title}\n"
    );

    std::fs::write(&file_path, &content)
        .map_err(|e| format!("Failed to create file: {e}"))?;

    // Index the new file
    let file_path_str = file_path.to_string_lossy().to_string();
    state.with_db(|conn| {
        index::index_file(conn, &file_path_str, &content)
            .map_err(|e| format!("Failed to index file: {e}"))
    })?;

    // Emit db-updated event
    let _ = app.emit("db-updated", ());

    Ok(file_path_str)
}

/// Import an image file into the vault's images/ directory.
/// Copies the source file and returns the relative org link path.
#[tauri::command]
pub async fn import_image(
    source_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let images_dir = vault_path.join("images");

    // Create images/ directory if it doesn't exist
    std::fs::create_dir_all(&images_dir)
        .map_err(|e| format!("Failed to create images directory: {e}"))?;

    let source = PathBuf::from(&source_path);
    if !source.exists() {
        return Err(format!("Source file not found: {source_path}"));
    }

    // Get filename, deduplicate if needed
    let original_name = source
        .file_name()
        .ok_or("Invalid filename")?
        .to_string_lossy()
        .to_string();

    let mut dest_name = original_name.clone();
    let mut dest = images_dir.join(&dest_name);
    let mut counter = 1;
    while dest.exists() {
        let stem = source
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let ext = source
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        dest_name = format!("{stem}-{counter}{ext}");
        dest = images_dir.join(&dest_name);
        counter += 1;
    }

    // Copy the file
    std::fs::copy(&source, &dest)
        .map_err(|e| format!("Failed to copy image: {e}"))?;

    // Return the relative path for the org link
    Ok(format!("images/{dest_name}"))
}

fn sanitize_filename(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .replace(' ', "-")
        .to_lowercase()
}

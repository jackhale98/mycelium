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

/// Quick capture: append a text snippet to today's daily note.
/// Creates the daily note if it doesn't exist.
/// This is the fastest way to jot down a thought on mobile.
#[tauri::command]
pub async fn quick_capture(
    app: AppHandle,
    text: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let daily_dir = vault_path.join("daily");
    std::fs::create_dir_all(&daily_dir)
        .map_err(|e| format!("Failed to create daily directory: {e}"))?;

    // Get today's date
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, _, _, _) = unix_to_datetime(secs);
    let date = format!("{year:04}-{month:02}-{day:02}");

    // Find or create today's daily note
    let daily_file = find_daily_file(&daily_dir, &date);
    let (file_path, mut content) = match daily_file {
        Some(path) => {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read daily note: {e}"))?;
            (path, content)
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            let ts = timestamp_now();
            let slug = date.replace('-', "_");
            let path = daily_dir.join(format!("{ts}-{slug}.org"));
            let content = format!(":PROPERTIES:\n:ID: {id}\n:END:\n#+TITLE: {date}\n\n");
            (path, content)
        }
    };

    // Append the captured text
    if !content.ends_with('\n') {
        content.push('\n');
    }
    let (hour, min, _) = {
        let s = secs;
        (((s / 3600) % 24) as u32, ((s / 60) % 60) as u32, (s % 60) as u32)
    };
    content.push_str(&format!("- [{hour:02}:{min:02}] {text}\n"));

    std::fs::write(&file_path, &content)
        .map_err(|e| format!("Failed to write: {e}"))?;

    let file_str = file_path.to_string_lossy().to_string();
    state.with_db(|conn| {
        index::index_file(conn, &file_str, &content)
            .map_err(|e| format!("Failed to index: {e}"))
    })?;

    let _ = app.emit("db-updated", ());
    Ok(file_str)
}

/// Find an existing daily note file for a given date
fn find_daily_file(daily_dir: &std::path::Path, date: &str) -> Option<std::path::PathBuf> {
    let date_slug = date.replace('-', "_");
    if let Ok(entries) = std::fs::read_dir(daily_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".org") && (name.contains(date) || name.contains(&date_slug)) {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Create a new org file with a UUID node (file-level property drawer).
/// Uses org-roam naming convention: YYYYMMDDHHmmss-slug.org
#[tauri::command]
pub async fn create_file(
    app: AppHandle,
    title: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let id = uuid::Uuid::new_v4().to_string();

    // Org-roam filename convention: YYYYMMDDHHmmss-slug.org
    let timestamp = timestamp_now();
    let slug = slugify(&title);
    let filename = format!("{timestamp}-{slug}");
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

/// Generate current timestamp in YYYYMMDDHHmmss format.
pub fn timestamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, hour, min, sec) = unix_to_datetime(secs);
    format!("{year:04}{month:02}{day:02}{hour:02}{min:02}{sec:02}")
}

/// Convert a title to an org-roam compatible slug.
/// "My Great Note!" -> "my_great_note"
fn slugify(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '-' || c == '_' {
                '_'
            } else {
                // skip other characters
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        // Collapse multiple underscores
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Convert unix timestamp to (year, month, day, hour, minute, second).
/// Simple implementation without chrono dependency.
fn unix_to_datetime(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    let min = ((secs / 60) % 60) as u32;
    let hour = ((secs / 3600) % 24) as u32;

    let mut days = (secs / 86400) as u32;

    // Calculate year
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    // Calculate month and day
    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for md in month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days + 1;

    (year, month, day, hour, min, sec)
}

fn is_leap_year(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

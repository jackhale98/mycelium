use tauri::State;
use crate::state::AppState;

/// Pick a folder using the native platform picker.
/// On desktop, uses tauri-plugin-dialog.
/// On iOS, returns the app's documents directory (folder picker not supported by dialog plugin).
/// The frontend handles the actual iOS folder picker via a custom approach.
#[tauri::command]
pub async fn get_documents_path() -> Result<String, String> {
    // Return the platform-appropriate documents directory
    #[cfg(target_os = "ios")]
    {
        // On iOS, use the app's Documents directory
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        Ok(format!("{home}/Documents"))
    }

    #[cfg(target_os = "android")]
    {
        // On Android, use external storage documents
        let path = std::env::var("EXTERNAL_STORAGE")
            .unwrap_or_else(|_| "/storage/emulated/0".to_string());
        Ok(format!("{path}/Documents"))
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop — use the home directory
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        Ok(home)
    }
}

/// List subdirectories in a given path (for iOS folder browsing UI)
#[tauri::command]
pub async fn list_subdirectories(path: String) -> Result<Vec<DirEntry>, String> {
    let mut entries = Vec::new();

    let read_dir = std::fs::read_dir(&path)
        .map_err(|e| format!("Cannot read directory: {e}"))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Error reading entry: {e}"))?;
        let metadata = entry.metadata().map_err(|e| format!("Error reading metadata: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') { continue; }

        entries.push(DirEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            has_org_files: if metadata.is_dir() {
                // Quick check if this directory contains .org files
                std::fs::read_dir(entry.path())
                    .map(|rd| rd.flatten().any(|e| {
                        e.path().extension().map(|ext| ext == "org").unwrap_or(false)
                    }))
                    .unwrap_or(false)
            } else {
                false
            },
        });
    }

    entries.sort_by(|a, b| {
        // Directories first, then by name
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });

    Ok(entries)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub has_org_files: bool,
}

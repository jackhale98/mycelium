const COMMANDS: &[&str] = &["pick_folder", "restore_access", "setup_toolbar"];

fn main() {
  tauri_plugin::Builder::new(COMMANDS)
    .ios_path("ios")
    .build();
}

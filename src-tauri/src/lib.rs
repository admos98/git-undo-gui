//! Desktop shell for git-undo-gui.
//!
//! Deliberately thin: the app is a self-contained HTML document and the shell
//! only hosts it. There is no IPC command that reads a repository or executes
//! git, so the security story stays "read-only viewer" even in the desktop build.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running git-undo-gui");
}

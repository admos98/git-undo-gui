//! Desktop shell for git-undo-gui.
//!
//! Weight contract with the web build:
//!   * the same single index.html is the entire UI (no bundled framework)
//!   * the OS webview is used — no Chromium is shipped
//!   * exactly four IPC commands, all thin wrappers over the `git` binary
//!   * no daemon, no local HTTP server, no background thread, no telemetry
//!
//! If `git` is missing, commands fail with `git_unavailable` and the UI falls back
//! to the copy-a-command experience.

mod git;
mod service;

use git_undo_logic::error::GitError;
use git_undo_logic::state::RepoState;
use service::{OpReport, ResetOutcome};
use std::path::PathBuf;

/// Only these four operations are reachable from the frontend.
#[tauri::command]
fn git_read_reflog(path: String, limit: Option<usize>) -> Result<String, GitError> {
    let root = PathBuf::from(path);
    service::read_reflog(&root, limit.unwrap_or(100))
}

#[tauri::command]
fn git_repo_state(path: String) -> Result<RepoState, GitError> {
    service::read_state(&PathBuf::from(path))
}

#[tauri::command]
fn git_create_backup(path: String) -> Result<OpReport, GitError> {
    let root = PathBuf::from(path);
    let g = git::Git::open(&root)?;
    let (name, created) = service::create_backup(&g)?;
    Ok(OpReport {
        ok: true,
        command: format!("git branch {name} HEAD"),
        // Name the branch that now holds the current history, so the safety net is
        // visible in the app rather than only in the terminal.
        now_at: {
            let head = g
                .probe(&["rev-parse", "--short", "HEAD"])
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            format!("{name} @ {head}")
        },
        message: if created {
            format!("Backup branch {name} created at the current HEAD.")
        } else {
            format!("Backup branch {name} already exists and was left untouched.")
        },
        undo: None,
        risks: vec![],
    })
}

#[tauri::command]
fn git_reset(
    path: String,
    target: String,
    strategy: String,
    confirm: Option<String>,
) -> Result<ResetOutcome, GitError> {
    service::reset(&PathBuf::from(path), &target, &strategy, confirm.as_deref())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            git_read_reflog,
            git_repo_state,
            git_create_backup,
            git_reset
        ])
        .run(tauri::generate_context!())
        .expect("error while running git-undo-gui");
}

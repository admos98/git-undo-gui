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

#[cfg(test)]
mod e2e;
mod embed;
mod git;
mod service;

pub use git_undo_logic::error::GitError;
#[cfg(test)]
pub use git::Git;
use service::{OpReport, RepoView, ResetOutcome};
use std::path::PathBuf;

/// Only these four operations are reachable from the frontend.
#[tauri::command]
fn git_read_reflog(path: String, limit: Option<usize>) -> Result<String, GitError> {
    let root = PathBuf::from(path);
    service::read_reflog(&root, limit.unwrap_or(100))
}

#[tauri::command]
fn git_repo_state(path: String) -> Result<RepoView, GitError> {
    service::read_view(&PathBuf::from(path))
}

#[tauri::command]
fn git_create_backup(path: String) -> Result<OpReport, GitError> {
    let root = PathBuf::from(path);
    let g = git::Git::open(&root)?;
    let backup = service::create_backup(&g)?;
    Ok(OpReport {
        ok: true,
        command: format!("git branch {} HEAD", backup.branch),
        // Name the branch *and* the commit it holds. "backup-before-recovery" is a
        // promise; the date is what tells you whether it still keeps it.
        now_at: format!("{} @ {} ({})", backup.branch, &backup.commit[..7.min(backup.commit.len())], backup.date),
        message: if backup.created {
            format!(
                "Backup branch {} now holds your current HEAD. Resetting cannot lose it.",
                backup.branch
            )
        } else if backup.covers_head {
            format!(
                "Backup branch {} already exists and already points at your current HEAD — nothing to add.",
                backup.branch
            )
        } else {
            format!(
                "Backup branch {} already exists from {} and was left untouched. It does NOT contain \
                 your latest work, so it is not a way back from a reset you do next.",
                backup.branch, backup.date
            )
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

/// Serve the embedded page. `app.localhost` is what Tauri registers as the
/// window's custom protocol, and on all three platforms it resolves to a URL the
/// webview accepts — no listening socket is opened.
fn serve_page(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.register_uri_scheme_protocol("app", |_ctx, _request| {
        tauri::http::Response::builder()
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", CSP)
            .body(embed::page().into_bytes())
            .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
    })
}

/// Same policy as the web build, minus the rules a webview needs for its own
/// IPC. Nothing is allowed to phone home.
const CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
                   img-src data:; font-src data:; connect-src 'none'";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            git_read_reflog,
            git_repo_state,
            git_create_backup,
            git_reset
        ]);

    serve_page(builder)
        .run(tauri::generate_context!())
        .expect("error while running git-undo-gui");
}

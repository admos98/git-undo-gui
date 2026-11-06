//! The four operations the UI is allowed to ask for. Nothing else is exposed.
//!
//! Every mutating call follows the same shape:
//!   1. gather facts (state.rs)
//!   2. ask the guard (guard.rs) — a refusal stops here, nothing has run
//!   3. create `backup-before-recovery` if it does not already exist
//!   4. run the single git command and report what actually happened

use crate::git::Git;
use git_undo_logic::error::{GitError, GitResult};
use git_undo_logic::guard::{assess, ResetKind, Risk};
use git_undo_logic::hash::is_valid_hash;
use git_undo_logic::state::{detect_in_progress, parse_porcelain, parse_unpushed_count, RepoState};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const BACKUP_BRANCH: &str = "backup-before-recovery";

#[derive(Debug, Clone, Serialize)]
pub struct OpReport {
    pub ok: bool,
    pub command: String,
    /// What the user should read next. Written as an instruction, not a log line.
    pub message: String,
    /// Branch (or hash) the repository now points at.
    pub now_at: String,
    /// How to get back to where they were before this call.
    pub undo: Option<String>,
    pub risks: Vec<Risk>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetOutcome {
    #[serde(flatten)]
    pub report: OpReport,
    /// HEAD before the reset — what `undo` walks back to.
    pub previous_head: String,
    pub backup_branch: Option<String>,
}

/// Read everything the UI needs. Read-only: safe to call on every keystroke.
pub fn read_state(root: &Path) -> GitResult<RepoState> {
    let git = Git::open(root)?;

    let dirty = parse_porcelain(&git.run(&["status", "--porcelain=v1"]).unwrap_or_default());
    let dirty_count = dirty.len();

    let head = git
        .probe(&["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let branch = git.current_branch();

    // Unpushed commits: compare against the configured upstream. No upstream means
    // we cannot know, and we must not imply "nothing to lose".
    let unpushed = match git.probe(&["rev-list", "--count", "@{upstream}..HEAD"]) {
        Some(count) if parse_unpushed_count(&count).unwrap_or(0) > 0 => git
            .probe(&["rev-list", "--max-count=50", "--abbrev-commit", "@{upstream}..HEAD"])
            .map(|out| out.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    let stash_count = git
        .probe(&["stash", "list"])
        .map(|out| out.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);

    let git_dir = git
        .probe(&["rev-parse", "--absolute-git-dir"])
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| git.root().join(".git"));
    let in_progress = detect_in_progress(|marker| git_dir.join(marker).exists());

    Ok(RepoState {
        branch,
        unpushed,
        dirty,
        dirty_count,
        in_progress,
        stash_count,
        head,
    })
}

pub fn read_reflog(root: &Path, limit: usize) -> GitResult<String> {
    let git = Git::open(root)?;
    let limit = limit.clamp(1, 500).to_string();
    // Same format the web build parses, so the timeline code is shared.
    git.run(&["reflog", "show", &format!("-{limit}"), "--date=iso", "--format=%h %gd: %gs"])
}

/// Create `backup-before-recovery` at the current HEAD, idempotently.
/// Returns (branch name, whether it was created now).
pub fn create_backup(git: &Git) -> GitResult<(String, bool)> {
    let existing = git
        .probe(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{BACKUP_BRANCH}")])
        .is_some();

    if existing {
        // Keep the newest backup; report the old one so nothing is silently lost.
        return Ok((BACKUP_BRANCH.to_string(), false));
    }

    git.run(&["branch", BACKUP_BRANCH, "HEAD"])?;
    Ok((BACKUP_BRANCH.to_string(), true))
}

pub fn reset(
    root: &Path,
    target: &str,
    strategy: &str,
    ack: Option<&str>,
) -> GitResult<ResetOutcome> {
    if !is_valid_hash(target) {
        return Err(GitError::refused(format!(
            "`{target}` is not a commit hash (expected 4-64 hex characters)."
        )));
    }

    let kind = ResetKind::parse(strategy).ok_or_else(|| {
        GitError::refused(format!(
            "Unknown strategy `{strategy}`. Use branch, soft, files or hard."
        ))
    })?;

    let git = Git::open(root)?;
    if !git.has_commit(target) {
        return Err(GitError::UnknownCommit { hash: target.to_string() });
    }

    // ---- gather ----
    let state = read_state(root)?;
    let previous_head = git
        .probe(&["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // ---- guard: a refusal happens before anything is written ----
    let risks = assess(&state, &kind, ack, &previous_head, target)
        .map_err(|reason| GitError::refused(reason))?;

    // ---- backup first, always ----
    let (backup_branch, created) = match kind {
        ResetKind::Branch => (None, false),
        _ => {
            let (name, created) = create_backup(&git)?;
            (Some(name), created)
        }
    };

    // ---- act ----
    let (command, message) = match kind {
        ResetKind::Branch => {
            let name = format!("rescue-{target}");
            git.run(&["branch", &name, target])?;
            (
                format!("git branch {name} {target}"),
                format!("Created branch {name} at {target}. Your current branch did not move."),
            )
        }
        ResetKind::Soft => {
            git.run(&["reset", "--soft", target])?;
            (
                format!("git reset --soft {target}"),
                format!("HEAD moved to {target}. Your changes are staged and ready to recommit."),
            )
        }
        ResetKind::Files => {
            git.run(&["restore", "--source", target, "--", "."])?;
            (
                format!("git restore --source={target} -- ."),
                format!("File contents restored from {target}. Commit history is unchanged."),
            )
        }
        ResetKind::Hard => {
            git.run(&["reset", "--hard", target])?;
            (
                format!("git reset --hard {target}"),
                format!("Branch and working tree are now at {target}. Uncommitted work is gone."),
            )
        }
    };

    let undo = match (&backup_branch, created) {
        (Some(name), true) => Some(format!(
            "git reset --hard {name}   # restores exactly the state you were in a moment ago"
        )),
        (Some(name), false) => Some(format!(
            "git reset --hard {name}   # note: this backup predates this reset, check `git log {name}`"
        )),
        (None, _) => None,
    };

    Ok(ResetOutcome {
        report: OpReport {
            ok: true,
            command,
            message,
            now_at: git
                .probe(&["rev-parse", "--short", "HEAD"])
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            undo,
            risks,
        },
        previous_head,
        backup_branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_branch_name_is_stable() {
        assert_eq!(BACKUP_BRANCH, "backup-before-recovery");
    }

    #[test]
    fn invalid_hashes_are_refused_before_git_is_touched() {
        // No repository needed: validation happens before `Git::open`.
        let err = reset(std::path::Path::new("/nonexistent"), "--help", "hard", Some("RESET"))
            .unwrap_err();
        assert!(matches!(err, GitError::Refused { .. }), "{err:?}");
    }

    #[test]
    fn unknown_strategies_are_refused() {
        let err = reset(std::path::Path::new("/nonexistent"), "a43c821", "yolo", Some("RESET"))
            .unwrap_err();
        assert!(matches!(err, GitError::Refused { .. }), "{err:?}");
    }
}

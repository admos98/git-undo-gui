//! The four operations the UI is allowed to ask for. Nothing else is exposed.
//!
//! Every mutating call follows the same shape:
//!   1. gather facts (state.rs)
//!   2. ask the guard (guard.rs) — a refusal stops here, nothing has run
//!   3. create `backup-before-recovery` if it does not already exist
//!   4. run the single git command and report what actually happened

use crate::git::Git;
use git_undo_logic::error::{GitError, GitResult};
use git_undo_logic::guard::{assess, ResetKind, Risk, TargetFacts};
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
    /// Kept for compatibility; `backup` carries the detail the UI displays.
    pub backup_branch: Option<String>,
    pub backup: Option<BackupReport>,
}

/// What the UI needs to describe the safety net honestly.
#[derive(Debug, Clone, Serialize)]
pub struct BackupReport {
    pub branch: String,
    /// Commit the branch holds — not necessarily the one you just left.
    pub commit: String,
    /// Short date of that commit.
    pub date: String,
    /// Created by this reset, or inherited from an earlier one.
    pub created: bool,
    /// True when the branch points at HEAD, i.e. it holds the state you are
    /// about to leave. False means it is older and is not a way back.
    pub covers_head: bool,
}

/// Read everything the UI needs. Read-only: safe to call on every keystroke.
pub fn read_state(root: &Path) -> GitResult<RepoState> {
    let git = Git::open(root)?;

    // `core.quotepath=false` keeps non-ASCII paths unquoted, so the guard's
    // path-overlap checks compare the real name instead of `"caf\303\251.md"`.
    let dirty = parse_porcelain(
        &git.run(&["-c", "core.quotepath=false", "status", "--porcelain=v1"])
            .unwrap_or_default(),
    );
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

/// What `git_repo_state` returns: the working-tree facts plus a read-only look
/// at the backup branch, so the UI can warn *before* a reset that the safety net
/// it is about to rely on is stale.
#[derive(Debug, Clone, Serialize)]
pub struct RepoView {
    #[serde(flatten)]
    pub state: RepoState,
    pub backup: Option<BackupReport>,
}

/// Inspect the backup branch without creating or moving anything.
pub fn probe_backup(git: &Git) -> GitResult<Option<BackupReport>> {
    let head = git
        .probe(&["rev-parse", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let Some(commit) = git
        .probe(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{BACKUP_BRANCH}")])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return Ok(None);
    };

    let date = git
        .probe(&["log", "-1", "--format=%cd", "--date=short", &commit])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    Ok(Some(BackupReport {
        branch: BACKUP_BRANCH.to_string(),
        covers_head: commit == head,
        commit,
        date,
        created: false,
    }))
}

pub fn read_view(root: &Path) -> GitResult<RepoView> {
    let state = read_state(root)?;
    let git = Git::open(root)?;
    let backup = probe_backup(&git)?;
    Ok(RepoView { state, backup })
}

pub fn read_reflog(root: &Path, limit: usize) -> GitResult<String> {
    let git = Git::open(root)?;
    let limit = limit.clamp(1, 500).to_string();
    // Same format the web build parses, so the timeline code is shared.
    git.run(&["reflog", "show", &format!("-{limit}"), "--date=iso", "--format=%h %gd: %gs"])
}

/// What a backup branch actually is — not just whether we created one.
///
/// The name `backup-before-recovery` promises "before *this* recovery". Since the
/// branch is idempotent, an old backup keeps that promise falsely, and an undo
/// hint pointing at it can rewind a user *past* the state they were trying to
/// recover. So callers get the commit and the date, and can say what it really is.
#[derive(Debug, Clone, PartialEq)]
pub struct Backup {
    pub branch: String,
    /// Commit the backup branch points at.
    pub commit: String,
    /// When that commit was made (short date), for "existing backup from ...".
    pub date: String,
    /// True when this call created the branch, false when one already existed.
    pub created: bool,
    /// True when the backup already points at HEAD, i.e. it is a valid way back
    /// from the reset about to happen. False for a stale backup.
    pub covers_head: bool,
}

impl Backup {
    /// Can `git reset --hard <branch>` reliably undo the reset we are about to do?
    ///
    /// Only if the branch already points at the current HEAD. If it points
    /// somewhere older, following it would discard everything since — the
    /// opposite of a safety net.
    pub fn is_safe_undo(&self) -> bool {
        self.covers_head
    }
}

/// Create `backup-before-recovery` at the current HEAD, idempotently.
pub fn create_backup(git: &Git) -> GitResult<Backup> {
    let head = git
        .probe(&["rev-parse", "HEAD"])
        .map(|s| s.trim().to_string())
        .ok_or_else(|| GitError::internal("cannot resolve HEAD — does this repository have commits?"))?;

    let existing = git
        .probe(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{BACKUP_BRANCH}")])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(commit) = existing {
        // Never overwrite: the newest backup wins, and the old one is reported.
        let date = git
            .probe(&["log", "-1", "--format=%cd", "--date=short", &commit])
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        return Ok(Backup {
            branch: BACKUP_BRANCH.to_string(),
            covers_head: commit == head,
            commit,
            date,
            created: false,
        });
    }

    git.run(&["branch", BACKUP_BRANCH, "HEAD"])?;
    let date = git
        .probe(&["log", "-1", "--format=%cd", "--date=short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Ok(Backup {
        branch: BACKUP_BRANCH.to_string(),
        commit: head.clone(),
        date,
        created: true,
        covers_head: true,
    })
}

/// Every path a snapshot tracks, recursively.
///
/// This is the list the working tree is rewritten from — by `reset --hard` and by
/// `restore --source` alike. The guard intersects it with the dirty files so an
/// untracked file on a tracked path is refused instead of silently replaced.
fn target_tree_paths(git: &Git, rev: &str) -> GitResult<Vec<String>> {
    // `-z` keeps paths unquoted and unambiguous; the output is NUL-separated.
    let out = git.run(&["ls-tree", "-r", "--name-only", "-z", rev])?;
    Ok(out
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect())
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

    // Resolve both sides to full commit ids before comparing anything. `--short`
    // HEAD against a pasted full hash never matched, so "reset to where I already
    // am" walked past the no-op refusal, created a backup and moved nothing.
    let target_full = git
        .resolve_commit(target)
        .ok_or_else(|| GitError::UnknownCommit { hash: target.to_string() })?;
    let head_full = git.resolve_commit("HEAD").unwrap_or_default();

    // ---- gather ----
    let state = read_state(root)?;
    let previous_head = git
        .probe(&["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    // What the working tree would be rewritten from: untracked files on these
    // paths are not safe just because git calls them untracked.
    let target_paths = target_tree_paths(&git, &target_full)?;
    let facts = TargetFacts { head: &head_full, target: &target_full, paths: &target_paths };

    // ---- guard: a refusal happens before anything is written ----
    let risks = assess(&state, &kind, ack, &facts).map_err(GitError::refused)?;

    // ---- backup first, always ----
    let backup = match kind {
        ResetKind::Branch => None,
        _ => Some(create_backup(&git)?),
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

    // The undo hint has to be safe to follow blindly, because that is what people
    // do with an undo. A stale backup branch is NOT one — following it would jump
    // back to whenever it was made, discarding everything since. So in that case
    // the way back is the pre-reset commit itself, and the stale branch is
    // mentioned as something to inspect rather than something to reset to.
    let undo = match &backup {
        Some(b) if b.is_safe_undo() => Some(format!(
            "git reset --hard {}   # restores exactly the state you were in a moment ago",
            b.branch
        )),
        Some(b) => Some(format!(
            "git reset --hard {previous_head}   # undo this reset ({} is older, from {} — inspect with `git log {}`)",
            b.branch, b.date, b.branch
        )),
        None => None,
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
        backup_branch: backup.as_ref().map(|b| b.branch.clone()),
        backup: backup.map(|b| BackupReport {
            branch: b.branch,
            commit: b.commit,
            date: b.date,
            created: b.created,
            covers_head: b.covers_head,
        }),
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

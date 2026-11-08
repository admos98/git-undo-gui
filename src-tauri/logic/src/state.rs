//! Reading the repository state the UI needs before offering to reset.
//!
//! Pure parsing lives here so it can be unit-tested without invoking git.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Files the user has modified but not committed. `--hard` destroys these, so the
/// UI has to know about them before it lets anyone click.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RepoState {
    pub branch: String,
    /// Short hashes of commits that exist only locally (not on any upstream).
    pub unpushed: Vec<String>,
    pub dirty: Vec<DirtyFile>,
    pub dirty_count: usize,
    /// Non-empty when a multi-step git operation was left unfinished.
    pub in_progress: Option<String>,
    pub stash_count: usize,
    pub head: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyFile {
    pub status: String,
    pub path: String,
}

impl RepoState {
    pub fn is_dirty(&self) -> bool {
        self.dirty_count > 0
    }
}

/// Parse `git status --porcelain=v1`.
///
/// Format is `XY <path>` where X is the index status and Y the worktree status.
/// We surface both so "staged but not committed" is distinguishable from
/// "modified and unstaged" — only the latter is lost by `reset --hard`.
pub fn parse_porcelain(output: &str) -> Vec<DirtyFile> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut chars = line.chars();
            let x = chars.next().unwrap_or(' ');
            let y = chars.next().unwrap_or(' ');
            let status = match (x, y) {
                ('?', '?') => "untracked",
                ('!', '!') => "ignored",
                (' ', 'M') => "modified",
                ('M', ' ') => "staged",
                ('M', 'M') => "staged+modified",
                (' ', 'D') => "deleted",
                ('D', ' ') => "staged deletion",
                ('A', ' ') => "added",
                ('R', _) => "renamed",
                ('C', _) => "copied",
                ('U', _) | (_, 'U') => "conflict",
                _ => "changed",
            };
            let path = line.chars().skip(3).collect::<String>();
            DirtyFile {
                status: status.to_string(),
                path: path.trim().to_string(),
            }
        })
        .collect()
}

/// Parse `git rev-list --count <upstream>..HEAD` — commits on this branch that the
/// upstream has never seen. `None` (no upstream configured) means we cannot know,
/// so we must not claim there is nothing to lose.
pub fn parse_unpushed_count(output: &str) -> Option<usize> {
    output.trim().parse::<usize>().ok()
}

/// Detect an interrupted operation from the presence of marker files/dirs.
pub fn detect_in_progress(has_marker: impl Fn(&str) -> bool) -> Option<String> {
    const MARKERS: [(&str, &str); 5] = [
        ("MERGE_HEAD", "merge"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("BISECT_LOG", "bisect"),
    ];
    MARKERS
        .iter()
        .find(|(marker, _)| has_marker(marker))
        .map(|(_, name)| (*name).to_string())
}

/// Is this entry something git is not tracking at all?
pub fn is_untracked(file: &DirtyFile) -> bool {
    matches!(file.status.as_str(), "untracked" | "ignored")
}

/// Is a dirty file something `git reset --hard` would destroy?
///
/// Untracked files are not destroyed *by the reset itself* — they have no
/// committed version to go back to. They can still be overwritten when the
/// target snapshot tracks the same path, which `guard` checks separately against
/// the target tree.
pub fn destroyed_by_hard_reset(file: &DirtyFile) -> bool {
    !is_untracked(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_parses_worktree_and_index_states() {
        let out = " M src/a.ts\nM  src/b.ts\nMM src/c.ts\n?? notes.md\n D gone.ts\n";
        let files = parse_porcelain(out);
        assert_eq!(files.len(), 5);
        assert_eq!(files[0].status, "modified");
        assert_eq!(files[0].path, "src/a.ts");
        assert_eq!(files[1].status, "staged");
        assert_eq!(files[2].status, "staged+modified");
        assert_eq!(files[3].status, "untracked");
        assert_eq!(files[4].status, "deleted");
    }

    #[test]
    fn porcelain_handles_renames_and_conflicts() {
        let out = "R  old.ts -> new.ts\nUU conflict.ts\n";
        let files = parse_porcelain(out);
        assert_eq!(files[0].status, "renamed");
        assert_eq!(files[1].status, "conflict");
    }

    #[test]
    fn untracked_files_are_not_destroyed_by_hard_reset() {
        let untracked = DirtyFile { status: "untracked".into(), path: "n.md".into() };
        let modified = DirtyFile { status: "modified".into(), path: "a.ts".into() };
        let ignored = DirtyFile { status: "ignored".into(), path: "target/".into() };
        assert!(!destroyed_by_hard_reset(&untracked));
        assert!(!destroyed_by_hard_reset(&ignored));
        assert!(destroyed_by_hard_reset(&modified));
        assert!(is_untracked(&untracked));
        assert!(!is_untracked(&modified));
    }

    #[test]
    fn unpushed_count_parses_and_rejects_nonsense() {
        assert_eq!(parse_unpushed_count("3\n"), Some(3));
        assert_eq!(parse_unpushed_count("0"), Some(0));
        assert_eq!(parse_unpushed_count("fatal: no upstream"), None);
    }

    #[test]
    fn in_progress_detection_reports_the_operation() {
        assert_eq!(detect_in_progress(|m| m == "MERGE_HEAD"), Some("merge".to_string()));
        assert_eq!(detect_in_progress(|m| m == "rebase-merge"), Some("rebase".to_string()));
        assert_eq!(detect_in_progress(|_| false), None);
    }
}

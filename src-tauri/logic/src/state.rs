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

/// Names for the two-letter porcelain status. X is the index status, Y the
/// worktree status; both are surfaced so "staged but not committed" is
/// distinguishable from "modified and unstaged" — only the latter is lost by
/// `reset --hard`.
fn classify(x: char, y: char) -> &'static str {
    match (x, y) {
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
    }
}

/// Parse NUL-separated `git status --porcelain=v1 -z`.
///
/// This is the parser to use for anything security- or data-loss-relevant.
/// The line-based format C-quotes any path containing a space, a quote or a
/// non-ASCII byte (`?? "my notes.md"`), so a naive `skip(3)` keeps the quotes
/// and the path no longer matches the same path as printed by `ls-tree`. An
/// untracked file whose name has a space in it then looked like it was *not*
/// on a path the target snapshot tracks, and `git reset --hard` overwrote it.
///
/// `-z` never quotes: fields are literal and NUL-terminated. Rename and copy
/// entries carry the original path as an extra field, which is consumed so it
/// cannot be mistaken for a separate entry.
pub fn parse_porcelain_z(output: &str) -> Vec<DirtyFile> {
    let mut files = Vec::new();
    let mut entries = output.split('\0');

    while let Some(entry) = entries.next() {
        if entry.trim().is_empty() {
            continue;
        }
        let mut chars = entry.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let status = classify(x, y);
        // Everything after the space that separates the status from the path.
        let path = entry.chars().skip(3).collect::<String>();

        // `R  new\0old\0` and `C  new\0old\0`: the origin is its own field.
        if matches!(x, 'R' | 'C') {
            entries.next();
        }

        files.push(DirtyFile { status: status.to_string(), path });
    }

    files
}

/// Parse line-based `git status --porcelain=v1`.
///
/// Only safe for paths that contain no spaces, quotes or non-ASCII bytes —
/// `-z` quoting makes that a real limitation, so repository state uses
/// [`parse_porcelain_z`] instead. Kept for callers reading text a human typed.
pub fn parse_porcelain(output: &str) -> Vec<DirtyFile> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut chars = line.chars();
            let x = chars.next().unwrap_or(' ');
            let y = chars.next().unwrap_or(' ');
            let status = classify(x, y);
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

/// The revisions to ask `rev-list` about when measuring unpushed commits.
///
/// `@{upstream}..HEAD` is the precise question but it has no answer on a branch
/// with no upstream — which is exactly a fresh local branch, the case where a
/// hard reset destroys the only copy of the work. `HEAD --not --remotes` asks a
/// slightly different question ("commits on no remote-tracking ref") that is
/// always answerable, and the two agree whenever an upstream exists.
///
/// Verified against git: no remotes → 2; remote added, nothing pushed → 2;
/// after `git push` → 0; one local commit after a push → 1, same as the
/// upstream form.
pub fn unpushed_rev_list_args() -> [&'static str; 5] {
    ["rev-list", "--count", "HEAD", "--not", "--remotes"]
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

    #[test]
    fn porcelain_z_keeps_paths_with_spaces_and_quotes_literal() {
        // The line-based format C-quotes these; `-z` never does. This is the
        // difference that let `git reset --hard` overwrite untracked work.
        let out = "?? my notes.md\0?? quo\"te.md\0?? caf\u{e9}.md\0 M src/a.ts\0";
        let files = parse_porcelain_z(out);
        assert_eq!(files.len(), 4, "{files:?}");
        assert_eq!(files[0].path, "my notes.md");
        assert_eq!(files[0].status, "untracked");
        assert_eq!(files[1].path, "quo\"te.md");
        assert_eq!(files[2].path, "caf\u{e9}.md");
        assert_eq!(files[3].status, "modified");
        assert_eq!(files[3].path, "src/a.ts");
    }

    #[test]
    fn porcelain_z_consumes_the_rename_origin_field() {
        // `R  new\0old\0` — the old path is its own field, not a second entry.
        let files = parse_porcelain_z("R  d e.txt\0b c.txt\0?? after.md\0");
        assert_eq!(files.len(), 2, "the origin must not become an entry: {files:?}");
        assert_eq!(files[0].status, "renamed");
        assert_eq!(files[0].path, "d e.txt");
        assert_eq!(files[1].path, "after.md");
    }

    #[test]
    fn porcelain_z_tolerates_empty_and_trailing_nuls() {
        assert!(parse_porcelain_z("").is_empty());
        assert!(parse_porcelain_z("\0\0").is_empty());
        assert_eq!(parse_porcelain_z("?? a.md\0").len(), 1);
    }

    #[test]
    fn the_line_based_parser_still_works_for_unquoted_input() {
        let out = " M src/a.ts\nM  src/b.ts\nMM src/c.ts\n?? notes.md\n D gone.ts\n";
        let files = parse_porcelain(out);
        assert_eq!(files.len(), 5);
        assert_eq!(files[3].path, "notes.md");
    }

    #[test]
    fn unpushed_counting_does_not_need_an_upstream() {
        // `@{upstream}..HEAD` answers `fatal: no upstream configured` on a fresh
        // local branch, which is exactly the case where a hard reset destroys the
        // only copy of the work. The args chosen here are always answerable.
        let args = unpushed_rev_list_args();
        assert_eq!(args, ["rev-list", "--count", "HEAD", "--not", "--remotes"]);
        assert!(
            !args.iter().any(|a| a.contains("upstream")),
            "counting must not depend on a configured upstream"
        );
    }
}

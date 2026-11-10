//! The rules that decide whether a reset is allowed.
//!
//! This module is intentionally free of git invocations so the policy can be read,
//! reviewed and unit-tested on its own. `service.rs` gathers facts, this decides.

use crate::paths::overlaps_tree;
use crate::state::{destroyed_by_hard_reset, is_untracked, DirtyFile, RepoState};
#[cfg(feature = "serde")]
use serde::Serialize;

/// `reset --hard` destroys uncommitted work, so it needs a stronger confirmation
/// than the safe strategies.
pub const ACK_PHRASE: &str = "RESET";

#[derive(Debug, Clone, PartialEq)]
pub enum ResetKind {
    /// Create a branch at the snapshot; HEAD never moves. Never blocked.
    Branch,
    /// Move HEAD, keep the working tree. Nothing is discarded.
    Soft,
    /// Rewrite file contents from the snapshot, history untouched.
    Files,
    /// Move HEAD and the working tree. Destructive.
    Hard,
}

impl ResetKind {
    pub fn parse(value: &str) -> Option<ResetKind> {
        match value {
            "branch" => Some(ResetKind::Branch),
            "soft" => Some(ResetKind::Soft),
            "files" => Some(ResetKind::Files),
            "hard" => Some(ResetKind::Hard),
            _ => None,
        }
    }

    pub fn is_destructive(&self) -> bool {
        matches!(self, ResetKind::Hard)
    }
}

/// The target-side facts a pure guard cannot read for itself.
///
/// `head` and `target` are **full** commit ids, already resolved by the caller.
/// Comparing short hashes was how "reset to the current HEAD" slipped past the
/// no-op refusal: `a43c821` and its 40-char id are the same commit and were not
/// equal as strings. `paths` is every path the target snapshot tracks, which is
/// exactly what the working tree will be rewritten from.
#[derive(Debug, Clone, Copy)]
pub struct TargetFacts<'a> {
    pub head: &'a str,
    pub target: &'a str,
    pub paths: &'a [String],
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Risk {
    pub level: &'static str,
    pub message: String,
}

/// Evaluate the guard. `Ok(risks)` means "allowed, but here is what you should
/// know"; `Err(reason)` means "this is refused, and here is why".
///
/// Order matters: hard blocks come before soft warnings, and every refusal is
/// specific enough for the UI to show next to the button that triggered it.
pub fn assess(
    state: &RepoState,
    kind: &ResetKind,
    ack: Option<&str>,
    facts: &TargetFacts<'_>,
) -> Result<Vec<Risk>, String> {
    let mut risks = Vec::new();

    // 1. Never reset into the middle of an unfinished operation.
    if let Some(operation) = &state.in_progress {
        return Err(format!(
            "A {operation} is in progress. Run `git {operation} --abort` (or finish it) first — \
             resetting now would make the conflict state worse, not better."
        ));
    }

    // 2. Resetting to where you already are is a no-op; say so instead of doing it.
    //    Both ids are full commit ids, so a short hash, a full hash and HEAD all
    //    compare equal when they are the same commit.
    //
    //    This applies to every strategy that would otherwise move something. The
    //    rescue branch is exempt because `git branch rescue-<hash>` at HEAD is a
    //    useful thing to ask for, not a no-op. `files` is exempt while the tree is
    //    dirty, because restoring HEAD over local edits does change something —
    //    that intent is judged by the discard rules below.
    let nothing_to_do = match kind {
        ResetKind::Branch => false,
        ResetKind::Hard | ResetKind::Soft => true,
        ResetKind::Files => !state.is_dirty(),
    };
    if nothing_to_do && !facts.head.is_empty() && facts.head == facts.target {
        return Err(format!(
            "{} is already at {} — there is nothing to change.",
            if matches!(kind, ResetKind::Files) { "Your working tree" } else { "HEAD" },
            short_id(facts.target)
        ));
    }

    // 3. Destructive work needs an explicit acknowledgement, not just a click.
    if kind.is_destructive() {
        let acknowledged = ack.map(|a| a.trim().eq_ignore_ascii_case(ACK_PHRASE)).unwrap_or(false);
        if !acknowledged {
            return Err(format!(
                "Hard reset requires typing {ACK_PHRASE} to confirm. Use the rescue-branch strategy \
                 if you are not certain."
            ));
        }
    }

    // 4. Losing unpushed commits is the scenario this whole app exists for. It is not
    //    a refusal — the backup branch created just before the reset holds them, so
    //    they stay recoverable — but the user has to be told what happened to them.
    if kind.is_destructive() && !state.unpushed.is_empty() {
        risks.push(Risk {
            level: "danger",
            message: format!(
                "{} unpushed commit(s) will no longer be on a branch. The recovery branch will still \
                 contain them — that is your undo for this undo.",
                state.unpushed.len()
            ),
        });
    }

    // 5. Uncommitted work in tracked files is irrecoverable after --hard (git has no
    //    record of it, and a backup branch only holds commits).
    let destroyed: Vec<&str> = state
        .dirty
        .iter()
        .filter(|f| destroyed_by_hard_reset(f))
        .map(|f| f.path.as_str())
        .collect();
    if kind.is_destructive() && !destroyed.is_empty() {
        return Err(format!(
            "{} file(s) have uncommitted changes that no backup can restore ({}). Commit them, \
             stash them (`git stash -u`), or choose the rescue-branch strategy.",
            destroyed.len(),
            list(&destroyed)
        ));
    }

    // 6. Untracked files usually survive a reset — but not when the snapshot tracks
    //    the same path. git replaces whatever is on disk there, including deleting
    //    an untracked file to create a directory (or the other way round). Nothing
    //    about that is included in the backup branch, so it is a refusal.
    let overwritten: Vec<&str> = state
        .dirty
        .iter()
        .filter(|f| is_untracked(f) && overlaps_tree(&f.path, facts.paths))
        .map(|f| f.path.as_str())
        .collect();
    if kind.is_destructive() && !overwritten.is_empty() {
        return Err(format!(
            "{} untracked file(s) sit at paths that snapshot {} tracks ({}). `git reset --hard` \
             replaces what is on disk there — including a file where the snapshot has a directory, \
             or a directory where it has a file — and no backup branch can bring untracked work \
             back. Move or commit them first, or choose the rescue-branch strategy.",
            overwritten.len(),
            short_id(facts.target),
            list(&overwritten)
        ));
    }

    // 6a. Untracked files that the snapshot does not touch really do survive. Say it
    //     plainly rather than leaving it to be discovered.
    if kind.is_destructive() {
        let survivors: Vec<&str> = state
            .dirty
            .iter()
            .filter(|f| !destroyed_by_hard_reset(f))
            .filter(|f| !overlaps_tree(&f.path, facts.paths))
            .map(|f| f.path.as_str())
            .collect();
        if !survivors.is_empty() {
            risks.push(Risk {
                level: "info",
                message: format!(
                    "{} untracked file(s) such as {} are not touched by this reset and stay on disk.",
                    survivors.len(),
                    survivors.first().copied().unwrap_or("")
                ),
            });
        }
    }

    // 7. `git restore --source` rewrites every path the snapshot tracks and removes
    //    tracked paths it does not. It does that to the *working tree*, so any
    //    uncommitted edit on those paths is erased — and a backup branch cannot
    //    hold uncommitted edits. Refuse until they are committed or stashed.
    //    This is the whole point of keeping the restore feature: it is useful, so
    //    it must not lie about what it does.
    if matches!(kind, ResetKind::Files) {
        let erased: Vec<&str> = state
            .dirty
            .iter()
            .filter(|f| files_restore_would_touch(f, facts.paths))
            .map(|f| f.path.as_str())
            .collect();
        if !erased.is_empty() {
            return Err(format!(
                "{} uncommitted change(s) are on paths that `git restore --source={}` rewrites \
                 ({}). Restoring file contents erases those edits, and a backup branch cannot \
                 restore uncommitted work. Commit them (`git commit -am \"wip\"`), stash them \
                 (`git stash -u`), or choose the rescue-branch strategy.",
                erased.len(),
                short_id(facts.target),
                list(&erased)
            ));
        }
    }

    // 8. Soft only moves HEAD: the index and the working tree are left exactly as
    //    they are, so the changes survive — say what happens to them.
    if matches!(kind, ResetKind::Soft) && state.is_dirty() {
        risks.push(Risk {
            level: "warn",
            message: format!(
                "{} uncommitted change(s) stay in your working tree: `--soft` moves HEAD and leaves \
                 the files alone. Check `git status` afterwards.",
                state.dirty_count
            ),
        });
    }

    // 8a. Files: anything still dirty is outside the set of paths the restore
    //     rewrites, so it is genuinely untouched (the destructive cases were
    //     refused above).
    if matches!(kind, ResetKind::Files) && state.is_dirty() {
        risks.push(Risk {
            level: "info",
            message: format!(
                "{} uncommitted file(s) are outside the paths this restore rewrites and are left \
                 alone.",
                state.dirty_count
            ),
        });
    }

    // 9. Stashes are easy to forget and hard to find later.
    if state.stash_count > 0 {
        risks.push(Risk {
            level: "info",
            message: format!(
                "{} stash entr(y/ies) exist independently of this reset and are unaffected.",
                state.stash_count
            ),
        });
    }

    Ok(risks)
}

/// Would `git restore --source=<snapshot>` touch the content of this dirty entry?
///
/// Two ways: the path is in the snapshot's tree (restore writes it — the
/// file/directory collisions included, and a local deletion is undone by
/// resurrecting the file), or it is tracked but absent from the snapshot
/// (restore removes it from the working tree, taking local edits with it).
/// A deleted path the snapshot does not have is already gone from disk, so
/// there is nothing left for restore to take.
fn files_restore_would_touch(file: &DirtyFile, tree_paths: &[String]) -> bool {
    if overlaps_tree(&file.path, tree_paths) {
        return true;
    }
    if is_untracked(file) {
        return false;
    }
    !matches!(file.status.as_str(), "deleted" | "staged deletion")
}

fn short_id(id: &str) -> &str {
    &id[..7.min(id.len())]
}

/// Name at most three paths, so a refusal stays readable in a dialog.
fn list(paths: &[&str]) -> String {
    let mut shown: Vec<&str> = paths.iter().take(3).copied().collect();
    if paths.len() > shown.len() {
        shown.push("…");
    }
    shown.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DirtyFile;

    const HEAD: &str = "f82a91c0000000000000000000000000000beef";
    const TARGET: &str = "a43c82111111111111111111111111111111cafe";

    fn clean() -> RepoState {
        RepoState {
            branch: "main".into(),
            unpushed: vec![],
            dirty: vec![],
            dirty_count: 0,
            in_progress: None,
            stash_count: 0,
            head: "f82a91c".into(),
        }
    }

    fn dirty() -> RepoState {
        RepoState {
            dirty: vec![DirtyFile { status: "modified".into(), path: "src/a.ts".into() }],
            dirty_count: 1,
            ..clean()
        }
    }

    /// Facts for a reset whose snapshot tracks nothing (the pre-fix situation).
    fn bare<'a>(head: &'a str, target: &'a str, paths: &'a [String]) -> TargetFacts<'a> {
        TargetFacts { head, target, paths }
    }

    fn tree(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_string()).collect()
    }

    fn dirty_file(status: &str, path: &str) -> DirtyFile {
        DirtyFile { status: status.to_string(), path: path.to_string() }
    }

    fn untracked_state(paths: &[&str]) -> RepoState {
        RepoState {
            dirty: paths.iter().map(|p| dirty_file("untracked", p)).collect(),
            dirty_count: paths.len(),
            ..clean()
        }
    }

    #[test]
    fn rescue_branch_is_never_blocked() {
        let mut state = dirty();
        state.unpushed = vec!["a".into(), "b".into()];
        let no_paths: Vec<String> = vec![];
        assert!(assess(&state, &ResetKind::Branch, None, &bare(HEAD, TARGET, &no_paths)).is_ok());
    }

    #[test]
    fn hard_reset_requires_the_ack_phrase() {
        let no_paths: Vec<String> = vec![];
        let facts = bare(HEAD, TARGET, &no_paths);
        let err = assess(&clean(), &ResetKind::Hard, None, &facts).unwrap_err();
        assert!(err.contains("RESET"), "{err}");
        let err = assess(&clean(), &ResetKind::Hard, Some("undo"), &facts).unwrap_err();
        assert!(err.contains("RESET"), "{err}");
        assert!(assess(&clean(), &ResetKind::Hard, Some("reset"), &facts).is_ok());
    }

    #[test]
    fn hard_reset_refuses_to_discard_uncommitted_work() {
        let no_paths: Vec<String> = vec![];
        let err = assess(&dirty(), &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &no_paths))
            .unwrap_err();
        assert!(err.contains("src/a.ts"), "{err}");
        assert!(err.contains("stash"), "{err}");
    }

    #[test]
    fn hard_reset_refuses_untracked_files_the_target_tracks() {
        // The data-loss bug: the path is untracked *now*, but the target commit has
        // it, so `git reset --hard` writes over it without asking.
        let state = untracked_state(&["notes.md"]);
        let paths = tree(&["notes.md", "src/a.ts"]);
        let err = assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &paths))
            .unwrap_err();
        assert!(err.contains("notes.md"), "{err}");
        assert!(err.contains("untracked"), "{err}");
        assert!(err.contains("backup"), "the refusal must say why a backup does not help: {err}");
    }

    #[test]
    fn hard_reset_refuses_file_and_directory_collisions() {
        // Untracked file `docs` where the snapshot has `docs/guide.md`…
        let state = untracked_state(&["docs"]);
        let paths = tree(&["docs/guide.md"]);
        let err = assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &paths));
        assert!(err.unwrap_err().contains("docs"));

        // …and an untracked directory `notes.txt/` where the snapshot has a file.
        let state = untracked_state(&["notes.txt/"]);
        let paths = tree(&["notes.txt"]);
        let err = assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &paths));
        assert!(err.unwrap_err().contains("notes.txt"));
    }

    #[test]
    fn untracked_files_the_target_does_not_track_do_not_block_a_hard_reset() {
        let state = untracked_state(&["notes.md"]);
        let paths = tree(&["src/a.ts"]);
        let risks =
            assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &paths)).unwrap();
        // Refusing would be wrong (nothing is lost), staying silent would be worse.
        let note = risks.iter().find(|r| r.level == "info").expect("a survivor note");
        assert!(note.message.contains("notes.md"), "{}", note.message);
        assert!(note.message.contains("stay on disk"), "{}", note.message);
    }

    #[test]
    fn survivor_notes_never_list_files_that_are_actually_lost() {
        // A hard reset refuses on destroyed files, so this only proves the note
        // itself is filtered correctly when both kinds are present at once.
        let state = RepoState {
            dirty: vec![
                DirtyFile { status: "untracked".into(), path: "notes.md".into() },
                DirtyFile { status: "M ".into(), path: "src/a.ts".into() },
            ],
            dirty_count: 2,
            ..clean()
        };
        let no_paths: Vec<String> = vec![];
        let err = assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &no_paths))
            .unwrap_err();
        assert!(err.contains("src/a.ts"), "{err}");
        assert!(!err.contains("notes.md"), "{err}");
    }

    #[test]
    fn a_refusal_lists_at_most_three_paths() {
        let state = untracked_state(&["a.md", "b.md", "c.md", "d.md"]);
        let paths = tree(&["a.md", "b.md", "c.md", "d.md"]);
        let err = assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &paths))
            .unwrap_err();
        assert!(err.contains("a.md") && err.contains("c.md"), "{err}");
        assert!(!err.contains("d.md"), "more than three paths in one line: {err}");
        assert!(err.contains('…'), "{err}");
    }

    #[test]
    fn restore_files_refuses_to_erase_uncommitted_edits() {
        // `git restore --source` overwrites the working tree; the old warning
        // claimed the strategy "keeps" these changes. It does not.
        let state = dirty();
        let paths = tree(&["src/a.ts"]);
        let err =
            assess(&state, &ResetKind::Files, None, &bare(HEAD, TARGET, &paths)).unwrap_err();
        assert!(err.contains("src/a.ts"), "{err}");
        assert!(err.contains("stash"), "{err}");
        assert!(err.contains("backup"), "{err}");
    }

    #[test]
    fn restore_files_refuses_untracked_collisions() {
        let state = untracked_state(&["added.ts"]);
        let paths = tree(&["added.ts"]);
        let err =
            assess(&state, &ResetKind::Files, None, &bare(HEAD, TARGET, &paths)).unwrap_err();
        assert!(err.contains("added.ts"), "{err}");
    }

    #[test]
    fn restore_files_refuses_a_tracked_path_the_snapshot_would_remove() {
        // `git restore --source=<old>` deletes tracked paths the snapshot does not
        // have. A local edit on such a path disappears with the file.
        let state = RepoState {
            dirty: vec![DirtyFile { status: "modified".into(), path: "src/newer.ts".into() }],
            dirty_count: 1,
            ..clean()
        };
        let paths = tree(&["src/a.ts"]);
        let err =
            assess(&state, &ResetKind::Files, None, &bare(HEAD, TARGET, &paths)).unwrap_err();
        assert!(err.contains("src/newer.ts"), "{err}");
    }

    #[test]
    fn restore_files_leaves_unrelated_untracked_files_alone() {
        // A clean tree plus an unrelated untracked note: nothing to erase, so the
        // strategy stays available and the note is mentioned.
        let state = RepoState {
            dirty: vec![DirtyFile { status: "untracked".into(), path: "scratch.md".into() }],
            dirty_count: 1,
            ..clean()
        };
        let paths = tree(&["src/a.ts"]);
        let risks =
            assess(&state, &ResetKind::Files, None, &bare(HEAD, TARGET, &paths)).unwrap();
        let note = risks.iter().find(|r| r.level == "info").expect("an info note");
        assert!(note.message.contains("left alone"), "{}", note.message);
        assert!(
            !risks.iter().any(|r| r.message.contains("keeps")),
            "the guard must not claim restore keeps uncommitted work: {risks:?}"
        );
    }

    #[test]
    fn restore_files_on_a_clean_tree_is_allowed_without_noise() {
        let no_paths: Vec<String> = vec![];
        let risks =
            assess(&clean(), &ResetKind::Files, None, &bare(HEAD, TARGET, &no_paths)).unwrap();
        assert!(risks.is_empty(), "{risks:?}");
    }

    #[test]
    fn soft_only_warns_and_says_the_changes_stay() {
        let no_paths: Vec<String> = vec![];
        let risks = assess(&dirty(), &ResetKind::Soft, None, &bare(HEAD, TARGET, &no_paths)).unwrap();
        assert_eq!(risks[0].level, "warn");
        assert!(risks[0].message.contains("stay in your working tree"), "{}", risks[0].message);
    }

    #[test]
    fn resets_are_blocked_mid_merge() {
        let state = RepoState { in_progress: Some("merge".into()), ..clean() };
        let no_paths: Vec<String> = vec![];
        let err = assess(&state, &ResetKind::Soft, None, &bare(HEAD, TARGET, &no_paths)).unwrap_err();
        assert!(err.contains("merge"), "{err}");
        assert!(err.contains("--abort"), "{err}");
    }

    #[test]
    fn no_op_resets_are_refused_even_when_the_ids_are_full() {
        // The bug: HEAD was compared as a 7-char short hash, so pasting the full
        // hash of the current commit looked like "somewhere else".
        let full = "a43c821aa11bb22cc33dd44ee55ff66aa77bb88";
        let no_paths: Vec<String> = vec![];
        let err =
            assess(&clean(), &ResetKind::Hard, Some("RESET"), &bare(full, full, &no_paths))
                .unwrap_err();
        assert!(err.contains("already at"), "{err}");
        assert!(err.contains("a43c821"), "{err}");
    }

    #[test]
    fn no_op_soft_resets_are_refused_too() {
        // Soft used to slip through and create a backup branch for a reset that
        // could not change anything.
        let full = "a43c821aa11bb22cc33dd44ee55ff66aa77bb88";
        let no_paths: Vec<String> = vec![];
        let err = assess(&clean(), &ResetKind::Soft, None, &bare(full, full, &no_paths))
            .unwrap_err();
        assert!(err.contains("already at"), "{err}");
    }

    #[test]
    fn a_rescue_branch_at_head_is_still_allowed() {
        // `git branch rescue-<hash>` at HEAD is a legitimate thing to ask for.
        let full = "a43c821aa11bb22cc33dd44ee55ff66aa77bb88";
        let no_paths: Vec<String> = vec![];
        assert!(assess(&clean(), &ResetKind::Branch, None, &bare(full, full, &no_paths)).is_ok());
    }

    #[test]
    fn restoring_head_onto_local_edits_is_not_treated_as_a_no_op() {
        // The tree is dirty, so `git restore --source=HEAD -- .` really would erase
        // something. The refusal must be about the edits, not about a no-op.
        let full = "a43c821aa11bb22cc33dd44ee55ff66aa77bb88";
        let paths = tree(&["src/a.ts"]);
        let err = assess(&dirty(), &ResetKind::Files, None, &bare(full, full, &paths))
            .unwrap_err();
        assert!(!err.contains("nothing to change"), "wrong refusal: {err}");
        assert!(err.contains("src/a.ts"), "{err}");
        assert!(err.contains("uncommitted"), "{err}");
    }

    #[test]
    fn restoring_head_onto_a_clean_tree_is_refused_as_a_no_op() {
        let full = "a43c821aa11bb22cc33dd44ee55ff66aa77bb88";
        let no_paths: Vec<String> = vec![];
        let err = assess(&clean(), &ResetKind::Files, None, &bare(full, full, &no_paths))
            .unwrap_err();
        assert!(err.contains("nothing to change"), "{err}");
    }

    #[test]
    fn no_op_detection_tolerates_an_unborn_head() {
        let no_paths: Vec<String> = vec![];
        assert!(
            assess(&clean(), &ResetKind::Hard, Some("RESET"), &bare("", TARGET, &no_paths)).is_ok()
        );
    }

    #[test]
    fn unpushed_commits_produce_a_danger_note() {
        let state = RepoState { unpushed: vec!["a".into(), "b".into(), "c".into()], ..clean() };
        let no_paths: Vec<String> = vec![];
        let risks =
            assess(&state, &ResetKind::Hard, Some("RESET"), &bare(HEAD, TARGET, &no_paths)).unwrap();
        assert!(risks.iter().any(|r| r.level == "danger" && r.message.contains('3')));
    }

    #[test]
    fn stashes_are_mentioned_once() {
        let state = RepoState { stash_count: 2, ..clean() };
        let no_paths: Vec<String> = vec![];
        let risks =
            assess(&state, &ResetKind::Branch, None, &bare(HEAD, TARGET, &no_paths)).unwrap();
        assert_eq!(risks.len(), 1);
        assert_eq!(risks[0].level, "info");
    }

    #[test]
    fn kind_parsing_is_strict() {
        assert_eq!(ResetKind::parse("hard"), Some(ResetKind::Hard));
        assert_eq!(ResetKind::parse("HARD"), None);
        assert_eq!(ResetKind::parse("--hard"), None);
    }
}

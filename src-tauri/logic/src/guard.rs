//! The rules that decide whether a reset is allowed.
//!
//! This module is intentionally free of git invocations so the policy can be read,
//! reviewed and unit-tested on its own. `service.rs` gathers facts, this decides.

use crate::state::{destroyed_by_hard_reset, RepoState};
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
    start_point: &str,
    target: &str,
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
    if start_point == target && kind.is_destructive() {
        return Err(format!("HEAD is already at {target} — there is nothing to move."));
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

    // 4. Losing unpushed commits is the scenario this whole app exists for, so it is
    //    a refusal on --hard unless the user was told and acknowledged.
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

    // 5. Uncommitted work is irrecoverable after --hard (git has no record of it).
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
            destroyed.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    // 6a. A destructive reset does not delete untracked files, but it is the one
    //     thing people are most surprised by afterwards ("did I just lose my
    //     notes?"). Say it plainly rather than leaving it to be discovered.
    if kind.is_destructive() {
        let survivors: Vec<&str> = state
            .dirty
            .iter()
            .filter(|f| !destroyed_by_hard_reset(f))
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

    // 6b. Soft/Files still deserve a heads-up about a dirty tree.
    if !kind.is_destructive() && state.is_dirty() {
        risks.push(Risk {
            level: "warn",
            message: format!(
                "{} uncommitted change(s) are present. This strategy keeps them, but check \
                 `git status` afterwards.",
                state.dirty_count
            ),
        });
    }

    // 7. Stashes are easy to forget and hard to find later.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DirtyFile;

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

    #[test]
    fn rescue_branch_is_never_blocked() {
        let mut state = dirty();
        state.unpushed = vec!["a".into(), "b".into()];
        assert!(assess(&state, &ResetKind::Branch, None, "f82a91c", "a43c821").is_ok());
    }

    #[test]
    fn hard_reset_requires_the_ack_phrase() {
        let err = assess(&clean(), &ResetKind::Hard, None, "f82a91c", "a43c821").unwrap_err();
        assert!(err.contains("RESET"), "{err}");
        let err = assess(&clean(), &ResetKind::Hard, Some("undo"), "f82a91c", "a43c821").unwrap_err();
        assert!(err.contains("RESET"), "{err}");
        assert!(assess(&clean(), &ResetKind::Hard, Some("reset"), "f82a91c", "a43c821").is_ok());
    }

    #[test]
    fn hard_reset_refuses_to_discard_uncommitted_work() {
        let err = assess(&dirty(), &ResetKind::Hard, Some("RESET"), "f82a91c", "a43c821").unwrap_err();
        assert!(err.contains("src/a.ts"), "{err}");
        assert!(err.contains("stash"), "{err}");
    }

    #[test]
    fn untracked_files_do_not_block_a_hard_reset() {
        let state = RepoState {
            dirty: vec![DirtyFile { status: "untracked".into(), path: "notes.md".into() }],
            dirty_count: 1,
            ..clean()
        };
        let risks = assess(&state, &ResetKind::Hard, Some("RESET"), "f82a91c", "a43c821").unwrap();
        // Refusing would be wrong (nothing is lost), staying silent would be worse.
        let note = risks.iter().find(|r| r.level == "info").expect("a survivor note");
        assert!(note.message.contains("notes.md"), "{}", note.message);
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
        let err = assess(&state, &ResetKind::Hard, Some("RESET"), "f82a91c", "a43c821").unwrap_err();
        assert!(err.contains("src/a.ts"), "{err}");
        assert!(!err.contains("notes.md"), "{err}");
    }

    #[test]
    fn soft_and_files_only_warn_about_a_dirty_tree() {
        let risks = assess(&dirty(), &ResetKind::Soft, None, "f82a91c", "a43c821").unwrap();
        assert_eq!(risks[0].level, "warn");
    }

    #[test]
    fn resets_are_blocked_mid_merge() {
        let state = RepoState { in_progress: Some("merge".into()), ..clean() };
        let err = assess(&state, &ResetKind::Soft, None, "f82a91c", "a43c821").unwrap_err();
        assert!(err.contains("merge"), "{err}");
        assert!(err.contains("--abort"), "{err}");
    }

    #[test]
    fn no_op_resets_are_refused() {
        let err = assess(&clean(), &ResetKind::Hard, Some("RESET"), "a43c821", "a43c821").unwrap_err();
        assert!(err.contains("already at"), "{err}");
    }

    #[test]
    fn unpushed_commits_produce_a_danger_note() {
        let state = RepoState { unpushed: vec!["a".into(), "b".into(), "c".into()], ..clean() };
        let risks = assess(&state, &ResetKind::Hard, Some("RESET"), "f82a91c", "a43c821").unwrap();
        assert!(risks.iter().any(|r| r.level == "danger" && r.message.contains('3')));
    }

    #[test]
    fn stashes_are_mentioned_once() {
        let state = RepoState { stash_count: 2, ..clean() };
        let risks = assess(&state, &ResetKind::Branch, None, "f82a91c", "a43c821").unwrap();
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

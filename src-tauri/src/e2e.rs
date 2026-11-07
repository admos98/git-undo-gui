//! End-to-end tests of the destructive path, against real repositories.
//!
//! The unit tests in `logic/` check the guard's decisions; these check that the
//! decisions are *wired up*: that a refusal really leaves the repository alone,
//! that a backup branch exists before the reset that needs it, and that each
//! strategy does what its button promises.
//!
//! They need a `git` binary and nothing else — no window, no Tauri runtime — so
//! they run in CI on every push. If `git` is absent they skip instead of failing,
//! because a machine without git cannot run the app either.

use crate::service;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A throwaway repository that deletes itself.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Option<Scratch> {
        if Command::new("git").arg("--version").output().is_err() {
            eprintln!("skipping: git is not installed");
            return None;
        }

        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "git-undo-e2e-{}-{}-{}",
            std::process::id(),
            n,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&path).ok()?;

        let scratch = Scratch { path };
        scratch.git(&["init", "--quiet", "--initial-branch=main"]);
        // A repository with no identity refuses to commit.
        scratch.configure();
        Some(scratch)
    }

    fn configure(&self) {
        for (key, value) in [
            ("user.name", "Undo Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            self.git(&["config", key, value]);
        }
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .env("GIT_AUTHOR_NAME", "Undo Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Undo Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.path)
            .output()
            .unwrap_or_else(|e| panic!("git {args:?} could not start: {e}"));
        assert!(
            out.status.success() || args.contains(&"--verify") || args.contains(&"--quiet"),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(&self, name: &str, body: &str) {
        std::fs::write(self.path.join(name), body).expect("write file");
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "--quiet", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    fn short_head(&self) -> String {
        let full = self.head();
        full[..7.min(full.len())].to_string()
    }

    fn has_branch(&self, name: &str) -> bool {
        let out = Command::new("git")
            .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{name}")])
            .current_dir(&self.path)
            .output()
            .expect("git");
        out.status.success()
    }

    fn contents(&self, name: &str) -> String {
        std::fs::read_to_string(self.path.join(name)).unwrap_or_default()
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: a leaked temp dir is untidy, not a test failure.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Two commits, `first` then `second`, returning their full hashes.
fn two_commits(scratch: &Scratch) -> (String, String) {
    scratch.write("notes.md", "first\n");
    let first = scratch.commit("first");
    scratch.write("notes.md", "second\n");
    let second = scratch.commit("second");
    (first, second)
}

#[test]
fn soft_reset_moves_head_and_keeps_the_work() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);

    let outcome = service::reset(scratch.path(), &first, "soft", None).expect("soft reset");

    assert_eq!(scratch.head(), first, "HEAD should be back at the first commit");
    assert_eq!(
        scratch.contents("notes.md"),
        "second\n",
        "a soft reset must not touch the working tree"
    );
    assert!(outcome.report.undo.is_some(), "every mutating reset reports a way back");
    assert!(
        scratch.has_branch("backup-before-recovery"),
        "the backup branch is taken before the reset"
    );
}

#[test]
fn branch_strategy_never_moves_head() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    let outcome = service::reset(scratch.path(), &first, "branch", None).expect("rescue branch");

    assert_eq!(scratch.head(), second, "the branch strategy must not move HEAD");
    assert!(scratch.has_branch(&format!("rescue-{first}")));
    assert!(
        !scratch.has_branch("backup-before-recovery"),
        "moving nothing needs no backup"
    );
    assert_eq!(outcome.backup_branch, None);
}

#[test]
fn hard_reset_refuses_uncommitted_work_and_changes_nothing() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    // Uncommitted edit to a tracked file: git cannot get this back.
    scratch.write("notes.md", "work in progress\n");

    let err = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect_err("must refuse to discard uncommitted work");

    assert!(matches!(err, crate::GitError::Refused { .. }), "{err:?}");
    assert!(err.to_string().contains("notes.md"), "{err}");
    assert_eq!(scratch.head(), second, "a refusal must not move HEAD");
    assert_eq!(scratch.contents("notes.md"), "work in progress\n");
    assert!(
        !scratch.has_branch("backup-before-recovery"),
        "a refusal must not even create the backup branch"
    );
}

#[test]
fn hard_reset_without_the_phrase_is_refused() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    for ack in [None, Some("yes"), Some("")] {
        let err = service::reset(scratch.path(), &first, "hard", ack)
            .expect_err("must demand the acknowledgement phrase");
        assert!(matches!(err, crate::GitError::Refused { .. }), "{err:?}");
        assert_eq!(scratch.head(), second);
    }
}

#[test]
fn hard_reset_moves_everything_and_the_backup_points_at_the_old_head() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    let outcome = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect("acknowledged hard reset");

    assert_eq!(scratch.head(), first);
    assert_eq!(scratch.contents("notes.md"), "first\n", "the tree follows HEAD");

    // The safety net must hold the state from *before* the reset.
    let backup = scratch.git(&["rev-parse", "backup-before-recovery"]);
    assert_eq!(backup, second, "backup-before-recovery must be the pre-reset HEAD");
    assert_eq!(outcome.backup_branch.as_deref(), Some("backup-before-recovery"));
    let undo = outcome.report.undo.clone().expect("an undo command");
    assert!(undo.contains("backup-before-recovery"), "{undo}");
}

#[test]
fn the_backup_is_idempotent_and_never_silently_overwritten() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    service::reset(scratch.path(), &first, "hard", Some("RESET")).expect("first reset");
    assert_eq!(scratch.git(&["rev-parse", "backup-before-recovery"]), second);

    // Move forward again, then reset a second time. The first backup must survive.
    scratch.write("notes.md", "third\n");
    scratch.commit("third");
    service::reset(scratch.path(), &second, "hard", Some("RESET")).expect("second reset");

    assert_eq!(
        scratch.git(&["rev-parse", "backup-before-recovery"]),
        second,
        "a second reset must not overwrite the original backup"
    );
}

#[test]
fn a_stale_backup_is_reported_and_never_offered_as_the_undo() {
    // The trap: `backup-before-recovery` exists from an earlier session. Following
    // it would rewind *past* today, so it must not be presented as the way back.
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    // An old backup pointing at the first commit…
    scratch.git(&["branch", "backup-before-recovery", &first]);
    // …then real work on top, which the reset will move away from.
    scratch.write("notes.md", "third\n");
    let third = scratch.commit("third");
    assert_ne!(first, second);
    assert_ne!(third, second);

    let outcome = service::reset(scratch.path(), &second, "hard", Some("RESET"))
        .expect("hard reset with an old backup present");

    let backup = outcome.backup.as_ref().expect("backup reported");
    assert!(!backup.created, "the existing branch is reused, not replaced");
    assert!(!backup.covers_head, "an older backup is not a safe undo for this reset");
    assert_eq!(backup.commit, first, "the stale branch was not moved");

    let undo = outcome.report.undo.expect("an undo hint");
    // The hint uses the short hash, so compare against the short form.
    assert!(
        undo.contains(&third[..7]),
        "the undo must point at the pre-reset commit: {undo}"
    );
    // The important half: it must not tell anyone to reset to the stale branch.
    assert!(
        !undo.contains("reset --hard backup-before-recovery"),
        "the stale branch must never be the undo target: {undo}"
    );
    // And following the hint must actually work.
    assert_eq!(scratch.head(), second);
    scratch.git(&["reset", "--hard", &third]);
    assert_eq!(scratch.contents("notes.md"), "third\n", "the offered undo restores the work");
}

#[test]
fn a_backup_that_already_holds_head_is_safe_and_reported_as_such() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    // Same commit, so the existing branch really is the way back.
    scratch.git(&["branch", "backup-before-recovery", &second]);
    let outcome = service::reset(scratch.path(), &first, "hard", Some("RESET")).expect("reset");

    let backup = outcome.backup.as_ref().expect("backup reported");
    assert!(!backup.created);
    assert!(backup.covers_head, "a backup at HEAD is a valid undo");
    assert_eq!(backup.commit, second);
    assert!(outcome.report.undo.expect("undo").contains("backup-before-recovery"));
}

#[test]
fn the_backup_report_carries_a_date_so_the_ui_can_say_how_old_it_is() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);

    let backup = service::create_backup(&crate::Git::open(scratch.path()).expect("open")).expect("backup");
    assert!(backup.created);
    assert!(backup.covers_head);
    assert_eq!(backup.date.len(), 10, "expected YYYY-MM-DD, got {:?}", backup.date);
    assert_eq!(backup.commit, scratch.git(&["rev-parse", "HEAD"]));
    assert_ne!(backup.commit, first);
}

#[test]
fn probing_the_backup_never_creates_or_moves_a_branch() {
    let Some(scratch) = Scratch::new() else { return };
    two_commits(&scratch);

    // Absent: the probe reports nothing and creates nothing.
    let view = service::read_view(scratch.path()).expect("view");
    assert!(view.backup.is_none());
    assert!(!scratch.has_branch("backup-before-recovery"), "a read must not create a branch");

    // Present: reported with its date, still not moved.
    let head = scratch.head();
    scratch.git(&["branch", "backup-before-recovery", &head]);
    let view = service::read_view(scratch.path()).expect("view");
    let backup = view.backup.expect("backup visible");
    assert!(backup.covers_head);
    assert_eq!(backup.commit, head);
    assert_eq!(backup.date.len(), 10);
    assert_eq!(scratch.git(&["rev-parse", "backup-before-recovery"]), head);
}

#[test]
fn files_strategy_restores_contents_without_moving_head() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    service::reset(scratch.path(), &first, "files", None).expect("restore file contents");

    assert_eq!(scratch.head(), second, "history is untouched");
    assert_eq!(scratch.contents("notes.md"), "first\n", "contents come from the snapshot");
}

#[test]
fn resets_are_refused_mid_merge() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);

    // A real conflicting merge leaves .git/MERGE_HEAD behind.
    scratch.git(&["checkout", "--quiet", "-b", "side", &first]);
    scratch.write("notes.md", "side\n");
    scratch.commit("side");
    scratch.git(&["checkout", "--quiet", "main"]);
    scratch.write("notes.md", "main\n");
    scratch.commit("main");
    let _ = Command::new("git")
        .args(["merge", "side"])
        .current_dir(scratch.path())
        .output()
        .expect("git merge");

    if !scratch.path().join(".git/MERGE_HEAD").exists() {
        eprintln!("skipping: the merge did not conflict here");
        return;
    }

    let err = service::reset(scratch.path(), &first, "soft", None).expect_err("refuse mid-merge");
    assert!(err.to_string().to_lowercase().contains("merge"), "{err}");
}

#[test]
fn reading_state_and_reflog_works_on_a_real_repository() {
    let Some(scratch) = Scratch::new() else { return };
    two_commits(&scratch);

    let state = service::read_state(scratch.path()).expect("state");
    assert_eq!(state.branch, "main");
    assert_eq!(state.dirty_count, 0);
    assert!(state.dirty.is_empty());
    assert!(state.in_progress.is_none());
    assert_eq!(state.head, scratch.short_head());

    let reflog = service::read_reflog(scratch.path(), 10).expect("reflog");
    let lines: Vec<&str> = reflog.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() >= 2, "two commits should produce reflog lines: {reflog:?}");
    assert!(lines[0].contains("commit"), "unexpected format: {:?}", lines[0]);

    // Dirty files are reported without a hard reset being involved.
    scratch.write("notes.md", "changed\n");
    let dirty = service::read_state(scratch.path()).expect("state");
    assert_eq!(dirty.dirty_count, 1);
    assert!(dirty.dirty.iter().any(|f| f.path == "notes.md"));
}

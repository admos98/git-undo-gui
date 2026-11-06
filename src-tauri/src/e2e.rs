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

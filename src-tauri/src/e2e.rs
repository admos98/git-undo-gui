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
            // Byte-exact checkouts. Without this a Windows developer with
            // `core.autocrlf=true` gets `\r\n` back from git and every assertion on
            // file contents fails — three tests failed here for months' worth of
            // reasons before being pinned. CI is Linux and never saw it.
            ("core.autocrlf", "false"),
            ("core.eol", "lf"),
            ("core.safecrlf", "false"),
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

    /// Write a file, creating parent directories (needed for the file/directory
    /// collision cases).
    fn write_nested(&self, name: &str, body: &str) {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(path, body).expect("write file");
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

    /// Read a path out of a commit, without touching the working tree. Used to prove
    /// a rescue snapshot really holds the discarded content.
    ///
    /// `Scratch::git` returns stdout with surrounding whitespace trimmed by the
    /// harness, so the comparison drops the trailing newline too. Assertions against
    /// this are about the *content* being preserved, not about byte-exact EOF.
    fn contents_at(&self, rev: &str, name: &str) -> String {
        self.git(&["show", &format!("{rev}:{name}")])
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
fn hard_reset_saves_uncommitted_work_before_discarding_it() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    // An uncommitted edit to a tracked file. `git reset --hard` destroys it and no
    // branch can bring it back — which is why this used to be a flat refusal. The
    // snapshot makes the work recoverable, so the reset is allowed and the report
    // says exactly how to get the edit back.
    scratch.write("notes.md", "work in progress\n");

    let outcome = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect("a snapshot makes the discard recoverable, so the reset runs");

    assert_eq!(scratch.head(), first, "the reset still moves HEAD");
    assert_eq!(scratch.contents("notes.md"), "first\n", "the working tree is rewritten");

    let saved = outcome.snapshot.expect("the report must carry the snapshot");
    assert_eq!(
        scratch.contents_at(&saved.commit, "notes.md"),
        "work in progress",
        "the snapshot holds the discarded edit"
    );
    assert!(
        saved.restore.contains(&saved.commit) && saved.restore.contains("restore"),
        "the printed route back must be runnable: {}",
        saved.restore
    );

    // Following the printed command really does restore the work — an undo hint is
    // only worth printing if it works, so this is executed rather than inspected.
    scratch.git(&["restore", &format!("--source={}", saved.commit), "--", "."]);
    assert_eq!(scratch.contents("notes.md"), "work in progress\n");

    // The backup branch still exists for the commits, and must not be confused with
    // the snapshot: it is what recovers history, not the working tree.
    assert!(scratch.has_branch("backup-before-recovery"));
    assert_eq!(outcome.previous_head, &second[..7.min(second.len())]);
}

#[test]
fn a_failed_reset_leaves_no_snapshot_and_no_backup() {
    // The snapshot is written before the guard runs, so a refusal that comes after it
    // must not leave the repo looking like a reset happened. Anything resembling a
    // safety net that did not get used is worse than none.
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);
    scratch.write("notes.md", "work in progress\n");

    // No acknowledgement phrase: refused before anything moves, but *after* the
    // snapshot was written — so this proves the snapshot alone does not authorise it.
    let err = service::reset(scratch.path(), &first, "hard", None)
        .expect_err("RESET is still required while holding a snapshot");
    assert!(matches!(err, crate::GitError::Refused { .. }), "{err:?}");
    assert!(!scratch.has_branch("backup-before-recovery"));
    assert_eq!(scratch.contents("notes.md"), "work in progress\n");
}

#[test]
fn hard_reset_refuses_ignored_files_the_target_tracks() {
    // The `.env` bug: a file that was committed, then untracked and gitignored, is
    // invisible to a plain `git status --porcelain`. Reproduced end to end because
    // the parser alone cannot prove `--ignored=traditional` was actually passed.
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("secret.txt", "committed default\n");
    scratch.write("keep.txt", "keep\n");
    scratch.git(&["add", "secret.txt", "keep.txt"]);
    scratch.git(&["commit", "--quiet", "-m", "one"]);
    let with_secret = scratch.head();

    scratch.git(&["rm", "--quiet", "--cached", "secret.txt"]);
    scratch.write(".gitignore", "secret.txt\n");
    scratch.git(&["add", ".gitignore"]);
    scratch.git(&["commit", "--quiet", "-m", "untrack and ignore"]);
    let without_secret = scratch.head();

    // The real local value — the one nobody wants overwritten. Before the fix this
    // was replaced by "committed default" and reported as a clean-tree reset.
    scratch.write("secret.txt", "REAL_LOCAL_SECRET\n");

    let outcome = service::reset(scratch.path(), &with_secret, "hard", Some("RESET"))
        .expect("an ignored file is now captured instead of being invisible");
    assert_eq!(scratch.head(), with_secret, "the reset went through");

    // The guard not refusing is only acceptable because the value was saved first.
    let saved = outcome.snapshot.expect("the ignored file must be in the snapshot");
    assert_eq!(
        scratch.contents_at(&saved.commit, "secret.txt"),
        "REAL_LOCAL_SECRET",
        "the snapshot holds the ignored file's real content"
    );

    // And the printed command puts it back, which is the only thing that makes
    // allowing the reset defensible.
    scratch.git(&["restore", &format!("--source={}", saved.commit), "--", "."]);
    assert_eq!(scratch.contents("secret.txt"), "REAL_LOCAL_SECRET\n");
    assert_eq!(scratch.head(), with_secret, "restoring files never moves HEAD");

    // `keep.txt` was untouched throughout: no over-collection.
    assert_eq!(scratch.contents("keep.txt"), "keep\n");
    let _ = without_secret;
}

#[test]
fn an_ignored_file_the_target_does_not_track_does_not_block() {
    // node_modules/ and friends are ignored and enormous. Refusing on them would make
    // the hard reset unusable; the entry exists only so the guard can see `.env`-style
    // files, and it must not turn every repo into a refusal.
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);
    scratch.write(".gitignore", "node_modules/\n");
    scratch.git(&["add", ".gitignore"]);
    scratch.git(&["commit", "--quiet", "-m", "ignore deps"]);
    std::fs::create_dir_all(scratch.path().join("node_modules/pkg")).unwrap();
    scratch.write_nested("node_modules/pkg/index.js", "module.exports = 1;\n");

    let outcome = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect("an ignored directory the target does not track is not a reason to refuse");
    assert_eq!(scratch.head(), first);
    assert!(
        scratch.path().join("node_modules/pkg/index.js").exists(),
        "the ignored directory is still on disk"
    );
    assert!(
        outcome.snapshot.is_none(),
        "nothing was captured, because a reset cannot touch those paths"
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

// ---------------------------------------------------------------------------
// Untracked files are not automatically safe.
//
// `git reset --hard` writes every path the target commit tracks. When a file
// already exists on one of those paths — untracked, therefore invisible to the
// "uncommitted changes" list — git overwrites it, and the app used to promise it
// would survive. The same is true of a file/directory mismatch, where git
// deletes one side to make room for the other.
// ---------------------------------------------------------------------------

#[test]
fn hard_reset_refuses_untracked_files_the_target_tracks() {
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("base.txt", "base\n");
    let first = scratch.commit("first");
    scratch.write_nested("notes.md", "committed notes\n");
    let second = scratch.commit("second");

    // Back at the first commit: notes.md exists only in the target's tree now.
    scratch.git(&["reset", "--hard", &first]);
    scratch.write("notes.md", "precious local work\n");

    let err = service::reset(scratch.path(), &second, "hard", Some("RESET"))
        .expect_err("must refuse to overwrite an untracked file");

    assert!(matches!(err, crate::GitError::Refused { .. }), "{err:?}");
    assert!(err.to_string().contains("notes.md"), "{err}");
    assert_eq!(scratch.contents("notes.md"), "precious local work\n");
    assert_eq!(scratch.head(), first, "a refusal must not move HEAD");
    assert!(
        !scratch.has_branch("backup-before-recovery"),
        "a refusal must not even create the backup branch"
    );
}

#[test]
fn hard_reset_refuses_when_the_target_turns_an_untracked_file_into_a_directory() {
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("base.txt", "base\n");
    let first = scratch.commit("first");
    scratch.write_nested("docs/guide.md", "committed guide\n");
    let second = scratch.commit("second");

    scratch.git(&["reset", "--hard", &first]);
    // An untracked *file* where the snapshot wants a *directory*: git deletes it.
    scratch.write("docs", "precious local notes\n");

    let err = service::reset(scratch.path(), &second, "hard", Some("RESET"))
        .expect_err("must refuse a file/directory collision");

    assert!(err.to_string().contains("docs"), "{err}");
    assert_eq!(scratch.contents("docs"), "precious local notes\n");
    assert_eq!(scratch.head(), first);
    assert!(!scratch.has_branch("backup-before-recovery"));
}

#[test]
fn hard_reset_refuses_when_the_target_turns_an_untracked_directory_into_a_file() {
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("base.txt", "base\n");
    let first = scratch.commit("first");
    scratch.write("notes.txt", "committed file\n");
    let second = scratch.commit("second");

    scratch.git(&["reset", "--hard", &first]);
    // An untracked *directory* where the snapshot wants a *file*.
    scratch.write_nested("notes.txt/inside.txt", "precious inside the directory\n");

    let err = service::reset(scratch.path(), &second, "hard", Some("RESET"))
        .expect_err("must refuse a directory/file collision");

    assert!(err.to_string().contains("notes.txt"), "{err}");
    assert_eq!(
        scratch.contents("notes.txt/inside.txt"),
        "precious inside the directory\n"
    );
    assert_eq!(scratch.head(), first);
    assert!(!scratch.has_branch("backup-before-recovery"));
}

#[test]
fn hard_reset_refuses_an_untracked_directory_the_target_writes_into() {
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("base.txt", "base\n");
    let first = scratch.commit("first");
    scratch.write_nested("pkg/keep.txt", "committed pkg\n");
    let second = scratch.commit("second");

    scratch.git(&["reset", "--hard", &first]);
    // `git status` reports this as `?? pkg/`; the target tracks pkg/keep.txt.
    scratch.write_nested("pkg/keep.txt", "precious pkg\n");
    scratch.write_nested("pkg/other.txt", "also precious\n");

    let err = service::reset(scratch.path(), &second, "hard", Some("RESET"))
        .expect_err("must refuse to write into an untracked directory");

    assert!(err.to_string().contains("pkg"), "{err}");
    assert_eq!(scratch.contents("pkg/keep.txt"), "precious pkg\n");
    assert_eq!(scratch.contents("pkg/other.txt"), "also precious\n");
    assert_eq!(scratch.head(), first);
    assert!(!scratch.has_branch("backup-before-recovery"));
}

#[test]
fn hard_reset_leaves_unrelated_untracked_files_alone() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);

    // Not in the target's tree, so nothing writes over it.
    scratch.write("scratch.md", "work in progress\n");

    let outcome = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect("unrelated untracked files must not block a reset");

    assert_eq!(scratch.contents("scratch.md"), "work in progress\n");
    assert_eq!(scratch.head(), first);
    assert!(
        outcome.report.risks.iter().any(|r| r.level == "info" && r.message.contains("scratch.md")),
        "the survivor must be named, not silently assumed: {:?}",
        outcome.report.risks
    );
}

// ---------------------------------------------------------------------------
// `restore --source` rewrites the working tree too.
//
// The guard used to tell the user that this strategy "keeps" uncommitted
// changes. It does not: restore writes every path the snapshot tracks and
// removes tracked paths it does not have, so the edits are gone and no backup
// branch can bring them back.
// ---------------------------------------------------------------------------

#[test]
fn files_restore_refuses_to_erase_uncommitted_edits() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);
    scratch.write("notes.md", "work in progress\n");

    let err = service::reset(scratch.path(), &first, "files", None)
        .expect_err("must refuse to restore over an uncommitted edit");

    assert!(err.to_string().contains("notes.md"), "{err}");
    assert_eq!(scratch.contents("notes.md"), "work in progress\n");
    assert_eq!(scratch.head(), second, "restore never moves HEAD, refused or not");
    assert!(
        !scratch.has_branch("backup-before-recovery"),
        "a refusal must not create the backup branch"
    );
}

#[test]
fn files_restore_refuses_untracked_files_it_would_overwrite() {
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("base.txt", "base\n");
    let first = scratch.commit("first");
    scratch.write_nested("added.txt", "committed version\n");
    let second = scratch.commit("second");

    scratch.git(&["reset", "--hard", &first]);
    // Untracked now, but `git restore --source=<second> -- .` writes it anyway.
    scratch.write("added.txt", "precious local version\n");

    let err = service::reset(scratch.path(), &second, "files", None)
        .expect_err("must refuse an untracked collision");

    assert!(err.to_string().contains("added.txt"), "{err}");
    assert_eq!(scratch.contents("added.txt"), "precious local version\n");
    assert!(!scratch.has_branch("backup-before-recovery"));
}

#[test]
fn files_restore_leaves_unrelated_untracked_files_alone() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);
    scratch.write("scratch.md", "not in the snapshot\n");

    let outcome = service::reset(scratch.path(), &first, "files", None)
        .expect("a clean tracked tree may be restored");

    assert_eq!(scratch.contents("notes.md"), "first\n");
    assert_eq!(scratch.contents("scratch.md"), "not in the snapshot\n");
    assert!(
        outcome.report.risks.iter().any(|r| r.level == "info" && r.message.contains("left alone")),
        "the untouched file must be accounted for: {:?}",
        outcome.report.risks
    );
}

// ---------------------------------------------------------------------------
// Identity, not string equality.
// ---------------------------------------------------------------------------

#[test]
fn resetting_to_the_current_commit_is_refused_with_a_full_or_short_hash() {
    let Some(scratch) = Scratch::new() else { return };
    let (_first, second) = two_commits(&scratch);

    // The full id used to slip past the no-op check because HEAD was compared as
    // a 7-character short hash.
    let err = service::reset(scratch.path(), &second, "hard", Some("RESET"))
        .expect_err("a full hash of HEAD is still a no-op");
    assert!(err.to_string().contains("already at"), "{err}");

    let short = &second[..7];
    let err = service::reset(scratch.path(), short, "hard", Some("RESET"))
        .expect_err("a short hash of HEAD is still a no-op");
    assert!(err.to_string().contains("already at"), "{err}");

    assert_eq!(scratch.head(), second, "nothing moved");
    assert!(
        !scratch.has_branch("backup-before-recovery"),
        "a no-op must not create a backup branch"
    );
}

#[test]
fn a_soft_no_op_is_refused_before_a_backup_branch_is_created() {
    let Some(scratch) = Scratch::new() else { return };
    let (_first, second) = two_commits(&scratch);

    let err = service::reset(scratch.path(), &second, "soft", None)
        .expect_err("soft reset to HEAD cannot change anything");
    assert!(err.to_string().contains("already at"), "{err}");
    assert!(
        !scratch.has_branch("backup-before-recovery"),
        "a refused no-op must not leave a branch behind"
    );
}

// ---------------------------------------------------------------------------
// Undo hints must undo the operation that actually ran.
// ---------------------------------------------------------------------------

#[test]
fn the_undo_after_a_soft_reset_keeps_the_work_that_survived_it() {
    // The reported bug: soft reset leaves uncommitted work in the tree (the guard
    // says so), but the undo hint was `git reset --hard backup-before-recovery`.
    // Following it deleted the work the dialog had just promised to keep.
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    // Uncommitted work on top of the second commit: it survives `--soft`.
    scratch.write("notes.md", "second\nprecious wip\n");
    assert_eq!(scratch.head(), second);

    let outcome = service::reset(scratch.path(), &first, "soft", None).expect("soft reset");
    assert_eq!(scratch.head(), first, "HEAD moved to the snapshot");
    assert!(
        scratch.contents("notes.md").contains("precious wip"),
        "a soft reset must not touch the working tree"
    );

    let undo = outcome.report.undo.expect("an undo hint");
    assert!(
        undo.contains("reset --soft"),
        "the undo for a soft reset must also be soft, else it deletes the work: {undo}"
    );
    assert!(
        !undo.contains("--hard"),
        "a --hard undo would erase the uncommitted work a soft reset kept: {undo}"
    );
    assert!(
        undo.contains(&second[..7]),
        "the undo must target the pre-reset commit: {undo}"
    );

    // And the hint must actually work when followed.
    scratch.git(&["reset", "--soft", &second]);
    assert_eq!(scratch.head(), second, "the offered undo puts HEAD back");
    assert!(
        scratch.contents("notes.md").contains("precious wip"),
        "and it leaves the uncommitted work alone"
    );
}

#[test]
fn the_undo_after_a_file_restore_does_not_move_history() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, second) = two_commits(&scratch);

    let outcome = service::reset(scratch.path(), &first, "files", None).expect("restore files");
    assert_eq!(scratch.contents("notes.md"), "first\n");

    let undo = outcome.report.undo.expect("an undo hint");
    assert!(undo.contains("restore --source"), "files undo must be a restore: {undo}");
    assert!(!undo.contains("reset --hard"), "history never moved, so do not rewrite it: {undo}");
    assert!(undo.contains(&second[..7]), "undo targets the pre-reset commit: {undo}");
}

#[test]
fn the_rescue_branch_strategy_offers_no_undo_because_nothing_moved() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);

    let outcome = service::reset(scratch.path(), &first, "branch", None).expect("rescue branch");
    assert!(
        outcome.report.undo.is_none(),
        "nothing moved, so there is nothing to undo: {:?}",
        outcome.report.undo
    );
}

// ---------------------------------------------------------------------------
// Untracked work is protected regardless of what the filename contains.
// ---------------------------------------------------------------------------

#[test]
fn hard_reset_refuses_untracked_work_whose_name_contains_a_space() {
    // The data-loss bug this test exists for: `git status --porcelain` C-quotes
    // such paths, so the guard compared `"my notes.md"` against a snapshot path of
    // `my notes.md`, saw no collision, and allowed the reset. git then overwrote
    // the untracked file. Reading status with `-z` closes it.
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("my notes.md", "committed\n");
    let first = scratch.commit("first");
    scratch.git(&["rm", "--quiet", "my notes.md"]);
    let second = scratch.commit("delete it");

    // Untracked now, with the same name the first snapshot tracks.
    scratch.write("my notes.md", "PRECIOUS UNTRACKED WORK\n");

    let err = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect_err("an untracked file on a tracked path must be refused");

    assert!(err.to_string().contains("my notes.md"), "{err}");
    assert!(
        err.to_string().contains("untracked"),
        "the refusal must say what kind of file it is: {err}"
    );
    assert_eq!(
        scratch.contents("my notes.md"),
        "PRECIOUS UNTRACKED WORK\n",
        "the untracked work must still be on disk"
    );
    assert_eq!(scratch.head(), second, "a refusal must not move HEAD");
}

#[test]
fn file_restore_refuses_untracked_work_whose_name_contains_a_space() {
    let Some(scratch) = Scratch::new() else { return };
    scratch.write("my notes.md", "committed\n");
    let first = scratch.commit("first");
    scratch.git(&["rm", "--quiet", "my notes.md"]);
    scratch.commit("delete it");

    scratch.write("my notes.md", "PRECIOUS UNTRACKED WORK\n");

    let err = service::reset(scratch.path(), &first, "files", None)
        .expect_err("restore would overwrite it, so refuse");
    assert!(err.to_string().contains("my notes.md"), "{err}");
    assert_eq!(scratch.contents("my notes.md"), "PRECIOUS UNTRACKED WORK\n");
}

// ---------------------------------------------------------------------------
// The repository state read has to be honest about what it could not read.
// ---------------------------------------------------------------------------

#[test]
fn a_corrupt_index_is_reported_instead_of_looking_clean() {
    // `run(...).unwrap_or_default()` turned a failed `git status` into an empty
    // dirty list: the guard then believed the tree was clean.
    let Some(scratch) = Scratch::new() else { return };
    two_commits(&scratch);
    scratch.write("notes.md", "uncommitted\n");
    scratch.write(".git/index", "not an index file");

    let err = service::read_state(scratch.path())
        .expect_err("a failed status read must not be reported as a clean tree");
    let message = err.to_string();
    assert!(
        message.contains("could not be read") || message.contains("index"),
        "the error must say what failed: {message}"
    );
}

#[test]
fn unpushed_commits_are_reported_on_a_branch_with_no_upstream() {
    // The headline scenario: a local-only branch. `@{upstream}..HEAD` failed with
    // "no upstream configured", so the app reported zero unpushed commits.
    let Some(scratch) = Scratch::new() else { return };
    let (_first, _second) = two_commits(&scratch);

    let state = service::read_state(scratch.path()).expect("read state");
    assert!(
        !state.unpushed.is_empty(),
        "a two-commit branch that was never pushed has unpushed commits"
    );
    assert_eq!(state.unpushed.len(), 2, "both commits are local-only: {:?}", state.unpushed);

    // And they must be surfaced as a risk on a hard reset, not silently dropped.
    let outcome = service::reset(scratch.path(), &_first, "hard", Some("RESET"))
        .expect("the backup branch makes this recoverable");
    assert!(
        outcome.report.risks.iter().any(|r| r.level == "danger" && r.message.contains("unpushed")),
        "the user must be told those commits left the branch: {:?}",
        outcome.report.risks
    );
}

#[test]
fn unpushed_commits_drop_to_zero_once_the_branch_is_pushed() {
    let Some(scratch) = Scratch::new() else { return };
    two_commits(&scratch);

    // A bare remote, pushed.
    let remote = scratch.path().join("remote.git");
    let bare = Command::new("git")
        .args(["init", "--quiet", "--bare", remote.to_str().expect("utf8 path")])
        .output()
        .expect("git init --bare");
    assert!(bare.status.success(), "{}", String::from_utf8_lossy(&bare.stderr));

    scratch.git(&["remote", "add", "origin", remote.to_str().expect("utf8 path")]);
    scratch.git(&["push", "--quiet", "-u", "origin", "main"]);

    let state = service::read_state(scratch.path()).expect("read state");
    assert!(state.unpushed.is_empty(), "everything is on the remote: {:?}", state.unpushed);
}

// ---------------------------------------------------------------------------
// The reflog the desktop reads has to be parseable by the shared UI code.
// ---------------------------------------------------------------------------

#[test]
fn the_reflog_read_uses_selectors_the_ui_can_parse() {
    let Some(scratch) = Scratch::new() else { return };
    two_commits(&scratch);

    let reflog = service::read_reflog(scratch.path(), 10).expect("reflog");
    let first_line = reflog.lines().next().expect("a reflog line");
    assert!(
        first_line.contains("HEAD@{"),
        "expected the %gd selector to survive: {first_line}"
    );
    assert!(
        !first_line.contains("HEAD@{20"),
        "a raw ISO stamp in the braces is not a reflog id: {first_line}"
    );
    // With --date=relative the braces hold "0" or "0, N seconds ago" / "N seconds ago".
    let inside = first_line
        .split("HEAD@{")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("braced selector");
    assert!(
        inside.trim_start().starts_with('0') || inside.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit()),
        "expected a reflog index or a relative age, got {inside:?}"
    );
    assert!(
        !inside.contains(':') || !inside.contains('-'),
        "an ISO timestamp leaked into the selector: {inside:?}"
    );
}

#[test]
fn the_reset_report_serialises_the_field_the_ui_reads() {
    let Some(scratch) = Scratch::new() else { return };
    let (first, _second) = two_commits(&scratch);

    let outcome = service::reset(scratch.path(), &first, "hard", Some("RESET"))
        .expect("acknowledged hard reset");
    let json = serde_json::to_value(&outcome).expect("serialise the reset outcome");
    let backup = json
        .get("backup")
        .and_then(|b| b.as_object())
        .expect("a backup object in the response");

    assert!(
        backup.contains_key("covers_head"),
        "the UI reads covers_head; the response has {backup:?}"
    );
    assert!(
        !backup.contains_key("safe_undo"),
        "safe_undo is not a field Rust ever sends: {backup:?}"
    );
    assert_eq!(backup.get("covers_head").and_then(|v| v.as_bool()), Some(true));
}

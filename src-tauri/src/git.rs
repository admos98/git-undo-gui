//! Thin, synchronous wrapper around the `git` binary.
//!
//! Deliberately dumb: no caching, no daemon, no background work, no network.
//! Each call spawns one short-lived process and returns. If `git` is missing the
//! app degrades to the copy-a-command experience instead of failing opaquely.

use git_undo_logic::error::{GitError, GitResult};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Upper bound on command output we keep in memory (~8 MB of text). A giant diff
/// must not be able to wedge the shell.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

pub struct Git {
    root: PathBuf,
}

impl Git {
    /// Verify `root` is the top level of a work tree, then bind to it.
    pub fn open(root: impl AsRef<Path>) -> GitResult<Self> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(GitError::NotARepository {
                detail: format!("{} is not a directory", root.display()),
            });
        }

        // `rev-parse --show-toplevel` fails for bare repos and for plain folders.
        let top = run_raw(root, &["rev-parse", "--show-toplevel"]).map_err(|e| {
            GitError::NotARepository {
                detail: format!("{}: {e}", root.display()),
            }
        })?;

        Ok(Git {
            root: PathBuf::from(top.trim()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Run git in the repository root and return trimmed stdout.
    pub fn run(&self, args: &[&str]) -> GitResult<String> {
        run_raw(&self.root, args).map_err(|detail| GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            detail,
        })
    }

    /// Run git and return the exit status without treating non-zero as fatal.
    /// Used for "is this dirty?" style queries where status is the answer.
    pub fn probe(&self, args: &[&str]) -> Option<String> {
        run_raw(&self.root, args).ok()
    }

    /// Run git against an alternate index file.
    ///
    /// `GIT_INDEX_FILE` is how the rescue snapshot reads and writes the working tree
    /// without disturbing the user's staging area: `git stash create` cannot do it
    /// (it ignores `-u` and refuses a tree of only-ignored files), so the snapshot is
    /// assembled in a throwaway index and the real one is never opened.
    pub fn run_with_index(&self, args: &[&str], index_file: &str) -> GitResult<String> {
        run_raw_with(&self.root, args, &[("GIT_INDEX_FILE", index_file)]).map_err(|detail| {
            GitError::CommandFailed {
                command: format!("git {}", args.join(" ")),
                detail,
            }
        })
    }

    /// Resolve a revision to the **full** commit id, or `None` when it is not a
    /// commit in this repository.
    ///
    /// Full ids are what make identity comparisons honest: `git rev-parse --short`
    /// produced `a43c821`, and comparing that to a pasted 40-char hash said
    /// "different commit" for the very same commit. Callers validate the input
    /// first (`hash::is_valid_hash`), so this only ever sees hex.
    pub fn resolve_commit(&self, rev: &str) -> Option<String> {
        self.probe(&["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Short, human name of the current branch, or `HEAD` when detached.
    pub fn current_branch(&self) -> String {
        self.probe(&["rev-parse", "--abbrev-ref", "HEAD"])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "HEAD".to_string())
    }
}

fn run_raw(dir: &Path, args: &[&str]) -> Result<String, String> {
    run_raw_with(dir, args, &[])
}

/// The one place a git process is spawned. `extra_env` lets a caller add
/// `GIT_INDEX_FILE` without a second copy of the spawn logic — and without a second
/// chance to forget `GIT_TERMINAL_PROMPT=0`.
fn run_raw_with(
    dir: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        // Never let git try to talk to a network or block on a prompt.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C");
    for (key, value) in extra_env {
        command.env(key, value);
    }

    let output = command.output().map_err(|e| format!("{e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let trimmed = stderr.trim();
        return Err(if trimmed.is_empty() {
            format!("exited with {}", output.status)
        } else {
            trimmed.to_string()
        });
    }

    let mut stdout = output.stdout;
    if stdout.len() > MAX_OUTPUT_BYTES {
        stdout.truncate(MAX_OUTPUT_BYTES);
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

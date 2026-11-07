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

    pub fn has_commit(&self, hash: &str) -> bool {
        self.probe(&["cat-file", "-e", &format!("{hash}^{{commit}}")])
            .is_some()
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
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        // Never let git try to talk to a network or block on a prompt.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("{e}"))?;

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

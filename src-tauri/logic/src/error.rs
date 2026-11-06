//! Every way a git operation can fail, in a shape the UI can render.
//!
//! The frontend shows these verbatim, so the messages are written for a person
//! who is already having a bad day — not for a log file.

#[cfg(feature = "serde")]
use serde::Serialize;

/// Serialised for the frontend as `{ "kind": "refused", "reason": "..." }`.
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
#[derive(Debug, Clone)]
pub enum GitError {
    /// The path is not a directory, or not a git worktree.
    NotARepository { detail: String },
    /// The commit does not exist in this repository.
    UnknownCommit { hash: String },
    /// A merge/rebase/cherry-pick/bisect is half-finished.
    OperationInProgress { operation: String },
    /// Refused on purpose: a guard said no.
    Refused { reason: String },
    /// `git` is missing or not executable.
    GitUnavailable { detail: String },
    /// git ran and exited non-zero.
    CommandFailed { command: String, detail: String },
    /// Anything unexpected.
    Internal { detail: String },
}

impl GitError {
    pub fn refused(reason: impl Into<String>) -> Self {
        GitError::Refused { reason: reason.into() }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        GitError::Internal { detail: detail.into() }
    }
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotARepository { detail } => write!(f, "Not a git repository: {detail}"),
            GitError::UnknownCommit { hash } => write!(f, "Commit {hash} does not exist here"),
            GitError::OperationInProgress { operation } => {
                write!(f, "A {operation} is in progress — finish or abort it first")
            }
            GitError::Refused { reason } => write!(f, "Refused: {reason}"),
            GitError::GitUnavailable { detail } => write!(f, "Could not run git: {detail}"),
            GitError::CommandFailed { command, detail } => {
                write!(f, "`{command}` failed: {detail}")
            }
            GitError::Internal { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for GitError {}

pub type GitResult<T> = Result<T, GitError>;

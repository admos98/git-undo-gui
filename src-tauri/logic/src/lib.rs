//! Pure safety logic for git-undo-gui.
//!
//! Deliberately free of `tauri`, process spawning and I/O: the rules that decide
//! whether a reset may proceed are the part most worth testing, so they live in a
//! crate that compiles in milliseconds and needs no system libraries.
//!
//! * [`state`]  — parses repository state (porcelain status, unpushed count, markers)
//! * [`paths`]  — decides which on-disk paths a snapshot write would overwrite
//! * [`guard`]  — decides allow/refuse and returns the risks to show
//! * [`hash`]   — validates commit hashes before they reach a command line
//! * [`error`]  — error shape the UI can render

pub mod error;
pub mod guard;
pub mod hash;
pub mod paths;
pub mod state;

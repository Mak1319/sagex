//! Headless archive library: same format and behaviour as the
//! `sagex-archive` binary, without CLI parsing or interactive prompts.
//!
//! All secrets arrive as function arguments; all outcomes return as
//! [`ArchError`] or report structs. Nothing here reads the terminal,
//! writes to stdout, or exits the process.

pub mod create;
pub mod error;
pub mod opts;
pub mod restore;
mod util;

pub use create::create;
pub use error::ArchError;
pub use opts::{CreateOptions, RestoreOptions, RestoreReport, SigStatus, SkippedEntry};
pub use restore::restore;

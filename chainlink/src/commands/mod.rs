//! Command implementations for chainlink CLI
//!
//! Commands are split into two categories:
//! - Print-only commands: Available in both std and no_std modes
//! - FS-dependent commands: Only available with the `std` feature

use crate::db::DbError;

/// Result type for commands
///
/// In std mode, this uses anyhow::Result for richer error context.
/// In no_std mode, this uses DbError directly.
#[cfg(feature = "std")]
pub type CmdResult<T> = anyhow::Result<T>;

#[cfg(not(feature = "std"))]
pub type CmdResult<T> = Result<T, DbError>;

// ============================================================================
// Print-only commands (available in both std and no_std)
// ============================================================================

pub mod archive;
pub mod comment;
pub mod create;
pub mod deps;
pub mod label;
pub mod list;
pub mod milestone;
pub mod next;
pub mod relate;
pub mod search;
pub mod session;
pub mod show;
pub mod timer;
pub mod tree;
pub mod update;

// ============================================================================
// FS-dependent commands (std only)
// ============================================================================

#[cfg(feature = "std")]
pub mod delete;
#[cfg(feature = "std")]
pub mod export;
#[cfg(feature = "std")]
pub mod import;
#[cfg(feature = "std")]
pub mod init;
#[cfg(feature = "std")]
pub mod status;
#[cfg(feature = "std")]
pub mod tested;

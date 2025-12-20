//! Migration framework adapters.
//!
//! This module provides abstractions for different migration frameworks (Diesel, SQLx, etc.).
//! Each framework implements the `MigrationAdapter` trait to handle framework-specific
//! file discovery, timestamp parsing, and validation.
//!
//! The framework is explicitly configured via the `framework` field in `diesel-guard.toml`.

use camino::{Utf8Path, Utf8PathBuf};
use std::error::Error;

mod diesel;
mod sqlx;

pub use diesel::DieselAdapter;
pub use sqlx::SqlxAdapter;

/// Result type for adapter operations.
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Migration direction (forward or rollback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDirection {
    /// Forward migration (apply changes).
    Up,
    /// Rollback migration (revert changes).
    Down,
}

/// Represents a single migration file to check.
#[derive(Debug, Clone)]
pub struct MigrationFile {
    /// Path to the SQL file.
    pub path: Utf8PathBuf,
    /// Timestamp extracted from migration name.
    pub timestamp: String,
    /// Migration direction (up or down).
    pub direction: MigrationDirection,
    /// Whether migration requires running outside a transaction (SQLx metadata).
    pub requires_no_transaction: bool,
}

/// Trait for migration framework adapters.
///
/// Each framework (Diesel, SQLx, etc.) implements this trait to provide
/// framework-specific behavior for discovering, parsing, and validating migrations.
pub trait MigrationAdapter: Send + Sync {
    /// Framework name for display/logging.
    fn name(&self) -> &'static str;

    /// Collect migration files from a directory.
    ///
    /// # Arguments
    /// * `dir` - Directory containing migrations
    /// * `start_after` - Optional timestamp filter (skip migrations before this)
    /// * `check_down` - Whether to include rollback (down) migrations
    ///
    /// # Returns
    /// Sorted list of migration files to check.
    fn collect_migration_files(
        &self,
        dir: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Vec<MigrationFile>>;

    /// Parse timestamp from migration name or path.
    ///
    /// Returns normalized timestamp string (typically YYYYMMDDHHMMSS format)
    /// or `None` if the name doesn't contain a valid timestamp.
    fn parse_timestamp(&self, name: &str) -> Option<String>;

    /// Validate timestamp format for this framework.
    ///
    /// Returns an error if the timestamp doesn't match the framework's
    /// expected format.
    fn validate_timestamp(&self, timestamp: &str) -> Result<()>;
}

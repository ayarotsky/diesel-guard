//! Migration framework adapters.
//!
//! This module provides abstractions for different migration frameworks (Diesel, SQLx, etc.).
//! Each framework implements the `MigrationAdapter` trait to handle framework-specific
//! file discovery, timestamp parsing, and validation.
//!
//! The framework is explicitly configured via the `framework` field in `diesel-guard.toml`.

use camino::{Utf8Path, Utf8PathBuf};
use std::error::Error;
use walkdir::{DirEntry, WalkDir};

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

impl MigrationFile {
    /// Create a new migration file with the most common defaults.
    ///
    /// Sets direction to Up and requires_no_transaction to false.
    pub fn new(path: Utf8PathBuf, timestamp: String) -> Self {
        Self {
            path,
            timestamp,
            direction: MigrationDirection::Up,
            requires_no_transaction: false,
        }
    }

    /// Builder method to set the direction.
    pub fn with_direction(mut self, direction: MigrationDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Builder method to set requires_no_transaction.
    pub fn with_no_transaction(mut self, requires: bool) -> Self {
        self.requires_no_transaction = requires;
        self
    }
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

    /// Normalize a timestamp by removing separators (underscores and dashes).
    ///
    /// Default implementation removes `_` and `-` characters.
    /// Frameworks can override if they need different normalization logic.
    fn normalize_timestamp(&self, timestamp: &str) -> String {
        timestamp.replace(['_', '-'], "")
    }
}

/// Check if migration should be checked based on start_after filter.
///
/// Returns true if the migration should be checked (timestamp is after the filter).
pub(crate) fn should_check_migration(start_after: Option<&str>, migration_timestamp: &str) -> bool {
    let Some(start_after) = start_after else {
        return true; // No filter, check all migrations
    };

    // Normalize both timestamps by removing separators
    let start_normalized = start_after.replace(['_', '-'], "");
    let migration_normalized = migration_timestamp.replace(['_', '-'], "");

    // String comparison works because YYYYMMDDHHMMSS is lexicographically ordered
    migration_normalized > start_normalized
}

/// Collect and sort directory entries from a directory.
///
/// Returns entries sorted by path, with errors filtered out.
pub(crate) fn collect_and_sort_entries(dir: &Utf8Path) -> Vec<DirEntry> {
    let mut entries: Vec<_> = WalkDir::new(dir)
        .max_depth(1)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    entries.sort_by(|a, b| a.path().cmp(b.path()));
    entries
}

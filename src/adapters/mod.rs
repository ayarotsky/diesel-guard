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
}

impl MigrationFile {
    /// Create a new migration file with the most common defaults.
    ///
    /// Sets direction to Up.
    pub fn new(path: Utf8PathBuf, timestamp: String) -> Self {
        Self {
            path,
            timestamp,
            direction: MigrationDirection::Up,
        }
    }

    /// Builder method to set the direction.
    pub fn with_direction(mut self, direction: MigrationDirection) -> Self {
        self.direction = direction;
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

    /// Extract the SQL section relevant to the given migration direction.
    ///
    /// Adapters that use marker-based migrations (e.g., `-- migrate:up` / `-- migrate:down`)
    /// override this to return only the relevant section. The default returns all SQL unchanged.
    fn extract_sql_for_direction<'a>(
        &self,
        sql: &'a str,
        _direction: MigrationDirection,
    ) -> &'a str {
        sql
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

    // If both are purely numeric, compare as integers to handle variable-width
    // version numbers (e.g. "2" vs "10"). Otherwise fall back to string comparison
    // which works for fixed-width timestamps like YYYYMMDDHHMMSS.
    match (
        migration_normalized.parse::<i64>(),
        start_normalized.parse::<i64>(),
    ) {
        (Ok(mig), Ok(start)) => mig > start,
        _ => migration_normalized > start_normalized,
    }
}

/// Check if a directory is a single migration directory (contains up.sql directly).
///
/// This is used to detect when the user points at a specific migration directory
/// (e.g., `migrations/2024_01_01_000000_create_users/`) rather than the parent.
pub(crate) fn is_single_migration_dir(dir: &Utf8Path) -> bool {
    dir.join("up.sql").exists()
}

/// Collect and sort directory entries from a directory.
///
/// Returns entries sorted by path, with errors filtered out.
pub(crate) fn collect_and_sort_entries(dir: &Utf8Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    for result in WalkDir::new(dir).max_depth(1).min_depth(1) {
        match result {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                eprintln!("Warning: Failed to read entry in {}: {}", dir, e);
            }
        }
    }

    entries.sort_by(|a, b| a.path().cmp(b.path()));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_single_migration_dir_with_up_sql() {
        let temp_dir = TempDir::new().unwrap();
        let dir = Utf8Path::from_path(temp_dir.path()).unwrap();
        fs::write(dir.join("up.sql"), "CREATE TABLE t();").unwrap();
        assert!(is_single_migration_dir(dir));
    }

    #[test]
    fn test_is_single_migration_dir_without_up_sql() {
        let temp_dir = TempDir::new().unwrap();
        let dir = Utf8Path::from_path(temp_dir.path()).unwrap();
        assert!(!is_single_migration_dir(dir));
    }
}

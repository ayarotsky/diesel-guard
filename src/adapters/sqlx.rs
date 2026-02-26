//! SQLx migration adapter.
//!
//! Supports SQLx migration formats:
//! 1. Suffix-based (reversible): `<VERSION>_<DESC>.up.sql` / `<VERSION>_<DESC>.down.sql`
//! 2. Single file (up-only): `<VERSION>_<DESC>.sql`
//!

use super::{
    MigrationAdapter, MigrationFile, Result, collect_and_sort_entries, should_check_migration,
};
use camino::Utf8Path;
use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for SQLx version format (one or more digits).
///
/// SQLx accepts any positive i64 as a migration version number, so this matches
/// any leading digit sequence (e.g. `1`, `001`, `42`, `20240101000000`).
static SQLX_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)(_|\.)?").expect("valid regex pattern"));

/// SQLx migration adapter.
pub struct SqlxAdapter;

impl MigrationAdapter for SqlxAdapter {
    fn name(&self) -> &'static str {
        "SQLx"
    }

    fn collect_migration_files(
        &self,
        dir: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Vec<MigrationFile>> {
        let entries = collect_and_sort_entries(dir);
        let mut files = Vec::new();

        for entry in entries {
            let Some(path) = Utf8Path::from_path(entry.path()) else {
                continue;
            };

            if entry.file_type().is_file() && path.extension() == Some("sql") {
                files.extend(self.process_migration_file(path, start_after, check_down)?);
            }
        }

        Ok(files)
    }

    fn parse_timestamp(&self, name: &str) -> Option<String> {
        SQLX_VERSION_REGEX
            .captures(name)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn validate_timestamp(&self, timestamp: &str) -> Result<()> {
        if !timestamp.is_empty() && timestamp.chars().all(|c| c.is_ascii_digit()) {
            Ok(())
        } else {
            Err(format!(
                "Invalid SQLx version format: {}. Expected: one or more digits",
                timestamp
            )
            .into())
        }
    }
}

impl SqlxAdapter {
    /// Process a migration file (formats 1 or 2).
    fn process_migration_file(
        &self,
        path: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Vec<MigrationFile>> {
        let filename = path.file_name().unwrap_or("");

        // Skip .down.sql files early when check_down is disabled,
        // before any format detection can match and include them
        if !check_down {
            let file_stem = path.file_stem().unwrap_or("");
            if file_stem.ends_with(".down") {
                return Ok(vec![]);
            }
        }

        // Check for suffix format (.up.sql / .down.sql)
        if let Some(mig_file) = self.try_suffix_format(path, start_after)? {
            return Ok(vec![mig_file]);
        }

        // Check for single file format
        if let Some(mig_file) = self.try_single_file_format(path, filename, start_after)? {
            return Ok(vec![mig_file]);
        }

        Ok(vec![])
    }

    /// Try to parse as suffix format (.up.sql / .down.sql).
    ///
    /// Pure format detection — does not filter by `check_down`.
    /// The caller (`process_migration_file`) is responsible for skipping
    /// `.down.sql` files when `check_down` is disabled.
    fn try_suffix_format(
        &self,
        path: &Utf8Path,
        start_after: Option<&str>,
    ) -> Result<Option<MigrationFile>> {
        let file_stem = path.file_stem().unwrap_or("");

        let timestamp_part = if let Some(part) = file_stem.strip_suffix(".up") {
            part
        } else if let Some(part) = file_stem.strip_suffix(".down") {
            part
        } else {
            return Ok(None);
        };

        let Some(timestamp) = self.parse_timestamp(timestamp_part) else {
            return Ok(None);
        };

        if !should_check_migration(start_after, &timestamp) {
            return Ok(None);
        }

        Ok(Some(MigrationFile::new(path.to_owned(), timestamp)))
    }

    /// Try to parse as single file format (format 2: up-only).
    fn try_single_file_format(
        &self,
        path: &Utf8Path,
        filename: &str,
        start_after: Option<&str>,
    ) -> Result<Option<MigrationFile>> {
        let Some(timestamp) = self.parse_timestamp(filename) else {
            return Ok(None);
        };

        if !should_check_migration(start_after, &timestamp) {
            return Ok(None);
        }

        Ok(Some(MigrationFile::new(path.to_owned(), timestamp)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::should_check_migration;
    use std::fs;

    #[test]
    fn test_parse_timestamp() {
        let adapter = SqlxAdapter;
        assert_eq!(
            adapter.parse_timestamp("20240101000000_create_users"),
            Some("20240101000000".to_string())
        );
        assert_eq!(
            adapter.parse_timestamp("20240101000000.up.sql"),
            Some("20240101000000".to_string())
        );
        assert_eq!(
            adapter.parse_timestamp("20240101000000"),
            Some("20240101000000".to_string())
        );
        // SQLx accepts any positive i64 as version number
        assert_eq!(adapter.parse_timestamp("1_init.sql"), Some("1".to_string()));
        assert_eq!(
            adapter.parse_timestamp("001_create_users.sql"),
            Some("001".to_string())
        );
        assert_eq!(
            adapter.parse_timestamp("42_add_columns.up.sql"),
            Some("42".to_string())
        );
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let adapter = SqlxAdapter;
        assert_eq!(adapter.parse_timestamp("invalid_name"), None);
        assert_eq!(adapter.parse_timestamp("_no_leading_digits"), None);
    }

    #[test]
    fn test_validate_timestamp() {
        let adapter = SqlxAdapter;
        assert!(adapter.validate_timestamp("20240101000000").is_ok());
        assert!(adapter.validate_timestamp("20231231235959").is_ok());
        assert!(adapter.validate_timestamp("1").is_ok());
        assert!(adapter.validate_timestamp("001").is_ok());
        assert!(adapter.validate_timestamp("42").is_ok());
        assert!(adapter.validate_timestamp("").is_err());
        assert!(adapter.validate_timestamp("invalid").is_err());
    }

    #[test]
    fn test_should_check_migration() {
        // No filter
        assert!(should_check_migration(None, "20240101000000"));

        // With filter (14-digit timestamps)
        assert!(should_check_migration(
            Some("20240101000000"),
            "20240102000000"
        ));
        assert!(!should_check_migration(
            Some("20240101000000"),
            "20240101000000"
        ));
        assert!(!should_check_migration(
            Some("20240101000000"),
            "20231231235959"
        ));

        // Short numeric versions use numeric comparison
        assert!(should_check_migration(Some("2"), "10"));
        assert!(should_check_migration(Some("9"), "10"));
        assert!(should_check_migration(Some("1"), "2"));
        assert!(!should_check_migration(Some("10"), "2"));
        assert!(!should_check_migration(Some("5"), "5"));
    }

    #[test]
    fn test_suffix_down_sql_skipped_when_check_down_false() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let down_file = temp_dir.path().join("20240101000000_create_users.down.sql");
        fs::write(&down_file, "ALTER TABLE users DROP COLUMN admin;").unwrap();

        let adapter = SqlxAdapter;
        let path = Utf8Path::from_path(&down_file).expect("path should be valid UTF-8");
        let files = adapter.process_migration_file(path, None, false).unwrap();

        assert!(files.is_empty());
    }
}

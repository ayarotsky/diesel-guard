//! Diesel migration adapter.
//!
//! Supports Diesel's directory-based migration structure:
//! ```text
//! migrations/
//! └── 2024_01_01_000000_create_users/
//!     ├── up.sql
//!     └── down.sql
//! ```

use super::{
    collect_and_sort_entries, is_single_migration_dir, should_check_migration, MigrationAdapter,
    MigrationDirection, MigrationFile, Result,
};
use camino::Utf8Path;
use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for Diesel timestamp formats.
/// Accepts: YYYY_MM_DD_HHMMSS, YYYY-MM-DD-HHMMSS, or YYYYMMDDHHMMSS
static DIESEL_TIMESTAMP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}_\d{2}_\d{2}_\d{6}|\d{4}-\d{2}-\d{2}-\d{6}|\d{14})")
        .expect("valid regex pattern")
});

/// Diesel migration adapter.
pub struct DieselAdapter;

impl MigrationAdapter for DieselAdapter {
    fn name(&self) -> &'static str {
        "Diesel"
    }

    fn collect_migration_files(
        &self,
        dir: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Vec<MigrationFile>> {
        if is_single_migration_dir(dir) {
            // When the user targets a specific migration directory, skip the
            // start_after filter — they explicitly chose this migration.
            return self.process_migration_directory(dir, None, check_down);
        }

        let entries = collect_and_sort_entries(dir);
        let mut files = Vec::new();

        for entry in entries {
            let Some(path) = Utf8Path::from_path(entry.path()) else {
                continue;
            };

            if entry.file_type().is_dir() {
                files.extend(self.process_migration_directory(path, start_after, check_down)?);
            } else if path.extension() == Some("sql") {
                let filename = path.file_name().unwrap_or("");
                let parsed_timestamp = self.parse_timestamp(filename);

                // Apply start_after filter when the file has a valid timestamp.
                // Files without timestamps (e.g., "migration.sql") are always checked.
                if let Some(ref ts) = parsed_timestamp {
                    if !should_check_migration(start_after, ts) {
                        continue;
                    }
                }

                let timestamp = parsed_timestamp.unwrap_or_else(|| filename.to_string());
                files.push(MigrationFile::new(path.to_owned(), timestamp));
            }
        }

        Ok(files)
    }

    fn parse_timestamp(&self, name: &str) -> Option<String> {
        DIESEL_TIMESTAMP_REGEX
            .captures(name)
            .and_then(|cap| cap.get(1))
            .map(|m| {
                // Normalize by removing separators
                m.as_str().replace(['_', '-'], "")
            })
    }

    fn validate_timestamp(&self, timestamp: &str) -> Result<()> {
        let Some(captures) = DIESEL_TIMESTAMP_REGEX.captures(timestamp) else {
            return Err(format!(
                "Invalid Diesel timestamp format: {}. Expected: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS",
                timestamp
            ).into());
        };

        // Check if the matched part is the entire string
        if captures.get(0).unwrap().as_str() == timestamp {
            Ok(())
        } else {
            Err(format!(
                "Invalid Diesel timestamp format: {}. Expected: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS",
                timestamp
            ).into())
        }
    }
}

impl DieselAdapter {
    /// Process a Diesel migration directory and return SQL files to check.
    fn process_migration_directory(
        &self,
        path: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Vec<MigrationFile>> {
        let dir_name = match path.file_name() {
            Some(name) => name,
            None => return Ok(vec![]),
        };

        // Parse timestamp from directory name (if present)
        let timestamp = self.parse_timestamp(dir_name).unwrap_or_else(|| {
            // If no timestamp found, use directory name as timestamp
            // This allows checking directories without timestamps (e.g., test fixtures)
            dir_name.to_string()
        });

        // Skip if migration is before start_after threshold (only if start_after is set)
        if !should_check_migration(start_after, &timestamp) {
            return Ok(vec![]);
        }

        let mut files = vec![];

        // Always check up.sql if it exists
        let up_sql = path.join("up.sql");
        if up_sql.exists() {
            files.push(MigrationFile::new(up_sql, timestamp.clone()));
        }

        // Check down.sql only if enabled in config
        if check_down {
            let down_sql = path.join("down.sql");
            if down_sql.exists() {
                files.push(
                    MigrationFile::new(down_sql, timestamp)
                        .with_direction(MigrationDirection::Down),
                );
            }
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::should_check_migration;

    #[test]
    fn test_parse_timestamp_with_underscores() {
        let adapter = DieselAdapter;
        assert_eq!(
            adapter.parse_timestamp("2024_01_01_000000_create_users"),
            Some("20240101000000".to_string())
        );
    }

    #[test]
    fn test_parse_timestamp_with_dashes() {
        let adapter = DieselAdapter;
        assert_eq!(
            adapter.parse_timestamp("2024-01-01-000000_create_users"),
            Some("20240101000000".to_string())
        );
    }

    #[test]
    fn test_parse_timestamp_no_separators() {
        let adapter = DieselAdapter;
        assert_eq!(
            adapter.parse_timestamp("20240101000000_create_users"),
            Some("20240101000000".to_string())
        );
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let adapter = DieselAdapter;
        assert_eq!(adapter.parse_timestamp("invalid_name"), None);
        assert_eq!(adapter.parse_timestamp("2024_01_01"), None);
    }

    #[test]
    fn test_validate_timestamp() {
        let adapter = DieselAdapter;
        assert!(adapter.validate_timestamp("2024_01_01_000000").is_ok());
        assert!(adapter.validate_timestamp("2024-01-01-000000").is_ok());
        assert!(adapter.validate_timestamp("20240101000000").is_ok());
        assert!(adapter.validate_timestamp("invalid").is_err());
    }

    #[test]
    fn test_should_check_migration() {
        // No filter - check all
        assert!(should_check_migration(None, "20240101000000"));
        assert!(should_check_migration(None, "20200101000000"));

        // With filter - check only after
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
    }

    #[test]
    fn test_should_check_migration_mixed_formats() {
        // start_after with underscores vs migration without
        assert!(should_check_migration(
            Some("2024_01_01_000000"),
            "20240102000000"
        ));
        assert!(!should_check_migration(
            Some("2024_01_01_000000"),
            "20240101000000"
        ));

        // start_after without separators vs migration with dashes
        assert!(should_check_migration(
            Some("20240101000000"),
            "2024-01-02-000000"
        ));
        assert!(!should_check_migration(
            Some("20240101000000"),
            "2024-01-01-000000"
        ));
    }

    #[test]
    fn test_single_migration_dir_skips_down_sql() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
        fs::create_dir(&migration_dir).unwrap();
        fs::write(
            migration_dir.join("up.sql"),
            "ALTER TABLE users ADD COLUMN admin BOOLEAN;",
        )
        .unwrap();
        fs::write(
            migration_dir.join("down.sql"),
            "ALTER TABLE users DROP COLUMN admin;",
        )
        .unwrap();

        let adapter = DieselAdapter;
        let migration_path =
            Utf8Path::from_path(&migration_dir).expect("path should be valid UTF-8");
        let files = adapter
            .collect_migration_files(migration_path, None, false)
            .unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].path.as_str().contains("up.sql"));
    }

    #[test]
    fn test_single_migration_dir_includes_down_sql() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
        fs::create_dir(&migration_dir).unwrap();
        fs::write(
            migration_dir.join("up.sql"),
            "ALTER TABLE users ADD COLUMN admin BOOLEAN;",
        )
        .unwrap();
        fs::write(
            migration_dir.join("down.sql"),
            "ALTER TABLE users DROP COLUMN admin;",
        )
        .unwrap();

        let adapter = DieselAdapter;
        let migration_path =
            Utf8Path::from_path(&migration_dir).expect("path should be valid UTF-8");
        let files = adapter
            .collect_migration_files(migration_path, None, true)
            .unwrap();

        assert_eq!(files.len(), 2);
        let paths: Vec<String> = files.iter().map(|f| f.path.to_string()).collect();
        assert!(paths.iter().any(|p| p.contains("up.sql")));
        assert!(paths.iter().any(|p| p.contains("down.sql")));
    }
}

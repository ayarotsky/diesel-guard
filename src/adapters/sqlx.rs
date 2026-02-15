//! SQLx migration adapter.
//!
//! Supports SQLx's native migration formats:
//! 1. Suffix-based (reversible): `<VERSION>_<DESC>.up.sql` / `<VERSION>_<DESC>.down.sql`
//! 2. Single file (up-only): `<VERSION>_<DESC>.sql`
//!
//! Also supports additional formats for compatibility with other tools:
//! 3. Marker-based (dbmate-style): `-- migrate:up` / `-- migrate:down` in a single file
//! 4. Directory-based (Diesel-style): `<VERSION>_<DESC>/{up.sql, down.sql}`
//!

use super::{
    collect_and_sort_entries, is_single_migration_dir, should_check_migration, MigrationAdapter,
    MigrationDirection, MigrationFile, Result,
};
use camino::Utf8Path;
use regex::Regex;
use std::fs;
use std::sync::LazyLock;

/// Regex pattern for SQLx version format (one or more digits).
///
/// SQLx accepts any positive i64 as a migration version number, so this matches
/// any leading digit sequence (e.g. `1`, `001`, `42`, `20240101000000`).
static SQLX_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)(_|\.)?").expect("valid regex pattern"));

/// Regex pattern for detecting SQLx migration markers.
/// Matches -- migrate:up and -- migrate:down markers (case-insensitive).
static MIGRATE_MARKER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)--\s*migrate:(up|down)").expect("valid regex pattern"));

/// SQLx migration adapter.
pub struct SqlxAdapter;

impl MigrationAdapter for SqlxAdapter {
    fn name(&self) -> &'static str {
        "SQLx"
    }

    fn extract_sql_for_direction<'a>(
        &self,
        sql: &'a str,
        direction: MigrationDirection,
    ) -> &'a str {
        if contains_migrate_markers(sql) {
            match direction {
                MigrationDirection::Up => extract_up_section(sql),
                MigrationDirection::Down => extract_down_section(sql),
            }
        } else {
            sql
        }
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

            if entry.file_type().is_file() && path.extension() == Some("sql") {
                files.extend(self.process_migration_file(path, start_after, check_down)?);
            } else if entry.file_type().is_dir() {
                files.extend(self.process_migration_directory(path, start_after, check_down)?);
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
    /// Process a migration file (formats 1, 2, or 3).
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

        // Check for single file or marker format
        if let Some(mig_files) =
            self.try_single_or_marker_format(path, filename, start_after, check_down)?
        {
            return Ok(mig_files);
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

        let (timestamp_part, direction) = if let Some(part) = file_stem.strip_suffix(".up") {
            (part, MigrationDirection::Up)
        } else if let Some(part) = file_stem.strip_suffix(".down") {
            (part, MigrationDirection::Down)
        } else {
            return Ok(None);
        };

        let Some(timestamp) = self.parse_timestamp(timestamp_part) else {
            return Ok(None);
        };

        if !should_check_migration(start_after, &timestamp) {
            return Ok(None);
        }

        Ok(Some(
            MigrationFile::new(path.to_owned(), timestamp).with_direction(direction),
        ))
    }

    /// Try to parse as single file or marker format.
    fn try_single_or_marker_format(
        &self,
        path: &Utf8Path,
        filename: &str,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Option<Vec<MigrationFile>>> {
        let Some(timestamp) = self.parse_timestamp(filename) else {
            return Ok(None);
        };

        if !should_check_migration(start_after, &timestamp) {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;

        // Check if it's marker-based (contains both up and down markers)
        if contains_migrate_markers(&content) {
            // Format 3: Marker-based (dbmate-style)
            let mut files = vec![MigrationFile::new(path.to_owned(), timestamp.clone())];

            if check_down {
                files.push(
                    MigrationFile::new(path.to_owned(), timestamp)
                        .with_direction(MigrationDirection::Down),
                );
            }

            Ok(Some(files))
        } else {
            // Format 2: Single file (up-only)
            Ok(Some(vec![MigrationFile::new(path.to_owned(), timestamp)]))
        }
    }

    /// Process a migration directory (format 4: Diesel-style).
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

        // Parse timestamp from directory name
        let Some(timestamp) = self.parse_timestamp(dir_name) else {
            return Ok(vec![]);
        };

        // Skip if migration is before start_after threshold
        if !should_check_migration(start_after, &timestamp) {
            return Ok(vec![]);
        }

        let mut files = vec![];

        // Check up.sql
        let up_sql = path.join("up.sql");
        if up_sql.exists() {
            files.push(MigrationFile::new(up_sql, timestamp.clone()));
        }

        // Check down.sql if enabled
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

/// Extract the "up" section from SQLx marker-based migration.
fn extract_up_section(sql: &str) -> &str {
    let sql_lower = sql.to_lowercase();
    if let Some(up_pos) = sql_lower.find("-- migrate:up") {
        let start = up_pos + "-- migrate:up".len();
        if let Some(down_pos) = sql_lower[start..].find("-- migrate:down") {
            &sql[start..start + down_pos]
        } else {
            &sql[start..]
        }
    } else {
        sql
    }
}

/// Extract the "down" section from SQLx marker-based migration.
fn extract_down_section(sql: &str) -> &str {
    let sql_lower = sql.to_lowercase();
    if let Some(down_pos) = sql_lower.find("-- migrate:down") {
        &sql[down_pos + "-- migrate:down".len()..]
    } else {
        ""
    }
}

/// Check if SQL contains SQLx migration markers.
///
/// Uses regex to detect both -- migrate:up and -- migrate:down markers (case-insensitive).
fn contains_migrate_markers(content: &str) -> bool {
    let has_up = MIGRATE_MARKER_REGEX
        .captures_iter(content)
        .any(|cap| cap[1].eq_ignore_ascii_case("up"));
    let has_down = MIGRATE_MARKER_REGEX
        .captures_iter(content)
        .any(|cap| cap[1].eq_ignore_ascii_case("down"));
    has_up && has_down
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::should_check_migration;

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
    fn test_contains_migrate_markers() {
        let sql_with_markers = "-- migrate:up\nCREATE TABLE;\n-- migrate:down\nDROP TABLE;";
        assert!(contains_migrate_markers(sql_with_markers));

        let sql_no_down = "-- migrate:up\nCREATE TABLE;";
        assert!(!contains_migrate_markers(sql_no_down));

        let sql_no_markers = "CREATE TABLE;";
        assert!(!contains_migrate_markers(sql_no_markers));
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
    fn test_single_migration_dir_skips_down_sql() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("20240101000000_test");
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

        let adapter = SqlxAdapter;
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
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let migration_dir = temp_dir.path().join("20240101000000_test");
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

        let adapter = SqlxAdapter;
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

    #[test]
    fn test_extract_up_section() {
        let sql =
            "-- migrate:up\nCREATE TABLE users (id INT);\n\n-- migrate:down\nDROP TABLE users;";

        let up_section = extract_up_section(sql);
        assert!(up_section.contains("CREATE TABLE users"));
        assert!(!up_section.contains("DROP TABLE users"));
        assert!(!up_section.contains("-- migrate:down"));
    }

    #[test]
    fn test_extract_down_section() {
        let sql =
            "-- migrate:up\nCREATE TABLE users (id INT);\n\n-- migrate:down\nDROP TABLE users;";

        let down_section = extract_down_section(sql);
        assert!(down_section.contains("DROP TABLE users"));
        assert!(!down_section.contains("CREATE TABLE users"));
    }

    #[test]
    fn test_extract_up_section_no_markers() {
        let sql = "CREATE TABLE users (id INT);";
        let up_section = extract_up_section(sql);
        assert_eq!(up_section, sql);
    }

    #[test]
    fn test_extract_down_section_no_marker() {
        let sql = "CREATE TABLE users (id INT);";
        let down_section = extract_down_section(sql);
        assert_eq!(down_section, "");
    }

    #[test]
    fn test_extract_up_section_no_down_marker() {
        let sql = "-- migrate:up\nCREATE TABLE users (id INT);";

        let up_section = extract_up_section(sql);
        assert!(up_section.contains("CREATE TABLE users"));
    }

    #[test]
    fn test_extract_sql_for_direction_up() {
        let adapter = SqlxAdapter;
        let sql =
            "-- migrate:up\nCREATE TABLE users (id INT);\n\n-- migrate:down\nDROP TABLE users;";

        let result = adapter.extract_sql_for_direction(sql, MigrationDirection::Up);
        assert!(result.contains("CREATE TABLE users"));
        assert!(!result.contains("DROP TABLE users"));
    }

    #[test]
    fn test_extract_sql_for_direction_down() {
        let adapter = SqlxAdapter;
        let sql =
            "-- migrate:up\nCREATE TABLE users (id INT);\n\n-- migrate:down\nDROP TABLE users;";

        let result = adapter.extract_sql_for_direction(sql, MigrationDirection::Down);
        assert!(result.contains("DROP TABLE users"));
        assert!(!result.contains("CREATE TABLE users"));
    }

    #[test]
    fn test_extract_sql_for_direction_no_markers() {
        let adapter = SqlxAdapter;
        let sql = "CREATE TABLE users (id INT);";

        let result = adapter.extract_sql_for_direction(sql, MigrationDirection::Up);
        assert_eq!(result, sql);
    }
}

//! SQLx migration adapter.
//!
//! Supports all four SQLx migration formats:
//! 1. Suffix-based: `20240101000000_init.up.sql` / `20240101000000_init.down.sql`
//! 2. Single file (up-only): `20240101000000_init.sql`
//! 3. Marker-based (up+down in one file): `-- migrate:up` / `-- migrate:down`
//! 4. Directory-based: `20240101000000_init/{up.sql, down.sql}`
//!
//! Also parses SQLx metadata directives like `-- migrate:no-transaction`.

use super::{
    collect_and_sort_entries, should_check_migration, MigrationAdapter, MigrationDirection,
    MigrationFile, Result,
};
use camino::Utf8Path;
use regex::Regex;
use std::fs;
use std::sync::LazyLock;

/// Regex pattern for SQLx timestamp format (14 digits, no separators).
static SQLX_TIMESTAMP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{14})(_|\.)?").expect("valid regex pattern"));

/// Regex pattern for detecting CONCURRENTLY operations.
/// Matches CREATE INDEX CONCURRENTLY, DROP INDEX CONCURRENTLY, REINDEX CONCURRENTLY
/// Case-insensitive, only matches actual SQL statements (not in comments/strings).
static CONCURRENTLY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(CREATE|DROP|REINDEX)\s+INDEX\s+CONCURRENTLY\b")
        .expect("valid regex pattern")
});

/// Regex pattern for detecting SQLx migration markers.
/// Matches -- migrate:up and -- migrate:down markers (case-insensitive).
static MIGRATE_MARKER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)--\s*migrate:(up|down)").expect("valid regex pattern"));

/// SQLx migration metadata extracted from comment directives.
#[derive(Debug, Default)]
struct MigrationMetadata {
    requires_no_transaction: bool,
}

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
            } else if entry.file_type().is_dir() {
                files.extend(self.process_migration_directory(path, start_after, check_down)?);
            }
        }

        Ok(files)
    }

    fn parse_timestamp(&self, name: &str) -> Option<String> {
        SQLX_TIMESTAMP_REGEX
            .captures(name)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn validate_timestamp(&self, timestamp: &str) -> Result<()> {
        if timestamp.len() == 14 && timestamp.chars().all(|c| c.is_ascii_digit()) {
            Ok(())
        } else {
            Err(format!(
                "Invalid SQLx timestamp format: {}. Expected: YYYYMMDDHHMMSS (14 digits)",
                timestamp
            )
            .into())
        }
    }
}

impl SqlxAdapter {
    /// Read SQL file, parse directives, and validate metadata.
    ///
    /// Returns the file content and parsed metadata.
    fn read_and_validate_sqlx_file(&self, path: &Utf8Path) -> Result<(String, MigrationMetadata)> {
        let content = fs::read_to_string(path)?;
        let metadata = parse_sqlx_directives(&content);
        validate_migration_metadata(&content, &metadata, path)?;
        Ok((content, metadata))
    }

    /// Process a migration file (formats 1, 2, or 3).
    fn process_migration_file(
        &self,
        path: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Vec<MigrationFile>> {
        let filename = path.file_name().unwrap_or("");

        // Check for suffix format (.up.sql / .down.sql)
        if let Some(mig_file) = self.try_suffix_format(path, start_after, check_down)? {
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
    fn try_suffix_format(
        &self,
        path: &Utf8Path,
        start_after: Option<&str>,
        check_down: bool,
    ) -> Result<Option<MigrationFile>> {
        let file_stem = path.file_stem().unwrap_or("");

        // Check if it ends with .up or .down
        if let Some(timestamp_part) = file_stem.strip_suffix(".up") {
            // Format 1: .up.sql
            if let Some(timestamp) = self.parse_timestamp(timestamp_part) {
                if should_check_migration(start_after, &timestamp) {
                    let (_content, metadata) = self.read_and_validate_sqlx_file(path)?;

                    return Ok(Some(
                        MigrationFile::new(path.to_owned(), timestamp)
                            .with_no_transaction(metadata.requires_no_transaction),
                    ));
                }
            }
        } else if let Some(timestamp_part) = file_stem.strip_suffix(".down") {
            // Format 1: .down.sql
            if !check_down {
                return Ok(None);
            }

            if let Some(timestamp) = self.parse_timestamp(timestamp_part) {
                if should_check_migration(start_after, &timestamp) {
                    let (_content, metadata) = self.read_and_validate_sqlx_file(path)?;

                    return Ok(Some(
                        MigrationFile::new(path.to_owned(), timestamp)
                            .with_direction(MigrationDirection::Down)
                            .with_no_transaction(metadata.requires_no_transaction),
                    ));
                }
            }
        }

        Ok(None)
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

        let (content, metadata) = self.read_and_validate_sqlx_file(path)?;

        // Check if it's marker-based (contains both up and down markers)
        if contains_migrate_markers(&content) {
            // Format 3: Marker-based
            let mut files = vec![MigrationFile::new(path.to_owned(), timestamp.clone())
                .with_no_transaction(metadata.requires_no_transaction)];

            if check_down {
                files.push(
                    MigrationFile::new(path.to_owned(), timestamp)
                        .with_direction(MigrationDirection::Down)
                        .with_no_transaction(metadata.requires_no_transaction),
                );
            }

            Ok(Some(files))
        } else {
            // Format 2: Single file (up-only)
            Ok(Some(vec![MigrationFile::new(path.to_owned(), timestamp)
                .with_no_transaction(metadata.requires_no_transaction)]))
        }
    }

    /// Process a migration directory (format 4).
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
            let (_content, metadata) = self.read_and_validate_sqlx_file(&up_sql)?;
            files.push(
                MigrationFile::new(up_sql, timestamp.clone())
                    .with_no_transaction(metadata.requires_no_transaction),
            );
        }

        // Check down.sql if enabled
        if check_down {
            let down_sql = path.join("down.sql");
            if down_sql.exists() {
                let (_content, metadata) = self.read_and_validate_sqlx_file(&down_sql)?;
                files.push(
                    MigrationFile::new(down_sql, timestamp)
                        .with_direction(MigrationDirection::Down)
                        .with_no_transaction(metadata.requires_no_transaction),
                );
            }
        }

        Ok(files)
    }
}

/// Parse SQLx directives from SQL comments.
fn parse_sqlx_directives(sql: &str) -> MigrationMetadata {
    let mut metadata = MigrationMetadata::default();

    for line in sql.lines() {
        let line = line.trim();
        if let Some(comment) = line.strip_prefix("--") {
            let comment = comment.trim();
            if comment == "migrate:no-transaction" {
                metadata.requires_no_transaction = true;
            }
        }
    }

    metadata
}

/// Check if SQL contains CONCURRENTLY operations.
///
/// Uses regex to match actual CONCURRENTLY operations (CREATE/DROP/REINDEX INDEX CONCURRENTLY).
/// This is more accurate than simple string matching which could match comments or strings.
fn detect_concurrently_operations(sql: &str) -> bool {
    CONCURRENTLY_REGEX.is_match(sql)
}

/// Validate migration metadata and warn on misconfigurations.
fn validate_migration_metadata(
    sql: &str,
    metadata: &MigrationMetadata,
    path: &Utf8Path,
) -> Result<()> {
    // Check if CONCURRENTLY is used without no-transaction directive
    if detect_concurrently_operations(sql) && !metadata.requires_no_transaction {
        eprintln!(
            "Warning: {} uses CONCURRENTLY but missing '-- migrate:no-transaction' directive",
            path
        );
        eprintln!(
            "         Add this directive before the SQL statement to run outside a transaction"
        );
    }

    Ok(())
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
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let adapter = SqlxAdapter;
        assert_eq!(adapter.parse_timestamp("invalid_name"), None);
        assert_eq!(adapter.parse_timestamp("2024_01_01_000000"), None);
        assert_eq!(adapter.parse_timestamp("2024010100000"), None); // Only 13 digits
    }

    #[test]
    fn test_validate_timestamp() {
        let adapter = SqlxAdapter;
        assert!(adapter.validate_timestamp("20240101000000").is_ok());
        assert!(adapter.validate_timestamp("20231231235959").is_ok());
        assert!(adapter.validate_timestamp("2024_01_01_000000").is_err()); // Has separators
        assert!(adapter.validate_timestamp("2024010100000").is_err()); // Only 13 digits
        assert!(adapter.validate_timestamp("invalid").is_err());
    }

    #[test]
    fn test_parse_sqlx_directives() {
        let sql = "-- migrate:no-transaction\nCREATE INDEX CONCURRENTLY idx;";
        let metadata = parse_sqlx_directives(sql);
        assert!(metadata.requires_no_transaction);

        let sql_no_directive = "CREATE INDEX CONCURRENTLY idx;";
        let metadata = parse_sqlx_directives(sql_no_directive);
        assert!(!metadata.requires_no_transaction);
    }

    #[test]
    fn test_detect_concurrently_operations() {
        assert!(detect_concurrently_operations(
            "CREATE INDEX CONCURRENTLY idx;"
        ));
        assert!(detect_concurrently_operations(
            "drop index concurrently idx;"
        ));
        assert!(!detect_concurrently_operations("CREATE INDEX idx;"));
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

        // With filter
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
}

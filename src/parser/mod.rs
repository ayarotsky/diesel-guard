use crate::adapters::MigrationDirection;
use crate::error::{DieselGuardError, Result};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

pub mod comment_parser;
mod drop_index_concurrently_detector;
mod primary_key_using_index_detector;
mod unique_using_index_detector;

pub use comment_parser::IgnoreRange;

/// Parsed SQL with metadata for safety-assured handling
pub struct ParsedSql {
    pub statements: Vec<Statement>,
    pub sql: String,
    pub ignore_ranges: Vec<IgnoreRange>,
}

pub struct SqlParser {
    dialect: PostgreSqlDialect,
}

impl SqlParser {
    pub fn new() -> Self {
        Self {
            dialect: PostgreSqlDialect {},
        }
    }

    /// Parse SQL string into AST statements
    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>> {
        Parser::parse_sql(&self.dialect, sql)
            .map_err(|e| DieselGuardError::parse_error(e.to_string()))
    }

    /// Parse SQL with metadata for safety-assured blocks
    /// Handles safe patterns that sqlparser can't parse
    pub fn parse_with_metadata(&self, sql: &str) -> Result<ParsedSql> {
        // Parse ignore ranges first
        let ignore_ranges = comment_parser::CommentParser::parse_ignore_ranges(sql)?;

        // Try to parse SQL
        match self.parse(sql) {
            Ok(statements) => Ok(ParsedSql {
                statements,
                sql: sql.to_string(),
                ignore_ranges,
            }),
            Err(e) => {
                // If parsing fails, check for safe patterns that sqlparser can't handle
                if let Some(pattern_name) = Self::detect_safe_pattern(sql) {
                    Self::warn_safe_pattern_skipped(pattern_name);
                    Ok(ParsedSql {
                        statements: vec![],
                        sql: sql.to_string(),
                        ignore_ranges,
                    })
                } else {
                    // Not a known safe pattern - return the original parse error
                    Err(e)
                }
            }
        }
    }

    /// Parse SQL with migration direction (for SQLx marker-based migrations).
    ///
    /// Extracts the appropriate section (up or down) from marker-based SQLx migrations:
    /// ```sql
    /// -- migrate:up
    /// CREATE TABLE users (...);
    ///
    /// -- migrate:down
    /// DROP TABLE users;
    /// ```
    pub fn parse_sql_with_direction(
        &self,
        sql: &str,
        direction: MigrationDirection,
    ) -> Result<ParsedSql> {
        // Extract the appropriate section based on direction
        let sql_section = match direction {
            MigrationDirection::Down => extract_down_section(sql),
            MigrationDirection::Up => extract_up_section(sql),
        };

        // Parse the extracted section
        self.parse_with_metadata(sql_section)
    }

    /// Detect if SQL contains known safe patterns that sqlparser can't parse
    /// Returns the pattern name if detected
    fn detect_safe_pattern(sql: &str) -> Option<&'static str> {
        if unique_using_index_detector::contains_unique_using_index(sql) {
            Some("UNIQUE USING INDEX")
        } else if primary_key_using_index_detector::contains_primary_key_using_index(sql) {
            Some("PRIMARY KEY USING INDEX")
        } else if drop_index_concurrently_detector::contains_drop_index_concurrently(sql) {
            Some("DROP INDEX CONCURRENTLY")
        } else {
            None
        }
    }

    /// Print warning about safe pattern causing other statements to be skipped
    fn warn_safe_pattern_skipped(pattern_name: &str) {
        eprintln!(
            "Warning: SQL contains {} (safe pattern) but parser failed. \
             Other statements in this file may not be checked due to sqlparser limitations.",
            pattern_name
        );
    }
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the "up" section from SQLx marker-based migration.
///
/// Returns SQL between `-- migrate:up` and `-- migrate:down` (or EOF).
/// If no markers found, returns the entire SQL string.
/// Marker matching is case-insensitive.
fn extract_up_section(sql: &str) -> &str {
    // Case-insensitive search for migrate:up marker
    let sql_lower = sql.to_lowercase();
    if let Some(up_pos) = sql_lower.find("-- migrate:up") {
        // Find the end of the marker line (to skip the marker itself)
        let start = up_pos + "-- migrate:up".len();

        // Look for migrate:down marker after the up section
        if let Some(down_pos) = sql_lower[start..].find("-- migrate:down") {
            &sql[start..start + down_pos]
        } else {
            &sql[start..]
        }
    } else {
        // No markers, return full SQL
        sql
    }
}

/// Extract the "down" section from SQLx marker-based migration.
///
/// Returns SQL after `-- migrate:down`.
/// If no marker found, returns empty string.
/// Marker matching is case-insensitive.
fn extract_down_section(sql: &str) -> &str {
    // Case-insensitive search for migrate:down marker
    let sql_lower = sql.to_lowercase();
    if let Some(down_pos) = sql_lower.find("-- migrate:down") {
        // Use the original sql (not lowercased) for the return value
        &sql[down_pos + "-- migrate:down".len()..]
    } else {
        // No down marker, return empty
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let parser = SqlParser::new();
        let result = parser.parse("SELECT * FROM users;");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_alter_table() {
        let parser = SqlParser::new();
        let result = parser.parse("ALTER TABLE users ADD COLUMN email VARCHAR(255);");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_sql() {
        let parser = SqlParser::new();
        let result = parser.parse("INVALID SQL HERE");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_with_metadata() {
        let parser = SqlParser::new();
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 1);
        assert!(!result.sql.is_empty());
    }

    #[test]
    fn test_parse_with_metadata_no_blocks() {
        let parser = SqlParser::new();
        let sql = "ALTER TABLE users DROP COLUMN email;";

        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 0);
        assert_eq!(result.sql, sql);
    }

    #[test]
    fn test_unique_using_index_returns_empty_statements() {
        let parser = SqlParser::new();
        let sql =
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;";

        // This should succeed (not error) but return empty statements
        // because sqlparser can't parse UNIQUE USING INDEX
        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "UNIQUE USING INDEX should return empty statements"
        );
    }

    #[test]
    fn test_unique_using_index_skips_all_statements() {
        let parser = SqlParser::new();
        // This file has both UNIQUE USING INDEX (safe) and DROP COLUMN (unsafe)
        let sql = r#"
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
ALTER TABLE users DROP COLUMN old_field;
        "#;

        // Due to parser limitation, ALL statements are skipped (returns empty)
        // This test documents the limitation - the unsafe DROP COLUMN is NOT detected
        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "When UNIQUE USING INDEX causes parse failure, ALL statements are skipped"
        );
    }

    #[test]
    fn test_drop_index_concurrently_returns_empty_statements() {
        let parser = SqlParser::new();
        let sql = "DROP INDEX CONCURRENTLY idx_users_email;";

        // This should succeed (not error) but return empty statements
        // because sqlparser can't parse DROP INDEX CONCURRENTLY
        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "DROP INDEX CONCURRENTLY should return empty statements"
        );
    }

    #[test]
    fn test_drop_index_concurrently_if_exists() {
        let parser = SqlParser::new();
        let sql = "DROP INDEX CONCURRENTLY IF EXISTS idx_users_email;";

        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "DROP INDEX CONCURRENTLY IF EXISTS should return empty statements"
        );
    }

    #[test]
    fn test_drop_index_concurrently_skips_all_statements() {
        let parser = SqlParser::new();
        // This file has both DROP INDEX CONCURRENTLY (safe) and DROP COLUMN (unsafe)
        let sql = r#"
DROP INDEX CONCURRENTLY idx_users_email;
ALTER TABLE users DROP COLUMN old_field;
        "#;

        // Due to parser limitation, ALL statements are skipped (returns empty)
        // This test documents the limitation - the unsafe DROP COLUMN is NOT detected
        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "When DROP INDEX CONCURRENTLY causes parse failure, ALL statements are skipped"
        );
    }

    #[test]
    fn test_primary_key_using_index_returns_empty_statements() {
        let parser = SqlParser::new();
        let sql = "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;";

        // This should succeed (not error) but return empty statements
        // because sqlparser can't parse PRIMARY KEY USING INDEX
        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "PRIMARY KEY USING INDEX should return empty statements"
        );
    }

    #[test]
    fn test_primary_key_using_index_skips_all_statements() {
        let parser = SqlParser::new();
        // This file has both PRIMARY KEY USING INDEX (safe) and DROP COLUMN (unsafe)
        let sql = r#"
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
ALTER TABLE users DROP COLUMN old_field;
        "#;

        // Due to parser limitation, ALL statements are skipped (returns empty)
        // This test documents the limitation - the unsafe DROP COLUMN is NOT detected
        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.statements.len(),
            0,
            "When PRIMARY KEY USING INDEX causes parse failure, ALL statements are skipped"
        );
    }

    #[test]
    fn test_extract_up_section() {
        let sql = r#"-- migrate:up
CREATE TABLE users (id INT);

-- migrate:down
DROP TABLE users;"#;

        let up_section = extract_up_section(sql);
        assert!(up_section.contains("CREATE TABLE users"));
        assert!(!up_section.contains("DROP TABLE users"));
        assert!(!up_section.contains("-- migrate:down"));
    }

    #[test]
    fn test_extract_down_section() {
        let sql = r#"-- migrate:up
CREATE TABLE users (id INT);

-- migrate:down
DROP TABLE users;"#;

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
        let sql = r#"-- migrate:up
CREATE TABLE users (id INT);"#;

        let up_section = extract_up_section(sql);
        assert!(up_section.contains("CREATE TABLE users"));
    }

    #[test]
    fn test_parse_sql_with_direction_up() {
        let parser = SqlParser::new();
        let sql = r#"-- migrate:up
CREATE TABLE users (id INT);

-- migrate:down
DROP TABLE users;"#;

        let result = parser
            .parse_sql_with_direction(sql, MigrationDirection::Up)
            .unwrap();
        assert_eq!(result.statements.len(), 1);
    }

    #[test]
    fn test_parse_sql_with_direction_down() {
        let parser = SqlParser::new();
        let sql = r#"-- migrate:up
CREATE TABLE users (id INT);

-- migrate:down
DROP TABLE users;"#;

        let result = parser
            .parse_sql_with_direction(sql, MigrationDirection::Down)
            .unwrap();
        assert_eq!(result.statements.len(), 1);
    }
}

use crate::error::{DieselGuardError, Result};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

pub mod comment_parser;
mod drop_index_concurrently_detector;
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

    /// Detect if SQL contains known safe patterns that sqlparser can't parse
    /// Returns the pattern name if detected
    fn detect_safe_pattern(sql: &str) -> Option<&'static str> {
        if unique_using_index_detector::contains_unique_using_index(sql) {
            Some("UNIQUE USING INDEX")
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
}

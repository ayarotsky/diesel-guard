use crate::error::{DieselGuardError, Result};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use std::collections::HashSet;

pub mod comment_parser;
pub use comment_parser::IgnoreRange;

/// SQL keywords that typically start statements
static STATEMENT_KEYWORDS: &[&str] = &[
    "ALTER", "CREATE", "DROP", "INSERT", "UPDATE", "DELETE", "SELECT", "GRANT", "REVOKE", "SET",
    "COMMENT", "TRUNCATE", "VACUUM", "ANALYZE",
];

/// Parsed SQL with metadata for safety-assured handling
pub struct ParsedSql {
    pub statements: Vec<Statement>,
    pub statement_lines: Vec<usize>, // Line number where each statement starts
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
    pub fn parse_with_metadata(&self, sql: &str) -> Result<ParsedSql> {
        let statements = self.parse(sql)?;
        let statement_lines = Self::extract_statement_lines(sql, &statements);
        let ignore_ranges = comment_parser::CommentParser::parse_ignore_ranges(sql)?;

        Ok(ParsedSql {
            statements,
            statement_lines,
            ignore_ranges,
        })
    }

    /// Extract line numbers where statements appear in SQL
    ///
    /// This is a heuristic-based approach since sqlparser doesn't preserve source positions.
    /// It uses keyword matching to identify where statements begin in the source SQL.
    ///
    /// # How It Works
    ///
    /// 1. For each statement, determines its starting SQL keyword (ALTER, CREATE, DROP, etc.)
    /// 2. Searches through source lines (excluding already-matched lines) for the keyword
    /// 3. Returns line numbers in 1-indexed format matching editor conventions
    ///
    /// # Limitations
    ///
    /// - Falls back to line 1 if keyword matching fails (rare with standard SQL)
    /// - May mistrack SQL with multiple statements on the same line
    /// - Logs warning to stderr when fallback occurs to aid debugging
    fn extract_statement_lines(sql: &str, statements: &[Statement]) -> Vec<usize> {
        let mut line_numbers = Vec::new();
        let mut matched_lines = HashSet::new(); // O(1) lookup instead of O(n) vector scan

        for stmt in statements {
            let stmt_str = stmt.to_string().to_uppercase();
            let first_keyword = STATEMENT_KEYWORDS
                .iter()
                .find(|&kw| stmt_str.starts_with(kw))
                .unwrap_or(&"");

            // Find line number where this keyword appears
            // We need to track which lines we've already matched to handle multiple statements
            let line_result = sql
                .lines()
                .enumerate()
                .find(|(idx, line)| {
                    let line_num = idx + 1; // 1-indexed
                    let trimmed = line.trim();

                    // Skip already matched lines and comments
                    if matched_lines.contains(&line_num) || trimmed.starts_with("--") {
                        return false;
                    }

                    // Check if line starts with the statement keyword
                    trimmed.to_uppercase().starts_with(first_keyword)
                })
                .map(|(idx, _)| idx + 1); // 1-indexed

            let line_num = match line_result {
                Some(line) => line,
                None => {
                    // Fallback to line 1 - this may cause incorrect safety-assured behavior
                    eprintln!(
                        "Warning: Could not determine line number for statement (keyword: '{}'). \
                         Defaulting to line 1. This may cause safety-assured blocks to behave \
                         incorrectly for this statement.",
                        if first_keyword.is_empty() {
                            "UNKNOWN"
                        } else {
                            first_keyword
                        }
                    );
                    eprintln!(
                        "  Statement: {}",
                        stmt.to_string().chars().take(100).collect::<String>()
                    );
                    1
                }
            };

            line_numbers.push(line_num);
            matched_lines.insert(line_num); // Track matched line for O(1) lookup
        }

        line_numbers
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
        assert_eq!(result.statement_lines.len(), 1);
    }

    #[test]
    fn test_extract_statement_lines() {
        let parser = SqlParser::new();
        let sql = r#"
ALTER TABLE users DROP COLUMN email;

CREATE INDEX idx ON users(email);
        "#;

        let statements = parser.parse(sql).unwrap();
        let lines = SqlParser::extract_statement_lines(sql, &statements);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], 2); // ALTER on line 2
        assert_eq!(lines[1], 4); // CREATE on line 4
    }

    #[test]
    fn test_extract_statement_lines_with_comments() {
        let parser = SqlParser::new();
        let sql = r#"
-- This is a comment
ALTER TABLE users DROP COLUMN email;
-- Another comment
CREATE INDEX idx ON users(email);
        "#;

        let statements = parser.parse(sql).unwrap();
        let lines = SqlParser::extract_statement_lines(sql, &statements);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], 3); // ALTER on line 3 (after comment)
        assert_eq!(lines[1], 5); // CREATE on line 5 (after comment)
    }

    #[test]
    fn test_parse_with_metadata_no_blocks() {
        let parser = SqlParser::new();
        let sql = "ALTER TABLE users DROP COLUMN email;";

        let result = parser.parse_with_metadata(sql).unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 0);
    }

    #[test]
    fn test_extract_statement_lines_same_keyword_multiple_times() {
        // This is the critical edge case - multiple statements with same keyword
        // Tests that filter() correctly excludes all previously matched lines
        let parser = SqlParser::new();
        let sql = r#"
ALTER TABLE users DROP COLUMN email;

ALTER TABLE posts DROP COLUMN body;

ALTER TABLE comments DROP COLUMN author;
        "#;

        let statements = parser.parse(sql).unwrap();
        let lines = SqlParser::extract_statement_lines(sql, &statements);

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], 2); // First ALTER on line 2
        assert_eq!(lines[1], 4); // Second ALTER on line 4
        assert_eq!(lines[2], 6); // Third ALTER on line 6
    }

    #[test]
    fn test_extract_statement_lines_with_leading_whitespace() {
        let parser = SqlParser::new();
        let sql = r#"

    ALTER TABLE users DROP COLUMN email;

        CREATE INDEX idx ON users(name);
        "#;

        let statements = parser.parse(sql).unwrap();
        let lines = SqlParser::extract_statement_lines(sql, &statements);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], 3); // ALTER on line 3 (with leading spaces)
        assert_eq!(lines[1], 5); // CREATE on line 5 (with leading spaces)
    }

    #[test]
    fn test_extract_statement_lines_keyword_in_comment_then_real() {
        let parser = SqlParser::new();
        let sql = r#"
-- ALTER TABLE users ADD COLUMN test TEXT;
-- The above ALTER was commented out

ALTER TABLE users DROP COLUMN email;
        "#;

        let statements = parser.parse(sql).unwrap();
        let lines = SqlParser::extract_statement_lines(sql, &statements);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], 5); // Real ALTER on line 5, not the commented one
    }
}

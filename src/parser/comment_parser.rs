//! Parse safety-assured directives from SQL comments

use crate::error::{DieselGuardError, Result};
use derive_more::Display;
use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for matching safety-assured:start directive
/// Matches: optional whitespace, --, optional whitespace, safety-assured:start, optional whitespace
/// Case-insensitive
static START_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*--\s*safety-assured:start\s*$").unwrap());

/// Regex pattern for matching safety-assured:end directive
/// Matches: optional whitespace, --, optional whitespace, safety-assured:end, optional whitespace
/// Case-insensitive
static END_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*--\s*safety-assured:end\s*$").unwrap());

/// Represents a range of lines that should be ignored
#[derive(Debug, Clone, PartialEq, Display)]
#[display("lines {}-{}", start_line, end_line)]
pub struct IgnoreRange {
    pub start_line: usize,
    pub end_line: usize,
}

pub struct CommentParser;

impl CommentParser {
    /// Parse SQL and extract safety-assured blocks
    /// Returns: Vec<IgnoreRange> and validates matching start/end pairs
    pub fn parse_ignore_ranges(sql: &str) -> Result<Vec<IgnoreRange>> {
        let mut ranges = Vec::new();
        let mut current_start: Option<usize> = None;

        for (line_num, line) in sql.lines().enumerate() {
            let line_num = line_num + 1; // 1-indexed
            let trimmed = line.trim();

            // Match start directive
            if Self::is_start_directive(trimmed) {
                if current_start.is_some() {
                    return Err(DieselGuardError::parse_error(format!(
                        "Nested 'safety-assured:start' at line {}. Nested blocks are not supported. Close the previous block before starting a new one.",
                        line_num
                    )));
                }
                current_start = Some(line_num);
            }
            // Match end directive
            else if Self::is_end_directive(trimmed) {
                match current_start.take() {
                    Some(start_line) => {
                        ranges.push(IgnoreRange {
                            start_line,
                            end_line: line_num,
                        });
                    }
                    None => {
                        return Err(DieselGuardError::parse_error(format!(
                            "Unmatched 'safety-assured:end' at line {}. Each 'safety-assured:end' must have a matching 'safety-assured:start' before it.",
                            line_num
                        )));
                    }
                }
            }
        }

        // Check for unclosed blocks
        if let Some(start_line) = current_start {
            return Err(DieselGuardError::parse_error(format!(
                "Unclosed 'safety-assured:start' at line {}. Did you forget to add 'safety-assured:end'?",
                start_line
            )));
        }

        Ok(ranges)
    }

    /// Check if line is a start directive
    fn is_start_directive(line: &str) -> bool {
        START_DIRECTIVE.is_match(line)
    }

    /// Check if line is an end directive
    fn is_end_directive(line: &str) -> bool {
        END_DIRECTIVE.is_match(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_block() {
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let ranges = CommentParser::parse_ignore_ranges(sql).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 2);
        assert_eq!(ranges[0].end_line, 4);
    }

    #[test]
    fn test_multiple_blocks() {
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end

ALTER TABLE posts ADD COLUMN body TEXT;

-- safety-assured:start
DROP INDEX old_index;
-- safety-assured:end
        "#;

        let ranges = CommentParser::parse_ignore_ranges(sql).unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start_line, 2);
        assert_eq!(ranges[0].end_line, 4);
        assert_eq!(ranges[1].start_line, 8);
        assert_eq!(ranges[1].end_line, 10);
    }

    #[test]
    fn test_case_insensitive() {
        let sql = r#"
-- SAFETY-ASSURED:START
ALTER TABLE users DROP COLUMN email;
-- safety-ASSURED:end
        "#;

        let ranges = CommentParser::parse_ignore_ranges(sql).unwrap();
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_unmatched_end() {
        let sql = r#"
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let result = CommentParser::parse_ignore_ranges(sql);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unmatched"));
    }

    #[test]
    fn test_unclosed_start() {
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
        "#;

        let result = CommentParser::parse_ignore_ranges(sql);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unclosed"));
    }

    #[test]
    fn test_nested_blocks_error() {
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:start
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end
-- safety-assured:end
        "#;

        // Nested blocks should be rejected with clear error
        let result = CommentParser::parse_ignore_ranges(sql);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Nested 'safety-assured:start'"));
    }

    #[test]
    fn test_empty_block() {
        let sql = r#"
-- safety-assured:start
-- safety-assured:end
        "#;

        let ranges = CommentParser::parse_ignore_ranges(sql).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 2);
        assert_eq!(ranges[0].end_line, 3);
    }

    #[test]
    fn test_block_with_comments() {
        let sql = r#"
-- safety-assured:start
-- This column was deprecated
ALTER TABLE users DROP COLUMN email;
-- All references removed
-- safety-assured:end
        "#;

        let ranges = CommentParser::parse_ignore_ranges(sql).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 2);
        assert_eq!(ranges[0].end_line, 6);
    }

    #[test]
    fn test_no_blocks() {
        let sql = r#"
ALTER TABLE users DROP COLUMN email;
ALTER TABLE posts ADD COLUMN body TEXT;
        "#;

        let ranges = CommentParser::parse_ignore_ranges(sql).unwrap();
        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_directive_variations() {
        // Test different whitespace and formatting
        assert!(CommentParser::is_start_directive("-- safety-assured:start"));
        assert!(CommentParser::is_start_directive("--safety-assured:start"));
        assert!(CommentParser::is_start_directive(
            "  -- safety-assured:start  "
        ));
        assert!(CommentParser::is_start_directive("-- SAFETY-ASSURED:START"));

        // Not start directives
        assert!(!CommentParser::is_start_directive("-- safety-assured:end"));
        assert!(!CommentParser::is_start_directive("ALTER TABLE users"));
        assert!(!CommentParser::is_start_directive("-- some comment"));
    }

    #[test]
    fn test_directive_requires_exact_match() {
        // These should NOT match - no extra characters allowed
        assert!(!CommentParser::is_start_directive(
            "-- safety-assured:start111"
        ));
        assert!(!CommentParser::is_start_directive(
            "-- safety-assured:startx"
        ));
        assert!(!CommentParser::is_start_directive(
            "-- xsafety-assured:start"
        ));
        assert!(!CommentParser::is_start_directive(
            "-- safety-assured:start extra text"
        ));

        assert!(!CommentParser::is_end_directive("-- safety-assured:end222"));
        assert!(!CommentParser::is_end_directive("-- safety-assured:endx"));
        assert!(!CommentParser::is_end_directive(
            "-- safety-assured:end extra text"
        ));

        // Invalid directive should cause an error (unmatched end)
        let sql = r#"
-- safety-assured:start111
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let result = CommentParser::parse_ignore_ranges(sql);
        assert!(
            result.is_err(),
            "Invalid start directive should not be recognized"
        );
        assert!(result.unwrap_err().to_string().contains("Unmatched"));
    }
}

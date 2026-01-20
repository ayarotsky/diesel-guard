//! Detection for safe REINDEX CONCURRENTLY statements.
//!
//! PostgreSQL REINDEX syntax (for types that support CONCURRENTLY):
//!   REINDEX [ ( option [, ...] ) ] { INDEX | TABLE | SCHEMA | DATABASE } [ CONCURRENTLY ] name
//!
//! Note: REINDEX SYSTEM does not support CONCURRENTLY.

use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern to detect REINDEX with CONCURRENTLY
/// Matches: REINDEX [( options )] INDEX|TABLE|SCHEMA|DATABASE CONCURRENTLY
///
/// Note: SYSTEM is excluded because it doesn't support CONCURRENTLY
static REINDEX_CONCURRENTLY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)REINDEX\s+(?:\([^)]*\)\s+)?(INDEX|TABLE|SCHEMA|DATABASE)\s+CONCURRENTLY\s+")
        .unwrap()
});

/// Check if SQL contains REINDEX CONCURRENTLY syntax that sqlparser can't parse
pub fn contains_reindex_concurrently(sql: &str) -> bool {
    REINDEX_CONCURRENTLY_PATTERN.is_match(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_reindex_index_concurrently() {
        assert!(contains_reindex_concurrently(
            "REINDEX INDEX CONCURRENTLY idx_users_email;"
        ));
    }

    #[test]
    fn test_detects_reindex_table_concurrently() {
        assert!(contains_reindex_concurrently(
            "REINDEX TABLE CONCURRENTLY users;"
        ));
    }

    #[test]
    fn test_detects_reindex_schema_concurrently() {
        assert!(contains_reindex_concurrently(
            "REINDEX SCHEMA CONCURRENTLY public;"
        ));
    }

    #[test]
    fn test_detects_reindex_database_concurrently() {
        assert!(contains_reindex_concurrently(
            "REINDEX DATABASE CONCURRENTLY mydb;"
        ));
    }

    #[test]
    fn test_ignores_reindex_system_concurrently() {
        // REINDEX SYSTEM doesn't support CONCURRENTLY (invalid syntax)
        assert!(!contains_reindex_concurrently(
            "REINDEX SYSTEM CONCURRENTLY mydb;"
        ));
    }

    #[test]
    fn test_detects_case_insensitive() {
        assert!(contains_reindex_concurrently(
            "reindex index concurrently idx_users_email;"
        ));
        assert!(contains_reindex_concurrently(
            "Reindex Table Concurrently users;"
        ));
    }

    #[test]
    fn test_ignores_reindex_without_concurrently() {
        assert!(!contains_reindex_concurrently(
            "REINDEX INDEX idx_users_email;"
        ));
        assert!(!contains_reindex_concurrently("REINDEX TABLE users;"));
        assert!(!contains_reindex_concurrently("REINDEX SCHEMA public;"));
        assert!(!contains_reindex_concurrently("REINDEX DATABASE mydb;"));
    }

    #[test]
    fn test_ignores_other_statements() {
        assert!(!contains_reindex_concurrently(
            "CREATE INDEX idx_test ON users(email);"
        ));
        assert!(!contains_reindex_concurrently("DROP INDEX idx_test;"));
        assert!(!contains_reindex_concurrently("SELECT * FROM users;"));
    }

    #[test]
    fn test_detects_reindex_with_options_concurrently() {
        // REINDEX with parenthesized options and CONCURRENTLY (PostgreSQL 12+)
        assert!(contains_reindex_concurrently(
            "REINDEX (VERBOSE) INDEX CONCURRENTLY idx_test;"
        ));
        assert!(contains_reindex_concurrently(
            "REINDEX (VERBOSE, TABLESPACE foo) TABLE CONCURRENTLY users;"
        ));
        assert!(contains_reindex_concurrently(
            "REINDEX (TABLESPACE new_space) SCHEMA CONCURRENTLY public;"
        ));
    }

    #[test]
    fn test_ignores_reindex_with_options_without_concurrently() {
        // REINDEX with options but without CONCURRENTLY should not match
        assert!(!contains_reindex_concurrently(
            "REINDEX (VERBOSE) INDEX idx_test;"
        ));
        assert!(!contains_reindex_concurrently(
            "REINDEX (TABLESPACE foo) TABLE users;"
        ));
    }
}

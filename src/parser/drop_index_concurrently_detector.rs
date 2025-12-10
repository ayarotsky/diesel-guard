use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern to detect DROP INDEX CONCURRENTLY
static DROP_INDEX_CONCURRENTLY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)DROP\s+INDEX\s+CONCURRENTLY\s+").unwrap());

/// Check if SQL contains DROP INDEX CONCURRENTLY syntax that sqlparser can't parse
pub fn contains_drop_index_concurrently(sql: &str) -> bool {
    DROP_INDEX_CONCURRENTLY_PATTERN.is_match(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_drop_index_concurrently() {
        assert!(contains_drop_index_concurrently(
            "DROP INDEX CONCURRENTLY idx_users_email;"
        ));
    }

    #[test]
    fn test_detects_with_if_exists() {
        assert!(contains_drop_index_concurrently(
            "DROP INDEX CONCURRENTLY IF EXISTS idx_users_email;"
        ));
    }

    #[test]
    fn test_detects_case_insensitive() {
        assert!(contains_drop_index_concurrently(
            "drop index concurrently idx_users_email;"
        ));
    }

    #[test]
    fn test_ignores_regular_drop_index() {
        assert!(!contains_drop_index_concurrently(
            "DROP INDEX idx_users_email;"
        ));
    }

    #[test]
    fn test_ignores_drop_index_if_exists() {
        assert!(!contains_drop_index_concurrently(
            "DROP INDEX IF EXISTS idx_users_email;"
        ));
    }

    #[test]
    fn test_ignores_other_drop_statements() {
        assert!(!contains_drop_index_concurrently("DROP TABLE users;"));
    }
}

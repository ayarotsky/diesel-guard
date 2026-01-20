//! Detection for REINDEX statements without CONCURRENTLY.
//!
//! Since sqlparser cannot parse REINDEX statements at all, this module uses
//! regex-based detection to find unsafe REINDEX usage and extract the target info.
//!
//! PostgreSQL REINDEX syntax (for types that support CONCURRENTLY):
//!   REINDEX [ ( option [, ...] ) ] { INDEX | TABLE | SCHEMA | DATABASE } [ CONCURRENTLY ] name
//!
//! Note: REINDEX SYSTEM does not support CONCURRENTLY, so it's excluded from this detector.

use regex::Regex;
use std::sync::LazyLock;

/// Match result for a REINDEX statement without CONCURRENTLY
#[derive(Debug, Clone, PartialEq)]
pub struct ReindexMatch {
    /// The type of REINDEX (INDEX, TABLE, SCHEMA, DATABASE)
    pub reindex_type: String,
    /// The target name (index name, table name, etc.)
    pub target_name: String,
}

/// Regex pattern to detect REINDEX statements that support CONCURRENTLY
/// Matches: REINDEX [( options )] INDEX|TABLE|SCHEMA|DATABASE [CONCURRENTLY] <target>
/// We capture all and filter out CONCURRENTLY matches afterward
///
/// Note: SYSTEM is excluded because it doesn't support CONCURRENTLY
static REINDEX_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match REINDEX with optional parenthesized options, followed by type, optional CONCURRENTLY, then target
    // Group 1: type (INDEX/TABLE/SCHEMA/DATABASE)
    // Group 2: CONCURRENTLY (if present)
    // Group 3: target name
    Regex::new(r"(?i)REINDEX\s+(?:\([^)]*\)\s+)?(INDEX|TABLE|SCHEMA|DATABASE)\s+(CONCURRENTLY\s+)?([^\s;]+)")
        .unwrap()
});

/// Detect REINDEX statements without CONCURRENTLY and extract match info
///
/// Note: PostgreSQL does NOT support IF EXISTS for REINDEX. If someone writes
/// `REINDEX INDEX IF EXISTS idx_test;`, we skip it rather than misreporting
/// `IF` as the target name. PostgreSQL will error on this invalid syntax.
pub fn detect_reindex_violations(sql: &str) -> Vec<ReindexMatch> {
    REINDEX_PATTERN
        .captures_iter(sql)
        .filter_map(|cap| {
            // If CONCURRENTLY was matched (group 2), skip this match
            if cap.get(2).is_some() {
                return None;
            }

            let target = &cap[3];

            // Skip if target is "IF" - this indicates invalid IF EXISTS syntax
            // that PostgreSQL doesn't support. We skip rather than misreport.
            if target.eq_ignore_ascii_case("IF") {
                return None;
            }

            Some(ReindexMatch {
                reindex_type: cap[1].to_uppercase(),
                target_name: target.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_reindex_index() {
        let matches = detect_reindex_violations("REINDEX INDEX idx_users_email;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "INDEX");
        assert_eq!(matches[0].target_name, "idx_users_email");
    }

    #[test]
    fn test_detects_reindex_table() {
        let matches = detect_reindex_violations("REINDEX TABLE users;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "TABLE");
        assert_eq!(matches[0].target_name, "users");
    }

    #[test]
    fn test_detects_reindex_schema() {
        let matches = detect_reindex_violations("REINDEX SCHEMA public;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "SCHEMA");
        assert_eq!(matches[0].target_name, "public");
    }

    #[test]
    fn test_detects_reindex_database() {
        let matches = detect_reindex_violations("REINDEX DATABASE mydb;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "DATABASE");
        assert_eq!(matches[0].target_name, "mydb");
    }

    #[test]
    fn test_ignores_reindex_system() {
        // REINDEX SYSTEM doesn't support CONCURRENTLY, so we don't flag it
        let matches = detect_reindex_violations("REINDEX SYSTEM mydb;");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_detects_case_insensitive() {
        let matches = detect_reindex_violations("reindex index idx_test;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "INDEX");
        assert_eq!(matches[0].target_name, "idx_test");

        let matches = detect_reindex_violations("Reindex Table Users;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "TABLE");
        assert_eq!(matches[0].target_name, "Users");
    }

    #[test]
    fn test_detects_multiple_reindex() {
        let sql = r#"
            REINDEX INDEX idx_users_email;
            REINDEX TABLE posts;
            REINDEX SCHEMA public;
        "#;
        let matches = detect_reindex_violations(sql);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].reindex_type, "INDEX");
        assert_eq!(matches[0].target_name, "idx_users_email");
        assert_eq!(matches[1].reindex_type, "TABLE");
        assert_eq!(matches[1].target_name, "posts");
        assert_eq!(matches[2].reindex_type, "SCHEMA");
        assert_eq!(matches[2].target_name, "public");
    }

    #[test]
    fn test_ignores_reindex_concurrently() {
        let matches = detect_reindex_violations("REINDEX INDEX CONCURRENTLY idx_users_email;");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations("REINDEX TABLE CONCURRENTLY users;");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations("reindex schema concurrently public;");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_mixed_safe_and_unsafe() {
        let sql = r#"
            REINDEX INDEX idx_old;
            REINDEX INDEX CONCURRENTLY idx_new;
            REINDEX TABLE users;
        "#;
        let matches = detect_reindex_violations(sql);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].target_name, "idx_old");
        assert_eq!(matches[1].target_name, "users");
    }

    #[test]
    fn test_ignores_other_statements() {
        let matches = detect_reindex_violations("CREATE INDEX idx_test ON users(email);");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations("DROP INDEX idx_test;");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations("SELECT * FROM users;");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_handles_schema_qualified_names() {
        let matches = detect_reindex_violations("REINDEX INDEX public.idx_users_email;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].target_name, "public.idx_users_email");
    }

    #[test]
    fn test_ignores_invalid_if_exists_syntax() {
        // PostgreSQL does NOT support IF EXISTS for REINDEX
        // We skip these rather than misreporting "IF" as the target name
        let matches = detect_reindex_violations("REINDEX INDEX IF EXISTS idx_test;");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations("REINDEX TABLE IF EXISTS users;");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations("reindex index if exists idx_test;");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_detects_reindex_with_options() {
        // REINDEX with parenthesized options (PostgreSQL 12+)
        let matches = detect_reindex_violations("REINDEX (VERBOSE) INDEX idx_test;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "INDEX");
        assert_eq!(matches[0].target_name, "idx_test");

        let matches = detect_reindex_violations("REINDEX (VERBOSE, TABLESPACE foo) TABLE users;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "TABLE");
        assert_eq!(matches[0].target_name, "users");

        let matches = detect_reindex_violations("REINDEX (TABLESPACE new_space) SCHEMA public;");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reindex_type, "SCHEMA");
        assert_eq!(matches[0].target_name, "public");
    }

    #[test]
    fn test_ignores_reindex_with_options_and_concurrently() {
        // REINDEX with options AND CONCURRENTLY should be safe
        let matches = detect_reindex_violations("REINDEX (VERBOSE) INDEX CONCURRENTLY idx_test;");
        assert_eq!(matches.len(), 0);

        let matches = detect_reindex_violations(
            "REINDEX (VERBOSE, TABLESPACE foo) TABLE CONCURRENTLY users;",
        );
        assert_eq!(matches.len(), 0);
    }
}

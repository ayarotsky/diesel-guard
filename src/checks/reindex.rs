//! Detection for REINDEX without CONCURRENTLY.
//!
//! This check identifies `REINDEX` statements that don't use the CONCURRENTLY
//! option, which blocks all operations during the reindex process.
//!
//! REINDEX without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the table,
//! blocking all operations (SELECT, INSERT, UPDATE, DELETE) until the reindex
//! completes. Duration depends on index size.
//!
//! Using CONCURRENTLY (PostgreSQL 12+) allows the index to be rebuilt while
//! permitting concurrent queries, though it takes longer and cannot be run
//! inside a transaction block.
//!
//! **Important:** Since sqlparser cannot parse REINDEX statements at all,
//! actual detection happens via raw SQL pattern matching in the SafetyChecker,
//! not in this check. This struct exists solely to enable configuration-based
//! disabling via `disable_checks = ["ReindexCheck"]`.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::Statement;

/// Stub check for REINDEX without CONCURRENTLY.
///
/// Actual detection is performed via raw SQL pattern matching in the SafetyChecker
/// because sqlparser cannot parse REINDEX statements. This struct enables the check
/// to be disabled via configuration.
pub struct ReindexCheck;

impl Check for ReindexCheck {
    fn check(&self, _stmt: &Statement) -> Vec<Violation> {
        // Actual detection happens in SafetyChecker via raw SQL pattern matching
        // because sqlparser cannot parse REINDEX statements at all.
        // This stub exists only to enable configuration-based disabling.
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_allows;

    // Note: These tests verify the stub behavior. The actual REINDEX detection
    // is tested in safety_checker.rs since it uses raw SQL pattern matching.

    #[test]
    fn test_stub_returns_empty() {
        // The ReindexCheck stub should always return empty since actual
        // detection happens via raw SQL matching
        assert_allows!(ReindexCheck, "SELECT 1;");
    }

    #[test]
    fn test_stub_ignores_other_statements() {
        assert_allows!(ReindexCheck, "CREATE INDEX idx_test ON users(email);");
        assert_allows!(ReindexCheck, "DROP INDEX idx_test;");
        assert_allows!(ReindexCheck, "ALTER TABLE users ADD COLUMN email TEXT;");
    }
}

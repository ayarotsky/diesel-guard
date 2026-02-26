//! Detection for REINDEX without CONCURRENTLY.
//!
//! This check identifies `REINDEX` statements that don't use the CONCURRENTLY
//! option, which blocks all operations during the reindex process.
//!
//! REINDEX without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the table,
//! blocking all operations (SELECT, INSERT, UPDATE, DELETE) until the reindex
//! completes. Duration depends on index size.
//!
//! Using CONCURRENTLY (Postgres 12+) allows the index to be rebuilt while
//! permitting concurrent queries, though it takes longer and cannot be run
//! inside a transaction block.

use crate::checks::pg_helpers::{Node, NodeEnum};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct ReindexCheck;

/// Map reindex object to its SQL keyword.
/// Values from pg_query protobuf ReindexObjectType enum.
fn reindex_object(object: i32) -> Option<&'static str> {
    match object {
        1 => Some("INDEX"),
        2 => Some("TABLE"),
        3 => Some("SCHEMA"),
        5 => Some("DATABASE"),
        // object=4 is SYSTEM, which doesn't support CONCURRENTLY -- skip it
        _ => None,
    }
}

/// Check if CONCURRENTLY is present in the REINDEX params.
fn has_concurrently(params: &[Node]) -> bool {
    params
        .iter()
        .any(|p| matches!(&p.node, Some(NodeEnum::DefElem(elem)) if elem.defname == "concurrently"))
}

impl Check for ReindexCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let NodeEnum::ReindexStmt(reindex) = node else {
            return vec![];
        };

        let Some(object) = reindex_object(reindex.kind) else {
            // SYSTEM (kind=4) or unknown -- skip
            return vec![];
        };

        if has_concurrently(&reindex.params) {
            return vec![];
        }

        // Determine target name based on kind
        let target = match reindex.kind {
            1 | 2 => {
                // INDEX or TABLE: use relation
                reindex
                    .relation
                    .as_ref()
                    .map(|rv| rv.relname.clone())
                    .unwrap_or_default()
            }
            3 | 5 => {
                // SCHEMA or DATABASE: use name field
                reindex.name.clone()
            }
            _ => String::new(),
        };

        vec![Violation::new(
            "REINDEX without CONCURRENTLY",
            format!(
                "REINDEX {object} '{target}' without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock, \
                blocking all operations on the {object} '{target}' until complete. Duration depends on index size.",
            ),
            format!(
                r#"Use REINDEX CONCURRENTLY for lock-free reindexing (Postgres 12+):

   REINDEX {object} CONCURRENTLY {target};

Note: CONCURRENTLY requires Postgres 12+ and cannot be run inside a transaction block.

For Diesel migrations:
1. Create metadata.toml in your migration directory:
   run_in_transaction = false

2. Use REINDEX CONCURRENTLY in your up.sql:
   REINDEX {object} CONCURRENTLY {target};

For SQLx migrations:
1. Add the no-transaction directive at the top of your migration file:
   -- no-transaction

2. Use REINDEX CONCURRENTLY:
   REINDEX {object} CONCURRENTLY {target};

Considerations:
- Takes longer to complete than regular REINDEX
- Allows concurrent read/write operations
- If it fails, the index may be left in "invalid" state and need manual cleanup
- Cannot be rolled back (no transaction support)"#,
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation, assert_detects_violation_containing};

    #[test]
    fn test_detects_reindex_index() {
        assert_detects_violation!(
            ReindexCheck,
            "REINDEX INDEX idx_users_email;",
            "REINDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_reindex_table() {
        assert_detects_violation!(
            ReindexCheck,
            "REINDEX TABLE users;",
            "REINDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_reindex_schema() {
        assert_detects_violation!(
            ReindexCheck,
            "REINDEX SCHEMA public;",
            "REINDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_reindex_database() {
        assert_detects_violation!(
            ReindexCheck,
            "REINDEX DATABASE mydb;",
            "REINDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_allows_reindex_index_concurrently() {
        assert_allows!(ReindexCheck, "REINDEX INDEX CONCURRENTLY idx_users_email;");
    }

    #[test]
    fn test_allows_reindex_table_concurrently() {
        assert_allows!(ReindexCheck, "REINDEX TABLE CONCURRENTLY users;");
    }

    #[test]
    fn test_reindex_violation_contains_target_name() {
        assert_detects_violation_containing!(
            ReindexCheck,
            "REINDEX INDEX idx_users_email;",
            "REINDEX without CONCURRENTLY",
            "idx_users_email",
            "INDEX"
        );
    }

    #[test]
    fn test_reindex_table_violation_contains_table_name() {
        assert_detects_violation_containing!(
            ReindexCheck,
            "REINDEX TABLE users;",
            "REINDEX without CONCURRENTLY",
            "users",
            "TABLE"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(ReindexCheck, "CREATE INDEX idx_test ON users(email);");
        assert_allows!(ReindexCheck, "DROP INDEX idx_test;");
        assert_allows!(ReindexCheck, "ALTER TABLE users ADD COLUMN email TEXT;");
    }
}

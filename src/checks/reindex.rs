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

use crate::checks::pg_helpers::{Node, NodeEnum};
use crate::checks::Check;
use crate::violation::Violation;

pub struct ReindexCheck;

/// Map reindex kind to the SQL type name string.
/// Values from pg_query protobuf ReindexObjectType enum.
fn reindex_type_name(kind: i32) -> Option<&'static str> {
    match kind {
        1 => Some("INDEX"),
        2 => Some("TABLE"),
        3 => Some("SCHEMA"),
        5 => Some("DATABASE"),
        // kind=4 is SYSTEM, which doesn't support CONCURRENTLY -- skip it
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
    fn check(&self, node: &NodeEnum) -> Vec<Violation> {
        let NodeEnum::ReindexStmt(reindex) = node else {
            return vec![];
        };

        let Some(type_name) = reindex_type_name(reindex.kind) else {
            // SYSTEM (kind=4) or unknown -- skip
            return vec![];
        };

        if has_concurrently(&reindex.params) {
            return vec![];
        }

        // Determine target name based on kind
        let target_name = match reindex.kind {
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

        let target_desc = format!("{} '{}'", type_name.to_lowercase(), target_name);

        vec![Violation::new(
            "REINDEX without CONCURRENTLY",
            format!(
                "REINDEX {type} '{target}' without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock, \
                blocking all operations on the {target_desc} until complete. Duration depends on index size.",
                r#type = type_name,
                target = target_name,
                target_desc = target_desc
            ),
            format!(r#"Use REINDEX CONCURRENTLY for lock-free reindexing (PostgreSQL 12+):

   REINDEX {type} CONCURRENTLY {target};

Note: CONCURRENTLY requires PostgreSQL 12+ and cannot be run inside a transaction block.

For Diesel migrations:
1. Create metadata.toml in your migration directory:
   run_in_transaction = false

2. Use REINDEX CONCURRENTLY in your up.sql:
   REINDEX {type} CONCURRENTLY {target};

For SQLx migrations:
1. Add the no-transaction directive at the top of your migration file:
   -- no-transaction

2. Use REINDEX CONCURRENTLY:
   REINDEX {type} CONCURRENTLY {target};

Considerations:
- Takes longer to complete than regular REINDEX
- Allows concurrent read/write operations
- If it fails, the index may be left in "invalid" state and need manual cleanup
- Cannot be rolled back (no transaction support)"#,
                r#type = type_name,
                target = target_name
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
    use crate::{assert_allows, assert_detects_violation};

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
        let stmt = parse_sql("REINDEX INDEX idx_users_email;");
        let violations = ReindexCheck.check(&stmt);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("idx_users_email"));
        assert!(violations[0].problem.contains("INDEX"));
    }

    #[test]
    fn test_reindex_table_violation_contains_table_name() {
        let stmt = parse_sql("REINDEX TABLE users;");
        let violations = ReindexCheck.check(&stmt);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("users"));
        assert!(violations[0].problem.contains("TABLE"));
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(ReindexCheck, "CREATE INDEX idx_test ON users(email);");
        assert_allows!(ReindexCheck, "DROP INDEX idx_test;");
        assert_allows!(ReindexCheck, "ALTER TABLE users ADD COLUMN email TEXT;");
    }
}

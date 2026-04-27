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

use crate::checks::pg_helpers::{Node, NodeEnum, concurrent_safe_alternative};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
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
    fn describe(&self) -> Vec<CheckDescription> {
        vec![
            CheckDescription {
                operation: "REINDEX without CONCURRENTLY".into(),
                problem: "REINDEX <object> '<target>' without CONCURRENTLY acquires an ACCESS \
                          EXCLUSIVE lock, blocking all operations on the <object> '<target>' until \
                          complete. Duration depends on index size."
                    .into(),
                safe_alternative:
                    "Use REINDEX CONCURRENTLY (Postgres 12+) to rebuild the index without \
                                   blocking reads or writes (cannot run inside a transaction)."
                        .into(),
                script_path: None,
            },
            CheckDescription {
                operation: "REINDEX CONCURRENTLY inside a transaction".into(),
                problem: "REINDEX <object> CONCURRENTLY '<target>' cannot run inside a transaction \
                          block. PostgreSQL will raise an error at runtime."
                    .into(),
                safe_alternative:
                    "Disable transaction wrapping for this migration (see your framework's documentation)."
                        .into(),
                script_path: None,
            },
        ]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, ctx: &MigrationContext) -> Vec<Violation> {
        let descriptions = self.describe();
        let NodeEnum::ReindexStmt(reindex) = node else {
            return vec![];
        };

        let Some(object) = reindex_object(reindex.kind) else {
            // SYSTEM (kind=4) or unknown -- skip
            return vec![];
        };

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

        if !has_concurrently(&reindex.params) {
            // REINDEX without CONCURRENTLY — always a violation
            let suggestion = format!(
                r#"Use REINDEX CONCURRENTLY for lock-free reindexing (Postgres 12+):

   REINDEX {object} CONCURRENTLY {target};

Note: CONCURRENTLY requires Postgres 12+.

Considerations:
- Takes longer to complete than regular REINDEX
- Allows concurrent read/write operations
- If it fails, the index may be left in "invalid" state and need manual cleanup
- Cannot be rolled back (no transaction support)"#,
            );

            let safe_alternative = concurrent_safe_alternative(suggestion, ctx);

            return vec![Violation::new(
                descriptions[0].operation.clone(),
                descriptions[0]
                    .problem
                    .replace("<object>", object)
                    .replace("<target>", &target),
                safe_alternative,
            )];
        }

        // REINDEX CONCURRENTLY — safe only if migration runs outside a transaction
        if !ctx.run_in_transaction {
            return vec![];
        }

        // REINDEX CONCURRENTLY inside a transaction — PostgreSQL will error at runtime
        vec![Violation::new(
            descriptions[1].operation.clone(),
            descriptions[1]
                .problem
                .replace("<object>", object)
                .replace("<target>", &target),
            ctx.no_transaction_hint,
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
    use crate::{
        assert_allows_with_context, assert_detects_violation, assert_detects_violation_containing,
        assert_detects_violation_with_context,
    };

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
    fn test_allows_reindex_index_concurrently_outside_transaction() {
        assert_allows_with_context!(
            ReindexCheck,
            "REINDEX INDEX CONCURRENTLY idx_users_email;",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_allows_reindex_table_concurrently_outside_transaction() {
        assert_allows_with_context!(
            ReindexCheck,
            "REINDEX TABLE CONCURRENTLY users;",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_detects_concurrent_in_transaction() {
        assert_detects_violation_with_context!(
            ReindexCheck,
            "REINDEX INDEX CONCURRENTLY idx_users_email;",
            "REINDEX CONCURRENTLY inside a transaction",
            MigrationContext {
                run_in_transaction: true,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_allows_concurrent_outside_transaction() {
        assert_allows_with_context!(
            ReindexCheck,
            "REINDEX INDEX CONCURRENTLY idx_users_email;",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
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
    fn test_safe_alternative_includes_transaction_hint_when_in_transaction() {
        let stmt = parse_sql("REINDEX INDEX idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                no_transaction_hint: "Create `metadata.toml` with `run_in_transaction = false`.",
            },
        );
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0]
                .safe_alternative
                .contains("Create `metadata.toml` with `run_in_transaction = false`."),
            "Expected transaction hint in safe_alternative"
        );
        assert!(
            violations[0]
                .safe_alternative
                .contains("CONCURRENTLY cannot run inside a transaction block"),
            "Expected transaction note in safe_alternative"
        );
    }

    #[test]
    fn test_safe_alternative_omits_transaction_hint_when_outside_transaction() {
        let stmt = parse_sql("REINDEX INDEX idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: false,
                no_transaction_hint: "Create `metadata.toml` with `run_in_transaction = false`.",
            },
        );
        assert_eq!(violations.len(), 1);
        assert!(
            !violations[0]
                .safe_alternative
                .contains("Create `metadata.toml` with `run_in_transaction = false`."),
            "Expected no transaction hint in safe_alternative"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        let ctx = MigrationContext::default();
        let config = Config::default();

        let stmt = parse_sql("CREATE INDEX idx_test ON users(email);");
        assert_eq!(ReindexCheck.check(&stmt, &config, &ctx).len(), 0);

        let stmt = parse_sql("DROP INDEX idx_test;");
        assert_eq!(ReindexCheck.check(&stmt, &config, &ctx).len(), 0);

        let stmt = parse_sql("ALTER TABLE users ADD COLUMN email TEXT;");
        assert_eq!(ReindexCheck.check(&stmt, &config, &ctx).len(), 0);
    }

    #[test]
    fn test_sqlx_framework_safe_alternative_message() {
        let stmt = parse_sql("REINDEX INDEX CONCURRENTLY idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                no_transaction_hint: "Add `-- no-transaction` as the first line of the migration file.",
            },
        );
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].safe_alternative.contains("-- no-transaction"),
            "Expected SQLx safe alternative message"
        );
    }

    #[test]
    fn test_diesel_framework_safe_alternative_message() {
        let stmt = parse_sql("REINDEX INDEX CONCURRENTLY idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                no_transaction_hint: "Create `metadata.toml` in the migration directory with `run_in_transaction = false`.",
            },
        );
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].safe_alternative.contains("metadata.toml"),
            "Expected Diesel safe alternative message"
        );
    }
}

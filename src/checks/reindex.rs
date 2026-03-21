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
use crate::checks::{Check, Config, MigrationContext};
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
    fn check(&self, node: &NodeEnum, _config: &Config, ctx: &MigrationContext) -> Vec<Violation> {
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

            let safe_alternative = if ctx.run_in_transaction {
                format!(
                    "{suggestion}\n\nNote: CONCURRENTLY cannot run inside a transaction block.\n{hint}",
                    hint = ctx.no_transaction_hint
                )
            } else {
                suggestion
            };

            return vec![Violation::new(
                "REINDEX without CONCURRENTLY",
                format!(
                    "REINDEX {object} '{target}' without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock, \
                    blocking all operations on the {object} '{target}' until complete. Duration depends on index size.",
                ),
                safe_alternative,
            )];
        }

        // REINDEX CONCURRENTLY — safe only if migration runs outside a transaction
        if !ctx.run_in_transaction {
            return vec![];
        }

        // REINDEX CONCURRENTLY inside a transaction — PostgreSQL will error at runtime
        vec![Violation::new(
            "REINDEX CONCURRENTLY inside a transaction",
            format!(
                "REINDEX {object} CONCURRENTLY '{target}' cannot run inside a transaction block. \
                PostgreSQL will raise an error at runtime.",
            ),
            ctx.no_transaction_hint,
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
    use crate::{assert_detects_violation, assert_detects_violation_containing};

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
        let stmt = parse_sql("REINDEX INDEX CONCURRENTLY idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            },
        );
        assert_eq!(
            violations.len(),
            0,
            "Expected no violations outside transaction"
        );
    }

    #[test]
    fn test_allows_reindex_table_concurrently_outside_transaction() {
        let stmt = parse_sql("REINDEX TABLE CONCURRENTLY users;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            },
        );
        assert_eq!(
            violations.len(),
            0,
            "Expected no violations outside transaction"
        );
    }

    #[test]
    fn test_detects_concurrent_in_transaction() {
        let stmt = parse_sql("REINDEX INDEX CONCURRENTLY idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                ..MigrationContext::default()
            },
        );
        assert_eq!(violations.len(), 1, "Expected 1 violation");
        assert_eq!(
            violations[0].operation,
            "REINDEX CONCURRENTLY inside a transaction"
        );
    }

    #[test]
    fn test_allows_concurrent_outside_transaction() {
        let stmt = parse_sql("REINDEX INDEX CONCURRENTLY idx_users_email;");
        let violations = ReindexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            },
        );
        assert_eq!(violations.len(), 0, "Expected no violations");
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

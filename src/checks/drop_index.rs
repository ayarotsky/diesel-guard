//! Detection for DROP INDEX without CONCURRENTLY.
//!
//! This check identifies `DROP INDEX` statements that don't use the CONCURRENTLY
//! option, which blocks queries during the index removal.
//!
//! Dropping an index without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the
//! table, which blocks all queries (SELECT, INSERT, UPDATE, DELETE) until the drop
//! operation completes. Duration depends on system load and concurrent transactions.
//!
//! Using CONCURRENTLY (Postgres 9.2+) allows the index to be dropped while permitting
//! concurrent queries, though it takes longer and cannot be run inside a transaction block.

use crate::checks::pg_helpers::{NodeEnum, ObjectType, drop_object_names};
use crate::checks::{Check, Config, MigrationContext, if_exists_clause};
use crate::violation::Violation;

pub struct DropIndexCheck;

impl Check for DropIndexCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::DropStmt(drop_stmt) = node else {
            return vec![];
        };

        if drop_stmt.remove_type != ObjectType::ObjectIndex as i32 {
            return vec![];
        }

        let if_exists_str = if_exists_clause(drop_stmt.missing_ok);

        if !drop_stmt.concurrent {
            // DROP INDEX without CONCURRENTLY — always a violation
            return drop_object_names(&drop_stmt.objects)
                .into_iter()
                .map(|name| {
                    let suggestion = format!(
                        r#"Use CONCURRENTLY to drop the index without blocking queries:
   DROP INDEX CONCURRENTLY{if_exists_str} {name};

Note: CONCURRENTLY requires Postgres 9.2+.

Considerations:
- Takes longer to complete than regular DROP INDEX
- Allows concurrent SELECT, INSERT, UPDATE, DELETE operations
- If it fails, the index may be marked "invalid" and should be dropped again
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

                    Violation::new(
                        "DROP INDEX without CONCURRENTLY",
                        format!(
                            "Dropping index '{name}'{if_exists_str} without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock, blocking all \
                            queries (SELECT, INSERT, UPDATE, DELETE) on the table until complete. Duration depends on system load and concurrent transactions."
                        ),
                        safe_alternative,
                    )
                })
                .collect();
        }

        // DROP INDEX CONCURRENTLY — safe only if migration runs outside a transaction
        if !ctx.run_in_transaction {
            return vec![];
        }

        // DROP INDEX CONCURRENTLY inside a transaction — PostgreSQL will error at runtime
        drop_object_names(&drop_stmt.objects)
            .into_iter()
            .map(|name| {
                Violation::new(
                    "DROP INDEX CONCURRENTLY inside a transaction",
                    format!(
                        "Dropping index '{name}'{if_exists_str} with CONCURRENTLY cannot run inside a transaction block. \
                        PostgreSQL will raise an error at runtime."
                    ),
                    ctx.no_transaction_hint,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
    use crate::{assert_allows, assert_detects_n_violations, assert_detects_violation};

    #[test]
    fn test_detects_drop_index() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX idx_users_email;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_if_exists() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX IF EXISTS idx_users_email;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_cascade() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX idx_users_email CASCADE;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_restrict() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX idx_users_email RESTRICT;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_multiple_indexes() {
        assert_detects_n_violations!(
            DropIndexCheck,
            "DROP INDEX idx1, idx2, idx3;",
            3,
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_if_exists_cascade() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX IF EXISTS idx_users_email CASCADE;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_allows_drop_index_concurrently_outside_transaction() {
        let stmt = parse_sql("DROP INDEX CONCURRENTLY idx_users_email;");
        let violations = DropIndexCheck.check(
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
        let stmt = parse_sql("DROP INDEX CONCURRENTLY idx_users_email;");
        let violations = DropIndexCheck.check(
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
            "DROP INDEX CONCURRENTLY inside a transaction"
        );
    }

    #[test]
    fn test_allows_concurrent_outside_transaction() {
        let stmt = parse_sql("DROP INDEX CONCURRENTLY idx_users_email;");
        let violations = DropIndexCheck.check(
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
    fn test_safe_alternative_includes_transaction_hint_when_in_transaction() {
        let stmt = parse_sql("DROP INDEX idx_users_email;");
        let violations = DropIndexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                no_transaction_hint: "Create `metadata.toml` with `run_in_transaction = false`.",
                ..Default::default()
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
        let stmt = parse_sql("DROP INDEX idx_users_email;");
        let violations = DropIndexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: false,
                no_transaction_hint: "Create `metadata.toml` with `run_in_transaction = false`.",
                ..Default::default()
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
    fn test_ignores_other_drop_statements() {
        assert_allows!(DropIndexCheck, "DROP TABLE users;");
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            DropIndexCheck,
            "CREATE INDEX idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_sqlx_framework_safe_alternative_message() {
        let stmt = parse_sql("DROP INDEX CONCURRENTLY idx_users_email;");
        let violations = DropIndexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                no_transaction_hint: "Add `-- no-transaction` as the first line of the migration file.",
                ..Default::default()
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
        let stmt = parse_sql("DROP INDEX CONCURRENTLY idx_users_email;");
        let violations = DropIndexCheck.check(
            &stmt,
            &Config::default(),
            &MigrationContext {
                run_in_transaction: true,
                no_transaction_hint: "Create `metadata.toml` in the migration directory with `run_in_transaction = false`.",
                ..Default::default()
            },
        );
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].safe_alternative.contains("metadata.toml"),
            "Expected Diesel safe alternative message"
        );
    }
}

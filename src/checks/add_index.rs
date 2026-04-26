//! Detection for CREATE INDEX without CONCURRENTLY.
//!
//! This check identifies `CREATE INDEX` statements that don't use the CONCURRENTLY
//! option, which blocks write operations during the index build.
//!
//! Creating an index without CONCURRENTLY acquires a SHARE lock on the table, which
//! blocks write operations (INSERT, UPDATE, DELETE). Duration depends on table size.
//! Reads (SELECT) are still allowed.
//!
//! Using CONCURRENTLY allows the index to be built while permitting concurrent writes,
//! though it takes longer and cannot be run inside a transaction block.

use crate::checks::pg_helpers::{NodeEnum, concurrent_safe_alternative, range_var_name};
use crate::checks::{Check, CheckDescription, Config, MigrationContext, unique_prefix};
use crate::violation::Violation;

pub struct AddIndexCheck;

impl Check for AddIndexCheck {
    fn describe(&self) -> CheckDescription {
        CheckDescription {
            operation: "ADD INDEX without CONCURRENTLY".into(),
            problem:
                "Creating an index without CONCURRENTLY acquires a SHARE lock, blocking writes \
                      (INSERT, UPDATE, DELETE) for the duration of the index build."
                    .into(),
            safe_alternative:
                "Use CREATE INDEX CONCURRENTLY to build the index without blocking writes. \
                               Note: CONCURRENTLY cannot run inside a transaction block."
                    .into(),
            script_path: None,
        }
    }

    fn check(&self, node: &NodeEnum, _config: &Config, ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::IndexStmt(index_stmt) = node else {
            return vec![];
        };

        let table_name = index_stmt
            .relation
            .as_ref()
            .map(range_var_name)
            .unwrap_or_default();
        let index_name = if index_stmt.idxname.is_empty() {
            "<unnamed>".to_string()
        } else {
            index_stmt.idxname.clone()
        };
        let unique_str = unique_prefix(index_stmt.unique);

        if !index_stmt.concurrent {
            // CREATE INDEX without CONCURRENTLY — always a violation
            let suggestion = format!(
                r#"Use CONCURRENTLY to build the index without blocking writes:
   CREATE {unique_str}INDEX CONCURRENTLY {index_name} ON {table_name};

Note: CONCURRENTLY takes longer and uses more resources, but allows concurrent INSERT, UPDATE, and DELETE operations. The index build may fail if there are deadlocks or unique constraint violations.

Considerations:
- Requires more total work and takes longer to complete
- If it fails, it leaves behind an "invalid" index that should be dropped"#,
            );

            let safe_alternative = concurrent_safe_alternative(suggestion, ctx);

            return vec![Violation::new(
                self.describe().operation,
                format!(
                    "Creating {unique_str}index '{index_name}' on table '{table_name}' without CONCURRENTLY acquires a SHARE lock, blocking writes \
                    (INSERT, UPDATE, DELETE). Duration depends on table size. Reads are still allowed."
                ),
                safe_alternative,
            )];
        }

        // CREATE INDEX CONCURRENTLY — safe only if migration runs outside a transaction
        if !ctx.run_in_transaction {
            return vec![];
        }

        // CREATE INDEX CONCURRENTLY inside a transaction — PostgreSQL will error at runtime
        vec![Violation::new(
            "CREATE INDEX CONCURRENTLY inside a transaction",
            format!(
                "Creating {unique_str}index '{index_name}' on table '{table_name}' with CONCURRENTLY cannot run inside a transaction block. \
                PostgreSQL will raise an error at runtime."
            ),
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
    fn test_detects_create_index_without_concurrently() {
        assert_detects_violation!(
            AddIndexCheck,
            "CREATE INDEX idx_users_email ON users(email);",
            "ADD INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_create_unique_index_without_concurrently() {
        assert_detects_violation_containing!(
            AddIndexCheck,
            "CREATE UNIQUE INDEX idx_users_email ON users(email);",
            "ADD INDEX without CONCURRENTLY",
            "UNIQUE"
        );
    }

    #[test]
    fn test_allows_create_index_with_concurrently_outside_transaction() {
        assert_allows_with_context!(
            AddIndexCheck,
            "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_allows_create_unique_index_with_concurrently_outside_transaction() {
        assert_allows_with_context!(
            AddIndexCheck,
            "CREATE UNIQUE INDEX CONCURRENTLY idx_users_email ON users(email);",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_detects_concurrent_in_transaction() {
        assert_detects_violation_with_context!(
            AddIndexCheck,
            "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
            "CREATE INDEX CONCURRENTLY inside a transaction",
            MigrationContext {
                run_in_transaction: true,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_allows_concurrent_outside_transaction() {
        assert_allows_with_context!(
            AddIndexCheck,
            "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        let stmt = parse_sql("CREATE TABLE users (id SERIAL PRIMARY KEY);");
        let violations =
            AddIndexCheck.check(&stmt, &Config::default(), &MigrationContext::default());
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_safe_alternative_includes_transaction_hint_when_in_transaction() {
        let stmt = parse_sql("CREATE INDEX idx_users_email ON users(email);");
        let violations = AddIndexCheck.check(
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
        let stmt = parse_sql("CREATE INDEX idx_users_email ON users(email);");
        let violations = AddIndexCheck.check(
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
    fn test_sqlx_framework_safe_alternative_message() {
        let stmt = parse_sql("CREATE INDEX CONCURRENTLY idx_users_email ON users(email);");
        let violations = AddIndexCheck.check(
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
        let stmt = parse_sql("CREATE INDEX CONCURRENTLY idx_users_email ON users(email);");
        let violations = AddIndexCheck.check(
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

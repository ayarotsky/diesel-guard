//! Detection for REFRESH MATERIALIZED VIEW without CONCURRENTLY.
//!
//! This check identifies `REFRESH MATERIALIZED VIEW` statements that don't use the CONCURRENTLY
//! option, which blocks all reads on the view for the duration of the refresh.
//!
//! Refreshing a materialized view without CONCURRENTLY acquires an AccessExclusiveLock on the view,
//! which blocks all reads (SELECT) until the refresh completes. Duration depends on view complexity
//! and underlying data size.
//!
//! Using CONCURRENTLY allows the view to be refreshed while permitting concurrent reads,
//! though it takes longer, requires a unique index on the view, and cannot run inside a
//! transaction block.

use crate::checks::pg_helpers::{NodeEnum, concurrent_safe_alternative, range_var_name};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct RefreshMatViewCheck;

impl Check for RefreshMatViewCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![
            CheckDescription {
                operation: "REFRESH MATERIALIZED VIEW without CONCURRENTLY".into(),
                problem: "Refreshing materialized view '<view>' without CONCURRENTLY acquires an \
                          AccessExclusiveLock, blocking all reads (SELECT) for the duration of the refresh. \
                          Duration depends on view complexity and underlying data size.".into(),
                safe_alternative: "Use REFRESH MATERIALIZED VIEW CONCURRENTLY (requires a unique index on the view, \
                                   cannot run inside a transaction).".into(),
                script_path: None,
            },
            CheckDescription {
                operation: "REFRESH MATERIALIZED VIEW CONCURRENTLY inside a transaction".into(),
                problem: "Refreshing materialized view '<view>' with CONCURRENTLY cannot run inside a \
                          transaction block. PostgreSQL will raise an error at runtime."
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
        let NodeEnum::RefreshMatViewStmt(stmt) = node else {
            return vec![];
        };

        let view_name = stmt
            .relation
            .as_ref()
            .map(range_var_name)
            .unwrap_or_default();

        if !stmt.concurrent {
            // REFRESH MATERIALIZED VIEW without CONCURRENTLY — always a violation
            let suggestion = format!(
                r#"Use CONCURRENTLY to refresh the view without blocking reads:
   REFRESH MATERIALIZED VIEW CONCURRENTLY {view_name};

Note: CONCURRENTLY takes longer and requires a unique index on the materialized view. Without a unique index, PostgreSQL will refuse the CONCURRENTLY option.

Considerations:
- Requires a unique index on the view (e.g. CREATE UNIQUE INDEX ON {view_name}(id);)
- Takes longer to complete than a non-concurrent refresh
- If it fails, the view data remains unchanged — no "partial" refresh state"#,
            );

            let safe_alternative = concurrent_safe_alternative(suggestion, ctx);

            return vec![Violation::new(
                descriptions[0].operation.clone(),
                descriptions[0].problem.replace("<view>", &view_name),
                safe_alternative,
            )];
        }

        // REFRESH MATERIALIZED VIEW CONCURRENTLY — safe only if migration runs outside a transaction
        if !ctx.run_in_transaction {
            return vec![];
        }

        // REFRESH MATERIALIZED VIEW CONCURRENTLY inside a transaction — PostgreSQL will error at runtime
        vec![Violation::new(
            descriptions[1].operation.clone(),
            descriptions[1].problem.replace("<view>", &view_name),
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
    fn test_detects_refresh_without_concurrently() {
        assert_detects_violation!(
            RefreshMatViewCheck,
            "REFRESH MATERIALIZED VIEW my_view;",
            "REFRESH MATERIALIZED VIEW without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_refresh_without_concurrently_mentions_view_name() {
        assert_detects_violation_containing!(
            RefreshMatViewCheck,
            "REFRESH MATERIALIZED VIEW my_view;",
            "REFRESH MATERIALIZED VIEW without CONCURRENTLY",
            "my_view"
        );
    }

    #[test]
    fn test_allows_refresh_with_concurrently_outside_transaction() {
        assert_allows_with_context!(
            RefreshMatViewCheck,
            "REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;",
            MigrationContext {
                run_in_transaction: false,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_detects_concurrent_in_transaction() {
        assert_detects_violation_with_context!(
            RefreshMatViewCheck,
            "REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;",
            "REFRESH MATERIALIZED VIEW CONCURRENTLY inside a transaction",
            MigrationContext {
                run_in_transaction: true,
                ..MigrationContext::default()
            }
        );
    }

    #[test]
    fn test_allows_concurrent_outside_transaction() {
        assert_allows_with_context!(
            RefreshMatViewCheck,
            "REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;",
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
            RefreshMatViewCheck.check(&stmt, &Config::default(), &MigrationContext::default());
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_safe_alternative_includes_transaction_hint_when_in_transaction() {
        let stmt = parse_sql("REFRESH MATERIALIZED VIEW my_view;");
        let violations = RefreshMatViewCheck.check(
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
        let stmt = parse_sql("REFRESH MATERIALIZED VIEW my_view;");
        let violations = RefreshMatViewCheck.check(
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
        let stmt = parse_sql("REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;");
        let violations = RefreshMatViewCheck.check(
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
        let stmt = parse_sql("REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;");
        let violations = RefreshMatViewCheck.check(
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

use crate::checks::pg_helpers::{NodeEnum, range_var_name};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct IdempotencyCreateCheck;

impl Check for IdempotencyCreateCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::CreateStmt(create_stmt) = node else {
            return vec![];
        };

        if create_stmt.if_not_exists {
            return vec![];
        }

        let table = create_stmt
            .relation
            .as_ref()
            .map_or_else(|| "<table_name>".to_string(), range_var_name);

        vec![Violation::new(
            "CREATE TABLE without IF NOT EXISTS",
            format!(
                "CREATE TABLE for '{table}' is not idempotent. If this migration is retried after a partial failure, it can error because the table already exists.",
            ),
            format!(
                "Use IF NOT EXISTS to make retries safe:\n   CREATE TABLE IF NOT EXISTS {table} (...);"
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};
    use pg_query::protobuf::CreateStmt;

    #[test]
    fn test_detects_create_table_without_if_not_exists() {
        assert_detects_violation!(
            IdempotencyCreateCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY);",
            "CREATE TABLE without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_allows_create_table_with_if_not_exists() {
        assert_allows!(
            IdempotencyCreateCheck,
            "CREATE TABLE IF NOT EXISTS users (id BIGINT PRIMARY KEY);"
        );
    }

    #[test]
    fn test_create_table_without_relation_uses_placeholder_name() {
        let violations = IdempotencyCreateCheck.check(
            &NodeEnum::CreateStmt(CreateStmt::default()),
            &Config::default(),
            &MigrationContext::default(),
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("<table_name>"));
        assert!(
            violations[0]
                .safe_alternative
                .contains("CREATE TABLE IF NOT EXISTS <table_name> (...);")
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            IdempotencyCreateCheck,
            "ALTER TABLE users DROP COLUMN email;"
        );
    }
}

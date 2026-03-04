use crate::checks::pg_helpers::{NodeEnum, range_var_name};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct IdempotencyIndexCheck;

impl Check for IdempotencyIndexCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::IndexStmt(index_stmt) = node else {
            return vec![];
        };

        if index_stmt.if_not_exists {
            return vec![];
        }

        let table = index_stmt
            .relation
            .as_ref()
            .map_or_else(|| "<table_name>".to_string(), range_var_name);
        let index = if index_stmt.idxname.is_empty() {
            "<unnamed>".to_string()
        } else {
            index_stmt.idxname.clone()
        };
        let suggested_index_name = if index_stmt.idxname.is_empty() {
            "<index_name>"
        } else {
            &index_stmt.idxname
        };
        let concurrently = if index_stmt.concurrent {
            " CONCURRENTLY"
        } else {
            ""
        };

        vec![Violation::new(
            "CREATE INDEX without IF NOT EXISTS",
            format!(
                "CREATE INDEX '{index}' on table '{table}' is not idempotent. If this migration is retried after a partial failure, it can error because the index already exists.",
            ),
            format!(
                "Use IF NOT EXISTS to make retries safe:\n   CREATE INDEX{concurrently} IF NOT EXISTS {suggested_index_name} ON {table} (...);"
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_create_index_without_if_not_exists() {
        assert_detects_violation!(
            IdempotencyIndexCheck,
            "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
            "CREATE INDEX without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_detects_create_index_without_name() {
        assert_detects_violation!(
            IdempotencyIndexCheck,
            "CREATE INDEX ON users(email);",
            "CREATE INDEX without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_allows_create_index_with_if_not_exists() {
        assert_allows!(
            IdempotencyIndexCheck,
            "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_create_index_without_relation_uses_placeholders() {
        let violations = IdempotencyIndexCheck.check(
            &NodeEnum::IndexStmt(Box::default()),
            &Config::default(),
            &MigrationContext::default(),
        );

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("<unnamed>"));
        assert!(
            violations[0]
                .safe_alternative
                .contains("CREATE INDEX IF NOT EXISTS <index_name> ON <table_name> (...);")
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(IdempotencyIndexCheck, "DROP TABLE users;");
    }
}

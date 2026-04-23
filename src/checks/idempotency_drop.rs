use crate::checks::pg_helpers::{DropBehavior, NodeEnum, ObjectType, drop_object_names};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct IdempotencyDropCheck;

impl Check for IdempotencyDropCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::DropStmt(drop_stmt) = node else {
            return vec![];
        };

        if drop_stmt.missing_ok {
            return vec![];
        }

        let (operation, safe_template) = match drop_stmt.remove_type {
            x if x == ObjectType::ObjectTable as i32 => (
                "DROP TABLE without IF EXISTS",
                "DROP TABLE IF EXISTS {name}{behavior};",
            ),
            x if x == ObjectType::ObjectIndex as i32 => (
                "DROP INDEX without IF EXISTS",
                "DROP INDEX{concurrently} IF EXISTS {name}{behavior};",
            ),
            _ => return vec![],
        };

        let concurrently = if drop_stmt.concurrent {
            " CONCURRENTLY"
        } else {
            ""
        };
        let behavior = match drop_stmt.behavior {
            x if x == DropBehavior::DropCascade as i32 => " CASCADE",
            x if x == DropBehavior::DropRestrict as i32 => " RESTRICT",
            _ => "",
        };

        drop_object_names(&drop_stmt.objects)
            .into_iter()
            .map(|name| {
                let safe_sql = safe_template
                    .replace("{name}", &name)
                    .replace("{concurrently}", concurrently)
                    .replace("{behavior}", behavior);

                Violation::new(
                    operation,
                    format!(
                        "{operation} for '{name}' is not idempotent. If this migration is retried after a partial failure, it can error because the object no longer exists."
                    ),
                    format!("Use IF EXISTS to make retries safe:\n   {safe_sql}"),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_n_violations, assert_detects_violation};
    use pg_query::protobuf::DropStmt;

    #[test]
    fn test_detects_drop_table_without_if_exists() {
        assert_detects_violation!(
            IdempotencyDropCheck,
            "DROP TABLE users;",
            "DROP TABLE without IF EXISTS"
        );
    }

    #[test]
    fn test_allows_drop_table_with_if_exists() {
        assert_allows!(IdempotencyDropCheck, "DROP TABLE IF EXISTS users;");
    }

    #[test]
    fn test_detects_drop_index_without_if_exists() {
        assert_detects_violation!(
            IdempotencyDropCheck,
            "DROP INDEX CONCURRENTLY idx_users_email;",
            "DROP INDEX without IF EXISTS"
        );
    }

    #[test]
    fn test_allows_drop_index_with_if_exists() {
        assert_allows!(
            IdempotencyDropCheck,
            "DROP INDEX CONCURRENTLY IF EXISTS idx_users_email;"
        );
    }

    #[test]
    fn test_detects_drop_index_without_concurrently_and_if_exists() {
        assert_detects_violation!(
            IdempotencyDropCheck,
            "DROP INDEX idx_users_email;",
            "DROP INDEX without IF EXISTS"
        );
    }

    #[test]
    fn test_detects_multiple_drop_indexes_without_if_exists() {
        assert_detects_n_violations!(
            IdempotencyDropCheck,
            "DROP INDEX idx_a, idx_b;",
            2,
            "DROP INDEX without IF EXISTS"
        );
    }

    #[test]
    fn test_detects_multiple_drop_tables_without_if_exists() {
        assert_detects_n_violations!(
            IdempotencyDropCheck,
            "DROP TABLE users, posts;",
            2,
            "DROP TABLE without IF EXISTS"
        );
    }

    #[test]
    fn test_ignores_unsupported_drop_types() {
        assert_allows!(IdempotencyDropCheck, "DROP VIEW active_users;");
    }

    #[test]
    fn test_ignores_drop_stmt_with_unsupported_object_type() {
        let violations = IdempotencyDropCheck.check(
            &NodeEnum::DropStmt(DropStmt::default()),
            &Config::default(),
            &MigrationContext::default(),
        );
        assert!(violations.is_empty());
    }
}

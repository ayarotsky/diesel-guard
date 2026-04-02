use crate::checks::pg_helpers::NodeEnum;
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct MissingLockTimeoutCheck;

/// Returns a description of the DDL operation if the node is a lock-acquiring DDL statement,
/// or `None` if the node is not DDL we care about.
fn ddl_description(node: &NodeEnum) -> Option<&'static str> {
    match node {
        NodeEnum::AlterTableStmt(_) => Some("ALTER TABLE"),
        NodeEnum::IndexStmt(_) => Some("CREATE INDEX"),
        NodeEnum::DropStmt(_) => Some("DROP"),
        NodeEnum::TruncateStmt(_) => Some("TRUNCATE TABLE"),
        NodeEnum::ReindexStmt(_) => Some("REINDEX"),
        NodeEnum::RenameStmt(_) => Some("RENAME"),
        NodeEnum::RefreshMatViewStmt(_) => Some("REFRESH MATERIALIZED VIEW"),
        NodeEnum::CreateExtensionStmt(_) => Some("CREATE EXTENSION"),
        _ => None,
    }
}

impl Check for MissingLockTimeoutCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, ctx: &MigrationContext) -> Vec<Violation> {
        let Some(operation) = ddl_description(node) else {
            return vec![];
        };

        if ctx.has_lock_timeout && ctx.has_statement_timeout {
            return vec![];
        }

        let missing = match (ctx.has_lock_timeout, ctx.has_statement_timeout) {
            (false, false) => "lock_timeout and statement_timeout",
            (true, false) => "statement_timeout",
            (false, true) => "lock_timeout",
            (true, true) => unreachable!(),
        };

        vec![Violation::new(
            format!("{operation} without {missing}"),
            format!(
                "{operation} requires an aggressive lock that blocks reads/writes. \
                 Without {missing}, the migration can hang indefinitely waiting \
                 for the lock, blocking all other queries on the table."
            ),
            "SET lock_timeout = '2s';\n\
                 SET statement_timeout = '60s';\n\n\
                 Or disable this check if timeouts are configured at the connection level.",
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_alter_table_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
            "ALTER TABLE without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_create_index_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "CREATE INDEX idx_users_email ON users(email);",
            "CREATE INDEX without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_table_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP TABLE users;",
            "DROP without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_truncate_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "TRUNCATE TABLE users;",
            "TRUNCATE TABLE without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_reindex_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "REINDEX INDEX idx_users_email;",
            "REINDEX without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_rename_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER TABLE users RENAME TO customers;",
            "RENAME without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_refresh_matview_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "REFRESH MATERIALIZED VIEW my_view;",
            "REFRESH MATERIALIZED VIEW without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_create_extension_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "CREATE EXTENSION IF NOT EXISTS pg_trgm;",
            "CREATE EXTENSION without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_allows_when_both_timeouts_set() {
        let stmt = parse_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;");
        let ctx = MigrationContext {
            has_lock_timeout: true,
            has_statement_timeout: true,
            ..MigrationContext::default()
        };
        let violations = MissingLockTimeoutCheck.check(&stmt, &Config::default(), &ctx);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_detects_when_only_lock_timeout_set() {
        let stmt = parse_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;");
        let ctx = MigrationContext {
            has_lock_timeout: true,
            has_statement_timeout: false,
            ..MigrationContext::default()
        };
        let violations = MissingLockTimeoutCheck.check(&stmt, &Config::default(), &ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("statement_timeout"));
    }

    #[test]
    fn test_detects_when_only_statement_timeout_set() {
        let stmt = parse_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;");
        let ctx = MigrationContext {
            has_lock_timeout: false,
            has_statement_timeout: true,
            ..MigrationContext::default()
        };
        let violations = MissingLockTimeoutCheck.check(&stmt, &Config::default(), &ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("lock_timeout"));
    }

    #[test]
    fn test_allows_non_ddl_statements() {
        assert_allows!(MissingLockTimeoutCheck, "SELECT 1;");
    }

    #[test]
    fn test_allows_create_table() {
        assert_allows!(
            MissingLockTimeoutCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_insert() {
        assert_allows!(
            MissingLockTimeoutCheck,
            "INSERT INTO users (name) VALUES ('alice');"
        );
    }
}

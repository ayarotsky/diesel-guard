use crate::checks::pg_helpers::{NodeEnum, ObjectType};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct MissingLockTimeoutCheck;

/// Returns a description of the DDL operation if the node is a lock-acquiring DDL statement,
/// or `None` if the node is not DDL we care about.
fn ddl_description(node: &NodeEnum) -> Option<&'static str> {
    match node {
        NodeEnum::AlterTableStmt(_) => Some("ALTER TABLE"),
        NodeEnum::IndexStmt(_) => Some("CREATE INDEX"),
        NodeEnum::DropStmt(stmt) => Some(drop_label(stmt.remove_type)),
        NodeEnum::TruncateStmt(_) => Some("TRUNCATE TABLE"),
        NodeEnum::ReindexStmt(_) => Some("REINDEX"),
        NodeEnum::RenameStmt(stmt) => Some(rename_label(stmt.rename_type)),
        NodeEnum::RefreshMatViewStmt(_) => Some("REFRESH MATERIALIZED VIEW"),
        NodeEnum::CreateExtensionStmt(_) => Some("CREATE EXTENSION"),
        _ => None,
    }
}

/// Map `DropStmt.remove_type` to a specific label such as `"DROP TABLE"`,
/// matching the labels used by the dedicated drop checks. Falls back to
/// `"DROP"` for object kinds we don't enumerate.
fn drop_label(remove_type: i32) -> &'static str {
    match remove_type {
        x if x == ObjectType::ObjectTable as i32 => "DROP TABLE",
        x if x == ObjectType::ObjectIndex as i32 => "DROP INDEX",
        x if x == ObjectType::ObjectSchema as i32 => "DROP SCHEMA",
        x if x == ObjectType::ObjectMatview as i32 => "DROP MATERIALIZED VIEW",
        x if x == ObjectType::ObjectView as i32 => "DROP VIEW",
        x if x == ObjectType::ObjectSequence as i32 => "DROP SEQUENCE",
        x if x == ObjectType::ObjectType as i32 => "DROP TYPE",
        x if x == ObjectType::ObjectExtension as i32 => "DROP EXTENSION",
        x if x == ObjectType::ObjectTrigger as i32 => "DROP TRIGGER",
        _ => "DROP",
    }
}

/// Map `RenameStmt.rename_type` to a specific label such as `"RENAME COLUMN"`,
/// matching the labels used by the dedicated rename checks. Falls back to
/// `"RENAME"` for object kinds we don't enumerate.
fn rename_label(rename_type: i32) -> &'static str {
    match rename_type {
        x if x == ObjectType::ObjectTable as i32 => "RENAME TABLE",
        x if x == ObjectType::ObjectColumn as i32 => "RENAME COLUMN",
        x if x == ObjectType::ObjectSchema as i32 => "RENAME SCHEMA",
        x if x == ObjectType::ObjectIndex as i32 => "RENAME INDEX",
        x if x == ObjectType::ObjectTabconstraint as i32 => "RENAME CONSTRAINT",
        _ => "RENAME",
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

        let (missing, problem) = match (ctx.has_lock_timeout, ctx.has_statement_timeout) {
            (false, false) => (
                "lock_timeout and statement_timeout",
                format!(
                    "{operation} requires aggressive locking or timeout protection. \
                     Without lock_timeout and statement_timeout, the migration can \
                     hang indefinitely and block other database activity."
                ),
            ),
            (true, false) => (
                "statement_timeout",
                format!(
                    "{operation} is bounded by lock_timeout while acquiring locks, \
                     but without statement_timeout it can still run indefinitely \
                     once it begins executing."
                ),
            ),
            (false, true) => (
                "lock_timeout",
                format!(
                    "{operation} is bounded by statement_timeout overall, but \
                     without lock_timeout it can spend that entire window waiting \
                     to acquire a lock and block other database activity."
                ),
            ),
            (true, true) => unreachable!(),
        };

        vec![Violation::new(
            format!("{operation} without {missing}"),
            problem,
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
            "DROP TABLE without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_index_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP INDEX idx_users_email;",
            "DROP INDEX without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_schema_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP SCHEMA tenant_a;",
            "DROP SCHEMA without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_materialized_view_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP MATERIALIZED VIEW user_stats;",
            "DROP MATERIALIZED VIEW without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_view_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP VIEW user_view;",
            "DROP VIEW without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_sequence_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP SEQUENCE user_id_seq;",
            "DROP SEQUENCE without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_type_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP TYPE user_status;",
            "DROP TYPE without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_extension_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP EXTENSION pg_trgm;",
            "DROP EXTENSION without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_trigger_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP TRIGGER trg_users_audit ON users;",
            "DROP TRIGGER without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_drop_other_falls_back_to_generic_label() {
        // DROP FUNCTION is not enumerated, so the label falls back to "DROP".
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "DROP FUNCTION my_func();",
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
    fn test_detects_rename_table_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER TABLE users RENAME TO customers;",
            "RENAME TABLE without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_rename_column_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER TABLE users RENAME COLUMN email TO email_address;",
            "RENAME COLUMN without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_rename_schema_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER SCHEMA tenant_a RENAME TO tenant_b;",
            "RENAME SCHEMA without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_rename_index_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER INDEX idx_users_email RENAME TO idx_users_addr;",
            "RENAME INDEX without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_rename_constraint_without_timeouts() {
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER TABLE users RENAME CONSTRAINT users_pk TO users_pkey;",
            "RENAME CONSTRAINT without lock_timeout and statement_timeout"
        );
    }

    #[test]
    fn test_detects_rename_other_falls_back_to_generic_label() {
        // RENAME on a view is not enumerated, so the label falls back to "RENAME".
        assert_detects_violation!(
            MissingLockTimeoutCheck,
            "ALTER VIEW v RENAME TO v2;",
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

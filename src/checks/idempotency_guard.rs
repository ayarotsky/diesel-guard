//! Detection for non-idempotent migration statements.
//!
//! This check identifies statements that lack idempotency guards (`IF NOT EXISTS`
//! for creates/adds, `IF EXISTS` for drops). Without these guards, rerunning a
//! partially-applied migration can fail on statements that already succeeded.

use crate::checks::pg_helpers::{
    AlterTableType, DropBehavior, NodeEnum, ObjectType, alter_table_cmds, cmd_def_as_column_def,
    drop_object_names, range_var_name,
};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct IdempotencyCheck;

impl Check for IdempotencyCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        match node {
            NodeEnum::CreateStmt(create_stmt) => check_create_table(create_stmt),
            NodeEnum::IndexStmt(index_stmt) => check_create_index(index_stmt),
            NodeEnum::AlterTableStmt(_) => check_alter_table(node),
            NodeEnum::DropStmt(drop_stmt) => check_drop_stmt(drop_stmt),
            _ => vec![],
        }
    }
}

fn check_create_table(create_stmt: &pg_query::protobuf::CreateStmt) -> Vec<Violation> {
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

fn check_create_index(index_stmt: &pg_query::protobuf::IndexStmt) -> Vec<Violation> {
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

fn check_alter_table(node: &NodeEnum) -> Vec<Violation> {
    let Some((table, cmds)) = alter_table_cmds(node) else {
        return vec![];
    };

    cmds.iter()
        .filter_map(|cmd| {
            if cmd.subtype == AlterTableType::AtAddColumn as i32 {
                let column_def = cmd_def_as_column_def(cmd)?;

                if column_def.colname.is_empty() || cmd.missing_ok {
                    return None;
                }

                let column = column_def.colname.clone();
                return Some(Violation::new(
                    "ADD COLUMN without IF NOT EXISTS",
                    format!(
                        "ALTER TABLE ADD COLUMN for '{table}.{column}' is not idempotent. If this migration is retried after a partial failure, it can error because the column already exists.",
                    ),
                    format!(
                        "Use IF NOT EXISTS to make retries safe:\n   ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {column} <type>;"
                    ),
                ));
            }

            // DROP COLUMN has no dedicated node in AlterTableCmd.def in pg_query's
            // protobuf AST (def is null), so subtype is the only reliable discriminator.
            if cmd.subtype == AlterTableType::AtDropColumn as i32 && !cmd.missing_ok {
                let column = cmd.name.clone();
                return Some(Violation::new(
                    "DROP COLUMN without IF EXISTS",
                    format!(
                        "ALTER TABLE DROP COLUMN for '{table}.{column}' is not idempotent. If this migration is retried after a partial failure, it can error because the column no longer exists.",
                    ),
                    format!(
                        "Use IF EXISTS to make retries safe:\n   ALTER TABLE {table} DROP COLUMN IF EXISTS {column};"
                    ),
                ));
            }

            None
        })
        .collect()
}

fn check_drop_stmt(drop_stmt: &pg_query::protobuf::DropStmt) -> Vec<Violation> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_n_violations, assert_detects_violation};
    use pg_query::protobuf::{CreateStmt, DropStmt, IndexStmt};

    #[test]
    fn test_detects_create_table_without_if_not_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY);",
            "CREATE TABLE without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_allows_create_table_with_if_not_exists() {
        assert_allows!(
            IdempotencyCheck,
            "CREATE TABLE IF NOT EXISTS users (id BIGINT PRIMARY KEY);"
        );
    }

    #[test]
    fn test_create_table_without_relation_uses_placeholder_name() {
        let violations = check_create_table(&CreateStmt::default());

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("<table_name>"));
        assert!(
            violations[0]
                .safe_alternative
                .contains("CREATE TABLE IF NOT EXISTS <table_name> (...);")
        );
    }

    #[test]
    fn test_detects_create_index_without_if_not_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
            "CREATE INDEX without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_detects_create_index_without_name() {
        assert_detects_violation!(
            IdempotencyCheck,
            "CREATE INDEX ON users(email);",
            "CREATE INDEX without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_allows_create_index_with_if_not_exists() {
        assert_allows!(
            IdempotencyCheck,
            "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_create_index_without_relation_uses_placeholders() {
        let violations = check_create_index(&IndexStmt::default());

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("<unnamed>"));
        assert!(
            violations[0]
                .safe_alternative
                .contains("CREATE INDEX IF NOT EXISTS <index_name> ON <table_name> (...);")
        );
    }

    #[test]
    fn test_detects_add_column_without_if_not_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;",
            "ADD COLUMN without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_allows_add_column_with_if_not_exists() {
        assert_allows!(
            IdempotencyCheck,
            "ALTER TABLE users ADD COLUMN IF NOT EXISTS email TEXT;"
        );
    }

    #[test]
    fn test_detects_drop_table_without_if_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "DROP TABLE users;",
            "DROP TABLE without IF EXISTS"
        );
    }

    #[test]
    fn test_allows_drop_table_with_if_exists() {
        assert_allows!(IdempotencyCheck, "DROP TABLE IF EXISTS users;");
    }

    #[test]
    fn test_detects_drop_index_without_if_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "DROP INDEX CONCURRENTLY idx_users_email;",
            "DROP INDEX without IF EXISTS"
        );
    }

    #[test]
    fn test_allows_drop_index_with_if_exists() {
        assert_allows!(
            IdempotencyCheck,
            "DROP INDEX CONCURRENTLY IF EXISTS idx_users_email;"
        );
    }

    #[test]
    fn test_detects_drop_index_without_concurrently_and_if_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "DROP INDEX idx_users_email;",
            "DROP INDEX without IF EXISTS"
        );
    }

    #[test]
    fn test_detects_drop_column_without_if_exists() {
        assert_detects_violation!(
            IdempotencyCheck,
            "ALTER TABLE users DROP COLUMN email;",
            "DROP COLUMN without IF EXISTS"
        );
    }

    #[test]
    fn test_allows_drop_column_with_if_exists() {
        assert_allows!(
            IdempotencyCheck,
            "ALTER TABLE users DROP COLUMN IF EXISTS email;"
        );
    }

    #[test]
    fn test_detects_multiple_drop_indexes_without_if_exists() {
        assert_detects_n_violations!(
            IdempotencyCheck,
            "DROP INDEX idx_a, idx_b;",
            2,
            "DROP INDEX without IF EXISTS"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(IdempotencyCheck, "ALTER TABLE users RENAME TO accounts;");
    }

    #[test]
    fn test_ignores_unsupported_drop_types() {
        assert_allows!(IdempotencyCheck, "DROP VIEW active_users;");
    }

    #[test]
    fn test_detects_multiple_add_columns_in_single_alter_table() {
        assert_detects_n_violations!(
            IdempotencyCheck,
            "ALTER TABLE users ADD COLUMN email TEXT, ADD COLUMN phone TEXT;",
            2,
            "ADD COLUMN without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_detects_multiple_drop_columns_in_single_alter_table() {
        assert_detects_n_violations!(
            IdempotencyCheck,
            "ALTER TABLE users DROP COLUMN email, DROP COLUMN phone;",
            2,
            "DROP COLUMN without IF EXISTS"
        );
    }

    #[test]
    fn test_detects_only_unguarded_commands_in_mixed_alter_table() {
        use crate::checks::test_utils::parse_sql;
        use std::collections::HashSet;

        let stmt = parse_sql(
            "ALTER TABLE users \
             ADD COLUMN IF NOT EXISTS email TEXT, \
             ADD COLUMN phone TEXT, \
             DROP COLUMN IF EXISTS old_email, \
             DROP COLUMN legacy_email;",
        );

        let violations =
            IdempotencyCheck.check(&stmt, &Config::default(), &MigrationContext::default());
        assert_eq!(violations.len(), 2, "Expected exactly 2 violations");

        let operations: HashSet<&str> = violations.iter().map(|v| v.operation.as_str()).collect();
        let expected: HashSet<&str> = [
            "ADD COLUMN without IF NOT EXISTS",
            "DROP COLUMN without IF EXISTS",
        ]
        .into_iter()
        .collect();

        assert_eq!(operations, expected);
    }

    #[test]
    fn test_allows_guarded_commands_in_mixed_alter_table() {
        assert_allows!(
            IdempotencyCheck,
            "ALTER TABLE users \
             ADD COLUMN IF NOT EXISTS email TEXT, \
             DROP COLUMN IF EXISTS legacy_email;"
        );
    }

    #[test]
    fn test_detects_multiple_drop_tables_without_if_exists() {
        assert_detects_n_violations!(
            IdempotencyCheck,
            "DROP TABLE users, posts;",
            2,
            "DROP TABLE without IF EXISTS"
        );
    }

    #[test]
    fn test_ignores_drop_stmt_with_unsupported_object_type() {
        let violations = check_drop_stmt(&DropStmt::default());
        assert!(violations.is_empty());
    }
}

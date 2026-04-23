use crate::checks::pg_helpers::{
    AlterTableType, NodeEnum, alter_table_cmds, cmd_def_as_column_def,
};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct IdempotencyAlterCheck;

impl Check for IdempotencyAlterCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_n_violations, assert_detects_violation};

    #[test]
    fn test_detects_add_column_without_if_not_exists() {
        assert_detects_violation!(
            IdempotencyAlterCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;",
            "ADD COLUMN without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_allows_add_column_with_if_not_exists() {
        assert_allows!(
            IdempotencyAlterCheck,
            "ALTER TABLE users ADD COLUMN IF NOT EXISTS email TEXT;"
        );
    }

    #[test]
    fn test_detects_drop_column_without_if_exists() {
        assert_detects_violation!(
            IdempotencyAlterCheck,
            "ALTER TABLE users DROP COLUMN email;",
            "DROP COLUMN without IF EXISTS"
        );
    }

    #[test]
    fn test_allows_drop_column_with_if_exists() {
        assert_allows!(
            IdempotencyAlterCheck,
            "ALTER TABLE users DROP COLUMN IF EXISTS email;"
        );
    }

    #[test]
    fn test_detects_multiple_add_columns_in_single_alter_table() {
        assert_detects_n_violations!(
            IdempotencyAlterCheck,
            "ALTER TABLE users ADD COLUMN email TEXT, ADD COLUMN phone TEXT;",
            2,
            "ADD COLUMN without IF NOT EXISTS"
        );
    }

    #[test]
    fn test_detects_multiple_drop_columns_in_single_alter_table() {
        assert_detects_n_violations!(
            IdempotencyAlterCheck,
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
            IdempotencyAlterCheck.check(&stmt, &Config::default(), &MigrationContext::default());
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
            IdempotencyAlterCheck,
            "ALTER TABLE users \
             ADD COLUMN IF NOT EXISTS email TEXT, \
             DROP COLUMN IF EXISTS legacy_email;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            IdempotencyAlterCheck,
            "ALTER TABLE users RENAME TO accounts;"
        );
    }
}

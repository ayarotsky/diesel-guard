//! Detection for DROP NOT NULL constraint operations.
//!
//! This check identifies `ALTER TABLE` statements that remove NOT NULL constraints
//! from existing columns, which changes a contract that application code may depend on.
//!
//! Removing a NOT NULL constraint means the column can now hold NULL values. Any
//! application code that reads this column without NULL handling will fail at runtime.
//! This operation should be intentional and coordinated across application changes.

use crate::checks::pg_helpers::{AlterTableType, NodeEnum, alter_table_cmds};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct DropNotNullCheck;

impl Check for DropNotNullCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "DROP NOT NULL".into(),
            problem: "Removing NOT NULL constraint from column '<column>' on table '<table>' changes a contract \
                      that application code may depend on. Once NULL values are written to this column, any \
                      code that reads it without handling NULL will fail at runtime.".into(),
            safe_alternative: "Ensure this change is intentional and coordinated with application code changes. \
                               Update all code paths that read '<table>.<column>' to handle NULL values before \
                               or alongside this migration.".into(),
            script_path: None,
        }]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let descriptions = self.describe();
        let desc = &descriptions[0];
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                if cmd.subtype != AlterTableType::AtDropNotNull as i32 {
                    return None;
                }

                let column_name = &cmd.name;

                Some(Violation::new(
                    desc.operation.clone(),
                    desc.problem
                        .replace("<table>", &table_name)
                        .replace("<column>", column_name),
                    desc.safe_alternative
                        .replace("<table>", &table_name)
                        .replace("<column>", column_name),
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_drop_not_null() {
        assert_detects_violation!(
            DropNotNullCheck,
            "ALTER TABLE users ALTER COLUMN email DROP NOT NULL;",
            "DROP NOT NULL"
        );
    }

    #[test]
    fn test_ignores_set_not_null() {
        assert_allows!(
            DropNotNullCheck,
            "ALTER TABLE users ALTER COLUMN email SET NOT NULL;"
        );
    }

    #[test]
    fn test_ignores_other_alter_column_operations() {
        assert_allows!(
            DropNotNullCheck,
            "ALTER TABLE users ALTER COLUMN email SET DEFAULT 'test@example.com';"
        );
    }

    #[test]
    fn test_ignores_other_operations() {
        assert_allows!(
            DropNotNullCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            DropNotNullCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

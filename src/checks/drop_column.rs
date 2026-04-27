//! Detection for DROP COLUMN operations.
//!
//! This check identifies `ALTER TABLE` statements that drop columns, which requires
//! an ACCESS EXCLUSIVE lock and typically rewrites the table.
//!
//! Dropping a column acquires an ACCESS EXCLUSIVE lock, blocking all operations.
//! On many Postgres versions, this triggers a table rewrite to physically remove the
//! column data, with duration depending on table size.
//!
//! Postgres does not support a CONCURRENTLY option for dropping columns.
//! The recommended approach is to stage the removal: mark the column as unused
//! in application code, deploy without references, and drop in a later migration.

use crate::checks::pg_helpers::{AlterTableType, NodeEnum, alter_table_cmds};
use crate::checks::{Check, CheckDescription, Config, MigrationContext, if_exists_clause};
use crate::violation::Violation;

pub struct DropColumnCheck;

impl Check for DropColumnCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "DROP COLUMN".into(),
            problem: "Dropping column '<column>' from table '<table>' requires an ACCESS EXCLUSIVE lock, \
                      blocking all operations. This typically triggers a table rewrite with duration \
                      depending on table size.".into(),
            safe_alternative: "1. Mark the column as unused in your application code first.\n\n\
                               2. Deploy the application without the column references.\n\n\
                               3. (Optional) Set column to NULL to reclaim space:\n   \
                               ALTER TABLE <table> ALTER COLUMN <column> DROP NOT NULL;\n   \
                               UPDATE <table> SET <column> = NULL;\n\n\
                               4. Drop the column in a later migration after confirming it's unused:\n   \
                               ALTER TABLE <table> DROP COLUMN <column><if_exists>;\n\n\
                               Note: Postgres doesn't support DROP COLUMN CONCURRENTLY. The rewrite is \
                               unavoidable but staging the removal reduces risk.".into(),
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
                if cmd.subtype != AlterTableType::AtDropColumn as i32 {
                    return None;
                }

                let column_name = &cmd.name;
                let if_exists = cmd.missing_ok;

                Some(Violation::new(
                    desc.operation.clone(),
                    desc.problem
                        .replace("<table>", &table_name)
                        .replace("<column>", column_name),
                    desc.safe_alternative
                        .replace("<table>", &table_name)
                        .replace("<column>", column_name)
                        .replace("<if_exists>", if_exists_clause(if_exists)),
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_n_violations, assert_detects_violation};

    #[test]
    fn test_detects_drop_column() {
        assert_detects_violation!(
            DropColumnCheck,
            "ALTER TABLE users DROP COLUMN email;",
            "DROP COLUMN"
        );
    }

    #[test]
    fn test_detects_drop_column_if_exists() {
        assert_detects_violation!(
            DropColumnCheck,
            "ALTER TABLE users DROP COLUMN IF EXISTS email;",
            "DROP COLUMN"
        );
    }

    #[test]
    fn test_detects_drop_multiple_columns() {
        assert_detects_n_violations!(
            DropColumnCheck,
            "ALTER TABLE users DROP COLUMN a, DROP COLUMN b;",
            2,
            "DROP COLUMN"
        );
    }

    #[test]
    fn test_ignores_other_operations() {
        assert_allows!(
            DropColumnCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            DropColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

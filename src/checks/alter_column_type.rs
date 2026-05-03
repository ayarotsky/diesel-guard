//! Detection for ALTER COLUMN TYPE operations.
//!
//! This check identifies `ALTER TABLE` statements that change column data types,
//! which typically requires a table rewrite and ACCESS EXCLUSIVE lock.
//!
//! Most type changes acquire an ACCESS EXCLUSIVE lock and trigger a full table rewrite,
//! blocking all operations for the duration. However, some type changes are safe and instant
//! (e.g., increasing VARCHAR length on Postgres 9.2+, VARCHAR to TEXT).
//!
//! The duration and impact depend heavily on the specific type change and table size.

use crate::checks::pg_helpers::{
    AlterTableType, NodeEnum, alter_table_cmds, cmd_def_as_column_def, column_type_name,
};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct AlterColumnTypeCheck;

impl Check for AlterColumnTypeCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "ALTER COLUMN TYPE".into(),
            problem: "Changing column '<column>' type to '<type>' on table '<table>' typically requires \
                      an ACCESS EXCLUSIVE lock and may trigger a full table rewrite, blocking all operations. \
                      Duration depends on table size and the specific type change.".into(),
            safe_alternative: "For safer type changes, consider a multi-step approach:\n\n\
                               1. Add a new column with the desired type:\n   \
                               ALTER TABLE <table> ADD COLUMN <column>_new <type>;\n\n\
                               2. Backfill data in batches (outside migration):\n   \
                               UPDATE <table> SET <column>_new = <column>::<type>;\n\n\
                               3. Deploy application code to use the new column.\n\n\
                               4. Drop the old column in a later migration:\n   \
                               ALTER TABLE <table> DROP COLUMN <column>;\n\n\
                               5. Rename the new column:\n   \
                               ALTER TABLE <table> RENAME COLUMN <column>_new TO <column>;\n\n\
                               Note: Some type changes are safe:\n\
                               - VARCHAR(n) to VARCHAR(m) where m > n (Postgres 9.2+)\n\
                               - VARCHAR to TEXT\n\
                               - Numeric precision increases\n\n\
                               Always test on a production-sized dataset to verify the impact.".into(),
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
                if cmd.subtype != AlterTableType::AtAlterColumnType as i32 {
                    return None;
                }

                let column_name = &cmd.name;

                // The new type is stored in cmd.def as a ColumnDef
                let new_type = cmd_def_as_column_def(cmd)
                    .map(column_type_name)
                    .unwrap_or_default();

                Some(Violation::new(
                    desc.operation.clone(),
                    desc.problem
                        .replace("<table>", &table_name)
                        .replace("<column>", column_name)
                        .replace("<type>", &new_type),
                    desc.safe_alternative
                        .replace("<table>", &table_name)
                        .replace("<column>", column_name)
                        .replace("<type>", &new_type),
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
    fn test_detects_alter_column_type() {
        assert_detects_violation!(
            AlterColumnTypeCheck,
            "ALTER TABLE users ALTER COLUMN age TYPE BIGINT;",
            "ALTER COLUMN TYPE"
        );
    }

    #[test]
    fn test_detects_alter_column_type_with_using() {
        // USING clause doesn't change the violation — the type change is still detected
        assert_detects_violation!(
            AlterColumnTypeCheck,
            "ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;",
            "ALTER COLUMN TYPE"
        );
    }

    #[test]
    fn test_detects_set_data_type_variant() {
        assert_detects_violation!(
            AlterColumnTypeCheck,
            "ALTER TABLE users ALTER COLUMN email SET DATA TYPE VARCHAR(500);",
            "ALTER COLUMN TYPE"
        );
    }

    #[test]
    fn test_ignores_other_alter_column_operations() {
        assert_allows!(
            AlterColumnTypeCheck,
            "ALTER TABLE users ALTER COLUMN email SET NOT NULL;"
        );
    }

    #[test]
    fn test_ignores_other_operations() {
        assert_allows!(
            AlterColumnTypeCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AlterColumnTypeCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

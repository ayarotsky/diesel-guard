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
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct AlterColumnTypeCheck;

impl Check for AlterColumnTypeCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
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
                    "ALTER COLUMN TYPE",
                    format!(
                        "Changing column '{column}' type to '{new_type}' on table '{table}' typically requires an ACCESS EXCLUSIVE lock and \
                        may trigger a full table rewrite, blocking all operations. Duration depends on table size and the specific type change.",
                        column = column_name, new_type = new_type, table = table_name
                    ),
                    format!(r#"For safer type changes, consider a multi-step approach:

1. Add a new column with the desired type:
   ALTER TABLE {table} ADD COLUMN {column}_new {new_type};

2. Backfill data in batches (outside migration):
   UPDATE {table} SET {column}_new = {column}::{new_type};

3. Deploy application code to use the new column.

4. Drop the old column in a later migration:
   ALTER TABLE {table} DROP COLUMN {column};

5. Rename the new column:
   ALTER TABLE {table} RENAME COLUMN {column}_new TO {column};

Note: Some type changes are safe:
- VARCHAR(n) to VARCHAR(m) where m > n (Postgres 9.2+)
- VARCHAR to TEXT
- Numeric precision increases

Always test on a production-sized dataset to verify the impact."#,
                        table = table_name,
                        column = column_name,
                        new_type = new_type
                    ),
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

//! Detection for ADD NOT NULL constraint operations.
//!
//! This check identifies `ALTER TABLE` statements that add NOT NULL constraints
//! to existing columns, which requires a full table scan and ACCESS EXCLUSIVE lock.
//!
//! Adding NOT NULL to an existing column requires PostgreSQL to scan the entire table
//! to verify all existing values are non-null. This acquires an ACCESS EXCLUSIVE lock,
//! blocking all operations for the duration of the scan.
//!
//! For large tables, a safer approach is to add a CHECK constraint first, validate it
//! separately, then add the NOT NULL constraint.

use crate::checks::pg_helpers::{alter_table_cmds, AlterTableType, NodeEnum};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct AddNotNullCheck;

impl Check for AddNotNullCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                if cmd.subtype != AlterTableType::AtSetNotNull as i32 {
                    return None;
                }

                let column_name = &cmd.name;

                Some(Violation::new(
                    "ADD NOT NULL constraint",
                    format!(
                        "Adding NOT NULL constraint to column '{column}' on table '{table}' requires a full table scan to verify \
                        all values are non-null, acquiring an ACCESS EXCLUSIVE lock and blocking all operations. \
                        Duration depends on table size.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"For safer constraint addition on large tables:

1. Add a CHECK constraint without validating existing rows:
   ALTER TABLE {table} ADD CONSTRAINT {column}_not_null CHECK ({column} IS NOT NULL) NOT VALID;

2. Validate the constraint separately (uses SHARE UPDATE EXCLUSIVE lock):
   ALTER TABLE {table} VALIDATE CONSTRAINT {column}_not_null;

3. Add the NOT NULL constraint (instant if CHECK constraint exists):
   ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;

4. Optionally drop the redundant CHECK constraint:
   ALTER TABLE {table} DROP CONSTRAINT {column}_not_null;

Note: The VALIDATE step allows concurrent reads and writes, only blocking other schema changes. On PostgreSQL 12+, NOT NULL constraints are more efficient, but the CHECK approach still provides better control over large migrations."#,
                        table = table_name,
                        column = column_name
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
    fn test_detects_add_not_null() {
        assert_detects_violation!(
            AddNotNullCheck,
            "ALTER TABLE users ALTER COLUMN email SET NOT NULL;",
            "ADD NOT NULL constraint"
        );
    }

    #[test]
    fn test_ignores_drop_not_null() {
        assert_allows!(
            AddNotNullCheck,
            "ALTER TABLE users ALTER COLUMN email DROP NOT NULL;"
        );
    }

    #[test]
    fn test_ignores_other_alter_column_operations() {
        assert_allows!(
            AddNotNullCheck,
            "ALTER TABLE users ALTER COLUMN email SET DEFAULT 'test@example.com';"
        );
    }

    #[test]
    fn test_ignores_other_operations() {
        assert_allows!(
            AddNotNullCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddNotNullCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

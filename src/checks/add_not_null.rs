//! Detection for ADD NOT NULL constraint operations.
//!
//! This check identifies `ALTER TABLE` statements that add NOT NULL constraints
//! to existing columns, which requires a full table scan and ACCESS EXCLUSIVE lock.
//!
//! Adding NOT NULL to an existing column requires Postgres to scan the entire table
//! to verify all existing values are non-null. This acquires an ACCESS EXCLUSIVE lock,
//! blocking all operations for the duration of the scan.
//!
//! For large tables, a safer approach is to add a CHECK constraint first, validate it
//! separately, then add the NOT NULL constraint.

use crate::checks::pg_helpers::{AlterTableType, NodeEnum, alter_table_cmds};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

pub struct AddNotNullCheck;

impl Check for AddNotNullCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
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
                        "Adding NOT NULL constraint to column '{column_name}' on table '{table_name}' requires a full table scan to verify \
                        all values are non-null, acquiring an ACCESS EXCLUSIVE lock and blocking all operations. \
                        Duration depends on table size."
                    ),
                    format!(r"For safer constraint addition on large tables:

1. Add a CHECK constraint without validating existing rows:
   ALTER TABLE {table_name} ADD CONSTRAINT {column_name}_not_null CHECK ({column_name} IS NOT NULL) NOT VALID;

2. Validate the constraint separately (uses SHARE UPDATE EXCLUSIVE lock):
   ALTER TABLE {table_name} VALIDATE CONSTRAINT {column_name}_not_null;

3. Add the NOT NULL constraint (instant if CHECK constraint exists):
   ALTER TABLE {table_name} ALTER COLUMN {column_name} SET NOT NULL;

4. Optionally drop the redundant CHECK constraint:
   ALTER TABLE {table_name} DROP CONSTRAINT {column_name}_not_null;

Note: The VALIDATE step allows concurrent reads and writes, only blocking other schema changes. On Postgres 12+, NOT NULL constraints are more efficient, but the CHECK approach still provides better control over large migrations."
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

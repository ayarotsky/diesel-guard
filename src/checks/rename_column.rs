//! Detection for RENAME COLUMN operations.
//!
//! This check identifies `ALTER TABLE` statements that rename columns.
//! While RENAME COLUMN acquires only a brief ACCESS EXCLUSIVE lock and executes quickly,
//! it causes immediate errors in running application instances that still reference the old column name.
//!
//! The primary issue is not the lock duration, but application compatibility.
//! Any running code that references the old column name will fail immediately after the rename,
//! causing downtime until all instances are updated to use the new name.
//!
//! The recommended approach is a multi-step migration that maintains compatibility:
//! add a new column, backfill data, update application code to use the new column,
//! and finally remove the old column in a subsequent migration.

use crate::checks::pg_helpers::{NodeEnum, ObjectType, range_var_name};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct RenameColumnCheck;

impl Check for RenameColumnCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let NodeEnum::RenameStmt(rename) = node else {
            return vec![];
        };

        if rename.rename_type != ObjectType::ObjectColumn as i32 {
            return vec![];
        }

        let table_name = rename
            .relation
            .as_ref()
            .map(range_var_name)
            .unwrap_or_default();

        let old_name = &rename.subname;
        let new_name = &rename.newname;

        vec![Violation::new(
            "RENAME COLUMN",
            format!(
                "Renaming column '{old}' to '{new}' in table '{table}' will cause immediate errors in running application instances. \
                Any code referencing the old column name will fail after the rename is applied, causing downtime.",
                old = old_name,
                new = new_name,
                table = table_name
            ),
            format!(
                r#"1. Add a new column with the desired name (allows NULL initially):
   ALTER TABLE {table} ADD COLUMN {new} <data_type>;

2. Backfill the new column with data from the old column:
   UPDATE {table} SET {new} = {old};

3. Add NOT NULL constraint if needed (after backfill):
   ALTER TABLE {table} ALTER COLUMN {new} SET NOT NULL;

4. Update your application code to reference the new column name.

5. Deploy the updated application code.

6. Drop the old column in a subsequent migration:
   ALTER TABLE {table} DROP COLUMN {old};

This approach maintains compatibility with running instances during the transition."#,
                table = table_name,
                old = old_name,
                new = new_name
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_rename_column() {
        assert_detects_violation!(
            RenameColumnCheck,
            "ALTER TABLE users RENAME COLUMN email TO email_address;",
            "RENAME COLUMN"
        );
    }

    #[test]
    fn test_detects_rename_column_with_schema() {
        assert_detects_violation!(
            RenameColumnCheck,
            "ALTER TABLE public.users RENAME COLUMN old_name TO new_name;",
            "RENAME COLUMN"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            RenameColumnCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }

    #[test]
    fn test_ignores_rename_table() {
        assert_allows!(RenameColumnCheck, "ALTER TABLE users RENAME TO customers;");
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            RenameColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

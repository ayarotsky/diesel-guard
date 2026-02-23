//! Detection for RENAME TABLE operations.
//!
//! This check identifies `ALTER TABLE ... RENAME TO ...` statements that rename tables.
//! While the rename operation itself is fast, it causes immediate errors in running
//! application instances that still reference the old table name.
//!
//! Additionally, on large, busy tables, acquiring the ACCESS EXCLUSIVE lock required
//! for renaming can block or timeout, making the operation risky in production.
//!
//! The recommended approach is a multi-step dual-write migration that maintains
//! compatibility with running instances and avoids dangerous locks.

use crate::checks::pg_helpers::{range_var_name, NodeEnum, ObjectType};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct RenameTableCheck;

impl Check for RenameTableCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let NodeEnum::RenameStmt(rename) = node else {
            return vec![];
        };

        if rename.rename_type != ObjectType::ObjectTable as i32 {
            return vec![];
        }

        let old_table_name = rename
            .relation
            .as_ref()
            .map(range_var_name)
            .unwrap_or_default();

        let new_table_name = &rename.newname;

        vec![Violation::new(
            "RENAME TABLE",
            format!(
                "Renaming table '{old}' to '{new}' will cause immediate errors in running application instances. \
                Any code referencing the old table name will fail after the rename is applied. \
                Additionally, this operation requires an ACCESS EXCLUSIVE lock which can block on busy tables.",
                old = old_table_name,
                new = new_table_name
            ),
            format!(
                r#"Use a multi-step migration to safely rename the table:

1. Create the new table with the same structure:
   CREATE TABLE {new} (LIKE {old} INCLUDING ALL);

2. Update your application code to write to both tables.

3. Backfill data from the old table to the new table in batches:
   INSERT INTO {new} SELECT * FROM {old} WHERE id > last_id LIMIT 10000;

4. Update your application code to read from the new table.

5. Deploy the updated application code.

6. Update your application code to stop writing to the old table.

7. Drop the old table in a later migration:
   DROP TABLE {old};

This approach avoids dangerous locks and maintains compatibility with running instances throughout the migration."#,
                old = old_table_name,
                new = new_table_name
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_rename_table() {
        assert_detects_violation!(
            RenameTableCheck,
            "ALTER TABLE users RENAME TO customers;",
            "RENAME TABLE"
        );
    }

    #[test]
    fn test_detects_rename_table_with_schema() {
        assert_detects_violation!(
            RenameTableCheck,
            "ALTER TABLE public.users RENAME TO customers;",
            "RENAME TABLE"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            RenameTableCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }

    #[test]
    fn test_ignores_rename_column() {
        assert_allows!(
            RenameTableCheck,
            "ALTER TABLE users RENAME COLUMN email TO email_address;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            RenameTableCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

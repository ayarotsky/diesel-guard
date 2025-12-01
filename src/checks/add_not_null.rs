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

use crate::checks::Check;
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::{AlterColumnOperation, AlterTableOperation, Statement};

pub struct AddNotNullCheck;

impl Check for AddNotNullCheck {
    fn name(&self) -> &str {
        "add_not_null_constraint"
    }

    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>> {
        let mut violations = vec![];

        if let Statement::AlterTable {
            name, operations, ..
        } = stmt
        {
            for op in operations {
                if let AlterTableOperation::AlterColumn {
                    column_name,
                    op: AlterColumnOperation::SetNotNull,
                } = op
                {
                    let table_name = name.to_string();
                    let column_name_str = column_name.to_string();

                    violations.push(Violation::new(
                        "ADD NOT NULL constraint",
                        format!(
                            "Adding NOT NULL constraint to column '{}' on table '{}' requires a full table scan to verify \
                            all values are non-null, acquiring an ACCESS EXCLUSIVE lock and blocking all operations. \
                            Duration depends on table size.",
                            column_name_str, table_name
                        ),
                        format!(
                            "For safer constraint addition on large tables:\\n\\n\
                             1. Add a CHECK constraint without validating existing rows:\\n   \
                             ALTER TABLE {} ADD CONSTRAINT {}_not_null CHECK ({} IS NOT NULL) NOT VALID;\\n\\n\
                             2. Validate the constraint separately (uses SHARE UPDATE EXCLUSIVE lock):\\n   \
                             ALTER TABLE {} VALIDATE CONSTRAINT {}_not_null;\\n\\n\
                             3. Add the NOT NULL constraint (instant if CHECK constraint exists):\\n   \
                             ALTER TABLE {} ALTER COLUMN {} SET NOT NULL;\\n\\n\
                             4. Optionally drop the redundant CHECK constraint:\\n   \
                             ALTER TABLE {} DROP CONSTRAINT {}_not_null;\\n\\n\
                             Note: The VALIDATE step allows concurrent reads and writes, only blocking \
                             other schema changes. On PostgreSQL 12+, NOT NULL constraints are more efficient, \
                             but the CHECK approach still provides better control over large migrations.",
                            table_name,
                            column_name_str,
                            column_name_str,
                            table_name,
                            column_name_str,
                            table_name,
                            column_name_str,
                            table_name,
                            column_name_str
                        ),
                    ));
                }
            }
        }

        Ok(violations)
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

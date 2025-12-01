//! Detection for ADD COLUMN with DEFAULT operations.
//!
//! This check identifies `ALTER TABLE` statements that add columns with DEFAULT
//! values, which can cause table locks and performance issues on PostgreSQL < 11.
//!
//! On PostgreSQL versions before 11, adding a column with a DEFAULT value requires
//! a full table rewrite to backfill the default value for existing rows. This locks
//! the table for both reads and writes, which can take hours on large tables.

use crate::checks::Check;
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::{AlterTableOperation, ColumnOption, Statement};

pub struct AddColumnCheck;

impl Check for AddColumnCheck {
    fn name(&self) -> &str {
        "add_column_with_default"
    }

    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>> {
        let mut violations = vec![];

        if let Statement::AlterTable {
            name, operations, ..
        } = stmt
        {
            for op in operations {
                if let AlterTableOperation::AddColumn { column_def, .. } = op {
                    // Check if column has a DEFAULT value
                    let has_default = column_def
                        .options
                        .iter()
                        .any(|opt| matches!(opt.option, ColumnOption::Default(_)));

                    if has_default {
                        let column_name = &column_def.name;
                        let table_name = name.to_string();

                        violations.push(Violation::new(
                            "ADD COLUMN with DEFAULT",
                            format!(
                                "Adding column '{}' with DEFAULT on table '{}' requires a full table rewrite on PostgreSQL < 11, \
                                which acquires an ACCESS EXCLUSIVE lock. On large tables, this can take significant time and block all operations.",
                                column_name, table_name
                            ),
                            format!(
                                "1. Add the column without a default:\n   \
                                 ALTER TABLE {} ADD COLUMN {} {};\n\n\
                                 2. Backfill data in batches (outside migration):\n   \
                                 UPDATE {} SET {} = <value> WHERE {} IS NULL;\n\n\
                                 3. Add default for new rows only:\n   \
                                 ALTER TABLE {} ALTER COLUMN {} SET DEFAULT <value>;\n\n\
                                 Note: For PostgreSQL 11+, this is safe if the default is a constant value.",
                                table_name,
                                column_name,
                                column_def.data_type,
                                table_name,
                                column_name,
                                column_name,
                                table_name,
                                column_name
                            ),
                        ));
                    }
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
    fn test_detects_add_column_with_default() {
        assert_detects_violation!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
            "ADD COLUMN with DEFAULT"
        );
    }

    #[test]
    fn test_allows_add_column_without_default() {
        assert_allows!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

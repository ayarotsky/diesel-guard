//! Detection for DROP COLUMN operations.
//!
//! This check identifies `ALTER TABLE` statements that drop columns, which requires
//! an ACCESS EXCLUSIVE lock and typically rewrites the table.
//!
//! Dropping a column acquires an ACCESS EXCLUSIVE lock, blocking all operations.
//! On many PostgreSQL versions, this triggers a table rewrite to physically remove the
//! column data, with duration depending on table size.
//!
//! PostgreSQL does not support a CONCURRENTLY option for dropping columns.
//! The recommended approach is to stage the removal: mark the column as unused
//! in application code, deploy without references, and drop in a later migration.

use crate::checks::Check;
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::{AlterTableOperation, Statement};

pub struct DropColumnCheck;

impl Check for DropColumnCheck {
    fn name(&self) -> &str {
        "drop_column"
    }

    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>> {
        let mut violations = vec![];

        if let Statement::AlterTable {
            name, operations, ..
        } = stmt
        {
            for op in operations {
                if let AlterTableOperation::DropColumn {
                    column_names,
                    if_exists,
                    ..
                } = op
                {
                    let table_name = name.to_string();

                    // Report a violation for each column being dropped
                    for column_name in column_names {
                        let column_name_str = column_name.to_string();

                        violations.push(Violation::new(
                            "DROP COLUMN",
                            format!(
                                "Dropping column '{}' from table '{}' requires an ACCESS EXCLUSIVE lock, blocking all operations. \
                                This typically triggers a table rewrite with duration depending on table size.",
                                column_name_str, table_name
                            ),
                            format!(
                                "1. Mark the column as unused in your application code first.\n\n\
                                 2. Deploy the application without the column references.\n\n\
                                 3. (Optional) Set column to NULL to reclaim space:\n   \
                                 ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL;\n   \
                                 UPDATE {} SET {} = NULL;\n\n\
                                 4. Drop the column in a later migration after confirming it's unused:\n   \
                                 ALTER TABLE {} DROP COLUMN {}{};\n\n\
                                 Note: PostgreSQL doesn't support DROP COLUMN CONCURRENTLY. \
                                 The rewrite is unavoidable but staging the removal reduces risk.",
                                table_name,
                                column_name_str,
                                table_name,
                                column_name_str,
                                table_name,
                                column_name_str,
                                if *if_exists { " IF EXISTS" } else { "" }
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

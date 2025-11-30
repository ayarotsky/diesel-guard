//! Detection for DROP COLUMN operations.
//!
//! This check identifies `ALTER TABLE` statements that drop columns, which requires
//! an exclusive lock and rewrites the entire table on PostgreSQL.
//!
//! Dropping a column acquires an ACCESS EXCLUSIVE lock and triggers a full table rewrite
//! to remove the column data. This blocks all reads and writes for the duration of the
//! operation, which can take hours on large tables.
//!
//! Unlike index creation, PostgreSQL does not support a CONCURRENTLY option for dropping
//! columns. The recommended approach is to stage the removal: mark the column as unused
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
                                "Dropping column '{}' from table '{}' requires an exclusive lock and rewrites the table. \
                                This can take hours on large tables and blocks all reads/writes during the operation.",
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
    use crate::checks::test_utils::parse_sql;

    #[test]
    fn test_detects_drop_column() {
        let check = DropColumnCheck;
        let stmt = parse_sql("ALTER TABLE users DROP COLUMN email;");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "DROP COLUMN");
    }

    #[test]
    fn test_detects_drop_column_if_exists() {
        let check = DropColumnCheck;
        let stmt = parse_sql("ALTER TABLE users DROP COLUMN IF EXISTS email;");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "DROP COLUMN");
    }

    #[test]
    fn test_ignores_other_operations() {
        let check = DropColumnCheck;
        let stmt = parse_sql("ALTER TABLE users ADD COLUMN email VARCHAR(255);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_ignores_other_statements() {
        let check = DropColumnCheck;
        let stmt = parse_sql("CREATE TABLE users (id SERIAL PRIMARY KEY);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }
}

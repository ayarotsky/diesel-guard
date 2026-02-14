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

use crate::checks::pg_helpers::{alter_table_cmds, AlterTableType, NodeEnum};
use crate::checks::{if_exists_clause, Check};
use crate::violation::Violation;

pub struct DropColumnCheck;

impl Check for DropColumnCheck {
    fn check(&self, node: &NodeEnum) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                if cmd.subtype != AlterTableType::AtDropColumn as i32 {
                    return None;
                }

                let column_name = &cmd.name;
                let if_exists = cmd.missing_ok;

                Some(Violation::new(
                    "DROP COLUMN",
                    format!(
                        "Dropping column '{column}' from table '{table}' requires an ACCESS EXCLUSIVE lock, blocking all operations. \
                        This typically triggers a table rewrite with duration depending on table size.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"1. Mark the column as unused in your application code first.

2. Deploy the application without the column references.

3. (Optional) Set column to NULL to reclaim space:
   ALTER TABLE {table} ALTER COLUMN {column} DROP NOT NULL;
   UPDATE {table} SET {column} = NULL;

4. Drop the column in a later migration after confirming it's unused:
   ALTER TABLE {table} DROP COLUMN {column}{if_exists};

Note: PostgreSQL doesn't support DROP COLUMN CONCURRENTLY. The rewrite is unavoidable but staging the removal reduces risk."#,
                        table = table_name,
                        column = column_name,
                        if_exists = if_exists_clause(if_exists)
                    ),
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
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
    fn test_detects_drop_multiple_columns() {
        let check = DropColumnCheck;
        let stmt = parse_sql("ALTER TABLE users DROP COLUMN a, DROP COLUMN b;");

        let violations = check.check(&stmt);
        assert_eq!(
            violations.len(),
            2,
            "Should detect both columns being dropped"
        );
        assert!(violations.iter().all(|v| v.operation == "DROP COLUMN"));
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

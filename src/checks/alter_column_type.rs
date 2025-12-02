//! Detection for ALTER COLUMN TYPE operations.
//!
//! This check identifies `ALTER TABLE` statements that change column data types,
//! which typically requires a table rewrite and ACCESS EXCLUSIVE lock.
//!
//! Most type changes acquire an ACCESS EXCLUSIVE lock and trigger a full table rewrite,
//! blocking all operations for the duration. However, some type changes are safe and instant
//! (e.g., increasing VARCHAR length on PostgreSQL 9.2+, VARCHAR to TEXT).
//!
//! The duration and impact depend heavily on the specific type change and table size.
//! Type changes with USING clauses always require a full rewrite.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{AlterColumnOperation, AlterTableOperation, Statement};

pub struct AlterColumnTypeCheck;

impl Check for AlterColumnTypeCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let Statement::AlterTable {
            name, operations, ..
        } = stmt
        else {
            return vec![];
        };

        let table_name = name.to_string();

        operations
            .iter()
            .filter_map(|op| {
                let AlterTableOperation::AlterColumn {
                    column_name,
                    op: AlterColumnOperation::SetDataType { data_type, using, .. },
                } = op else {
                    return None;
                };

                let column_name_str = column_name.to_string();
                let new_type = data_type.to_string();

                let using_clause = if using.is_some() {
                    "\n\nNote: This migration includes a USING clause, which always triggers a full table rewrite."
                } else {
                    ""
                };

                Some(Violation::new(
                    "ALTER COLUMN TYPE",
                    format!(
                        "Changing column '{column}' type to '{new_type}' on table '{table}' typically requires an ACCESS EXCLUSIVE lock and \
                        may trigger a full table rewrite, blocking all operations. Duration depends on table size and the specific type change.{using_clause}",
                        column = column_name_str, new_type = new_type, table = table_name, using_clause = using_clause
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
- VARCHAR(n) to VARCHAR(m) where m > n (PostgreSQL 9.2+)
- VARCHAR to TEXT
- Numeric precision increases

Always test on a production-sized dataset to verify the impact."#,
                        table = table_name,
                        column = column_name_str,
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
    use crate::checks::test_utils::parse_sql;
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
        let check = AlterColumnTypeCheck;
        let stmt = parse_sql("ALTER TABLE users ALTER COLUMN data TYPE JSONB USING data::JSONB;");

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "ALTER COLUMN TYPE");
        assert!(violations[0].problem.contains("USING clause"));
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

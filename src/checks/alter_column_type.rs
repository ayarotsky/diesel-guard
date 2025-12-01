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
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::{AlterColumnOperation, AlterTableOperation, Statement};

pub struct AlterColumnTypeCheck;

impl Check for AlterColumnTypeCheck {
    fn name(&self) -> &str {
        "alter_column_type"
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
                    op:
                        AlterColumnOperation::SetDataType {
                            data_type, using, ..
                        },
                } = op
                {
                    let table_name = name.to_string();
                    let column_name_str = column_name.to_string();
                    let new_type = data_type.to_string();

                    let using_clause = if using.is_some() {
                        "\n\nNote: This migration includes a USING clause, which always triggers a full table rewrite."
                    } else {
                        ""
                    };

                    violations.push(Violation::new(
                        "ALTER COLUMN TYPE",
                        format!(
                            "Changing column '{}' type to '{}' on table '{}' typically requires an ACCESS EXCLUSIVE lock and \
                            may trigger a full table rewrite, blocking all operations. Duration depends on table size and the specific type change.{}",
                            column_name_str, new_type, table_name, using_clause
                        ),
                        format!(
                            "For safer type changes, consider a multi-step approach:\n\n\
                             1. Add a new column with the desired type:\n   \
                             ALTER TABLE {} ADD COLUMN {}_new {};\n\n\
                             2. Backfill data in batches (outside migration):\n   \
                             UPDATE {} SET {}_new = {}::{};\n\n\
                             3. Deploy application code to use the new column.\n\n\
                             4. Drop the old column in a later migration:\n   \
                             ALTER TABLE {} DROP COLUMN {};\n\n\
                             5. Rename the new column:\n   \
                             ALTER TABLE {} RENAME COLUMN {}_new TO {};\n\n\
                             Note: Some type changes are safe:\n\
                             - VARCHAR(n) to VARCHAR(m) where m > n (PostgreSQL 9.2+)\n\
                             - VARCHAR to TEXT\n\
                             - Numeric precision increases\n\n\
                             Always test on a production-sized dataset to verify the impact.",
                            table_name,
                            column_name_str,
                            new_type,
                            table_name,
                            column_name_str,
                            column_name_str,
                            new_type,
                            table_name,
                            column_name_str,
                            table_name,
                            column_name_str,
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

        let violations = check.check(&stmt).unwrap();
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

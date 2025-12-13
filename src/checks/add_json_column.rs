//! Detection for ADD COLUMN with JSON type.
//!
//! This check identifies `ALTER TABLE ... ADD COLUMN` statements that use the `json`
//! data type instead of `jsonb`. The `json` type lacks equality operators, which can
//! cause runtime errors for existing SELECT DISTINCT queries.
//!
//! In PostgreSQL, the `json` type stores an exact copy of the input text and lacks
//! proper equality operators. This means operations like SELECT DISTINCT, GROUP BY,
//! and UNION will fail when applied to json columns.
//!
//! The `jsonb` type stores data in a decomposed binary format with proper indexing
//! and equality operators, making it suitable for all PostgreSQL operations.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{AlterTable, AlterTableOperation, DataType, Statement};

pub struct AddJsonColumnCheck;

impl Check for AddJsonColumnCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let Statement::AlterTable(AlterTable {
            name, operations, ..
        }) = stmt
        else {
            return vec![];
        };

        let table_name = name.to_string();

        operations
            .iter()
            .filter_map(|op| {
                let AlterTableOperation::AddColumn { column_def, .. } = op else {
                    return None;
                };

                // Check if column type is JSON (not JSONB)
                if !matches!(column_def.data_type, DataType::JSON) {
                    return None;
                }

                let column_name = &column_def.name;

                Some(Violation::new(
                    "ADD COLUMN with JSON type",
                    format!(
                        "Adding column '{column}' with JSON type on table '{table}' can break existing SELECT DISTINCT queries. \
                        The JSON type has no equality operator, causing runtime errors for DISTINCT, GROUP BY, and UNION operations.",
                        column = column_name,
                        table = table_name
                    ),
                    format!(
                        r#"Use JSONB instead of JSON:

   ALTER TABLE {table} ADD COLUMN {column} JSONB;

Benefits of JSONB over JSON:
- Has proper equality and comparison operators (supports DISTINCT, GROUP BY, UNION)
- Supports indexing (GIN indexes for efficient queries)
- Faster to process (binary format, no reparsing)
- Generally better performance for most use cases

Note: The only advantage of JSON over JSONB is that it preserves exact formatting and key order,
which is rarely needed in practice."#,
                        table = table_name,
                        column = column_name
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
    fn test_detects_add_json_column() {
        assert_detects_violation!(
            AddJsonColumnCheck,
            "ALTER TABLE users ADD COLUMN properties JSON;",
            "ADD COLUMN with JSON type"
        );
    }

    #[test]
    fn test_detects_add_json_column_with_constraint() {
        assert_detects_violation!(
            AddJsonColumnCheck,
            "ALTER TABLE users ADD COLUMN metadata JSON NOT NULL;",
            "ADD COLUMN with JSON type"
        );
    }

    #[test]
    fn test_allows_add_jsonb_column() {
        // JSONB is the safe alternative
        assert_allows!(
            AddJsonColumnCheck,
            "ALTER TABLE users ADD COLUMN properties JSONB;"
        );
    }

    #[test]
    fn test_allows_add_jsonb_column_with_constraint() {
        assert_allows!(
            AddJsonColumnCheck,
            "ALTER TABLE users ADD COLUMN metadata JSONB NOT NULL;"
        );
    }

    #[test]
    fn test_allows_other_column_types() {
        assert_allows!(
            AddJsonColumnCheck,
            "ALTER TABLE users ADD COLUMN name TEXT;"
        );
    }

    #[test]
    fn test_allows_create_table_with_json() {
        // Only ALTER TABLE ADD COLUMN is problematic - CREATE TABLE is fine
        // because there are no existing queries to break
        assert_allows!(
            AddJsonColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY, data JSON);"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            AddJsonColumnCheck,
            "ALTER TABLE users DROP COLUMN old_field;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(AddJsonColumnCheck, "SELECT * FROM users;");
    }
}

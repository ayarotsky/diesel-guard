//! Detection for ADD COLUMN with DEFAULT operations.
//!
//! This check identifies `ALTER TABLE` statements that add columns with DEFAULT
//! values, which can cause table locks and performance issues on PostgreSQL < 11.
//!
//! On PostgreSQL versions before 11, adding a column with a DEFAULT value requires
//! a full table rewrite to backfill the default value for existing rows. This acquires
//! an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{AlterTableOperation, ColumnOption, Statement};

pub struct AddColumnCheck;

impl Check for AddColumnCheck {
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
                let AlterTableOperation::AddColumn { column_def, .. } = op else {
                    return None;
                };

                // Check if column has a DEFAULT value
                let has_default = column_def
                    .options
                    .iter()
                    .any(|opt| matches!(opt.option, ColumnOption::Default(_)));

                if !has_default {
                    return None;
                }

                let column_name = &column_def.name;

                Some(Violation::new(
                    "ADD COLUMN with DEFAULT",
                    format!(
                        "Adding column '{column}' with DEFAULT on table '{table}' requires a full table rewrite on PostgreSQL < 11, \
                        which acquires an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"1. Add the column without a default:
   ALTER TABLE {table} ADD COLUMN {column} {data_type};

2. Backfill data in batches (outside migration):
   UPDATE {table} SET {column} = <value> WHERE {column} IS NULL;

3. Add default for new rows only:
   ALTER TABLE {table} ALTER COLUMN {column} SET DEFAULT <value>;

Note: For PostgreSQL 11+, this is safe if the default is a constant value."#,
                        table = table_name,
                        column = column_name,
                        data_type = column_def.data_type
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

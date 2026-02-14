//! Detection for ADD COLUMN with DEFAULT operations.
//!
//! This check identifies `ALTER TABLE` statements that add columns with DEFAULT
//! values, which can cause table locks and performance issues on PostgreSQL < 11.
//!
//! On PostgreSQL versions before 11, adding a column with a DEFAULT value requires
//! a full table rewrite to backfill the default value for existing rows. This acquires
//! an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size.

use crate::checks::pg_helpers::{
    alter_table_cmds, cmd_def_as_column_def, column_has_constraint, column_type_name, ConstrType,
    NodeEnum,
};
use crate::checks::Check;
use crate::violation::Violation;

pub struct AddColumnCheck;

impl Check for AddColumnCheck {
    fn check(&self, node: &NodeEnum) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                let col = cmd_def_as_column_def(cmd)?;

                if !column_has_constraint(col, ConstrType::ConstrDefault as i32) {
                    return None;
                }

                let column_name = &col.colname;
                let data_type = column_type_name(col);

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
                        data_type = data_type
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

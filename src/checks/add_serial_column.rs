//! Detection for ADD COLUMN with SERIAL data types.
//!
//! This check identifies `ALTER TABLE` statements that add columns with SERIAL,
//! SMALLSERIAL, or BIGSERIAL data types, which trigger a full table rewrite.
//!
//! Adding a SERIAL column to an existing table requires Postgres to:
//! 1. Create a new sequence
//! 2. Rewrite the entire table to populate the sequence values for existing rows
//! 3. Update all indexes
//!
//! This operation acquires an ACCESS EXCLUSIVE lock, blocking all operations.
//! Duration depends on table size and number of indexes.

use crate::checks::pg_helpers::{
    NodeEnum, alter_table_cmds, cmd_def_as_column_def, column_type_name, is_serial_pattern,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct AddSerialColumnCheck;

impl Check for AddSerialColumnCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                let col = cmd_def_as_column_def(cmd)?;

                if !is_serial_pattern(col) {
                    return None;
                }

                let column_name = &col.colname;
                let type_name = column_type_name(col);

                Some(Violation::new(
                    "ADD COLUMN with SERIAL",
                    format!(
                        "Adding column '{column}' with SERIAL type on table '{table}' requires a full table rewrite to populate sequence values for existing rows, \
                        which acquires an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size and number of indexes.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"1. Create a sequence:
   CREATE SEQUENCE {table}_{column}_seq;

2. Add the column WITHOUT default (fast, no rewrite):
   ALTER TABLE {table} ADD COLUMN {column} {type_name};

3. Backfill existing rows in batches (outside migration):
   UPDATE {table} SET {column} = nextval('{table}_{column}_seq') WHERE {column} IS NULL;

4. Set default for future inserts only:
   ALTER TABLE {table} ALTER COLUMN {column} SET DEFAULT nextval('{table}_{column}_seq');

5. Set NOT NULL if needed (Postgres 11+: safe if all values present):
   ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;

6. Set sequence ownership:
   ALTER SEQUENCE {table}_{column}_seq OWNED BY {table}.{column};"#,
                        table = table_name,
                        column = column_name,
                        type_name = type_name
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
    fn test_detects_add_column_with_serial() {
        assert_detects_violation!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN id SERIAL;",
            "ADD COLUMN with SERIAL"
        );
    }

    #[test]
    fn test_detects_add_column_with_bigserial() {
        assert_detects_violation!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN id BIGSERIAL;",
            "ADD COLUMN with SERIAL"
        );
    }

    #[test]
    fn test_detects_add_column_with_smallserial() {
        assert_detects_violation!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN id SMALLSERIAL;",
            "ADD COLUMN with SERIAL"
        );
    }

    #[test]
    fn test_allows_add_column_with_integer() {
        assert_allows!(
            AddSerialColumnCheck,
            "ALTER TABLE users ADD COLUMN count INTEGER;"
        );
    }

    #[test]
    fn test_allows_create_table_with_serial() {
        assert_allows!(
            AddSerialColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddSerialColumnCheck,
            "CREATE INDEX idx_users_email ON users(email);"
        );
    }
}

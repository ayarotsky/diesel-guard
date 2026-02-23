//! Detection for ADD COLUMN with GENERATED STORED operations.
//!
//! This check identifies `ALTER TABLE ADD COLUMN` statements that add
//! `GENERATED ALWAYS AS ... STORED` columns, which requires an ACCESS EXCLUSIVE
//! lock and triggers a full table rewrite.
//!
//! Adding a stored generated column requires PostgreSQL to compute and store the
//! expression value for every existing row. This acquires an ACCESS EXCLUSIVE lock,
//! blocking all operations for the duration of the rewrite.
//!
//! Stored generated columns were introduced in PostgreSQL 12. PostgreSQL does not
//! support VIRTUAL generated columns (only STORED), so there is no safe GENERATED
//! column option for existing tables.
//!
//! CREATE TABLE with GENERATED STORED is safe because the table is empty.

use crate::checks::pg_helpers::{
    alter_table_cmds, cmd_def_as_column_def, column_has_constraint, column_type_name, ConstrType,
    NodeEnum,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct GeneratedColumnCheck;

impl Check for GeneratedColumnCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                let col = cmd_def_as_column_def(cmd)?;

                if !column_has_constraint(col, ConstrType::ConstrGenerated as i32) {
                    return None;
                }

                let column_name = &col.colname;
                let data_type = column_type_name(col);

                Some(Violation::new(
                    "ADD COLUMN with GENERATED STORED",
                    format!(
                        "Adding column '{column}' with GENERATED ALWAYS AS ... STORED on table '{table}' \
                        triggers a full table rewrite because PostgreSQL must compute and store the expression \
                        value for every existing row. This acquires an ACCESS EXCLUSIVE lock and blocks all operations. \
                        Duration depends on table size.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"1. Add a regular nullable column instead:
   ALTER TABLE {table} ADD COLUMN {column} {data_type};

2. Backfill values in batches (outside migration):
   UPDATE {table} SET {column} = <expression> WHERE {column} IS NULL;

3. Optionally add NOT NULL constraint:
   ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL;

4. Use a trigger to compute values for new rows:
   CREATE FUNCTION compute_{column}() RETURNS TRIGGER AS $$
   BEGIN
     NEW.{column} := <expression>;
     RETURN NEW;
   END;
   $$ LANGUAGE plpgsql;

   CREATE TRIGGER trg_{column}
   BEFORE INSERT OR UPDATE ON {table}
   FOR EACH ROW EXECUTE FUNCTION compute_{column}();

5. If the table rewrite is acceptable (e.g., small table or maintenance window),
   use a safety-assured block:
   -- safety-assured:start
   ALTER TABLE {table} ADD COLUMN {column} {data_type} GENERATED ALWAYS AS (<expression>) STORED;
   -- safety-assured:end

Note: PostgreSQL does not support VIRTUAL generated columns (only STORED).
For new empty tables, GENERATED STORED columns are acceptable."#,
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
    fn test_detects_add_column_generated_stored() {
        assert_detects_violation!(
            GeneratedColumnCheck,
            "ALTER TABLE products ADD COLUMN total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED;",
            "ADD COLUMN with GENERATED STORED"
        );
    }

    #[test]
    fn test_detects_add_column_generated_stored_with_string_expression() {
        assert_detects_violation!(
            GeneratedColumnCheck,
            "ALTER TABLE users ADD COLUMN full_name TEXT GENERATED ALWAYS AS (first_name || ' ' || last_name) STORED;",
            "ADD COLUMN with GENERATED STORED"
        );
    }

    #[test]
    fn test_ignores_safe_variant_regular_column() {
        assert_allows!(
            GeneratedColumnCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;"
        );
    }

    #[test]
    fn test_ignores_safe_variant_column_with_default() {
        assert_allows!(
            GeneratedColumnCheck,
            "ALTER TABLE users ADD COLUMN status TEXT DEFAULT 'active';"
        );
    }

    #[test]
    fn test_ignores_safe_variant_identity_column() {
        // GENERATED ALWAYS AS IDENTITY is not a stored generated column
        assert_allows!(
            GeneratedColumnCheck,
            "ALTER TABLE users ADD COLUMN id INTEGER GENERATED ALWAYS AS IDENTITY;"
        );
    }

    #[test]
    fn test_ignores_create_table() {
        // CREATE TABLE is safe because the table is empty
        assert_allows!(
            GeneratedColumnCheck,
            "CREATE TABLE products (id SERIAL PRIMARY KEY, price INTEGER, quantity INTEGER, total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED);"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(GeneratedColumnCheck, "ALTER TABLE users DROP COLUMN email;");
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(GeneratedColumnCheck, "SELECT * FROM users;");
    }
}

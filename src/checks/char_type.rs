//! Detection for CHAR/CHARACTER column types.
//!
//! This check identifies columns using CHAR or CHARACTER data types and recommends
//! using TEXT or VARCHAR instead.
//!
//! CHAR types are fixed-length and padded with spaces, which:
//! - Wastes storage space
//! - Causes subtle bugs with string comparisons (trailing spaces)
//! - Affects DISTINCT, GROUP BY, and joins unexpectedly
//! - Provides no performance benefit over VARCHAR or TEXT in Postgres
//!
//! ## Lock type
//! None - this is a best practices check, not a locking concern.
//!
//! ## Rewrite behavior
//! None - no table rewrite is involved.
//!
//! ## Postgres version specifics
//! Applies to all Postgres versions.

use crate::checks::pg_helpers::{
    ColumnDef, NodeEnum, alter_table_cmds, cmd_def_as_column_def, column_type_name,
    for_each_column_def, is_char_type,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct CharTypeCheck;

impl Check for CharTypeCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        // Handle CREATE TABLE via for_each_column_def
        if let NodeEnum::CreateStmt(_) = node {
            return for_each_column_def(node)
                .into_iter()
                .filter_map(|(table, col)| {
                    if !is_char_type(&column_type_name(col)) {
                        return None;
                    }
                    let length = get_char_length(col);
                    Some(create_create_table_violation(&table, &col.colname, &length))
                })
                .collect();
        }

        // Handle ALTER TABLE ADD COLUMN
        if let NodeEnum::AlterTableStmt(_) = node {
            let Some((table_name, cmds)) = alter_table_cmds(node) else {
                return vec![];
            };

            return cmds
                .iter()
                .filter_map(|cmd| {
                    let col = cmd_def_as_column_def(cmd)?;
                    if !is_char_type(&column_type_name(col)) {
                        return None;
                    }
                    let length = get_char_length(col);
                    Some(create_alter_table_violation(
                        &table_name,
                        &col.colname,
                        &length,
                    ))
                })
                .collect();
        }

        vec![]
    }
}

/// Extract the CHAR length from a ColumnDef's TypeName typmods.
///
/// pg_query uses "bpchar" as the internal type name. The length modifier
/// is stored in the TypeName's `typmods` field as an AConst node wrapping
/// an Integer value. If no typmods are present, CHAR defaults to length 1.
fn get_char_length(col: &ColumnDef) -> String {
    col.type_name
        .as_ref()
        .and_then(|tn| tn.typmods.first())
        .and_then(|n| match &n.node {
            Some(NodeEnum::AConst(ac)) => match &ac.val {
                Some(pg_query::protobuf::a_const::Val::Ival(i)) => Some(i.ival.to_string()),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or_else(|| "1".to_string())
}

/// Create a violation for ALTER TABLE ADD COLUMN with CHAR type
fn create_alter_table_violation(table_name: &str, column_name: &str, length: &str) -> Violation {
    Violation::new(
        "ADD COLUMN with CHAR type",
        format!(
            "Column '{column}' uses CHAR({length}) which is fixed-length and padded with spaces. \
            This wastes storage and can cause subtle bugs with string comparisons. \
            This is a best practice warning (no locking impact).",
            column = column_name,
            length = length
        ),
        format!(
            r#"Use TEXT or VARCHAR instead of CHAR:

1. For variable-length strings (most cases):
   ALTER TABLE {table} ADD COLUMN {column} TEXT;

2. If you need a length constraint:
   ALTER TABLE {table} ADD COLUMN {column} VARCHAR({length});
   -- Or use TEXT with a CHECK constraint:
   ALTER TABLE {table} ADD COLUMN {column} TEXT CHECK (length({column}) <= {length});

CHAR is only appropriate for truly fixed-length codes (e.g., ISO country codes).
If this is intentional, use a safety-assured block:
   -- safety-assured:start
   ALTER TABLE {table} ADD COLUMN {column} CHAR({length});
   -- safety-assured:end"#,
            table = table_name,
            column = column_name,
            length = length
        ),
    )
}

/// Create a violation for CREATE TABLE with CHAR type column
fn create_create_table_violation(table_name: &str, column_name: &str, length: &str) -> Violation {
    Violation::new(
        "CREATE TABLE with CHAR type",
        format!(
            "Column '{column}' uses CHAR({length}) which is fixed-length and padded with spaces. \
            This wastes storage and can cause subtle bugs with string comparisons. \
            This is a best practice warning (no locking impact).",
            column = column_name,
            length = length
        ),
        format!(
            r#"Use TEXT or VARCHAR instead of CHAR:

1. For variable-length strings (most cases):
   CREATE TABLE {table} (
       {column} TEXT
   );

2. If you need a length constraint:
   CREATE TABLE {table} (
       {column} VARCHAR({length})
   );
   -- Or use TEXT with a CHECK constraint:
   CREATE TABLE {table} (
       {column} TEXT CHECK (length({column}) <= {length})
   );

CHAR is only appropriate for truly fixed-length codes (e.g., ISO country codes).
If this is intentional, use a safety-assured block:
   -- safety-assured:start
   CREATE TABLE {table} (
       {column} CHAR({length})
   );
   -- safety-assured:end"#,
            table = table_name,
            column = column_name,
            length = length
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_allows, assert_detects_n_violations_any_containing, assert_detects_violation,
        assert_detects_violation_containing,
    };

    // === Detection tests ===

    #[test]
    fn test_detects_char_column_alter_table() {
        assert_detects_violation!(
            CharTypeCheck,
            "ALTER TABLE users ADD COLUMN country_code CHAR(2);",
            "ADD COLUMN with CHAR type"
        );
    }

    #[test]
    fn test_detects_character_column_alter_table() {
        assert_detects_violation!(
            CharTypeCheck,
            "ALTER TABLE users ADD COLUMN status CHARACTER(1);",
            "ADD COLUMN with CHAR type"
        );
    }

    #[test]
    fn test_detects_char_column_create_table() {
        assert_detects_violation!(
            CharTypeCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY, country_code CHAR(2));",
            "CREATE TABLE with CHAR type"
        );
    }

    #[test]
    fn test_detects_char_with_explicit_length() {
        assert_detects_violation_containing!(
            CharTypeCheck,
            "ALTER TABLE products ADD COLUMN sku CHAR(10);",
            "ADD COLUMN with CHAR type",
            "CHAR(10)"
        );
    }

    #[test]
    fn test_detects_char_without_explicit_length() {
        // CHAR without length defaults to CHAR(1)
        assert_detects_violation_containing!(
            CharTypeCheck,
            "ALTER TABLE flags ADD COLUMN flag CHAR;",
            "ADD COLUMN with CHAR type",
            "CHAR(1)"
        );
    }

    #[test]
    fn test_detects_multiple_char_columns() {
        assert_detects_n_violations_any_containing!(
            CharTypeCheck,
            "CREATE TABLE locations (id SERIAL PRIMARY KEY, country CHAR(2), region CHAR(3));",
            2,
            "country",
            "region"
        );
    }

    // === Safe variant tests ===

    #[test]
    fn test_allows_varchar_column() {
        assert_allows!(
            CharTypeCheck,
            "ALTER TABLE users ADD COLUMN name VARCHAR(255);"
        );
    }

    #[test]
    fn test_allows_text_column() {
        assert_allows!(CharTypeCheck, "ALTER TABLE users ADD COLUMN bio TEXT;");
    }

    #[test]
    fn test_allows_other_column_types() {
        assert_allows!(CharTypeCheck, "ALTER TABLE users ADD COLUMN age INT;");
        assert_allows!(
            CharTypeCheck,
            "ALTER TABLE users ADD COLUMN active BOOLEAN;"
        );
        assert_allows!(
            CharTypeCheck,
            "ALTER TABLE users ADD COLUMN created_at TIMESTAMP;"
        );
    }

    #[test]
    fn test_allows_create_table_without_char() {
        assert_allows!(
            CharTypeCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email VARCHAR(255));"
        );
    }

    // === Unrelated operation tests ===

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(CharTypeCheck, "ALTER TABLE users DROP COLUMN old_field;");
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(CharTypeCheck, "SELECT * FROM users;");
    }
}

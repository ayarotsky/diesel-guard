//! Detection for CHAR/CHARACTER column types.
//!
//! This check identifies columns using CHAR or CHARACTER data types and recommends
//! using TEXT or VARCHAR instead.
//!
//! CHAR types are fixed-length and padded with spaces, which:
//! - Wastes storage space
//! - Causes subtle bugs with string comparisons (trailing spaces)
//! - Affects DISTINCT, GROUP BY, and joins unexpectedly
//! - Provides no performance benefit over VARCHAR or TEXT in PostgreSQL
//!
//! ## Lock type
//! None - this is a best practices check, not a locking concern.
//!
//! ## Rewrite behavior
//! None - no table rewrite is involved.
//!
//! ## PostgreSQL version specifics
//! Applies to all PostgreSQL versions.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{
    AlterTable, AlterTableOperation, ColumnDef, CreateTable, DataType, ObjectName, Statement,
};

pub struct CharTypeCheck;

impl Check for CharTypeCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        match stmt {
            Statement::AlterTable(AlterTable {
                name, operations, ..
            }) => check_alter_table_operations(name, operations),
            Statement::CreateTable(CreateTable { name, columns, .. }) => {
                check_create_table_columns(name, columns)
            }
            _ => vec![],
        }
    }
}

/// Check if a data type is CHAR or CHARACTER
fn is_char_type(data_type: &DataType) -> bool {
    matches!(data_type, DataType::Char(_) | DataType::Character(_))
}

/// Extract the length from a CHAR/CHARACTER type for display
fn get_char_length(data_type: &DataType) -> String {
    match data_type {
        DataType::Char(Some(len)) | DataType::Character(Some(len)) => len.to_string(),
        DataType::Char(None) | DataType::Character(None) => "1".to_string(),
        _ => "".to_string(),
    }
}

/// Check ALTER TABLE operations for CHAR type columns
fn check_alter_table_operations(
    table_name: &ObjectName,
    operations: &[AlterTableOperation],
) -> Vec<Violation> {
    operations
        .iter()
        .filter_map(|op| {
            let AlterTableOperation::AddColumn { column_def, .. } = op else {
                return None;
            };

            if !is_char_type(&column_def.data_type) {
                return None;
            }

            Some(create_alter_table_violation(
                &table_name.to_string(),
                &column_def.name.to_string(),
                &get_char_length(&column_def.data_type),
            ))
        })
        .collect()
}

/// Check CREATE TABLE columns for CHAR type
fn check_create_table_columns(table_name: &ObjectName, columns: &[ColumnDef]) -> Vec<Violation> {
    columns
        .iter()
        .filter_map(|col| {
            if !is_char_type(&col.data_type) {
                return None;
            }

            Some(create_create_table_violation(
                &table_name.to_string(),
                &col.name.to_string(),
                &get_char_length(&col.data_type),
            ))
        })
        .collect()
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
        "CREATE TABLE with CHAR column",
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
    use crate::{assert_allows, assert_detects_violation};

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
            "CREATE TABLE with CHAR column"
        );
    }

    #[test]
    fn test_detects_char_with_explicit_length() {
        use crate::checks::test_utils::parse_sql;

        let check = CharTypeCheck;
        let stmt = parse_sql("ALTER TABLE products ADD COLUMN sku CHAR(10);");
        let violations = check.check(&stmt);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].problem.contains("CHAR(10)"));
    }

    #[test]
    fn test_detects_char_without_explicit_length() {
        use crate::checks::test_utils::parse_sql;

        let check = CharTypeCheck;
        let stmt = parse_sql("ALTER TABLE flags ADD COLUMN flag CHAR;");
        let violations = check.check(&stmt);

        assert_eq!(violations.len(), 1);
        // CHAR without length defaults to CHAR(1)
        assert!(violations[0].problem.contains("CHAR(1)"));
    }

    #[test]
    fn test_detects_multiple_char_columns() {
        use crate::checks::test_utils::parse_sql;

        let check = CharTypeCheck;
        let stmt = parse_sql(
            "CREATE TABLE locations (id SERIAL PRIMARY KEY, country CHAR(2), region CHAR(3));",
        );
        let violations = check.check(&stmt);

        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.problem.contains("country")));
        assert!(violations.iter().any(|v| v.problem.contains("region")));
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

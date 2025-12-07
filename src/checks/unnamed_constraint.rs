//! Detection for unnamed constraints in migrations.
//!
//! This check identifies constraints added without explicit names (UNIQUE, FOREIGN KEY, CHECK).
//!
//! Unnamed constraints receive auto-generated names from PostgreSQL (like "users_email_key"
//! or "posts_user_id_fkey"), which can vary between databases and make future migrations
//! difficult. When you need to modify or drop the constraint later, you'll need to query
//! the database to find the generated name, which is error-prone and environment-specific.
//!
//! Always name constraints explicitly for maintainable migrations.

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{AlterTableOperation, Statement, TableConstraint};

pub struct UnnamedConstraintCheck;

impl Check for UnnamedConstraintCheck {
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
                let AlterTableOperation::AddConstraint { constraint, .. } = op else {
                    return None;
                };

                // Check if constraint has a name
                let (constraint_type, columns_desc) = match constraint {
                    TableConstraint::Unique { name, columns, .. } => {
                        if name.is_some() {
                            return None;
                        }
                        let cols = columns
                            .iter()
                            .map(|c| c.column.expr.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        ("UNIQUE", cols)
                    }
                    TableConstraint::ForeignKey {
                        name,
                        columns,
                        foreign_table,
                        referred_columns,
                        ..
                    } => {
                        if name.is_some() {
                            return None;
                        }
                        let cols = columns
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let foreign_cols = referred_columns
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        (
                            "FOREIGN KEY",
                            format!(
                                "({}) REFERENCES {}({})",
                                cols, foreign_table, foreign_cols
                            ),
                        )
                    }
                    TableConstraint::Check { name, expr, .. } => {
                        if name.is_some() {
                            return None;
                        }
                        ("CHECK", format!("({})", expr))
                    }
                    _ => return None, // Ignore other constraint types
                };

                Some(Violation::new(
                    "Unnamed constraint",
                    format!(
                        "Adding unnamed {constraint_type} constraint on table '{table}' will receive an auto-generated name from PostgreSQL. \
                        This makes future migrations difficult, as the generated name varies between databases and requires querying \
                        the database to find the constraint name before modifying or dropping it.",
                        constraint_type = constraint_type,
                        table = table_name
                    ),
                    format!(
                        r#"Always name constraints explicitly using the CONSTRAINT keyword:

Instead of:
   ALTER TABLE {table} ADD {constraint_type} {columns};

Use:
   ALTER TABLE {table} ADD CONSTRAINT {table}_{suggested_name} {constraint_type} {columns};

Named constraints make future migrations predictable and maintainable:
   -- Easy to reference in later migrations
   ALTER TABLE {table} DROP CONSTRAINT {table}_{suggested_name};

Note: Choose descriptive names that indicate the table, columns, and constraint type.
Common patterns:
  - UNIQUE: {table}_<column>_key or {table}_<column1>_<column2>_key
  - FOREIGN KEY: {table}_<column>_fkey
  - CHECK: {table}_<column>_check or {table}_<description>_check"#,
                        table = table_name,
                        constraint_type = constraint_type,
                        columns = columns_desc,
                        suggested_name = match constraint_type {
                            "UNIQUE" => "column_key",
                            "FOREIGN KEY" => "column_fkey",
                            "CHECK" => "column_check",
                            _ => "constraint",
                        }
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
    fn test_detects_unnamed_unique_constraint() {
        assert_detects_violation!(
            UnnamedConstraintCheck,
            "ALTER TABLE users ADD UNIQUE (email);",
            "Unnamed constraint"
        );
    }

    #[test]
    fn test_detects_unnamed_foreign_key_constraint() {
        assert_detects_violation!(
            UnnamedConstraintCheck,
            "ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id);",
            "Unnamed constraint"
        );
    }

    #[test]
    fn test_detects_unnamed_check_constraint() {
        assert_detects_violation!(
            UnnamedConstraintCheck,
            "ALTER TABLE users ADD CHECK (age >= 0);",
            "Unnamed constraint"
        );
    }

    #[test]
    fn test_allows_named_unique_constraint() {
        assert_allows!(
            UnnamedConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);"
        );
    }

    #[test]
    fn test_allows_named_foreign_key_constraint() {
        assert_allows!(
            UnnamedConstraintCheck,
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);"
        );
    }

    #[test]
    fn test_allows_named_check_constraint() {
        assert_allows!(
            UnnamedConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            UnnamedConstraintCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            UnnamedConstraintCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

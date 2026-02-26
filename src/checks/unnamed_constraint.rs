//! Detection for unnamed constraints in migrations.
//!
//! This check identifies constraints added without explicit names (UNIQUE, FOREIGN KEY, CHECK).
//!
//! Unnamed constraints receive auto-generated names from Postgres (like "users_email_key"
//! or "posts_user_id_fkey"), which can vary between databases and make future migrations
//! difficult. When you need to modify or drop the constraint later, you'll need to query
//! the database to find the generated name, which is error-prone and environment-specific.
//!
//! Always name constraints explicitly for maintainable migrations.

use crate::checks::pg_helpers::{
    ConstrType, NodeEnum, alter_table_cmds, cmd_def_as_constraint, constraint_columns_str,
    range_var_name,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct UnnamedConstraintCheck;

impl Check for UnnamedConstraintCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                let c = cmd_def_as_constraint(cmd)?;

                // Only check unnamed constraints
                if !c.conname.is_empty() {
                    return None;
                }

                let (constraint_type, columns_desc) = match c.contype {
                    x if x == ConstrType::ConstrUnique as i32 => {
                        ("UNIQUE", constraint_columns_str(c))
                    }
                    x if x == ConstrType::ConstrForeign as i32 => {
                        // FK columns are in fk_attrs, not keys
                        let fk_cols = c
                            .fk_attrs
                            .iter()
                            .filter_map(|n| match &n.node {
                                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(", ");

                        let ref_table = c
                            .pktable
                            .as_ref()
                            .map(range_var_name)
                            .unwrap_or_default();

                        let ref_cols = c
                            .pk_attrs
                            .iter()
                            .filter_map(|n| match &n.node {
                                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(", ");

                        (
                            "FOREIGN KEY",
                            format!("({}) REFERENCES {}({})", fk_cols, ref_table, ref_cols),
                        )
                    }
                    x if x == ConstrType::ConstrCheck as i32 => {
                        ("CHECK", "(...)".to_string())
                    }
                    _ => return None,
                };

                Some(Violation::new(
                    "CONSTRAINT without name",
                    format!(
                        "Adding unnamed {constraint_type} constraint on table '{table}' will receive an auto-generated name from Postgres. \
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
            "CONSTRAINT without name"
        );
    }

    #[test]
    fn test_detects_unnamed_foreign_key_constraint() {
        assert_detects_violation!(
            UnnamedConstraintCheck,
            "ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id);",
            "CONSTRAINT without name"
        );
    }

    #[test]
    fn test_detects_unnamed_check_constraint() {
        assert_detects_violation!(
            UnnamedConstraintCheck,
            "ALTER TABLE users ADD CHECK (age >= 0);",
            "CONSTRAINT without name"
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

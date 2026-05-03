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
    fk_cols_constraint, ref_columns_constraint, ref_table_constraint,
};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct UnnamedConstraintCheck;

impl Check for UnnamedConstraintCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "CONSTRAINT without name".into(),
            problem: "Adding unnamed <type> constraint on table '<table>' will receive an auto-generated \
                      name from Postgres. This makes future migrations difficult, as the generated name \
                      varies between databases and requires querying the database to find the constraint \
                      name before modifying or dropping it.".into(),
            safe_alternative: "Always name constraints explicitly using the CONSTRAINT keyword:\n\n\
                               Instead of:\n   \
                               ALTER TABLE <table> ADD <type> <cols>;\n\n\
                               Use:\n   \
                               ALTER TABLE <table> ADD CONSTRAINT <table>_<suggested> <type> <cols>;\n\n\
                               Named constraints make future migrations predictable and maintainable:\n   \
                               -- Easy to reference in later migrations\n   \
                               ALTER TABLE <table> DROP CONSTRAINT <table>_<suggested>;\n\n\
                               Note: Choose descriptive names that indicate the table, columns, and constraint type.\n\
                               Common patterns:\n  \
                               - UNIQUE: <table>_<column>_key or <table>_<column1>_<column2>_key\n  \
                               - FOREIGN KEY: <table>_<column>_fkey\n  \
                               - CHECK: <table>_<column>_check or <table>_<description>_check".into(),
            script_path: None,
        }]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let descriptions = self.describe();
        let desc = &descriptions[0];
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
                        let fk_cols = fk_cols_constraint(c);

                        let ref_table = ref_table_constraint(c);

                        let ref_cols = ref_columns_constraint(c);

                        (
                            "FOREIGN KEY",
                            format!("({fk_cols}) REFERENCES {ref_table}({ref_cols})"),
                        )
                    }
                    x if x == ConstrType::ConstrCheck as i32 => ("CHECK", "(...)".to_string()),
                    _ => return None,
                };

                Some(Violation::new(
                    desc.operation.clone(),
                    desc.problem
                        .replace("<table>", &table_name)
                        .replace("<type>", constraint_type),
                    desc.safe_alternative
                        .replace("<table>", &table_name)
                        .replace("<type>", constraint_type)
                        .replace("<cols>", &columns_desc)
                        .replace(
                            "<suggested>",
                            match constraint_type {
                                "UNIQUE" => "column_key",
                                "FOREIGN KEY" => "column_fkey",
                                "CHECK" => "column_check",
                                _ => "constraint",
                            },
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

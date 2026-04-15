//! Detection for ADD CHECK CONSTRAINT operations.
//!
//! This check identifies `ALTER TABLE` statements that add CHECK constraints,
//! which acquires an ACCESS EXCLUSIVE lock and scans all existing rows to validate
//! the constraint. Duration depends on table size.
//!
//! Use `NOT VALID` to add the constraint without scanning existing rows, then
//! `VALIDATE CONSTRAINT` in a separate migration to check existing data.

use crate::checks::Check;
use crate::checks::pg_helpers::{alter_table_cmds, cmd_def_as_constraint, constraint_display_name};
use crate::{Config, MigrationContext, Violation};
use pg_query::NodeEnum;
use pg_query::protobuf::ConstrType;

pub struct AddCheckConstraintCheck;

impl Check for AddCheckConstraintCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };
        cmds.iter().filter_map(|cmd| {
            let constraint = cmd_def_as_constraint(cmd)?;
            if constraint.contype != ConstrType::ConstrCheck as i32 {
                return None;
            }

            if !constraint.initially_valid {
                return None;
            }

            let constraint_name = constraint_display_name(constraint);

            Some(Violation::new(
                "ADD CHECK CONSTRAINT",
                format!("Adding a check constraint '{constraint_name}' on table '{table_name}' without NOT VALID scans the entire table to validate existing rows,\
             which can block autovacuum. On larger tables this can cause performance issues."),
                format!(
                    r"For a safer check constraint addition on large tables:

1. Create a check constraint without any immediate validation:
   ALTER TABLE {table_name} ADD CONSTRAINT {constraint_name} CHECK <expr> NOT VALID;

2. Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
  ALTER TABLE {table_name} VALIDATE CONSTRAINT {constraint_name};

Benefits:
- Table remains readable and writable during constraint creation
- No blocking of SELECT, INSERT, UPDATE, or DELETE operations
- Safe for production deployments on large tables
",
                )))
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_add_check_constraint_unsafe() {
        assert_detects_violation!(
            AddCheckConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT price_check CHECK (price > 0);",
            "ADD CHECK CONSTRAINT"
        );
    }

    #[test]
    fn test_allows_add_check_constraint_safe() {
        assert_allows!(
            AddCheckConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT price_check CHECK (price > 0) NOT VALID;"
        );
    }

    #[test]
    fn test_detects_unnamed_check_constraint_unsafe() {
        assert_detects_violation!(
            AddCheckConstraintCheck,
            "ALTER TABLE users ADD CHECK (price > 0);",
            "ADD CHECK CONSTRAINT"
        );
    }

    #[test]
    fn test_allows_unnamed_check_constraint_safe() {
        assert_allows!(
            AddCheckConstraintCheck,
            "ALTER TABLE users ADD CHECK (price > 0) NOT VALID;"
        );
    }

    #[test]
    fn test_allows_validate_constraint() {
        assert_allows!(
            AddCheckConstraintCheck,
            "ALTER TABLE users VALIDATE CONSTRAINT price_check;"
        );
    }

    #[test]
    fn test_ignores_other_alter_table_commands() {
        assert_allows!(
            AddCheckConstraintCheck,
            "ALTER TABLE users ALTER COLUMN price SET NOT NULL;"
        );
    }

    #[test]
    fn test_ignores_non_check_constraints() {
        // FOREIGN KEY constraint
        assert_allows!(
            AddCheckConstraintCheck,
            "ALTER TABLE orders ADD CONSTRAINT orders_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);"
        );

        // UNIQUE constraint
        assert_allows!(
            AddCheckConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_unique UNIQUE (email);"
        );
    }
}

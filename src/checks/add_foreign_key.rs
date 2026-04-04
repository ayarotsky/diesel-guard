use crate::checks::Check;
use crate::checks::pg_helpers::{
    alter_table_cmds, cmd_def_as_constraint, constraint_display_name, fk_cols_constraint,
    ref_columns_constraint, ref_table_constraint,
};
use crate::{Config, MigrationContext, Violation};
use pg_query::NodeEnum;
use pg_query::protobuf::ConstrType;

pub struct AddForeignKeyCheck;

impl Check for AddForeignKeyCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };
        cmds.iter().filter_map(|cmd| {
            let constraint = cmd_def_as_constraint(cmd)?;
            if constraint.contype != ConstrType::ConstrForeign as i32 {
                return None;
            }

            if !constraint.initially_valid {
                return None;
            }

            let fk_cols = fk_cols_constraint(constraint);

            let ref_table = ref_table_constraint(constraint);

            let ref_cols = ref_columns_constraint(constraint);

            let constraint_name = constraint_display_name(constraint);

            Some(Violation::new(
                "ADD FOREIGN KEY",
                format!("Adding a foreign key constraint '{constraint_name}' on table '{table_name}' ({fk_cols}) without NOT VALID scans the entire table to validate existing rows,\
             acquiring ShareRowExclusiveLock for the duration. On large tables this blocks writes and is a common cause of migration-induced outages."),
                format!(
                    r"For a safer foreign key addition on large tables:

1. Create a foreign key without any immediate validation:
   ALTER TABLE {table_name} ADD CONSTRAINT {constraint_name}
    FOREIGN KEY ({fk_cols}) REFERENCES {ref_table} ({ref_cols}) NOT VALID;

2. Step 2 (separate migration, acquires ShareUpdateExclusiveLock only)
  ALTER TABLE {table_name} VALIDATE CONSTRAINT {constraint_name};

Benefits:
- Table remains readable and writable during foreign key creation
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
    use crate::checks::test_utils::parse_sql;
    use crate::{assert_allows, assert_detects_violation, assert_detects_violation_containing};

    #[test]
    fn test_detects_add_foreign_key() {
        assert_detects_violation!(
            AddForeignKeyCheck,
            "ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id);",
            "ADD FOREIGN KEY"
        );
    }

    #[test]
    fn test_detects_named_foreign_key() {
        assert_detects_violation!(
            AddForeignKeyCheck,
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);",
            "ADD FOREIGN KEY"
        );
    }

    #[test]
    fn test_allows_foreign_key_not_valid() {
        assert_allows!(
            AddForeignKeyCheck,
            "ALTER TABLE posts ADD FOREIGN KEY (user_id) REFERENCES users(id) NOT VALID;"
        );
    }

    #[test]
    fn test_allows_named_foreign_key_not_valid() {
        assert_allows!(
            AddForeignKeyCheck,
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) NOT VALID;"
        );
    }

    #[test]
    fn test_allows_non_fk_constraint() {
        assert_allows!(
            AddForeignKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT age_positive CHECK (age > 0);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddForeignKeyCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_ignores_add_column() {
        assert_allows!(
            AddForeignKeyCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;"
        );
    }

    #[test]
    fn test_violation_contains_table_and_column_names() {
        assert_detects_violation_containing!(
            AddForeignKeyCheck,
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);",
            "ADD FOREIGN KEY",
            "posts",
            "posts_user_id_fkey",
            "user_id"
        );
    }

    #[test]
    fn test_safe_alternative_contains_not_valid_steps() {
        let stmt = parse_sql(
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);",
        );
        let violations =
            AddForeignKeyCheck.check(&stmt, &Config::default(), &MigrationContext::default());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].safe_alternative.contains("NOT VALID"),
            "Expected NOT VALID in safe_alternative"
        );
        assert!(
            violations[0]
                .safe_alternative
                .contains("VALIDATE CONSTRAINT"),
            "Expected VALIDATE CONSTRAINT in safe_alternative"
        );
    }
}

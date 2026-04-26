//! Detection for RENAME SCHEMA operations.
//!
//! This check identifies `ALTER SCHEMA ... RENAME TO ...` statements that rename schemas.
//! Renaming a schema breaks all application code, ORM models, and connection strings that
//! reference any object within the schema — the blast radius is as wide as the schema itself.
//!
//! Unlike renaming a column or table (where only references to that specific object break),
//! a schema rename invalidates every qualified reference of the form `old_schema.table`,
//! `old_schema.function`, `old_schema.type`, etc., across the entire application.
//!
//! Additionally, this operation requires an ACCESS EXCLUSIVE lock which can block on a
//! busy database.

use crate::checks::pg_helpers::{NodeEnum, ObjectType};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct RenameSchemaCheck;

impl Check for RenameSchemaCheck {
    fn describe(&self) -> CheckDescription {
        CheckDescription {
            operation: "RENAME SCHEMA".into(),
            problem: "Renaming a schema breaks all application code, ORM models, and connection strings \
                      that reference any object within the schema — every qualified reference fails immediately.".into(),
            safe_alternative: "Use a search_path alias so both names resolve temporarily while updating all \
                               application references.".into(),
            script_path: None,
        }
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::RenameStmt(rename) = node else {
            return vec![];
        };

        if rename.rename_type != ObjectType::ObjectSchema as i32 {
            return vec![];
        }

        let old_name = &rename.subname;
        let new_name = &rename.newname;

        vec![Violation::new(
            self.describe().operation,
            format!(
                "Renaming schema '{old_name}' to '{new_name}' breaks all application code, ORM \
                models, and connection strings that reference any object within the schema. Every \
                qualified reference of the form '{old_name}.<object>' across the entire application \
                will fail immediately after the rename is applied."
            ),
            format!(
                r"Avoid renaming schemas in production. If a rename is unavoidable:

1. Add a search_path alias so both names resolve temporarily:
   ALTER DATABASE mydb SET search_path TO {new_name}, {old_name};

2. Update all application code, ORM models, and connection strings to use the new schema name.

3. Deploy the updated application.

4. Remove the search_path alias once all references have been updated."
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_rename_schema() {
        assert_detects_violation!(
            RenameSchemaCheck,
            "ALTER SCHEMA myschema RENAME TO newschema;",
            "RENAME SCHEMA"
        );
    }

    #[test]
    fn test_detects_rename_public_schema() {
        assert_detects_violation!(
            RenameSchemaCheck,
            "ALTER SCHEMA public RENAME TO app;",
            "RENAME SCHEMA"
        );
    }

    #[test]
    fn test_ignores_rename_column() {
        assert_allows!(
            RenameSchemaCheck,
            "ALTER TABLE users RENAME COLUMN email TO email_address;"
        );
    }

    #[test]
    fn test_ignores_rename_table() {
        assert_allows!(RenameSchemaCheck, "ALTER TABLE users RENAME TO customers;");
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            RenameSchemaCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

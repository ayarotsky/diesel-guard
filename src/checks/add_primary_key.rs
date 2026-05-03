//! Detection for ADD PRIMARY KEY constraint via ALTER TABLE on existing tables.
//!
//! This check identifies `ALTER TABLE ... ADD PRIMARY KEY` constraint statements, which
//! acquire ACCESS EXCLUSIVE locks and implicitly create an index.
//!
//! Adding a PRIMARY KEY constraint to an existing table acquires an ACCESS EXCLUSIVE lock,
//! blocking all reads and writes. Additionally, it implicitly creates a unique index
//! (a blocking operation) and validates all existing rows for uniqueness, which can
//! take a long time on large tables.
//!
//! The safe alternative is to create a UNIQUE INDEX CONCURRENTLY first, then add the
//! PRIMARY KEY constraint using that existing index (Postgres 11+).

use crate::checks::pg_helpers::{
    ConstrType, NodeEnum, alter_table_cmds, cmd_def_as_constraint, constraint_columns_str,
};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct AddPrimaryKeyCheck;

impl Check for AddPrimaryKeyCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "ADD PRIMARY KEY".into(),
            problem: "Adding PRIMARY KEY constraint '<name>' on table '<table>' (<cols>) via ALTER TABLE \
                      acquires an ACCESS EXCLUSIVE lock, blocking all reads and writes. This also implicitly \
                      creates a unique index (blocking operation) and validates all rows for uniqueness.".into(),
            safe_alternative: "Use CREATE UNIQUE INDEX CONCURRENTLY first, then add the constraint:\n\n\
                               1. Create the unique index concurrently (no blocking):\n   \
                               CREATE UNIQUE INDEX CONCURRENTLY <index> ON <table> (<cols>);\n\n\
                               2. Add PRIMARY KEY constraint using the existing index (fast, minimal blocking):\n   \
                               ALTER TABLE <table> ADD CONSTRAINT <name> PRIMARY KEY USING INDEX <index>;\n\n\
                               Benefits:\n\
                               - Table remains readable and writable during index creation\n\
                               - No blocking of SELECT, INSERT, UPDATE, or DELETE operations\n\
                               - Index creation can be canceled if needed\n\
                               - Safe for production deployments on large tables\n\n\
                               Considerations:\n\
                               - Requires Postgres 11+ for PRIMARY KEY USING INDEX\n\
                               - Cannot run CONCURRENTLY inside a transaction block\n\
                               For Diesel migrations: Create metadata.toml with run_in_transaction = false\n\
                               For SQLx migrations: Add -- no-transaction directive at the top of the file\n\
                               - Takes longer than non-concurrent creation\n\
                               - May fail if duplicate or NULL values exist (leaves behind invalid index that should be dropped)\n\n\
                               Note: Ensure all columns in the primary key have NOT NULL constraints before creating the index.".into(),
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

                if c.contype != ConstrType::ConstrPrimary as i32 {
                    return None;
                }

                // Non-empty indexname means USING INDEX, which is the safe pattern
                if !c.indexname.is_empty() {
                    return None;
                }

                let cols = constraint_columns_str(c);

                let constraint_name = if c.conname.is_empty() {
                    format!("{table_name}_pkey")
                } else {
                    c.conname.clone()
                };

                let suggested_index_name = format!("{table_name}_pkey");

                Some(Violation::new(
                    desc.operation.clone(),
                    desc.problem
                        .replace("<table>", &table_name)
                        .replace("<name>", &constraint_name)
                        .replace("<cols>", &cols),
                    desc.safe_alternative
                        .replace("<table>", &table_name)
                        .replace("<name>", &constraint_name)
                        .replace("<cols>", &cols)
                        .replace("<index>", &suggested_index_name),
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
    fn test_detects_add_primary_key_single_column() {
        assert_detects_violation!(
            AddPrimaryKeyCheck,
            "ALTER TABLE users ADD PRIMARY KEY (id);",
            "ADD PRIMARY KEY"
        );
    }

    #[test]
    fn test_detects_add_primary_key_composite() {
        assert_detects_violation!(
            AddPrimaryKeyCheck,
            "ALTER TABLE user_roles ADD PRIMARY KEY (user_id, role_id);",
            "ADD PRIMARY KEY"
        );
    }

    #[test]
    fn test_detects_add_primary_key_with_constraint_name() {
        assert_detects_violation!(
            AddPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (id);",
            "ADD PRIMARY KEY"
        );
    }

    #[test]
    fn test_allows_primary_key_using_index() {
        assert_allows!(
            AddPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;"
        );
    }

    #[test]
    fn test_allows_create_table_with_primary_key() {
        // Creating a table with PK is fine - only ALTER TABLE is problematic
        assert_allows!(
            AddPrimaryKeyCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY, email TEXT);"
        );
    }

    #[test]
    fn test_allows_add_unique_constraint() {
        // UNIQUE constraints are handled by AddUniqueConstraintCheck
        assert_allows!(
            AddPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);"
        );
    }

    #[test]
    fn test_allows_add_foreign_key() {
        assert_allows!(
            AddPrimaryKeyCheck,
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);"
        );
    }

    #[test]
    fn test_allows_add_check_constraint() {
        assert_allows!(
            AddPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            AddPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(AddPrimaryKeyCheck, "SELECT * FROM users;");
    }
}

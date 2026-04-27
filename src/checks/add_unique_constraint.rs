//! Detection for ADD UNIQUE constraint via ALTER TABLE.
//!
//! This check identifies `ALTER TABLE ... ADD UNIQUE` constraint statements, which
//! acquire ACCESS EXCLUSIVE locks that block all table operations.
//!
//! Adding a UNIQUE constraint via ALTER TABLE acquires an ACCESS EXCLUSIVE lock,
//! blocking all reads and writes during index creation. This is more restrictive
//! than CREATE INDEX without CONCURRENTLY (which only blocks writes with a SHARE lock).
//!
//! The safe alternative is to use CREATE UNIQUE INDEX CONCURRENTLY instead.

use crate::checks::pg_helpers::{
    ConstrType, NodeEnum, alter_table_cmds, cmd_def_as_constraint, constraint_columns_str,
    constraint_display_name,
};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct AddUniqueConstraintCheck;

impl Check for AddUniqueConstraintCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "ADD UNIQUE constraint".into(),
            problem: "Adding UNIQUE constraint '<name>' on table '<table>' (<cols>) via ALTER TABLE acquires \
                      an ACCESS EXCLUSIVE lock, blocking all reads and writes during index creation. \
                      Duration depends on table size.".into(),
            safe_alternative: "Use CREATE UNIQUE INDEX CONCURRENTLY instead:\n\n\
                               1. Create the unique index concurrently:\n   \
                               CREATE UNIQUE INDEX CONCURRENTLY <index> ON <table> (<cols>);\n\n\
                               2. (Optional) Add constraint using the existing index:\n   \
                               ALTER TABLE <table> ADD CONSTRAINT <name> UNIQUE USING INDEX <index>;\n\n\
                               Benefits:\n\
                               - Table remains readable and writable during index creation\n\
                               - No blocking of SELECT, INSERT, UPDATE, or DELETE operations\n\
                               - Safe for production deployments on large tables\n\n\
                               Considerations:\n\
                               - Cannot run inside a transaction block\n\
                               For Diesel migrations: Create metadata.toml with run_in_transaction = false\n\
                               For SQLx migrations: Add -- no-transaction directive at the top of the file\n\
                               - Takes longer than non-concurrent creation\n\
                               - May fail if duplicate values exist (leaves behind invalid index that should be dropped)".into(),
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

                if c.contype != ConstrType::ConstrUnique as i32 {
                    return None;
                }

                // Non-empty indexname means USING INDEX, which is the safe pattern
                if !c.indexname.is_empty() {
                    return None;
                }

                let cols = constraint_columns_str(c);

                let constraint_name = constraint_display_name(c);

                let suggested_index_name = if c.conname.is_empty() {
                    format!("{table_name}_unique_idx")
                } else {
                    c.conname.clone()
                };
                let suggested_constraint_name = if c.conname.is_empty() {
                    format!("{table_name}_unique_constraint")
                } else {
                    constraint_name.clone()
                };

                Some(Violation::new(
                    desc.operation.clone(),
                    desc.problem
                        .replace("<table>", &table_name)
                        .replace("<name>", &constraint_name)
                        .replace("<cols>", &cols),
                    desc.safe_alternative
                        .replace("<table>", &table_name)
                        .replace("<name>", &suggested_constraint_name)
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
    fn test_detects_add_unique_constraint_named() {
        assert_detects_violation!(
            AddUniqueConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);",
            "ADD UNIQUE constraint"
        );
    }

    #[test]
    fn test_detects_add_unique_constraint_unnamed() {
        assert_detects_violation!(
            AddUniqueConstraintCheck,
            "ALTER TABLE users ADD UNIQUE (email);",
            "ADD UNIQUE constraint"
        );
    }

    #[test]
    fn test_detects_add_unique_constraint_multiple_columns() {
        assert_detects_violation!(
            AddUniqueConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_username_key UNIQUE (email, username);",
            "ADD UNIQUE constraint"
        );
    }

    #[test]
    fn test_allows_unique_using_index() {
        assert_allows!(
            AddUniqueConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;"
        );
    }

    #[test]
    fn test_ignores_create_unique_index() {
        // CREATE UNIQUE INDEX is handled by AddIndexCheck
        assert_allows!(
            AddUniqueConstraintCheck,
            "CREATE UNIQUE INDEX idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_ignores_create_unique_index_concurrently() {
        // This is the safe way, handled by AddIndexCheck
        assert_allows!(
            AddUniqueConstraintCheck,
            "CREATE UNIQUE INDEX CONCURRENTLY idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_ignores_other_constraints() {
        assert_allows!(
            AddUniqueConstraintCheck,
            "ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);"
        );
    }

    #[test]
    fn test_ignores_foreign_key_constraints() {
        assert_allows!(
            AddUniqueConstraintCheck,
            "ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);"
        );
    }

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            AddUniqueConstraintCheck,
            "ALTER TABLE users ADD COLUMN email TEXT;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddUniqueConstraintCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

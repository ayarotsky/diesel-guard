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
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct AddPrimaryKeyCheck;

impl Check for AddPrimaryKeyCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
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
                    format!("{}_pkey", table_name)
                } else {
                    c.conname.clone()
                };

                let suggested_index_name = format!("{}_pkey", table_name);

                Some(Violation::new(
                    "ADD PRIMARY KEY",
                    format!(
                        "Adding PRIMARY KEY constraint '{constraint}' on table '{table}' ({columns}) via ALTER TABLE acquires an ACCESS EXCLUSIVE lock, \
                        blocking all reads and writes. This also implicitly creates a unique index (blocking operation) and validates all rows for uniqueness.",
                        constraint = constraint_name,
                        table = table_name,
                        columns = cols
                    ),
                    format!(
                        r#"Use CREATE UNIQUE INDEX CONCURRENTLY first, then add the constraint:

1. Create the unique index concurrently (no blocking):
   CREATE UNIQUE INDEX CONCURRENTLY {index_name} ON {table} ({columns});

2. Add PRIMARY KEY constraint using the existing index (fast, minimal blocking):
   ALTER TABLE {table} ADD CONSTRAINT {constraint_name} PRIMARY KEY USING INDEX {index_name};

Benefits:
- Table remains readable and writable during index creation
- No blocking of SELECT, INSERT, UPDATE, or DELETE operations
- Index creation can be canceled if needed
- Safe for production deployments on large tables

Considerations:
- Requires Postgres 11+ for PRIMARY KEY USING INDEX
- Cannot run CONCURRENTLY inside a transaction block
  For Diesel migrations: Create metadata.toml with run_in_transaction = false
  For SQLx migrations: Add -- no-transaction directive at the top of the file
- Takes longer than non-concurrent creation
- May fail if duplicate or NULL values exist (leaves behind invalid index that should be dropped)

Note: Ensure all columns in the primary key have NOT NULL constraints before creating the index."#,
                        index_name = suggested_index_name,
                        table = table_name,
                        columns = cols,
                        constraint_name = constraint_name
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

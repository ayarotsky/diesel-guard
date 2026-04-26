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
    fn describe(&self) -> CheckDescription {
        CheckDescription {
            operation: "ADD UNIQUE constraint".into(),
            problem: "Adding a UNIQUE constraint via ALTER TABLE acquires an ACCESS EXCLUSIVE lock, blocking \
                      all reads and writes during index creation.".into(),
            safe_alternative: "Use CREATE UNIQUE INDEX CONCURRENTLY first, then optionally add the constraint \
                               USING INDEX (no blocking reads or writes, cannot run inside a transaction).".into(),
            script_path: None,
        }
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
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

                Some(Violation::new(
                    self.describe().operation,
                    format!(
                        "Adding UNIQUE constraint '{constraint_name}' on table '{table_name}' ({cols}) via ALTER TABLE acquires an ACCESS EXCLUSIVE lock, \
                        blocking all reads and writes during index creation. Duration depends on table size."
                    ),
                    format!(
                        r"Use CREATE UNIQUE INDEX CONCURRENTLY instead:

1. Create the unique index concurrently:
   CREATE UNIQUE INDEX CONCURRENTLY {index_name} ON {table} ({columns});

2. (Optional) Add constraint using the existing index:
   ALTER TABLE {table} ADD CONSTRAINT {constraint_name} UNIQUE USING INDEX {index_name};

Benefits:
- Table remains readable and writable during index creation
- No blocking of SELECT, INSERT, UPDATE, or DELETE operations
- Safe for production deployments on large tables

Considerations:
- Cannot run inside a transaction block
  For Diesel migrations: Create metadata.toml with run_in_transaction = false
  For SQLx migrations: Add -- no-transaction directive at the top of the file
- Takes longer than non-concurrent creation
- May fail if duplicate values exist (leaves behind invalid index that should be dropped)",
                        index_name = suggested_index_name,
                        table = table_name,
                        columns = cols,
                        constraint_name = if c.conname.is_empty() {
                            format!("{table_name}_unique_constraint")
                        } else {
                            constraint_name
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

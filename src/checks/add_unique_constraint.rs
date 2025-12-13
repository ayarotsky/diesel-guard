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

use crate::checks::Check;
use crate::violation::Violation;
use sqlparser::ast::{AlterTable, AlterTableOperation, Statement, TableConstraint};

pub struct AddUniqueConstraintCheck;

impl Check for AddUniqueConstraintCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let Statement::AlterTable(AlterTable {
            name, operations, ..
        }) = stmt
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

                if let TableConstraint::Unique(unique) = constraint {
                    let constraint_name = unique
                        .name
                        .as_ref()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "<unnamed>".to_string());

                    let cols = unique
                        .columns
                        .iter()
                        .map(|ic| ic.column.expr.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");

                    let suggested_index_name = if let Some(name_ident) = &unique.name {
                        name_ident.to_string()
                    } else {
                        format!("{}_unique_idx", table_name)
                    };

                    Some(Violation::new(
                        "ADD UNIQUE constraint",
                        format!(
                            "Adding UNIQUE constraint '{constraint}' on table '{table}' ({columns}) via ALTER TABLE acquires an ACCESS EXCLUSIVE lock, \
                            blocking all reads and writes during index creation. Duration depends on table size.",
                            constraint = constraint_name,
                            table = table_name,
                            columns = cols
                        ),
                        format!(
                            r#"Use CREATE UNIQUE INDEX CONCURRENTLY instead:

1. Create the unique index concurrently:
   CREATE UNIQUE INDEX CONCURRENTLY {index_name} ON {table} ({columns});

2. (Optional) Add constraint using the existing index:
   ALTER TABLE {table} ADD CONSTRAINT {constraint_name} UNIQUE USING INDEX {index_name};

Benefits:
- Table remains readable and writable during index creation
- No blocking of SELECT, INSERT, UPDATE, or DELETE operations
- Safe for production deployments on large tables

Considerations:
- Cannot run inside a transaction block (requires metadata.toml with run_in_transaction = false)
- Takes longer than non-concurrent creation
- May fail if duplicate values exist (leaves behind invalid index that should be dropped)"#,
                            index_name = suggested_index_name,
                            table = table_name,
                            columns = cols,
                            constraint_name = if unique.name.is_some() {
                                constraint_name
                            } else {
                                format!("{}_unique_constraint", table_name)
                            }
                        ),
                    ))
                } else {
                    None
                }
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

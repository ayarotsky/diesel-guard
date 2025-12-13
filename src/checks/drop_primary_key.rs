//! Detection for DROP PRIMARY KEY operations.
//!
//! This check identifies `ALTER TABLE` statements that drop primary key constraints.
//!
//! Dropping a primary key acquires an ACCESS EXCLUSIVE lock, blocking all operations.
//! More critically, it breaks foreign key relationships in other tables that reference
//! this table, and removes the uniqueness constraint that applications may depend on.
//!
//! **Limitation:** This check uses heuristic detection based on constraint naming patterns.
//! It may not detect primary keys with non-standard names, and may occasionally flag
//! non-primary-key constraints that follow similar naming patterns.
//!
//! **Future Enhancement:** Future versions of diesel-guard will support optional database
//! connections to verify constraint types with certainty.

use crate::checks::Check;
use crate::violation::Violation;
use regex::Regex;
use sqlparser::ast::{AlterTable, AlterTableOperation, Statement};
use std::sync::LazyLock;

/// Uses common PostgreSQL naming conventions:
/// - `*_pkey` (standard PostgreSQL convention)
/// - `*_pk` suffix
/// - `pk_*` prefix
/// - `*_primary_key` variations
static PRIMARY_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)((_pkey|_pk)$|^pk_|_primary_key|primarykey)")
        .expect("Invalid primary key regex pattern")
});

pub struct DropPrimaryKeyCheck;

impl DropPrimaryKeyCheck {
    /// Check if a constraint name likely refers to a primary key.
    fn is_likely_primary_key(constraint_name: &str) -> bool {
        PRIMARY_KEY_PATTERN.is_match(constraint_name)
    }
}

impl Check for DropPrimaryKeyCheck {
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
                let AlterTableOperation::DropConstraint {
                    name: constraint_name,
                    ..
                } = op
                else {
                    return None;
                };

                let constraint_name_str = constraint_name.to_string();

                // Only flag if the constraint name matches primary key patterns
                if !Self::is_likely_primary_key(&constraint_name_str) {
                    return None;
                }

                Some(Violation::new(
                    "DROP PRIMARY KEY",
                    format!(
                        "Dropping primary key constraint '{constraint}' from table '{table}' requires an ACCESS EXCLUSIVE lock, blocking all operations. \
                        More critically, this breaks foreign key relationships in other tables and removes the uniqueness constraint.",
                        constraint = constraint_name_str,
                        table = table_name
                    ),
                    format!(r#"Consider the following before dropping a primary key:

1. Identify all foreign key dependencies:
   SELECT
     tc.table_name, kcu.column_name, rc.constraint_name
   FROM information_schema.table_constraints tc
   JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
   JOIN information_schema.referential_constraints rc ON tc.constraint_name = rc.unique_constraint_name
   WHERE tc.table_name = '{table}' AND tc.constraint_type = 'PRIMARY KEY';

2. If you must change the primary key:
   - Create the new primary key constraint FIRST
   - Update all foreign keys to reference the new key
   - Then drop the old primary key

3. If migrating to a different key strategy:
   - Consider using a transition period with both keys
   - Update application code gradually
   - Drop the old key only after full migration

Note: This check uses naming pattern detection (e.g., '{constraint}' matches '*_pkey' pattern) and may not catch all cases.
Future versions will support database connections for accurate constraint type verification.
If this is a false positive, use a safety-assured block."#,
                        table = table_name,
                        constraint = constraint_name_str
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
    fn test_detects_drop_primary_key_pkey_suffix() {
        assert_detects_violation!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users DROP CONSTRAINT users_pkey;",
            "DROP PRIMARY KEY"
        );
    }

    #[test]
    fn test_detects_drop_primary_key_pk_suffix() {
        assert_detects_violation!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users DROP CONSTRAINT users_pk;",
            "DROP PRIMARY KEY"
        );
    }

    #[test]
    fn test_detects_drop_primary_key_pk_prefix() {
        assert_detects_violation!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users DROP CONSTRAINT pk_users;",
            "DROP PRIMARY KEY"
        );
    }

    #[test]
    fn test_detects_drop_primary_key_primary_key_in_name() {
        assert_detects_violation!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users DROP CONSTRAINT users_primary_key;",
            "DROP PRIMARY KEY"
        );
    }

    #[test]
    fn test_allows_drop_unique_constraint() {
        assert_allows!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users DROP CONSTRAINT users_email_key;"
        );
    }

    #[test]
    fn test_allows_drop_foreign_key_constraint() {
        assert_allows!(
            DropPrimaryKeyCheck,
            "ALTER TABLE posts DROP CONSTRAINT posts_user_id_fkey;"
        );
    }

    #[test]
    fn test_allows_drop_check_constraint() {
        assert_allows!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users DROP CONSTRAINT users_age_check;"
        );
    }

    #[test]
    fn test_ignores_add_constraint() {
        assert_allows!(
            DropPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (id);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            DropPrimaryKeyCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

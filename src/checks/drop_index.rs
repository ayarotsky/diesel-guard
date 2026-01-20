//! Detection for DROP INDEX without CONCURRENTLY.
//!
//! This check identifies `DROP INDEX` statements that don't use the CONCURRENTLY
//! option, which blocks queries during the index removal.
//!
//! Dropping an index without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock on the
//! table, which blocks all queries (SELECT, INSERT, UPDATE, DELETE) until the drop
//! operation completes. Duration depends on system load and concurrent transactions.
//!
//! Using CONCURRENTLY (PostgreSQL 9.2+) allows the index to be dropped while permitting
//! concurrent queries, though it takes longer and cannot be run inside a transaction block.
//!
//! **Parser Handling**: sqlparser cannot parse `DROP INDEX CONCURRENTLY` syntax, but
//! diesel-guard detects this safe pattern and treats it as valid (returns no violations).
//! A warning is shown that the file contains this safe pattern. Like CREATE INDEX
//! CONCURRENTLY, it requires `metadata.toml` with `run_in_transaction = false`.

use crate::checks::{if_exists_clause, Check};
use crate::violation::Violation;
use sqlparser::ast::{ObjectType, Statement};

pub struct DropIndexCheck;

impl Check for DropIndexCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let mut violations = vec![];

        if let Statement::Drop {
            object_type,
            if_exists,
            names,
            ..
        } = stmt
        {
            // Check if this is dropping an index
            if matches!(object_type, ObjectType::Index) {
                // Flag all DROP INDEX statements since sqlparser cannot distinguish
                // DROP INDEX CONCURRENTLY (which fails to parse)
                for name in names {
                    let index_name = name.to_string();
                    let if_exists_str = if_exists_clause(*if_exists);

                    violations.push(Violation::new(
                        "DROP INDEX without CONCURRENTLY",
                        format!(
                            "Dropping index '{index}'{if_exists} without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock, blocking all \
                            queries (SELECT, INSERT, UPDATE, DELETE) on the table until complete. Duration depends on system load and concurrent transactions.",
                            index = index_name,
                            if_exists = if_exists_str
                        ),
                        format!(r#"Use CONCURRENTLY to drop the index without blocking queries:
   DROP INDEX CONCURRENTLY{if_exists} {index};

Note: CONCURRENTLY requires PostgreSQL 9.2+ and cannot be run inside a transaction block.

For Diesel migrations:
1. Create metadata.toml in your migration directory:
   run_in_transaction = false

2. Use DROP INDEX CONCURRENTLY in your up.sql:
   DROP INDEX CONCURRENTLY{if_exists} {index};

For SQLx migrations:
1. Add the no-transaction directive at the top of your migration file:
   -- no-transaction

2. Use DROP INDEX CONCURRENTLY:
   DROP INDEX CONCURRENTLY{if_exists} {index};

Considerations:
- Takes longer to complete than regular DROP INDEX
- Allows concurrent SELECT, INSERT, UPDATE, DELETE operations
- If it fails, the index may be marked "invalid" and should be dropped again
- Cannot be rolled back (no transaction support)"#,
                            if_exists = if_exists_str,
                            index = index_name
                        ),
                    ));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_drop_index() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX idx_users_email;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_if_exists() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX IF EXISTS idx_users_email;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_cascade() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX idx_users_email CASCADE;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_index_restrict() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX idx_users_email RESTRICT;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_drop_multiple_indexes() {
        use crate::checks::test_utils::parse_sql;

        let check = DropIndexCheck;
        let stmt = parse_sql("DROP INDEX idx1, idx2, idx3;");

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 3, "Should detect all 3 indexes");
        assert!(violations
            .iter()
            .all(|v| v.operation == "DROP INDEX without CONCURRENTLY"));
    }

    #[test]
    fn test_detects_drop_index_if_exists_cascade() {
        assert_detects_violation!(
            DropIndexCheck,
            "DROP INDEX IF EXISTS idx_users_email CASCADE;",
            "DROP INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_ignores_other_drop_statements() {
        assert_allows!(DropIndexCheck, "DROP TABLE users;");
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            DropIndexCheck,
            "CREATE INDEX idx_users_email ON users(email);"
        );
    }
}

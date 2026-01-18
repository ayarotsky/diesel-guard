//! Detection for DROP TABLE operations.
//!
//! This check identifies `DROP TABLE` statements, which permanently delete tables
//! and all their data. DROP TABLE acquires an ACCESS EXCLUSIVE lock and cannot be
//! undone after the transaction commits.
//!
//! Dropping a table is an irreversible operation that deletes all data, indexes,
//! triggers, and constraints. Foreign key relationships in other tables may block
//! the drop or cause cascading deletes if CASCADE is used.
//!
//! The recommended approach is to verify the table is no longer in use, ensure
//! backups exist, and check for foreign key dependencies before dropping.

use crate::checks::{if_exists_clause, Check};
use crate::violation::Violation;
use sqlparser::ast::{ObjectType, Statement};

pub struct DropTableCheck;

impl Check for DropTableCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let mut violations = vec![];

        if let Statement::Drop {
            object_type,
            if_exists,
            names,
            cascade,
            restrict,
            ..
        } = stmt
        {
            // Check if this is dropping a table
            if matches!(object_type, ObjectType::Table) {
                for name in names {
                    let table_name = name.to_string();
                    let if_exists_str = if_exists_clause(*if_exists);

                    // Build modifiers string for display
                    let mut modifiers = String::new();
                    if *cascade {
                        modifiers.push_str(" CASCADE");
                    }
                    if *restrict {
                        modifiers.push_str(" RESTRICT");
                    }

                    violations.push(Violation::new(
                        "DROP TABLE",
                        format!(
                            "Dropping table '{table}' permanently deletes all data and acquires an ACCESS EXCLUSIVE lock. \
                            This operation cannot be undone after the transaction commits.",
                            table = table_name
                        ),
                        format!(r#"Before dropping a table in production:

1. Verify this is intentional and the table is no longer in use
2. Ensure a backup exists or data has been migrated
3. Check for foreign key dependencies that may block the drop

If this drop is intentional, wrap it in a safety-assured block:
   -- safety-assured:start
   DROP TABLE{if_exists} {table}{modifiers};
   -- safety-assured:end

Note: DROP TABLE acquires ACCESS EXCLUSIVE lock, blocking all operations until complete."#,
                            if_exists = if_exists_str,
                            table = table_name,
                            modifiers = modifiers
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
    fn test_detects_drop_table() {
        assert_detects_violation!(DropTableCheck, "DROP TABLE users;", "DROP TABLE");
    }

    #[test]
    fn test_detects_drop_table_if_exists() {
        assert_detects_violation!(DropTableCheck, "DROP TABLE IF EXISTS users;", "DROP TABLE");
    }

    #[test]
    fn test_detects_drop_table_cascade() {
        assert_detects_violation!(DropTableCheck, "DROP TABLE users CASCADE;", "DROP TABLE");
    }

    #[test]
    fn test_detects_drop_table_restrict() {
        assert_detects_violation!(DropTableCheck, "DROP TABLE users RESTRICT;", "DROP TABLE");
    }

    #[test]
    fn test_detects_drop_multiple_tables() {
        use crate::checks::test_utils::parse_sql;

        let check = DropTableCheck;
        let stmt = parse_sql("DROP TABLE users, orders, products;");

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 3, "Should detect all 3 tables");
        assert!(violations.iter().all(|v| v.operation == "DROP TABLE"));
    }

    #[test]
    fn test_ignores_drop_index() {
        assert_allows!(DropTableCheck, "DROP INDEX idx_users_email;");
    }

    #[test]
    fn test_ignores_truncate() {
        assert_allows!(DropTableCheck, "TRUNCATE TABLE users;");
    }

    #[test]
    fn test_ignores_create_table() {
        assert_allows!(
            DropTableCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_ignores_alter_table() {
        assert_allows!(
            DropTableCheck,
            "ALTER TABLE users ADD COLUMN email VARCHAR(255);"
        );
    }
}

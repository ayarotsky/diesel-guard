//! Detection for CREATE INDEX without CONCURRENTLY.
//!
//! This check identifies `CREATE INDEX` statements that don't use the CONCURRENTLY
//! option, which blocks write operations during the index build.
//!
//! Creating an index without CONCURRENTLY acquires a SHARE lock on the table, which
//! blocks write operations (INSERT, UPDATE, DELETE). Duration depends on table size.
//! Reads (SELECT) are still allowed.
//!
//! Using CONCURRENTLY allows the index to be built while permitting concurrent writes,
//! though it takes longer and cannot be run inside a transaction block.

use crate::checks::{display_or_default, unique_prefix, Check};
use crate::violation::Violation;
use sqlparser::ast::Statement;

pub struct AddIndexCheck;

impl Check for AddIndexCheck {
    fn check(&self, stmt: &Statement) -> Vec<Violation> {
        let mut violations = vec![];

        if let Statement::CreateIndex(create_index) = stmt {
            // Check if CONCURRENTLY is NOT used
            if !create_index.concurrently {
                let table_name = create_index.table_name.to_string();
                let index_name = display_or_default(create_index.name.as_ref(), "<unnamed>");
                let unique_str = unique_prefix(create_index.unique);

                violations.push(Violation::new(
                    "ADD INDEX without CONCURRENTLY",
                    format!(
                        "Creating {unique}index '{index}' on table '{table}' without CONCURRENTLY acquires a SHARE lock, blocking writes \
                        (INSERT, UPDATE, DELETE). Duration depends on table size. Reads are still allowed.",
                        unique = unique_str, index = index_name, table = table_name
                    ),
                    format!(r#"Use CONCURRENTLY to build the index without blocking writes:
   CREATE {unique}INDEX CONCURRENTLY {index} ON {table};

Note: CONCURRENTLY takes longer and uses more resources, but allows concurrent INSERT, UPDATE, and DELETE operations. The index build may fail if there are deadlocks or unique constraint violations.

For Diesel migrations:
1. Create metadata.toml in your migration directory:
   run_in_transaction = false

2. Use CREATE INDEX CONCURRENTLY in your up.sql:
   CREATE {unique}INDEX CONCURRENTLY {index} ON {table}(...);

For SQLx migrations:
1. Add the no-transaction directive at the top of your migration file:
   -- no-transaction

2. Use CREATE INDEX CONCURRENTLY:
   CREATE {unique}INDEX CONCURRENTLY {index} ON {table}(...);

Considerations:
- Requires more total work and takes longer to complete
- If it fails, it leaves behind an "invalid" index that should be dropped"#,
                        unique = unique_str,
                        index = index_name,
                        table = table_name
                    ),
                ));
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_create_index_without_concurrently() {
        assert_detects_violation!(
            AddIndexCheck,
            "CREATE INDEX idx_users_email ON users(email);",
            "ADD INDEX without CONCURRENTLY"
        );
    }

    #[test]
    fn test_detects_create_unique_index_without_concurrently() {
        let check = AddIndexCheck;
        let stmt = parse_sql("CREATE UNIQUE INDEX idx_users_email ON users(email);");

        let violations = check.check(&stmt);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
        assert!(violations[0].problem.contains("UNIQUE"));
    }

    #[test]
    fn test_allows_create_index_with_concurrently() {
        assert_allows!(
            AddIndexCheck,
            "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_allows_create_unique_index_with_concurrently() {
        assert_allows!(
            AddIndexCheck,
            "CREATE UNIQUE INDEX CONCURRENTLY idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(AddIndexCheck, "CREATE TABLE users (id SERIAL PRIMARY KEY);");
    }
}

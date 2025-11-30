//! Detection for CREATE INDEX without CONCURRENTLY.
//!
//! This check identifies `CREATE INDEX` statements that don't use the CONCURRENTLY
//! option, which can cause table locks and downtime on production databases.
//!
//! Creating an index without CONCURRENTLY acquires a SHARE lock on the table, which
//! blocks all write operations (INSERT, UPDATE, DELETE) for the duration of the index
//! build. On large tables, this can take hours and cause significant downtime.
//!
//! Using CONCURRENTLY allows the index to be built while permitting concurrent writes,
//! though it takes longer and cannot be run inside a transaction block.

use crate::checks::Check;
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::Statement;

pub struct AddIndexCheck;

impl Check for AddIndexCheck {
    fn name(&self) -> &str {
        "add_index_without_concurrently"
    }

    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>> {
        let mut violations = vec![];

        if let Statement::CreateIndex(create_index) = stmt {
            // Check if CONCURRENTLY is NOT used
            if !create_index.concurrently {
                let table_name = create_index.table_name.to_string();
                let index_name = create_index
                    .name
                    .as_ref()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "<unnamed>".to_string());

                let unique_str = if create_index.unique { "UNIQUE " } else { "" };

                violations.push(Violation::new(
                    "ADD INDEX without CONCURRENTLY",
                    format!(
                        "Creating {}index '{}' on table '{}' without CONCURRENTLY locks the table for writes. \
                        This can cause downtime on production databases with active traffic.",
                        unique_str, index_name, table_name
                    ),
                    format!(
                        "Use CONCURRENTLY to build the index without blocking writes:\n   \
                         CREATE {}INDEX CONCURRENTLY {} ON {};\n\n\
                         Note: CONCURRENTLY takes longer and uses more resources, but allows \
                         concurrent INSERT, UPDATE, and DELETE operations. The index build may \
                         fail if there are deadlocks or unique constraint violations.\n\n\
                         Considerations:\n\
                         - Cannot be run inside a transaction block\n\
                         - Requires more total work and takes longer to complete\n\
                         - If it fails, it leaves behind an \"invalid\" index that should be dropped",
                        unique_str,
                        index_name,
                        table_name
                    ),
                ));
            }
        }

        Ok(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::test_utils::parse_sql;

    #[test]
    fn test_detects_create_index_without_concurrently() {
        let check = AddIndexCheck;
        let stmt = parse_sql("CREATE INDEX idx_users_email ON users(email);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
    }

    #[test]
    fn test_detects_create_unique_index_without_concurrently() {
        let check = AddIndexCheck;
        let stmt = parse_sql("CREATE UNIQUE INDEX idx_users_email ON users(email);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
        assert!(violations[0].problem.contains("UNIQUE"));
    }

    #[test]
    fn test_allows_create_index_with_concurrently() {
        let check = AddIndexCheck;
        let stmt = parse_sql("CREATE INDEX CONCURRENTLY idx_users_email ON users(email);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_allows_create_unique_index_with_concurrently() {
        let check = AddIndexCheck;
        let stmt = parse_sql("CREATE UNIQUE INDEX CONCURRENTLY idx_users_email ON users(email);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_ignores_other_statements() {
        let check = AddIndexCheck;
        let stmt = parse_sql("CREATE TABLE users (id SERIAL PRIMARY KEY);");

        let violations = check.check(&stmt).unwrap();
        assert_eq!(violations.len(), 0);
    }
}

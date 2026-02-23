//! Detection for TRUNCATE TABLE operations.
//!
//! This check identifies `TRUNCATE TABLE` statements, which acquire an ACCESS EXCLUSIVE
//! lock and block all operations on the table.
//!
//! TRUNCATE acquires an ACCESS EXCLUSIVE lock, blocking all reads and writes during the
//! operation. Unlike DELETE, TRUNCATE cannot be batched or throttled, making it unsuitable
//! for removing data from large tables in production.
//!
//! The recommended approach is to use DELETE with batching to remove rows incrementally,
//! allowing concurrent access to the table.

use crate::checks::pg_helpers::{range_var_name, NodeEnum};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct TruncateTableCheck;

impl Check for TruncateTableCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let NodeEnum::TruncateStmt(truncate) = node else {
            return vec![];
        };

        truncate
            .relations
            .iter()
            .filter_map(|rel_node| {
                if let Some(NodeEnum::RangeVar(rv)) = &rel_node.node {
                    let table_name_str = range_var_name(rv);

                    Some(Violation::new(
                        "TRUNCATE TABLE",
                        format!(
                            "TRUNCATE TABLE on '{table}' acquires an ACCESS EXCLUSIVE lock, blocking all operations (reads and writes). \
                            Unlike DELETE, TRUNCATE cannot be batched or throttled, making it unsafe for large tables in production.",
                            table = table_name_str
                        ),
                        format!(r#"Use DELETE with batching instead:

1. Delete rows in small batches to allow concurrent access:
   DELETE FROM {table} WHERE id IN (
     SELECT id FROM {table} LIMIT 1000
   );

2. Repeat the batched DELETE until all rows are removed.

3. (Optional) If you need to reset sequences:
   ALTER SEQUENCE {table}_id_seq RESTART WITH 1;

4. (Optional) Run VACUUM to reclaim space:
   VACUUM {table};

Note: If you absolutely must use TRUNCATE (e.g., in a test environment), use a safety-assured block."#,
                            table = table_name_str
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
    use crate::{assert_allows, assert_detects_n_violations, assert_detects_violation};

    #[test]
    fn test_detects_truncate_table() {
        assert_detects_violation!(
            TruncateTableCheck,
            "TRUNCATE TABLE users;",
            "TRUNCATE TABLE"
        );
    }

    #[test]
    fn test_detects_truncate_multiple_tables() {
        assert_detects_n_violations!(
            TruncateTableCheck,
            "TRUNCATE TABLE users, orders;",
            2,
            "TRUNCATE TABLE"
        );
    }

    #[test]
    fn test_detects_truncate_with_cascade() {
        assert_detects_violation!(
            TruncateTableCheck,
            "TRUNCATE TABLE users CASCADE;",
            "TRUNCATE TABLE"
        );
    }

    #[test]
    fn test_ignores_delete_statement() {
        assert_allows!(TruncateTableCheck, "DELETE FROM users;");
    }

    #[test]
    fn test_ignores_drop_table() {
        assert_allows!(TruncateTableCheck, "DROP TABLE users;");
    }
}

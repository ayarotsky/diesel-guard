//! Detection for TRUNCATE TABLE operations.
//!
//! This check identifies `TRUNCATE TABLE` statements, which acquire an ACCESS EXCLUSIVE
//! lock and block all operations on the table.
//!
//! TRUNCATE acquires an ACCESS EXCLUSIVE lock, blocking all reads and writes during the
//! operation. Unlike DELETE, TRUNCATE cannot be batched or throttled, making it unsuitable
//! for removing data from large tables in production.
//!
//! **When this fires legitimately:** TRUNCATE is often intentional in migrations — for
//! example, clearing a lookup/seed table before re-populating it, wiping a staging
//! environment, or truncating a table that is known to be empty or small. In those cases,
//! silence the check per-statement with a `safety-assured` block, or project-wide with
//! `warn_checks = ["TruncateTableCheck"]` (warning only) or
//! `disable_checks = ["TruncateTableCheck"]` (fully disabled).

use crate::checks::pg_helpers::{NodeEnum, range_var_name};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

pub struct TruncateTableCheck;

impl Check for TruncateTableCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "TRUNCATE TABLE".into(),
            problem: "TRUNCATE TABLE on '<table>' acquires an ACCESS EXCLUSIVE lock, blocking all reads \
                      and writes. Unlike DELETE, it cannot be batched or throttled. This is safe for \
                      empty or small tables or non-production environments, but dangerous on large \
                      production tables.".into(),
            safe_alternative: "If this table can be large in production, prefer batched DELETE:\n\n\
                               1. Delete rows in small batches:\n   \
                               DELETE FROM <table> WHERE id IN (\n     SELECT id FROM <table> LIMIT 1000\n   );\n\n\
                               2. Repeat until all rows are removed.\n\n\
                               3. (Optional) Reset sequences:\n   \
                               ALTER SEQUENCE <table>_id_seq RESTART WITH 1;\n\n\
                               4. (Optional) Reclaim space:\n   \
                               VACUUM <table>;\n\n\
                               If TRUNCATE is intentional (e.g. lookup table, test/staging environment,\n\
                               or table is known to be small), silence this check:\n\n  \
                               Per-statement — wrap in a safety-assured block:\n    \
                               -- safety-assured:start\n    \
                               -- Safe because: lookup table, always small\n    \
                               TRUNCATE TABLE <table>;\n    \
                               -- safety-assured:end\n\n  \
                               Project-wide as a warning (reported but non-blocking):\n    \
                               # diesel-guard.toml\n    \
                               warn_checks = [\"TruncateTableCheck\"]\n\n  \
                               Project-wide silenced:\n    \
                               # diesel-guard.toml\n    \
                               disable_checks = [\"TruncateTableCheck\"]".into(),
            script_path: None,
        }]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let descriptions = self.describe();
        let desc = &descriptions[0];
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
                        desc.operation.clone(),
                        desc.problem.replace("<table>", &table_name_str),
                        desc.safe_alternative.replace("<table>", &table_name_str),
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

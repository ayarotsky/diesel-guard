//! Detection for DELETE and UPDATE statements with no WHERE clause.
//!
//! A `DELETE FROM table` or `UPDATE table SET ...` without a WHERE clause affects every row
//! in the table. In a migration this is almost always a mistake — a forgotten filter rather
//! than intentional bulk mutation. The ROW EXCLUSIVE
//! lock held for a full-table write can cause severe contention on large tables.

use crate::checks::pg_helpers::{NodeEnum, range_var_name};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;
use pg_query::protobuf::RangeVar;

pub struct MutationWithoutWhereCheck;

impl Check for MutationWithoutWhereCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        match node {
            NodeEnum::DeleteStmt(stmt) => check_mutation(
                stmt.where_clause.is_some(),
                stmt.relation.as_ref(),
                "DELETE without WHERE",
                |table| {
                    format!(
                        "DELETE on '{table}' has no WHERE clause and will remove every row in the \
                        table. This is almost always a mistake in a migration. The lock held for \
                        a full-table delete can cause severe contention on large tables."
                    )
                },
                |table| {
                    format!(
                        "Add a WHERE clause to target only the rows you intend to remove:\n\n\
                        DELETE FROM {table} WHERE <condition>;\n\n\
                        If a full-table delete is intentional, wrap it in a safety-assured block:\n\n  \
                        -- safety-assured:start\n  \
                        -- Safe because: table is a temporary staging table, always empty in production\n  \
                        DELETE FROM {table};\n  \
                        -- safety-assured:end"
                    )
                },
            ),
            NodeEnum::UpdateStmt(stmt) => check_mutation(
                stmt.where_clause.is_some(),
                stmt.relation.as_ref(),
                "UPDATE without WHERE",
                |table| {
                    format!(
                        "UPDATE on '{table}' has no WHERE clause and will modify every row in the \
                        table. This is almost always a mistake in a migration. A full-table update \
                        holds a ROW EXCLUSIVE lock for the duration and rewrites every row, which \
                        can cause severe contention and bloat on large tables."
                    )
                },
                |table| {
                    format!(
                        "Add a WHERE clause to target only the rows you intend to update:\n\n\
                        UPDATE {table} SET ... WHERE <condition>;\n\n\
                        If updating every row is intentional (e.g. a one-time backfill), \
                        add a safety-assured block:\n\n  \
                        -- safety-assured:start\n  \
                        -- Safe because: one-time backfill, table has fewer than 10k rows\n  \
                        UPDATE {table} SET ...;\n  \
                        -- safety-assured:end"
                    )
                },
            ),
            _ => vec![],
        }
    }
}

fn check_mutation(
    where_clause_present: bool,
    relation: Option<&RangeVar>,
    operation: &'static str,
    problem: impl FnOnce(&str) -> String,
    safe_alternative: impl FnOnce(&str) -> String,
) -> Vec<Violation> {
    if where_clause_present {
        return vec![];
    }
    let table = relation.map(range_var_name).unwrap_or_default();
    vec![Violation::new(
        operation,
        problem(&table),
        safe_alternative(&table),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_delete_without_where() {
        assert_detects_violation!(
            MutationWithoutWhereCheck,
            "DELETE FROM users;",
            "DELETE without WHERE"
        );
    }

    #[test]
    fn test_detects_update_without_where() {
        assert_detects_violation!(
            MutationWithoutWhereCheck,
            "UPDATE users SET active = false;",
            "UPDATE without WHERE"
        );
    }

    #[test]
    fn test_allows_delete_with_where() {
        assert_allows!(MutationWithoutWhereCheck, "DELETE FROM users WHERE id = 1;");
    }

    #[test]
    fn test_allows_update_with_where() {
        assert_allows!(
            MutationWithoutWhereCheck,
            "UPDATE users SET active = false WHERE last_login < '2020-01-01';"
        );
    }

    #[test]
    fn test_ignores_truncate() {
        assert_allows!(MutationWithoutWhereCheck, "TRUNCATE TABLE users;");
    }

    #[test]
    fn test_ignores_select() {
        assert_allows!(MutationWithoutWhereCheck, "SELECT * FROM users;");
    }

    #[test]
    fn test_ignores_insert() {
        assert_allows!(
            MutationWithoutWhereCheck,
            "INSERT INTO users (name) VALUES ('alice');"
        );
    }
}

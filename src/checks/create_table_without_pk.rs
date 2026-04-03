//! Detection for CREATE TABLE statements that define no primary key.
//!
//! Tables without a primary key cannot be replicated with logical replication:
//! replication slots require a primary key or an explicit REPLICA IDENTITY setting
//! (FULL or USING INDEX). They are also a general anti-pattern — without a PK
//! there is no guaranteed way to uniquely identify a row for updates, deletes, or
//! foreign key references.

use crate::checks::pg_helpers::{ConstrType, NodeEnum, column_has_constraint, range_var_name};
use crate::checks::{Check, Config, MigrationContext};
use crate::violation::Violation;

const CONSTR_PRIMARY: i32 = ConstrType::ConstrPrimary as i32;

pub struct CreateTableWithoutPkCheck;

impl Check for CreateTableWithoutPkCheck {
    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::CreateStmt(stmt) = node else {
            return vec![];
        };

        // TEMPORARY tables are session-scoped and never replicated — no PK required.
        if stmt
            .relation
            .as_ref()
            .is_some_and(|r| r.relpersistence == "t")
        {
            return vec![];
        }

        // LIKE-based tables copy their definition from another table; whether a PK
        // is inherited depends on the INCLUDING CONSTRAINTS option, which we cannot
        // know at parse time.
        let has_like = stmt
            .table_elts
            .iter()
            .any(|elt| matches!(&elt.node, Some(NodeEnum::TableLikeClause(_))));
        if has_like {
            return vec![];
        }

        let has_pk = stmt.table_elts.iter().any(|elt| match &elt.node {
            Some(NodeEnum::ColumnDef(col)) => column_has_constraint(col, CONSTR_PRIMARY),
            Some(NodeEnum::Constraint(c)) => c.contype == CONSTR_PRIMARY,
            _ => false,
        });

        if has_pk {
            return vec![];
        }

        let table_name = stmt
            .relation
            .as_ref()
            .map(range_var_name)
            .unwrap_or_default();

        vec![Violation::new(
            "CREATE TABLE without PRIMARY KEY",
            format!(
                "Table '{table_name}' is defined without a primary key. \
                Tables without a primary key cannot use logical replication: replication slots \
                require a primary key or an explicit REPLICA IDENTITY setting. \
                They are also harder to work with — without a PK there is no guaranteed way \
                to uniquely identify a row for updates, deletes, or foreign key references."
            ),
            format!(
                r"Add a primary key to the table definition.

Option 1 — identity column (recommended):
   CREATE TABLE {table_name} (
     id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
     ...
   );

Option 2 — UUID:
   CREATE TABLE {table_name} (
     id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
     ...
   );

Option 3 — separate constraint:
   CREATE TABLE {table_name} (
     id BIGINT GENERATED ALWAYS AS IDENTITY,
     ...,
     PRIMARY KEY (id)
   );

If the table is intentionally without a primary key (e.g., a log table where you
will set REPLICA IDENTITY FULL), use a safety-assured block to bypass this check."
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_table_without_pk() {
        assert_detects_violation!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE events (name TEXT, payload JSONB);",
            "CREATE TABLE without PRIMARY KEY"
        );
    }

    #[test]
    fn test_detects_single_column_table_without_pk() {
        assert_detects_violation!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE tags (name TEXT);",
            "CREATE TABLE without PRIMARY KEY"
        );
    }

    #[test]
    fn test_allows_inline_pk() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY, email TEXT);"
        );
    }

    #[test]
    fn test_allows_table_level_pk() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE users (id BIGINT, email TEXT, PRIMARY KEY (id));"
        );
    }

    #[test]
    fn test_allows_composite_pk() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE order_items (order_id BIGINT, item_id BIGINT, PRIMARY KEY (order_id, item_id));"
        );
    }

    #[test]
    fn test_allows_identity_column_pk() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE users (id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY, name TEXT);"
        );
    }

    #[test]
    fn test_ignores_temp_table() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "CREATE TEMP TABLE staging (raw TEXT);"
        );
    }

    #[test]
    fn test_ignores_like_clause() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "CREATE TABLE users_archive (LIKE users);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            CreateTableWithoutPkCheck,
            "ALTER TABLE users ADD COLUMN age INT;"
        );
    }
}

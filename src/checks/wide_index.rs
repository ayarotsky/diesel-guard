//! Detection for wide indexes (indexes with 4+ columns).
//!
//! This check identifies `CREATE INDEX` statements with more than 3 columns.
//!
//! Wide indexes (with 4+ columns) are often ineffective because Postgres can only use
//! the index efficiently when filtering on the leftmost columns in order. They also
//! consume more storage and slow down write operations.
//!
//! Consider using partial indexes, separate narrower indexes, or rethinking your
//! query patterns instead.

use crate::checks::pg_helpers::{NodeEnum, range_var_name};
use crate::checks::{Check, CheckDescription, Config, MigrationContext};
use crate::violation::Violation;

const MAX_COLUMNS: usize = 3;

pub struct WideIndexCheck;

impl Check for WideIndexCheck {
    fn describe(&self) -> Vec<CheckDescription> {
        vec![CheckDescription {
            operation: "CREATE INDEX with too many columns".into(),
            problem: "Index '<index>' on table '<table>' has <count> columns (<cols>). Wide indexes \
                      (4+ columns) are rarely effective because Postgres can only use them efficiently \
                      when filtering on leftmost columns in order. They also increase storage costs and \
                      slow down writes.".into(),
            safe_alternative: "Consider these alternatives:\n\n\
                               1. Use a partial index for specific query patterns:\n   \
                               CREATE INDEX <index> ON <table>(<first_col>)\n   \
                               WHERE <condition>;\n\n\
                               2. Create separate narrower indexes for different queries:\n   \
                               CREATE INDEX idx_<table>_<first_col> ON <table>(<first_col>);\n   \
                               CREATE INDEX idx_<table>_<second_col> ON <table>(<second_col>);\n\n\
                               3. Rethink your query patterns - do you really need to filter on all <count> columns?\n\n\
                               4. Use a covering index (INCLUDE clause) if you need extra columns for data:\n   \
                               CREATE INDEX <index> ON <table>(<first_col>)\n   \
                               INCLUDE (<other_cols>);\n\n\
                               Note: Multi-column indexes are occasionally useful (e.g., for composite foreign \
                               keys or specific query patterns). If you've verified this index is necessary, use \
                               a safety-assured block.".into(),
            script_path: None,
        }]
    }

    fn check(&self, node: &NodeEnum, _config: &Config, _ctx: &MigrationContext) -> Vec<Violation> {
        let NodeEnum::IndexStmt(index_stmt) = node else {
            return vec![];
        };

        let column_names: Vec<String> = index_stmt
            .index_params
            .iter()
            .filter_map(|n| match &n.node {
                Some(NodeEnum::IndexElem(elem)) => {
                    if elem.name.is_empty() {
                        Some("<expr>".to_string())
                    } else {
                        Some(elem.name.clone())
                    }
                }
                _ => None,
            })
            .collect();

        let column_count = column_names.len();

        if column_count <= MAX_COLUMNS {
            return vec![];
        }

        let table_name = index_stmt
            .relation
            .as_ref()
            .map(range_var_name)
            .unwrap_or_default();
        let index_name = if index_stmt.idxname.is_empty() {
            "<unnamed>".to_string()
        } else {
            index_stmt.idxname.clone()
        };
        let columns_list = column_names.join(", ");
        let first_col = column_names
            .first()
            .cloned()
            .unwrap_or_else(|| "column1".to_string());
        let second_col = column_names
            .get(1)
            .cloned()
            .unwrap_or_else(|| "column2".to_string());
        let other_cols = column_names
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");

        vec![Violation::new(
            self.describe()[0].operation.clone(),
            self.describe()[0]
                .problem
                .replace("<index>", &index_name)
                .replace("<table>", &table_name)
                .replace("<count>", &column_count.to_string())
                .replace("<cols>", &columns_list),
            self.describe()[0]
                .safe_alternative
                .replace("<index>", &index_name)
                .replace("<table>", &table_name)
                .replace("<count>", &column_count.to_string())
                .replace("<first_col>", &first_col)
                .replace("<second_col>", &second_col)
                .replace("<other_cols>", &other_cols),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_allows, assert_detects_violation};

    #[test]
    fn test_detects_index_with_four_columns() {
        assert_detects_violation!(
            WideIndexCheck,
            "CREATE INDEX idx_users_composite ON users(a, b, c, d);",
            "CREATE INDEX with too many columns"
        );
    }

    #[test]
    fn test_detects_index_with_five_columns() {
        assert_detects_violation!(
            WideIndexCheck,
            "CREATE INDEX idx_users_composite ON users(a, b, c, d, e);",
            "CREATE INDEX with too many columns"
        );
    }

    #[test]
    fn test_detects_unique_index_with_four_columns() {
        assert_detects_violation!(
            WideIndexCheck,
            "CREATE UNIQUE INDEX idx_users_composite ON users(tenant_id, user_id, email, status);",
            "CREATE INDEX with too many columns"
        );
    }

    #[test]
    fn test_allows_index_with_one_column() {
        assert_allows!(
            WideIndexCheck,
            "CREATE INDEX idx_users_email ON users(email);"
        );
    }

    #[test]
    fn test_allows_index_with_two_columns() {
        assert_allows!(
            WideIndexCheck,
            "CREATE INDEX idx_users_composite ON users(tenant_id, user_id);"
        );
    }

    #[test]
    fn test_allows_index_with_three_columns() {
        assert_allows!(
            WideIndexCheck,
            "CREATE INDEX idx_users_composite ON users(email, name, status);"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            WideIndexCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }
}

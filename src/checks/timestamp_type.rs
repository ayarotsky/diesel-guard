//! Detection for TIMESTAMP without time zone column types.
//!
//! This check identifies columns using TIMESTAMP or TIMESTAMP WITHOUT TIME ZONE
//! and recommends using TIMESTAMPTZ (TIMESTAMP WITH TIME ZONE) instead.
//!
//! TIMESTAMP without time zone stores values without timezone context, which:
//! - Can cause issues in multi-timezone applications or server environments
//! - Creates problems during DST (Daylight Saving Time) transitions
//! - Makes it difficult to determine the actual point in time represented
//!
//! TIMESTAMPTZ stores values in UTC internally and converts on input/output based
//! on the session's timezone setting, providing consistent behavior across timezones.
//!
//! ## Lock type
//! None - this is a best practices check, not a locking concern.
//!
//! ## Rewrite behavior
//! None - no table rewrite is involved.
//!
//! ## PostgreSQL version specifics
//! Applies to all PostgreSQL versions.

use crate::checks::pg_helpers::{
    alter_table_cmds, cmd_def_as_column_def, column_type_name, for_each_column_def,
    is_timestamp_without_tz, NodeEnum,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;

pub struct TimestampTypeCheck;

impl Check for TimestampTypeCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let is_create = matches!(node, NodeEnum::CreateStmt(_));

        // Handle CREATE TABLE via for_each_column_def
        if is_create {
            return for_each_column_def(node)
                .into_iter()
                .filter_map(|(table, col)| {
                    if !is_timestamp_without_tz(&column_type_name(col)) {
                        return None;
                    }
                    Some(create_create_table_violation(&table, &col.colname))
                })
                .collect();
        }

        // Handle ALTER TABLE ADD COLUMN
        if let NodeEnum::AlterTableStmt(_) = node {
            let Some((table_name, cmds)) = alter_table_cmds(node) else {
                return vec![];
            };

            return cmds
                .iter()
                .filter_map(|cmd| {
                    let col = cmd_def_as_column_def(cmd)?;
                    if !is_timestamp_without_tz(&column_type_name(col)) {
                        return None;
                    }
                    Some(create_alter_table_violation(&table_name, &col.colname))
                })
                .collect();
        }

        vec![]
    }
}

/// Create a violation for ALTER TABLE ADD COLUMN with TIMESTAMP
fn create_alter_table_violation(table_name: &str, column_name: &str) -> Violation {
    Violation::new(
        "ADD COLUMN with TIMESTAMP",
        format!(
            "Column '{column}' uses TIMESTAMP without time zone. \
            This stores values without timezone context, which can cause issues in \
            multi-timezone applications, during DST transitions, and makes it difficult \
            to determine the actual point in time. \
            This is a best practice warning (no locking impact).",
            column = column_name
        ),
        format!(
            r#"Use TIMESTAMPTZ instead of TIMESTAMP:

1. Replace TIMESTAMP with TIMESTAMPTZ:
   ALTER TABLE {table} ADD COLUMN {column} TIMESTAMPTZ;

TIMESTAMPTZ stores values in UTC internally and converts on input/output based
on the session's timezone setting, providing consistent behavior across timezones.

2. If you intentionally need timezone-naive timestamps, use a safety-assured block:
   -- safety-assured:start
   ALTER TABLE {table} ADD COLUMN {column} TIMESTAMP;
   -- safety-assured:end"#,
            table = table_name,
            column = column_name
        ),
    )
}

/// Create a violation for CREATE TABLE with TIMESTAMP column
fn create_create_table_violation(table_name: &str, column_name: &str) -> Violation {
    Violation::new(
        "CREATE TABLE with TIMESTAMP",
        format!(
            "Column '{column}' uses TIMESTAMP without time zone. \
            This stores values without timezone context, which can cause issues in \
            multi-timezone applications, during DST transitions, and makes it difficult \
            to determine the actual point in time. \
            This is a best practice warning (no locking impact).",
            column = column_name
        ),
        format!(
            r#"Use TIMESTAMPTZ instead of TIMESTAMP:

1. Replace TIMESTAMP with TIMESTAMPTZ:
   CREATE TABLE {table} (
       {column} TIMESTAMPTZ
   );

TIMESTAMPTZ stores values in UTC internally and converts on input/output based
on the session's timezone setting, providing consistent behavior across timezones.

2. If you intentionally need timezone-naive timestamps, use a safety-assured block:
   -- safety-assured:start
   CREATE TABLE {table} (
       {column} TIMESTAMP
   );
   -- safety-assured:end"#,
            table = table_name,
            column = column_name
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_allows, assert_detects_n_violations_any_containing, assert_detects_violation,
    };

    // === Detection tests ===

    #[test]
    fn test_detects_timestamp_column_alter_table() {
        assert_detects_violation!(
            TimestampTypeCheck,
            "ALTER TABLE events ADD COLUMN created_at TIMESTAMP;",
            "ADD COLUMN with TIMESTAMP"
        );
    }

    #[test]
    fn test_detects_timestamp_without_time_zone_alter_table() {
        assert_detects_violation!(
            TimestampTypeCheck,
            "ALTER TABLE events ADD COLUMN updated_at TIMESTAMP WITHOUT TIME ZONE;",
            "ADD COLUMN with TIMESTAMP"
        );
    }

    #[test]
    fn test_detects_timestamp_column_create_table() {
        assert_detects_violation!(
            TimestampTypeCheck,
            "CREATE TABLE events (id SERIAL PRIMARY KEY, created_at TIMESTAMP);",
            "CREATE TABLE with TIMESTAMP"
        );
    }

    #[test]
    fn test_detects_timestamp_without_time_zone_create_table() {
        assert_detects_violation!(
            TimestampTypeCheck,
            "CREATE TABLE events (id SERIAL PRIMARY KEY, created_at TIMESTAMP WITHOUT TIME ZONE);",
            "CREATE TABLE with TIMESTAMP"
        );
    }

    #[test]
    fn test_detects_multiple_timestamp_columns() {
        assert_detects_n_violations_any_containing!(
            TimestampTypeCheck,
            "CREATE TABLE events (id SERIAL PRIMARY KEY, created_at TIMESTAMP, updated_at TIMESTAMP);",
            2,
            "created_at",
            "updated_at"
        );
    }

    // === Safe variant tests ===

    #[test]
    fn test_allows_timestamptz_column() {
        assert_allows!(
            TimestampTypeCheck,
            "ALTER TABLE events ADD COLUMN created_at TIMESTAMPTZ;"
        );
    }

    #[test]
    fn test_allows_timestamp_with_time_zone_column() {
        assert_allows!(
            TimestampTypeCheck,
            "ALTER TABLE events ADD COLUMN created_at TIMESTAMP WITH TIME ZONE;"
        );
    }

    #[test]
    fn test_allows_timestamptz_create_table() {
        assert_allows!(
            TimestampTypeCheck,
            "CREATE TABLE events (id SERIAL PRIMARY KEY, created_at TIMESTAMPTZ);"
        );
    }

    #[test]
    fn test_allows_timestamp_with_time_zone_create_table() {
        assert_allows!(
            TimestampTypeCheck,
            "CREATE TABLE events (id SERIAL PRIMARY KEY, created_at TIMESTAMP WITH TIME ZONE);"
        );
    }

    #[test]
    fn test_allows_other_column_types() {
        assert_allows!(TimestampTypeCheck, "ALTER TABLE users ADD COLUMN age INT;");
        assert_allows!(
            TimestampTypeCheck,
            "ALTER TABLE users ADD COLUMN active BOOLEAN;"
        );
        assert_allows!(
            TimestampTypeCheck,
            "ALTER TABLE users ADD COLUMN name TEXT;"
        );
        assert_allows!(
            TimestampTypeCheck,
            "ALTER TABLE users ADD COLUMN birth_date DATE;"
        );
    }

    #[test]
    fn test_allows_create_table_without_timestamp() {
        assert_allows!(
            TimestampTypeCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email VARCHAR(255));"
        );
    }

    // === Unrelated operation tests ===

    #[test]
    fn test_ignores_other_alter_operations() {
        assert_allows!(
            TimestampTypeCheck,
            "ALTER TABLE users DROP COLUMN old_field;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(TimestampTypeCheck, "SELECT * FROM users;");
    }
}

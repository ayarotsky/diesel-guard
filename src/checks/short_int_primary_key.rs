//! Detection for short integer types (SMALLINT, INT) used in primary keys.
//!
//! This check identifies primary key columns that use SMALLINT or INT/INTEGER data types,
//! which risk ID exhaustion. SMALLINT maxes out at ~32,767 records, and INT at ~2.1 billion.
//!
//! While 2.1 billion seems large, active applications can exhaust this faster than expected,
//! especially with high-frequency inserts, soft deletes, or partitioned data.
//!
//! Changing the type later requires an ALTER COLUMN TYPE operation that triggers a full
//! table rewrite with an ACCESS EXCLUSIVE lock, blocking all operations.
//!
//! SERIAL is equivalent to INT with a sequence, so SERIAL PRIMARY KEY is correctly
//! flagged by this check. Use BIGSERIAL PRIMARY KEY instead.

use crate::checks::pg_helpers::{
    alter_table_cmds, cmd_def_as_column_def, cmd_def_as_constraint, column_has_constraint,
    column_type_name, for_each_column_def, is_short_integer, range_var_name, ColumnDef, ConstrType,
    Constraint, NodeEnum,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;

const CONSTR_PRIMARY: i32 = ConstrType::ConstrPrimary as i32;

pub struct ShortIntegerPrimaryKeyCheck;

impl Check for ShortIntegerPrimaryKeyCheck {
    fn check(&self, node: &NodeEnum, _config: &Config) -> Vec<Violation> {
        let mut violations = vec![];

        // Inline PRIMARY KEY on column definitions
        // (for_each_column_def handles both CreateStmt and AlterTableStmt)
        violations.extend(
            for_each_column_def(node)
                .into_iter()
                .filter(|(_, col)| column_has_constraint(col, CONSTR_PRIMARY))
                .filter_map(|(table, col)| check_column_type(&table, col)),
        );

        // Separate PRIMARY KEY constraints referencing column defs by name
        match node {
            NodeEnum::CreateStmt(create) => {
                let table_name = create
                    .relation
                    .as_ref()
                    .map(range_var_name)
                    .unwrap_or_default();

                let col_defs: Vec<&ColumnDef> = create
                    .table_elts
                    .iter()
                    .filter_map(|n| match &n.node {
                        Some(NodeEnum::ColumnDef(col)) => Some(col.as_ref()),
                        _ => None,
                    })
                    .collect();

                for elt in &create.table_elts {
                    if let Some(NodeEnum::Constraint(c)) = &elt.node {
                        if c.contype == CONSTR_PRIMARY {
                            violations.extend(check_pk_key_columns(&table_name, c, &col_defs));
                        }
                    }
                }
            }
            NodeEnum::AlterTableStmt(_) => {
                if let Some((table_name, cmds)) = alter_table_cmds(node) {
                    let col_defs: Vec<&ColumnDef> = cmds
                        .iter()
                        .filter_map(|cmd| cmd_def_as_column_def(cmd))
                        .collect();

                    if !col_defs.is_empty() {
                        for cmd in &cmds {
                            if let Some(c) = cmd_def_as_constraint(cmd) {
                                if c.contype == CONSTR_PRIMARY {
                                    violations.extend(check_pk_key_columns(
                                        &table_name,
                                        c,
                                        &col_defs,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        violations
    }
}

/// Look up each constraint key column by name and check its type.
fn check_pk_key_columns(
    table: &str,
    constraint: &Constraint,
    col_defs: &[&ColumnDef],
) -> Vec<Violation> {
    constraint
        .keys
        .iter()
        .filter_map(|key| {
            let name = match &key.node {
                Some(NodeEnum::String(s)) => &s.sval,
                _ => return None,
            };
            let col = col_defs.iter().find(|cd| cd.colname == *name)?;
            check_column_type(table, col)
        })
        .collect()
}

/// Check if a column's type is a short integer and return a violation if so.
fn check_column_type(table_name: &str, col: &ColumnDef) -> Option<Violation> {
    let type_name = column_type_name(col);
    if !is_short_integer(&type_name) {
        return None;
    }

    let (display_name, limit) = short_integer_info(&type_name)?;
    Some(create_violation(
        table_name.to_string(),
        col.colname.clone(),
        display_name,
        limit,
    ))
}

/// Map pg_query internal type names to display names and limits.
fn short_integer_info(type_name: &str) -> Option<(&'static str, &'static str)> {
    match type_name {
        "int2" | "smallserial" => Some(("SMALLINT", "~32,767")),
        "int4" | "serial" => Some(("INT", "~2.1 billion")),
        _ => None,
    }
}

/// Create a violation for a short integer primary key
fn create_violation(
    table_name: String,
    column_name: String,
    type_name: &str,
    limit: &str,
) -> Violation {
    Violation::new(
        "PRIMARY KEY with short integer type",
        format!(
            "Using {type_name} for primary key column '{column}' on table '{table}' risks ID exhaustion at {limit} records. \
            {type_name} can be quickly exhausted in production applications. \
            Changing the type later requires an ALTER COLUMN TYPE operation that triggers a full table rewrite with an \
            ACCESS EXCLUSIVE lock, blocking all operations. Duration depends on table size.",
            type_name = type_name,
            column = column_name,
            table = table_name,
            limit = limit
        ),
        format!(
            r#"Use BIGINT for primary keys to avoid ID exhaustion:

Instead of:
   CREATE TABLE {table} ({column} {type_name} PRIMARY KEY);

Use:
   CREATE TABLE {table} ({column} BIGINT PRIMARY KEY);

BIGINT provides 8 bytes (range: -9.2 quintillion to 9.2 quintillion), which is effectively unlimited
for auto-incrementing IDs. The minimal storage overhead (4 extra bytes per row) is negligible.

If using SERIAL/SMALLSERIAL, use BIGSERIAL instead:
   {column} BIGSERIAL PRIMARY KEY

Note: If this is an intentionally small table (e.g., lookup table with <100 entries),
use 'safety-assured' to bypass this check."#,
            table = table_name,
            column = column_name,
            type_name = type_name
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_allows, assert_detects_n_violations_any_containing, assert_detects_violation,
        assert_detects_violation_containing,
    };

    // === CREATE TABLE with inline PRIMARY KEY ===

    #[test]
    fn test_detects_create_table_int_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT PRIMARY KEY);",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_create_table_integer_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INTEGER PRIMARY KEY);",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_create_table_smallint_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id SMALLINT PRIMARY KEY);",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_create_table_int2_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT2 PRIMARY KEY);",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_create_table_int4_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT4 PRIMARY KEY);",
            "PRIMARY KEY with short integer type"
        );
    }

    // === CREATE TABLE with separate PRIMARY KEY constraint ===

    #[test]
    fn test_detects_create_table_separate_pk_constraint() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT, name TEXT, PRIMARY KEY (id));",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_composite_primary_key_with_int() {
        assert_detects_violation_containing!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE events (tenant_id BIGINT, id INT, PRIMARY KEY (tenant_id, id));",
            "PRIMARY KEY with short integer type",
            "id",
            "INT"
        );
    }

    #[test]
    fn test_detects_multiple_short_int_columns_in_composite_pk() {
        assert_detects_n_violations_any_containing!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE data (tenant_id INT, user_id SMALLINT, PRIMARY KEY (tenant_id, user_id));",
            2,
            "tenant_id",
            "user_id"
        );
    }

    // === ALTER TABLE ADD COLUMN ===

    #[test]
    fn test_detects_alter_add_column_int_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id INT PRIMARY KEY;",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_alter_add_column_smallint_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id SMALLINT PRIMARY KEY;",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_serial_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);",
            "PRIMARY KEY with short integer type"
        );
    }

    // === Safe cases (should not trigger) ===

    #[test]
    fn test_allows_bigint_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_int8_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT8 PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_bigserial_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGSERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_uuid_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id UUID PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_int_column_without_primary_key() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY, age INT);"
        );
    }

    #[test]
    fn test_allows_int_unique_not_primary() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id BIGINT PRIMARY KEY, code INT UNIQUE);"
        );
    }

    #[test]
    fn test_allows_composite_pk_all_bigint() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE events (tenant_id BIGINT, id BIGINT, PRIMARY KEY (tenant_id, id));"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users DROP COLUMN age;"
        );
    }

    #[test]
    fn test_ignores_alter_add_column_without_pk() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN age INT;"
        );
    }

    // === ALTER TABLE ADD CONSTRAINT ===

    #[test]
    fn test_detects_alter_add_constraint_primary_key() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id INT, ADD CONSTRAINT pk_users PRIMARY KEY (id);",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_alter_add_constraint_smallint_pk() {
        assert_detects_violation!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id SMALLINT, ADD CONSTRAINT pk_users PRIMARY KEY (id);",
            "PRIMARY KEY with short integer type"
        );
    }

    #[test]
    fn test_detects_alter_add_constraint_composite_pk_with_int() {
        assert_detects_violation_containing!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE events ADD COLUMN tenant_id BIGINT, ADD COLUMN id INT, ADD CONSTRAINT pk_events PRIMARY KEY (tenant_id, id);",
            "PRIMARY KEY with short integer type",
            "id",
            "INT"
        );
    }

    #[test]
    fn test_allows_alter_add_constraint_bigint_pk() {
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD COLUMN id BIGINT, ADD CONSTRAINT pk_users PRIMARY KEY (id);"
        );
    }

    #[test]
    fn test_ignores_alter_add_constraint_on_existing_column() {
        // Can't detect type when column already exists (not added in same statement)
        assert_allows!(
            ShortIntegerPrimaryKeyCheck,
            "ALTER TABLE users ADD CONSTRAINT pk_users PRIMARY KEY (id);"
        );
    }

    // === Exhaustion limit messages ===

    #[test]
    fn test_smallint_shows_correct_limit() {
        assert_detects_violation_containing!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id SMALLINT PRIMARY KEY);",
            "PRIMARY KEY with short integer type",
            "~32,767"
        );
    }

    #[test]
    fn test_int_shows_correct_limit() {
        assert_detects_violation_containing!(
            ShortIntegerPrimaryKeyCheck,
            "CREATE TABLE users (id INT PRIMARY KEY);",
            "PRIMARY KEY with short integer type",
            "~2.1 billion"
        );
    }
}

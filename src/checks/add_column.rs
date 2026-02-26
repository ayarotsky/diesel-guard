//! Detection for ADD COLUMN with DEFAULT operations.
//!
//! This check identifies `ALTER TABLE` statements that add columns with DEFAULT
//! values, which can cause table locks and performance issues on Postgres < 11.
//!
//! On Postgres versions before 11, adding a column with a DEFAULT value requires
//! a full table rewrite to backfill the default value for existing rows. This acquires
//! an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size.
//!
//! On Postgres 11+, constant defaults (literals like FALSE, 0, 'active') are safe
//! as they are stored as metadata without a table rewrite. Volatile defaults (function
//! calls like now() or gen_random_uuid()) still require a table rewrite on all versions.

use crate::checks::pg_helpers::{
    ConstrType, NodeEnum, alter_table_cmds, cmd_def_as_column_def, column_has_constraint,
    column_type_name,
};
use crate::checks::{Check, Config};
use crate::violation::Violation;
use pg_query::protobuf::ColumnDef;

pub struct AddColumnCheck;

impl Check for AddColumnCheck {
    fn check(&self, node: &NodeEnum, config: &Config) -> Vec<Violation> {
        let Some((table_name, cmds)) = alter_table_cmds(node) else {
            return vec![];
        };

        cmds.iter()
            .filter_map(|cmd| {
                let col = cmd_def_as_column_def(cmd)?;

                if !column_has_constraint(col, ConstrType::ConstrDefault as i32) {
                    return None;
                }

                // On PG 11+, constant defaults are safe (metadata-only change).
                // Volatile defaults (function calls, etc.) still require a table rewrite.
                if config.postgres_version >= Some(11) && is_constant_default(col) {
                    return None;
                }

                let column_name = &col.colname;
                let data_type = column_type_name(col);

                Some(Violation::new(
                    "ADD COLUMN with DEFAULT",
                    format!(
                        "Adding column '{column}' with DEFAULT on table '{table}' requires a full table rewrite on Postgres < 11, \
                        which acquires an ACCESS EXCLUSIVE lock and blocks all operations. Duration depends on table size.",
                        column = column_name, table = table_name
                    ),
                    format!(r#"1. Add the column without a default:
   ALTER TABLE {table} ADD COLUMN {column} {data_type};

2. Backfill data in batches (outside migration):
   UPDATE {table} SET {column} = <value> WHERE {column} IS NULL;

3. Add default for new rows only:
   ALTER TABLE {table} ALTER COLUMN {column} SET DEFAULT <value>;

Note: For Postgres 11+, this is safe if the default is a constant value."#,
                        table = table_name,
                        column = column_name,
                        data_type = data_type
                    ),
                ))
            })
            .collect()
    }
}

/// Returns true if the column's DEFAULT constraint expression is a constant literal.
///
/// `AConst` covers booleans (FALSE), integers (0), strings ('active'), and NULL.
/// Function calls, operators, and type casts are non-constant and always produce violations.
fn is_constant_default(col: &ColumnDef) -> bool {
    col.constraints.iter().any(|c| {
        let Some(NodeEnum::Constraint(constraint)) = &c.node else {
            return false;
        };
        constraint.contype == ConstrType::ConstrDefault as i32
            && matches!(
                constraint.raw_expr.as_ref().and_then(|e| e.node.as_ref()),
                Some(NodeEnum::AConst(_))
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_allows, assert_allows_with_config, assert_detects_violation,
        assert_detects_violation_with_config,
    };

    fn pg_config(version: u32) -> Config {
        Config {
            postgres_version: Some(version),
            ..Default::default()
        }
    }

    #[test]
    fn test_detects_add_column_with_default() {
        assert_detects_violation!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
            "ADD COLUMN with DEFAULT"
        );
    }

    #[test]
    fn test_allows_add_column_without_default() {
        assert_allows!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN;"
        );
    }

    #[test]
    fn test_ignores_other_statements() {
        assert_allows!(
            AddColumnCheck,
            "CREATE TABLE users (id SERIAL PRIMARY KEY);"
        );
    }

    #[test]
    fn test_allows_constant_default_on_pg11() {
        assert_allows_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
            &pg_config(11)
        );
    }

    #[test]
    fn test_allows_constant_default_on_pg16() {
        assert_allows_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN status VARCHAR DEFAULT 'active';",
            &pg_config(16)
        );
    }

    #[test]
    fn test_allows_integer_constant_default_on_pg11() {
        assert_allows_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN retries INT DEFAULT 0;",
            &pg_config(11)
        );
    }

    #[test]
    fn test_detects_constant_default_on_pg10() {
        assert_detects_violation_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
            "ADD COLUMN with DEFAULT",
            &pg_config(10)
        );
    }

    #[test]
    fn test_detects_volatile_default_on_pg11() {
        assert_detects_violation_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN created_at TIMESTAMP DEFAULT now();",
            "ADD COLUMN with DEFAULT",
            &pg_config(11)
        );
    }

    #[test]
    fn test_detects_volatile_default_on_pg16() {
        assert_detects_violation_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN id UUID DEFAULT gen_random_uuid();",
            "ADD COLUMN with DEFAULT",
            &pg_config(16)
        );
    }

    #[test]
    fn test_detects_typecast_default_on_pg11() {
        // TypeCast nodes ('active'::text) are not AConst — treated as non-constant
        assert_detects_violation_with_config!(
            AddColumnCheck,
            "ALTER TABLE users ADD COLUMN status TEXT DEFAULT 'active'::text;",
            "ADD COLUMN with DEFAULT",
            &pg_config(11)
        );
    }
}

//! Low-level AST navigation functions for pg_query's protobuf AST.
//!
//! Each function is pure (no state). These become Rhai-registerable in Phase 2.

// Re-export commonly used pg_query types so check files don't need direct pg_query imports.
pub use pg_query::protobuf::node::Node as NodeEnum;
pub use pg_query::protobuf::{
    AlterTableType, ColumnDef, ConstrType, DropBehavior, Node, ObjectType,
};

pub use pg_query::protobuf::Constraint;

use pg_query::protobuf::{AlterTableCmd, RangeVar, TypeName};

// ---------------------------------------------------------------------------
// Primitive extractors
// ---------------------------------------------------------------------------

/// Extract table name from RangeVar (schema-qualified if present).
pub fn range_var_name(rv: &RangeVar) -> String {
    if rv.schemaname.is_empty() {
        rv.relname.clone()
    } else {
        format!("{}.{}", rv.schemaname, rv.relname)
    }
}

/// Extract the last type name segment from TypeName (e.g., "int4", "bpchar", "json").
pub fn type_name_str(tn: &TypeName) -> String {
    tn.names
        .iter()
        .filter_map(|n| match &n.node {
            Some(NodeEnum::String(s)) => Some(s.sval.clone()),
            _ => None,
        })
        .next_back()
        .unwrap_or_default()
}

/// Shortcut: extract type name string from a ColumnDef.
pub fn column_type_name(col: &ColumnDef) -> String {
    col.type_name
        .as_ref()
        .map(type_name_str)
        .unwrap_or_default()
}

/// Get constraint column names as comma-separated string.
pub fn constraint_columns_str(c: &Constraint) -> String {
    c.keys
        .iter()
        .filter_map(|n| match &n.node {
            Some(NodeEnum::String(s)) => Some(s.sval.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Extract ColumnDef from an AlterTableCmd's `def` field.
pub fn cmd_def_as_column_def(cmd: &AlterTableCmd) -> Option<&ColumnDef> {
    cmd.def.as_ref().and_then(|node| match &node.node {
        Some(NodeEnum::ColumnDef(col)) => Some(col.as_ref()),
        _ => None,
    })
}

/// Extract Constraint from an AlterTableCmd's `def` field.
pub fn cmd_def_as_constraint(cmd: &AlterTableCmd) -> Option<&Constraint> {
    cmd.def.as_ref().and_then(|node| match &node.node {
        Some(NodeEnum::Constraint(c)) => Some(c.as_ref()),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Type classification predicates
// ---------------------------------------------------------------------------

/// Check if type name is fixed-length CHAR (pg internal: "bpchar").
pub fn is_char_type(type_name: &str) -> bool {
    type_name == "bpchar"
}

/// Check if type name is TIMESTAMP without timezone (not "timestamptz").
pub fn is_timestamp_without_tz(type_name: &str) -> bool {
    type_name == "timestamp"
}

/// Check if type name is a short integer (SMALLINT, INT, SERIAL, SMALLSERIAL).
pub fn is_short_integer(type_name: &str) -> bool {
    matches!(type_name, "int2" | "int4" | "serial" | "smallserial")
}

/// Check if type name is JSON (not JSONB).
pub fn is_json_type(type_name: &str) -> bool {
    type_name == "json"
}

/// Check if column has a constraint of the given `contype`.
pub fn column_has_constraint(col: &ColumnDef, contype: i32) -> bool {
    col.constraints.iter().any(|c| {
        matches!(
            &c.node,
            Some(NodeEnum::Constraint(constraint)) if constraint.contype == contype
        )
    })
}

/// Check if column is a SERIAL type (SERIAL, BIGSERIAL, SMALLSERIAL).
pub fn is_serial_pattern(col: &ColumnDef) -> bool {
    let type_name = column_type_name(col);
    matches!(type_name.as_str(), "serial" | "bigserial" | "smallserial")
}

// ---------------------------------------------------------------------------
// Higher-level iteration helpers
// ---------------------------------------------------------------------------

/// Extract AlterTableCmd entries if node is an AlterTableStmt.
/// Returns `(table_name, cmds)`.
pub fn alter_table_cmds(node: &NodeEnum) -> Option<(String, Vec<&AlterTableCmd>)> {
    match node {
        NodeEnum::AlterTableStmt(alter) => {
            let table = alter
                .relation
                .as_ref()
                .map(range_var_name)
                .unwrap_or_default();
            let cmds: Vec<&AlterTableCmd> = alter
                .cmds
                .iter()
                .filter_map(|n| match &n.node {
                    Some(NodeEnum::AlterTableCmd(cmd)) => Some(cmd.as_ref()),
                    _ => None,
                })
                .collect();
            Some((table, cmds))
        }
        _ => None,
    }
}

/// Extract schema-qualified object names from a DropStmt's `objects` field.
///
/// DropStmt stores each object as a List of String nodes (for schema-qualified names).
/// This function joins them with "." to produce names like "public.my_table".
pub fn drop_object_names(objects: &[Node]) -> Vec<String> {
    objects
        .iter()
        .filter_map(|obj_node| match &obj_node.node {
            Some(NodeEnum::List(list)) => {
                let name = list
                    .items
                    .iter()
                    .filter_map(|n| match &n.node {
                        Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(".");
                Some(name)
            }
            _ => None,
        })
        .collect()
}

/// Iterate ColumnDef from both CreateStmt.table_elts AND AlterTableStmt ADD COLUMN.
/// Returns `(table_name, column_def)` pairs for dual-context checks.
pub fn for_each_column_def(node: &NodeEnum) -> Vec<(String, &ColumnDef)> {
    match node {
        NodeEnum::CreateStmt(create) => {
            let table = create
                .relation
                .as_ref()
                .map(range_var_name)
                .unwrap_or_default();
            create
                .table_elts
                .iter()
                .filter_map(|n| match &n.node {
                    Some(NodeEnum::ColumnDef(col)) => Some((table.clone(), col.as_ref())),
                    _ => None,
                })
                .collect()
        }
        NodeEnum::AlterTableStmt(alter) => {
            let table = alter
                .relation
                .as_ref()
                .map(range_var_name)
                .unwrap_or_default();
            alter
                .cmds
                .iter()
                .filter_map(|n| match &n.node {
                    Some(NodeEnum::AlterTableCmd(cmd)) => {
                        cmd_def_as_column_def(cmd).map(|col| (table.clone(), col))
                    }
                    _ => None,
                })
                .collect()
        }
        _ => vec![],
    }
}

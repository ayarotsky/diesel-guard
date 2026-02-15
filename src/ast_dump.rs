use crate::checks::pg_helpers::extract_node;
use crate::error::{DieselGuardError, Result};
use crate::parser;

/// Parse SQL and return the AST as a pretty-printed JSON string.
///
/// Each top-level statement is serialized as a JSON object. The outer
/// `RawStmt` / `Node` wrappers are stripped so the output starts at the
/// concrete node type (e.g. `IndexStmt`, `CreateStmt`).
pub fn dump_ast(sql: &str) -> Result<String> {
    let stmts = parser::parse(sql)?;

    let nodes: Vec<serde_json::Value> = stmts
        .iter()
        .filter_map(|raw_stmt| extract_node(raw_stmt).and_then(|n| serde_json::to_value(n).ok()))
        .collect();

    serde_json::to_string_pretty(&nodes)
        .map_err(|e| DieselGuardError::parse_error(format!("JSON serialization failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_statement() {
        let json = dump_ast("CREATE INDEX idx ON users(email);").unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        // Should contain IndexStmt variant
        assert!(json.contains("IndexStmt"));
    }

    #[test]
    fn test_multiple_statements() {
        let json = dump_ast("CREATE TABLE t (id INT); CREATE INDEX idx ON t(id);").unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn test_invalid_sql() {
        let result = dump_ast("NOT VALID SQL HERE");
        assert!(result.is_err());
    }
}

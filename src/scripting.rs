use crate::checks::Check;
use crate::violation::Violation;
use camino::Utf8Path;
use pg_query::protobuf::node::Node as NodeEnum;
use rhai::{Dynamic, Engine, AST};
use std::fmt;
use std::sync::Arc;

/// Error encountered while loading or running a custom Rhai check script.
#[derive(Debug)]
pub struct ScriptError {
    pub file: String,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

/// A custom check backed by a compiled Rhai script.
pub struct CustomCheck {
    name: &'static str,
    engine: Arc<Engine>,
    ast: AST,
}

impl Check for CustomCheck {
    fn name(&self) -> &'static str {
        self.name
    }

    fn check(&self, node: &NodeEnum) -> Vec<Violation> {
        // Serialize the pg_query node to a Rhai Dynamic value via serde
        let dynamic_node = match rhai::serde::to_dynamic(node) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "Warning: custom check '{}': failed to serialize node: {e}",
                    self.name
                );
                return vec![];
            }
        };

        let mut scope = rhai::Scope::new();
        scope.push("node", dynamic_node);

        match self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &self.ast)
        {
            Ok(result) => parse_script_result(self.name, result),
            Err(e) => {
                // Don't warn on "ErrorTerminated" — that's just max_operations kicking in
                // for scripts that intentionally don't early-return on unmatched node types.
                // All other runtime errors are worth reporting.
                let err_str = e.to_string();
                if !err_str.contains("ErrorTerminated") {
                    eprintln!("Warning: custom check '{}': runtime error: {e}", self.name);
                }
                vec![]
            }
        }
    }
}

/// Parse the return value of a Rhai script into violations.
///
/// Accepted return types:
/// - `()` — no violation
/// - `#{ operation: "...", problem: "...", safe_alternative: "..." }` — one violation
/// - Array of maps — multiple violations
fn parse_script_result(check_name: &str, result: Dynamic) -> Vec<Violation> {
    if result.is_unit() {
        return vec![];
    }

    if result.is_map() {
        return match map_to_violation(check_name, result) {
            Some(v) => vec![v],
            None => vec![],
        };
    }

    if result.is_array() {
        return result
            .into_array()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| map_to_violation(check_name, v))
            .collect();
    }

    vec![Violation::new(
        format!("SCRIPT ERROR: {check_name}"),
        format!(
            "Custom check returned {}, expected (), map, or array",
            result.type_name()
        ),
        "Fix the custom check script to return a valid type.",
    )]
}

/// Convert a Rhai map Dynamic to a Violation.
fn map_to_violation(check_name: &str, value: Dynamic) -> Option<Violation> {
    let map = value.try_cast::<rhai::Map>()?;

    let operation = map
        .get("operation")
        .and_then(|v| v.clone().into_string().ok());
    let problem = map
        .get("problem")
        .and_then(|v| v.clone().into_string().ok());
    let safe_alternative = map
        .get("safe_alternative")
        .and_then(|v| v.clone().into_string().ok());

    match (operation, problem, safe_alternative) {
        (Some(op), Some(prob), Some(alt)) => Some(Violation::new(op, prob, alt)),
        _ => {
            let keys: Vec<_> = map.keys().map(|k| k.to_string()).collect();
            Some(Violation::new(
                format!("SCRIPT ERROR: {check_name}"),
                format!(
                    "Custom check returned a map missing required keys \
                     (need 'operation', 'problem', 'safe_alternative'), got keys: [{}]",
                    keys.join(", ")
                ),
                "Fix the custom check script to return all three required keys.",
            ))
        }
    }
}

/// Build a Rhai module exposing commonly needed pg_query protobuf enum constants.
///
/// Scripts access these as `pg::OBJECT_TABLE`, `pg::AT_ADD_COLUMN`, etc.
fn create_pg_constants_module() -> rhai::Module {
    use pg_query::protobuf::{AlterTableType, ConstrType, DropBehavior, ObjectType};

    let mut m = rhai::Module::new();

    // ObjectType — used by DropStmt.remove_type, RenameStmt.rename_type, etc.
    m.set_var("OBJECT_INDEX", ObjectType::ObjectIndex as i64);
    m.set_var("OBJECT_TABLE", ObjectType::ObjectTable as i64);
    m.set_var("OBJECT_COLUMN", ObjectType::ObjectColumn as i64);
    m.set_var("OBJECT_DATABASE", ObjectType::ObjectDatabase as i64);
    m.set_var("OBJECT_SCHEMA", ObjectType::ObjectSchema as i64);
    m.set_var("OBJECT_SEQUENCE", ObjectType::ObjectSequence as i64);
    m.set_var("OBJECT_VIEW", ObjectType::ObjectView as i64);
    m.set_var("OBJECT_FUNCTION", ObjectType::ObjectFunction as i64);
    m.set_var("OBJECT_EXTENSION", ObjectType::ObjectExtension as i64);
    m.set_var("OBJECT_TRIGGER", ObjectType::ObjectTrigger as i64);
    m.set_var("OBJECT_TYPE", ObjectType::ObjectType as i64);

    // AlterTableType — used by AlterTableCmd.subtype
    m.set_var("AT_ADD_COLUMN", AlterTableType::AtAddColumn as i64);
    m.set_var("AT_COLUMN_DEFAULT", AlterTableType::AtColumnDefault as i64);
    m.set_var("AT_DROP_NOT_NULL", AlterTableType::AtDropNotNull as i64);
    m.set_var("AT_SET_NOT_NULL", AlterTableType::AtSetNotNull as i64);
    m.set_var("AT_DROP_COLUMN", AlterTableType::AtDropColumn as i64);
    m.set_var(
        "AT_ALTER_COLUMN_TYPE",
        AlterTableType::AtAlterColumnType as i64,
    );
    m.set_var("AT_ADD_CONSTRAINT", AlterTableType::AtAddConstraint as i64);
    m.set_var(
        "AT_DROP_CONSTRAINT",
        AlterTableType::AtDropConstraint as i64,
    );
    m.set_var(
        "AT_VALIDATE_CONSTRAINT",
        AlterTableType::AtValidateConstraint as i64,
    );

    // ConstrType — used by Constraint.contype
    m.set_var("CONSTR_NOTNULL", ConstrType::ConstrNotnull as i64);
    m.set_var("CONSTR_DEFAULT", ConstrType::ConstrDefault as i64);
    m.set_var("CONSTR_IDENTITY", ConstrType::ConstrIdentity as i64);
    m.set_var("CONSTR_GENERATED", ConstrType::ConstrGenerated as i64);
    m.set_var("CONSTR_CHECK", ConstrType::ConstrCheck as i64);
    m.set_var("CONSTR_PRIMARY", ConstrType::ConstrPrimary as i64);
    m.set_var("CONSTR_UNIQUE", ConstrType::ConstrUnique as i64);
    m.set_var("CONSTR_EXCLUSION", ConstrType::ConstrExclusion as i64);
    m.set_var("CONSTR_FOREIGN", ConstrType::ConstrForeign as i64);

    // DropBehavior — used by DropStmt.behavior
    m.set_var("DROP_RESTRICT", DropBehavior::DropRestrict as i64);
    m.set_var("DROP_CASCADE", DropBehavior::DropCascade as i64);

    m
}

/// Create a sandboxed Rhai engine with safety limits.
fn create_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_string_size(10_000);
    engine.set_max_array_size(1_000);
    engine.set_max_map_size(1_000);
    engine.register_static_module("pg", create_pg_constants_module().into());
    engine
}

/// Load all `.rhai` files from a directory and compile them into custom checks.
///
/// Returns successfully compiled checks and any errors encountered.
/// Compilation errors are non-fatal — they're collected as `ScriptError`s.
pub fn load_custom_checks(
    dir: &Utf8Path,
    config: &crate::config::Config,
) -> (Vec<Box<dyn Check>>, Vec<ScriptError>) {
    let mut checks: Vec<Box<dyn Check>> = Vec::new();
    let mut errors: Vec<ScriptError> = Vec::new();

    let engine = Arc::new(create_engine());

    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            errors.push(ScriptError {
                file: dir.to_string(),
                message: format!("Failed to read directory: {e}"),
            });
            return (checks, errors);
        }
    };

    let mut entries: Vec<_> = read_dir
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rhai"))
        .collect();

    // Sort for deterministic order
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Skip scripts disabled via config
        if !config.is_check_enabled(stem) {
            continue;
        }

        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                errors.push(ScriptError {
                    file: path.display().to_string(),
                    message: format!("Failed to read: {e}"),
                });
                continue;
            }
        };

        match engine.compile(&source) {
            Ok(ast) => {
                // Leak the name — finite: one per script at startup
                let name: &'static str = Box::leak(stem.to_string().into_boxed_str());
                checks.push(Box::new(CustomCheck {
                    name,
                    engine: Arc::clone(&engine),
                    ast,
                }));
            }
            Err(e) => {
                errors.push(ScriptError {
                    file: path.display().to_string(),
                    message: format!("Compilation error: {e}"),
                });
            }
        }
    }

    (checks, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::pg_helpers::extract_node;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: run a script against a node and return violations.
    fn run_script(script: &str, sql: &str) -> Vec<Violation> {
        let engine = Arc::new(create_engine());
        let ast = engine.compile(script).expect("script should compile");
        let name: &'static str = Box::leak("test_check".to_string().into_boxed_str());
        let check = CustomCheck { name, engine, ast };

        let stmts = crate::parser::parse(sql).expect("SQL should parse");
        let mut all_violations = Vec::new();
        for raw_stmt in &stmts {
            if let Some(node) = extract_node(raw_stmt) {
                all_violations.extend(check.check(node));
            }
        }
        all_violations
    }

    #[test]
    fn test_script_returns_unit_no_violations() {
        let violations = run_script(
            r#"
            // Script that always returns unit (no violation)
            let stmt = node.CreateStmt;
            if stmt == () { return; }
            "#,
            "CREATE INDEX idx ON t(id);",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_script_returns_map_one_violation() {
        let violations = run_script(
            r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            if !stmt.concurrent {
                #{
                    operation: "INDEX without CONCURRENTLY",
                    problem: "locks table",
                    safe_alternative: "use CONCURRENTLY"
                }
            }
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "INDEX without CONCURRENTLY");
        assert_eq!(violations[0].problem, "locks table");
    }

    #[test]
    fn test_script_returns_array_multiple_violations() {
        let violations = run_script(
            r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            [
                #{ operation: "violation 1", problem: "p1", safe_alternative: "s1" },
                #{ operation: "violation 2", problem: "p2", safe_alternative: "s2" }
            ]
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].operation, "violation 1");
        assert_eq!(violations[1].operation, "violation 2");
    }

    #[test]
    fn test_script_invalid_return_type_no_crash() {
        // Returning a string instead of map — should produce an error violation
        let violations = run_script(
            r#"
            "not a valid return type"
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].operation.contains("SCRIPT ERROR"));
    }

    #[test]
    fn test_script_infinite_loop_hits_max_operations() {
        // Engine's max_operations limit should kick in
        let violations = run_script(
            r#"
            loop { }
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        // Should not hang; returns empty due to runtime error
        assert!(violations.is_empty());
    }

    #[test]
    fn test_script_wrong_node_type_returns_unit() {
        // Script looks for CreateStmt but we give it an IndexStmt
        let violations = run_script(
            r#"
            let stmt = node.CreateStmt;
            if stmt == () { return; }
            #{ operation: "found", problem: "p", safe_alternative: "s" }
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_compilation_error_reported() {
        let engine = Arc::new(create_engine());
        let result = engine.compile("this is not valid rhai {{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_custom_checks_from_directory() {
        let dir = TempDir::new().unwrap();
        let dir_path = Utf8Path::from_path(dir.path()).unwrap();

        // Write a valid check script
        fs::write(
            dir.path().join("require_concurrent.rhai"),
            r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            if !stmt.concurrent {
                #{ operation: "custom", problem: "no concurrently", safe_alternative: "use it" }
            }
            "#,
        )
        .unwrap();

        // Write an invalid script
        fs::write(dir.path().join("broken.rhai"), "this is not valid {{{").unwrap();

        // Write a non-rhai file (should be ignored)
        fs::write(dir.path().join("notes.txt"), "not a script").unwrap();

        let config = crate::config::Config::default();
        let (checks, errors) = load_custom_checks(dir_path, &config);

        // One valid check loaded
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name(), "require_concurrent");

        // One compilation error reported
        assert_eq!(errors.len(), 1);
        assert!(errors[0].file.contains("broken.rhai"));
    }

    #[test]
    fn test_empty_script_no_violations() {
        // An empty .rhai file evaluates to () — should produce no violations
        let violations = run_script("", "CREATE INDEX idx ON users(email);");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_map_with_missing_keys_produces_error_violation() {
        // Map missing "safe_alternative" — should produce an error violation
        let violations = run_script(
            r#"
            #{ operation: "op", problem: "p" }
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].operation.contains("SCRIPT ERROR"));
    }

    #[test]
    fn test_map_with_misspelled_key_produces_error_violation() {
        // Typo: "safe_alterative" instead of "safe_alternative"
        let violations = run_script(
            r#"
            #{ operation: "op", problem: "p", safe_alterative: "s" }
            "#,
            "CREATE INDEX idx ON users(email);",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].operation.contains("SCRIPT ERROR"));
    }

    #[test]
    fn test_pg_constants_accessible_in_scripts() {
        let violations = run_script(
            r#"
            let stmt = node.DropStmt;
            if stmt == () { return; }
            if stmt.remove_type == pg::OBJECT_INDEX {
                #{ operation: "DROP INDEX", problem: "not concurrent", safe_alternative: "use CONCURRENTLY" }
            }
            "#,
            "DROP INDEX idx_users_email;",
        );
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "DROP INDEX");
    }

    #[test]
    fn test_pg_constants_no_match() {
        // Script checks for OBJECT_TABLE but SQL drops an index — should not match
        let violations = run_script(
            r#"
            let stmt = node.DropStmt;
            if stmt == () { return; }
            if stmt.remove_type == pg::OBJECT_TABLE {
                #{ operation: "DROP TABLE", problem: "dangerous", safe_alternative: "be careful" }
            }
            "#,
            "DROP INDEX idx_users_email;",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_load_custom_checks_respects_disable() {
        let dir = TempDir::new().unwrap();
        let dir_path = Utf8Path::from_path(dir.path()).unwrap();

        fs::write(dir.path().join("my_check.rhai"), r#"return;"#).unwrap();

        let config = crate::config::Config {
            disable_checks: vec!["my_check".to_string()],
            ..Default::default()
        };

        let (checks, errors) = load_custom_checks(dir_path, &config);
        assert_eq!(checks.len(), 0);
        assert_eq!(errors.len(), 0);
    }
}

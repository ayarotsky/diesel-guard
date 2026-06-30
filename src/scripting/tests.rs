use super::*;
use crate::checks::pg_helpers::extract_node;
use crate::violation::Violation;
use rhai::Dynamic;
use std::fs;
use tempfile::tempdir;

/// Helper: run a script against a node and return violations.
fn run_script(script: &str, sql: &str) -> Vec<Violation> {
    run_script_with_config(script, sql, &crate::config::Config::default())
}

/// Helper: run a script against a node with explicit config and return violations.
fn run_script_with_config(
    script: &str,
    sql: &str,
    config: &crate::config::Config,
) -> Vec<Violation> {
    run_script_with_ctx(
        script,
        sql,
        config,
        &crate::checks::MigrationContext::default(),
    )
}

/// Helper: run a script against a node with explicit config and ctx and return violations.
fn run_script_with_ctx(
    script: &str,
    sql: &str,
    config: &crate::config::Config,
    ctx: &crate::checks::MigrationContext,
) -> Vec<Violation> {
    let engine = Arc::new(create_engine());
    let ast = engine.compile(script).expect("script should compile");
    let check = CustomCheck {
        name: "test_check",
        engine,
        ast,
        path: String::new(),
    };

    let stmts = crate::parser::parse(sql).expect("SQL should parse");
    let mut all_violations = Vec::new();
    for raw_stmt in &stmts {
        if let Some(node) = extract_node(raw_stmt) {
            all_violations.extend(check.check(node, config, ctx));
        }
    }
    all_violations
}

#[test]
/// Verifies that unit script results produce no violations.
fn test_script_returns_unit_no_violations() {
    let violations = run_script(
        r"
            // Script that always returns unit (no violation)
            let stmt = node.CreateStmt;
            if stmt == () { return; }
            ",
        "CREATE INDEX idx ON t(id);",
    );
    assert!(violations.is_empty());
}

#[test]
/// Verifies that a map script result produces one violation.
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
/// Verifies that an array of map script results produces multiple violations.
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
/// Verifies that non-map array elements are reported as script errors.
fn test_script_array_with_non_map_element_produces_error_violation() {
    let violations = run_script(
        r#"
            [
                #{ operation: "v1", problem: "p1", safe_alternative: "s1" },
                "not a map"
            ]
            "#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].operation, "v1");
    assert_eq!(violations[1].operation, "SCRIPT ERROR: test_check");
    assert_eq!(
        violations[1].problem,
        "Custom check returned a non-map array element: expected a map, got string"
    );
    assert_eq!(
        violations[1].safe_alternative,
        "Fix the custom check script to return maps with operation, problem, and safe_alternative keys."
    );
}

#[test]
/// Verifies that unsupported scalar script results produce script errors.
fn test_script_invalid_return_type_no_crash() {
    // Returning a string instead of map — should produce an error violation
    let violations = run_script(
        r#"
            "not a valid return type"
            "#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
}

#[test]
/// Verifies that infinite scripts hit the Rhai operation limit.
fn test_script_infinite_loop_hits_max_operations() {
    // Engine's max_operations limit should kick in and surface as a SCRIPT ERROR
    let violations = run_script(
        r"
            loop { }
            ",
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(
        violations.len(),
        1,
        "expected 1 SCRIPT ERROR, got: {violations:?}"
    );
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
}

#[test]
/// Verifies that scripts ignore nodes of the wrong AST type.
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
/// Verifies that invalid Rhai source fails compilation.
fn test_compilation_error_reported() {
    let engine = Arc::new(create_engine());
    let result = engine.compile("this is not valid rhai {{{");
    assert!(result.is_err());
}

#[test]
/// Verifies loading valid scripts and reporting invalid scripts from a directory.
fn test_load_custom_checks_from_directory() {
    let dir = tempdir().expect("Failed to create temp dir");
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
/// Verifies that an empty script produces no violations.
fn test_empty_script_no_violations() {
    // An empty .rhai file evaluates to () — should produce no violations
    let violations = run_script("", "CREATE INDEX idx ON users(email);");
    assert!(violations.is_empty());
}

#[test]
/// Verifies that maps missing required keys produce script errors.
fn test_map_with_missing_keys_produces_error_violation() {
    // Map missing "safe_alternative" — should produce an error violation
    let violations = run_script(
        r#"
            #{ operation: "op", problem: "p" }
            "#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
    assert_eq!(
        violations[0].problem,
        "Custom check returned an invalid map: 'safe_alternative' is missing"
    );
}

#[test]
/// Verifies that misspelled required keys are reported as missing keys.
fn test_map_with_misspelled_key_produces_error_violation() {
    // Unrecognized key "safe_alt" instead of "safe_alternative"
    let violations = run_script(
        r#"
            #{ operation: "op", problem: "p", safe_alt: "s" }
            "#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
    assert_eq!(
        violations[0].problem,
        "Custom check returned an invalid map: 'safe_alternative' is missing"
    );
}

#[test]
/// Verifies that PostgreSQL object constants are available to scripts.
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
/// Verifies that config values are available to scripts.
fn test_config_postgres_version_accessible_in_scripts() {
    let config = crate::config::Config {
        postgres_version: Some(14),
        ..Default::default()
    };
    // Script skips violation when postgres_version >= 14
    let violations = run_script_with_config(
        r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            if config.postgres_version != () && config.postgres_version >= 14 { return; }
            #{ operation: "INDEX without CONCURRENTLY", problem: "locks table", safe_alternative: "use CONCURRENTLY" }
            "#,
        "CREATE INDEX idx ON users(email);",
        &config,
    );
    assert!(violations.is_empty());

    // Same script with pg 10 should produce a violation
    let config_old = crate::config::Config {
        postgres_version: Some(10),
        ..Default::default()
    };
    let violations = run_script_with_config(
        r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            if config.postgres_version != () && config.postgres_version >= 14 { return; }
            #{ operation: "INDEX without CONCURRENTLY", problem: "locks table", safe_alternative: "use CONCURRENTLY" }
            "#,
        "CREATE INDEX idx ON users(email);",
        &config_old,
    );
    assert_eq!(violations.len(), 1);
}

#[test]
/// Verifies that PostgreSQL constants do not match unrelated node types.
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
/// Verifies that disabled custom checks are skipped during loading.
fn test_load_custom_checks_respects_disable() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();

    fs::write(dir.path().join("my_check.rhai"), r"return;").unwrap();

    let config = crate::config::Config {
        disable_checks: vec!["my_check".to_string()],
        ..Default::default()
    };

    let (checks, errors) = load_custom_checks(dir_path, &config);
    assert_eq!(checks.len(), 0);
    assert_eq!(errors.len(), 0);
}

#[test]
/// Verifies that custom check names come from sorted `.rhai` file stems.
fn test_custom_check_names_lists_rhai_file_stems() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    fs::write(dir.path().join("zeta.rhai"), "return;").unwrap();
    fs::write(dir.path().join("alpha.rhai"), "return;").unwrap();
    fs::write(dir.path().join("notes.txt"), "return;").unwrap();

    assert_eq!(
        custom_check_names(dir_path),
        vec!["alpha".to_string(), "zeta".to_string()]
    );
}

#[test]
/// Verifies that missing custom check directories report a read error.
fn test_load_custom_checks_nonexistent_directory() {
    let dir = tempdir().expect("Failed to create temp dir");
    let missing = dir.path().join("does_not_exist");
    let dir_path = Utf8Path::from_path(&missing).unwrap();
    let config = crate::config::Config::default();
    let (checks, errors) = load_custom_checks(dir_path, &config);
    assert_eq!(checks.len(), 0);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("Failed to read directory"));
}

#[test]
/// Verifies that transaction context can suppress a script violation.
fn test_ctx_run_in_transaction_false_no_violation() {
    // CONCURRENTLY outside a transaction — no violation
    let ctx = crate::checks::MigrationContext {
        run_in_transaction: false,
        no_transaction_hint: "",
        ..crate::checks::MigrationContext::default()
    };
    let violations = run_script_with_ctx(
        r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            if stmt.concurrent && ctx.run_in_transaction {
                #{ operation: "CONCURRENTLY in transaction", problem: "will fail", safe_alternative: ctx.no_transaction_hint }
            }
            "#,
        "CREATE INDEX CONCURRENTLY idx ON users(email);",
        &crate::config::Config::default(),
        &ctx,
    );
    assert!(violations.is_empty());
}

#[test]
/// Verifies that transaction context can produce a script violation.
fn test_ctx_run_in_transaction_true_produces_violation() {
    // CONCURRENTLY inside a transaction — should flag it
    let ctx = crate::checks::MigrationContext {
        run_in_transaction: true,
        no_transaction_hint: "Add -- diesel:no-transaction to the migration file.",
        ..crate::checks::MigrationContext::default()
    };
    let violations = run_script_with_ctx(
        r#"
            let stmt = node.IndexStmt;
            if stmt == () { return; }
            if stmt.concurrent && ctx.run_in_transaction {
                #{
                    operation: "CONCURRENTLY in transaction",
                    problem: "will fail",
                    safe_alternative: ctx.no_transaction_hint
                }
            }
            "#,
        "CREATE INDEX CONCURRENTLY idx ON users(email);",
        &crate::config::Config::default(),
        &ctx,
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "CONCURRENTLY in transaction");
    assert!(
        violations[0]
            .safe_alternative
            .contains("diesel:no-transaction")
    );
}

#[test]
/// Verifies that non-file `.rhai` paths are rejected.
fn test_load_custom_checks_unreadable_file() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();

    // A directory at the .rhai path always fails fs::read_to_string,
    // even under root — unlike chmod 0o000 which root can bypass.
    let script_path = dir.path().join("unreadable.rhai");
    fs::create_dir(&script_path).unwrap();

    let config = crate::config::Config::default();
    let (checks, errors) = load_custom_checks(dir_path, &config);

    assert_eq!(checks.len(), 0);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("not a regular file"));
}

#[test]
/// Verifies that invalid UTF-8 script source is reported as a read error.
fn test_load_custom_checks_reports_invalid_utf8_script_source() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    fs::write(dir.path().join("invalid_utf8.rhai"), [0xff]).unwrap();

    let config = crate::config::Config::default();
    let (checks, errors) = load_custom_checks(dir_path, &config);

    assert!(checks.is_empty());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].file.contains("invalid_utf8.rhai"));
    assert!(errors[0].message.contains("Failed to read"));
}

#[cfg(unix)]
#[test]
/// Verifies that symlinked `.rhai` paths are rejected on Unix.
fn test_load_custom_checks_rejects_symlink() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("linked.rhai");
    fs::write(&target, "return;").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let config = crate::config::Config::default();
    let (checks, errors) = load_custom_checks(dir_path, &config);

    assert_eq!(checks.len(), 0);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("not a regular file"));
}

#[test]
/// Verifies that oversized scripts are rejected.
fn test_load_custom_checks_rejects_oversized_script() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    let script_path = dir.path().join("too_large.rhai");
    fs::write(
        &script_path,
        " ".repeat(usize::try_from(MAX_CUSTOM_CHECK_SOURCE_BYTES).unwrap() + 1),
    )
    .unwrap();

    let config = crate::config::Config::default();
    let (checks, errors) = load_custom_checks(dir_path, &config);

    assert!(checks.is_empty());
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .contains("Custom check script is larger than")
    );
}

#[test]
/// Verifies that directories with too many scripts report an error.
fn test_load_custom_checks_rejects_too_many_scripts() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    for index in 0..=MAX_CUSTOM_CHECK_FILES {
        fs::write(dir.path().join(format!("check_{index}.rhai")), "return;").unwrap();
    }

    let config = crate::config::Config::default();
    let (_checks, errors) = load_custom_checks(dir_path, &config);

    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("Custom checks directory has more than")
    }));
}

#[test]
/// Verifies that discovery sorts `.rhai` files and filters other entries.
fn test_discover_custom_check_files_sorts_and_filters_entries() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    fs::write(dir.path().join("zeta.rhai"), "return;").unwrap();
    fs::write(dir.path().join("alpha.rhai"), "return;").unwrap();
    fs::write(dir.path().join("notes.txt"), "return;").unwrap();
    fs::create_dir(dir.path().join("nested.rhai")).unwrap();

    let (files, errors) = discover_custom_check_files(dir_path);

    let names: Vec<&str> = files
        .iter()
        .map(|file| file.path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(names, vec!["alpha.rhai", "zeta.rhai"]);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("not a regular file"));
}

#[test]
/// Verifies that scalar parser results are rejected.
fn test_parse_script_result_rejects_scalar() {
    let violations = parse_script_result("scalar_check", Dynamic::from(42_i64));

    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: scalar_check");
    assert!(violations[0].problem.contains("expected (), map, or array"));
}

#[test]
/// Verifies that array result parsing reports non-map elements.
fn test_array_script_violations_reports_non_maps() {
    let mut valid = rhai::Map::new();
    valid.insert("operation".into(), Dynamic::from("op"));
    valid.insert("problem".into(), Dynamic::from("problem"));
    valid.insert("safe_alternative".into(), Dynamic::from("safe"));
    let result = Dynamic::from_array(vec![Dynamic::from(7_i64), Dynamic::from_map(valid)]);

    let violations = parse_script_result("array_check", result);

    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: array_check");
    assert_eq!(
        violations[0].problem,
        "Custom check returned a non-map array element: expected a map, got i64"
    );
    assert_eq!(violations[1].operation, "op");
}

#[test]
/// Verifies that cumulative custom check source size is enforced.
fn test_load_custom_checks_rejects_total_source_bytes() {
    let dir = tempdir().expect("Failed to create temp dir");
    let dir_path = Utf8Path::from_path(dir.path()).unwrap();
    let script = format!(
        "let x = 1;{}",
        " ".repeat(usize::try_from(MAX_CUSTOM_CHECK_SOURCE_BYTES).unwrap() - 10)
    );
    let script_len = u64::try_from(script.len()).unwrap();
    let files_needed = (MAX_CUSTOM_CHECK_TOTAL_SOURCE_BYTES / script_len) + 1;
    for index in 0..files_needed {
        fs::write(dir.path().join(format!("check_{index}.rhai")), &script).unwrap();
    }

    let config = crate::config::Config::default();
    let (_checks, errors) = load_custom_checks(dir_path, &config);

    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("Custom check scripts are larger than")
    }));
}

#[test]
/// Verifies that non-string `operation` fields produce script errors.
fn test_map_with_non_string_operation_field() {
    // operation is an integer, not a string — into_string() returns None,
    // so the match falls through to the error-violation arm in map_to_violation.
    let violations = run_script(
        r#"
            #{ operation: 42, problem: "p", safe_alternative: "s" }
            "#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
    assert_eq!(
        violations[0].problem,
        "Custom check returned an invalid map: 'operation' must be a string (got i64)"
    );
}

#[test]
/// Verifies that non-string `problem` fields produce script errors.
fn test_map_with_non_string_problem_field() {
    let violations = run_script(
        r#"#{ operation: "op", problem: 42, safe_alternative: "s" }"#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
    assert!(
        violations[0].problem.contains("'problem' must be a string"),
        "got: {}",
        violations[0].problem
    );
}

#[test]
/// Verifies that non-string `safe_alternative` fields produce script errors.
fn test_map_with_non_string_safe_alternative_field() {
    let violations = run_script(
        r#"#{ operation: "op", problem: "p", safe_alternative: false }"#,
        "CREATE INDEX idx ON users(email);",
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].operation, "SCRIPT ERROR: test_check");
    assert!(
        violations[0]
            .problem
            .contains("'safe_alternative' must be a string"),
        "got: {}",
        violations[0].problem
    );
}

#[test]
/// Verifies that runtime errors are exposed as script error violations.
fn test_script_runtime_error_yields_script_error_violation() {
    // Division by zero is a runtime error that does NOT contain "ErrorTerminated".
    // A broken script must not silently disable the safety check — it must surface
    // as a SCRIPT ERROR violation.
    let violations = run_script("1 / 0", "CREATE INDEX idx ON users(email);");
    assert_eq!(
        violations.len(),
        1,
        "expected 1 SCRIPT ERROR violation, got: {violations:?}"
    );
    let v = &violations[0];
    assert_eq!(v.operation, "SCRIPT ERROR: test_check");
    assert_eq!(
        v.problem,
        "Runtime error in custom check 'test_check': Division by zero: 1 / 0"
    );
    assert_eq!(
        v.safe_alternative,
        "Fix the custom check script to eliminate the runtime error."
    );
}

#[test]
/// Verifies that representative PostgreSQL enum constants are registered.
fn test_pg_alter_table_constraint_and_drop_constants_accessible() {
    // Verify one representative constant from each untested group:
    // AT_ADD_COLUMN (AlterTableType), CONSTR_PRIMARY (ConstrType), DROP_CASCADE (DropBehavior).
    let violations = run_script(
        r#"
            let at = pg::AT_ADD_COLUMN;
            let ct = pg::CONSTR_PRIMARY;
            let db = pg::DROP_CASCADE;
            if at == () || ct == () || db == () {
                return #{ operation: "MISSING CONSTANT", problem: "a pg constant was ()", safe_alternative: "" };
            }
            "#,
        "SELECT 1;",
    );
    assert!(
        violations.is_empty(),
        "All pg constants should be accessible, got: {violations:?}"
    );
}

#[test]
/// Verifies direct `describe` invocation against a functions-only AST.
fn test_describe_call_fn_debug() {
    let engine = Arc::new(create_engine());
    // Script with fn describe() AND body that references `node` (like a real check script).
    // Verifies that clone_functions_only() lets call_fn succeed even though the body
    // references the `node` variable that is only injected at check time.
    let script = r#"
fn describe() {
    "This check validates something important."
}
let stmt = node.IndexStmt;
if stmt == () { return; }
#{ operation: "MY OPERATION", problem: "my problem", safe_alternative: "my safe alternative" }
"#;
    let ast = engine.compile(script).expect("script should compile");
    let fns_ast = ast.clone_functions_only();
    let mut scope = rhai::Scope::new();
    let result = engine.call_fn::<rhai::Dynamic>(&mut scope, &fns_ast, "describe", ());
    let result = result.expect("call_fn on fns-only AST should succeed");
    assert_eq!(
        result.into_string().ok(),
        Some("This check validates something important.".to_string())
    );
}

#[test]
/// Verifies that custom checks expose script-provided descriptions.
fn test_describe_with_fn_describe_in_script() {
    let engine = Arc::new(create_engine());
    let script = r#"
fn describe() {
    "This check validates something important."
}
let stmt = node.IndexStmt;
if stmt == () { return; }
#{ operation: "MY OPERATION", problem: "my problem", safe_alternative: "my safe alternative" }
"#;
    let ast = engine.compile(script).expect("script should compile");
    let check = CustomCheck {
        name: "my_check",
        engine,
        ast,
        path: "/path/my_check.rhai".to_string(),
    };

    assert_eq!(
        check.describe(),
        Some("This check validates something important.".to_string())
    );
}

#[test]
/// Verifies that custom checks without `describe` return no documentation.
fn test_describe_without_fn_describe_in_script_falls_back() {
    let engine = Arc::new(create_engine());
    let ast = engine.compile("()").expect("script should compile");
    let check = CustomCheck {
        name: "no_desc",
        engine,
        ast,
        path: "/path/no_desc.rhai".to_string(),
    };

    assert_eq!(check.describe(), None);
}

#[test]
/// Verifies that custom checks expose their source script path.
fn test_custom_check_script_path_returns_path() {
    let engine = Arc::new(create_engine());
    let ast = engine.compile("()").expect("script should compile");
    let check = CustomCheck {
        name: "path_check",
        engine,
        ast,
        path: "/checks/my_check.rhai".to_string(),
    };

    assert_eq!(check.script_path(), Some("/checks/my_check.rhai"));
}

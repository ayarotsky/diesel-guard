use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_custom_check_fires_alongside_builtin() {
    let dir = TempDir::new().unwrap();

    // Write a custom check that flags non-concurrent indexes
    fs::write(
        dir.path().join("require_concurrent.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        if !stmt.concurrent {
            #{
                operation: "CUSTOM: INDEX without CONCURRENTLY",
                problem: "Blocks writes on the table",
                safe_alternative: "Use CREATE INDEX CONCURRENTLY"
            }
        }
        "#,
    )
    .unwrap();

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    // This SQL triggers both the built-in AddIndexCheck and our custom check
    let violations = checker
        .check_sql("CREATE INDEX idx ON users(email);")
        .unwrap();

    // Should have at least 2 violations: built-in + custom
    assert!(
        violations.len() >= 2,
        "Expected at least 2 violations (built-in + custom), got {}",
        violations.len()
    );

    // Verify custom violation is present
    assert!(
        violations
            .iter()
            .any(|v| v.operation == "CUSTOM: INDEX without CONCURRENTLY"),
        "Custom check violation should be present"
    );

    // Verify built-in violation is also present
    assert!(
        violations.iter().any(|v| v.operation.contains("ADD INDEX")),
        "Built-in AddIndexCheck violation should be present"
    );
}

#[test]
fn test_disable_checks_works_for_custom_check() {
    let dir = TempDir::new().unwrap();

    fs::write(
        dir.path().join("my_custom.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        #{
            operation: "custom violation",
            problem: "p",
            safe_alternative: "s"
        }
        "#,
    )
    .unwrap();

    // Disable the custom check by its filename stem
    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        disable_checks: vec!["my_custom".to_string()],
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let violations = checker
        .check_sql("CREATE INDEX idx ON users(email);")
        .unwrap();

    // Custom check should be disabled — no "custom violation"
    assert!(
        !violations.iter().any(|v| v.operation == "custom violation"),
        "Disabled custom check should not fire"
    );
}

#[test]
fn test_safety_assured_blocks_skip_custom_checks() {
    let dir = TempDir::new().unwrap();

    fs::write(
        dir.path().join("always_fires.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        #{
            operation: "always fires",
            problem: "p",
            safe_alternative: "s"
        }
        "#,
    )
    .unwrap();

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let sql = r#"
-- safety-assured:start
CREATE INDEX idx ON users(email);
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();

    // Both built-in and custom checks should be skipped in safety-assured block
    assert!(
        violations.is_empty(),
        "No violations should fire inside safety-assured block, got {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_custom_check_in_migration_directory() {
    let checks_dir = TempDir::new().unwrap();
    let migrations_dir = TempDir::new().unwrap();

    // Write a custom check
    fs::write(
        checks_dir.path().join("no_drop.rhai"),
        r#"
        let stmt = node.DropStmt;
        if stmt == () { return; }
        #{
            operation: "CUSTOM DROP",
            problem: "dropping things is scary",
            safe_alternative: "don't do it"
        }
        "#,
    )
    .unwrap();

    // Create a migration directory with a drop statement
    let mig_dir = migrations_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&mig_dir).unwrap();
    fs::write(mig_dir.join("up.sql"), "DROP TABLE IF EXISTS old_table;").unwrap();

    let config = Config {
        custom_checks_dir: Some(checks_dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let results = checker
        .check_path(Utf8Path::from_path(migrations_dir.path()).unwrap())
        .unwrap();

    // Should have violations from both built-in DropTableCheck and custom no_drop
    assert!(!results.is_empty());
    let violations: Vec<_> = results.iter().flat_map(|(_, v)| v).collect();
    assert!(
        violations.iter().any(|v| v.operation == "CUSTOM DROP"),
        "Custom check should fire on migration files"
    );
}

#[test]
fn test_nonexistent_custom_checks_dir_is_ignored() {
    let config = Config {
        custom_checks_dir: Some("/nonexistent/path/to/checks".to_string()),
        ..Default::default()
    };

    // Should not panic or error — just ignored
    let checker = SafetyChecker::with_config(config);
    let violations = checker.check_sql("SELECT 1;").unwrap();
    assert!(violations.is_empty());
}

use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_start_after_plus_custom_checks_plus_safety_assured() {
    let checks_dir = TempDir::new().unwrap();
    let migrations_dir = TempDir::new().unwrap();

    // Custom check that flags DROP TABLE
    fs::write(
        checks_dir.path().join("no_drop_table.rhai"),
        r#"
        let stmt = node.DropStmt;
        if stmt == () { return; }
        if stmt.remove_type == pg::OBJECT_TABLE {
            #{
                operation: "CUSTOM: DROP TABLE",
                problem: "Dropping tables is dangerous",
                safe_alternative: "Use soft deletes"
            }
        }
        "#,
    )
    .unwrap();

    // Old migration (before start_after — should be skipped entirely)
    let old_dir = migrations_dir.path().join("2023_01_01_000000_old");
    fs::create_dir(&old_dir).unwrap();
    fs::write(old_dir.join("up.sql"), "DROP TABLE old_table;").unwrap();

    // New migration with safety-assured block and unprotected statement
    let new_dir = migrations_dir.path().join("2025_01_01_000000_new");
    fs::create_dir(&new_dir).unwrap();
    fs::write(
        new_dir.join("up.sql"),
        r#"-- safety-assured:start
DROP TABLE protected_table;
-- safety-assured:end
DROP TABLE unprotected_table;
"#,
    )
    .unwrap();

    let config = Config {
        framework: "diesel".to_string(),
        start_after: Some("2024_01_01_000000".to_string()),
        custom_checks_dir: Some(checks_dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(migrations_dir.path()).unwrap())
        .unwrap();

    // Old migration should be skipped
    assert!(
        !results.iter().any(|(p, _)| p.contains("2023_01_01")),
        "Old migration should be skipped by start_after"
    );

    // New migration should have violations only for unprotected statement
    assert_eq!(
        results.len(),
        1,
        "Only the new migration should have violations"
    );
    let violations = &results[0].1;

    // Should have built-in DropTableCheck violation
    assert!(
        violations
            .iter()
            .any(|v| v.operation.contains("DROP TABLE") && !v.operation.contains("CUSTOM")),
        "Built-in DropTableCheck should fire on unprotected DROP TABLE"
    );

    // Should have custom check violation
    assert!(
        violations
            .iter()
            .any(|v| v.operation == "CUSTOM: DROP TABLE"),
        "Custom check should fire on unprotected DROP TABLE"
    );

    // No violations should reference the safety-assured block
    // (protected_table is inside safety-assured, unprotected_table is outside)
}

#[test]
fn test_disable_builtin_and_custom_checks_simultaneously() {
    let checks_dir = TempDir::new().unwrap();

    // Custom check that flags every DropStmt
    fs::write(
        checks_dir.path().join("my_check.rhai"),
        r#"
        let stmt = node.DropStmt;
        if stmt == () { return; }
        #{
            operation: "CUSTOM: DROP",
            problem: "custom detected drop",
            safe_alternative: "don't drop"
        }
        "#,
    )
    .unwrap();

    // Disable both DropColumnCheck (built-in) and my_check (custom)
    let config = Config {
        custom_checks_dir: Some(checks_dir.path().to_str().unwrap().to_string()),
        disable_checks: vec!["DropColumnCheck".to_string(), "my_check".to_string()],
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    // SQL that triggers DropTableCheck, DropColumnCheck, and my_check
    let violations = checker
        .check_sql("DROP TABLE t; ALTER TABLE t DROP COLUMN c;")
        .unwrap();

    // DropTableCheck should still fire (not disabled)
    assert!(
        violations
            .iter()
            .any(|v| v.operation.contains("DROP TABLE")),
        "DropTableCheck should still fire (not disabled)"
    );

    // DropColumnCheck should NOT fire (disabled)
    assert!(
        !violations
            .iter()
            .any(|v| v.operation.contains("DROP COLUMN")),
        "DropColumnCheck should be disabled"
    );

    // Custom my_check should NOT fire (disabled)
    assert!(
        !violations.iter().any(|v| v.operation.contains("CUSTOM")),
        "Custom check my_check should be disabled"
    );
}

#[test]
fn test_check_down_with_safety_assured_in_down_sql() {
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();

    // Safe up.sql
    fs::write(
        migration_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN email VARCHAR(255);",
    )
    .unwrap();

    // down.sql with safety-assured block and unprotected statement
    fs::write(
        migration_dir.join("down.sql"),
        r#"-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
ALTER TABLE users DROP COLUMN phone;
"#,
    )
    .unwrap();

    let config = Config {
        check_down: true,
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Only down.sql should have violations (up.sql is safe)
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("down.sql"));

    // The safety-assured DROP COLUMN email should not produce violations
    // but unprotected DROP COLUMN phone should
    let violations = &results[0].1;
    assert!(
        !violations.is_empty(),
        "Unprotected DROP COLUMN should produce a violation"
    );

    // All violations should be from the unprotected statement only
    assert_eq!(
        violations.len(),
        1,
        "Only the unprotected DROP COLUMN should produce a violation, got {:?}",
        violations
    );
}

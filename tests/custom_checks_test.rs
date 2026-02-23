use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker, Violation};
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

#[test]
fn test_registry_check_names_includes_custom_checks() {
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

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let registry = diesel_guard::checks::Registry::with_config(&config);

    // Before adding custom checks, my_custom should NOT be in the list
    let names_before = registry.active_check_names();
    assert!(
        !names_before.contains(&"my_custom"),
        "Custom check should not appear in registry before loading"
    );

    // After building SafetyChecker (which loads custom checks), rebuild
    // a registry that includes custom checks to verify check_names works
    let mut registry = diesel_guard::checks::Registry::with_config(&config);
    let (checks, _) = diesel_guard::scripting::load_custom_checks(
        Utf8Path::new(dir.path().to_str().unwrap()),
        &config,
    );
    for check in checks {
        registry.add_check(check);
    }

    let names_after = registry.active_check_names();
    assert!(
        names_after.contains(&"my_custom"),
        "Custom check 'my_custom' should appear in check_names after loading, got: {:?}",
        names_after
    );
}

#[test]
fn test_unknown_check_name_warning_not_emitted_for_custom_check() {
    // When a disable_checks entry matches a custom check script name,
    // SafetyChecker::with_config() should NOT warn about it.
    // The validation checks .rhai file stems on disk, not just what was
    // loaded into the registry (disabled checks are skipped during loading).
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

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        disable_checks: vec!["my_custom".to_string()],
        ..Default::default()
    };

    // Building SafetyChecker should not panic or error.
    // The "my_custom" name matches a .rhai file stem so no warning is expected.
    let checker = SafetyChecker::with_config(config);

    // Verify the custom check is actually disabled
    let violations = checker
        .check_sql("CREATE INDEX idx ON users(email);")
        .unwrap();
    assert!(
        !violations.iter().any(|v| v.operation == "custom violation"),
        "Disabled custom check should not fire"
    );
}

#[test]
fn test_unknown_check_name_detected_after_custom_checks_loaded() {
    // A truly unknown name (typo) should not match any built-in name or
    // custom .rhai file stem. SafetyChecker::with_config() will warn about it
    // on stderr. We verify indirectly: the name doesn't appear in either the
    // built-in list or the custom check file stems.
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

    let bogus = "TotallyBogusCheckName";

    // Verify it doesn't match any built-in name
    assert!(
        !diesel_guard::checks::Registry::builtin_check_names().contains(&bogus),
        "Typo should not match any built-in check name"
    );

    // Verify it doesn't match any custom check file stem
    let rhai_stems: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| {
            let path = e.ok()?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    assert!(
        !rhai_stems.contains(&bogus.to_string()),
        "Typo should not match any custom check file stem"
    );

    // Building SafetyChecker should still succeed (warning on stderr, not an error)
    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        disable_checks: vec![bogus.to_string()],
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let violations = checker.check_sql("SELECT 1;").unwrap();
    assert!(violations.is_empty());
}

// ---------------------------------------------------------------------------
// Example script validation tests
// ---------------------------------------------------------------------------

/// Run SQL through the real `examples/` directory with all built-in checks
/// disabled so only example Rhai scripts produce violations.
fn check_with_examples(sql: &str) -> Vec<Violation> {
    let config = Config {
        custom_checks_dir: Some("examples".to_string()),
        disable_checks: vec![
            "AddColumnCheck".into(),
            "AddIndexCheck".into(),
            "AddJsonColumnCheck".into(),
            "AddNotNullCheck".into(),
            "AddPrimaryKeyCheck".into(),
            "AddSerialColumnCheck".into(),
            "AddUniqueConstraintCheck".into(),
            "AlterColumnTypeCheck".into(),
            "CharTypeCheck".into(),
            "CreateExtensionCheck".into(),
            "DropColumnCheck".into(),
            "DropDatabaseCheck".into(),
            "DropIndexCheck".into(),
            "DropPrimaryKeyCheck".into(),
            "DropTableCheck".into(),
            "GeneratedColumnCheck".into(),
            "ReindexCheck".into(),
            "RenameColumnCheck".into(),
            "RenameTableCheck".into(),
            "ShortIntegerPrimaryKeyCheck".into(),
            "TimestampTypeCheck".into(),
            "TruncateTableCheck".into(),
            "UnnamedConstraintCheck".into(),
            "WideIndexCheck".into(),
        ],
        ..Default::default()
    };
    SafetyChecker::with_config(config).check_sql(sql).unwrap()
}

fn has_violation_containing(violations: &[Violation], substring: &str) -> bool {
    violations.iter().any(|v| v.operation.contains(substring))
}

// -- require_concurrent_index.rhai --

#[test]
fn test_example_require_concurrent_index_detects() {
    let violations = check_with_examples("CREATE INDEX idx ON users(email);");
    assert!(
        has_violation_containing(&violations, "INDEX without CONCURRENTLY"),
        "Expected violation for non-concurrent index, got: {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_example_require_concurrent_index_allows() {
    let violations = check_with_examples("CREATE INDEX CONCURRENTLY idx ON users(email);");
    assert!(
        !has_violation_containing(&violations, "INDEX without CONCURRENTLY"),
        "Concurrent index should not trigger violation"
    );
}

// -- no_truncate_in_production.rhai --

#[test]
fn test_example_no_truncate_detects_single() {
    let violations = check_with_examples("TRUNCATE users;");
    let truncate_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.operation.starts_with("TRUNCATE:"))
        .collect();
    assert_eq!(
        truncate_violations.len(),
        1,
        "Expected 1 TRUNCATE violation, got {:?}",
        truncate_violations
    );
}

#[test]
fn test_example_no_truncate_detects_multiple() {
    let violations = check_with_examples("TRUNCATE users, orders;");
    let truncate_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.operation.starts_with("TRUNCATE:"))
        .collect();
    assert_eq!(
        truncate_violations.len(),
        2,
        "Expected 2 TRUNCATE violations (one per table), got {:?}",
        truncate_violations
    );
}

#[test]
fn test_example_no_truncate_ignores_unrelated() {
    let violations = check_with_examples("DROP TABLE users;");
    assert!(
        !has_violation_containing(&violations, "TRUNCATE"),
        "DROP TABLE should not trigger TRUNCATE violation"
    );
}

// -- require_if_exists_on_drop.rhai --

#[test]
fn test_example_require_if_exists_on_drop_detects() {
    let violations = check_with_examples("DROP TABLE users;");
    assert!(
        has_violation_containing(&violations, "DROP TABLE without IF EXISTS"),
        "Expected violation for DROP TABLE without IF EXISTS, got: {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_example_require_if_exists_on_drop_allows() {
    let violations = check_with_examples("DROP TABLE IF EXISTS users;");
    assert!(
        !has_violation_containing(&violations, "DROP TABLE without IF EXISTS"),
        "DROP TABLE IF EXISTS should not trigger violation"
    );
}

// -- require_index_name_prefix.rhai --

#[test]
fn test_example_require_index_name_prefix_detects() {
    let violations = check_with_examples("CREATE INDEX CONCURRENTLY users_email ON users(email);");
    assert!(
        has_violation_containing(&violations, "Index naming violation"),
        "Expected violation for missing idx_ prefix, got: {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_example_require_index_name_prefix_allows() {
    let violations =
        check_with_examples("CREATE INDEX CONCURRENTLY idx_users_email ON users(email);");
    assert!(
        !has_violation_containing(&violations, "Index naming violation"),
        "idx_ prefixed index should not trigger naming violation"
    );
}

// -- limit_columns_per_index.rhai --

#[test]
fn test_example_limit_columns_per_index_detects() {
    let violations = check_with_examples("CREATE INDEX CONCURRENTLY idx_t ON t(a, b, c, d);");
    assert!(
        has_violation_containing(&violations, "Wide index"),
        "Expected violation for 4-column index, got: {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_custom_check_returning_array_of_violations() {
    let dir = TempDir::new().unwrap();

    // Script that always returns an array of 3 violations for any IndexStmt
    fs::write(
        dir.path().join("triple.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        [
            #{ operation: "v1", problem: "p1", safe_alternative: "s1" },
            #{ operation: "v2", problem: "p2", safe_alternative: "s2" },
            #{ operation: "v3", problem: "p3", safe_alternative: "s3" }
        ]
        "#,
    )
    .unwrap();

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let violations = checker
        .check_sql("CREATE INDEX idx ON users(email);")
        .unwrap();

    // Filter to only our custom violations
    let custom: Vec<_> = violations
        .iter()
        .filter(|v| ["v1", "v2", "v3"].contains(&v.operation.as_str()))
        .collect();
    assert_eq!(
        custom.len(),
        3,
        "Script returning array of 3 maps should produce exactly 3 custom violations, got: {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_custom_check_using_pg_constants() {
    let dir = TempDir::new().unwrap();

    // Script that uses pg::OBJECT_TABLE to detect DROP TABLE
    fs::write(
        dir.path().join("detect_drop_table.rhai"),
        r#"
        let stmt = node.DropStmt;
        if stmt == () { return; }
        if stmt.remove_type == pg::OBJECT_TABLE {
            #{
                operation: "CUSTOM: DROP TABLE detected",
                problem: "Table drops are dangerous",
                safe_alternative: "Use soft deletes"
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

    // DROP TABLE should fire
    let violations_table = checker.check_sql("DROP TABLE users;").unwrap();
    assert!(
        violations_table
            .iter()
            .any(|v| v.operation == "CUSTOM: DROP TABLE detected"),
        "Script using pg::OBJECT_TABLE should fire on DROP TABLE"
    );

    // DROP INDEX should NOT fire
    let violations_index = checker.check_sql("DROP INDEX idx_users_email;").unwrap();
    assert!(
        !violations_index
            .iter()
            .any(|v| v.operation == "CUSTOM: DROP TABLE detected"),
        "Script checking for OBJECT_TABLE should not fire on DROP INDEX"
    );
}

#[test]
fn test_multiple_custom_checks_loaded_in_sorted_order() {
    let dir = TempDir::new().unwrap();

    // Two scripts that both fire on IndexStmt — named to test sort order
    fs::write(
        dir.path().join("aaa_check.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        #{ operation: "aaa_check", problem: "p", safe_alternative: "s" }
        "#,
    )
    .unwrap();

    fs::write(
        dir.path().join("zzz_check.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        #{ operation: "zzz_check", problem: "p", safe_alternative: "s" }
        "#,
    )
    .unwrap();

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let violations = checker
        .check_sql("CREATE INDEX idx ON users(email);")
        .unwrap();

    let custom_ops: Vec<&str> = violations
        .iter()
        .map(|v| v.operation.as_str())
        .filter(|op| *op == "aaa_check" || *op == "zzz_check")
        .collect();

    assert_eq!(custom_ops.len(), 2, "Both custom checks should fire");

    // aaa should come before zzz (alphabetical load order)
    let aaa_pos = custom_ops.iter().position(|&op| op == "aaa_check").unwrap();
    let zzz_pos = custom_ops.iter().position(|&op| op == "zzz_check").unwrap();
    assert!(
        aaa_pos < zzz_pos,
        "aaa_check should appear before zzz_check (alphabetical order)"
    );
}

#[test]
fn test_custom_check_name_conflicts_with_builtin() {
    let dir = TempDir::new().unwrap();

    // Script with the same name as a built-in check
    fs::write(
        dir.path().join("AddColumnCheck.rhai"),
        r#"
        let stmt = node.AlterTableStmt;
        if stmt == () { return; }
        #{ operation: "CUSTOM AddColumnCheck", problem: "p", safe_alternative: "s" }
        "#,
    )
    .unwrap();

    // Disabling "AddColumnCheck" should disable BOTH the built-in and the custom check
    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        disable_checks: vec!["AddColumnCheck".to_string()],
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let violations = checker
        .check_sql("ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;")
        .unwrap();

    // Built-in AddColumnCheck should be disabled
    assert!(
        !violations
            .iter()
            .any(|v| v.operation.contains("ADD COLUMN") && !v.operation.contains("CUSTOM")),
        "Built-in AddColumnCheck should be disabled"
    );

    // Custom AddColumnCheck.rhai should also be disabled (same name used by is_check_enabled)
    assert!(
        !violations
            .iter()
            .any(|v| v.operation == "CUSTOM AddColumnCheck"),
        "Custom check with same name as built-in should also be disabled"
    );
}

#[test]
fn test_custom_check_compilation_error_nonfatal() {
    let dir = TempDir::new().unwrap();

    // Broken script (invalid syntax)
    fs::write(dir.path().join("broken.rhai"), "this is not valid rhai {{{").unwrap();

    // Valid script
    fs::write(
        dir.path().join("valid.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        #{ operation: "valid_check", problem: "p", safe_alternative: "s" }
        "#,
    )
    .unwrap();

    let config = Config {
        custom_checks_dir: Some(dir.path().to_str().unwrap().to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    // check_sql should still work — broken script is skipped, valid one fires
    let violations = checker
        .check_sql("CREATE INDEX idx ON users(email);")
        .unwrap();

    assert!(
        violations.iter().any(|v| v.operation == "valid_check"),
        "Valid check should still fire despite broken sibling script"
    );
}

#[test]
fn test_example_limit_columns_per_index_allows() {
    let violations = check_with_examples("CREATE INDEX CONCURRENTLY idx_t ON t(a, b, c);");
    assert!(
        !has_violation_containing(&violations, "Wide index"),
        "3-column index should not trigger wide index violation"
    );
}

// -- config variable in custom checks --

#[test]
fn test_custom_check_can_read_postgres_version() {
    let dir = TempDir::new().unwrap();

    // Script that skips the violation when postgres_version >= 14
    fs::write(
        dir.path().join("version_aware.rhai"),
        r#"
        let stmt = node.IndexStmt;
        if stmt == () { return; }
        if config.postgres_version != () && config.postgres_version >= 14 { return; }
        #{
            operation: "CUSTOM: needs pg14",
            problem: "only needed below pg14",
            safe_alternative: "upgrade postgres"
        }
        "#,
    )
    .unwrap();

    let dir_path = dir.path().to_str().unwrap().to_string();

    // With postgres_version = 14: no violation
    let config_pg14 = Config {
        custom_checks_dir: Some(dir_path.clone()),
        postgres_version: Some(14),
        ..Default::default()
    };
    let violations = SafetyChecker::with_config(config_pg14)
        .check_sql("CREATE INDEX CONCURRENTLY idx ON users(email);")
        .unwrap();
    assert!(
        !violations
            .iter()
            .any(|v| v.operation == "CUSTOM: needs pg14"),
        "Version-aware custom check should not fire on pg14"
    );

    // With postgres_version = 13: violation
    let config_pg13 = Config {
        custom_checks_dir: Some(dir_path.clone()),
        postgres_version: Some(13),
        ..Default::default()
    };
    let violations = SafetyChecker::with_config(config_pg13)
        .check_sql("CREATE INDEX CONCURRENTLY idx ON users(email);")
        .unwrap();
    assert!(
        violations
            .iter()
            .any(|v| v.operation == "CUSTOM: needs pg14"),
        "Version-aware custom check should fire on pg13"
    );

    // With no postgres_version set: violation (None → () in Rhai, condition is skipped)
    let config_no_ver = Config {
        custom_checks_dir: Some(dir_path),
        postgres_version: None,
        ..Default::default()
    };
    let violations = SafetyChecker::with_config(config_no_ver)
        .check_sql("CREATE INDEX CONCURRENTLY idx ON users(email);")
        .unwrap();
    assert!(
        violations
            .iter()
            .any(|v| v.operation == "CUSTOM: needs pg14"),
        "Version-aware custom check should fire when no version is set"
    );
}

// -- no_unlogged_tables.rhai --

#[test]
fn test_example_no_unlogged_tables_detects() {
    let violations = check_with_examples("CREATE UNLOGGED TABLE t (id INT);");
    assert!(
        has_violation_containing(&violations, "UNLOGGED TABLE"),
        "Expected violation for UNLOGGED TABLE, got: {:?}",
        violations.iter().map(|v| &v.operation).collect::<Vec<_>>()
    );
}

#[test]
fn test_example_no_unlogged_tables_allows() {
    let violations = check_with_examples("CREATE TABLE t (id INT);");
    assert!(
        !has_violation_containing(&violations, "UNLOGGED TABLE"),
        "Regular table should not trigger UNLOGGED violation"
    );
}

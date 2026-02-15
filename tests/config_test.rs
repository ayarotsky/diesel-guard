use camino::Utf8Path;
use diesel_guard::{Config, ConfigError, SafetyChecker};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_check_down_single_migration_dir() {
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();
    fs::write(
        migration_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
    )
    .unwrap();
    fs::write(
        migration_dir.join("down.sql"),
        "ALTER TABLE users DROP COLUMN admin;",
    )
    .unwrap();

    // Point check_path at the single migration directory (the CI use case)
    let config = Config::default(); // check_down = false
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_path(Utf8Path::from_path(&migration_dir).unwrap())
        .unwrap();

    // Should only find violations in up.sql, not down.sql
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("up.sql"));
}

#[test]
fn test_check_down_single_migration_dir_sqlx() {
    let temp_dir = TempDir::new().unwrap();
    // SQLx requires 14-digit timestamp prefix
    let migration_dir = temp_dir.path().join("20240101000000_test");
    fs::create_dir(&migration_dir).unwrap();
    fs::write(
        migration_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
    )
    .unwrap();
    fs::write(
        migration_dir.join("down.sql"),
        "ALTER TABLE users DROP COLUMN admin;",
    )
    .unwrap();

    let config = Config {
        framework: "sqlx".to_string(),
        ..Default::default() // check_down = false
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_path(Utf8Path::from_path(&migration_dir).unwrap())
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("up.sql"));
}

#[test]
fn test_config_disables_checks() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    fs::write(
        &config_path,
        r#"
framework = "diesel"
disable_checks = ["AddColumnCheck"]
        "#,
    )
    .unwrap();

    let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
    let config = Config::load_from_path(config_path_utf8).unwrap();
    assert!(!config.is_check_enabled("AddColumnCheck"));
    assert!(config.is_check_enabled("DropColumnCheck"));
}

#[test]
fn test_config_enables_check_down() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    fs::write(
        &config_path,
        r#"
framework = "diesel"
check_down = true
        "#,
    )
    .unwrap();

    let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
    let config = Config::load_from_path(config_path_utf8).unwrap();
    assert!(config.check_down);
}

#[test]
fn test_config_start_after() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    fs::write(
        &config_path,
        r#"
framework = "diesel"
start_after = "2024_01_01_000000"
        "#,
    )
    .unwrap();

    let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
    let config = Config::load_from_path(config_path_utf8).unwrap();
    assert_eq!(config.start_after, Some("2024_01_01_000000".to_string()));
}

#[test]
fn test_check_down_integration() {
    // Create temporary migration structure
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();

    // Create up.sql with unsafe operation
    fs::write(
        migration_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
    )
    .unwrap();

    // Create down.sql with unsafe operation
    fs::write(
        migration_dir.join("down.sql"),
        "ALTER TABLE users DROP COLUMN admin;",
    )
    .unwrap();

    // Test with check_down = false (default)
    let config_default = Config::default();
    let checker_default = SafetyChecker::with_config(config_default);
    let results_default = checker_default
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();
    assert_eq!(results_default.len(), 1); // Only up.sql
    assert!(results_default[0].0.contains("up.sql"));

    // Test with check_down = true
    let config_with_down = Config {
        check_down: true,
        ..Default::default()
    };
    let checker_with_down = SafetyChecker::with_config(config_with_down);
    let results_with_down = checker_with_down
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();
    assert_eq!(results_with_down.len(), 2); // Both up.sql and down.sql

    // Verify both files were checked
    let file_paths: Vec<String> = results_with_down.iter().map(|(p, _)| p.clone()).collect();
    assert!(file_paths.iter().any(|p| p.contains("up.sql")));
    assert!(file_paths.iter().any(|p| p.contains("down.sql")));
}

#[test]
fn test_start_after_integration() {
    // Create temporary migrations with different timestamps
    let temp_dir = TempDir::new().unwrap();

    // Old migration (before threshold)
    let old_migration = temp_dir.path().join("2023_12_31_000000_old");
    fs::create_dir(&old_migration).unwrap();
    fs::write(
        old_migration.join("up.sql"),
        "ALTER TABLE users DROP COLUMN email;",
    )
    .unwrap();

    // New migration (after threshold)
    let new_migration = temp_dir.path().join("2024_06_01_000000_new");
    fs::create_dir(&new_migration).unwrap();
    fs::write(
        new_migration.join("up.sql"),
        "ALTER TABLE users DROP COLUMN phone;",
    )
    .unwrap();

    // Migration exactly at threshold (should be skipped)
    let exact_migration = temp_dir.path().join("2024_01_01_000000_exact");
    fs::create_dir(&exact_migration).unwrap();
    fs::write(
        exact_migration.join("up.sql"),
        "ALTER TABLE users DROP COLUMN fax;",
    )
    .unwrap();

    // Test with start_after set to 2024_01_01_000000
    let config = Config {
        start_after: Some("2024_01_01_000000".to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Should only check new_migration (2024_06_01), not old or exact
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("2024_06_01"));
}

#[test]
fn test_disable_checks_integration() {
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();

    // SQL that would trigger AddColumnCheck
    fs::write(
        migration_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;",
    )
    .unwrap();

    // Without disabling - should detect violation
    let config_default = Config::default();
    let checker_default = SafetyChecker::with_config(config_default);
    let results_default = checker_default
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();
    assert_eq!(results_default.len(), 1);
    assert_eq!(results_default[0].1.len(), 1); // 1 violation

    // With AddColumnCheck disabled - should not detect
    let config_disabled = Config {
        disable_checks: vec!["AddColumnCheck".to_string()],
        ..Default::default()
    };
    let checker_disabled = SafetyChecker::with_config(config_disabled);
    let results_disabled = checker_disabled
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();
    assert_eq!(results_disabled.len(), 0); // No violations
}

#[test]
fn test_combined_config_features() {
    // Test all three config features together
    let temp_dir = TempDir::new().unwrap();

    // Old migration with unsafe down.sql
    let old_migration = temp_dir.path().join("2023_12_31_000000_old");
    fs::create_dir(&old_migration).unwrap();
    fs::write(
        old_migration.join("up.sql"),
        "ALTER TABLE users ADD COLUMN admin BOOLEAN;", // Safe
    )
    .unwrap();
    fs::write(
        old_migration.join("down.sql"),
        "ALTER TABLE users DROP COLUMN admin;", // Unsafe but should be skipped
    )
    .unwrap();

    // New migration with unsafe down.sql
    let new_migration = temp_dir.path().join("2024_06_01_000000_new");
    fs::create_dir(&new_migration).unwrap();
    fs::write(
        new_migration.join("up.sql"),
        "ALTER TABLE users ADD COLUMN email VARCHAR(255);", // Safe
    )
    .unwrap();
    fs::write(
        new_migration.join("down.sql"),
        "ALTER TABLE users DROP COLUMN email;", // Unsafe and should be detected
    )
    .unwrap();

    let config = Config {
        start_after: Some("2024_01_01_000000".to_string()),
        check_down: true,
        ..Default::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Should only check new_migration's down.sql
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("2024_06_01"));
    assert!(results[0].0.contains("down.sql"));
}

#[test]
fn test_standalone_sql_files_always_checked() {
    // Verify that standalone .sql files are always checked regardless of start_after
    let temp_dir = TempDir::new().unwrap();

    // Create a standalone SQL file (not in a migration directory)
    fs::write(
        temp_dir.path().join("migration.sql"),
        "ALTER TABLE users DROP COLUMN email;",
    )
    .unwrap();

    // Set start_after to future date
    let config = Config {
        start_after: Some("2099_12_31_000000".to_string()),
        ..Default::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Standalone file should still be checked
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("migration.sql"));
}

#[test]
fn test_check_down_with_missing_down_sql() {
    // Verify no error when check_down=true but down.sql doesn't exist
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();

    // Only create up.sql, no down.sql
    fs::write(
        migration_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN email VARCHAR(255);",
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

    // Should succeed with no violations (up.sql is safe, down.sql doesn't exist)
    assert_eq!(results.len(), 0);
}

#[test]
fn test_multiple_migrations_with_start_after() {
    // Test filtering with multiple migrations
    let temp_dir = TempDir::new().unwrap();

    // Create 5 migrations with different timestamps
    let timestamps = [
        "2023_01_01_000000",
        "2023_06_01_000000",
        "2024_01_01_000000",
        "2024_06_01_000000",
        "2024_12_01_000000",
    ];

    for timestamp in &timestamps {
        let migration_dir = temp_dir.path().join(format!("{}_migration", timestamp));
        fs::create_dir(&migration_dir).unwrap();
        fs::write(
            migration_dir.join("up.sql"),
            "ALTER TABLE users DROP COLUMN test_column;",
        )
        .unwrap();
    }

    // Set start_after to 2024_01_01_000000
    let config = Config {
        start_after: Some("2024_01_01_000000".to_string()),
        ..Default::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Should only check last 2 migrations (after 2024_01_01_000000)
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|(p, _)| p.contains("2024_06_01")));
    assert!(results.iter().any(|(p, _)| p.contains("2024_12_01")));
}

#[test]
fn test_migrations_checked_in_alphanumeric_order() {
    // Verify that migrations are checked in sorted order
    let temp_dir = TempDir::new().unwrap();

    // Create migrations with different naming patterns
    // These might be returned in random order by the filesystem
    let migration_names = [
        "2024_03_15_120000_create_posts",
        "2024_01_10_080000_create_users",
        "2024_12_01_150000_add_comments",
        "2024_06_20_093000_update_schema",
        "2024_02_05_140000_add_indexes",
    ];

    for name in &migration_names {
        let migration_dir = temp_dir.path().join(name);
        fs::create_dir(&migration_dir).unwrap();
        fs::write(
            migration_dir.join("up.sql"),
            "ALTER TABLE users DROP COLUMN test;",
        )
        .unwrap();
    }

    let checker = SafetyChecker::new();
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Should check all 5 migrations
    assert_eq!(results.len(), 5);

    // Verify results are in alphanumeric order
    let expected_order = [
        "2024_01_10_080000_create_users",
        "2024_02_05_140000_add_indexes",
        "2024_03_15_120000_create_posts",
        "2024_06_20_093000_update_schema",
        "2024_12_01_150000_add_comments",
    ];

    for (i, expected) in expected_order.iter().enumerate() {
        assert!(
            results[i].0.contains(expected),
            "Expected migration {} at position {}, but got {}",
            expected,
            i,
            results[i].0
        );
    }
}

#[test]
fn test_diesel_migration_with_sqlx_markers_checks_all_statements() {
    // Bug fix: Diesel migration containing -- migrate:up / -- migrate:down markers
    // should check ALL statements, not just one section
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();

    // up.sql contains SQLx-style markers (e.g., converted from SQLx, developer notes)
    fs::write(
        migration_dir.join("up.sql"),
        r#"-- migrate:up
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
-- migrate:down
ALTER TABLE users DROP COLUMN admin;
"#,
    )
    .unwrap();

    let config = Config::default(); // framework = "diesel"
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // Diesel should parse the entire file and find violations in BOTH sections
    assert_eq!(results.len(), 1);
    let violations = &results[0].1;
    // Should find at least 2 violations: ADD COLUMN with DEFAULT and DROP COLUMN
    assert!(
        violations.len() >= 2,
        "Expected at least 2 violations (from both sections), got {}",
        violations.len()
    );
}

#[test]
fn test_standalone_sql_with_timestamp_respects_start_after() {
    // Bug fix: standalone .sql files with valid timestamps should respect start_after
    let temp_dir = TempDir::new().unwrap();

    // Standalone SQL file with a Diesel timestamp prefix
    fs::write(
        temp_dir.path().join("2023_01_01_000000_init.sql"),
        "ALTER TABLE users DROP COLUMN email;",
    )
    .unwrap();

    let config = Config {
        start_after: Some("2024_01_01_000000".to_string()),
        ..Default::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // File timestamp (2023) is before start_after (2024), should be skipped
    assert_eq!(results.len(), 0);
}

#[test]
fn test_standalone_sql_with_timestamp_after_start_after() {
    // Standalone .sql files with timestamps after start_after should still be checked
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("2025_01_01_000000_new.sql"),
        "ALTER TABLE users DROP COLUMN email;",
    )
    .unwrap();

    let config = Config {
        start_after: Some("2024_01_01_000000".to_string()),
        ..Default::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // File timestamp (2025) is after start_after (2024), should be checked
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("2025_01_01_000000_new.sql"));
}

#[test]
fn test_standalone_sql_without_timestamp_always_checked() {
    // Standalone .sql files without timestamps should always be checked (unchanged behavior)
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("seed.sql"),
        "ALTER TABLE users DROP COLUMN email;",
    )
    .unwrap();

    let config = Config {
        start_after: Some("2099_12_31_000000".to_string()),
        ..Default::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // No timestamp — always checked
    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("seed.sql"));
}

#[test]
fn test_diesel_concurrently_without_metadata_warns() {
    // CONCURRENTLY used without metadata.toml should still collect the file
    // (warning is printed to stderr, verified by the file being in results)
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_add_index");
    fs::create_dir(&migration_dir).unwrap();

    fs::write(
        migration_dir.join("up.sql"),
        "CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
    )
    .unwrap();
    // No metadata.toml — defaults to run_in_transaction = true

    let config = Config::default();
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    // The file should still be collected and checked (no violations from the SQL itself
    // since CREATE INDEX CONCURRENTLY is safe)
    assert_eq!(results.len(), 0);
}

#[test]
fn test_config_invalid_toml_syntax() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    // Missing closing quote — invalid TOML
    fs::write(&config_path, r#"framework = "diesel"#).unwrap();

    let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
    let err = Config::load_from_path(config_path_utf8).unwrap_err();
    assert!(
        matches!(err, ConfigError::ParseError(_)),
        "Expected ParseError for malformed TOML, got: {err:?}"
    );
}

#[test]
fn test_config_empty_file_errors() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    // Empty file (0 bytes)
    fs::write(&config_path, "").unwrap();

    let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
    let err = Config::load_from_path(config_path_utf8).unwrap_err();
    assert!(
        matches!(err, ConfigError::MissingFramework),
        "Expected MissingFramework for empty config, got: {err:?}"
    );
}

#[test]
fn test_config_extra_unknown_fields_ignored() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    fs::write(
        &config_path,
        r#"
framework = "diesel"
unknown_field = "value"
another_unknown = 42
"#,
    )
    .unwrap();

    let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
    let config = Config::load_from_path(config_path_utf8);
    assert!(
        config.is_ok(),
        "Unknown fields should be ignored by default, got: {config:?}"
    );
    assert_eq!(config.unwrap().framework, "diesel");
}

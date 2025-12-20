use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use tempfile::TempDir;

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

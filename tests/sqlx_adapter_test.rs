use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_sqlx_numeric_version_comparison() {
    let temp_dir = TempDir::new().unwrap();

    // Create 3 suffix-format migrations with numeric versions: 1, 2, 10
    for version in &["1", "2", "10"] {
        fs::write(
            temp_dir.path().join(format!("{version}_migration.up.sql")),
            "ALTER TABLE users DROP COLUMN old_col;",
        )
        .unwrap();
    }

    // start_after = "2" — with numeric comparison, only version 10 should be checked
    // (string comparison would wrongly exclude "10" since "10" < "2" lexicographically)
    let config = Config {
        framework: "sqlx".to_string(),
        start_after: Some("2".to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        1,
        "Only version 10 should be checked, got: {:?}",
        results.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
    assert!(results[0].0.contains("10_migration"));
}

#[test]
fn test_sqlx_single_file_format_no_markers() {
    let temp_dir = TempDir::new().unwrap();

    // Single file format: VERSION_DESC.sql (no .up/.down suffix, no markers)
    fs::write(
        temp_dir.path().join("20240101000000_create.sql"),
        "ALTER TABLE users DROP COLUMN old_col;",
    )
    .unwrap();

    let config = Config {
        framework: "sqlx".to_string(),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        1,
        "Single file format should be discovered and checked"
    );
}

#[test]
fn test_sqlx_start_after_with_suffix_format() {
    let temp_dir = TempDir::new().unwrap();

    // Old migration (should be skipped)
    fs::write(
        temp_dir.path().join("1_old.up.sql"),
        "ALTER TABLE users DROP COLUMN a;",
    )
    .unwrap();

    // New migration (should be checked)
    fs::write(
        temp_dir.path().join("42_new.up.sql"),
        "ALTER TABLE users DROP COLUMN b;",
    )
    .unwrap();

    let config = Config {
        framework: "sqlx".to_string(),
        start_after: Some("1".to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("42_new"));
}

#[test]
fn test_sqlx_check_down_marker_format() {
    let temp_dir = TempDir::new().unwrap();

    // Marker-based migration with unsafe SQL in both sections
    fs::write(
        temp_dir.path().join("1_test.sql"),
        r#"-- migrate:up
ALTER TABLE users DROP COLUMN up_col;

-- migrate:down
ALTER TABLE users DROP COLUMN down_col;
"#,
    )
    .unwrap();

    // check_down = true: violations from both sections
    let config_down = Config {
        framework: "sqlx".to_string(),
        check_down: true,
        ..Default::default()
    };
    let checker_down = SafetyChecker::with_config(config_down);
    let results_down = checker_down
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    let total_violations_down: usize = results_down.iter().map(|(_, v)| v.len()).sum();
    assert!(
        total_violations_down >= 2,
        "check_down=true should find violations from both sections, got {total_violations_down}"
    );

    // check_down = false: only up section violations
    let config_no_down = Config {
        framework: "sqlx".to_string(),
        check_down: false,
        ..Default::default()
    };
    let checker_no_down = SafetyChecker::with_config(config_no_down);
    let results_no_down = checker_no_down
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    let total_violations_no_down: usize = results_no_down.iter().map(|(_, v)| v.len()).sum();
    assert!(
        total_violations_no_down >= 1,
        "check_down=false should find violations from up section"
    );
    assert!(
        total_violations_no_down < total_violations_down,
        "check_down=false should find fewer violations than check_down=true"
    );
}

#[test]
fn test_sqlx_check_down_suffix_format() {
    let temp_dir = TempDir::new().unwrap();

    // Suffix format with both up and down files
    fs::write(
        temp_dir.path().join("1_test.up.sql"),
        "ALTER TABLE users DROP COLUMN up_col;",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("1_test.down.sql"),
        "ALTER TABLE users DROP COLUMN down_col;",
    )
    .unwrap();

    // check_down = false: only up violations
    let config_no_down = Config {
        framework: "sqlx".to_string(),
        check_down: false,
        ..Default::default()
    };
    let checker_no_down = SafetyChecker::with_config(config_no_down);
    let results_no_down = checker_no_down
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(results_no_down.len(), 1, "Only up file should be checked");
    assert!(results_no_down[0].0.contains(".up.sql"));

    // check_down = true: both up and down violations
    let config_down = Config {
        framework: "sqlx".to_string(),
        check_down: true,
        ..Default::default()
    };
    let checker_down = SafetyChecker::with_config(config_down);
    let results_down = checker_down
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results_down.len(),
        2,
        "Both up and down files should be checked"
    );
    let paths: Vec<&str> = results_down.iter().map(|(p, _)| p.as_str()).collect();
    assert!(paths.iter().any(|p| p.contains(".up.sql")));
    assert!(paths.iter().any(|p| p.contains(".down.sql")));
}

#[test]
fn test_sqlx_start_after_with_directory_format() {
    let temp_dir = TempDir::new().unwrap();

    // Old migration directory (should be skipped)
    let old_dir = temp_dir.path().join("1_old");
    fs::create_dir(&old_dir).unwrap();
    fs::write(old_dir.join("up.sql"), "ALTER TABLE users DROP COLUMN a;").unwrap();

    // New migration directory (should be checked)
    let new_dir = temp_dir.path().join("42_new");
    fs::create_dir(&new_dir).unwrap();
    fs::write(new_dir.join("up.sql"), "ALTER TABLE users DROP COLUMN b;").unwrap();

    let config = Config {
        framework: "sqlx".to_string(),
        start_after: Some("1".to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].0.contains("42_new"));
}

#[test]
fn test_sqlx_mixed_formats_in_same_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Suffix-based file
    fs::write(
        temp_dir.path().join("1_suffix.up.sql"),
        "ALTER TABLE users DROP COLUMN a;",
    )
    .unwrap();

    // Directory-based migration
    let dir_mig = temp_dir.path().join("2_directory");
    fs::create_dir(&dir_mig).unwrap();
    fs::write(dir_mig.join("up.sql"), "ALTER TABLE users DROP COLUMN b;").unwrap();

    let config = Config {
        framework: "sqlx".to_string(),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        2,
        "Both suffix and directory formats should be discovered, got: {:?}",
        results.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
}

#[test]
fn test_sqlx_check_file_marker_format_only_checks_up() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("1_test.sql");

    // Marker-based file with unsafe SQL in both sections
    fs::write(
        &file_path,
        r#"-- migrate:up
ALTER TABLE users DROP COLUMN up_col;

-- migrate:down
ALTER TABLE users DROP COLUMN down_col;
"#,
    )
    .unwrap();

    // Use check_file() directly — should only check the "up" section
    let config = Config {
        framework: "sqlx".to_string(),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let utf8_path = Utf8Path::from_path(&file_path).unwrap();
    let violations = checker.check_file(utf8_path).unwrap();

    // Should only have violations from the "up" section (DROP COLUMN up_col)
    assert!(
        !violations.is_empty(),
        "Should find violations from the up section"
    );
    // The "down" section DROP should NOT be included
    assert!(
        !violations
            .iter()
            .any(|v| v.problem.contains("down_col") || v.operation.contains("down_col")),
        "check_file should not include violations from the down section"
    );
}

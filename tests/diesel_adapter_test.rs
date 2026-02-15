use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_diesel_loose_sql_and_directory_migrations_coexist() {
    let temp_dir = TempDir::new().unwrap();

    // Directory-based migration
    let dir_mig = temp_dir.path().join("2024_01_01_000000_create_users");
    fs::create_dir(&dir_mig).unwrap();
    fs::write(dir_mig.join("up.sql"), "ALTER TABLE users DROP COLUMN a;").unwrap();

    // Loose SQL file with timestamp
    fs::write(
        temp_dir.path().join("2024_06_01_000000_add_indexes.sql"),
        "ALTER TABLE users DROP COLUMN b;",
    )
    .unwrap();

    let config = Config {
        framework: "diesel".to_string(),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        2,
        "Both directory and loose SQL migrations should be discovered, got: {:?}",
        results.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
}

#[test]
fn test_diesel_mixed_timestamp_separators() {
    let temp_dir = TempDir::new().unwrap();

    // Underscore separator
    let dir1 = temp_dir.path().join("2023_01_01_000000_old");
    fs::create_dir(&dir1).unwrap();
    fs::write(dir1.join("up.sql"), "ALTER TABLE users DROP COLUMN a;").unwrap();

    // Dash separator
    let dir2 = temp_dir.path().join("2024-06-01-000000_middle");
    fs::create_dir(&dir2).unwrap();
    fs::write(dir2.join("up.sql"), "ALTER TABLE users DROP COLUMN b;").unwrap();

    // No separator
    let dir3 = temp_dir.path().join("20250101000000_new");
    fs::create_dir(&dir3).unwrap();
    fs::write(dir3.join("up.sql"), "ALTER TABLE users DROP COLUMN c;").unwrap();

    // start_after = "2024_01_01_000000" should skip only the underscore dir
    let config = Config {
        framework: "diesel".to_string(),
        start_after: Some("2024_01_01_000000".to_string()),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        2,
        "Only dirs after start_after should be checked, got: {:?}",
        results.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );

    // Should include the dash-separator and no-separator dirs
    assert!(results.iter().any(|(p, _)| p.contains("2024-06-01")));
    assert!(results.iter().any(|(p, _)| p.contains("20250101")));

    // Should NOT include the old underscore dir
    assert!(!results.iter().any(|(p, _)| p.contains("2023_01_01")));
}

#[test]
fn test_diesel_no_separator_timestamp() {
    let temp_dir = TempDir::new().unwrap();

    // YYYYMMDDHHMMSS format (no separators)
    let dir = temp_dir.path().join("20240101000000_create_users");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("up.sql"), "ALTER TABLE users DROP COLUMN old_col;").unwrap();

    let config = Config {
        framework: "diesel".to_string(),
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        1,
        "YYYYMMDDHHMMSS timestamp should be discovered"
    );
    assert!(results[0].0.contains("20240101000000_create_users"));
}

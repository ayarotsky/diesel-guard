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
fn test_concurrently_violations_include_diesel_transaction_hint() {
    // No metadata.toml → run_in_transaction = true; all three "without CONCURRENTLY"
    // violations should carry the Diesel-specific hint in safe_alternative.
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_indexes");
    fs::create_dir(&migration_dir).unwrap();
    fs::write(
        migration_dir.join("up.sql"),
        "CREATE INDEX idx_a ON users(email);\nDROP INDEX idx_b;\nREINDEX INDEX idx_a;",
    )
    .unwrap();

    let config = Config {
        framework: "diesel".to_string(),
        ..Default::default()
    };
    let results = SafetyChecker::with_config(config)
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(results.len(), 1);
    let violations = &results[0].1;
    assert_eq!(violations.len(), 5);

    let add_index_without_concurrently = violations
        .iter()
        .find(|(_, violation)| violation.operation == "ADD INDEX without CONCURRENTLY")
        .expect("missing ADD INDEX without CONCURRENTLY violation");
    assert_eq!(
        add_index_without_concurrently.1.safe_alternative,
        "Use CONCURRENTLY to build the index without blocking writes:\n   CREATE INDEX CONCURRENTLY idx_a ON users;\n\nNote: CONCURRENTLY takes longer and uses more resources, but allows concurrent INSERT, UPDATE, and DELETE operations. The index build may fail if there are deadlocks or unique constraint violations.\n\nConsiderations:\n- Requires more total work and takes longer to complete\n- If it fails, it leaves behind an \"invalid\" index that should be dropped\n\nNote: CONCURRENTLY cannot run inside a transaction block.\nCreate `metadata.toml` in the migration directory with `run_in_transaction = false`."
    );

    let drop_index_without_concurrently = violations
        .iter()
        .find(|(_, violation)| violation.operation == "DROP INDEX without CONCURRENTLY")
        .expect("missing DROP INDEX without CONCURRENTLY violation");
    assert_eq!(
        drop_index_without_concurrently.1.safe_alternative,
        "Use CONCURRENTLY to drop the index without blocking queries:\n   DROP INDEX CONCURRENTLY idx_b;\n\nNote: CONCURRENTLY requires Postgres 9.2+.\n\nConsiderations:\n- Takes longer to complete than regular DROP INDEX\n- Allows concurrent SELECT, INSERT, UPDATE, DELETE operations\n- If it fails, the index may be marked \"invalid\" and should be dropped again\n- Cannot be rolled back (no transaction support)\n\nNote: CONCURRENTLY cannot run inside a transaction block.\nCreate `metadata.toml` in the migration directory with `run_in_transaction = false`."
    );

    let reindex_without_concurrently = violations
        .iter()
        .find(|(_, violation)| violation.operation == "REINDEX without CONCURRENTLY")
        .expect("missing REINDEX without CONCURRENTLY violation");
    assert_eq!(
        reindex_without_concurrently.1.safe_alternative,
        "Use REINDEX CONCURRENTLY for lock-free reindexing (Postgres 12+):\n\n   REINDEX INDEX CONCURRENTLY idx_a;\n\nNote: CONCURRENTLY requires Postgres 12+.\n\nConsiderations:\n- Takes longer to complete than regular REINDEX\n- Allows concurrent read/write operations\n- If it fails, the index may be left in \"invalid\" state and need manual cleanup\n- Cannot be rolled back (no transaction support)\n\nNote: CONCURRENTLY cannot run inside a transaction block.\nCreate `metadata.toml` in the migration directory with `run_in_transaction = false`."
    );
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

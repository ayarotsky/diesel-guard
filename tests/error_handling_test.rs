use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_invalid_sql_produces_parse_error() {
    let checker = SafetyChecker::new();
    let result = checker.check_sql("NOT VALID SQL;");
    assert!(result.is_err(), "Invalid SQL should produce an error");

    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.to_lowercase().contains("parse"),
        "Error should mention 'parse', got: {err_msg}"
    );
}

#[test]
fn test_invalid_sql_in_migration_file_has_file_context() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("up.sql");
    fs::write(&file_path, "NOT VALID SQL;").unwrap();

    let checker = SafetyChecker::new();
    let utf8_path = Utf8Path::from_path(&file_path).unwrap();
    let result = checker.check_file(utf8_path);

    assert!(
        result.is_err(),
        "Invalid SQL in file should produce an error"
    );

    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains(utf8_path.as_str()),
        "Error should contain the file path '{}', got: {err_msg}",
        utf8_path
    );
}

#[test]
fn test_empty_migration_file() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("up.sql");
    fs::write(&file_path, "").unwrap();

    let checker = SafetyChecker::new();
    let utf8_path = Utf8Path::from_path(&file_path).unwrap();
    let result = checker.check_file(utf8_path);

    assert!(result.is_ok(), "Empty file should parse successfully");
    assert_eq!(
        result.unwrap().len(),
        0,
        "Empty file should have 0 violations"
    );
}

#[test]
fn test_migration_file_with_only_comments() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("up.sql");
    fs::write(&file_path, "-- comment\n-- another comment\n").unwrap();

    let checker = SafetyChecker::new();
    let utf8_path = Utf8Path::from_path(&file_path).unwrap();
    let result = checker.check_file(utf8_path);

    assert!(
        result.is_ok(),
        "Comments-only file should parse successfully"
    );
    assert_eq!(
        result.unwrap().len(),
        0,
        "Comments-only file should have 0 violations"
    );
}

#[test]
fn test_nonexistent_file_returns_error() {
    let checker = SafetyChecker::new();
    let result = checker.check_file(Utf8Path::new("/nonexistent/path/to/migration.sql"));
    assert!(result.is_err(), "Nonexistent file should return an error");
}

#[test]
fn test_nonexistent_path_returns_error() {
    let checker = SafetyChecker::new();
    let result = checker.check_path(Utf8Path::new("/nonexistent/path/to/migrations"));
    assert!(result.is_err(), "Nonexistent path should return an error");
}

#[test]
fn test_check_directory_fails_on_invalid_sql_file() {
    let temp_dir = TempDir::new().unwrap();

    // First migration: valid SQL
    let valid_dir = temp_dir.path().join("2024_01_01_000000_valid");
    fs::create_dir(&valid_dir).unwrap();
    fs::write(
        valid_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN email VARCHAR(255);",
    )
    .unwrap();

    // Second migration: invalid SQL
    let invalid_dir = temp_dir.path().join("2024_06_01_000000_invalid");
    fs::create_dir(&invalid_dir).unwrap();
    fs::write(invalid_dir.join("up.sql"), "NOT VALID SQL SYNTAX HERE;").unwrap();

    let checker = SafetyChecker::new();
    let result = checker.check_directory(Utf8Path::from_path(temp_dir.path()).unwrap());

    // One bad file should fail the entire batch
    assert!(
        result.is_err(),
        "Directory with one invalid SQL file should fail the entire check"
    );
}

#[test]
fn test_empty_directory_returns_no_results() {
    let temp_dir = TempDir::new().unwrap();

    let checker = SafetyChecker::new();
    let result = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert!(
        result.is_empty(),
        "Empty directory should return no results"
    );
}

#[test]
fn test_directory_with_non_sql_files_ignored() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("readme.txt"), "Hello").unwrap();
    fs::write(temp_dir.path().join("notes.md"), "# Notes").unwrap();

    let config = Config::default();
    let checker = SafetyChecker::with_config(config);
    let result = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert!(
        result.is_empty(),
        "Directory with only non-SQL files should return no results"
    );
}

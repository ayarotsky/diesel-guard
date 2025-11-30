//! Integration tests for test fixtures.
//!
//! These tests verify that our fixture files behave as expected:
//! - Safe fixtures should produce no violations
//! - Unsafe fixtures should produce the expected violations

use diesel_guard::SafetyChecker;
use std::path::Path;

/// Helper to get fixture path
fn fixture_path(name: &str) -> String {
    format!("tests/fixtures/{}/up.sql", name)
}

#[test]
fn test_safe_fixtures_pass() {
    let checker = SafetyChecker::new();
    let safe_fixtures = vec!["add_column_safe", "add_index_with_concurrently"];

    for fixture in safe_fixtures {
        let path = fixture_path(fixture);
        let violations = checker
            .check_file(Path::new(&path))
            .unwrap_or_else(|e| panic!("Failed to check {}: {}", fixture, e));

        assert_eq!(
            violations.len(),
            0,
            "Expected {} to be safe but found {} violation(s)",
            fixture,
            violations.len()
        );
    }
}

#[test]
fn test_add_column_with_default_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_column_with_default");

    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with DEFAULT");
}

#[test]
fn test_add_index_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_index_without_concurrently");

    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
}

#[test]
fn test_add_unique_index_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_unique_index_without_concurrently");

    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
    assert!(
        violations[0].problem.contains("UNIQUE"),
        "Expected problem to mention UNIQUE"
    );
}

#[test]
fn test_drop_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_column");

    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_drop_column_if_exists_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_column_if_exists");

    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_drop_multiple_columns_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_multiple_columns");

    let violations = checker.check_file(Path::new(&path)).unwrap();

    assert_eq!(
        violations.len(),
        2,
        "Expected 2 violations (one per column)"
    );
    assert_eq!(violations[0].operation, "DROP COLUMN");
    assert_eq!(violations[1].operation, "DROP COLUMN");
}

#[test]
fn test_check_entire_fixtures_directory() {
    let checker = SafetyChecker::new();
    let results = checker
        .check_directory(Path::new("tests/fixtures"))
        .unwrap();

    // We have 8 fixtures total:
    // - 2 safe: add_column_safe, add_index_with_concurrently
    // - 6 unsafe: add_column_with_default, add_index_without_concurrently,
    //             add_unique_index_without_concurrently, drop_column,
    //             drop_column_if_exists, drop_multiple_columns
    //
    // Total violations: 7 (5 files with 1 violation + drop_multiple_columns with 2)

    let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();

    assert_eq!(
        results.len(),
        6,
        "Expected violations in 6 files, got {}",
        results.len()
    );

    assert_eq!(
        total_violations, 7,
        "Expected 7 total violations (drop_multiple_columns has 2), got {}",
        total_violations
    );
}

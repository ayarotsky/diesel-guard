//! Integration tests for test fixtures.
//!
//! These tests verify that our fixture files behave as expected:
//! - Safe fixtures should produce no violations
//! - Unsafe fixtures should produce the expected violations

use camino::Utf8Path;
use diesel_guard::SafetyChecker;

/// Helper to get fixture path
fn fixture_path(name: &str) -> String {
    format!("tests/fixtures/{}/up.sql", name)
}

#[test]
fn test_safe_fixtures_pass() {
    let checker = SafetyChecker::new();
    let safe_fixtures = vec![
        "add_column_safe",
        "add_index_with_concurrently",
        "drop_not_null",
        "safety_assured_drop",
        "safety_assured_multiple",
        "unnamed_constraint_safe",
    ];

    for fixture in safe_fixtures {
        let path = fixture_path(fixture);
        let violations = checker
            .check_file(Utf8Path::new(&path))
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

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with DEFAULT");
}

#[test]
fn test_add_not_null_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_not_null");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD NOT NULL constraint");
}

#[test]
fn test_add_index_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_index_without_concurrently");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
}

#[test]
fn test_add_unique_index_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_unique_index_without_concurrently");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
    assert!(
        violations[0].problem.contains("UNIQUE"),
        "Expected problem to mention UNIQUE"
    );
}

#[test]
fn test_alter_column_type_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("alter_column_type");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ALTER COLUMN TYPE");
}

#[test]
fn test_alter_column_type_with_using_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("alter_column_type_with_using");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ALTER COLUMN TYPE");
    assert!(
        violations[0].problem.contains("USING clause"),
        "Expected problem to mention USING clause"
    );
}

#[test]
fn test_create_extension_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("create_extension_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "CREATE EXTENSION");
}

#[test]
fn test_unnamed_constraint_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("unnamed_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 3, "Expected 3 violations");
    assert_eq!(violations[0].operation, "Unnamed constraint");
    assert_eq!(violations[1].operation, "Unnamed constraint");
    assert_eq!(violations[2].operation, "Unnamed constraint");
}

#[test]
fn test_drop_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_column");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_drop_column_if_exists_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_column_if_exists");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_drop_multiple_columns_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_multiple_columns");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

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
        .check_directory(Utf8Path::new("tests/fixtures"))
        .unwrap();

    let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();

    assert_eq!(
        results.len(),
        11,
        "Expected violations in 11 files, got {}",
        results.len()
    );

    assert_eq!(
        total_violations, 14,
        "Expected 14 total violations (drop_multiple_columns has 2, unnamed_constraint_unsafe has 3), got {}",
        total_violations
    );
}

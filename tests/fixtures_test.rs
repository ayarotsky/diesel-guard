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
        "add_index_safe",
        "add_json_column_safe",
        "add_primary_key_safe",
        "add_serial_column_safe",
        "add_unique_constraint_safe",
        "char_type_safe",
        "drop_index_safe",
        "drop_not_null_safe",
        "generated_column_safe",
        "reindex_safe",
        "safety_assured_drop",
        "safety_assured_multiple",
        "short_int_pk_safe",
        "timestamp_type_safe",
        "unnamed_constraint_safe",
        "wide_index_safe",
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
    let path = fixture_path("add_column_with_default_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with DEFAULT");
}

#[test]
fn test_add_not_null_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_not_null_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD NOT NULL constraint");
}

#[test]
fn test_add_index_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD INDEX without CONCURRENTLY");
}

#[test]
fn test_add_json_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_json_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with JSON type");
}

#[test]
fn test_add_unique_index_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_unique_index_unsafe");

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
    let path = fixture_path("alter_column_type_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ALTER COLUMN TYPE");
}

#[test]
fn test_alter_column_type_with_using_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("alter_column_type_using_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ALTER COLUMN TYPE");
    assert!(
        violations[0].problem.contains("USING clause"),
        "Expected problem to mention USING clause"
    );
}

#[test]
fn test_char_type_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("char_type_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with CHAR type");
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
fn test_add_unique_constraint_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_unique_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD UNIQUE constraint");
}

#[test]
fn test_unique_using_index_is_safe() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_unique_constraint_safe");

    // Should parse successfully (even though sqlparser can't parse it)
    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Should have NO violations (UNIQUE USING INDEX is the safe way)
    assert_eq!(
        violations.len(),
        0,
        "UNIQUE USING INDEX should not be flagged as unsafe"
    );
}

#[test]
fn test_unique_using_index_skips_other_statements() {
    let checker = SafetyChecker::new();
    let path = fixture_path("unique_using_index_parser_limitation");

    // This file contains both UNIQUE USING INDEX (safe) and DROP COLUMN (unsafe)
    // Due to parser limitation, ALL statements are skipped when UNIQUE USING INDEX
    // causes a parse error, so even the unsafe DROP COLUMN is not detected
    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // LIMITATION: Should be 1 violation (DROP COLUMN) but is 0 because parser fails
    // This test documents the known limitation
    assert_eq!(
        violations.len(),
        0,
        "Parser limitation: UNIQUE USING INDEX causes ALL statements to be skipped, \
         including the unsafe DROP COLUMN in this file"
    );
}

#[test]
fn test_unnamed_constraint_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("unnamed_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Note: Unnamed UNIQUE is caught by both AddUniqueConstraintCheck and UnnamedConstraintCheck
    assert_eq!(violations.len(), 4, "Expected 4 violations");
    assert_eq!(violations[0].operation, "ADD UNIQUE constraint");
    assert_eq!(violations[1].operation, "Unnamed constraint");
    assert_eq!(violations[2].operation, "Unnamed constraint");
    assert_eq!(violations[3].operation, "Unnamed constraint");
}

#[test]
fn test_drop_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_drop_column_if_exists_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_column_if_exists_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_drop_multiple_columns_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_multiple_columns_unsafe");

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
fn test_drop_index_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP INDEX without CONCURRENTLY");
}

#[test]
fn test_drop_table_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_table_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP TABLE");
}

#[test]
fn test_drop_database_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_database_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP DATABASE");
}

#[test]
fn test_drop_index_concurrently_is_safe() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_index_safe");

    // Should parse successfully (even though sqlparser can't parse it)
    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Should have NO violations (DROP INDEX CONCURRENTLY is the safe way)
    assert_eq!(
        violations.len(),
        0,
        "DROP INDEX CONCURRENTLY should not be flagged as unsafe"
    );
}

#[test]
fn test_generated_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("generated_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with GENERATED STORED");
}

#[test]
fn test_reindex_without_concurrently_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("reindex_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "REINDEX without CONCURRENTLY");
}

#[test]
fn test_reindex_concurrently_is_safe() {
    let checker = SafetyChecker::new();
    let path = fixture_path("reindex_safe");

    // Should parse successfully (even though sqlparser can't parse it)
    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Should have NO violations (REINDEX CONCURRENTLY is the safe way)
    assert_eq!(
        violations.len(),
        0,
        "REINDEX CONCURRENTLY should not be flagged as unsafe"
    );
}

#[test]
fn test_rename_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("rename_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "RENAME COLUMN");
}

#[test]
fn test_rename_table_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("rename_table_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "RENAME TABLE");
}

#[test]
fn test_add_serial_column_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_serial_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with SERIAL");
}

#[test]
fn test_timestamp_type_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("timestamp_type_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD COLUMN with TIMESTAMP");
}

#[test]
fn test_short_int_pk_unsafe_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("short_int_pk_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Expected 5 violations:
    // - 4 from ShortIntegerPrimaryKeyCheck (INT and SMALLINT PKs)
    // - 1 from AddPrimaryKeyCheck (ALTER TABLE ADD PRIMARY KEY with INT)
    assert_eq!(violations.len(), 5, "Expected 5 violations");

    // Check that we have violations from both checks
    let short_int_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.operation == "Short integer primary key")
        .collect();
    let add_pk_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.operation == "ADD PRIMARY KEY")
        .collect();

    assert_eq!(
        short_int_violations.len(),
        4,
        "Expected 4 short int PK violations"
    );
    assert_eq!(
        add_pk_violations.len(),
        1,
        "Expected 1 ADD PRIMARY KEY violation"
    );
}

#[test]
fn test_truncate_table_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("truncate_table_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "TRUNCATE TABLE");
}

#[test]
fn test_wide_index_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("wide_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "Wide index");
}

#[test]
fn test_add_primary_key_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("add_primary_key_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "ADD PRIMARY KEY");
}

#[test]
fn test_drop_primary_key_detected() {
    let checker = SafetyChecker::new();
    let path = fixture_path("drop_primary_key_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "DROP PRIMARY KEY");
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
        28,
        "Expected violations in 28 files, got {}",
        results.len()
    );

    assert_eq!(
        total_violations, 36,
        "Expected 36 total violations: 25 files with 1 each, drop_multiple_columns with 2, unnamed_constraint_unsafe with 4, short_int_pk_unsafe with 5 (4 short int + 1 add pk), got {}",
        total_violations
    );
}

// SQLx Integration Tests

#[test]
fn test_sqlx_suffix_format_detected() {
    use diesel_guard::Config;

    let config = Config {
        check_down: true, // Check both up and down
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_suffix_add_column_unsafe",
        ))
        .unwrap();

    // Should find violations in both .up.sql and .down.sql files
    assert_eq!(results.len(), 2, "Expected 2 files with violations");

    // Find the ADD COLUMN violation from up.sql
    let add_column_result = results
        .iter()
        .find(|(path, _)| path.contains(".up.sql"))
        .expect("Should find .up.sql file");
    assert_eq!(
        add_column_result.1.len(),
        1,
        "Expected 1 violation in up.sql"
    );
    assert_eq!(add_column_result.1[0].operation, "ADD COLUMN with DEFAULT");

    // Find the DROP COLUMN violation from down.sql
    let drop_column_result = results
        .iter()
        .find(|(path, _)| path.contains(".down.sql"))
        .expect("Should find .down.sql file");
    assert_eq!(
        drop_column_result.1.len(),
        1,
        "Expected 1 violation in down.sql"
    );
    assert_eq!(drop_column_result.1[0].operation, "DROP COLUMN");
}

#[test]
fn test_sqlx_marker_format_up_direction() {
    use diesel_guard::Config;

    let config = Config {
        check_down: false, // Only check up
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let results = checker
        .check_directory(Utf8Path::new("tests/fixtures_sqlx/sqlx_marker_drop_column"))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    let (_, violations) = &results[0];
    assert_eq!(violations.len(), 1, "Expected 1 violation in UP section");
    assert_eq!(violations[0].operation, "DROP COLUMN");
}

#[test]
fn test_sqlx_marker_format_down_direction() {
    use diesel_guard::Config;

    let config = Config {
        check_down: true, // Check both up and down
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    let results = checker
        .check_directory(Utf8Path::new("tests/fixtures_sqlx/sqlx_marker_drop_column"))
        .unwrap();

    // Should check both up and down sections from the marker-based file
    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    let (_, violations) = &results[0];
    // Up section has DROP COLUMN (1 violation)
    // Down section has ADD COLUMN without DEFAULT (0 violations)
    assert_eq!(
        violations.len(),
        1,
        "Expected 1 violation (DROP COLUMN in up section)"
    );
}

#[test]
fn test_safe_sqlx_fixtures_pass() {
    let checker = SafetyChecker::new();

    let safe_sqlx_fixtures = vec![
        "tests/fixtures_sqlx/sqlx_concurrently_missing_directive",
        "tests/fixtures_sqlx/sqlx_concurrently_with_directive",
        "tests/fixtures_sqlx/sqlx_reindex_safe",
    ];

    for fixture in safe_sqlx_fixtures {
        let results = checker
            .check_directory(Utf8Path::new(fixture))
            .unwrap_or_else(|e| panic!("Failed to check {}: {}", fixture, e));

        let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();
        assert_eq!(
            total_violations, 0,
            "Expected {} to be safe but found {} violation(s)",
            fixture, total_violations
        );
    }
}

#[test]
fn test_sqlx_directory_format() {
    let checker = SafetyChecker::new();

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_directory_rename_column",
        ))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    let (_, violations) = &results[0];
    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].operation, "RENAME COLUMN");
}

#[test]
fn test_check_all_sqlx_fixtures() {
    use diesel_guard::Config;

    let config = Config {
        framework: "sqlx".to_string(),
        check_down: false, // Only check up migrations
        ..Default::default()
    };
    let checker = SafetyChecker::with_config(config);

    // Check each fixture directory individually and collect results
    let fixture_dirs = vec![
        "tests/fixtures_sqlx/sqlx_suffix_add_column_unsafe",
        "tests/fixtures_sqlx/sqlx_marker_drop_column",
        "tests/fixtures_sqlx/sqlx_concurrently_missing_directive",
        "tests/fixtures_sqlx/sqlx_concurrently_with_directive",
        "tests/fixtures_sqlx/sqlx_directory_rename_column",
        "tests/fixtures_sqlx/sqlx_reindex_unsafe",
        "tests/fixtures_sqlx/sqlx_reindex_safe",
    ];

    let mut all_violations = 0;
    let mut files_with_violations = 0;

    for dir in fixture_dirs {
        let results = checker.check_directory(Utf8Path::new(dir)).unwrap();
        for (_, violations) in results {
            if !violations.is_empty() {
                files_with_violations += 1;
                all_violations += violations.len();
            }
        }
    }

    // Expected violations (with check_down = false):
    // 1. sqlx_suffix_add_column_unsafe/.up.sql - 1 violation (ADD COLUMN with DEFAULT)
    // 2. sqlx_marker_drop_column - 1 violation (DROP COLUMN in up section)
    // 3. sqlx_directory_rename_column/up.sql - 1 violation (RENAME COLUMN)
    // 4. sqlx_reindex_unsafe - 1 violation (REINDEX without CONCURRENTLY)
    // Note: .down.sql correctly skipped, CONCURRENTLY and safe fixtures have 0 violations
    assert_eq!(
        files_with_violations, 4,
        "Expected 4 files with violations, got {}",
        files_with_violations
    );
    assert_eq!(
        all_violations, 4,
        "Expected 4 total violations, got {}",
        all_violations
    );
}

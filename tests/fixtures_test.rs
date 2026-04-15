//! Integration tests for test fixtures.
//!
//! These tests verify that our fixture files behave as expected:
//! - Safe fixtures should produce no violations
//! - Unsafe fixtures should produce the expected violations

use camino::Utf8Path;
use diesel_guard::{Config, SafetyChecker};

const IDEMPOTENCY_CHECKS: &[&str] = &[
    "IdempotencyAlterCheck",
    "IdempotencyCreateCheck",
    "IdempotencyDropCheck",
    "IdempotencyIndexCheck",
];

/// Helper to get fixture path
fn fixture_path(name: &str) -> String {
    format!("tests/fixtures/{name}/up.sql")
}

fn checker_with_enabled_checks(checks: &[&str]) -> SafetyChecker {
    SafetyChecker::with_config(Config {
        enable_checks: checks.iter().map(|check| (*check).to_string()).collect(),
        ..Default::default()
    })
}

fn sqlx_checker_with_enabled_checks(checks: &[&str], check_down: bool) -> SafetyChecker {
    SafetyChecker::with_config(Config {
        framework: "sqlx".to_string(),
        check_down,
        enable_checks: checks.iter().map(|check| (*check).to_string()).collect(),
        ..Default::default()
    })
}

fn checker_with_disabled_checks(checks: &[&str]) -> SafetyChecker {
    SafetyChecker::with_config(Config {
        disable_checks: checks.iter().map(|check| (*check).to_string()).collect(),
        ..Default::default()
    })
}

fn sqlx_checker_with_disabled_checks(checks: &[&str], check_down: bool) -> SafetyChecker {
    SafetyChecker::with_config(Config {
        framework: "sqlx".to_string(),
        check_down,
        disable_checks: checks.iter().map(|check| (*check).to_string()).collect(),
        ..Default::default()
    })
}

#[test]
fn test_safe_fixtures_pass() {
    let safe_fixtures = vec![
        ("add_check_constraint_safe", vec!["AddCheckConstraintCheck"]),
        ("add_column_safe", vec!["AddColumnCheck"]),
        ("add_foreign_key_safe", vec!["AddForeignKeyCheck"]),
        ("add_identity_column_safe", vec!["AddIdentityColumnCheck"]),
        ("add_index_safe", vec!["AddIndexCheck"]),
        ("add_json_column_safe", vec!["AddJsonColumnCheck"]),
        ("add_primary_key_safe", vec!["AddPrimaryKeyCheck"]),
        ("add_serial_column_safe", vec!["AddSerialColumnCheck"]),
        (
            "add_unique_constraint_safe",
            vec!["AddUniqueConstraintCheck"],
        ),
        ("char_type_safe", vec!["CharTypeCheck"]),
        ("create_table_serial_safe", vec!["CreateTableSerialCheck"]),
        (
            "create_table_without_pk_safe",
            vec!["CreateTableWithoutPkCheck"],
        ),
        ("delete_with_where_safe", vec!["MutationWithoutWhereCheck"]),
        ("update_with_where_safe", vec!["MutationWithoutWhereCheck"]),
        (
            "domain_check_constraint_safe",
            vec!["AddDomainCheckConstraintCheck"],
        ),
        ("drop_index_safe", vec!["DropIndexCheck"]),
        ("drop_not_null_safe", vec!["DropNotNullCheck"]),
        ("generated_column_safe", vec!["GeneratedColumnCheck"]),
        ("idempotency_guard_safe", IDEMPOTENCY_CHECKS.to_vec()),
        ("refresh_matview_safe", vec!["RefreshMatViewCheck"]),
        ("reindex_safe", vec!["ReindexCheck"]),
        ("safety_assured_drop", vec!["DropColumnCheck"]),
        (
            "safety_assured_multiple",
            vec!["AddIndexCheck", "DropColumnCheck"],
        ),
        (
            "short_int_pk_safe",
            vec!["ShortIntegerPrimaryKeyCheck", "CreateTableWithoutPkCheck"],
        ),
        ("timestamp_type_safe", vec!["TimestampTypeCheck"]),
        ("unnamed_constraint_safe", vec!["UnnamedConstraintCheck"]),
        ("wide_index_safe", vec!["WideIndexCheck"]),
    ];

    for (fixture, checks) in safe_fixtures {
        let checker = checker_with_enabled_checks(&checks);
        let path = fixture_path(fixture);
        let violations = checker
            .check_file(Utf8Path::new(&path))
            .unwrap_or_else(|e| panic!("Failed to check {fixture}: {e}"));

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
    let checker = checker_with_enabled_checks(&["AddColumnCheck"]);
    let path = fixture_path("add_column_with_default_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD COLUMN with DEFAULT");
}

#[test]
fn test_add_identity_column_detected() {
    let checker = checker_with_enabled_checks(&["AddIdentityColumnCheck"]);
    let path = fixture_path("add_identity_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD COLUMN with IDENTITY");
}

#[test]
fn test_add_foreign_key_unsafe_detected() {
    let checker = checker_with_enabled_checks(&["AddForeignKeyCheck"]);
    let path = fixture_path("add_foreign_key_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD FOREIGN KEY");
}

#[test]
fn test_add_exclude_constraint_detected() {
    let checker = checker_with_enabled_checks(&["AddExcludeConstraintCheck"]);
    let path = fixture_path("add_exclude_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD EXCLUDE constraint");
}

#[test]
fn test_add_check_constraint_unsafe_detected() {
    let checker = checker_with_enabled_checks(&["AddCheckConstraintCheck"]);
    let path = fixture_path("add_check_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD CHECK CONSTRAINT");
}

#[test]
fn test_add_not_null_detected() {
    let checker = checker_with_enabled_checks(&["AddNotNullCheck"]);
    let path = fixture_path("add_not_null_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD NOT NULL constraint");
}

#[test]
fn test_add_index_without_concurrently_detected() {
    let checker = checker_with_enabled_checks(&["AddIndexCheck"]);
    let path = fixture_path("add_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD INDEX without CONCURRENTLY");
}

#[test]
fn test_add_json_column_detected() {
    let checker = checker_with_enabled_checks(&["AddJsonColumnCheck"]);
    let path = fixture_path("add_json_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD COLUMN with JSON type");
}

#[test]
fn test_add_unique_index_without_concurrently_detected() {
    let checker = checker_with_enabled_checks(&["AddIndexCheck"]);
    let path = fixture_path("add_unique_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD INDEX without CONCURRENTLY");
    assert!(
        violations[0].1.problem.contains("UNIQUE"),
        "Expected problem to mention UNIQUE"
    );
}

#[test]
fn test_alter_column_type_detected() {
    let checker = checker_with_enabled_checks(&["AlterColumnTypeCheck"]);
    let path = fixture_path("alter_column_type_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ALTER COLUMN TYPE");
}

#[test]
fn test_alter_column_type_with_using_detected() {
    let checker = checker_with_enabled_checks(&["AlterColumnTypeCheck"]);
    let path = fixture_path("alter_column_type_using_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ALTER COLUMN TYPE");
}

#[test]
fn test_char_type_detected() {
    let checker = checker_with_enabled_checks(&["CharTypeCheck"]);
    let path = fixture_path("char_type_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD COLUMN with CHAR type");
}

#[test]
fn test_create_table_without_pk_detected() {
    let checker = checker_with_enabled_checks(&["CreateTableWithoutPkCheck"]);
    let path = fixture_path("create_table_without_pk_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(
        violations[0].1.operation,
        "CREATE TABLE without PRIMARY KEY"
    );
}

#[test]
fn test_create_extension_detected() {
    let checker = checker_with_enabled_checks(&["CreateExtensionCheck"]);
    let path = fixture_path("create_extension_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "CREATE EXTENSION");
}

#[test]
fn test_create_table_serial_detected() {
    let checker = checker_with_enabled_checks(&["CreateTableSerialCheck"]);
    let path = fixture_path("create_table_serial_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "CREATE TABLE with SERIAL");
}

#[test]
fn test_add_unique_constraint_detected() {
    let checker = checker_with_enabled_checks(&["AddUniqueConstraintCheck"]);
    let path = fixture_path("add_unique_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD UNIQUE constraint");
}

#[test]
fn test_unique_using_index_is_safe() {
    let checker = checker_with_enabled_checks(&["AddUniqueConstraintCheck"]);
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
fn test_unnamed_constraint_detected() {
    let checker = checker_with_enabled_checks(&[
        "AddCheckConstraintCheck",
        "AddForeignKeyCheck",
        "AddUniqueConstraintCheck",
        "UnnamedConstraintCheck",
    ]);
    let path = fixture_path("unnamed_constraint_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Note: Unnamed UNIQUE is caught by both AddUniqueConstraintCheck and UnnamedConstraintCheck
    assert_eq!(violations.len(), 6, "Expected 6 violations");
    assert_eq!(violations[0].1.operation, "ADD UNIQUE constraint");
    assert_eq!(violations[1].1.operation, "CONSTRAINT without name");
    assert_eq!(violations[2].1.operation, "ADD CHECK CONSTRAINT");
    assert_eq!(violations[3].1.operation, "CONSTRAINT without name");
    assert_eq!(violations[4].1.operation, "ADD FOREIGN KEY");
    assert_eq!(violations[5].1.operation, "CONSTRAINT without name");
}

#[test]
fn test_drop_column_detected() {
    let checker = checker_with_enabled_checks(&["DropColumnCheck"]);
    let path = fixture_path("drop_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP COLUMN");
}

#[test]
fn test_drop_column_if_exists_detected() {
    let checker = checker_with_enabled_checks(&["DropColumnCheck"]);
    let path = fixture_path("drop_column_if_exists_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP COLUMN");
}

#[test]
fn test_drop_multiple_columns_detected() {
    let checker = checker_with_enabled_checks(&["DropColumnCheck"]);
    let path = fixture_path("drop_multiple_columns_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(
        violations.len(),
        2,
        "Expected 2 violations (one per column)"
    );
    assert_eq!(violations[0].1.operation, "DROP COLUMN");
    assert_eq!(violations[1].1.operation, "DROP COLUMN");
}

#[test]
fn test_drop_index_detected() {
    let checker = checker_with_enabled_checks(&["DropIndexCheck"]);
    let path = fixture_path("drop_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP INDEX without CONCURRENTLY");
}

#[test]
fn test_drop_not_null_detected() {
    let checker = checker_with_enabled_checks(&["DropNotNullCheck"]);
    let path = fixture_path("drop_not_null_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP NOT NULL");
}

#[test]
fn test_drop_table_detected() {
    let checker = checker_with_enabled_checks(&["DropTableCheck"]);
    let path = fixture_path("drop_table_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP TABLE");
}

#[test]
fn test_drop_database_detected() {
    let checker = checker_with_enabled_checks(&["DropDatabaseCheck"]);
    let path = fixture_path("drop_database_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP DATABASE");
}

#[test]
fn test_drop_index_concurrently_is_safe() {
    let checker = checker_with_enabled_checks(&["DropIndexCheck"]);
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
    let checker = checker_with_enabled_checks(&["GeneratedColumnCheck"]);
    let path = fixture_path("generated_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(
        violations[0].1.operation,
        "ADD COLUMN with GENERATED STORED"
    );
}

#[test]
fn test_refresh_matview_without_concurrently_detected() {
    let checker = checker_with_enabled_checks(&["RefreshMatViewCheck"]);
    let path = fixture_path("refresh_matview_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(
        violations[0].1.operation,
        "REFRESH MATERIALIZED VIEW without CONCURRENTLY"
    );
}

#[test]
fn test_reindex_without_concurrently_detected() {
    let checker = checker_with_enabled_checks(&["ReindexCheck"]);
    let path = fixture_path("reindex_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "REINDEX without CONCURRENTLY");
}

#[test]
fn test_reindex_concurrently_is_safe() {
    let checker = checker_with_enabled_checks(&["ReindexCheck"]);
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
    let checker = checker_with_enabled_checks(&["RenameColumnCheck"]);
    let path = fixture_path("rename_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "RENAME COLUMN");
}

#[test]
fn test_rename_schema_detected() {
    let checker = checker_with_enabled_checks(&["RenameSchemaCheck"]);
    let path = fixture_path("rename_schema_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "RENAME SCHEMA");
}

#[test]
fn test_rename_table_detected() {
    let checker = checker_with_enabled_checks(&["RenameTableCheck"]);
    let path = fixture_path("rename_table_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "RENAME TABLE");
}

#[test]
fn test_add_serial_column_detected() {
    let checker = checker_with_enabled_checks(&["AddSerialColumnCheck"]);
    let path = fixture_path("add_serial_column_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD COLUMN with SERIAL");
}

#[test]
fn test_timestamp_type_detected() {
    let checker = checker_with_enabled_checks(&["TimestampTypeCheck"]);
    let path = fixture_path("timestamp_type_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD COLUMN with TIMESTAMP");
}

#[test]
fn test_short_int_pk_unsafe_detected() {
    let checker = checker_with_enabled_checks(&[
        "AddPrimaryKeyCheck",
        "CreateTableWithoutPkCheck",
        "ShortIntegerPrimaryKeyCheck",
    ]);
    let path = fixture_path("short_int_pk_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    // Expected 6 violations:
    // - 4 from ShortIntegerPrimaryKeyCheck (INT and SMALLINT PKs)
    // - 1 from AddPrimaryKeyCheck (ALTER TABLE ADD PRIMARY KEY with INT)
    // - 1 from CreateTableWithoutPkCheck (products table defined without a PK, added later via ALTER)
    assert_eq!(violations.len(), 6, "Expected 6 violations");

    // Check that we have violations from each check
    let short_int_violations: Vec<_> = violations
        .iter()
        .filter(|(_, v)| v.operation == "PRIMARY KEY with short integer type")
        .collect();
    let add_pk_violations: Vec<_> = violations
        .iter()
        .filter(|(_, v)| v.operation == "ADD PRIMARY KEY")
        .collect();
    let no_pk_violations: Vec<_> = violations
        .iter()
        .filter(|(_, v)| v.operation == "CREATE TABLE without PRIMARY KEY")
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
    assert_eq!(
        no_pk_violations.len(),
        1,
        "Expected 1 CREATE TABLE without PRIMARY KEY violation"
    );
}

#[test]
fn test_truncate_table_detected() {
    let checker = checker_with_enabled_checks(&["TruncateTableCheck"]);
    let path = fixture_path("truncate_table_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "TRUNCATE TABLE");
}

#[test]
fn test_wide_index_detected() {
    let checker = checker_with_enabled_checks(&["WideIndexCheck"]);
    let path = fixture_path("wide_index_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(
        violations[0].1.operation,
        "CREATE INDEX with too many columns"
    );
}

#[test]
fn test_add_primary_key_detected() {
    let checker = checker_with_enabled_checks(&["AddPrimaryKeyCheck"]);
    let path = fixture_path("add_primary_key_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD PRIMARY KEY");
}

#[test]
fn test_drop_primary_key_detected() {
    let checker = checker_with_enabled_checks(&["DropPrimaryKeyCheck"]);
    let path = fixture_path("drop_primary_key_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DROP PRIMARY KEY");
}

#[test]
fn test_domain_check_constraint_alter_detected() {
    let checker = checker_with_enabled_checks(&["AddDomainCheckConstraintCheck"]);
    let path = fixture_path("domain_check_constraint_alter_unsafe");
    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();
    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "ADD CHECK CONSTRAINT TO DOMAIN");
}

#[test]
fn test_delete_without_where_detected() {
    let checker = checker_with_enabled_checks(&["MutationWithoutWhereCheck"]);
    let path = fixture_path("delete_without_where_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "DELETE without WHERE");
}

#[test]
fn test_update_without_where_detected() {
    let checker = checker_with_enabled_checks(&["MutationWithoutWhereCheck"]);
    let path = fixture_path("update_without_where_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 1, "Expected 1 violation");
    assert_eq!(violations[0].1.operation, "UPDATE without WHERE");
}

#[test]
fn test_check_entire_fixtures_directory() {
    let checker = checker_with_disabled_checks(IDEMPOTENCY_CHECKS);
    let results = checker
        .check_directory(Utf8Path::new("tests/fixtures"))
        .unwrap();

    let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();

    assert_eq!(
        results.len(),
        42,
        "Expected violations in 42 files, got {}",
        results.len()
    );

    assert_eq!(
        total_violations, 57,
        "Expected 57 total violations: 38 files with 1 each, drop_multiple_columns with 2, unnamed_constraint_unsafe with 6, short_int_pk_unsafe with 6 (4 short int + 1 add pk + 1 no pk), add_identity_column_unsafe with 1, and drop_column_if_exists_unsafe with 1, got {total_violations}"
    );
}

// SQLx Integration Tests

#[test]
fn test_sqlx_suffix_format_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["AddColumnCheck", "DropColumnCheck"], true);

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
        "Expected 1 AddColumnCheck violation in up.sql"
    );
    assert_eq!(
        add_column_result.1[0].1.operation,
        "ADD COLUMN with DEFAULT"
    );

    // Find the DROP COLUMN violation from down.sql
    let drop_column_result = results
        .iter()
        .find(|(path, _)| path.contains(".down.sql"))
        .expect("Should find .down.sql file");
    assert_eq!(
        drop_column_result.1.len(),
        1,
        "Expected 1 DropColumnCheck violation in down.sql"
    );
    assert_eq!(drop_column_result.1[0].1.operation, "DROP COLUMN");
}

#[test]
fn test_safe_sqlx_fixtures_pass() {
    let safe_sqlx_fixtures = vec![
        (
            "tests/fixtures_sqlx/sqlx_concurrently_with_directive",
            vec!["AddIndexCheck"],
        ),
        (
            "tests/fixtures_sqlx/sqlx_drop_index_safe",
            vec!["DropIndexCheck"],
        ),
        (
            "tests/fixtures_sqlx/sqlx_reindex_safe",
            vec!["ReindexCheck"],
        ),
        (
            "tests/fixtures_sqlx/sqlx_refresh_matview_safe",
            vec!["RefreshMatViewCheck"],
        ),
    ];

    for (fixture, checks) in safe_sqlx_fixtures {
        let checker = sqlx_checker_with_enabled_checks(&checks, false);
        let results = checker
            .check_directory(Utf8Path::new(fixture))
            .unwrap_or_else(|e| panic!("Failed to check {fixture}: {e}"));

        let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();
        assert_eq!(
            total_violations, 0,
            "Expected {fixture} to be safe but found {total_violations} violation(s)"
        );
    }
}

#[test]
fn test_sqlx_concurrently_without_no_transaction_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["AddIndexCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_concurrently_missing_directive",
        ))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "CREATE INDEX CONCURRENTLY inside a transaction"
    );
}

#[test]
fn test_sqlx_add_index_without_concurrently_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["AddIndexCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new("tests/fixtures_sqlx/sqlx_add_index_unsafe"))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "ADD INDEX without CONCURRENTLY"
    );
}

#[test]
fn test_sqlx_drop_index_without_concurrently_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["DropIndexCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new("tests/fixtures_sqlx/sqlx_drop_index_unsafe"))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "DROP INDEX without CONCURRENTLY"
    );
}

#[test]
fn test_sqlx_drop_index_concurrently_missing_directive_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["DropIndexCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_drop_index_missing_directive",
        ))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "DROP INDEX CONCURRENTLY inside a transaction"
    );
}

#[test]
fn test_sqlx_reindex_concurrently_missing_directive_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["ReindexCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_reindex_missing_directive",
        ))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "REINDEX CONCURRENTLY inside a transaction"
    );
}

#[test]
fn test_sqlx_refresh_matview_without_concurrently_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["RefreshMatViewCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_refresh_matview_unsafe",
        ))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "REFRESH MATERIALIZED VIEW without CONCURRENTLY"
    );
}

#[test]
fn test_sqlx_refresh_matview_concurrently_missing_directive_detected() {
    let checker = sqlx_checker_with_enabled_checks(&["RefreshMatViewCheck"], false);

    let results = checker
        .check_directory(Utf8Path::new(
            "tests/fixtures_sqlx/sqlx_refresh_matview_missing_directive",
        ))
        .unwrap();

    assert_eq!(results.len(), 1, "Expected 1 file with violations");
    assert_eq!(results[0].1.len(), 1, "Expected 1 violation");
    assert_eq!(
        results[0].1[0].1.operation,
        "REFRESH MATERIALIZED VIEW CONCURRENTLY inside a transaction"
    );
}

#[test]
fn test_check_all_sqlx_fixtures() {
    let checker = sqlx_checker_with_disabled_checks(IDEMPOTENCY_CHECKS, false);

    // Check each fixture directory individually and collect results
    let fixture_dirs = vec![
        "tests/fixtures_sqlx/sqlx_suffix_add_column_unsafe",
        "tests/fixtures_sqlx/sqlx_add_index_unsafe",
        "tests/fixtures_sqlx/sqlx_concurrently_missing_directive",
        "tests/fixtures_sqlx/sqlx_concurrently_with_directive",
        "tests/fixtures_sqlx/sqlx_drop_index_unsafe",
        "tests/fixtures_sqlx/sqlx_drop_index_safe",
        "tests/fixtures_sqlx/sqlx_drop_index_missing_directive",
        "tests/fixtures_sqlx/sqlx_reindex_unsafe",
        "tests/fixtures_sqlx/sqlx_reindex_safe",
        "tests/fixtures_sqlx/sqlx_reindex_missing_directive",
        "tests/fixtures_sqlx/sqlx_refresh_matview_unsafe",
        "tests/fixtures_sqlx/sqlx_refresh_matview_safe",
        "tests/fixtures_sqlx/sqlx_refresh_matview_missing_directive",
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

    // Expected violations (with check_down = false and idempotency checks excluded):
    // - sqlx_suffix_add_column_unsafe up.sql: 1
    // - sqlx_add_index_unsafe: 1
    // - sqlx_concurrently_missing_directive: 1
    // - sqlx_drop_index_unsafe: 1
    // - sqlx_drop_index_missing_directive: 1
    // - sqlx_reindex_unsafe: 1
    // - sqlx_reindex_missing_directive: 1
    // - sqlx_refresh_matview_unsafe: 1
    // - sqlx_refresh_matview_missing_directive: 1
    // Note: .down.sql is skipped here, and idempotency has dedicated fixture coverage.
    assert_eq!(
        files_with_violations, 9,
        "Expected 9 files with violations, got {files_with_violations}"
    );
    assert_eq!(
        all_violations, 9,
        "Expected 9 total violations, got {all_violations}"
    );
}

#[test]
fn test_idempotency_guard_detected() {
    let checker = checker_with_enabled_checks(IDEMPOTENCY_CHECKS);
    let path = fixture_path("idempotency_guard_unsafe");

    let violations = checker.check_file(Utf8Path::new(&path)).unwrap();

    assert_eq!(violations.len(), 6, "Expected 6 idempotency violations");
    assert_eq!(
        violations[0].1.operation,
        "CREATE TABLE without IF NOT EXISTS"
    );
    assert_eq!(
        violations[1].1.operation,
        "CREATE INDEX without IF NOT EXISTS"
    );
    assert_eq!(
        violations[2].1.operation,
        "ADD COLUMN without IF NOT EXISTS"
    );
    assert_eq!(violations[3].1.operation, "DROP TABLE without IF EXISTS");
    assert_eq!(violations[4].1.operation, "DROP INDEX without IF EXISTS");
    assert_eq!(violations[5].1.operation, "DROP COLUMN without IF EXISTS");
}

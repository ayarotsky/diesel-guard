use camino::Utf8Path;
use diesel_guard::SafetyChecker;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_safety_assured_block_ignores_violations() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "safety-assured block should ignore violations"
    );
}

#[test]
fn test_without_safety_assured_detects_violations() {
    let checker = SafetyChecker::new();
    let sql = r#"
ALTER TABLE users DROP COLUMN email;
ALTER TABLE posts DROP COLUMN body;
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        2,
        "should detect both DROP COLUMN violations"
    );
}

#[test]
fn test_partial_safety_assured() {
    let checker = SafetyChecker::new();
    let sql = r#"
ALTER TABLE users DROP COLUMN email;

-- safety-assured:start
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end

ALTER TABLE comments DROP COLUMN author;
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        2,
        "should detect violations outside block"
    );

    // Verify the operations detected are the ones outside the block
    assert!(
        violations
            .iter()
            .all(|v| v.operation.contains("DROP COLUMN"))
    );
}

#[test]
fn test_multiple_blocks() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end

ALTER TABLE posts ADD COLUMN body TEXT;

-- safety-assured:start
ALTER TABLE comments DROP COLUMN author;
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(violations.len(), 0, "both blocks should be ignored");
}

#[test]
fn test_case_insensitive() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- SAFETY-ASSURED:START
ALTER TABLE users DROP COLUMN email;
-- safety-assured:END
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "should handle case-insensitive directives"
    );
}

#[test]
fn test_unclosed_block_error() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
    "#;

    let result = checker.check_sql(sql);
    assert!(result.is_err(), "should error on unclosed block");
    assert!(result.unwrap_err().to_string().contains("Unclosed"));
}

#[test]
fn test_unmatched_end_error() {
    let checker = SafetyChecker::new();
    let sql = r#"
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
    "#;

    let result = checker.check_sql(sql);
    assert!(result.is_err(), "should error on unmatched end");
    assert!(result.unwrap_err().to_string().contains("Unmatched"));
}

#[test]
fn test_safety_assured_in_migration_file() {
    let temp_dir = TempDir::new().unwrap();
    let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
    fs::create_dir(&migration_dir).unwrap();

    fs::write(
        migration_dir.join("up.sql"),
        r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN deprecated_column;
-- safety-assured:end
        "#,
    )
    .unwrap();

    let checker = SafetyChecker::new();
    let results = checker
        .check_directory(Utf8Path::from_path(temp_dir.path()).unwrap())
        .unwrap();

    assert_eq!(
        results.len(),
        0,
        "migration with safety-assured should pass"
    );
}

#[test]
fn test_empty_block() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(violations.len(), 0, "empty block should not error");
}

#[test]
fn test_comments_within_block() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
-- This column was deprecated 6 months ago
-- All code references have been removed
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "comments within block should not interfere"
    );
}

#[test]
fn test_multiline_statement_in_block() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users
    DROP COLUMN email;
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "multiline statements should be ignored"
    );
}

#[test]
fn test_mixed_safe_and_unsafe_operations() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- Safe operation - no default
ALTER TABLE users ADD COLUMN email VARCHAR(255);

-- safety-assured:start
-- Unsafe but assured
ALTER TABLE users DROP COLUMN deprecated_field;
-- safety-assured:end

-- Another safe operation
CREATE INDEX CONCURRENTLY users_email_idx ON users(email);
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "only safe operations and assured blocks"
    );
}

#[test]
fn test_nested_blocks() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:start
ALTER TABLE posts DROP COLUMN body;
-- safety-assured:end
-- safety-assured:end
    "#;

    // Nested blocks should be rejected with a clear error
    let result = checker.check_sql(sql);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Nested 'safety-assured:start'")
    );
}

#[test]
fn test_block_with_multiple_statement_types() {
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
ALTER TABLE posts ADD COLUMN admin BOOLEAN DEFAULT FALSE;
CREATE INDEX users_idx ON users(name);
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "all operations in block should be ignored"
    );
}

#[test]
fn test_block_with_multiple_same_operation_type() {
    // Edge case: Multiple ALTER statements (same keyword) within block
    let checker = SafetyChecker::new();
    let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN deprecated_a;
ALTER TABLE users DROP COLUMN deprecated_b;
ALTER TABLE users DROP COLUMN deprecated_c;
-- safety-assured:end
    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(
        violations.len(),
        0,
        "all ALTER operations in block should be ignored"
    );
}

#[test]
fn test_interleaved_blocks_and_statements_same_keyword() {
    // Edge case: Same keyword appears in block, outside block, and in another block
    let checker = SafetyChecker::new();
    let sql = r#"
ALTER TABLE users DROP COLUMN a;

-- safety-assured:start
ALTER TABLE users DROP COLUMN b;
ALTER TABLE users DROP COLUMN c;
-- safety-assured:end

ALTER TABLE users DROP COLUMN d;

-- safety-assured:start
ALTER TABLE users DROP COLUMN e;
-- safety-assured:end

ALTER TABLE users DROP COLUMN f;
    "#;

    let violations = checker.check_sql(sql).unwrap();
    // Should detect only lines 2, 10, 16 (a, d, f) - not b, c, e which are in blocks
    assert_eq!(
        violations.len(),
        3,
        "should detect only violations outside blocks"
    );
    assert!(
        violations
            .iter()
            .all(|v| v.operation.contains("DROP COLUMN"))
    );
}

#[test]
fn test_safety_assured_with_leading_whitespace() {
    let checker = SafetyChecker::new();
    let sql = r#"

    -- safety-assured:start
        ALTER TABLE users DROP COLUMN email;
    -- safety-assured:end

    "#;

    let violations = checker.check_sql(sql).unwrap();
    assert_eq!(violations.len(), 0, "should handle leading whitespace");
}

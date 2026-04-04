use diesel_guard::Violation;
use diesel_guard::output::OutputFormatter;

#[test]
fn test_format_json_valid_structure() {
    let violations = vec![(
        2usize,
        Violation::new(
            "DROP TABLE",
            "Dropping a table is dangerous",
            "Use a soft-delete pattern instead",
        ),
    )];
    let results = vec![("migrations/001_init/up.sql".to_string(), violations)];

    let json_str = OutputFormatter::format_json(&results);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("format_json should return valid JSON");

    // Top-level is an array of file objects
    let arr = parsed.as_array().expect("top-level should be an array");
    assert_eq!(arr.len(), 1);

    let entry = &arr[0];
    assert_eq!(
        entry["file"].as_str().unwrap(),
        "migrations/001_init/up.sql"
    );

    let violations = entry["violations"]
        .as_array()
        .expect("violations should be an array");
    assert_eq!(violations.len(), 1);

    let f = &violations[0];
    assert_eq!(f["line"].as_u64().unwrap(), 2);
    assert!(
        f.get("operation").is_some(),
        "finding should have 'operation' key"
    );
    assert!(
        f.get("problem").is_some(),
        "finding should have 'problem' key"
    );
    assert!(
        f.get("safe_alternative").is_some(),
        "finding should have 'safe_alternative' key"
    );
    assert_eq!(f["operation"].as_str().unwrap(), "DROP TABLE");
}

#[test]
fn test_format_json_empty_results() {
    let json_str = OutputFormatter::format_json(&[]);
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .expect("format_json with empty input should return valid JSON");

    let arr = parsed.as_array().expect("should be an array");
    assert!(arr.is_empty());
}

#[test]
fn test_format_json_multiple_files() {
    let results = vec![
        (
            "migrations/001/up.sql".to_string(),
            vec![(1usize, Violation::new("DROP TABLE", "p1", "s1"))],
        ),
        (
            "migrations/002/up.sql".to_string(),
            vec![
                (3usize, Violation::new("DROP COLUMN", "p2", "s2")),
                (7usize, Violation::new("ADD INDEX", "p3", "s3")),
            ],
        ),
    ];

    let json_str = OutputFormatter::format_json(&results);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let arr = parsed.as_array().unwrap();

    assert_eq!(arr.len(), 2);

    assert_eq!(arr[0]["file"].as_str().unwrap(), "migrations/001/up.sql");
    assert_eq!(arr[0]["violations"].as_array().unwrap().len(), 1);

    assert_eq!(arr[1]["file"].as_str().unwrap(), "migrations/002/up.sql");
    assert_eq!(arr[1]["violations"].as_array().unwrap().len(), 2);
}

#[test]
fn test_format_text_contains_expected_sections() {
    // Strip ANSI codes for predictable assertions
    colored::control::set_override(false);

    let violations = vec![(
        5usize,
        Violation::new(
            "DROP TABLE",
            "Dropping a table is dangerous",
            "Use a soft-delete pattern instead",
        ),
    )];

    let output = OutputFormatter::format_text("migrations/001/up.sql", &violations);

    assert!(
        output.contains("migrations/001/up.sql"),
        "Output should contain the file path"
    );
    assert!(
        output.contains("DROP TABLE"),
        "Output should contain the operation"
    );
    assert!(
        output.contains("line 5"),
        "Output should contain the line number"
    );
    assert!(
        output.contains("Problem:"),
        "Output should contain 'Problem:' section"
    );
    assert!(
        output.contains("Safe alternative:"),
        "Output should contain 'Safe alternative:' section"
    );
}

#[test]
fn test_format_text_warning_severity() {
    use diesel_guard::violation::Severity;
    colored::control::set_override(false);

    let violations = vec![(
        3usize,
        Violation::new("SOME WARNING", "might be slow", "use X instead")
            .with_severity(Severity::Warning),
    )];

    let output = OutputFormatter::format_text("migrations/001/up.sql", &violations);

    assert!(output.contains("⚠️"), "Output should contain warning icon");
    assert!(
        output.contains("SOME WARNING"),
        "Output should contain the operation"
    );
    assert!(
        output.contains("line 3"),
        "Output should contain the line number"
    );
}

#[test]
fn test_format_text_empty_violations() {
    colored::control::set_override(false);

    let output = OutputFormatter::format_text("file.sql", &vec![]);

    // Header with file path should still be present
    assert!(
        output.contains("file.sql"),
        "Output should contain the file path even with no violations"
    );
    // No "Problem:" sections
    assert!(
        !output.contains("Problem:"),
        "Output should not contain 'Problem:' section when there are no violations"
    );
}

#[test]
fn test_format_github_errors() {
    let violations = vec![(
        3usize,
        Violation::new(
            "ADD COLUMN with DEFAULT",
            "Requires table rewrite",
            "Do it in steps",
        ),
    )];
    let output = OutputFormatter::format_github("migrations/001/up.sql", &violations);
    assert_eq!(
        output,
        "::error file=migrations/001/up.sql,line=3::ADD COLUMN with DEFAULT: Requires table rewrite\n"
    );
}

#[test]
fn test_format_github_warnings() {
    use diesel_guard::violation::Severity;
    let violations = vec![(
        7usize,
        Violation::new("SOME WARNING", "might be slow", "use X instead")
            .with_severity(Severity::Warning),
    )];
    let output = OutputFormatter::format_github("migrations/002/up.sql", &violations);
    assert_eq!(
        output,
        "::warning file=migrations/002/up.sql,line=7::SOME WARNING: might be slow\n"
    );
}

#[test]
fn test_format_github_file_path_with_comma() {
    let violations = vec![(
        1usize,
        Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("path/with,comma/up.sql", &violations);
    assert_eq!(
        output,
        "::error file=path/with%2Ccomma/up.sql,line=1::DROP TABLE: dangerous\n"
    );
}

#[test]
fn test_format_github_file_path_with_percent() {
    let violations = vec![(
        1usize,
        Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("path/50%_done/up.sql", &violations);
    assert_eq!(
        output,
        "::error file=path/50%25_done/up.sql,line=1::DROP TABLE: dangerous\n"
    );
}

#[test]
fn test_format_github_message_with_newline() {
    let violations = vec![(
        2usize,
        Violation::new("DROP TABLE", "line one\nline two", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("up.sql", &violations);
    assert_eq!(
        output,
        "::error file=up.sql,line=2::DROP TABLE: line one%0Aline two\n"
    );
}

#[test]
fn test_format_github_message_with_percent() {
    let violations = vec![(
        3usize,
        Violation::new("DROP TABLE", "100% data loss", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("up.sql", &violations);
    assert_eq!(
        output,
        "::error file=up.sql,line=3::DROP TABLE: 100%25 data loss\n"
    );
}

#[test]
fn test_format_github_file_path_with_newline() {
    let violations = vec![(
        1usize,
        Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("path/with\nnewline/up.sql", &violations);
    assert_eq!(
        output,
        "::error file=path/with%0Anewline/up.sql,line=1::DROP TABLE: dangerous\n"
    );
}

#[test]
fn test_format_github_file_path_with_carriage_return() {
    let violations = vec![(
        1usize,
        Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("path/with\rreturn/up.sql", &violations);
    assert_eq!(
        output,
        "::error file=path/with%0Dreturn/up.sql,line=1::DROP TABLE: dangerous\n"
    );
}

#[test]
fn test_format_github_file_path_with_colon() {
    let violations = vec![(
        1usize,
        Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("C:\\path:with/colon/up.sql", &violations);
    assert_eq!(
        output,
        "::error file=C%3A\\path%3Awith/colon/up.sql,line=1::DROP TABLE: dangerous\n"
    );
}

#[test]
fn test_format_github_message_with_carriage_return() {
    let violations = vec![(
        2usize,
        Violation::new("DROP TABLE", "line one\rline two", "use soft-delete"),
    )];
    let output = OutputFormatter::format_github("up.sql", &violations);
    assert_eq!(
        output,
        "::error file=up.sql,line=2::DROP TABLE: line one%0Dline two\n"
    );
}

#[test]
fn test_format_summary_no_violations() {
    colored::control::set_override(false);
    let output = OutputFormatter::format_summary(0, 0);
    assert_eq!(output, "✅ No unsafe migrations detected!");
}

#[test]
fn test_format_summary_with_errors() {
    colored::control::set_override(false);
    let output = OutputFormatter::format_summary(3, 0);
    assert_eq!(output, "\n❌ 3 unsafe migration(s) detected");
}

#[test]
fn test_format_summary_with_warnings_only() {
    colored::control::set_override(false);
    let output = OutputFormatter::format_summary(0, 2);
    assert_eq!(
        output,
        "
⚠️  2 migration warning(s) detected (not blocking)"
    );
}

#[test]
fn test_format_summary_with_errors_and_warnings() {
    colored::control::set_override(false);
    let output = OutputFormatter::format_summary(1, 2);
    assert_eq!(
        output,
        "
❌ 1 unsafe migration(s) and 2 warning(s) detected"
    );
}

#[test]
fn test_format_json_end_to_end() {
    use diesel_guard::SafetyChecker;

    let checker = SafetyChecker::new();
    // Line 1: safe (no violation). Line 2: DROP COLUMN — exactly one violation.
    let sql = "SELECT 1;\nALTER TABLE users DROP COLUMN email;";
    let violations = checker.check_sql(sql).unwrap();

    let results = vec![("migrations/001/up.sql".to_string(), violations)];
    let json_str = OutputFormatter::format_json(&results);

    // Parse both sides so the assertion is whitespace-insensitive but structure-exact.
    let actual: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let expected = serde_json::json!([{
        "file": "migrations/001/up.sql",
        "violations": [{
            "line": 2,
            "operation": "DROP COLUMN",
            "problem": "Dropping column 'email' from table 'users' requires an ACCESS EXCLUSIVE lock, blocking all operations. This typically triggers a table rewrite with duration depending on table size.",
            "safe_alternative": "1. Mark the column as unused in your application code first.\n\n2. Deploy the application without the column references.\n\n3. (Optional) Set column to NULL to reclaim space:\n   ALTER TABLE users ALTER COLUMN email DROP NOT NULL;\n   UPDATE users SET email = NULL;\n\n4. Drop the column in a later migration after confirming it's unused:\n   ALTER TABLE users DROP COLUMN email;\n\nNote: Postgres doesn't support DROP COLUMN CONCURRENTLY. The rewrite is unavoidable but staging the removal reduces risk.",
            "severity": "error"
        }]
    }]);
    assert_eq!(actual, expected);
}

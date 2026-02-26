use diesel_guard::Violation;
use diesel_guard::output::OutputFormatter;

#[test]
fn test_format_json_valid_structure() {
    let violations = vec![Violation::new(
        "DROP TABLE",
        "Dropping a table is dangerous",
        "Use a soft-delete pattern instead",
    )];
    let results = vec![("migrations/001_init/up.sql".to_string(), violations)];

    let json_str = OutputFormatter::format_json(&results);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("format_json should return valid JSON");

    // Top-level is an array
    let arr = parsed.as_array().expect("top-level should be an array");
    assert_eq!(arr.len(), 1);

    // Each entry is [filepath, [violations]]
    let entry = arr[0].as_array().expect("entry should be an array");
    assert_eq!(entry[0].as_str().unwrap(), "migrations/001_init/up.sql");

    let violations_arr = entry[1].as_array().expect("violations should be an array");
    assert_eq!(violations_arr.len(), 1);

    // Each violation has required keys
    let v = &violations_arr[0];
    assert!(
        v.get("operation").is_some(),
        "violation should have 'operation' key"
    );
    assert!(
        v.get("problem").is_some(),
        "violation should have 'problem' key"
    );
    assert!(
        v.get("safe_alternative").is_some(),
        "violation should have 'safe_alternative' key"
    );
    assert_eq!(v["operation"].as_str().unwrap(), "DROP TABLE");
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
            vec![Violation::new("DROP TABLE", "p1", "s1")],
        ),
        (
            "migrations/002/up.sql".to_string(),
            vec![
                Violation::new("DROP COLUMN", "p2", "s2"),
                Violation::new("ADD INDEX", "p3", "s3"),
            ],
        ),
    ];

    let json_str = OutputFormatter::format_json(&results);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let arr = parsed.as_array().unwrap();

    assert_eq!(arr.len(), 2);

    // First file: 1 violation
    let entry0 = arr[0].as_array().unwrap();
    assert_eq!(entry0[0].as_str().unwrap(), "migrations/001/up.sql");
    assert_eq!(entry0[1].as_array().unwrap().len(), 1);

    // Second file: 2 violations
    let entry1 = arr[1].as_array().unwrap();
    assert_eq!(entry1[0].as_str().unwrap(), "migrations/002/up.sql");
    assert_eq!(entry1[1].as_array().unwrap().len(), 2);
}

#[test]
fn test_format_text_contains_expected_sections() {
    // Strip ANSI codes for predictable assertions
    colored::control::set_override(false);

    let violations = vec![Violation::new(
        "DROP TABLE",
        "Dropping a table is dangerous",
        "Use a soft-delete pattern instead",
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
        output.contains("Problem:"),
        "Output should contain 'Problem:' section"
    );
    assert!(
        output.contains("Safe alternative:"),
        "Output should contain 'Safe alternative:' section"
    );
}

#[test]
fn test_format_text_empty_violations() {
    colored::control::set_override(false);

    let output = OutputFormatter::format_text("file.sql", &[]);

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

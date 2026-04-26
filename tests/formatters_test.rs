use diesel_guard::formatters::{Formatter, GithubFormatter, JsonFormatter, TextFormatter};
use diesel_guard::{Config, SafetyChecker, Violation};

fn only(check_name: &str) -> SafetyChecker {
    SafetyChecker::with_config(Config {
        enable_checks: vec![check_name.to_string()],
        ..Config::default()
    })
}

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

    let json_str = JsonFormatter.format_results(&results);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("format_results should return valid JSON");

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
        f.get("check_name").is_some(),
        "finding should have 'check_name' key"
    );
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
    let json_str = JsonFormatter.format_results(&[]);
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .expect("format_results with empty input should return valid JSON");

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

    let json_str = JsonFormatter.format_results(&results);
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
    colored::control::set_override(false);

    let violations = vec![(
        5usize,
        Violation::new(
            "DROP TABLE",
            "Dropping a table is dangerous",
            "Use a soft-delete pattern instead",
        ),
    )];
    let results = vec![("migrations/001/up.sql".to_string(), violations)];

    let output = TextFormatter.format_results(&results);

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
    let results = vec![("migrations/001/up.sql".to_string(), violations)];

    let output = TextFormatter.format_results(&results);

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

    let results = vec![("file.sql".to_string(), vec![])];
    let output = TextFormatter.format_results(&results);

    assert!(
        output.contains("file.sql"),
        "Output should contain the file path even with no violations"
    );
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
    let results = vec![("migrations/001/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("migrations/002/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("path/with,comma/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("path/50%_done/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("path/with\nnewline/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("path/with\rreturn/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("C:\\path:with/colon/up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
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
    let results = vec![("up.sql".to_string(), violations)];
    let output = GithubFormatter.format_results(&results);
    assert_eq!(
        output,
        "::error file=up.sql,line=2::DROP TABLE: line one%0Dline two\n"
    );
}

#[test]
fn test_format_summary_no_violations() {
    colored::control::set_override(false);
    let output = TextFormatter.format_results(&[]);
    assert!(output.contains("✅ No unsafe migrations detected!"));
}

#[test]
fn test_format_summary_with_errors() {
    colored::control::set_override(false);
    let violations: Vec<_> = (0..3)
        .map(|i| {
            (
                i,
                Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
            )
        })
        .collect();
    let results = vec![("file.sql".to_string(), violations)];
    let output = TextFormatter.format_results(&results);
    assert!(output.contains("3 unsafe migration(s) detected"));
}

#[test]
fn test_format_summary_with_warnings_only() {
    use diesel_guard::violation::Severity;
    colored::control::set_override(false);
    let violations: Vec<_> = (0..2)
        .map(|i| {
            (
                i,
                Violation::new("SOME WARNING", "might be slow", "use X instead")
                    .with_severity(Severity::Warning),
            )
        })
        .collect();
    let results = vec![("file.sql".to_string(), violations)];
    let output = TextFormatter.format_results(&results);
    assert!(output.contains("2 migration warning(s) detected (not blocking)"));
}

#[test]
fn test_format_summary_with_errors_and_warnings() {
    use diesel_guard::violation::Severity;
    colored::control::set_override(false);
    let violations = vec![
        (
            0usize,
            Violation::new("DROP TABLE", "dangerous", "use soft-delete"),
        ),
        (
            1usize,
            Violation::new("SOME WARNING", "might be slow", "use X instead")
                .with_severity(Severity::Warning),
        ),
        (
            2usize,
            Violation::new("SOME WARNING", "might be slow", "use X instead")
                .with_severity(Severity::Warning),
        ),
    ];
    let results = vec![("file.sql".to_string(), violations)];
    let output = TextFormatter.format_results(&results);
    assert!(output.contains("1 unsafe migration(s) and 2 warning(s) detected"));
}

#[test]
fn test_format_json_end_to_end() {
    let checker = SafetyChecker::with_config(Config {
        enable_checks: vec!["DropColumnCheck".to_string()],
        ..Default::default()
    });
    let sql = "SELECT 1;\nALTER TABLE users DROP COLUMN email;";
    let violations = checker.check_sql(sql).unwrap();

    let results = vec![("migrations/001/up.sql".to_string(), violations)];
    let json_str = JsonFormatter.format_results(&results);

    let actual: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let expected = serde_json::json!([{
        "file": "migrations/001/up.sql",
        "violations": [{
            "line": 2,
            "check_name": "DropColumnCheck",
            "operation": "DROP COLUMN",
            "problem": "Dropping column 'email' from table 'users' requires an ACCESS EXCLUSIVE lock, blocking all operations. This typically triggers a table rewrite with duration depending on table size.",
            "safe_alternative": "1. Mark the column as unused in your application code first.\n\n2. Deploy the application without the column references.\n\n3. (Optional) Set column to NULL to reclaim space:\n   ALTER TABLE users ALTER COLUMN email DROP NOT NULL;\n   UPDATE users SET email = NULL;\n\n4. Drop the column in a later migration after confirming it's unused:\n   ALTER TABLE users DROP COLUMN email;\n\nNote: Postgres doesn't support DROP COLUMN CONCURRENTLY. The rewrite is unavoidable but staging the removal reduces risk.",
            "severity": "error"
        }]
    }]);
    assert_eq!(actual, expected);
}

#[test]
fn test_format_checks_text() {
    colored::control::set_override(false);
    let checker = only("AddColumnCheck");
    let checks: Vec<_> = checker.registry().iter_checks().collect();

    let default_config = Config::default();
    let output = TextFormatter.format_checks(&checks, &default_config);
    assert!(output.contains("NAME"), "header should have NAME column");
    assert!(
        output.contains("SEVERITY"),
        "header should have SEVERITY column"
    );
    assert!(
        output.contains("ENABLED"),
        "header should have ENABLED column"
    );
    assert!(
        output.contains("AddColumnCheck"),
        "row should have check name"
    );
    assert!(output.contains("builtin"), "row should show builtin type");
    assert!(output.contains("error"), "default severity should be error");
    assert!(output.contains("yes"), "should be enabled by default");

    let disabled_config = Config {
        disable_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let output = TextFormatter.format_checks(&checks, &disabled_config);
    assert!(output.contains("no"), "should show disabled");

    let warn_config = Config {
        warn_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let output = TextFormatter.format_checks(&checks, &warn_config);
    assert!(output.contains("warning"), "should show warning severity");
}

#[test]
fn test_format_checks_json() {
    let checker = only("AddColumnCheck");
    let checks: Vec<_> = checker.registry().iter_checks().collect();

    let default_config = Config::default();
    let json_str = JsonFormatter.format_checks(&checks, &default_config);
    let arr: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let entry = arr
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "AddColumnCheck")
        .unwrap();
    assert_eq!(entry["type"].as_str().unwrap(), "builtin");
    assert_eq!(entry["severity"].as_str().unwrap(), "error");
    assert!(entry["enabled"].as_bool().unwrap());
    assert!(entry.get("script_path").is_none() || entry["script_path"].is_null());

    let disabled_config = Config {
        disable_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let json_str = JsonFormatter.format_checks(&checks, &disabled_config);
    let arr: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let entry = arr
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "AddColumnCheck")
        .unwrap();
    assert!(!entry["enabled"].as_bool().unwrap());

    let warn_config = Config {
        warn_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let json_str = JsonFormatter.format_checks(&checks, &warn_config);
    let arr: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let entry = arr
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "AddColumnCheck")
        .unwrap();
    assert_eq!(entry["severity"].as_str().unwrap(), "warning");
}

#[test]
fn test_format_explain_text() {
    colored::control::set_override(false);
    let checker = only("AddColumnCheck");
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "AddColumnCheck")
        .unwrap();

    let default_config = Config::default();
    let output = TextFormatter.format_explain(check, &default_config);
    assert!(
        output.starts_with("AddColumnCheck (builtin | error | enabled)"),
        "header format"
    );
    assert!(
        output.contains("ADD COLUMN"),
        "doc content should mention ADD COLUMN"
    );

    let disabled_config = Config {
        disable_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let output = TextFormatter.format_explain(check, &disabled_config);
    assert!(output.contains("disabled"), "should show disabled state");
}

#[test]
fn test_format_explain_json() {
    let checker = only("AddColumnCheck");
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "AddColumnCheck")
        .unwrap();

    let default_config = Config::default();
    let json_str = JsonFormatter.format_explain(check, &default_config);
    let obj: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(obj["name"].as_str().unwrap(), "AddColumnCheck");
    assert_eq!(obj["type"].as_str().unwrap(), "builtin");
    assert_eq!(obj["severity"].as_str().unwrap(), "error");
    assert!(obj["enabled"].as_bool().unwrap());
    let doc = obj["doc"]
        .as_str()
        .expect("doc should be a non-null string");
    assert!(doc.contains("ADD COLUMN"), "doc should describe the check");

    let disabled_config = Config {
        disable_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let json_str = JsonFormatter.format_explain(check, &disabled_config);
    let obj: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(!obj["enabled"].as_bool().unwrap());

    let warn_config = Config {
        warn_checks: vec!["AddColumnCheck".to_string()],
        ..Config::default()
    };
    let json_str = JsonFormatter.format_explain(check, &warn_config);
    let obj: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(obj["severity"].as_str().unwrap(), "warning");
}

#[test]
fn test_github_formatter_listing() {
    colored::control::set_override(false);
    let checker = only("AddColumnCheck");
    let checks: Vec<_> = checker.registry().iter_checks().collect();
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "AddColumnCheck")
        .unwrap();
    let config = Config::default();

    assert_eq!(
        GithubFormatter.format_checks(&checks, &config),
        TextFormatter.format_checks(&checks, &config),
        "GithubFormatter::format_checks should delegate to TextFormatter"
    );
    assert_eq!(
        GithubFormatter.format_explain(check, &config),
        TextFormatter.format_explain(check, &config),
        "GithubFormatter::format_explain should delegate to TextFormatter"
    );
}

// --- Custom check formatter paths ---

fn setup_custom_checks_dir(script_name: &str, script_content: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let custom_dir = dir.path().join("custom_checks");
    std::fs::create_dir(&custom_dir).unwrap();
    std::fs::write(custom_dir.join(script_name), script_content).unwrap();
    dir
}

const SCRIPT_WITH_DESCRIBE: &str = r#"
fn describe() { "Flags any ALTER TABLE statement" }
fn check(node, ctx) { () }
"#;

const SCRIPT_WITHOUT_DESCRIBE: &str = r"
fn check(node, ctx) { () }
";

fn checker_with_custom_checks(
    dir: &tempfile::TempDir,
    script_name: &str,
) -> (SafetyChecker, Config) {
    let custom_dir = dir.path().join("custom_checks");
    let config = Config {
        custom_checks_dir: Some(custom_dir.to_str().unwrap().to_string()),
        ..Config::default()
    };
    let checker = SafetyChecker::with_config(config.clone());
    // Verify the custom check was loaded
    let check_stem = script_name.trim_end_matches(".rhai");
    let loaded = checker
        .registry()
        .iter_checks()
        .any(|c| c.name() == check_stem);
    assert!(loaded, "Custom check '{check_stem}' should be loaded");
    (checker, config)
}

#[test]
fn test_json_format_checks_custom_check_includes_script_path() {
    let dir = setup_custom_checks_dir("my_check.rhai", SCRIPT_WITH_DESCRIBE);
    let (checker, config) = checker_with_custom_checks(&dir, "my_check.rhai");
    let checks: Vec<_> = checker.registry().iter_checks().collect();
    let json_str = JsonFormatter.format_checks(&checks, &config);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let arr = parsed.as_array().unwrap();
    let custom = arr
        .iter()
        .find(|e| e["name"] == "my_check")
        .expect("my_check should appear");
    assert_eq!(custom["type"], "custom");
    assert!(
        custom.get("script_path").is_some(),
        "custom check should have script_path"
    );
}

#[test]
fn test_text_format_checks_custom_check_shows_custom_type() {
    let dir = setup_custom_checks_dir("my_check.rhai", SCRIPT_WITH_DESCRIBE);
    let (checker, config) = checker_with_custom_checks(&dir, "my_check.rhai");
    let checks: Vec<_> = checker.registry().iter_checks().collect();
    colored::control::set_override(false);
    let output = TextFormatter.format_checks(&checks, &config);
    assert!(
        output.contains("custom"),
        "Text format_checks should show 'custom' type for custom check, got:\n{output}"
    );
}

#[test]
fn test_json_format_explain_custom_check_includes_script_path() {
    let dir = setup_custom_checks_dir("explain_check.rhai", SCRIPT_WITH_DESCRIBE);
    let (checker, config) = checker_with_custom_checks(&dir, "explain_check.rhai");
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "explain_check")
        .unwrap();
    let json_str = JsonFormatter.format_explain(check, &config);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed["type"], "custom");
    assert!(
        parsed.get("script_path").is_some(),
        "format_explain JSON should include script_path for custom checks"
    );
}

#[test]
fn test_json_format_explain_no_describe_has_null_doc() {
    let dir = setup_custom_checks_dir("nodoc_check.rhai", SCRIPT_WITHOUT_DESCRIBE);
    let (checker, config) = checker_with_custom_checks(&dir, "nodoc_check.rhai");
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "nodoc_check")
        .unwrap();
    let json_str = JsonFormatter.format_explain(check, &config);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(
        parsed["doc"].is_null(),
        "format_explain JSON doc should be null when no describe fn, got: {:?}",
        parsed["doc"]
    );
}

#[test]
fn test_text_format_explain_custom_check_shows_custom_type() {
    let dir = setup_custom_checks_dir("text_check.rhai", SCRIPT_WITH_DESCRIBE);
    let (checker, config) = checker_with_custom_checks(&dir, "text_check.rhai");
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "text_check")
        .unwrap();
    colored::control::set_override(false);
    let output = TextFormatter.format_explain(check, &config);
    assert!(
        output.contains("custom"),
        "Text format_explain should show 'custom' type, got:\n{output}"
    );
}

#[test]
fn test_text_format_explain_no_describe_shows_hint() {
    let dir = setup_custom_checks_dir("nodesc_check.rhai", SCRIPT_WITHOUT_DESCRIBE);
    let (checker, config) = checker_with_custom_checks(&dir, "nodesc_check.rhai");
    let check = checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == "nodesc_check")
        .unwrap();
    colored::control::set_override(false);
    let output = TextFormatter.format_explain(check, &config);
    assert!(
        output.contains("No description available"),
        "format_explain text should hint to add fn describe(), got:\n{output}"
    );
    assert!(
        output.contains("fn describe()"),
        "format_explain text should show the fn describe() hint, got:\n{output}"
    );
}

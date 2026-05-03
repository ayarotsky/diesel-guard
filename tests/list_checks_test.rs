use std::{fs, path::PathBuf, process::Command};

fn diesel_guard_bin() -> PathBuf {
    let status = Command::new("cargo")
        .args(["build", "--quiet"])
        .status()
        .expect("Failed to build diesel-guard");
    assert!(status.success(), "Failed to build diesel-guard");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("diesel-guard");
    path
}

// --- list-checks (text) ---

#[test]
fn test_list_checks_includes_enabled_column_header() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(&bin)
        .arg("list-checks")
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("ENABLED"),
        "Expected ENABLED column in header"
    );
}

#[test]
fn test_list_checks_shows_all_checks_as_enabled_by_default() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(&bin)
        .arg("list-checks")
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // All checks should show "yes" with default config
    assert!(
        !stdout.contains(" no"),
        "Expected all checks enabled by default"
    );
    assert!(
        stdout.contains(" yes"),
        "Expected at least one enabled check"
    );
}

#[test]
fn test_list_checks_shows_disabled_check_as_no() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(
        tempdir.path().join("diesel-guard.toml"),
        "framework = \"diesel\"\ndisable_checks = [\"AddColumnCheck\"]\n",
    )
    .unwrap();

    let output = Command::new(&bin)
        .arg("list-checks")
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    // AddColumnCheck must appear with "no"
    let line = stdout
        .lines()
        .find(|l| l.contains("AddColumnCheck"))
        .expect("AddColumnCheck not found in output");
    assert!(
        line.ends_with("no"),
        "Expected AddColumnCheck to show 'no', got: {line}"
    );
}

#[test]
fn test_list_checks_disabled_check_still_appears() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(
        tempdir.path().join("diesel-guard.toml"),
        "framework = \"diesel\"\ndisable_checks = [\"DropTableCheck\"]\n",
    )
    .unwrap();

    let output = Command::new(&bin)
        .arg("list-checks")
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("DropTableCheck"),
        "Disabled check must still appear in list"
    );
}

// --- list-checks (json) ---

#[test]
fn test_list_checks_json_includes_enabled_field() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(&bin)
        .args(["list-checks", "--format", "json"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert!(!arr.is_empty());
    for entry in arr {
        assert!(
            entry.get("enabled").is_some(),
            "Missing 'enabled' field in: {entry}"
        );
    }
}

#[test]
fn test_list_checks_json_disabled_check_has_enabled_false() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(
        tempdir.path().join("diesel-guard.toml"),
        "framework = \"diesel\"\ndisable_checks = [\"AddColumnCheck\"]\n",
    )
    .unwrap();

    let output = Command::new(&bin)
        .args(["list-checks", "--format", "json"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = json.as_array().unwrap();

    let entry = arr
        .iter()
        .find(|e| e["name"] == "AddColumnCheck")
        .expect("AddColumnCheck not found in JSON output");
    assert_eq!(
        entry["enabled"], false,
        "Expected enabled=false for disabled check"
    );
}

// --- explain (text) ---

#[test]
fn test_explain_shows_enabled_for_active_check() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(&bin)
        .args(["explain", "AddIndexCheck"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("| enabled)"),
        "Expected 'enabled' in header, got: {stdout}"
    );
}

#[test]
fn test_explain_shows_disabled_for_disabled_check() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(
        tempdir.path().join("diesel-guard.toml"),
        "framework = \"diesel\"\ndisable_checks = [\"AddIndexCheck\"]\n",
    )
    .unwrap();

    let output = Command::new(&bin)
        .args(["explain", "AddIndexCheck"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "explain for disabled check must not exit 1"
    );
    assert!(
        stdout.contains("| disabled)"),
        "Expected 'disabled' in header, got: {stdout}"
    );
}

#[test]
fn test_explain_disabled_check_exits_zero() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(
        tempdir.path().join("diesel-guard.toml"),
        "framework = \"diesel\"\ndisable_checks = [\"DropTableCheck\"]\n",
    )
    .unwrap();

    let output = Command::new(&bin)
        .args(["explain", "DropTableCheck"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "explain for disabled check must succeed, got stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_explain_unknown_check_exits_one() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(&bin)
        .args(["explain", "NonExistentCheck"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "explain for unknown check must exit 1"
    );
}

// --- explain (json) ---

#[test]
fn test_explain_json_includes_enabled_field() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(&bin)
        .args(["explain", "AddIndexCheck", "--format", "json"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["enabled"], true);
}

#[test]
fn test_explain_json_disabled_check_has_enabled_false() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(
        tempdir.path().join("diesel-guard.toml"),
        "framework = \"diesel\"\ndisable_checks = [\"AddIndexCheck\"]\n",
    )
    .unwrap();

    let output = Command::new(&bin)
        .args(["explain", "AddIndexCheck", "--format", "json"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["enabled"], false);
}

// --- severity reflects warn_checks ---

const WARN_CONFIG: &str = "framework = \"diesel\"\nwarn_checks = [\"TruncateTableCheck\"]\n";

#[test]
fn test_list_checks_text_warn_check_shows_warning() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(tempdir.path().join("diesel-guard.toml"), WARN_CONFIG).unwrap();

    let output = Command::new(&bin)
        .arg("list-checks")
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let line = stdout
        .lines()
        .find(|l| l.contains("TruncateTableCheck"))
        .expect("TruncateTableCheck not found");
    assert!(
        line.contains("warning"),
        "Expected 'warning' severity for warn_checks entry, got: {line}"
    );
}

#[test]
fn test_list_checks_json_warn_check_has_severity_warning() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(tempdir.path().join("diesel-guard.toml"), WARN_CONFIG).unwrap();

    let output = Command::new(&bin)
        .args(["list-checks", "--format", "json"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let entry = json
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "TruncateTableCheck")
        .expect("TruncateTableCheck not found");
    assert_eq!(entry["severity"], "warning");
}

#[test]
fn test_explain_text_warn_check_shows_warning() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(tempdir.path().join("diesel-guard.toml"), WARN_CONFIG).unwrap();

    let output = Command::new(&bin)
        .args(["explain", "TruncateTableCheck"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("| warning |"),
        "Expected '| warning |' in header, got: {stdout}"
    );
}

#[test]
fn test_explain_json_warn_check_has_severity_warning() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().unwrap();
    fs::write(tempdir.path().join("diesel-guard.toml"), WARN_CONFIG).unwrap();

    let output = Command::new(&bin)
        .args(["explain", "TruncateTableCheck", "--format", "json"])
        .current_dir(tempdir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["severity"], "warning");
}

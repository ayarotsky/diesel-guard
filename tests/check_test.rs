use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_default_migrations_dir() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let migrations_dir = temp_dir.path().join("migrations");
    fs::create_dir(&migrations_dir).expect("Failed to create migrations dir");
    fs::write(migrations_dir.join("up.sql"), "SELECT 1;").expect("Failed to write migration");

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .arg("check")
        .current_dir(temp_dir.path())
        .assert()
        .success()
        .stdout("✅ No unsafe migrations detected!\n");
}

#[test]
fn test_stdin_input_safe() {
    Command::cargo_bin("diesel-guard")
        .unwrap()
        .args(["check", "-"])
        .write_stdin("SELECT 1;")
        .assert()
        .success()
        .stdout("✅ No unsafe migrations detected!\n");
}

use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

/// Get the path to the diesel-guard binary
fn diesel_guard_bin() -> PathBuf {
    // Build the binary first to ensure it exists
    let status = Command::new("cargo")
        .args(["build", "--quiet"])
        .status()
        .expect("Failed to build diesel-guard");
    assert!(status.success(), "Failed to build diesel-guard");

    // Get the binary path
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("diesel-guard");
    path
}

#[test]
fn test_default_migrations_dir() {
    let bin = diesel_guard_bin();
    let tempdir = tempfile::tempdir().expect("Failed to create tempdir");
    let migrations_dir = tempdir.path().join("migrations");
    fs::create_dir(&migrations_dir).expect("Failed to create migrations dir");
    fs::write(
        migrations_dir.join("up.sql"),
        "ALTER TABLE users ADD COLUMN foo TEXT;",
    )
    .expect("Failed to write migration");

    let output = Command::new(&bin)
        .arg("check")
        .current_dir(tempdir.path())
        .output()
        .expect("Failed to execute check command");

    assert!(
        output.status.success(),
        "Check command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("✅ No unsafe migrations detected!\n"));
}

#[test]
fn test_stdin_input_safe() {
    // Create pipes for the command
    let command_input = Stdio::piped();
    let command_output = Stdio::piped();

    // Create test data for the command
    let test_data = "ALTER TABLE users ADD COLUMN foo TEXT;";

    // Run check command
    let mut handle = Command::new(diesel_guard_bin())
        .arg("check")
        .arg("-")
        .stdin(command_input)
        .stdout(command_output)
        .spawn()
        .expect("Failed to execute check command");

    let handle_stdin = handle.stdin.as_mut().unwrap();
    handle_stdin.write_all(test_data.as_bytes()).unwrap();

    let output = handle.wait_with_output().unwrap();

    // Verify command succeeded
    assert!(
        output.status.success(),
        "Check command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify output message
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("✅ No unsafe migrations detected!\n"));
}

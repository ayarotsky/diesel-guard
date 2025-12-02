use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

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
fn test_init_creates_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    // Run init command
    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute init command");

    // Verify command succeeded
    assert!(
        output.status.success(),
        "Init command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify file was created
    assert!(config_path.exists(), "Config file was not created");

    // Verify output message
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("✓ Created diesel-guard.toml"));
    assert!(stdout.contains("Next steps:"));
}

#[test]
fn test_init_content_matches_example() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    // Run init command
    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute init command");

    assert!(output.status.success());

    // Read created config
    let created_content = fs::read_to_string(&config_path).unwrap();

    // Read example config
    let example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diesel-guard.toml.example");
    let example_content = fs::read_to_string(example_path).unwrap();

    // Verify content matches exactly
    assert_eq!(
        created_content, example_content,
        "Created config does not match example"
    );
}

#[test]
fn test_init_fails_when_config_exists() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    // Create existing config file
    fs::write(&config_path, "# existing config").unwrap();

    // Run init command (should fail)
    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute init command");

    // Verify command failed
    assert!(
        !output.status.success(),
        "Init should fail when config exists"
    );

    // Verify error message
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
    assert!(stderr.contains("--force"));

    // Verify original file was not modified
    let content = fs::read_to_string(&config_path).unwrap();
    assert_eq!(content, "# existing config");
}

#[test]
fn test_init_force_overwrites_existing() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    // Create existing config file
    fs::write(&config_path, "# old config").unwrap();

    // Run init command with --force
    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--force"])
        .output()
        .expect("Failed to execute init command");

    // Verify command succeeded
    assert!(
        output.status.success(),
        "Init --force failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify output message indicates overwrite
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("✓ Overwrote diesel-guard.toml"));

    // Verify file was overwritten with template
    let created_content = fs::read_to_string(&config_path).unwrap();
    let example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diesel-guard.toml.example");
    let example_content = fs::read_to_string(example_path).unwrap();
    assert_eq!(created_content, example_content);
}

#[test]
fn test_init_in_empty_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Verify directory is empty
    let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 0, "Temp directory should be empty");

    // Run init
    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute init command");

    assert!(output.status.success());

    // Verify only config file was created
    let config_path = temp_dir.path().join("diesel-guard.toml");
    assert!(config_path.exists());
}

#[test]
fn test_init_preserves_other_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create some other files
    fs::write(temp_dir.path().join("README.md"), "test").unwrap();
    fs::create_dir(temp_dir.path().join("migrations")).unwrap();

    // Run init
    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute init command");

    assert!(output.status.success());

    // Verify other files still exist
    assert!(temp_dir.path().join("README.md").exists());
    assert!(temp_dir.path().join("migrations").exists());
    assert!(temp_dir.path().join("diesel-guard.toml").exists());
}

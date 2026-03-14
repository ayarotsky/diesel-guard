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

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .expect("Failed to execute init command");

    assert!(
        output.status.success(),
        "Init command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(config_path.exists(), "Config file was not created");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Auto-detected:"),
        "Expected auto-summary in output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("✓ Created diesel-guard.toml"),
        "Expected creation message, got:\n{stdout}"
    );
    assert!(
        stdout.contains("To add diesel-guard to CI:"),
        "Expected CI hint, got:\n{stdout}"
    );
}

#[test]
fn test_init_auto_minimal_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .expect("Failed to execute init command");

    assert!(output.status.success());

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("framework = \"diesel\""),
        "Expected framework = \"diesel\" in config, got:\n{}",
        content
    );
}

#[test]
fn test_init_fails_when_config_exists() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    fs::write(&config_path, "# existing config").unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .expect("Failed to execute init command");

    assert!(
        !output.status.success(),
        "Init should fail when config exists"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr,
        "Error: diesel-guard.toml already exists in current directory\n\
         Use --force to overwrite the existing file\n"
    );

    let content = fs::read_to_string(&config_path).unwrap();
    assert_eq!(content, "# existing config");
}

#[test]
fn test_init_force_overwrites_existing() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("diesel-guard.toml");

    fs::write(&config_path, "# old config").unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--force", "--auto"])
        .output()
        .expect("Failed to execute init command");

    assert!(
        output.status.success(),
        "Init --force failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Auto-detected:"),
        "Expected auto-summary in output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("✓ Overwrote diesel-guard.toml"),
        "Expected overwrite message, got:\n{stdout}"
    );

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("framework = \"diesel\""),
        "Expected framework in overwritten config, got:\n{}",
        content
    );
}

#[test]
fn test_init_in_empty_directory() {
    let temp_dir = TempDir::new().unwrap();

    let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 0, "Temp directory should be empty");

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .expect("Failed to execute init command");

    assert!(output.status.success());

    let config_path = temp_dir.path().join("diesel-guard.toml");
    assert!(config_path.exists());
}

#[test]
fn test_init_preserves_other_files() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("README.md"), "test").unwrap();
    fs::create_dir(temp_dir.path().join("migrations")).unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .expect("Failed to execute init command");

    assert!(output.status.success());

    assert!(temp_dir.path().join("README.md").exists());
    assert!(temp_dir.path().join("migrations").exists());
    assert!(temp_dir.path().join("diesel-guard.toml").exists());
}

// ─── New integration tests ────────────────────────────────────────────────────

#[test]
fn test_init_auto_detects_diesel_from_cargo_toml() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\n[dependencies]\ndiesel = \"2.0\"\n",
    )
    .unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(temp_dir.path().join("diesel-guard.toml")).unwrap();
    assert!(
        content.contains("framework = \"diesel\""),
        "Expected framework = \"diesel\", got:\n{}",
        content
    );
}

#[test]
fn test_init_auto_detects_sqlx_from_cargo_toml() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\n[dependencies]\nsqlx = \"0.7\"\n",
    )
    .unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(temp_dir.path().join("diesel-guard.toml")).unwrap();
    assert!(
        content.contains("framework = \"sqlx\""),
        "Expected framework = \"sqlx\", got:\n{}",
        content
    );
}

#[test]
fn test_init_auto_detects_postgres_version() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("docker-compose.yml"),
        "services:\n  db:\n    image: postgres:16\n",
    )
    .unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(temp_dir.path().join("diesel-guard.toml")).unwrap();
    assert!(
        content.contains("postgres_version = 16"),
        "Expected postgres_version = 16, got:\n{}",
        content
    );
}

#[test]
fn test_init_auto_sets_start_after_with_existing_migrations() {
    let temp_dir = TempDir::new().unwrap();

    // Framework detection
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\n[dependencies]\ndiesel = \"2.0\"\n",
    )
    .unwrap();

    // Diesel-style migration directory
    let migrations = temp_dir.path().join("migrations");
    fs::create_dir(&migrations).unwrap();
    let mig_dir = migrations.join("20241130120000_create_users");
    fs::create_dir(&mig_dir).unwrap();
    fs::write(
        mig_dir.join("up.sql"),
        "CREATE TABLE users (id SERIAL PRIMARY KEY);",
    )
    .unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(temp_dir.path().join("diesel-guard.toml")).unwrap();
    assert!(
        content.contains("start_after"),
        "Expected start_after in config, got:\n{}",
        content
    );
    assert!(
        content.contains("20241130120000"),
        "Expected timestamp 20241130120000, got:\n{}",
        content
    );
}

#[test]
fn test_init_auto_no_start_after_when_no_migrations() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\n[dependencies]\ndiesel = \"2.0\"\n",
    )
    .unwrap();

    // Empty migrations directory
    fs::create_dir(temp_dir.path().join("migrations")).unwrap();

    let output = Command::new(diesel_guard_bin())
        .current_dir(temp_dir.path())
        .args(["init", "--auto"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(temp_dir.path().join("diesel-guard.toml")).unwrap();
    assert!(
        !content.contains("start_after"),
        "Expected no start_after in config, got:\n{}",
        content
    );
}


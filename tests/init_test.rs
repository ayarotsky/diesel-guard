use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_init_creates_config() {
    let temp_dir = tempdir().expect("Failed to create temp dir");

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success()
        .stdout(
            "✓ Created diesel-guard.toml\n\nNext steps:\n\
             1. Edit diesel-guard.toml and set the 'framework' field to \"diesel\" or \"sqlx\"\n\
             2. Customize other configuration options as needed\n\
             3. Run 'diesel-guard check' to check your migrations\n",
        )
        .stderr("");

    assert!(
        temp_dir.path().join("diesel-guard.toml").exists(),
        "Config file was not created"
    );
}

#[test]
fn test_init_content_matches_example() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("diesel-guard.toml");

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success();

    let created_content = fs::read_to_string(&config_path).unwrap();
    let example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diesel-guard.toml.example");
    let example_content = fs::read_to_string(example_path).unwrap();

    assert_eq!(
        created_content, example_content,
        "Created config does not match example"
    );
}

#[test]
fn test_init_fails_when_config_exists() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("diesel-guard.toml");
    fs::write(&config_path, "# existing config").unwrap();

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(
            "Error: diesel-guard.toml already exists in current directory\n\
             Use --force to overwrite the existing file\n",
        );

    let content = fs::read_to_string(&config_path).unwrap();
    assert_eq!(content, "# existing config");
}

#[test]
fn test_init_force_overwrites_existing() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("diesel-guard.toml");
    fs::write(&config_path, "# old config").unwrap();

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .current_dir(temp_dir.path())
        .args(["init", "--force"])
        .assert()
        .success()
        .stdout(
            "✓ Overwrote diesel-guard.toml\n\nNext steps:\n\
             1. Edit diesel-guard.toml and set the 'framework' field to \"diesel\" or \"sqlx\"\n\
             2. Customize other configuration options as needed\n\
             3. Run 'diesel-guard check' to check your migrations\n",
        );

    let created_content = fs::read_to_string(&config_path).unwrap();
    let example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diesel-guard.toml.example");
    let example_content = fs::read_to_string(example_path).unwrap();
    assert_eq!(created_content, example_content);
}

#[test]
fn test_init_in_empty_directory() {
    let temp_dir = tempdir().expect("Failed to create temp dir");

    let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 0, "Temp directory should be empty");

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success();

    assert!(temp_dir.path().join("diesel-guard.toml").exists());
}

#[test]
fn test_init_preserves_other_files() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    fs::write(temp_dir.path().join("README.md"), "test").unwrap();
    fs::create_dir(temp_dir.path().join("migrations")).unwrap();

    Command::cargo_bin("diesel-guard")
        .unwrap()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success();

    assert!(temp_dir.path().join("README.md").exists());
    assert!(temp_dir.path().join("migrations").exists());
    assert!(temp_dir.path().join("diesel-guard.toml").exists());
}

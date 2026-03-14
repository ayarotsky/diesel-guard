//! Interactive setup wizard for `diesel-guard init`.

use crate::adapters::{DieselAdapter, MigrationAdapter, SqlxAdapter};
use crate::config::Config;
use crate::output::OutputFormatter;
use crate::safety_checker::SafetyChecker;
use camino::Utf8Path;
use miette::IntoDiagnostic;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static POSTGRES_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"postgres:(\d+)").expect("valid regex"));

static POSTGRES_ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:POSTGRES_VERSION|PGVERSION|PG_VERSION)\s*=\s*(\d+)").expect("valid regex")
});

// ─── Detection ───────────────────────────────────────────────────────────────

/// Detect the migration framework from `Cargo.toml` `[dependencies]`.
///
/// Returns `Some("diesel")` or `Some("sqlx")` if exactly one is present.
/// Returns `None` if both, neither, or `Cargo.toml` is absent/unparseable.
pub fn detect_framework(dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    let deps = parsed.get("dependencies")?.as_table()?;
    let has_diesel = deps.contains_key("diesel");
    let has_sqlx = deps.contains_key("sqlx");
    match (has_diesel, has_sqlx) {
        (true, false) => Some("diesel".to_string()),
        (false, true) => Some("sqlx".to_string()),
        _ => None,
    }
}

/// Detect the migrations directory by checking common locations.
pub fn detect_migrations_path(dir: &Path) -> Option<PathBuf> {
    for candidate in &["migrations", "db/migrations", "db/migrate"] {
        let path = dir.join(candidate);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// Detect the Postgres major version from `docker-compose.yml`, `Dockerfile`, or `.env`.
pub fn detect_postgres_version(dir: &Path) -> Option<u32> {
    detect_postgres_version_with_source(dir).map(|(v, _)| v)
}

/// Detect the Postgres major version and the source file it was found in.
fn detect_postgres_version_with_source(dir: &Path) -> Option<(u32, String)> {
    // Try docker-compose / Dockerfile first with the image-tag regex.
    for filename in &["compose.yml", "compose.yaml", "docker-compose.yml", "docker-compose.yaml", "Dockerfile"] {
        let path = dir.join(filename);
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Some(cap) = POSTGRES_VERSION_REGEX.captures(&content)
            && let Ok(v) = cap[1].parse::<u32>()
        {
            return Some((v, filename.to_string()));
        }
    }
    // Fall back to env-var patterns in .env files.
    for filename in &[".env", ".env.example", ".env.local", ".env.development"] {
        let path = dir.join(filename);
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Some(cap) = POSTGRES_ENV_VAR_REGEX.captures(&content)
            && let Ok(v) = cap[1].parse::<u32>()
        {
            return Some((v, filename.to_string()));
        }
    }
    None
}

/// Detect the latest migration timestamp and total migration count.
///
/// Returns `None` if the directory is empty, doesn't exist, or can't be
/// represented as UTF-8.
fn detect_latest_migration_timestamp(dir: &Path, framework: &str) -> Option<(String, usize)> {
    let utf8_dir = Utf8Path::from_path(dir)?;
    let adapter: Box<dyn MigrationAdapter> = match framework {
        "sqlx" => Box::new(SqlxAdapter),
        _ => Box::new(DieselAdapter),
    };
    let files = adapter
        .collect_migration_files(utf8_dir, None, false)
        .ok()?;
    if files.is_empty() {
        return None;
    }
    let count = files.len();
    let last = files.last()?;
    Some((last.timestamp.clone(), count))
}

/// Collect all migration timestamps as `(timestamp, display_label)` pairs.
///
/// Returns up to the last 10, oldest→newest.
fn detect_all_migration_timestamps(dir: &Path, framework: &str) -> Vec<(String, String)> {
    let Some(utf8_dir) = Utf8Path::from_path(dir) else {
        return vec![];
    };
    let adapter: Box<dyn MigrationAdapter> = match framework {
        "sqlx" => Box::new(SqlxAdapter),
        _ => Box::new(DieselAdapter),
    };
    let Ok(files) = adapter.collect_migration_files(utf8_dir, None, false) else {
        return vec![];
    };
    files
        .into_iter()
        .rev()
        .take(10)
        .rev()
        .map(|f| {
            // Diesel: migrations/<timestamp>_name/up.sql — parent dir holds the name.
            // SQLx:   migrations/<timestamp>_name.sql   — file stem holds the name.
            let label = f
                .path
                .parent()
                .and_then(|p| p.file_name())
                .filter(|name| name.contains(f.timestamp.as_str()))
                .map(|s| s.to_string())
                .or_else(|| f.path.file_stem().map(|s| s.to_string()))
                .unwrap_or_else(|| f.timestamp.clone());
            (f.timestamp, label)
        })
        .collect()
}

// ─── Detections bundle ───────────────────────────────────────────────────────

struct Detections {
    framework: Option<String>,
    migrations_path: Option<PathBuf>,
    postgres_version: Option<u32>,
    postgres_version_source: Option<String>,
    /// Latest migration timestamp and count, if any migrations were found.
    latest_migration: Option<(String, usize)>,
}

impl Detections {
    fn detect(dir: &Path) -> Self {
        let framework = detect_framework(dir);
        let migrations_path = detect_migrations_path(dir);
        let (postgres_version, postgres_version_source) =
            match detect_postgres_version_with_source(dir) {
                Some((v, src)) => (Some(v), Some(src)),
                None => (None, None),
            };
        let latest_migration = migrations_path.as_ref().and_then(|mp| {
            let fw = framework.as_deref().unwrap_or("diesel");
            detect_latest_migration_timestamp(mp, fw)
        });
        Self {
            framework,
            migrations_path,
            postgres_version,
            postgres_version_source,
            latest_migration,
        }
    }
}

// ─── Answers ─────────────────────────────────────────────────────────────────

struct WizardAnswers {
    framework: String,
    migrations_path: Option<PathBuf>,
    postgres_version: Option<u32>,
    start_after: Option<String>,
    check_down: bool,
}

impl WizardAnswers {
    /// Collect answers interactively via prompts.
    fn from_interactive(detections: &Detections) -> miette::Result<Self> {
        use inquire::{Confirm, Select};

        // Step 1: Framework
        let framework = match &detections.framework {
            Some(fw) => {
                let fw_display = if fw == "diesel" { "Diesel" } else { "SQLx" };
                let confirmed = Confirm::new(&format!("Detected `{}` — use {}?", fw, fw_display))
                    .with_default(true)
                    .prompt()
                    .map_err(inquire_err)?;
                if confirmed {
                    fw.clone()
                } else {
                    Select::new(
                        "Which migration framework are you using?",
                        vec!["diesel", "sqlx"],
                    )
                    .prompt()
                    .map_err(inquire_err)?
                    .to_string()
                }
            }
            None => Select::new(
                "Which migration framework are you using?",
                vec!["diesel", "sqlx"],
            )
            .prompt()
            .map_err(inquire_err)?
            .to_string(),
        };

        // Step 2: Migrations path
        let migrations_path = match &detections.migrations_path {
            Some(mp) => {
                let display = mp.display().to_string();
                let confirmed =
                    Confirm::new(&format!("Detected migrations at `{}` — use this?", display))
                        .with_default(true)
                        .prompt()
                        .map_err(inquire_err)?;
                if confirmed {
                    Some(mp.clone())
                } else {
                    ask_migrations_path(&framework)?
                }
            }
            None => ask_migrations_path(&framework)?,
        };

        // Step 3: Postgres version
        let postgres_version = match detections.postgres_version {
            Some(v) => {
                let confirmed = Confirm::new(&format!("Detected Postgres {} — use this?", v))
                    .with_default(true)
                    .prompt()
                    .map_err(inquire_err)?;
                if confirmed {
                    Some(v)
                } else {
                    ask_postgres_version()?
                }
            }
            None => ask_postgres_version()?,
        };

        // Step 4: start_after (only if migrations are found)
        let start_after = if let Some(ref mp) = migrations_path {
            let latest = detect_latest_migration_timestamp(mp, &framework);
            if let Some((ts, count)) = latest {
                let choice = Select::new(
                    &format!(
                        "Found {} existing migration{} — how to handle them?",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                    vec!["Skip all existing migrations (recommended)", "Start after a specific migration", "Check all migrations"],
                )
                .prompt()
                .map_err(inquire_err)?;

                match choice {
                    "Skip all existing migrations (recommended)" => {
                        println!("  → start_after = \"{}\"", ts);
                        Some(ts)
                    }
                    "Start after a specific migration" => {
                        let timestamps =
                            detect_all_migration_timestamps(mp, &framework);
                        if timestamps.is_empty() {
                            None
                        } else {
                            let labels: Vec<String> =
                                timestamps.iter().map(|(_, label)| label.clone()).collect();
                            let idx = Select::new(
                                "Start checking after which migration?",
                                labels.clone(),
                            )
                            .prompt()
                            .map_err(inquire_err)?;
                            let pos = labels.iter().position(|l| l == &idx).unwrap_or(0);
                            let selected_ts = timestamps[pos].0.clone();
                            println!("  → start_after = \"{}\"", selected_ts);
                            Some(selected_ts)
                        }
                    }
                    _ => None, // "Check all"
                }
            } else {
                None
            }
        } else {
            None
        };

        // Step 5: check_down
        let check_down = Confirm::new("Also check rollback (down) migrations?")
            .with_default(false)
            .prompt()
            .map_err(inquire_err)?;

        Ok(Self {
            framework,
            migrations_path,
            postgres_version,
            start_after,
            check_down,
        })
    }

    /// Build answers from auto-detected values without prompting.
    fn from_non_interactive(detections: &Detections) -> Self {
        let framework = detections
            .framework
            .clone()
            .unwrap_or_else(|| "diesel".to_string());
        let start_after = detections
            .latest_migration
            .as_ref()
            .map(|(ts, _)| ts.clone());
        Self {
            framework,
            migrations_path: detections.migrations_path.clone(),
            postgres_version: detections.postgres_version,
            start_after,
            check_down: false,
        }
    }
}

fn ask_postgres_version() -> miette::Result<Option<u32>> {
    use inquire::{Text, validator::Validation};
    let s = Text::new("Target Postgres version? (Enter to skip):")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                return Ok(Validation::Valid);
            }
            if input.trim().parse::<u32>().is_ok() {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(
                    format!(
                        "'{}' is not a valid Postgres major version (e.g. 14, 16)",
                        input.trim()
                    )
                    .into(),
                ))
            }
        })
        .prompt()
        .map_err(inquire_err)?;
    Ok(s.trim().parse::<u32>().ok())
}

fn ask_migrations_path(framework: &str) -> miette::Result<Option<PathBuf>> {
    use inquire::{Text, validator::Validation};
    let s = Text::new("Path to migrations directory (Enter to skip):")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                return Ok(Validation::Valid);
            }
            if std::path::Path::new(input.trim()).is_dir() {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(
                    format!("Directory '{}' does not exist", input.trim()).into(),
                ))
            }
        })
        .prompt()
        .map_err(inquire_err)?;
    let s = s.trim().to_string();
    // suppress unused warning — framework could inform future default suggestions
    let _ = framework;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(s)))
    }
}

// ─── TOML generation ─────────────────────────────────────────────────────────

fn generate_toml(answers: &WizardAnswers) -> String {
    let mut lines = vec!["# Generated by diesel-guard init".to_string()];
    lines.push(format!("framework = \"{}\"", answers.framework));
    if let Some(ref ts) = answers.start_after {
        lines.push(format!("start_after = \"{}\"", ts));
    }
    if let Some(v) = answers.postgres_version {
        lines.push(format!("postgres_version = {}", v));
    }
    if answers.check_down {
        lines.push("check_down = true".to_string());
    }
    lines.join("\n") + "\n"
}

// ─── Preview run ─────────────────────────────────────────────────────────────

fn maybe_run_preview(answers: &WizardAnswers, config_path: &Path) -> miette::Result<()> {
    use inquire::{Confirm, MultiSelect};
    use std::collections::BTreeSet;

    let Some(ref mp) = answers.migrations_path else {
        return Ok(());
    };
    let Some(utf8_path) = Utf8Path::from_path(mp) else {
        return Ok(());
    };
    if !utf8_path.exists() {
        return Ok(());
    }

    let run_preview = Confirm::new("Run a quick check now?")
        .with_default(true)
        .prompt()
        .map_err(inquire_err)?;

    if !run_preview {
        return Ok(());
    }

    let config = Config {
        framework: answers.framework.clone(),
        start_after: answers.start_after.clone(),
        check_down: answers.check_down,
        postgres_version: answers.postgres_version,
        ..Config::default()
    };

    let checker = SafetyChecker::with_config(config);
    let results = checker
        .check_path(utf8_path)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Preview check failed: {}", e))?;

    if results.is_empty() {
        println!("{}", OutputFormatter::format_summary(0));
        return Ok(());
    }

    let total: usize = results.iter().map(|(_, v)| v.len()).sum();
    for (file_path, violations) in &results {
        print!("{}", OutputFormatter::format_text(file_path, violations));
    }
    println!("{}", OutputFormatter::format_summary(total));

    // Collect unique fired check names.
    let fired_names: BTreeSet<&'static str> = results
        .iter()
        .flat_map(|(_, violations)| violations.iter().map(|v| v.check_name))
        .filter(|name| !name.is_empty())
        .collect();

    if fired_names.is_empty() {
        return Ok(());
    }

    let names: Vec<&'static str> = fired_names.into_iter().collect();
    let to_disable = MultiSelect::new("Disable any of these checks?", names)
        .with_help_message("Space to toggle, Enter to confirm (select none to skip)")
        .prompt()
        .map_err(inquire_err)?;

    if to_disable.is_empty() {
        return Ok(());
    }

    // Append disable_checks to the config file.
    let mut existing = std::fs::read_to_string(config_path)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to read config: {}", e))?;

    if existing.contains("enable_checks") {
        println!(
            "  ⚠ Skipped: cannot add disable_checks while enable_checks is set in the config.\n  \
             Edit diesel-guard.toml manually to disable: {}",
            to_disable.join(", ")
        );
        return Ok(());
    }

    let entries: Vec<String> = to_disable.iter().map(|n| format!("\"{}\"", n)).collect();
    let line = format!("disable_checks = [{}]\n", entries.join(", "));
    existing.push_str(&line);
    std::fs::write(config_path, existing)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to write config: {}", e))?;

    println!("  → Disabled: {}", to_disable.join(", "));

    Ok(())
}

// ─── Auto summary ─────────────────────────────────────────────────────────────

fn print_auto_summary(detections: &Detections, answers: &WizardAnswers) {
    println!("Auto-detected:");

    let fw_source = if detections.framework.is_some() {
        "(from Cargo.toml)"
    } else {
        "(default)"
    };
    println!("  framework:   {}   {}", answers.framework, fw_source);

    match &answers.migrations_path {
        Some(mp) => {
            let count = detections
                .latest_migration
                .as_ref()
                .map(|(_, c)| *c)
                .unwrap_or(0);
            println!("  migrations:  {}   ({} migrations)", mp.display(), count);
        }
        None => println!("  migrations:  (none detected)"),
    }

    match (answers.postgres_version, &detections.postgres_version_source) {
        (Some(v), Some(src)) => println!("  postgres:    {}   (from {})", v, src),
        (Some(v), None) => println!("  postgres:    {}", v),
        _ => println!("  postgres:    (not detected)"),
    }

    match (&answers.start_after, &detections.latest_migration) {
        (Some(ts), Some((_, count))) => {
            println!("  start_after: {}   (skipping {} migrations)", ts, count)
        }
        (Some(ts), None) => println!("  start_after: {}", ts),
        _ => println!("  start_after: (none)"),
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Run the init wizard.
///
/// If `auto` is true, skip all prompts and use auto-detected defaults.
pub fn run(force: bool, auto: bool) -> miette::Result<()> {
    let cwd = std::env::current_dir()
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to get current directory: {}", e))?;

    let config_path = cwd.join("diesel-guard.toml");
    let file_existed = config_path.exists();
    if file_existed && !force {
        eprintln!("Error: diesel-guard.toml already exists in current directory");
        eprintln!("Use --force to overwrite the existing file");
        std::process::exit(1);
    }

    let detections = Detections::detect(&cwd);
    let answers = if auto {
        let a = WizardAnswers::from_non_interactive(&detections);
        print_auto_summary(&detections, &a);
        a
    } else {
        WizardAnswers::from_interactive(&detections)?
    };

    let toml_content = generate_toml(&answers);
    std::fs::write(&config_path, &toml_content)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to write config file: {}", e))?;

    if file_existed {
        println!("✓ Overwrote diesel-guard.toml");
    } else {
        println!("✓ Created diesel-guard.toml");
    }

    if !auto {
        maybe_run_preview(&answers, &config_path)?;
    }

    println!();
    println!("To add diesel-guard to CI:");
    println!("  - uses: ayarotsky/diesel-guard-action@v1");

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn inquire_err(e: inquire::InquireError) -> miette::Report {
    match e {
        inquire::InquireError::NotTTY => miette::miette!(
            "This command requires an interactive terminal.\n\
             Use `diesel-guard init --auto` for non-interactive mode."
        ),
        inquire::InquireError::OperationCanceled => miette::miette!("Init cancelled."),
        inquire::InquireError::OperationInterrupted => miette::miette!("Init interrupted."),
        e => miette::miette!("Prompt error: {}", e),
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── detect_framework ──

    #[test]
    fn test_detect_framework_diesel_only() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n[dependencies]\ndiesel = \"2.0\"\n",
        )
        .unwrap();
        assert_eq!(detect_framework(dir.path()), Some("diesel".to_string()));
    }

    #[test]
    fn test_detect_framework_sqlx_only() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n[dependencies]\nsqlx = \"0.7\"\n",
        )
        .unwrap();
        assert_eq!(detect_framework(dir.path()), Some("sqlx".to_string()));
    }

    #[test]
    fn test_detect_framework_both() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n[dependencies]\ndiesel = \"2.0\"\nsqlx = \"0.7\"\n",
        )
        .unwrap();
        assert_eq!(detect_framework(dir.path()), None);
    }

    #[test]
    fn test_detect_framework_neither() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n[dependencies]\nserde = \"1.0\"\n",
        )
        .unwrap();
        assert_eq!(detect_framework(dir.path()), None);
    }

    #[test]
    fn test_detect_framework_ignores_dev_dependencies() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n[dependencies]\nserde = \"1.0\"\n[dev-dependencies]\ndiesel = \"2.0\"\n",
        )
        .unwrap();
        assert_eq!(detect_framework(dir.path()), None);
    }

    // ── detect_migrations_path ──

    #[test]
    fn test_detect_migrations_path_standard() {
        let dir = TempDir::new().unwrap();
        let migrations = dir.path().join("migrations");
        fs::create_dir(&migrations).unwrap();
        assert_eq!(detect_migrations_path(dir.path()), Some(migrations));
    }

    #[test]
    fn test_detect_migrations_path_db_migrations() {
        let dir = TempDir::new().unwrap();
        let migrations = dir.path().join("db").join("migrations");
        fs::create_dir_all(&migrations).unwrap();
        assert_eq!(detect_migrations_path(dir.path()), Some(migrations));
    }

    #[test]
    fn test_detect_migrations_path_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(detect_migrations_path(dir.path()), None);
    }

    // ── detect_postgres_version ──

    #[test]
    fn test_detect_postgres_version_compose_yml() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("compose.yml"),
            "services:\n  db:\n    image: postgres:17\n",
        )
        .unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(17));
    }

    #[test]
    fn test_detect_postgres_version_compose_yaml() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("compose.yaml"),
            "services:\n  db:\n    image: postgres:17\n",
        )
        .unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(17));
    }

    #[test]
    fn test_detect_postgres_version_docker_compose() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  db:\n    image: postgres:16\n",
        )
        .unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(16));
    }

    #[test]
    fn test_detect_postgres_version_dockerfile() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM postgres:14\nRUN something\n",
        )
        .unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(14));
    }

    #[test]
    fn test_detect_postgres_version_env_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "DATABASE_URL=postgres://...\nPOSTGRES_VERSION=16\n")
            .unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(16));
    }

    #[test]
    fn test_detect_postgres_version_env_pgversion() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PGVERSION=15\n").unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(15));
    }

    #[test]
    fn test_detect_postgres_version_env_pg_version() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env.example"), "PG_VERSION=14\n").unwrap();
        assert_eq!(detect_postgres_version(dir.path()), Some(14));
    }

    #[test]
    fn test_detect_postgres_version_docker_takes_precedence_over_env() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  db:\n    image: postgres:16\n",
        )
        .unwrap();
        fs::write(dir.path().join(".env"), "POSTGRES_VERSION=14\n").unwrap();
        // docker-compose should win
        assert_eq!(detect_postgres_version(dir.path()), Some(16));
    }

    #[test]
    fn test_detect_postgres_version_with_source_docker_compose() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  db:\n    image: postgres:16\n",
        )
        .unwrap();
        let result = detect_postgres_version_with_source(dir.path());
        assert_eq!(result, Some((16, "docker-compose.yml".to_string())));
    }

    #[test]
    fn test_detect_postgres_version_with_source_env() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "POSTGRES_VERSION=15\n").unwrap();
        let result = detect_postgres_version_with_source(dir.path());
        assert_eq!(result, Some((15, ".env".to_string())));
    }

    #[test]
    fn test_detect_postgres_version_not_found() {
        let dir = TempDir::new().unwrap();
        assert_eq!(detect_postgres_version(dir.path()), None);
    }

    // ── generate_toml ──

    #[test]
    fn test_generate_toml_minimal() {
        let answers = WizardAnswers {
            framework: "diesel".to_string(),
            migrations_path: None,
            postgres_version: None,
            start_after: None,
            check_down: false,
        };
        assert_eq!(
            generate_toml(&answers),
            "# Generated by diesel-guard init\nframework = \"diesel\"\n"
        );
    }

    #[test]
    fn test_generate_toml_full() {
        let answers = WizardAnswers {
            framework: "diesel".to_string(),
            migrations_path: None,
            postgres_version: Some(16),
            start_after: Some("20241130120000".to_string()),
            check_down: true,
        };
        let toml = generate_toml(&answers);
        assert!(toml.contains("framework = \"diesel\""));
        assert!(toml.contains("start_after = \"20241130120000\""));
        assert!(toml.contains("postgres_version = 16"));
        assert!(toml.contains("check_down = true"));
    }

    #[test]
    fn test_generate_toml_defaults_omitted() {
        let answers = WizardAnswers {
            framework: "sqlx".to_string(),
            migrations_path: None,
            postgres_version: None,
            start_after: None,
            check_down: false,
        };
        let toml = generate_toml(&answers);
        assert!(!toml.contains("check_down"));
        assert!(!toml.contains("postgres_version"));
        assert!(!toml.contains("start_after"));
    }

    // ── from_non_interactive ──

    #[test]
    fn test_non_interactive_defaults_no_detections() {
        let detections = Detections {
            framework: None,
            migrations_path: None,
            postgres_version: None,
            postgres_version_source: None,
            latest_migration: None,
        };
        let answers = WizardAnswers::from_non_interactive(&detections);
        assert_eq!(answers.framework, "diesel");
        assert!(answers.start_after.is_none());
        assert!(answers.postgres_version.is_none());
        assert!(!answers.check_down);
    }

    #[test]
    fn test_non_interactive_uses_detected_values() {
        let detections = Detections {
            framework: Some("sqlx".to_string()),
            migrations_path: Some(PathBuf::from("/tmp/migrations")),
            postgres_version: Some(16),
            postgres_version_source: Some("docker-compose.yml".to_string()),
            latest_migration: Some(("20241130120000".to_string(), 5)),
        };
        let answers = WizardAnswers::from_non_interactive(&detections);
        assert_eq!(answers.framework, "sqlx");
        assert_eq!(answers.postgres_version, Some(16));
        assert_eq!(answers.start_after, Some("20241130120000".to_string()));
    }
}

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use diesel_guard::ast_dump;
use diesel_guard::checks::CheckDescription;
use diesel_guard::output::OutputFormatter;
use diesel_guard::violation::Severity;
use diesel_guard::{Config, SafetyChecker};
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::io::Write;
use std::process::exit;

const CONFIG_TEMPLATE: &str = include_str!("../diesel-guard.toml.example");

#[derive(Parser)]
#[command(
    name = "diesel-guard",
    version,
    about = "Catch unsafe Postgres migrations in Diesel and SQLx before they take down production",
    long_about = "Catch unsafe Postgres migrations in Diesel and SQLx before they take down production.

diesel-guard parses SQL with PostgreSQL's own parser (libpg_query) and flags operations
that acquire dangerous locks or cause table rewrites.

QUICK START:
  diesel-guard init              Create diesel-guard.toml in the current directory
  diesel-guard check             Check all migrations in ./migrations/
  diesel-guard check up.sql      Check a single file
  diesel-guard check -           Read SQL from stdin

Exit codes:
  0  No violations found (warnings do not affect exit code)
  1  One or more errors found (or a fatal error occurred)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check migrations for unsafe operations
    #[command(long_about = "Check migrations for unsafe operations.

PATH can be:
  - A directory — scans all up.sql files recursively
  - A single .sql file
  - \"-\" to read from stdin

If PATH is omitted, defaults to \"migrations/\".

diesel-guard looks for diesel-guard.toml in the current directory. If no config
file is found, default settings are used with a warning.

Exit codes:
  0  No errors found (warnings do not affect exit code)
  1  One or more errors found

EXAMPLES:
  diesel-guard check
  diesel-guard check migrations/
  diesel-guard check db/migrate/20240101_add_users/up.sql
  cat migration.sql | diesel-guard check -
  diesel-guard check migrations/ --format json")]
    Check {
        /// Path to migration file or directory, or "-" for stdin (default: "migrations/")
        path: Option<Utf8PathBuf>,

        /// Output format: "text" (default) or "json"
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Initialize diesel-guard configuration file
    #[command(long_about = "Initialize diesel-guard configuration file.

Creates diesel-guard.toml in the current directory with all available options
documented. Edit the file to set your migration framework (\"diesel\" or \"sqlx\")
and any other options.

Use --force to regenerate the config file and reset it to defaults.

EXAMPLES:
  diesel-guard init
  diesel-guard init --force")]
    Init {
        /// Overwrite existing config file if it exists
        #[arg(long)]
        force: bool,
    },

    /// Dump the pg_query AST for SQL as JSON
    #[command(long_about = "Dump the pg_query AST for SQL as JSON.

Useful when writing custom Rhai checks — shows the exact AST structure that
your scripts receive. Provide either --sql for an inline string or --file for
a .sql file (not both).

EXAMPLES:
  diesel-guard dump-ast --sql \"ALTER TABLE users ADD COLUMN email TEXT\"
  diesel-guard dump-ast --file migrations/20240101/up.sql")]
    DumpAst {
        /// SQL string to parse
        #[arg(long)]
        sql: Option<String>,

        /// Path to a .sql file to parse
        #[arg(long)]
        file: Option<Utf8PathBuf>,
    },

    /// List all available checks
    ListChecks {
        /// Output format: "text" (default) or "json"
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Show full description of a specific check
    Explain {
        /// Check name (e.g. AddIndexCheck or require_concurrent)
        check_name: String,

        /// Output format: "text" (default) or "json"
        #[arg(long, default_value = "text")]
        format: String,
    },
}

fn run_check(path: &camino::Utf8Path, format: &str) -> Result<()> {
    if !Utf8PathBuf::from("diesel-guard.toml").exists() {
        eprintln!("Warning: No config file found. Using default configuration.");
    }
    let config = Config::load().map_err(|e| miette::miette!(e))?;

    let checker = SafetyChecker::with_config(config);
    let results = checker.check_path(path)?;

    if results.is_empty() {
        match format {
            "json" => println!("[]"),
            "github" => {}
            _ => println!("{}", OutputFormatter::format_summary(0, 0)),
        }
        return Ok(());
    }

    let total_errors: usize = results
        .iter()
        .flat_map(|(_, v)| v)
        .filter(|(_, v)| v.severity == Severity::Error)
        .count();

    match format {
        "json" => {
            println!("{}", OutputFormatter::format_json(&results));
        }
        "github" => {
            for (file_path, violations) in &results {
                print!("{}", OutputFormatter::format_github(file_path, violations));
            }
        }
        _ => {
            let total_warnings: usize = results
                .iter()
                .flat_map(|(_, v)| v)
                .filter(|(_, v)| v.severity == Severity::Warning)
                .count();
            for (file_path, violations) in &results {
                print!("{}", OutputFormatter::format_text(file_path, violations));
            }
            println!(
                "{}",
                OutputFormatter::format_summary(total_errors, total_warnings)
            );
        }
    }

    if total_errors > 0 {
        let _ = std::io::stdout().flush();
        exit(1);
    }

    Ok(())
}

fn run_list_checks(format: &str) -> Result<()> {
    if !Utf8PathBuf::from("diesel-guard.toml").exists() {
        eprintln!("Warning: No config file found. Using default configuration.");
    }
    let config = Config::load().map_err(|e| miette::miette!(e))?;
    let full_checker = SafetyChecker::with_config(Config {
        disable_checks: vec![],
        enable_checks: vec![],
        ..config.clone()
    });

    let checks: Vec<_> = full_checker.registry().iter_checks().collect();

    if format == "json" {
        let json: Vec<_> = checks
            .iter()
            .map(|c| {
                let check_type = if c.describe()[0].script_path.is_some() {
                    "custom"
                } else {
                    "builtin"
                };
                let severity = if config.is_check_warning(c.name()) {
                    "warning"
                } else {
                    "error"
                };
                serde_json::json!({
                    "name": c.name(),
                    "type": check_type,
                    "severity": severity,
                    "enabled": config.is_check_enabled(c.name()),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    } else {
        println!("{:<40} {:<10} {:<10} ENABLED", "NAME", "TYPE", "SEVERITY");
        for check in &checks {
            let check_type = if check.describe()[0].script_path.is_some() {
                "custom"
            } else {
                "builtin"
            };
            let severity = if config.is_check_warning(check.name()) {
                "warning"
            } else {
                "error"
            };
            let enabled = if config.is_check_enabled(check.name()) {
                "yes"
            } else {
                "no"
            };
            println!(
                "{:<40} {:<10} {:<10} {}",
                check.name(),
                check_type,
                severity,
                enabled
            );
        }
    }
    Ok(())
}

fn run_explain(check_name: &str, format: &str) -> Result<()> {
    if !Utf8PathBuf::from("diesel-guard.toml").exists() {
        eprintln!("Warning: No config file found. Using default configuration.");
    }
    let config = Config::load().map_err(|e| miette::miette!(e))?;
    let full_checker = SafetyChecker::with_config(Config {
        disable_checks: vec![],
        enable_checks: vec![],
        ..config.clone()
    });

    let Some(check) = full_checker
        .registry()
        .iter_checks()
        .find(|c| c.name() == check_name)
    else {
        eprintln!("Error: No check named '{check_name}'.");
        eprintln!("Run 'diesel-guard list-checks' to see available checks.");
        exit(1);
    };

    let descriptions = check.describe();
    let check_type = if descriptions[0].script_path.is_some() {
        "custom"
    } else {
        "builtin"
    };
    let enabled = config.is_check_enabled(check.name());
    let severity = if config.is_check_warning(check.name()) {
        "warning"
    } else {
        "error"
    };

    if format == "json" {
        print_explain_json(check.name(), check_type, severity, enabled, &descriptions);
    } else {
        print_explain_text(check.name(), check_type, severity, enabled, &descriptions);
    }
    Ok(())
}

fn print_explain_json(
    check_name: &str,
    check_type: &str,
    severity: &str,
    enabled: bool,
    descriptions: &[CheckDescription],
) {
    let descs_json: Vec<_> = descriptions
        .iter()
        .map(|desc| {
            serde_json::json!({
                "operation": json_string_or_null(&desc.operation),
                "problem": json_string_or_null(&desc.problem),
                "safe_alternative": json_string_or_null(&desc.safe_alternative),
            })
        })
        .collect();
    let mut obj = serde_json::json!({
        "name": check_name,
        "type": check_type,
        "severity": severity,
        "enabled": enabled,
        "descriptions": descs_json,
    });
    if let Some(path) = &descriptions[0].script_path {
        obj["script_path"] = serde_json::Value::String(path.clone());
    }
    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
}

fn json_string_or_null(value: &str) -> serde_json::Value {
    if value.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(value.to_string())
    }
}

fn print_explain_text(
    check_name: &str,
    check_type: &str,
    severity: &str,
    enabled: bool,
    descriptions: &[CheckDescription],
) {
    let enabled_str = if enabled { "enabled" } else { "disabled" };
    println!("{check_name} ({check_type} | {severity} | {enabled_str})\n");
    let first = &descriptions[0];
    if first.operation.is_empty() && first.script_path.is_some() {
        println!("No description available. Add fn describe() to the script:");
        println!("  {}", first.script_path.as_deref().unwrap_or(""));
        return;
    }
    if descriptions.len() == 1 {
        println!("Operation: {}\n", first.operation);
        print_description_body(first);
        return;
    }
    for (i, desc) in descriptions.iter().enumerate() {
        println!("[{}] {}\n", i + 1, desc.operation);
        print_description_body(desc);
        if i + 1 < descriptions.len() {
            println!();
        }
    }
}

fn print_description_body(description: &CheckDescription) {
    println!("Problem:");
    for line in description.problem.lines() {
        println!("  {line}");
    }
    println!("\nSafe alternative:");
    for line in description.safe_alternative.lines() {
        println!("  {line}");
    }
}

fn main() -> Result<()> {
    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .unicode(true)
                .context_lines(3)
                .build(),
        )
    }))?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Check { path, format } => {
            let path = path.unwrap_or_else(|| Utf8PathBuf::from("migrations"));
            run_check(&path, &format)?;
        }

        Commands::DumpAst { sql, file } => {
            let sql_input = match (sql, file) {
                (Some(s), _) => s,
                (None, Some(path)) => fs::read_to_string(&path)
                    .into_diagnostic()
                    .map_err(|e| miette::miette!("Failed to read file '{}': {}", path, e))?,
                (None, None) => {
                    eprintln!("Error: provide either --sql or --file");
                    exit(1);
                }
            };

            let json = ast_dump::dump_ast(&sql_input)?;
            println!("{json}");
        }

        Commands::ListChecks { format } => run_list_checks(&format)?,

        Commands::Explain { check_name, format } => run_explain(&check_name, &format)?,

        Commands::Init { force } => {
            let config_path = Utf8PathBuf::from("diesel-guard.toml");

            // Check if config file already exists
            let file_existed = config_path.exists();
            if file_existed && !force {
                eprintln!("Error: diesel-guard.toml already exists in current directory");
                eprintln!("Use --force to overwrite the existing file");
                exit(1);
            }

            // Write config template to file
            fs::write(&config_path, CONFIG_TEMPLATE)
                .into_diagnostic()
                .map_err(|e| miette::miette!("Failed to write config file: {}", e))?;

            if file_existed {
                println!("✓ Overwrote diesel-guard.toml");
            } else {
                println!("✓ Created diesel-guard.toml");
            }
            println!();
            println!("Next steps:");
            println!(
                "1. Edit diesel-guard.toml and set the 'framework' field to \"diesel\" or \"sqlx\""
            );
            println!("2. Customize other configuration options as needed");
            println!("3. Run 'diesel-guard check' to check your migrations");
        }
    }

    Ok(())
}

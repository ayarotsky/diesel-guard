use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use diesel_guard::ast_dump;
use diesel_guard::output::OutputFormatter;
use diesel_guard::{Config, SafetyChecker};
use miette::{IntoDiagnostic, Result};
use std::fs;
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
  0  No violations found
  1  One or more violations found (or a fatal error occurred)"
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
  0  No violations found
  1  One or more violations found

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
            // Load configuration with explicit error handling
            let config = match Config::load() {
                Ok(config) => config,
                Err(diesel_guard::config::ConfigError::IoError(_))
                    if !Utf8PathBuf::from("diesel-guard.toml").exists() =>
                {
                    // File doesn't exist - use defaults with warning
                    eprintln!("Warning: No config file found. Using default configuration.");
                    Config::default()
                }
                Err(e) => {
                    // Config file exists but has errors - this is fatal
                    return Err(e.into());
                }
            };

            let checker = SafetyChecker::with_config(config);

            let results = checker.check_path(&path)?;

            if results.is_empty() {
                println!("{}", OutputFormatter::format_summary(0));
                exit(0);
            }

            let total_violations: usize = results.iter().map(|(_, v)| v.len()).sum();

            match format.as_str() {
                "json" => {
                    println!("{}", OutputFormatter::format_json(&results));
                }
                _ => {
                    // text format
                    for (file_path, violations) in &results {
                        print!("{}", OutputFormatter::format_text(file_path, violations));
                    }
                    println!("{}", OutputFormatter::format_summary(total_violations));
                }
            }

            if total_violations > 0 {
                exit(1);
            }
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

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
#[command(name = "diesel-guard")]
#[command(version, about = "Catch unsafe Postgres migrations in Diesel and SQLx before they take down production", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check migrations for unsafe operations
    Check {
        /// Path to migration file or directory
        path: Utf8PathBuf,

        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Initialize diesel-guard configuration file
    Init {
        /// Overwrite existing config file if it exists
        #[arg(long)]
        force: bool,
    },

    /// Dump the pg_query AST for SQL as JSON (useful for writing custom Rhai checks)
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
                OutputFormatter::print_summary(0);
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
                    OutputFormatter::print_summary(total_violations);
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
            println!("3. Run 'diesel-guard check <path>' to check your migrations");
        }
    }

    Ok(())
}

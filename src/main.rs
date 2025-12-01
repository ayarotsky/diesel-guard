use clap::{Parser, Subcommand};
use diesel_guard::output::OutputFormatter;
use diesel_guard::{Config, SafetyChecker};
use std::fs;
use std::path::PathBuf;
use std::process::exit;

const CONFIG_TEMPLATE: &str = include_str!("../diesel-guard.toml.example");

#[derive(Parser)]
#[command(name = "diesel-guard")]
#[command(version, about = "Catch unsafe PostgreSQL migrations in Diesel before they take down production", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check migrations for unsafe operations
    Check {
        /// Path to migration file or directory
        path: PathBuf,

        /// Output format (text or json)
        #[arg(long, default_value = "text")]
        format: String,

        /// Allow unsafe operations (exit with 0 even if violations found)
        #[arg(long)]
        allow_unsafe: bool,
    },

    /// Initialize diesel-guard configuration file
    Init {
        /// Overwrite existing config file if it exists
        #[arg(long)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            path,
            format,
            allow_unsafe,
        } => {
            // Load configuration with explicit error handling
            let config = match Config::load() {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Error loading configuration: {}", e);
                    eprintln!("Using default configuration.");
                    Config::default()
                }
            };

            let checker = SafetyChecker::with_config(config);

            let results = match checker.check_path(&path) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

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

            if !allow_unsafe && total_violations > 0 {
                exit(1);
            }
        }

        Commands::Init { force } => {
            let config_path = PathBuf::from("diesel-guard.toml");

            // Check if config file already exists
            let file_existed = config_path.exists();
            if file_existed && !force {
                eprintln!("Error: diesel-guard.toml already exists in current directory");
                eprintln!("Use --force to overwrite the existing file");
                exit(1);
            }

            // Write config template to file
            match fs::write(&config_path, CONFIG_TEMPLATE) {
                Ok(_) => {
                    if file_existed {
                        println!("✓ Overwrote diesel-guard.toml");
                    } else {
                        println!("✓ Created diesel-guard.toml");
                    }
                    println!();
                    println!("Next steps:");
                    println!("1. Edit diesel-guard.toml to customize your configuration");
                    println!("2. Run 'diesel-guard check <path>' to check your migrations");
                }
                Err(e) => {
                    eprintln!("Error: Failed to write config file: {}", e);
                    exit(1);
                }
            }
        }
    }
}

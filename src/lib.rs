pub mod adapters;
pub mod ast_dump;
pub mod checks;
pub mod config;
pub mod error;
pub mod output;
pub mod parser;
pub mod safety_checker;
pub mod scripting;
pub mod violation;

pub use adapters::{MigrationAdapter, MigrationContext, MigrationFile};
pub use config::{Config, ConfigError};
pub use safety_checker::{CheckEntry, SafetyChecker};
pub use violation::Violation;

mod check_descriptions {
    include!(concat!(env!("OUT_DIR"), "/check_descriptions.rs"));
}

/// Return the description for a built-in check by its struct name, or `""` for
/// custom checks (which have no `//!` module doc in the generated table).
pub fn description_for_check(name: &str) -> &'static str {
    check_descriptions::CHECK_DESCRIPTIONS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or("", |(_, d)| d)
}

/// A list of `(line_number, violation)` pairs produced by a single SQL file.
///
/// The line number is 1-indexed and points to the first token of the statement
/// that triggered the violation.
pub type ViolationList = Vec<(usize, Violation)>;

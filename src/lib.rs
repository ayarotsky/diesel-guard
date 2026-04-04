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
pub use safety_checker::SafetyChecker;
pub use violation::Violation;

/// A list of `(line_number, violation)` pairs produced by a single SQL file.
///
/// The line number is 1-indexed and points to the first token of the statement
/// that triggered the violation.
pub type ViolationList = Vec<(usize, Violation)>;

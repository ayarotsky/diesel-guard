//! Safety linter for Diesel and SQLx migration files.
//!
//! `diesel-guard` inspects SQL migration files and reports statements that are
//! unsafe to run on a live PostgreSQL database (e.g. adding a NOT NULL column
//! without a default, dropping an index non-concurrently, locking a table).
//!
//! # Quick start
//!
//! ```no_run
//! use diesel_guard::SafetyChecker;
//! use camino::Utf8Path;
//!
//! let checker = SafetyChecker::default();
//! let violations = checker.check_path(Utf8Path::new("migrations/")).unwrap();
//! ```

pub mod adapters;
pub mod ast_dump;
pub mod checks;
pub mod config;
pub mod error;
pub mod formatters;
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

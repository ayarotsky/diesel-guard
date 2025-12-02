use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum DieselGuardError {
    #[error("Failed to parse SQL: {msg}")]
    #[diagnostic(
        code(diesel_guard::parse_error),
        help("Check that your SQL syntax is valid PostgreSQL"),
        url("https://www.postgresql.org/docs/current/sql-syntax.html")
    )]
    ParseError {
        msg: String,
        #[source_code]
        src: Option<String>,
        #[label("problematic SQL")]
        span: Option<SourceSpan>,
    },

    #[error("Failed to read file")]
    #[diagnostic(
        code(diesel_guard::io_error),
        help("Ensure the file exists and you have read permissions")
    )]
    IoError(#[from] std::io::Error),

    #[error("Failed to traverse directory")]
    #[diagnostic(
        code(diesel_guard::walkdir_error),
        help("Check directory permissions and path validity")
    )]
    WalkDirError(#[from] walkdir::Error),

    #[error("Configuration error")]
    #[diagnostic(
        code(diesel_guard::config_error),
        help("Run 'diesel-guard init' to create a valid configuration file")
    )]
    ConfigError(#[from] crate::config::ConfigError),
}

impl DieselGuardError {
    /// Create a simple parse error with just a message (backward compatible)
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::ParseError {
            msg: msg.into(),
            src: None,
            span: None,
        }
    }

    /// Create a parse error with source code context
    pub fn parse_error_with_source(
        msg: impl Into<String>,
        src: impl Into<String>,
        span: Option<SourceSpan>,
    ) -> Self {
        Self::ParseError {
            msg: msg.into(),
            src: Some(src.into()),
            span,
        }
    }
}

pub type Result<T> = std::result::Result<T, DieselGuardError>;

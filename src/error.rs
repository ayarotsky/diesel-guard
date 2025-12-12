use miette::{Diagnostic, NamedSource, SourceOffset, SourceSpan};
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
        src: Option<NamedSource<String>>,
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

    /// Attach file context to an existing error.
    ///
    /// For parse errors, this adds the source code with filename and computes
    /// the span from any line/column info in the error message.
    pub fn with_file_context(self, path: &str, source: String) -> Self {
        match self {
            Self::ParseError { msg, .. } => {
                let span = parse_location(&msg)
                    .map(|(line, col)| SourceOffset::from_location(&source, line, col).into());

                Self::ParseError {
                    msg,
                    src: Some(NamedSource::new(path, source)),
                    span,
                }
            }
            other => other,
        }
    }
}

/// Parse line and column from sqlparser error messages.
///
/// Format: `"... at Line: {line}, Column: {column}"`
fn parse_location(msg: &str) -> Option<(usize, usize)> {
    let (_, after_line) = msg.split_once("at Line: ")?;
    let (line_str, col_str) = after_line.split_once(", Column: ")?;

    let line = line_str.parse().ok()?;
    let col = col_str.parse().ok()?;

    Some((line, col))
}

pub type Result<T> = std::result::Result<T, DieselGuardError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_location() {
        let msg = "sql parser error: Expected: a list of columns in parentheses, found: INDEX at Line: 5, Column: 59";
        assert_eq!(parse_location(msg), Some((5, 59)));
    }

    #[test]
    fn test_parse_location_no_location() {
        let msg = "some error without location info";
        assert_eq!(parse_location(msg), None);
    }

    #[test]
    fn test_parse_location_single_digit() {
        let msg = "error at Line: 1, Column: 1";
        assert_eq!(parse_location(msg), Some((1, 1)));
    }
}

use miette::{Diagnostic, NamedSource, SourceOffset, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum DieselGuardError {
    #[error("Failed to parse SQL: {msg}")]
    #[diagnostic(
        code(diesel_guard::parse_error),
        help("Check that your SQL syntax is valid Postgres"),
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

    #[error(transparent)]
    #[diagnostic(transparent)]
    ConfigError(#[from] crate::config::ConfigError),
}

impl DieselGuardError {
    /// Create a parse error with just a message (no source location).
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::ParseError {
            msg: msg.into(),
            src: None,
            span: None,
        }
    }

    /// Attach file context to a parse error.
    ///
    /// Adds source code with filename and computes the span from any
    /// position info in the error message. Non-parse errors are returned as-is.
    pub fn with_file_context(self, path: &str, source: String) -> Self {
        match self {
            Self::ParseError { msg, .. } => {
                let span = parse_byte_position(&msg)
                    .map(|pos| SourceSpan::new(SourceOffset::from(pos), 0));

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

/// Parse byte position from pg_query error messages.
///
/// pg_query format: `"... at position N"` where N is a 1-based byte offset.
fn parse_byte_position(msg: &str) -> Option<usize> {
    let pos_str = msg.rsplit_once("at position ")?.1;
    let end = pos_str
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(pos_str.len());
    let pos: usize = pos_str[..end].parse().ok()?;
    // pg_query positions are 1-based; convert to 0-based
    Some(pos.saturating_sub(1))
}

pub type Result<T> = std::result::Result<T, DieselGuardError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_byte_position() {
        let msg = "syntax error at or near \"INVALID\" at position 42";
        assert_eq!(parse_byte_position(msg), Some(41)); // 1-based → 0-based
    }

    #[test]
    fn test_parse_byte_position_no_position() {
        let msg = "some error without position info";
        assert_eq!(parse_byte_position(msg), None);
    }

    #[test]
    fn test_parse_byte_position_single_digit() {
        let msg = "error at position 1";
        assert_eq!(parse_byte_position(msg), Some(0)); // 1-based → 0-based
    }
}

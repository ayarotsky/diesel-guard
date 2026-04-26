use miette::{Diagnostic, NamedSource, SourceOffset, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum DieselGuardError {
    #[error("Failed to parse SQL: {msg}")]
    #[diagnostic(help("Check that your SQL syntax is valid"))]
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
    #[must_use]
    pub fn with_file_context(self, path: &str, source: String) -> Self {
        match self {
            Self::ParseError { msg, span, .. } => {
                // Prefer pre-computed offset from parser; fall back to position in message; then byte 0.
                let span = span
                    .or_else(|| {
                        parse_byte_position(&msg).map(|p| SourceSpan::new(SourceOffset::from(p), 0))
                    })
                    .unwrap_or_else(|| SourceSpan::new(SourceOffset::from(0), 0));

                Self::ParseError {
                    msg,
                    src: Some(NamedSource::new(path, source)),
                    span: Some(span),
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

    #[test]
    fn test_with_file_context_extracts_position_from_message() {
        // ParseError with span=None but message containing "at position N" → span derived from message.
        let err = DieselGuardError::ParseError {
            msg: "syntax error at or near \"X\" at position 5".to_string(),
            src: None,
            span: None,
        };
        let result = err.with_file_context("migration.sql", "SELECT @X;".to_string());
        match result {
            DieselGuardError::ParseError { src, span, .. } => {
                assert!(src.is_some(), "source should be attached");
                let span = span.expect("span should be derived from message position");
                assert_eq!(span.offset(), 4); // position 5 → 0-based offset 4
            }
            other => panic!("Expected ParseError, got {other:?}"),
        }
    }

    #[test]
    fn test_with_file_context_passes_through_non_parse_errors() {
        let err = DieselGuardError::IoError(std::io::Error::other("disk full"));
        let result = err.with_file_context("migration.sql", "SELECT 1;".to_string());
        assert!(matches!(result, DieselGuardError::IoError(_)));
    }
}

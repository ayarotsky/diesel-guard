use crate::error::{DieselGuardError, Result};
use miette::{SourceOffset, SourceSpan};
use pg_query::protobuf::RawStmt;

pub mod comment_parser;

pub use comment_parser::IgnoreRange;

/// Parsed SQL with metadata for safety-assured handling
pub struct ParsedSql {
    pub stmts: Vec<RawStmt>,
    pub sql: String,
    pub ignore_ranges: Vec<IgnoreRange>,
}

/// Parse SQL string into AST statements
pub fn parse(sql: &str) -> Result<Vec<RawStmt>> {
    // Split first so each statement's byte offset is known before parsing.
    // If the scanner itself fails, fall back to whole-file parse (no position info).
    let Ok(stmts) = pg_query::split_with_scanner(sql) else {
        return pg_query::parse(sql)
            .map(|r| r.protobuf.stmts)
            .map_err(|e| DieselGuardError::parse_error(e.to_string()));
    };

    let mut all_stmts = Vec::new();
    for stmt in stmts {
        let leading = stmt.len() - stmt.trim_start().len();
        let offset = stmt.as_ptr() as usize - sql.as_ptr() as usize + leading;
        let parsed = pg_query::parse(stmt).map_err(|e| DieselGuardError::ParseError {
            msg: e.to_string(),
            src: None,
            span: Some(SourceSpan::new(SourceOffset::from(offset), 0)),
        })?;
        // Adjust stmt_location to be relative to the full SQL, not the individual statement.
        let adjusted = parsed.protobuf.stmts.into_iter().map(|mut s| {
            s.stmt_location += i32::try_from(offset).unwrap_or(0);
            s
        });
        all_stmts.extend(adjusted);
    }
    Ok(all_stmts)
}

/// Parse SQL with metadata for safety-assured blocks
pub fn parse_with_metadata(sql: &str) -> Result<ParsedSql> {
    let ignore_ranges = comment_parser::CommentParser::parse_ignore_ranges(sql)?;
    let stmts = parse(sql)?;

    Ok(ParsedSql {
        stmts,
        sql: sql.to_string(),
        ignore_ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let result = parse("SELECT * FROM users;");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_alter_table() {
        let result = parse("ALTER TABLE users ADD COLUMN email VARCHAR(255);");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_sql() {
        let result = parse("INVALID SQL HERE");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_with_metadata() {
        let sql = r"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        ";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 1);
        assert!(!result.sql.is_empty());
    }

    #[test]
    fn test_parse_with_metadata_no_blocks() {
        let sql = "ALTER TABLE users DROP COLUMN email;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 1);
        assert_eq!(result.ignore_ranges.len(), 0);
        assert_eq!(result.sql, sql);
    }

    #[test]
    fn test_parse_unterminated_string_falls_back_to_whole_file_parse() {
        // An unterminated string literal causes split_with_scanner to fail,
        // exercising the fallback path on lines 21-23.
        let result = parse("SELECT 'unterminated");
        assert!(result.is_err());
    }

    // pg_query parses everything sqlparser couldn't:

    #[test]
    fn test_unique_using_index_parsed() {
        let sql =
            "ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 1, "UNIQUE USING INDEX should be parsed");
    }

    #[test]
    fn test_unique_using_index_with_other_statements() {
        let sql = r"
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
ALTER TABLE users DROP COLUMN old_field;
        ";

        // pg_query parses BOTH statements (no more parser limitation)
        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 2, "Both statements should be parsed");
    }

    #[test]
    fn test_drop_index_concurrently_parsed() {
        let sql = "DROP INDEX CONCURRENTLY idx_users_email;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.stmts.len(),
            1,
            "DROP INDEX CONCURRENTLY should be parsed"
        );
    }

    #[test]
    fn test_drop_index_concurrently_if_exists() {
        let sql = "DROP INDEX CONCURRENTLY IF EXISTS idx_users_email;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 1);
    }

    #[test]
    fn test_drop_index_concurrently_with_other_statements() {
        let sql = r"
DROP INDEX CONCURRENTLY idx_users_email;
ALTER TABLE users DROP COLUMN old_field;
        ";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 2, "Both statements should be parsed");
    }

    #[test]
    fn test_primary_key_using_index_parsed() {
        let sql = "ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.stmts.len(),
            1,
            "PRIMARY KEY USING INDEX should be parsed"
        );
    }

    #[test]
    fn test_primary_key_using_index_with_other_statements() {
        let sql = r"
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
ALTER TABLE users DROP COLUMN old_field;
        ";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 2, "Both statements should be parsed");
    }

    #[test]
    fn test_reindex_concurrently_parsed() {
        let sql = "REINDEX INDEX CONCURRENTLY idx_users_email;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(
            result.stmts.len(),
            1,
            "REINDEX CONCURRENTLY should be parsed"
        );
    }

    #[test]
    fn test_reindex_table_concurrently() {
        let sql = "REINDEX TABLE CONCURRENTLY users;";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 1);
    }

    #[test]
    fn test_reindex_with_other_statements() {
        let sql = r"
REINDEX INDEX CONCURRENTLY idx_users_email;
ALTER TABLE users DROP COLUMN old_field;
        ";

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 2, "Both statements should be parsed");
    }
}

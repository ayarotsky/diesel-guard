use crate::error::{DieselGuardError, Result};
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
    pg_query::parse(sql)
        .map(|result| result.protobuf.stmts)
        .map_err(|e| DieselGuardError::parse_error(e.to_string()))
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
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

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
        let sql = r#"
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;
ALTER TABLE users DROP COLUMN old_field;
        "#;

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
        let sql = r#"
DROP INDEX CONCURRENTLY idx_users_email;
ALTER TABLE users DROP COLUMN old_field;
        "#;

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
        let sql = r#"
ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_pkey;
ALTER TABLE users DROP COLUMN old_field;
        "#;

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
        let sql = r#"
REINDEX INDEX CONCURRENTLY idx_users_email;
ALTER TABLE users DROP COLUMN old_field;
        "#;

        let result = parse_with_metadata(sql).unwrap();
        assert_eq!(result.stmts.len(), 2, "Both statements should be parsed");
    }
}

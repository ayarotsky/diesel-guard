//! Common test utilities for check modules.
//!
//! This module provides shared helper functions and macros used across all check tests,
//! reducing code duplication and ensuring consistent test setup.

/// Assert that a check detects exactly one violation with the given operation name.
#[cfg(test)]
#[macro_export]
macro_rules! assert_detects_violation {
    ($check:expr, $sql:expr, $operation:expr) => {{
        use $crate::checks::test_utils::parse_sql;
        let stmt = parse_sql($sql);
        let violations = $check.check(&stmt);
        assert_eq!(violations.len(), 1, "Expected exactly 1 violation");
        assert_eq!(
            violations[0].operation, $operation,
            "Expected operation '{}' but got '{}'",
            $operation, violations[0].operation
        );
    }};
}

/// Assert that a check allows (finds no violations in) the given SQL.
#[cfg(test)]
#[macro_export]
macro_rules! assert_allows {
    ($check:expr, $sql:expr) => {{
        use $crate::checks::test_utils::parse_sql;
        let stmt = parse_sql($sql);
        let violations = $check.check(&stmt);
        assert_eq!(
            violations.len(),
            0,
            "Expected no violations but found {}",
            violations.len()
        );
    }};
}

#[cfg(test)]
pub use test_helpers::*;

#[cfg(test)]
mod test_helpers {
    use crate::checks::pg_helpers::NodeEnum;

    /// Parse a SQL string into a NodeEnum for testing.
    ///
    /// # Panics
    /// Panics if the SQL cannot be parsed or contains no statements.
    pub fn parse_sql(sql: &str) -> NodeEnum {
        let result = pg_query::parse(sql).expect("Failed to parse SQL");
        result
            .protobuf
            .stmts
            .into_iter()
            .next()
            .expect("No statements found")
            .stmt
            .expect("No stmt node")
            .node
            .expect("No node")
    }
}

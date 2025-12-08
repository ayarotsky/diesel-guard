mod add_column;
mod add_index;
mod add_not_null;
mod alter_column_type;
mod create_extension;
mod drop_column;
mod rename_column;
mod unnamed_constraint;

#[cfg(test)]
mod test_utils;

pub use add_column::AddColumnCheck;
pub use add_index::AddIndexCheck;
pub use add_not_null::AddNotNullCheck;
pub use alter_column_type::AlterColumnTypeCheck;
pub use create_extension::CreateExtensionCheck;
pub use drop_column::DropColumnCheck;
pub use rename_column::RenameColumnCheck;
pub use unnamed_constraint::UnnamedConstraintCheck;

use crate::config::Config;

/// Helper functions for check implementations
mod helpers {
    use std::fmt::Display;

    /// Convert an optional displayable value to String, using default if None
    pub fn display_or_default<T: Display>(value: Option<&T>, default: &str) -> String {
        value
            .map(|v| v.to_string())
            .unwrap_or_else(|| default.to_string())
    }

    /// Get prefix string for unique indexes
    pub fn unique_prefix(is_unique: bool) -> &'static str {
        if is_unique {
            "UNIQUE "
        } else {
            ""
        }
    }
}

use crate::parser::IgnoreRange;
use crate::violation::Violation;
pub use helpers::*;
use sqlparser::ast::Statement;

/// Trait for implementing safety checks on SQL statements
pub trait Check: Send + Sync {
    /// Run the check on a statement and return any violations found
    fn check(&self, stmt: &Statement) -> Vec<Violation>;
}

/// All available check names (single source of truth)
pub const ALL_CHECK_NAMES: &[&str] = &[
    "AddColumnCheck",
    "AddIndexCheck",
    "AddNotNullCheck",
    "AlterColumnTypeCheck",
    "CreateExtensionCheck",
    "DropColumnCheck",
    "RenameColumnCheck",
    "UnnamedConstraintCheck",
];

/// Registry of all available checks
pub struct CheckRegistry {
    checks: Vec<Box<dyn Check>>,
}

impl CheckRegistry {
    /// Create registry with all checks enabled (uses default config)
    pub fn new() -> Self {
        Self::with_config(&Config::default())
    }

    /// Create registry with configuration-based filtering
    pub fn with_config(config: &Config) -> Self {
        let mut registry = Self { checks: vec![] };
        registry.register_enabled_checks(config);
        registry
    }

    /// Register all enabled checks based on configuration
    fn register_enabled_checks(&mut self, config: &Config) {
        self.register_check(config, AddColumnCheck);
        self.register_check(config, AddIndexCheck);
        self.register_check(config, AddNotNullCheck);
        self.register_check(config, AlterColumnTypeCheck);
        self.register_check(config, CreateExtensionCheck);
        self.register_check(config, DropColumnCheck);
        self.register_check(config, RenameColumnCheck);
        self.register_check(config, UnnamedConstraintCheck);
    }

    /// Register a check if it's enabled in configuration
    fn register_check<C: Check + 'static>(&mut self, config: &Config, check: C) {
        // Extract just the type name (e.g., "AddColumnCheck" from "diesel_guard::checks::AddColumnCheck")
        let full_name = std::any::type_name::<C>();
        let name = full_name.split("::").last().unwrap_or(full_name);

        if config.is_check_enabled(name) {
            self.checks.push(Box::new(check));
        }
    }

    /// Check a single statement against all registered checks
    pub fn check_statement(&self, stmt: &Statement) -> Vec<Violation> {
        self.checks
            .iter()
            .flat_map(|check| check.check(stmt))
            .collect()
    }

    /// Check multiple statements against all registered checks
    pub fn check_statements(&self, stmts: &[Statement]) -> Vec<Violation> {
        stmts
            .iter()
            .flat_map(|stmt| self.check_statement(stmt))
            .collect()
    }

    /// Check statements with safety-assured context
    ///
    /// Uses a line-based approach: if any line of a statement's SQL falls within
    /// a safety-assured block, the entire statement is skipped.
    pub fn check_statements_with_context(
        &self,
        statements: &[Statement],
        sql: &str,
        ignore_ranges: &[IgnoreRange],
    ) -> Vec<Violation> {
        // Build set of all ignored line numbers for fast lookup
        let ignored_lines: std::collections::HashSet<usize> = ignore_ranges
            .iter()
            .flat_map(|range| (range.start_line + 1)..range.end_line)
            .collect();

        // Track which lines have been matched to handle multiple statements with same keyword
        let mut matched_lines = std::collections::HashSet::new();
        let mut violations = Vec::new();

        for stmt in statements {
            // Find where this statement appears in source SQL
            let stmt_line = Self::find_statement_line(stmt, sql, &matched_lines);
            matched_lines.insert(stmt_line);

            // Skip checks if statement is in an ignored line
            if !ignored_lines.contains(&stmt_line) {
                violations.extend(self.check_statement(stmt));
            }
        }

        violations
    }

    /// Find the first unmatched line where a statement appears in the source SQL
    ///
    /// Uses simple keyword matching to locate the statement, excluding already-matched lines.
    /// Returns line 1 if the statement cannot be found (safe fallback).
    fn find_statement_line(
        stmt: &Statement,
        sql: &str,
        matched_lines: &std::collections::HashSet<usize>,
    ) -> usize {
        let stmt_str = stmt.to_string().to_uppercase();
        let first_word = stmt_str.split_whitespace().next().unwrap_or("");

        sql.lines()
            .enumerate()
            .find(|(idx, line)| {
                let line_num = idx + 1; // 1-indexed
                let trimmed = line.trim();

                // Skip already matched lines and comments
                if matched_lines.contains(&line_num) || trimmed.starts_with("--") {
                    return false;
                }

                // Check if line starts with the statement keyword
                trimmed.to_uppercase().starts_with(first_word)
            })
            .map(|(idx, _)| idx + 1) // 1-indexed
            .unwrap_or(1) // Fallback to line 1 (won't be in ignore range)
    }
}

impl Default for CheckRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = CheckRegistry::new();
        assert_eq!(registry.checks.len(), ALL_CHECK_NAMES.len());
    }

    #[test]
    fn test_registry_with_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string()],
            ..Default::default()
        };

        let registry = CheckRegistry::with_config(&config);
        assert_eq!(registry.checks.len(), ALL_CHECK_NAMES.len() - 1); // One check disabled
    }

    #[test]
    fn test_registry_with_multiple_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string(), "DropColumnCheck".to_string()],
            ..Default::default()
        };

        let registry = CheckRegistry::with_config(&config);
        assert_eq!(registry.checks.len(), ALL_CHECK_NAMES.len() - 2); // Two checks disabled
    }

    #[test]
    fn test_registry_with_all_checks_disabled() {
        let config = Config {
            disable_checks: ALL_CHECK_NAMES.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };

        let registry = CheckRegistry::with_config(&config);
        assert_eq!(registry.checks.len(), 0); // All checks disabled
    }

    #[test]
    fn test_check_with_safety_assured_block() {
        use sqlparser::dialect::PostgreSqlDialect;
        use sqlparser::parser::Parser;

        let registry = CheckRegistry::new();
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).unwrap();
        let ignore_ranges = vec![IgnoreRange {
            start_line: 2,
            end_line: 4,
        }];

        let violations = registry.check_statements_with_context(&statements, sql, &ignore_ranges);
        assert_eq!(violations.len(), 0); // Statement is in safety-assured block
    }

    #[test]
    fn test_check_without_safety_assured_block() {
        use sqlparser::dialect::PostgreSqlDialect;
        use sqlparser::parser::Parser;

        let registry = CheckRegistry::new();
        let sql = "ALTER TABLE users DROP COLUMN email;";

        let statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).unwrap();
        let ignore_ranges = vec![];

        let violations = registry.check_statements_with_context(&statements, sql, &ignore_ranges);
        assert_eq!(violations.len(), 1); // DropColumnCheck should trigger
    }
}

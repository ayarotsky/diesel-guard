mod add_column;
mod add_index;
mod add_not_null;
mod alter_column_type;
mod drop_column;

#[cfg(test)]
mod test_utils;

pub use add_column::AddColumnCheck;
pub use add_index::AddIndexCheck;
pub use add_not_null::AddNotNullCheck;
pub use alter_column_type::AlterColumnTypeCheck;
pub use drop_column::DropColumnCheck;

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
    "DropColumnCheck",
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
        macro_rules! register_check {
            ($checks:expr, $config:expr, $check_name:expr, $check_instance:expr) => {
                if $config.is_check_enabled($check_name) {
                    $checks.push(Box::new($check_instance));
                }
            };
        }

        let mut checks: Vec<Box<dyn Check>> = vec![];

        // Register checks using macro to reduce duplication
        register_check!(checks, config, "AddColumnCheck", AddColumnCheck);
        register_check!(checks, config, "AddIndexCheck", AddIndexCheck);
        register_check!(checks, config, "AddNotNullCheck", AddNotNullCheck);
        register_check!(checks, config, "AlterColumnTypeCheck", AlterColumnTypeCheck);
        register_check!(checks, config, "DropColumnCheck", DropColumnCheck);

        Self { checks }
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
    /// # Note on Statement Line Tracking
    ///
    /// This method relies on `statement_lines` accurately tracking where each statement
    /// appears in the source SQL. The line tracking uses keyword matching and works
    /// reliably for standard SQL. Edge cases (rare non-standard SQL) may trigger a
    /// fallback to line 1 with a warning logged to stderr.
    ///
    /// See `src/parser/mod.rs::extract_statement_lines` for implementation details.
    pub fn check_statements_with_context(
        &self,
        statements: &[Statement],
        statement_lines: &[usize],
        ignore_ranges: &[IgnoreRange],
    ) -> Vec<Violation> {
        statements
            .iter()
            .zip(statement_lines.iter())
            .flat_map(|(stmt, &line)| self.check_statement_with_context(stmt, line, ignore_ranges))
            .collect()
    }

    /// Check a single statement with safety-assured context
    ///
    /// Bypasses all checks if the statement's line number falls within a safety-assured block.
    fn check_statement_with_context(
        &self,
        stmt: &Statement,
        stmt_line: usize,
        ignore_ranges: &[IgnoreRange],
    ) -> Vec<Violation> {
        // Check if statement is in a safety-assured block
        if Self::is_line_ignored(stmt_line, ignore_ranges) {
            return vec![];
        }

        // Run all checks
        self.checks
            .iter()
            .flat_map(|check| check.check(stmt))
            .collect()
    }

    /// Check if a line is within any ignore range
    ///
    /// Returns true if the line number falls between start_line and end_line
    /// (exclusive of the comment directive lines themselves).
    fn is_line_ignored(line: usize, ranges: &[IgnoreRange]) -> bool {
        ranges.iter().any(|range| {
            // Line must be within range (exclusive of start/end comment lines)
            line > range.start_line && line < range.end_line
        })
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
    fn test_is_line_ignored_in_range() {
        let ranges = vec![IgnoreRange {
            start_line: 5,
            end_line: 10,
        }];

        assert!(!CheckRegistry::is_line_ignored(5, &ranges)); // Start line excluded
        assert!(CheckRegistry::is_line_ignored(6, &ranges));
        assert!(CheckRegistry::is_line_ignored(9, &ranges));
        assert!(!CheckRegistry::is_line_ignored(10, &ranges)); // End line excluded
        assert!(!CheckRegistry::is_line_ignored(11, &ranges));
    }

    #[test]
    fn test_multiple_ranges() {
        let ranges = vec![
            IgnoreRange {
                start_line: 5,
                end_line: 10,
            },
            IgnoreRange {
                start_line: 15,
                end_line: 20,
            },
        ];

        assert!(CheckRegistry::is_line_ignored(7, &ranges));
        assert!(!CheckRegistry::is_line_ignored(12, &ranges));
        assert!(CheckRegistry::is_line_ignored(17, &ranges));
    }
}

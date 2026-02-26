mod add_column;
mod add_index;
mod add_json_column;
mod add_not_null;
mod add_primary_key;
mod add_serial_column;
mod add_unique_constraint;
mod alter_column_type;
mod char_type;
mod create_extension;
mod drop_column;
mod drop_database;
mod drop_index;
mod drop_primary_key;
mod drop_table;
mod generated_column;
pub mod pg_helpers;
mod reindex;
mod rename_column;
mod rename_table;
mod short_int_primary_key;
mod timestamp_type;
mod truncate_table;
mod unnamed_constraint;
mod wide_index;

#[cfg(test)]
mod test_utils;

pub use add_column::AddColumnCheck;
pub use add_index::AddIndexCheck;
pub use add_json_column::AddJsonColumnCheck;
pub use add_not_null::AddNotNullCheck;
pub use add_primary_key::AddPrimaryKeyCheck;
pub use add_serial_column::AddSerialColumnCheck;
pub use add_unique_constraint::AddUniqueConstraintCheck;
pub use alter_column_type::AlterColumnTypeCheck;
pub use char_type::CharTypeCheck;
pub use create_extension::CreateExtensionCheck;
pub use drop_column::DropColumnCheck;
pub use drop_database::DropDatabaseCheck;
pub use drop_index::DropIndexCheck;
pub use drop_primary_key::DropPrimaryKeyCheck;
pub use drop_table::DropTableCheck;
pub use generated_column::GeneratedColumnCheck;
pub use reindex::ReindexCheck;
pub use rename_column::RenameColumnCheck;
pub use rename_table::RenameTableCheck;
pub use short_int_primary_key::ShortIntegerPrimaryKeyCheck;
pub use timestamp_type::TimestampTypeCheck;
pub use truncate_table::TruncateTableCheck;
pub use unnamed_constraint::UnnamedConstraintCheck;
pub use wide_index::WideIndexCheck;

pub use crate::config::Config;

/// Helper functions for check implementations
mod helpers {
    /// Get prefix string for unique indexes
    pub fn unique_prefix(is_unique: bool) -> &'static str {
        if is_unique { "UNIQUE " } else { "" }
    }

    /// Get SQL clause for IF EXISTS modifier
    pub fn if_exists_clause(if_exists: bool) -> &'static str {
        if if_exists { " IF EXISTS" } else { "" }
    }
}

use crate::parser::IgnoreRange;
use crate::violation::Violation;
pub use helpers::*;
use pg_helpers::{NodeEnum, extract_node};
use pg_query::protobuf::RawStmt;
use std::sync::LazyLock;

/// Lazily-derived list of all built-in check names from an unfiltered registry.
/// This avoids maintaining a manual list that can drift from the actual checks.
static BUILTIN_CHECK_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let registry = Registry::new();
    registry.checks.iter().map(|c| c.name()).collect()
});

/// Trait for implementing safety checks on SQL statements
pub trait Check: Send + Sync {
    /// The check's name, used for config-based disabling (e.g., "AddColumnCheck").
    /// Derived automatically from the struct name via `type_name`.
    fn name(&self) -> &'static str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap_or(full)
    }

    /// Run the check on a pg_query AST node and return any violations found
    fn check(&self, node: &NodeEnum, config: &Config) -> Vec<Violation>;
}

/// Registry of all available checks
pub struct Registry {
    checks: Vec<Box<dyn Check>>,
}

impl Registry {
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
        self.register_check(config, AddJsonColumnCheck);
        self.register_check(config, AddNotNullCheck);
        self.register_check(config, AddPrimaryKeyCheck);
        self.register_check(config, AddSerialColumnCheck);
        self.register_check(config, AddUniqueConstraintCheck);
        self.register_check(config, AlterColumnTypeCheck);
        self.register_check(config, CharTypeCheck);
        self.register_check(config, CreateExtensionCheck);
        self.register_check(config, DropColumnCheck);
        self.register_check(config, DropDatabaseCheck);
        self.register_check(config, DropIndexCheck);
        self.register_check(config, DropPrimaryKeyCheck);
        self.register_check(config, DropTableCheck);
        self.register_check(config, GeneratedColumnCheck);
        self.register_check(config, ReindexCheck);
        self.register_check(config, RenameColumnCheck);
        self.register_check(config, RenameTableCheck);
        self.register_check(config, ShortIntegerPrimaryKeyCheck);
        self.register_check(config, TimestampTypeCheck);
        self.register_check(config, TruncateTableCheck);
        self.register_check(config, UnnamedConstraintCheck);
        self.register_check(config, WideIndexCheck);
    }

    /// Add a check to the registry.
    pub fn add_check(&mut self, check: Box<dyn Check>) {
        self.checks.push(check);
    }

    /// Return the names of all currently active checks (built-in + custom, minus disabled).
    pub fn active_check_names(&self) -> Vec<&str> {
        self.checks.iter().map(|c| c.name()).collect()
    }

    /// Register a check if it's enabled in configuration
    fn register_check(&mut self, config: &Config, check: impl Check + 'static) {
        if !config.is_check_enabled(check.name()) {
            return;
        }
        self.checks.push(Box::new(check));
    }

    /// Check a single AST node against all registered checks
    pub fn check_node(&self, node: &NodeEnum, config: &Config) -> Vec<Violation> {
        self.checks
            .iter()
            .flat_map(|check| check.check(node, config))
            .collect()
    }

    /// Check statements with safety-assured context.
    ///
    /// Uses RawStmt.stmt_location (byte offset) to determine which line each
    /// statement falls on, then skips checks for statements in safety-assured blocks.
    pub fn check_stmts_with_context(
        &self,
        stmts: &[RawStmt],
        sql: &str,
        ignore_ranges: &[IgnoreRange],
        config: &Config,
    ) -> Vec<Violation> {
        // Build set of all ignored line numbers for fast lookup
        let ignored_lines: std::collections::HashSet<usize> = ignore_ranges
            .iter()
            .flat_map(|range| (range.start_line + 1)..range.end_line)
            .collect();

        // pg_query's stmt_location can point to whitespace/comments preceding a
        // statement. Use the scanner to get accurate token positions.
        let token_starts = non_comment_token_starts(sql);

        let mut violations = Vec::new();

        for raw_stmt in stmts {
            let node = match extract_node(raw_stmt) {
                Some(node) => node,
                None => continue,
            };

            let offset = first_token_at_or_after(&token_starts, raw_stmt.stmt_location as usize);
            let stmt_line = byte_offset_to_line(sql, offset);

            if !ignored_lines.contains(&stmt_line) {
                violations.extend(self.check_node(node, config));
            }
        }

        violations
    }

    /// Get all built-in check names (regardless of which are enabled).
    pub fn builtin_check_names() -> &'static [&'static str] {
        &BUILTIN_CHECK_NAMES
    }
}

/// Convert a byte offset to a 1-indexed line number.
fn byte_offset_to_line(sql: &str, byte_offset: usize) -> usize {
    let offset = byte_offset.min(sql.len());
    sql[..offset].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Sorted byte positions of all non-comment tokens, via pg_query's scanner.
fn non_comment_token_starts(sql: &str) -> Vec<usize> {
    use pg_query::protobuf::Token;

    let scan_result = match pg_query::scan(sql) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    scan_result
        .tokens
        .iter()
        .filter(|t| t.token != Token::SqlComment as i32 && t.token != Token::CComment as i32)
        .map(|t| t.start as usize)
        .collect()
}

/// First non-comment token position at or after `offset`.
fn first_token_at_or_after(token_starts: &[usize], offset: usize) -> usize {
    match token_starts.binary_search(&offset) {
        Ok(i) => token_starts[i],
        Err(i) => token_starts.get(i).copied().unwrap_or(offset),
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = Registry::new();
        assert_eq!(registry.checks.len(), Registry::builtin_check_names().len());
    }

    #[test]
    fn test_registry_includes_all_checks_when_no_version_set() {
        let registry = Registry::new();
        assert!(registry.active_check_names().contains(&"AddColumnCheck"));
        assert_eq!(registry.checks.len(), Registry::builtin_check_names().len());
    }

    #[test]
    fn test_registry_with_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string()],
            ..Default::default()
        };

        let registry = Registry::with_config(&config);
        assert_eq!(
            registry.checks.len(),
            Registry::builtin_check_names().len() - 1
        );
    }

    #[test]
    fn test_registry_with_multiple_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string(), "DropColumnCheck".to_string()],
            ..Default::default()
        };

        let registry = Registry::with_config(&config);
        assert_eq!(
            registry.checks.len(),
            Registry::builtin_check_names().len() - 2
        );
    }

    #[test]
    fn test_registry_with_all_checks_disabled() {
        let config = Config {
            disable_checks: Registry::builtin_check_names()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        };

        let registry = Registry::with_config(&config);
        assert_eq!(registry.checks.len(), 0);
    }

    #[test]
    fn test_check_with_safety_assured_block() {
        let registry = Registry::new();
        let sql = r#"
-- safety-assured:start
ALTER TABLE users DROP COLUMN email;
-- safety-assured:end
        "#;

        let result = pg_query::parse(sql).unwrap();
        let ignore_ranges = vec![IgnoreRange {
            start_line: 2,
            end_line: 4,
        }];

        let violations = registry.check_stmts_with_context(
            &result.protobuf.stmts,
            sql,
            &ignore_ranges,
            &Config::default(),
        );
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_check_without_safety_assured_block() {
        let registry = Registry::new();
        let sql = "ALTER TABLE users DROP COLUMN email;";

        let result = pg_query::parse(sql).unwrap();
        let ignore_ranges = vec![];

        let violations = registry.check_stmts_with_context(
            &result.protobuf.stmts,
            sql,
            &ignore_ranges,
            &Config::default(),
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_byte_offset_to_line() {
        let sql = "line1\nline2\nline3";
        assert_eq!(byte_offset_to_line(sql, 0), 1);
        assert_eq!(byte_offset_to_line(sql, 5), 1); // at '\n'
        assert_eq!(byte_offset_to_line(sql, 6), 2); // start of line2
        assert_eq!(byte_offset_to_line(sql, 12), 3); // start of line3
    }

    #[test]
    fn test_first_token_at_or_after_skips_comments() {
        let sql = "/* outer /* inner */ still outer */ SELECT 1;";
        let tokens = non_comment_token_starts(sql);
        let offset = first_token_at_or_after(&tokens, 0);
        assert_eq!(&sql[offset..offset + 6], "SELECT");
    }
}

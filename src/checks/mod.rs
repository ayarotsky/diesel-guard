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
use crate::error::Result;
use crate::violation::Violation;
use sqlparser::ast::Statement;

/// Trait for implementing safety checks on SQL statements
pub trait Check: Send + Sync {
    /// Name of the check
    fn name(&self) -> &str;

    /// Run the check on a statement and return any violations found
    fn check(&self, stmt: &Statement) -> Result<Vec<Violation>>;
}

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
        let mut checks: Vec<Box<dyn Check>> = vec![];

        // Conditionally register each check based on config
        if config.is_check_enabled("AddColumnCheck") {
            checks.push(Box::new(AddColumnCheck));
        }
        if config.is_check_enabled("AddIndexCheck") {
            checks.push(Box::new(AddIndexCheck));
        }
        if config.is_check_enabled("AddNotNullCheck") {
            checks.push(Box::new(AddNotNullCheck));
        }
        if config.is_check_enabled("AlterColumnTypeCheck") {
            checks.push(Box::new(AlterColumnTypeCheck));
        }
        if config.is_check_enabled("DropColumnCheck") {
            checks.push(Box::new(DropColumnCheck));
        }

        Self { checks }
    }

    /// Check a single statement against all registered checks
    pub fn check_statement(&self, stmt: &Statement) -> Vec<Violation> {
        self.checks
            .iter()
            .flat_map(|check| check.check(stmt).unwrap_or_default())
            .collect()
    }

    /// Check multiple statements against all registered checks
    pub fn check_statements(&self, stmts: &[Statement]) -> Vec<Violation> {
        stmts
            .iter()
            .flat_map(|stmt| self.check_statement(stmt))
            .collect()
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
        assert_eq!(registry.checks.len(), 5);
    }

    #[test]
    fn test_registry_with_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string()],
            ..Default::default()
        };

        let registry = CheckRegistry::with_config(&config);
        assert_eq!(registry.checks.len(), 4); // One check disabled
    }

    #[test]
    fn test_registry_with_multiple_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string(), "DropColumnCheck".to_string()],
            ..Default::default()
        };

        let registry = CheckRegistry::with_config(&config);
        assert_eq!(registry.checks.len(), 3); // Two checks disabled
    }

    #[test]
    fn test_registry_with_all_checks_disabled() {
        let config = Config {
            disable_checks: vec![
                "AddColumnCheck".to_string(),
                "AddIndexCheck".to_string(),
                "AddNotNullCheck".to_string(),
                "AlterColumnTypeCheck".to_string(),
                "DropColumnCheck".to_string(),
            ],
            ..Default::default()
        };

        let registry = CheckRegistry::with_config(&config);
        assert_eq!(registry.checks.len(), 0); // All checks disabled
    }
}

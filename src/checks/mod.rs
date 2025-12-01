mod add_column;
mod add_index;
mod alter_column_type;
mod drop_column;

#[cfg(test)]
mod test_utils;

pub use add_column::AddColumnCheck;
pub use add_index::AddIndexCheck;
pub use alter_column_type::AlterColumnTypeCheck;
pub use drop_column::DropColumnCheck;

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
    pub fn new() -> Self {
        Self {
            checks: vec![
                Box::new(AddColumnCheck),
                Box::new(AddIndexCheck),
                Box::new(AlterColumnTypeCheck),
                Box::new(DropColumnCheck),
            ],
        }
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
        assert_eq!(registry.checks.len(), 4);
    }
}

use crate::adapters::{DieselAdapter, MigrationAdapter, SqlxAdapter};
use crate::checks::Registry;
use crate::config::Config;
use crate::error::Result;
use crate::parser::reindex_detector::{detect_reindex_violations, ReindexMatch};
use crate::parser::SqlParser;
use crate::violation::Violation;
use camino::Utf8Path;
use std::fs;
use std::sync::Arc;

pub struct SafetyChecker {
    parser: SqlParser,
    registry: Registry,
    config: Config,
}

impl SafetyChecker {
    /// Create with configuration loaded from diesel-guard.toml
    /// Falls back to defaults if config file doesn't exist or has errors
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
            Config::default()
        });
        Self::with_config(config)
    }

    /// Create with specific configuration (useful for testing)
    pub fn with_config(config: Config) -> Self {
        Self {
            parser: SqlParser::new(),
            registry: Registry::with_config(&config),
            config,
        }
    }

    /// Check SQL string for violations
    pub fn check_sql(&self, sql: &str) -> Result<Vec<Violation>> {
        // Check for unsafe REINDEX patterns before parsing (sqlparser can't parse REINDEX)
        let mut violations = self.detect_reindex_violations(sql);

        // Also check if ANY REINDEX exists (even if check is disabled, we need to know
        // so we don't fail on parse errors from REINDEX statements)
        let has_any_reindex = !detect_reindex_violations(sql).is_empty();

        // Try to parse the SQL
        match self.parser.parse_with_metadata(sql) {
            Ok(parsed) => {
                // Parsing succeeded, add AST-based violations
                violations.extend(self.registry.check_statements_with_context(
                    &parsed.statements,
                    &parsed.sql,
                    &parsed.ignore_ranges,
                ));
                Ok(violations)
            }
            Err(e) => {
                // Parsing failed - if REINDEX is in the SQL (either safe or unsafe),
                // return whatever violations we found instead of the parse error
                // (since REINDEX causes parse failures due to sqlparser limitation)
                if !violations.is_empty() || has_any_reindex {
                    Ok(violations)
                } else {
                    // No REINDEX found, return the original parse error
                    Err(e)
                }
            }
        }
    }

    /// Detect unsafe REINDEX patterns in raw SQL and create violations
    fn detect_reindex_violations(&self, sql: &str) -> Vec<Violation> {
        // Only check if ReindexCheck is enabled
        if !self.config.is_check_enabled("ReindexCheck") {
            return vec![];
        }

        detect_reindex_violations(sql)
            .into_iter()
            .map(|m| Self::create_reindex_violation(&m))
            .collect()
    }

    /// Create a violation for an unsafe REINDEX match
    fn create_reindex_violation(m: &ReindexMatch) -> Violation {
        let target_desc = match m.reindex_type.as_str() {
            "INDEX" => format!("index '{}'", m.target_name),
            "TABLE" => format!("table '{}'", m.target_name),
            "SCHEMA" => format!("schema '{}'", m.target_name),
            "DATABASE" => format!("database '{}'", m.target_name),
            "SYSTEM" => format!("system catalogs in '{}'", m.target_name),
            _ => m.target_name.clone(),
        };

        Violation::new(
            "REINDEX without CONCURRENTLY",
            format!(
                "REINDEX {type} '{target}' without CONCURRENTLY acquires an ACCESS EXCLUSIVE lock, \
                blocking all operations on the {target_desc} until complete. Duration depends on index size.",
                type = m.reindex_type,
                target = m.target_name,
                target_desc = target_desc
            ),
            format!(r#"Use REINDEX CONCURRENTLY for lock-free reindexing (PostgreSQL 12+):

   REINDEX {type} CONCURRENTLY {target};

Note: CONCURRENTLY requires PostgreSQL 12+ and cannot be run inside a transaction block.

For Diesel migrations:
1. Create metadata.toml in your migration directory:
   run_in_transaction = false

2. Use REINDEX CONCURRENTLY in your up.sql:
   REINDEX {type} CONCURRENTLY {target};

For SQLx migrations:
1. Add the no-transaction directive at the top of your migration file:
   -- no-transaction

2. Use REINDEX CONCURRENTLY:
   REINDEX {type} CONCURRENTLY {target};

Considerations:
- Takes longer to complete than regular REINDEX
- Allows concurrent read/write operations
- If it fails, the index may be left in "invalid" state and need manual cleanup
- Cannot be rolled back (no transaction support)"#,
                type = m.reindex_type,
                target = m.target_name
            ),
        )
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Utf8Path) -> Result<Vec<Violation>> {
        let sql = fs::read_to_string(path)?;

        // Check for unsafe REINDEX patterns before parsing (sqlparser can't parse REINDEX)
        let mut violations = self.detect_reindex_violations(&sql);

        // Also check if ANY REINDEX exists (even if check is disabled)
        let has_any_reindex = !detect_reindex_violations(&sql).is_empty();

        // For most files, just parse normally
        // Direction-aware parsing is only needed for marker-based SQLx migrations
        // which will be handled by check_directory when using SqlxAdapter
        match self.parser.parse_with_metadata(&sql) {
            Ok(parsed) => {
                violations.extend(self.registry.check_statements_with_context(
                    &parsed.statements,
                    &parsed.sql,
                    &parsed.ignore_ranges,
                ));
                Ok(violations)
            }
            Err(e) => {
                // Parsing failed - if REINDEX is in the SQL, return violations instead of error
                if !violations.is_empty() || has_any_reindex {
                    Ok(violations)
                } else {
                    Err(e.with_file_context(path.as_str(), sql))
                }
            }
        }
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Utf8Path) -> Result<Vec<(String, Vec<Violation>)>> {
        // Get framework adapter from config
        let adapter: Arc<dyn MigrationAdapter> = match self.config.framework.as_str() {
            "diesel" => Arc::new(DieselAdapter),
            "sqlx" => Arc::new(SqlxAdapter),
            _ => {
                return Err(crate::error::DieselGuardError::parse_error(format!(
                    "Invalid framework: {}",
                    self.config.framework
                )));
            }
        };

        // Collect migration files using adapter
        let migration_files = adapter
            .collect_migration_files(
                dir,
                self.config.start_after.as_deref(),
                self.config.check_down,
            )
            .map_err(|e| crate::error::DieselGuardError::parse_error(e.to_string()))?;

        // Check each migration file
        let mut results = Vec::new();

        for mig_file in migration_files {
            let sql = fs::read_to_string(&mig_file.path)?;

            // Check for unsafe REINDEX patterns before parsing (sqlparser can't parse REINDEX)
            let mut violations = self.detect_reindex_violations(&sql);

            // Also check if ANY REINDEX exists (even if check is disabled)
            let has_any_reindex = !detect_reindex_violations(&sql).is_empty();

            // Parse with direction awareness only for marker-based files
            // (files that contain both up and down sections)
            // For regular files (separate up.sql/down.sql), just parse normally
            let use_direction_parsing =
                sql.contains("-- migrate:up") && sql.contains("-- migrate:down");

            let parse_result = if use_direction_parsing {
                self.parser
                    .parse_sql_with_direction(&sql, mig_file.direction)
            } else {
                self.parser.parse_with_metadata(&sql)
            };

            match parse_result {
                Ok(parsed) => {
                    violations.extend(self.registry.check_statements_with_context(
                        &parsed.statements,
                        &parsed.sql,
                        &parsed.ignore_ranges,
                    ));
                }
                Err(e) => {
                    // Parsing failed - if REINDEX is in the SQL, continue with violations
                    // Otherwise return the error
                    if violations.is_empty() && !has_any_reindex {
                        return Err(e.with_file_context(mig_file.path.as_str(), sql));
                    }
                    // Otherwise, we continue with just the REINDEX violations (may be empty if disabled)
                }
            }

            if !violations.is_empty() {
                results.push((mig_file.path.to_string(), violations));
            }
        }

        Ok(results)
    }

    /// Check a path (file or directory)
    pub fn check_path(&self, path: &Utf8Path) -> Result<Vec<(String, Vec<Violation>)>> {
        if path.is_dir() {
            self.check_directory(path)
        } else {
            let violations = self.check_file(path)?;
            if violations.is_empty() {
                Ok(vec![])
            } else {
                Ok(vec![(path.to_string(), violations)])
            }
        }
    }
}

impl Default for SafetyChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_safe_sql() {
        let checker = SafetyChecker::new();
        let sql = "ALTER TABLE users ADD COLUMN email VARCHAR(255);";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_check_unsafe_sql() {
        let checker = SafetyChecker::new();
        let sql = "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_with_disabled_checks() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string()],
            ..Default::default()
        };
        let checker = SafetyChecker::with_config(config);

        // This would normally trigger AddColumnCheck
        let sql = "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 0); // Check is disabled
    }

    #[test]
    fn test_reindex_without_concurrently_detected() {
        let checker = SafetyChecker::new();
        let sql = "REINDEX INDEX idx_users_email;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "REINDEX without CONCURRENTLY");
    }

    #[test]
    fn test_reindex_table_without_concurrently_detected() {
        let checker = SafetyChecker::new();
        let sql = "REINDEX TABLE users;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].operation, "REINDEX without CONCURRENTLY");
    }

    #[test]
    fn test_reindex_concurrently_safe() {
        let checker = SafetyChecker::new();
        let sql = "REINDEX INDEX CONCURRENTLY idx_users_email;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_reindex_check_can_be_disabled() {
        let config = Config {
            disable_checks: vec!["ReindexCheck".to_string()],
            ..Default::default()
        };
        let checker = SafetyChecker::with_config(config);

        let sql = "REINDEX INDEX idx_users_email;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 0); // Check is disabled
    }

    #[test]
    fn test_multiple_reindex_violations() {
        let checker = SafetyChecker::new();
        let sql = r#"
            REINDEX INDEX idx_users_email;
            REINDEX TABLE posts;
        "#;
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 2);
    }
}

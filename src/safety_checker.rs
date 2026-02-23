use crate::adapters::{DieselAdapter, MigrationAdapter, MigrationDirection, SqlxAdapter};
use crate::checks::Registry;
use crate::config::Config;
use crate::error::Result;
use crate::parser;
use crate::scripting;
use crate::violation::Violation;
use camino::Utf8Path;
use std::fs;

pub struct SafetyChecker {
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
        let mut registry = Registry::with_config(&config);

        if let Some(ref dir) = config.custom_checks_dir {
            let dir = Utf8Path::new(dir);
            if dir.exists() {
                let (checks, errors) = scripting::load_custom_checks(dir, &config);
                for err in errors {
                    eprintln!("Warning: {err}");
                }
                for check in checks {
                    registry.add_check(check);
                }
            }
        }

        // Warn about unknown check names.
        // We check against all built-in names (not just enabled ones) and all
        // custom script stems so that disabling a valid check doesn't trigger
        // a spurious warning.
        if !config.disable_checks.is_empty() {
            let builtin_names = Registry::builtin_check_names();
            let custom_names: Vec<String> = config
                .custom_checks_dir
                .as_deref()
                .and_then(|d| {
                    let dir = Utf8Path::new(d);
                    if dir.exists() {
                        std::fs::read_dir(dir).ok()
                    } else {
                        None
                    }
                })
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    let path = entry.ok()?.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();

            for name in &config.disable_checks {
                if !builtin_names.contains(&name.as_str())
                    && !custom_names.iter().any(|c| c == name)
                {
                    eprintln!("Warning: Unknown check name '{name}' in disable_checks. Run --list-checks to see available checks.");
                }
            }
        }

        Self { registry, config }
    }

    /// Build the migration adapter for the configured framework.
    fn adapter(&self) -> Result<Box<dyn MigrationAdapter>> {
        match self.config.framework.as_str() {
            "diesel" => Ok(Box::new(DieselAdapter)),
            "sqlx" => Ok(Box::new(SqlxAdapter)),
            _ => Err(crate::error::DieselGuardError::parse_error(format!(
                "Invalid framework: {}",
                self.config.framework
            ))),
        }
    }

    /// Check SQL string for violations
    pub fn check_sql(&self, sql: &str) -> Result<Vec<Violation>> {
        let parsed = parser::parse_with_metadata(sql)?;
        Ok(self.registry.check_stmts_with_context(
            &parsed.stmts,
            &parsed.sql,
            &parsed.ignore_ranges,
            &self.config,
        ))
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Utf8Path) -> Result<Vec<Violation>> {
        let adapter = self.adapter()?;
        let sql = fs::read_to_string(path)?;
        let sql_section = adapter.extract_sql_for_direction(&sql, MigrationDirection::Up);

        match parser::parse_with_metadata(sql_section) {
            Ok(parsed) => Ok(self.registry.check_stmts_with_context(
                &parsed.stmts,
                &parsed.sql,
                &parsed.ignore_ranges,
                &self.config,
            )),
            Err(e) => Err(e.with_file_context(path.as_str(), sql)),
        }
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Utf8Path) -> Result<Vec<(String, Vec<Violation>)>> {
        let adapter = self.adapter()?;

        let migration_files = adapter
            .collect_migration_files(
                dir,
                self.config.start_after.as_deref(),
                self.config.check_down,
            )
            .map_err(|e| crate::error::DieselGuardError::parse_error(e.to_string()))?;

        let mut results = Vec::new();

        for mig_file in migration_files {
            let sql = fs::read_to_string(&mig_file.path)?;
            let sql_section = adapter.extract_sql_for_direction(&sql, mig_file.direction);

            match parser::parse_with_metadata(sql_section) {
                Ok(parsed) => {
                    let violations = self.registry.check_stmts_with_context(
                        &parsed.stmts,
                        &parsed.sql,
                        &parsed.ignore_ranges,
                        &self.config,
                    );
                    if !violations.is_empty() {
                        results.push((mig_file.path.to_string(), violations));
                    }
                }
                Err(e) => {
                    return Err(e.with_file_context(mig_file.path.as_str(), sql));
                }
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

        let sql = "ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;";
        let violations = checker.check_sql(sql).unwrap();
        assert_eq!(violations.len(), 0);
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
        assert_eq!(violations.len(), 0);
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

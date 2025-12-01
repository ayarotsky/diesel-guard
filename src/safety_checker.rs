use crate::checks::CheckRegistry;
use crate::config::Config;
use crate::error::Result;
use crate::parser::SqlParser;
use crate::violation::Violation;
use std::fs;
use std::path::Path;

pub struct SafetyChecker {
    parser: SqlParser,
    registry: CheckRegistry,
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
            registry: CheckRegistry::with_config(&config),
            config,
        }
    }

    /// Check SQL string for violations
    pub fn check_sql(&self, sql: &str) -> Result<Vec<Violation>> {
        let statements = self.parser.parse(sql)?;
        Ok(self.registry.check_statements(&statements))
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Path) -> Result<Vec<Violation>> {
        let sql = fs::read_to_string(path)?;
        self.check_sql(&sql)
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Path) -> Result<Vec<(String, Vec<Violation>)>> {
        let mut results = vec![];

        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    // Extract directory name for timestamp filtering
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    // Skip if migration is before start_after threshold
                    if !self.config.should_check_migration(dir_name) {
                        continue;
                    }

                    // Check up.sql (always checked if migration passes filter)
                    let up_sql = path.join("up.sql");
                    if up_sql.exists() {
                        let violations = self.check_file(&up_sql)?;
                        if !violations.is_empty() {
                            results.push((up_sql.display().to_string(), violations));
                        }
                    }

                    // Check down.sql (only if check_down is enabled)
                    if self.config.check_down {
                        let down_sql = path.join("down.sql");
                        if down_sql.exists() {
                            let violations = self.check_file(&down_sql)?;
                            if !violations.is_empty() {
                                results.push((down_sql.display().to_string(), violations));
                            }
                        }
                    }
                } else if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                    // Individual SQL files (not in migration directories)
                    // These are always checked regardless of config
                    let violations = self.check_file(&path)?;
                    if !violations.is_empty() {
                        results.push((path.display().to_string(), violations));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Check a path (file or directory)
    pub fn check_path(&self, path: &Path) -> Result<Vec<(String, Vec<Violation>)>> {
        if path.is_dir() {
            self.check_directory(path)
        } else {
            let violations = self.check_file(path)?;
            if violations.is_empty() {
                Ok(vec![])
            } else {
                Ok(vec![(path.display().to_string(), violations)])
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
}

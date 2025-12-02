use crate::checks::CheckRegistry;
use crate::config::Config;
use crate::error::Result;
use crate::parser::SqlParser;
use crate::violation::Violation;
use camino::Utf8Path;
use std::fs;
use walkdir::WalkDir;

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
        let parsed = self.parser.parse_with_metadata(sql)?;

        let violations = self.registry.check_statements_with_context(
            &parsed.statements,
            &parsed.statement_lines,
            &parsed.ignore_ranges,
        );

        Ok(violations)
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Utf8Path) -> Result<Vec<Violation>> {
        let sql = fs::read_to_string(path)?;
        self.check_sql(&sql)
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Utf8Path) -> Result<Vec<(String, Vec<Violation>)>> {
        // Collect all files to check
        let files_to_check: Vec<_> = WalkDir::new(dir)
            .max_depth(1)
            .min_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .flat_map(|entry| {
                let path = entry.path();
                let path_utf8 = Utf8Path::from_path(path).expect("Path contains invalid UTF-8");

                let mut files = vec![];

                if entry.file_type().is_dir() {
                    // Extract directory name for timestamp filtering
                    let dir_name = path_utf8.file_name().unwrap_or("");

                    // Skip if migration is before start_after threshold
                    if !self.config.should_check_migration(dir_name) {
                        return files;
                    }

                    // Check up.sql (always checked if migration passes filter)
                    let up_sql = path_utf8.join("up.sql");
                    if up_sql.exists() {
                        files.push(up_sql);
                    }

                    // Check down.sql (only if check_down is enabled)
                    if self.config.check_down {
                        let down_sql = path_utf8.join("down.sql");
                        if down_sql.exists() {
                            files.push(down_sql);
                        }
                    }
                } else if path_utf8.extension() == Some("sql") {
                    // Individual SQL files (not in migration directories)
                    // These are always checked regardless of config
                    files.push(path_utf8.to_owned());
                }

                files
            })
            .collect();

        // Check files in parallel using rayon
        files_to_check
            .iter()
            .map(|file_path| {
                let violations = self.check_file(file_path)?;
                if violations.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some((file_path.to_string(), violations)))
                }
            })
            .collect::<Result<Vec<_>>>()
            .map(|results| results.into_iter().flatten().collect())
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
}

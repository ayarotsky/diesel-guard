use crate::checks::Registry;
use crate::config::Config;
use crate::error::Result;
use crate::parser::SqlParser;
use crate::violation::Violation;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use walkdir::WalkDir;

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
        let parsed = self.parser.parse_with_metadata(sql)?;

        let violations = self.registry.check_statements_with_context(
            &parsed.statements,
            &parsed.sql,
            &parsed.ignore_ranges,
        );

        Ok(violations)
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Utf8Path) -> Result<Vec<Violation>> {
        let sql = fs::read_to_string(path)?;
        self.check_sql(&sql)
            .map_err(|e| e.with_file_context(path.as_str(), sql.clone()))
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Utf8Path) -> Result<Vec<(String, Vec<Violation>)>> {
        let files_to_check = self.collect_files(dir);

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

    /// Collect all SQL files to check from a directory
    fn collect_files(&self, dir: &Utf8Path) -> Vec<Utf8PathBuf> {
        // Collect and sort directory entries
        let mut entries: Vec<_> = WalkDir::new(dir)
            .max_depth(1)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        entries.sort_by(|a, b| a.path().cmp(b.path()));

        // Process each entry
        entries
            .into_iter()
            .flat_map(|entry| {
                let Some(path) = Utf8Path::from_path(entry.path()) else {
                    return vec![];
                };

                if entry.file_type().is_dir() {
                    self.process_migration_directory(path)
                } else if path.extension() == Some("sql") {
                    vec![path.to_owned()]
                } else {
                    vec![]
                }
            })
            .collect()
    }

    /// Process a migration directory and return SQL files to check
    fn process_migration_directory(&self, path: &Utf8Path) -> Vec<Utf8PathBuf> {
        let dir_name = match path.file_name() {
            Some(name) => name,
            None => return vec![],
        };

        // Skip if migration is before start_after threshold
        if !self.config.should_check_migration(dir_name) {
            return vec![];
        }

        let mut files = vec![];

        // Always check up.sql if it exists
        let up_sql = path.join("up.sql");
        if up_sql.exists() {
            files.push(up_sql);
        }

        // Check down.sql only if enabled in config
        if self.config.check_down {
            let down_sql = path.join("down.sql");
            if down_sql.exists() {
                files.push(down_sql);
            }
        }

        files
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

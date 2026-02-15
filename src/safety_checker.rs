use crate::adapters::{DieselAdapter, MigrationAdapter, MigrationFile, SqlxAdapter};
use crate::checks::pg_helpers::{extract_node, NodeEnum};
use crate::checks::Registry;
use crate::config::Config;
use crate::error::Result;
use crate::parser;
use crate::scripting;
use crate::violation::Violation;
use camino::Utf8Path;
use pg_query::protobuf::{ObjectType, RawStmt};
use std::fs;

/// Check if any parsed statements use CONCURRENTLY operations.
///
/// Detects:
/// - `CREATE INDEX CONCURRENTLY` (IndexStmt with concurrent=true)
/// - `DROP INDEX CONCURRENTLY` (DropStmt with concurrent=true and ObjectIndex)
/// - `REINDEX ... CONCURRENTLY` (ReindexStmt with DefElem "concurrently" in params)
fn has_concurrently_operations(stmts: &[RawStmt]) -> bool {
    stmts.iter().any(|raw_stmt| {
        let Some(node) = extract_node(raw_stmt) else {
            return false;
        };
        match node {
            NodeEnum::IndexStmt(stmt) => stmt.concurrent,
            NodeEnum::DropStmt(stmt) => {
                stmt.remove_type == ObjectType::ObjectIndex as i32 && stmt.concurrent
            }
            NodeEnum::ReindexStmt(stmt) => stmt.params.iter().any(|p| {
                matches!(&p.node, Some(NodeEnum::DefElem(elem)) if elem.defname == "concurrently")
            }),
            _ => false,
        }
    })
}

/// Emit a warning if a migration uses CONCURRENTLY but runs in a transaction.
fn warn_concurrently_in_transaction(mig_file: &MigrationFile, framework: &str) {
    if mig_file.requires_no_transaction {
        return;
    }
    let hint = if framework == "diesel" {
        "Create metadata.toml with `run_in_transaction = false`"
    } else {
        "Add `-- no-transaction` directive at the start of the file"
    };
    eprintln!(
        "Warning: {} uses CONCURRENTLY but migration runs in a transaction",
        mig_file.path
    );
    eprintln!("         {hint}");
}

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

        Self { registry, config }
    }

    /// Check SQL string for violations
    pub fn check_sql(&self, sql: &str) -> Result<Vec<Violation>> {
        let parsed = parser::parse_with_metadata(sql)?;
        Ok(self.registry.check_stmts_with_context(
            &parsed.stmts,
            &parsed.sql,
            &parsed.ignore_ranges,
        ))
    }

    /// Check a single migration file
    pub fn check_file(&self, path: &Utf8Path) -> Result<Vec<Violation>> {
        use crate::adapters::MigrationDirection;

        let sql = fs::read_to_string(path)?;

        // If the file contains marker-based sections, only check the "up" section
        let sql_lower = sql.to_lowercase();
        let has_markers =
            sql_lower.contains("-- migrate:up") && sql_lower.contains("-- migrate:down");

        let parse_result = if has_markers {
            parser::parse_sql_with_direction(&sql, MigrationDirection::Up)
        } else {
            parser::parse_with_metadata(&sql)
        };

        match parse_result {
            Ok(parsed) => Ok(self.registry.check_stmts_with_context(
                &parsed.stmts,
                &parsed.sql,
                &parsed.ignore_ranges,
            )),
            Err(e) => Err(e.with_file_context(path.as_str(), sql)),
        }
    }

    /// Check all migration files in a directory
    pub fn check_directory(&self, dir: &Utf8Path) -> Result<Vec<(String, Vec<Violation>)>> {
        let adapter: Box<dyn MigrationAdapter> = match self.config.framework.as_str() {
            "diesel" => Box::new(DieselAdapter),
            "sqlx" => Box::new(SqlxAdapter),
            _ => {
                return Err(crate::error::DieselGuardError::parse_error(format!(
                    "Invalid framework: {}",
                    self.config.framework
                )));
            }
        };

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

            let sql_lower = sql.to_lowercase();
            let use_direction_parsing = self.config.framework == "sqlx"
                && sql_lower.contains("-- migrate:up")
                && sql_lower.contains("-- migrate:down");

            let parse_result = if use_direction_parsing {
                parser::parse_sql_with_direction(&sql, mig_file.direction)
            } else {
                parser::parse_with_metadata(&sql)
            };

            match parse_result {
                Ok(parsed) => {
                    if has_concurrently_operations(&parsed.stmts) {
                        warn_concurrently_in_transaction(&mig_file, &self.config.framework);
                    }
                    let violations = self.registry.check_stmts_with_context(
                        &parsed.stmts,
                        &parsed.sql,
                        &parsed.ignore_ranges,
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

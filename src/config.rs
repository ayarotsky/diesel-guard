//! Configuration file parsing and validation.
//!
//! This module handles loading and validating diesel-guard.toml configuration files.

use camino::{Utf8Path, Utf8PathBuf};
use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Generate help text for invalid check names from the registry
fn valid_check_names_help() -> String {
    format!(
        "Valid check names: {}",
        crate::checks::Registry::builtin_check_names().join(", ")
    )
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse config file")]
    ParseError(#[from] toml::de::Error),

    #[error("Invalid check name: {invalid_name}")]
    InvalidCheckName { invalid_name: String },

    #[error("Invalid timestamp format: {0}")]
    InvalidTimestampFormat(String),

    #[error("Missing required field 'framework' in diesel-guard.toml")]
    MissingFramework,

    #[error("Invalid framework \"{framework}\". Expected \"diesel\" or \"sqlx\".")]
    InvalidFramework { framework: String },

    #[error("enable_checks and disable_checks cannot both be set")]
    ConflictingCheckConfig,
}

impl Diagnostic for ConfigError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        match self {
            Self::IoError(_) => Some(Box::new("diesel_guard::config::io_error")),
            Self::ParseError(_) => Some(Box::new("diesel_guard::config::parse_error")),
            Self::InvalidCheckName { .. } => Some(Box::new("diesel_guard::config::invalid_check")),
            Self::InvalidTimestampFormat(_) => {
                Some(Box::new("diesel_guard::config::invalid_timestamp"))
            }
            Self::MissingFramework => Some(Box::new("diesel_guard::config::missing_framework")),
            Self::InvalidFramework { .. } => {
                Some(Box::new("diesel_guard::config::invalid_framework"))
            }
            Self::ConflictingCheckConfig => {
                Some(Box::new("diesel_guard::config::conflicting_check_config"))
            }
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        match self {
            Self::InvalidCheckName { .. } => Some(Box::new(valid_check_names_help())),
            Self::InvalidTimestampFormat(_) => Some(Box::new(
                "Expected format: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS (e.g., 20240101000000, 2024_01_01_000000, or 2024-01-01-000000)",
            )),
            Self::MissingFramework => Some(Box::new(
                "Add one of the following to your diesel-guard.toml file:\n  framework = \"diesel\"\n  framework = \"sqlx\"",
            )),
            Self::InvalidFramework { .. } => Some(Box::new("Valid values: \"diesel\", \"sqlx\"")),
            Self::ConflictingCheckConfig => Some(Box::new(
                "Use either enable_checks (whitelist) or disable_checks (blacklist), not both.",
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Migration framework being used (required)
    ///
    /// Valid values: "diesel" or "sqlx"
    ///
    /// This field is required and must be explicitly set in diesel-guard.toml
    pub framework: String,

    /// Skip migrations before this timestamp
    ///
    /// Accepts multiple formats: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS
    ///
    /// Examples: "20240101000000", "2024_01_01_000000", or "2024-01-01-000000"
    ///
    /// Note: Timestamps are normalized automatically during comparison
    #[serde(default)]
    pub start_after: Option<String>,

    /// Whether to check down.sql/down migrations in addition to up.sql/up migrations
    #[serde(default)]
    pub check_down: bool,

    /// List of check struct names to disable
    #[serde(default)]
    pub disable_checks: Vec<String>,

    /// List of check names to run exclusively (whitelist). Cannot be used with disable_checks.
    #[serde(default)]
    pub enable_checks: Vec<String>,

    /// List of check names to treat as warnings instead of errors.
    ///
    /// Warnings are reported in output but do not cause a non-zero exit code.
    /// Useful for checks like `TruncateTableCheck` that may be acceptable in
    /// specific contexts (test setup, maintenance windows, seeding data).
    #[serde(default)]
    pub warn_checks: Vec<String>,

    /// Directory containing custom Rhai check scripts (.rhai files)
    #[serde(default)]
    pub custom_checks_dir: Option<String>,

    /// Target Postgres major version (e.g., 11, 14, 16).
    /// When set, checks that are safe from that version onward are skipped.
    #[serde(default)]
    pub postgres_version: Option<u32>,
}

impl Config {
    /// Load config from diesel-guard.toml in current directory
    /// Returns default config if file doesn't exist
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Utf8PathBuf::from("diesel-guard.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        Self::load_from_path(&config_path)
    }

    /// Load config from specific path (useful for testing)
    pub fn load_from_path(path: &Utf8Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents).map_err(|e| {
            // Check if the error is due to missing framework field
            if e.to_string().contains("missing field `framework`") {
                ConfigError::MissingFramework
            } else {
                ConfigError::ParseError(e)
            }
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate framework field
        match self.framework.as_str() {
            "diesel" | "sqlx" => {}
            _ => {
                return Err(ConfigError::InvalidFramework {
                    framework: self.framework.clone(),
                });
            }
        }

        // Timestamp validation is framework-specific and done by adapters
        // during migration file collection

        if !self.enable_checks.is_empty() && !self.disable_checks.is_empty() {
            return Err(ConfigError::ConflictingCheckConfig);
        }

        Ok(())
    }

    /// Return true if the given check should produce warnings instead of errors
    pub fn is_check_warning(&self, check_name: &str) -> bool {
        self.warn_checks.iter().any(|c| c == check_name)
    }

    /// Check if a specific check is enabled
    pub fn is_check_enabled(&self, check_name: &str) -> bool {
        if !self.enable_checks.is_empty() {
            return self.enable_checks.iter().any(|c| c == check_name);
        }
        !self.disable_checks.iter().any(|c| c == check_name)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            framework: "diesel".to_string(),
            start_after: None,
            check_down: false,
            disable_checks: Vec::new(),
            enable_checks: Vec::new(),
            warn_checks: Vec::new(),
            custom_checks_dir: None,
            postgres_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.start_after, None);
        assert!(!config.check_down);
        assert_eq!(config.disable_checks.len(), 0);
    }

    #[test]
    fn test_is_check_enabled() {
        let config = Config {
            disable_checks: vec!["AddColumnCheck".to_string(), "DropColumnCheck".to_string()],
            ..Default::default()
        };

        assert!(!config.is_check_enabled("AddColumnCheck"));
        assert!(!config.is_check_enabled("DropColumnCheck"));
        assert!(config.is_check_enabled("AddIndexCheck"));
        assert!(config.is_check_enabled("AddNotNullCheck"));
    }

    #[test]
    fn test_valid_check_names() {
        let config_str = r#"
            framework = "diesel"
            disable_checks = ["AddColumnCheck", "DropColumnCheck"]
        "#;

        let config: Config = toml::from_str(config_str).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_check_name_help_includes_all_checks() {
        use miette::Diagnostic;

        let error = ConfigError::InvalidCheckName {
            invalid_name: "FooCheck".to_string(),
        };

        let help = error.help().unwrap().to_string();

        // Verify help text includes all check names from the registry
        for &check_name in crate::checks::Registry::builtin_check_names() {
            assert!(
                help.contains(check_name),
                "Help text should include '{}', got: {}",
                check_name,
                help
            );
        }

        // Verify format
        assert!(help.starts_with("Valid check names: "));
    }

    #[test]
    fn test_load_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("diesel-guard.toml");

        fs::write(
            &config_path,
            r#"
framework = "diesel"
start_after = "2024_01_01_000000"
check_down = true
disable_checks = ["AddColumnCheck"]
            "#,
        )
        .unwrap();

        let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
        let config = Config::load_from_path(config_path_utf8).unwrap();
        assert_eq!(config.framework, "diesel");
        assert_eq!(config.start_after, Some("2024_01_01_000000".to_string()));
        assert!(config.check_down);
        assert_eq!(config.disable_checks, vec!["AddColumnCheck".to_string()]);
    }

    #[test]
    fn test_valid_diesel_framework() {
        let config = Config {
            framework: "diesel".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_valid_sqlx_framework() {
        let config = Config {
            framework: "sqlx".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_framework_value() {
        let config = Config {
            framework: "rails".to_string(),
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidFramework { .. }));
    }

    #[test]
    fn test_framework_case_sensitive() {
        let config = Config {
            framework: "Diesel".to_string(),
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidFramework { .. }));
    }

    #[test]
    fn test_missing_framework_field_errors() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("diesel-guard.toml");

        // Config file without framework field
        fs::write(
            &config_path,
            r#"
start_after = "2024_01_01_000000"
check_down = true
            "#,
        )
        .unwrap();

        let config_path_utf8 = Utf8Path::from_path(&config_path).unwrap();
        let err = Config::load_from_path(config_path_utf8).unwrap_err();
        assert!(matches!(err, ConfigError::MissingFramework));
    }

    #[test]
    fn test_default_config_has_valid_framework() {
        let config = Config::default();
        assert_eq!(config.framework, "diesel");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_postgres_version_loads_from_toml() {
        let config: Config = toml::from_str(
            r#"
framework = "diesel"
postgres_version = 14
            "#,
        )
        .unwrap();
        assert_eq!(config.postgres_version, Some(14));
    }

    #[test]
    fn test_postgres_version_defaults_to_none() {
        let config: Config = toml::from_str(r#"framework = "diesel""#).unwrap();
        assert_eq!(config.postgres_version, None);
    }

    // --- Diagnostic::code() tests — one per variant ---

    #[test]
    fn test_diagnostic_code_io_error() {
        use miette::Diagnostic;
        let err = ConfigError::IoError(std::io::Error::other("oops"));
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::io_error");
    }

    #[test]
    fn test_diagnostic_code_parse_error() {
        use miette::Diagnostic;
        let toml_err = toml::from_str::<Config>("not_valid_toml = [[[").unwrap_err();
        let err = ConfigError::ParseError(toml_err);
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::parse_error");
    }

    #[test]
    fn test_diagnostic_code_invalid_check_name() {
        use miette::Diagnostic;
        let err = ConfigError::InvalidCheckName {
            invalid_name: "Bogus".to_string(),
        };
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::invalid_check");
    }

    #[test]
    fn test_diagnostic_code_invalid_timestamp_format() {
        use miette::Diagnostic;
        let err = ConfigError::InvalidTimestampFormat("bad-ts".to_string());
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::invalid_timestamp");
    }

    #[test]
    fn test_diagnostic_code_missing_framework() {
        use miette::Diagnostic;
        let err = ConfigError::MissingFramework;
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::missing_framework");
    }

    #[test]
    fn test_diagnostic_code_invalid_framework() {
        use miette::Diagnostic;
        let err = ConfigError::InvalidFramework {
            framework: "rails".to_string(),
        };
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::invalid_framework");
    }

    // --- Diagnostic::help() tests ---

    #[test]
    fn test_diagnostic_help_invalid_timestamp_format() {
        use miette::Diagnostic;
        let err = ConfigError::InvalidTimestampFormat("bad".to_string());
        let help = err.help().unwrap().to_string();
        assert_eq!(
            help,
            "Expected format: YYYYMMDDHHMMSS, YYYY_MM_DD_HHMMSS, or YYYY-MM-DD-HHMMSS (e.g., 20240101000000, 2024_01_01_000000, or 2024-01-01-000000)"
        );
    }

    #[test]
    fn test_diagnostic_help_missing_framework() {
        use miette::Diagnostic;
        let err = ConfigError::MissingFramework;
        let help = err.help().unwrap().to_string();
        assert_eq!(
            help,
            "Add one of the following to your diesel-guard.toml file:\n  framework = \"diesel\"\n  framework = \"sqlx\""
        );
    }

    #[test]
    fn test_diagnostic_help_invalid_framework() {
        use miette::Diagnostic;
        let err = ConfigError::InvalidFramework {
            framework: "mongo".to_string(),
        };
        let help = err.help().unwrap().to_string();
        assert_eq!(help, "Valid values: \"diesel\", \"sqlx\"");
    }

    #[test]
    fn test_diagnostic_help_returns_none_for_io_error() {
        use miette::Diagnostic;
        let err = ConfigError::IoError(std::io::Error::other("disk full"));
        assert!(err.help().is_none());
    }

    #[test]
    fn test_diagnostic_help_returns_none_for_parse_error() {
        use miette::Diagnostic;
        let toml_err = toml::from_str::<Config>("bad = [[[").unwrap_err();
        let err = ConfigError::ParseError(toml_err);
        assert!(err.help().is_none());
    }

    #[test]
    fn test_is_check_enabled_with_whitelist() {
        let config = Config {
            enable_checks: vec!["AddIndexCheck".to_string(), "AddNotNullCheck".to_string()],
            ..Default::default()
        };

        assert!(config.is_check_enabled("AddIndexCheck"));
        assert!(config.is_check_enabled("AddNotNullCheck"));
        assert!(!config.is_check_enabled("AddColumnCheck"));
        assert!(!config.is_check_enabled("DropColumnCheck"));
    }

    #[test]
    fn test_enable_and_disable_checks_conflict() {
        let config = Config {
            enable_checks: vec!["AddIndexCheck".to_string()],
            disable_checks: vec!["DropColumnCheck".to_string()],
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::ConflictingCheckConfig));
    }

    #[test]
    fn test_enable_checks_loads_from_toml() {
        let config: Config = toml::from_str(
            r#"
framework = "diesel"
enable_checks = ["AddIndexCheck", "AddNotNullCheck"]
            "#,
        )
        .unwrap();
        assert_eq!(
            config.enable_checks,
            vec!["AddIndexCheck".to_string(), "AddNotNullCheck".to_string()]
        );
    }

    #[test]
    fn test_diagnostic_code_conflicting_check_config() {
        use miette::Diagnostic;
        let err = ConfigError::ConflictingCheckConfig;
        let code = err.code().unwrap().to_string();
        assert_eq!(code, "diesel_guard::config::conflicting_check_config");
    }

    #[test]
    fn test_diagnostic_help_conflicting_check_config() {
        use miette::Diagnostic;
        let err = ConfigError::ConflictingCheckConfig;
        let help = err.help().unwrap().to_string();
        assert_eq!(
            help,
            "Use either enable_checks (whitelist) or disable_checks (blacklist), not both."
        );
    }

    #[test]
    fn test_is_check_warning() {
        let config = Config {
            warn_checks: vec!["TruncateTableCheck".to_string()],
            ..Default::default()
        };

        assert!(config.is_check_warning("TruncateTableCheck"));
        assert!(!config.is_check_warning("DropTableCheck"));
        assert!(!config.is_check_warning("AddColumnCheck"));
    }

    #[test]
    fn test_warn_checks_loads_from_toml() {
        let config: Config = toml::from_str(
            r#"
framework = "diesel"
warn_checks = ["TruncateTableCheck", "DropTableCheck"]
            "#,
        )
        .unwrap();
        assert_eq!(
            config.warn_checks,
            vec![
                "TruncateTableCheck".to_string(),
                "DropTableCheck".to_string()
            ]
        );
    }

    #[test]
    fn test_warn_checks_default_is_empty() {
        let config = Config::default();
        assert!(config.warn_checks.is_empty());
    }
}

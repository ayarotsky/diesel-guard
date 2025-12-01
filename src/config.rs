//! Configuration file parsing and validation.
//!
//! This module handles loading and validating diesel-guard.toml configuration files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Invalid check name: {0}. Valid names: AddColumnCheck, AddIndexCheck, AddNotNullCheck, AlterColumnTypeCheck, DropColumnCheck")]
    InvalidCheckName(String),

    #[error("Invalid timestamp format: {0}. Expected format: YYYY_MM_DD_HHMMSS (e.g., 2024_01_01_000000)")]
    InvalidTimestampFormat(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Skip migrations before this timestamp
    /// Format: YYYY_MM_DD_HHMMSS (e.g., "2024_01_01_000000")
    #[serde(default)]
    pub start_after: Option<String>,

    /// Whether to check down.sql files in addition to up.sql
    #[serde(default)]
    pub check_down: bool,

    /// List of check struct names to disable
    #[serde(default)]
    pub disable_checks: Vec<String>,
}

impl Config {
    /// Load config from diesel-guard.toml in current directory
    /// Returns default config if file doesn't exist
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = std::path::PathBuf::from("diesel-guard.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        Self::load_from_path(&config_path)
    }

    /// Load config from specific path (useful for testing)
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate timestamp format if present
        if let Some(ref timestamp) = self.start_after {
            Self::validate_timestamp(timestamp)?;
        }

        // Validate check names
        const VALID_CHECKS: &[&str] = &[
            "AddColumnCheck",
            "AddIndexCheck",
            "AddNotNullCheck",
            "AlterColumnTypeCheck",
            "DropColumnCheck",
        ];

        for check_name in &self.disable_checks {
            if !VALID_CHECKS.contains(&check_name.as_str()) {
                return Err(ConfigError::InvalidCheckName(check_name.clone()));
            }
        }

        Ok(())
    }

    /// Validate timestamp format: YYYY_MM_DD_HHMMSS
    fn validate_timestamp(timestamp: &str) -> Result<(), ConfigError> {
        // Expected format: 2024_01_01_000000 (17 chars: 4+1+2+1+2+1+6)
        if timestamp.len() != 17 {
            return Err(ConfigError::InvalidTimestampFormat(timestamp.to_string()));
        }

        // Validate underscores in correct positions (indices 4, 7, 10)
        let chars: Vec<char> = timestamp.chars().collect();
        if chars[4] != '_' || chars[7] != '_' || chars[10] != '_' {
            return Err(ConfigError::InvalidTimestampFormat(timestamp.to_string()));
        }

        // Validate all parts are digits (YYYY, MM, DD, HHMMSS)
        let parts: Vec<&str> = timestamp.split('_').collect();
        if parts.len() != 4 {
            return Err(ConfigError::InvalidTimestampFormat(timestamp.to_string()));
        }

        // Validate part lengths: YYYY (4), MM (2), DD (2), HHMMSS (6)
        if parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 || parts[3].len() != 6
        {
            return Err(ConfigError::InvalidTimestampFormat(timestamp.to_string()));
        }

        for part in parts {
            if !part.chars().all(|c| c.is_ascii_digit()) {
                return Err(ConfigError::InvalidTimestampFormat(timestamp.to_string()));
            }
        }

        Ok(())
    }

    /// Check if a specific check is enabled
    pub fn is_check_enabled(&self, check_name: &str) -> bool {
        !self.disable_checks.contains(&check_name.to_string())
    }

    /// Check if migration should be checked based on start_after
    /// Returns true if migration timestamp is AFTER start_after (or if no filter set)
    pub fn should_check_migration(&self, migration_dir_name: &str) -> bool {
        if let Some(ref start_after) = self.start_after {
            // Extract timestamp from migration directory name
            // Format: YYYY_MM_DD_HHMMSS_description
            if migration_dir_name.len() >= 17 {
                let migration_timestamp = &migration_dir_name[..17];
                // Lexicographic comparison works for this timestamp format
                // Returns true only if migration_timestamp > start_after (strictly after)
                return migration_timestamp > start_after.as_str();
            }
        }
        true // Check by default if no filter or if dir name too short
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
    fn test_valid_timestamp() {
        assert!(Config::validate_timestamp("2024_01_01_000000").is_ok());
        assert!(Config::validate_timestamp("2023_12_31_235959").is_ok());
        assert!(Config::validate_timestamp("2025_06_15_120000").is_ok());
    }

    #[test]
    fn test_invalid_timestamp_format() {
        // Wrong separators
        assert!(Config::validate_timestamp("2024-01-01-000000").is_err());
        assert!(Config::validate_timestamp("20240101000000").is_err());

        // Wrong length
        assert!(Config::validate_timestamp("2024_01_01").is_err());
        assert!(Config::validate_timestamp("2024_01_01_00000").is_err());

        // Non-numeric characters
        assert!(Config::validate_timestamp("202a_01_01_000000").is_err());
    }

    #[test]
    fn test_should_check_migration_no_filter() {
        let config = Config::default();
        assert!(config.should_check_migration("2024_01_01_000000_create_users"));
        assert!(config.should_check_migration("2020_01_01_000000_old_migration"));
    }

    #[test]
    fn test_should_check_migration_with_filter() {
        let config = Config {
            start_after: Some("2024_01_01_000000".to_string()),
            ..Default::default()
        };

        // Should check (strictly after threshold)
        assert!(config.should_check_migration("2024_01_02_000000_new_migration"));
        assert!(config.should_check_migration("2024_06_15_120000_another_migration"));

        // Should NOT check (before or equal to threshold)
        assert!(!config.should_check_migration("2024_01_01_000000_exact_match"));
        assert!(!config.should_check_migration("2023_12_31_235959_old_migration"));
        assert!(!config.should_check_migration("2020_01_01_000000_very_old"));
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
    fn test_invalid_check_name() {
        let config_str = r#"
            disable_checks = ["InvalidCheckName"]
        "#;

        let config: Config = toml::from_str(config_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_valid_check_names() {
        let config_str = r#"
            disable_checks = ["AddColumnCheck", "DropColumnCheck"]
        "#;

        let config: Config = toml::from_str(config_str).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_load_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("diesel-guard.toml");

        fs::write(
            &config_path,
            r#"
start_after = "2024_01_01_000000"
check_down = true
disable_checks = ["AddColumnCheck"]
            "#,
        )
        .unwrap();

        let config = Config::load_from_path(&config_path).unwrap();
        assert_eq!(config.start_after, Some("2024_01_01_000000".to_string()));
        assert!(config.check_down);
        assert_eq!(config.disable_checks, vec!["AddColumnCheck".to_string()]);
    }
}

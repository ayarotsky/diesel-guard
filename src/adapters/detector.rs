//! Framework auto-detection logic.

use super::{DieselAdapter, MigrationAdapter, SqlxAdapter};
use camino::Utf8Path;
use std::sync::Arc;

/// Framework detector for automatic framework detection.
pub struct FrameworkDetector;

impl FrameworkDetector {
    /// Auto-detect framework from migrations directory.
    ///
    /// Scores each framework's detection method and returns the adapter
    /// with the highest confidence score. Defaults to Diesel if scores are tied.
    pub fn detect(path: &Utf8Path) -> Arc<dyn MigrationAdapter> {
        let diesel_score = DieselAdapter::detect(path).unwrap_or(0);
        let sqlx_score = SqlxAdapter::detect(path).unwrap_or(0);

        if sqlx_score > diesel_score {
            Arc::new(SqlxAdapter)
        } else {
            // Default to Diesel on tie or if both score 0
            Arc::new(DieselAdapter)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_sqlx_higher_score() {
        // Create a directory with SQLx-specific patterns (.up.sql files)
        let temp_dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create a SQLx-style migration file
        std::fs::write(
            temp_dir.path().join("20240101000000_test.up.sql"),
            "CREATE TABLE users (id SERIAL PRIMARY KEY);",
        )
        .unwrap();

        let adapter = FrameworkDetector::detect(path);
        assert_eq!(adapter.name(), "SQLx");
    }

    #[test]
    fn test_detect_diesel_higher_score() {
        // Create a directory with Diesel-specific patterns (directory-based)
        let temp_dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create a Diesel-style migration directory
        let migration_dir = temp_dir.path().join("2024_01_01_000000_test");
        std::fs::create_dir(&migration_dir).unwrap();
        std::fs::write(
            migration_dir.join("up.sql"),
            "CREATE TABLE users (id SERIAL PRIMARY KEY);",
        )
        .unwrap();

        let adapter = FrameworkDetector::detect(path);
        assert_eq!(adapter.name(), "Diesel");
    }

    #[test]
    fn test_detect_tie_scores_defaults_to_diesel() {
        // Create a directory that could match either pattern equally
        // In practice, this is rare, but we test the tie-breaking behavior
        let temp_dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Empty directory - both adapters score 0, which is a tie
        let adapter = FrameworkDetector::detect(path);
        assert_eq!(
            adapter.name(),
            "Diesel",
            "Should default to Diesel on tie scores"
        );
    }

    #[test]
    fn test_detect_both_scores_zero_defaults_to_diesel() {
        // Create an empty directory with no migration patterns
        let temp_dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let adapter = FrameworkDetector::detect(path);
        assert_eq!(
            adapter.name(),
            "Diesel",
            "Should default to Diesel when both scores are 0"
        );
    }

    #[test]
    fn test_detect_nonexistent_path_defaults_to_diesel() {
        // Test with a path that doesn't exist
        let path = Utf8Path::new("/nonexistent/path/that/does/not/exist");

        let adapter = FrameworkDetector::detect(path);
        assert_eq!(
            adapter.name(),
            "Diesel",
            "Should default to Diesel for nonexistent paths"
        );
    }

    #[test]
    fn test_detect_sqlx_with_sqlx_directory() {
        // Create a directory with .sqlx subdirectory (strong SQLx signal)
        let temp_dir = TempDir::new().unwrap();
        let path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create .sqlx directory (SQLx CLI metadata directory)
        std::fs::create_dir(temp_dir.path().join(".sqlx")).unwrap();

        let adapter = FrameworkDetector::detect(path);
        assert_eq!(
            adapter.name(),
            "SQLx",
            ".sqlx directory should strongly indicate SQLx"
        );
    }
}

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

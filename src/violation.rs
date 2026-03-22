use derive_more::Display;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl Default for Severity {
    fn default() -> Self {
        Self::Error
    }
}

#[derive(Debug, Clone, Serialize, Display)]
#[display("{}: {}", operation, problem)]
pub struct Violation {
    pub operation: String,
    pub problem: String,
    pub safe_alternative: String,
    #[serde(default)]
    pub severity: Severity,
}

impl Violation {
    pub fn new(
        operation: impl Into<String>,
        problem: impl Into<String>,
        safe_alternative: impl Into<String>,
    ) -> Self {
        Self {
            operation: operation.into(),
            problem: problem.into(),
            safe_alternative: safe_alternative.into(),
            severity: Severity::Error,
        }
    }

    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

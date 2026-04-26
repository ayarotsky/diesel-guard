use derive_more::Display;
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Display)]
#[display("{}: {}", operation, problem)]
pub struct Violation {
    pub check_name: String,
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
            check_name: String::new(),
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

    #[must_use]
    pub fn with_check_name(mut self, name: &str) -> Self {
        self.check_name = name.to_string();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_has_empty_check_name() {
        let v = Violation::new("op", "prob", "alt");
        assert_eq!(v.check_name, "");
    }

    #[test]
    fn test_with_check_name() {
        let v = Violation::new("op", "prob", "alt").with_check_name("AddIndexCheck");
        assert_eq!(v.check_name, "AddIndexCheck");
    }
}

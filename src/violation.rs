use derive_more::Display;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Display)]
#[display("{}: {}", operation, problem)]
pub struct Violation {
    pub operation: &'static str,
    pub problem: String,
    pub safe_alternative: String,
}

impl Violation {
    pub fn new(
        operation: &'static str,
        problem: impl Into<String>,
        safe_alternative: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            problem: problem.into(),
            safe_alternative: safe_alternative.into(),
        }
    }
}

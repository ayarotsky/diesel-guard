use derive_more::Display;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Display)]
#[display("{}: {}", operation, problem)]
pub struct Violation {
    pub operation: String,
    pub problem: String,
    pub safe_alternative: String,
    pub check_name: &'static str,
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
            check_name: "",
        }
    }

    pub fn with_check_name(mut self, name: &'static str) -> Self {
        self.check_name = name;
        self
    }
}

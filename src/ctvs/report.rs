use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationReport {
    pub test_id: String,
    pub description: String,
    pub status: ValidationStatus,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationStatus {
    Pass,
    Fail,
    Inconclusive,
    NotEstimable,
    NotApplicable,
}

impl ValidationReport {
    pub fn new(test_id: &str, description: &str) -> Self {
        ValidationReport {
            test_id: test_id.to_string(),
            description: description.to_string(),
            status: ValidationStatus::Inconclusive,
            details: None,
        }
    }

    pub fn pass(mut self) -> Self {
        self.status = ValidationStatus::Pass;
        self
    }

    pub fn fail(mut self, details: &str) -> Self {
        self.status = ValidationStatus::Fail;
        self.details = Some(details.to_string());
        self
    }
}


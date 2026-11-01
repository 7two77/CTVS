use serde::{Deserialize, Serialize};

/// Decision layer — ЕДИНСТВЕННЫЙ модуль, который может
/// принимать решения на основе predictive validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Decision {
    NoTrade,
    EnterLong,
    EnterShort,
    ExitPosition,
}

/// Policy outcome — результат решения
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyOutcome {
    pub decision: Decision,
    pub confidence: f64,
    pub state: String,
}

impl PolicyOutcome {
    pub fn new(decision: Decision, confidence: f64, state: &str) -> Self {
        PolicyOutcome {
            decision,
            confidence,
            state: state.to_string(),
        }
    }
}

/// ПРОВЕРКА ИЗОЛЯЦИИ:
/// Этот модуль не может быть вызван из StateEstimator,
/// TransitionModel или PredictiveValidation.
///
/// Архитектурный firewall:
/// State → Predictive → Policy → Execution
///   ↑                         │
///   └──────── NO FEEDBACK ────┘
pub trait PolicyIsolated {
    fn decision(&self) -> Decision;
}

impl PolicyIsolated for PolicyOutcome {
    fn decision(&self) -> Decision {
        self.decision.clone()
    }
}


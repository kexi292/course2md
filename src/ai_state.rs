//! Shared AI request state and retry policy.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Completed,
    Failed,
    Uncertain,
    Skipped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryDecision {
    Automatic,
    AuthorizationRequired,
    NoRetry,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageCounts {
    pub completed: usize,
    pub failed: usize,
    pub uncertain: usize,
    pub skipped: usize,
}

impl StageCounts {
    pub fn total(self) -> usize {
        self.completed + self.failed + self.uncertain + self.skipped
    }
}

/// Keep the duplicate-risk decision in one place so callers cannot accidentally
/// turn an uncertain response into an ordinary retry.
pub fn retry_decision(state: AttemptState, attempt: u32, idempotent: bool) -> RetryDecision {
    match state {
        AttemptState::Completed | AttemptState::Skipped => RetryDecision::NoRetry,
        AttemptState::Failed => RetryDecision::Automatic,
        AttemptState::Uncertain if idempotent || attempt < 2 => RetryDecision::Automatic,
        AttemptState::Uncertain => RetryDecision::AuthorizationRequired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_is_automatic_once_then_requires_authorization() {
        assert_eq!(
            retry_decision(AttemptState::Uncertain, 1, false),
            RetryDecision::Automatic
        );
        assert_eq!(
            retry_decision(AttemptState::Uncertain, 2, false),
            RetryDecision::AuthorizationRequired
        );
        assert_eq!(
            retry_decision(AttemptState::Uncertain, 9, true),
            RetryDecision::Automatic
        );
    }
}

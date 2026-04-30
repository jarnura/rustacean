use std::time::Duration;

/// Maximum number of delivery attempts before a message is sent to the DLQ.
pub const MAX_ATTEMPTS: u32 = 3;

/// Exponential backoff schedule: attempt 1 → 30 s, attempt 2 → 2 min, attempt 3 → 10 min.
const BACKOFF_SCHEDULE: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
];

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: MAX_ATTEMPTS }
    }
}

impl RetryPolicy {
    /// Returns the delay before the next attempt, or `None` if `attempt >= max_attempts`.
    /// `attempt` is 1-based: first failure = attempt 1, second = attempt 2, …
    #[must_use]
    pub fn next_delay(&self, attempt: u32) -> Option<Duration> {
        if attempt >= self.max_attempts {
            return None;
        }
        let idx = (attempt as usize).saturating_sub(1).min(BACKOFF_SCHEDULE.len() - 1);
        Some(BACKOFF_SCHEDULE[idx])
    }

    /// Computes the `process_after_ms` epoch timestamp for a given attempt.
    #[must_use]
    pub fn process_after_ms(&self, attempt: u32) -> Option<u64> {
        let delay = self.next_delay(attempt)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Some((now + delay).as_millis() as u64)
    }

    #[must_use]
    pub fn is_terminal(&self, attempt: u32) -> bool {
        attempt >= self.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_gets_30s_delay() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.next_delay(1), Some(Duration::from_secs(30)));
    }

    #[test]
    fn second_attempt_gets_2m_delay() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.next_delay(2), Some(Duration::from_secs(120)));
    }

    #[test]
    fn third_attempt_is_terminal() {
        let policy = RetryPolicy::default();
        assert!(policy.is_terminal(3));
        assert_eq!(policy.next_delay(3), None);
    }
}

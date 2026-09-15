//! Optional per-process spacing for serial capture calls, never retry logic.
use crate::{AdapterError, Result};
use std::time::{Duration, Instant};

const VARIABLE: &str = "ARB_RPC_MIN_INTERVAL_MS";

pub(crate) struct RequestPacer {
    minimum: Duration,
    previous_start: Option<Instant>,
}

fn parse(value: Option<&str>) -> Result<Duration> {
    let Some(text) = value else {
        return Ok(Duration::ZERO);
    };
    if text.is_empty()
        || text.len() > 4
        || !text.bytes().all(|c| c.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(AdapterError("invalid bounded RPC pacing configuration"));
    }
    let millis: u64 = text
        .parse()
        .map_err(|_| AdapterError("invalid bounded RPC pacing configuration"))?;
    if millis > 1000 {
        return Err(AdapterError("invalid bounded RPC pacing configuration"));
    }
    Ok(Duration::from_millis(millis))
}

impl RequestPacer {
    pub(crate) fn from_environment() -> Result<Self> {
        let value = match std::env::var(VARIABLE) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => return Err(AdapterError("invalid bounded RPC pacing configuration")),
        };
        Ok(Self {
            minimum: parse(value.as_deref())?,
            previous_start: None,
        })
    }

    fn delay(&self, now: Instant) -> Duration {
        self.previous_start.map_or(Duration::ZERO, |previous| {
            self.minimum
                .saturating_sub(now.saturating_duration_since(previous))
        })
    }

    /// Spacing consumes the existing total capture deadline; it never resets it.
    /// Called only after quota admission, on the existing blocking capture thread.
    pub(crate) fn before_request(&mut self, capture_started: Instant) -> Result<()> {
        let now = Instant::now();
        let remaining = Duration::from_secs(60)
            .checked_sub(now.saturating_duration_since(capture_started))
            .filter(|duration| !duration.is_zero())
            .ok_or(AdapterError("capture RPC deadline exceeded"))?;
        let delay = self.delay(now);
        if delay >= remaining {
            return Err(AdapterError("capture RPC deadline exceeded"));
        }
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        // The transport rechecks its total budget before issuing the request.
        self.previous_start = Some(Instant::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_and_zero_preserve_unpaced_default() {
        assert_eq!(parse(None).unwrap(), Duration::ZERO);
        assert_eq!(parse(Some("0")).unwrap(), Duration::ZERO);
        assert_eq!(parse(Some("75")).unwrap(), Duration::from_millis(75));
        assert_eq!(parse(Some("1000")).unwrap(), Duration::from_secs(1));
    }

    #[test]
    fn invalid_values_are_bounded_and_never_echoed() {
        for value in ["", "01", "+1", "-1", "1.5", " 75", "1001", "PRIVATE_VALUE"] {
            let error = parse(Some(value)).unwrap_err();
            assert_eq!(error.0, "invalid bounded RPC pacing configuration");
        }
    }

    #[test]
    fn first_request_is_immediate_and_later_calls_account_for_elapsed_time() {
        let now = Instant::now();
        let mut pacing = RequestPacer {
            minimum: Duration::from_millis(75),
            previous_start: None,
        };
        assert_eq!(pacing.delay(now), Duration::ZERO);
        pacing.previous_start = Some(now);
        assert_eq!(pacing.delay(now), Duration::from_millis(75));
        assert_eq!(
            pacing.delay(now + Duration::from_millis(30)),
            Duration::from_millis(45)
        );
        assert_eq!(pacing.delay(now + Duration::from_millis(100)), Duration::ZERO);
    }

    #[test]
    fn configured_spacing_actually_waits_before_next_admission() {
        let started = Instant::now();
        let mut pacing = RequestPacer {
            minimum: Duration::from_millis(20),
            previous_start: Some(started),
        };
        pacing.before_request(started).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(20));
        assert!(pacing.previous_start.unwrap() >= started + Duration::from_millis(20));
    }

    #[test]
    fn insufficient_capture_budget_does_not_extend_or_reset_the_deadline() {
        let now = Instant::now();
        let mut pacing = RequestPacer {
            minimum: Duration::from_secs(1),
            previous_start: Some(now),
        };
        let started = now.checked_sub(Duration::from_millis(59_500)).unwrap();
        assert_eq!(
            pacing.before_request(started).unwrap_err().0,
            "capture RPC deadline exceeded"
        );
        assert_eq!(pacing.previous_start, Some(now));
    }

    #[test]
    fn expired_capture_is_rejected_even_when_pacing_is_disabled() {
        let now = Instant::now();
        let mut pacing = RequestPacer {
            minimum: Duration::ZERO,
            previous_start: None,
        };
        assert!(pacing.before_request(now - Duration::from_secs(61)).is_err());
        assert!(pacing.previous_start.is_none());
    }
}

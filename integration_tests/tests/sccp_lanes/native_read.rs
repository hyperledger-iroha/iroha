//! Narrow SCCP State-busy readiness under the original configured HTTP deadline.

use std::time::{Duration, Instant};

use eyre::{Result, ensure, eyre};
use iroha::blocking::Client;
use iroha_torii_shared::ErrorEnvelope;

const BUSY_CODE: &str = "sccp_state_busy";
const BUSY_MESSAGE: &str = "The SCCP committed state is temporarily busy.";

/// Retry only the canonical busy observation, retaining one original HTTP bound.
pub(super) fn read<T>(
    client: &Client,
    operation: &'static str,
    mut request: impl FnMut(&Client) -> iroha::Result<T>,
) -> Result<T> {
    let timeout = client.client().torii_request_timeout();
    ensure!(
        !timeout.is_zero(),
        "SCCP fixture requires its finite HTTP timeout"
    );
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| eyre!("SCCP fixture HTTP deadline overflow"))?;
    let bounded = client.with_request_deadline(deadline)?;
    read_before(
        operation,
        deadline,
        || request(&bounded),
        Instant::now,
        std::thread::sleep,
    )
}

fn state_busy_delay(error: &iroha::Error, expected_operation: &str) -> Option<Duration> {
    let iroha::Error::Http {
        operation,
        status: 503,
        retry_after,
        body,
    } = error
    else {
        return None;
    };
    if *operation != expected_operation || body.len() > 4096 {
        return None;
    }
    // Sccp::get explicitly requests JSON. No alternate decoder or text matching.
    let envelope: ErrorEnvelope = norito::json::from_slice(body).ok()?;
    if envelope.code() != BUSY_CODE || envelope.message() != BUSY_MESSAGE {
        return None;
    }
    let mut details = envelope.details?;
    let seconds = details.retry_after_seconds.take()?;
    let delay = Duration::from_secs(seconds);
    if seconds == 0 || !details.is_empty() || *retry_after != Some(delay) {
        return None;
    }
    Some(delay)
}

fn contextualize(error: iroha::Error, exhausted: bool) -> eyre::Report {
    let bound = if exhausted {
        "; original HTTP deadline exhausted"
    } else {
        ""
    };
    let context = match &error {
        iroha::Error::Http {
            operation,
            status,
            body,
            ..
        } => format!(
            "{operation} HTTP {status}{bound}; response body prefix: {:?}",
            String::from_utf8_lossy(&body[..body.len().min(512)]),
        ),
        _ => format!("native SCCP read failed{bound}"),
    };
    eyre::Report::from(error).wrap_err(context)
}

fn read_before<T>(
    operation: &str,
    deadline: Instant,
    mut request: impl FnMut() -> iroha::Result<T>,
    mut now: impl FnMut() -> Instant,
    mut pause: impl FnMut(Duration),
) -> Result<T> {
    let mut last_busy = None;
    loop {
        if now() >= deadline {
            return Err(last_busy.map_or_else(
                || eyre!("{operation}: original HTTP deadline exhausted"),
                |error| contextualize(error, true),
            ));
        }
        let result = request();
        let completed = now();
        match result {
            Ok(value) => {
                ensure!(
                    completed < deadline,
                    "{operation}: response arrived after its original HTTP deadline"
                );
                return Ok(value);
            }
            Err(error) => {
                let Some(delay) = state_busy_delay(&error, operation) else {
                    return Err(contextualize(error, completed >= deadline));
                };
                if completed >= deadline {
                    return Err(contextualize(error, true));
                }
                let before_pause = now();
                if before_pause >= deadline {
                    return Err(contextualize(error, true));
                }
                last_busy = Some(error);
                pause(delay.min(deadline.saturating_duration_since(before_pause)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_torii_shared::ErrorDetails;
    use std::cell::Cell;

    const OPERATION: &str = "sccp.evidence.anchor";

    fn refusal(status: u16, code: &str, message: &str, seconds: u64) -> iroha::Error {
        let envelope = ErrorEnvelope::new(code, message).with_details(ErrorDetails {
            retry_after_seconds: Some(seconds),
            ..ErrorDetails::default()
        });
        iroha::Error::Http {
            operation: OPERATION,
            status,
            retry_after: Some(Duration::from_secs(seconds)),
            body: norito::json::to_vec(&envelope).unwrap(),
        }
    }

    fn busy() -> iroha::Error {
        refusal(503, BUSY_CODE, BUSY_MESSAGE, 1)
    }

    #[test]
    fn busy_requires_exact_route_status_envelope_and_retry_delay() {
        assert_eq!(
            state_busy_delay(&busy(), OPERATION),
            Some(Duration::from_secs(1))
        );
        assert!(state_busy_delay(&busy(), "sccp.forgery_hold.read").is_none());
        let mut hold_busy = busy();
        if let iroha::Error::Http { operation, .. } = &mut hold_busy {
            *operation = "sccp.forgery_hold.read";
        }
        assert_eq!(
            state_busy_delay(&hold_busy, "sccp.forgery_hold.read"),
            Some(Duration::from_secs(1))
        );
        for error in [
            refusal(429, BUSY_CODE, BUSY_MESSAGE, 1),
            refusal(503, "sccp_unavailable", BUSY_MESSAGE, 1),
            refusal(503, "sccp_capacity", BUSY_MESSAGE, 1),
            refusal(503, BUSY_CODE, "original State reader is poisoned", 1),
            refusal(408, "request_timeout", "deadline", 1),
            refusal(503, BUSY_CODE, BUSY_MESSAGE, 0),
        ] {
            assert!(state_busy_delay(&error, OPERATION).is_none(), "{error:?}");
        }
        let mut mismatched = busy();
        if let iroha::Error::Http { retry_after, .. } = &mut mismatched {
            *retry_after = Some(Duration::from_secs(2));
        }
        assert!(state_busy_delay(&mismatched, OPERATION).is_none());
        let mut malformed = busy();
        if let iroha::Error::Http { body, .. } = &mut malformed {
            *body = b"not JSON".to_vec();
        }
        assert!(state_busy_delay(&malformed, OPERATION).is_none());
        let mut extra_detail = busy();
        if let iroha::Error::Http { body, .. } = &mut extra_detail {
            let mut envelope: ErrorEnvelope = norito::json::from_slice(body).unwrap();
            envelope.details.as_mut().unwrap().reject_code = Some("other_reason".into());
            *body = norito::json::to_vec(&envelope).unwrap();
        }
        assert!(state_busy_delay(&extra_detail, OPERATION).is_none());
    }

    #[test]
    fn exact_busy_retry_preserves_success_and_the_original_deadline() {
        let start = Instant::now();
        let clock = Cell::new(start);
        let calls = Cell::new(0);
        let value = read_before(
            OPERATION,
            start + Duration::from_secs(2),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 { Err(busy()) } else { Ok(7) }
            },
            || clock.get(),
            |delay| clock.set(clock.get() + delay),
        )
        .unwrap();
        assert_eq!((value, calls.get()), (7, 2));
        assert_eq!(clock.get(), start + Duration::from_secs(1));
    }

    #[test]
    fn fast_pause_retries_only_its_exact_busy_envelope_within_the_original_deadline() {
        const FAST_PAUSE: &str = "sccp.fast_pause";
        let fast_pause_refusal = |code, message| {
            let mut error = refusal(503, code, message, 1);
            if let iroha::Error::Http { operation, .. } = &mut error {
                *operation = FAST_PAUSE;
            }
            error
        };
        let start = Instant::now();
        let clock = Cell::new(start);
        let calls = Cell::new(0);
        let deadline = start + Duration::from_secs(2);
        let value = read_before(
            FAST_PAUSE,
            deadline,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    Err(fast_pause_refusal(BUSY_CODE, BUSY_MESSAGE))
                } else {
                    Ok(7)
                }
            },
            || clock.get(),
            |delay| clock.set(clock.get() + delay),
        )
        .unwrap();
        assert_eq!((value, calls.get()), (7, 2));
        assert_eq!(clock.get(), start + Duration::from_secs(1));
        for refusal in [
            fast_pause_refusal("sccp_capacity", BUSY_MESSAGE),
            fast_pause_refusal("sccp_unavailable", BUSY_MESSAGE),
            fast_pause_refusal(BUSY_CODE, "original State reader is poisoned"),
            busy(), // Another operation's busy response cannot authorize this retry.
        ] {
            clock.set(start);
            calls.set(0);
            let mut refusal = Some(refusal);
            let error = read_before::<()>(
                FAST_PAUSE,
                deadline,
                || {
                    calls.set(calls.get() + 1);
                    Err(refusal.take().expect("permanent refusal must not retry"))
                },
                || clock.get(),
                |_| panic!("non-busy fast-pause refusal cannot wait"),
            )
            .unwrap_err();
            assert_eq!(calls.get(), 1);
            assert_eq!(clock.get(), start);
            assert!(matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::Http { status: 503, .. })
            ));
        }
    }

    #[test]
    fn exhausted_and_late_reads_cannot_gain_another_attempt_or_success() {
        let start = Instant::now();
        let clock = Cell::new(start);
        let calls = Cell::new(0);
        let deadline = start + Duration::from_millis(500);
        let error = read_before::<()>(
            OPERATION,
            deadline,
            || {
                calls.set(calls.get() + 1);
                Err(busy())
            },
            || clock.get(),
            |delay| clock.set(clock.get() + delay),
        )
        .unwrap_err();
        assert_eq!(calls.get(), 1);
        assert_eq!(clock.get(), deadline);
        assert!(error.downcast_ref::<iroha::Error>().is_some());
        assert!(
            error
                .to_string()
                .contains("original HTTP deadline exhausted")
        );
        assert!(
            read_before::<()>(
                OPERATION,
                deadline,
                || panic!("expired read"),
                || deadline,
                |_| panic!("expired sleep")
            )
            .is_err()
        );
        clock.set(start);
        assert!(
            read_before(
                OPERATION,
                deadline,
                || {
                    clock.set(deadline);
                    Ok(7)
                },
                || clock.get(),
                |_| panic!("late success cannot retry")
            )
            .is_err()
        );
        clock.set(start);
        assert!(
            read_before::<()>(
                OPERATION,
                deadline,
                || {
                    clock.set(deadline);
                    Err(busy())
                },
                || clock.get(),
                |_| panic!("late refusal cannot retry")
            )
            .is_err()
        );
    }

    #[test]
    fn permanent_refusal_preserves_original_error_without_retry_and_bounds_diagnostics() {
        let start = Instant::now();
        let body = [vec![b'x'; 512], b"UNPRINTED_SUFFIX".to_vec()].concat();
        let error = read_before::<()>(
            OPERATION,
            start + Duration::from_secs(1),
            || {
                Err(iroha::Error::Http {
                    operation: OPERATION,
                    status: 503,
                    retry_after: None,
                    body: body.clone(),
                })
            },
            || start,
            |_| panic!("permanent refusal must not retry"),
        )
        .unwrap_err();
        assert!(
            matches!(error.downcast_ref::<iroha::Error>(), Some(iroha::Error::Http { status: 503, body: original, .. }) if *original == body)
        );
        assert!(!error.to_string().contains("UNPRINTED_SUFFIX"));
        let escaped = contextualize(
            iroha::Error::Http {
                operation: OPERATION,
                status: 503,
                retry_after: None,
                body: b"\x1b[31m\n\xff".to_vec(),
            },
            false,
        )
        .to_string();
        assert!(!escaped.contains('\x1b') && !escaped.contains('\n'));
    }
}

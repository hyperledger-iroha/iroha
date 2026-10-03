//! One lease-window relation for live writes, restore and retained-state checks.

use super::Error;

/// Check the persisted relation without allocating an error or consulting time.
/// Expired bindings remain representable until the ordinary cleanup owner runs.
pub(super) fn violation(
    expiry: Option<u64>,
    grace: Option<u64>,
    bound: u64,
) -> Option<&'static str> {
    match (expiry, grace) {
        (None, Some(_)) => Some("alias grace_until_ms requires lease_expiry_ms"),
        (Some(expiry), _) if expiry <= bound => {
            Some("alias lease_expiry_ms must be greater than bound_at_ms")
        }
        (Some(expiry), Some(grace)) if grace < expiry => {
            Some("alias grace_until_ms must not precede lease_expiry_ms")
        }
        _ => None,
    }
}

/// Project the same relation into the ordinary instruction error type.
pub(super) fn validate_alias_lease_window(
    expiry: Option<u64>,
    grace: Option<u64>,
    bound: u64,
) -> Result<(), Error> {
    violation(expiry, grace, bound).map_or(Ok(()), |reason| {
        Err(Error::InvariantViolation(reason.into()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_error_projection_agree_at_all_boundaries() {
        for (expiry, grace, bound, valid) in [
            (None, None, u64::MAX, true),
            (None, Some(0), 0, false),
            (Some(0), None, 0, false),
            (Some(9), Some(11), 10, false),
            (Some(10), None, 10, false),
            (Some(11), None, 10, true),
            (Some(11), Some(10), 10, false),
            (Some(11), Some(11), 10, true),
            (Some(u64::MAX), Some(u64::MAX), u64::MAX - 1, true),
            (Some(u64::MAX), Some(u64::MAX), u64::MAX, false),
        ] {
            assert_eq!(violation(expiry, grace, bound).is_none(), valid);
            let result = validate_alias_lease_window(expiry, grace, bound);
            assert_eq!(result.is_ok(), valid);
            if let Some(reason) = violation(expiry, grace, bound) {
                let Err(Error::InvariantViolation(actual)) = result else {
                    panic!("same instruction error relation");
                };
                assert_eq!(actual.as_ref(), reason);
            }
        }
    }
}

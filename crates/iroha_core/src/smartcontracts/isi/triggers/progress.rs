//! Bounded autonomous Time carrier authorization. This is an attempt allowance,
//! not native gas escrow: callback funding remains an application responsibility.

use iroha_model_base::{metadata::Metadata, name::Name};
use iroha_primitives::json::Json;
use std::sync::OnceLock;

/// Registration-only declaration; absent schedules retain transaction-driven semantics.
pub const DECLARATION_KEY: &str = "native_progress_attempts";
/// Native lifecycle counter, never writable by a user instruction or callback.
pub(crate) const REMAINING_KEY: &str = "__native_progress_remaining";
/// Maximum autonomous attempts acquired by one admitted registration.
pub const MAX_ATTEMPTS: u32 = 1_024;

fn remaining_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| REMAINING_KEY.parse().expect("valid native progress key"))
}

/// Read a bounded registration declaration without allocating a metadata copy.
/// # Errors
/// Rejects malformed, zero or excess attempt declarations.
pub(crate) fn declaration(metadata: &Metadata) -> Result<Option<u32>, &'static str> {
    static KEY: OnceLock<Name> = OnceLock::new();
    let key = KEY.get_or_init(|| DECLARATION_KEY.parse().expect("valid declaration key"));
    let Some(value) = metadata.get(key) else {
        return Ok(None);
    };
    let count = value
        .try_into_any_norito::<u32>()
        .map_err(|_| "native progress attempts must be an integer")?;
    if !(1..=MAX_ATTEMPTS).contains(&count) {
        return Err("native progress attempts must be between 1 and 1024");
    }
    Ok(Some(count))
}

/// Native initialization after registration metadata has been validated.
pub(crate) fn initialize(metadata: &mut Metadata, count: u32) {
    metadata.insert(remaining_key().clone(), Json::new(count));
}

/// A malformed or missing native counter grants no autonomous carrier authority.
pub(crate) fn remaining(metadata: &Metadata) -> u32 {
    metadata
        .get(remaining_key())
        .and_then(|value| value.try_into_any_norito::<u32>().ok())
        .filter(|count| *count <= MAX_ATTEMPTS)
        .unwrap_or(0)
}

/// Validate the retained declaration/counter pair on restore and World validation.
pub(crate) fn validate_stored(metadata: &Metadata) -> Result<(), &'static str> {
    match (declaration(metadata)?, metadata.get(remaining_key())) {
        (None, None) => Ok(()),
        (Some(declared), Some(value)) => {
            let retained = value
                .try_into_any_norito::<u32>()
                .map_err(|_| "malformed native progress counter")?;
            if retained <= declared {
                Ok(())
            } else {
                Err("native progress counter exceeds its declared allowance")
            }
        }
        _ => Err("incomplete native progress declaration/counter pair"),
    }
}

/// Consume one actual native Time attempt, independently of business success.
/// Caller persists this in the original invocation's success or rejection policy overlay.
pub(crate) fn consume(metadata: &mut Metadata) -> bool {
    let count = remaining(metadata);
    if count == 0 {
        return false;
    }
    initialize(metadata, count - 1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_business_cannot_replenish_native_attempts() {
        let mut native = Metadata::default();
        native.insert(DECLARATION_KEY.parse().unwrap(), Json::new(3_u32));
        let declared = declaration(&native).unwrap().unwrap();
        initialize(&mut native, declared);
        for expected in (0..3).rev() {
            let mut disposable = native.clone();
            initialize(&mut disposable, MAX_ATTEMPTS);
            drop(disposable);
            assert!(consume(&mut native));
            assert_eq!(remaining(&native), expected);
        }
        assert!(!consume(&mut native));
        assert_eq!(remaining(&native), 0);
    }

    #[test]
    fn declaration_bound_and_native_counter_fail_closed() {
        for count in [0, MAX_ATTEMPTS + 1, u32::MAX] {
            let mut metadata = Metadata::default();
            metadata.insert(DECLARATION_KEY.parse().unwrap(), Json::new(count));
            assert!(declaration(&metadata).is_err());
            initialize(&mut metadata, count);
            assert_eq!(remaining(&metadata), 0);
        }
        assert_eq!(declaration(&Metadata::default()), Ok(None));
    }

    #[test]
    fn snapshot_progress_counter_is_complete_bounded_and_cannot_gain_attempts() {
        let mut metadata = Metadata::default();
        assert!(validate_stored(&metadata).is_ok());
        metadata.insert(DECLARATION_KEY.parse().unwrap(), Json::new(2_u32));
        assert!(validate_stored(&metadata).is_err());
        initialize(&mut metadata, 2);
        assert!(validate_stored(&metadata).is_ok());
        initialize(&mut metadata, 0);
        assert!(validate_stored(&metadata).is_ok());
        initialize(&mut metadata, 3);
        assert!(validate_stored(&metadata).is_err());
        initialize(&mut metadata, MAX_ATTEMPTS + 1);
        assert!(validate_stored(&metadata).is_err());
    }
}

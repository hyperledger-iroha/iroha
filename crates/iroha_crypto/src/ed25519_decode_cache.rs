//! Bounded reuse of canonical Ed25519 public-key validation during decoding.
//!
//! Validating a key decompresses a point and multiplies it by the group order, so a
//! stored record that names many accounts spends most of every read here. Every miss
//! uses the existing complete parser. Entries contain public bytes that passed it, never
//! secrets or signature verdicts, and are compared exactly without a digest, so a hit can
//! only repeat a success for these same bytes; rejections are never stored. Fixed
//! thread-local storage avoids additional decode allocations, and a colliding key only
//! falls back to the complete parser, so history can change the cost but never the
//! outcome of a decode.

use std::cell::RefCell;

use crate::signature::ed25519::{Ed25519Sha512, KeyRejection};

const KEY_BYTES: usize = 32;
const WAYS: usize = 4;
/// Together with [`WAYS`], keeps a roster at the record's JSON ceiling almost entirely
/// resident: about 1,360 keys leave 5% of them in a set that has overflowed.
const SETS: usize = 1024;
/// The parser rejects the all-zero encoding, so it can mark an empty slot.
const EMPTY: [u8; KEY_BYTES] = [0; KEY_BYTES];

struct ValidationCache {
    sets: [[[u8; KEY_BYTES]; WAYS]; SETS],
}

impl ValidationCache {
    #[expect(
        clippy::large_stack_arrays,
        reason = "the const thread-local initializer builds it in place; only tests construct one at runtime"
    )]
    const fn new() -> Self {
        Self {
            sets: [[EMPTY; WAYS]; SETS],
        }
    }

    /// Honest keys are uniform here. Grinding a set only evicts keys back to the parser.
    fn set_index(key: &[u8; KEY_BYTES]) -> usize {
        usize::from(u16::from_le_bytes([key[0], key[1]])) % SETS
    }

    fn contains(&self, key: &[u8; KEY_BYTES]) -> bool {
        *key != EMPTY && self.sets[Self::set_index(key)].contains(key)
    }

    fn remember_validated(&mut self, key: &[u8; KEY_BYTES]) {
        if *key == EMPTY || self.contains(key) {
            return;
        }
        let set = &mut self.sets[Self::set_index(key)];
        // A slot chosen by the key leaves the rest of an overflowing set resident, where
        // round-robin replacement would miss every key of a repeated read.
        let way = set
            .iter()
            .position(|slot| *slot == EMPTY)
            .unwrap_or_else(|| usize::from(key[2]) % WAYS);
        set[way] = *key;
    }
}

thread_local! {
    static VALIDATED_KEYS: RefCell<ValidationCache> = const {
        RefCell::new(ValidationCache::new())
    };
    #[cfg(test)]
    static UNCACHED_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Reuse only a complete canonical validation of these exact public-key bytes.
pub fn validate(payload: &[u8]) -> Result<(), KeyRejection> {
    let Ok(key) = <&[u8; KEY_BYTES]>::try_from(payload) else {
        return validate_uncached(payload);
    };
    let cached = VALIDATED_KEYS
        .try_with(|cache| cache.try_borrow().is_ok_and(|cache| cache.contains(key)))
        .unwrap_or(false);
    if cached {
        return Ok(());
    }
    validate_uncached(payload)?;
    // Reentrancy or TLS teardown only removes the optimization. Neither can
    // skip validation or turn a successful parse into a decoding failure.
    let _ = VALIDATED_KEYS.try_with(|cache| {
        if let Ok(mut cache) = cache.try_borrow_mut() {
            cache.remember_validated(key);
        }
    });
    Ok(())
}

fn validate_uncached(payload: &[u8]) -> Result<(), KeyRejection> {
    #[cfg(test)]
    UNCACHED_VALIDATIONS.with(|count| count.set(count.get() + 1));
    Ed25519Sha512::parse_public_key_uncached_for_decode(payload).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Algorithm, KeyPair, PublicKey};
    use curve25519_dalek::{constants::EIGHT_TORSION, edwards::EdwardsPoint};

    fn reset() {
        VALIDATED_KEYS.with(|cache| *cache.borrow_mut() = ValidationCache::new());
        UNCACHED_VALIDATIONS.with(|count| count.set(0));
    }

    fn uncached() -> usize {
        UNCACHED_VALIDATIONS.with(std::cell::Cell::get)
    }

    fn public_key(seed: u32) -> PublicKey {
        let mut seed_bytes = vec![0x5A; 32];
        seed_bytes[..4].copy_from_slice(&seed.to_le_bytes());
        KeyPair::try_from_seed(seed_bytes, Algorithm::Ed25519)
            .expect("Ed25519 fixture")
            .public_key()
            .clone()
    }

    fn key_bytes(seed: u32) -> [u8; KEY_BYTES] {
        public_key(seed)
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 key length")
    }

    /// Exact keys of one set, built directly because the table only compares bytes.
    fn same_set_key(index: u8) -> [u8; KEY_BYTES] {
        let mut key = [0; KEY_BYTES];
        key[0] = 7;
        key[2] = index;
        key[31] = 1;
        key
    }

    #[test]
    fn storage_is_fixed_and_matches_only_the_exact_key() {
        let mut cache = ValidationCache::new();
        assert!(std::mem::size_of::<ValidationCache>() <= 128 * 1024);
        assert!(SETS.is_power_of_two());
        let key = same_set_key(1);
        assert!(!cache.contains(&key));
        cache.remember_validated(&key);
        assert!(cache.contains(&key));
        for byte in 0..KEY_BYTES {
            let mut changed = key;
            changed[byte] ^= 1;
            assert!(!cache.contains(&changed), "byte {byte} must be compared");
        }
    }

    #[test]
    fn the_empty_marker_is_never_a_key() {
        let mut cache = ValidationCache::new();
        assert!(!cache.contains(&EMPTY));
        cache.remember_validated(&EMPTY);
        assert!(!cache.contains(&EMPTY));
        assert_eq!(cache.sets[0], [EMPTY; WAYS]);
        reset();
        assert_eq!(validate(&EMPTY), Err(KeyRejection::AllZero));
        validate(&key_bytes(1)).expect("valid key");
        assert_eq!(validate(&EMPTY), Err(KeyRejection::AllZero));
    }

    #[test]
    fn a_full_set_replaces_one_way_and_duplicates_displace_nothing() {
        let mut cache = ValidationCache::new();
        let resident = [0, 1, 2, 3].map(same_set_key);
        for key in &resident {
            cache.remember_validated(key);
        }
        for key in &resident {
            cache.remember_validated(key);
        }
        for key in &resident {
            assert!(cache.contains(key));
        }
        let newcomer = same_set_key(6);
        cache.remember_validated(&newcomer);
        assert!(cache.contains(&newcomer));
        let survivors = resident.iter().filter(|key| cache.contains(key)).count();
        assert_eq!(survivors, WAYS - 1);
    }

    #[test]
    fn repeated_decodes_avoid_reparsing() {
        let key = key_bytes(2);
        reset();
        validate(&key).expect("complete cold validation");
        validate(&key).expect("exact warm validation");
        assert_eq!(uncached(), 1);
    }

    #[test]
    fn a_roster_at_the_record_ceiling_stays_almost_entirely_resident() {
        reset();
        let keys = (0..1_360).map(key_bytes).collect::<Vec<_>>();
        for key in &keys {
            validate(key).expect("cold roster key");
        }
        assert_eq!(uncached(), keys.len());
        for key in &keys {
            validate(key).expect("warm roster key");
        }
        let misses = uncached() - keys.len();
        assert!(misses * 10 <= keys.len(), "{misses} misses");
    }

    #[test]
    fn every_rejection_is_reparsed_and_never_stored() {
        let valid = key_bytes(3);
        let mut non_canonical = [0xFF; KEY_BYTES];
        non_canonical[31] = 0x7F;
        let small_order = EIGHT_TORSION[1].compress().to_bytes();
        let mixed_order: EdwardsPoint =
            curve25519_dalek::constants::ED25519_BASEPOINT_POINT + EIGHT_TORSION[1];
        let torsioned = mixed_order.compress().to_bytes();
        let cases: [(&[u8], KeyRejection); 6] = [
            (&[], KeyRejection::Length(0)),
            (&valid[..31], KeyRejection::Length(31)),
            (&EMPTY, KeyRejection::AllZero),
            (&non_canonical, KeyRejection::NonCanonical),
            (&small_order, KeyRejection::SmallOrder),
            (&torsioned, KeyRejection::Torsion),
        ];
        reset();
        validate(&valid).expect("warm valid key");
        for (invalid, rejection) in cases {
            let before = uncached();
            assert_eq!(validate(invalid), Err(rejection));
            assert_eq!(validate(invalid), Err(rejection));
            assert_eq!(uncached(), before + 2);
        }
        let mut changed = valid;
        changed[0] ^= 1;
        let before = uncached();
        let _ = validate(&changed);
        assert_eq!(uncached(), before + 1, "a warm key accepts no other bytes");
        validate(&valid).expect("valid entry survives rejected inputs");
    }

    #[test]
    fn borrowed_cache_falls_back_to_complete_validation() {
        let key = key_bytes(4);
        reset();
        VALIDATED_KEYS.with(|cache| {
            let _borrow = cache.borrow_mut();
            validate(&key).expect("uncached fallback");
            assert_eq!(validate(&EMPTY), Err(KeyRejection::AllZero));
        });
        assert_eq!(uncached(), 2);
        VALIDATED_KEYS.with(|cache| assert!(!cache.borrow().contains(&key)));
        validate(&key).expect("cache after reentrant fallback");
        assert_eq!(uncached(), 3);
    }

    #[test]
    fn cache_history_never_changes_decode_allocation_admission_or_wire_bytes() {
        let limits = |bytes| {
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        };
        let key = public_key(5);
        let (_, bytes) = key.to_bytes();
        let exact = bytes.len() + 1;
        for warm in [false, true] {
            reset();
            if warm {
                validate(bytes).expect("warm validation cache");
            }
            let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
                PublicKey::from_bytes_for_decode(Algorithm::Ed25519, bytes)
            });
            assert_eq!(
                decoded.expect("identical admission on cold and warm paths"),
                key
            );
            assert_eq!(usage.total_allocated_bytes(), exact);
            reset();
            if warm {
                validate(bytes).expect("warm validation cache");
            }
            let (rejected, usage) =
                norito::core::with_decode_limits_measured(limits(exact - 1), || {
                    PublicKey::from_bytes_for_decode(Algorithm::Ed25519, bytes)
                });
            assert!(rejected.is_err());
            assert!(usage.total_allocated_bytes() < exact);
        }
        let wire = norito::to_bytes(&key).expect("canonical wire");
        let json = norito::json::to_json(&key).expect("canonical JSON");
        reset();
        for _ in 0..2 {
            let decoded: PublicKey = norito::decode_from_bytes(&wire).expect("wire decode");
            assert_eq!(decoded, key);
            assert_eq!(norito::to_bytes(&decoded).expect("same wire"), wire);
        }
        reset();
        for _ in 0..2 {
            assert_eq!(
                norito::json::from_str::<PublicKey>(&json).expect("JSON decode"),
                key
            );
        }
    }
}

//! Bounded reuse of canonical BLS public-key validation during decoding.
//!
//! Every miss uses the existing complete parser, including canonical encoding,
//! subgroup and identity checks. Entries contain public bytes, never secrets or
//! authority/PoP verdicts. Exact bytes and orientation are compared without a
//! digest. Fixed process-wide storage avoids additional decode allocations and
//! static TLS reservations on small Linux thread stacks. A nonblocking cache cell
//! falls back to the complete parser on contention or reentrancy; successful and
//! rejected validations allocate no diagnostic or point backing.

use crate::{Algorithm, decode_validation_cache::TryCell, signature::bls};

const CAPACITY: usize = 128;
const MAX_KEY_BYTES: usize = 96;

#[derive(Clone, Copy)]
struct Entry {
    algorithm: Algorithm,
    length: u8,
    bytes: [u8; MAX_KEY_BYTES],
}

impl Entry {
    const EMPTY: Self = Self {
        algorithm: Algorithm::BlsNormal,
        length: 0,
        bytes: [0; MAX_KEY_BYTES],
    };
}

struct ValidationCache {
    entries: [Entry; CAPACITY],
    next: usize,
}

impl ValidationCache {
    const fn new() -> Self {
        Self {
            entries: [Entry::EMPTY; CAPACITY],
            next: 0,
        }
    }

    fn contains(&self, algorithm: Algorithm, payload: &[u8]) -> bool {
        self.entries.iter().any(|entry| {
            entry.length != 0
                && entry.algorithm == algorithm
                && usize::from(entry.length) == payload.len()
                && entry.bytes[..usize::from(entry.length)] == *payload
        })
    }

    fn remember_validated(&mut self, algorithm: Algorithm, payload: &[u8]) {
        if payload.is_empty() || payload.len() > MAX_KEY_BYTES || self.contains(algorithm, payload)
        {
            return;
        }
        let mut entry = Entry::EMPTY;
        entry.algorithm = algorithm;
        entry.length = u8::try_from(payload.len()).expect("BLS key fits fixed storage");
        entry.bytes[..payload.len()].copy_from_slice(payload);
        self.entries[self.next] = entry;
        self.next = (self.next + 1) % CAPACITY;
    }
}

static VALIDATED_KEYS: TryCell<ValidationCache> = TryCell::new(ValidationCache::new());

#[cfg(test)]
thread_local! {
    static UNCACHED_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Reuse only a complete canonical validation of these exact public-key bytes.
pub fn validate(algorithm: Algorithm, payload: &[u8]) -> Result<(), bls::canonical::Failure> {
    validate_with_cache(&VALIDATED_KEYS, algorithm, payload)
}

fn validate_with_cache(
    cache: &TryCell<ValidationCache>,
    algorithm: Algorithm,
    payload: &[u8],
) -> Result<(), bls::canonical::Failure> {
    if cache
        .try_with(|cache| cache.contains(algorithm, payload))
        .unwrap_or(false)
    {
        return Ok(());
    }
    // Release the cache before parsing. Contention and reentrancy only remove
    // the optimization; they never alter the complete canonical validation.
    validate_uncached(algorithm, payload)?;
    let _ = cache.try_with(|cache| cache.remember_validated(algorithm, payload));
    Ok(())
}

fn validate_uncached(algorithm: Algorithm, payload: &[u8]) -> Result<(), bls::canonical::Failure> {
    #[cfg(test)]
    UNCACHED_VALIDATIONS.with(|count| count.set(count.get() + 1));
    match algorithm {
        Algorithm::BlsNormal => bls::BlsNormal::validate_public_key_for_decode(payload),
        Algorithm::BlsSmall => bls::BlsSmall::validate_public_key_for_decode(payload),
        _ => Err(bls::canonical::Failure::PublicKeyAlgorithm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KeyPair, PublicKey};

    fn reset_count() {
        UNCACHED_VALIDATIONS.with(|count| count.set(0));
    }

    fn reset_public_cache() {
        while VALIDATED_KEYS
            .try_with(|cache| *cache = ValidationCache::new())
            .is_none()
        {
            std::thread::yield_now();
        }
        reset_count();
    }

    fn key(algorithm: Algorithm) -> PublicKey {
        KeyPair::try_from_seed(vec![0x59; 32], algorithm)
            .expect("BLS fixture")
            .public_key()
            .clone()
    }

    #[test]
    fn storage_is_fixed_and_matches_the_full_key_and_orientation() {
        let mut cache = ValidationCache::new();
        assert!(std::mem::size_of::<ValidationCache>() <= 16 * 1024);
        assert!(!cache.contains(Algorithm::BlsNormal, &[]));
        let bytes = [7; 48];
        cache.remember_validated(Algorithm::BlsNormal, &bytes);
        assert!(cache.contains(Algorithm::BlsNormal, &bytes));
        assert!(!cache.contains(Algorithm::BlsSmall, &bytes));
        assert!(!cache.contains(Algorithm::BlsNormal, &bytes[..47]));
        let mut changed = bytes;
        changed[47] ^= 1;
        assert!(!cache.contains(Algorithm::BlsNormal, &changed));
        cache.remember_validated(Algorithm::BlsNormal, &[]);
        cache.remember_validated(Algorithm::BlsNormal, &[0; MAX_KEY_BYTES + 1]);
        assert_eq!(cache.next, 1);
    }

    #[test]
    fn eviction_is_bounded_and_duplicate_hits_do_not_displace_entries() {
        let mut cache = ValidationCache::new();
        for index in 0..CAPACITY {
            cache.remember_validated(Algorithm::BlsNormal, &[u8::try_from(index).unwrap(); 48]);
        }
        assert_eq!(cache.next, 0);
        cache.remember_validated(Algorithm::BlsNormal, &[1; 48]);
        assert_eq!(cache.next, 0);
        cache.remember_validated(Algorithm::BlsNormal, &[128; 48]);
        assert!(!cache.contains(Algorithm::BlsNormal, &[0; 48]));
        assert!(cache.contains(Algorithm::BlsNormal, &[1; 48]));
        assert!(cache.contains(Algorithm::BlsNormal, &[128; 48]));
    }

    #[test]
    fn repeated_decodes_avoid_reparsing_for_both_orientations() {
        for algorithm in [Algorithm::BlsNormal, Algorithm::BlsSmall] {
            let key = key(algorithm);
            let (_, bytes) = key.to_bytes();
            let cache = TryCell::new(ValidationCache::new());
            reset_count();
            validate_with_cache(&cache, algorithm, bytes).expect("complete cold validation");
            validate_with_cache(&cache, algorithm, bytes).expect("exact warm validation");
            assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 1);
        }
    }

    #[test]
    fn failed_parses_are_not_cached_and_warm_keys_do_not_accept_other_bytes() {
        for algorithm in [Algorithm::BlsNormal, Algorithm::BlsSmall] {
            let key = key(algorithm);
            let (_, bytes) = key.to_bytes();
            let cache = TryCell::new(ValidationCache::new());
            reset_count();
            validate_with_cache(&cache, algorithm, bytes).expect("warm valid key");
            let mut trailing = bytes.to_vec();
            trailing.push(0);
            for invalid in [
                Vec::new(),
                vec![0; bytes.len()],
                bytes[..bytes.len() - 1].to_vec(),
                trailing,
            ] {
                let before = UNCACHED_VALIDATIONS.with(std::cell::Cell::get);
                assert!(validate_with_cache(&cache, algorithm, &invalid).is_err());
                assert!(validate_with_cache(&cache, algorithm, &invalid).is_err());
                assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), before + 2);
            }
            assert!(validate_with_cache(&cache, Algorithm::Ed25519, bytes).is_err());
            validate_with_cache(&cache, algorithm, bytes)
                .expect("valid entry survives rejected inputs");
        }
    }

    #[test]
    fn occupied_cache_falls_back_to_complete_validation() {
        let key = key(Algorithm::BlsNormal);
        let (_, bytes) = key.to_bytes();
        let cache = TryCell::new(ValidationCache::new());
        reset_count();
        cache
            .try_with(|_| {
                validate_with_cache(&cache, Algorithm::BlsNormal, bytes)
                    .expect("uncached fallback");
                assert!(validate_with_cache(&cache, Algorithm::BlsNormal, &[]).is_err());
            })
            .expect("hold local cache");
        assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 2);
        assert_eq!(
            cache.try_with(|cache| cache.contains(Algorithm::BlsNormal, bytes)),
            Some(false)
        );
        validate_with_cache(&cache, Algorithm::BlsNormal, bytes)
            .expect("cache after reentrant fallback");
        assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 3);
    }

    #[test]
    fn cold_warm_and_contended_validation_allocate_no_native_backing() {
        for algorithm in [Algorithm::BlsNormal, Algorithm::BlsSmall] {
            let key = key(algorithm);
            let (_, bytes) = key.to_bytes();
            let cache = TryCell::new(ValidationCache::new());
            reset_count();
            for _ in 0..2 {
                let result = crate::test_allocations::without_allocations(|| {
                    validate_with_cache(&cache, algorithm, bytes)
                });
                result.expect("canonical public key");
            }
            assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 1);
            cache
                .try_with(|_| {
                    crate::test_allocations::without_allocations(|| {
                        validate_with_cache(&cache, algorithm, bytes)
                    })
                    .expect("contended canonical validation");
                    assert!(
                        crate::test_allocations::without_allocations(|| {
                            validate_with_cache(&cache, algorithm, &[])
                        })
                        .is_err()
                    );
                })
                .expect("hold local cache");
            assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 3);
        }
    }

    #[test]
    fn unwinding_releases_cache_without_accepting_invalid_keys() {
        let cache = TryCell::new(ValidationCache::new());
        let key = key(Algorithm::BlsNormal);
        let (_, bytes) = key.to_bytes();
        validate_with_cache(&cache, Algorithm::BlsNormal, bytes).expect("warm validated key");
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.try_with(|_| panic!("cache ownership unwind control"));
        }));
        assert!(unwind.is_err());
        reset_count();
        validate_with_cache(&cache, Algorithm::BlsNormal, bytes)
            .expect("unwind released ownership");
        assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 0);
        assert!(validate_with_cache(&cache, Algorithm::BlsNormal, &[]).is_err());
        assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 1);
    }

    #[test]
    fn shared_cache_validates_and_reuses_both_orientations_on_a_small_stack() {
        for algorithm in [Algorithm::BlsNormal, Algorithm::BlsSmall] {
            let key = key(algorithm);
            let (_, bytes) = key.to_bytes();
            let cache = TryCell::new(ValidationCache::new());
            std::thread::scope(|scope| {
                std::thread::Builder::new()
                    .name("bls-cache-small-stack".to_owned())
                    .stack_size(128 * 1024)
                    .spawn_scoped(scope, || {
                        reset_count();
                        validate_with_cache(&cache, algorithm, bytes)
                            .expect("cold canonical validation");
                        validate_with_cache(&cache, algorithm, bytes)
                            .expect("exact shared cache hit");
                        assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 1);
                        assert!(validate_with_cache(&cache, algorithm, &[]).is_err());
                        assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 2);
                        validate(algorithm, bytes)
                            .expect("actual public validation fits the same bounded stack");
                    })
                    .expect("spawn bounded BLS decoder thread")
                    .join()
                    .expect("bounded BLS decoder thread");
            });
            reset_count();
            validate_with_cache(&cache, algorithm, bytes).expect("parent reuses child validation");
            assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 0);
            cache
                .try_with(|_| {
                    std::thread::scope(|scope| {
                        scope
                            .spawn(|| {
                                reset_count();
                                validate_with_cache(&cache, algorithm, bytes)
                                    .expect("contended cross-thread fallback");
                                assert!(validate_with_cache(&cache, algorithm, &[]).is_err());
                                assert_eq!(UNCACHED_VALIDATIONS.with(std::cell::Cell::get), 2);
                            })
                            .join()
                            .expect("contended decoder thread");
                    });
                })
                .expect("hold parent cache ownership");
        }
    }

    #[test]
    fn cache_history_never_changes_decode_allocation_admission_or_wire_bytes() {
        let limits = |bytes| {
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        };
        for algorithm in [Algorithm::BlsNormal, Algorithm::BlsSmall] {
            let key = key(algorithm);
            let (_, bytes) = key.to_bytes();
            let exact = bytes.len() + 1;
            for warm in [false, true] {
                reset_public_cache();
                if warm {
                    validate(algorithm, bytes).expect("warm validation cache");
                }
                let (decoded, usage) =
                    norito::core::with_decode_limits_measured(limits(exact), || {
                        PublicKey::from_bytes_for_decode(algorithm, bytes)
                    });
                assert_eq!(
                    decoded.expect("identical admission on cold and warm paths"),
                    key
                );
                assert_eq!(usage.total_allocated_bytes(), exact);
                reset_public_cache();
                if warm {
                    validate(algorithm, bytes).expect("warm validation cache");
                }
                let (rejected, usage) =
                    norito::core::with_decode_limits_measured(limits(exact - 1), || {
                        PublicKey::from_bytes_for_decode(algorithm, bytes)
                    });
                assert!(rejected.is_err());
                assert!(usage.total_allocated_bytes() < exact);
            }
            let wire = norito::to_bytes(&key).expect("canonical wire");
            let json = norito::json::to_json(&key).expect("canonical JSON");
            reset_public_cache();
            for _ in 0..2 {
                let decoded: PublicKey = norito::decode_from_bytes(&wire).expect("wire decode");
                assert_eq!(decoded, key);
                assert_eq!(norito::to_bytes(&decoded).expect("same wire"), wire);
            }
            reset_public_cache();
            for _ in 0..2 {
                assert_eq!(
                    norito::json::from_str::<PublicKey>(&json).expect("JSON decode"),
                    key
                );
            }
        }
    }
}

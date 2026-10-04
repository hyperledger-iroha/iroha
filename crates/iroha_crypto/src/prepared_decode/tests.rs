//! Original initialized backing, canonical parity and refusal retirement controls.

use std::alloc::Layout;

use iroha_allocation::{ChargedBufferFromChargeError, release::ReleaseRegistration};
use norito::core::{DecodeFlagsGuard, DecodeFromSlice, DecodeLimits, header_flags};

use super::*;
use crate::{Algorithm, KeyPair, Signature, test_allocations::*};

fn charge(pool: &AllocationBudget, bytes: usize) -> AllocationCharge {
    let layout = Layout::array::<u8>(bytes).unwrap();
    pool.try_reserve(layout).unwrap().try_split(layout).unwrap()
}
fn signature(pool: &AllocationBudget, bytes: usize) -> PreparedSignatureDecode {
    PreparedSignatureDecode::try_from_charge(bytes, pool, charge(pool, bytes))
        .unwrap_or_else(|(_, error)| panic!("{error}"))
}
fn public_key(pool: &AllocationBudget, bytes: usize) -> PreparedPublicKeyDecode {
    PreparedPublicKeyDecode::try_from_charge(bytes, pool, charge(pool, bytes))
        .unwrap_or_else(|(_, error)| panic!("{error}"))
}
fn encode(value: &impl SerializePayload) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .serialize(&mut Encoder::for_buffer(&mut bytes))
        .unwrap();
    bytes
}

#[test]
fn prepared_signature_decodes_and_seals_exact_original_backing_without_allocation() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for len in [48, 64, 96] {
            let value = Signature::from_bytes(&vec![0x53; len]);
            let bytes = encode(&value);
            let pool = AllocationBudget::new(len);
            let mut destination = signature(&pool, len);
            let pointer = destination.0.bytes.as_slice().as_ptr();
            pool.set_limit_bytes(0);
            without_allocations(|| destination.decode_payload(&bytes)).unwrap();
            assert!(destination.belongs_to(&pool));
            assert_eq!(
                destination.decoded_payload(),
                Some(value.payload().as_ref())
            );
            let mut encoded = Vec::with_capacity(bytes.len());
            without_allocations(|| destination.serialize(&mut Encoder::for_buffer(&mut encoded)))
                .unwrap();
            assert_eq!(encoded, bytes);
            let bound = without_allocations(|| destination.finish())
                .unwrap_or_else(|_| panic!("complete leaf must seal"));
            assert_eq!(bound.get(), &value);
            assert_eq!(bound.get().payload().as_ptr(), pointer);
            assert!(bound.belongs_to(&pool));
            assert_eq!(pool.reserved_bytes(), len);
            let (_, frees) = with_deallocation_observation(len, || drop(bound));
            assert_eq!(frees, 1);
            assert_eq!(pool.reserved_bytes(), 0);
        }
    }
}

#[test]
fn prepared_public_key_matches_canonical_validator_and_exact_owned_bytes() {
    let mut algorithms = vec![Algorithm::Ed25519, Algorithm::Secp256k1];
    #[cfg(feature = "bls")]
    algorithms.extend([Algorithm::BlsNormal, Algorithm::BlsSmall]);
    for algorithm in algorithms {
        let pair = KeyPair::from_seed(vec![0x59; 32], algorithm);
        let key = pair.public_key();
        let len = key.retained_allocation_layout().size();
        for flags in [0, header_flags::COMPACT_LEN] {
            let _flags = DecodeFlagsGuard::enter(flags);
            let bytes = encode(key);
            let pool = AllocationBudget::new(len);
            let mut destination = public_key(&pool, len);
            let pointer = destination.0.bytes.as_slice().as_ptr();
            pool.set_limit_bytes(0);
            without_allocations(|| destination.decode_payload(&bytes)).unwrap();
            assert!(destination.belongs_to(&pool));
            assert_eq!(
                destination.decoded_compact(),
                Some(key.0.algorithm_and_payload.as_ref())
            );
            let mut encoded = Vec::with_capacity(bytes.len());
            without_allocations(|| destination.serialize(&mut Encoder::for_buffer(&mut encoded)))
                .unwrap();
            assert_eq!(encoded, bytes);
            let bound = without_allocations(|| destination.finish())
                .unwrap_or_else(|_| panic!("validated key must seal"));
            assert_eq!(bound.get(), key);
            assert_eq!(bound.get().0.algorithm_and_payload.as_ptr(), pointer);
            assert_eq!(pool.reserved_bytes(), len);
            drop(bound);
            assert_eq!(pool.reserved_bytes(), 0);
        }
    }
}

#[test]
fn prepared_key_retains_canonical_late_allocation_limit_and_original_retry_backing() {
    let pair = KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519);
    let key = pair.public_key();
    let len = key.retained_allocation_layout().size();
    let pool = AllocationBudget::new(len);
    let mut destination = public_key(&pool, len);
    let pointer = destination.0.bytes.as_slice().as_ptr();
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let bytes = encode(key);
        // The compact sequence's count charge fits; its canonical retained
        // storage charge must still refuse after point validation. Prepaid
        // physical backing is independent of this logical work ceiling.
        for allowed in [len, 2 * len - 1] {
            without_allocations(|| destination.decode_payload(&bytes)).unwrap();
            let limits = DecodeLimits::new(1024, 4096, 1024, allowed, 32);
            let protocol = DecodeLimits::new(1024, 4096, 1024, 2 * len, 32);
            // The observer must see the new protocol scope inside the attempt;
            // the stricter caller scope remains outside it. A bare leaf alone
            // cannot authenticate a fresh decode-budget family.
            let ordinary = norito::core::with_decode_limits_scope(limits, || {
                norito::core::classify_decode_attempt(|| {
                    norito::core::with_decode_limits_scope(protocol, || {
                        PublicKey::decode_from_slice(&bytes)
                    })
                })
            })
            .unwrap_err();
            let prepared = norito::core::with_decode_limits_scope(limits, || {
                norito::core::classify_decode_attempt(|| {
                    norito::core::with_decode_limits_scope(protocol, || {
                        match destination.decode_payload(&bytes) {
                            Err(PreparedCryptoDecodeError::Codec(error)) => Err::<(), _>(error),
                            result => {
                                panic!("canonical retained-key charge must refuse: {result:?}")
                            }
                        }
                    })
                })
            })
            .unwrap_err();
            // Both enclosing scopes have ended before inspecting their original
            // resource classification; no error is reconstructed from numbers.
            assert_eq!(
                ordinary.kind(),
                norito::core::DecodeAttemptErrorKind::EnclosingLimit
            );
            assert_eq!(prepared.kind(), ordinary.kind());
            assert_eq!(prepared.to_string(), ordinary.to_string());
            assert_eq!(
                prepared.into_error().decode_resource_error(),
                ordinary.into_error().decode_resource_error()
            );
            assert!(destination.decoded_compact().is_none());
            assert_eq!(destination.0.bytes.as_slice().as_ptr(), pointer);
            assert_eq!(pool.reserved_bytes(), len);
        }
        let exact = DecodeLimits::new(1024, 4096, 1024, 2 * len, 32);
        let (ordinary, used) =
            norito::core::with_decode_limits_scope(exact, || PublicKey::decode_from_slice(&bytes))
                .unwrap();
        assert_eq!(ordinary, *key);
        assert_eq!(used, bytes.len());
        norito::core::with_decode_limits_scope(exact, || destination.decode_payload(&bytes))
            .unwrap();
        assert_eq!(destination.decoded_compact().unwrap().as_ptr(), pointer);
        assert_eq!(
            destination.decoded_compact(),
            Some(key.0.algorithm_and_payload.as_ref())
        );
    }
    pool.set_limit_bytes(0);
    let bound = without_allocations(|| destination.finish())
        .unwrap_or_else(|_| panic!("exact retry must seal"));
    assert_eq!(bound.get(), key);
    assert_eq!(bound.get().0.algorithm_and_payload.as_ptr(), pointer);
    drop(bound);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn malformed_signature_attempts_clear_validity_and_reuse_original_allocation() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let valid = encode(&Signature::from_bytes(&[0x75; 64]));
        let zero = encode(&Signature::from_bytes(&[0; 64]));
        let pool = AllocationBudget::new(64);
        let mut destination = signature(&pool, 64);
        let pointer = destination.0.bytes.as_slice().as_ptr();
        without_allocations(|| destination.decode_payload(&valid)).unwrap();
        assert!(matches!(
            without_allocations(|| destination.decode_payload(&zero)),
            Err(PreparedCryptoDecodeError::Signature(
                SignaturePayloadError::AllZero
            ))
        ));
        assert!(destination.decoded_payload().is_none());
        destination = destination
            .finish()
            .err()
            .expect("invalid leaf retains owner");
        for length in 0..valid.len() {
            let prepared = without_allocations(|| destination.decode_payload(&valid[..length]));
            let ordinary = Signature::decode_from_slice(&valid[..length]);
            let Err(PreparedCryptoDecodeError::Codec(prepared)) = prepared else {
                panic!("truncation must be canonical framing error")
            };
            assert_eq!(prepared.to_string(), ordinary.unwrap_err().to_string());
            assert!(destination.decoded_payload().is_none());
            assert_eq!(destination.0.bytes.as_slice().as_ptr(), pointer);
            assert_eq!(pool.reserved_bytes(), 64);
        }
        without_allocations(|| destination.decode_payload(&valid)).unwrap();
        without_allocations(|| destination.reset());
        assert!(destination.finish().is_err());
        assert_eq!(pool.reserved_bytes(), 0);
    }
    let pool = AllocationBudget::new(0);
    let mut empty = signature(&pool, 0);
    let bytes = encode(&Signature::from_bytes(&[]));
    assert!(matches!(
        without_allocations(|| empty.decode_payload(&bytes)),
        Err(PreparedCryptoDecodeError::Signature(
            SignaturePayloadError::Empty
        ))
    ));
}

#[test]
fn key_invalid_point_tag_trailing_and_truncation_keep_canonical_error_precedence() {
    let pair = KeyPair::from_seed(vec![0x62; 32], Algorithm::Ed25519);
    let key = pair.public_key();
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let valid = encode(key);
        let pool = AllocationBudget::new(33);
        let mut destination = public_key(&pool, 33);
        let mut malformed = vec![vec![0; 8]];
        let mut invalid_tag = key.0.algorithm_and_payload.to_vec();
        invalid_tag[0] = 0xff;
        let mut invalid_point = invalid_tag.clone();
        invalid_point[0] = PublicKeyCompact::algorithm_tag(Algorithm::Ed25519);
        invalid_point[1..].fill(0);
        for compact in [invalid_tag, invalid_point] {
            let mut encoded = Vec::new();
            norito::core::write_element_sequence::<u8, _>(
                &mut Encoder::for_buffer(&mut encoded),
                compact.iter(),
            )
            .unwrap();
            malformed.push(encoded);
        }
        malformed.extend((0..valid.len()).map(|length| valid[..length].to_vec()));
        let mut trailing = valid.clone();
        trailing.push(0x73);
        malformed.push(trailing);
        for bytes in malformed {
            let prepared = without_allocations(|| destination.decode_payload(&bytes));
            let ordinary =
                public_key_decode::with_decoded_compact(&bytes, true, |_, _| Ok::<_, Error>(()));
            let Err(PreparedCryptoDecodeError::Codec(prepared)) = prepared else {
                panic!("same canonical key relation must reject malformed input")
            };
            assert_eq!(prepared.to_string(), ordinary.unwrap_err().to_string());
            assert!(destination.decoded_compact().is_none());
        }
        without_allocations(|| destination.decode_payload(&valid)).unwrap();
        destination.reset();
        assert!(destination.finish().is_err());
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn geometry_and_original_scope_refusal_preserve_initialized_storage_for_retry() {
    let bytes = encode(&Signature::from_bytes(&[0x41; 64]));
    let pool = AllocationBudget::new(129);
    let mut wrong = signature(&pool, 65);
    assert!(matches!(
        without_allocations(|| wrong.decode_payload(&bytes)),
        Err(PreparedCryptoDecodeError::Geometry {
            expected: 65,
            offered: 64
        })
    ));
    assert!(wrong.decoded_payload().is_none());
    let mut destination = signature(&pool, 64);
    let pointer = destination.0.bytes.as_slice().as_ptr();
    let error = norito::core::with_decode_limits_scope(
        DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || {
            norito::core::classify_decode_attempt(|| {
                // The ordinary scope owner is outside this leaf-only allocator
                // observation. The future prepared frame workspace must replace
                // this scope's physical allocations before production use.
                norito::core::with_decode_limits_scope(
                    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX),
                    || match without_allocations(|| destination.decode_payload(&bytes)) {
                        Err(PreparedCryptoDecodeError::Codec(error)) => Err::<(), _>(error),
                        _ => panic!(
                            "actual enclosing source must refuse before destination mutation"
                        ),
                    },
                )
            })
        },
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    assert!(destination.decoded_payload().is_none());
    assert_eq!(destination.0.bytes.as_slice().as_ptr(), pointer);
    assert_eq!(pool.reserved_bytes(), 129);
    pool.set_limit_bytes(0);
    without_allocations(|| destination.decode_payload(&bytes)).unwrap();
    assert_eq!(destination.decoded_payload().unwrap().as_ptr(), pointer);
    drop((destination, wrong));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn preparation_returns_exact_charge_on_foreign_layout_and_physical_allocator_refusal() {
    let pool = AllocationBudget::new(96);
    let foreign = AllocationBudget::new(96);
    let original = charge(&pool, 96);
    let (original, failure) = PreparedSignatureDecode::try_from_charge(96, &foreign, original)
        .err()
        .unwrap();
    assert_eq!(failure, SignatureAllocationError::ForeignPool);
    assert!(original.belongs_to(&pool));
    let (original, failure) = PreparedSignatureDecode::try_from_charge(95, &pool, original)
        .err()
        .unwrap();
    assert!(matches!(
        failure,
        SignatureAllocationError::Allocation(ChargedBufferFromChargeError::LayoutMismatch { .. })
    ));
    let (original, failure) = with_allocation_failure(96, || {
        PreparedSignatureDecode::try_from_charge(96, &pool, original)
    })
    .err()
    .unwrap();
    assert_eq!(
        failure,
        SignatureAllocationError::Allocation(ChargedBufferFromChargeError::Allocator {
            layout: Layout::array::<u8>(96).unwrap()
        })
    );
    assert!(original.belongs_to(&pool));
    assert_eq!(pool.reserved_bytes(), 96);
    pool.set_limit_bytes(0);
    let destination = PreparedSignatureDecode::try_from_charge(96, &pool, original)
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    drop(destination);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);

    let pool = AllocationBudget::new(33);
    let original = charge(&pool, 33);
    let (original, failure) = with_allocation_failure(33, || {
        PreparedPublicKeyDecode::try_from_charge(33, &pool, original)
    })
    .err()
    .unwrap();
    assert_eq!(
        failure,
        PublicKeyAllocationError::Allocation(ChargedBufferFromChargeError::Allocator {
            layout: Layout::array::<u8>(33).unwrap()
        })
    );
    assert_eq!(pool.reserved_bytes(), 33);
    drop(
        PreparedPublicKeyDecode::try_from_charge(33, &pool, original)
            .unwrap_or_else(|(_, error)| panic!("{error}")),
    );
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_leaf_unwind_deallocates_backing_before_original_pool_notification() {
    use std::{
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    struct Check {
        pool: AllocationBudget,
    }
    impl Wake for Check {
        fn wake(self: Arc<Self>) {
            assert!(
                observed_deallocations() > 0,
                "backing is physically gone before refund wake"
            );
            assert_eq!(
                self.pool.reserved_bytes(),
                ReleaseRegistration::allocation_layout().size()
            );
        }
    }
    let registration_bytes = ReleaseRegistration::allocation_layout().size();
    let pool = AllocationBudget::new(96 + registration_bytes);
    let mut prepaid = pool
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    drop(prepaid);
    let destination = signature(&pool, 96);
    let failure = pool
        .try_reserve(Layout::array::<u8>(1).unwrap())
        .unwrap_err();
    let iroha_allocation::AllocationRefusal::Capacity { release: wait, .. } = failure else {
        panic!("the live original destination must be the capacity blocker")
    };
    let waker = Waker::from(Arc::new(Check { pool: pool.clone() }));
    assert_eq!(
        registration.poll_wait(&wait, &mut Context::from_waker(&waker)),
        Poll::Pending
    );
    let ((unwind, _), _) = with_deallocation_observation(96, || {
        allocations_during(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _owner = destination;
                panic!("prepared destination interrupted")
            }))
        })
    });
    assert!(unwind.is_err());
    registration.cancel();
    drop((wait, registration));
    assert_eq!(pool.reserved_bytes(), 0);
}

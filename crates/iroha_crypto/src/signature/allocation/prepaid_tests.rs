//! Physical prepaid signature backing, original-pool custody and direct producer tests.

use super::*;
use crate::test_allocations::without_allocations;

#[test]
fn fully_initialized_signature_buffer_moves_without_reallocation_and_refunds_after_drop() {
    for bytes in [&[][..], &[0; 64][..], &[0x73; 96][..]] {
        let budget = AllocationBudget::new(bytes.len());
        let mut output = ChargedBuffer::new(bytes.len(), &budget).unwrap();
        output.append(bytes).unwrap();
        let pointer = output.as_slice().as_ptr();
        budget.set_limit_bytes(0);
        let bound =
            without_allocations(|| ChargedSignature::try_from_preallocated(&budget, output))
                .unwrap_or_else(|(_, error)| panic!("{error}"));
        assert_eq!(bound.get().payload(), bytes);
        assert_eq!(bound.get().payload().as_ptr(), pointer);
        assert!(bound.belongs_to(&budget));
        assert_eq!(budget.reserved_bytes(), bytes.len());
        drop(bound);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn preallocated_signature_rejects_foreign_and_incomplete_owners_without_losing_retry() {
    let original = AllocationBudget::new(64);
    let foreign = AllocationBudget::new(64);
    let mut output = ChargedBuffer::new(64, &original).unwrap();
    output.append(&[0x35; 32]).unwrap();
    let pointer = output.as_slice().as_ptr();
    let (output, reason) =
        without_allocations(|| ChargedSignature::try_from_preallocated(&foreign, output))
            .err()
            .unwrap();
    assert_eq!(reason, SignatureAllocationError::ForeignPool);
    let (mut output, reason) =
        without_allocations(|| ChargedSignature::try_from_preallocated(&original, output))
            .err()
            .unwrap();
    assert_eq!(
        reason,
        SignatureAllocationError::IncompleteBuffer {
            initialized: 32,
            capacity: 64
        }
    );
    assert_eq!(output.as_slice(), &[0x35; 32]);
    assert_eq!(output.as_slice().as_ptr(), pointer);
    assert_eq!(original.reserved_bytes(), 64);
    output.append(&[0x71; 32]).unwrap();
    original.set_limit_bytes(0);
    let bound = without_allocations(|| ChargedSignature::try_from_preallocated(&original, output))
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    assert_eq!(bound.get().payload().as_ptr(), pointer);
    assert_eq!(original.reserved_bytes(), 64);
    drop(bound);
    assert_eq!(original.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
}

#[cfg(feature = "bls")]
#[test]
fn prepaid_bls_signing_retains_exact_original_backing_and_canonical_signatures() {
    use crate::{Algorithm, KeyPair, verify_signature_borrowed};
    for (algorithm, length) in [(Algorithm::BlsNormal, 96), (Algorithm::BlsSmall, 48)] {
        let pair = KeyPair::from_seed(vec![0x75; 32], algorithm);
        let message = b"prepaid committee record";
        let ordinary = Signature::try_new(pair.private_key(), message).unwrap();
        let budget = AllocationBudget::new(length);
        let output = ChargedBuffer::new(length, &budget).unwrap();
        let pointer = output.as_slice().as_ptr();
        budget.set_limit_bytes(0);
        let (signed, draws) = super::super::bls::signing::observe_entropy_draws(|| {
            without_allocations(|| {
                Signature::try_new_bls_prepaid(pair.private_key(), message, output)
            })
        });
        let signed = signed.unwrap_or_else(|(_, error)| panic!("{error}"));
        assert_eq!(draws, usize::from(cfg!(feature = "rand")));
        assert_eq!(signed.get(), &ordinary);
        assert_eq!(signed.get().payload().as_ptr(), pointer);
        assert!(signed.belongs_to(&budget));
        assert_eq!(budget.reserved_bytes(), length);
        verify_signature_borrowed(signed.get(), pair.public_key(), message).unwrap();
        assert!(verify_signature_borrowed(signed.get(), pair.public_key(), b"changed").is_err());
        drop(signed);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[cfg(feature = "bls")]
#[test]
fn prepaid_bls_geometry_and_key_failures_return_the_unchanged_output_owner() {
    use crate::{Algorithm, KeyPair, PrivateKey, PrivateKeyInner, secrecy::Secret};
    let valid = KeyPair::from_seed(vec![0x31; 32], Algorithm::BlsNormal);
    let other = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let invalid = PrivateKey(Box::new(Secret::new(PrivateKeyInner::BlsNormal(
        super::super::bls::BlsNormalPrivateKey::from_unchecked_bytes_for_test(vec![0xff; 32]),
    ))));
    let budget = AllocationBudget::new(512);
    for (capacity, initialized, key, code) in [
        (95, 0, valid.private_key(), 0),
        (95, 0, &invalid, 0),
        (96, 1, valid.private_key(), 0),
        (96, 0, other.private_key(), 1),
        (96, 0, &invalid, 2),
    ] {
        let mut output = ChargedBuffer::new(capacity, &budget).unwrap();
        if initialized != 0 {
            output.append(&[0x71]).unwrap();
        }
        let pointer = output.as_slice().as_ptr();
        let before = budget.reserved_bytes();
        let before_bytes = output.as_slice().to_vec();
        let (refused, draws) = super::super::bls::signing::observe_entropy_draws(|| {
            without_allocations(|| Signature::try_new_bls_prepaid(key, b"refused output", output))
        });
        assert_eq!(
            draws, 0,
            "preflight failure must precede the actual RNG draw"
        );
        let (output, error) = refused.err().expect("original output refused");
        match (code, error) {
            (
                0,
                PrepaidBlsSignatureError::OutputGeometry {
                    expected,
                    capacity: actual,
                    initialized: actual_initialized,
                },
            ) => {
                assert_eq!(
                    (expected, actual, actual_initialized),
                    (96, capacity, initialized)
                );
            }
            (1, PrepaidBlsSignatureError::Algorithm)
            | (
                2,
                PrepaidBlsSignatureError::Signing(super::super::bls::BlsSigningError::PrivateKey(
                    _,
                )),
            ) => {}
            _ => panic!("original failure category"),
        }
        assert_eq!(output.capacity(), capacity);
        assert_eq!(output.as_slice(), before_bytes);
        assert_eq!(output.as_slice().len(), initialized);
        assert_eq!(output.as_slice().as_ptr(), pointer);
        assert_eq!(budget.reserved_bytes(), before);
        if code == 2 {
            let (signed, draws) = super::super::bls::signing::observe_entropy_draws(|| {
                without_allocations(|| {
                    Signature::try_new_bls_prepaid(valid.private_key(), b"refused output", output)
                })
            });
            let signed = signed.unwrap_or_else(|(_, error)| panic!("{error}"));
            assert_eq!(draws, usize::from(cfg!(feature = "rand")));
            assert_eq!(signed.get().payload().as_ptr(), pointer);
            drop(signed);
        } else {
            drop(output);
        }
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[cfg(all(feature = "bls", not(feature = "rand")))]
#[test]
fn prepaid_bls_without_rand_uses_exact_original_owner_and_no_entropy() {
    // This named gate must exist and run in the explicit no-default-features,
    // bls-only build; the same custody test runs under the normal rand build too.
    prepaid_bls_signing_retains_exact_original_backing_and_canonical_signatures();
    prepaid_bls_geometry_and_key_failures_return_the_unchanged_output_owner();
}

//! Verifier tests: every tamper of the proof bytes is rejected, decoding is
//! canonical with typed reasons, instance shapes are exact in both modes,
//! and the parameters, the verifying key and the descriptor are bound.

use ff::{Field, PrimeField};
use group::{Curve, Group, GroupEncoding};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, poseidon::PoseidonField};

use super::*;
use crate::{
    cs::{InstanceModeV1, descriptor::ExprNodeV1},
    pcs::ipa::accumulator::batch_decide,
    test_circuits::{Arithmetic, BUDGET, CHOICES, Lookups, Permutations, Setup, setup},
    transcript::MESSAGE_BYTES,
};

const ARITHMETIC: Arithmetic = Arithmetic {
    start: 5,
    rows: 6,
    tamper: None,
};

const LOOKUPS: Lookups = Lookups {
    rows: 5,
    tamper: None,
    out_of_range: false,
    offset: 0,
};

const PERMUTATIONS: Permutations = Permutations {
    rows: 4,
    tamper: None,
};

/// Flips bit 0 of every byte, and bit 7 of the last byte of every message
/// (a point's sign bit, a scalar's top bit); every result must be rejected.
fn every_flip_is_rejected<C: PastaCurve>(
    setup: &Setup<C>,
    instances: &[Vec<C::ScalarExt>],
    proof: &[u8],
) where
    C::ScalarExt: PoseidonField,
{
    assert_eq!(setup.verify(instances, proof), Ok(()));
    let mut tampered = proof.to_vec();
    for index in 0..proof.len() {
        let masks: &[u8] = if index % MESSAGE_BYTES == MESSAGE_BYTES - 1 {
            &[0x01, 0x80]
        } else {
            &[0x01]
        };
        for mask in masks {
            tampered[index] ^= mask;
            assert!(
                setup.verify(instances, &tampered).is_err(),
                "byte {index} mask {mask:#04x} accepted"
            );
            tampered[index] ^= mask;
        }
    }
    assert_eq!(tampered, proof);
}

#[test]
fn every_byte_flip_is_rejected_on_both_curves() {
    let arithmetic = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let instances = ARITHMETIC.instances::<Fq>();
    let proof = arithmetic.prove(&ARITHMETIC, &instances, 1).expect("proof");
    every_flip_is_rejected(&arithmetic, &instances, &proof);

    let lookups = setup::<Eq, _>(&LOOKUPS, CHOICES[1]);
    let proof = lookups.prove(&LOOKUPS, &[], 2).expect("proof");
    every_flip_is_rejected(&lookups, &[], &proof);
}

#[test]
fn every_byte_flip_is_rejected_with_linked_permutation_sets() {
    let permutations = setup::<Eq, _>(&PERMUTATIONS, CHOICES[3]);
    let instances = PERMUTATIONS.instances::<Fp>();
    let proof = permutations
        .prove(&PERMUTATIONS, &instances, 3)
        .expect("proof");
    every_flip_is_rejected(&permutations, &instances, &proof);
}

/// The message kinds of a proof in order (`true` for a point).
fn message_kinds<C: PastaCurve>(setup: &Setup<C>) -> Vec<bool> {
    let protocol = Protocol::new(setup.pk.binding().descriptor()).expect("protocol");
    let shape = protocol.shape();
    let k = shape.k as usize;
    let mut kinds = Vec::new();
    let mut push = |point: bool, count: usize| kinds.extend(std::iter::repeat_n(point, count));
    push(true, shape.num_advice + 3 * shape.lookups + shape.permutation_sets + 1);
    push(true, shape.quotient_pieces);
    let instance_evals = if shape.committed_instances {
        shape.instance_queries
    } else {
        0
    };
    push(
        false,
        instance_evals
            + shape.advice_queries
            + shape.fixed_queries
            + 1
            + shape.permutation_columns
            + (3 * shape.permutation_sets).saturating_sub(1)
            + 5 * shape.lookups,
    );
    push(true, 1); // q'
    push(false, protocol.plan().sets().len());
    push(true, 1 + 2 * k); // S, then L_j and R_j
    push(false, 2); // c, f
    if shape.folded_generator_suffix {
        push(true, 1);
    }
    assert_eq!(kinds.len() * MESSAGE_BYTES, protocol.proof_length());
    kinds
}

#[test]
fn structured_tampers_are_rejected_with_typed_reasons() {
    for choice in [CHOICES[0], CHOICES[1]] {
        let setup = setup::<Ep, _>(&ARITHMETIC, choice);
        let instances = ARITHMETIC.instances::<Fq>();
        let proof = setup.prove(&ARITHMETIC, &instances, 5).expect("proof");
        let kinds = message_kinds(&setup);
        let suffix = choice.2 == ProofSuffixV1::FoldedGenerator;
        let generator = Ep::generator().to_affine().to_bytes();
        let modulus = crate::cs::descriptor::modulus_le_bytes::<Fq>();
        for (index, point) in kinds.iter().enumerate() {
            let range = index * MESSAGE_BYTES..(index + 1) * MESSAGE_BYTES;
            let replace = |bytes: &[u8]| {
                let mut tampered = proof.clone();
                tampered[range.clone()].copy_from_slice(bytes);
                setup.verify(&instances, &tampered)
            };
            if *point {
                assert_eq!(
                    replace(&[0; 32]),
                    Err(VerifyError::Transcript(TranscriptError::IdentityPoint)),
                    "message {index}"
                );
                let last = suffix && index + 1 == kinds.len();
                let other = replace(&generator);
                if last {
                    assert_eq!(
                        other,
                        Err(VerifyError::Ipa(IpaError::FoldedGeneratorMismatch))
                    );
                } else {
                    assert!(other.is_err(), "message {index}");
                }
            } else {
                assert_eq!(
                    replace(&modulus),
                    Err(VerifyError::Transcript(
                        TranscriptError::NonCanonicalScalar
                    )),
                    "message {index}"
                );
                let value = Fq::from_repr(proof[range.clone()].try_into().expect("32"))
                    .expect("canonical");
                assert!(
                    replace(&(value + Fq::ONE).to_repr()).is_err(),
                    "message {index}"
                );
            }
        }
    }
}

#[test]
fn the_proof_length_is_exact() {
    let setup = setup::<Eq, _>(&ARITHMETIC, CHOICES[1]);
    let instances = ARITHMETIC.instances::<Fp>();
    let proof = setup.prove(&ARITHMETIC, &instances, 6).expect("proof");
    let expected = proof.len();
    let mut longer = proof.clone();
    longer.push(0);
    assert_eq!(
        setup.verify(&instances, &longer),
        Err(VerifyError::ProofLength {
            expected,
            actual: expected + 1
        })
    );
    longer.extend_from_slice(&[0; 31]);
    assert_eq!(
        setup.verify(&instances, &longer),
        Err(VerifyError::ProofLength {
            expected,
            actual: expected + 32
        })
    );
    assert_eq!(
        setup.verify(&instances, &proof[..expected - 1]),
        Err(VerifyError::ProofLength {
            expected,
            actual: expected - 1
        })
    );
    assert_eq!(
        setup.verify(&instances, &[]),
        Err(VerifyError::ProofLength {
            expected,
            actual: 0
        })
    );
}

#[test]
fn instance_shapes_are_exact_in_both_modes() {
    for choice in CHOICES {
        let setup = setup::<Ep, _>(&ARITHMETIC, choice);
        assert_eq!(
            setup.pk.binding().descriptor().instance_mode,
            choice.1,
            "{choice:?}"
        );
        let instances = ARITHMETIC.instances::<Fq>();
        let proof = setup.prove(&ARITHMETIC, &instances, 8).expect("proof");
        assert_eq!(setup.verify(&instances, &proof), Ok(()));
        assert_eq!(
            setup.verify(&[], &proof),
            Err(VerifyError::InstanceColumns {
                expected: 1,
                found: 0
            })
        );
        let mut extra = instances.clone();
        extra.push(Vec::new());
        assert_eq!(
            setup.verify(&extra, &proof),
            Err(VerifyError::InstanceColumns {
                expected: 1,
                found: 2
            })
        );
        // A trailing zero would describe the same polynomial; it is still
        // rejected (S4, DEV-04).
        let mut padded = instances.clone();
        padded[0].push(Fq::ZERO);
        assert_eq!(
            setup.verify(&padded, &proof),
            Err(VerifyError::InstanceLength {
                column: 0,
                expected: 2,
                found: 3
            })
        );
        let mut short = instances.clone();
        short[0].pop();
        assert_eq!(
            setup.verify(&short, &proof),
            Err(VerifyError::InstanceLength {
                column: 0,
                expected: 2,
                found: 1
            })
        );
        for position in 0..2 {
            let mut changed = instances.clone();
            changed[0][position] += Fq::ONE;
            assert!(setup.verify(&changed, &proof).is_err(), "{choice:?}");
        }
    }
}

#[test]
fn parameters_and_keys_are_bound() {
    let fixture = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let instances = ARITHMETIC.instances::<Fq>();
    let proof = fixture.prove(&ARITHMETIC, &instances, 9).expect("proof");
    // Parameters of another k.
    let other = PinnedParams::<Ep>::derive(7).expect("params");
    assert_eq!(
        verify_full(
            &other,
            fixture.pk.binding(),
            fixture.pk.vk(),
            &instances,
            &proof,
            BUDGET
        ),
        Err(VerifyError::ParamsMismatch)
    );
    // A key bound to another descriptor.
    let poseidon = setup::<Ep, _>(&ARITHMETIC, CHOICES[3]);
    assert_eq!(
        verify_full(
            &fixture.params,
            fixture.pk.binding(),
            poseidon.pk.vk(),
            &instances,
            &proof,
            BUDGET
        ),
        Err(VerifyError::KeyMismatch)
    );
    // Malformed descriptor and key bytes.
    assert!(matches!(
        verify_full_from_bytes(
            &fixture.params,
            &[1, 2, 3],
            fixture.pk.vk().to_bytes(),
            &instances,
            &proof,
            BUDGET
        ),
        Err(VerifyError::Descriptor(_))
    ));
    let mut vk = fixture.pk.vk().to_bytes().to_vec();
    vk.push(0);
    assert!(matches!(
        verify_full_from_bytes(
            &fixture.params,
            fixture.pk.binding().encoded(),
            &vk,
            &instances,
            &proof,
            BUDGET
        ),
        Err(VerifyError::VerifyingKey(VkError::Length { .. }))
    ));
}

#[test]
fn the_descriptor_digest_binds_the_transcript() {
    // The same VK bytes read under a descriptor that differs only in one
    // gate constant: the relation changes, so does transcript_repr, and the
    // proof is rejected (hash what you verify).
    let setup = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let instances = ARITHMETIC.instances::<Fq>();
    let proof = setup.prove(&ARITHMETIC, &instances, 10).expect("proof");
    let mut descriptor = setup.pk.binding().descriptor().clone();
    let scaled = descriptor
        .gates
        .iter_mut()
        .flatten()
        .flatten()
        .find_map(|node| match node {
            ExprNodeV1::Scaled(bytes) => Some(bytes),
            _ => None,
        })
        .expect("the add gate is scaled");
    *scaled = Fq::from(8).to_repr();
    let binding = DescriptorBinding::new(descriptor).expect("valid descriptor");
    assert_ne!(binding.digest(), setup.pk.binding().digest());
    let vk = VerifyingKey::<Ep>::read(setup.pk.vk().to_bytes(), &binding).expect("same bytes");
    assert_ne!(vk.transcript_repr(), setup.pk.vk().transcript_repr());
    assert_eq!(
        verify_full(&setup.params, &binding, &vk, &instances, &proof, BUDGET),
        Err(VerifyError::Ipa(IpaError::OpeningFailed))
    );
}

#[test]
fn succinct_verification_returns_a_pending_accumulator() {
    let setup = setup::<Eq, _>(&PERMUTATIONS, CHOICES[1]);
    let instances = PERMUTATIONS.instances::<Fp>();
    let first = setup.prove(&PERMUTATIONS, &instances, 1).expect("proof");
    let second = setup.prove(&PERMUTATIONS, &instances, 2).expect("proof");
    let succinct = |proof: &[u8]| {
        verify_succinct(
            &setup.params,
            setup.pk.binding(),
            setup.pk.vk(),
            &instances,
            proof,
            BUDGET,
        )
    };
    let accumulators = vec![
        succinct(&first).expect("first"),
        succinct(&second).expect("second"),
    ];
    assert_eq!(batch_decide(accumulators, &setup.params, BUDGET), Ok(()));
    // A suffix that is a valid point but not G'_0 passes the succinct check
    // only as a claim; deciding it fails.
    let mut forged = first.clone();
    let start = forged.len() - MESSAGE_BYTES;
    forged[start..].copy_from_slice(&Eq::generator().to_affine().to_bytes());
    assert!(succinct(&forged).is_err());
    assert!(verify_full(
        &setup.params,
        setup.pk.binding(),
        setup.pk.vk(),
        &instances,
        &forged,
        BUDGET
    )
    .is_err());
}

#[test]
fn errors_convert_to_typed_reasons() {
    assert_eq!(
        VerifyError::from(MultiopenError::Ipa(IpaError::Transcript(
            TranscriptError::IdentityPoint
        ))),
        VerifyError::Transcript(TranscriptError::IdentityPoint)
    );
    assert_eq!(
        VerifyError::from(MultiopenError::DegenerateChallenge),
        VerifyError::Multiopen(MultiopenError::DegenerateChallenge)
    );
    assert_eq!(
        VerifyError::from(IpaError::OpeningFailed),
        VerifyError::Ipa(IpaError::OpeningFailed)
    );
    for error in [
        VerifyError::KeyMismatch,
        VerifyError::ParamsMismatch,
        VerifyError::DegenerateChallenge,
        VerifyError::SuffixRequired,
        VerifyError::IdentityInstanceCommitment { column: 0 },
        VerifyError::from(ProtocolError::Overflow),
        VerifyError::from(VkError::Shape),
    ] {
        assert!(!error.to_string().is_empty());
    }
    let _ = (InstanceModeV1::Direct, Fp::ZERO, Fq::ZERO);
}

#[test]
fn batch_verification_merges_proofs_of_mixed_k() {
    let committed = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let folded = setup::<Ep, _>(&ARITHMETIC, CHOICES[2]);
    let instances = ARITHMETIC.instances::<Fq>();
    let first = committed.prove(&ARITHMETIC, &instances, 1).expect("proof");
    let second = committed.prove(&ARITHMETIC, &instances, 2).expect("proof");
    let third = folded.prove(&ARITHMETIC, &instances, 3).expect("proof");
    // A k = 7 proof with a FoldedGenerator suffix.
    let params7 = PinnedParams::<Ep>::derive(7).expect("params");
    let pk7 = crate::keys::keygen_pk(
        &params7,
        &ARITHMETIC,
        &crate::test_circuits::keygen_config(CHOICES[1]),
    )
    .expect("pk");
    let fourth = crate::prover::prove_circuit(
        &params7,
        &pk7,
        &ARITHMETIC,
        &instances,
        crate::prover::ProverRandomness::fixed_seed_for_tests([4; 32]),
        &crate::prover::ProverConfig::default(),
    )
    .expect("proof");
    let item = |setup: &'static str, proof| -> BatchItem<'_, Ep> {
        let (params, pk) = match setup {
            "committed" => (&committed.params, &committed.pk),
            "folded" => (&folded.params, &folded.pk),
            _ => (&params7, &pk7),
        };
        BatchItem {
            params,
            binding: pk.binding(),
            vk: pk.vk(),
            instances: &instances,
            proof,
        }
    };
    let items = [
        item("committed", first.as_slice()),
        item("committed", second.as_slice()),
        item("folded", third.as_slice()),
        item("k7", fourth.as_slice()),
    ];
    assert_eq!(batch_verify(&items, BUDGET), Ok(()));
    assert_eq!(batch_verify::<Ep>(&[], BUDGET), Ok(()));
    for index in [0, 2, 3] {
        assert_eq!(batch_verify(&[items[index]], BUDGET), Ok(()), "{index}");
    }

    // Another statement for one item: decoding succeeds, the equation fails.
    let mut changed = instances.clone();
    changed[0][1] += Fq::ONE;
    let mut bad = items;
    bad[1].instances = &changed;
    assert_eq!(batch_verify(&bad, BUDGET), Err(VerifyError::BatchRejected));

    // A suffix that is not G'_0 fails through its accumulator item.
    let mut forged = fourth.clone();
    let start = forged.len() - MESSAGE_BYTES;
    forged[start..].copy_from_slice(&Ep::generator().to_affine().to_bytes());
    let mut bad = items;
    bad[3].proof = &forged;
    assert_eq!(batch_verify(&bad, BUDGET), Err(VerifyError::BatchRejected));

    // Steps 1-8 failures name the item.
    let mut bad = items;
    bad[2].proof = &third[..third.len() - 1];
    assert_eq!(
        batch_verify(&bad, BUDGET),
        Err(VerifyError::BatchItem {
            index: 2,
            error: Box::new(VerifyError::ProofLength {
                expected: third.len(),
                actual: third.len() - 1
            })
        })
    );
    assert!(
        VerifyError::BatchItem {
            index: 2,
            error: Box::new(VerifyError::BatchRejected)
        }
        .to_string()
        .contains("item 2")
    );
}

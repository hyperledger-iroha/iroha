//! End-to-end prover tests: proofs of the test circuits verify on both
//! curves under every protocol option; the constraint checker and the
//! verifier agree on honest and tampered witnesses; proofs do not depend on
//! the thread count; the zero-knowledge budget holds and fresh randomness
//! changes every message; the randomness sources bind what they must; and a
//! malicious prover that commits two advice columns identically is caught
//! by static grouping (S1).

use std::sync::{Arc, Mutex};

use ff::{Field, PrimeField};
use group::Curve;
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, poseidon::PoseidonField};
use rand_chacha::ChaCha20Rng;
use rand_core_06::SeedableRng;

use super::{
    advice::{Advice, InstanceColumns},
    multiopen::Opened,
    quotient::{CompiledExpressions, QuotientInputs},
    vanishing::{commit_quotient, commit_random},
    *,
};
use crate::{
    check::{CheckMode, check_circuit},
    cs::{InstanceModeV1, ProofSuffixV1, TranscriptV1},
    frontend::Circuit,
    pcs::ipa::{
        IpaError, commit::Secrecy, commit::commit_lagrange, evaluate_polynomial,
    },
    protocol::{Protocol, check_zero_knowledge_budget, evaluate_expression, rotate},
    test_circuits::{
        Arithmetic, BUDGET, CHOICES, Choice, Forgeable, K, Lookups, Permutations, Setup, setup,
    },
    transcript::MESSAGE_BYTES,
    verifier::{VerifyError, verify_full, verify_full_from_bytes, verify_succinct},
};

const ARITHMETIC: Arithmetic = Arithmetic {
    start: 3,
    rows: 12,
    tamper: None,
};

const LOOKUPS: Lookups = Lookups {
    rows: 9,
    tamper: None,
    out_of_range: false,
    offset: 0,
};

const PERMUTATIONS: Permutations = Permutations {
    rows: 10,
    tamper: None,
};

/// Proves `circuit` under every choice and checks every verifier entry
/// point.
fn round_trip<C, Ci>(circuit: &Ci, instances: &[Vec<C::ScalarExt>])
where
    C: PastaCurve,
    C::ScalarExt: PoseidonField,
    Ci: Circuit<C::ScalarExt>,
{
    for choice in CHOICES {
        let setup = setup::<C, _>(circuit, choice);
        let proof = setup.prove(circuit, instances, 7).expect("proof");
        let protocol = Protocol::new(setup.pk.binding().descriptor()).expect("protocol");
        assert_eq!(proof.len(), protocol.proof_length(), "{choice:?}");
        assert_eq!(setup.verify(instances, &proof), Ok(()), "{choice:?}");
        assert_eq!(
            verify_full_from_bytes(
                &setup.params,
                setup.pk.binding().encoded(),
                setup.pk.vk().to_bytes(),
                instances,
                &proof,
                BUDGET,
            ),
            Ok(()),
            "{choice:?}"
        );
        let succinct = verify_succinct(
            &setup.params,
            setup.pk.binding(),
            setup.pk.vk(),
            instances,
            &proof,
            BUDGET,
        );
        if choice.2 == ProofSuffixV1::FoldedGenerator {
            let accumulator = succinct.expect("succinct");
            assert_eq!(accumulator.transcript_repr(), setup.pk.vk().transcript_repr());
            assert_eq!(accumulator.decide(&setup.params, BUDGET), Ok(()));
        } else {
            assert_eq!(succinct.err(), Some(VerifyError::SuffixRequired));
        }
    }
}

#[test]
fn arithmetic_proofs_verify_on_both_curves() {
    round_trip::<Ep, _>(&ARITHMETIC, &ARITHMETIC.instances::<Fq>());
    round_trip::<Eq, _>(&ARITHMETIC, &ARITHMETIC.instances::<Fp>());
}

#[test]
fn lookup_proofs_verify_on_both_curves() {
    round_trip::<Ep, _>(&LOOKUPS, &[]);
    round_trip::<Eq, _>(&LOOKUPS, &[]);
}

#[test]
fn permutation_heavy_proofs_verify_on_both_curves() {
    let setup = setup::<Ep, _>(&PERMUTATIONS, CHOICES[0]);
    let shape = *Protocol::new(setup.pk.binding().descriptor())
        .expect("protocol")
        .shape();
    // Degree 3: one column per set, eight linked sets.
    assert_eq!((shape.degree, shape.chunk_len), (3, 1));
    assert_eq!(shape.permutation_sets, 8);
    round_trip::<Ep, _>(&PERMUTATIONS, &PERMUTATIONS.instances::<Fq>());
    round_trip::<Eq, _>(&PERMUTATIONS, &PERMUTATIONS.instances::<Fp>());
}

/// The checker's verdict and the proof's verdict for one witness.
fn verdicts<C, Ci>(
    setup: &Setup<C>,
    circuit: &Ci,
    instances: &[Vec<C::ScalarExt>],
) -> (bool, bool)
where
    C: PastaCurve,
    C::ScalarExt: PoseidonField,
    Ci: Circuit<C::ScalarExt>,
{
    let checked = check_circuit(circuit, K, instances, CheckMode::Strict)
        .expect("synthesis")
        .is_satisfied();
    let proved = setup
        .prove(circuit, instances, 3)
        .is_ok_and(|proof| setup.verify(instances, &proof).is_ok());
    (checked, proved)
}

#[test]
fn the_constraint_checker_and_the_prover_agree() {
    let arithmetic = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let instances = ARITHMETIC.instances::<Fq>();
    assert_eq!(verdicts(&arithmetic, &ARITHMETIC, &instances), (true, true));
    for row in [0, 5, 11] {
        let tampered = Arithmetic {
            tamper: Some(row),
            ..ARITHMETIC
        };
        assert_eq!(
            verdicts(&arithmetic, &tampered, &instances),
            (false, false),
            "row {row}"
        );
    }
    let mut wrong_output = instances.clone();
    wrong_output[0][1] += Fq::ONE;
    assert_eq!(
        verdicts(&arithmetic, &ARITHMETIC, &wrong_output),
        (false, false)
    );

    let lookups = setup::<Eq, _>(&LOOKUPS, CHOICES[1]);
    assert_eq!(verdicts(&lookups, &LOOKUPS, &[]), (true, true));
    for tampered in [
        Lookups {
            tamper: Some(4),
            ..LOOKUPS
        },
        Lookups {
            out_of_range: true,
            ..LOOKUPS
        },
    ] {
        assert_eq!(verdicts(&lookups, &tampered, &[]), (false, false));
    }

    let permutations = setup::<Ep, _>(&PERMUTATIONS, CHOICES[2]);
    let instances = PERMUTATIONS.instances::<Fq>();
    assert_eq!(
        verdicts(&permutations, &PERMUTATIONS, &instances),
        (true, true)
    );
    for row in [0, 4, 9] {
        let tampered = Permutations {
            tamper: Some(row),
            ..PERMUTATIONS
        };
        assert_eq!(
            verdicts(&permutations, &tampered, &instances),
            (false, false),
            "row {row}"
        );
    }
}

/// Proves `circuit` in pools of 1, 2, 4 and 7 threads and requires equal
/// bytes.
fn thread_independent<C, Ci>(circuit: &Ci, instances: &[Vec<C::ScalarExt>], choice: Choice)
where
    C: PastaCurve,
    C::ScalarExt: PoseidonField,
    Ci: Circuit<C::ScalarExt> + Sync,
{
    let setup = setup::<C, _>(circuit, choice);
    let reference = setup.prove(circuit, instances, 11).expect("proof");
    for threads in [1, 2, 4, 7] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        let proof = pool
            .install(|| setup.prove(circuit, instances, 11))
            .expect("proof");
        assert_eq!(proof, reference, "{threads} threads");
    }
    assert_eq!(setup.verify(instances, &reference), Ok(()));
}

#[test]
fn proofs_do_not_depend_on_the_thread_count() {
    thread_independent::<Ep, _>(&ARITHMETIC, &ARITHMETIC.instances::<Fq>(), CHOICES[0]);
    thread_independent::<Eq, _>(&LOOKUPS, &[], CHOICES[1]);
    thread_independent::<Ep, _>(
        &PERMUTATIONS,
        &PERMUTATIONS.instances::<Fq>(),
        CHOICES[3],
    );
}

#[test]
fn the_zero_knowledge_budget_holds_for_every_relation() {
    let descriptors = [
        setup::<Ep, _>(&ARITHMETIC, CHOICES[0]).pk.binding().descriptor().clone(),
        setup::<Eq, _>(&LOOKUPS, CHOICES[1]).pk.binding().descriptor().clone(),
        setup::<Ep, _>(&PERMUTATIONS, CHOICES[2]).pk.binding().descriptor().clone(),
    ];
    for descriptor in &descriptors {
        let protocol = Protocol::new(descriptor).expect("protocol");
        assert_eq!(check_zero_knowledge_budget(&protocol), Ok(()));
        // Fewer blinding rows than the masks formula: the plan-derived check
        // catches it independently of descriptor rule 9.
        let mut short = descriptor.clone();
        short.blinding_factors = 3;
        let protocol = Protocol::new(&short).expect("protocol");
        let violation = check_zero_knowledge_budget(&protocol).expect_err("over budget");
        assert_eq!(violation.budget, 2);
        assert!(violation.revealed > violation.budget);
    }
}

/// The 32-byte messages of a proof.
fn messages(proof: &[u8]) -> Vec<&[u8]> {
    proof.chunks(MESSAGE_BYTES).collect()
}

#[test]
fn fresh_randomness_changes_every_message() {
    // Same witness, two seeds: every commitment and every evaluation of the
    // proof changes (spec section 15, zero-knowledge).
    for (circuit, choice) in [(0, CHOICES[0]), (1, CHOICES[1]), (2, CHOICES[3])] {
        let (first, second) = match circuit {
            0 => {
                let setup = setup::<Ep, _>(&ARITHMETIC, choice);
                let instances = ARITHMETIC.instances::<Fq>();
                (
                    setup.prove(&ARITHMETIC, &instances, 1).expect("proof"),
                    setup.prove(&ARITHMETIC, &instances, 2).expect("proof"),
                )
            }
            1 => {
                let setup = setup::<Eq, _>(&LOOKUPS, choice);
                (
                    setup.prove(&LOOKUPS, &[], 1).expect("proof"),
                    setup.prove(&LOOKUPS, &[], 2).expect("proof"),
                )
            }
            _ => {
                let setup = setup::<Ep, _>(&PERMUTATIONS, choice);
                let instances = PERMUTATIONS.instances::<Fq>();
                (
                    setup.prove(&PERMUTATIONS, &instances, 1).expect("proof"),
                    setup.prove(&PERMUTATIONS, &instances, 2).expect("proof"),
                )
            }
        };
        assert_eq!(first.len(), second.len());
        for (index, (a, b)) in messages(&first).iter().zip(messages(&second)).enumerate() {
            assert_ne!(*a, b, "circuit {circuit}, message {index}");
        }
    }
}

/// A recovery derivation that records its contexts.
fn recording_derivation(
    seed: [u8; 32],
    log: Arc<Mutex<Vec<[u8; 32]>>>,
) -> impl FnOnce(&[u8; 32]) -> Result<ChaCha20Rng, ()> + Send {
    move |context: &[u8; 32]| {
        log.lock().map_err(|_| ())?.push(*context);
        let key = crate::cs::descriptor::blake2b_personal::<32>(b"recovery-test-v1", &[
            &seed, context,
        ]);
        Ok(ChaCha20Rng::from_seed(key))
    }
}

#[test]
fn recovery_streams_bind_the_witness() {
    let setup = setup::<Ep, _>(&LOOKUPS, CHOICES[0]);
    let other = Lookups {
        offset: 4,
        ..LOOKUPS
    };
    let log = Arc::new(Mutex::new(Vec::new()));
    let prove = |circuit: &Lookups| {
        prove_circuit(
            &setup.params,
            &setup.pk,
            circuit,
            &[],
            ProverRandomness::recovery(recording_derivation([9; 32], Arc::clone(&log))),
            &ProverConfig::default(),
        )
        .expect("proof")
    };
    let first = prove(&LOOKUPS);
    let again = prove(&LOOKUPS);
    let different = prove(&other);
    // Deterministic for one witness and statement (recovery).
    assert_eq!(first, again);
    assert_eq!(setup.verify(&[], &first), Ok(()));
    assert_eq!(setup.verify(&[], &different), Ok(()));
    let contexts = log.lock().expect("log").clone();
    assert_eq!(contexts.len(), 3);
    assert_eq!(contexts[0], contexts[1]);
    assert_ne!(contexts[0], contexts[2]);
    // The statement is the same (no instances), so the context separates the
    // witnesses: the R commitment, a pure function of the stream position,
    // differs.
    let protocol = Protocol::new(setup.pk.binding().descriptor()).expect("protocol");
    let shape = protocol.shape();
    let r_index = shape.num_advice + 3 * shape.lookups + shape.permutation_sets;
    assert_ne!(messages(&first)[r_index], messages(&different)[r_index]);
    // A failing derivation is a typed error.
    assert_eq!(
        prove_circuit(
            &setup.params,
            &setup.pk,
            &LOOKUPS,
            &[],
            ProverRandomness::recovery(|_: &[u8; 32]| Err::<ChaCha20Rng, ()>(())),
            &ProverConfig::default(),
        ),
        Err(ProverError::RecoveryStream)
    );
}

#[test]
fn production_randomness_sources_prove() {
    let setup = setup::<Eq, _>(&ARITHMETIC, CHOICES[1]);
    let instances = ARITHMETIC.instances::<Fp>();
    let prove = |randomness: ProverRandomness<'_>| {
        prove_circuit(
            &setup.params,
            &setup.pk,
            &ARITHMETIC,
            &instances,
            randomness,
            &ProverConfig::default(),
        )
        .expect("proof")
    };
    let hedged = prove(ProverRandomness::hedged());
    let hedged_again = prove(ProverRandomness::hedged());
    let os = prove(ProverRandomness::os());
    for proof in [&hedged, &hedged_again, &os] {
        assert_eq!(setup.verify(&instances, proof), Ok(()));
    }
    assert_ne!(hedged, hedged_again);
    assert!(format!("{:?}", ProverRandomness::hedged()).contains("Hedged"));
}

#[test]
fn the_prover_rejects_mismatched_inputs() {
    let setup = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let instances = ARITHMETIC.instances::<Fq>();
    assert_eq!(
        Witness::from_circuit(&setup.pk, &ARITHMETIC, &[]).err(),
        Some(ProverError::InstanceColumns {
            expected: 1,
            found: 0
        })
    );
    assert_eq!(
        Witness::from_circuit(&setup.pk, &ARITHMETIC, &[vec![Fq::ONE]]).err(),
        Some(ProverError::InstanceLength {
            column: 0,
            expected: 2,
            found: 1
        })
    );
    assert_eq!(
        Witness::<Fq>::from_columns(&setup.pk, vec![], instances.clone()).err(),
        Some(ProverError::WitnessShape {
            expected: 3,
            found: 0
        })
    );
    assert_eq!(
        Witness::<Fq>::from_columns(&setup.pk, vec![vec![Fq::ZERO; 3]; 3], instances.clone())
            .err(),
        Some(ProverError::WitnessShape {
            expected: 64,
            found: 3
        })
    );
    // Another circuit shape (fewer rows enables fewer selectors).
    let shorter = Arithmetic {
        rows: 10,
        ..ARITHMETIC
    };
    assert_eq!(
        Witness::from_circuit(&setup.pk, &shorter, &shorter.instances::<Fq>()).err(),
        Some(ProverError::CircuitMismatch)
    );
    // Parameters of another k.
    let witness = Witness::from_circuit(&setup.pk, &ARITHMETIC, &instances).expect("witness");
    let large = PinnedParams::<Ep>::derive(K + 1).expect("params");
    assert_eq!(
        create_proof(
            &large,
            &setup.pk,
            &witness,
            ProverRandomness::fixed_seed_for_tests([1; 32]),
            &ProverConfig::default(),
        ),
        Err(ProverError::ParamsMismatch)
    );
    assert!(format!("{witness:?}").contains("advice_columns"));
    assert_eq!(witness.instances(), instances.as_slice());
    // A lookup input outside its table is a prover error.
    let lookups = setup_lookups();
    assert_eq!(
        lookups.prove(
            &Lookups {
                out_of_range: true,
                ..LOOKUPS
            },
            &[],
            1
        ),
        Err(ProverError::LookupInputMissing { lookup: 1 })
    );
    assert!(
        ProverError::LookupInputMissing { lookup: 1 }
            .to_string()
            .contains("lookup 1")
    );
}

fn setup_lookups() -> Setup<Ep> {
    setup::<Ep, _>(&LOOKUPS, CHOICES[0])
}

#[test]
fn oracle_mode_binds_the_injected_repr() {
    let setup = setup::<Eq, _>(&ARITHMETIC, CHOICES[1]);
    let instances = ARITHMETIC.instances::<Fp>();
    let witness = Witness::from_circuit(&setup.pk, &ARITHMETIC, &instances).expect("witness");
    let vendored = Fp::from(0x5eed);
    let proof = create_proof_oracle(
        &setup.params,
        &setup.pk,
        &witness,
        ProverRandomness::fixed_seed_for_tests([4; 32]),
        &ProverConfig::default(),
        vendored,
    )
    .expect("proof");
    assert_eq!(
        crate::verifier::verify_full_oracle(
            &setup.params,
            setup.pk.binding(),
            setup.pk.vk(),
            &instances,
            &proof,
            BUDGET,
            vendored,
        ),
        Ok(())
    );
    // Not a production proof, and not for another injected value.
    assert!(setup.verify(&instances, &proof).is_err());
    assert!(
        crate::verifier::verify_full_oracle(
            &setup.params,
            setup.pk.binding(),
            setup.pk.vk(),
            &instances,
            &proof,
            BUDGET,
            vendored + Fp::ONE,
        )
        .is_err()
    );
}

/// The malicious prover of soundness invariant S1: advice columns `a` and
/// `b` are committed to the same polynomial with the same blind, so their
/// commitments are equal. The claim for `b` is honest; the claim for `a` is
/// forged so that the gate identity holds at `x` although `a - b - 1 != 0`.
/// A verifier that grouped openings by commitment value and let the later
/// claim overwrite the earlier one would never check the forged claim; with
/// static grouping both slots are opened and the proof is rejected.
#[test]
fn equal_advice_commitments_with_different_evaluations_are_rejected() {
    let circuit = Forgeable { rows: 8 };
    let setup = setup::<Ep, _>(&circuit, CHOICES[0]);
    let pk = &setup.pk;
    let params = &setup.params;
    let descriptor = pk.binding().descriptor();
    let protocol = Protocol::new(descriptor).expect("protocol");
    let shape = *protocol.shape();
    assert_eq!((shape.permutation_sets, shape.lookups), (0, 0));
    let witness = Witness::from_circuit(pk, &circuit, &[]).expect("witness");
    let mut rng = ChaCha20Rng::seed_from_u64(77);
    let mut transcript =
        TranscriptWriter::<Ep, _>::new(DescriptorHash::<Ep>::production(descriptor.transcript));
    absorb_prelude::<Ep, _>(&mut transcript, pk.vk().transcript_repr(), &[]);
    let instance = InstanceColumns::new(pk, &[]).expect("instances");

    // Row 1: one blinded column, committed twice.
    let mut values = witness.advice()[0].clone();
    for value in &mut values[shape.usable_rows..] {
        *value = Fq::random(&mut rng);
    }
    let blind = Fq::random(&mut rng);
    let commitment = commit_lagrange(params.params(), &values, &blind, Secrecy::Secret, BUDGET)
        .expect("commit")
        .to_affine();
    transcript.write_point(&commitment).expect("a");
    transcript.write_point(&commitment).expect("b");
    let mut poly = values.clone();
    pk.domain().ifft(&mut poly).expect("ifft");
    let advice = Advice {
        values: vec![values.clone(), values],
        polys: vec![poly.clone(), poly.clone()],
        blinds: vec![blind, blind],
    };
    let theta = transcript.squeeze_challenge();
    let beta = transcript.squeeze_challenge();
    let gamma = transcript.squeeze_challenge();
    let random =
        commit_random(params, pk, &shape, &mut rng, &mut transcript, BUDGET).expect("random");
    let y = transcript.squeeze_challenge();
    let compiled = CompiledExpressions::compile(descriptor, true).expect("compile");
    let h = super::quotient::evaluate(
        pk,
        &protocol,
        &compiled,
        &QuotientInputs {
            advice: &advice.polys,
            instance: &instance.polys,
            permutation_products: Vec::new(),
            lookups: Vec::new(),
        },
        Challenges {
            theta,
            beta,
            gamma,
            y,
        },
    )
    .expect("quotient");
    let quotient = commit_quotient(params, pk, &shape, h, &mut rng, &mut transcript, BUDGET)
        .expect("pieces");
    let x = transcript.squeeze_challenge();
    let xn = x.pow_vartime([shape.n as u64]);
    let combined = quotient.combine(xn);
    let h_at_x = evaluate_polynomial(&combined.coeffs, x);

    // Evaluations: the forged claim for a, the honest one for b.
    let omega = pk.domain().omega();
    let omega_inv = pk.domain().omega_inv();
    let fixed_evals: Vec<Fq> = descriptor
        .fixed_queries
        .iter()
        .map(|query| {
            evaluate_polynomial(
                &pk.fixed_polys()[query.column as usize],
                rotate(x, omega, omega_inv, query.rotation),
            )
        })
        .collect();
    let honest = evaluate_polynomial(&poly, x);
    // The gate is s (a - b - 1); solve s (a* - b - 1) = h(x) (x^n - 1).
    let selector = fixed_evals[0];
    let forged = honest
        + Fq::ONE
        + h_at_x * (xn - Fq::ONE) * selector.invert().expect("nonzero selector");
    assert_ne!(forged, honest);
    let advice_evals = [forged, honest];
    assert_eq!(
        descriptor
            .advice_queries
            .iter()
            .map(|query| query.column)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    // With the forged claim the gate identity holds at x.
    let gate = evaluate_expression(&descriptor.gates[0][0], &fixed_evals, &advice_evals, &[])
        .expect("gate");
    assert_eq!(gate, h_at_x * (xn - Fq::ONE));
    for value in advice_evals.iter().chain(&fixed_evals) {
        transcript.write_scalar(value);
    }
    transcript.write_scalar(&evaluate_polynomial(&random.coeffs, x));
    let opened = Opened {
        instance: &instance,
        advice: &advice,
        products: &[],
        lookups: &[],
        random: &random,
        quotient: combined,
    };
    opened
        .open(params, pk, &protocol, x, &mut rng, &mut transcript, BUDGET)
        .expect("open");
    let proof = transcript.finish();
    assert_eq!(proof.len(), protocol.proof_length());
    assert_eq!(
        verify_full(&setup.params, pk.binding(), pk.vk(), &[], &proof, BUDGET),
        Err(VerifyError::Ipa(IpaError::OpeningFailed))
    );
}

#[test]
fn statement_digests_frame_the_instances() {
    let digest = [3_u8; 32];
    let repr = Fq::from(5);
    let base = statement_digest(&digest, &repr, &[vec![Fq::ONE], vec![]]);
    assert_ne!(base, statement_digest(&digest, &repr, &[vec![], vec![Fq::ONE]]));
    assert_ne!(base, statement_digest(&digest, &(repr + Fq::ONE), &[vec![Fq::ONE], vec![]]));
    assert_eq!(base, statement_digest(&digest, &repr, &[vec![Fq::ONE], vec![]]));
    assert_ne!(recovery_context(&[1; 32], &[2; 32]), recovery_context(&[2; 32], &[1; 32]));
    let _ = Fq::from_repr([0; 32]);
}

#[test]
fn errors_display_their_cause() {
    for error in [
        ProverError::CircuitMismatch,
        ProverError::ParamsMismatch,
        ProverError::Entropy,
        ProverError::RecoveryStream,
        ProverError::DegenerateChallenge,
        ProverError::IdentityInstanceCommitment { column: 1 },
        ProverError::from(TranscriptError::IdentityPoint),
        ProverError::from(MultiopenError::NoQueries),
        ProverError::from(ProtocolError::Overflow),
    ] {
        assert!(!error.to_string().is_empty());
    }
    assert_eq!(
        ProverError::from(CsError::Overflow),
        ProverError::Synthesis(frontend::Error::ConstraintSystem(Box::new(CsError::Overflow)))
    );
    let _ = (InstanceModeV1::Direct, TranscriptV1::Blake2bChallenge255);
}

/// Prove and verify timings of the arithmetic chain filling the usable rows
/// (run with `--release -- --ignored --nocapture`).
#[test]
#[ignore = "timing measurement; run in release"]
fn measure_prove_and_verify() {
    use std::time::Instant;

    for k in [10_u32, 12, 14] {
        let n = 1_usize << k;
        let circuit = Arithmetic {
            start: 3,
            rows: n - 8,
            tamper: None,
        };
        let instances = circuit.instances::<Fp>();
        let started = Instant::now();
        let params = PinnedParams::<Eq>::derive(k).expect("params");
        let derive = started.elapsed();
        let started = Instant::now();
        let pk = crate::keys::keygen_pk(
            &params,
            &circuit,
            &crate::test_circuits::keygen_config(CHOICES[0]),
        )
        .expect("pk");
        let keygen = started.elapsed();
        let witness = Witness::from_circuit(&pk, &circuit, &instances).expect("witness");
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool");
            let started = Instant::now();
            let proof = pool
                .install(|| {
                    create_proof(
                        &params,
                        &pk,
                        &witness,
                        ProverRandomness::fixed_seed_for_tests([1; 32]),
                        &ProverConfig::default(),
                    )
                })
                .expect("proof");
            let prove = started.elapsed();
            let started = Instant::now();
            pool.install(|| {
                verify_full(&params, pk.binding(), pk.vk(), &instances, &proof, BUDGET)
            })
            .expect("verify");
            let verify = started.elapsed();
            println!(
                "k={k} threads={threads}: params {derive:?}, keygen {keygen:?}, prove {prove:?}, \
                 verify {verify:?}, proof {} bytes",
                proof.len()
            );
        }
    }
}

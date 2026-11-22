//! Local Core30 ordinary step proofs and the lean Receive incoming-only Q0 checkpoint. The root is
//! independent witness data carried by both heads, never a verifying-key input.

use super::{SigmaCircuit, SigmaParams};
use crate::{
    Mutation, SigmaProver, SigmaRelation, SigmaShape, StateV1, StepPublic, sample_witness,
    wallet_monetary_shape, witness::CORE_DOMAIN,
};
use iroha_pasta::{Eq, Fp, poseidon::hash_with_domain};
use iroha_plonk::{ProverConfig, ProverRandomness, prove_circuit};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn candidate_head(state: &StateV1<Fp>, prior_root: Fp) -> Fp {
    let core = state.core.fields();
    let mut fields = Vec::from(&core[..8]);
    fields.push(prior_root);
    fields.extend_from_slice(&core[8..]);
    fields.push(state.rest.digest());
    assert_eq!(fields.len(), 31);
    hash_with_domain(CORE_DOMAIN, &fields)
}

#[test]
#[ignore = "generates a local Core30 candidate key and measures one complete Receive proof"]
fn local_receive_proof_binds_the_prior_root_and_candidate_statement() {
    drop(candidate_step(SigmaRelation::RECEIVE));
}

fn candidate_step(
    relation: SigmaRelation,
) -> (
    crate::SigmaVerifier<Eq>,
    crate::SigmaProof<Fp>,
    StepPublic<Fp>,
) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let seed = now.as_secs().rotate_left(32) ^ u64::from(now.subsec_nanos());
    let witness = sample_witness::<Fp>(seed, relation, Mutation::None);
    prove_candidate_step(relation, witness, Fp::from(seed | 1))
}

fn prove_candidate_step(
    relation: SigmaRelation,
    mut witness: crate::StepWitness<Fp>,
    prior_root: Fp,
) -> (
    crate::SigmaVerifier<Eq>,
    crate::SigmaProof<Fp>,
    StepPublic<Fp>,
) {
    let base = wallet_monetary_shape(relation).unwrap();
    let mut layout = base.params.relation();
    layout.core30_candidate = true;
    let shape = SigmaShape::new(
        SigmaParams::new(layout, base.params.lanes(), base.params.limb_bits()).unwrap(),
        base.k,
    );
    let native = witness.evaluate(relation);
    assert!(native.is_honest());
    let ordinary = native.public();
    let statement = witness.statement(relation).unwrap();
    let successor = native.successor_state.as_ref().unwrap();
    let candidate_statement = |root| {
        let mut expected = statement.clone();
        expected.predecessor = candidate_head(&witness.predecessor, root);
        expected.successor = candidate_head(successor, root);
        expected
    };
    let expected = candidate_statement(prior_root);
    let changed_root = StepPublic {
        statement: candidate_statement(prior_root + Fp::from(1))
            .digest()
            .unwrap(),
    };
    assert_ne!(expected.predecessor, statement.predecessor);
    assert_ne!(expected.successor, statement.successor);
    witness.core30_prior_root = Some(prior_root);
    let circuit = SigmaCircuit::new(shape.params, witness);
    let candidate = circuit.native().unwrap();
    assert_eq!(candidate.statement, expected.encode().unwrap());
    assert_eq!(candidate.digests.statement, expected.digest().unwrap());
    let public = candidate.public();
    assert_ne!(public, ordinary);
    assert_ne!(public, changed_root);
    let workers = 4;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap();
    pool.install(|| {
        let start = Instant::now();
        let prover = SigmaProver::<Eq>::keygen(shape).unwrap();
        let verifier = prover.verifier();
        let keygen = start.elapsed();
        let start = Instant::now();
        let proof = prove_circuit(
            prover.params(),
            prover.proving_key(),
            &circuit,
            &[public.instance()],
            ProverRandomness::hedged(),
            ProverConfig::default(),
        )
        .unwrap();
        let proving = start.elapsed();
        let start = Instant::now();
        verifier.verify(&public, &proof).unwrap();
        let verifying = start.elapsed();
        assert_eq!(proof.len(), shape.proof_length::<Eq>().unwrap());
        assert!(verifier.verify(&ordinary, &proof).is_err());
        assert!(verifier.verify(&changed_root, &proof).is_err());
        let role = match relation.step() {
            crate::StepRelation::Receive => "RECEIVE",
            crate::StepRelation::Send => "SEND",
        };
        println!(
            "CORE30_{role} prior_root={prior_root:?} core29_statement={:?} \
             core30_statement={:?} k={} lanes={} workers={workers} proof_bytes={} \
             keygen_ms={:.3} prove_ms={:.3} verify_ms={:.3}",
            ordinary.statement,
            public.statement,
            shape.k,
            shape.params.lanes(),
            proof.len(),
            keygen.as_secs_f64() * 1e3,
            proving.as_secs_f64() * 1e3,
            verifying.as_secs_f64() * 1e3,
        );
        (
            verifier,
            crate::SigmaProof {
                public,
                bytes: proof,
            },
            ordinary,
        )
    })
}

#[cfg(unix)]
#[test]
#[ignore = "generates a local candidate Send key and an incoming-only k16 Q proof; run in release"]
fn candidate_send_binds_lean_receive_incoming_q() {
    let total = Instant::now();
    let relation = crate::common::SEND_EVERY;
    let (verifier, send, ordinary_statement) = candidate_step(relation);
    prove_incoming_q(relation, verifier, send, ordinary_statement, 5, total);
}

#[cfg(unix)]
fn prove_incoming_q(
    relation: SigmaRelation,
    verifier: crate::SigmaVerifier<Eq>,
    send: crate::SigmaProof<Fp>,
    ordinary_statement: StepPublic<Fp>,
    expected_selector: u8,
    total: Instant,
) -> (
    crate::q_sigma::native::QSigmaProof,
    iroha_plonk::VerifyingKey<iroha_pasta::Ep>,
    iroha_plonk::DescriptorBinding,
) {
    use crate::a_relation::schedule::sigma_selector;
    use crate::q_sigma::{
        QSigmaPlan, SigmaClass, SigmaSlotWitness,
        native::{
            IncomingMode, IncomingSigma, QSigmaError, QSigmaFoldSource, QSigmaProver, QSigmaSource,
        },
    };
    use ff::{Field, PrimeField};
    use iroha_pasta::{Ep, Fq, PastaField, msm::MemoryBudget};
    use iroha_plonk::{pcs::ipa::PinnedParams, verify_full};
    use iroha_plonk_recursion::{FoldConfig, FoldTape};
    use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};
    use std::io::Read;

    let (tag, mask) = relation.selector();
    let selector = sigma_selector(tag, u8::try_from(mask).unwrap()).unwrap();
    assert_eq!(selector, expected_selector);

    rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(|| {
        let class = SigmaClass::from_verifiers(&[(selector, &verifier)]).unwrap();
        let candidate_digest = verifier.verifying_key_digest().unwrap();
        assert_eq!(class.selector_key_digest(selector).unwrap().to_repr(), candidate_digest);
        let params = crate::common::vesta_params(16);
        let plan = QSigmaPlan::incoming_only(class, &params).unwrap();
        assert!(!plan.has_own());
        assert_eq!(plan.slot_count(), 1);
        assert_eq!(plan.position(QSigmaPlan::INCOMING), Some(0));
        let source = QSigmaSource::incoming_only(plan.clone(), verifier.vk().clone()).unwrap();
        let ordinary = crate::common::vesta_prover(wallet_monetary_shape(relation).unwrap()).verifier();
        assert_ne!(ordinary.verifying_key_digest().unwrap(), candidate_digest);
        assert!(ordinary.verify(&send.public, &send.bytes).is_err());
        assert!(matches!(QSigmaSource::incoming_only(plan.clone(), ordinary.vk().clone()),
            Err(QSigmaError::UnauthorizedKey)));
        let slot = SigmaSlotWitness {
            key: verifier.vk().clone(), statement: send.public.statement,
            length: u32::try_from(send.bytes.len()).unwrap(), proof: send.bytes,
        };
        let budget = MemoryBudget::DEFAULT;
        let started = Instant::now();
        let opening = plan.incoming_original(&slot, selector, budget).unwrap().unwrap();
        opening.decide(&params, budget).unwrap();
        let mut seed = [0; 32];
        std::fs::File::open("/dev/urandom").unwrap().read_exact(&mut seed).unwrap();
        let mut rng = ChaCha20Rng::from_seed(seed);
        let tape = FoldTape::<Eq>::try_fill::<std::convert::Infallible>(|coin| {
            *coin = Fp::random(&mut rng);
            Ok(())
        }).unwrap();
        let public_statement = Fq::from_canonical_limbs(slot.statement.to_canonical_limbs()).unwrap();
        let prepared = source.plan().prepare(None, Some(IncomingSigma { sigma: slot, mode: IncomingMode::Accept }),
            &params, QSigmaFoldSource::Fresh(tape), &FoldConfig::default()).unwrap();
        assert_eq!(prepared.instances()[0][0], public_statement);
        assert_eq!(prepared.instances()[2], [Fq::from(u64::from(selector))]);
        assert_eq!(prepared.instances()[3], [Fq::ONE, Fq::ONE, Fq::ZERO, Fq::ZERO]);
        prepared.part().decide(&params, budget).unwrap();
        let prepare_time = started.elapsed();

        let started = Instant::now();
        let outer = PinnedParams::<Ep>::derive(16).unwrap();
        // Same two-range-bus serialization as QSigmaSource::source_circuit.
        let q = QSigmaProver::keygen_serialized_foreign(&prepared, outer, 2).unwrap();
        let keygen_time = started.elapsed();
        let started = Instant::now();
        let proof = q.prove(&prepared, ProverRandomness::hedged(), ProverConfig::default()).unwrap();
        let prove_time = started.elapsed();
        assert_eq!(proof.instances, prepared.instances());
        assert_eq!(proof.part, *prepared.part());
        let started = Instant::now();
        verify_full(q.params(), q.binding(), q.verifying_key(), prepared.instances(), &proof.bytes, budget).unwrap();
        let memoized_time = started.elapsed();
        // The prover's parameter owner already remembers its self-verification. A newly
        // derived owner has identical generators and an empty verification memo.
        let independent = PinnedParams::<Ep>::derive(16).unwrap();
        let started = Instant::now();
        verify_full(
            &independent,
            q.binding(),
            q.verifying_key(),
            prepared.instances(),
            &proof.bytes,
            budget,
        )
        .unwrap();
        let independent_time = started.elapsed();
        drop(independent);
        let mut changed = proof.instances.clone();
        changed[0][0] = Fq::from_canonical_limbs(ordinary_statement.statement.to_canonical_limbs()).unwrap();
        assert_ne!(changed[0][0], public_statement);
        let independent_wrong = PinnedParams::<Ep>::derive(16).unwrap();
        let started = Instant::now();
        assert!(verify_full(
            &independent_wrong,
            q.binding(),
            q.verifying_key(),
            &changed,
            &proof.bytes,
            budget,
        )
        .is_err());
        let rejected_time = started.elapsed();
        let operation = match relation.step() {
            crate::StepRelation::Send => "RECEIVE",
            crate::StepRelation::Receive => "ARCHIVE_RECEIVE",
        };
        println!(
            "CORE30_{operation}_INCOMING_Q0 selector={selector} k=16 buses=2 workers=4 q_bytes={} \
             q_prepare_ms={:.3} q_keygen_ms={:.3} q_prove_selfverify_ms={:.3} \
             q_memoized_verify_ms={:.3} q_fresh_verify_ms={:.3} q_fresh_reject_ms={:.3} total_ms={:.3}",
            proof.bytes.len(),
            prepare_time.as_secs_f64() * 1e3,
            keygen_time.as_secs_f64() * 1e3,
            prove_time.as_secs_f64() * 1e3,
            memoized_time.as_secs_f64() * 1e3,
            independent_time.as_secs_f64() * 1e3,
            rejected_time.as_secs_f64() * 1e3,
            total.elapsed().as_secs_f64() * 1e3,
        );
        (proof, q.verifying_key().clone(), q.binding().clone())
    })
}

// These existing real-signature and indexed-map fixtures are also used by the
// ordinary integration owners. The candidate only changes the head commitments.
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tests/common/bootstrap.rs"]
mod bootstrap;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tests/common/bootstrap_objects.rs"]
mod bootstrap_objects;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tests/common/load_objects.rs"]
mod load_objects;
#[cfg(unix)]
#[path = "core30_paired_request_test.rs"]
mod paired_request;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tests/common/receive_objects.rs"]
mod receive_objects;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tests/common/send_objects.rs"]
mod send_objects;

#[cfg(unix)]
#[path = "core30_funded_predecessor_test.rs"]
mod funded_predecessor;
#[cfg(unix)]
#[path = "core30_incoming_omega_test.rs"]
mod incoming_omega;
#[cfg(unix)]
#[path = "../tests/common/ordinary_load_receipt.rs"]
mod ordinary_load_receipt;

#[cfg(unix)]
use crate::common;
#[cfg(unix)]
#[allow(dead_code)]
mod load_chain {
    use crate::common;
    pub(super) mod bootstrap_outer {
        pub(crate) use crate::omega_boundary_tests::RootedBootstrapOmega;
    }
    include!("../tests/common/proof_fixtures/a_load_recursive_body.rs");
}
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tests/common/ordinary_load_fixture.rs"]
mod ordinary_load_fixture;

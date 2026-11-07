//! The compression wraps end to end on the toy two-curve chain (iroha_fold's
//! fixtures: Q steps with the genuine Receive signature leaf, toy A steps,
//! each side keeping its claim accumulator): the native producer's P and
//! Omega are satisfied, a changed crossing row or public value is not, both
//! proofs are made, Omega verifies natively, and its opening, `acc_V` and
//! `acc_P` decide. The test prints each wrap's rows and each stage's time.

#[path = "../../iroha_fold/tests/common/mod.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
mod common;

use std::{sync::Arc, time::Instant};

use common::{
    qstep::{QStep, leaf},
    step::{
        BASE, CLAIM_A, CLAIM_Q, DIGESTS, DOMAIN_A, DOMAIN_Q, IVC_A, IVC_Q, K, Side, Step, business,
        business_digests, claim_a, claim_q,
    },
};
use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    fold::fresh_instance,
    ivc::{self, Fresh, Kept, embed, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::{
    a_relation::LINEAGE_FIELDS,
    fold_wrap::{
        Domains, WrapSpec,
        native::{self, FinalChain, WrapTapes},
    },
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, PastaField, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    frontend::{Circuit, synthesize},
    pcs::ipa::PinnedParams,
    verifier::accumulate_generator,
};
use iroha_plonk_recursion::{FoldConfig, FoldInput, FoldTape, verifier::diagnostic_take_usage};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

/// The toy chain's relations under the monomial generators.
struct Relations {
    q: FoldedRelation<Fq>,
    key_q: CommitKey<Ep>,
    a: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
    params_q: PinnedParams<Ep>,
    params_a: PinnedParams<Eq>,
}

fn relations() -> Relations {
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![3],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let a_relation = |shape: FoldShape| {
        let layout = synthesize(
            &Step::<Ep> {
                side: Side::A,
                shape,
                witness: None,
            },
            K,
            None,
        )
        .expect("A layout");
        FoldedRelation::<Fp>::from_synthesized(&layout).expect("A relation")
    };
    let first = a_relation(placeholder).shape();
    let q_step = QStep {
        leaf: leaf(0).without_witnesses(),
        shape: first,
        witness: None,
    };
    let q =
        FoldedRelation::<Fq>::from_synthesized(&synthesize(&q_step, K, None).expect("Q layout"))
            .expect("Q relation");
    let a = a_relation(q.shape());
    let params_q = PinnedParams::<Ep>::derive(K).expect("params");
    let params_a = PinnedParams::<Eq>::derive(K).expect("params");
    Relations {
        key_q: CommitKey::monomial(params_q.params()),
        key_a: CommitKey::monomial(params_a.params()),
        q,
        a,
        params_q,
        params_a,
    }
}

/// Runs Q1, A2, Q3, A4 and returns the finished chain: `A_4`, the Q
/// accumulator `A_4` keeps (`Q_1`, `Q_3`), the A accumulator `Q_3` keeps
/// (`A_2`), both states and both claim accumulators.
fn chain(rel: &Relations) -> FinalChain {
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    let (q_chain, a_chain) = (Side::Q.chain(), Side::A.chain());
    let (mut q_state, mut a_state) = (Fq::from(BASE), Fp::from(BASE));
    let (mut q_digests, mut a_digests) = (vec![Fq::ZERO; DIGESTS], vec![Fp::ZERO; DIGESTS]);
    let (mut q_kept, mut a_kept): (Option<Kept<Eq>>, Option<Kept<Ep>>) = (None, None);
    let (mut last_a, mut last_q): (Option<Fresh<Eq>>, Option<Fresh<Ep>>) = (None, None);
    let start = embed(&ivc::ivc_hash::<Fp>(
        &IVC_A,
        0,
        Fp::from(BASE),
        &[Fp::from(BASE)],
        &[Fp::ZERO; DIGESTS],
    ));
    for j in 1..=4_u64 {
        if j % 2 == 1 {
            let prepared = prepare_step(
                &q_chain,
                j,
                &rel.a,
                q_kept.as_ref(),
                last_a.as_ref(),
                vec![q_state],
                q_digests.clone(),
                start,
            )
            .expect("prepare");
            let next = business(Side::Q, q_state, prepared.bridge[0]);
            q_digests = business_digests(Side::Q, j == 1, prepared.digests.clone());
            let leaf = leaf(j);
            let leaf_public =
                leaf.instances(&[true, true, true]).expect("leaf instances")[0].clone();
            let frame = step_public(
                &q_chain,
                j,
                Fq::from(BASE),
                prepared.passthrough,
                &[next],
                &q_digests,
                &[Fq::from(j)],
            );
            let circuit = QStep {
                leaf,
                shape: rel.a.shape(),
                witness: Some(prepared.witness),
            };
            let synthesized =
                synthesize(&circuit, K, Some(&[frame, leaf_public])).expect("witness");
            let (advice, public) = rel.q.stage_witness(&synthesized).expect("stage");
            last_q = Some(
                fresh_instance(&rel.q, &rel.key_q, &DOMAIN_Q, advice, public, &mut rng)
                    .expect("fresh"),
            );
            q_kept = prepared.kept;
            q_state = next;
        } else {
            let prepared = prepare_step(
                &a_chain,
                j,
                &rel.q,
                a_kept.as_ref(),
                last_q.as_ref(),
                vec![a_state],
                a_digests.clone(),
                Fp::ZERO,
            )
            .expect("prepare");
            let next = business(Side::A, a_state, prepared.bridge[0]);
            a_digests = business_digests(Side::A, j == 2, prepared.digests.clone());
            let public = vec![step_public(
                &a_chain,
                j,
                Fp::from(BASE),
                prepared.passthrough,
                &[next],
                &a_digests,
                &[Fp::from(j)],
            )];
            let circuit = Step::<Ep> {
                side: Side::A,
                shape: rel.q.shape(),
                witness: Some(prepared.witness),
            };
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.a.stage_witness(&synthesized).expect("stage");
            last_a = Some(
                fresh_instance(&rel.a, &rel.key_a, &DOMAIN_A, advice, public, &mut rng)
                    .expect("fresh"),
            );
            a_kept = prepared.kept;
            a_state = next;
        }
    }
    FinalChain {
        step: 4,
        a_state: vec![a_state],
        q_state: vec![q_state],
        u_q: a_kept.expect("A keeps the Q accumulator"),
        u_a: q_kept.expect("Q keeps the A accumulator"),
        a_n: last_a.expect("A_4"),
        pallas_claims: claim_a().clone(),
        vesta_claims: claim_q().clone(),
    }
}

/// A deterministic test tape (SplitMix64 words; production samples OS coins).
fn tape<C: PastaCurve>(seed: u64) -> FoldTape<C> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    FoldTape::<C>::try_fill::<core::convert::Infallible>(|coin| {
        *coin = C::ScalarExt::from_raw_reduced([next(), next(), next(), next() >> 2]);
        Ok(())
    })
    .expect("infallible")
}

fn usage(label: &str) {
    for entry in diagnostic_take_usage() {
        println!(
            "{label} rows: sponge {}, glue {}, ECC {}, range {}",
            entry.sponge, entry.arithmetic, entry.curve, entry.range
        );
    }
}

#[test]
#[ignore = "toy chain with genuine P-256 leaves, two k16 keygens, checks and proofs; run optimized"]
fn the_wraps_finish_the_toy_chain_into_an_omega_that_decides() {
    let started = Instant::now();
    let rel = relations();
    let chain = chain(&rel);
    println!("chain: {:.1?}", started.elapsed());
    let spec = Arc::new(WrapSpec {
        q: rel.q.clone(),
        a: rel.a.clone(),
        key_q: rel.key_q.clone(),
        key_a: rel.key_a.clone(),
        params_q: rel.params_q.clone(),
        params_a: rel.params_a.clone(),
        domains: Domains {
            q_fold: DOMAIN_Q,
            a_fold: DOMAIN_A,
            a_ivc: IVC_A,
            q_ivc: IVC_Q,
            pallas_claims: CLAIM_A,
            vesta_claims: CLAIM_Q,
        },
        base_a: Fp::from(BASE),
        base_q: Fq::from(BASE),
        a_state: 1,
        q_state: 1,
        lineage: [0; LINEAGE_FIELDS],
    });
    let now = Instant::now();
    let keys = native::keygen(Arc::clone(&spec)).expect("keys");
    println!(
        "keygen P and Omega: {:.1?}; P has {} public rows",
        now.elapsed(),
        keys.p_plan.layout.rows()
    );
    let _ = diagnostic_take_usage();
    let config = FoldConfig::default();
    let tapes = WrapTapes {
        pallas_restructure: tape(1),
        pallas_fold: tape(2),
        vesta_restructure: tape(3),
        vesta_fold: tape(4),
    };
    let now = Instant::now();
    let prepared = native::prepare(&keys, &chain, tapes, &config).expect("native wraps");
    println!(
        "native deciders, folds and restructurings: {:.1?}",
        now.elapsed()
    );

    // P is satisfied by the native values and by nothing else.
    let p = native::p_circuit(&keys, &prepared);
    let report = check_circuit(&p, K, &[prepared.p_public.clone()], CheckMode::Strict).expect("P");
    usage("P");
    assert!(report.is_satisfied(), "P: {:?}", report.failures().first());
    let layout = keys.p_plan.layout;
    let parsed = layout.parse(&prepared.p_public).expect("P layout");
    for (label, row) in [
        ("h_out_n", 1),
        ("D_A", layout.a_public),
        (
            "Q alpha lo",
            prepared
                .p_public
                .iter()
                .position(|v| *v == parsed.q.alpha[0])
                .expect("row"),
        ),
        ("last row", layout.rows() - 1),
    ] {
        let mut changed = prepared.p_public.clone();
        changed[row] += Fp::ONE;
        let report = check_circuit(&p, K, &[changed], CheckMode::Strict).expect("P");
        let _ = diagnostic_take_usage();
        assert!(!report.is_satisfied(), "P accepts another {label}");
    }

    let now = Instant::now();
    let (p_proof, p_opening) = native::prove_p(&keys, &prepared).expect("P proof");
    println!("P proof ({} bytes): {:.1?}", p_proof.len(), now.elapsed());
    let acc_p = prepared.acc_p.clone();
    let now = Instant::now();
    let (omega, instances, acc_v) =
        native::omega_circuit(&keys, prepared, p_proof, &p_opening, &config).expect("Omega");
    println!("acc_V fold: {:.1?}", now.elapsed());

    // Omega is satisfied, and not with another D_A or acc_V.
    let report = check_circuit(&omega, K, &instances, CheckMode::Strict).expect("Omega");
    usage("Omega");
    assert!(
        report.is_satisfied(),
        "Omega: {:?}",
        report.failures().first()
    );
    for (column, row) in [(0, 0), (1, 0), (2, 15)] {
        let mut changed = instances.clone();
        changed[column][row] += Fq::ONE;
        let report = check_circuit(&omega, K, &changed, CheckMode::Strict).expect("Omega");
        let _ = diagnostic_take_usage();
        assert!(
            !report.is_satisfied(),
            "Omega accepts another column {column} row {row}"
        );
    }

    let now = Instant::now();
    let columns = Witness::from_circuit(&keys.omega_key, &omega, &instances).expect("witness");
    let output = create_proof_owned_with_claim(
        &spec.params_q,
        &keys.omega_key,
        columns,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .expect("Omega proof");
    println!(
        "Omega proof ({} bytes): {:.1?}",
        output.proof.len(),
        now.elapsed()
    );

    // The consumer's checks: Omega verifies, its opening, acc_V and acc_P decide.
    let budget = MemoryBudget::DEFAULT;
    let claim = accumulate_generator(
        &spec.params_q,
        keys.omega_key.binding(),
        keys.omega_key.vk(),
        &instances,
        &output.proof,
        budget,
    )
    .expect("Omega verifies");
    FoldInput::from_opening(*claim.g(), claim.challenges())
        .expect("claim")
        .decide(&spec.params_q, budget)
        .expect("Omega's opening decides");
    acc_v.decide(&spec.params_a, budget).expect("acc_V decides");
    acc_p.decide(&spec.params_q, budget).expect("acc_P decides");
    println!("total: {:.1?}", started.elapsed());
}

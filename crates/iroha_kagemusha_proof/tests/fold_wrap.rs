//! The compression wraps end to end on the two-kind toy chain of
//! `support/fold_wrap_toy.rs` (two kinds per side, today's 20-word A state,
//! a second A public column).
//!
//! A chain of four ends with `A_b` while `Q_b` kept no `A_b`, so R starts that
//! accumulator. A chain of eight ends with `A_b` folding into the kept one.
//! For both chains:
//! - the native producer's R, P and Omega are satisfied;
//! - a changed crossing row or public value is rejected, including an R row
//!   P's copy of R's public column changes;
//! - all three proofs are made, Omega verifies natively, and its opening,
//!   `acc_V` and `acc_P` decide.
//!
//! The wraps' sizes without witnesses (the sizing other relations use) equal
//! the rows the checked wraps use. The Omega fixture other tests use carries
//! the chosen lineage in `D_A` and verifies. The tests print each wrap's rows
//! and each stage's time.

#[path = "support/fold_wrap_toy.rs"]
mod fold_wrap_toy;

use std::{sync::Arc, time::Instant};

use ff::{Field, PrimeField};
use fold_wrap_toy::{A_A, A_B, A_CONTEXT, A_PC, K};
use iroha_kagemusha_proof::{
    a_relation::{LINEAGE_DOMAIN, LINEAGE_FIELDS},
    fold_wrap::{
        native::{self, FinalChain},
        p::{PCircuit, PWitness},
    },
};
use iroha_pasta::{Ep, Eq, Fp, Fq, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    pcs::ipa::PinnedParams,
    transcript::generator_coordinates,
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::statement::foreign_limbs;
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, FoldWitness,
    verifier::{VerifierUsage, diagnostic_take_usage},
};

/// The lineage the chains' `A_b` write.
fn lineage(seed: u64) -> [Fp; LINEAGE_FIELDS] {
    core::array::from_fn(|i| Fp::from(seed + 7 * i as u64))
}

fn copy_fold(witness: &FoldWitness<Ep>) -> FoldWitness<Ep> {
    FoldWitness::from_private_bytes(&witness.to_private_bytes()).expect("canonical witness")
}

/// P whose copy of R's public column has one row changed, so R's proof no
/// longer verifies inside P.
fn with_r_row_changed(keys: &native::WrapKeys, stage: &native::PStage, row: usize) -> PCircuit {
    let original: &PWitness = &stage.p_witness;
    let mut r_public = original.r_public.clone();
    r_public[row] += Fq::ONE;
    let witness = PWitness {
        step: original.step,
        state: original.state.clone(),
        r_public,
        r_proof: original.r_proof.clone(),
        q: original.q.clone(),
        a: original.a.clone(),
        pallas: original.pallas.clone(),
        vesta: original.vesta.clone(),
        restructure: copy_fold(&original.restructure),
        fold: copy_fold(&original.fold),
    };
    PCircuit {
        plan: Arc::clone(&keys.p_plan),
        witness: Some(Arc::new(witness)),
    }
}

/// Wraps one chain: R, P and Omega checked (with mutations when `mutate`),
/// proved, Omega verified natively and every carried accumulator decided.
/// Returns the rows R, P and Omega used.
#[allow(clippy::too_many_lines)]
fn wrap(
    keys: &native::WrapKeys,
    chain: &FinalChain,
    seed: u64,
    mutate: bool,
) -> [VerifierUsage; 3] {
    let spec = &keys.p_plan.spec;
    let config = FoldConfig::default();
    let _ = diagnostic_take_usage();
    let now = Instant::now();
    let prepared =
        native::prepare(keys, chain, fold_wrap_toy::tapes(seed), &config).expect("native wraps");
    println!(
        "native deciders, folds and restructurings: {:.1?}",
        now.elapsed()
    );
    // R.
    let r = native::r_circuit(keys, &prepared);
    let report = check_circuit(&r, K, &[prepared.r_public.clone()], CheckMode::Strict).expect("R");
    let r_rows = fold_wrap_toy::take_rows("R").expect("R rows");
    assert!(report.is_satisfied(), "R: {:?}", report.failures().first());
    if mutate {
        let layout = &keys.r_plan.layout;
        for (label, row) in [
            ("passthrough", 0),
            ("a value of A_n's second column", layout.a_public - 1),
            ("an A member digest", layout.a_public),
            ("the claim digest", layout.rows() - 1),
        ] {
            let mut changed = prepared.r_public.clone();
            changed[row] += Fq::ONE;
            let report = check_circuit(&r, K, &[changed], CheckMode::Strict).expect("R");
            let _ = diagnostic_take_usage();
            assert!(!report.is_satisfied(), "R accepts another {label}");
        }
    }
    let now = Instant::now();
    let (r_proof, r_opening) = native::prove_r(keys, &prepared).expect("R proof");
    println!("R proof ({} bytes): {:.1?}", r_proof.len(), now.elapsed());
    let stage = native::p_stage(keys, prepared, r_proof, &r_opening, &config).expect("P stage");
    // P (R's proving recorded R's rows again).
    let _ = diagnostic_take_usage();
    let p = native::p_circuit(keys, &stage);
    let report = check_circuit(&p, K, &[stage.p_public.clone()], CheckMode::Strict).expect("P");
    let p_rows = fold_wrap_toy::take_rows("P").expect("P rows");
    assert!(report.is_satisfied(), "P: {:?}", report.failures().first());
    if mutate {
        let layout = &keys.p_plan.layout;
        let parsed = layout.parse(&stage.p_public).expect("layout");
        let row_of = |value: &Fp| stage.p_public.iter().position(|v| v == value).expect("row");
        for (label, row) in [
            ("D_A", 0),
            ("a Q member digest", row_of(&parsed.q.members[1].acc_digest)),
            ("an A claim challenge", row_of(&parsed.a_claim.u[3])),
            ("the last row", layout.rows() - 1),
        ] {
            let mut changed = stage.p_public.clone();
            changed[row] += Fp::ONE;
            let report = check_circuit(&p, K, &[changed], CheckMode::Strict).expect("P");
            let _ = diagnostic_take_usage();
            assert!(!report.is_satisfied(), "P accepts another {label}");
        }
        // A_n's h_out as R hands it to P.
        let circuit = with_r_row_changed(keys, &stage, 1);
        let report =
            check_circuit(&circuit, K, &[stage.p_public.clone()], CheckMode::Strict).expect("P");
        let _ = diagnostic_take_usage();
        assert!(!report.is_satisfied(), "P accepts another R row");
    }
    let now = Instant::now();
    let (p_proof, p_opening) = native::prove_p(keys, &stage).expect("P proof");
    println!("P proof ({} bytes): {:.1?}", p_proof.len(), now.elapsed());
    let acc_p = stage.acc_p.clone();
    let (omega, instances, acc_v) =
        native::omega_circuit(keys, stage, p_proof, &p_opening, &config).expect("Omega");
    let _ = diagnostic_take_usage();
    let report = check_circuit(&omega, K, &instances, CheckMode::Strict).expect("Omega");
    let omega_rows = fold_wrap_toy::take_rows("Omega").expect("Omega rows");
    assert!(
        report.is_satisfied(),
        "Omega: {:?}",
        report.failures().first()
    );
    if mutate {
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
    [r_rows, p_rows, omega_rows]
}

#[test]
#[ignore = "two-kind toy chains, k16 keygens, checks and proofs; run optimized"]
fn the_wraps_finish_two_kind_chains_into_omegas_that_decide() {
    let _serial = fold_wrap_toy::serial();
    let started = Instant::now();
    let toy = fold_wrap_toy::toy();
    let keys = &toy.keys;
    // Four half-steps: Q_b kept no A_b, so R starts it from A_4.
    let short = fold_wrap_toy::chain(&toy.relations, 4, &lineage(3));
    assert!(short.u_a[A_B].is_none() && short.u_a[A_A].is_some());
    let rows = wrap(keys, &short, 1, true);
    // Eight half-steps: A_8 folds into the A_b accumulator Q_7 kept.
    let long = fold_wrap_toy::chain(&toy.relations, 8, &lineage(5));
    assert!(long.u_a.iter().all(Option::is_some));
    assert_eq!(wrap(keys, &long, 11, false), rows, "the wraps' rows");
    // Sizing without witnesses counts exactly the rows the wraps use.
    let now = Instant::now();
    let sizes = fold_wrap_toy::sizes(&keys.p_plan.spec);
    println!("sizes without witnesses: {:.1?}", now.elapsed());
    assert_eq!(sizes.len(), 3, "every wrap is sized");
    for (size, used) in sizes.iter().zip(rows) {
        assert!(size.fits, "{} fits k16", size.wrap);
        assert_eq!(size.usage, used, "{}'s rows", size.wrap);
    }
    println!("total: {:.1?}", started.elapsed());
}

#[test]
#[ignore = "toy chain, k16 keygens and proofs; run optimized"]
fn the_toy_omega_fixture_carries_the_chosen_lineage() {
    let _serial = fold_wrap_toy::serial();
    let lineage = lineage(1_000);
    let now = Instant::now();
    let fixture = fold_wrap_toy::omega_fixture(lineage);
    println!("Omega fixture: {:.1?}", now.elapsed());
    // The final A state: the lineage at an operation boundary.
    assert_eq!(fixture.z_a[..LINEAGE_FIELDS], lineage);
    assert_eq!(
        (fixture.z_a.len(), fixture.z_a[A_CONTEXT], fixture.z_a[A_PC]),
        (LINEAGE_FIELDS + 2, Fp::ZERO, Fp::ZERO)
    );
    // D_A hashes the lineage and acc_P (today's 52-word preimage).
    let acc_p = AccumulatorT::<Ep>::from_bytes(&fixture.acc_p).expect("acc_P");
    let acc_v = AccumulatorT::<Eq>::from_bytes(&fixture.acc_v).expect("acc_V");
    let mut words = lineage.to_vec();
    let (x, y) = generator_coordinates::<Ep>(acc_p.g());
    words.extend([x, y]);
    for challenge in acc_p.challenges() {
        words.extend(foreign_limbs(challenge).map(Fp::from_u128));
    }
    assert_eq!(words.len(), 52);
    assert_eq!(hash_with_domain(LINEAGE_DOMAIN, &words), fixture.d_a);
    // Omega's public columns: [D_A], acc_V.G, acc_V.u.
    let embed = |value: &Fp| Fq::from_repr(value.to_repr()).expect("canonical");
    let (x, y) = generator_coordinates::<Eq>(acc_v.g());
    let expected = vec![
        vec![embed(&fixture.d_a)],
        vec![x, y],
        acc_v.challenges().iter().map(embed).collect(),
    ];
    assert_eq!(fixture.instances, expected);
    // Omega verifies and every transported claim decides.
    let params_q = PinnedParams::<Ep>::derive(K).expect("params");
    let params_a = PinnedParams::<Eq>::derive(K).expect("params");
    let budget = MemoryBudget::DEFAULT;
    let claim = accumulate_generator(
        &params_q,
        &fixture.binding,
        &fixture.vk,
        &fixture.instances,
        &fixture.proof,
        budget,
    )
    .expect("Omega verifies");
    FoldInput::from_opening(*claim.g(), claim.challenges())
        .expect("claim")
        .decide(&params_q, budget)
        .expect("Omega's opening decides");
    acc_v.decide(&params_a, budget).expect("acc_V decides");
    acc_p.decide(&params_q, budget).expect("acc_P decides");
}

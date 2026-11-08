//! The compression wraps end to end on the two-kind toy chain of
//! `support/fold_wrap_toy.rs` (two kinds per side, today's 20-word A state,
//! a second A public column).
//!
//! A chain of four ends with `A_b` while `Q_b` kept no `A_b`, so R starts that
//! accumulator. A chain of eight ends with `A_b` folding into the kept one.
//! For both chains:
//! - the native producer's S, R, P and Omega are satisfied;
//! - a changed crossing row or public value is rejected, including a row of
//!   R's copy of S's public column and of P's copy of R's;
//! - all four proofs are made, Omega verifies natively, and its opening,
//!   `acc_V` and `acc_P` decide.
//!
//! The wraps' sizes without witnesses (the sizing other relations use) equal
//! the rows the checked wraps use. A third test proves the toy chain with one
//! wrap proving key at a time, as a phone does, and prints every stage's
//! time. The Omega fixture other tests use carries
//! the chosen lineage in `D_A` and verifies. The tests print each wrap's rows
//! and each stage's time.

#[path = "support/fold_wrap_toy.rs"]
mod fold_wrap_toy;

use std::time::Instant;

use ff::{Field, PrimeField};
use fold_wrap_toy::{A_A, A_B, A_CONTEXT, A_PC, K};
use iroha_kagemusha_proof::a_relation::{LINEAGE_DOMAIN, LINEAGE_FIELDS};
use iroha_pasta::{Ep, Eq, Fp, Fq, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    pcs::ipa::PinnedParams, transcript::generator_coordinates, verifier::accumulate_generator,
};
use iroha_plonk_gadgets::statement::foreign_limbs;
use iroha_plonk_recursion::{AccumulatorT, FoldInput};

/// The lineage the chains' `A_b` write.
fn lineage(seed: u64) -> [Fp; LINEAGE_FIELDS] {
    core::array::from_fn(|i| Fp::from(seed + 7 * i as u64))
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
    let rows = fold_wrap_toy::wrap(keys, &short, 1, true);
    // Eight half-steps: A_8 folds into the A_b accumulator Q_7 kept.
    let long = fold_wrap_toy::chain(&toy.relations, 8, &lineage(5));
    assert!(long.u_a.iter().all(Option::is_some));
    assert_eq!(
        fold_wrap_toy::wrap(keys, &long, 11, false),
        rows,
        "the wraps' rows"
    );
    // Sizing without witnesses counts exactly the rows the wraps use.
    let now = Instant::now();
    let sizes = fold_wrap_toy::sizes(&keys.plans.p_plan.spec);
    println!("sizes without witnesses: {:.1?}", now.elapsed());
    assert_eq!(sizes.len(), 4, "every wrap is sized");
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

#[test]
#[ignore = "toy chain and its wraps, one proving key at a time; run optimized"]
fn the_toy_chain_proves_with_one_wrap_key_at_a_time() {
    let _serial = fold_wrap_toy::serial();
    let started = Instant::now();
    let relations = fold_wrap_toy::relations();
    let spec = fold_wrap_toy::spec(&relations);
    let chain = fold_wrap_toy::chain(&relations, 8, &lineage(9));
    drop(relations);
    println!("toy relations and eight half-steps: {:.1?}", started.elapsed());
    fold_wrap_toy::prove_one_key_at_a_time(spec, chain, 13);
    println!("total: {:.1?}", started.elapsed());
}

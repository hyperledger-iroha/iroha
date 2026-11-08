//! The compression wraps on the folded Receive's real relations
//! (`support/fold_receive_chain.rs`: A1, A3, A4, A5; Frame, Sigma,
//! Signature).
//!
//! S, R, P and Omega are sized without witnesses (`fold_wrap_toy::sizes`):
//! each is synthesized at k16, or counted at k17 when it overflows k16 (a
//! wrap is sized only once the wrap it verifies fits). The test prints every
//! wrap's rows per lane against the k16 limit and its public rows, and
//! requires all four to fit.
//!
//! The second test wraps the genuine Receive after its eight half-steps:
//! S, R, P and Omega are checked strictly and proved, Omega verifies
//! natively, and its opening, `acc_V` and `acc_P` decide.
//!
//! The third test proves the same Receive as a phone does, with bounded
//! memory: no strict checks, and each wrap's proving key (about 1.5 GiB) made
//! just before its proof and dropped after it. It prints every stage's time;
//! run it on the phone to time the Receive there.
//!
//! Measurements:
//! - three wraps on iroha 5bde12ef2c's relations: R fit (sponge 35%, glue
//!   16%, ECC 69%, range 98%), P did not (sponge 115%, glue 70%, ECC 100%,
//!   range 41%);
//! - four wraps on the relations after the key cuts (iroha 5927848dae), with
//!   S on two range buses and R on four, all fit (sponge/glue/ECC/range):
//!   S 28/15/48/50%, R 54/57/87/67%, P 89/66/50/32%, Omega 78/74/58/34%.

#[path = "support/fold_receive_chain.rs"]
mod fold_receive_chain;
#[path = "support/fold_wrap_toy.rs"]
mod fold_wrap_toy;

use std::{sync::Arc, time::Instant};

use fold_receive_chain::{
    Chain, HalfStep, ReceiveRelations, full_receive, receive_keys, receive_relations, run_chain,
};
use iroha_kagemusha_proof::{
    fold_lineage::{
        A_CONTEXT, A_PC, AKind, BASE, CLAIM_A, CLAIM_Q, FOLD_A, FOLD_Q, IVC_A, IVC_Q, Q_STATE,
    },
    fold_wrap::{
        Domains, WrapSpec,
        native::{self, FinalChain},
    },
};
use iroha_pasta::{Fp, Fq};

/// Usable rows of a k16 circuit (2^16 less the blinding rows).
const K16_ROWS: usize = 65_529;

/// The wraps' spec for the folded Receive: `A_n` is an A5, `D_A` hashes the
/// lineage fields, and the context and program counter are zero at the
/// operation boundary.
fn spec(rel: &ReceiveRelations) -> Arc<WrapSpec> {
    Arc::new(WrapSpec {
        q: rel.q.clone(),
        a: rel.a.clone(),
        last: AKind::Receive5 as usize,
        key_q: rel.key_q.clone(),
        key_a: rel.key_a.clone(),
        params_q: rel.params_q.clone(),
        params_a: rel.params_a.clone(),
        domains: Domains {
            q_fold: FOLD_Q,
            a_fold: FOLD_A,
            a_ivc: IVC_A,
            q_ivc: IVC_Q,
            pallas_claims: CLAIM_A,
            vesta_claims: CLAIM_Q,
        },
        base_a: Fp::from(BASE),
        base_q: Fq::from(BASE),
        a_state: rel.base.len(),
        q_state: Q_STATE,
        lineage: core::array::from_fn(|i| i),
        boundary: vec![A_CONTEXT, A_PC],
    })
}

/// The finished chain the wraps take: the Receive after its eight
/// half-steps, which A5 (step 8) closes after Q's Frame (step 7). Q keeps no
/// state.
fn final_chain(chain: Chain) -> FinalChain {
    assert!(
        matches!(chain.last, Some(HalfStep::A(AKind::Receive5))),
        "the Receive ends with A5"
    );
    FinalChain {
        step: 8,
        a_state: chain.a_state,
        q_state: Vec::new(),
        u_q: chain
            .a_kept
            .into_iter()
            .map(|kept| kept.expect("A keeps every Q kind"))
            .collect(),
        u_a: chain.q_kept,
        a_n: chain.last_a.expect("A5's instance"),
        pallas_claims: chain.claims_a.expect("A's claim accumulator"),
        vesta_claims: chain.claims_q.expect("Q's claim accumulator"),
    }
}

fn percent(rows: usize) -> String {
    format!("{rows} ({}%)", rows * 100 / K16_ROWS)
}

#[test]
#[ignore = "seven k16 relations and the wraps' syntheses; run optimized"]
fn the_wraps_size_on_the_real_receive_relations() {
    let _serial = fold_wrap_toy::serial();
    let started = Instant::now();
    let keys = receive_keys();
    let full = full_receive(&keys, None);
    let rel = receive_relations(&keys, &full);
    println!("keys, fixture and relations: {:.1?}", started.elapsed());
    for (name, relation) in ["A1", "A3", "A4", "A5"].iter().zip(&rel.a) {
        println!("{name}: {:?}", relation.summary());
    }
    for (name, relation) in ["Frame", "Sigma", "Signature"].iter().zip(&rel.q) {
        println!("Q {name}: {:?}", relation.summary());
    }
    let now = Instant::now();
    let sizes = fold_wrap_toy::sizes(&spec(&rel));
    println!("sizes: {:.1?}", now.elapsed());
    for size in &sizes {
        let usage = size.usage;
        println!(
            "{} ({}): sponge {}, glue {}, ECC {}, range {}; {} public rows",
            size.wrap,
            if size.fits { "fits k16" } else { "over k16" },
            percent(usage.sponge),
            percent(usage.arithmetic),
            percent(usage.curve),
            percent(usage.range),
            size.public_rows,
        );
    }
    assert_eq!(
        sizes.iter().map(|size| size.wrap).collect::<Vec<_>>(),
        ["S", "R", "P", "Omega"],
        "every wrap is sized"
    );
    for size in &sizes {
        assert!(size.fits, "{} fits k16", size.wrap);
    }
}

#[test]
#[ignore = "the real eight-half-step Receive, four k16 keygens, checks and proofs; run optimized"]
fn the_wraps_finish_the_real_receive_into_an_omega_that_decides() {
    let _serial = fold_wrap_toy::serial();
    let started = Instant::now();
    let keys = receive_keys();
    let full = full_receive(&keys, None);
    let rel = receive_relations(&keys, &full);
    let chain = final_chain(run_chain(&keys, &rel, &full, 8));
    assert_eq!(Q_STATE, 0, "Q keeps no state");
    println!(
        "keys, fixture, relations and eight half-steps: {:.1?}",
        started.elapsed()
    );
    let now = Instant::now();
    let wrap_keys = native::keygen(spec(&rel)).expect("keys");
    println!("keygen S, R, P and Omega: {:.1?}", now.elapsed());
    let now = Instant::now();
    fold_wrap_toy::wrap(&wrap_keys, &chain, 7, false);
    println!("wraps checked, proved and decided: {:.1?}", now.elapsed());
    println!("total: {:.1?}", started.elapsed());
}

#[test]
#[ignore = "the real Receive and its wraps, one proving key at a time; run optimized"]
fn the_real_receive_proves_with_one_wrap_key_at_a_time() {
    let _serial = fold_wrap_toy::serial();
    let started = Instant::now();
    let (spec, chain) = {
        let keys = receive_keys();
        let full = full_receive(&keys, None);
        let rel = receive_relations(&keys, &full);
        println!(
            "receive keys, fixture and relations: {:.1?}",
            started.elapsed()
        );
        let now = Instant::now();
        let chain = run_chain(&keys, &rel, &full, 8);
        println!(
            "eight half-steps, each checked strictly, committed and folded: {:.1?}",
            now.elapsed()
        );
        (spec(&rel), final_chain(chain))
    };
    fold_wrap_toy::prove_one_key_at_a_time(spec, chain, 7);
    println!("total: {:.1?}", started.elapsed());
}

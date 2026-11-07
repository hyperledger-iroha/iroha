//! The compression wraps on the folded Receive's real relations
//! (`support/fold_receive_chain.rs`: A1, A3, A4, A5; Frame, Sigma,
//! Signature).
//!
//! R, P and Omega are sized without witnesses (`fold_wrap_toy::sizes`): each
//! is synthesized at k16, or counted at k17 when it overflows k16 (Omega is
//! sized only once P fits). The test prints every wrap's rows per lane
//! against the k16 limit, and its public rows.
//!
//! First measurement (iroha 5bde12ef2c relations): R fits (sponge 35%, glue
//! 16%, ECC 69%, range 98%); P does not (sponge 115%, glue 70%, ECC 100%,
//! range 41%).

#[path = "support/fold_receive_chain.rs"]
mod fold_receive_chain;
#[path = "support/fold_wrap_toy.rs"]
mod fold_wrap_toy;

use std::{sync::Arc, time::Instant};

use fold_receive_chain::{ReceiveRelations, full_receive, receive_keys, receive_relations};
use iroha_kagemusha_proof::{
    fold_lineage::{
        A_CONTEXT, A_PC, AKind, BASE, CLAIM_A, CLAIM_Q, FOLD_A, FOLD_Q, IVC_A, IVC_Q, Q_STATE,
    },
    fold_wrap::{Domains, WrapSpec},
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
    assert!(!sizes.is_empty(), "R is sized");
}

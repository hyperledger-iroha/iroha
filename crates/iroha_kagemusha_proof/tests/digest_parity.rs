//! In-circuit digests against the native Poseidon:
//!
//! - the native reference (`iroha_pasta::poseidon::hash_with_domain`)
//!   reproduces every `kagemusha_v1_poseidon` vector of
//!   `fixtures/native_prover/kats_v1.json` (the vendored
//!   `kagemusha_v1_poseidon::hash` outputs) on both fields;
//! - every digest the relation computes in circuit (predecessor, successor,
//!   chain, Request, statement) equals the native reference, for many
//!   witnesses, every relation shape and both fields, and does not depend
//!   on the prefix mode or the lane count;
//! - the statement digest equals the gadgets' `StatementV1::digest`;
//! - pinned known answers guard the prototype encoding against drift.

mod common;

use common::{in_circuit_digests, relation_shapes, repo_root, smallest_shape};
use ff::PrimeField;
use iroha_kagemusha_proof::{
    Mutation, PrefixMode, RelationShape, SigmaParams, SigmaShape, StateLayout, StepRelation,
    limb_bits_for, sample_witness,
};
use iroha_pasta::{
    Fp, Fq,
    poseidon::{PoseidonField, hash_with_domain},
};
use iroha_plonk_gadgets::statement::StatementV1;
use norito::json::Value;

fn hex_decode(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64, "32-byte hex");
    let bytes = (0..32)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).expect("hex digit"))
        .collect::<Vec<_>>();
    bytes.try_into().expect("32 bytes")
}

fn scalar<F: PrimeField<Repr = [u8; 32]>>(text: &str) -> F {
    Option::from(F::from_repr(hex_decode(text))).expect("canonical scalar")
}

fn hex<F: PrimeField<Repr = [u8; 32]>>(value: &F) -> String {
    use core::fmt::Write as _;
    value.to_repr().iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn shared_vectors<F: PoseidonField>(parity: &str) -> usize {
    let path = repo_root().join("fixtures/native_prover/kats_v1.json");
    let text = std::fs::read_to_string(&path).expect("read kats_v1.json");
    let fixture = norito::json::parse_value(&text).expect("parse kats_v1.json");
    let vectors = fixture
        .get("kagemusha_v1_poseidon")
        .and_then(|section| section.get(parity))
        .and_then(|section| section.get("vectors"))
        .and_then(Value::as_array)
        .expect("kagemusha_v1_poseidon vectors");
    for vector in vectors {
        let text = |key: &str| vector.get(key).and_then(Value::as_str).expect("string");
        let label = text("domain");
        let domain = u64::from_le_bytes(label.as_bytes().try_into().expect("8-byte domain"));
        let inputs = vector
            .get("inputs")
            .and_then(Value::as_array)
            .expect("inputs")
            .iter()
            .map(|input| scalar::<F>(input.as_str().expect("hex")))
            .collect::<Vec<F>>();
        assert_eq!(
            hash_with_domain(domain, &inputs),
            scalar::<F>(text("output")),
            "{parity} {label}"
        );
    }
    vectors.len()
}

#[test]
fn the_native_reference_reproduces_the_shared_vectors() {
    assert_eq!(shared_vectors::<Fp>("fp"), 12);
    assert_eq!(shared_vectors::<Fq>("fq"), 12);
}

fn parity_on<F: PoseidonField>(seeds: core::ops::Range<u64>) -> usize {
    let mut checked = 0;
    for relation in relation_shapes() {
        let shape = smallest_shape(relation);
        for seed in seeds.clone() {
            for mutation in [Mutation::None, Mutation::Overdraft, Mutation::Overflow] {
                let witness = sample_witness::<F>(seed, relation.step, mutation);
                let native = witness.evaluate(relation.layout);
                assert_eq!(
                    in_circuit_digests(&shape, &witness),
                    native.digests,
                    "{} seed {seed} {mutation:?}",
                    relation.label()
                );
                checked += 1;
            }
        }
    }
    checked
}

#[test]
fn in_circuit_digests_equal_the_native_reference_on_both_fields() {
    assert_eq!(parity_on::<Fp>(0..2), 8 * 2 * 3);
    assert_eq!(parity_on::<Fq>(2..3), 8 * 3);
}

#[test]
#[ignore = "more witnesses; run in release"]
fn in_circuit_digests_equal_the_native_reference_on_many_witnesses() {
    assert_eq!(parity_on::<Fp>(0..16), 8 * 16 * 3);
    assert_eq!(parity_on::<Fq>(0..16), 8 * 16 * 3);
}

#[test]
fn digests_do_not_depend_on_the_prefix_mode_or_the_lanes() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let witness = sample_witness::<Fp>(21, step, Mutation::None);
        let mut seen = None;
        for prefix in [PrefixMode::Folded, PrefixMode::Absorbed] {
            for lanes in 1..=3 {
                let relation = RelationShape::new(step, StateLayout::TwoLevel, prefix);
                let params = SigmaParams::new(relation, lanes, limb_bits_for(12)).expect("params");
                let digests = in_circuit_digests(&SigmaShape::new(params, 12), &witness);
                assert_eq!(*seen.get_or_insert(digests), digests, "{prefix:?} {lanes}");
            }
        }
    }
}

#[test]
fn statement_digests_equal_the_gadget_encoding() {
    for relation in relation_shapes() {
        let shape = smallest_shape(relation);
        let witness = sample_witness::<Fp>(5, relation.step, Mutation::None);
        let native = witness.evaluate(relation.layout);
        let state = &witness.predecessor;
        let effect_len = match relation.step {
            StepRelation::Send => 13,
            StepRelation::Receive => 7,
        };
        let statement = StatementV1 {
            relation: relation.step,
            scheme_id: state.remainder.scheme_id,
            credential: state.remainder.credential,
            asset: state.remainder.asset,
            lifecycle: state.core.lifecycle,
            sequence: state.core.sequence + 1,
            next_load: state.core.next_load,
            predecessor: native.digests.predecessor,
            predecessor_other: witness.predecessor_other,
            successor: native.digests.successor,
            successor_other: witness.successor_other,
            effect: native.statement[19..19 + effect_len].to_vec(),
        };
        assert_eq!(
            Some(in_circuit_digests(&shape, &witness).statement),
            statement.digest(),
            "{}",
            relation.label()
        );
    }
}

/// Known answers of seed 0 on `Fp` (prototype encoding; a change here is a
/// change of the measured relation).
#[test]
fn pinned_known_answers() {
    let answers = [
        (StepRelation::Send, StateLayout::TwoLevel),
        (StepRelation::Receive, StateLayout::TwoLevel),
        (StepRelation::Send, StateLayout::Flat),
        (StepRelation::Receive, StateLayout::Flat),
    ]
    .map(|(step, layout)| {
        let digests = sample_witness::<Fp>(0, step, Mutation::None)
            .evaluate(layout)
            .digests;
        let request = digests.request.map(|request| hex(&request));
        println!(
            "KAT step={step:?} layout={layout:?} statement={} request={request:?}",
            hex(&digests.statement)
        );
        (hex(&digests.statement), request)
    });
    let expected: [(&str, Option<&str>); 4] = PINNED;
    for ((statement, request), (pinned_statement, pinned_request)) in answers.iter().zip(expected) {
        assert_eq!(statement, pinned_statement);
        assert_eq!(request.as_deref(), pinned_request);
    }
}

/// The pinned statement and Request digests of [`pinned_known_answers`].
const PINNED: [(&str, Option<&str>); 4] = [
    (
        "db5d9fb21d1a76aab8daf04ed0b780953273b4ce7d83f54e894b0628c6d6123a",
        Some("46653d643185762e9a9e2dca37239d6f86358eff5f5652a3a7a4b08d5d527518"),
    ),
    (
        "8e170324970a09d693526c55524d581be5225348789f5c3d3dfabdd28f7a911a",
        None,
    ),
    (
        "61d2d14e2b416dbfb6ba5d59164aac5afa531890c26879c23947629539d4aa17",
        Some("46653d643185762e9a9e2dca37239d6f86358eff5f5652a3a7a4b08d5d527518"),
    ),
    (
        "eef7e2e3ed2fbb9dab707b3458baa0a5fa550498b8d85cea8897cb4f8404b316",
        None,
    ),
];

//! The KAGEMUSHA sponge chip against shared vectors:
//!
//! - every `kagemusha_v1_poseidon` vector of
//!   `fixtures/native_prover/kats_v1.json` (exported from the vendored
//!   `kagemusha_v1_poseidon::hash` backend), on both fields, with and without
//!   the folded prefix;
//! - the raw-sponge known answers of `iroha_pasta/tests/poseidon_kat.rs`;
//! - native `iroha_pasta::poseidon` hashes of edge inputs and a short
//!   replay-tree chain;
//!
//! plus the per-cell tamper suite and the permutation inventory of the M7
//! hash shapes.

mod common;

use common::{
    Chips, GadgetCircuit, Inputs, Shape, accepts, assigned, extent, lane_columns, repo_root, report,
};
use ff::PrimeField;
use iroha_pasta::{
    Fp, Fq,
    poseidon::{PoseidonField, hash, hash_with_domain},
};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    AbsorbInput, Word,
    poseidon::{ROWS_PER_PERMUTATION, domain_permutations, raw_permutations},
    tamper::undetected_tampers,
};
use norito::json::Value;

/// `hash_with_domain(arg 0, inputs)` on lane 0.
fn domain_hash<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let words = chips.glue.witnesses(region, &inputs.all())?;
    Ok(vec![chips.sponges[0].hash_words(
        region,
        inputs.arg(0),
        &words,
    )?])
}

/// `hash(inputs)` on lane 0.
fn raw_hash<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let words = chips.glue.witnesses(region, &inputs.all())?;
    let absorbed = words.iter().map(AbsorbInput::Word).collect::<Vec<_>>();
    Ok(vec![chips.sponges[0].hash_raw(region, &absorbed)?])
}

/// The smallest `k` whose usable rows hold `permutations` blocks and the
/// glue rows of `inputs` witnesses.
fn k_for(permutations: usize) -> u32 {
    let rows = permutations * ROWS_PER_PERMUTATION;
    (7..=12)
        .find(|k| rows + 6 <= 1 << k)
        .unwrap_or_else(|| panic!("{permutations} permutations need k > 12"))
}

fn domain_circuit<F: PoseidonField>(domain: u64, inputs: Vec<F>, folded: bool) -> GadgetCircuit<F> {
    let mut shape = Shape::new(1, 4, 1).with_args(&[domain]);
    if folded {
        shape = shape.folding(&[(domain, inputs.len())]);
    }
    GadgetCircuit::new(shape, domain_hash::<F>, inputs)
}

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

fn fixture() -> Value {
    let path = repo_root().join("fixtures/native_prover/kats_v1.json");
    let text = std::fs::read_to_string(&path).expect("read kats_v1.json");
    norito::json::parse_value(&text).expect("parse kats_v1.json")
}

fn kagemusha_vectors<F: PoseidonField>(parity: &str) {
    let fixture = fixture();
    let vectors = fixture
        .get("kagemusha_v1_poseidon")
        .and_then(|section| section.get(parity))
        .and_then(|section| section.get("vectors"))
        .and_then(Value::as_array)
        .expect("kagemusha_v1_poseidon vectors");
    assert_eq!(vectors.len(), 12);
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
        let output = scalar::<F>(text("output"));
        assert_eq!(hash_with_domain(domain, &inputs), output, "native {label}");
        for folded in [false, true] {
            let permutations = domain_permutations(inputs.len(), folded);
            let k = k_for(permutations);
            let circuit = domain_circuit(domain, inputs.clone(), folded);
            assert!(
                accepts(&circuit, k, &[output]),
                "{parity} {label} arity {} folded {folded}: {}",
                inputs.len(),
                report(&circuit, k, &[output])
            );
            assert!(!accepts(&circuit, k, &[output + F::ONE]));
            let lane = assigned(&circuit, k, &[output]);
            assert_eq!(
                extent(&lane[lane_columns(0)[0]]),
                permutations * ROWS_PER_PERMUTATION,
                "{parity} {label} lane rows"
            );
        }
    }
}

#[test]
fn shared_kagemusha_vectors_fp() {
    kagemusha_vectors::<Fp>("fp");
}

#[test]
fn shared_kagemusha_vectors_fq() {
    kagemusha_vectors::<Fq>("fq");
}

/// The first raw-sponge known answers of `iroha_pasta/tests/poseidon_kat.rs`
/// (preimages `i * 0x9e3779b9 + 1`).
const FP_SEQ: [&str; 7] = [
    "23f1b32003877d36d483529dffb86a64860e264cff6d1ac5a5d41f56a024c737",
    "712798cdffb3d3f4f533af5e7fb89d76515be8066f3acc8f678fe80947099807",
    "d00001a978ab605be788a836d6dc7ca2df1deebc8b6954090238c1460538832f",
    "a75b6782d2fc8eeda44a50c3954ce62f609be22977a762a3d1b0087a1eb07f2e",
    "104f5caebfcb4a90d8d07c0ffaea50ba63a87613552ca5c92787e9b8d135d23d",
    "0f7baf85a0129ad4ef1218b110217a88a0d2667afb40648ee4ef043bdafbba23",
    "249779afa6b638c9ff86e8fef94e88c5f28e36cfae3ffc94948c9ed10709e83c",
];

/// The Fq counterparts of [`FP_SEQ`].
const FQ_SEQ: [&str; 7] = [
    "e9040fd5b92dd549b30d5f1c370eb3e6156bcdf59540809c81b75596ee74ed04",
    "0be5dbd1d8df9f839bb1434b74ae9fe4e795785803d9b9d118f3875ca6a4d718",
    "0e855fd3efcfb3a2d9d943cbf8d5b8301559b1936f20a458fec4e87ae493f104",
    "9e521c1c018a89591e1174c3e2d0d42f0a65aa4a923f26e1995de1bb4d71893b",
    "32b6c80830a8384b052cba0a115581041915890872d137e7753835804df2363a",
    "051399b49536ea54bc5f78e1ecd200c52fc923d10042659604da4570c85e043e",
    "5b34b93a58b8e67c9a2ef1c0b24c90262f672ba0dc9ef506169ba73d855b7812",
];

fn raw_sequence_vectors<F: PoseidonField>(expected: &[&str; 7]) {
    for (len, expected) in expected.iter().enumerate() {
        let preimage = (0..len as u64)
            .map(|i| F::from(i * 0x9e37_79b9 + 1))
            .collect::<Vec<F>>();
        let output = scalar::<F>(expected);
        assert_eq!(hash(&preimage), output, "native length {len}");
        let k = k_for(raw_permutations(len));
        let circuit = GadgetCircuit::new(Shape::new(1, 4, 1), raw_hash::<F>, preimage);
        assert!(
            accepts(&circuit, k, &[output]),
            "length {len}: {}",
            report(&circuit, k, &[output])
        );
    }
}

#[test]
fn raw_sponge_matches_the_iroha_pasta_known_answers() {
    raw_sequence_vectors::<Fp>(&FP_SEQ);
    raw_sequence_vectors::<Fq>(&FQ_SEQ);
}

fn edge_inputs<F: PoseidonField>() {
    let specials = [F::ZERO, F::ONE, -F::ONE, F::from_u128(u128::MAX)];
    for len in 0..=4 {
        for special in specials {
            let preimage = vec![special; len];
            let output = hash_with_domain(42, &preimage);
            for folded in [false, true] {
                let circuit = domain_circuit(42, preimage.clone(), folded);
                let k = k_for(domain_permutations(len, folded));
                assert!(
                    accepts(&circuit, k, &[output]),
                    "{}",
                    report(&circuit, k, &[output])
                );
            }
        }
    }
}

#[test]
fn edge_inputs_match_the_native_sponge() {
    edge_inputs::<Fp>();
    edge_inputs::<Fq>();
}

/// `r = hash(kgmemp_1, [])`, then `arg 0` levels of `r = hash(kgmnode1, [r,
/// r])`: the empty replay-tree root chain.
fn replay_chain<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let empty = u64::from_le_bytes(*b"kgmemp_1");
    let node = u64::from_le_bytes(*b"kgmnode1");
    let mut root = chips.sponges[0].hash(region, empty, &[])?;
    for _ in 0..inputs.arg(0) {
        root = chips.sponges[0].hash_words(region, node, &[root.clone(), root.clone()])?;
    }
    Ok(vec![root])
}

#[test]
fn replay_tree_chain_matches_the_native_hash() {
    let empty = u64::from_le_bytes(*b"kgmemp_1");
    let node = u64::from_le_bytes(*b"kgmnode1");
    let levels = 2_u64;
    let mut root = hash_with_domain::<Fq>(empty, &[]);
    for _ in 0..levels {
        root = hash_with_domain(node, &[root, root]);
    }
    // Folded prefixes: one permutation per level, two for the empty leaf
    // (arity 0 also folds).
    let shape = Shape::new(1, 4, 1)
        .with_args(&[levels])
        .folding(&[(empty, 0), (node, 2)]);
    let circuit = GadgetCircuit::new(shape, replay_chain::<Fq>, Vec::new());
    let k = 8;
    assert!(
        accepts(&circuit, k, &[root]),
        "{}",
        report(&circuit, k, &[root])
    );
    let lane = assigned(&circuit, k, &[root]);
    let blocks = domain_permutations(0, true) + 2 * domain_permutations(2, true);
    assert_eq!(blocks, 1 + 2 * 2);
    assert_eq!(
        extent(&lane[lane_columns(0)[0]]),
        blocks * ROWS_PER_PERMUTATION
    );
    assert_eq!(
        undetected_tampers(&circuit, k, &[vec![root]]),
        Ok(Vec::new())
    );
}

/// A folded 3-input hash and a raw 2-input hash on one lane.
fn two_hashes<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let words = chips.glue.witnesses(region, &inputs.all())?;
    let first = chips.sponges[0].hash_words(region, inputs.arg(0), &words[..3])?;
    let raw = [AbsorbInput::Word(&words[3]), AbsorbInput::Word(&first)];
    let second = chips.sponges[0].hash_raw(region, &raw)?;
    Ok(vec![first, second])
}

fn sponge_cells_are_pinned<F: PoseidonField>() {
    let domain = u64::from_le_bytes(*b"kgmleaf1");
    let inputs = vec![F::from(3u64), -F::ONE, F::from(5u64), F::from(8u64)];
    let first = hash_with_domain(domain, &inputs[..3]);
    let second = hash(&[inputs[3], first]);
    let shape = Shape::new(1, 4, 2)
        .with_args(&[domain])
        .folding(&[(domain, 3)]);
    let circuit = GadgetCircuit::new(shape, two_hashes::<F>, inputs);
    let public = vec![first, second];
    let k = 8;
    assert!(
        accepts(&circuit, k, &public),
        "{}",
        report(&circuit, k, &public)
    );
    let lane = assigned(&circuit, k, &public);
    let blocks = domain_permutations(3, true) + raw_permutations(2);
    assert_eq!(
        extent(&lane[lane_columns(0)[0]]),
        blocks * ROWS_PER_PERMUTATION
    );
    assert_eq!(undetected_tampers(&circuit, k, &[public]), Ok(Vec::new()));
}

#[test]
fn every_sponge_cell_is_pinned() {
    sponge_cells_are_pinned::<Fp>();
    sponge_cells_are_pinned::<Fq>();
}

#[test]
fn m7_hash_shapes_inventory() {
    // (fields, M7 permutations): statement 32, Request 24, send chain 10,
    // recv chain 6, two-level core 11 (10 fields + rest digest).
    let shapes = [(32, 18), (24, 14), (10, 7), (6, 5), (11, 7)];
    for (fields, m7) in shapes {
        assert_eq!(domain_permutations(fields, false), m7);
        assert_eq!(domain_permutations(fields, true), m7 - 1);
    }
    // Two-level sigma_send (M7: 53 permutations in 5 sponges) costs 48
    // blocks with every prefix folded, 1,776 rows, 7,104 lane cells.
    let send: usize = [11, 11, 10, 24, 32]
        .iter()
        .map(|fields| domain_permutations(*fields, true))
        .sum();
    assert_eq!(send, 48);
    assert_eq!(send * ROWS_PER_PERMUTATION, 1_776);
    assert_eq!(
        send * iroha_plonk_gadgets::poseidon::CELLS_PER_PERMUTATION,
        7_104
    );
    // The statement digest in circuit uses 17 blocks when folded.
    let statement = (0..32_u64).map(Fp::from).collect::<Vec<_>>();
    let domain = u64::from_le_bytes(*b"m7stmnt1");
    let output = hash_with_domain(domain, &statement);
    let circuit = domain_circuit(domain, statement, true);
    let k = 10;
    assert!(
        accepts(&circuit, k, &[output]),
        "{}",
        report(&circuit, k, &[output])
    );
    let lane = assigned(&circuit, k, &[output]);
    assert_eq!(extent(&lane[lane_columns(0)[3]]), 17 * ROWS_PER_PERMUTATION);
}

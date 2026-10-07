//! Capacity and original-key layout qualification only.
//!
//! The child keys below belong to real initial aggregation/BLS leaves, not
//! complete source programs. No child proofs are created and no certificate
//! acceptance is claimed. They exercise the full descriptor-sized hard verifier,
//! four-claim fold and certificate linkage to measure the actual fixed layout.
//! The same imported sources then qualify the actual certificate layout and a
//! real result-scan Start layout for the next composition's capacity check.
//! They are reused again with original parser/hash/schedule keys to qualify the
//! current-schedule composition, without claiming complete child availability.

use super::*;
use crate::finality::{
    aggregate::{AggregateLeafCircuit, prepare_aggregation},
    bls::{BlsBatchCircuit, prepare_bls_batches},
    certified_result::CertifiedResultCircuit,
    continuity::test_support::qualified,
    history::{
        GenesisSourceCircuit, HistoryAnchor, HistoryAppendCircuit, HistoryAppendPlan,
        HistoryStepCircuit,
    },
    load_source::{LoadSourceCircuit, LoadSourcePlan},
    receipt_finality::ReceiptFinalityCircuit,
    result_scan::{ResultScanBatchCircuit, ResultScanBatchPlan},
    roster::key_tree_native,
    schedule::{
        complete::ScheduleCircuit, context_hash::prepare_context_batches,
        source::prepare_schedule_source,
    },
    scheduled_result::ScheduledResultCircuit,
};
use ark_bls12_381::{G1Affine, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::Zero;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn native_leaves() -> (CertificateContext, AggregateLeafCircuit, BlsBatchCircuit) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    let json: norito::json::Value =
        norito::json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let bytes = |name: &str| hex(json.get(name).unwrap().as_str().unwrap());
    let keys = json
        .get("committee_public_keys_hex")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|key| hex(key.as_str().unwrap()).try_into().unwrap())
        .collect::<Vec<[u8; 48]>>();
    let bits = bytes("qc_bitmap_hex");
    let mut bitmap = [0; 4];
    bitmap[..bits.len()].copy_from_slice(&bits);
    let mut sum = G1Projective::zero();
    for (i, key) in keys.iter().enumerate() {
        if bitmap[i / 8] & (1 << (i % 8)) != 0 {
            sum += G1Affine::deserialize_compressed(key.as_slice()).unwrap();
        }
    }
    let mut aggregate = Vec::new();
    sum.into_affine()
        .serialize_compressed(&mut aggregate)
        .unwrap();
    let aggregate: [u8; 48] = aggregate.try_into().unwrap();
    let (root, _) = key_tree_native(&keys).unwrap();
    let context = CertificateContext {
        aggregation: AggregateContext {
            roster_root: root,
            members: u8::try_from(keys.len()).unwrap(),
            faults: (u8::try_from(keys.len()).unwrap() - 1) / 3,
            bitmap,
            aggregate_key: aggregate,
        },
        message: bytes("commit_vote_preimage_hex").try_into().unwrap(),
        signature: bytes("qc_aggregate_signature_hex").try_into().unwrap(),
    };
    let aggregate = prepare_aggregation(&keys, &bits, aggregate)
        .unwrap()
        .remove(0);
    let bls = prepare_bls_batches(
        context.message,
        context.aggregation.aggregate_key,
        context.signature,
    )
    .unwrap()
    .remove(0);
    (context, aggregate, bls)
}

#[test]
#[ignore = "original k16 leaf/wrapper key imports and full certificate layout; no complete child proofs"]
fn certificate_hard_verifier_layout_fits_k16_with_original_source_imports() {
    let started = std::time::Instant::now();
    let pallas = PinnedParams::<Ep>::derive(16).unwrap();
    let vesta = PinnedParams::<Eq>::derive(16).unwrap();
    let (context, aggregate, bls) = native_leaves();
    for (source, expected_program, width) in [
        (aggregate.endpoints(), aggregate::PROGRAM_ID, 1_u64),
        (bls.endpoints(), BlsLeafPlan::PROGRAM_ID, 2_u64),
    ] {
        assert_eq!(source[0], Fp::from(expected_program));
        assert_eq!(source[2], Fp::ZERO);
        assert_eq!(
            source[3],
            Fp::from(width),
            "these are only initial sources, not complete certificate evidence"
        );
    }
    assert!(
        check_circuit(
            &aggregate,
            16,
            &aggregate.instances().unwrap(),
            CheckMode::Strict
        )
        .unwrap()
        .is_satisfied()
    );
    assert!(
        check_circuit(&bls, 16, &bls.instances().unwrap(), CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let sources = [
        qualified(&aggregate, &pallas, &vesta),
        qualified(&bls, &pallas, &vesta),
    ];
    let plan = SourcePairPlan::new(sources, &pallas).unwrap();
    let blank = CertificateCircuit::for_source(plan).unwrap();
    let unknown = synthesize(&blank, 16, None)
        .expect("certificate linkage and both hard verifiers must fit k16");
    let rows = unknown
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    assert!(rows < 1 << 16);
    eprintln!(
        "certificate original-source layout: {rows} advice rows at k16, {:?}; layout only",
        started.elapsed()
    );

    // Set known byte/state witnesses, keeping deliberately invalid zero proof
    // tapes. Their hard verification must fail, while all fixed source tables
    // stay identical. This is not a synthetic acceptance or monetary test.
    let mut invalid = blank.clone();
    invalid.pair.known = true;
    invalid.context = context;
    for (child, endpoints) in invalid
        .pair
        .children
        .iter_mut()
        .zip(context.source_endpoints())
    {
        child.endpoints = endpoints;
    }
    let digest = context.digest();
    invalid.endpoints = [
        Fp::from(PROGRAM_ID),
        digest,
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        digest,
    ];
    invalid.public = invalid
        .pair
        .frame(invalid.endpoints, &vesta, MemoryBudget::DEFAULT)
        .unwrap();
    let known = synthesize(&invalid, 16, Some(&[invalid.public.to_vec()]))
        .expect("known invalid witness uses the same bounded layout");
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let report = iroha_plonk::check::check(&known.cs, &known.tables, CheckMode::Strict).unwrap();
    assert!(
        !report.is_satisfied(),
        "placeholder proofs never establish a quorum certificate"
    );
    drop((known, unknown, report));

    // Reuse the same qualified child plan when importing the actual certificate
    // circuit's original keys. Availability of a satisfying complete program is
    // deliberately not claimed: its children here are initial leaf keys only.
    let certificate_source = qualified(&blank, &pallas, &vesta);
    let scan = ResultScanBatchCircuit::for_source(ResultScanBatchPlan::StartAbsorb).unwrap();
    let scan_source = qualified(&scan, &pallas, &vesta);
    let plan = SourcePairPlan::new([certificate_source, scan_source], &pallas).unwrap();
    let certified = CertifiedResultCircuit::for_source(plan).unwrap();
    let compiled = synthesize(&certified, 16, None)
        .expect("certificate and complete-result linkage with both hard verifiers must fit k16");
    let rows = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    assert!(rows < 1 << 16);
    eprintln!(
        "certified-result original-source layout: {rows} advice rows at k16, {:?}; layout only",
        started.elapsed()
    );
    drop(compiled);

    // Keep the already imported original key tree and qualify its actual
    // composed circuit once. The schedule side likewise uses real initial
    // parser/hash source keys; no fabricated SourceVerifier or live proof is
    // accepted as a substitute for original source qualification.
    let certified_source = qualified(&certified, &pallas, &vesta);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    let fixture: norito::json::Value =
        norito::json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let frame = hex(fixture
        .get("result_preimage_hex")
        .unwrap()
        .as_str()
        .unwrap());
    let id = core::array::from_fn(|i| context.message[53 + i]);
    let parser = prepare_schedule_source(frame.clone(), false, id)
        .unwrap()
        .remove(0);
    let input = parser.input();
    let hash = prepare_context_batches(
        frame,
        input.epoch_hash.payload_start,
        input.epoch_hash.payload_len,
        id,
    )
    .unwrap()
    .remove(0);
    let plan = SourcePairPlan::new(
        [
            qualified(&parser, &pallas, &vesta),
            qualified(&hash, &pallas, &vesta),
        ],
        &pallas,
    )
    .unwrap();
    let schedule = ScheduleCircuit::for_source(plan).unwrap();
    let schedule_source = qualified(&schedule, &pallas, &vesta);
    let plan = SourcePairPlan::new([certified_source, schedule_source.clone()], &pallas).unwrap();
    let scheduled = ScheduledResultCircuit::for_source(plan).unwrap();
    let compiled = synthesize(&scheduled, 16, None)
        .expect("certified result and current schedule with both hard verifiers must fit k16");
    let rows = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    assert!(rows < 1 << 16);
    eprintln!(
        "scheduled-result original-source layout: {rows} advice rows at k16, {:?}; layout only",
        started.elapsed()
    );
    drop(compiled);

    // Qualify the actual fixed-anchor history step using those same imported
    // ScheduledResult and Schedule layouts. This checks capacity, not full
    // child program availability or authentication of a caller-selected policy.
    let certified_schedule_source = qualified(&scheduled, &pallas, &vesta);
    let plan = SourcePairPlan::new([certified_schedule_source, schedule_source], &pallas).unwrap();
    let anchor = HistoryAnchor {
        network: input.projection.network,
        instance: core::array::from_fn(|i| context.message[13 + i]),
        initial_context: input.epoch_hash.context_id,
        initial_epoch: input.projection.epoch,
        parameters: input.projection.slots[0].parameters,
    };
    let history = HistoryStepCircuit::for_source(anchor, plan).unwrap();
    let compiled = synthesize(&history, 16, None)
        .expect("exact genesis-bound history step with both hard verifiers must fit k16");
    let rows = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    let cells: usize = compiled
        .tables
        .advice_assigned()
        .iter()
        .map(|column| column.iter().filter(|assigned| **assigned).count())
        .sum();
    assert!(rows < 1 << 16);
    eprintln!(
        "history-step original-source layout: {rows} advice rows, {cells} assigned cells at k16, {:?}; layout only",
        started.elapsed()
    );
    drop(compiled);

    // The terminal key uses an actual genesis/state-history join and an
    // original receipt-inclusion source. As above, this is source-key/layout
    // qualification only: an initial Load leaf cannot satisfy all35 stages.
    let genesis_source = qualified(&GenesisSourceCircuit::for_source(anchor), &pallas, &vesta);
    let history_source = qualified(&history, &pallas, &vesta);
    let plan = HistoryAppendPlan::new(
        anchor,
        genesis_source.binding().clone(),
        history_source,
        pallas.clone(),
    )
    .unwrap();
    let history = HistoryAppendCircuit::for_source(plan).unwrap();
    let history_source = qualified(&history, &pallas, &vesta);
    let load = LoadSourceCircuit::for_source(LoadSourcePlan::Start).unwrap();
    let load_source = qualified(&load, &pallas, &vesta);
    let plan = SourcePairPlan::new([history_source, load_source], &pallas).unwrap();
    let terminal = ReceiptFinalityCircuit::for_source(anchor, plan).unwrap();
    let compiled = synthesize(&terminal, 16, None)
        .expect("genesis-rooted history and complete receipt inclusion must fit k16");
    let rows = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    let cells: usize = compiled
        .tables
        .advice_assigned()
        .iter()
        .map(|column| column.iter().filter(|assigned| **assigned).count())
        .sum();
    assert!(rows < 1 << 16);
    eprintln!(
        "receipt-finality original-source layout: {rows} advice rows, {cells} assigned cells at k16, {:?}; layout only",
        started.elapsed()
    );
}

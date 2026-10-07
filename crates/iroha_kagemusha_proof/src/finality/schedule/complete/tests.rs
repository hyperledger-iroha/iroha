//! Exact native complete-source geometry; hard-verifier capacity is qualified separately.

use super::*;

fn fixture() -> (Vec<u8>, [u8; 32]) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    let j: norito::json::Value =
        norito::json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let hex = |s: &str| {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>()
    };
    let frame = hex(j.get("result_preimage_hex").unwrap().as_str().unwrap());
    let context = j
        .get("authenticated_schedule")
        .unwrap()
        .get("current")
        .unwrap();
    let id = hex(context.get("context_id_hex").unwrap().as_str().unwrap())
        .try_into()
        .unwrap();
    (frame, id)
}
fn fixture_input() -> ScheduleSourceInput {
    let (frame, id) = fixture();
    *source::prepare_schedule_source(frame, false, id).unwrap()[0].input()
}
#[test]
fn schedule_composition_requires_both_exact_complete_source_intervals() {
    let input = fixture_input();
    let [parser, epoch] = input.source_endpoints().unwrap();
    assert_eq!(parser[0], Fp::from(source::PROGRAM_ID));
    assert_eq!(parser[1], input.digest());
    assert_eq!(parser[2], Fp::ZERO);
    assert_eq!(parser[3], Fp::from(43));
    assert_eq!(parser[4], source::boundary_digest_native(&input, false));
    assert_eq!(parser[5], source::boundary_digest_native(&input, true));
    assert_eq!(epoch[0], Fp::from(context_hash::PROGRAM_ID));
    assert_eq!(epoch[1], input.epoch_hash.digest());
    assert_eq!(epoch[2], Fp::ZERO);
    assert_eq!(epoch[3], Fp::from(u64::from(context_hash::PROGRAM_LENGTH)));
    assert_ne!(epoch[3], Fp::from(u64::from(input.epoch_hash.crc_steps())));
    assert_eq!(
        epoch[4],
        context_hash::boundary_digest_native(&input.epoch_hash, false)
    );
    assert_eq!(
        epoch[5],
        context_hash::boundary_digest_native(&input.epoch_hash, true)
    );
    let mut substituted = input;
    substituted.epoch_hash.context_id[0] ^= 1;
    let changed = substituted.source_endpoints().unwrap();
    assert_ne!(parser[1], changed[0][1]);
    assert_ne!(epoch[1], changed[1][1]);
    let mut empty = input;
    empty.epoch_hash.payload_len = 0;
    assert!(empty.source_endpoints().is_err());
    let mut overrun = input;
    overrun.epoch_hash.payload_start = overrun.epoch_hash.result_len;
    assert!(overrun.source_endpoints().is_err());
}

#[test]
#[ignore = "original k16 parser/hash source key imports and both hard verifiers; no complete child proofs"]
fn schedule_hard_verifier_layout_fits_k16_with_original_source_imports() {
    use crate::finality::continuity::test_support::qualified;
    use iroha_pasta::msm::MemoryBudget;
    use iroha_plonk::{
        check::{CheckMode, check_circuit},
        frontend::synthesize,
    };

    let started = std::time::Instant::now();
    let pallas = PinnedParams::<Ep>::derive(16).unwrap();
    let vesta = PinnedParams::<Eq>::derive(16).unwrap();
    let (frame, id) = fixture();
    let parser = source::prepare_schedule_source(frame.clone(), false, id)
        .unwrap()
        .remove(0);
    let input = *parser.input();
    let hash = context_hash::prepare_context_batches(
        frame,
        input.epoch_hash.payload_start,
        input.epoch_hash.payload_len,
        id,
    )
    .unwrap()
    .remove(0);
    for (endpoints, width) in [(parser.endpoints(), 1_u64), (hash.endpoints(), 2_u64)] {
        assert_eq!(endpoints[2], Fp::ZERO);
        assert_eq!(
            endpoints[3],
            Fp::from(width),
            "only original initial source keys, not complete program intervals"
        );
    }
    assert!(
        check_circuit(&parser, 16, &parser.instances().unwrap(), CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    assert!(
        check_circuit(&hash, 16, &hash.instances().unwrap(), CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    // The installed keys are original sealed-source leaf keys. They qualify the
    // actual descriptor-sized verifiers; they cannot prove these required full
    // intervals. Full parser/hash source trees are separately mandatory.
    let plan = SourcePairPlan::new(
        [
            qualified(&parser, &pallas, &vesta),
            qualified(&hash, &pallas, &vesta),
        ],
        &pallas,
    )
    .unwrap();
    let blank = ScheduleCircuit::for_source(plan).unwrap();
    let unknown = synthesize(&blank, 16, None)
        .expect("complete schedule linkage plus two hard verifiers must fit k16");
    let rows = unknown
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    let cells: usize = unknown
        .tables
        .advice_assigned()
        .iter()
        .map(|column| column.iter().filter(|assigned| **assigned).count())
        .sum();
    assert!(rows < 1 << 16);
    eprintln!(
        "schedule original-source layout: {rows} advice rows, {cells} assigned cells at k16, {:?}; layout only",
        started.elapsed()
    );

    let mut invalid = blank.clone();
    invalid.pair.known = true;
    invalid.input = input;
    let expected = input.source_endpoints().unwrap();
    for (child, endpoints) in invalid.pair.children.iter_mut().zip(expected) {
        child.endpoints = endpoints;
    }
    // Neither reordered/truncated source statements nor any changed boundary
    // reaches proof preparation. Even exact endpoint claims still require real
    // child proofs and the original qualified verifier keys.
    for child in 0..2 {
        for word in 0..6 {
            let mut children = invalid.pair.children.clone();
            children[child].endpoints[word] += Fp::ONE;
            assert!(
                ScheduleCircuit::prepare(
                    invalid.pair.plan.clone(),
                    &input,
                    children,
                    &vesta,
                    Fp::ONE,
                    &FoldConfig::default(),
                )
                .is_err()
            );
        }
    }
    let digest = input.digest();
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
        .expect("known zero-proof witnesses have the same bounded circuit shape");
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
        "zero proof tapes cannot authenticate a schedule"
    );
}

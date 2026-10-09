//! Complete-span preservation, intermediate-state attacks and fixed source layouts.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn original(length: usize) -> (Vec<u8>, [u8; 32]) {
    let payload = (0..length)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect::<Vec<_>>();
    let mut preimage = EPOCH_TAG.to_vec();
    preimage.extend_from_slice(b"NRT0\0\0");
    preimage.extend_from_slice(&EPOCH_CODEC_ID);
    preimage.push(0);
    preimage.extend_from_slice(&u64::try_from(length).unwrap().to_le_bytes());
    preimage.extend_from_slice(&norito::core::hardware_crc64(&payload).to_le_bytes());
    preimage.push(2);
    preimage.extend_from_slice(&payload);
    (payload, iroha_crypto::Hash::new(preimage).into())
}
fn batches(length: usize) -> Vec<ContextHashBatchCircuit> {
    let (frame, id) = original(length);
    prepare_context_batches(frame, 0, u32::try_from(length).unwrap(), id).unwrap()
}
fn accepts(source: &ContextHashBatchCircuit) -> bool {
    source.instances().is_ok_and(|public| {
        check_circuit(source, 16, &public, CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
    })
}

#[test]
fn fixed_pairs_cover_every_original_instruction_and_full_boundaries() {
    for length in [1, 32, 33, 289, 65_536] {
        let (frame, id) = original(length);
        let leaves =
            prepare_context_hash(frame.clone(), 0, u32::try_from(length).unwrap(), id).unwrap();
        let batches =
            prepare_context_batches(frame, 0, u32::try_from(length).unwrap(), id).unwrap();
        assert_eq!(batches.len(), 1_281);
        assert_eq!(batches.len(), BATCH_LENGTH as usize);
        let mut offset = 0;
        for (position, batch) in batches.iter().enumerate() {
            assert_eq!(
                batch.plan,
                ContextHashBatchPlan::at(u32::try_from(position).unwrap()).unwrap()
            );
            assert_eq!(batch.leaves.len(), batch.plan.count());
            let first = leaves[offset].endpoints();
            offset += batch.leaves.len();
            let last = leaves[offset - 1].endpoints();
            assert_eq!(
                batch.endpoints(),
                [first[0], first[1], first[2], last[3], first[4], last[5]]
            );
        }
        assert_eq!(offset, PROGRAM_LENGTH as usize);
        assert_eq!(
            batches[0].endpoints()[4],
            boundary_digest_native(batches[0].input(), false)
        );
        assert_eq!(
            batches.last().unwrap().endpoints()[5],
            boundary_digest_native(batches[0].input(), true)
        );
        for adjacent in batches.windows(2) {
            let a = adjacent[0].endpoints();
            let b = adjacent[1].endpoints();
            assert_eq!(a[..2], b[..2]);
            assert_eq!(a[3], b[2]);
            assert_eq!(a[5], b[4]);
        }
    }
    assert!(ContextHashBatchPlan::at(BATCH_LENGTH).is_none());
    assert!(ContextHashBatchPlan::at(u32::MAX).is_none());
}

#[test]
fn every_batch_class_fits_and_preserves_known_unknown_and_position_layouts() {
    let batches = batches(289);
    for indices in [vec![0, 1, 1_023], vec![1_024, 1_025, 1_279], vec![1_280]] {
        let first = &batches[indices[0]];
        assert!(accepts(first));
        let known = synthesize(first, 16, Some(&first.instances().unwrap())).unwrap();
        let unknown = synthesize(&first.without_witnesses(), 16, None).unwrap();
        let imported =
            synthesize(&ContextHashBatchCircuit::for_source(first.plan), 16, None).unwrap();
        for other in [&unknown, &imported] {
            assert_eq!(known.cs, other.cs);
            assert_eq!(known.tables.fixed(), other.tables.fixed());
            assert_eq!(known.tables.selectors(), other.tables.selectors());
            assert_eq!(known.tables.permutation(), other.tables.permutation());
            assert_eq!(
                known.tables.advice_assigned(),
                other.tables.advice_assigned()
            );
        }
        let rows = known
            .tables
            .advice_assigned()
            .iter()
            .map(|column| column.iter().rposition(|v| *v).map_or(0, |i| i + 1))
            .max()
            .unwrap();
        eprintln!(
            "CONTEXT_BATCH_SOURCE plan={:?} steps={} max_rows={rows} hard_k16=true source_proof=false",
            first.plan,
            first.plan.count()
        );
        for index in indices.into_iter().skip(1) {
            assert!(accepts(&batches[index]));
            let actual = synthesize(
                &batches[index],
                16,
                Some(&batches[index].instances().unwrap()),
            )
            .unwrap();
            assert_eq!(known.cs, actual.cs);
            assert_eq!(known.tables.fixed(), actual.tables.fixed());
            assert_eq!(known.tables.selectors(), actual.tables.selectors());
            assert_eq!(known.tables.permutation(), actual.tables.permutation());
            assert_eq!(
                known.tables.advice_assigned(),
                actual.tables.advice_assigned()
            );
        }
    }
}

#[test]
fn batch_joins_bind_every_intermediate_register_context_and_cursor() {
    let batches = batches(289);
    for index in [0, 1_024] {
        let original = &batches[index];
        assert!(accepts(original));
        for register in 0..9 {
            let mut changed = original.clone();
            if register == 0 {
                changed.leaves[1].before.crc ^= 1;
            } else {
                changed.leaves[1].before.blake[register - 1] ^= 1;
            }
            assert!(
                !accepts(&changed),
                "unbound intermediate register{register}"
            );
        }
        let mut context = original.clone();
        context.leaves[1].input.context_id[0] ^= 1;
        assert!(!accepts(&context));
        let mut skipped = original.clone();
        skipped.leaves[1].cursor += 1;
        assert!(!accepts(&skipped));
        let mut reversed = original.clone();
        reversed.leaves.reverse();
        assert!(!accepts(&reversed));
        let mut dropped = original.clone();
        dropped.leaves.pop();
        assert!(!accepts(&dropped));
        let mut phase = original.clone();
        phase.leaves[1].phase = if phase.plan.phase() == ContextHashPhase::Crc {
            ContextHashPhase::Blake
        } else {
            ContextHashPhase::Crc
        };
        assert!(!accepts(&phase));
        let mut last = original.clone();
        last.leaves[1].after.blake[7] ^= 1;
        assert!(!accepts(&last));
        let mut detached = original.leaves.clone();
        detached[1].before.crc ^= 1;
        assert!(ContextHashBatchCircuit::new(original.plan, detached).is_err());
    }
}

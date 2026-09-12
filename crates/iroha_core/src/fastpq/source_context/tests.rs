//! Exact source capture, sticky conflict and rollback-merge boundary checks.

use super::*;
use iroha_crypto::HashOf;
use iroha_data_model::NetworkId;

fn snapshot() -> FastpqBlockStartSourceContext {
    FastpqBlockStartSourceContext {
        source: FastpqSourceStatementContextV1 {
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"genesis",
            ))),
            height: 42,
        },
        lane_incarnations: BTreeMap::from([
            (LaneId::SINGLE, Some(Hash::new(b"full active incarnation"))),
            (LaneId::new(9), None),
        ]),
    }
}

#[test]
fn captures_full_lane_incarnation_and_explicit_unrouted_protocol_context() {
    let snapshot = snapshot();
    let hash = Hash::new(b"call");
    let source = snapshot
        .capture_transcript(
            Some(hash),
            hash,
            Some(LaneId::SINGLE),
            Some(DataSpaceId::new(7)),
            3,
        )
        .unwrap();
    assert_eq!(source.source(), snapshot.source);
    assert_eq!(source.entry_hash(), hash);
    assert_eq!(source.first_fragment_index(), 3);
    assert_eq!(source.dataspace_id(), DataSpaceId::new(7));
    assert!(!source.is_protocol_purpose());
    assert_eq!(
        source.execution_kind(),
        FastpqSourceExecutionKindV1::ExecutionCall
    );
    assert_eq!(
        source.route(),
        FastpqCapturedSourceRoute::Lane(FastpqSourceLaneV1 {
            lane_id: LaneId::SINGLE,
            lane_incarnation: snapshot.lane_incarnations[&LaneId::SINGLE].unwrap(),
        })
    );
    for dataspace in [None, Some(DataSpaceId::new(7))] {
        let source = snapshot
            .capture_transcript(None, hash, None, dataspace, 0)
            .unwrap();
        assert!(source.is_protocol_purpose());
        assert_eq!(
            source.execution_kind(),
            FastpqSourceExecutionKindV1::ProtocolPurpose
        );
        assert_eq!(source.route(), FastpqCapturedSourceRoute::Unrouted);
        assert_eq!(
            source.dataspace_id(),
            dataspace.unwrap_or(DataSpaceId::UNIVERSAL)
        );
    }
}

#[test]
fn rejects_unresolved_lane_and_active_call_mismatch() {
    let snapshot = snapshot();
    let hash = Hash::new(b"call");
    for lane in [LaneId::new(9), LaneId::new(100)] {
        assert_eq!(
            snapshot.capture_transcript(Some(hash), hash, Some(lane), None, 0),
            Err(FastpqSourceCaptureError::MissingLaneIncarnation {
                lane_id: lane,
                entry_hash: hash
            })
        );
    }
    assert_eq!(
        snapshot.capture_transcript(Some(Hash::new(b"other")), hash, None, None, 0),
        Err(FastpqSourceCaptureError::ExecutionIdentityMismatch)
    );
}

#[test]
fn merge_preserves_unique_sources_and_earliest_fragment() {
    let snapshot = snapshot();
    let hash = Hash::new(b"call");
    let mut outer = FastpqSourceCaptureAccumulator::default();
    outer.record(snapshot.capture_transcript(Some(hash), hash, None, None, 3));
    let mut pending = FastpqSourceCaptureAccumulator::default();
    pending.record(snapshot.capture_transcript(Some(hash), hash, None, None, 1));
    let purpose = Hash::new(b"typed protocol purpose");
    pending.record(snapshot.capture_transcript(None, purpose, None, None, 4));
    outer.merge(pending);
    let sources = outer.sources().unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[&hash].first_fragment_index(), 1);
    assert_eq!(sources[&purpose].first_fragment_index(), 4);
    assert!(sources[&purpose].is_protocol_purpose());
}

#[test]
fn every_conflict_is_sticky_and_hides_all_partial_sources() {
    let snapshot = snapshot();
    let hash = Hash::new(b"call");
    let original = snapshot
        .capture_transcript(Some(hash), hash, Some(LaneId::SINGLE), None, 0)
        .unwrap();
    for mutation in 0..6 {
        let mut changed = original;
        match mutation {
            0 => changed.source.height += 1,
            1 => {
                changed.source.network_id = NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"other genesis")),
                )
            }
            2 => {
                let FastpqCapturedSourceRoute::Lane(FastpqSourceLaneV1 {
                    ref mut lane_incarnation,
                    ..
                }) = changed.route
                else {
                    unreachable!()
                };
                let mut bytes: [u8; 32] = (*lane_incarnation).into();
                bytes[20] ^= 1;
                *lane_incarnation = Hash::prehashed(bytes);
            }
            3 => changed.route = FastpqCapturedSourceRoute::Unrouted,
            4 => changed.execution_kind = FastpqSourceExecutionKindV1::ProtocolPurpose,
            5 => changed.dataspace_id = DataSpaceId::new(7),
            _ => unreachable!(),
        }
        let mut outer = FastpqSourceCaptureAccumulator::default();
        outer.record(Ok(original));
        let mut pending = FastpqSourceCaptureAccumulator::default();
        pending.record(Ok(changed));
        outer.merge(pending);
        assert_eq!(
            outer.sources(),
            Err(&FastpqSourceCaptureError::ConflictingSource { entry_hash: hash }),
            "mutation {mutation}"
        );
        outer.record(Ok(original));
        outer.merge(FastpqSourceCaptureAccumulator::default());
        assert!(outer.sources().is_err());
    }
    let mut outer = FastpqSourceCaptureAccumulator::default();
    outer.record(Ok(original));
    let mut pending = FastpqSourceCaptureAccumulator::default();
    pending.record(Err(FastpqSourceCaptureError::ExecutionIdentityMismatch));
    outer.merge(pending);
    assert_eq!(
        outer.sources(),
        Err(&FastpqSourceCaptureError::ExecutionIdentityMismatch)
    );
}

#[test]
fn final_source_access_requires_one_successful_seal_and_reseal_preserves_it() {
    let snapshot = snapshot();
    let hash = Hash::new(b"seal once");
    for with_capture in [false, true] {
        let mut captures = FastpqSourceCaptureAccumulator::default();
        assert_eq!(
            captures.sealed_sources(),
            Err(FastpqSourceCaptureError::CaptureNotSealed)
        );
        if with_capture {
            captures.record(snapshot.capture_transcript(Some(hash), hash, None, None, 3));
        }
        let expected = captures.sources().unwrap().clone();
        assert_eq!(captures.seal(), Ok(()));
        assert_eq!(captures.sealed_sources(), Ok(&expected));
        assert_eq!(
            captures.seal(),
            Err(FastpqSourceCaptureError::CaptureAlreadySealed)
        );
        assert_eq!(captures.sealed_sources(), Ok(&expected));
        assert_eq!(captures.sources(), Ok(&expected));
    }
}

#[test]
fn every_record_after_sealing_invalidates_even_an_identical_existing_key() {
    let snapshot = snapshot();
    let hash = Hash::new(b"record before seal");
    let original = snapshot
        .capture_transcript(Some(hash), hash, None, None, 3)
        .unwrap();
    let new_hash = Hash::new(b"record after seal");
    let other = snapshot
        .capture_transcript(Some(new_hash), new_hash, None, None, 4)
        .unwrap();
    for late in [
        Ok(original),
        Ok(other),
        Err(FastpqSourceCaptureError::ExecutionIdentityMismatch),
    ] {
        let mut captures = FastpqSourceCaptureAccumulator::default();
        captures.record(Ok(original));
        captures.seal().unwrap();
        captures.record(late);
        assert_eq!(
            captures.sources(),
            Err(&FastpqSourceCaptureError::AppliedAfterSeal)
        );
        assert_eq!(
            captures.sealed_sources(),
            Err(FastpqSourceCaptureError::AppliedAfterSeal)
        );
        assert!(captures.entries.is_empty());
        assert_eq!(
            captures.seal(),
            Err(FastpqSourceCaptureError::AppliedAfterSeal)
        );
        captures.record(Ok(original));
        captures.record(Err(FastpqSourceCaptureError::FragmentIndexOutOfRange));
        captures.merge(FastpqSourceCaptureAccumulator::default());
        assert_eq!(
            captures.sealed_sources(),
            Err(FastpqSourceCaptureError::AppliedAfterSeal)
        );
    }
}

#[test]
fn late_nonempty_and_failed_merges_invalidate_the_sealed_map() {
    let snapshot = snapshot();
    let hash = Hash::new(b"merge before seal");
    let original = snapshot
        .capture_transcript(Some(hash), hash, None, None, 3)
        .unwrap();
    let new_hash = Hash::new(b"merge after seal");
    let other = snapshot
        .capture_transcript(Some(new_hash), new_hash, None, None, 4)
        .unwrap();
    for late in [
        Ok(original),
        Ok(other),
        Err(FastpqSourceCaptureError::ExecutionIdentityMismatch),
    ] {
        let mut captures = FastpqSourceCaptureAccumulator::default();
        captures.record(Ok(original));
        captures.seal().unwrap();
        let mut pending = FastpqSourceCaptureAccumulator::default();
        pending.record(late);
        // A failed pending accumulator has no entries, but its failure still means that
        // an applied capture occurred after the destination's sealed inventory boundary.
        captures.merge(pending);
        assert_eq!(
            captures.sealed_sources(),
            Err(FastpqSourceCaptureError::AppliedAfterSeal)
        );
        assert_eq!(
            captures.sources(),
            Err(&FastpqSourceCaptureError::AppliedAfterSeal)
        );
        assert!(captures.entries.is_empty());
    }
}

#[test]
fn healthy_empty_merges_preserve_open_and_sealed_destinations() {
    let snapshot = snapshot();
    let hash = Hash::new(b"empty merges are inert");
    let original = snapshot
        .capture_transcript(Some(hash), hash, None, None, 3)
        .unwrap();
    for destination_sealed in [false, true] {
        for empty_sealed in [false, true] {
            let mut captures = FastpqSourceCaptureAccumulator::default();
            captures.record(Ok(original));
            if destination_sealed {
                captures.seal().unwrap();
            }
            let mut empty = FastpqSourceCaptureAccumulator::default();
            if empty_sealed {
                empty.seal().unwrap();
            }
            captures.merge(empty);
            assert_eq!(captures.sources().unwrap().get(&hash), Some(&original));
            assert_eq!(captures.sealed, destination_sealed);
            assert_eq!(captures.sealed_sources().is_ok(), destination_sealed);
        }
    }
}

#[test]
fn sealing_and_late_merges_preserve_the_first_existing_failure() {
    let snapshot = snapshot();
    let hash = Hash::new(b"first error survives");
    let original = snapshot
        .capture_transcript(Some(hash), hash, None, None, 3)
        .unwrap();
    for first in [
        FastpqSourceCaptureError::ExecutionIdentityMismatch,
        FastpqSourceCaptureError::ConflictingSource { entry_hash: hash },
        FastpqSourceCaptureError::AppliedAfterSeal,
    ] {
        let mut captures = FastpqSourceCaptureAccumulator::default();
        captures.record(Ok(original));
        captures.record(Err(first));
        assert_eq!(captures.seal(), Err(first));
        assert!(!captures.sealed);
        assert_eq!(captures.sealed_sources(), Err(first));
        let mut pending = FastpqSourceCaptureAccumulator::default();
        pending.record(Ok(original));
        captures.merge(pending);
        let mut failed = FastpqSourceCaptureAccumulator::default();
        failed.record(Err(FastpqSourceCaptureError::FragmentIndexOutOfRange));
        captures.merge(failed);
        captures.record(Ok(original));
        assert_eq!(captures.sources(), Err(&first));
        assert_eq!(captures.sealed_sources(), Err(first));
        assert!(captures.entries.is_empty());
    }
}

#[test]
fn cloned_seals_remain_closed_and_transaction_local_defaults_remain_open() {
    let snapshot = snapshot();
    let hash = Hash::new(b"transaction local scope");
    let original = snapshot
        .capture_transcript(Some(hash), hash, None, None, 3)
        .unwrap();
    let mut captures = FastpqSourceCaptureAccumulator::default();
    captures.record(Ok(original));
    captures.seal().unwrap();
    let mut copied = captures.clone();
    copied.record(Ok(original));
    assert_eq!(
        copied.sealed_sources(),
        Err(FastpqSourceCaptureError::AppliedAfterSeal)
    );
    assert_eq!(
        captures.sealed_sources().unwrap().get(&hash),
        Some(&original)
    );
    let mut pending = FastpqSourceCaptureAccumulator::default();
    pending.record(Ok(original));
    assert_eq!(pending.sources().unwrap().get(&hash), Some(&original));
    assert_eq!(
        pending.sealed_sources(),
        Err(FastpqSourceCaptureError::CaptureNotSealed)
    );
    // A rollback discards this local scope and therefore cannot affect the closed block scope.
    drop(pending);
    assert_eq!(
        captures.sealed_sources().unwrap().get(&hash),
        Some(&original)
    );
}

#[test]
fn extraction_moves_only_exact_selected_sources_and_rejects_missing_sources_atomically() {
    let context = snapshot();
    let first = Hash::new(b"lane transcript");
    let other = Hash::new(b"carrier transcript");
    let absent = Hash::new(b"missing capture");
    let mut captures = FastpqSourceCaptureAccumulator::default();
    for hash in [first, other] {
        captures.record(context.capture_transcript(Some(hash), hash, None, None, 0));
    }
    let original = captures.sources().unwrap().clone();
    assert_eq!(
        captures.take_unsealed_sources(&BTreeSet::from([first, absent])),
        Err(FastpqSourceCaptureError::MissingSource { entry_hash: absent })
    );
    assert_eq!(captures.sources(), Ok(&original));
    let extracted = captures
        .take_unsealed_sources(&BTreeSet::from([first]))
        .unwrap();
    assert_eq!(extracted, BTreeMap::from([(first, original[&first])]));
    assert_eq!(
        captures.sources().unwrap(),
        &BTreeMap::from([(other, original[&other])])
    );
    captures.seal().unwrap();
    assert_eq!(captures.sealed_sources().unwrap().len(), 1);
}

#[test]
fn extraction_cannot_mutate_a_sealed_inventory_or_clear_a_sticky_failure() {
    let context = snapshot();
    let hash = Hash::new(b"sealed transcript");
    let mut captures = FastpqSourceCaptureAccumulator::default();
    captures.record(context.capture_transcript(Some(hash), hash, None, None, 0));
    let original = captures.sources().unwrap().clone();
    captures.seal().unwrap();
    assert_eq!(
        captures.take_unsealed_sources(&BTreeSet::from([hash])),
        Err(FastpqSourceCaptureError::CaptureAlreadySealed)
    );
    assert_eq!(captures.sealed_sources(), Ok(&original));

    let mut failed = FastpqSourceCaptureAccumulator::default();
    failed.record(Err(FastpqSourceCaptureError::ExecutionIdentityMismatch));
    for selection in [BTreeSet::new(), BTreeSet::from([hash])] {
        assert_eq!(
            failed.take_unsealed_sources(&selection),
            Err(FastpqSourceCaptureError::ExecutionIdentityMismatch)
        );
        assert_eq!(
            failed.sources(),
            Err(&FastpqSourceCaptureError::ExecutionIdentityMismatch)
        );
    }
}

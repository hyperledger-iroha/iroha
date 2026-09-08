//! Complete source-entry commitments and independently expected archive projections.

use super::*;
use crate::fastpq::{
    FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1, FastpqOrdinarySourceStatementArchiveV1,
    FastpqSourceArchiveDecodeLimits, decode_fastpq_ordinary_source_statement_archive_v1,
    verify_fastpq_ordinary_source_statement_archive_v1,
};

fn source() -> FastpqSourceStatementContextV1 {
    FastpqSourceStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"complete source entry test network",
        ))),
        height: 19,
    }
}

fn entries() -> Vec<FastpqSourceExecutionEntryV1> {
    (0_u32..3)
        .map(|index| FastpqSourceExecutionEntryV1 {
            entry_hash: Hash::new(index.to_le_bytes()),
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Lane(FastpqSourceLaneV1 {
                lane_id: LaneId::new(2),
                lane_incarnation: Hash::new(b"complete source entry test incarnation"),
            }),
            dataspace_id: DataSpaceId::new(4),
        })
        .collect()
}

fn leaf(entry: FastpqSourceExecutionEntryV1) -> FastpqOrdinarySourceStatementLeafV1 {
    FastpqOrdinarySourceStatementLeafV1 {
        source: source(),
        statement_index: 0,
        entry_index: 0,
        transcript_index: 0,
        entry_transcript_count: 1,
        entry_hash: entry.entry_hash,
        execution_kind: entry.execution_kind,
        route: entry.route,
        dataspace_id: entry.dataspace_id,
        statement_digest: [7; 32],
    }
}

fn archive(
    entries: &[FastpqSourceExecutionEntryV1],
    leaves: Vec<FastpqOrdinarySourceStatementLeafV1>,
) -> FastpqOrdinarySourceStatementArchiveV1 {
    FastpqOrdinarySourceStatementArchiveV1 {
        version: FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1,
        manifest: build_fastpq_ordinary_source_statement_manifest_v1(
            source(),
            entries,
            &leaves,
            3,
            1,
        )
        .unwrap(),
        leaves,
        manifest_siblings: [Hash::new([]); 256],
    }
}

fn ordinary_root(archive: &FastpqOrdinarySourceStatementArchiveV1) -> Hash {
    let path = Hash::new(FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1);
    let value = Hash::new(norito::encode_canonical(&archive.manifest).unwrap());
    let mut preimage = [0_u8; 65];
    preimage[1..33].copy_from_slice(path.as_ref());
    preimage[33..].copy_from_slice(value.as_ref());
    let mut current = Hash::new(preimage);
    preimage[0] = 1;
    for (level, sibling) in archive.manifest_siblings.iter().enumerate() {
        let bit = 255 - level;
        let right = path.as_ref()[bit / 8] & (1 << (bit % 8)) != 0;
        let (left, right) = if right {
            (*sibling, current)
        } else {
            (current, *sibling)
        };
        preimage[1..33].copy_from_slice(left.as_ref());
        preimage[33..].copy_from_slice(right.as_ref());
        current = Hash::new(preimage);
    }
    current
}

fn bounds(bytes: usize, max_entries: u32) -> FastpqSourceArchiveDecodeLimits {
    FastpqSourceArchiveDecodeLimits {
        max_wire_bytes: bytes,
        max_executed_entries: max_entries,
        max_statements: 1,
        norito: norito::DecodeLimits::new(16, 1024 * 1024, 1024, 4 * 1024 * 1024, 64),
    }
}

#[test]
fn source_entry_digest_streams_canonical_frames_at_exact_entry_cap() {
    let all = entries();
    for count in 0..=all.len() {
        let projection = &all[..count];
        let count = u32::try_from(count).unwrap();
        let mut preimage = b"iroha:fastpq:source-execution-entries:v1\0".to_vec();
        preimage.extend_from_slice(&count.to_le_bytes());
        for entry in projection {
            let frame = norito::encode_canonical(entry).unwrap();
            assert!(frame.len() <= FASTPQ_SOURCE_EXECUTION_ENTRY_MAX_BYTES_V1);
            assert_eq!(
                norito::decode_canonical::<FastpqSourceExecutionEntryV1>(&frame).unwrap(),
                *entry,
            );
            preimage.extend_from_slice(&u32::try_from(frame.len()).unwrap().to_le_bytes());
            preimage.extend_from_slice(&frame);
        }
        assert_eq!(
            fastpq_source_execution_entries_digest_v1(projection, count),
            Some(Hash::new(preimage)),
        );
        if count > 0 {
            assert_eq!(
                fastpq_source_execution_entries_digest_v1(projection, count - 1),
                None,
            );
        }
    }
}

#[test]
fn source_entry_digest_binds_order_multiplicity_and_every_entry_field() {
    let original = entries();
    let digest = fastpq_source_execution_entries_digest_v1(&original, 4).unwrap();
    for mutation in 0..9 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.swap(0, 1),
            1 => changed.push(changed[2]),
            2 => {
                changed.remove(1);
            }
            3 => changed[1].entry_hash = Hash::new(b"changed zero-transfer execution"),
            4 => changed[1].execution_kind = FastpqSourceExecutionKindV1::ProtocolPurpose,
            5 => changed[1].route = FastpqSourceRouteV1::Unrouted,
            6 => {
                let FastpqSourceRouteV1::Lane(mut route) = changed[1].route else {
                    unreachable!()
                };
                route.lane_id = LaneId::new(u32::MAX);
                changed[1].route = FastpqSourceRouteV1::Lane(route);
            }
            7 => {
                let FastpqSourceRouteV1::Lane(mut route) = changed[1].route else {
                    unreachable!()
                };
                let mut incarnation: [u8; 32] = route.lane_incarnation.into();
                incarnation[29] ^= 1;
                route.lane_incarnation = Hash::prehashed(incarnation);
                changed[1].route = FastpqSourceRouteV1::Lane(route);
            }
            8 => changed[1].dataspace_id = DataSpaceId::new(u64::MAX),
            _ => unreachable!(),
        }
        assert_ne!(
            fastpq_source_execution_entries_digest_v1(&changed, 4).unwrap(),
            digest,
            "mutation {mutation}",
        );
    }
}

#[test]
fn source_entry_digest_uses_canonical_layout_and_restores_ambient_flags() {
    let entries = entries();
    let digest = fastpq_source_execution_entries_digest_v1(&entries, 3).unwrap();
    for flags in [0, norito::core::default_encode_flags(), 0x1f] {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let before = norito::core::get_decode_flags();
        assert_eq!(
            fastpq_source_execution_entries_digest_v1(&entries, 3),
            Some(digest),
        );
        assert_eq!(norito::core::get_decode_flags(), before);
    }
}

#[test]
fn source_entry_manifest_binds_nontransfer_entries_with_unchanged_leaf_root() {
    let entries = entries();
    for leaves in [vec![], vec![leaf(entries[0])]] {
        let original =
            build_fastpq_ordinary_source_statement_manifest_v1(source(), &entries, &leaves, 3, 1)
                .unwrap();
        let mut changed = entries.clone();
        changed[2].entry_hash = Hash::new(b"different entry with no transfers");
        let changed =
            build_fastpq_ordinary_source_statement_manifest_v1(source(), &changed, &leaves, 3, 1)
                .unwrap();
        assert_eq!(original.statement_root, changed.statement_root);
        assert_eq!(original.statement_count, changed.statement_count);
        assert_eq!(original.executed_entry_count, changed.executed_entry_count);
        assert_ne!(
            original.source_entries_digest,
            changed.source_entries_digest
        );
        assert_ne!(
            norito::encode_canonical(&original).unwrap(),
            norito::encode_canonical(&changed).unwrap(),
        );
    }
}

#[test]
fn source_entry_manifest_rejects_leaf_fields_outside_its_expected_entry() {
    let entries = entries();
    for mutation in 0..6 {
        let mut changed = leaf(entries[0]);
        match mutation {
            0 => changed.entry_hash = entries[1].entry_hash,
            1 => changed.execution_kind = FastpqSourceExecutionKindV1::ProtocolPurpose,
            2 => changed.route = FastpqSourceRouteV1::Unrouted,
            3 => changed.dataspace_id = DataSpaceId::new(9),
            4 => changed.entry_index = 1,
            5 => changed.entry_index = u32::MAX,
            _ => unreachable!(),
        }
        assert!(
            build_fastpq_ordinary_source_statement_manifest_v1(
                source(),
                &entries,
                &[changed],
                3,
                1,
            )
            .is_none(),
            "mutation {mutation}",
        );
    }
}

#[test]
fn source_entry_manifest_rejects_the_unqualified_fieldless_digest_layout() {
    #[derive(NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::iroha_data_model::PreviousManifest",
        frame = "iroha_data_model::fastpq::FastpqOrdinarySourceStatementManifestV1"
    )]
    struct PreviousManifest {
        source: FastpqSourceStatementContextV1,
        executed_entry_count: u32,
        statement_count: u32,
        statement_root: Hash,
    }
    let old = PreviousManifest {
        source: source(),
        executed_entry_count: 3,
        statement_count: 0,
        statement_root: fastpq_ordinary_source_statement_empty_root_v1(),
    };
    assert_eq!(
        norito::schema::identity::frame_hash::<PreviousManifest>(),
        norito::schema::identity::frame_hash::<FastpqOrdinarySourceStatementManifestV1>(),
    );
    let frame = norito::encode_canonical(&old).unwrap();
    assert!(norito::decode_canonical::<FastpqOrdinarySourceStatementManifestV1>(&frame).is_err());
}

#[test]
fn source_entry_archive_checks_independent_nontransfer_entries_and_empty_projection() {
    let entries = entries();
    for leaves in [vec![], vec![leaf(entries[0])]] {
        let archive = archive(&entries, leaves);
        let root = ordinary_root(&archive);
        let frame = norito::encode_canonical(&archive).unwrap();
        assert!(verify_fastpq_ordinary_source_statement_archive_v1(
            &archive,
            source(),
            &entries,
            root,
            3,
            1,
        ));
        assert_eq!(
            decode_fastpq_ordinary_source_statement_archive_v1(
                &frame,
                source(),
                &entries,
                root,
                bounds(frame.len(), 3),
            )
            .unwrap(),
            archive,
        );
        let mut changed = entries.clone();
        changed[1].entry_hash = Hash::new(b"substituted zero-transfer expectation");
        for expected in [&changed[..], &entries[..2]] {
            assert!(!verify_fastpq_ordinary_source_statement_archive_v1(
                &archive,
                source(),
                expected,
                root,
                3,
                1,
            ));
            assert!(
                decode_fastpq_ordinary_source_statement_archive_v1(
                    &frame,
                    source(),
                    expected,
                    root,
                    bounds(frame.len(), 3),
                )
                .is_err()
            );
        }
    }
    let archive = archive(&[], vec![]);
    let root = ordinary_root(&archive);
    let frame = norito::encode_canonical(&archive).unwrap();
    assert_eq!(
        decode_fastpq_ordinary_source_statement_archive_v1(
            &frame,
            source(),
            &[],
            root,
            bounds(frame.len(), 0),
        )
        .unwrap(),
        archive,
    );
    let mut malformed = archive;
    malformed.manifest.source_entries_digest = Hash::new(b"not the empty projection digest");
    assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
        &malformed.manifest,
        source(),
        &malformed.manifest_siblings,
        ordinary_root(&malformed),
        0,
        1,
    ));
}

#[test]
fn source_entry_archive_caps_expected_entries_before_decoding() {
    let entries = entries();
    let limits = bounds(1, 2);
    let (result, usage) = norito::core::with_decode_limits_measured(limits.norito, || {
        decode_fastpq_ordinary_source_statement_archive_v1(
            &[0],
            source(),
            &entries,
            Hash::new([]),
            limits,
        )
    });
    assert!(matches!(result, Err(norito::Error::Message(message))
        if message == "FASTPQ expected source entries exceed the executed-entry limit"));
    assert_eq!(usage.total_elements(), 0);
    assert_eq!(usage.total_allocated_bytes(), 0);
}

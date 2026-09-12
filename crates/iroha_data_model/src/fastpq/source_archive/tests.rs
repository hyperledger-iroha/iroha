//! Complete archive transport, source/root binding and cumulative decoding budgets.

use super::*;
use crate::{
    NetworkId,
    execution_witness::FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
    fastpq::{FastpqSourceExecutionKindV1, FastpqSourceLaneV1, FastpqSourceRouteV1},
};
use iroha_crypto::HashOf;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};

// The transport cases share this independently defined five-entry source
// projection. Dedicated entry-binding tests call the public APIs with altered
// expectations directly; these helpers never derive expectations from input bytes.
fn expected_entries() -> Vec<FastpqSourceExecutionEntryV1> {
    (0_u32..5)
        .map(|index| FastpqSourceExecutionEntryV1 {
            entry_hash: match index {
                0 => Hash::new(b"first"),
                2 => Hash::new(b"second"),
                4 => Hash::new(b"third"),
                _ => Hash::new(index.to_le_bytes()),
            },
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Unrouted,
            dataspace_id: DataSpaceId::new(4),
        })
        .collect()
}

fn verify_archive(
    archive: &FastpqOrdinarySourceStatementArchiveV1,
    source: FastpqSourceStatementContextV1,
    root: Hash,
    max_entries: u32,
    max_statements: u32,
) -> bool {
    verify_fastpq_ordinary_source_statement_archive_v1(
        archive,
        source,
        &expected_entries(),
        root,
        max_entries,
        max_statements,
    )
}

fn decode_archive(
    bytes: &[u8],
    source: FastpqSourceStatementContextV1,
    root: Hash,
    limits: FastpqSourceArchiveDecodeLimits,
) -> Result<FastpqOrdinarySourceStatementArchiveV1, norito::Error> {
    decode_fastpq_ordinary_source_statement_archive_v1(
        bytes,
        source,
        &expected_entries(),
        root,
        limits,
    )
}

fn source() -> FastpqSourceStatementContextV1 {
    FastpqSourceStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"archive network",
        ))),
        height: 19,
    }
}

fn fixture(count: u32) -> FastpqOrdinarySourceStatementArchiveV1 {
    let leaves = (0..count)
        .map(|index| FastpqOrdinarySourceStatementLeafV1 {
            source: source(),
            statement_index: index,
            entry_index: index * 2,
            entry_transcript_count: match index {
                0 => 2,
                1 => 4,
                _ => 1,
            },
            entry_hash: match index {
                0 => Hash::new(b"first"),
                1 => Hash::new(b"second"),
                _ => Hash::new(b"third"),
            },
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Unrouted,
            dataspace_id: DataSpaceId::new(4),
            statement_digest: [u8::try_from(index).expect("fixture value fits u8") + 9; 32],
        })
        .collect::<Vec<_>>();
    let manifest = build_fastpq_ordinary_source_statement_manifest_v1(
        source(),
        &expected_entries(),
        &leaves,
        5,
        count,
    )
    .unwrap();
    FastpqOrdinarySourceStatementArchiveV1 {
        version: FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1,
        manifest,
        leaves,
        manifest_siblings: [Hash::new([]); 256],
    }
}

// Model-local fixture for a tree containing exactly the fixed manifest write.
// The receiver's separate tests cover the real Core ordinary-tree constructor.
fn expected_root(archive: &FastpqOrdinarySourceStatementArchiveV1) -> Hash {
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

fn limits(wire: usize, statements: u32) -> FastpqSourceArchiveDecodeLimits {
    FastpqSourceArchiveDecodeLimits {
        max_wire_bytes: wire,
        max_executed_entries: 5,
        max_statements: statements,
        norito: norito::DecodeLimits::new(1024, 1024 * 1024, 1024 * 1024, 4 * 1024 * 1024, 64),
    }
}

#[test]
fn complete_empty_and_ragged_archives_roundtrip_at_exact_caps() {
    for count in [0, 1, 2, 3] {
        let archive = fixture(count);
        let root = expected_root(&archive);
        let bytes = norito::encode_canonical(&archive).unwrap();
        assert!(verify_archive(&archive, source(), root, 5, count));
        if count > 0 {
            assert!(
                archive
                    .leaves
                    .iter()
                    .any(|leaf| leaf.entry_transcript_count > count),
                "transcript cardinality must remain independent of the leaf cap"
            );
        }
        assert_eq!(
            decode_archive(&bytes, source(), root, limits(bytes.len(), count)).unwrap(),
            archive
        );
        assert!(decode_archive(&bytes, source(), root, limits(bytes.len() - 1, count)).is_err());
        if count > 0 {
            assert!(
                decode_archive(&bytes, source(), root, limits(bytes.len(), count - 1)).is_err()
            );
        }
    }
    let mut archive = fixture(0);
    archive.manifest =
        build_fastpq_ordinary_source_statement_manifest_v1(source(), &[], &[], 0, 0).unwrap();
    let root = expected_root(&archive);
    let bytes = norito::encode_canonical(&archive).unwrap();
    let mut zero = limits(bytes.len(), 0);
    zero.max_executed_entries = 0;
    zero.norito = norito::DecodeLimits::new(0, 1024 * 1024, 1024 * 1024, 4 * 1024 * 1024, 64);
    assert_eq!(
        decode_fastpq_ordinary_source_statement_archive_v1(&bytes, source(), &[], root, zero)
            .unwrap(),
        archive
    );
}

#[test]
fn archive_verification_binds_complete_contents_source_root_and_version() {
    let original = fixture(3);
    let root = expected_root(&original);
    for mutation in 0..13 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.version = 0,
            1 => changed.version = 2,
            2 => {
                changed.leaves.pop();
            }
            3 => changed.leaves.swap(0, 1),
            4 => changed.leaves[0].statement_digest[0] ^= 1,
            5 => changed.leaves[1].entry_transcript_count = 1,
            6 => changed.manifest.statement_count -= 1,
            7 => changed.manifest.statement_root = Hash::new(b"substituted root"),
            8 => changed.manifest_siblings[128] = Hash::new(b"substituted sibling"),
            9 => changed.manifest.executed_entry_count = 4,
            10 => {
                changed.leaves[1] = changed.leaves[0];
                changed.leaves[1].statement_index = 1;
            }
            11 => changed.leaves[1].entry_transcript_count = 0,
            12 => {
                changed.leaves.remove(0);
                for (index, leaf) in changed.leaves.iter_mut().enumerate() {
                    leaf.statement_index = index as u32;
                }
            }
            _ => unreachable!(),
        }
        assert!(
            !verify_archive(&changed, source(), root, 5, 3),
            "mutation {mutation}"
        );
        let bytes = norito::encode_canonical(&changed).unwrap();
        assert!(decode_archive(&bytes, source(), root, limits(bytes.len(), 3)).is_err());
    }
    let mut wrong = source();
    wrong.height += 1;
    assert!(!verify_archive(&original, wrong, root, 5, 3));
    wrong = source();
    wrong.network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"other network")));
    assert!(!verify_archive(&original, wrong, root, 5, 3));
    assert!(!verify_archive(
        &original,
        source(),
        Hash::new(b"untrusted replacement root"),
        5,
        3
    ));
    assert!(!verify_archive(&original, source(), root, 4, 3));
}

#[test]
fn raw_wire_limit_precedes_header_and_nominal_schema_is_required() {
    let error = decode_archive(&[0xFF], source(), Hash::new([]), limits(0, 0)).unwrap_err();
    assert!(
        matches!(error, norito::Error::Message(message) if message == "FASTPQ source archive exceeds wire-byte limit")
    );
    let archive = fixture(0);
    let wrong_type = norito::encode_canonical(&archive.manifest).unwrap();
    assert!(matches!(
        decode_archive(
            &wrong_type,
            source(),
            expected_root(&archive),
            limits(wrong_type.len(), 0)
        ),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn statement_and_caller_sequence_caps_reject_before_archive_validation() {
    let archive = fixture(3);
    let bytes = norito::encode_canonical(&archive).unwrap();
    let root = expected_root(&archive);
    for cap in [0, 1, 2] {
        let error = decode_archive(&bytes, source(), root, limits(bytes.len(), cap)).unwrap_err();
        assert!(
            matches!(error, norito::Error::SequenceLengthExceeded { length: 3, limit } if limit == u64::from(cap)),
            "leaf cap must fail in decode: {error}"
        );
    }
    let mut strict = limits(bytes.len(), 3);
    strict.norito = norito::DecodeLimits::new(2, 1024 * 1024, 1024 * 1024, 4 * 1024 * 1024, 64);
    assert!(matches!(
        decode_archive(&bytes, source(), root, strict),
        Err(norito::Error::SequenceLengthExceeded {
            length: 3,
            limit: 2
        })
    ));
    let outer = norito::DecodeLimits::new(1, 1024 * 1024, 1024 * 1024, 4 * 1024 * 1024, 64);
    assert!(matches!(
        norito::core::with_decode_limits_scope(outer, || {
            decode_archive(&bytes, source(), root, strict)
        }),
        Err(norito::Error::SequenceLengthExceeded {
            length: 3,
            limit: 1
        })
    ));
    assert_eq!(
        decode_archive(&bytes, source(), root, limits(bytes.len(), 3)).unwrap(),
        archive
    );
}

#[test]
fn caller_and_outer_field_element_and_depth_limits_remain_effective() {
    let archive = fixture(3);
    let bytes = norito::encode_canonical(&archive).unwrap();
    let root = expected_root(&archive);
    let bounds = limits(bytes.len(), 3);
    for (dimension, strict) in [
        (
            "field",
            norito::DecodeLimits::new(1024, 0, 1024 * 1024, 4 * 1024 * 1024, 64),
        ),
        (
            "elements",
            norito::DecodeLimits::new(1024, 1024 * 1024, 2, 4 * 1024 * 1024, 64),
        ),
        (
            "depth",
            norito::DecodeLimits::new(1024, 1024 * 1024, 1024 * 1024, 4 * 1024 * 1024, 0),
        ),
    ] {
        for outer in [false, true] {
            let error = if outer {
                norito::core::with_decode_limits_scope(strict, || {
                    decode_archive(&bytes, source(), root, bounds)
                })
            } else {
                decode_archive(
                    &bytes,
                    source(),
                    root,
                    FastpqSourceArchiveDecodeLimits {
                        norito: strict,
                        ..bounds
                    },
                )
            }
            .unwrap_err();
            let expected = match dimension {
                "field" => {
                    matches!(error, norito::Error::FieldLengthExceeded { length, limit: 0 } if length > 0)
                }
                "elements" => matches!(
                    error,
                    norito::Error::TotalElementsExceeded {
                        attempted: 3,
                        limit: 2
                    }
                ),
                "depth" => {
                    matches!(error, norito::Error::NestingDepthExceeded { depth, limit: 0, .. } if depth > 0)
                }
                _ => unreachable!(),
            };
            assert!(expected, "{dimension} limit, outer={outer}: {error}");
            assert_eq!(
                decode_archive(&bytes, source(), root, bounds).unwrap(),
                archive
            );
        }
    }
}

#[test]
fn archive_decoding_counts_only_leaves_in_cumulative_outer_element_limits() {
    for count in [0, 1, 3] {
        let archive = fixture(count);
        let bytes = norito::encode_canonical(&archive).unwrap();
        let root = expected_root(&archive);
        let bounds = limits(bytes.len(), count);
        let run = || decode_archive(&bytes, source(), root, bounds);
        let (result, usage) = norito::core::with_decode_limits_measured(bounds.norito, run);
        assert_eq!(result.unwrap(), archive);
        let charged = usage.total_elements();
        assert_eq!(
            charged,
            usize::try_from(count).unwrap(),
            "fixed arrays must not consume variable-sequence elements"
        );
        let exact = norito::DecodeLimits::new(1024, 1024 * 1024, charged, 4 * 1024 * 1024, 64);
        norito::core::with_decode_limits_scope(exact, || {
            assert_eq!(run().unwrap(), archive);
            if count == 0 {
                assert_eq!(
                    run().unwrap(),
                    archive,
                    "empty archives remain valid under a zero element budget"
                );
            } else {
                let error = run().unwrap_err();
                assert!(
                    matches!(error, norito::Error::TotalElementsExceeded { attempted, limit } if attempted == 2 * u64::from(count) && limit == u64::from(count)),
                    "later archives must retain their caller's charges: {error}"
                );
            }
        });
        if count > 0 {
            let short =
                norito::DecodeLimits::new(1024, 1024 * 1024, charged - 1, 4 * 1024 * 1024, 64);
            let error = norito::core::with_decode_limits_scope(short, run).unwrap_err();
            assert!(
                matches!(error, norito::Error::TotalElementsExceeded { attempted, limit } if attempted == u64::from(count) && limit == u64::from(count - 1)),
                "one element short must reject: {error}"
            );
        }
        assert_eq!(run().unwrap(), archive);
    }
}

#[test]
fn checksummed_oversized_leaf_counts_fail_before_sequence_allocation() {
    let archive = fixture(3);
    let root = expected_root(&archive);
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (bare, flags) = norito::codec::encode_with_header_flags(&archive);
    // Read the actual version and manifest field boundaries, then locate the
    // leaf count through the codec. Only that fixed u64 count is replaced.
    let mut offset = 0;
    for _ in 0..2 {
        let (length, prefix) =
            norito::core::read_len_from_slice_with_flags(&bare[offset..], flags).unwrap();
        offset += prefix + length;
    }
    let (length, prefix) =
        norito::core::read_len_from_slice_with_flags(&bare[offset..], flags).unwrap();
    let start = offset + prefix;
    let (count, count_prefix) =
        norito::core::inspect_seq_len_slice(&bare[start..start + length]).unwrap();
    assert_eq!(count, archive.leaves.len());
    assert_eq!(count_prefix, core::mem::size_of::<u64>());
    for declared in [u64::from(u32::MAX) + 1, u64::MAX] {
        let mut hostile = bare.clone();
        hostile[start..start + count_prefix].copy_from_slice(&declared.to_le_bytes());
        let frame = norito::core::frame_bare_with_header_flags::<
            FastpqOrdinarySourceStatementArchiveV1,
        >(&hostile, flags)
        .unwrap();
        let bounds = limits(frame.len(), 3);
        let (result, usage) = norito::core::with_decode_limits_measured(bounds.norito, || {
            decode_archive(&frame, source(), root, bounds)
        });
        let error = result.unwrap_err();
        assert!(
            matches!(error, norito::Error::SequenceLengthExceeded { length, limit: 1024 } if length == declared),
            "forged count must fail before platform conversion and vector reservation: {error}"
        );
        assert_eq!(
            usage.total_elements(),
            0,
            "a rejected count must not be charged or materialized"
        );
    }
}

#[test]
fn archive_rejects_alternate_layout_checksum_damage_truncation_and_suffixes() {
    let archive = fixture(3);
    let canonical = norito::encode_canonical(&archive).unwrap();
    let root = expected_root(&archive);
    let alternate = {
        let flags = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(flags);
        norito::core::to_bytes(&archive).unwrap()
    };
    assert_ne!(alternate, canonical);
    assert_eq!(
        norito::decode_from_bytes_with_limits::<FastpqOrdinarySourceStatementArchiveV1>(
            &alternate,
            limits(alternate.len(), 3).norito
        )
        .unwrap(),
        archive,
        "the alternate frame must be valid for the ordinary decoder"
    );
    assert!(matches!(
        decode_archive(&alternate, source(), root, limits(alternate.len(), 3)),
        Err(norito::Error::NonCanonicalEncoding)
    ));
    let mut damaged = canonical.clone();
    *damaged.last_mut().unwrap() ^= 0x80;
    assert!(matches!(
        decode_archive(&damaged, source(), root, limits(damaged.len(), 3)),
        Err(norito::Error::ChecksumMismatch)
    ));
    let mut extended = canonical.clone();
    extended.push(0);
    for malformed in [&canonical[..canonical.len() - 1], extended.as_slice()] {
        assert!(matches!(
            decode_archive(malformed, source(), root, limits(malformed.len(), 3)),
            Err(norito::Error::LengthMismatch)
        ));
    }
}

#[test]
fn fixed_manifest_path_rejects_other_lengths_and_vector_layouts() {
    let archive = fixture(0);
    let root = expected_root(&archive);
    macro_rules! path_frame {
        ($path_type:ty, $path:expr) => {{
            #[derive(NoritoSerialize, norito::NoritoSchema)]
            #[norito_schema(
                name = "test::iroha_data_model::ForgedArchive",
                frame = "iroha_data_model::fastpq::FastpqOrdinarySourceStatementArchiveV1"
            )]
            struct ForgedArchive {
                version: u16,
                manifest: FastpqOrdinarySourceStatementManifestV1,
                leaves: Vec<FastpqOrdinarySourceStatementLeafV1>,
                manifest_siblings: $path_type,
            }
            assert_eq!(
                norito::schema::identity::frame_hash::<ForgedArchive>(),
                norito::schema::identity::frame_hash::<FastpqOrdinarySourceStatementArchiveV1>()
            );
            norito::encode_canonical(&ForgedArchive {
                version: archive.version,
                manifest: archive.manifest,
                leaves: archive.leaves.clone(),
                manifest_siblings: $path,
            })
            .unwrap()
        }};
    }
    let exact = path_frame!([Hash; 256], archive.manifest_siblings);
    assert_eq!(exact, norito::encode_canonical(&archive).unwrap());
    assert_eq!(
        decode_archive(&exact, source(), root, limits(exact.len(), 0)).unwrap(),
        archive
    );
    let short = path_frame!([Hash; 255], [Hash::new([]); 255]);
    let long = path_frame!([Hash; 257], [Hash::new([]); 257]);
    for malformed in [short, long] {
        assert!(decode_archive(&malformed, source(), root, limits(malformed.len(), 0)).is_err());
    }
    // A vector has an extra count header: even 256 values are not the fixed
    // array wire representation. Use the same nominal schema deliberately.
    for count in [0, 255, 256, 257] {
        let vector = path_frame!(Vec<Hash>, vec![Hash::new([]); count]);
        assert!(decode_archive(&vector, source(), root, limits(vector.len(), 0)).is_err());
    }
}

#[test]
fn routed_and_native_purpose_archives_roundtrip_and_bind_exact_route_identity() {
    let lane = FastpqSourceLaneV1 {
        lane_id: LaneId::new(u32::MAX),
        lane_incarnation: Hash::new(b"archive source lane incarnation"),
    };
    for kind in [
        FastpqSourceExecutionKindV1::ExecutionCall,
        FastpqSourceExecutionKindV1::ProtocolPurpose,
    ] {
        for route in [
            FastpqSourceRouteV1::Unrouted,
            FastpqSourceRouteV1::Lane(lane),
        ] {
            let mut archive = fixture(2);
            for leaf in &mut archive.leaves {
                leaf.execution_kind = kind;
                leaf.route = route;
                leaf.dataspace_id = DataSpaceId::new(u64::MAX);
            }
            let mut entries = expected_entries();
            for entry_index in [0, 2] {
                entries[entry_index].execution_kind = kind;
                entries[entry_index].route = route;
                entries[entry_index].dataspace_id = DataSpaceId::new(u64::MAX);
            }
            archive.manifest = build_fastpq_ordinary_source_statement_manifest_v1(
                source(),
                &entries,
                &archive.leaves,
                5,
                2,
            )
            .unwrap();
            let root = expected_root(&archive);
            let frame = norito::encode_canonical(&archive).unwrap();
            assert_eq!(
                decode_fastpq_ordinary_source_statement_archive_v1(
                    &frame,
                    source(),
                    &entries,
                    root,
                    limits(frame.len(), 2)
                )
                .unwrap(),
                archive
            );
            for mutation in 0..3 {
                let mut changed = archive.clone();
                for leaf in &mut changed.leaves {
                    match mutation {
                        0 => {
                            leaf.execution_kind = match kind {
                                FastpqSourceExecutionKindV1::ExecutionCall => {
                                    FastpqSourceExecutionKindV1::ProtocolPurpose
                                }
                                FastpqSourceExecutionKindV1::ProtocolPurpose => {
                                    FastpqSourceExecutionKindV1::ExecutionCall
                                }
                            }
                        }
                        1 => {
                            leaf.route = FastpqSourceRouteV1::Lane(FastpqSourceLaneV1 {
                                lane_incarnation: Hash::new(b"replacement incarnation"),
                                ..lane
                            })
                        }
                        2 => leaf.dataspace_id = DataSpaceId::new(0),
                        _ => unreachable!(),
                    }
                }
                // Keep each entry structurally consistent and rebuild its leaf
                // root; the separately expected ordinary root must still reject it.
                let mut changed_entries = entries.clone();
                for leaf in &changed.leaves {
                    let entry = &mut changed_entries[usize::try_from(leaf.entry_index).unwrap()];
                    entry.execution_kind = leaf.execution_kind;
                    entry.route = leaf.route;
                    entry.dataspace_id = leaf.dataspace_id;
                }
                changed.manifest = build_fastpq_ordinary_source_statement_manifest_v1(
                    source(),
                    &changed_entries,
                    &changed.leaves,
                    5,
                    2,
                )
                .unwrap();
                assert!(!verify_fastpq_ordinary_source_statement_archive_v1(
                    &changed,
                    source(),
                    &changed_entries,
                    root,
                    5,
                    2,
                ));
                assert!(!verify_archive(&changed, source(), root, 5, 2));
                let frame = norito::encode_canonical(&changed).unwrap();
                assert!(
                    decode_fastpq_ordinary_source_statement_archive_v1(
                        &frame,
                        source(),
                        &changed_entries,
                        root,
                        limits(frame.len(), 2),
                    )
                    .is_err()
                );
                assert!(decode_archive(&frame, source(), root, limits(frame.len(), 2)).is_err());
            }
        }
    }
}

#[test]
fn archive_decoding_preserves_exact_and_repeated_outer_allocation_limits() {
    for count in [0, 3] {
        let archive = fixture(count);
        let bytes = norito::encode_canonical(&archive).unwrap();
        let root = expected_root(&archive);
        let bounds = limits(bytes.len(), count);
        let run = || decode_archive(&bytes, source(), root, bounds);
        let (result, usage) = norito::core::with_decode_limits_measured(bounds.norito, run);
        assert_eq!(result.unwrap(), archive);
        let charged = usage.total_allocated_bytes();
        assert!(charged > 0);
        let exact = norito::DecodeLimits::new(1024, 1024 * 1024, 1024 * 1024, charged, 64);
        norito::core::with_decode_limits_scope(exact, || {
            assert_eq!(run().unwrap(), archive);
            assert!(
                run().is_err(),
                "second archive must share its caller's cumulative charge"
            );
        });
        let short = norito::DecodeLimits::new(1024, 1024 * 1024, 1024 * 1024, charged - 1, 64);
        assert!(norito::core::with_decode_limits_scope(short, run).is_err());
        assert_eq!(run().unwrap(), archive);
    }
}

#[test]
fn canonical_archive_bytes_and_caller_flags_remain_stable() {
    let archive = fixture(3);
    let canonical = norito::encode_canonical(&archive).unwrap();
    let root = expected_root(&archive);
    for flags in (u8::MIN..=u8::MAX).filter(|&f| norito::core::validate_header_flags(f).is_ok()) {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&archive).unwrap(), canonical);
        assert_eq!(
            decode_archive(&canonical, source(), root, limits(canonical.len(), 3)).unwrap(),
            archive
        );
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
}

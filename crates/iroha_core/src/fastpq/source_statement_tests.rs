//! Cross-crate FASTPQ source-opening parity against the actual ordinary-write tree.
use super::{FastpqSourceOpeningBuildLimits, fastpq_ordinary_source_statement_opening_v1};
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    NetworkId, execution_witness::FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1, fastpq::*,
};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([seed])))
}

fn entries() -> Vec<FastpqSourceExecutionEntryV1> {
    (0..5)
        .map(|i| FastpqSourceExecutionEntryV1 {
            entry_hash: if i % 2 == 0 {
                Hash::new([i / 2])
            } else {
                Hash::new([100 + i])
            },
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Lane(FastpqSourceLaneV1 {
                lane_id: LaneId::new(2),
                lane_incarnation: Hash::new(b"source lane incarnation"),
            }),
            dataspace_id: DataSpaceId::new(4),
        })
        .collect()
}

fn leaves() -> Vec<FastpqOrdinarySourceStatementLeafV1> {
    (0..3)
        .map(|i| FastpqOrdinarySourceStatementLeafV1 {
            source: FastpqSourceStatementContextV1 {
                network_id: network(7),
                height: 19,
            },
            statement_index: i,
            entry_index: i * 2,
            entry_transcript_count: 1,
            entry_hash: Hash::new([i as u8]),
            execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
            route: FastpqSourceRouteV1::Lane(FastpqSourceLaneV1 {
                lane_id: LaneId::new(2),
                lane_incarnation: Hash::new(b"source lane incarnation"),
            }),
            dataspace_id: DataSpaceId::new(4),
            statement_digest: [i as u8 + 9; 32],
        })
        .collect()
}

#[test]
fn manifest_write_matches_existing_core_sparse_tree_and_rejects_substitution() {
    let leaves = leaves();
    let manifest = build_fastpq_ordinary_source_statement_manifest_v1(
        leaves[0].source,
        &entries(),
        &leaves,
        5,
        5,
    )
    .unwrap();
    let encoded = norito::encode_canonical(&manifest).unwrap();
    assert!(encoded.len() < 512);
    let writes = [crate::sumeragi::smt::KvPair::new(
        FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
        encoded,
    )];
    let root = crate::sumeragi::smt::compute_post_state_root(&[], &writes);
    let siblings = [Hash::new([]); 256];
    assert!(verify_fastpq_ordinary_source_statement_manifest_write_v1(
        &manifest,
        manifest.source,
        &siblings,
        root,
        5,
        5
    ));
    for mutation in 0..6 {
        let mut changed = manifest;
        match mutation {
            0 => changed.source.network_id = network(8),
            1 => changed.source.height += 1,
            2 => changed.executed_entry_count += 1,
            3 => changed.statement_count -= 1,
            4 => changed.statement_root = Hash::new(b"another statement tree"),
            5 => changed.source_entries_digest = Hash::new(b"another source inventory"),
            _ => unreachable!(),
        }
        assert!(
            !verify_fastpq_ordinary_source_statement_manifest_write_v1(
                &changed,
                changed.source,
                &siblings,
                root,
                6,
                6
            ),
            "field {mutation}"
        );
    }
    for level in [0, 1, 127, 255] {
        let mut changed = siblings;
        changed[level] = Hash::new([level as u8]);
        assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
            &manifest,
            manifest.source,
            &changed,
            root,
            5,
            5
        ));
    }
    for len in [0, 255, 257] {
        assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
            &manifest,
            manifest.source,
            &vec![Hash::new([]); len],
            root,
            5,
            5
        ));
    }
    assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
        &manifest,
        manifest.source,
        &siblings,
        Hash::new(b"other root"),
        5,
        5
    ));
    assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
        &manifest,
        manifest.source,
        &siblings,
        root,
        4,
        4
    ));
}

#[test]
fn empty_manifest_write_is_present_and_layout_invariant() {
    let source = leaves()[0].source;
    let manifest =
        build_fastpq_ordinary_source_statement_manifest_v1(source, &entries(), &[], 5, 5).unwrap();
    let writes = [crate::sumeragi::smt::KvPair::new(
        FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
        norito::encode_canonical(&manifest).unwrap(),
    )];
    let root = crate::sumeragi::smt::compute_post_state_root(&[], &writes);
    let siblings = [Hash::new([]); 256];
    for flags in (u8::MIN..=u8::MAX).filter(|&f| norito::core::validate_header_flags(f).is_ok()) {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert!(verify_fastpq_ordinary_source_statement_manifest_write_v1(
            &manifest, source, &siblings, root, 5, 5
        ));
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
    assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
        &manifest,
        source,
        &siblings,
        crate::sumeragi::smt::compute_post_state_root(&[], &[]),
        5,
        5
    ));
    let mut changed = manifest;
    changed.statement_root = Hash::new(b"nonempty root with zero count");
    assert!(!verify_fastpq_ordinary_source_statement_manifest_write_v1(
        &changed, source, &siblings, root, 5, 5
    ));
}
fn opening_fixture() -> (FastpqOrdinarySourceStatementOpeningV1, Hash) {
    let leaves = leaves();
    let manifest = build_fastpq_ordinary_source_statement_manifest_v1(
        leaves[0].source,
        &entries(),
        &leaves,
        5,
        5,
    )
    .unwrap();
    let tree: MerkleTree<_> = leaves
        .iter()
        .map(|leaf| fastpq_ordinary_source_statement_leaf_hash_v1(leaf).unwrap())
        .collect();
    let writes = [crate::sumeragi::smt::KvPair::new(
        FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
        norito::encode_canonical(&manifest).unwrap(),
    )];
    let root = crate::sumeragi::smt::compute_post_state_root(&[], &writes);
    (
        FastpqOrdinarySourceStatementOpeningV1 {
            manifest,
            leaf: leaves[2],
            membership: tree.get_proof(2).unwrap(),
            manifest_siblings: vec![Hash::new([]); 256],
        },
        root,
    )
}

fn opening_limits() -> norito::DecodeLimits {
    norito::DecodeLimits::new(1_024, 32 * 1_024, 64 * 1_024, 512 * 1_024, 32)
}

#[test]
fn bounded_opening_roundtrip_and_both_inclusions_require_expected_context() {
    let (opening, root) = opening_fixture();
    let bytes = norito::encode_canonical(&opening).unwrap();
    let decoded =
        decode_fastpq_ordinary_source_statement_opening_v1(&bytes, bytes.len(), opening_limits())
            .unwrap();
    assert_eq!(decoded, opening);
    assert!(verify_fastpq_ordinary_source_statement_opening_v1(
        &decoded,
        &opening.leaf,
        root,
        5,
        5
    ));
    let mut changed = decoded.clone();
    changed.leaf.entry_hash = Hash::new(b"another entry");
    assert!(!verify_fastpq_ordinary_source_statement_opening_v1(
        &changed,
        &changed.leaf,
        root,
        5,
        5
    ));
    let mut changed = decoded.clone();
    changed.manifest.source.network_id = network(8);
    assert!(!verify_fastpq_ordinary_source_statement_opening_v1(
        &changed,
        &opening.leaf,
        root,
        5,
        5
    ));
    assert!(!verify_fastpq_ordinary_source_statement_opening_v1(
        &decoded,
        &opening.leaf,
        Hash::new(b"another finalized root"),
        5,
        5
    ));
    assert!(
        decode_fastpq_ordinary_source_statement_opening_v1(
            &bytes,
            bytes.len() - 1,
            opening_limits()
        )
        .is_err()
    );
    assert!(matches!(
        decode_fastpq_ordinary_source_statement_opening_v1(&[0xff], 0, opening_limits()),
        Err(norito::Error::Message(message)) if message == "source opening exceeds wire-byte limit"
    ));
    let wrong_schema = norito::encode_canonical(&opening.manifest).unwrap();
    assert!(
        decode_fastpq_ordinary_source_statement_opening_v1(
            &wrong_schema,
            bytes.len(),
            opening_limits()
        )
        .is_err()
    );
}

#[test]
fn bounded_opening_keeps_enclosing_cumulative_allocation_limits() {
    let (opening, _) = opening_fixture();
    let bytes = norito::encode_canonical(&opening).unwrap();
    let run = || {
        decode_fastpq_ordinary_source_statement_opening_v1(&bytes, bytes.len(), opening_limits())
    };
    let (result, measured) = norito::core::with_decode_limits_measured(opening_limits(), run);
    result.unwrap();
    let charged = measured.total_allocated_bytes();
    assert!(charged > 0);
    let exact = norito::DecodeLimits::new(1_024, 32 * 1_024, 64 * 1_024, charged, 32);
    norito::core::with_decode_limits_scope(exact, || {
        run().unwrap();
        assert!(
            run().is_err(),
            "a second opening cannot reset the caller budget"
        );
    });
    let short = norito::DecodeLimits::new(1_024, 32 * 1_024, 64 * 1_024, charged - 1, 32);
    assert!(norito::core::with_decode_limits_scope(short, run).is_err());
    for count in [255, 257] {
        let mut changed = opening.clone();
        changed.manifest_siblings.resize(count, Hash::new([]));
        let bytes = norito::encode_canonical(&changed).unwrap();
        assert!(
            decode_fastpq_ordinary_source_statement_opening_v1(
                &bytes,
                bytes.len(),
                opening_limits()
            )
            .is_err()
        );
    }
}

fn witness_fixture() -> (
    iroha_data_model::block::consensus::ExecWitness,
    Vec<FastpqOrdinarySourceStatementLeafV1>,
) {
    use iroha_data_model::block::consensus::{ExecKv, ExecWitness};
    let leaves = leaves();
    let manifest = build_fastpq_ordinary_source_statement_manifest_v1(
        leaves[0].source,
        &entries(),
        &leaves,
        5,
        5,
    )
    .unwrap();
    let witness = ExecWitness {
        reads: vec![ExecKv {
            key: b"irrelevant read".to_vec(),
            value: b"read value".to_vec(),
        }],
        writes: vec![
            ExecKv {
                key: b"ordinary write".to_vec(),
                value: b"first".to_vec(),
            },
            ExecKv {
                key: FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1.to_vec(),
                value: norito::encode_canonical(&manifest).unwrap(),
            },
            ExecKv {
                key: b"another ordinary write".to_vec(),
                value: b"other value".to_vec(),
            },
            ExecKv {
                key: b"ordinary write".to_vec(),
                value: b"last".to_vec(),
            },
        ],
        ..ExecWitness::default()
    };
    (witness, leaves)
}

fn source_build_limits() -> FastpqSourceOpeningBuildLimits {
    FastpqSourceOpeningBuildLimits {
        max_ordinary_writes: 16,
        max_ordinary_write_bytes: 64 * 1024,
        max_executed_entries: 8,
        max_statements: 8,
        manifest_decode: opening_limits(),
    }
}

#[test]
fn source_opening_producer_matches_mixed_write_tree_and_every_ragged_leaf() {
    let (witness, leaves) = witness_fixture();
    let ordinary = witness
        .writes
        .iter()
        .map(|write| crate::sumeragi::smt::KvPair::new(write.key.clone(), write.value.clone()))
        .collect::<Vec<_>>();
    let expected = crate::sumeragi::smt::compute_post_state_root(&[], &ordinary);
    for index in 0..3 {
        let (opening, root) = fastpq_ordinary_source_statement_opening_v1(
            &witness,
            leaves[0].source,
            &entries(),
            &leaves,
            index,
            source_build_limits(),
        )
        .unwrap();
        assert_eq!(root, expected);
        assert_eq!(opening.manifest_siblings.len(), 256);
        assert!(
            opening
                .manifest_siblings
                .iter()
                .any(|sibling| *sibling != Hash::new([]))
        );
        assert!(verify_fastpq_ordinary_source_statement_opening_v1(
            &opening,
            &leaves[index as usize],
            root,
            5,
            5
        ));
        let frame = norito::encode_canonical(&opening).unwrap();
        assert_eq!(
            decode_fastpq_ordinary_source_statement_opening_v1(
                &frame,
                frame.len(),
                opening_limits()
            )
            .unwrap(),
            opening
        );
    }
}

#[test]
fn source_opening_producer_rejects_entire_malformed_or_duplicate_reserved_family() {
    let (original, leaves) = witness_fixture();
    for mutation in 0..5 {
        let mut witness = original.clone();
        match mutation {
            0 => {
                witness.writes.remove(1);
            }
            1 => witness.writes.push(witness.writes[1].clone()),
            2 => witness.writes[1].key.truncate(1),
            3 => witness.writes[1].key.push(0),
            4 => {
                let mut malformed = witness.writes[1].clone();
                malformed.key.truncate(1);
                witness.writes.push(malformed);
            }
            _ => unreachable!(),
        }
        assert!(
            fastpq_ordinary_source_statement_opening_v1(
                &witness,
                leaves[0].source,
                &entries(),
                &leaves,
                0,
                source_build_limits()
            )
            .is_err(),
            "reserved family mutation {mutation}"
        );
    }
}

#[test]
fn source_opening_producer_requires_exact_complete_archive_source_and_position() {
    let (witness, leaves) = witness_fixture();
    for mutation in 0..6 {
        let mut changed = leaves.clone();
        match mutation {
            0 => changed.swap(0, 1),
            1 => {
                changed.remove(1);
                changed[1].statement_index = 1;
            }
            2 => changed[1].statement_digest[0] ^= 1,
            3 => {
                changed[1].route = FastpqSourceRouteV1::Lane(FastpqSourceLaneV1 {
                    lane_id: LaneId::new(2),
                    lane_incarnation: Hash::new(b"another incarnation"),
                })
            }
            4 => changed[1].route = FastpqSourceRouteV1::Unrouted,
            5 => changed[1].execution_kind = FastpqSourceExecutionKindV1::ProtocolPurpose,
            _ => unreachable!(),
        }
        assert!(
            fastpq_ordinary_source_statement_opening_v1(
                &witness,
                leaves[0].source,
                &entries(),
                &changed,
                0,
                source_build_limits()
            )
            .is_err()
        );
    }
    for source in [
        FastpqSourceStatementContextV1 {
            network_id: network(9),
            ..leaves[0].source
        },
        FastpqSourceStatementContextV1 {
            height: 20,
            ..leaves[0].source
        },
    ] {
        assert!(
            fastpq_ordinary_source_statement_opening_v1(
                &witness,
                source,
                &entries(),
                &leaves,
                0,
                source_build_limits()
            )
            .is_err()
        );
    }
    assert!(
        fastpq_ordinary_source_statement_opening_v1(
            &witness,
            leaves[0].source,
            &entries(),
            &leaves,
            3,
            source_build_limits()
        )
        .is_err()
    );
}

#[test]
fn source_opening_producer_enforces_write_entry_byte_and_decode_budgets() {
    let (witness, leaves) = witness_fixture();
    let mut limits = source_build_limits();
    limits.max_ordinary_writes = witness.writes.len();
    limits.max_executed_entries = 5;
    limits.max_ordinary_write_bytes = witness
        .writes
        .iter()
        .map(|write| write.key.len() + write.value.len())
        .sum();
    assert!(
        fastpq_ordinary_source_statement_opening_v1(
            &witness,
            leaves[0].source,
            &entries(),
            &leaves,
            0,
            limits
        )
        .is_ok()
    );
    for mutation in 0..4 {
        let mut changed = limits;
        match mutation {
            0 => changed.max_ordinary_writes -= 1,
            1 => changed.max_ordinary_write_bytes -= 1,
            2 => changed.max_executed_entries -= 1,
            3 => changed.manifest_decode = norito::DecodeLimits::new(1, 1, 1, 1, 1),
            _ => unreachable!(),
        }
        assert!(
            fastpq_ordinary_source_statement_opening_v1(
                &witness,
                leaves[0].source,
                &entries(),
                &leaves,
                0,
                changed
            )
            .is_err(),
            "budget mutation {mutation}"
        );
    }
}

#[test]
fn source_opening_producer_rejects_wrong_schema_and_oversized_manifest_values() {
    let (witness, leaves) = witness_fixture();
    for value in [
        norito::encode_canonical(&leaves[0]).unwrap(),
        vec![0xff; FASTPQ_SOURCE_STATEMENT_MANIFEST_MAX_BYTES_V1 + 1],
    ] {
        let mut changed = witness.clone();
        changed.writes[1].value = value;
        assert!(
            fastpq_ordinary_source_statement_opening_v1(
                &changed,
                leaves[0].source,
                &entries(),
                &leaves,
                0,
                source_build_limits()
            )
            .is_err()
        );
    }
}

#[test]
fn ordinary_recorder_cannot_alias_the_protected_source_manifest_key() {
    use crate::sumeragi::witness;
    use iroha_data_model::execution_witness::ExecutionWitnessKeyTagV1;
    use iroha_model_base::name::Name;
    use iroha_primitives::json::Json;
    let _guard = witness::exec_witness_guard();
    witness::start_block();
    let key: Name = "fastpq_source_statements".parse().unwrap();
    let value = Json::new("iroha:fastpq:ordinary-source-statements:v1");
    witness::record_write_account_kv(&iroha_test_samples::ALICE_ID, &key, &value);
    witness::record_read_account_kv(&iroha_test_samples::ALICE_ID, &key, Some(&value));
    let captured = witness::drain_exec_witness();
    assert_eq!(captured.reads.len(), 1);
    assert_eq!(captured.writes.len(), 1);
    for entry in captured.reads.iter().chain(&captured.writes) {
        assert_eq!(
            entry.key[0],
            ExecutionWitnessKeyTagV1::AccountMetadata as u8
        );
        assert_ne!(
            entry.key[0],
            FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1[0]
        );
    }
}

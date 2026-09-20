//! Prepared public-API archive, crypto, SDK transport and committed-rejection controls.

use super::*;

#[test]
fn public_offline_block_writer_yields_complete_one_and_four_lane_evidence() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, None);
        assert_eq!(
            fixture.network_id,
            NetworkId::from_genesis_hash(fixture.genesis.hash())
        );
        assert_eq!(fixture.entry.active_lanes.len(), lanes);
        assert_eq!(fixture.requests.len(), 8);
        assert_eq!(fixture.requests.iter().filter(|r| r.warmup).count(), 4);
        for (index, request) in fixture.requests.iter().enumerate() {
            assert_eq!(request.logical_id, format!("{index:064x}"));
            assert_eq!(
                request.route,
                RoutingDecision::new(LaneId::new((index % lanes) as u32), DataSpaceId::UNIVERSAL)
            );
        }
        let complete = fixture.complete();
        assert_eq!(complete.committed_height(), 2);
        assert_eq!(complete.carrier_count(), 2);
        assert_eq!(complete.merge_frames(), 1);
        assert!(complete.output_bytes() > 0);
        complete.recheck_sources().unwrap();
    }
}

#[test]
fn real_global_finality_covers_exact_canonical_wire_and_merge_reference() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, None);
        let mut verifier = BridgeFinalityVerifier::with_context(
            fixture.network_id,
            fixture.first.finality_artifact.context_id(),
        );
        for (proof, block) in [
            (&fixture.first, &fixture.genesis),
            (&fixture.second, &fixture.carrier),
        ] {
            verifier.verify(proof).unwrap();
            proof
                .finality_artifact
                .validate_for_header(&block.header())
                .unwrap();
            let wire = block.encode_wire().unwrap();
            let execution = &proof.finality_artifact.commit_qc.execution_commitment;
            assert_eq!(execution.executed_block_wire_hash, Hash::new(&wire));
            assert_eq!(execution.executed_block_wire_len, wire.len() as u64);
            assert_eq!(proof.finality_artifact.height_context.roster.len(), 4);
            assert_eq!(proof.finality_artifact.commit_qc.signers, vec![0, 1, 2]);
            assert!(block.has_results());
            block.validate_entrypoint_merkle_cache().unwrap();
            block.validate_result_merkle_cache().unwrap();
        }
        assert_eq!(
            fixture
                .second
                .finality_artifact
                .commit_qc
                .execution_commitment
                .merge_carrier,
            Some(MergeCarrierCommitmentV1::new(
                fixture.entry.canonical_hash()
            ))
        );
    }
}

#[test]
fn rejected_committed_leaf_retains_valid_proofs_and_exact_offer_identity() {
    for lanes in [1, 4] {
        for rejected in [0, 7] {
            let fixture = Fixture::new(lanes, Some(rejected));
            let queries = fixture.queries();
            assert_eq!(queries.len(), 8);
            assert_eq!(queries.iter().filter(|q| q.result.is_err()).count(), 1);
            for query in &queries {
                assert!(query.verify_inclusion_in_block(&fixture.carrier));
                assert_eq!(
                    query.result.is_err(),
                    query.entrypoint_hash == fixture.requests[rejected].signed.hash_as_entrypoint()
                );
            }
            fixture.complete().recheck_sources().unwrap();
        }
    }
}

#[test]
fn public_sdk_returns_exact_finality_and_preserves_committed_rejection() {
    for lanes in [1, 4] {
        for rejected in [None, Some(7)] {
            let fixture = Fixture::new(lanes, rejected);
            let queries = fixture.queries();
            let mut replies = vec![
                Reply::capabilities(),
                Reply::finality(&fixture.first),
                Reply::finality(&fixture.second),
            ];
            replies.extend(queries.iter().cloned().map(Reply::details));
            let transport = ScriptedTransport::new(replies);
            let client = fixture.client(transport.clone());
            let mut verifier = BridgeFinalityVerifier::with_context(
                fixture.network_id,
                fixture.first.finality_artifact.context_id(),
            );
            for expected in [&fixture.first, &fixture.second] {
                let proof = client
                    .get_bridge_finality_proof(
                        expected.block_header.height(),
                        expected.block_header.hash(),
                        &mut verifier,
                    )
                    .unwrap();
                assert_eq!(
                    norito::encode_canonical(&proof).unwrap(),
                    norito::encode_canonical(expected).unwrap()
                );
            }
            let mut rejected_count = 0;
            for expected in queries {
                let details = client
                    .get_transaction_details(expected.entrypoint_hash)
                    .unwrap();
                assert_eq!(details.transaction, expected);
                assert!(
                    details
                        .transaction
                        .verify_inclusion_in_block(&fixture.carrier)
                );
                rejected_count += usize::from(details.transaction.result.is_err());
            }
            assert_eq!(rejected_count, usize::from(rejected.is_some()));
            transport.assert_drained(11);
            fixture.complete().recheck_sources().unwrap();
        }
    }
}

#[test]
fn public_sdk_rejects_changed_signed_global_commitment_without_advancing() {
    let fixture = Fixture::new(4, None);
    let mut invalid = fixture.second.clone();
    invalid
        .finality_artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_hash = h("changed wire");
    let transport = ScriptedTransport::new(vec![
        Reply::capabilities(),
        Reply::finality(&fixture.first),
        Reply::finality(&invalid),
        Reply::finality(&fixture.second),
    ]);
    let client = fixture.client(transport.clone());
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.first.finality_artifact.context_id(),
    );
    client
        .get_bridge_finality_proof(
            NonZeroU64::new(1).unwrap(),
            fixture.genesis.hash(),
            &mut verifier,
        )
        .unwrap();
    assert!(
        client
            .get_bridge_finality_proof(
                NonZeroU64::new(2).unwrap(),
                fixture.carrier.hash(),
                &mut verifier
            )
            .is_err()
    );
    client
        .get_bridge_finality_proof(
            NonZeroU64::new(2).unwrap(),
            fixture.carrier.hash(),
            &mut verifier,
        )
        .unwrap();
    transport.assert_drained(4);
}

#[test]
fn public_sdk_rejects_query_selected_for_a_different_request() {
    let fixture = Fixture::new(4, None);
    let queries = fixture.queries();
    let mut wrong = Reply::details(queries[1].clone());
    wrong.expected_query_hash = Some(queries[0].entrypoint_hash);
    let transport = ScriptedTransport::new(vec![Reply::capabilities(), wrong]);
    let client = fixture.client(transport.clone());
    assert!(
        client
            .get_transaction_details(queries[0].entrypoint_hash)
            .is_err()
    );
    transport.assert_drained(2);
}

#[test]
fn committed_query_result_and_entrypoint_mutations_break_actual_merkle_proofs() {
    let fixture = Fixture::new(4, Some(7));
    let original = fixture.queries();
    for change in 0..4 {
        let mut query = original[0].clone();
        match change {
            0 => query.result = original.last().unwrap().result.clone(),
            1 => {
                query.result = original.last().unwrap().result.clone();
                query.result_hash = query.result.hash();
            }
            2 => query.entrypoint = original[1].entrypoint.clone(),
            3 => query.entrypoint_proof = original[1].entrypoint_proof.clone(),
            _ => unreachable!(),
        }
        assert!(
            !query.verify_inclusion_in_block(&fixture.carrier),
            "mutation {change}"
        );
    }
}

#[test]
fn real_reader_cannot_finish_with_missing_merge_or_carrier() {
    let fixture = Fixture::new(1, None);
    let mut missing_merge = fixture.open_reader();
    missing_merge.read_carrier(1).unwrap();
    missing_merge.read_carrier(2).unwrap();
    assert!(missing_merge.finish().is_err());
    let mut missing_carrier = fixture.open_reader();
    missing_carrier.read_carrier(1).unwrap();
    missing_carrier
        .scan_merge_entries(
            &[CanonicalKuraMergeRequest {
                carrier_height: 2,
                reference: CertifiedMergeLedgerReference::new(&fixture.entry),
            }],
            |_, _, _| Ok(()),
        )
        .unwrap();
    assert!(missing_carrier.finish().is_err());
}

#[test]
fn real_reader_rejects_omitted_merge_request_and_small_carrier_cap() {
    let fixture = Fixture::new(4, None);
    let mut reader = fixture.open_reader();
    assert!(reader.scan_merge_entries(&[], |_, _, _| Ok(())).is_err());
    assert!(reader.finish().is_err());
    let (blocks, merge) = fixture.paths();
    let mut limits = fixture.limits();
    limits.max_carrier_bytes = fixture.genesis.encode_wire().unwrap().len() - 1;
    let result = CanonicalKuraEvidenceReader::open(&blocks, &merge, limits);
    if let Ok(mut reader) = result {
        assert!(reader.read_carrier(1).is_err());
        assert!(reader.finish().is_err());
    }
}

#[test]
fn complete_capability_detects_original_archive_mutation() {
    use std::io::Write;
    let fixture = Fixture::new(1, None);
    let complete = fixture.complete();
    let (_, merge) = fixture.paths();
    std::fs::OpenOptions::new()
        .append(true)
        .open(merge)
        .unwrap()
        .write_all(&[0])
        .unwrap();
    assert!(complete.recheck_sources().is_err());
}

#[test]
fn public_transport_enforces_request_bound_and_at_most_one_dispatch() {
    let transport = ScriptedTransport::new(vec![Reply {
        method: Method::GET,
        path: "/cap".into(),
        media_type: "application/x-norito",
        expected_query_hash: None,
        body: vec![1, 2],
    }]);
    let request = || TransportRequest {
        method: Method::GET,
        url: "http://127.0.0.1:1/cap".parse().unwrap(),
        headers: Vec::new(),
        body: Vec::new(),
        timeout: None,
        max_response_bytes: 1,
        direct_loopback: true,
    };
    assert!(
        transport
            .send_blocking(request())
            .unwrap_err()
            .to_string()
            .contains("SDK response byte cap")
    );
    assert!(
        transport
            .send_blocking(request())
            .unwrap_err()
            .to_string()
            .contains("unexpected SDK request")
    );
    transport.assert_drained(1);
}

#[test]
fn wrong_executed_wire_is_validly_signed_but_differs_from_actual_archive() {
    let fixture = Fixture::new(4, None);
    let wrong = fixture.second_with_wrong_executed_wire();
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.first.finality_artifact.context_id(),
    );
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&wrong).unwrap();
    assert_eq!(wrong.block_header, fixture.second.block_header);
    let wire = fixture.carrier.encode_wire().unwrap();
    assert_ne!(
        wrong
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash,
        Hash::new(&wire)
    );
    assert_ne!(
        wrong
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_len,
        wire.len() as u64
    );
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn rewritten_lane_order_remains_publicly_stored_and_globally_signed() {
    let mut fixture = Fixture::new(4, None);
    let original_path = fixture.paths();
    fixture
        .rewrite_entry_for_test(|entry| entry.execution_batch.as_mut().unwrap().lanes.reverse())
        .unwrap();
    assert_ne!(fixture.paths(), original_path);
    assert_eq!(
        fixture.entry.execution_batch.as_ref().unwrap().lanes[0]
            .proposal
            .descriptor
            .lane_id,
        LaneId::new(3)
    );
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.first.finality_artifact.context_id(),
    );
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&fixture.second).unwrap();
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn offline_archive_permits_signed_duplicate_for_collector_negative_controls() {
    let mut fixture = Fixture::new(4, None);
    let before = fixture.paths();
    fixture
        .rewrite_entry_for_test(|entry| {
            let batch = entry.execution_batch.as_mut().unwrap();
            batch.lanes[1].entrypoints[0] = batch.lanes[0].entrypoints[0].clone();
        })
        .unwrap();
    assert_ne!(fixture.paths(), before);
    let batch = fixture.entry.execution_batch.as_ref().unwrap();
    assert_eq!(batch.lanes[0].entrypoints[0], batch.lanes[1].entrypoints[0]);
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.first.finality_artifact.context_id(),
    );
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&fixture.second).unwrap();
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn public_transport_checks_actual_signed_query_selection() {
    let fixture = Fixture::new(1, None);
    let query = fixture.queries().remove(0);
    let mut reply = Reply::details(query.clone());
    reply.expected_query_hash = Some(fixture.requests[1].signed.hash_as_entrypoint());
    let transport = ScriptedTransport::new(vec![Reply::capabilities(), reply]);
    let client = fixture.client(transport.clone());
    let error = client
        .get_transaction_details(query.entrypoint_hash)
        .unwrap_err();
    assert!(format!("{error:?}").contains("exact queried entrypoint hash"));
    assert_eq!(transport.consumed(), 2);
    transport.assert_drained(2);
}

#[test]
fn public_transport_async_path_has_same_bounded_single_dispatch() {
    let transport = ScriptedTransport::new(vec![Reply {
        method: Method::GET,
        path: "/async".into(),
        media_type: "application/x-norito",
        expected_query_hash: None,
        body: vec![7],
    }]);
    let request = TransportRequest {
        method: Method::GET,
        url: "http://127.0.0.1:1/async".parse().unwrap(),
        headers: Vec::new(),
        body: Vec::new(),
        timeout: None,
        max_response_bytes: 1,
        direct_loopback: true,
    };
    let response = futures::executor::block_on(transport.send(request)).unwrap();
    assert_eq!(response.body(), &[7]);
    assert_eq!(transport.consumed(), 1);
    transport.assert_drained(1);
}

#[test]
fn offline_archive_is_owner_only_and_uses_exact_public_merge_codec() {
    use norito::codec::Encode as _;
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new(4, Some(7));
    let (root, merge) = fixture.paths();
    assert_eq!(
        std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in [
        "blocks.data",
        "blocks.index",
        "blocks.hashes",
        "blocks.count.norito",
        "merge.log",
    ] {
        assert_eq!(
            std::fs::metadata(root.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let bytes = std::fs::read(merge).unwrap();
    let payload = fixture.entry.encode();
    assert_eq!(
        u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
        payload.len()
    );
    assert_eq!(&bytes[4..], payload.as_slice());
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn failed_bounded_rewrite_preserves_original_offline_archive() {
    let mut fixture = Fixture::new(1, None);
    let before = fixture.paths();
    let entry = fixture.entry.clone();
    assert!(
        fixture
            .rewrite_entry_for_test(|entry| entry.execution_batch = None)
            .is_err()
    );
    assert_eq!(fixture.paths(), before);
    assert_eq!(fixture.entry, entry);
    fixture.complete().recheck_sources().unwrap();
}

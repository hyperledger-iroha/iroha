//! Prepared public-API archive, crypto, SDK transport and committed-rejection controls.

use super::*;

#[test]
fn public_offline_block_writer_yields_complete_one_and_four_lane_evidence() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, None);
        assert_eq!(
            fixture.network_id,
            NetworkId::from_genesis_hash(fixture.heights[0].block.hash())
        );
        assert_eq!(fixture.lane_count, lanes);
        assert_eq!(fixture.requests.len(), 8);
        assert_eq!(fixture.requests.iter().filter(|r| r.warmup).count(), 4);
        for (index, request) in fixture.requests.iter().enumerate() {
            assert_eq!(request.logical_id, format!("{index:064x}"));
            assert_eq!(
                request.route,
                RoutingDecision::new(LaneId::new((index % lanes) as u32), DataSpaceId::UNIVERSAL)
            );
        }
        assert_eq!(fixture.heights.len(), 2 + 8 / lanes);
        for height in &fixture.heights[2..] {
            assert_eq!(height.block.network_entrypoint_count(), lanes);
        }
        let complete = fixture.complete();
        assert_eq!(complete.committed_height(), fixture.heights.len() as u64);
        assert_eq!(complete.carrier_count(), fixture.heights.len() as u64);
        assert_eq!(complete.merge_frames(), 0);
        assert!(complete.output_bytes() > 0);
        complete.recheck_sources().unwrap();
    }
}

#[test]
fn real_global_finality_covers_exact_canonical_wire_and_native_contexts() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, None);
        let mut verifier = BridgeFinalityVerifier::with_context(
            fixture.network_id,
            fixture.heights[0].proof.finality_artifact.context_id(),
        );
        let mut native = iroha_core::state::NativeExecutionEvidenceVerifier::new(
            fixture.network_id,
            fixture.heights[0].proof.finality_artifact.context_id(),
            iroha_core::state::NativeExecutionEvidenceLimits {
                max_carriers: fixture.heights.len() as u64,
                max_carrier_bytes: 1024 * 1024,
                max_proof_bytes: 16 * 1024 * 1024,
                max_retained_bytes: 16 * 1024 * 1024,
            },
        )
        .unwrap();
        for height in &fixture.heights {
            let (proof, block) = (&height.proof, &height.block);
            verifier.verify(proof).unwrap();
            proof
                .finality_artifact
                .validate_for_header(&block.header())
                .unwrap();
            let wire = block.encode_wire().unwrap();
            let execution = &proof.finality_artifact.commit_qc.execution_commitment;
            assert_eq!(execution.executed_block_wire_hash, Hash::new(&wire));
            assert_eq!(execution.executed_block_wire_len, wire.len() as u64);
            assert_eq!(
                execution.transaction_input_commitment,
                block.network_input_merkle_commitment()
            );
            assert_eq!(
                execution.transaction_output_commitment,
                block.output_merkle_commitment()
            );
            assert!(execution.merge_carrier.is_none());
            assert_eq!(proof.finality_artifact.height_context.roster.len(), 4);
            assert_eq!(proof.finality_artifact.commit_qc.signers, vec![0, 1, 2]);
            assert!(block.has_results());
            block.validate_proposal_commitments().unwrap();
            block.validate_output_merkle_cache().unwrap();
            native
                .push_height(proof, block.clone(), &height.evidence)
                .unwrap();
        }
    }
}

#[test]
fn rejected_committed_leaf_retains_valid_proofs_and_exact_offer_identity() {
    for lanes in [1, 4] {
        for rejected in [0, 7] {
            let fixture = Fixture::new(lanes, Some(rejected));
            let queries = fixture.queries();
            assert_eq!(queries.len(), 8);
            assert_eq!(queries.iter().filter(|q| q.result().0.is_err()).count(), 1);
            for query in &queries {
                let height = fixture
                    .heights
                    .iter()
                    .find(|height| height.block.hash() == query.block_hash)
                    .unwrap();
                assert!(query.verify_inclusion_in_block(&height.block));
                assert!(
                    query.verify_inclusion_in_authenticated_execution(
                        &height.block,
                        &height
                            .proof
                            .finality_artifact
                            .commit_qc
                            .execution_commitment
                    )
                );
                assert_eq!(
                    query.result().0.is_err(),
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
            let mut replies = vec![Reply::capabilities()];
            replies.extend(
                fixture
                    .heights
                    .iter()
                    .map(|height| Reply::finality(&height.proof)),
            );
            replies.extend(queries.iter().cloned().map(Reply::details));
            let transport = ScriptedTransport::new(replies);
            let client = fixture.client(transport.clone());
            let mut verifier = BridgeFinalityVerifier::with_context(
                fixture.network_id,
                fixture.heights[0].proof.finality_artifact.context_id(),
            );
            for height in &fixture.heights {
                let expected = &height.proof;
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
                let height = fixture
                    .heights
                    .iter()
                    .find(|height| height.block.hash() == expected.block_hash)
                    .unwrap();
                assert!(details.transaction.verify_inclusion_in_block(&height.block));
                rejected_count += usize::from(details.transaction.result().0.is_err());
            }
            assert_eq!(rejected_count, usize::from(rejected.is_some()));
            transport.assert_drained(1 + fixture.heights.len() + 8);
            fixture.complete().recheck_sources().unwrap();
        }
    }
}

#[test]
fn public_sdk_rejects_changed_signed_global_commitment_without_advancing() {
    let fixture = Fixture::new(4, None);
    let mut invalid = fixture.heights[1].proof.clone();
    invalid
        .finality_artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_hash = h("changed wire");
    let transport = ScriptedTransport::new(vec![
        Reply::capabilities(),
        Reply::finality(&fixture.heights[0].proof),
        Reply::finality(&invalid),
        Reply::finality(&fixture.heights[1].proof),
    ]);
    let client = fixture.client(transport.clone());
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.heights[0].proof.finality_artifact.context_id(),
    );
    client
        .get_bridge_finality_proof(
            NonZeroU64::new(1).unwrap(),
            fixture.heights[0].block.hash(),
            &mut verifier,
        )
        .unwrap();
    assert!(
        client
            .get_bridge_finality_proof(
                NonZeroU64::new(2).unwrap(),
                fixture.heights[1].block.hash(),
                &mut verifier
            )
            .is_err()
    );
    client
        .get_bridge_finality_proof(
            NonZeroU64::new(2).unwrap(),
            fixture.heights[1].block.hash(),
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
fn committed_query_output_and_entrypoint_mutations_break_actual_merkle_proofs() {
    let fixture = Fixture::new(4, Some(7));
    let original = fixture.queries();
    let successful = original
        .iter()
        .find(|query| query.result().0.is_ok())
        .unwrap();
    let rejected = original
        .iter()
        .find(|query| query.result().0.is_err())
        .unwrap();
    let block = &fixture
        .heights
        .iter()
        .find(|height| height.block.hash() == successful.block_hash)
        .unwrap()
        .block;
    let different = original
        .iter()
        .find(|query| {
            query.block_hash == successful.block_hash
                && query.entrypoint_hash != successful.entrypoint_hash
        })
        .unwrap();
    for change in 0..5 {
        let mut query = successful.clone();
        match change {
            0 => query.output = rejected.output.clone(),
            1 => {
                query.output = rejected.output.clone();
                query.output_hash = HashOf::new(&query.output);
            }
            2 => query.entrypoint = different.entrypoint.clone(),
            3 => query.entrypoint_proof = different.entrypoint_proof.clone(),
            4 => query.output_proof = different.output_proof.clone(),
            _ => unreachable!(),
        }
        assert!(!query.verify_inclusion_in_block(block), "mutation {change}");
    }
}

#[test]
fn real_reader_cannot_finish_with_missing_merge_scan_or_carrier() {
    let fixture = Fixture::new(1, None);
    let mut missing_scan = fixture.open_reader();
    for height in &fixture.heights {
        missing_scan
            .read_carrier(height.block.header().height().get())
            .unwrap();
    }
    assert!(missing_scan.finish().is_err());
    let mut missing_carrier = fixture.open_reader();
    missing_carrier.read_carrier(1).unwrap();
    missing_carrier
        .scan_merge_entries(&[], |_, _, _| Ok(()))
        .unwrap();
    assert!(missing_carrier.finish().is_err());
}

#[test]
fn real_reader_rejects_nonempty_native_merge_log_and_small_carrier_cap() {
    let fixture = Fixture::new(4, None);
    let (blocks, merge) = fixture.paths();
    std::fs::write(&merge, [0]).unwrap();
    let mut reader = fixture.open_reader();
    assert!(reader.scan_merge_entries(&[], |_, _, _| Ok(())).is_err());
    assert!(reader.finish().is_err());
    let mut limits = fixture.limits();
    limits.max_carrier_bytes = fixture.heights[0].block.encode_wire().unwrap().len() - 1;
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
    let wrong = fixture.height_with_wrong_executed_wire(1);
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.heights[0].proof.finality_artifact.context_id(),
    );
    verifier.verify(&fixture.heights[0].proof).unwrap();
    verifier.verify(&wrong).unwrap();
    assert_eq!(wrong.block_header, fixture.heights[1].proof.block_header);
    let wire = fixture.heights[1].block.encode_wire().unwrap();
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
fn rewritten_native_order_remains_publicly_stored_and_globally_signed() {
    let mut fixture = Fixture::new(4, None);
    let original_path = fixture.paths();
    let original_first = fixture.heights[2]
        .block
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_ref()
        .unwrap()
        .groups[0]
        .clone();
    fixture
        .rewrite_native_for_test(2, |batch| batch.groups.reverse())
        .unwrap();
    assert_ne!(fixture.paths(), original_path);
    let batch = fixture.heights[2]
        .block
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_ref()
        .unwrap();
    assert_eq!(batch.groups.last().unwrap(), &original_first);
    let carrier = &fixture.heights[2];
    let execution = &carrier.proof.finality_artifact.commit_qc.execution_commitment;
    assert_eq!(
        execution.transaction_input_commitment,
        carrier.block.network_input_merkle_commitment()
    );
    assert_eq!(
        execution.transaction_output_commitment,
        carrier.block.output_merkle_commitment()
    );
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.heights[0].proof.finality_artifact.context_id(),
    );
    for height in &fixture.heights {
        verifier.verify(&height.proof).unwrap();
    }
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn offline_archive_permits_signed_duplicate_for_collector_negative_controls() {
    let mut fixture = Fixture::new(4, None);
    let before = fixture.paths();
    fixture
        .rewrite_native_for_test(2, |batch| {
            batch.groups[1].payload = batch.groups[0].payload.clone()
        })
        .unwrap();
    assert_ne!(fixture.paths(), before);
    let batch = fixture.heights[2]
        .block
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_ref()
        .unwrap();
    assert_eq!(batch.groups[0].payload, batch.groups[1].payload);
    let carrier = &fixture.heights[2];
    let execution = &carrier.proof.finality_artifact.commit_qc.execution_commitment;
    assert_eq!(
        execution.transaction_input_commitment,
        carrier.block.network_input_merkle_commitment()
    );
    assert_eq!(
        execution.transaction_output_commitment,
        carrier.block.output_merkle_commitment()
    );
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.network_id,
        fixture.heights[0].proof.finality_artifact.context_id(),
    );
    for height in &fixture.heights {
        verifier.verify(&height.proof).unwrap();
    }
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn public_transport_checks_actual_signed_query_selection() {
    let fixture = Fixture::new(1, None);
    let queries = fixture.queries();
    let query = &queries[0];
    let mut reply = Reply::details(query.clone());
    reply.expected_query_hash = Some(queries[1].entrypoint_hash);
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
fn offline_archive_is_owner_only_and_has_no_executable_merge_entries() {
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
    assert!(bytes.is_empty());
    fixture.complete().recheck_sources().unwrap();
}

#[test]
fn failed_bounded_rewrite_preserves_original_offline_archive() {
    let mut fixture = Fixture::new(1, None);
    let before = fixture.paths();
    let block = fixture.heights[2].block.clone();
    assert!(
        fixture
            .rewrite_native_for_test(2, |batch| batch.groups.clear())
            .is_err()
    );
    assert_eq!(fixture.paths(), before);
    assert_eq!(fixture.heights[2].block, block);
    fixture.complete().recheck_sources().unwrap();
}

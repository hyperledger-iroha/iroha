fn canonical_executed_block_fixture() -> (NonZeroU64, SignedBlock, CommittedTransaction) {
    use crate::crypto::{PrivateKey, PublicKey};
    use iroha_data_model::block::builder::BlockBuilder;
    let public_key: PublicKey =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("fixture public key");
    let private_key: PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .expect("fixture private key");
    let transaction = TransactionBuilder::new(
        test_network_id(),
        AccountId::new(public_key),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .try_sign(&private_key)
    .expect("sign executed-block fixture transaction");
    let height = NonZeroU64::new(1).expect("non-zero fixture height");
    let header = BlockHeader::new(height, None, None, None, 0, 0);
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction);
    builder.push_result(Ok(
        iroha_data_model::transaction::DataTriggerSequence::default(),
    ));
    let block = builder
        .try_build_with_signature(0, &private_key)
        .expect("sign canonical result-bearing block");
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: block.entrypoint_hashes().next().expect("entrypoint hash"),
        entrypoint_proof: block.entrypoint_proofs().next().expect("entrypoint proof"),
        entrypoint: block.entrypoints_cloned().next().expect("entrypoint"),
        result_hash: block.result_hashes().next().expect("result hash"),
        result_proof: block.result_proofs().next().expect("result proof"),
        result: block.results().next().cloned().expect("result"),
        merge_inclusion: None,
    };
    assert!(committed.verify_inclusion_in_block(&block));
    (height, block, committed)
}

#[test]
fn canonical_executed_block_reader_binds_route_wire_and_committed_evidence() {
    let mut client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    let (height, block, committed) = canonical_executed_block_fixture();
    let wire = block.encode_wire().expect("canonical executed block wire");
    let (actual, snapshot) = capture_request(
        mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
        || client.get_canonical_executed_block_wire(height, &committed),
    );
    assert_eq!(actual.expect("verified executed block wire"), wire);
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/ledger/block/1");
    assert!(snapshot.url.query().is_none());
    assert_eq!(
        snapshot.max_response_bytes,
        AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type")),
        "GET must not carry Content-Type: {:?}",
        snapshot.headers
    );
}

#[test]
fn canonical_executed_block_reader_rejects_trailing_wire_and_wrong_carrier_hash() {
    let client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    let (height, block, committed) = canonical_executed_block_fixture();
    let mut trailing = block.encode_wire().expect("canonical executed block wire");
    trailing.push(0);
    let error = capture_request(
        mk_response(StatusCode::OK, trailing, Some(APPLICATION_NORITO)),
        || client.get_canonical_executed_block_wire(height, &committed),
    )
    .0
    .expect_err("trailing executed-block bytes must fail");
    assert!(
        error
            .to_string()
            .contains("decode canonical executed block wire")
            || error
                .to_string()
                .contains("exact canonical SignedBlock wire")
    );

    let mut wrong_carrier = committed;
    wrong_carrier.block_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0x91; Hash::LENGTH]));
    let wire = block.encode_wire().expect("canonical executed block wire");
    let error = capture_request(
        mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)),
        || client.get_canonical_executed_block_wire(height, &wrong_carrier),
    )
    .0
    .expect_err("wrong carrier hash must fail");
    assert!(error.to_string().contains("carrier hash"));
}

fn sign_bridge_finality_qc(commit_qc: &mut QuorumCertificate, keys: &[KeyPair]) {
    let preimage = Vote {
        round: commit_qc.round,
        proposal_round: commit_qc.proposal_round,
        phase: commit_qc.phase,
        subject: commit_qc.subject,
        execution_commitment: commit_qc.execution_commitment,
        signer: commit_qc.signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let signature_payloads = commit_qc
        .signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("fixture signer index");
            Signature::try_new(keys[index].private_key(), &preimage)
                .expect("sign finality fixture vote")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signature_payloads
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate finality fixture votes");
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture builds a self-contained cryptographically valid v2 proof chain"
)]
fn bridge_finality_chain_fixture() -> (
    BridgeFinalityProof,
    BridgeFinalityProof,
    BridgeFinalityVerifier,
) {
    let mut keys = (0..4)
        .map(|_| {
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("generate BLS finality fixture key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        iroha_data_model::peer::PeerId::new(left.public_key().clone()).cmp(
            &iroha_data_model::peer::PeerId::new(right.public_key().clone()),
        )
    });
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: iroha_data_model::peer::PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let proofs_of_possession = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("derive finality fixture proof of possession")
        })
        .collect::<Vec<_>>();
    let height = NonZeroU64::new(1).expect("non-zero finality height");
    let header = BlockHeader::new(height, None, None, None, 0, 0);
    let context = HeightContext {
        network_id: test_network_id(),
        protocol_version: PROTOCOL_VERSION,
        height: height.get(),
        epoch: 0,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid finality fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"client finality fixture nexus context"),
        execution_policy_hash: Hash::new(b"client finality fixture execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x5A; Hash::LENGTH],
    };
    let context_id = context.id();
    let subject = BlockSubject {
        parent_block_hash: None,
        block_hash: header.hash(),
        payload_hash: Hash::new(b"client finality fixture payload"),
    };
    let round = ConsensusRound {
        context_id,
        height: height.get(),
        view: 0,
    };
    let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"client finality fixture parent state"),
        Hash::new(b"client finality fixture post state"),
        Hash::new(b"client finality fixture ordinary writes"),
        1,
        Hash::new(b"client finality fixture executed wire"),
    );
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    sign_bridge_finality_qc(&mut commit_qc, &keys);
    let finality_artifact =
        iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact::new(
            context,
            subject,
            commit_qc,
            proofs_of_possession,
        );
    let proof = BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: header,
        finality_artifact,
    };
    let parent_artifact = &proof.finality_artifact;
    let successor_height = NonZeroU64::new(height.get() + 1).expect("non-zero successor height");
    let successor_header = BlockHeader::new(
        successor_height,
        Some(parent_artifact.block_hash),
        None,
        None,
        1,
        0,
    );
    let successor_context = HeightContext {
        network_id: parent_artifact.height_context.network_id,
        protocol_version: PROTOCOL_VERSION,
        height: successor_height.get(),
        epoch: parent_artifact.height_context.epoch,
        epoch_end_height: parent_artifact.height_context.epoch_end_height,
        next_epoch_snapshot: None,
        mode: parent_artifact.height_context.mode,
        parent_commit_qc: Some(parent_artifact.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: parent_artifact.height_context.quorum,
        roster: parent_artifact.height_context.roster.clone(),
        nexus_amx_context_hash: Hash::new(b"client finality successor nexus context"),
        execution_policy_hash: parent_artifact.height_context.execution_policy_hash,
        da_layout: parent_artifact.height_context.da_layout,
        leader_seed: parent_artifact.height_context.leader_seed,
    };
    let successor_subject = BlockSubject {
        parent_block_hash: Some(parent_artifact.block_hash),
        block_hash: successor_header.hash(),
        payload_hash: Hash::new(b"client finality successor payload"),
    };
    let successor_round = ConsensusRound {
        context_id: successor_context.id(),
        height: successor_height.get(),
        view: 0,
    };
    let successor_execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"client finality successor parent state"),
        Hash::new(b"client finality successor post state"),
        Hash::new(b"client finality successor ordinary writes"),
        1,
        Hash::new(b"client finality successor executed wire"),
    );
    let mut successor_commit_qc = QuorumCertificate {
        round: successor_round,
        proposal_round: successor_round,
        phase: GlobalPhase::Commit,
        subject: successor_subject,
        execution_commitment: successor_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    sign_bridge_finality_qc(&mut successor_commit_qc, &keys);
    let successor_artifact =
        iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact::new(
            successor_context,
            successor_subject,
            successor_commit_qc,
            parent_artifact.validator_set_pops.clone(),
        );
    let successor = BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: successor_header,
        finality_artifact: successor_artifact,
    };
    let verifier = BridgeFinalityVerifier::with_context(test_network_id(), context_id);
    (proof, successor, verifier)
}

fn rejected_next_bridge_finality_response(
    client: &Client,
    height: NonZeroU64,
    verifier: &mut BridgeFinalityVerifier,
    response: HttpResponse<Vec<u8>>,
) -> String {
    capture_request(response, || {
        client.get_next_bridge_finality_proof(height, verifier)
    })
    .0
    .expect_err("bridge finality response must fail")
    .to_string()
}

#[test]
fn bridge_finality_anchor_reader_returns_standalone_verified_proof_and_hash() {
    let mut client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    let (proof, _, _) = bridge_finality_chain_fixture();
    let expected_hash = proof.block_header.hash();
    let body = norito::to_bytes(&proof).expect("encode canonical bridge finality proof");
    let (actual, snapshot) = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        || client.get_bridge_finality_anchor(proof.block_header.height(), test_network_id()),
    );
    let (actual_proof, actual_hash) = actual.expect("standalone finality proof must verify");
    assert_eq!(actual_proof, proof);
    assert_eq!(actual_hash, expected_hash);
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/1");
    assert!(snapshot.url.query().is_none());
    assert_eq!(
        snapshot.max_response_bytes,
        BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type")),
        "GET must not carry Content-Type: {:?}",
        snapshot.headers
    );
}

#[test]
fn bridge_finality_anchor_reader_rejects_wrong_network_and_invalid_signature() {
    let client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    let (proof, _, _) = bridge_finality_chain_fixture();
    let body = norito::to_bytes(&proof).expect("encode canonical bridge finality proof");
    let mismatched_height =
        NonZeroU64::new(proof.block_header.height().get() + 1).expect("non-zero mismatched height");
    let (error, snapshot) = capture_request(
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_NORITO)),
        || client.get_bridge_finality_anchor(mismatched_height, test_network_id()),
    );
    assert!(
        error
            .expect_err("wrong requested anchor height must fail")
            .to_string()
            .contains("requested height")
    );
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/2");

    let wrong_network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"wrong client finality network",
    )));
    let error = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        || client.get_bridge_finality_anchor(proof.block_header.height(), wrong_network_id),
    )
    .0
    .expect_err("wrong finality network must fail");
    assert!(error.to_string().contains("network"));

    let mut invalid = proof.clone();
    invalid.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x80;
    let body = norito::to_bytes(&invalid).expect("encode invalid bridge finality proof");
    let error = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        || client.get_bridge_finality_anchor(proof.block_header.height(), test_network_id()),
    )
    .0
    .expect_err("invalid finality signature must fail");
    assert!(error.to_string().contains("verification failed"));
}

#[test]
fn bridge_finality_reader_checks_requested_binding_before_advancing_anchor() {
    let mut client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    let (proof, _, mut verifier) = bridge_finality_chain_fixture();
    let body = norito::to_bytes(&proof).expect("encode canonical bridge finality proof");
    let wrong_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0x92; Hash::LENGTH]));
    let (error, snapshot) = capture_request(
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_NORITO)),
        || {
            client
                .get_bridge_finality_proof(proof.block_header.height(), wrong_hash, &mut verifier)
                .expect_err("wrong requested hash must fail")
        },
    );
    assert!(error.to_string().contains("requested block hash"));
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/1");
    assert!(snapshot.url.query().is_none());
    assert_eq!(
        snapshot.max_response_bytes,
        BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type")),
        "GET must not carry Content-Type: {:?}",
        snapshot.headers
    );

    let expected_hash = proof.block_header.hash();
    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        || {
            client.get_bridge_finality_proof(
                proof.block_header.height(),
                expected_hash,
                &mut verifier,
            )
        },
    )
    .0
    .expect("correct proof must verify after a rejected requested binding");
    assert_eq!(actual, proof);
}

#[test]
fn bridge_finality_next_reader_rejects_height_mismatch_before_advancing() {
    let client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier
        .verify(&anchor)
        .expect("fixture anchor must initialize verifier progress");
    let successor_height = successor.block_header.height();
    let anchor_body = norito::to_bytes(&anchor).expect("encode canonical anchor proof");
    let (error, snapshot) = capture_request(
        mk_response(StatusCode::OK, anchor_body, Some(APPLICATION_NORITO)),
        || {
            client
                .get_next_bridge_finality_proof(successor_height, &mut verifier)
                .expect_err("proof from the wrong height must fail")
        },
    );
    assert!(error.to_string().contains("requested height"));
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/2");
    assert!(snapshot.url.query().is_none());

    let successor_body = norito::to_bytes(&successor).expect("encode canonical successor proof");
    let actual = capture_request(
        mk_response(StatusCode::OK, successor_body, Some(APPLICATION_NORITO)),
        || client.get_next_bridge_finality_proof(successor_height, &mut verifier),
    )
    .0
    .expect("valid successor must verify after a height mismatch");
    assert_eq!(actual, successor);
}

#[test]
fn bridge_finality_next_reader_response_contract_failures_do_not_advance() {
    let client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier
        .verify(&anchor)
        .expect("fixture anchor must initialize verifier progress");
    let height = successor.block_header.height();
    let body = norito::to_bytes(&successor).expect("encode canonical successor proof");

    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(
            StatusCode::BAD_GATEWAY,
            b"upstream failure".to_vec(),
            Some(APPLICATION_NORITO),
        ),
    );
    assert!(error.contains("Failed to get bridge finality proof"));

    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_JSON)),
    );
    assert!(error.contains("invalid content-type"));

    let mut duplicate_content_type =
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_NORITO));
    duplicate_content_type.headers_mut().append(
        "content-type",
        APPLICATION_NORITO.parse().expect("Norito media type"),
    );
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        duplicate_content_type,
    );
    assert!(error.contains("multiple Content-Type"));

    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(
            StatusCode::OK,
            vec![0; BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES + 1],
            Some(APPLICATION_NORITO),
        ),
    );
    assert!(error.contains("response exceeds"));

    let mut trailing = body.clone();
    trailing.push(0);
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, trailing, Some(APPLICATION_NORITO)),
    );
    assert!(error.contains("canonical Norito"));

    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        || client.get_next_bridge_finality_proof(height, &mut verifier),
    )
    .0
    .expect("valid successor must verify after rejected responses");
    assert_eq!(actual, successor);
}

#[test]
fn bridge_finality_next_reader_verification_failure_does_not_advance() {
    let client = client_with_base_url(base_url());
    mark_data_model_compatible(&client);
    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier
        .verify(&anchor)
        .expect("fixture anchor must initialize verifier progress");
    let height = successor.block_header.height();
    let mut invalid = successor.clone();
    invalid.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x40;
    let invalid_body = norito::to_bytes(&invalid).expect("encode invalid successor finality proof");
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, invalid_body, Some(APPLICATION_NORITO)),
    );
    assert!(error.contains("verification failed"));

    let body = norito::to_bytes(&successor).expect("encode canonical successor proof");
    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        || client.get_next_bridge_finality_proof(height, &mut verifier),
    )
    .0
    .expect("valid successor must verify after a rejected invalid signature");
    assert_eq!(actual, successor);
}

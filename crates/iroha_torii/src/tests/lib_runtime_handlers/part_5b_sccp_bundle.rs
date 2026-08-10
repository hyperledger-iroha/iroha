fn app_with_indexed_sccp_message_for_test(
    persist_finality: bool,
) -> (SharedAppState, [u8; 32], V2FinalityArtifact) {
    const HEIGHT: u64 = 1;
    let keypair =
        checked_torii_test_ed25519_keypair(0x31, "derive indexed Torii SCCP-message fixture key");
    let chain: ChainId = iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1
        .parse()
        .expect("SCCP Taira chain label");
    let app = mk_app_state_for_tests_with_chain_id(chain.clone());
    let authority = AccountId::new(keypair.public_key().clone());
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce: 7,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: iroha_sccp::SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
        amount: 123,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"alice".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x91; 20],
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
            .as_bytes()
            .to_vec(),
    });
    let context = iroha_data_model::bridge::SccpOutboundMessageContextV1::new(
        iroha_data_model::bridge::SccpLaneIdV1 {
            source: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
            target: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
        },
        [0xd1; 32],
        [0xc1; 32],
    )
    .expect("well-formed SCCP context");
    let record = iroha_data_model::isi::bridge::RecordSccpMessage::new(
        context,
        iroha_sccp::canonical_sccp_payload_bytes(&payload)
            .expect("valid SCCP indexed-message fixture payload encodes"),
    );
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            *app.state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([record]),
        &keypair,
        "sign indexed Torii SCCP-message fixture transaction",
    );
    let entry_hash = tx.hash_as_entrypoint();
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(HEIGHT).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let signature = checked_torii_test_block_signature(
        0,
        &keypair,
        &header,
        "sign indexed Torii SCCP-message fixture block",
    );
    let mut block = SignedBlock::presigned(signature, header, vec![tx]);
    block
        .set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("test block entrypoint hash should match payload");
    let legacy_post_state_root = block
        .header()
        .result_merkle_root()
        .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
        .expect("SCCP fixture result root");
    let messages = iroha_core::bridge::collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    let commitment_root = iroha_core::bridge::sccp_commitment_root_from_messages(&messages)
        .expect("SCCP commitment root");
    block.set_sccp_commitment_root(Some(commitment_root));
    let block_hash = block.hash();
    let message_id = message.commitment.message_id;
    let key = iroha_data_model::bridge::SccpOutboundMessageKeyV1::new(context.lane, message_id)
        .expect("valid outbound key");
    let durable = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
        destination_binding_hash: context.destination_binding_hash,
        route_configuration_hash: context.route_configuration_hash,
        payload_hash: message.commitment.payload_hash,
        payload_bytes: iroha_sccp::canonical_sccp_payload_bytes(&message.payload)
            .expect("canonical indexed Torii SCCP-message payload"),
        recorded_at_height: HEIGHT,
        commitment_index: 0,
    };
    app.state
        .insert_sccp_outbound_message_for_testing(key, durable)
        .expect("insert indexed outbound record");
    let mut validator_keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive deterministic SCCP finality validator")
        })
        .collect::<Vec<_>>();
    validator_keys.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = validator_keys
        .iter()
        .zip([1_u64; 4])
        .map(|(key, power)| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();
    let context = HeightContext {
        network_id: *app.state.network_id_ref(),
        protocol_version: PROTOCOL_VERSION,
        height: HEIGHT,
        epoch: 0,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid SCCP finality roster"),
        roster,
        nexus_amx_context_hash: Hash::new(b"Torii SCCP exact-v2 finality context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x42; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash,
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("hash exact SCCP fixture proposal wire"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: HEIGHT,
        view: block.header().view_change_index(),
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"Torii SCCP exact-v2 parent state"),
            Hash::new(b"Torii SCCP exact-v2 post state"),
            Hash::new(b"Torii SCCP exact-v2 ordinary writes"),
            u64::try_from(block.encode_wire().expect("exact block wire").len())
                .expect("exact block wire length fits u64"),
            block
                .executed_block_wire_hash()
                .expect("hash exact SCCP fixture block wire"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid SCCP finality signer");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                validator_keys[usize::try_from(*index).expect("fixture signer index")]
                    .private_key(),
                &preimage,
            )
            .expect("sign exact SCCP Commit vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate exact SCCP Commit votes");
    let validator_set_pops = validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("derive SCCP finality validator PoP")
        })
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    artifact
        .validate_for_header(&block.header())
        .expect("SCCP finality fixture binds the exact block header");
    artifact
        .verify()
        .expect("SCCP finality fixture is cryptographically valid");

    // Seed the retired QC model as an adversarial control. Proof routes
    // must still require the exact durable v2 artifact below; a valid
    // legacy QC in world state is not an alternative finality source.
    let (legacy_qc, legacy_validator_pop) = sample_commit_qc(
        app.state.network_id_ref(),
        block_hash,
        legacy_post_state_root,
        HEIGHT,
        HEIGHT.saturating_add(1),
        0,
    );

    let stored_block_hash = store_block(&app, block);
    assert_eq!(stored_block_hash, artifact.block_hash);
    if persist_finality {
        let receipt = app
            .kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist exact SCCP v2 finality artifact");
        assert_eq!(receipt.height(), artifact.height);
        assert_eq!(receipt.block_hash(), artifact.block_hash);
        assert_eq!(receipt.context_id(), artifact.context_id());
        assert_eq!(receipt.subject(), artifact.subject);
        assert_eq!(receipt.certificate(), artifact.commit_qc.as_ref());
        assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
    }
    let mut app = app;
    let app_mut = Arc::get_mut(&mut app).expect("unique app state for SCCP fixture");
    let state = Arc::get_mut(&mut app_mut.state).expect("unique core state for SCCP fixture");
    state.world.register_validator_pop_for_testing(
        legacy_qc.validator_set[0].public_key().clone(),
        legacy_validator_pop,
    );
    state.insert_commit_qc_for_testing(block_hash, legacy_qc);
    assert!(
        state.world_view().commit_qcs().get(&block_hash).is_some(),
        "SCCP adversarial fixture retains a valid legacy QC"
    );
    (app, message_id, artifact)
}

#[tokio::test]
async fn sccp_bundle_endpoint_uses_exact_v2_artifact_and_authoritative_index() {
    let (app, message_id, expected_artifact) = app_with_indexed_sccp_message_for_test(true);
    let message_id_hex = hex::encode(message_id);
    let bundle_response = routing::handle_v1_sccp_message_bundle(
        Arc::clone(&app.state),
        message_id_hex.clone(),
        utils::ResponseFormat::Json,
        acquire_query_admission(app.as_ref(), true)
            .await
            .expect("acquire bundle test admission"),
    )
    .await
    .expect("indexed bundle response");
    let bundle_bytes = axum::body::to_bytes(bundle_response.into_body(), usize::MAX)
        .await
        .expect("bundle body");
    let bundle = norito::json::from_slice::<iroha_sccp::TairaSccpMessageProofV1>(&bundle_bytes)
        .expect("typed bundle JSON");
    assert_eq!(bundle.commitment.message_id, message_id);
    assert!(iroha_sccp::verify_message_bundle_structure(&bundle));
    let verified_finality =
        iroha_sccp::verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(
            &bundle,
        )
        .expect("bundle carries a cryptographically self-consistent exact-v2 proof");
    assert_eq!(verified_finality.finality_artifact, expected_artifact);

    let request_error = routing::handle_v1_sccp_proof_request(
        Arc::clone(&app.state),
        message_id_hex,
        utils::ResponseFormat::Json,
        acquire_query_admission(app.as_ref(), true)
            .await
            .expect("acquire proof-request test admission"),
    )
    .await
    .expect_err("proof request must require its historical governed route");
    let Error::Query(ValidationFail::InternalError(message)) = request_error else {
        panic!("unexpected missing-route error: {request_error}");
    };
    assert!(message.contains("retained destination binding"));
}

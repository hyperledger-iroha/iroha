// Mock HTTP attestation tests reuse the native, cryptographically signed proof fixture.

fn client_attestation_fixture() -> iroha_data_model::bridge::BridgeFinalityAttestationV1 {
    use iroha_data_model::{
        block::consensus_v2::{
            SumeragiV2BodyState, SumeragiV2CommitQcStatus, SumeragiV2HeightContextStatus,
            SumeragiV2LivenessStatus, SumeragiV2Status, SumeragiV2StatusPhase,
        },
        bridge::{
            BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BridgeFinalityAttestationBodyV1,
            BridgeFinalityAttestationV1,
        },
    };
    let (proof, _, _) = bridge_finality_chain_fixture();
    let artifact = &proof.finality_artifact;
    let context = &artifact.height_context;
    let signer = KeyPair::try_from_seed(vec![93; 32], Algorithm::Ed25519).expect("reporter key");
    let node_id = iroha_model_base::peer::PeerId::new(signer.public_key().clone());
    let node_fingerprint = Hash::new(norito::codec::Encode::encode(&node_id));
    let signed_power = artifact
        .commit_qc
        .signers
        .iter()
        .map(|index| context.roster[usize::try_from(*index).expect("signer index")].power)
        .sum();
    let status = SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint,
        build_fingerprint: Hash::new(b"attestation client build"),
        config_fingerprint: Hash::new(b"attestation client config"),
        restart_required: false,
        height_context_id: context.id(),
        height: artifact.height,
        view: artifact.commit_qc.round.view,
        phase: SumeragiV2StatusPhase::PendingApply,
        leader: context.leader(artifact.commit_qc.round.view),
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Applied,
        pending_persistence_id: None,
        last_committed_height: artifact.height,
        last_committed_subject: Some(artifact.subject),
        height_context: SumeragiV2HeightContextStatus {
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            mode: context.mode,
            epoch_seed: context.leader_seed,
            validator_count: u32::try_from(context.roster.len()).expect("validator count"),
            quorum: context.quorum,
        },
        last_commit_qc: Some(SumeragiV2CommitQcStatus {
            certificate: artifact.commit_qc.as_ref(),
            validator_count: u32::try_from(context.roster.len()).expect("validator count"),
            signer_count: u32::try_from(artifact.commit_qc.signers.len()).expect("signer count"),
            min_signers: context.quorum.min_signers,
            signed_power,
            total_power: context.quorum.total_power,
        }),
        liveness: SumeragiV2LivenessStatus::default(),
    };
    let body = BridgeFinalityAttestationBodyV1 {
        version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
        challenge: [17; 32],
        network_id: context.network_id,
        node_id,
        node_fingerprint,
        genesis_block_hash: proof.block_header.hash(),
        genesis_finality_proof: proof.clone(),
        status,
        finality_proof: proof,
    };
    let signature =
        iroha_crypto::SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
            .expect("sign attestation");
    let value = BridgeFinalityAttestationV1 { body, signature };
    value.verify().expect("valid native attestation fixture");
    value
}

#[test]
fn bridge_finality_attestation_reader_binds_exact_request_headers_and_signed_body() {
    let attestation = client_attestation_fixture();
    let mut client = client_with_base_url(base_url());
    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("x-iroha-finality-challenge".to_owned(), "stale".to_owned());
    let response = mk_response(
        StatusCode::OK,
        norito::to_bytes(&attestation).expect("wire"),
        Some(APPLICATION_NORITO),
    );
    let (actual, request) = capture_request(response, |transport| {
        let client = client.with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        client.get_bridge_finality_attestation(
            NonZeroU64::new(1).unwrap(),
            attestation.body.challenge,
            &attestation.body.node_id,
        )
    });
    assert_eq!(actual.expect("attestation"), attestation);
    assert_eq!(request.method, HttpMethod::GET);
    assert_eq!(request.url.path(), "/v1/bridge/finality/attestation/1");
    assert!(request.url.query().is_none());
    assert!(request.body.is_empty());
    assert_eq!(
        request.max_response_bytes,
        BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&request, APPLICATION_NORITO);
    let challenges = request
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("x-iroha-finality-challenge"))
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>();
    assert_eq!(challenges, vec![hex::encode(attestation.body.challenge)]);
    assert!(
        request
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type"))
    );
}

#[test]
fn bridge_finality_attestation_reader_rejects_wrong_bindings_and_invalid_http_body() {
    let attestation = client_attestation_fixture();
    let client = client_with_base_url(base_url());
    let wire = norito::to_bytes(&attestation).expect("wire");
    let (zero, requests) = capture_requests(
        mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
        |transport| {
            client
                .clone()
                .with_test_http_transport(transport)
                .get_bridge_finality_attestation(
                    NonZeroU64::new(1).unwrap(),
                    [0; 32],
                    &attestation.body.node_id,
                )
        },
    );
    assert!(zero.is_err());
    assert!(
        requests.is_empty(),
        "zero challenge must fail before compatibility or HTTP"
    );
    let wrong_key =
        KeyPair::try_from_seed(vec![94; 32], Algorithm::Ed25519).expect("other reporter");
    let wrong_node = iroha_model_base::peer::PeerId::new(wrong_key.public_key().clone());
    for (height, challenge, node) in [
        (
            2,
            attestation.body.challenge,
            attestation.body.node_id.clone(),
        ),
        (1, [18; 32], attestation.body.node_id.clone()),
        (1, attestation.body.challenge, wrong_node),
    ] {
        let (result, _) = capture_request(
            mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
            |transport| {
                let client = client.clone().with_test_http_transport(transport);
                mark_data_model_compatible(&client);
                client.get_bridge_finality_attestation(
                    NonZeroU64::new(height).unwrap(),
                    challenge,
                    &node,
                )
            },
        );
        assert!(result.is_err(), "wrong request binding must fail");
    }
    let mut altered = attestation.clone();
    altered.body.status.build_fingerprint = Hash::new(b"tampered unsigned status");
    let mut trailing = wire.clone();
    trailing.push(0);
    let mut wrong_network_client = client.clone();
    wrong_network_client.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::new(b"foreign network"),
    ));
    let (result, _) = capture_request(
        mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
        |transport| {
            let client = wrong_network_client.with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_bridge_finality_attestation(
                NonZeroU64::new(1).unwrap(),
                attestation.body.challenge,
                &attestation.body.node_id,
            )
        },
    );
    assert!(result.is_err());
    for response in [
        mk_response(
            StatusCode::NOT_FOUND,
            wire.clone(),
            Some(APPLICATION_NORITO),
        ),
        mk_response(StatusCode::OK, wire, Some(APPLICATION_JSON)),
        mk_response(StatusCode::OK, trailing, Some(APPLICATION_NORITO)),
        mk_response(StatusCode::OK, Vec::new(), Some(APPLICATION_NORITO)),
        mk_response(
            StatusCode::OK,
            norito::to_bytes(&altered).expect("tampered wire"),
            Some(APPLICATION_NORITO),
        ),
    ] {
        let (result, _) = capture_request(response, |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_bridge_finality_attestation(
                NonZeroU64::new(1).unwrap(),
                attestation.body.challenge,
                &attestation.body.node_id,
            )
        });
        assert!(result.is_err(), "invalid response must fail");
    }
}

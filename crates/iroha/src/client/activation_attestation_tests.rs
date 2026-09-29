// HTTP transport fixtures use a current result-only genesis plus real BLS node signatures.
// Full current quorum, ancestry and transaction checks live in the model and real-node tests.
fn current_finality_fixture() -> (
    iroha_data_model::sumeragi_finality::SumeragiFinalityProof,
    iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
    KeyPair,
) {
    use iroha_data_model::sumeragi_finality::*;
    let mut keys: Vec<_> = (71..75)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
    let validators: Vec<_> = keys
        .iter()
        .map(|key| FinalityValidator {
            public_key: key.public_key().clone(),
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
        })
        .collect();
    // Public multiples 1..4 of (-1, 2) on the two Pasta curves, shared with
    // DataModel's authenticated epoch fixtures; these are public test points.
    let pallas = [
        "00000000ed302d991bf94c09fc98462200000000000000000000000000000040",
        "030000b067c50313fcac1144eee2fe0e0000000000000000000000000000001c",
        "63d232eb3b8af0b75cfcf55ade47f6ff4cdf4e47a7454cb8ed67a9ba6f56e788",
        "fc86bc8efbbcb878f49427618b6940409b9157e3d777a4c4c0514a8e0d92db18",
    ];
    let vesta = [
        "0000000021eb468cdda89409fc98462200000000000000000000000000000040",
        "03000070de065fede0093144eee2fe0e0000000000000000000000000000001c",
        "5fce556feb6fee5a15560ddabae10224b026a5d0281af4c613955c39a8797837",
        "f79037a77e26a2c0794dc326d866c664616499c064073a8f8ebf3080297be5ab",
    ];
    use iroha_data_model::{
        block::consensus_v2::SumeragiV2GenesisContextParameters,
        isi::{
            InstructionBox, RegisterPeerWithPop, SetParameter,
            kagemusha_v1::{
                KagemushaMintFinalityAuthorityGenerationTemplateV1,
                KagemushaMintFinalityGenesisParametersV1, KagemushaMintFinalityValidatorKeysV1,
            },
        },
        parameter::{
            CustomParameter, Parameter,
            system::{
                ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
                consensus_metadata,
            },
        },
        transaction::FeePaymentIntent,
    };
    let metadata = ConsensusHandshakeMetadata {
        mode: SumeragiConsensusMode::Permissioned,
        block_cadence_ms: NonZeroU64::new(1000).unwrap(),
        wire_protocol_version: u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION),
        consensus_fingerprint: ConsensusFingerprint::new([0x71; 32]),
        kagemusha_mint_finality: KagemushaMintFinalityGenesisParametersV1 {
            authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
                version: 1,
                generation: 0,
                validators: validators
                    .iter()
                    .enumerate()
                    .map(|(index, validator)| KagemushaMintFinalityValidatorKeysV1 {
                        validator: iroha_model_base::peer::PeerId::new(
                            validator.public_key.clone(),
                        ),
                        eq_proof_public_key: hex::decode(pallas[index])
                            .unwrap()
                            .try_into()
                            .unwrap(),
                        ep_proof_public_key: hex::decode(vesta[index]).unwrap().try_into().unwrap(),
                    })
                    .collect(),
            },
        },
        sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
    };
    let mut instructions = validators
        .iter()
        .map(|validator| {
            InstructionBox::from(RegisterPeerWithPop::new(
                iroha_model_base::peer::PeerId::new(validator.public_key.clone()),
                validator.proof_of_possession.clone(),
            ))
        })
        .collect::<Vec<_>>();
    instructions.push(
        SetParameter::new(Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            iroha_primitives::json::Json::new(metadata),
        )))
        .into(),
    );
    let authority = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
    let mut transaction = TransactionBuilder::new_genesis(
        AccountId::new(authority.public_key().clone()),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions(instructions);
    transaction.set_creation_time(Duration::from_millis(1));
    let transaction = transaction.sign(authority.private_key());
    let genesis =
        SignedBlock::try_genesis(vec![transaction], authority.private_key(), None, None).unwrap();
    let epoch = genesis_epoch(&genesis).unwrap();
    let mut block = genesis.clone();
    let output = client_fixture_network_output(
        0,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()).into(),
    );
    attach_client_fixture_outputs(&mut block, vec![output], 1);
    let params = ChainParamsRecord {
        block_time_ms: 1000,
        payload_retry_interval_ms: 1000,
        exec_budget_ms: 100,
        apply_budget_ms: 100,
        max_block_bytes: 1024 * 1024,
        epoch_length_blocks: 7200,
    };
    let slot = |height| {
        ScheduledSlot::Ready(ScheduledConfig {
            height,
            epoch: epoch.clone(),
            params,
        })
    };
    let (len, hash) = block.executed_block_wire_identity().unwrap();
    let (native_lanes, ordinary_root) =
        NativeLaneStateProof::empty_for_testing(epoch.network_id, 1);
    let result = ExecutionResultCommitment {
        height: 1,
        execution: ExecutionCommitment {
            parent_state_root: Hash::new(b"parent"),
            post_state_root: ordinary_root,
            ordinary_writes_root: ordinary_root,
            kagemusha_top_up_root: None,
            kagemusha_top_up_count: 0,
            executed_block_wire_len: len,
            executed_block_wire_hash: hash,
            transaction_input_commitment: block.network_input_merkle_commitment(),
            transaction_output_commitment: block.output_merkle_commitment(),
        },
        schedule: ScheduleOutcome {
            height: 1,
            current: epoch.clone(),
            boundary: None,
            next: slot(2),
            after_next: slot(3),
        },
        beacon: None,
        native_lanes,
    };
    block.set_commit_certificate(Some(
        iroha_data_model::block::CommitCertificate::from_untrusted_parts(
            vec![],
            vec![],
            result.preimage().unwrap(),
        ),
    ));
    let proof = SumeragiFinalityProof {
        block_header: block.header(),
        block_wire: block.encode_wire().unwrap(),
        committee: validators.clone(),
    };
    let verifier =
        SumeragiFinalityVerifier::new(&genesis, "sdk-current-finality", validators).unwrap();
    (proof, verifier, keys.remove(0))
}
fn current_finality_client() -> Client {
    let mut client = client_with_base_url(base_url());
    client.network_id =
        NetworkId::from_genesis_hash(current_finality_fixture().0.block_header.hash());
    client
}
fn client_attestation_fixture() -> iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation
{
    use iroha_data_model::sumeragi_finality::*;
    let (proof, verifier, signer) = current_finality_fixture();
    let node_id = iroha_model_base::peer::PeerId::new(signer.public_key().clone());
    let body = SumeragiFinalityAttestationBody {
        challenge: [17; 32],
        network_id: NetworkId::from_genesis_hash(proof.block_header.hash()),
        node_fingerprint: Hash::new(norito::codec::Encode::encode(&node_id)),
        node_id,
        build_fingerprint: Hash::new(b"build"),
        config_fingerprint: Hash::new(b"config"),
        genesis_block_hash: proof.block_header.hash(),
        genesis_finality_proof: proof.clone(),
        status: iroha_data_model::sumeragi::SumeragiStatus {
            protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
            config_fingerprint: Hash::new(b"config"),
            beacon_horizon: None,
            instance: verifier.instance().0,
            height: 2,
            view: 0,
            stage: 0,
            leader: None,
            proxy_tail: None,
            high_qc_view: None,
            level: 0,
            start_level: 0,
            t_retx_ms: 100,
            committed_height: 1,
            applied_height: 1,
            awaiting: false,
            signer: Some(signer.public_key().clone()),
            unanchored: false,
            abstaining: false,
            halted: None,
            footprint: Default::default(),
        },
        finality_proof: proof,
    };
    let signature =
        iroha_crypto::SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
            .unwrap();
    let value = SumeragiFinalityAttestation { body, signature };
    value.verify().unwrap();
    value
}

#[test]
fn bridge_finality_attestation_reader_binds_exact_request_headers_and_signed_body() {
    let attestation = client_attestation_fixture();
    let mut client = current_finality_client();
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
        client.get_sumeragi_finality_attestation(
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
        SUMERAGI_FINALITY_RESPONSE_MAX_BYTES
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
    let client = current_finality_client();
    let wire = norito::to_bytes(&attestation).expect("wire");
    let (zero, requests) = capture_requests(
        mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
        |transport| {
            client
                .clone()
                .with_test_http_transport(transport)
                .get_sumeragi_finality_attestation(
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
        KeyPair::try_from_seed(vec![94; 32], Algorithm::BlsNormal).expect("other reporter");
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
                client.get_sumeragi_finality_attestation(
                    NonZeroU64::new(height).unwrap(),
                    challenge,
                    &node,
                )
            },
        );
        assert!(result.is_err(), "wrong request binding must fail");
    }
    let mut altered = attestation.clone();
    altered.body.build_fingerprint = Hash::new(b"tampered unsigned status");
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
            client.get_sumeragi_finality_attestation(
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
            client.get_sumeragi_finality_attestation(
                NonZeroU64::new(1).unwrap(),
                attestation.body.challenge,
                &attestation.body.node_id,
            )
        });
        assert!(result.is_err(), "invalid response must fail");
    }
}

fn client_tip_progress_fixture()
-> iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
    let key = KeyPair::try_from_seed(vec![96; 32], Algorithm::BlsNormal).expect("reporter key");
    iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
        requested_height: 10,
        applied_height: 9,
        status_height: 10,
        challenge: [17; 32],
        node_id: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
        network_id: current_finality_client().network_id,
    }
}

fn client_tip_progress_envelope(
    progress: iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1,
) -> iroha_torii_shared::ErrorEnvelope {
    iroha_torii_shared::bridge_attestation::FinalityAttestationFailure {
        height: progress.requested_height,
        challenge: progress.challenge,
        reason:
            iroha_torii_shared::bridge_attestation::FinalityAttestationFailureReason::TipChanged,
        tip_mismatch: Some(progress),
    }
    .into_error_envelope()
}

#[test]
fn bridge_finality_attestation_reader_preserves_only_bound_typed_tip_progress() {
    for (requested, applied, status) in [(10, 9, 10), (9, 10, 10), (10, 10, 9)] {
        let mut progress = client_tip_progress_fixture();
        progress.requested_height = requested;
        progress.applied_height = applied;
        progress.status_height = status;
        let response = mk_response(
            StatusCode::CONFLICT,
            norito::to_bytes(&client_tip_progress_envelope(progress.clone())).expect("wire"),
            Some(APPLICATION_NORITO),
        );
        let (result, requests) = capture_requests(response, |transport| {
            let client = current_finality_client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_sumeragi_finality_attestation(
                NonZeroU64::new(requested).unwrap(),
                progress.challenge,
                &progress.node_id,
            )
        });
        assert_eq!(requests.len(), 1, "one-shot reader does not poll");
        assert_eq!(
            requests[0].url.path(),
            format!("/v1/bridge/finality/attestation/{requested}")
        );
        let error = result.expect_err("a changing tip is not finality evidence");
        let typed = error
            .downcast_ref::<BridgeFinalityAttestationTipMismatch>()
            .expect("exact typed progress");
        assert_eq!(typed.response(), &progress);
        assert!(typed.to_string().contains("finality tip is changing"));
    }
}

#[test]
fn bridge_finality_attestation_reader_rejects_malformed_or_unbound_tip_progress() {
    let valid = client_tip_progress_fixture();
    let mut cases = Vec::new();
    let mut changed = valid.clone();
    changed.requested_height = 11;
    cases.push(changed);
    let mut changed = valid.clone();
    changed.challenge = [18; 32];
    cases.push(changed);
    let mut changed = valid.clone();
    changed.challenge = [0; 32];
    cases.push(changed);
    let mut changed = valid.clone();
    changed.node_id = iroha_model_base::peer::PeerId::new(
        KeyPair::try_from_seed(vec![97; 32], Algorithm::BlsNormal)
            .expect("other reporter")
            .public_key()
            .clone(),
    );
    cases.push(changed);
    let mut changed = valid.clone();
    changed.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign finality progress network",
    )));
    cases.push(changed);
    for (requested, applied, status) in [(0, 9, 10), (10, 0, 10), (10, 9, 0), (10, 10, 10)] {
        let mut changed = valid.clone();
        changed.requested_height = requested;
        changed.applied_height = applied;
        changed.status_height = status;
        cases.push(changed);
    }
    for progress in cases {
        let response = mk_response(
            StatusCode::CONFLICT,
            norito::to_bytes(&client_tip_progress_envelope(progress)).expect("wire"),
            Some(APPLICATION_NORITO),
        );
        let (result, _) = capture_request(response, |transport| {
            let client = current_finality_client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_sumeragi_finality_attestation(
                NonZeroU64::new(valid.requested_height).unwrap(),
                valid.challenge,
                &valid.node_id,
            )
        });
        let error = result.expect_err("malformed or foreign progress must fail");
        assert!(
            error
                .downcast_ref::<BridgeFinalityAttestationTipMismatch>()
                .is_none()
        );
    }
}

#[test]
fn bridge_finality_attestation_reader_rejects_untyped_or_noncanonical_progress_http() {
    use iroha_torii_shared::bridge_attestation::{
        FINALITY_ATTESTATION_FAILURE_MAX_BYTES, FinalityAttestationFailureReason as Reason,
    };
    let progress = client_tip_progress_fixture();
    let envelope = client_tip_progress_envelope(progress.clone());
    let wire = norito::to_bytes(&envelope).expect("wire");
    let mut wrong_code = envelope.clone();
    wrong_code.code = "query_validation_failed".into();
    let mut missing_details = envelope.clone();
    missing_details.details = None;
    let mut wrong_details = envelope.clone();
    wrong_details
        .details
        .as_mut()
        .unwrap()
        .finality_attestation_failure
        .as_mut()
        .unwrap()
        .reason = Reason::ConflictingState;
    let mut mixed_details = envelope.clone();
    mixed_details.details.as_mut().unwrap().reject_code = Some("invalid_finality_proof".into());
    let mut trailing = wire.clone();
    trailing.push(0);
    let mut duplicate_type =
        mk_response(StatusCode::CONFLICT, wire.clone(), Some(APPLICATION_NORITO));
    duplicate_type.headers_mut().append(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static(APPLICATION_NORITO),
    );
    let mut responses = vec![duplicate_type];
    for status in [
        StatusCode::NOT_FOUND,
        StatusCode::SERVICE_UNAVAILABLE,
        StatusCode::OK,
    ] {
        responses.push(mk_response(status, wire.clone(), Some(APPLICATION_NORITO)));
    }
    for body in [
        Vec::new(),
        trailing,
        vec![0; FINALITY_ATTESTATION_FAILURE_MAX_BYTES + 1],
        norito::to_bytes(&wrong_code).expect("wire"),
        norito::to_bytes(&missing_details).expect("wire"),
        norito::to_bytes(&wrong_details).expect("wire"),
        norito::to_bytes(&mixed_details).expect("wire"),
        b"query_validation_failed: Query not found in the live query store.".to_vec(),
        norito::to_bytes(&iroha_torii_shared::ErrorEnvelope::new(
            "bridge_finality_attestation_tip_mismatch",
            "retired envelope",
        ))
        .expect("obsolete envelope is negative data"),
    ] {
        responses.push(mk_response(
            StatusCode::CONFLICT,
            body,
            Some(APPLICATION_NORITO),
        ));
    }
    for content_type in [
        None,
        Some(APPLICATION_JSON),
        Some("application/x-norito; arbitrary=1"),
    ] {
        responses.push(mk_response(
            StatusCode::CONFLICT,
            wire.clone(),
            content_type,
        ));
    }
    responses.push(mk_response(
        StatusCode::CONFLICT,
        norito::json::to_json(&envelope).expect("json").into_bytes(),
        Some(APPLICATION_JSON),
    ));
    for response in responses {
        let (result, _) = capture_request(response, |transport| {
            let client = current_finality_client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_sumeragi_finality_attestation(
                NonZeroU64::new(progress.requested_height).unwrap(),
                progress.challenge,
                &progress.node_id,
            )
        });
        let error = result.expect_err("only canonical bound HTTP409 is progress");
        assert!(
            error
                .downcast_ref::<BridgeFinalityAttestationTipMismatch>()
                .is_none()
        );
    }
}

fn rejected_next_bridge_finality_response(
    client: &Client,
    height: NonZeroU64,
    verifier: &mut iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
    response: HttpResponse<Vec<u8>>,
) -> String {
    capture_request(response, |mock_transport| {
        let client = client
            .clone()
            .with_test_http_transport(mock_transport.clone());
        mark_data_model_compatible(&client);

        client.get_next_sumeragi_finality_proof(height, verifier)
    })
    .0
    .expect_err("bridge finality response must fail")
    .to_string()
}

#[test]
fn bridge_finality_reader_retries_only_backpressure_within_original_deadline() {
    let (successor, mut verifier, _) = current_finality_fixture();
    let expected = successor.clone();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observations = Arc::clone(&calls);
    let started = std::time::Instant::now();
    let deadline = started + Duration::from_secs(10);
    let result = with_mock_http(
        move |request| {
            let mut observations = observations.lock().expect("observations");
            observations.push((std::time::Instant::now(), request));
            if observations.len() == 1 {
                let mut response = empty_response(StatusCode::TOO_MANY_REQUESTS);
                response
                    .headers_mut()
                    .insert("retry-after", "1".parse().unwrap());
                Ok(response)
            } else {
                Ok(norito_response(StatusCode::OK, &expected))
            }
        },
        |transport| {
            let client = current_finality_client()
                .with_test_http_transport(transport)
                .with_request_deadline(deadline);
            mark_data_model_compatible(&client);
            client.get_next_sumeragi_finality_proof(successor.block_header.height(), &mut verifier)
        },
    )
    .expect("bounded retry accepts exact successor");
    assert_eq!(result, successor);
    let calls = calls.lock().expect("observations");
    assert_eq!(calls.len(), 2);
    assert!(calls[1].0.duration_since(calls[0].0) >= Duration::from_secs(1));
    for (_, request) in calls.iter() {
        assert_eq!(request.method, HttpMethod::GET);
        assert_eq!(request.url.path(), "/v1/bridge/finality/1");
        assert!(request.timeout.unwrap() <= deadline.duration_since(started));
    }
    assert!(calls[1].1.timeout.unwrap() < calls[0].1.timeout.unwrap());
}

#[test]
fn bridge_finality_reader_rejects_unbounded_or_invalid_backpressure_without_advancing() {
    let (successor, mut verifier, _) = current_finality_fixture();
    let height = successor.block_header.height();
    // A missing operation deadline, malformed/duplicate hints, an excessive delay,
    // and non-429 errors must all make exactly one dispatch without changing trust.
    for (status, hints, budget) in [
        (StatusCode::TOO_MANY_REQUESTS, vec!["0"], None),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["-1"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["0", "1"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["18446744073709551615"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["1"],
            Some(Duration::from_millis(500)),
        ),
        (
            StatusCode::UNAUTHORIZED,
            vec!["0"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            vec!["0"],
            Some(Duration::from_secs(5)),
        ),
    ] {
        let mut response = empty_response(status);
        for hint in hints {
            response
                .headers_mut()
                .append("retry-after", hint.parse().unwrap());
        }
        let (result, _) = capture_request(response, |transport| {
            let client = current_finality_client().with_test_http_transport(transport);
            let client = budget.map_or_else(
                || client.clone(),
                |budget| client.with_request_deadline(std::time::Instant::now() + budget),
            );
            mark_data_model_compatible(&client);
            client.get_next_sumeragi_finality_proof(height, &mut verifier)
        });
        assert!(result.is_err(), "{status} must fail");
    }
    let actual = capture_request(norito_response(StatusCode::OK, &successor), |transport| {
        let client = current_finality_client().with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        client.get_next_sumeragi_finality_proof(height, &mut verifier)
    })
    .0
    .expect("all failed reads retained the original chain anchor");
    assert_eq!(actual, successor);
}

#[test]
fn activation_evidence_backpressure_preserves_challenge_and_response_bounds() {
    for hint in [None, Some("0")] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observations = Arc::clone(&calls);
        let challenge = [0x73; 32];
        with_mock_http(
            move |request| {
                let mut observations = observations.lock().unwrap();
                observations.push(request);
                if observations.len() == 1 {
                    let mut response = empty_response(StatusCode::TOO_MANY_REQUESTS);
                    if let Some(hint) = hint {
                        response
                            .headers_mut()
                            .insert("retry-after", hint.parse().unwrap());
                    }
                    Ok(response)
                } else {
                    Ok(empty_response(StatusCode::CONFLICT))
                }
            },
            |transport| {
                let client = current_finality_client()
                    .with_test_http_transport(transport)
                    .with_request_deadline(std::time::Instant::now() + Duration::from_secs(5));
                let result = client
                    .send_activation_evidence_read(
                        "/v1/bridge/finality/2/attestation",
                        2048,
                        Some(challenge),
                        ActivationEvidenceReadAuth::Public,
                    )
                    .expect("read-only retry");
                assert_eq!(result.status(), StatusCode::CONFLICT);
            },
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for request in calls.iter() {
            assert_eq!(request.method, HttpMethod::GET);
            assert_eq!(request.max_response_bytes, 2048);
            let challenges: Vec<_> = request
                .headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case("x-iroha-finality-challenge"))
                .collect();
            assert_eq!(challenges.len(), 1);
            assert_eq!(challenges[0].1, hex::encode(challenge));
        }
    }
}

#[test]
fn bridge_finality_next_reader_response_contract_failures_do_not_advance() {
    let client = current_finality_client();

    let (successor, mut verifier, _) = current_finality_fixture();
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
    assert!(error.contains("Failed to get current finality proof"));

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
            vec![0; SUMERAGI_FINALITY_RESPONSE_MAX_BYTES + 1],
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
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_next_sumeragi_finality_proof(height, &mut verifier)
        },
    )
    .0
    .expect("valid successor must verify after rejected responses");
    assert_eq!(actual, successor);
}

#[test]
fn bridge_finality_next_reader_verification_failure_does_not_advance() {
    let client = current_finality_client();

    let (successor, mut verifier, _) = current_finality_fixture();
    let height = successor.block_header.height();
    let mut invalid = successor.clone();
    invalid.committee[0].proof_of_possession[0] ^= 0x40;
    let invalid_body = norito::to_bytes(&invalid).expect("encode invalid successor finality proof");
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, invalid_body, Some(APPLICATION_NORITO)),
    );
    assert!(!error.is_empty());

    let body = norito::to_bytes(&successor).expect("encode canonical successor proof");
    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_next_sumeragi_finality_proof(height, &mut verifier)
        },
    )
    .0
    .expect("valid successor must verify after a rejected invalid signature");
    assert_eq!(actual, successor);
}

#[test]
fn bridge_finality_reader_expired_deadline_does_not_dispatch_or_advance() {
    let (successor, mut verifier, _) = current_finality_fixture();
    with_mock_http(
        |_| panic!("expired deadline must not dispatch"),
        |transport| {
            let client = current_finality_client()
                .with_test_http_transport(transport)
                .with_request_deadline(std::time::Instant::now());
            mark_data_model_compatible(&client);
            assert!(
                client
                    .get_next_sumeragi_finality_proof(
                        successor.block_header.height(),
                        &mut verifier
                    )
                    .is_err()
            );
        },
    );
    verifier
        .verify(&successor)
        .expect("deadline retained original anchor");
}

#[test]
fn bridge_finality_next_reader_rejects_height_mismatch_before_advancing() {
    let (proof, mut verifier, _) = current_finality_fixture();
    let (result, request) = capture_request(norito_response(StatusCode::OK, &proof), |transport| {
        let client = current_finality_client().with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        client.get_next_sumeragi_finality_proof(NonZeroU64::new(2).unwrap(), &mut verifier)
    });
    assert!(result.unwrap_err().to_string().contains("requested height"));
    assert_eq!(request.url.path(), "/v1/bridge/finality/2");
    verifier
        .verify(&proof)
        .expect("mismatched height retained original prefix");
}
#[test]
fn current_genesis_readiness_authenticates_selected_root_and_instance() {
    let attestation = client_attestation_fixture();
    let (_, verifier, _) = current_finality_fixture();
    let (result, _) = capture_request(norito_response(StatusCode::OK, &attestation), |transport| {
        let client = current_finality_client().with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        client.poll_sumeragi_genesis_readiness(
            attestation.body.challenge,
            &attestation.body.node_id,
            &verifier,
            std::time::Instant::now() + Duration::from_secs(5),
        )
    });
    assert!(matches!(
        result.unwrap(),
        GenesisFinalityReadiness::Ready(_)
    ));
    let mut wrong = attestation.clone();
    wrong.body.status.instance[0] ^= 1;
    let (_, _, signer) = current_finality_fixture();
    wrong.signature =
        iroha_crypto::SignatureOf::try_from_hash(signer.private_key(), wrong.body.signing_hash())
            .unwrap();
    let (result, _) = capture_request(norito_response(StatusCode::OK, &wrong), |transport| {
        let client = current_finality_client().with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        client.poll_sumeragi_genesis_readiness(
            wrong.body.challenge,
            &wrong.body.node_id,
            &verifier,
            std::time::Instant::now() + Duration::from_secs(5),
        )
    });
    assert!(
        result.is_err(),
        "valid node signature cannot substitute the selected consensus instance"
    );
}

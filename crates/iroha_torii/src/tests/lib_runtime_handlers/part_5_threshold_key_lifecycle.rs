use iroha_data_model::isi::consensus_keys::{
    ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
    ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
};

fn lifecycle_ordinary_fixture(
    persist_finality: bool,
) -> (
    SharedAppState,
    KeyPair,
    Vec<KeyPair>,
    ThresholdKeyLifecycleCertificateV1,
    tempfile::TempDir,
) {
    let (mut app, _, _) = app_with_indexed_sccp_message_for_test(persist_finality);
    let authority_key = checked_torii_test_ed25519_keypair(0x39, "lifecycle ingress authority");
    let authority = AccountId::new(authority_key.public_key().clone());
    let mut validators = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive exact durable-finality validator")
        })
        .collect::<Vec<_>>();
    validators.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let roster = validators
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    {
        let app = Arc::get_mut(&mut app).expect("unique lifecycle app");
        app.local_peer_id = Some(roster[0].clone());
        app.torii_proxy_bridge_signer = validators[0].clone();
        let state = Arc::get_mut(&mut app.state).expect("unique lifecycle state");
        state.world = world_with_account(&authority);
        let bindings = validators
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let validator = AccountId::new(key.public_key().clone());
                ensure_runtime_peer_binding_for_test(
                    state,
                    &validator,
                    key,
                    &format!("lifecycle-{index}"),
                );
                (validator, PeerId::new(key.public_key().clone()))
            })
            .collect::<Vec<_>>();
        let mut topology = state.commit_topology.block();
        topology.clear();
        for peer in &roster {
            topology.push(peer.clone());
        }
        topology.commit();
        install_lane_manifest_registry_for_test(state, &[(LaneId::SINGLE, bindings)]);
    }
    let mut certificate = ThresholdKeyLifecycleCertificateV1 {
        version: iroha_core::state::THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        action: ThresholdKeyLifecycleActionV1::RetireGlobalBeaconKey,
        expected_active_session_id: Some([0x41; 32]),
        effective_height: 2,
        network_id: *app.state.network_id_ref(),
        roster_hash: iroha_core::beacon::global_threshold_beacon_roster_hash_v1(&roster),
        committee_size: 4,
        quorum: 3,
        session_id: [0x41; 32],
        transcript_hash: [0x42; 32],
        public_state: Vec::new(),
        signatures: Vec::new(),
    };
    lifecycle_sign_certificate(&mut certificate, &validators);
    let journal = tempfile::tempdir().expect("lifecycle durable journal");
    app.queue
        .install_plan_journal(&journal.path().join("queue.norito"), 1024 * 1024, true)
        .expect("install lifecycle durable journal");
    (app, authority_key, validators, certificate, journal)
}

fn lifecycle_sign_certificate(
    certificate: &mut ThresholdKeyLifecycleCertificateV1,
    keys: &[KeyPair],
) {
    let preimage = iroha_core::state::threshold_key_lifecycle_certificate_preimage_v1(certificate)
        .expect("native lifecycle signing preimage");
    certificate.signatures = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, key)| ThresholdKeyLifecycleSignatureV1 {
            signer_index: u16::try_from(index).expect("small fixture index"),
            signature: Signature::try_new(key.private_key(), &preimage)
                .expect("sign exact lifecycle QC"),
        })
        .collect();
}

fn lifecycle_ordinary_transaction(
    app: &SharedAppState,
    key: &KeyPair,
    instructions: Vec<iroha_data_model::isi::InstructionBox>,
) -> SignedTransaction {
    TransactionBuilder::new(
        *app.state.network_id_ref(),
        AccountId::new(key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .with_admission_intent(TransactionAdmissionIntent::Ordinary)
    .sign(key.private_key())
}

async fn lifecycle_submit(app: &SharedAppState, transaction: SignedTransaction) -> Response {
    super::submit_signed_transaction_for_ingress(app.clone(), HeaderMap::new(), None, transaction)
        .await
        .expect("public lifecycle submission returns a classified response")
}

#[tokio::test]
async fn lifecycle_ordinary_ingress_accepts_exact_quorum_and_preserves_wire_identity() {
    let (app, key, _, certificate, journal) = lifecycle_ordinary_fixture(true);
    let transaction = lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into()],
    );
    let submitted_hash = transaction.hash();
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let submitted_wire =
        <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(
            &transaction,
        );
    let response = lifecycle_submit(&app, transaction).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-signed-transaction-hash")
            .unwrap(),
        submitted_hash.to_string().as_str()
    );
    assert_eq!(
        response.headers().get("x-iroha-entrypoint-hash").unwrap(),
        entrypoint_hash.to_string().as_str()
    );
    let body = torii_body_bytes(response, "ordinary lifecycle receipt").await;
    let receipt: TransactionSubmissionReceipt = norito::decode_from_bytes(&body)
        .expect("ordinary submission receipt, not QueuePlan certificate");
    receipt.verify().expect("signed ordinary receipt");
    assert_eq!(receipt.payload.entrypoint_hash, entrypoint_hash);
    assert_eq!(
        receipt.payload.signed_transaction_hash,
        Some(submitted_hash)
    );
    assert_eq!(app.queue.active_len(), 1);
    let state = app.state.view();
    let queued = app.queue.all_transactions(&state).collect::<Vec<_>>();
    assert_eq!(queued.len(), 1);
    let stored = queued[0]
        .external()
        .expect("exact external lifecycle transaction");
    assert_eq!(
        <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(stored),
        submitted_wire
    );
    assert!(
        std::fs::metadata(journal.path().join("queue.norito"))
            .unwrap()
            .len()
            > 0
    );
}

#[tokio::test]
async fn ordinary_single_route_application_is_durable_and_mixed_lifecycle_is_rejected() {
    let (app, key, _, certificate, journal) = lifecycle_ordinary_fixture(true);
    let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
    let response = lifecycle_submit(
        &app,
        lifecycle_ordinary_transaction(
            &app,
            &key,
            vec![Log::new(Level::INFO, "ordinary application".to_owned()).into()],
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 1);
    assert_ne!(
        std::fs::read(journal.path().join("queue.norito")).unwrap(),
        before
    );
    let response = lifecycle_submit(
        &app,
        lifecycle_ordinary_transaction(
            &app,
            &key,
            vec![
                ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into(),
                Log::new(Level::INFO, "mixed application".to_owned()).into(),
            ],
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(app.queue.active_len(), 1);
}

#[tokio::test]
async fn ordinary_sealed_commitment_is_durable_and_requires_one_route() {
    use iroha_data_model::transaction::signed::{
        SealedTransactionCommitmentPayload, SignedSealedTransactionCommitment,
    };

    let (app, key, _, _, journal) = lifecycle_ordinary_fixture(true);
    let network_id = *app.state.network_id_ref();
    let signed = lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![Log::new(Level::INFO, "sealed application".to_owned()).into()],
    );
    let commitment_hash =
        compute_sealed_transaction_commitment(&network_id, &signed, [0x51; 32], 9);
    let entrypoint =
        TransactionEntrypoint::SealedCommitment(SignedSealedTransactionCommitment::sign(
            SealedTransactionCommitmentPayload::new(
                network_id,
                AccountId::new(key.public_key().clone()),
                commitment_hash,
                3,
                9,
                None,
            ),
            key.private_key(),
        ));
    let coordinator = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let participant = iroha_core::queue::RouteLeg::new(
        RoutingDecision::new(LaneId::new(9), DataSpaceId::new(9)),
        iroha_core::queue::RouteLegRole::Participant,
    );
    let multi_route = RoutingPlan::native_amx(coordinator, vec![participant]);
    assert!(
        super::ordinary_transaction_ingress::authenticate(&app, &entrypoint, &multi_route)
            .expect_err("sealed commitment must not bypass the single-route guard")
            .contains("QueuePlanSynced")
    );
    let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
    let response = super::handler_post_transaction_entrypoint(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_entrypoint_for_test(entrypoint.clone()),
    )
    .await
    .expect("ordinary sealed commitment returns a classified response")
    .into_response();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 1);
    let state = app.state.view();
    let queued = app.queue.all_transactions(&state).collect::<Vec<_>>();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].entrypoint(), &entrypoint);
    assert_ne!(
        std::fs::read(journal.path().join("queue.norito")).unwrap(),
        before
    );
}

#[test]
fn ordinary_sealed_reveal_authenticates_exact_lifecycle_certificate() {
    let (app, key, _, certificate, _) = lifecycle_ordinary_fixture(true);
    let network_id = *app.state.network_id_ref();
    let route = RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL));
    let reveal = |signed: SignedTransaction| {
        let salt = [0x52; 32];
        let commitment = compute_sealed_transaction_commitment(&network_id, &signed, salt, 9);
        TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(commitment, signed, salt))
    };
    let ordinary = reveal(lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![Log::new(Level::INFO, "revealed application".to_owned()).into()],
    ));
    super::ordinary_transaction_ingress::authenticate(&app, &ordinary, &route)
        .expect("ordinary sealed reveal accepts its single route");
    let exact = reveal(lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![
            ApplyThresholdKeyLifecycleCertificateV1 {
                certificate: certificate.clone(),
            }
            .into(),
        ],
    ));
    super::ordinary_transaction_ingress::authenticate(&app, &exact, &route)
        .expect("exact certified lifecycle reveal authenticates its frozen roster");
    let mixed = reveal(lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![
            ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into(),
            Log::new(Level::INFO, "mixed revealed application".to_owned()).into(),
        ],
    ));
    assert!(
        super::ordinary_transaction_ingress::authenticate(&app, &mixed, &route)
            .expect_err("mixed lifecycle reveal must be rejected")
            .contains("one exact certificate")
    );
}

#[tokio::test]
async fn ordinary_multi_route_application_requires_certified_intent() {
    let (app, key, _, _, journal) = lifecycle_ordinary_fixture(true);
    let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
    let transaction = lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![Log::new(Level::INFO, "multi-route application".to_owned()).into()],
    );
    let coordinator = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let participant = iroha_core::queue::RouteLeg::new(
        RoutingDecision::new(LaneId::new(9), DataSpaceId::new(9)),
        iroha_core::queue::RouteLegRole::Participant,
    );
    let routing_plan = RoutingPlan::native_amx(coordinator, vec![participant]);
    let error = super::ordinary_transaction_ingress::authenticate(
        &app,
        &TransactionEntrypoint::External(transaction),
        &routing_plan,
    )
    .expect_err("multi-route ordinary ingress must refuse before durable custody");
    assert!(error.contains("QueuePlanSynced"));
    assert_eq!(app.queue.active_len(), 0);
    assert_eq!(
        std::fs::read(journal.path().join("queue.norito")).unwrap(),
        before
    );
}

#[tokio::test]
async fn lifecycle_ordinary_ingress_rejects_invalid_certificate_authority() {
    let (app, key, validators, certificate, journal) = lifecycle_ordinary_fixture(true);
    let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
    let invalid_outer =
        transaction_with_invalid_signature_for_test(lifecycle_ordinary_transaction(
            &app,
            &key,
            vec![
                ApplyThresholdKeyLifecycleCertificateV1 {
                    certificate: certificate.clone(),
                }
                .into(),
            ],
        ));
    let error = super::submit_signed_transaction_for_ingress(
        app.clone(),
        HeaderMap::new(),
        None,
        invalid_outer,
    )
    .await
    .expect_err("invalid account signature must fail before lifecycle admission");
    assert_eq!(
        torii_response_header(&error.into_response(), "x-iroha-reject-code"),
        Some(SignatureRejectionCode::InvalidSignature.as_str())
    );
    let mut wrong_height = certificate.clone();
    wrong_height.effective_height += 1;
    lifecycle_sign_certificate(&mut wrong_height, &validators);
    let mut wrong_network = certificate.clone();
    wrong_network.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign lifecycle network")),
    );
    lifecycle_sign_certificate(&mut wrong_network, &validators);
    let mut wrong_roster = certificate.clone();
    wrong_roster.roster_hash = [0x99; 32];
    lifecycle_sign_certificate(&mut wrong_roster, &validators);
    let mut forged = certificate.clone();
    forged.signatures[0].signature =
        Signature::try_new(validators[3].private_key(), b"forged lifecycle QC").unwrap();
    let mut duplicate = certificate.clone();
    duplicate.signatures[1].signer_index = duplicate.signatures[0].signer_index;
    let mut insufficient = certificate;
    insufficient.signatures.pop();
    for invalid in [
        wrong_height,
        wrong_network,
        wrong_roster,
        forged,
        duplicate,
        insufficient,
    ] {
        let response = lifecycle_submit(
            &app,
            lifecycle_ordinary_transaction(
                &app,
                &key,
                vec![
                    ApplyThresholdKeyLifecycleCertificateV1 {
                        certificate: invalid,
                    }
                    .into(),
                ],
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(app.queue.active_len(), 0);
    assert_eq!(
        std::fs::read(journal.path().join("queue.norito")).unwrap(),
        before
    );
}

#[tokio::test]
async fn lifecycle_ordinary_ingress_requires_authenticated_parent_and_global_route() {
    for persist in [false, true] {
        let (mut app, key, _, certificate, journal) = lifecycle_ordinary_fixture(persist);
        if persist {
            Arc::get_mut(&mut app).unwrap().local_peer_id =
                Some(PeerId::new(key.public_key().clone()));
        }
        let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
        let response = lifecycle_submit(
            &app,
            lifecycle_ordinary_transaction(
                &app,
                &key,
                vec![ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into()],
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(app.queue.active_len(), 0);
        assert_eq!(
            std::fs::read(journal.path().join("queue.norito")).unwrap(),
            before
        );
    }
    let (app, key, _, certificate, journal) = lifecycle_ordinary_fixture(true);
    let before = std::fs::read(journal.path().join("queue.norito")).unwrap();
    let transaction = lifecycle_ordinary_transaction(
        &app,
        &key,
        vec![
            ApplyThresholdKeyLifecycleCertificateV1 {
                certificate: certificate.clone(),
            }
            .into(),
        ],
    );
    let parameters = app.state.view().world().parameters().clone();
    let accepted = iroha_core::tx::AcceptedTransaction::accept_entrypoint(
        TransactionEntrypoint::External(transaction),
        app.state.network_id_ref(),
        parameters.sumeragi().max_clock_drift(),
        parameters.transaction(),
        app.state.crypto().as_ref(),
    )
    .expect("exact signed lifecycle entrypoint");
    let response = super::execute_torii_transaction_via_proxy(
        &app,
        accepted,
        RoutingPlan::single(RoutingDecision::new(LaneId::new(9), DataSpaceId::new(9))),
        None,
        true,
        ResponseFormat::Norito,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let finality_path = app.kura.v2_finality_artifact_path_for_testing(1);
    std::fs::write(finality_path, b"corrupt authenticated parent").unwrap();
    let response = lifecycle_submit(
        &app,
        lifecycle_ordinary_transaction(
            &app,
            &key,
            vec![ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into()],
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(app.queue.active_len(), 0);
    assert_eq!(
        std::fs::read(journal.path().join("queue.norito")).unwrap(),
        before
    );
}

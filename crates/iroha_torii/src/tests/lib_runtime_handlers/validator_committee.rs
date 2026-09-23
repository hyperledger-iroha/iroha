// Finality-backed committee status transport and immutable selection bindings.

#[tokio::test]
async fn validator_committee_status_serves_exact_finality_and_closed_queries() {
    use crate::validator_committee::{CommitteeStatusQuery, handler_validator_committee_status};
    use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
    let (app, _, artifact) = committed_network_proof_app_for_test();
    let peer = "127.0.0.1:12345".parse().unwrap();
    for accept in ["application/x-norito", "application/json"] {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(axum::http::header::ACCEPT, accept.parse().unwrap());
        let response = handler_validator_committee_status(
            State(app.clone()),
            crate::NoritoQuery(CommitteeStatusQuery { target_epoch: None }),
            headers,
            axum::extract::ConnectInfo(peer),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status: ValidatorCommitteeStatusV1 = if accept == "application/x-norito" {
            norito::decode_from_bytes(&body).unwrap()
        } else {
            norito::json::from_slice(&body).unwrap()
        };
        assert_eq!(status.target_epoch, 1);
        assert_eq!(status.latest_finality, artifact);
        assert_eq!(status.network_id, artifact.height_context.network_id);
        assert_eq!(status.selected, None);
        assert!(status.candidate_keys.is_empty());
        assert_eq!(status.pending_beacon_session, None);
    }
    for query in [
        "unknown=2",
        "target_epoch=-1",
        "target_epoch=2&target_epoch=3",
    ] {
        use axum::extract::FromRequestParts as _;
        let (mut parts, _) = axum::http::Request::builder()
            .uri(format!("/v1/nexus/validator-committee?{query}"))
            .body(())
            .unwrap()
            .into_parts();
        assert!(
            crate::NoritoQuery::<CommitteeStatusQuery>::from_request_parts(&mut parts, &())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn validator_committee_status_rejects_missing_committed_finality() {
    use crate::validator_committee::{CommitteeStatusQuery, handler_validator_committee_status};
    let app = mk_app_state_for_tests();
    let block = make_empty_signed_block(1, None, 1);
    let header = block.header();
    let hash = store_block(&app, block);
    record_committed_block_hash_for_test(&app, header, hash);
    let result = handler_validator_committee_status(
        State(app),
        crate::NoritoQuery(CommitteeStatusQuery {
            target_epoch: Some(1),
        }),
        axum::http::HeaderMap::new(),
        axum::extract::ConnectInfo("127.0.0.1:12345".parse().unwrap()),
    )
    .await;
    assert!(
        result.is_err(),
        "raw committed State without exact durable finality is not status proof"
    );
}

#[tokio::test]
async fn validator_committee_status_selection_matches_signed_boundary_and_roundtrips() {
    use iroha_data_model::{
        block::consensus_v2::FinalizedNextEpochSnapshot,
        isi::kagemusha_v1::{
            BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
            KagemushaMintFinalityEpochDecisionV1,
        },
        nexus::{
            ValidatorCommitteePreparationV1, ValidatorCommitteeSelectionStatusV1,
            ValidatorCommitteeStatusV1, ValidatorCommitteeTransitionV1,
        },
    };
    let app = mk_app_state_for_tests();
    let network = *app.state.network_id_ref();
    let first = make_empty_signed_block(1, None, 1);
    let first_finality = crate::test_utils::torii_proof_finality_for_block_with_context(
        &first,
        network,
        None,
        |context| {
            context.mode = iroha_data_model::block::consensus_v2::ConsensusMode::Npos;
            context.epoch_end_height = 2;
            context.kagemusha_mint_finality_authorization.last_height = 2;
        },
    );
    let second = make_empty_signed_block(2, Some(first.hash()), 2);
    let selecting = crate::test_utils::torii_proof_finality_for_block_with_context(
        &second,
        network,
        Some(&first_finality),
        |context| {
            context.mode = iroha_data_model::block::consensus_v2::ConsensusMode::Npos;
            context.epoch_end_height = 2;
            context.kagemusha_mint_finality_authorization.last_height = 2;
            let mut preparing = context.kagemusha_mint_finality_authorization;
            preparing.epoch = 1;
            preparing.first_height = 3;
            preparing.last_height = 4;
            preparing.previous_authorization_id = context
                .kagemusha_mint_finality_authorization
                .authorization_id()
                .unwrap();
            preparing.decision = KagemushaMintFinalityEpochDecisionV1::Retain;
            preparing.beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                session_id: [0xA1; 32],
                transcript_hash: [0xA2; 32],
            });
            let preparation = ValidatorCommitteePreparationV1 {
                version: 1,
                network_id: network,
                selection_epoch: 0,
                selection_height: 2,
                selection_anchor: first.hash(),
                target_epoch: 2,
                first_height: 5,
                last_height: 6,
                authority_generation: 1,
                preparing_authorization_id: preparing.authorization_id().unwrap(),
                election_seed: [0xA3; 32],
                roster: context.roster.clone(),
                validator_set_pops: first_finality.validator_set_pops.clone(),
            };
            context.next_epoch_snapshot = Some(FinalizedNextEpochSnapshot {
                committee_preparation: Some(preparation),
                epoch: 1,
                epoch_end_height: 4,
                kagemusha_mint_finality_authority: context
                    .kagemusha_mint_finality_authority
                    .clone(),
                kagemusha_mint_finality_authorization: preparing,
                mode: context.mode,
                roster: context.roster.clone(),
                quorum: context.quorum,
                validator_set_pops: first_finality.validator_set_pops.clone(),
                leader_seed: [0xA4; 32],
            });
        },
    );
    let preparation = selecting
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .unwrap()
        .committee_preparation
        .clone()
        .unwrap();
    let selected = ValidatorCommitteeSelectionStatusV1 {
        transition: ValidatorCommitteeTransitionV1 {
            preparation,
            credentials: None,
            readiness: Vec::new(),
            outcome: None,
        },
        selecting_finality: selecting.clone(),
    };
    crate::validator_committee::validate_selection(&selected, &selecting, 2).unwrap();
    for mutation in 0..5 {
        let mut changed = selected.clone();
        match mutation {
            0 => {
                changed
                    .selecting_finality
                    .height_context
                    .next_epoch_snapshot = None
            }
            1 => changed.transition.preparation.election_seed[0] ^= 1,
            2 => {
                changed.transition.preparation.selection_anchor =
                    HashOf::from_untyped_unchecked(Hash::new(b"foreign anchor"))
            }
            3 => changed.selecting_finality.height += 1,
            _ => changed.transition.preparation.preparing_authorization_id[0] ^= 1,
        }
        assert!(crate::validator_committee::validate_selection(&changed, &selecting, 2).is_err());
    }
    assert!(crate::validator_committee::validate_selection(&selected, &selecting, 3).is_err());
    let status = ValidatorCommitteeStatusV1 {
        network_id: network,
        target_epoch: 2,
        latest_finality: selecting,
        selected: Some(selected),
        candidate_keys: vec![],
        pending_beacon_session: None,
    };
    let binary = norito::to_bytes(&status).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<ValidatorCommitteeStatusV1>(&binary).unwrap(),
        status
    );
    let json = norito::json::to_vec(&status).unwrap();
    assert_eq!(
        norito::json::from_slice::<ValidatorCommitteeStatusV1>(&json).unwrap(),
        status
    );
    let mut missing = norito::json::to_value(&status).unwrap();
    missing
        .as_object_mut()
        .unwrap()
        .remove("pending_beacon_session");
    assert!(norito::json::from_value::<ValidatorCommitteeStatusV1>(missing).is_err());
}

#[cfg(all(test, feature = "telemetry"))]
mod tests {
    use super::{sorafs_capacity_tests::build_por_challenge, *};
    use crate::mk_app_state_for_tests;
    use http::StatusCode;
    use http_body_util::BodyExt;
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, state::World, sumeragi::status,
        telemetry::StateTelemetry,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf};
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus_v2::{
                ConsensusMode, DualQuorum, HeightContext, HeightContextId, PROTOCOL_VERSION,
                SumeragiV2BodyState, SumeragiV2HeightContextStatus, SumeragiV2Status,
                SumeragiV2StatusPhase,
            },
        },
        events::{
            EventBox,
            pipeline::{BlockEvent, BlockStatus},
        },
        metadata::Metadata,
        sorafs::capacity::ProviderId,
    };
    use iroha_telemetry::metrics::{
        Metrics, MicropaymentCreditSnapshot, MicropaymentSampleStatus, MicropaymentTicketCounters,
        NexusStatus,
    };
    use std::{
        io::Cursor,
        sync::{Arc, Mutex},
    };
    use tokio::runtime::Runtime;
    static SUMERAGI_V2_STATUS_TEST_LOCK: Mutex<()> = Mutex::new(());
    fn install_passive_diagnostic_lane_artifact(
        state: &CoreState,
        kura: &Kura,
    ) -> iroha_data_model::block::consensus::LaneBlockProposalV1 {
        use iroha_data_model::{
            block::{
                BlockExecutionContextBundle, SignedBlock,
                consensus::{
                    LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1, LaneBlockProposalV1,
                    SumeragiLanePayloadOwnership,
                },
            },
            consensus::VALIDATOR_SET_HASH_VERSION_V1,
            nexus::{DataSpaceId, LaneId},
            peer::PeerId,
        };
        let block_signer = checked_routing_fixture_keypair(
            0xe2,
            Algorithm::Ed25519,
            "derive passive diagnostic block signer",
        );
        let mut block: SignedBlock =
            iroha_core::block::BlockBuilder::new(vec![dummy_accepted_transaction()])
                .chain(0, None)
                .sign(block_signer.private_key())
                .unpack(|_| {})
                .into();
        let entrypoint_hashes = block
            .external_entrypoints_cloned()
            .map(|entrypoint| entrypoint.hash())
            .collect::<Vec<_>>();
        let accepted_transaction_hashes = entrypoint_hashes
            .iter()
            .copied()
            .map(Hash::from)
            .collect::<Vec<_>>();
        let accepted_candidate_indices = (0..accepted_transaction_hashes.len())
            .map(|index| u64::try_from(index).expect("diagnostic fixture index fits u64"))
            .collect::<Vec<_>>();
        let validator = checked_routing_fixture_keypair(
            0xe3,
            Algorithm::Ed25519,
            "derive passive diagnostic lane validator",
        );
        let validator_set = vec![PeerId::new(validator.public_key().clone())];
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let lane_incarnation = state
            .lane_incarnation(lane_id)
            .expect("default diagnostic lane incarnation");
        let mut ownership = SumeragiLanePayloadOwnership {
            proposal_height: block.header().height().get(),
            proposal_view: block.header().view_change_index(),
            lane_id,
            dataspace_id,
            lane_incarnation,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::prehashed([0; Hash::LENGTH]),
            qc_mode_tag: "permissioned:torii-passive-diagnostics".to_owned(),
            accepted_candidate_indices: accepted_candidate_indices.clone(),
            accepted_transaction_hashes: accepted_transaction_hashes.clone(),
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(Hash::new(b"passive diagnostic placeholder")),
            lane_block_descriptor_validator_set: validator_set.clone(),
            lane_block_descriptor_validator_count: 1,
            lane_block_descriptor_min_quorum: 1,
            payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        let replay = ownership
            .compute_replay_hashes()
            .expect("passive diagnostic ownership replay hashes");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        let descriptor = LaneBlockDescriptorV1 {
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height: ownership.proposal_height,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: ownership.lane_block_view,
            subject_hash: ownership.subject_hash,
            payload_ownership_hash: ownership.payload_ownership_hash,
            rbc_instance_hash: ownership.rbc_instance_hash,
            accepted_candidate_indices,
            accepted_transaction_hashes,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: ownership.qc_mode_tag.clone(),
            descriptor_hash: replay.lane_block_descriptor_hash,
        };
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: Some(LaneBlockProposalPayloadHintV1 {
                proposal_height: ownership.proposal_height,
                proposal_view: ownership.proposal_view,
                proposal_block_hash: block.hash(),
            }),
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        block.set_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership]),
        ));
        kura.store_block(Arc::new(block))
            .expect("store passive diagnostic lane artifact");
        proposal
    }
    #[test]
    fn openapi_handler_emits_alias_spec() {
        Runtime::new().expect("runtime").block_on(async {
            let app = crate::mk_app_state_for_tests();
            let response = super::handler_openapi_spec(State(app)).await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = response
                .into_body()
                .collect()
                .await
                .expect("collect body")
                .to_bytes();
            let doc: norito::json::Value =
                norito::json::from_slice(body.as_ref()).expect("decode openapi spec");
            let paths = doc
                .get("paths")
                .and_then(norito::json::Value::as_object)
                .expect("paths section");
            assert!(!paths.contains_key("/v1/aliases/voprf/evaluate"));
            assert!(paths.contains_key("/v1/aliases/resolve"));
            assert!(paths.contains_key("/v1/aliases/resolve-index"));
        });
    }
    #[test]
    fn status_tail_accesses_field() {
        let policy = ActualLaneRoutingPolicy {
            default_lane: LaneId::new(0),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![iroha_config::parameters::actual::LaneRoutingRule {
                lane: LaneId::new(3),
                dataspace: Some(DataSpaceId::new(6647857470246403404)),
                matcher: iroha_config::parameters::actual::LaneRoutingMatcher {
                    account: None,
                    instruction: Some("smartcontract::deploy".into()),
                    description: Some("Route contract deployments to private is".into()),
                },
            }],
        };
        let metrics = Metrics::default();
        let mut status = Status::from(&metrics);
        status.nexus = Some(NexusStatus::from_routing_policy(&policy));
        status.observed_at_ms = 1_000;
        status.queue_queued = 3;
        status.queue_inflight = 2;
        status.last_block_committed_at_ms = 900;
        status.last_non_empty_block_committed_at_ms = 800;
        status.time_since_last_block_ms = 100;
        status.time_since_last_non_empty_block_ms = 200;
        status.last_rejection_at_ms = Some(1_234);
        status.txs_rejected_recent_5m = 7;
        status.sorafs_micropayments = vec![MicropaymentSampleStatus {
            provider_id_hex: "feed".into(),
            credits: MicropaymentCreditSnapshot {
                deterministic_charge: 3_u64.into(),
                credit_generated: 2_u64.into(),
                credit_applied: 1_u64.into(),
                credit_carry: 0_u64.into(),
                outstanding: 7_u64.into(),
            },
            tickets: MicropaymentTicketCounters {
                processed: 4,
                won: 1,
                duplicate: 0,
            },
        }];
        let peers = status_value_by_path(&status, "peers").unwrap();
        assert_eq!(peers, json_value(&0u64));
        let observed = status_value_by_path(&status, "observed_at_ms").unwrap();
        assert_eq!(observed, json_value(&1_000u64));
        let queued = status_value_by_path(&status, "queue_queued").unwrap();
        assert_eq!(queued, json_value(&3u64));
        let inflight = status_value_by_path(&status, "queue_inflight").unwrap();
        assert_eq!(inflight, json_value(&2u64));
        let last_block = status_value_by_path(&status, "last_block_committed_at_ms").unwrap();
        assert_eq!(last_block, json_value(&900u64));
        let last_non_empty =
            status_value_by_path(&status, "last_non_empty_block_committed_at_ms").unwrap();
        assert_eq!(last_non_empty, json_value(&800u64));
        let since_block = status_value_by_path(&status, "time_since_last_block_ms").unwrap();
        assert_eq!(since_block, json_value(&100u64));
        let since_non_empty =
            status_value_by_path(&status, "time_since_last_non_empty_block_ms").unwrap();
        assert_eq!(since_non_empty, json_value(&200u64));
        let last_rejection = status_value_by_path(&status, "last_rejection_at_ms").unwrap();
        assert_eq!(last_rejection, json_value(&Some(1_234u64)));
        let rejected_recent = status_value_by_path(&status, "txs_rejected_recent_5m").unwrap();
        assert_eq!(rejected_recent, json_value(&7u64));
        let secs = status_value_by_path(&status, "uptime/secs").unwrap();
        assert_eq!(secs, json_value(&0u64));
        let crypto = status_value_by_path(&status, "crypto").unwrap();
        assert!(crypto.is_object());
        let sm_helpers = status_value_by_path(&status, "crypto/sm_helpers_available").unwrap();
        assert_eq!(sm_helpers, json_value(&cfg!(feature = "sm")));
        let sm_preview =
            status_value_by_path(&status, "crypto/sm_openssl_preview_enabled").unwrap();
        assert_eq!(sm_preview, json_value(&false));
        let governance = status_value_by_path(&status, "governance").unwrap();
        assert!(governance.is_object());
        let nexus = status_value_by_path(&status, "nexus").unwrap();
        assert!(nexus.is_object());
        let policy = status_value_by_path(&status, "nexus/routing_policy").unwrap();
        assert!(policy.is_object());
        let rules = status_value_by_path(&status, "nexus/routing_policy/rules").unwrap();
        let rules = rules.as_array().expect("routing rules array");
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0]
                .get("matcher")
                .and_then(|matcher| matcher.get("instruction"))
                .and_then(norito::json::Value::as_str),
            Some("smartcontract::deploy")
        );
        let micropayments = status_value_by_path(&status, "sorafs_micropayments").unwrap();
        assert!(micropayments.is_array());
        let sample = status_value_by_path(&status, "sorafs_micropayments/feed").unwrap();
        assert!(sample.is_object());
        let outstanding =
            status_value_by_path(&status, "sorafs_micropayments/feed/credits/outstanding").unwrap();
        assert_eq!(outstanding, json_value(&Quantity::from(7_u64)));
        assert!(status_value_by_path(&status, "sorafs_micropayments/unknown").is_none());
    }
    #[tokio::test]
    async fn status_response_bounds_unavailable_fresh_block_counter_sync() {
        let metrics = Arc::new(Metrics::default());
        metrics.block_height.inc_by(4_193);
        let telemetry = MaybeTelemetry::from_profile(
            Some(Telemetry::new(metrics, true)),
            TelemetryProfile::Full,
        );
        let error = super::handle_status(
            &telemetry,
            Some(axum::http::HeaderValue::from_static("application/json")),
            None,
            ActualLaneRoutingPolicy::default(),
            4_274,
            None,
        )
        .await
        .expect_err("an unavailable telemetry actor must fail status retriably");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "status_metrics_unavailable",
                ..
            }
        ));
    }
    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn status_tail_returns_micropayment_sample() {
        use http_body_util::BodyExt;
        let telemetry = MaybeTelemetry::for_tests();
        let provider_hex = "feedcafe";
        telemetry.with_metrics(|tel| {
            tel.record_sorafs_micropayment_sample(
                provider_hex,
                MicropaymentCreditSnapshot {
                    deterministic_charge: 11_u64.into(),
                    credit_generated: 5_u64.into(),
                    credit_applied: 3_u64.into(),
                    credit_carry: 2_u64.into(),
                    outstanding: 6_u64.into(),
                },
                MicropaymentTicketCounters {
                    processed: 9,
                    won: 2,
                    duplicate: 1,
                },
            );
        });
        let path = format!("sorafs_micropayments/{provider_hex}");
        let response = super::handle_status(
            &telemetry,
            None,
            Some(&path),
            ActualLaneRoutingPolicy::default(),
            0,
            None,
        )
        .await
        .expect("status tail succeeds");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("decode status payload");
        assert_eq!(
            payload
                .get("provider_id_hex")
                .and_then(norito::json::Value::as_str),
            Some(provider_hex)
        );
        assert_eq!(
            payload
                .get("credits")
                .and_then(|credits| credits.get("outstanding"))
                .and_then(norito::json::Value::as_u64),
            Some(6)
        );
        assert_eq!(
            payload
                .get("tickets")
                .and_then(|tickets| tickets.get("won"))
                .and_then(norito::json::Value::as_u64),
            Some(2)
        );
    }
    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn status_root_includes_effective_nexus_routing_policy() {
        use http_body_util::BodyExt;
        let telemetry = MaybeTelemetry::for_tests();
        let policy = ActualLaneRoutingPolicy {
            default_lane: LaneId::new(0),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![iroha_config::parameters::actual::LaneRoutingRule {
                lane: LaneId::new(3),
                dataspace: Some(DataSpaceId::new(6647857470246403404)),
                matcher: iroha_config::parameters::actual::LaneRoutingMatcher {
                    account: None,
                    instruction: Some("smartcontract::deploy".into()),
                    description: Some("Route contract deployments to private is".into()),
                },
            }],
        };
        let response = super::handle_status(
            &telemetry,
            Some(axum::http::HeaderValue::from_static("application/json")),
            None,
            policy,
            0,
            None,
        )
        .await
        .expect("status succeeds");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("decode status payload");
        let rules = payload
            .get("nexus")
            .and_then(|nexus| nexus.get("routing_policy"))
            .and_then(|routing| routing.get("rules"))
            .and_then(norito::json::Value::as_array)
            .expect("routing rules");
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0].get("lane").and_then(norito::json::Value::as_u64),
            Some(3)
        );
        assert_eq!(
            rules[0]
                .get("dataspace_id")
                .and_then(norito::json::Value::as_u64),
            Some(6647857470246403404)
        );
        assert_eq!(
            rules[0]
                .get("matcher")
                .and_then(|matcher| matcher.get("instruction"))
                .and_then(norito::json::Value::as_str),
            Some("smartcontract::deploy")
        );
    }
    #[tokio::test]
    async fn status_root_and_tail_include_universal_offline_capability() {
        use http_body_util::BodyExt;
        use iroha_torii_shared::offline_api::OfflineStatus;
        let telemetry = MaybeTelemetry::for_tests();
        let offline = OfflineStatus {
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            required_bridge_abi_version: 23,
            max_hops: 8,
            ready: true,
        };
        let response = super::handle_status(
            &telemetry,
            Some(axum::http::HeaderValue::from_static("application/json")),
            None,
            ActualLaneRoutingPolicy::default(),
            0,
            Some(offline.clone()),
        )
        .await
        .expect("status succeeds");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect status body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("decode status payload");
        let projected = payload.get("offline").expect("offline projection");
        assert_eq!(
            projected
                .get("cash_handoff_capability")
                .and_then(norito::json::Value::as_str),
            Some("cash_handoff_v1")
        );
        assert_eq!(
            projected
                .get("required_bridge_abi_version")
                .and_then(norito::json::Value::as_u64),
            Some(22)
        );
        let response = super::handle_status(
            &telemetry,
            None,
            Some("offline/cash_handoff_capability"),
            ActualLaneRoutingPolicy::default(),
            0,
            Some(offline),
        )
        .await
        .expect("offline status tail succeeds");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect status tail")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("decode status tail");
        assert_eq!(payload.as_str(), Some("cash_handoff_v1"));
    }
    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn metrics_handler_exports_lane_labels() {
        let telemetry = MaybeTelemetry::for_tests();
        telemetry
            .metrics()
            .await
            .set_lane_block_height("lane-0", "global", 3);
        let rendered = super::handle_metrics(&telemetry)
            .await
            .expect("metrics should render");
        assert!(
            rendered.contains("nexus_lane_block_height"),
            "lane metrics are part of every first-release Nexus exposition"
        );
    }
    #[tokio::test]
    async fn sumeragi_status_fails_closed_before_v2_replay() {
        let _guard = SUMERAGI_V2_STATUS_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status::clear_v2_status();
        let state = std::sync::Arc::new(CoreState::new_for_testing(
            iroha_core::state::World::default(),
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        ));
        let response = super::handle_v1_sumeragi_status(axum::extract::State(state), None, false)
            .await
            .expect("status handler");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
    #[tokio::test]
    async fn sumeragi_status_json_is_exact_authoritative_v2_schema() {
        let _guard = SUMERAGI_V2_STATUS_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let expected = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"node"),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                Hash::new(b"height-context"),
            )),
            height: 42,
            view: 3,
            phase: SumeragiV2StatusPhase::Prepare,
            leader: 2,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Validated,
            pending_persistence_id: Some(17),
            last_committed_height: 41,
            last_committed_subject: None,
            height_context: SumeragiV2HeightContextStatus {
                epoch: 1,
                epoch_end_height: 100,
                mode: ConsensusMode::Permissioned,
                epoch_seed: [0xA5; 32],
                validator_count: 4,
                quorum: DualQuorum {
                    min_signers: 3,
                    total_power: 4,
                },
            },
            last_commit_qc: None,
            liveness: Default::default(),
        };
        status::set_v2_status(expected.clone());
        let state = std::sync::Arc::new(CoreState::new_for_testing(
            iroha_core::state::World::default(),
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        ));
        let response = super::handle_v1_sumeragi_status(
            axum::extract::State(std::sync::Arc::clone(&state)),
            Some(axum::http::HeaderValue::from_static("application/json")),
            false,
        )
        .await
        .expect("status handler");
        // Simulate Kura/snapshot activating the shared process output guard
        // after the reducer's last publication. Serving must monotonically
        // overlay that state without waiting for another reducer event.
        let restart_response = super::handle_v1_sumeragi_status(
            axum::extract::State(state),
            Some(axum::http::HeaderValue::from_static("application/json")),
            true,
        )
        .await
        .expect("restart-required status handler");
        status::clear_v2_status();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect status body")
            .to_bytes();
        let decoded: SumeragiV2Status =
            norito::json::from_slice(&body).expect("decode authoritative v2 status");
        let mut expected_at_first_read = expected.clone();
        expected_at_first_read.liveness.no_progress_age_ms = decoded.liveness.no_progress_age_ms;
        assert_eq!(decoded, expected_at_first_read);
        let restart_body = restart_response
            .into_body()
            .collect()
            .await
            .expect("collect restart-required status body")
            .to_bytes();
        let restart_decoded: SumeragiV2Status = norito::json::from_slice(&restart_body)
            .expect("decode restart-required authoritative status");
        assert!(
            restart_decoded.liveness.no_progress_age_ms >= decoded.liveness.no_progress_age_ms,
            "read-time liveness age must be monotonic"
        );
        let mut expected_at_restart_read = expected;
        expected_at_restart_read.restart_required = true;
        expected_at_restart_read.liveness.no_progress_age_ms =
            restart_decoded.liveness.no_progress_age_ms;
        assert_eq!(restart_decoded, expected_at_restart_read);
        assert_eq!(
            status::v2_status(),
            None,
            "test cleanup must clear the slot"
        );
        let json: norito::json::Value =
            norito::json::from_slice(&body).expect("decode status JSON object");
        for retired in [
            "canonical",
            "rbc_status",
            "missing_qc_total",
            "consensus_missing_qc_reacquire_attempt_total",
            "lane_settlement_commitments",
            "lane_relay_envelopes",
            "native_amx_participant_applications",
            "autonomous_lane_executions",
        ] {
            assert!(
                json.get(retired).is_none(),
                "retired field {retired} leaked"
            );
        }
    }
    #[tokio::test]
    async fn permissioned_sumeragi_diagnostics_omit_npos_and_canonical_state() {
        let kura = Kura::blank_kura_for_testing();
        let state = std::sync::Arc::new(CoreState::new_for_testing(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        ));
        let response = super::handle_v1_sumeragi_diagnostics(
            axum::extract::State(Arc::clone(&state)),
            None,
            Some(axum::http::HeaderValue::from_static("application/json")),
        )
        .await
        .expect("diagnostics handler");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect diagnostics body")
            .to_bytes();
        let decoded: SumeragiDiagnosticsStatus =
            norito::json::from_slice(&body).expect("decode diagnostics");
        assert!(decoded.npos.is_none());
        assert!(decoded.lane_commitments.is_empty());
        assert!(decoded.lane_relay_envelopes.is_empty());
        assert!(decoded.native_amx_participant_applications.is_empty());
        assert!(decoded.autonomous_lane_executions.is_empty());
        let json: norito::json::Value =
            norito::json::from_slice(&body).expect("decode diagnostics JSON object");
        assert!(json.get("npos").is_none());
        assert_eq!(
            json.get("native_amx_participant_applications")
                .and_then(|value| value.as_array())
                .map(|rows| rows.len()),
            Some(0),
            "diagnostics expose the durable Native AMX evidence vector independently of status"
        );
        assert_eq!(
            json.get("autonomous_lane_executions")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0),
            "autonomous stage evidence belongs only to the diagnostics endpoint"
        );
        for canonical in ["height", "view", "phase", "leader", "locked_prepare_qc"] {
            assert!(
                json.get(canonical).is_none(),
                "leaked canonical field {canonical}"
            );
        }
        let proposal = install_passive_diagnostic_lane_artifact(&state, &kura);
        let lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(
            &iroha_data_model::nexus::LaneCatalog::default(),
        );
        let lane_artifact_dir = lane_config
            .entry(proposal.descriptor.lane_id)
            .expect("Torii diagnostic lane entry")
            .blocks_dir(kura.store_root())
            .join("lane_artifacts");
        let ownership_data = lane_artifact_dir.join("ownerships.norito");
        let ownership_index = lane_artifact_dir.join("ownerships.index");
        let ownership_data_temp = ownership_data.with_extension("norito.tmp");
        let ownership_index_temp = ownership_index.with_extension("index.tmp");
        std::fs::rename(&ownership_data, &ownership_data_temp)
            .expect("stage Torii diagnostic ownership data");
        std::fs::rename(&ownership_index, &ownership_index_temp)
            .expect("stage Torii diagnostic ownership index");
        let staged_data =
            std::fs::read(&ownership_data_temp).expect("read staged Torii ownership data");
        let staged_index =
            std::fs::read(&ownership_index_temp).expect("read staged Torii ownership index");
        for _ in 0..2 {
            let response = super::handle_v1_sumeragi_diagnostics(
                axum::extract::State(Arc::clone(&state)),
                None,
                None,
            )
            .await
            .expect("passive diagnostics handler");
            assert_eq!(response.status(), StatusCode::OK);
        }
        assert!(!ownership_data.exists());
        assert!(!ownership_index.exists());
        assert_eq!(
            std::fs::read(&ownership_data_temp).expect("reread staged Torii ownership data"),
            staged_data,
        );
        assert_eq!(
            std::fs::read(&ownership_index_temp).expect("reread staged Torii ownership index"),
            staged_index,
        );
        kura.recover_lane_block_payload(&proposal)
            .expect("explicitly recover Torii diagnostic ownership evidence");
        assert!(ownership_data.is_file());
        assert!(ownership_index.is_file());
        assert!(!ownership_data_temp.exists());
        assert!(!ownership_index_temp.exists());
    }
    #[test]
    fn malformed_npos_diagnostics_are_rejected() {
        let reducer = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"node"),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                Hash::new(b"height-context"),
            )),
            height: 42,
            view: 3,
            phase: SumeragiV2StatusPhase::Prepare,
            leader: 2,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Validated,
            pending_persistence_id: None,
            last_committed_height: 41,
            last_committed_subject: None,
            height_context: SumeragiV2HeightContextStatus {
                epoch: 1,
                epoch_end_height: 100,
                mode: ConsensusMode::Permissioned,
                epoch_seed: [0xA5; 32],
                validator_count: 4,
                quorum: DualQuorum {
                    min_signers: 3,
                    total_power: 4,
                },
            },
            last_commit_qc: None,
            liveness: Default::default(),
        };
        let zero_seed = iroha_data_model::parameter::system::SumeragiNposParameters {
            epoch_seed: [0; 32],
            ..Default::default()
        };
        assert!(super::sumeragi_npos_diagnostics(&zero_seed, &reducer).is_err());
        let invalid_windows = iroha_data_model::parameter::system::SumeragiNposParameters {
            epoch_length_blocks: NonZeroU64::new(10).expect("non-zero epoch length"),
            vrf_commit_window_blocks: 8,
            vrf_reveal_window_blocks: 4,
            ..Default::default()
        };
        assert!(super::sumeragi_npos_diagnostics(&invalid_windows, &reducer).is_err());
    }
    #[tokio::test]
    async fn status_accept_header_returns_codec_norito() {
        let telemetry = MaybeTelemetry::for_tests();
        let expected = Status::from(telemetry.metrics().await);
        let response = super::handle_status(
            &telemetry,
            Some(axum::http::HeaderValue::from_static(
                crate::utils::NORITO_MIME_TYPE,
            )),
            None,
            ActualLaneRoutingPolicy::default(),
            0,
            None,
        )
        .await
        .expect("status handler");
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&axum::http::HeaderValue::from_static(
                crate::utils::NORITO_MIME_TYPE
            ))
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        let decoded: Status = norito::decode_from_bytes(&body).expect("decode Norito status");
        assert_eq!(decoded.blocks, expected.blocks);
        assert_eq!(decoded.blocks_non_empty, expected.blocks_non_empty);
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn committed_block_height_detects_commits() {
        use std::num::NonZeroU64;
        let header = BlockHeader {
            height: NonZeroU64::new(7).unwrap(),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            npos_effects_hash: None,
            sccp_commitment_root: None,
            execution_context_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let committed: EventBox = BlockEvent {
            header,
            status: BlockStatus::Committed,
        }
        .into();
        assert_eq!(super::committed_block_height(&committed), Some(7));
        let committed_batch = EventBox::PipelineBatch(vec![PipelineEventBox::from(BlockEvent {
            header: BlockHeader {
                height: NonZeroU64::new(7).unwrap(),
                prev_block_hash: None,
                merkle_root: None,
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                npos_effects_hash: None,
                sccp_commitment_root: None,
                execution_context_hash: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            },
            status: BlockStatus::Committed,
        })]);
        assert_eq!(super::committed_block_height(&committed_batch), Some(7));
        let created_header = BlockHeader {
            height: NonZeroU64::new(3).unwrap(),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            npos_effects_hash: None,
            sccp_commitment_root: None,
            execution_context_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let created: EventBox = BlockEvent {
            header: created_header,
            status: BlockStatus::Created,
        }
        .into();
        assert!(super::committed_block_height(&created).is_none());
        let created_batch = EventBox::PipelineBatch(vec![PipelineEventBox::from(BlockEvent {
            header: BlockHeader {
                height: NonZeroU64::new(3).unwrap(),
                prev_block_hash: None,
                merkle_root: None,
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                npos_effects_hash: None,
                sccp_commitment_root: None,
                execution_context_hash: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            },
            status: BlockStatus::Created,
        })]);
        assert!(super::committed_block_height(&created_batch).is_none());
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn average_block_time_handles_empty_chain() {
        let kura = iroha_core::kura::Kura::blank_kura_for_testing();
        assert!(super::average_block_time_ms(&kura, 0, 10).is_none());
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn latest_block_created_at_missing_when_height_zero() {
        let kura = iroha_core::kura::Kura::blank_kura_for_testing();
        assert!(super::latest_block_created_at(&kura, 0).is_none());
    }
    #[cfg(feature = "app_api")]
    #[test]
    fn por_status_export_and_report_handlers() {
        let coordinator = std::sync::Arc::new(crate::sorafs::PorCoordinator::new());
        let provider_a = [0xAB; 32];
        let provider_b = [0xBC; 32];
        let challenge_a = build_por_challenge(0x10, provider_a, 540, 1_672_620_000);
        let challenge_b = build_por_challenge(0x20, provider_b, 880, 1_700_000_000);
        coordinator
            .record_challenge(&challenge_a)
            .expect("first challenge recorded");
        coordinator
            .record_challenge(&challenge_b)
            .expect("second challenge recorded");
        let status_query = PorStatusQueryDto {
            manifest: Some(hex::encode(challenge_a.manifest_digest)),
            provider: Some(hex::encode(challenge_a.provider_id)),
            epoch: Some(challenge_a.epoch_id),
            status: Some("pending".to_string()),
            limit: 5,
            max_bytes: POR_STATUS_PAGE_MAX_CANONICAL_BYTES_V1,
            cursor: None,
        };
        let status_page = super::handle_get_sorafs_por_status(coordinator.clone(), status_query)
            .expect("status handler responds");
        assert_eq!(status_page.statuses.len(), 1);
        assert_eq!(
            status_page.statuses[0].challenge_id,
            challenge_a.challenge_id
        );
        let oversized_status_query = PorStatusQueryDto {
            manifest: None,
            provider: None,
            epoch: None,
            status: None,
            limit: POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 + 1,
            max_bytes: POR_STATUS_PAGE_MAX_CANONICAL_BYTES_V1,
            cursor: None,
        };
        assert!(
            super::handle_get_sorafs_por_status(coordinator.clone(), oversized_status_query)
                .is_err()
        );
        let export = super::handle_get_sorafs_por_export(
            coordinator.clone(),
            PorExportQueryDto {
                start_epoch: Some(challenge_a.epoch_id),
                end_epoch: Some(challenge_a.epoch_id),
                limit: 5,
                max_bytes: POR_STATUS_PAGE_MAX_CANONICAL_BYTES_V1,
                cursor: None,
            },
        )
        .expect("export handler responds");
        assert_eq!(export.page.statuses.len(), 1);
        assert_eq!(
            export.page.statuses[0].challenge_id,
            challenge_a.challenge_id
        );
        let invalid_report_response = super::handle_get_sorafs_por_report(
            coordinator.clone(),
            PorReportIsoWeek {
                year: 9999,
                week: 52,
            },
        )
        .expect_err("an ISO week whose end is not representable must be rejected")
        .into_response();
        assert_eq!(
            invalid_report_response.status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        let report = super::handle_get_sorafs_por_report(
            coordinator.clone(),
            PorReportIsoWeek {
                year: 2023,
                week: 1,
            },
        )
        .expect("report handler responds");
        assert!(
            report.challenges_total >= 1,
            "weekly report must include the recorded challenge"
        );
    }
}
#[cfg(feature = "profiling")]
pub mod profiling {
    use super::*;
    use nonzero_ext::nonzero;
    use pprof::protos::Message;
    use std::num::{NonZeroU16, NonZeroU64};
    /// Query params used to configure profile gathering
    #[allow(clippy::unsafe_derive_deserialize)]
    #[derive(
        crate::json_macros::JsonSerialize,
        norito::derive::NoritoSerialize,
        crate::json_macros::JsonDeserialize,
        norito::derive::NoritoDeserialize,
        Clone,
        Copy,
    )]
    pub struct ProfileParams {
        /// How often to sample Iroha
        #[norito(default = "ProfileParams::default_frequency")]
        frequency: NonZeroU16,
        /// How long to sample Iroha
        #[norito(default = "ProfileParams::default_seconds")]
        seconds: NonZeroU64,
    }
    impl ProfileParams {
        fn default_frequency() -> NonZeroU16 {
            nonzero!(99_u16)
        }
        fn default_seconds() -> NonZeroU64 {
            nonzero!(10_u64)
        }
    }
    /// Serve pprof profile data
    pub async fn handle_profile(
        ProfileParams { frequency, seconds }: ProfileParams,
        profiling_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    ) -> Result<Vec<u8>> {
        match profiling_lock.try_lock() {
            Ok(_guard) => {
                let mut body = Vec::new();
                {
                    // Create profiler guard
                    let guard = pprof::ProfilerGuardBuilder::default()
                        .frequency(i32::from(frequency.get()))
                        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
                        .build()
                        .map_err(|e| {
                            Error::Pprof(eyre::eyre!(
                                "pprof::ProfilerGuardBuilder::build fail: {}",
                                e
                            ))
                        })?;
                    // Collect profiles for seconds
                    tokio::time::sleep(tokio::time::Duration::from_secs(seconds.get())).await;
                    let report = guard
                        .report()
                        .build()
                        .map_err(|e| Error::Pprof(eyre::eyre!("generate report fail: {}", e)))?;
                    let profile = report.pprof().map_err(|e| {
                        Error::Pprof(eyre::eyre!("generate pprof from report fail: {}", e))
                    })?;
                    profile.encode(&mut body).map_err(|e| {
                        Error::Pprof(eyre::eyre!("encode pprof into bytes fail: {}", e))
                    })?;
                }
                Ok(body)
            }
            Err(_) => {
                // profile already running return error
                Err(Error::Pprof(eyre::eyre!("profiling already running")))
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[tokio::test]
        async fn profiling_encodes_pprof_payload() {
            let lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
            let params = ProfileParams {
                frequency: nonzero!(99_u16),
                seconds: nonzero!(1_u64),
            };
            let payload = handle_profile(params, lock).await.expect("profile payload");
            assert!(!payload.is_empty(), "pprof payload should not be empty");
        }
    }
}
#[cfg(all(test, feature = "ws_integration_tests"))]
mod event_stream_tests {
    use super::event::handle_events_stream_with_receiver;
    use axum::{Router, extract::ws::WebSocketUpgrade, routing::get};
    use futures_util::{SinkExt as _, StreamExt as _};
    use iroha_core::EventsSender;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        events::{
            EventBox, EventFilterBox,
            pipeline::{
                PipelineEventBox, TransactionEvent, TransactionEventFilter, TransactionStatus,
            },
            stream::{EventMessage, EventSubscriptionRequest},
        },
        nexus::{DataSpaceId, LaneId},
        transaction::SignedTransaction,
    };
    use norito::{decode_from_bytes, to_bytes};
    use std::{io::ErrorKind, sync::Arc};
    use tokio::{net::TcpListener, sync::Mutex};
    async fn spawn_event_stream_server(
        receiver: tokio::sync::broadcast::Receiver<EventBox>,
    ) -> Option<std::net::SocketAddr> {
        let rx_holder = Arc::new(Mutex::new(Some(receiver)));
        let app = Router::new().route(
            "/ws",
            get({
                let rx_holder = Arc::clone(&rx_holder);
                move |ws: WebSocketUpgrade| {
                    let rx_holder = Arc::clone(&rx_holder);
                    async move {
                        ws.on_upgrade(move |ws| async move {
                            let mut guard = rx_holder.lock().await;
                            let rx = guard.take().expect("event receiver already used");
                            let _ = handle_events_stream_with_receiver(
                                rx,
                                ws,
                                std::time::Duration::from_millis(
                                    iroha_config::parameters::defaults::torii::WS_MESSAGE_TIMEOUT_MS,
                                ),
                            )
                            .await;
                        })
                    }
                }
            }),
        );
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return None,
            Err(err) => panic!("tcp bind failed: {err}"),
        };
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum server");
        });
        Some(addr)
    }
    async fn connect_event_stream(
        addr: std::net::SocketAddr,
    ) -> Option<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    > {
        match tokio_tungstenite::connect_async(format!("ws://{addr}/ws")).await {
            Ok((stream, _response)) => Some(stream),
            Err(tokio_tungstenite::tungstenite::Error::Io(io_err))
                if io_err.kind() == ErrorKind::PermissionDenied =>
            {
                None
            }
            Err(err) => panic!("ws connect failed: {err}"),
        }
    }
    async fn next_close_frame(
        stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> tokio_tungstenite::tungstenite::protocol::CloseFrame {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match stream.next().await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(Some(frame)))) => {
                        break frame;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => panic!("ws message error: {err}"),
                    None => panic!("ws stream closed without a close frame"),
                }
            }
        })
        .await
        .expect("timed out waiting for close frame")
    }
    #[tokio::test]
    async fn ws_stream_receives_buffered_events() {
        let events: EventsSender = tokio::sync::broadcast::channel(16).0;
        let hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x11; Hash::LENGTH],
        ));
        let other_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x22; Hash::LENGTH],
        ));
        let queued_event = EventBox::PipelineBatch(vec![
            PipelineEventBox::Transaction(TransactionEvent {
                hash,
                block_height: None,
                lane_id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(0),
                status: TransactionStatus::Queued,
            }),
            PipelineEventBox::Transaction(TransactionEvent {
                hash: other_hash,
                block_height: None,
                lane_id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(0),
                status: TransactionStatus::Queued,
            }),
        ]);
        let rx_holder = Arc::new(Mutex::new(Some(events.subscribe())));
        events
            .send(queued_event)
            .expect("receiver should be subscribed");
        let app = Router::new().route(
            "/ws",
            get({
                let rx_holder = Arc::clone(&rx_holder);
                move |ws: WebSocketUpgrade| {
                    let rx_holder = Arc::clone(&rx_holder);
                    async move {
                        ws.on_upgrade(move |ws| async move {
                            let mut guard = rx_holder.lock().await;
                            let rx = guard.take().expect("event receiver already used");
                            let _ = handle_events_stream_with_receiver(
                                rx,
                                ws,
                                std::time::Duration::from_millis(
                                    iroha_config::parameters::defaults::torii::WS_MESSAGE_TIMEOUT_MS,
                                ),
                            )
                            .await;
                        })
                    }
                }
            }),
        );
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
            Err(err) => panic!("tcp bind failed: {err}"),
        };
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum server");
        });
        let (mut ws_stream, _resp) =
            match tokio_tungstenite::connect_async(format!("ws://{addr}/ws")).await {
                Ok(pair) => pair,
                Err(tokio_tungstenite::tungstenite::Error::Io(io_err))
                    if io_err.kind() == ErrorKind::PermissionDenied =>
                {
                    return;
                }
                Err(err) => panic!("ws connect failed: {err}"),
            };
        let sub = EventSubscriptionRequest::new(vec![EventFilterBox::Pipeline(
            TransactionEventFilter::default().for_hash(hash).into(),
        )]);
        let sub_bytes = to_bytes(&sub).expect("encode subscription");
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                sub_bytes.into(),
            ))
            .await
            .expect("send subscription");
        let mut got_event = None;
        while let Some(msg) = ws_stream.next().await {
            let msg = msg.expect("ws message");
            if let tokio_tungstenite::tungstenite::Message::Binary(bytes) = msg {
                let event_msg: EventMessage =
                    decode_from_bytes(bytes.as_ref()).expect("decode event message");
                let event_box: EventBox = event_msg.into();
                if let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event_box {
                    got_event = Some(event);
                    break;
                }
            }
        }
        let event = got_event.expect("transaction event");
        assert_eq!(event.hash(), &hash);
        assert_eq!(event.status(), &TransactionStatus::Queued);
    }
    #[tokio::test]
    async fn ws_stream_reports_lag_and_closes() {
        let events: EventsSender = tokio::sync::broadcast::channel(1).0;
        let lagged_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x11; Hash::LENGTH],
        ));
        let wanted_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x22; Hash::LENGTH],
        ));
        let lagged_event = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
            hash: lagged_hash,
            block_height: None,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            status: TransactionStatus::Queued,
        }));
        let wanted_event = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
            hash: wanted_hash.clone(),
            block_height: None,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            status: TransactionStatus::Queued,
        }));
        let rx_holder = Arc::new(Mutex::new(Some(events.subscribe())));
        events
            .send(lagged_event)
            .expect("receiver should be subscribed");
        events
            .send(wanted_event)
            .expect("receiver should be subscribed");
        let app = Router::new().route(
            "/ws",
            get({
                let rx_holder = Arc::clone(&rx_holder);
                move |ws: WebSocketUpgrade| {
                    let rx_holder = Arc::clone(&rx_holder);
                    async move {
                        ws.on_upgrade(move |ws| async move {
                            let mut guard = rx_holder.lock().await;
                            let rx = guard.take().expect("event receiver already used");
                            let _ = handle_events_stream_with_receiver(
                                rx,
                                ws,
                                std::time::Duration::from_millis(
                                    iroha_config::parameters::defaults::torii::WS_MESSAGE_TIMEOUT_MS,
                                ),
                            )
                            .await;
                        })
                    }
                }
            }),
        );
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
            Err(err) => panic!("tcp bind failed: {err}"),
        };
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum server");
        });
        let (mut ws_stream, _resp) =
            match tokio_tungstenite::connect_async(format!("ws://{addr}/ws")).await {
                Ok(pair) => pair,
                Err(tokio_tungstenite::tungstenite::Error::Io(io_err))
                    if io_err.kind() == ErrorKind::PermissionDenied =>
                {
                    return;
                }
                Err(err) => panic!("ws connect failed: {err}"),
            };
        let sub = EventSubscriptionRequest::new(vec![EventFilterBox::Pipeline(
            TransactionEventFilter::default()
                .for_hash(wanted_hash.clone())
                .into(),
        )]);
        let sub_bytes = to_bytes(&sub).expect("encode subscription");
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                sub_bytes.into(),
            ))
            .await
            .expect("send subscription");
        let close = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match ws_stream.next().await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(Some(frame)))) => {
                        break frame;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => panic!("ws message error: {err}"),
                    None => panic!("ws stream closed without a close frame"),
                }
            }
        })
        .await
        .expect("timed out waiting for close frame");
        assert_eq!(u16::from(close.code), crate::stream::CLOSE_TRY_AGAIN_LATER);
        assert_eq!(close.reason, "event_stream_lagged:1");
    }
    #[tokio::test]
    async fn ws_stream_rejects_text_subscription_payload() {
        let events: EventsSender = tokio::sync::broadcast::channel(4).0;
        let Some(addr) = spawn_event_stream_server(events.subscribe()).await else {
            return;
        };
        let Some(mut ws_stream) = connect_event_stream(addr).await else {
            return;
        };
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                "not-norito".into(),
            ))
            .await
            .expect("send text subscription");
        let close = next_close_frame(&mut ws_stream).await;
        assert_eq!(u16::from(close.code), crate::stream::CLOSE_INVALID_PAYLOAD);
        assert_eq!(close.reason, "invalid_subscription_payload");
    }
    #[tokio::test]
    async fn ws_stream_rejects_empty_event_filter_set() {
        let events: EventsSender = tokio::sync::broadcast::channel(4).0;
        let Some(addr) = spawn_event_stream_server(events.subscribe()).await else {
            return;
        };
        let Some(mut ws_stream) = connect_event_stream(addr).await else {
            return;
        };
        let subscription = EventSubscriptionRequest::new(Vec::new());
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                to_bytes(&subscription)
                    .expect("encode empty subscription")
                    .into(),
            ))
            .await
            .expect("send empty subscription");
        let close = next_close_frame(&mut ws_stream).await;
        assert_eq!(u16::from(close.code), crate::stream::CLOSE_POLICY_VIOLATION);
        assert_eq!(close.reason, "invalid_event_subscription");
    }
    #[tokio::test]
    async fn ws_stream_rejects_data_after_subscription() {
        let events: EventsSender = tokio::sync::broadcast::channel(4).0;
        let Some(addr) = spawn_event_stream_server(events.subscribe()).await else {
            return;
        };
        let Some(mut ws_stream) = connect_event_stream(addr).await else {
            return;
        };
        let subscription = EventSubscriptionRequest::new(vec![EventFilterBox::Pipeline(
            TransactionEventFilter::default().into(),
        )]);
        let bytes = to_bytes(&subscription).expect("encode subscription");
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                bytes.clone().into(),
            ))
            .await
            .expect("send subscription");
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                bytes.into(),
            ))
            .await
            .expect("send unexpected second request");
        let close = next_close_frame(&mut ws_stream).await;
        assert_eq!(u16::from(close.code), crate::stream::CLOSE_INVALID_PAYLOAD);
        assert_eq!(close.reason, "invalid_subscription_payload");
    }
    #[tokio::test]
    async fn ws_stream_emits_transport_heartbeat() {
        let events: EventsSender = tokio::sync::broadcast::channel(4).0;
        let Some(addr) = spawn_event_stream_server(events.subscribe()).await else {
            return;
        };
        let Some(mut ws_stream) = connect_event_stream(addr).await else {
            return;
        };
        let subscription = EventSubscriptionRequest::new(vec![EventFilterBox::Pipeline(
            TransactionEventFilter::default().into(),
        )]);
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                to_bytes(&subscription).expect("encode subscription").into(),
            ))
            .await
            .expect("send subscription");
        let heartbeat = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                match ws_stream.next().await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(payload))) => {
                        break payload;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => panic!("ws heartbeat error: {err}"),
                    None => panic!("ws stream closed before heartbeat"),
                }
            }
        })
        .await
        .expect("timed out waiting for heartbeat");
        assert!(heartbeat.is_empty());
    }
}

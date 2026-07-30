#[cfg(all(test, feature = "telemetry"))]
mod tests {
    use std::{
        io::Cursor,
        sync::{Arc, Mutex},
    };

    use http::StatusCode;
    use http_body_util::BodyExt;
    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, state::World, sumeragi::status,
        telemetry::StateTelemetry,
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
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
    };
    use tokio::runtime::Runtime;

    use super::{sorafs_capacity_tests::build_por_challenge, *};
    use crate::mk_app_state_for_tests;

    static SUMERAGI_V2_STATUS_TEST_LOCK: Mutex<()> = Mutex::new(());

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

    #[test]
    fn status_block_visibility_falls_back_to_sumeragi_commit_height() {
        let metrics = Metrics::default();
        let mut status = Status::from(&metrics);
        status.blocks = 0;
        status.blocks_non_empty = 0;
        let sumeragi = status.sumeragi.as_mut().expect("sumeragi status");
        sumeragi.commit_qc_height = 4_274;
        sumeragi.highest_qc_height = 4_275;
        sumeragi.locked_qc_height = 4_273;

        super::normalize_status_block_visibility(&mut status, None);

        assert_eq!(status.blocks, 4_274);
        assert_eq!(status.blocks_non_empty, 0);

        status.blocks = 4_273;
        super::normalize_status_block_visibility(&mut status, None);
        assert_eq!(status.blocks, 4_274);
    }

    #[test]
    fn status_block_visibility_uses_authoritative_applied_height() {
        let metrics = Metrics::default();
        metrics.block_height.inc_by(4_193);
        let mut status = Status::from(&metrics);
        let sumeragi = status.sumeragi.as_mut().expect("sumeragi status");
        sumeragi.commit_qc_height = 4_275;

        super::normalize_status_block_visibility(&mut status, Some(4_274));

        assert_eq!(
            status.blocks, 4_274,
            "a CommitQC pending apply must not lead query-visible state"
        );

        let metrics = Metrics::default();
        metrics.block_height.inc_by(4_275);
        let mut status = Status::from(&metrics);
        let sumeragi = status.sumeragi.as_mut().expect("sumeragi status");
        sumeragi.commit_qc_height = 4_276;

        super::normalize_status_block_visibility(&mut status, Some(4_274));
        assert_eq!(
            status.blocks, 4_274,
            "a Kura-backed telemetry scan pending WSV apply must not lead state"
        );
    }

    #[tokio::test]
    async fn status_response_does_not_wait_for_lazy_block_counter_sync() {
        let metrics = Arc::new(Metrics::default());
        metrics.block_height.inc_by(4_193);
        let telemetry = MaybeTelemetry::from_profile(
            Some(Telemetry::new(metrics, true)),
            TelemetryProfile::Full,
        );

        let response = super::handle_status(
            &telemetry,
            Some(axum::http::HeaderValue::from_static("application/json")),
            None,
            true,
            None,
            Some(4_274),
            None,
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

        assert_eq!(
            payload.get("blocks").and_then(norito::json::Value::as_u64),
            Some(4_274)
        );
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
        let response = super::handle_status(&telemetry, None, Some(&path), true, None, None, None)
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
            true,
            Some(&policy),
            None,
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
    async fn status_root_and_tail_include_typed_offline_readiness() {
        use http_body_util::BodyExt;
        use iroha_torii_shared::offline_api::{
            OfflineReadiness, OfflineReadinessBlocker, OfflineStatus,
        };

        let telemetry = MaybeTelemetry::for_tests();
        let asset = OfflineReadiness {
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            required_bridge_abi_version: 21,
            max_hops: 8,
            asset_definition_id: "ds#boi.is".to_owned(),
            asset_scale: Some(2),
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: None,
            active_topup_shield_verifier: None,
            active_unshield_verifier: None,
            active_recursive_step_eq_verifier: None,
            active_recursive_step_ep_verifier: None,
            artifact_set: None,
            proof_backend_available: false,
            recursive_lineage_supported: false,
            ready: false,
            blockers: vec![OfflineReadinessBlocker {
                code: "transfer_verifier_unavailable".to_owned(),
                message: "test fixture".to_owned(),
            }],
        };
        let offline = OfflineStatus {
            mandatory: true,
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            required_bridge_abi_version: 21,
            max_hops: 8,
            ready: false,
            assets: vec![asset],
            blockers: Vec::new(),
        };

        let response = super::handle_status(
            &telemetry,
            Some(axum::http::HeaderValue::from_static("application/json")),
            None,
            true,
            None,
            None,
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
            Some(21)
        );

        let response = super::handle_status(
            &telemetry,
            None,
            Some("offline/assets/0/asset_definition_id"),
            true,
            None,
            None,
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
        assert_eq!(payload.as_str(), Some("ds#boi.is"));
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn status_tail_rejects_nexus_fields_when_disabled() {
        let telemetry = MaybeTelemetry::for_tests();
        let err = super::handle_status(
            &telemetry,
            None,
            Some("teu_lane_commit"),
            false,
            None,
            None,
            None,
        )
        .await
        .expect_err("lane-specific tails must be rejected when nexus is disabled");
        assert!(matches!(err, Error::StatusSegmentNotFound(_)));
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn metrics_handler_strips_lane_labels_when_nexus_disabled() {
        let telemetry = MaybeTelemetry::for_tests();
        telemetry
            .metrics()
            .await
            .set_lane_block_height("lane-0", "global", 3);

        let enabled = super::handle_metrics(&telemetry, true)
            .await
            .expect("metrics should render when Nexus is enabled");
        assert!(
            enabled.contains("nexus_lane_block_height"),
            "lane metrics should be present when Nexus is enabled"
        );

        let filtered = super::handle_metrics(&telemetry, false)
            .await
            .expect("metrics should render when Nexus is disabled");
        assert!(
            !filtered.contains("nexus_lane_block_height"),
            "lane metrics must be stripped when Nexus is disabled: {filtered}"
        );
        assert!(
            filtered.contains("block_height"),
            "non-lane metrics must remain after filtering: {filtered}"
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
        let response =
            super::handle_v1_sumeragi_status(axum::extract::State(state), None, false, false)
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
            None,
            true,
            false,
        )
        .await
        .expect("status handler");
        // Simulate Kura/snapshot activating the shared process output guard
        // after the reducer's last publication. Serving must monotonically
        // overlay that state without waiting for another reducer event.
        let restart_response =
            super::handle_v1_sumeragi_status(axum::extract::State(state), None, true, true)
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
        assert_eq!(decoded, expected);
        let restart_body = restart_response
            .into_body()
            .collect()
            .await
            .expect("collect restart-required status body")
            .to_bytes();
        let restart_decoded: SumeragiV2Status = norito::json::from_slice(&restart_body)
            .expect("decode restart-required authoritative status");
        assert!(restart_decoded.restart_required);
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
        ] {
            assert!(
                json.get(retired).is_none(),
                "retired field {retired} leaked"
            );
        }
    }

    #[tokio::test]
    async fn permissioned_sumeragi_diagnostics_omit_npos_and_canonical_state() {
        let state = std::sync::Arc::new(CoreState::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let response =
            super::handle_v1_sumeragi_diagnostics(axum::extract::State(state), None, None, false)
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
            true,
            None,
            None,
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
            prev_roster_evidence_hash: None,
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
                prev_roster_evidence_hash: None,
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
            prev_roster_evidence_hash: None,
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
                prev_roster_evidence_hash: None,
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

    #[test]
    fn sumeragi_telemetry_endpoint_returns_snapshot() {
        Runtime::new().expect("runtime").block_on(async {
            let peer = iroha_data_model::peer::PeerId::new(
                checked_routing_fixture_keypair(
                    0xF3,
                    Algorithm::Ed25519,
                    "derive Sumeragi telemetry fixture peer key",
                )
                .public_key()
                .clone(),
            );
            let availability_before = status::availability_snapshot();
            status::record_availability_vote(4, &peer);
            status::record_qc_latency("availability", 123);
            status::set_rbc_backlog_snapshot(7, 5, 2);

            let world = iroha_core::state::World::new();
            {
                let mut block = world.block();
                block.vrf_epochs_mut_for_testing().insert(
                    0,
                    iroha_data_model::consensus::VrfEpochRecord {
                        epoch: 0,
                        seed: [0x42; 32],
                        epoch_length: 10,
                        commit_deadline_offset: 2,
                        reveal_deadline_offset: 4,
                        roster_len: 1,
                        finalized: false,
                        updated_at_height: 4,
                        participants: Vec::new(),
                        late_reveals: Vec::new(),
                        committed_no_reveal: Vec::new(),
                        no_participation: Vec::new(),
                        penalties_applied: false,
                        penalties_applied_at_height: None,
                        validator_election: None,
                    },
                );
                block.commit();
            }

            let state = Arc::new(iroha_core::state::State::new_for_testing(
                world,
                iroha_core::kura::Kura::blank_kura_for_testing(),
                iroha_core::query::store::LiveQueryStore::start_test(),
            ));

            let resp = super::handle_v1_sumeragi_telemetry(state.clone())
                .await
                .expect("telemetry handler")
                .into_response();
            let body = resp
                .into_body()
                .collect()
                .await
                .expect("collect body")
                .to_bytes();
            let json: norito::json::Value =
                norito::json::from_slice(body.as_ref()).expect("decode telemetry response");

            let availability = json
                .get("availability")
                .and_then(|v| v.as_object())
                .expect("availability section present");
            let total = availability
                .get("total_votes_ingested")
                .and_then(norito::json::Value::as_u64)
                .expect("total votes ingested present");
            assert!(total > availability_before.total);
            let collectors = availability
                .get("collectors")
                .and_then(|v| v.as_array())
                .expect("collectors array present");
            assert!(collectors.iter().any(|entry| {
                entry
                    .get("votes_ingested")
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or_default()
                    >= 1
            }));

            let vrf = json
                .get("vrf")
                .and_then(|v| v.as_object())
                .expect("vrf section present");
            assert_eq!(
                vrf.get("found").and_then(norito::json::Value::as_bool),
                Some(true),
                "vrf summary should mark record as found"
            );
            assert!(vrf.contains_key("participants_total"));
            assert!(vrf.contains_key("reveals_total"));

            let qc = json
                .get("qc_latency_ms")
                .and_then(|v| v.as_array())
                .expect("qc_latency array present");
            assert!(qc.iter().any(|entry| {
                entry
                    .get("kind")
                    .and_then(norito::json::Value::as_str)
                    .map(|s| s == "availability")
                    .unwrap_or(false)
            }));

            let backlog = json
                .get("rbc_backlog")
                .and_then(|v| v.as_object())
                .expect("rbc_backlog present");
            assert_eq!(
                backlog
                    .get("pending_sessions")
                    .and_then(norito::json::Value::as_u64),
                Some(2)
            );
        });
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
            limit: Some(5),
            page_token: None,
        };
        let statuses = super::handle_get_sorafs_por_status(coordinator.clone(), status_query)
            .expect("status handler responds");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].challenge_id, challenge_a.challenge_id);

        let oversized_status_query = PorStatusQueryDto {
            manifest: None,
            provider: None,
            epoch: None,
            status: None,
            limit: Some(POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 + 1),
            page_token: None,
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
            },
        )
        .expect("export handler responds");
        assert_eq!(export.statuses.len(), 1);
        assert_eq!(export.statuses[0].challenge_id, challenge_a.challenge_id);

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
    use std::num::{NonZeroU16, NonZeroU64};

    use nonzero_ext::nonzero;
    use pprof::protos::Message;

    use super::*;

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
    use std::{io::ErrorKind, sync::Arc};

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
    use tokio::{net::TcpListener, sync::Mutex};

    use super::event::handle_events_stream_with_receiver;

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

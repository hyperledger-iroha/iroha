// Relay runtime, authenticated VPN helper, and fail-closed persistence regressions.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capability::{
            GreaseEntry, KemAdvertisement, KemId, NegotiatedCapabilities, SignatureAdvertisement,
            SignatureId,
        },
        config::VpnConfig,
        constant_rate,
        privacy::{PrivacyAggregator, PrivacyConfig, ProxyPolicyEventBuffer},
        scheduler::CellClass,
    };
    use ed25519_dalek::SigningKey;
    use iroha_crypto::{
        Signature,
        soranet::{
            certificate::{
                CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
                RelayCertificateBundleV2, RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
            },
            handshake::HandshakeSuite,
            pow,
            puzzle::{self, ChallengeBinding as PuzzleBinding, Parameters as PuzzleParameters},
        },
    };
    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        soranet::{
            incentives::{BandwidthConfidenceV1, RelayBandwidthProofV1},
            privacy_metrics::{
                SoranetPowFailureReasonV1, SoranetPrivacyModeV1, SoranetPrivacyThrottleScopeV1,
            },
            vpn::{
                VPN_CELL_LEN, VpnCellFlagsV1, VpnCellV1, VpnUsageVoucherBodyV1, VpnUsageVoucherV1,
            },
        },
    };

    use iroha_primitives::numeric::Numeric;
    use norito::{codec::Encode, decode_from_bytes, to_bytes};
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
    use std::{
        io::ErrorKind,
        net::TcpListener as StdTcpListener,
        num::NonZeroU32,
        sync::Arc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tempfile::{NamedTempFile, TempDir};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt, duplex},
        net::TcpStream,
        time::sleep,
    };
    const TEST_RELAY_ID: RelayId = [0xAB; 32];
    #[test]
    fn exit_compliance_context_uses_stable_reason_without_raw_channel() {
        let error = ExitStreamError::RouteNotProvisioned {
            stream: "norito-stream",
            channel: "sensitive-channel-id".to_owned(),
        };
        let (stream, channel, reason) = error.compliance_context();
        assert_eq!(stream, Some("norito-stream"));
        assert_eq!(channel, Some("sensitive-channel-id"));
        assert_eq!(reason, "route_not_provisioned");
        assert!(!reason.contains("sensitive-channel-id"));
    }
    #[test]
    fn operational_tracing_omits_network_and_session_identifiers() {
        let sources = [include_str!("../runtime.rs"), include_str!("../circuit.rs")];
        for forbidden in [
            "remote = %remote",
            "peer = %peer",
            "neighbors = ?neighbors",
            "payload = %String::from_utf8_lossy",
            "session_id = %hex::encode",
            "channel = %",
            "route = %",
            "room = %",
            "backend = %",
            "exit_multiaddr = %",
            "target = %target_url",
            "measurement = %",
            "relay = %",
            "ignoring text frame from exit adapter: {text}",
        ] {
            assert!(
                sources.iter().all(|source| !source.contains(forbidden)),
                "operational tracing must not contain `{forbidden}`"
            );
        }
    }
    fn secure_test_tempfile() -> NamedTempFile {
        NamedTempFile::new_in(std::env::current_dir().expect("current test directory"))
            .expect("create private test file")
    }
    fn secure_test_identity_manifest() -> NamedTempFile {
        let file = secure_test_tempfile();
        std::fs::write(
            file.path(),
            r#"{"identity_private_key_hex":"00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"}"#,
        )
        .expect("write private identity manifest");
        file
    }
    fn secure_test_tempdir() -> TempDir {
        tempfile::Builder::new()
            .prefix("soranet-relay-test-")
            .tempdir_in(std::env::current_dir().expect("current test directory"))
            .expect("create private test directory")
    }
    fn signed_usage_voucher(key_pair: &KeyPair, body: VpnUsageVoucherBodyV1) -> VpnUsageVoucherV1 {
        VpnUsageVoucherV1::try_sign(body, key_pair.private_key())
            .expect("usage voucher fixture should sign")
    }
    #[test]
    fn unix_time_ms_saturates_pre_epoch_clock() {
        assert_eq!(unix_time_ms(UNIX_EPOCH - Duration::from_secs(1)), 0);
        assert_eq!(unix_time_ms(UNIX_EPOCH + Duration::from_millis(42)), 42);
    }
    #[test]
    fn handshake_frame_len_prefix_encodes_boundary() {
        assert_eq!(
            handshake_frame_len_prefix(MAX_HANDSHAKE_FRAME_LEN).expect("max frame fits"),
            u16::try_from(MAX_HANDSHAKE_FRAME_LEN)
                .expect("max frame length fits u16")
                .to_be_bytes()
        );
    }
    #[test]
    fn handshake_frame_len_prefix_rejects_oversized_payload_without_panic() {
        let err = handshake_frame_len_prefix(MAX_HANDSHAKE_FRAME_LEN + 1)
            .expect_err("oversized handshake frame must fail");
        assert!(matches!(
            err,
            HandshakeError::FrameTooLarge(length) if length == MAX_HANDSHAKE_FRAME_LEN + 1
        ));
    }
    fn sample_metering_key_pair() -> KeyPair {
        KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519)
            .expect("derive VPN metering fixture key")
    }
    fn quantity_nanos(value: u64) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, 9))
            .expect("u64 nano-XOR test value fits Quantity")
    }
    fn sample_vpn_tariff() -> VpnTariffV1 {
        VpnTariffV1 {
            lease_fee: quantity_nanos(10_000),
            active_fee_per_minute: quantity_nanos(3_180),
            ingress_fee_per_mib: quantity_nanos(1_000),
            egress_fee_per_mib: quantity_nanos(2_000),
        }
    }
    fn sample_helper_ticket(session_id: [u8; 16]) -> VpnHelperTicketV1 {
        let key_pair = sample_metering_key_pair();
        VpnHelperTicketV1 {
            session_id,
            quote_id: [0x11; 32],
            account_hash: [0x22; 32],
            relay_id: [0x33; 32],
            payment_tx_hash: [0x44; 32],
            metering_public_key: key_pair.public_key().clone(),
            tariff: sample_vpn_tariff(),
            expires_at_ms: u64::MAX,
        }
    }
    #[test]
    fn route_open_metrics_use_adapter_once() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xAA; 16]);
        let adapter = VpnAdapter::new(handle.session().clone(), overlay);
        record_route_open_ingress_metrics(Some(&adapter), Some(&handle));
        let snapshot = metrics.snapshot();
        let bytes = RouteOpenFrame::length() as u64;
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_ingress_frames);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(1, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(bytes, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_ingress_bytes);
        record_route_open_egress_metrics(Some(&adapter), Some(&handle));
        let snapshot = metrics.snapshot();
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_egress_frames);
        assert_eq!(0, snapshot.vpn_egress_bytes);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(2, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(1, snapshot.vpn_control_egress_frames);
        assert_eq!(bytes * 2, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_egress_bytes);
    }
    #[test]
    fn route_open_metrics_fallback_to_session() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xBB; 16]);
        record_route_open_ingress_metrics(None, Some(&handle));
        let snapshot = metrics.snapshot();
        let bytes = RouteOpenFrame::length() as u64;
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_ingress_frames);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(1, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(bytes, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_ingress_bytes);
        record_route_open_egress_metrics(None, Some(&handle));
        let snapshot = metrics.snapshot();
        assert_eq!(0, snapshot.vpn_frames);
        assert_eq!(0, snapshot.vpn_egress_frames);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(2, snapshot.vpn_control_frames);
        assert_eq!(1, snapshot.vpn_control_ingress_frames);
        assert_eq!(1, snapshot.vpn_control_egress_frames);
        assert_eq!(bytes * 2, snapshot.vpn_control_bytes);
        assert_eq!(bytes, snapshot.vpn_control_egress_bytes);
    }
    #[test]
    fn handshake_byte_guard_does_not_touch_vpn_bytes() {
        let metrics = Arc::new(Metrics::new());
        let overlay = VpnOverlay::from_config(Default::default());
        let _session = overlay.start_session(Arc::clone(&metrics));
        let mut guard = HandshakeByteGuard::new(metrics.as_ref());
        guard.add(128);
        guard.finish();
        let snapshot = metrics.snapshot();
        assert_eq!(128, snapshot.handshake_bytes);
        assert_eq!(0, snapshot.vpn_bytes);
        assert_eq!(0, snapshot.vpn_ingress_bytes);
    }
    #[tokio::test]
    async fn vpn_backend_bootstrap_encodes_session_address_plan() {
        let bootstrap = build_vpn_backend_bootstrap([0x5A; 16]);
        let (mut writer, mut reader) = duplex(4096);
        let secret = [0xA5; 32];
        write_vpn_backend_bootstrap(&mut writer, &bootstrap, &secret)
            .await
            .expect("bootstrap write");
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic).await.expect("magic");
        assert_eq!(&magic, VPN_BACKEND_BOOTSTRAP_MAGIC);
        let mut len = [0u8; 2];
        reader.read_exact(&mut len).await.expect("len");
        let len = usize::from(u16::from_be_bytes(len));
        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).await.expect("payload");
        let mut payload = payload.as_slice();
        let decoded = VpnBackendBootstrapEnvelope::decode(&mut payload).expect("bootstrap norito");
        assert!(payload.is_empty());
        assert_eq!(
            decoded.mac,
            vpn_backend_bootstrap_mac(&secret, &bootstrap, decoded.timestamp_ms, &decoded.nonce)
        );
        assert_eq!(decoded.bootstrap, bootstrap);
        assert_eq!(decoded.bootstrap.server_tunnel_addresses.len(), 2);
        assert_eq!(decoded.bootstrap.session_routes.len(), 2);
    }
    #[tokio::test]
    async fn vpn_backend_status_reports_rejection_message() {
        let (mut writer, mut reader) = duplex(256);
        writer.write_all(&[0u8]).await.expect("status");
        writer.write_all(&4u16.to_be_bytes()).await.expect("len");
        writer.write_all(b"fail").await.expect("payload");
        let error = read_vpn_backend_status(&mut reader)
            .await
            .expect_err("status must reject");
        assert!(error.to_string().contains("fail"));
    }
    #[tokio::test]
    async fn vpn_backend_bridge_forwards_backend_payloads_into_vpn_frames() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig::default()));
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xA1; 16]);
        let helper_ticket = sample_helper_ticket([0xA1; 16]);
        let adapter = VpnAdapter::new(handle.session().clone(), Arc::clone(&overlay));
        let bridge = VpnBridge::new(
            adapter.clone(),
            [0xA1; 16],
            vpn_flow_label_from_session_id([0xA1; 16]),
        )
        .expect("cover scheduler seed");
        let (vpn_runtime, mut vpn_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut vpn_read, mut vpn_write) = tokio::io::split(vpn_runtime);
        let (backend_runtime, mut backend_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut backend_read, mut backend_write) = tokio::io::split(backend_runtime);
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let bridge_task = tokio::spawn(async move {
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                VpnBackendBridgeContext {
                    bridge,
                    adapter: &adapter,
                    vpn_session: &handle,
                    helper_ticket: &helper_ticket,
                    mtu: VpnCellV1::max_payload_len(),
                },
                &mut backend_read,
                &mut backend_write,
            )
            .await
            .expect("bridge should forward backend payload");
        });
        backend_peer
            .write_all(&payload)
            .await
            .expect("write backend payload");
        backend_peer
            .shutdown()
            .await
            .expect("shutdown backend peer");
        let parsed = crate::vpn::read_frame(overlay.as_ref(), &mut vpn_peer)
            .await
            .expect("vpn frame");
        assert_eq!(payload, parsed.payload);
        bridge_task.await.expect("bridge task joined");
    }
    #[tokio::test]
    async fn vpn_backend_bridge_forwards_vpn_payloads_into_backend_stream() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig::default()));
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xB2; 16]);
        let helper_ticket = sample_helper_ticket([0xB2; 16]);
        let adapter = VpnAdapter::new(handle.session().clone(), Arc::clone(&overlay));
        let bridge = VpnBridge::new(
            adapter.clone(),
            [0xB2; 16],
            vpn_flow_label_from_session_id([0xB2; 16]),
        )
        .expect("cover scheduler seed");
        let (vpn_runtime, mut vpn_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut vpn_read, mut vpn_write) = tokio::io::split(vpn_runtime);
        let (backend_runtime, mut backend_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut backend_read, mut backend_write) = tokio::io::split(backend_runtime);
        let payload = vec![0xFA, 0xCE, 0xB0, 0x0C];
        let bridge_task = tokio::spawn(async move {
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                VpnBackendBridgeContext {
                    bridge,
                    adapter: &adapter,
                    vpn_session: &handle,
                    helper_ticket: &helper_ticket,
                    mtu: VpnCellV1::max_payload_len(),
                },
                &mut backend_read,
                &mut backend_write,
            )
            .await
            .expect("bridge should forward vpn payload");
        });
        let cell = overlay
            .data_cell(
                [0xB2; 16],
                vpn_flow_label_from_session_id([0xB2; 16]),
                0,
                0,
                VpnCellFlagsV1::new(false, false, false, false),
                payload.clone(),
            )
            .expect("vpn cell");
        let frame = overlay.pad_cell(cell).expect("vpn frame");
        crate::vpn::write_frame(&mut vpn_peer, &frame)
            .await
            .expect("write vpn frame");
        vpn_peer.shutdown().await.expect("shutdown vpn peer");
        let mut actual = vec![0u8; payload.len()];
        backend_peer
            .read_exact(&mut actual)
            .await
            .expect("read backend payload");
        assert_eq!(payload, actual);
        bridge_task.await.expect("bridge task joined");
    }
    #[test]
    fn vpn_voucher_debt_window_rejects_unvouched_overrun() {
        let helper_ticket = sample_helper_ticket([0xA3; 16]);
        let mut window = VpnVoucherDebtWindow::new(&helper_ticket, 4);
        window.record_ingress(4).expect("within debt window");
        assert!(window.record_ingress(1).is_err());
    }
    #[test]
    fn vpn_voucher_debt_window_rejects_wrong_metering_public_key() {
        let helper_ticket = sample_helper_ticket([0xA5; 16]);
        let wrong_key_pair = KeyPair::try_from_seed(vec![0x77; 32], Algorithm::Ed25519)
            .expect("derive wrong VPN metering fixture key");
        let body = VpnUsageVoucherBodyV1 {
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            relay_id: helper_ticket.relay_id,
            sequence: 1,
            ingress_bytes: 1,
            egress_bytes: 1,
            active_ms: 1_000,
            issued_at_ms: 2_000,
        };
        let voucher = signed_usage_voucher(&wrong_key_pair, body);
        let envelope = VpnUsageVoucherEnvelopeV1 {
            earned_fee: helper_ticket
                .tariff
                .earned_fee(&voucher.body)
                .expect("bounded fixture fee"),
            voucher,
        };
        let mut window = VpnVoucherDebtWindow::new(&helper_ticket, 64);
        let error = window
            .accept_envelope(&envelope)
            .expect_err("wrong metering key must fail");
        assert!(error.to_string().contains("public key"));
    }
    #[test]
    fn vpn_usage_voucher_control_updates_receipt() {
        let helper_ticket = sample_helper_ticket([0xA4; 16]);
        let key_pair = sample_metering_key_pair();
        let body = VpnUsageVoucherBodyV1 {
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            relay_id: helper_ticket.relay_id,
            sequence: 7,
            ingress_bytes: 10,
            egress_bytes: 20,
            active_ms: 1_000,
            issued_at_ms: 2_000,
        };
        let voucher = signed_usage_voucher(&key_pair, body);
        let envelope = VpnUsageVoucherEnvelopeV1 {
            voucher,
            earned_fee: quantity_nanos(55),
        };
        let mut payload = Vec::from(VPN_USAGE_VOUCHER_CONTROL_MAGIC.as_slice());
        payload.extend_from_slice(&envelope.encode());
        let decoded = decode_usage_voucher_control(&payload)
            .expect("decode")
            .expect("voucher payload");
        let mut window = VpnVoucherDebtWindow::new(&helper_ticket, 64);
        window.record_ingress(10).expect("ingress debt");
        window.record_egress(20).expect("egress debt");
        window.accept_envelope(&decoded).expect("voucher accepted");
        let mut lower_fee_body = body;
        lower_fee_body.sequence = 8;
        lower_fee_body.ingress_bytes = 11;
        lower_fee_body.egress_bytes = 21;
        let lower_fee_envelope = VpnUsageVoucherEnvelopeV1 {
            voucher: signed_usage_voucher(&key_pair, lower_fee_body),
            earned_fee: quantity_nanos(54),
        };
        let lower_fee_error = window
            .accept_envelope(&lower_fee_envelope)
            .expect_err("wrong earned fee must fail");
        assert!(lower_fee_error.to_string().contains("earned fee"));
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_helper_session(session, helper_ticket.clone());
        handle.record_usage_voucher(decoded);
        let receipt = handle.receipt();
        assert_eq!(receipt.highest_voucher_sequence, 7);
        assert_eq!(receipt.earned_fee, quantity_nanos(55));
        assert_eq!(receipt.client_voucher_hash, envelope.voucher.hash());
        let artifact = handle
            .settlement_artifact()
            .expect("accepted voucher should produce settlement artifact");
        assert_eq!(
            artifact.receipt.client_voucher_hash,
            envelope.voucher.hash()
        );
        let spool_dir = secure_test_tempdir();
        let path = spool_vpn_settlement_artifact(spool_dir.path(), &artifact)
            .expect("spool settlement artifact");
        let encoded = std::fs::read(&path).expect("read settlement artifact");
        assert!(encoded.len() <= VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1);
        let record: VpnSettlementSpoolRecord =
            norito::json::from_slice(&encoded).expect("settlement artifact json");
        assert_eq!(record.version, 1);
        assert_eq!(record.torii_receipt_path, "/v1/vpn/receipts");
        assert_eq!(record.session_id_hex, hex::encode(helper_ticket.session_id));
        assert_eq!(record.quote_id_hex, hex::encode(helper_ticket.quote_id));
        assert_eq!(
            record.payment_tx_hash_hex,
            hex::encode(helper_ticket.payment_tx_hash)
        );
        assert_eq!(record.earned_fee, quantity_nanos(55));
        assert!(
            String::from_utf8(encoded)
                .expect("settlement artifact is UTF-8 JSON")
                .contains("\"earned_fee\": \"0.000000055\"")
        );
        assert_eq!(
            record.submit_receipt_request.relay_receipt_hex,
            hex::encode(artifact.receipt.encode())
        );
        assert_eq!(
            record.submit_receipt_request.client_voucher_hex,
            hex::encode(artifact.voucher.encode())
        );
        assert_eq!(
            record.submit_receipt_request.lease_id_hex,
            hex::encode(helper_ticket.quote_id)
        );
    }
    #[test]
    fn downgrade_detail_prefers_first_non_empty_warning() {
        let warnings = vec![
            CapabilityWarning {
                capability_type: 0x0203,
                message: "   ".to_string(),
            },
            CapabilityWarning {
                capability_type: 0x0102,
                message: "Client omitted suite_list capability despite relay marking it required"
                    .to_string(),
            },
        ];
        let slug = downgrade_detail_from_warnings(&warnings);
        assert_eq!(
            slug.as_deref(),
            Some("client_suite_list_missing"),
            "expected to sanitize suite_list warning"
        );
    }
    #[test]
    fn downgrade_detail_sanitizes_constant_rate_warning() {
        let warnings = vec![CapabilityWarning {
            capability_type: 0x0203,
            message: "Constant-rate capability missing on hop".to_string(),
        }];
        let slug = downgrade_detail_from_warnings(&warnings);
        assert_eq!(
            slug.as_deref(),
            Some("constant_rate_capability_missing_on_hop"),
            "constant-rate warnings should produce deterministic slug"
        );
    }
    #[test]
    fn handshake_suite_downgrade_records_only_nk3() {
        let metrics = Metrics::new();
        record_handshake_suite_downgrade(&metrics, HandshakeSuite::Nk2Hybrid);
        record_handshake_suite_downgrade(&metrics, HandshakeSuite::Nk3PqForwardSecure);
        let snapshot = metrics.snapshot();
        assert_eq!(
            snapshot
                .downgrade_counts
                .get("handshake_suite_nk3")
                .copied(),
            Some(1)
        );
    }
    #[test]
    fn constant_rate_lane_manager_auto_disables_and_restores() {
        let spec = constant_rate::profile_by_name("core").expect("core profile");
        let registry = Arc::new(CircuitRegistry::default());
        let manager = ConstantRateLaneManager::new(spec, Arc::clone(&registry));
        let metrics = Metrics::new();
        metrics.set_constant_rate_profile(
            spec.name,
            u64::from(spec.neighbor_cap),
            spec.tick_millis,
            u64::from(spec.dummy_lane_floor),
        );
        // 7/8 neighbors => 87.5% saturation, exceeding the 85% disable threshold.
        manager.apply_active_sample(7, &metrics);
        assert_eq!(manager.current_cap(), spec.dummy_lane_floor);
        // Drop to 3/8 neighbors (37.5%), which is below the 75% restore threshold.
        manager.apply_active_sample(3, &metrics);
        assert_eq!(manager.current_cap(), spec.neighbor_cap);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.constant_rate_queue_depth, 3);
        assert_eq!(snapshot.constant_rate_active_neighbors, 3);
        assert_eq!(snapshot.constant_rate_dummy_lanes, 1);
        assert!(
            (snapshot.constant_rate_dummy_ratio - (1.0 / f64::from(spec.neighbor_cap))).abs()
                < f64::EPSILON,
            "expected dummy ratio to track remaining cover lanes"
        );
        assert!(
            (snapshot.constant_rate_slot_rate_hz - (1000.0 / spec.tick_millis)).abs()
                < f64::EPSILON,
            "slot rate gauge must reflect the profile tick"
        );
        assert_eq!(
            snapshot.constant_rate_ceiling_hits, 1,
            "saturation event should increment the ceiling counter"
        );
        assert!(
            !snapshot.constant_rate_degraded,
            "restoring the neighbor cap should reset the degraded gauge"
        );
        assert!(
            snapshot.constant_rate_saturation_percent <= 38,
            "expected saturation gauge to reflect reduced utilization"
        );
    }
    #[test]
    fn constant_rate_engine_tracks_dummy_ratio_and_queue_depth() {
        let spec = constant_rate::profile_by_name("null").expect("null profile");
        let mut engine = ConstantRateEngine::new(spec);
        let first = engine.next_cell();
        assert!(first.cell.is_dummy);
        assert_eq!(first.queues.total(), 0);
        assert_eq!(first.dummy_ratio, 1.0);
        assert!(engine.enqueue(Cell::new(CellClass::Interactive, vec![0xAA, 0xBB])));
        assert!(engine.enqueue(Cell::new(CellClass::Bulk, vec![0xCC])));
        let second = engine.next_cell();
        assert!(
            !second.cell.is_dummy,
            "queued payload should be sent ahead of dummy cells"
        );
        assert_eq!(
            second.queues,
            QueueDepths {
                control: 0,
                interactive: 1,
                bulk: 1
            }
        );
        assert!(
            second.dummy_ratio < 1.0,
            "dummy ratio should drop after sending a real cell"
        );
    }
    #[test]
    fn constant_rate_engine_drops_low_priority_on_congestion_signal() {
        let spec = constant_rate::profile_by_name("home").expect("home profile");
        let mut engine = ConstantRateEngine::new(spec);
        assert!(engine.enqueue(Cell::new(CellClass::Control, vec![0x01])));
        assert!(engine.enqueue(Cell::new(CellClass::Interactive, vec![0x02])));
        assert!(engine.enqueue(Cell::new(CellClass::Bulk, vec![0x03])));
        let action = engine
            .apply_congestion_hint(CELL_SIZE_BYTES.saturating_sub(1))
            .expect("buffer pressure should emit a congestion action");
        assert_eq!(action.dropped_class, Some(CellClass::Bulk));
        // If congestion clears, the signal should return None and avoid spurious drops.
        assert!(engine.apply_congestion_hint(CELL_SIZE_BYTES).is_none());
    }
    fn in_memory_ticket_replays(capacity: usize) -> StdMutex<TicketReplayState> {
        let limits =
            TicketRevocationStoreLimits::new(capacity, Duration::from_secs(300)).expect("limits");
        let persisted = TicketRevocationStore::in_memory(limits).expect("replay store");
        StdMutex::new(TicketReplayState {
            persisted,
            pending: HashSet::new(),
            capacity,
        })
    }
    fn client_hello_frame_with_resume(resume_hash: Option<&[u8]>) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.push(crate::handshake::CLIENT_HELLO_TYPE);
        frame.extend_from_slice(&32u16.to_be_bytes());
        frame.extend_from_slice(&[0xAA; 32]);
        frame.push(1);
        frame.push(1);
        frame.extend_from_slice(&[0x11; 32]);
        frame.extend_from_slice(&4u16.to_be_bytes());
        frame.extend_from_slice(&[0x22; 4]);
        frame.extend_from_slice(&2u16.to_be_bytes());
        frame.extend_from_slice(&[0x80, 0x01]);
        match resume_hash {
            Some(resume_hash) => {
                frame.push(1);
                frame.extend_from_slice(
                    &u16::try_from(resume_hash.len())
                        .expect("test resume hash length fits")
                        .to_be_bytes(),
                );
                frame.extend_from_slice(resume_hash);
            }
            None => frame.push(0),
        }
        frame.resize(crate::handshake::NOISE_PADDING_BLOCK, 0);
        frame
    }
    #[test]
    fn admission_transcript_commits_to_the_exact_client_hello() {
        let without_resume = client_hello_frame_with_resume(None);
        let with_resume = client_hello_frame_with_resume(Some(&[0x44; 32]));
        ClientHello::parse(&without_resume).expect("parse hello without resume hash");
        ClientHello::parse(&with_resume).expect("parse hello with resume hash");
        let first = pow::derive_admission_transcript(&without_resume);
        assert_eq!(
            first,
            pow::derive_admission_transcript(&without_resume),
            "the same client hello must derive the same binding"
        );
        assert_ne!(
            first,
            pow::derive_admission_transcript(&with_resume),
            "changing any client hello field must change the admission binding"
        );
    }
    #[test]
    fn rejected_admission_never_runs_expensive_handshake() {
        let expensive_ran = std::cell::Cell::new(false);
        let result = continue_after_admission::<()>(
            Err(HandshakeError::ReplayStore("rejected".to_owned())),
            || {
                expensive_ran.set(true);
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(HandshakeError::ReplayStore(message)) if message == "rejected"
        ));
        assert!(
            !expensive_ran.get(),
            "ML-KEM handshake work must stay behind admission"
        );
    }
    #[test]
    fn verify_puzzle_ticket_requires_binding_and_consumes_once() {
        let params = PuzzleParameters::new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(180),
            Duration::from_secs(45),
        );
        let descriptor = vec![0xD4; 32];
        let relay_id = vec![0xC3; 32];
        let admission_transcript = [0x9Au8; 32];
        let mut rng = StdRng::from_seed([0x5Au8; 32]);
        let replays = in_memory_ticket_replays(4);
        let binding = PuzzleBinding::new(&descriptor, &relay_id, &admission_transcript);
        let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
            .expect("mint transcript-bound ticket");
        verify_puzzle_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_id,
            &admission_transcript,
            &replays,
        )
        .expect("ticket should verify with matching admission transcript");
        assert!(matches!(
            verify_puzzle_ticket_binding(
                &ticket,
                &params,
                &descriptor,
                &relay_id,
                &admission_transcript,
                &replays,
            ),
            Err(HandshakeError::Pow(pow::Error::Replay))
        ));
        let mismatched = [0x44u8; 32];
        let mismatched_replays = in_memory_ticket_replays(4);
        let mismatched_ticket =
            puzzle::mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
                .expect("mint second ticket");
        let err = verify_puzzle_ticket_binding(
            &mismatched_ticket,
            &params,
            &descriptor,
            &relay_id,
            &mismatched,
            &mismatched_replays,
        )
        .expect_err("mismatched admission transcript must fail verification");
        match err {
            HandshakeError::Puzzle(puzzle::Error::InvalidSolution) => {}
            other => panic!("unexpected puzzle verification error: {other:?}"),
        }
    }
    #[test]
    fn verify_puzzle_ticket_rejects_wrong_relay_binding() {
        let params = PuzzleParameters::new(
            NonZeroU32::new(4_096).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            5,
            Duration::from_secs(120),
            Duration::from_secs(30),
        );
        let descriptor = vec![0x51; 32];
        let relay_id = vec![0x42; 32];
        let admission_transcript = [0x24u8; 32];
        let mut rng = StdRng::from_seed([0x91u8; 32]);
        let binding = PuzzleBinding::new(&descriptor, &relay_id, &admission_transcript);
        let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(50), &mut rng)
            .expect("mint ticket with relay binding");
        let mismatched_relay = vec![0x99; 32];
        let replays = in_memory_ticket_replays(4);
        let err = verify_puzzle_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &mismatched_relay,
            &admission_transcript,
            &replays,
        )
        .expect_err("relay mismatch must fail verification");
        match err {
            HandshakeError::Puzzle(puzzle::Error::InvalidSolution) => {}
            other => panic!("unexpected puzzle verification error: {other:?}"),
        }
    }
    #[test]
    fn verify_pow_ticket_rejects_wrong_relay_binding() {
        let params = PowParameters::new(16, Duration::from_secs(180), Duration::from_secs(45));
        let descriptor = [0xAA; 32];
        let relay_a = [0x01; 32];
        let relay_b = [0x02; 32];
        let transcript = [0x03; 32];
        let mut rng = StdRng::from_seed([0x22; 32]);
        let binding = pow::ChallengeBinding::new(&descriptor, &relay_a, &transcript);
        let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint pow ticket");
        let replays = in_memory_ticket_replays(4);
        let err = verify_pow_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_b,
            &transcript,
            &replays,
        )
        .expect_err("relay mismatch must fail verification");
        match err {
            HandshakeError::Pow(pow::Error::InvalidSolution) => {}
            other => panic!("unexpected pow verification error: {other:?}"),
        }
    }
    #[test]
    fn verify_pow_ticket_respects_transcript_binding() {
        let params = PowParameters::new(16, Duration::from_secs(120), Duration::from_secs(30));
        let descriptor = [0x0C; 32];
        let relay_id = [0x0D; 32];
        let transcript = [0xFE; 32];
        let mut rng = StdRng::from_seed([0x33; 32]);
        let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
        let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(40), &mut rng)
            .expect("mint pow ticket with transcript");
        let replays = in_memory_ticket_replays(4);
        let mismatched = [0xAA; 32];
        let err = verify_pow_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_id,
            &mismatched,
            &replays,
        )
        .expect_err("mismatched transcript must fail verification");
        match err {
            HandshakeError::Pow(pow::Error::InvalidSolution) => {}
            other => panic!("unexpected pow verification error: {other:?}"),
        }
    }
    #[test]
    fn relay_ticket_replay_is_rejected_after_store_reload() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("relay-ticket-replays.norito");
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        let params = PowParameters::new(0, Duration::from_secs(180), Duration::from_secs(30));
        let descriptor = [0x35; 32];
        let relay_id = [0x46; 32];
        let transcript = [0x57; 32];
        let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
        let mut rng = StdRng::from_seed([0x68; 32]);
        let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint ticket");
        let persisted =
            TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("load store");
        let replays = StdMutex::new(TicketReplayState {
            persisted,
            pending: HashSet::new(),
            capacity: limits.max_entries,
        });
        verify_pow_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_id,
            &transcript,
            &replays,
        )
        .expect("first ticket use");
        drop(replays);
        let persisted =
            TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("reload store");
        let reloaded = StdMutex::new(TicketReplayState {
            persisted,
            pending: HashSet::new(),
            capacity: limits.max_entries,
        });
        assert!(matches!(
            verify_pow_ticket_binding(
                &ticket,
                &params,
                &descriptor,
                &relay_id,
                &transcript,
                &reloaded,
            ),
            Err(HandshakeError::Pow(pow::Error::Replay))
        ));
    }
    #[test]
    fn full_replay_store_rejects_before_costly_ticket_verification() {
        let replays = in_memory_ticket_replays(1);
        let params = PowParameters::new(0, Duration::from_secs(180), Duration::from_secs(30));
        let descriptor = [0x11; 32];
        let relay_id = [0x22; 32];
        let transcript = [0x33; 32];
        let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
        let mut rng = StdRng::from_seed([0x44; 32]);
        let first = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint first");
        let second = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint second");
        verify_and_consume_ticket(&first, &replays, || Ok(())).expect("consume first");
        let costly_verify_ran = std::cell::Cell::new(false);
        let err = verify_and_consume_ticket(&second, &replays, || {
            costly_verify_ran.set(true);
            Ok(())
        })
        .expect_err("capacity must fail closed");
        assert!(matches!(err, HandshakeError::ReplayStore(_)));
        assert!(
            !costly_verify_ran.get(),
            "capacity gate must run before Argon2 or ML-KEM work"
        );
    }
    #[test]
    fn concurrent_duplicate_ticket_is_rejected_while_first_use_is_pending() {
        let replays = Arc::new(in_memory_ticket_replays(2));
        let params = PowParameters::new(0, Duration::from_secs(180), Duration::from_secs(30));
        let descriptor = [0x71; 32];
        let relay_id = [0x72; 32];
        let transcript = [0x73; 32];
        let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
        let mut rng = StdRng::from_seed([0x74; 32]);
        let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint ticket");
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first_replays = Arc::clone(&replays);
        let first = std::thread::spawn(move || {
            verify_and_consume_ticket(&ticket, first_replays.as_ref(), || {
                entered_tx.send(()).expect("signal pending verification");
                release_rx.recv().expect("release pending verification");
                Ok(())
            })
        });
        entered_rx.recv().expect("first verification entered");
        let second_verify_ran = std::cell::Cell::new(false);
        let duplicate = verify_and_consume_ticket(&ticket, replays.as_ref(), || {
            second_verify_ran.set(true);
            Ok(())
        })
        .expect_err("concurrent duplicate must fail");
        assert!(matches!(duplicate, HandshakeError::Pow(pow::Error::Replay)));
        assert!(
            !second_verify_ran.get(),
            "duplicate must be rejected before verification work"
        );
        release_tx.send(()).expect("release first verification");
        first
            .join()
            .expect("first verification thread")
            .expect("first ticket use succeeds");
    }
    #[test]
    fn pow_failure_reason_labels_signature_and_absent_key_cases() {
        let signature = pow::Error::InvalidSignature;
        assert_eq!(
            pow_failure_reason(&signature),
            SoranetPowFailureReasonV1::SignatureInvalid
        );
        let malformed = pow::Error::Malformed("signed ticket payload".to_string());
        assert_eq!(
            pow_failure_reason(&malformed),
            SoranetPowFailureReasonV1::UnsupportedVersion
        );
        let overflow = pow::Error::ExpiryTimestampOverflow(u64::MAX);
        assert_eq!(
            pow_failure_reason(&overflow),
            SoranetPowFailureReasonV1::ClockError
        );
    }
    #[test]
    fn norito_stream_open_roundtrip() {
        let open = NoritoStreamOpen {
            channel_id: [0xA1; 32],
            route_id: [0xB2; 32],
            stream_id: [0xC3; 32],
            authenticated: true,
            padding_budget_ms: Some(37),
            access_kind: SoranetAccessKind::Authenticated,
            exit_token: vec![0x45, 0x67, 0x89],
        };
        let bytes = to_bytes(&open).expect("encode handshake");
        let decoded: NoritoStreamOpen = decode_from_bytes(&bytes).expect("decode handshake");
        assert_eq!(decoded.channel_id, open.channel_id);
        assert_eq!(decoded.route_id, open.route_id);
        assert_eq!(decoded.stream_id, open.stream_id);
        assert_eq!(decoded.exit_token, open.exit_token);
        assert_eq!(decoded.authenticated, open.authenticated);
        assert_eq!(decoded.padding_budget_ms, open.padding_budget_ms);
        assert_eq!(decoded.access_kind, open.access_kind);
    }
    #[test]
    fn kaigi_stream_open_roundtrip() {
        let open = KaigiStreamOpen {
            channel_id: [0xAA; 32],
            route_id: [0xBB; 32],
            stream_id: [0xCC; 32],
            room_id: [0xDD; 32],
            authenticated: false,
            access_kind: SoranetAccessKind::ReadOnly,
            exit_token: vec![0x10, 0x20, 0x30],
            exit_multiaddr: "/dns/kaigi.example/tcp/9443/ws".into(),
        };
        let bytes = to_bytes(&open).expect("encode kaigi handshake");
        let decoded: KaigiStreamOpen = decode_from_bytes(&bytes).expect("decode kaigi handshake");
        assert_eq!(decoded.channel_id, open.channel_id);
        assert_eq!(decoded.route_id, open.route_id);
        assert_eq!(decoded.stream_id, open.stream_id);
        assert_eq!(decoded.room_id, open.room_id);
        assert_eq!(decoded.exit_token, open.exit_token);
        assert_eq!(decoded.exit_multiaddr, open.exit_multiaddr);
        assert_eq!(decoded.access_kind, open.access_kind);
        assert_eq!(decoded.authenticated, open.authenticated);
    }
    #[test]
    fn norito_padding_delay_matches_expected_formula() {
        let channel_id = [0x11; 32];
        let period = Duration::from_millis(100);
        let now = UNIX_EPOCH + Duration::from_millis(45);
        let delay = RelayRuntime::norito_padding_delay(&channel_id, period, now);
        let period_millis = period.as_millis();
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&channel_id[..8]);
        let seed = u64::from_le_bytes(seed_bytes);
        let offset = u128::from(seed) % period_millis;
        let now_mod = now
            .duration_since(UNIX_EPOCH)
            .expect("time since epoch")
            .as_millis()
            % period_millis;
        let expected = (period_millis + offset - now_mod) % period_millis;
        assert_eq!(delay.as_millis(), expected);
    }
    #[test]
    fn norito_padding_delay_zero_when_on_schedule() {
        let channel_id = [0x42; 32];
        let period = Duration::from_millis(80);
        let period_millis = period.as_millis() as u64;
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&channel_id[..8]);
        let seed = u64::from_le_bytes(seed_bytes);
        let offset = seed % period_millis;
        let now = UNIX_EPOCH + Duration::from_millis(2 * period_millis + offset);
        let delay = RelayRuntime::norito_padding_delay(&channel_id, period, now);
        assert_eq!(delay.as_millis(), 0);
    }
    #[test]
    fn kaigi_multiaddr_to_websocket_converts_basic_multiaddr() {
        let url = RelayRuntime::kaigi_multiaddr_to_websocket("/dns/kaigi.test/tcp/9443/ws")
            .expect("convert dns multiaddr");
        assert_eq!(url, "ws://kaigi.test:9443/");
        let ipv6_url = RelayRuntime::kaigi_multiaddr_to_websocket("/ip6/2001:db8::1/tcp/8443/wss")
            .expect("convert ipv6 multiaddr");
        assert_eq!(ipv6_url, "wss://[2001:db8::1]:8443/");
    }
    #[test]
    fn kaigi_multiaddr_to_websocket_rejects_invalid_multiaddr() {
        assert!(RelayRuntime::kaigi_multiaddr_to_websocket("/udp/host/9999").is_none());
        assert!(RelayRuntime::kaigi_multiaddr_to_websocket("").is_none());
    }
    fn load_config(json: &str) -> RelayConfig {
        let file = secure_test_tempfile();
        std::fs::write(file.path(), json).expect("write config");
        let mut config = RelayConfig::load(file.path()).expect("load config");
        let default_replay_path = config::PowConfig::default().revocation_store_path;
        if config.pow_config().revocation_store_path == default_replay_path {
            config
                .pow
                .as_mut()
                .expect("PoW defaults applied")
                .revocation_store_path = file.path().with_extension("ticket-replays.norito");
        }
        config
    }
    fn sample_account(seed: u8) -> AccountId {
        let (public_key, _) = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive relay runtime fixture account key")
            .into_parts();
        AccountId::new(public_key)
    }
    #[test]
    fn fixture_key_helpers_use_checked_seed_derivation() {
        let expected_metering = KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519)
            .expect("derive VPN metering fixture key");
        assert_eq!(
            sample_metering_key_pair().public_key(),
            expected_metering.public_key()
        );
        let (public_key, _) = KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519)
            .expect("derive relay runtime fixture account key")
            .into_parts();
        assert_eq!(sample_account(0x21), AccountId::new(public_key));
    }
    fn sample_bandwidth_proof(
        epoch: u32,
        measurement_seed: u8,
        verified_bytes: u128,
    ) -> RelayBandwidthProofV1 {
        let mut measurement_id = [0u8; 32];
        measurement_id.fill(measurement_seed);
        RelayBandwidthProofV1 {
            relay_id: TEST_RELAY_ID,
            measurement_id,
            epoch,
            verified_bytes,
            verifier_id: sample_account(measurement_seed),
            issued_at_unix: 1,
            confidence: BandwidthConfidenceV1 {
                sample_count: 16,
                jitter_p95_ms: 4,
                confidence_per_mille: 900,
            },
            signature: Signature::try_from_bytes(&[0x55; 64])
                .expect("relay bandwidth fixture signature is non-empty and nonzero"),
            metadata: Metadata::default(),
        }
    }
    struct CertificateTestFixture {
        descriptor_commit: [u8; 32],
        bundle: RelayCertificateBundleV2,
        bundle_file: NamedTempFile,
        manifest_file: NamedTempFile,
        issuer_ed25519_hex: String,
        issuer_mldsa_hex: String,
    }
    const TEST_ML_KEM_PUBLIC_LEN: usize = 1_184;
    const TEST_ML_KEM_SECRET_LEN: usize = 2_400;
    impl CertificateTestFixture {
        fn new() -> Self {
            Self::with_valid_until(i64::MAX)
        }
        fn with_valid_until(valid_until: i64) -> Self {
            let descriptor_commit = [0xAB; 32];
            let identity_seed = [0x11; 32];
            let identity_seed_hex = hex::encode(identity_seed);
            let identity_signing = SigningKey::from_bytes(&identity_seed);
            let identity_public = identity_signing.verifying_key();
            let identity_public_bytes = identity_public.to_bytes();
            let relay_mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
                .expect("ML-DSA keypair generation should succeed");
            let kem_policy = KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: 0x01,
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            };
            let ml_kem_private = vec![0x33; TEST_ML_KEM_SECRET_LEN];
            let ml_kem_public = vec![0x44; TEST_ML_KEM_PUBLIC_LEN];
            let certificate = RelayCertificateV2 {
                relay_id: identity_public_bytes,
                identity_ed25519: identity_public_bytes,
                identity_mldsa65: relay_mldsa_keys.public_key.clone(),
                descriptor_commit,
                roles: RelayRolesV2 {
                    entry: true,
                    middle: true,
                    exit: false,
                },
                guard_weight: 25,
                bandwidth_bytes_per_sec: 1_000_000,
                reputation_weight: 50,
                endpoints: vec![RelayEndpointV2 {
                    quic_multiaddr: "/dns/relay.test/udp/443/quic".to_string(),
                    tls_server_name: "relay.test".to_string(),
                    tls_spki_sha256: [0xA5; 32],
                    priority: 0,
                    tags: vec!["norito".to_string()],
                }],
                capability_flags: RelayCapabilityFlagsV1::new(
                    CapabilityToggle::Enabled,
                    CapabilityToggle::Disabled,
                    CapabilityToggle::Enabled,
                    CapabilityToggle::Disabled,
                ),
                kem_policy,
                handshake_suites: vec![
                    HandshakeSuite::Nk3PqForwardSecure,
                    HandshakeSuite::Nk2Hybrid,
                ],
                published_at: 1,
                valid_after: 1,
                valid_until,
                directory_hash: [0x66; 32],
                issuer_fingerprint: [0x77; 32],
                pq_kem_public: ml_kem_public.clone(),
            };
            let issuer_seed = [0x99; 32];
            let issuer_signing = SigningKey::from_bytes(&issuer_seed);
            let issuer_mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
                .expect("ML-DSA keypair generation should succeed");
            let bundle = certificate
                .issue(&issuer_signing, issuer_mldsa_keys.secret_key())
                .expect("issue certificate");
            let bundle_file = secure_test_tempfile();
            std::fs::write(bundle_file.path(), bundle.to_cbor()).expect("write bundle");
            let manifest_file = secure_test_tempfile();
            let manifest_identity_hex = identity_seed_hex.clone();
            std::fs::write(
                manifest_file.path(),
                format!(
                    r#"{{
                        "version": 1,
                        "identity": {{
                            "ed25519_private_key_hex": "{}",
                            "ml_kem_private_key_hex": "{}",
                            "ml_kem_public_hex": "{}"
                        }}
                    }}"#,
                    manifest_identity_hex,
                    hex::encode(&ml_kem_private),
                    hex::encode(&ml_kem_public)
                ),
            )
            .expect("write manifest");
            let issuer_ed25519_hex = hex::encode(issuer_signing.verifying_key().to_bytes());
            let issuer_mldsa_hex = hex::encode(issuer_mldsa_keys.public_key());
            Self {
                descriptor_commit,
                bundle,
                bundle_file,
                manifest_file,
                issuer_ed25519_hex,
                issuer_mldsa_hex,
            }
        }
    }
    #[test]
    fn generates_self_signed_config() {
        let config = RelayRuntime::self_signed_server_config("relay.test");
        assert!(config.is_ok());
    }
    #[test]
    fn relay_quic_server_rejects_tls_early_data() {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
                .expect("generate test certificate");
        let key =
            PrivateKeyDer::try_from(signing_key.serialize_der()).expect("encode test private key");
        let tls = RelayRuntime::tls_server_config(vec![cert.der().clone()], key)
            .expect("build relay TLS configuration");
        assert_eq!(tls.max_early_data_size, 0);
    }
    #[test]
    fn relay_tls_file_loaders_enforce_first_release_bounds() {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
                .expect("generate test certificate");
        let directory = secure_test_tempdir();
        let certificate_path = directory.path().join("relay-chain.pem");
        let private_key_path = directory.path().join("relay-key.pem");
        let certificate_pem = format!("{}\n", cert.pem());
        std::fs::write(
            &certificate_path,
            certificate_pem.repeat(TLS_CERTIFICATE_CHAIN_MAX_ENTRIES_V1),
        )
        .expect("write exact certificate chain");
        let certificates = RelayRuntime::load_certificates(&certificate_path)
            .expect("exact certificate chain must load");
        assert_eq!(certificates.len(), TLS_CERTIFICATE_CHAIN_MAX_ENTRIES_V1);
        std::fs::write(
            &certificate_path,
            certificate_pem.repeat(TLS_CERTIFICATE_CHAIN_MAX_ENTRIES_V1 + 1),
        )
        .expect("write oversized certificate chain");
        let error = RelayRuntime::load_certificates(&certificate_path)
            .expect_err("max+1 certificates must fail");
        assert!(error.to_string().contains("certificate limit"), "{error}");
        std::fs::write(&private_key_path, signing_key.serialize_pem()).expect("write private key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&private_key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect private key");
        }
        RelayRuntime::load_private_key(&private_key_path).expect("valid private key must load");
        std::fs::write(
            &private_key_path,
            vec![0_u8; TLS_PRIVATE_KEY_MAX_BYTES_V1 + 1],
        )
        .expect("write oversized private key");
        let error = RelayRuntime::load_private_key(&private_key_path)
            .expect_err("max+1 private key must fail before parse");
        assert!(error.to_string().contains("first-release limit"), "{error}");
    }
    #[cfg(unix)]
    #[test]
    fn relay_tls_private_key_requires_private_direct_file() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let rcgen::CertifiedKey { signing_key, .. } =
            rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
                .expect("generate test certificate");
        let directory = secure_test_tempdir();
        let private_key_path = directory.path().join("relay-key.pem");
        let link_path = directory.path().join("relay-key.link");
        std::fs::write(&private_key_path, signing_key.serialize_pem()).expect("write private key");
        std::fs::set_permissions(&private_key_path, std::fs::Permissions::from_mode(0o644))
            .expect("set unsafe private-key permissions");
        let error = RelayRuntime::load_private_key(&private_key_path)
            .expect_err("world-readable private key must fail closed");
        assert!(error.to_string().contains("group or other"), "{error}");

        std::fs::set_permissions(&private_key_path, std::fs::Permissions::from_mode(0o600))
            .expect("protect private key");
        symlink(&private_key_path, &link_path).expect("create private-key symlink");
        let error = RelayRuntime::load_private_key(&link_path)
            .expect_err("private-key symlink must fail closed");
        assert!(error.to_string().contains("regular file"), "{error}");
    }
    #[test]
    fn vpn_helper_binding_commits_to_authenticated_transport_trust() {
        let relay_id = [0x11; 32];
        let descriptor_commit = [0x22; 32];
        let trust = RelayTransportTrust {
            quic_multiaddr: "/dns/relay.test/udp/443/quic".to_owned(),
            tls_server_name: "relay.test".to_owned(),
            tls_spki_sha256: [0x33; 32],
            relay_certificate_sha256: [0x44; 32],
            directory_snapshot_digest: [0x55; 32],
            valid_until_ms: u64::MAX,
        };
        let expected =
            vpn_helper_handshake_binding(b"ticket", &relay_id, &descriptor_commit, &trust);
        assert_ne!(expected, [0; 32]);
        let mut changed = trust.clone();
        changed.directory_snapshot_digest[0] ^= 0x01;
        assert_ne!(
            expected,
            vpn_helper_handshake_binding(b"ticket", &relay_id, &descriptor_commit, &changed)
        );
        let mut changed = trust;
        changed.tls_server_name = "other.relay.test".to_owned();
        assert_ne!(
            expected,
            vpn_helper_handshake_binding(b"ticket", &relay_id, &descriptor_commit, &changed)
        );
    }
    #[test]
    fn vpn_helper_ticket_cannot_outlive_authenticated_transport_trust() {
        let mut ticket = sample_helper_ticket([0x11; 16]);
        ticket.expires_at_ms = 101;
        let trust = RelayTransportTrust {
            quic_multiaddr: "/dns/relay.test/udp/443/quic".to_owned(),
            tls_server_name: "relay.test".to_owned(),
            tls_spki_sha256: [0x33; 32],
            relay_certificate_sha256: [0x44; 32],
            directory_snapshot_digest: [0x55; 32],
            valid_until_ms: 100,
        };
        let error = ensure_vpn_helper_ticket_within_trust(&ticket, &trust)
            .expect_err("ticket past trust expiry must fail");
        assert!(error.to_string().contains("outlives"));
        ticket.expires_at_ms = trust.valid_until_ms;
        ensure_vpn_helper_ticket_within_trust(&ticket, &trust)
            .expect("ticket ending at the exclusive trust boundary is accepted");
    }
    #[tokio::test]
    async fn vpn_helper_ticket_replay_is_rejected_after_relay_restart() {
        let directory = secure_test_tempdir();
        let config = VpnConfig {
            enabled: true,
            lease_secs: 60,
            helper_ticket_secret_path: Some(directory.path().join("helper-secret.hex")),
            helper_ticket_replay_store_capacity: 4,
            helper_ticket_replay_store_path: directory.path().join("helper-replays.norito"),
            ..VpnConfig::default()
        };
        let now_ms = 1_000_000;
        let mut ticket = sample_helper_ticket([0x71; 16]);
        ticket.expires_at_ms = now_ms + 30_000;
        let relay_id = ticket.relay_id;
        {
            let ledger = load_vpn_helper_ticket_replay_ledger(&config, &relay_id, now_ms)
                .expect("create durable replay ledger");
            let ledger = Arc::new(StdMutex::new(ledger));
            redeem_vpn_helper_ticket(ledger, &ticket, now_ms)
                .await
                .expect("first redemption must be persisted");
        }
        let reloaded = load_vpn_helper_ticket_replay_ledger(&config, &relay_id, now_ms + 1)
            .expect("reload durable replay ledger");
        let error =
            redeem_vpn_helper_ticket(Arc::new(StdMutex::new(reloaded)), &ticket, now_ms + 1)
                .await
                .expect_err("persisted redemption must survive restart");
        assert!(matches!(
            error,
            HandshakeError::HelperTicket(VpnHelperTicketError::Replayed)
        ));
    }
    #[test]
    fn vpn_helper_ticket_replay_ledger_fails_closed_on_corrupt_state() {
        let directory = secure_test_tempdir();
        let replay_path = directory.path().join("helper-replays.norito");
        std::fs::write(&replay_path, b"not a norito replay snapshot")
            .expect("write corrupt replay ledger");
        let config = VpnConfig {
            enabled: true,
            lease_secs: 60,
            helper_ticket_secret_path: Some(directory.path().join("helper-secret.hex")),
            helper_ticket_replay_store_capacity: 4,
            helper_ticket_replay_store_path: replay_path,
            ..VpnConfig::default()
        };
        let error = load_vpn_helper_ticket_replay_ledger(&config, &[0x33; 32], 1_000_000)
            .expect_err("corrupt replay state must prevent startup");
        assert!(matches!(
            error,
            ConfigError::Vpn(message) if message.contains("failed to load VPN helper-ticket replay ledger")
        ));
    }
    #[test]
    fn vpn_helper_ticket_lifetime_is_bounded_by_relay_lease_policy() {
        let directory = secure_test_tempdir();
        let config = VpnConfig {
            enabled: true,
            lease_secs: 1,
            helper_ticket_secret_path: Some(directory.path().join("helper-secret.hex")),
            helper_ticket_replay_store_capacity: 4,
            helper_ticket_replay_store_path: directory.path().join("helper-replays.norito"),
            ..VpnConfig::default()
        };
        let now_ms = 50_000;
        let mut ticket = sample_helper_ticket([0x72; 16]);
        ticket.expires_at_ms = now_ms + 1_001;
        let ledger = load_vpn_helper_ticket_replay_ledger(&config, &ticket.relay_id, now_ms)
            .expect("create replay ledger");
        let error = consume_vpn_helper_ticket(
            &StdMutex::new(ledger),
            vpn_helper_ticket_replay_id(&ticket),
            ticket.expires_at_ms,
            now_ms,
        )
        .expect_err("overlong helper ticket must fail closed");
        assert!(matches!(
            error,
            HandshakeError::ReplayStore(message) if message.contains("vpn.lease_secs")
        ));
    }
    #[test]
    fn runtime_rejects_missing_identity() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0"
            }
        "#;
        let config = load_config(json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::Handshake(message))) => {
                assert!(message.contains("identity key is required"));
                assert!(message.contains("handshake.descriptor_manifest_path"));
            }
            Err(other) => panic!("expected missing-identity config error, got {other}"),
            Ok(_) => panic!("runtime must fail closed without a persistent identity key"),
        }
    }
    #[test]
    fn runtime_loads_descriptor_commit_from_certificate() {
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}",
                        "issuer_mldsa_hex": "{issuer_mldsa}"
                    }}
                }}
            }}"#,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
            issuer_mldsa = fixture.issuer_mldsa_hex,
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        assert_eq!(runtime.descriptor_commit(), fixture.descriptor_commit);
        let stored_bundle = runtime
            .certificate_bundle()
            .expect("certificate bundle available");
        assert_eq!(
            stored_bundle.certificate.descriptor_commit,
            fixture.descriptor_commit
        );
    }
    #[test]
    fn runtime_rejects_expired_certificate_at_startup() {
        let fixture = CertificateTestFixture::with_valid_until(2);
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}",
                        "issuer_mldsa_hex": "{issuer_mldsa}"
                    }}
                }}
            }}"#,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
            issuer_mldsa = fixture.issuer_mldsa_hex,
        );
        let config = load_config(&json);
        let err = match RelayRuntime::new(config) {
            Ok(_) => panic!("expired certificate must fail at startup"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("expired"),
            "unexpected startup error: {err}"
        );
    }
    #[test]
    fn resolve_handshake_suites_defaults_without_certificate() {
        let suites = resolve_handshake_suites(None).expect("suites");
        assert_eq!(
            suites,
            vec![
                HandshakeSuite::Nk2Hybrid,
                HandshakeSuite::Nk3PqForwardSecure
            ]
        );
    }
    #[test]
    fn resolve_handshake_suites_uses_certificate_order() {
        let fixture = CertificateTestFixture::new();
        let suites = resolve_handshake_suites(Some(&fixture.bundle)).expect("suites");
        assert_eq!(suites, fixture.bundle.certificate.handshake_suites);
    }
    #[test]
    fn runtime_rejects_descriptor_commit_mismatch() {
        let fixture = CertificateTestFixture::new();
        let mismatch_hex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest}",
                    "descriptor_commit_hex": "{mismatch}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}",
                        "issuer_mldsa_hex": "{issuer_mldsa}"
                    }}
                }}
            }}"#,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
            issuer_mldsa = fixture.issuer_mldsa_hex,
            mismatch = mismatch_hex,
        );
        let config = load_config(&json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::Handshake(message))) => {
                assert!(
                    message.contains("descriptor_commit_hex"),
                    "unexpected error message: {message}"
                );
            }
            Err(other) => panic!("expected handshake config error, got {other:?}"),
            Ok(_) => panic!("expected mismatch to error"),
        }
    }
    #[test]
    fn runtime_config_requires_mldsa65_issuer_key() {
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest}",
                    "certificate": {{
                        "bundle_path": "{bundle}",
                        "issuer_ed25519_hex": "{issuer_ed}"
                    }}
                }}
            }}"#,
            manifest = fixture.manifest_file.path().display(),
            bundle = fixture.bundle_file.path().display(),
            issuer_ed = fixture.issuer_ed25519_hex,
        );
        let file = secure_test_tempfile();
        std::fs::write(file.path(), json).expect("write temp config");
        match RelayConfig::load(file.path()) {
            Err(ConfigError::Handshake(message)) => {
                assert!(
                    message.contains("issuer_mldsa_hex"),
                    "unexpected error message: {message}"
                );
            }
            Err(other) => panic!("expected handshake config error, got {other:?}"),
            Ok(_) => panic!("missing ML-DSA issuer key must fail config validation"),
        }
    }
    fn negotiated_caps_fixture() -> NegotiatedCapabilities {
        NegotiatedCapabilities {
            kem: KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            },
            signatures: vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            padding: 1024,
            descriptor_commit: None,
            grease: Vec::new(),
            constant_rate: None,
        }
    }
    #[test]
    fn validate_client_selection_rejects_kem_mismatch() {
        let negotiated = negotiated_caps_fixture();
        let err = validate_client_selection(
            &negotiated,
            KemId::MlKem1024.code(),
            SignatureId::Dilithium3.code(),
        )
        .expect_err("kem mismatch should fail");
        match err {
            HandshakeError::InvalidClient(field) => {
                assert_eq!(field, "client kem_id does not match negotiated capability");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn validate_client_selection_rejects_signature_mismatch() {
        let negotiated = negotiated_caps_fixture();
        let err = validate_client_selection(
            &negotiated,
            KemId::MlKem768.code(),
            SignatureId::Falcon512.code(),
        )
        .expect_err("signature mismatch should fail");
        match err {
            HandshakeError::InvalidClient(field) => {
                assert_eq!(field, "client sig_id does not match negotiated capability");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn validate_client_selection_accepts_matching_ids() {
        let negotiated = negotiated_caps_fixture();
        validate_client_selection(
            &negotiated,
            KemId::MlKem768.code(),
            SignatureId::Dilithium3.code(),
        )
        .expect("matching ids accepted");
    }
    #[test]
    fn append_grease_tlvs_preserves_order() {
        let base = vec![0xAA, 0xBB];
        let grease = vec![
            GreaseEntry {
                ty: 0x7f10,
                value: vec![0x01],
            },
            GreaseEntry {
                ty: 0x7f11,
                value: vec![0x02, 0x03],
            },
        ];
        let appended = append_grease_tlvs(base.clone(), &grease).expect("append grease");
        let expected = [
            0xAA, 0xBB, 0x7f, 0x10, 0x00, 0x01, 0x01, 0x7f, 0x11, 0x00, 0x02, 0x02, 0x03,
        ];
        assert_eq!(appended, expected);
    }
    #[test]
    fn append_grease_tlvs_rejects_oversized_values_without_truncation() {
        let err = append_grease_tlvs(
            Vec::new(),
            &[GreaseEntry {
                ty: 0x7F20,
                value: vec![0xAB; usize::from(u16::MAX) + 1],
            }],
        )
        .expect_err("oversized GREASE TLV must fail");
        assert!(matches!(
            err,
            CapabilityError::CapabilityValueTooLarge {
                ty: 0x7F20,
                length
            } if length == usize::from(u16::MAX) + 1
        ));
    }
    #[test]
    fn append_grease_tlvs_rejects_oversized_aggregate_before_growth() {
        let err = append_grease_tlvs(vec![0xAA; capability::MAX_CAP_VECTOR_LEN + 1], &[])
            .expect_err("oversized base vector must fail before appending");
        assert!(matches!(err, CapabilityError::CapabilityVectorTooLarge));

        let base = vec![0xAA; capability::MAX_CAP_VECTOR_LEN - 4];
        let err = append_grease_tlvs(
            base,
            &[GreaseEntry {
                ty: 0x7F21,
                value: vec![0xBB],
            }],
        )
        .expect_err("aggregate GREASE vector must stay within the parser ceiling");
        assert!(matches!(err, CapabilityError::CapabilityVectorTooLarge));
    }
    #[test]
    fn runtime_honours_private_manifest_identity_key() {
        let seed_hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let manifest = secure_test_tempfile();
        std::fs::write(
            manifest.path(),
            format!(r#"{{"identity_private_key_hex":"{seed_hex}"}}"#),
        )
        .expect("write identity manifest");
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#,
            manifest_path = manifest.path().display(),
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();
        let seed_bytes = hex::decode(seed_hex).expect("valid hex");
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_bytes);
        let expected_private =
            PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("configured key parse");
        let expected_pair =
            KeyPair::from_private_key(expected_private).expect("configured keypair derive");
        assert_eq!(
            context.identity_key.public_key(),
            expected_pair.public_key()
        );
    }
    #[test]
    fn runtime_enables_pow_when_required() {
        let dir = secure_test_tempdir();
        let replay_path = dir.path().join("ticket-replays.norito");
        let manifest = secure_test_identity_manifest();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{}"
                }},
                "pow": {{
                    "required": true,
                    "difficulty": 6,
                    "max_future_skew_secs": 120,
                    "min_ticket_ttl_secs": 10,
                    "revocation_store_path": "{}"
                }}
            }}"#,
            manifest.path().display(),
            replay_path.display()
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();
        assert!(context.dos.is_pow_required());
        assert_eq!(context.dos.current_pow_parameters().difficulty(), 6);
        let replay_state = context.ticket_replays.lock().expect("ticket replay lock");
        assert_eq!(replay_state.capacity, 8_192);
    }
    #[cfg(unix)]
    #[test]
    fn runtime_fails_closed_on_corrupt_ticket_replay_snapshot() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = secure_test_tempdir();
        let replay_path = dir.path().join("ticket-replays.norito");
        std::fs::write(&replay_path, b"corrupt replay snapshot").expect("write corrupt snapshot");
        std::fs::set_permissions(&replay_path, std::fs::Permissions::from_mode(0o600))
            .expect("make corrupt replay snapshot owner-private");
        let manifest = secure_test_identity_manifest();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{}"
                }},
                "pow": {{
                    "required": true,
                    "difficulty": 6,
                    "max_future_skew_secs": 120,
                    "min_ticket_ttl_secs": 10,
                    "revocation_store_path": "{}"
                }}
            }}"#,
            manifest.path().display(),
            replay_path.display()
        );
        let config = load_config(&json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::TicketReplayStore(message))) => {
                assert!(
                    message.contains("parse"),
                    "unexpected replay-store error: {message}"
                );
            }
            Err(other) => panic!("expected ticket replay-store error, got {other:?}"),
            Ok(_) => panic!("corrupt ticket replay state must fail startup"),
        }
    }
    #[test]
    fn runtime_loads_identity_from_manifest() {
        let seed_hex = "c1d1c2f493ad2db3fbc5ff0bfb8bb4e0f2c5c2d9e9caa8ffd5d38a1808fa4c55";
        let manifest = secure_test_tempfile();
        std::fs::write(
            manifest.path(),
            format!(
                r#"{{
                    "version": 1,
                    "identity": {{
                        "ed25519_private_key_hex": "{seed_hex}"
                    }}
                }}"#
            ),
        )
        .expect("write manifest");
        let manifest_path = manifest.path().to_str().expect("path to utf-8");
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new(config).expect("runtime");
        let context = runtime.circuit_context();
        let seed_bytes = hex::decode(seed_hex).expect("valid hex");
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_bytes);
        let expected_private =
            PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("manifest key parse");
        let expected_pair =
            KeyPair::from_private_key(expected_private).expect("manifest keypair derive");
        assert_eq!(
            context.identity_key.public_key(),
            expected_pair.public_key()
        );
    }
    #[test]
    fn runtime_fails_when_manifest_missing_key() {
        let manifest = secure_test_tempfile();
        std::fs::write(
            manifest.path(),
            r#"{ "version": 1, "identity": { "note": "no private key yet" } }"#,
        )
        .expect("write manifest");
        let manifest_path = manifest.path().to_str().expect("path to utf-8");
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
        );
        let config = load_config(&json);
        match RelayRuntime::new(config) {
            Err(RelayError::Config(ConfigError::DescriptorManifest { message, .. })) => {
                assert!(
                    message.contains("missing"),
                    "unexpected manifest error message: {message}"
                );
            }
            Err(other) => panic!("expected manifest error, got {other:?}"),
            Ok(_) => panic!("expected manifest error, got Ok(_)"),
        }
    }
    #[tokio::test]
    async fn bandwidth_proof_populates_accumulator() {
        let accumulator = Arc::new(Mutex::new(RelayPerformanceAccumulator::new(TEST_RELAY_ID)));
        let proof = sample_bandwidth_proof(7, 0x34, 1_024);
        let encoded = proof.encode();
        let config = PrivacyConfig {
            min_handshakes: 0,
            flush_delay_buckets: 1,
            force_flush_buckets: 1,
            ..PrivacyConfig::default()
        };
        let privacy = Arc::new(PrivacyAggregator::new(config));
        let privacy_events = Arc::new(PrivacyEventBuffer::new(64));
        let mode = RelayMode::Entry;
        let remote: SocketAddr = "127.0.0.1:0".parse().expect("socket addr");
        RelayRuntime::handle_bandwidth_proof(
            &encoded,
            &accumulator,
            TEST_RELAY_ID,
            None,
            Arc::clone(&privacy),
            Arc::clone(&privacy_events),
            mode,
            None,
            remote,
        )
        .await
        .expect("proof accepted");
        {
            let guard = accumulator.lock().await;
            let summaries = guard.summaries();
            assert_eq!(summaries.len(), 1);
            let summary = &summaries[0];
            assert_eq!(summary.epoch, proof.epoch);
            assert_eq!(summary.verified_bandwidth_bytes, proof.verified_bytes);
            assert_eq!(summary.measurement_ids, vec![proof.measurement_id]);
        }
        // Duplicate proof must be ignored.
        RelayRuntime::handle_bandwidth_proof(
            &encoded,
            &accumulator,
            TEST_RELAY_ID,
            None,
            Arc::clone(&privacy),
            Arc::clone(&privacy_events),
            mode,
            None,
            remote,
        )
        .await
        .expect("duplicate handled");
        let guard = accumulator.lock().await;
        let summaries = guard.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].measurement_ids.len(), 1);
        let rendered = privacy.render_prometheus(
            RelayMode::Entry,
            SystemTime::now() + Duration::from_secs(600),
        );
        assert!(
            rendered.contains("soranet_privacy_verified_bytes_total"),
            "privacy metrics missing bandwidth line: {rendered}"
        );
    }
    #[test]
    fn incentive_metrics_expose_relay_label() {
        let summary = EpochSummary {
            epoch: 3,
            uptime_seconds: 90,
            scheduled_uptime_seconds: 120,
            verified_bandwidth_bytes: 2_048,
            confidence_floor_per_mille: 875,
            measurement_ids: vec![[0x99; 32]],
        };
        let metrics = render_incentive_prometheus(TEST_RELAY_ID, &[summary], RelayMode::Entry)
            .expect("bounded incentive metrics");
        let relay_hex = hex::encode(TEST_RELAY_ID);
        assert!(
            metrics.contains(&format!("relay=\"{relay_hex}\"")),
            "metrics should include relay label: {metrics}"
        );
        assert!(
            metrics.contains("soranet_relay_bandwidth_verified_bytes_total"),
            "bandwidth metric missing: {metrics}"
        );
    }
    #[test]
    fn incentive_metrics_reject_epoch_count_max_plus_one() {
        let summary = EpochSummary {
            epoch: 3,
            uptime_seconds: u64::MAX,
            scheduled_uptime_seconds: u64::MAX,
            verified_bandwidth_bytes: u128::MAX,
            confidence_floor_per_mille: 1_000,
            measurement_ids: Vec::new(),
        };
        let exact = vec![summary.clone(); INCENTIVE_MAX_ACTIVE_EPOCHS_V1];
        render_incentive_prometheus(TEST_RELAY_ID, &exact, RelayMode::Entry)
            .expect("exact epoch corridor");
        let overflow = vec![summary; INCENTIVE_MAX_ACTIVE_EPOCHS_V1 + 1];
        assert!(render_incentive_prometheus(TEST_RELAY_ID, &overflow, RelayMode::Entry).is_err());
    }
    #[test]
    fn ensure_nonzero_accepts_non_zero_bytes() {
        let bytes = [0u8, 1, 0, 2];
        assert!(ensure_nonzero("test", &bytes).is_ok());
    }
    #[test]
    fn ensure_nonzero_rejects_all_zero_bytes() {
        let bytes = [0u8; 4];
        let err = ensure_nonzero("all zero rejected", &bytes).expect_err("should fail");
        assert!(matches!(
            err,
            HandshakeError::InvalidClient("all zero rejected")
        ));
    }
    #[test]
    fn admin_authorization_verifies_the_complete_bearer_token() {
        let file = secure_test_tempfile();
        std::fs::write(file.path(), b"soranet-admin-token-0123456789abcdef\n")
            .expect("write admin token");
        let authorization =
            AdminAuthorization::load(file.path()).expect("load protected admin token");
        let rendered = format!("{authorization:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(&authorization.token_hash.to_hex().to_string()));
        assert!(authorization.matches("soranet-admin-token-0123456789abcdef"));
        assert!(!authorization.matches("soranet-admin-token-0123456789abcdeg"));
        assert!(!authorization.matches("soranet-admin-token-0123456789abcde"));
    }
    #[test]
    fn admin_authorization_rejects_placeholder_secret() {
        let file = secure_test_tempfile();
        std::fs::write(file.path(), b"REPLACE_ME").expect("write placeholder token");
        let err = AdminAuthorization::load(file.path()).expect_err("short token must fail closed");
        assert!(matches!(err, ConfigError::Admin(message) if message.contains("32 to 256")));
    }
    #[cfg(unix)]
    #[test]
    fn admin_authorization_rejects_group_readable_secret() {
        use std::os::unix::fs::PermissionsExt as _;
        let file = secure_test_tempfile();
        std::fs::write(file.path(), b"soranet-admin-token-0123456789abcdef")
            .expect("write admin token");
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o640))
            .expect("set dedicated-group token permissions");
        let err = AdminAuthorization::load(file.path()).expect_err("group access must fail");
        assert!(matches!(err, ConfigError::Admin(message) if message.contains("group or other")));
    }
    #[cfg(unix)]
    #[test]
    fn admin_authorization_rejects_world_readable_secret() {
        use std::os::unix::fs::PermissionsExt as _;
        let file = secure_test_tempfile();
        std::fs::write(file.path(), b"soranet-admin-token-0123456789abcdef")
            .expect("write admin token");
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o644))
            .expect("set world-readable token permissions");
        let err = AdminAuthorization::load(file.path()).expect_err("broad permissions must fail");
        assert!(matches!(err, ConfigError::Admin(message) if message.contains("group or other")));
    }
    #[cfg(unix)]
    #[test]
    fn admin_authorization_rejects_symbolic_link() {
        use std::os::unix::fs::symlink;
        let directory = secure_test_tempdir();
        let target = directory.path().join("admin.token");
        let link = directory.path().join("admin.link");
        std::fs::write(&target, b"soranet-admin-token-0123456789abcdef")
            .expect("write admin token");
        symlink(&target, &link).expect("create admin token symlink");
        let err = AdminAuthorization::load(&link).expect_err("symlink must fail closed");
        assert!(matches!(err, ConfigError::Admin(message) if message.contains("regular file")));
    }
    #[tokio::test]
    async fn admin_request_reader_accepts_fragmented_headers() {
        let (mut writer, mut reader) = duplex(ADMIN_MAX_HEADER_BYTES_V1);
        let long_header = "a".repeat(2_048);
        let request = format!("GET /healthz HTTP/1.1\r\nX-Padding: {long_header}\r\n\r\n");
        writer
            .write_all(request.as_bytes())
            .await
            .expect("write fragmented request fixture");
        let parsed = RelayRuntime::read_admin_request(&mut reader, Duration::from_secs(1))
            .await
            .expect("bounded complete request");
        assert_eq!(parsed, request);
    }
    #[tokio::test]
    async fn admin_request_reader_rejects_oversized_incomplete_headers() {
        let (mut writer, mut reader) = duplex(ADMIN_MAX_HEADER_BYTES_V1);
        writer
            .write_all(&vec![b'a'; ADMIN_MAX_HEADER_BYTES_V1])
            .await
            .expect("write oversized request fixture");
        let error = RelayRuntime::read_admin_request(&mut reader, Duration::from_secs(1))
            .await
            .expect_err("oversized headers must fail closed");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
    }
    #[tokio::test]
    async fn admin_request_reader_times_out_stalled_clients() {
        let (_writer, mut reader) = duplex(64);
        let error = RelayRuntime::read_admin_request(&mut reader, Duration::from_millis(10))
            .await
            .expect_err("stalled request must time out");
        assert_eq!(error.kind(), ErrorKind::TimedOut);
    }
    #[test]
    fn admin_connection_permits_enforce_capacity() {
        let permits = Arc::new(Semaphore::new(1));
        let permit = RelayRuntime::try_admin_connection_permit(&permits)
            .expect("first connection should be admitted");
        assert!(RelayRuntime::try_admin_connection_permit(&permits).is_none());
        drop(permit);
        assert!(RelayRuntime::try_admin_connection_permit(&permits).is_some());
    }
    #[tokio::test]
    async fn admin_endpoint_serves_privacy_events() {
        const ADMIN_TOKEN: &str = "soranet-test-admin-token-00000001";
        let metrics = Arc::new(Metrics::new());
        let privacy = Arc::new(PrivacyAggregator::new(PrivacyConfig::default()));
        let privacy_events = Arc::new(PrivacyEventBuffer::new(8));
        let proxy_policy_events = Arc::new(ProxyPolicyEventBuffer::new(8));
        let performance = Arc::new(Mutex::new(RelayPerformanceAccumulator::new(TEST_RELAY_ID)));
        let mode = RelayMode::Middle;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let event_time = SystemTime::now();
        privacy_events.record_handshake_success(privacy_mode, event_time, Some(37), Some(5));
        privacy_events.record_throttle(
            privacy_mode,
            event_time,
            SoranetPrivacyThrottleScopeV1::DescriptorQuota,
        );
        proxy_policy_events.record_downgrade(privacy_mode, event_time, Some("downgrade"));
        let listener = match StdTcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping admin_endpoint_serves_privacy_events: {error}");
                return;
            }
            Err(error) => panic!("failed to bind test listener: {error}"),
        };
        let addr = listener
            .local_addr()
            .expect("retrieve listener addr for admin endpoint");
        drop(listener);
        let server = {
            let resources = AdminResources {
                metrics: Arc::clone(&metrics),
                privacy: Arc::clone(&privacy),
                privacy_events: Arc::clone(&privacy_events),
                proxy_policy_events: Arc::clone(&proxy_policy_events),
                performance: Arc::clone(&performance),
            };
            let authorization = Arc::new(AdminAuthorization {
                token_hash: blake3::hash(ADMIN_TOKEN.as_bytes()),
            });
            tokio::spawn(async move {
                let _ =
                    RelayRuntime::serve_admin(resources, TEST_RELAY_ID, addr, mode, authorization)
                        .await;
            })
        };
        sleep(Duration::from_millis(25)).await;
        let mut unauthorized_stream = TcpStream::connect(addr)
            .await
            .expect("connect to protected admin endpoint");
        unauthorized_stream
            .write_all(
                b"GET /privacy/events HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("write unauthenticated HTTP request");
        let mut unauthorized_response = Vec::new();
        unauthorized_stream
            .read_to_end(&mut unauthorized_response)
            .await
            .expect("read unauthenticated HTTP response");
        let unauthorized_text =
            String::from_utf8(unauthorized_response).expect("response must be UTF-8");
        assert!(
            unauthorized_text.starts_with("HTTP/1.1 401 Unauthorized"),
            "expected authentication failure, got: {unauthorized_text}"
        );
        let mut stream = TcpStream::connect(addr)
            .await
            .expect("connect to admin endpoint");
        stream
            .write_all(
                concat!(
                    "GET /privacy/events HTTP/1.1\r\n",
                    "Host: localhost\r\n",
                    "Authorization: Bearer soranet-test-admin-token-00000001\r\n",
                    "Connection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .expect("write HTTP request");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read HTTP response");
        let text = String::from_utf8(response).expect("response must be UTF-8");
        assert!(
            text.starts_with("HTTP/1.1 200 OK"),
            "expected 200 OK, got: {text}"
        );
        assert!(
            text.to_ascii_lowercase()
                .contains("content-type: application/x-ndjson"),
            "missing NDJSON content-type: {text}"
        );
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default();
        assert!(
            body.contains("HandshakeSuccess"),
            "handshake event missing from body: {body}"
        );
        assert!(
            body.contains("Throttle"),
            "throttle event missing from body: {body}"
        );
        assert!(
            privacy_events.drain_ndjson().is_empty(),
            "buffer should drain after serving HTTP response"
        );
        stream.shutdown().await.expect("shutdown admin stream");
        let mut downgrade_stream = TcpStream::connect(addr)
            .await
            .expect("connect to proxy policy endpoint");
        downgrade_stream
            .write_all(
                concat!(
                    "GET /policy/proxy-toggle HTTP/1.1\r\n",
                    "Host: localhost\r\n",
                    "Authorization: Bearer soranet-test-admin-token-00000001\r\n",
                    "Connection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .expect("write downgrade HTTP request");
        let mut downgrade_response = Vec::new();
        downgrade_stream
            .read_to_end(&mut downgrade_response)
            .await
            .expect("read downgrade response");
        let downgrade_text =
            String::from_utf8(downgrade_response).expect("downgrade response must be UTF-8");
        assert!(
            downgrade_text.starts_with("HTTP/1.1 200 OK"),
            "expected downgrade endpoint to return 200 OK: {downgrade_text}"
        );
        assert!(
            downgrade_text
                .to_ascii_lowercase()
                .contains("content-type: application/x-ndjson"),
            "downgrade endpoint missing NDJSON content-type: {downgrade_text}"
        );
        assert!(
            downgrade_text.contains("\"reason\":\"downgrade\""),
            "downgrade payload missing downgrade event: {downgrade_text}"
        );
        assert!(
            downgrade_text.contains("\"detail\":\"downgrade\""),
            "downgrade payload missing slug: {downgrade_text}"
        );
        downgrade_stream
            .shutdown()
            .await
            .expect("shutdown proxy policy stream");
        server.abort();
    }
    #[tokio::test]
    async fn privacy_events_endpoint_drains_buffer() {
        let metrics = Metrics::new();
        let privacy = PrivacyAggregator::new(PrivacyConfig::default());
        let privacy_events = PrivacyEventBuffer::new(8);
        let proxy_policy_events = ProxyPolicyEventBuffer::new(8);
        let performance = Mutex::new(RelayPerformanceAccumulator::new(TEST_RELAY_ID));
        let mode = RelayMode::Entry;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let sample_time = SystemTime::now();
        privacy_events.record_handshake_success(privacy_mode, sample_time, Some(42), Some(7));
        privacy_events.record_throttle(
            privacy_mode,
            sample_time,
            SoranetPrivacyThrottleScopeV1::Congestion,
        );
        let context = AdminRenderContext {
            metrics: &metrics,
            privacy: &privacy,
            privacy_events: &privacy_events,
            proxy_policy_events: &proxy_policy_events,
            performance: &performance,
        };
        let response =
            RelayRuntime::render_admin_response("/privacy/events", context, TEST_RELAY_ID, mode)
                .await;
        let parts: Vec<&str> = response.split("\r\n\r\n").collect();
        assert_eq!(parts.len(), 2, "expected HTTP header/body split");
        let headers = parts[0];
        let body = parts[1];
        assert!(
            headers.contains(NDJSON_CONTENT_TYPE),
            "response should advertize ndjson content-type: {headers}"
        );
        assert!(
            !body.trim().is_empty(),
            "expected privacy events body to contain entries"
        );
        let expected_length = format!("content-length: {}", body.len());
        assert!(
            headers.to_ascii_lowercase().contains(&expected_length),
            "content-length header should match body size: {headers}"
        );
        let drained_context = AdminRenderContext {
            metrics: &metrics,
            privacy: &privacy,
            privacy_events: &privacy_events,
            proxy_policy_events: &proxy_policy_events,
            performance: &performance,
        };
        let drained = RelayRuntime::render_admin_response(
            "/privacy/events",
            drained_context,
            TEST_RELAY_ID,
            mode,
        )
        .await;
        let drained_parts: Vec<&str> = drained.split("\r\n\r\n").collect();
        assert_eq!(drained_parts.len(), 2, "expected HTTP header/body split");
        assert!(
            drained_parts[1].is_empty(),
            "privacy event buffer should be empty after drain"
        );
        assert!(
            drained_parts[0]
                .to_ascii_lowercase()
                .contains("content-length: 0"),
            "empty response should advertise zero content length"
        );
    }
    #[test]
    fn downgrade_events_hit_metrics_and_proxy_queue() {
        let metrics = Metrics::new();
        let proxy_policy_events = ProxyPolicyEventBuffer::new(4);
        let mode = RelayMode::Entry;
        let privacy_mode: SoranetPrivacyModeV1 = mode.into();
        let warnings = vec![CapabilityWarning {
            capability_type: 0x0101,
            message: "No overlapping handshake suite between client and relay".to_string(),
        }];
        let detail = downgrade_detail_from_warnings(&warnings).expect("detail slug");
        for warning in &warnings {
            metrics.record_downgrade(&warning.message);
        }
        let event_time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        proxy_policy_events.record_downgrade(privacy_mode, event_time, Some(&detail));
        let rendered = metrics.render_prometheus(mode, proxy_policy_events.queue_depth() as u64);
        let label_block =
            "mode=\"entry\",constant_rate_profile=\"unknown\",constant_rate_neighbors=\"0\"";
        assert!(
            rendered.contains(&format!(
                "sn16_handshake_downgrade_total{{{label_block},reason=\"{detail}\"}} 1"
            )),
            "downgrade counter missing or mislabeled: {rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "soranet_proxy_policy_queue_depth{{{label_block}}} 1"
            )),
            "proxy queue depth gauge missing: {rendered}"
        );
        let ndjson = proxy_policy_events.drain_ndjson();
        assert!(
            ndjson.contains("\"reason\":\"downgrade\""),
            "proxy policy NDJSON must tag downgrade reason slug: {ndjson}"
        );
        assert!(
            ndjson.contains(&format!("\"detail\":\"{detail}\"")),
            "proxy policy NDJSON should carry the slugged detail: {ndjson}"
        );
    }
}

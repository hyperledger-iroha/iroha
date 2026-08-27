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
        vpn::VpnSession,
    };
    use ed25519_dalek::SigningKey;
    use iroha_crypto::{
        SessionKey, Signature,
        soranet::{
            certificate::{
                CapabilityToggle, RelayCapabilityFlagsV1, RelayCertificateBundleV2,
                RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
            },
            handshake::{
                DEFAULT_CLIENT_CAPABILITIES, HandshakeSuite, RuntimeParams as NoiseRuntimeParams,
                build_client_hello, update_suite_list,
            },
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
    use norito::codec::Encode;
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
    use std::{
        io::ErrorKind,
        net::TcpListener as StdTcpListener,
        num::NonZeroU32,
        path::Path,
        sync::Arc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tempfile::{NamedTempFile, TempDir};
    #[cfg(unix)]
    use tokio::net::UnixListener;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt, duplex},
        net::TcpStream,
        time::sleep,
    };
    const TEST_RELAY_ID: RelayId = [0xAB; 32];
    #[test]
    fn exit_compliance_context_uses_stable_reason_without_raw_channel() {
        let error = ExitStreamError::FilesystemPublicationDisabled {
            stream: "norito-stream",
            channel: "sensitive-channel-id".to_owned(),
        };
        let (stream, channel, reason) = error.compliance_context();
        assert_eq!(stream, Some("norito-stream"));
        assert_eq!(channel, Some("sensitive-channel-id"));
        assert_eq!(reason, "filesystem_publication_disabled");
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
    fn secure_test_tempdir() -> TempDir {
        let directory = tempfile::Builder::new()
            .prefix("soranet-relay-test-")
            .tempdir_in(std::env::current_dir().expect("current test directory"))
            .expect("create private test directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("protect test directory");
        }
        directory
    }
    fn test_settlement_store(directory: &TempDir) -> Arc<VpnSettlementStore> {
        let spool_dir = std::fs::canonicalize(directory.path()).expect("canonical spool directory");
        let owner_lock = open_private_vpn_spool_lock(&spool_dir.join(VPN_SETTLEMENT_OWNER_LOCK_V1))
            .expect("test settlement owner lock");
        Arc::new(VpnSettlementStore {
            spool_dir,
            _owner_lock: owner_lock,
            operation: StdMutex::new(VpnSettlementOperationState::default()),
            poisoned: AtomicBool::new(false),
        })
    }
    fn signed_usage_voucher(key_pair: &KeyPair, body: VpnUsageVoucherBodyV1) -> VpnUsageVoucherV1 {
        VpnUsageVoucherV1::try_sign(body, key_pair.private_key())
            .expect("usage voucher fixture should sign")
    }
    fn usage_voucher_envelope(
        helper_ticket: &VpnHelperTicketV1,
        key_pair: &KeyPair,
        sequence: u64,
        ingress_bytes: u64,
        egress_bytes: u64,
        active_ms: u64,
        issued_at_ms: u64,
    ) -> VpnUsageVoucherEnvelopeV1 {
        let body = VpnUsageVoucherBodyV1 {
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            relay_id: helper_ticket.relay_id,
            sequence,
            ingress_bytes,
            egress_bytes,
            active_ms,
            issued_at_ms,
        };
        VpnUsageVoucherEnvelopeV1 {
            fee_ceiling: helper_ticket
                .tariff
                .fee_ceiling(&body)
                .expect("bounded voucher fixture fee"),
            voucher: signed_usage_voucher(key_pair, body),
        }
    }
    fn active_voucher_authorization(
        helper_ticket: &VpnHelperTicketV1,
    ) -> Arc<Mutex<VpnVoucherAuthorization>> {
        let now_ms = unix_time_ms(SystemTime::now());
        let envelope = usage_voucher_envelope(
            helper_ticket,
            &sample_metering_key_pair(),
            0,
            64 * 1024,
            64 * 1024,
            5_000,
            now_ms,
        );
        let mut authorization = VpnVoucherAuthorization::new(helper_ticket, 1_048_576, 5_000);
        authorization
            .accept_envelope_at(&envelope, now_ms)
            .expect("test prepaid voucher must be valid");
        authorization.begin_service();
        Arc::new(Mutex::new(authorization))
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
    fn sample_relay_identity_key_pair() -> KeyPair {
        KeyPair::try_from_seed(vec![0x67; 32], Algorithm::Ed25519)
            .expect("derive VPN relay identity fixture key")
    }
    fn bind_sample_helper_session(
        overlay: &VpnOverlay,
        session: VpnSession,
        helper_ticket: VpnHelperTicketV1,
    ) -> VpnSessionHandle {
        overlay
            .bind_helper_session(
                session,
                &helper_ticket,
                Arc::new(sample_relay_identity_key_pair()),
            )
            .expect("fixture helper ticket matches the relay identity signer")
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
        let relay_key_pair = sample_relay_identity_key_pair();
        let (_, relay_public_key) = relay_key_pair
            .public_key()
            .try_to_bytes()
            .expect("fixture relay identity public key");
        let mut relay_id = [0_u8; 32];
        relay_id.copy_from_slice(relay_public_key);
        let address_plan = derive_vpn_session_address_plan_v1(session_id);
        VpnHelperTicketV1 {
            session_id,
            quote_id: [0x11; 32],
            lease_id: [0x12; 32],
            account_hash: [0x22; 32],
            relay_id,
            payment_tx_hash: [0x44; 32],
            metering_public_key: key_pair.public_key().clone(),
            tariff: sample_vpn_tariff(),
            client_ipv4_address: address_plan.client_ipv4_address,
            client_ipv6_address: address_plan.client_ipv6_address,
            network_policy_hash: [0x55; 32],
            valid_after_ms: 1,
            expires_at_ms: u64::MAX,
        }
    }
    #[test]
    fn route_open_ingress_metrics_use_adapter_once() {
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
    }
    #[test]
    fn route_open_ingress_metrics_fallback_to_session() {
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
        let helper_ticket = sample_helper_ticket([0x5A; 16]);
        let bootstrap = build_vpn_backend_bootstrap(&helper_ticket);
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
        assert_eq!(
            decoded.bootstrap.client_ipv4_address,
            helper_ticket.client_ipv4_address
        );
        assert_eq!(
            decoded.bootstrap.client_ipv6_address,
            helper_ticket.client_ipv6_address
        );
        assert_eq!(decoded.bootstrap.session_routes.len(), 2);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn vpn_backend_connection_authenticates_socket_and_peer_before_handoff() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let directory = secure_test_tempdir();
        let path = std::fs::canonicalize(directory.path())
            .expect("canonical backend directory")
            .join("backend.sock");
        let listener = UnixListener::bind(&path).expect("bind backend test socket");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660))
            .expect("protect backend test socket");
        let metadata = std::fs::symlink_metadata(&path).expect("backend socket metadata");
        let expected_uid = metadata.uid();
        let expected_gid = metadata.gid();
        let accept = tokio::spawn(async move {
            listener
                .accept()
                .await
                .expect("accept authenticated relay")
                .0
        });

        let backend = connect_authenticated_vpn_backend(&path, expected_uid, expected_gid)
            .await
            .expect("authenticate backend socket and peer");
        let _relay_peer = accept.await.expect("backend accept task");
        verify_vpn_backend_peer_credentials(&backend, expected_uid, expected_gid)
            .expect("matching peer credentials");
        let wrong_peer = verify_vpn_backend_peer_credentials(&backend, u32::MAX, expected_gid)
            .expect_err("wrong backend peer UID must fail closed");
        assert_eq!(wrong_peer.kind(), ErrorKind::PermissionDenied);
    }
    #[test]
    fn helper_ticket_is_consumed_before_accounting_registration_and_backend_work() {
        let source = include_str!("../runtime.rs");
        let start = source
            .find("async fn establish_circuit(")
            .expect("connection handler");
        let end = source[start..]
            .find("async fn monitor_circuit(")
            .map(|offset| start + offset)
            .expect("circuit monitor");
        let handler = &source[start..end];
        let consume = handler
            .find("commit_vpn_helper_ticket_reservation")
            .expect("durable helper-ticket consumption");
        let success = handler
            .find("metrics.record_success()")
            .expect("success accounting");
        let register = handler
            .find("registry.register(")
            .expect("circuit registration");
        let monitor = handler
            .find("Self::monitor_circuit(")
            .expect("post-handshake circuit tasks");
        assert!(
            consume < success,
            "ticket must be spent before success accounting"
        );
        assert!(
            consume < register,
            "ticket must be spent before registry admission"
        );
        assert!(
            consume < monitor,
            "ticket must be spent before backend/task dispatch"
        );
    }
    #[test]
    fn vpn_settlement_is_durable_before_backend_protocol_handoff() {
        let source = include_str!("../runtime.rs");
        let start = source
            .find("async fn serve_vpn_backend_tunnel(")
            .expect("VPN backend tunnel function");
        let end = source[start..]
            .find("async fn serve_vpn_backend_tunnel_stream")
            .map(|offset| start + offset)
            .expect("VPN backend stream handoff function");
        let admission = &source[start..end];
        let authenticate = admission
            .find("connect_authenticated_vpn_backend")
            .expect("authenticated backend connect");
        let settlement = admission
            .find("persist_initial_settlement")
            .expect("durable settlement reservation");
        let handoff = admission
            .find("Self::serve_vpn_backend_tunnel_stream")
            .expect("backend protocol handoff");
        assert!(
            authenticate < settlement,
            "backend authentication must precede settlement reservation"
        );
        assert!(
            settlement < handoff,
            "settlement reservation must precede bootstrap handoff"
        );
        assert!(
            !admission[..settlement].contains("write_vpn_backend_bootstrap"),
            "no backend bootstrap byte may precede durable settlement"
        );
    }
    #[cfg(unix)]
    #[test]
    fn vpn_backend_socket_rejects_wrong_identity_mode_and_symlink() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let directory = secure_test_tempdir();
        let canonical = std::fs::canonicalize(directory.path()).expect("canonical backend dir");
        let path = canonical.join("backend.sock");
        let _listener =
            std::os::unix::net::UnixListener::bind(&path).expect("bind backend socket fixture");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660))
            .expect("protect backend socket fixture");
        let metadata = std::fs::symlink_metadata(&path).expect("backend socket metadata");
        let expected_uid = metadata.uid();
        let expected_gid = metadata.gid();
        inspect_vpn_backend_socket(&path, expected_uid, expected_gid)
            .expect("direct pinned backend socket");

        let wrong_uid = inspect_vpn_backend_socket(&path, u32::MAX, expected_gid)
            .expect_err("wrong socket owner must fail closed");
        assert_eq!(wrong_uid.kind(), ErrorKind::PermissionDenied);
        let wrong_gid = inspect_vpn_backend_socket(&path, expected_uid, u32::MAX)
            .expect_err("wrong socket group must fail closed");
        assert_eq!(wrong_gid.kind(), ErrorKind::PermissionDenied);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666))
            .expect("make backend socket other-writable");
        let unsafe_mode = inspect_vpn_backend_socket(&path, expected_uid, expected_gid)
            .expect_err("other-writable socket must fail closed");
        assert_eq!(unsafe_mode.kind(), ErrorKind::PermissionDenied);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660))
            .expect("restore backend socket mode");

        let alias = canonical.join("backend-alias.sock");
        std::os::unix::fs::symlink(&path, &alias).expect("symlink backend socket");
        let symlink_error = inspect_vpn_backend_socket(&alias, expected_uid, expected_gid)
            .expect_err("socket symlink must fail closed");
        assert_eq!(symlink_error.kind(), ErrorKind::PermissionDenied);
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
    async fn vpn_initial_prepaid_voucher_is_accepted_before_service_starts() {
        let helper_ticket = sample_helper_ticket([0xA0; 16]);
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig::default()));
        let now_ms = unix_time_ms(SystemTime::now());
        let envelope = usage_voucher_envelope(
            &helper_ticket,
            &sample_metering_key_pair(),
            0,
            64 * 1024,
            64 * 1024,
            2_000,
            now_ms,
        );
        let mut payload = Vec::from(VPN_USAGE_VOUCHER_CONTROL_MAGIC.as_slice());
        payload.extend_from_slice(&envelope.encode());
        let cell = VpnCellV1 {
            header: iroha_data_model::soranet::vpn::VpnCellHeaderV1 {
                version: 1,
                class: VpnCellClassV1::Control,
                flags: VpnCellFlagsV1::new(false, false, false, false),
                circuit_id: helper_ticket.session_id,
                flow_label: vpn_flow_label_from_session_id(helper_ticket.session_id),
                sequence: 0,
                ack: 0,
                padding_budget_ms: overlay.config().padding_budget_ms,
                payload_len: 0,
            },
            payload,
        };
        let frame = cell.into_padded_frame().expect("prepaid control frame");
        let (mut peer, mut relay) = duplex(VPN_CELL_LEN * 2);
        peer.write_all(frame.as_ref()).await.expect("write voucher");
        let session = overlay.start_session(Arc::new(Metrics::new()));
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket.clone());
        let adapter = VpnAdapter::new(handle.session().clone(), overlay);
        let authorization = Arc::new(Mutex::new(VpnVoucherAuthorization::new(
            &helper_ticket,
            1_048_576,
            5_000,
        )));

        let accepted = accept_initial_usage_voucher(
            &adapter,
            &mut relay,
            helper_ticket.session_id,
            vpn_flow_label_from_session_id(helper_ticket.session_id),
        )
        .await
        .expect("initial voucher must decode");
        let accepted = authorization
            .lock()
            .await
            .accept_envelope(&accepted)
            .expect("initial voucher must validate");
        assert_eq!(accepted.voucher.body.sequence, 0);
        assert!(authorization.lock().await.service_started_at.is_none());
        assert!(
            handle
                .settlement_artifact()
                .expect("relay receipt signing is configured")
                .is_none()
        );
        handle
            .record_usage_voucher(accepted)
            .expect("record accepted voucher");
        assert!(
            handle
                .settlement_artifact()
                .expect("relay receipt signing succeeds")
                .is_some()
        );
    }
    #[test]
    fn vpn_helper_session_rejects_a_non_owner_relay_identity_key() {
        let helper_ticket = sample_helper_ticket([0xA9; 16]);
        let wrong_relay_key = Arc::new(
            KeyPair::try_from_seed(vec![0x68; 32], Algorithm::Ed25519)
                .expect("derive wrong relay fixture key"),
        );
        let overlay = VpnOverlay::from_config(VpnConfig::default());
        let session = overlay.start_session(Arc::new(Metrics::new()));
        let error = overlay
            .bind_helper_session(session, &helper_ticket, wrong_relay_key)
            .expect_err("a relay must not sign another relay identity's receipts");
        assert!(error.contains("does not match"));
    }
    #[tokio::test]
    async fn vpn_backend_bridge_forwards_backend_payloads_into_vpn_frames() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig::default()));
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = overlay.bind_session(session, [0xA1; 16]);
        let helper_ticket = sample_helper_ticket([0xA1; 16]);
        let voucher_authorization = active_voucher_authorization(&helper_ticket);
        let adapter = VpnAdapter::new(handle.session().clone(), Arc::clone(&overlay));
        let bridge = VpnBridge::new(
            adapter.clone(),
            [0xA1; 16],
            vpn_flow_label_from_session_id([0xA1; 16]),
        )
        .expect("cover scheduler seed");
        let record_context =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 7);
        let relay_record = RecordLayer::new(SessionKey::new(vec![0xA5; 32]), RecordEndpoint::Relay)
            .expect("relay record layer")
            .stream(record_context)
            .expect("relay record stream");
        let client_record =
            RecordLayer::new(SessionKey::new(vec![0xA5; 32]), RecordEndpoint::Client)
                .expect("client record layer")
                .stream(record_context)
                .expect("client record stream");
        let (vpn_runtime, vpn_peer) = duplex(VPN_CELL_LEN * 8);
        let (vpn_read, vpn_write) = tokio::io::split(vpn_runtime);
        let (vpn_peer_read, _vpn_peer_write) = tokio::io::split(vpn_peer);
        let mut vpn_read = RecordReader::new(vpn_read, relay_record.opener);
        let mut vpn_write = RecordWriter::new(vpn_write, relay_record.sealer);
        let mut vpn_peer = RecordReader::new(vpn_peer_read, client_record.opener);
        let (backend_runtime, mut backend_peer) = duplex(VPN_CELL_LEN * 8);
        let (mut backend_read, mut backend_write) = tokio::io::split(backend_runtime);
        let settlement_dir = secure_test_tempdir();
        let settlement_store = test_settlement_store(&settlement_dir);
        let packet = [0xDE, 0xAD, 0xBE, 0xEF];
        let mut payload = Vec::from(
            u16::try_from(packet.len())
                .expect("fixture packet length fits u16")
                .to_be_bytes(),
        );
        payload.extend_from_slice(&packet);
        let bridge_task = tokio::spawn(async move {
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                VpnBackendBridgeContext {
                    bridge,
                    adapter: &adapter,
                    vpn_session: &handle,
                    voucher_authorization,
                    settlement_store,
                    expected_circuit_id: [0xA1; 16],
                    expected_flow_label: vpn_flow_label_from_session_id([0xA1; 16]),
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
        let parsed = timeout(
            Duration::from_secs(1),
            crate::vpn::read_frame(overlay.as_ref(), &mut vpn_peer),
        )
        .await
        .expect("one protected VPN frame must not wait for a second backend packet")
        .expect("vpn frame");
        assert_eq!(payload, parsed.payload);
        backend_peer
            .shutdown()
            .await
            .expect("shutdown backend peer");
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
        let voucher_authorization = active_voucher_authorization(&helper_ticket);
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
        let settlement_dir = secure_test_tempdir();
        let settlement_store = test_settlement_store(&settlement_dir);
        let packet = [0xFA, 0xCE, 0xB0, 0x0C];
        let mut payload = Vec::from(
            u16::try_from(packet.len())
                .expect("fixture packet length fits u16")
                .to_be_bytes(),
        );
        payload.extend_from_slice(&packet);
        let bridge_task = tokio::spawn(async move {
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                VpnBackendBridgeContext {
                    bridge,
                    adapter: &adapter,
                    vpn_session: &handle,
                    voucher_authorization,
                    settlement_store,
                    expected_circuit_id: [0xB2; 16],
                    expected_flow_label: vpn_flow_label_from_session_id([0xB2; 16]),
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
    #[tokio::test]
    async fn vpn_backend_bridge_closes_when_prepaid_active_credit_expires() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = Arc::new(VpnOverlay::from_config(VpnConfig {
            usage_voucher_max_age_ms: 2_000,
            usage_voucher_setup_timeout_ms: 2_000,
            ..VpnConfig::default()
        }));
        let session = overlay.start_session(Arc::clone(&metrics));
        let helper_ticket = sample_helper_ticket([0xB3; 16]);
        let now_ms = unix_time_ms(SystemTime::now());
        let envelope = usage_voucher_envelope(
            &helper_ticket,
            &sample_metering_key_pair(),
            0,
            64 * 1024,
            64 * 1024,
            20,
            now_ms,
        );
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 1_048_576, 2_000);
        authorization
            .accept_envelope_at(&envelope, now_ms)
            .expect("initial prepaid voucher");
        authorization.begin_service();
        let voucher_authorization = Arc::new(Mutex::new(authorization));
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket.clone());
        let adapter = VpnAdapter::new(handle.session().clone(), Arc::clone(&overlay));
        let bridge = VpnBridge::new(
            adapter.clone(),
            helper_ticket.session_id,
            vpn_flow_label_from_session_id(helper_ticket.session_id),
        )
        .expect("cover scheduler seed");
        let (vpn_runtime, _vpn_peer) = duplex(VPN_CELL_LEN * 2);
        let (mut vpn_read, mut vpn_write) = tokio::io::split(vpn_runtime);
        let (backend_runtime, _backend_peer) = duplex(VPN_CELL_LEN * 2);
        let (mut backend_read, mut backend_write) = tokio::io::split(backend_runtime);
        let settlement_dir = secure_test_tempdir();
        let settlement_store = test_settlement_store(&settlement_dir);

        let error = timeout(
            Duration::from_secs(3),
            RelayRuntime::bridge_vpn_backend_streams(
                &mut vpn_write,
                &mut vpn_read,
                VpnBackendBridgeContext {
                    bridge,
                    adapter: &adapter,
                    vpn_session: &handle,
                    voucher_authorization,
                    settlement_store,
                    expected_circuit_id: helper_ticket.session_id,
                    expected_flow_label: vpn_flow_label_from_session_id(helper_ticket.session_id),
                    mtu: VpnCellV1::max_payload_len(),
                },
                &mut backend_read,
                &mut backend_write,
            ),
        )
        .await
        .expect("voucher watchdog must finish")
        .expect_err("missing voucher must close the bridge");
        assert!(error.to_string().contains("prepaid active-time ceiling"));
    }
    #[test]
    fn vpn_voucher_authorization_rejects_packet_beyond_signed_ceiling() {
        let helper_ticket = sample_helper_ticket([0xA3; 16]);
        let key_pair = sample_metering_key_pair();
        let envelope = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 4, 1, 1_000, 2_000);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 4, 5_000);
        authorization
            .accept_envelope_at(&envelope, 2_000)
            .expect("initial prepaid voucher");
        authorization.begin_service();
        authorization
            .authorize_ingress_packet(4)
            .expect("packet exactly at the ceiling");
        assert!(authorization.authorize_ingress_packet(1).is_err());
    }
    #[test]
    fn vpn_voucher_authorization_bounds_unbillable_wire_cell_floods() {
        let helper_ticket = sample_helper_ticket([0xA4; 16]);
        let key_pair = sample_metering_key_pair();
        let envelope = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 16, 16, 1_000, 2_000);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 16, 5_000);
        authorization
            .accept_envelope_at(&envelope, 2_000)
            .expect("initial prepaid voucher");
        authorization.begin_service();

        authorization
            .authorize_ingress_wire_cell()
            .expect("one fixed cell fits the 64x first-release expansion budget");
        let error = authorization
            .authorize_ingress_wire_cell()
            .expect_err("a second zero-progress cell must exceed the signed budget");
        assert!(error.to_string().contains("wire traffic"));
        assert_eq!(authorization.observed_ingress_bytes, 0);
        assert_eq!(
            authorization.observed_ingress_wire_bytes,
            VPN_CELL_LEN as u64
        );
    }
    #[test]
    fn vpn_voucher_authorization_caps_control_signature_work_per_session() {
        let helper_ticket = sample_helper_ticket([0xB4; 16]);
        let key_pair = sample_metering_key_pair();
        let envelope = usage_voucher_envelope(&helper_ticket, &key_pair, 1, 1, 1, 1_000, 2_000);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 16, 5_000);
        authorization.accepted_vouchers = VPN_MAX_ACCEPTED_VOUCHERS_V1;
        let error = authorization
            .accept_envelope_at(&envelope, 2_000)
            .expect_err("voucher verification work must have a hard lifetime ceiling");
        assert!(error.to_string().contains("usage vouchers"));
    }
    #[test]
    fn vpn_voucher_authorization_rejects_wrong_metering_public_key() {
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
            fee_ceiling: helper_ticket
                .tariff
                .fee_ceiling(&voucher.body)
                .expect("bounded fixture fee"),
            voucher,
        };
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        let error = authorization
            .accept_envelope(&envelope)
            .expect_err("wrong metering key must fail");
        assert!(error.to_string().contains("public key"));
    }
    #[test]
    fn vpn_voucher_authorization_rejects_ceiling_below_observed_active_time() {
        let helper_ticket = sample_helper_ticket([0xA6; 16]);
        let key_pair = sample_metering_key_pair();
        let body = VpnUsageVoucherBodyV1 {
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            relay_id: helper_ticket.relay_id,
            sequence: 1,
            ingress_bytes: 1,
            egress_bytes: 1,
            active_ms: 5_000,
            issued_at_ms: 2_000,
        };
        let envelope = VpnUsageVoucherEnvelopeV1 {
            fee_ceiling: helper_ticket
                .tariff
                .fee_ceiling(&body)
                .expect("bounded fixture fee"),
            voucher: signed_usage_voucher(&key_pair, body),
        };
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        authorization.service_started_at = Some(
            TokioInstant::now()
                .checked_sub(Duration::from_secs(6))
                .expect("test instant supports six-second history"),
        );
        let error = authorization
            .accept_envelope_at(&envelope, 2_000)
            .expect_err("client must not erase relay-observed active time");
        assert!(error.to_string().contains("relay-observed service time"));
    }
    #[test]
    fn vpn_voucher_authorization_accepts_initial_credit_before_service() {
        let helper_ticket = sample_helper_ticket([0xA7; 16]);
        let key_pair = sample_metering_key_pair();
        let envelope = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 64, 64, 2_000, 20_000);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        authorization
            .accept_envelope_at(&envelope, 20_000)
            .expect("initial prepaid credit is valid before backend setup");
        assert!(authorization.has_voucher);
        assert!(authorization.service_started_at.is_none());
    }
    #[test]
    fn vpn_voucher_authorization_rejects_zero_credit_initial_voucher() {
        let helper_ticket = sample_helper_ticket([0xAD; 16]);
        let key_pair = sample_metering_key_pair();
        let envelope = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 0, 0, 0, 20_000);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        let error = authorization
            .accept_envelope_at(&envelope, 20_000)
            .expect_err("initial voucher must carry usable prepaid credit");
        assert!(error.to_string().contains("must preauthorize non-zero"));
        assert!(!authorization.has_voucher);
    }
    #[test]
    fn vpn_voucher_authorization_enforces_escrowed_fee_boundary_without_mutation() {
        let mut helper_ticket = sample_helper_ticket([0xAF; 16]);
        helper_ticket.tariff = VpnTariffV1 {
            lease_fee: quantity_nanos(10),
            active_fee_per_minute: quantity_nanos(10),
            ingress_fee_per_mib: Quantity::zero(),
            egress_fee_per_mib: Quantity::zero(),
        };
        let key_pair = sample_metering_key_pair();
        let at_escrow = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 1, 1, 60_000, 2_000);
        assert_eq!(at_escrow.fee_ceiling, helper_ticket.tariff.lease_fee);
        let mut exact_authorization = VpnVoucherAuthorization::new(&helper_ticket, 1, 60_000);
        exact_authorization
            .accept_envelope_at(&at_escrow, 2_000)
            .expect("a voucher exactly at the escrowed lease fee must be accepted");

        let mut above_escrow = at_escrow;
        above_escrow.fee_ceiling = quantity_nanos(11);
        let mut rejected_authorization = VpnVoucherAuthorization::new(&helper_ticket, 1, 60_000);
        let error = rejected_authorization
            .accept_envelope_at(&above_escrow, 2_000)
            .expect_err("a voucher above escrow must fail before admission state advances");
        assert!(error.to_string().contains("exceeds the escrowed"));
        assert!(!rejected_authorization.has_voucher);
        assert_eq!(rejected_authorization.highest_sequence, 0);
        assert_eq!(
            rejected_authorization.authorized_fee_ceiling,
            Quantity::zero()
        );
        assert_eq!(rejected_authorization.last_issued_at_ms, None);
    }
    #[test]
    fn vpn_voucher_authorization_rejects_excessive_credit_without_poisoning_state() {
        let helper_ticket = sample_helper_ticket([0xA8; 16]);
        let key_pair = sample_metering_key_pair();
        let forged = usage_voucher_envelope(&helper_ticket, &key_pair, 7, 1, 1, 5_001, 2_000);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        let error = authorization
            .accept_envelope_at(&forged, 2_000)
            .expect_err("excessive active-time credit must fail");
        assert!(error.to_string().contains("configured limit"));
        assert!(!authorization.has_voucher);
        assert_eq!(authorization.highest_sequence, 0);

        let valid = usage_voucher_envelope(&helper_ticket, &key_pair, 1, 1, 1, 1_000, 2_000);
        authorization
            .accept_envelope_at(&valid, 2_000)
            .expect("rejected higher sequence must not poison the window");
        assert_eq!(authorization.highest_sequence, 1);
    }
    #[test]
    fn vpn_voucher_authorization_caps_unused_setup_time_to_one_active_window() {
        let helper_ticket = sample_helper_ticket([0xAE; 16]);
        let key_pair = sample_metering_key_pair();
        let initial = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 1, 1, 2_000, 2_000);
        let after_slow_setup =
            usage_voucher_envelope(&helper_ticket, &key_pair, 1, 1, 1, 10_000, 2_001);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        authorization
            .accept_envelope_at(&initial, 2_000)
            .expect("initial bounded credit");
        authorization.begin_service();
        authorization
            .accept_envelope_at(&after_slow_setup, 2_001)
            .expect("unused helper setup time must not poison the live session");

        assert_eq!(authorization.signed_active_ms, 10_000);
        assert!(
            authorization
                .authorized_active_ms
                .saturating_sub(authorization.observed_active_ms())
                <= 5_000
        );
    }
    #[test]
    fn vpn_voucher_authorization_rejects_unsettleable_issue_times_without_mutation() {
        let mut helper_ticket = sample_helper_ticket([0xA9; 16]);
        helper_ticket.valid_after_ms = 1_000;
        helper_ticket.expires_at_ms = 10_000;
        let key_pair = sample_metering_key_pair();
        for (issued_at_ms, now_ms, expected) in [
            (999, 2_000, "outside"),
            (10_000, 10_000, "outside"),
            (3_000, 2_000, "ahead"),
            (1_000, 7_001, "older"),
        ] {
            let envelope =
                usage_voucher_envelope(&helper_ticket, &key_pair, 9, 0, 0, 0, issued_at_ms);
            let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
            let error = authorization
                .accept_envelope_at(&envelope, now_ms)
                .expect_err("unsettleable issuance time must fail");
            assert!(error.to_string().contains(expected));
            assert!(!authorization.has_voucher);
            assert_eq!(authorization.last_issued_at_ms, None);
        }
    }
    #[test]
    fn vpn_voucher_authorization_rejects_backwards_issue_time_without_replacement() {
        let helper_ticket = sample_helper_ticket([0xAA; 16]);
        let key_pair = sample_metering_key_pair();
        let first = usage_voucher_envelope(&helper_ticket, &key_pair, 1, 1, 1, 1_000, 2_000);
        let backwards = usage_voucher_envelope(&helper_ticket, &key_pair, 2, 1, 1, 1_000, 1_999);
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        authorization
            .accept_envelope_at(&first, 2_000)
            .expect("initial voucher");
        let error = authorization
            .accept_envelope_at(&backwards, 2_000)
            .expect_err("issuance time must be monotonic");
        assert!(error.to_string().contains("backwards"));
        assert_eq!(authorization.highest_sequence, 1);
        assert_eq!(authorization.last_issued_at_ms, Some(2_000));
    }
    #[test]
    fn vpn_packet_stream_decoder_waits_for_atomic_packets_across_fragments() {
        let mut decoder = VpnPacketStreamDecoder::default();
        assert!(
            decoder
                .ingest(&[0, 4, 0xAA], 1_280)
                .expect("prefix")
                .is_empty()
        );
        assert_eq!(
            decoder
                .ingest(&[0xBB, 0xCC, 0xDD, 0, 2, 0xEE, 0xFF], 1_280)
                .expect("fragmented packets"),
            vec![vec![0xAA, 0xBB, 0xCC, 0xDD], vec![0xEE, 0xFF]]
        );
        assert!(decoder.ingest(&[0, 0], 1_280).is_err());
    }
    #[test]
    fn vpn_client_data_cells_reject_empty_and_tiny_partial_progress() {
        let mut empty = VpnPacketStreamDecoder::default();
        assert!(
            empty
                .ingest_client_data_cell(&[], VpnCellV1::max_payload_len())
                .is_err()
        );

        let mut tiny_partial = VpnPacketStreamDecoder::default();
        let error = tiny_partial
            .ingest_client_data_cell(&[0, 4, 0xAA], VpnCellV1::max_payload_len())
            .expect_err("a short cell that completes no packet is a flood primitive");
        assert!(error.to_string().contains("fill the complete cell"));

        let mut bounded_fragment = VpnPacketStreamDecoder::default();
        let mut full = vec![0xAA; VpnCellV1::max_payload_len()];
        let packet_len = u16::try_from(VpnCellV1::max_payload_len()).expect("VPN payload fits u16");
        full[..2].copy_from_slice(&packet_len.to_be_bytes());
        assert!(
            bounded_fragment
                .ingest_client_data_cell(&full, VpnCellV1::max_payload_len())
                .expect("one full-cell partial packet is canonical")
                .is_empty()
        );
        assert_eq!(
            bounded_fragment
                .ingest_client_data_cell(&[0xAA; 2], VpnCellV1::max_payload_len())
                .expect("the short final fragment completes the packet")
                .len(),
            1
        );
    }
    #[test]
    fn vpn_client_cannot_supply_unmetered_cover_or_keepalive_cells() {
        assert!(validate_client_originated_vpn_class(VpnCellClassV1::Data).is_ok());
        assert!(validate_client_originated_vpn_class(VpnCellClassV1::Control).is_ok());
        for class in [VpnCellClassV1::Cover, VpnCellClassV1::KeepAlive] {
            let error = validate_client_originated_vpn_class(class)
                .expect_err("relay-generated traffic classes must fail on client ingress");
            assert!(error.to_string().contains("client-originated"));
        }
    }
    #[test]
    fn vpn_client_control_cells_cannot_be_ignored_zero_byte_traffic() {
        for payload in [&[][..], b"keepalive".as_slice()] {
            let error = decode_required_usage_voucher_control(payload)
                .expect_err("every client control cell must be a signed voucher");
            assert!(
                error
                    .to_string()
                    .contains("exactly one signed usage voucher")
            );
        }
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
            fee_ceiling: quantity_nanos(55),
        };
        let mut payload = Vec::from(VPN_USAGE_VOUCHER_CONTROL_MAGIC.as_slice());
        payload.extend_from_slice(&envelope.encode());
        let decoded = decode_usage_voucher_control(&payload)
            .expect("decode")
            .expect("voucher payload");
        let mut authorization = VpnVoucherAuthorization::new(&helper_ticket, 64, 5_000);
        authorization
            .accept_envelope_at(&decoded, 2_000)
            .expect("voucher accepted");
        let mut lower_fee_body = body;
        lower_fee_body.sequence = 8;
        let lower_fee_envelope = VpnUsageVoucherEnvelopeV1 {
            voucher: signed_usage_voucher(&key_pair, lower_fee_body),
            fee_ceiling: quantity_nanos(54),
        };
        let lower_fee_error = authorization
            .accept_envelope_at(&lower_fee_envelope, 2_000)
            .expect_err("wrong fee ceiling must fail");
        assert!(lower_fee_error.to_string().contains("fee ceiling"));
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(Arc::clone(&metrics));
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket.clone());
        let initial_reservation = handle
            .pre_service_settlement_artifact(&decoded)
            .expect("initial settlement reservation");
        let spool_dir = secure_test_tempdir();
        let settlement_store = test_settlement_store(&spool_dir);
        settlement_store
            .write_initial_reservation(&handle, &initial_reservation)
            .expect("write initial settlement WAL");
        let stable_entries_with_wal =
            vpn_settlement_spool_entry_count(settlement_store.spool_dir.as_path())
                .expect("count stable WAL entries");
        handle
            .record_usage_voucher(decoded)
            .expect("record accepted voucher");
        handle
            .begin_metered_service(body.issued_at_ms)
            .expect("begin metered service");
        handle
            .record_metered_ingress(10)
            .expect("record ingress usage");
        handle
            .record_metered_egress(20)
            .expect("record egress usage");
        handle
            .end_metered_service(body.issued_at_ms)
            .expect("end metered service");
        let receipt = handle.receipt().expect("finalize receipt");
        let active_ms = receipt.ended_at_ms.saturating_sub(receipt.started_at_ms);
        let expected_earned_fee = helper_ticket
            .tariff
            .fee_for_usage(10, 20, active_ms)
            .expect("actual fixture fee");
        assert_eq!(receipt.highest_voucher_sequence, 7);
        assert_eq!(receipt.earned_fee, expected_earned_fee);
        assert_eq!(receipt.client_voucher_hash, envelope.voucher.hash());
        let artifact = handle
            .settlement_artifact()
            .expect("relay receipt signing succeeds")
            .expect("accepted voucher should produce settlement artifact");
        artifact
            .receipt
            .verify()
            .expect("settlement artifact relay signature");
        assert_eq!(
            artifact.receipt.receipt.client_voucher_hash,
            envelope.voucher.hash()
        );
        let path = settlement_store
            .finalize(&handle, &artifact)
            .expect("finalize settlement artifact");
        assert_eq!(
            vpn_settlement_spool_entry_count(settlement_store.spool_dir.as_path())
                .expect("count stable final entries"),
            stable_entries_with_wal,
            "WAL-to-final promotion replaces one logical artifact instead of consuming another slot"
        );
        let encoded = std::fs::read(&path).expect("read settlement artifact");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
            let metadata = std::fs::symlink_metadata(&path).expect("settlement metadata");
            assert!(metadata.is_file());
            assert_eq!(metadata.nlink(), 1);
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
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
        assert_eq!(record.earned_fee, expected_earned_fee);
        let encoded_json = String::from_utf8(encoded).expect("settlement artifact is UTF-8 JSON");
        assert!(encoded_json.contains(&format!("\"earned_fee\": \"{}\"", record.earned_fee)));
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
            hex::encode(helper_ticket.lease_id)
        );
        assert_ne!(helper_ticket.lease_id, helper_ticket.quote_id);
    }
    #[test]
    fn vpn_settlement_crash_recovery_never_promotes_prepaid_ceilings() {
        let spool_dir = secure_test_tempdir();
        let now_ms = unix_time_ms(SystemTime::now());
        let mut helper_ticket = sample_helper_ticket([0xC1; 16]);
        helper_ticket.valid_after_ms = now_ms.saturating_sub(1_000);
        helper_ticket.expires_at_ms = now_ms.saturating_add(30_000);
        let replay_config = VpnConfig {
            lease_secs: 60,
            helper_ticket_replay_store_capacity: 16,
            helper_ticket_replay_store_path: spool_dir.path().join("helper-replays.norito"),
            ..VpnConfig::default()
        };
        let replay_ledger =
            load_vpn_helper_ticket_replay_ledger(&replay_config, &helper_ticket.relay_id, now_ms)
                .expect("create replay ledger");
        let store = VpnSettlementStore::open(spool_dir.path(), &replay_ledger)
            .expect("open settlement store");
        let envelope = usage_voucher_envelope(
            &helper_ticket,
            &sample_metering_key_pair(),
            0,
            256 * 1_024,
            256 * 1_024,
            2_000,
            now_ms,
        );
        let overlay = VpnOverlay::from_config(VpnConfig::default());
        let session = overlay.start_session(Arc::new(Metrics::new()));
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket.clone());
        let initial = handle
            .pre_service_settlement_artifact(&envelope)
            .expect("initial zero reservation");
        let wal_path = store
            .write_initial_reservation(&handle, &initial)
            .expect("persist initial WAL before ticket redemption");
        consume_vpn_helper_ticket(
            &replay_ledger,
            vpn_helper_ticket_replay_id(&helper_ticket),
            helper_ticket.expires_at_ms,
            now_ms,
        )
        .expect("durably redeem helper ticket");
        handle
            .record_usage_voucher(envelope.clone())
            .expect("record accepted voucher");
        handle
            .begin_metered_service(now_ms)
            .expect("begin metered service");
        handle
            .record_metered_ingress(1_024)
            .expect("record ingress usage");
        handle
            .record_metered_egress(2_048)
            .expect("record egress usage");
        assert!(wal_path.is_file());
        let wal_bytes = std::fs::read(&wal_path).expect("read live WAL");
        assert!(
            norito::json::from_slice::<VpnSettlementSpoolRecord>(&wal_bytes).is_err(),
            "a live WAL must not be accepted as a submit-ready settlement artifact"
        );
        assert!(
            std::fs::read_dir(spool_dir.path())
                .expect("enumerate live spool")
                .filter_map(Result::ok)
                .all(|entry| entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "json")),
            "no submit-ready artifact may exist while the live owner holds the WAL"
        );

        // Dropping both held locks models process death immediately after
        // service admission/forwarding. Startup recovery must promote only the
        // durable zero-usage receipt, never the prepaid ceilings or volatile
        // observed counters.
        drop(store);
        drop(replay_ledger);
        let replay_ledger = load_vpn_helper_ticket_replay_ledger(
            &replay_config,
            &helper_ticket.relay_id,
            now_ms.saturating_add(1),
        )
        .expect("reload replay ledger");
        let recovered_store =
            VpnSettlementStore::open(spool_dir.path(), &replay_ledger).expect("recover crash WAL");
        assert!(!wal_path.exists());
        let final_paths = std::fs::read_dir(spool_dir.path())
            .expect("enumerate recovered spool")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        assert_eq!(final_paths.len(), 1);
        assert!(
            final_paths[0]
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(&hex::encode(helper_ticket.lease_id)))
        );
        let final_bytes = std::fs::read(&final_paths[0]).expect("read recovered artifact");
        let record: VpnSettlementSpoolRecord =
            norito::json::from_slice(&final_bytes).expect("recovered artifact JSON");
        let (signed_receipt, voucher, lease_id) =
            decode_vpn_spool_payload(&record).expect("decode recovered settlement");
        signed_receipt
            .verify()
            .expect("recovered settlement keeps relay authentication");
        let receipt = &signed_receipt.receipt;
        let active_ms = receipt.ended_at_ms - receipt.started_at_ms;
        assert_eq!(lease_id, helper_ticket.lease_id);
        assert_eq!(receipt.ingress_bytes, 0);
        assert_eq!(receipt.egress_bytes, 0);
        assert_eq!(active_ms, 0);
        assert!(receipt.started_at_ms >= helper_ticket.valid_after_ms);
        assert!(receipt.ended_at_ms <= helper_ticket.expires_at_ms);
        assert!(
            voucher
                .body
                .authorizes(receipt.ingress_bytes, receipt.egress_bytes, active_ms)
        );
        assert_eq!(
            receipt.earned_fee,
            helper_ticket
                .tariff
                .fee_for_usage(receipt.ingress_bytes, receipt.egress_bytes, active_ms)
                .expect("recompute recovered fee")
        );
        drop(recovered_store);
    }
    #[test]
    fn vpn_settlement_spool_entry_limit_accepts_exact_and_rejects_plus_one() {
        let spool_dir = secure_test_tempdir();
        for index in 0..3 {
            std::fs::write(
                spool_dir.path().join(format!("artifact-{index}.json")),
                b"{}",
            )
            .expect("write bounded spool fixture");
        }
        assert_eq!(
            vpn_settlement_spool_entry_count_with_limit(spool_dir.path(), 3)
                .expect("exact entry ceiling"),
            3
        );
        std::fs::write(spool_dir.path().join("artifact-overflow.json"), b"{}")
            .expect("write overflowing spool fixture");
        let error = vpn_settlement_spool_entry_count_with_limit(spool_dir.path(), 3)
            .expect_err("one entry above the ceiling must fail closed");
        assert!(error.contains("more than the configured limit of 3"));
    }
    #[test]
    fn vpn_settlement_entry_reservation_releases_on_failure_and_commits_once() {
        let mut state = VpnSettlementOperationState {
            stable_entries: VPN_SETTLEMENT_SPOOL_MAX_ENTRIES_V1 - 1,
            reserved_new_entries: 0,
        };
        drop(
            state
                .reserve_new_entry()
                .expect("last slot can be reserved"),
        );
        assert_eq!(state.reserved_new_entries, 0);
        assert_eq!(
            state.stable_entries,
            VPN_SETTLEMENT_SPOOL_MAX_ENTRIES_V1 - 1
        );

        state
            .reserve_new_entry()
            .expect("last slot can be reserved again")
            .commit();
        assert_eq!(state.stable_entries, VPN_SETTLEMENT_SPOOL_MAX_ENTRIES_V1);
        assert!(
            state.reserve_new_entry().is_err(),
            "a concurrent/new session must fail after the final slot commits"
        );
    }
    #[test]
    fn vpn_settlement_persistence_failure_poison_closes_future_service() {
        let spool_dir = secure_test_tempdir();
        let store = test_settlement_store(&spool_dir);
        let lock_error =
            open_private_vpn_spool_lock(&store.spool_dir.join(VPN_SETTLEMENT_OWNER_LOCK_V1))
                .expect_err("a live settlement spool owner must exclude a second relay");
        assert!(lock_error.to_string().contains("exclusive"));
        let now_ms = unix_time_ms(SystemTime::now());
        let mut helper_ticket = sample_helper_ticket([0xC2; 16]);
        helper_ticket.valid_after_ms = now_ms.saturating_sub(1_000);
        helper_ticket.expires_at_ms = now_ms.saturating_add(30_000);
        let envelope = usage_voucher_envelope(
            &helper_ticket,
            &sample_metering_key_pair(),
            0,
            256 * 1_024,
            256 * 1_024,
            2_000,
            now_ms,
        );
        let overlay = VpnOverlay::from_config(VpnConfig::default());
        let session = overlay.start_session(Arc::new(Metrics::new()));
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket);
        let initial = handle
            .pre_service_settlement_artifact(&envelope)
            .expect("initial reservation");
        let wal_path = store
            .write_initial_reservation(&handle, &initial)
            .expect("write initial WAL");
        handle
            .record_usage_voucher(envelope.clone())
            .expect("record accepted voucher");
        handle
            .begin_metered_service(now_ms)
            .expect("begin metered service");
        handle
            .record_metered_egress(1)
            .expect("record egress usage");
        handle
            .end_metered_service(now_ms.saturating_add(1))
            .expect("end metered service");
        let final_artifact = handle
            .settlement_artifact()
            .expect("sign final settlement")
            .expect("accepted voucher yields a final settlement");
        let impossible_phase =
            vpn_settlement_wal_record(&handle, &final_artifact, VpnSettlementWalPhase::PreService)
                .expect("construct malformed phase fixture");
        assert!(
            validate_vpn_settlement_wal(&impossible_phase)
                .expect_err("pre-service WAL cannot carry service usage")
                .contains("exactly zero")
        );
        let mut tampered_wal =
            vpn_settlement_wal_record(&handle, &final_artifact, VpnSettlementWalPhase::Finalizing)
                .expect("construct signed finalizing WAL fixture");
        let (mut tampered_receipt, _, _) =
            decode_vpn_spool_payload(&tampered_wal.reserved_settlement)
                .expect("decode signed service WAL receipt");
        tampered_receipt.receipt.egress_bytes =
            tampered_receipt.receipt.egress_bytes.saturating_add(1);
        tampered_wal
            .reserved_settlement
            .submit_receipt_request
            .relay_receipt_hex = hex::encode(tampered_receipt.encode());
        let error = validate_vpn_settlement_wal(&tampered_wal)
            .expect_err("WAL recovery must reject a receipt body changed after relay signing");
        assert!(error.contains("relay receipt signature"), "{error}");

        std::fs::remove_file(&wal_path).expect("remove WAL to simulate storage corruption");
        std::fs::create_dir(&wal_path).expect("replace WAL with a non-file");
        let error = store
            .finalize(&handle, &final_artifact)
            .expect_err("invalid persistence target must fail closed");
        assert!(error.contains("not an owner-owned"));
        assert!(store.ensure_healthy().is_err());
    }
    #[test]
    fn vpn_expiry_finalization_clamps_receipt_to_signed_lease_end() {
        let mut helper_ticket = sample_helper_ticket([0xAB; 16]);
        let now_ms = unix_time_ms(SystemTime::now());
        helper_ticket.valid_after_ms = now_ms.saturating_sub(1_000);
        helper_ticket.expires_at_ms = now_ms.saturating_add(10);
        let key_pair = sample_metering_key_pair();
        let envelope = usage_voucher_envelope(&helper_ticket, &key_pair, 0, 1, 1, 1, now_ms);
        let voucher_issued_at_ms = envelope.voucher.body.issued_at_ms;
        let metrics = Arc::new(Metrics::new());
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(metrics);
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket.clone());
        handle
            .record_usage_voucher(envelope)
            .expect("record accepted voucher");
        std::thread::sleep(Duration::from_millis(20));
        let artifact = handle
            .settlement_artifact()
            .expect("relay receipt signing succeeds")
            .expect("accepted voucher should settle at expiry");
        assert_eq!(artifact.lease_id, helper_ticket.lease_id);
        assert_eq!(artifact.receipt.receipt.ended_at_ms, voucher_issued_at_ms);
        assert!(artifact.receipt.receipt.ended_at_ms < helper_ticket.expires_at_ms);
        assert!(artifact.receipt.receipt.started_at_ms <= artifact.receipt.receipt.ended_at_ms);
    }
    #[test]
    fn accepted_initial_voucher_without_backend_service_yields_zero_usage_artifact() {
        let helper_ticket = sample_helper_ticket([0xAC; 16]);
        let key_pair = sample_metering_key_pair();
        let issued_at_ms = unix_time_ms(SystemTime::now());
        let envelope = usage_voucher_envelope(
            &helper_ticket,
            &key_pair,
            0,
            64 * 1024,
            64 * 1024,
            2_000,
            issued_at_ms,
        );
        let metrics = Arc::new(Metrics::new());
        let overlay = VpnOverlay::from_config(Default::default());
        let session = overlay.start_session(metrics);
        let handle = bind_sample_helper_session(&overlay, session, helper_ticket);
        handle
            .record_usage_voucher(envelope)
            .expect("record accepted voucher");

        let artifact = handle
            .settlement_artifact()
            .expect("relay receipt signing succeeds")
            .expect("accepted initial voucher must remain settleable");
        assert_eq!(artifact.receipt.receipt.started_at_ms, issued_at_ms);
        assert_eq!(artifact.receipt.receipt.ended_at_ms, issued_at_ms);
        assert_eq!(artifact.receipt.receipt.uptime_secs, 0);
        assert_eq!(artifact.receipt.receipt.ingress_bytes, 0);
        assert_eq!(artifact.receipt.receipt.egress_bytes, 0);
        assert!(artifact.earned_fee.is_zero());
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
    fn replay_test_ticket(marker: u8) -> PowTicket {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock after Unix epoch")
            .as_secs()
            .saturating_add(60);
        PowTicket {
            version: PowTicket::VERSION,
            difficulty: 1,
            expires_at,
            client_nonce: [marker; 32],
            solution: [marker ^ 0xFF; 32],
        }
    }
    fn current_client_hello_frame(
        suite: HandshakeSuite,
        resume_hash: Option<&[u8]>,
    ) -> (Vec<u8>, Vec<u8>) {
        let capabilities = update_suite_list(&DEFAULT_CLIENT_CAPABILITIES, &[suite], true)
            .expect("encode current suite list");
        let mut params = NoiseRuntimeParams::soranet_defaults();
        params.client_capabilities = &capabilities;
        params.relay_capabilities = &capabilities;
        params.resume_hash = resume_hash;
        let mut rng = StdRng::seed_from_u64(0x534f_5241_4e45_5401);
        let (frame, _state) = build_client_hello(&params, &mut rng)
            .expect("crypto engine must build current ClientHello");
        (frame, capabilities)
    }
    fn matching_server_capabilities(client_capabilities: &[u8]) -> ServerCapabilities {
        let advertised = parse_client_advertisement(client_capabilities)
            .expect("crypto capability fixture must parse in the relay");
        ServerCapabilities::new(
            advertised.kem,
            advertised.signatures,
            advertised.padding.expect("fixture advertises padding"),
            advertised.transcript_commit,
            0x01,
            advertised.constant_rate,
        )
    }
    #[test]
    fn relay_preflight_accepts_current_nk2_and_nk3_client_hello_frames() {
        for suite in [
            HandshakeSuite::Nk2Hybrid,
            HandshakeSuite::Nk3PqForwardSecure,
        ] {
            let (frame, capabilities) = current_client_hello_frame(suite, None);
            let current_wire_type = match suite {
                HandshakeSuite::Nk2Hybrid => 0x11,
                HandshakeSuite::Nk3PqForwardSecure => 0x21,
            };
            assert_eq!(frame.first().copied(), Some(current_wire_type));
            let server_capabilities = matching_server_capabilities(&capabilities);
            let preflight = preflight_client_hello(&frame, &server_capabilities)
                .unwrap_or_else(|error| panic!("relay rejected current {suite} frame: {error}"));
            assert_eq!(preflight.metadata.handshake_suite(), suite);
            assert_eq!(
                preflight.metadata.client_capabilities(),
                capabilities.as_slice()
            );
            assert_eq!(
                preflight.negotiated.kem.id.code(),
                preflight.metadata.kem_id()
            );
        }
    }
    #[test]
    fn relay_preflight_rejects_strict_constant_rate_without_downgrading() {
        let (frame, capabilities) = current_client_hello_frame(HandshakeSuite::Nk2Hybrid, None);
        let mut server_capabilities = matching_server_capabilities(&capabilities);
        server_capabilities.constant_rate = Some(capability::ConstantRateCapability {
            version: 1,
            mode: ConstantRateMode::Strict,
            cell_bytes: constant_rate::CONSTANT_RATE_CELL_BYTES as u16,
        });

        let error = match preflight_client_hello(&frame, &server_capabilities) {
            Ok(_) => panic!("strict mode must fail before the relay handshake response"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            HandshakeError::StrictConstantRateUnavailable
        ));

        server_capabilities.constant_rate = Some(capability::ConstantRateCapability {
            version: 1,
            mode: ConstantRateMode::BestEffort,
            cell_bytes: constant_rate::CONSTANT_RATE_CELL_BYTES as u16,
        });
        let negotiated = preflight_client_hello(&frame, &server_capabilities)
            .expect("best-effort cover mode remains admissible")
            .negotiated
            .constant_rate
            .expect("server best-effort mode must be negotiated");
        assert_eq!(negotiated.mode, ConstantRateMode::BestEffort);
    }
    #[test]
    fn admission_transcript_commits_to_the_exact_client_hello() {
        let (without_resume, _) = current_client_hello_frame(HandshakeSuite::Nk2Hybrid, None);
        let (with_resume, _) =
            current_client_hello_frame(HandshakeSuite::Nk2Hybrid, Some(&[0x44; 32]));
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
    #[tokio::test]
    async fn blocking_admission_gate_fails_closed_without_running_work() {
        let gate = Arc::new(Semaphore::new(1));
        let _held = Arc::clone(&gate)
            .try_acquire_owned()
            .expect("test gate permit");
        let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_ran = Arc::clone(&ran);
        let error = run_blocking_admission_work_with_gate(Arc::clone(&gate), move || {
            worker_ran.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .expect_err("a saturated verifier corridor must reject immediately");
        assert!(matches!(error, HandshakeError::AdmissionWorkUnavailable));
        assert!(!ran.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn cancelled_admission_future_does_not_release_a_running_worker_permit() {
        let gate = Arc::new(Semaphore::new(1));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker_gate = Arc::clone(&gate);
        let task = tokio::spawn(async move {
            run_blocking_admission_work_with_gate(worker_gate, move || {
                let _ = started_tx.send(());
                release_rx.recv().expect("test releases blocking worker");
                Ok(())
            })
            .await
        });
        started_rx.await.expect("blocking worker started");
        task.abort();
        let _ = task.await;
        assert!(
            Arc::clone(&gate).try_acquire_owned().is_err(),
            "cancelling the request must not free physical crypto capacity"
        );
        release_tx.send(()).expect("release blocking worker");
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(permit) = Arc::clone(&gate).try_acquire_owned() {
                    drop(permit);
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker releases its permit after physical completion");
    }
    #[test]
    fn verify_puzzle_ticket_requires_binding_and_consumes_once() {
        let params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(180),
            Duration::from_secs(45),
        )
        .expect("test puzzle parameters must be valid");
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
    fn verify_signed_puzzle_ticket_authenticates_argon2_and_consumes_shared_identity() {
        let params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(180),
            Duration::from_secs(45),
        )
        .expect("test puzzle parameters must be valid");
        let descriptor = [0xB4; 32];
        let relay_id = [0xA3; 32];
        let transcript = [0x8A; 32];
        let binding = PuzzleBinding::new(&descriptor, &relay_id, &transcript);
        let mut rng = StdRng::from_seed([0x6A; 32]);
        let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
            .expect("mint Argon2 ticket");
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("generate signing key");
        let signed = SignedTicket::sign(ticket, &relay_id, &transcript, keypair.secret_key())
            .expect("sign Argon2 ticket");
        let replays = in_memory_ticket_replays(4);

        verify_signed_puzzle_ticket_binding(
            &signed,
            keypair.public_key(),
            &params,
            &descriptor,
            &relay_id,
            &transcript,
            &replays,
        )
        .expect("signed Argon2 ticket verifies");
        assert!(matches!(
            verify_signed_puzzle_ticket_binding(
                &signed,
                keypair.public_key(),
                &params,
                &descriptor,
                &relay_id,
                &transcript,
                &replays,
            ),
            Err(HandshakeError::Pow(pow::Error::Replay))
        ));

        let mut persisted = replays.lock().expect("replay state");
        assert!(
            persisted
                .persisted
                .is_ticket_payload_revoked(&signed.ticket, SystemTime::now())
                .expect("query replay identity"),
            "signed and raw presentations must share the underlying ticket identity"
        );
    }
    #[test]
    fn verify_puzzle_ticket_rejects_wrong_relay_binding() {
        let params = PuzzleParameters::try_new(
            NonZeroU32::new(4_096).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            5,
            Duration::from_secs(120),
            Duration::from_secs(30),
        )
        .expect("test puzzle parameters must be valid");
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
    fn relay_ticket_replay_is_rejected_after_store_reload() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("relay-ticket-replays.norito");
        let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
        let params = PuzzleParameters::try_new(
            NonZeroU32::new(4_096).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            Duration::from_secs(180),
            Duration::from_secs(30),
        )
        .expect("test puzzle parameters must be valid");
        let descriptor = [0x35; 32];
        let relay_id = [0x46; 32];
        let transcript = [0x57; 32];
        let binding = PuzzleBinding::new(&descriptor, &relay_id, &transcript);
        let mut rng = StdRng::from_seed([0x68; 32]);
        let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
            .expect("mint ticket");
        let persisted =
            TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("load store");
        let replays = StdMutex::new(TicketReplayState {
            persisted,
            pending: HashSet::new(),
            capacity: limits.max_entries,
        });
        verify_puzzle_ticket_binding(
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
            verify_puzzle_ticket_binding(
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
        let first = replay_test_ticket(0x44);
        let second = replay_test_ticket(0x45);
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
        let ticket = replay_test_ticket(0x74);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first_replays = Arc::clone(&replays);
        let first_ticket_bytes = ticket.to_bytes();
        let first_ticket = pow::Ticket::parse(first_ticket_bytes.as_ref())
            .expect("parse a second owned ticket for the concurrency test");
        let first = std::thread::spawn(move || {
            verify_and_consume_ticket(&first_ticket, first_replays.as_ref(), || {
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
    fn puzzle_failure_reason_preserves_policy_diagnostics() {
        assert_eq!(
            puzzle_failure_reason(&puzzle::Error::DifficultyMismatch {
                ticket: 4,
                required: 6,
            }),
            SoranetPowFailureReasonV1::DifficultyMismatch
        );
        assert_eq!(
            puzzle_failure_reason(&puzzle::Error::Expired(10, 11)),
            SoranetPowFailureReasonV1::Expired
        );
        assert_eq!(
            puzzle_failure_reason(&puzzle::Error::ExpiryWindowTooSmall(Duration::from_secs(
                30
            ))),
            SoranetPowFailureReasonV1::TtlTooShort
        );
    }
    #[cfg(any())]
    #[test]
    fn norito_stream_open_roundtrip() {
        let open = NoritoStreamOpen {
            channel_id: [0xA1; 32],
            route_id: [0xB2; 32],
            stream_id: [0xC3; 32],
            padding_budget_ms: Some(37),
            access_kind: SoranetAccessKind::ReadOnly,
            exit_token: vec![0x45, 0x67, 0x89],
        };
        let bytes = to_bytes(&open).expect("encode handshake");
        let decoded: NoritoStreamOpen = decode_from_bytes(&bytes).expect("decode handshake");
        assert_eq!(decoded.channel_id, open.channel_id);
        assert_eq!(decoded.route_id, open.route_id);
        assert_eq!(decoded.stream_id, open.stream_id);
        assert_eq!(decoded.exit_token, open.exit_token);
        assert_eq!(decoded.padding_budget_ms, open.padding_budget_ms);
        assert_eq!(decoded.access_kind, open.access_kind);
        let debug = format!("{open:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("69, 103, 137"));
    }
    #[cfg(any())]
    #[test]
    fn kaigi_stream_open_roundtrip() {
        let open = KaigiStreamOpen {
            channel_id: [0xAA; 32],
            route_id: [0xBB; 32],
            stream_id: [0xCC; 32],
            room_id: [0xDD; 32],
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
        let debug = format!("{open:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("16, 32, 48"));
    }
    #[cfg(any())]
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
    #[cfg(any())]
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
    struct LoadedTestConfig {
        config: RelayConfig,
        _replay_directory: TempDir,
    }
    fn load_config(json: &str) -> LoadedTestConfig {
        let file = secure_test_tempfile();
        std::fs::write(file.path(), json).expect("write config");
        let mut config = RelayConfig::load(file.path()).expect("load config");
        let replay_directory = secure_test_tempdir();
        let default_replay_path = config::PowConfig::default().revocation_store_path;
        if config.pow_config().revocation_store_path == default_replay_path {
            config
                .pow
                .as_mut()
                .expect("PoW defaults applied")
                .revocation_store_path = replay_directory.path().join("ticket-replays.norito");
        }
        LoadedTestConfig {
            config,
            _replay_directory: replay_directory,
        }
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
        identity_seed: [u8; 32],
        descriptor_commit: [u8; 32],
        bundle: RelayCertificateBundleV2,
        bundle_file: NamedTempFile,
        manifest_file: NamedTempFile,
        issuer_ed25519_hex: String,
        issuer_mldsa_hex: String,
    }
    impl CertificateTestFixture {
        fn new() -> Self {
            Self::with_valid_until(i64::MAX)
        }
        fn with_valid_until(valid_until: i64) -> Self {
            Self::with_valid_until_and_spki(valid_until, [0xA5; 32])
        }
        fn with_spki(tls_spki_sha256: [u8; 32]) -> Self {
            Self::with_valid_until_and_spki(i64::MAX, tls_spki_sha256)
        }
        fn with_identity_seed(identity_seed: [u8; 32]) -> Self {
            Self::with_parameters(i64::MAX, [0xA5; 32], identity_seed)
        }
        fn with_valid_until_and_spki(valid_until: i64, tls_spki_sha256: [u8; 32]) -> Self {
            Self::with_parameters(valid_until, tls_spki_sha256, [0x11; 32])
        }
        fn with_parameters(
            valid_until: i64,
            tls_spki_sha256: [u8; 32],
            identity_seed: [u8; 32],
        ) -> Self {
            let descriptor_commit = [0xAB; 32];
            let identity_seed_hex = hex::encode(identity_seed);
            let identity_signing = SigningKey::from_bytes(&identity_seed);
            let identity_public = identity_signing.verifying_key();
            let identity_public_bytes = identity_public.to_bytes();
            let relay_mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
                .expect("ML-DSA keypair generation should succeed");
            let relay_mldsa_private_hex = hex::encode(relay_mldsa_keys.secret_key());
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
                    tls_spki_sha256,
                    priority: 0,
                    tags: vec!["norito".to_string()],
                }],
                capability_flags: RelayCapabilityFlagsV1::new(
                    CapabilityToggle::Enabled,
                    CapabilityToggle::Disabled,
                    CapabilityToggle::Enabled,
                    CapabilityToggle::Disabled,
                ),
                handshake_suites: vec![
                    HandshakeSuite::Nk3PqForwardSecure,
                    HandshakeSuite::Nk2Hybrid,
                ],
                published_at: 1,
                valid_after: 1,
                valid_until,
                directory_hash: [0x66; 32],
                issuer_fingerprint: [0x77; 32],
            };
            let issuer_seed = [0x99; 32];
            let issuer_signing = SigningKey::from_bytes(&issuer_seed);
            let issuer_mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
                .expect("ML-DSA keypair generation should succeed");
            let bundle = certificate
                .issue(&issuer_signing, issuer_mldsa_keys.secret_key())
                .expect("issue certificate");
            let bundle_file = secure_test_tempfile();
            std::fs::write(
                bundle_file.path(),
                bundle
                    .try_to_cbor()
                    .expect("sample relay bundle should encode"),
            )
            .expect("write bundle");
            let manifest_file = secure_test_tempfile();
            std::fs::write(
                manifest_file.path(),
                format!(
                    r#"{{
                        "version": 1,
                        "identity": {{
                            "ed25519_private_key_hex": "{}",
                            "mldsa65_private_key_hex": "{}"
                        }}
                    }}"#,
                    identity_seed_hex, relay_mldsa_private_hex,
                ),
            )
            .expect("write manifest");
            let issuer_ed25519_hex = hex::encode(issuer_signing.verifying_key().to_bytes());
            let issuer_mldsa_hex = hex::encode(issuer_mldsa_keys.public_key());
            Self {
                identity_seed,
                descriptor_commit,
                bundle,
                bundle_file,
                manifest_file,
                issuer_ed25519_hex,
                issuer_mldsa_hex,
            }
        }
        fn certificate_config_json(&self) -> String {
            format!(
                r#"{{
                    "bundle_path": "{}",
                    "issuer_ed25519_hex": "{}",
                    "issuer_mldsa_hex": "{}"
                }}"#,
                self.bundle_file.path().display(),
                self.issuer_ed25519_hex,
                self.issuer_mldsa_hex,
            )
        }
        fn handshake_config_json(&self, manifest_path: &Path) -> String {
            format!(
                r#"{{
                    "descriptor_manifest_path": "{}",
                    "certificate": {}
                }}"#,
                manifest_path.display(),
                self.certificate_config_json(),
            )
        }
    }
    #[test]
    fn generates_self_signed_config() {
        let config = RelayRuntime::self_signed_server_config("relay.test");
        assert!(config.is_ok());
    }
    #[test]
    fn relay_quic_server_applies_first_release_resource_limits() {
        let config = RelayRuntime::self_signed_server_config("relay.test")
            .expect("build relay QUIC configuration");
        let rendered = format!("{config:?}");
        for expected in [
            "max_concurrent_bidi_streams: 32",
            "max_concurrent_uni_streams: 8",
            "max_idle_timeout: Some(30000)",
            "stream_receive_window: 262144",
            "receive_window: 4194304",
            "send_window: 4194304",
            "crypto_buffer_size: 65536",
            "allow_spin: false",
            "datagram_receive_buffer_size: Some(65536)",
            "datagram_send_buffer_size: 65536",
            "migration: false",
            "max_incoming: 64",
            "incoming_buffer_size: 65536",
            "incoming_buffer_size_total: 4194304",
        ] {
            assert!(
                rendered.contains(expected),
                "missing `{expected}` in QUIC config: {rendered}"
            );
        }
    }
    #[tokio::test]
    async fn relay_quic_address_gate_requires_retry_validation() {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
                .expect("generate test certificate");
        let key =
            PrivateKeyDer::try_from(signing_key.serialize_der()).expect("encode test private key");
        let server_config = RelayRuntime::server_config(vec![cert.der().clone()], key)
            .expect("build relay QUIC configuration");
        let server = Endpoint::server(
            server_config,
            "127.0.0.1:0".parse().expect("parse loopback address"),
        )
        .expect("bind relay QUIC endpoint");

        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(cert.der().clone())
            .expect("trust test relay certificate");
        let mut client_tls =
            rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_root_certificates(roots)
                .with_no_client_auth();
        client_tls.alpn_protocols = vec![SORANET_QUIC_ALPN.to_vec()];
        let client_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(client_tls)
            .expect("build QUIC client crypto");
        let mut client = Endpoint::client(
            "127.0.0.1:0"
                .parse()
                .expect("parse client loopback address"),
        )
        .expect("bind QUIC client endpoint");
        client.set_default_client_config(quinn::ClientConfig::new(Arc::new(client_crypto)));
        let connecting = client
            .connect(server.local_addr().expect("relay address"), "relay.test")
            .expect("start QUIC connection");
        let client_task = tokio::spawn(connecting);

        let initial = timeout(Duration::from_secs(2), server.accept())
            .await
            .expect("initial connection attempt timed out")
            .expect("initial incoming connection");
        assert!(!initial.remote_address_validated());
        assert!(RelayRuntime::require_validated_quic_address(initial).is_none());

        let retried = timeout(Duration::from_secs(2), server.accept())
            .await
            .expect("retried connection attempt timed out")
            .expect("retried incoming connection");
        let retried = RelayRuntime::require_validated_quic_address(retried)
            .expect("retry token must validate the peer address");
        assert!(retried.remote_address_validated());
        let server_connecting = retried.accept().expect("accept validated connection");
        let (server_connection, client_connection) = timeout(Duration::from_secs(2), async {
            let (server_result, client_result) = tokio::join!(server_connecting, client_task);
            (
                server_result.expect("server QUIC handshake"),
                client_result
                    .expect("client task")
                    .expect("client QUIC handshake"),
            )
        })
        .await
        .expect("validated QUIC handshake timed out");
        server_connection.close(0u32.into(), b"test complete");
        client_connection.close(0u32.into(), b"test complete");
    }
    #[tokio::test]
    async fn finished_quic_stream_delivers_final_protected_vpn_record() {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
                .expect("generate test certificate");
        let key =
            PrivateKeyDer::try_from(signing_key.serialize_der()).expect("encode test private key");
        let server_config = RelayRuntime::server_config(vec![cert.der().clone()], key)
            .expect("build relay QUIC configuration");
        let server = Endpoint::server(
            server_config,
            "127.0.0.1:0".parse().expect("parse loopback address"),
        )
        .expect("bind relay QUIC endpoint");

        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(cert.der().clone())
            .expect("trust test relay certificate");
        let mut client_tls =
            rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_root_certificates(roots)
                .with_no_client_auth();
        client_tls.alpn_protocols = vec![SORANET_QUIC_ALPN.to_vec()];
        let client_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(client_tls)
            .expect("build QUIC client crypto");
        let mut client = Endpoint::client(
            "127.0.0.1:0"
                .parse()
                .expect("parse client loopback address"),
        )
        .expect("bind QUIC client endpoint");
        client.set_default_client_config(quinn::ClientConfig::new(Arc::new(client_crypto)));
        let connecting = client
            .connect(server.local_addr().expect("relay address"), "relay.test")
            .expect("start QUIC connection");
        let client_task = tokio::spawn(connecting);
        let initial = timeout(Duration::from_secs(2), server.accept())
            .await
            .expect("initial connection attempt timed out")
            .expect("initial incoming connection");
        assert!(RelayRuntime::require_validated_quic_address(initial).is_none());
        let retried = timeout(Duration::from_secs(2), server.accept())
            .await
            .expect("retried connection attempt timed out")
            .expect("retried incoming connection");
        let server_connecting = RelayRuntime::require_validated_quic_address(retried)
            .expect("retry token must validate the peer address")
            .accept()
            .expect("accept validated connection");
        let (server_connection, client_connection) = timeout(Duration::from_secs(2), async {
            let (server_result, client_result) = tokio::join!(server_connecting, client_task);
            (
                server_result.expect("server QUIC handshake"),
                client_result
                    .expect("client task")
                    .expect("client QUIC handshake"),
            )
        })
        .await
        .expect("validated QUIC handshake timed out");

        let sender = async {
            let (mut send, _recv) = client_connection.open_bi().await.expect("open stream");
            let context = record_stream_context(send.id());
            let record = RecordLayer::new(SessionKey::new(vec![0xB6; 32]), RecordEndpoint::Client)
                .expect("client record layer")
                .stream(context)
                .expect("client record stream");
            let overlay = VpnOverlay::from_config(VpnConfig::default());
            let cell = overlay
                .data_cell(
                    [0xB6; 16],
                    vpn_flow_label_from_session_id([0xB6; 16]),
                    0,
                    0,
                    VpnCellFlagsV1::new(false, false, false, false),
                    b"final protected packet".to_vec(),
                )
                .expect("vpn cell");
            let frame = overlay.pad_cell(cell).expect("vpn frame");
            let mut protected = RecordWriter::new(&mut send, record.sealer);
            crate::vpn::write_frame(&mut protected, &frame)
                .await
                .expect("write final frame");
            protected.shutdown().await.expect("finish protected stream");
            drop(protected);
            timeout(
                Duration::from_secs(2),
                wait_for_finished_quic_send_stream(&send),
            )
            .await
            .expect("peer acknowledgement timed out")
            .expect("peer acknowledged final stream bytes");
        };
        let receiver = async {
            let (_send, recv) = server_connection.accept_bi().await.expect("accept stream");
            let context = record_stream_context(recv.id());
            let record = RecordLayer::new(SessionKey::new(vec![0xB6; 32]), RecordEndpoint::Relay)
                .expect("relay record layer")
                .stream(context)
                .expect("relay record stream");
            let overlay = VpnOverlay::from_config(VpnConfig::default());
            let mut protected = RecordReader::new(recv, record.opener);
            let cell = crate::vpn::read_frame(&overlay, &mut protected)
                .await
                .expect("read final frame");
            let mut trailing = Vec::new();
            protected
                .read_to_end(&mut trailing)
                .await
                .expect("read finished stream");
            assert!(trailing.is_empty());
            cell
        };
        let ((), cell) = tokio::join!(sender, receiver);
        assert_eq!(cell.payload, b"final protected packet");
        server_connection.close(0u32.into(), b"test complete");
        client_connection.close(0u32.into(), b"test complete");
        server.close(0u32.into(), b"test complete");
        client.close(0u32.into(), b"test complete");
        tokio::join!(server.wait_idle(), client.wait_idle());
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
    #[test]
    fn production_transport_requires_tls_bundle_directory_and_exact_leaf_pin() {
        let missing_tls = load_config(r#"{"mode":"Entry","listen":"127.0.0.1:0"}"#);
        let error = RelayRuntime::prepare_server_transport(&missing_tls.config, None, None, false)
            .expect_err("production transport must not synthesize a self-signed leaf");
        assert!(error.to_string().contains("tls.certificate_path"));

        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
                .expect("generate pinned test certificate");
        let leaf_spki =
            leaf_certificate_spki_sha256(cert.der().as_ref()).expect("extract test leaf SPKI");
        let directory = secure_test_tempdir();
        let certificate_path = directory.path().join("relay-chain.pem");
        let private_key_path = directory.path().join("relay-key.pem");
        std::fs::write(&certificate_path, cert.pem()).expect("write certificate");
        std::fs::write(&private_key_path, signing_key.serialize_pem()).expect("write private key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&private_key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect private key");
        }
        let config = load_config(&format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "tls": {{
                    "certificate_path": "{}",
                    "private_key_path": "{}"
                }},
                "guard_directory": {{
                    "snapshot_path": "{}",
                    "expected_snapshot_digest_hex": "{}"
                }}
            }}"#,
            certificate_path.display(),
            private_key_path.display(),
            directory.path().join("directory.norito").display(),
            "11".repeat(32),
        ));
        let matching = CertificateTestFixture::with_spki(leaf_spki);
        let (_, trust) = RelayRuntime::prepare_server_transport(
            &config.config,
            Some(&matching.bundle),
            Some(2_000_000_000),
            false,
        )
        .expect("exact signed leaf pin and directory trust");
        assert_eq!(
            trust
                .expect("authenticated transport trust")
                .tls_spki_sha256,
            leaf_spki
        );

        let missing_bundle = RelayRuntime::prepare_server_transport(
            &config.config,
            None,
            Some(2_000_000_000),
            false,
        )
        .expect_err("TLS files without an authenticated relay certificate must fail");
        assert!(
            missing_bundle
                .to_string()
                .contains("authenticated relay certificate")
        );
        let mismatched = CertificateTestFixture::new();
        let mismatch = RelayRuntime::prepare_server_transport(
            &config.config,
            Some(&mismatched.bundle),
            Some(2_000_000_000),
            false,
        )
        .expect_err("leaf key substitution must fail for non-VPN relays too");
        assert!(
            mismatch
                .to_string()
                .contains("selected signed relay endpoint pin")
        );
        let missing_directory = RelayRuntime::prepare_server_transport(
            &config.config,
            Some(&matching.bundle),
            None,
            false,
        )
        .expect_err("authenticated directory validity is mandatory");
        assert!(missing_directory.to_string().contains("directory validity"));
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
            relay_mldsa65_public_key: [0x23; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1],
            tls_spki_sha256: [0x33; 32],
            relay_certificate_sha256: [0x44; 32],
            directory_snapshot_digest: [0x55; 32],
            valid_until_ms: u64::MAX,
        };
        let expected =
            vpn_helper_handshake_binding(b"ticket", &relay_id, &descriptor_commit, &trust);
        let mut manual = blake3::Hasher::new();
        for value in [
            b"iroha.soranet.vpn.helper-handshake-dual-auth.v1".as_slice(),
            b"ticket".as_slice(),
            trust.quic_multiaddr.as_bytes(),
            relay_id.as_slice(),
            trust.relay_mldsa65_public_key.as_slice(),
            descriptor_commit.as_slice(),
            trust.tls_spki_sha256.as_slice(),
            trust.relay_certificate_sha256.as_slice(),
            trust.directory_snapshot_digest.as_slice(),
            trust.tls_server_name.as_bytes(),
            SORANET_QUIC_ALPN,
        ] {
            manual.update(
                &u64::try_from(value.len())
                    .expect("test transcript field length fits u64")
                    .to_be_bytes(),
            );
            manual.update(value);
        }
        assert_eq!(expected, *manual.finalize().as_bytes());
        let mut changed = trust.clone();
        changed.relay_mldsa65_public_key[0] ^= 0x01;
        assert_ne!(
            expected,
            vpn_helper_handshake_binding(b"ticket", &relay_id, &descriptor_commit, &changed)
        );
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
            relay_mldsa65_public_key: [0x23; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1],
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
    #[test]
    fn authenticated_transport_trust_expires_before_handshake_admission() {
        let trust = RelayTransportTrust {
            quic_multiaddr: "/dns/relay.test/udp/443/quic".to_owned(),
            tls_server_name: "relay.test".to_owned(),
            relay_mldsa65_public_key: [0x23; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1],
            tls_spki_sha256: [0x33; 32],
            relay_certificate_sha256: [0x44; 32],
            directory_snapshot_digest: [0x55; 32],
            valid_until_ms: 100,
        };
        ensure_transport_trust_current(Some(&trust), 99)
            .expect("trust is valid strictly before its authenticated boundary");
        assert!(matches!(
            ensure_transport_trust_current(Some(&trust), 100),
            Err(HandshakeError::TransportTrustExpired)
        ));
        assert!(matches!(
            ensure_transport_trust_current(Some(&trust), 101),
            Err(HandshakeError::TransportTrustExpired)
        ));
        ensure_transport_trust_current(None, u64::MAX)
            .expect("the crate-private self-signed test constructor has no production trust");
    }
    #[tokio::test]
    async fn vpn_helper_ticket_replay_is_rejected_after_relay_restart() {
        let directory = secure_test_tempdir();
        let config = VpnConfig {
            enabled: true,
            lease_secs: 60,
            helper_ticket_issuer_public_key_path: Some(
                directory.path().join("helper-ticket-issuer-public-key.hex"),
            ),
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
            let ledger = Arc::new(ledger);
            redeem_vpn_helper_ticket(ledger, &ticket, now_ms)
                .await
                .expect("first redemption must be persisted");
        }
        let reloaded = load_vpn_helper_ticket_replay_ledger(&config, &relay_id, now_ms + 1)
            .expect("reload durable replay ledger");
        let error = redeem_vpn_helper_ticket(Arc::new(reloaded), &ticket, now_ms + 1)
            .await
            .expect_err("persisted redemption must survive restart");
        assert!(matches!(
            error,
            HandshakeError::HelperTicket(VpnHelperTicketError::Replayed)
        ));
    }
    #[test]
    fn vpn_helper_ticket_pending_reservation_is_atomic_and_failure_releases_it() {
        let directory = secure_test_tempdir();
        let now_ms = 2_000_000;
        let mut ticket = sample_helper_ticket([0x73; 16]);
        ticket.expires_at_ms = now_ms + 30_000;
        let config = VpnConfig {
            lease_secs: 60,
            helper_ticket_replay_store_capacity: 4,
            helper_ticket_replay_store_path: directory.path().join("helper-replays.norito"),
            ..VpnConfig::default()
        };
        let state = load_vpn_helper_ticket_replay_ledger(&config, &ticket.relay_id, now_ms)
            .expect("create replay state");
        let state = Arc::new(state);
        let reservation =
            VpnHelperTicketReplayReservation::reserve(Arc::clone(&state), &ticket, now_ms)
                .expect("first reservation");
        let duplicate =
            VpnHelperTicketReplayReservation::reserve(Arc::clone(&state), &ticket, now_ms)
                .expect_err("concurrent duplicate must fail before Noise");
        assert!(matches!(
            duplicate,
            HandshakeError::HelperTicket(VpnHelperTicketError::Replayed)
        ));
        drop(reservation);
        VpnHelperTicketReplayReservation::reserve(state, &ticket, now_ms)
            .expect("failed handshake must leave the ticket retryable");
    }
    #[tokio::test]
    async fn cancelling_helper_handshake_releases_pending_replay_reservation() {
        let directory = secure_test_tempdir();
        let now_ms = 2_100_000;
        let mut ticket = sample_helper_ticket([0x74; 16]);
        ticket.expires_at_ms = now_ms + 30_000;
        let config = VpnConfig {
            lease_secs: 60,
            helper_ticket_replay_store_capacity: 4,
            helper_ticket_replay_store_path: directory.path().join("helper-replays.norito"),
            ..VpnConfig::default()
        };
        let state = Arc::new(
            load_vpn_helper_ticket_replay_ledger(&config, &ticket.relay_id, now_ms)
                .expect("create replay state"),
        );
        let reservation =
            VpnHelperTicketReplayReservation::reserve(Arc::clone(&state), &ticket, now_ms)
                .expect("reserve ticket");
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _reservation = reservation;
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx
            .await
            .expect("reservation moved into handshake task");
        task.abort();
        let _ = task.await;
        VpnHelperTicketReplayReservation::reserve(state, &ticket, now_ms)
            .expect("cancellation must release the pending ticket");
    }
    #[tokio::test]
    async fn completed_helper_handshake_spends_ticket_before_later_abort() {
        let directory = secure_test_tempdir();
        let now_ms = 2_200_000;
        let mut ticket = sample_helper_ticket([0x75; 16]);
        ticket.expires_at_ms = now_ms + 30_000;
        let config = VpnConfig {
            lease_secs: 60,
            helper_ticket_replay_store_capacity: 4,
            helper_ticket_replay_store_path: directory.path().join("helper-replays.norito"),
            ..VpnConfig::default()
        };
        let state = Arc::new(
            load_vpn_helper_ticket_replay_ledger(&config, &ticket.relay_id, now_ms)
                .expect("create replay state"),
        );
        let reservation =
            VpnHelperTicketReplayReservation::reserve(Arc::clone(&state), &ticket, now_ms)
                .expect("reserve authenticated helper ticket");
        commit_vpn_helper_ticket_reservation(reservation, now_ms)
            .await
            .expect("application-handshake success burns ticket durably");

        let replay = VpnHelperTicketReplayReservation::reserve(state, &ticket, now_ms + 1)
            .expect_err("post-handshake abort must not make the ticket reusable");
        assert!(matches!(
            replay,
            HandshakeError::HelperTicket(VpnHelperTicketError::Replayed)
        ));
    }
    #[test]
    fn helper_ticket_reservation_precedes_client_hello_and_noise() {
        let source = include_str!("../runtime.rs");
        let start = source
            .find("async fn perform_handshake(")
            .expect("relay handshake function");
        let handshake = &source[start..];
        let reserve = handshake
            .find("VpnHelperTicketReplayReservation::reserve")
            .expect("helper replay reservation");
        let client_hello = handshake
            .find("preflight_client_hello")
            .expect("client hello preflight");
        let noise = handshake
            .find("process_client_hello")
            .expect("Noise/ML-KEM processing");
        assert!(reserve < client_hello && reserve < noise);
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
            helper_ticket_issuer_public_key_path: Some(
                directory.path().join("helper-ticket-issuer-public-key.hex"),
            ),
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
            helper_ticket_issuer_public_key_path: Some(
                directory.path().join("helper-ticket-issuer-public-key.hex"),
            ),
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
            &ledger,
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
    fn runtime_rejects_missing_private_manifest() {
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "certificate": {}
                }}
            }}"#,
            fixture.certificate_config_json(),
        );
        let config = load_config(&json);
        match RelayRuntime::new_for_test(config.config) {
            Err(RelayError::Config(ConfigError::Handshake(message))) => {
                assert!(message.contains("handshake.descriptor_manifest_path"));
            }
            Err(other) => panic!("expected missing-manifest config error, got {other}"),
            Ok(_) => panic!("runtime must fail closed without a private descriptor manifest"),
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
        let runtime = RelayRuntime::new_for_test(config.config).expect("runtime");
        assert_eq!(runtime.descriptor_commit(), fixture.descriptor_commit);
        let stored_bundle = runtime.certificate_bundle();
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
        let err = match RelayRuntime::new_for_test(config.config) {
            Ok(_) => panic!("expired certificate must fail at startup"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("expired"),
            "unexpected startup error: {err}"
        );
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
        match RelayRuntime::new_for_test(config.config) {
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
    fn validate_client_selection_rejects_unsupported_signature_id() {
        let negotiated = negotiated_caps_fixture();
        let err = validate_client_selection(&negotiated, KemId::MlKem768.code(), 0x02)
            .expect_err("unsupported signature identifier should fail");
        match err {
            HandshakeError::InvalidClient(field) => {
                assert_eq!(field, "client sig_id is not a supported signature suite");
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
    fn runtime_honours_exact_private_manifest_and_certificate_identities() {
        let seed_hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let seed: [u8; 32] = hex::decode(seed_hex)
            .expect("valid fixture seed")
            .try_into()
            .expect("fixture seed has exact Ed25519 width");
        let fixture = CertificateTestFixture::with_identity_seed(seed);
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {}
            }}"#,
            fixture.handshake_config_json(fixture.manifest_file.path()),
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new_for_test(config.config).expect("runtime");
        let context = runtime.circuit_context();
        let expected_private =
            PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("configured key parse");
        let expected_pair =
            KeyPair::from_private_key(expected_private).expect("configured keypair derive");
        assert_eq!(
            context.identity_key.public_key(),
            expected_pair.public_key()
        );
        assert_eq!(
            context.relay_authentication_signer.ed25519_public_key(),
            expected_pair.public_key()
        );
        let (_, signer_mldsa65) = context
            .relay_authentication_signer
            .mldsa65_public_key()
            .try_to_bytes()
            .expect("fixture ML-DSA-65 public key");
        assert_eq!(
            signer_mldsa65,
            fixture.bundle.certificate.identity_mldsa65.as_slice()
        );
        let expected_binding: [u8; 32] = Sha256::digest(
            fixture
                .bundle
                .try_to_cbor()
                .expect("canonical fixture bundle"),
        )
        .into();
        assert_eq!(
            context
                .relay_authentication_signer
                .authenticated_binding_digest(),
            &expected_binding
        );
    }
    #[test]
    fn runtime_uses_mandatory_pow_policy() {
        let dir = secure_test_tempdir();
        let replay_path = dir.path().join("ticket-replays.norito");
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {},
                "pow": {{
                    "difficulty": 6,
                    "max_future_skew_secs": 120,
                    "min_ticket_ttl_secs": 10,
                    "revocation_store_path": "{}"
                }}
            }}"#,
            fixture.handshake_config_json(fixture.manifest_file.path()),
            replay_path.display()
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new_for_test(config.config).expect("runtime");
        let context = runtime.circuit_context();
        assert_eq!(context.dos.current_puzzle_parameters().difficulty(), 6);
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
        let fixture = CertificateTestFixture::new();
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {},
                "pow": {{
                    "difficulty": 6,
                    "max_future_skew_secs": 120,
                    "min_ticket_ttl_secs": 10,
                    "revocation_store_path": "{}"
                }}
            }}"#,
            fixture.handshake_config_json(fixture.manifest_file.path()),
            replay_path.display()
        );
        let config = load_config(&json);
        match RelayRuntime::new_for_test(config.config) {
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
        let seed: [u8; 32] = hex::decode(seed_hex)
            .expect("valid fixture seed")
            .try_into()
            .expect("fixture seed has exact Ed25519 width");
        let fixture = CertificateTestFixture::with_identity_seed(seed);
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {}
            }}"#,
            fixture.handshake_config_json(fixture.manifest_file.path()),
        );
        let config = load_config(&json);
        let runtime = RelayRuntime::new_for_test(config.config).expect("runtime");
        let context = runtime.circuit_context();
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
        let fixture = CertificateTestFixture::new();
        std::fs::write(
            fixture.manifest_file.path(),
            format!(
                r#"{{
                    "version": 1,
                    "identity": {{
                        "ed25519_private_key_hex": "{}"
                    }}
                }}"#,
                hex::encode(fixture.identity_seed),
            ),
        )
        .expect("write manifest");
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {}
            }}"#,
            fixture.handshake_config_json(fixture.manifest_file.path()),
        );
        let config = load_config(&json);
        match RelayRuntime::new_for_test(config.config) {
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
    fn admin_auth_rejects_ambiguous_or_body_framed_headers() {
        const TOKEN: &str = "soranet-test-admin-token-00000001";
        let valid = format!(
            "GET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
        );
        assert_eq!(
            RelayRuntime::parse_admin_request(&valid).and_then(|request| request.bearer_token),
            Some(TOKEN)
        );
        for invalid in [
            format!("GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n folded\r\n\r\n"),
            format!(
                "GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: 0\r\n\r\n"
            ),
            format!(
                "GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nTransfer-Encoding: chunked\r\n\r\n"
            ),
            format!(
                "GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
            format!("GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nMalformed\r\n\r\n"),
        ] {
            assert!(
                RelayRuntime::parse_admin_request(&invalid).is_none(),
                "ambiguous request was accepted: {invalid:?}"
            );
        }
    }
    #[test]
    fn admin_parser_rejects_request_line_and_host_smuggling_forms() {
        const TOKEN: &str = "soranet-test-admin-token-00000001";
        let http_10 = format!("GET /metrics HTTP/1.0\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
        assert_eq!(
            RelayRuntime::parse_admin_request(&http_10).and_then(|request| request.bearer_token),
            Some(TOKEN)
        );
        for invalid in [
            format!("GET /metrics HTTP/1.1\nHost: localhost\nAuthorization: Bearer {TOKEN}\n\n"),
            format!(
                "GET\t/metrics\tHTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
            format!(
                "GET  /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
            format!(
                "GET http://localhost/metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
            format!(
                "GET //localhost/metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
            format!("GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"),
            format!(
                "GET /metrics HTTP/1.1\r\nHost: localhost\r\nHost: proxy\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
            format!("GET /metrics HTTP/1.1\r\nHost:\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"),
            format!(
                "GET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization:  Bearer {TOKEN}\r\n\r\n"
            ),
            format!(
                "GET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization:\tBearer {TOKEN}\r\n\r\n"
            ),
            format!(
                "GET /ignored HTTP/1.1\nGET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
            ),
        ] {
            assert!(
                RelayRuntime::parse_admin_request(&invalid).is_none(),
                "ambiguous request was accepted: {invalid:?}"
            );
        }
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
    #[test]
    fn quic_handshake_permits_enforce_capacity() {
        let permits = Arc::new(Semaphore::new(1));
        let permit = RelayRuntime::try_quic_handshake_permit(&permits)
            .expect("first pending handshake should be admitted");
        assert!(RelayRuntime::try_quic_handshake_permit(&permits).is_none());
        drop(permit);
        assert!(RelayRuntime::try_quic_handshake_permit(&permits).is_some());
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
            SoranetPrivacyThrottleScopeV1::RemoteQuota,
        );
        proxy_policy_events.record_downgrade(privacy_mode, event_time);
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
        let mut health_stream = TcpStream::connect(addr)
            .await
            .expect("connect to protected health endpoint");
        health_stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("write unauthenticated health request");
        let mut health_response = Vec::new();
        health_stream
            .read_to_end(&mut health_response)
            .await
            .expect("read unauthenticated health response");
        let health_text = String::from_utf8(health_response).expect("response must be UTF-8");
        assert!(
            health_text.starts_with("HTTP/1.1 401 Unauthorized"),
            "health endpoint bypassed authentication: {health_text}"
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
            !downgrade_text.contains("\"detail\""),
            "downgrade payload must not expose free-form detail: {downgrade_text}"
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
        proxy_policy_events.record_downgrade(privacy_mode, event_time);
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
            !ndjson.contains("\"detail\""),
            "proxy policy NDJSON must not carry free-form detail: {ndjson}"
        );
    }
}

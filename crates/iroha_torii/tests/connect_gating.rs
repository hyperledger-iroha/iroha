#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Verify that Connect routes are stable while runtime configuration controls availability.

use std::{path::PathBuf, sync::Arc};

use axum::http::{Request, StatusCode, Uri};
use iroha_config::base::WithOrigin;
use iroha_core::{
    kiso::KisoHandle, kura::Kura, prelude::World, query::store::LiveQueryStore, queue::Queue,
    state::State,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{ChainId, block::BlockHeader};
use iroha_primitives::addr::socket_addr;
use nonzero_ext::nonzero;
use tower::ServiceExt;

fn request_with_loopback_connect_info(
    request: Request<axum::body::Body>,
) -> Request<axum::body::Body> {
    let mut request = request;
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    request
}

fn connect_session_request_body(seed: u8) -> (String, String) {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let network_id = iroha_torii::test_utils::signed_query_network_id();
    let app_pk = [seed; 32];
    let nonce = [seed.wrapping_add(1); 16];
    let sid = iroha_torii_shared::connect_sdk::derive_session_id(&network_id, &app_pk, &nonce);
    let sid_b64 = B64.encode(sid);
    let body = norito::json::to_json(&iroha_torii::json_object(vec![
        ("sid", Some(sid_b64.clone())),
        ("network_id", Some(network_id.to_string())),
        ("app_pk", Some(B64.encode(app_pk))),
        ("nonce", Some(B64.encode(nonce))),
        ("node", Option::<String>::None),
    ]))
    .expect("Connect session JSON serialization");
    (sid_b64, body)
}

#[cfg(feature = "ws_integration_tests")]
fn spawn_test_server(listener: tokio::net::TcpListener, app: axum::Router) {
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
}

fn checked_connect_key_fixture() -> iroha_crypto::KeyPair {
    iroha_crypto::KeyPair::try_random().expect("generate checked connect fixture keypair")
}

fn checked_connect_transport_key_fixture() -> iroha_crypto::KeyPair {
    iroha_crypto::KeyPair::try_from_seed(
        b"iroha:torii:connect-gating:soranet-transport:v1".to_vec(),
        iroha_crypto::Algorithm::Ed25519,
    )
    .expect("generate dedicated connect SoraNet transport fixture keypair")
}

#[test]
fn connect_config_fixture_uses_checked_key_generation() {
    let key_pair = checked_connect_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture connect public key has a valid algorithm");

    assert_eq!(algorithm, iroha_crypto::Algorithm::Ed25519);

    let transport_key_pair = checked_connect_transport_key_fixture();
    assert_eq!(
        transport_key_pair.algorithm(),
        iroha_crypto::Algorithm::Ed25519
    );
    assert_ne!(transport_key_pair.public_key(), key_pair.public_key());
}

#[allow(clippy::too_many_lines)]
fn minimal_actual_config(connect_enabled: bool) -> iroha_config::parameters::actual::Root {
    use iroha_config::parameters::actual as A;
    use iroha_config::parameters::defaults::governance::sorafs_pin_fee;
    use iroha_crypto::{
        soranet::handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        },
        streaming::StreamingKeyMaterial,
    };
    use iroha_data_model::peer::Peer;
    use iroha_logger::Level;

    let connect = A::Connect {
        enabled: connect_enabled,
        ws_max_sessions: iroha_config::parameters::defaults::connect::WS_MAX_SESSIONS,
        ws_per_ip_max_sessions: iroha_config::parameters::defaults::connect::WS_PER_IP_MAX_SESSIONS,
        ws_rate_per_ip_per_min: iroha_config::parameters::defaults::connect::WS_RATE_PER_IP_PER_MIN,
        session_ttl: iroha_config::parameters::defaults::connect::SESSION_TTL,
        frame_max_bytes: iroha_config::parameters::defaults::connect::FRAME_MAX_BYTES,
        session_buffer_max_bytes:
            iroha_config::parameters::defaults::connect::SESSION_BUFFER_MAX_BYTES,
        ping_interval: iroha_config::parameters::defaults::connect::PING_INTERVAL,
        ping_miss_tolerance: iroha_config::parameters::defaults::connect::PING_MISS_TOLERANCE,
        ping_min_interval: iroha_config::parameters::defaults::connect::PING_MIN_INTERVAL,
        dedupe_ttl: iroha_config::parameters::defaults::connect::DEDUPE_TTL,
        dedupe_cap: iroha_config::parameters::defaults::connect::DEDUPE_CAP,
        relay_enabled: iroha_config::parameters::defaults::connect::RELAY_ENABLED,
        relay_strategy: iroha_config::parameters::defaults::connect::RELAY_STRATEGY,
        p2p_ttl_hops: iroha_config::parameters::defaults::connect::P2P_TTL_HOPS,
    };

    A::Root {
        common: A::Common {
            chain: ChainId::from("test-chain"),
            key_pair: checked_connect_key_fixture(),
            soranet_transport_key_pair: checked_connect_transport_key_fixture(),
            peer: Peer::new(
                socket_addr!(127.0.0.1:0),
                checked_connect_key_fixture().public_key().clone(),
            ),
            trusted_peers: WithOrigin::inline(A::TrustedPeers {
                myself: Peer::new(
                    socket_addr!(127.0.0.1:0),
                    checked_connect_key_fixture().public_key().clone(),
                ),
                others: iroha_primitives::unique_vec::UniqueVec::new(),
                pops: std::collections::BTreeMap::new(),
            }),
            chain_discriminant: WithOrigin::inline(
                iroha_config::parameters::defaults::common::chain_discriminant(),
            ),
        },
        network: A::Network {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            relay_mode: A::RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            relay_ttl: iroha_config::parameters::defaults::network::RELAY_TTL,
            peer_gossip_period: iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            trust_decay_half_life: iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip: iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer: iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            debug_packet_loss_inbound_percent: 0,
            debug_packet_loss_outbound_percent: 0,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            soranet_handshake: A::SoranetHandshake {
                descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
                client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
                relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
                trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
                kem_id: 1,
                sig_id: 1,
                resume_hash: None,
                pow: A::SoranetPow::default(),
            },
            soranet_privacy: A::SoranetPrivacy::default(),
            soranet_vpn: A::SoranetVpn::default(),
            lane_profile: A::LaneProfile::Core,
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout: core::time::Duration::from_secs(5),
            reply_writer_flush_timeout:
                iroha_config::parameters::defaults::network::REPLY_WRITER_FLUSH_TIMEOUT,
            connect_startup_delay:
                iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            dial_timeout: iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
            deferred_send_ttl: core::time::Duration::from_millis(
                iroha_config::parameters::defaults::network::DEFERRED_SEND_TTL_MS,
            ),
            deferred_send_max_per_peer:
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_PER_PEER,
            deferred_send_max_bytes_per_peer:
                iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_BYTES_PER_PEER,
            deferred_send_max_bytes_total: iroha_config::parameters::defaults::network::DEFERRED_SEND_MAX_BYTES_TOTAL,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            p2p_proxy: None,
            p2p_proxy_required: false,
            p2p_no_proxy: Vec::new(),
            p2p_proxy_tls_verify: true,
            p2p_proxy_tls_pinned_cert_der_base64: None,
            quic_enabled: false,
            quic_datagrams_enabled:
                iroha_config::parameters::defaults::network::QUIC_DATAGRAMS_ENABLED,
            quic_datagram_max_payload_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get(),
            quic_datagram_receive_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
            quic_datagram_send_buffer_bytes: iroha_config::parameters::defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
            scion: A::ScionConfig::default(),
            tls_enabled: false,
            tls_fallback_to_plain: false,
            tls_listen_address: None,
            tls_inbound_only: false,
            prefer_ws_fallback: false,
            p2p_queue_cap_high: nonzero!(128usize),
            p2p_queue_cap_low: nonzero!(512usize),
            p2p_post_queue_cap: nonzero!(128usize),
            p2p_outbound_frame_queue_max_high_bytes:
                iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_BYTES,
            p2p_outbound_frame_queue_max_low_bytes:
                iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_LOW_BYTES,
            p2p_outbound_frame_queue_max_high_frames:
                iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_HIGH_FRAMES,
            p2p_outbound_frame_queue_max_low_frames:
                iroha_config::parameters::defaults::network::P2P_OUTBOUND_FRAME_QUEUE_MAX_LOW_FRAMES,
            p2p_subscriber_queue_cap: nonzero!(128usize),
            consensus_ingress_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RATE_PER_SEC,
            consensus_ingress_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BURST,
            consensus_ingress_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_PER_SEC,
            consensus_ingress_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_BURST,
            consensus_ingress_critical_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
            consensus_ingress_critical_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BURST,
            consensus_ingress_critical_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
            consensus_ingress_critical_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
            consensus_ingress_penalty_threshold:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
            consensus_ingress_penalty_window: core::time::Duration::from_millis(
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_WINDOW_MS,
            ),
            consensus_ingress_penalty_cooldown: core::time::Duration::from_millis(
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS,
            ),
            happy_eyeballs_stagger: core::time::Duration::from_millis(100),
            addr_ipv6_first: false,
            max_incoming: None,
            max_total_connections: None,
            accept_rate_per_ip_per_sec: None,
            accept_burst_per_ip: None,
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits: iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
            accept_rate_per_prefix_per_sec: None,
            accept_burst_per_prefix: None,
            low_priority_rate_per_sec: None,
            low_priority_burst: None,
            low_priority_bytes_per_sec: None,
            low_priority_bytes_burst: None,
            allowlist_only: false,
            allow_keys: Vec::new(),
            deny_keys: Vec::new(),
            allow_cidrs: Vec::new(),
            deny_cidrs: Vec::new(),
            disconnect_on_post_overflow: false,
            max_frame_bytes: 256 * 1024,
            tcp_nodelay: true,
            tcp_keepalive: None,
            max_frame_bytes_consensus: 128 * 1024,
            max_frame_bytes_control: 128 * 1024,
            max_frame_bytes_block_sync: 512 * 1024,
            max_frame_bytes_tx_gossip: 128 * 1024,
            max_frame_bytes_peer_gossip: 64 * 1024,
            max_frame_bytes_health: 32 * 1024,
            max_frame_bytes_other: 128 * 1024,
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        },
        genesis: A::Genesis {
            public_key: checked_connect_key_fixture().public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"Connect gating test genesis trust anchor",
            )),
        },
        torii: A::Torii {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            max_content_len: (1_048_576u64).into(),
            data_dir: iroha_config::parameters::defaults::torii::data_dir(),
            receipt_signer: None,
            transport: A::ToriiTransport::default(),
            mcp: A::ToriiMcp::default(),
            cors: A::ToriiCors::default(),
            ram_lfe: None,
            tx_history: None,
            recipient_lookup: A::ToriiRecipientLookup::default(),
            // minimal defaults
            query_rate_per_authority_per_sec: None,
            query_burst_per_authority: None,
            query_max_inflight: iroha_config::parameters::defaults::torii::QUERY_MAX_INFLIGHT,
            query_heavy_max_inflight:
                iroha_config::parameters::defaults::torii::QUERY_HEAVY_MAX_INFLIGHT,
            query_queue_timeout: std::time::Duration::from_millis(
                iroha_config::parameters::defaults::torii::QUERY_QUEUE_TIMEOUT_MS,
            ),
            tx_rate_per_authority_per_sec: None,
            tx_burst_per_authority: None,
            deploy_rate_per_origin_per_sec: None,
            deploy_burst_per_origin: None,
            soracloud_public_rate_per_ip_per_sec: iroha_config::parameters::defaults::torii::SORACLOUD_PUBLIC_RATE_PER_IP_PER_SEC
                .and_then(std::num::NonZeroU32::new),
            soracloud_public_burst_per_ip: iroha_config::parameters::defaults::torii::SORACLOUD_PUBLIC_BURST_PER_IP
                .and_then(std::num::NonZeroU32::new),
            soracloud_public_max_inflight: iroha_config::parameters::defaults::torii::SORACLOUD_PUBLIC_MAX_INFLIGHT,
            soracloud_public_max_response_bytes:
                iroha_config::parameters::defaults::torii::SORACLOUD_PUBLIC_MAX_RESPONSE_BYTES,
            soracloud_mutation_rate_per_account_origin_per_sec: iroha_config::parameters::defaults::torii::SORACLOUD_MUTATION_RATE_PER_ACCOUNT_ORIGIN_PER_SEC
                .and_then(std::num::NonZeroU32::new),
            soracloud_mutation_burst_per_account_origin: iroha_config::parameters::defaults::torii::SORACLOUD_MUTATION_BURST_PER_ACCOUNT_ORIGIN
                .and_then(std::num::NonZeroU32::new),
            soracloud_mutation_max_inflight: iroha_config::parameters::defaults::torii::SORACLOUD_MUTATION_MAX_INFLIGHT,
            soracloud_mutation_max_body_bytes:
                iroha_config::parameters::defaults::torii::SORACLOUD_MUTATION_MAX_BODY_BYTES,
            soracloud_upload_max_body_bytes:
                iroha_config::parameters::defaults::torii::SORACLOUD_UPLOAD_MAX_BODY_BYTES,
            proof_api: A::ProofApi {
                rate_per_minute: iroha_config::parameters::defaults::torii::PROOF_RATE_PER_MIN
                    .and_then(std::num::NonZeroU32::new),
                burst: iroha_config::parameters::defaults::torii::PROOF_BURST
                    .and_then(std::num::NonZeroU32::new),
                max_body_bytes: iroha_config::parameters::defaults::torii::PROOF_MAX_BODY_BYTES,
                body_max_inflight:
                    iroha_config::parameters::defaults::torii::PROOF_BODY_MAX_INFLIGHT,
                body_read_timeout: std::time::Duration::from_millis(
                    iroha_config::parameters::defaults::torii::PROOF_BODY_READ_TIMEOUT_MS,
                ),
                egress_bytes_per_sec: iroha_config::parameters::defaults::torii::PROOF_EGRESS_BYTES_PER_SEC
                    .and_then(std::num::NonZeroU64::new),
                egress_burst_bytes: iroha_config::parameters::defaults::torii::PROOF_EGRESS_BURST_BYTES
                    .and_then(std::num::NonZeroU64::new),
                max_list_limit: std::num::NonZeroU32::new(
                    iroha_config::parameters::defaults::torii::PROOF_MAX_LIST_LIMIT.max(1),
                )
                .expect("proof page limit must be non-zero"),
                request_timeout: core::time::Duration::from_millis(
                    iroha_config::parameters::defaults::torii::PROOF_REQUEST_TIMEOUT_MS,
                ),
                cache_max_age: core::time::Duration::from_secs(
                    iroha_config::parameters::defaults::torii::PROOF_CACHE_MAX_AGE_SECS,
                ),
                retry_after: core::time::Duration::from_secs(
                    iroha_config::parameters::defaults::torii::PROOF_RETRY_AFTER_SECS,
                ),
            },
            require_api_token: false,
            api_tokens: Vec::new(),
            soranet_privacy_ingest: iroha_config::parameters::actual::SoranetPrivacyIngest::default(),
            privacy_bootle_lantern_issuer: None,
            api_fee_asset_id: None,
            api_fee_amount: None,
            api_fee_receiver: None,
            api_rate_limit_bypass_cidrs: Vec::new(),
            internal_api_trusted_cidrs:
                iroha_config::parameters::defaults::torii::internal_api_trusted_cidrs(),
            peer_telemetry_urls: Vec::new(),
            peer_geo: A::ToriiPeerGeo::default(),
            debug_match_filters: false,
            operator_auth: A::ToriiOperatorAuth::default(),
            operator_signatures: A::ToriiOperatorSignatures::default(),
            preauth_max_connections: None,
            preauth_max_connections_per_ip: None,
            preauth_rate_per_ip_per_sec: None,
            preauth_burst_per_ip: None,
            preauth_temp_ban: None,
            preauth_ban_capacity:
                iroha_config::parameters::defaults::torii::PREAUTH_BAN_CAPACITY,
            preauth_allow_cidrs: Vec::new(),
            preauth_scheme_limits: Vec::new(),
            api_high_load_tx_threshold: None,
            api_high_load_stream_threshold: None,
            api_high_load_subscription_threshold: None,
            events_buffer_capacity: iroha_config::parameters::defaults::torii::events_buffer_capacity(
            ),
            ws_message_timeout: std::time::Duration::from_millis(
                iroha_config::parameters::defaults::torii::WS_MESSAGE_TIMEOUT_MS,
            ),
            webhooks_enabled: iroha_config::parameters::defaults::torii::WEBHOOKS_ENABLED,
            zk_attachments_enabled:
                iroha_config::parameters::defaults::torii::ZK_ATTACHMENTS_ENABLED,
            app_api: iroha_config::parameters::actual::AppApi {
                default_list_limit: std::num::NonZeroU32::new(
                    iroha_config::parameters::defaults::torii::APP_API_DEFAULT_LIST_LIMIT.max(1),
                )
                .expect("default list limit must be non-zero"),
                max_list_limit: std::num::NonZeroU32::new(
                    iroha_config::parameters::defaults::torii::APP_API_MAX_LIST_LIMIT.max(1),
                )
                .expect("max list limit must be non-zero"),
                max_fetch_size: std::num::NonZeroU32::new(
                    iroha_config::parameters::defaults::torii::APP_API_MAX_FETCH_SIZE.max(1),
                )
                .expect("max fetch size must be non-zero"),
                rate_limit_cost_per_row: std::num::NonZeroU32::new(
                    iroha_config::parameters::defaults::torii::APP_API_RATE_LIMIT_COST_PER_ROW
                        .max(1),
                )
                .expect("rate limit cost must be non-zero"),
                request_signature_max_clock_skew: std::time::Duration::from_secs(
                    iroha_config::parameters::defaults::torii::app_auth::MAX_CLOCK_SKEW_SECS,
                ),
                request_signature_nonce_ttl: std::time::Duration::from_secs(
                    iroha_config::parameters::defaults::torii::app_auth::NONCE_TTL_SECS,
                ),
                request_signature_replay_cache_capacity: std::num::NonZeroUsize::new(
                    iroha_config::parameters::defaults::torii::app_auth::REPLAY_CACHE_CAPACITY
                        .max(1),
                )
                .expect("request signature replay cache must be non-zero"),
            },
            attachments_ttl_secs: 7 * 24 * 60 * 60,
            attachments_max_bytes: 4 * 1024 * 1024,
            attachments_per_tenant_max_count:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT,
            attachments_per_tenant_max_bytes:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_BYTES,
            attachments_allowed_mime_types:
                iroha_config::parameters::defaults::torii::attachments_allowed_mime_types(),
            attachments_max_expanded_bytes:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES,
            attachments_max_archive_depth:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH,
            attachments_sanitizer_mode:
                iroha_config::parameters::actual::AttachmentSanitizerMode::Subprocess,
            attachments_sanitize_timeout_ms:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS,
            webhook: iroha_config::parameters::actual::Webhook::default(),
            webhook_security: A::WebhookSecurity::default(),
            push: iroha_config::parameters::actual::Push::default(),
            zk_prover_enabled: false,
            zk_prover_scan_period_secs: 30,
            zk_prover_reports_ttl_secs:
                iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_TTL_SECS,
            zk_prover_max_inflight:
                iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_INFLIGHT,
            zk_prover_max_scan_bytes:
                iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_SCAN_BYTES,
            zk_prover_max_scan_millis:
                iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_SCAN_MILLIS,
            zk_prover_keys_dir: iroha_config::parameters::defaults::torii::zk_prover_keys_dir(),
            zk_prover_allowed_backends:
                iroha_config::parameters::defaults::torii::zk_prover_allowed_backends(),
            zk_prover_allowed_circuits:
                iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits(),
            zk_ivm_prove_max_inflight:
                iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_MAX_INFLIGHT,
            zk_ivm_prove_max_queue: iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_MAX_QUEUE,
            zk_ivm_tooling_timeout_ms:
                iroha_config::parameters::defaults::torii::ZK_IVM_TOOLING_TIMEOUT_MS,
            zk_ivm_prove_job_ttl_secs:
                iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_JOB_TTL_SECS,
            zk_ivm_prove_job_max_entries:
                iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_JOB_MAX_ENTRIES,
            zk_ivm_prove_job_max_retained_bytes:
                iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_JOB_MAX_RETAINED_BYTES,
            zk_ivm_prove_job_max_entries_per_owner:
                iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_JOB_MAX_ENTRIES_PER_OWNER,
            zk_ivm_prove_job_max_retained_bytes_per_owner:
                iroha_config::parameters::defaults::torii::ZK_IVM_PROVE_JOB_MAX_RETAINED_BYTES_PER_OWNER,
            transaction_ingress: A::TransactionIngress::default(),
            da_ingest: A::DaIngest::default(),
            connect,
            iso_bridge: iroha_config::parameters::actual::IsoBridge {
                enabled: false,
                max_body_bytes:
                    iroha_config::parameters::defaults::torii::ISO_BRIDGE_MAX_BODY_BYTES,
                dedupe_ttl_secs:
                    iroha_config::parameters::defaults::torii::ISO_BRIDGE_DEDUPE_TTL_SECS,
                default_profile: iroha_config::parameters::defaults::torii::ISO_BRIDGE_DEFAULT_PROFILE
                    .to_owned(),
                profiles: Vec::new(),
                store_dir: None,
                store_retention_secs:
                    iroha_config::parameters::defaults::torii::ISO_BRIDGE_STORE_RETENTION_SECS,
                store_max_records:
                    iroha_config::parameters::defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS,
                audit_export_dir: None,
                embedded_signature_policy: None,
                signer: None,
                account_aliases: Vec::new(),
                currency_assets: Vec::new(),
                reference_data: A::IsoReferenceData::default(),
            },
            sorafs_discovery: A::SorafsDiscovery::default(),
            sorafs_storage: iroha_config::parameters::actual::SorafsStorage::default(),
            sorafs_repair: iroha_config::parameters::actual::SorafsRepair::default(),
            sorafs_gc: iroha_config::parameters::actual::SorafsGc::default(),
            sorafs_quota: iroha_config::parameters::actual::SorafsQuota::default(),
            sorafs_alias_cache: iroha_config::parameters::actual::SorafsAliasCachePolicy::default(),
            sorafs_gateway: iroha_config::parameters::actual::SorafsGateway::default(),
            sorafs_por: iroha_config::parameters::actual::SorafsPor::default(),
            sorafs_appeal_finance_settlement:
                iroha_config::parameters::actual::SorafsAppealFinanceSettlement::default(),
            account_onboarding: None,
            faucet: None,
            kagemusha_commands: None,
        },
        soracloud_runtime: A::SoracloudRuntime::default(),
        kura: A::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: WithOrigin::inline(std::env::temp_dir()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: nonzero!(10usize),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        sumeragi: A::Sumeragi::default(),
        block_sync: A::BlockSync {
            gossip_period: core::time::Duration::from_millis(200),
            gossip_max_period: core::time::Duration::from_millis(200),
            gossip_size: nonzero!(32u32),
        },
        transaction_gossiper: A::TransactionGossiper {
            gossip_period: core::time::Duration::from_millis(200),
            gossip_size: nonzero!(32u32),
            gossip_resend_ticks: iroha_config::parameters::defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
            dataspace: A::DataspaceGossip::default(),
        },
        live_query_store: A::LiveQueryStore::default(),
        logger: A::Logger {
            level: Level::INFO,
            filter: None,
            format: iroha_config::logger::Format::default(),
            terminal_colors: false,
        },
        queue: A::Queue::default(),
        nexus: A::Nexus::default(),
        snapshot: iroha_config::parameters::user::Snapshot {
            mode: iroha_config::snapshot::Mode::Disabled,
            create_every_ms: iroha_config::base::util::DurationMs(core::time::Duration::from_secs(
                60,
            )),
            store_dir: WithOrigin::inline(std::env::temp_dir()),
            merkle_chunk_size_bytes:
                iroha_config::parameters::defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES,
            max_payload_bytes:
                iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES,
            resources: Default::default(),
            verification_public_key: None,
            signing_private_key: None,
            bootstrap: Default::default(),
        },
        telemetry_enabled: false,
        telemetry_profile: A::TelemetryProfile::Disabled,
        telemetry: None,
        telemetry_redaction: A::TelemetryRedaction::default(),
        telemetry_integrity: A::TelemetryIntegrity::default(),
        dev_telemetry: iroha_config::parameters::user::DevTelemetry {
            out_file: None,
            panic_on_duplicate_metrics: iroha_config::parameters::defaults::telemetry::PANIC_ON_DUPLICATE_METRICS,
        },
        pipeline: iroha_config::parameters::actual::Pipeline {
            dynamic_prepass: false,
            access_set_cache_enabled:
                iroha_config::parameters::defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
            parallel_overlay: false,
            workers: iroha_config::parameters::defaults::pipeline::WORKERS,
            stateless_cache_cap: iroha_config::parameters::defaults::pipeline::STATELESS_CACHE_CAP,
            parallel_apply: true,
            ready_queue_heap: iroha_config::parameters::defaults::pipeline::READY_QUEUE_HEAP,
            gpu_key_bucket: iroha_config::parameters::defaults::pipeline::GPU_KEY_BUCKET,
            debug_trace_scheduler_inputs:
                iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
            debug_trace_tx_eval:
                iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_TX_EVAL,
            signature_batch_max: iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX,
            signature_batch_max_ed25519:
                iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
            signature_batch_max_secp256k1:
                iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
            signature_batch_max_pqc:
                iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
            signature_batch_max_bls:
                iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
            cache_size: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
            ivm_cache_max_decoded_ops:
                iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
            ivm_cache_max_bytes:
                iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
            ivm_prover_threads: iroha_config::parameters::defaults::pipeline::IVM_PROVER_THREADS,
            overlay_max_instructions:
                iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
            overlay_max_bytes: iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_BYTES,
            overlay_chunk_instructions:
                iroha_config::parameters::defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
            gas: iroha_config::parameters::actual::Gas {
                tech_account_id: iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID
                    .to_string(),
                accepted_assets: Vec::new(),
                units_per_gas: Vec::new(),
            },
            ivm_max_cycles_upper_bound:
                iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
            ivm_max_decoded_instructions:
                iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
            ivm_max_decoded_bytes:
                iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES,
            quarantine_max_txs_per_block:
                iroha_config::parameters::defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
            quarantine_tx_max_cycles:
                iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
            query_default_cursor_mode: iroha_config::parameters::actual::QueryCursorMode::Ephemeral,
            query_max_fetch_size: iroha_config::parameters::defaults::pipeline::QUERY_MAX_FETCH_SIZE,
            query_stored_min_gas_units: 0,
            amx_per_dataspace_budget_ms:
                iroha_config::parameters::defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
            amx_group_budget_ms: iroha_config::parameters::defaults::pipeline::AMX_GROUP_BUDGET_MS,
            amx_per_instruction_ns:
                iroha_config::parameters::defaults::pipeline::AMX_PER_INSTRUCTION_NS,
            amx_per_memory_access_ns:
                iroha_config::parameters::defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
            amx_per_syscall_ns: iroha_config::parameters::defaults::pipeline::AMX_PER_SYSCALL_NS,
        },
        tiered_state: iroha_config::parameters::actual::TieredState {
            enabled: false,
            hot_retained_keys: 0,
            hot_retained_bytes: iroha_config::parameters::defaults::tiered_state::HOT_RETAINED_BYTES,
            hot_retained_grace_snapshots:
                iroha_config::parameters::defaults::tiered_state::HOT_RETAINED_GRACE_SNAPSHOTS,
            cold_store_root: None,
            da_store_root: None,
            max_snapshots: 0,
            max_cold_bytes: iroha_config::parameters::defaults::tiered_state::MAX_COLD_BYTES,
        },
        compute: iroha_config::parameters::actual::Compute {
            enabled: iroha_config::parameters::defaults::compute::ENABLED,
            namespaces: iroha_config::parameters::defaults::compute::default_namespaces()
                .into_iter()
                .collect(),
            default_ttl_slots: iroha_config::parameters::defaults::compute::default_ttl_slots(),
            max_ttl_slots: iroha_config::parameters::defaults::compute::max_ttl_slots(),
            max_request_bytes: iroha_config::parameters::defaults::compute::MAX_REQUEST_BYTES,
            max_response_bytes: iroha_config::parameters::defaults::compute::MAX_RESPONSE_BYTES,
            max_gas_per_call: iroha_config::parameters::defaults::compute::max_gas_per_call(),
            resource_profiles: iroha_config::parameters::defaults::compute::resource_profiles(),
            default_resource_profile:
                iroha_config::parameters::defaults::compute::default_resource_profile(),
            price_families: iroha_config::parameters::defaults::compute::price_families(),
            default_price_family:
                iroha_config::parameters::defaults::compute::default_price_family(),
            auth_policy: iroha_config::parameters::defaults::compute::default_auth_policy(),
            sandbox: iroha_config::parameters::defaults::compute::sandbox_rules(),
            economics: iroha_config::parameters::actual::ComputeEconomics {
                max_cu_per_call: iroha_config::parameters::defaults::compute::max_cu_per_call(),
                max_amplification_ratio:
                    iroha_config::parameters::defaults::compute::max_amplification_ratio(),
                fee_split: iroha_config::parameters::defaults::compute::fee_split(),
                sponsor_policy: iroha_config::parameters::defaults::compute::sponsor_policy(),
                price_bounds: iroha_config::parameters::defaults::compute::price_bounds(),
                price_risk_classes:
                    iroha_config::parameters::defaults::compute::price_risk_classes(),
                price_family_baseline:
                    iroha_config::parameters::defaults::compute::price_families(),
                price_amplifiers: iroha_config::parameters::defaults::compute::price_amplifiers(),
            },
            slo: iroha_config::parameters::actual::ComputeSlo {
                max_inflight_per_route:
                    iroha_config::parameters::defaults::compute::max_inflight_per_route(),
                queue_depth_per_route:
                    iroha_config::parameters::defaults::compute::queue_depth_per_route(),
                max_requests_per_second:
                    iroha_config::parameters::defaults::compute::max_requests_per_second(),
                target_p50_latency_ms:
                    iroha_config::parameters::defaults::compute::target_p50_latency_ms(),
                target_p95_latency_ms:
                    iroha_config::parameters::defaults::compute::target_p95_latency_ms(),
                target_p99_latency_ms:
                    iroha_config::parameters::defaults::compute::target_p99_latency_ms(),
            },
        },
        content: iroha_config::parameters::actual::Content {
            max_bundle_bytes: iroha_config::parameters::defaults::content::MAX_BUNDLE_BYTES,
            max_files: iroha_config::parameters::defaults::content::MAX_FILES,
            max_path_len: iroha_config::parameters::defaults::content::MAX_PATH_LEN,
            max_retention_blocks: iroha_config::parameters::defaults::content::MAX_RETENTION_BLOCKS,
            chunk_size_bytes: iroha_config::parameters::defaults::content::CHUNK_SIZE_BYTES,
            publish_allow_accounts: Vec::new(),
            limits: iroha_config::parameters::actual::ContentLimits {
                max_requests_per_second: nonzero!(
                    iroha_config::parameters::defaults::content::MAX_REQUESTS_PER_SECOND
                ),
                request_burst: nonzero!(iroha_config::parameters::defaults::content::REQUEST_BURST),
                max_egress_bytes_per_second: std::num::NonZeroU64::new(
                    u64::from(
                        iroha_config::parameters::defaults::content::MAX_EGRESS_BYTES_PER_SECOND,
                    ),
                )
                .expect("non-zero egress limit"),
                egress_burst_bytes: std::num::NonZeroU64::new(
                    iroha_config::parameters::defaults::content::EGRESS_BURST_BYTES
                )
                .expect("non-zero egress burst"),
            },
            default_cache_max_age_secs:
                iroha_config::parameters::defaults::content::DEFAULT_CACHE_MAX_AGE_SECS,
            max_cache_max_age_secs: iroha_config::parameters::defaults::content::MAX_CACHE_MAX_AGE_SECS,
            immutable_bundles: iroha_config::parameters::defaults::content::IMMUTABLE_BUNDLES,
            default_auth_mode: iroha_data_model::content::ContentAuthMode::Public,
            slo: iroha_config::parameters::actual::ContentSlo {
                target_p50_latency_ms: nonzero!(
                    iroha_config::parameters::defaults::content::TARGET_P50_LATENCY_MS
                ),
                target_p99_latency_ms: nonzero!(
                    iroha_config::parameters::defaults::content::TARGET_P99_LATENCY_MS
                ),
                target_availability_bps: nonzero!(
                    iroha_config::parameters::defaults::content::TARGET_AVAILABILITY_BPS
                ),
            },
            pow: iroha_config::parameters::actual::ContentPow {
                difficulty_bits: iroha_config::parameters::defaults::content::POW_DIFFICULTY_BITS,
                header_name: iroha_config::parameters::defaults::content::default_pow_header(),
            },
            stripe_layout: iroha_config::parameters::defaults::content::default_stripe_layout(),
        },
        oracle: iroha_config::parameters::actual::Oracle {
            history_depth: iroha_config::parameters::defaults::oracle::history_depth(),
            economics: iroha_config::parameters::actual::OracleEconomics {
                reward_asset: iroha_config::parameters::defaults::oracle::reward_asset(),
                reward_pool: iroha_config::parameters::defaults::oracle::reward_pool(),
                reward_amount: iroha_config::parameters::defaults::oracle::reward_amount(),
                slash_asset: iroha_config::parameters::defaults::oracle::slash_asset(),
                slash_receiver: iroha_config::parameters::defaults::oracle::slash_receiver(),
                slash_outlier_amount: iroha_config::parameters::defaults::oracle::slash_outlier_amount(),
                slash_error_amount: iroha_config::parameters::defaults::oracle::slash_error_amount(),
                slash_no_show_amount: iroha_config::parameters::defaults::oracle::slash_no_show_amount(),
                dispute_bond_asset: iroha_config::parameters::defaults::oracle::dispute_bond_asset(),
                dispute_bond_amount: iroha_config::parameters::defaults::oracle::dispute_bond_amount(),
                dispute_reward_amount: iroha_config::parameters::defaults::oracle::dispute_reward_amount(),
                frivolous_slash_amount: iroha_config::parameters::defaults::oracle::frivolous_slash_amount(),
            },
            governance: iroha_config::parameters::actual::OracleGovernance {
                intake_sla_blocks: iroha_config::parameters::defaults::oracle::intake_sla_blocks(),
                rules_sla_blocks: iroha_config::parameters::defaults::oracle::rules_sla_blocks(),
                cop_sla_blocks: iroha_config::parameters::defaults::oracle::cop_sla_blocks(),
                technical_sla_blocks: iroha_config::parameters::defaults::oracle::technical_sla_blocks(),
                policy_jury_sla_blocks: iroha_config::parameters::defaults::oracle::policy_jury_sla_blocks(),
                enact_sla_blocks: iroha_config::parameters::defaults::oracle::enact_sla_blocks(),
                intake_min_votes: iroha_config::parameters::defaults::oracle::intake_min_votes(),
                rules_min_votes: iroha_config::parameters::defaults::oracle::rules_min_votes(),
                cop_min_votes: iroha_config::parameters::actual::OracleChangeThresholds {
                    low: iroha_config::parameters::defaults::oracle::cop_low_votes(),
                    medium: iroha_config::parameters::defaults::oracle::cop_medium_votes(),
                    high: iroha_config::parameters::defaults::oracle::cop_high_votes(),
                },
                technical_min_votes: iroha_config::parameters::defaults::oracle::technical_min_votes(),
                policy_jury_min_votes: iroha_config::parameters::actual::OracleChangeThresholds {
                    low: iroha_config::parameters::defaults::oracle::policy_jury_low_votes(),
                    medium: iroha_config::parameters::defaults::oracle::policy_jury_medium_votes(),
                    high: iroha_config::parameters::defaults::oracle::policy_jury_high_votes(),
                },
            },
            twitter_binding: iroha_config::parameters::actual::OracleTwitterBinding {
                feed_id: iroha_config::parameters::defaults::oracle::twitter_binding_feed_id(),
                pepper_id: iroha_config::parameters::defaults::oracle::twitter_binding_pepper_id(),
                max_ttl_ms: iroha_config::parameters::defaults::oracle::twitter_binding_max_ttl_ms(),
                min_ttl_ms: iroha_config::parameters::defaults::oracle::twitter_binding_min_ttl_ms(),
                min_update_spacing_ms: iroha_config::parameters::defaults::oracle::twitter_binding_min_update_spacing_ms(),
            },
        },
        zk: iroha_config::parameters::actual::Zk {
            halo2: iroha_config::parameters::actual::Halo2 {
                enabled: false,
                curve: iroha_config::parameters::actual::ZkCurve::Pallas,
                backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
                max_k: 16,
                verifier_budget_ms: 1000,
                verifier_max_batch: 8,
                ..iroha_config::parameters::actual::Halo2::default()
            },
            fastpq: iroha_config::parameters::actual::Fastpq {
                execution_mode: iroha_config::parameters::actual::FastpqExecutionMode::Cpu,
                poseidon_mode: iroha_config::parameters::actual::FastpqPoseidonMode::Cpu,
                proof_sidecar_queue_cap:
                    iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
                proof_sidecar_max_bytes:
                    iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
                proof_sidecar_max_retries:
                    iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
                device_class: None,
                chip_family: None,
                gpu_kind: None,
                metal_queue_fanout: None,
                metal_queue_column_threshold: None,
                metal_max_in_flight: None,
                metal_threadgroup_width: None,
                metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
                metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
                metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
            },
            stark: iroha_config::parameters::actual::Stark::default(),
            sccp: iroha_config::parameters::actual::Sccp::default(),
            ballot_history_cap: iroha_config::parameters::defaults::zk::vote::BALLOT_HISTORY_CAP,
            preverify_max_bytes: iroha_config::parameters::defaults::zk::preverify::MAX_BYTES,
            preverify_budget_bytes: iroha_config::parameters::defaults::zk::preverify::BUDGET_BYTES,
            proof_history_cap: iroha_config::parameters::defaults::zk::proof::RECORD_HISTORY_CAP,
            proof_retention_grace_blocks:
                iroha_config::parameters::defaults::zk::proof::RETENTION_GRACE_BLOCKS,
            proof_prune_batch: iroha_config::parameters::defaults::zk::proof::PRUNE_BATCH_SIZE,
            bridge_proof_max_range_len:
                iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
            bridge_proof_max_past_age_blocks:
                iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
            bridge_proof_max_future_drift_blocks:
                iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
            poseidon_params_id:
                iroha_config::parameters::defaults::confidential::POSEIDON_PARAMS_ID,
            pedersen_params_id:
                iroha_config::parameters::defaults::confidential::PEDERSEN_PARAMS_ID,
            kaigi_roster_join_vk: None,
            kaigi_roster_leave_vk: None,
            kaigi_usage_vk: None,
            max_proof_size_bytes:
                iroha_config::parameters::defaults::confidential::MAX_PROOF_SIZE_BYTES,
            max_nullifiers_per_tx:
                iroha_config::parameters::defaults::confidential::MAX_NULLIFIERS_PER_TX,
            max_commitments_per_tx:
                iroha_config::parameters::defaults::confidential::MAX_COMMITMENTS_PER_TX,
            max_confidential_ops_per_block:
                iroha_config::parameters::defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
            verify_timeout: iroha_config::parameters::defaults::confidential::VERIFY_TIMEOUT,
            max_anchor_age_blocks:
                iroha_config::parameters::defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
            max_proof_bytes_block:
                iroha_config::parameters::defaults::confidential::MAX_PROOF_BYTES_BLOCK,
            max_verify_calls_per_tx:
                iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
            max_verify_calls_per_block:
                iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
            max_public_inputs: iroha_config::parameters::defaults::confidential::MAX_PUBLIC_INPUTS,
            reorg_depth_bound: iroha_config::parameters::defaults::confidential::REORG_DEPTH_BOUND,
            policy_transition_delay_blocks:
                iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
            policy_transition_window_blocks:
                iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
            policy_transition_max_per_height:
                iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_MAX_PER_HEIGHT,
            tree_roots_history_len:
                iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            tree_frontier_checkpoint_interval:
                iroha_config::parameters::defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
            registry_max_vk_entries:
                iroha_config::parameters::defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
            registry_max_params_entries:
                iroha_config::parameters::defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
            registry_max_delta_per_block:
                iroha_config::parameters::defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
            gas: iroha_config::parameters::actual::ConfidentialGas {
                proof_base: iroha_config::parameters::defaults::confidential::gas::PROOF_BASE,
                per_public_input:
                    iroha_config::parameters::defaults::confidential::gas::PER_PUBLIC_INPUT,
                per_proof_byte:
                    iroha_config::parameters::defaults::confidential::gas::PER_PROOF_BYTE,
                per_nullifier: iroha_config::parameters::defaults::confidential::gas::PER_NULLIFIER,
                per_commitment:
                    iroha_config::parameters::defaults::confidential::gas::PER_COMMITMENT,
            },
        },
        norito: iroha_config::parameters::actual::Norito {
            allow_gpu_compression:
                iroha_config::parameters::defaults::norito::ALLOW_GPU_COMPRESSION,
            max_archive_len: iroha_config::parameters::defaults::norito::MAX_ARCHIVE_LEN,
        },
        hijiri: A::Hijiri::new(None),
        fraud_monitoring: iroha_config::parameters::actual::FraudMonitoring {
            enabled: iroha_config::parameters::defaults::fraud_monitoring::ENABLED,
            service_endpoints: Vec::new(),
            connect_timeout: iroha_config::parameters::defaults::fraud_monitoring::CONNECT_TIMEOUT,
            request_timeout: iroha_config::parameters::defaults::fraud_monitoring::REQUEST_TIMEOUT,
            missing_assessment_grace: core::time::Duration::from_secs(
                iroha_config::parameters::defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS,
            ),
            required_minimum_band: None,
            attesters: Vec::new(),
        },
        // Minimal governance config for tests
        gov: iroha_config::parameters::actual::Governance {
            vk_ballot: None,
            vk_tally: None,
            voting_asset_id: iroha_config::parameters::defaults::governance::voting_asset_id()
                .parse()
                .expect("valid default governance asset id"),
            citizenship_asset_id:
                iroha_config::parameters::defaults::governance::citizenship_asset_id()
                .parse()
                .expect("valid default citizenship asset id"),
            citizenship_bond_amount:
                iroha_config::parameters::defaults::governance::CITIZENSHIP_BOND_AMOUNT.into(),
            citizenship_escrow_account:
                iroha_config::parameters::defaults::governance::citizenship_escrow_account_id(),
            min_bond_amount: 150_u64.into(),
            bond_escrow_account:
                iroha_config::parameters::defaults::governance::bond_escrow_account_id(),
            slash_receiver_account:
                iroha_config::parameters::defaults::governance::slash_receiver_account_id(),
            slash_double_vote_bps: 0,
            slash_invalid_proof_bps: 0,
            slash_ineligible_proof_bps: 0,
            citizen_service: iroha_config::parameters::actual::CitizenServiceDiscipline {
                seat_cooldown_blocks: iroha_config::parameters::defaults::governance::citizen_service::SEAT_COOLDOWN_BLOCKS,
                max_seats_per_epoch: iroha_config::parameters::defaults::governance::citizen_service::MAX_SEATS_PER_EPOCH,
                free_declines_per_epoch: iroha_config::parameters::defaults::governance::citizen_service::FREE_DECLINES_PER_EPOCH,
                decline_slash_bps: iroha_config::parameters::defaults::governance::citizen_service::DECLINE_SLASH_BPS,
                no_show_slash_bps: iroha_config::parameters::defaults::governance::citizen_service::NO_SHOW_SLASH_BPS,
                misconduct_slash_bps: iroha_config::parameters::defaults::governance::citizen_service::MISCONDUCT_SLASH_BPS,
                role_bond_multipliers: iroha_config::parameters::defaults::governance::citizen_service::role_bond_multipliers(
                ),
            },
            viral_incentives: iroha_config::parameters::actual::ViralIncentives::default(),
            sorafs_pin_policy:
                iroha_config::parameters::actual::SorafsPinPolicyConstraints::default(),
            sorafs_pin_fee_asset_id: sorafs_pin_fee::asset_id()
                .parse()
                .expect("valid default SoraFS pin fee asset id"),
            sorafs_pin_fee_treasury_account:
                iroha_data_model::account::AccountId::parse_encoded(
                    &sorafs_pin_fee::treasury_account(),
                )
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("valid default SoraFS pin fee treasury account"),
            sorafs_pricing:
                iroha_data_model::sorafs::pricing::PricingScheduleRecord::launch_default(),
            alias_teu_minimum:
                iroha_config::parameters::defaults::governance::alias_teu_minimum(),
            alias_frontier_telemetry:
                iroha_config::parameters::defaults::governance::alias_frontier_telemetry(),
            debug_trace_pipeline:
                iroha_config::parameters::defaults::governance::DEBUG_TRACE_PIPELINE,
            jdg_signature_schemes: iroha_config::parameters::defaults::governance::jdg_signature_schemes()
                .into_iter()
                .map(|scheme| {
                    scheme
                        .parse::<iroha_data_model::jurisdiction::JdgSignatureScheme>()
                        .expect("valid default JDG signature scheme")
                })
                .collect(),
            runtime_upgrade_provenance:
                iroha_config::parameters::actual::RuntimeUpgradeProvenancePolicy::default(),
            sorafs_penalty: iroha_config::parameters::actual::SorafsPenaltyPolicy::default(),
            sorafs_telemetry: iroha_config::parameters::actual::SorafsTelemetryPolicy::default(),
            sorafs_provider_owners: std::collections::BTreeMap::new(),
            conviction_step_blocks: 1,
            max_conviction: 1,
            min_enactment_delay: 1,
            window_span: 1,
            plain_voting_enabled: false,
            approval_threshold_q_num: 1,
            approval_threshold_q_den: 1,
            min_turnout: 0,
            parliament_committee_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_COMMITTEE_SIZE,
            parliament_term_blocks:
                iroha_config::parameters::defaults::governance::PARLIAMENT_TERM_BLOCKS,
            parliament_min_stake:
                iroha_config::parameters::defaults::governance::parliament_min_stake(),
            parliament_eligibility_asset_id:
                iroha_config::parameters::defaults::governance::parliament_eligibility_asset_id()
                    .parse()
                    .expect("valid default governance asset id"),
            parliament_alternate_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_ALTERNATE_SIZE,
            parliament_quorum_bps:
                iroha_config::parameters::defaults::governance::PARLIAMENT_QUORUM_BPS,
            rules_committee_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE,
            agenda_council_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE,
            interest_panel_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE,
            review_panel_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE,
            policy_jury_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_POLICY_JURY_SIZE,
            oversight_committee_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE,
            fma_committee_size:
                iroha_config::parameters::defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE,
            pipeline_study_sla_blocks: 1,
            pipeline_review_sla_blocks: 1,
            pipeline_decision_sla_blocks: 1,
            pipeline_enactment_sla_blocks: 2,
            pipeline_rules_sla_blocks:
                iroha_config::parameters::defaults::governance::PIPELINE_RULES_SLA_BLOCKS,
            pipeline_agenda_sla_blocks:
                iroha_config::parameters::defaults::governance::PIPELINE_AGENDA_SLA_BLOCKS,
        },
        // Acceleration defaults
        accel: iroha_config::parameters::actual::Acceleration {
            enable_simd: false,
            enable_cuda: false,
            enable_metal: false,
            max_gpus: None,
            merkle_min_leaves_gpu: iroha_config::parameters::defaults::accel::MERKLE_MIN_LEAVES_GPU,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        },
        ivm: iroha_config::parameters::actual::Ivm::default(),
        // Conservative concurrency defaults
        concurrency: iroha_config::parameters::actual::Concurrency {
            scheduler_min_threads: iroha_config::parameters::defaults::concurrency::SCHEDULER_MIN,
            scheduler_max_threads: iroha_config::parameters::defaults::concurrency::SCHEDULER_MAX,
            rayon_global_threads: iroha_config::parameters::defaults::concurrency::RAYON_GLOBAL,
            tokio_stack_bytes:
                iroha_config::parameters::defaults::concurrency::TOKIO_STACK_BYTES,
            scheduler_stack_bytes:
                iroha_config::parameters::defaults::concurrency::SCHEDULER_STACK_BYTES,
            prover_stack_bytes: iroha_config::parameters::defaults::concurrency::PROVER_STACK_BYTES,
            sumeragi_stack_bytes:
                iroha_config::parameters::defaults::concurrency::SUMERAGI_STACK_BYTES,
        },
        confidential: iroha_config::parameters::actual::Confidential {
            enabled: iroha_config::parameters::defaults::confidential::ENABLED,
            assume_valid: iroha_config::parameters::defaults::confidential::ASSUME_VALID,
            verifier_backend: iroha_config::parameters::defaults::confidential::VERIFIER_BACKEND
                .to_string(),
            max_proof_size_bytes:
                iroha_config::parameters::defaults::confidential::MAX_PROOF_SIZE_BYTES,
            max_nullifiers_per_tx:
                iroha_config::parameters::defaults::confidential::MAX_NULLIFIERS_PER_TX,
            max_commitments_per_tx:
                iroha_config::parameters::defaults::confidential::MAX_COMMITMENTS_PER_TX,
            max_confidential_ops_per_block:
                iroha_config::parameters::defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
            verify_timeout: iroha_config::parameters::defaults::confidential::VERIFY_TIMEOUT,
            max_anchor_age_blocks:
                iroha_config::parameters::defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
            max_proof_bytes_block:
                iroha_config::parameters::defaults::confidential::MAX_PROOF_BYTES_BLOCK,
            max_verify_calls_per_tx:
                iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
            max_verify_calls_per_block:
                iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
            max_public_inputs: iroha_config::parameters::defaults::confidential::MAX_PUBLIC_INPUTS,
            reorg_depth_bound: iroha_config::parameters::defaults::confidential::REORG_DEPTH_BOUND,
            policy_transition_delay_blocks:
                iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
            policy_transition_window_blocks:
                iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
            policy_transition_max_per_height:
                iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_MAX_PER_HEIGHT,
            tree_roots_history_len:
                iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            tree_frontier_checkpoint_interval:
                iroha_config::parameters::defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
            registry_max_vk_entries:
                iroha_config::parameters::defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
            registry_max_params_entries:
                iroha_config::parameters::defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
            registry_max_delta_per_block:
                iroha_config::parameters::defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
            gas: iroha_config::parameters::actual::ConfidentialGas {
                proof_base: iroha_config::parameters::defaults::confidential::gas::PROOF_BASE,
                per_public_input:
                    iroha_config::parameters::defaults::confidential::gas::PER_PUBLIC_INPUT,
                per_proof_byte:
                    iroha_config::parameters::defaults::confidential::gas::PER_PROOF_BYTE,
                per_nullifier: iroha_config::parameters::defaults::confidential::gas::PER_NULLIFIER,
                per_commitment:
                    iroha_config::parameters::defaults::confidential::gas::PER_COMMITMENT,
            },
        },
        settlement: iroha_config::parameters::actual::Settlement {
            offline: iroha_config::parameters::actual::Offline::default(),
            router: iroha_config::parameters::actual::Router::default(),
        },
        nts: iroha_config::parameters::actual::Nts {
            sample_interval: iroha_config::parameters::defaults::time::NTS_SAMPLE_INTERVAL,
            sample_cap_per_round:
                iroha_config::parameters::defaults::time::NTS_SAMPLE_CAP_PER_ROUND,
            max_rtt_ms: iroha_config::parameters::defaults::time::NTS_MAX_RTT_MS,
            trim_percent: iroha_config::parameters::defaults::time::NTS_TRIM_PERCENT,
            per_peer_buffer: iroha_config::parameters::defaults::time::NTS_PER_PEER_BUFFER,
            smoothing_enabled: iroha_config::parameters::defaults::time::NTS_SMOOTHING_ENABLED,
            smoothing_alpha: iroha_config::parameters::defaults::time::NTS_SMOOTHING_ALPHA,
            max_adjust_ms_per_min:
                iroha_config::parameters::defaults::time::NTS_MAX_ADJUST_MS_PER_MIN,
            min_samples: iroha_config::parameters::defaults::time::NTS_MIN_SAMPLES,
            max_offset_ms: iroha_config::parameters::defaults::time::NTS_MAX_OFFSET_MS,
            max_confidence_ms:
                iroha_config::parameters::defaults::time::NTS_MAX_CONFIDENCE_MS,
            enforcement_mode: A::NtsEnforcementMode::Warn,
        },
        crypto: A::Crypto::default(),
        streaming: A::Streaming {
            key_material: StreamingKeyMaterial::new(checked_connect_key_fixture())
                .expect("streaming key material"),
            session_store_dir: PathBuf::from(
                iroha_config::parameters::defaults::streaming::SESSION_STORE_DIR,
            ),
            feature_bits: iroha_config::parameters::defaults::streaming::FEATURE_BITS,
            soranet: iroha_config::parameters::actual::StreamingSoranet::from_defaults(),
            soravpn: iroha_config::parameters::actual::StreamingSoravpn::from_defaults(),
            sync: iroha_config::parameters::actual::StreamingSync::from_defaults(),
            codec: iroha_config::parameters::actual::StreamingCodec::from_defaults(),
        },
    }
}

fn build_torii(cfg: &iroha_config::parameters::actual::Root) -> iroha_torii::Torii {
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let (_mh, time_source) =
        iroha_primitives::time::TimeSource::new_mock(core::time::Duration::default());
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: nonzero!(1usize),
        capacity_per_user: nonzero!(1usize),
        transaction_time_to_live: core::time::Duration::from_secs(1),
        ..Default::default()
    };
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = (peers_tx, time_source);

    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    iroha_torii::Torii::new_with_handle(
        cfg.common.chain.clone(),
        iroha_torii::test_utils::signed_query_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        Kura::blank_kura_for_testing(),
        state,
        cfg.common.key_pair.clone(),
        iroha_torii::OnlinePeersProvider::new(peers_rx),
        None,
        telemetry,
    )
}

#[tokio::test]
async fn connect_endpoints_report_typed_unavailability_when_disabled() {
    let cfg = minimal_actual_config(false);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    // The WebSocket route remains mounted but rejects the upgrade while disabled.
    let resp = app
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/ws?sid=AA&role=app"))
                .body(axum::body::Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    // Ordinary REST routes return the shared typed error envelope.
    let resp = app
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .header(axum::http::header::ACCEPT, "application/json")
                .body(axum::body::Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .expect("connect disabled response body")
        .to_bytes();
    let error: norito::json::Value =
        norito::json::from_slice(&body).expect("typed Connect disabled error envelope");
    assert_eq!(
        error.get("code").and_then(norito::json::Value::as_str),
        Some("connect_disabled")
    );
}

#[tokio::test]
async fn connect_status_present_when_enabled() {
    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("status should be valid JSON");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    assert_eq!(
        p2p_rebroadcasts_total, 0,
        "fresh status snapshot should start with zero rebroadcasts"
    );
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(!relay_p2p_attached);
}

#[tokio::test]
async fn connect_status_forces_unknown_relay_strategy_to_local_only() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "bogus_strategy";
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("status should be valid JSON");
    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    assert_eq!(relay_strategy, "local_only");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(!relay_p2p_attached);
}

#[tokio::test]
async fn connect_status_normalizes_relay_strategy_aliases() {
    for (raw_strategy, expected) in [
        ("local_only", "local_only"),
        ("local-only", "local_only"),
        ("local", "local_only"),
        ("  BROADCAST  ", "broadcast"),
    ] {
        let mut cfg = minimal_actual_config(true);
        cfg.torii.connect.relay_strategy = raw_strategy;
        let torii = build_torii(&cfg);
        let app = torii.api_router_for_tests();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_strategy = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_strategy"))
            .and_then(norito::json::Value::as_str)
            .expect("connect status should include policy.relay_strategy");
        let p2p_rebroadcasts_total = payload
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .expect("connect status should include p2p_rebroadcasts_total");
        let p2p_rebroadcast_skipped_total = payload
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .expect("connect status should include p2p_rebroadcast_skipped_total");
        let relay_effective_strategy = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .expect("connect status should include policy.relay_effective_strategy");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        assert_eq!(
            relay_strategy, expected,
            "raw relay strategy {raw_strategy:?} should normalize"
        );
        assert_eq!(
            p2p_rebroadcasts_total, 0,
            "status-only probe should not rebroadcast p2p frames"
        );
        assert_eq!(p2p_rebroadcast_skipped_total, 0);
        assert_eq!(
            relay_effective_strategy, "local_only",
            "without a connected P2P network, status should report effective local-only relay"
        );
        assert!(!relay_p2p_attached);
    }
}

#[tokio::test]
async fn connect_status_reports_broadcast_effective_when_p2p_attached() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "broadcast";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let mut payload_opt = None;
    for _ in 0..50 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        if relay_p2p_attached {
            payload_opt = Some(payload);
            break;
        }
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
    let payload = payload_opt.expect("p2p should attach to connect bus");

    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");

    assert_eq!(relay_strategy, "broadcast");
    assert_eq!(relay_effective_strategy, "broadcast");
    assert!(relay_p2p_attached);
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
}

#[tokio::test]
async fn connect_status_reports_local_only_when_relay_disabled_with_p2p_attached() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_enabled = false;
    cfg.torii.connect.relay_strategy = "broadcast";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let mut payload_opt = None;
    for _ in 0..50 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        if relay_p2p_attached {
            payload_opt = Some(payload);
            break;
        }
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
    let payload = payload_opt.expect("p2p should attach to connect bus");

    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");

    assert_eq!(relay_strategy, "broadcast");
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
}

#[tokio::test]
async fn connect_status_reports_unknown_strategy_as_local_only_with_p2p_attached() {
    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "bogus_strategy";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let mut payload_opt = None;
    for _ in 0..50 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .expect("collect body")
            .to_bytes();
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("status should be valid JSON");
        let relay_p2p_attached = payload
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .expect("connect status should include policy.relay_p2p_attached");
        if relay_p2p_attached {
            payload_opt = Some(payload);
            break;
        }
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
    let payload = payload_opt.expect("p2p should attach to connect bus");

    let relay_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_strategy");
    let relay_effective_strategy = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_effective_strategy"))
        .and_then(norito::json::Value::as_str)
        .expect("connect status should include policy.relay_effective_strategy");
    let relay_p2p_attached = payload
        .get("policy")
        .and_then(|policy| policy.get("relay_p2p_attached"))
        .and_then(norito::json::Value::as_bool)
        .expect("connect status should include policy.relay_p2p_attached");
    let p2p_rebroadcasts_total = payload
        .get("p2p_rebroadcasts_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcasts_total");
    let p2p_rebroadcast_skipped_total = payload
        .get("p2p_rebroadcast_skipped_total")
        .and_then(norito::json::Value::as_u64)
        .expect("connect status should include p2p_rebroadcast_skipped_total");

    assert_eq!(relay_strategy, "local_only");
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
    assert_eq!(p2p_rebroadcasts_total, 0);
    assert_eq!(p2p_rebroadcast_skipped_total, 0);
}

#[tokio::test]
async fn connect_session_delete_endpoint_removes_tokens() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0x24);
    let create_resp = app
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body.clone()))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(create_resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = payload
        .get("sid")
        .and_then(|x| x.as_str())
        .expect("sid present")
        .to_owned();
    let token_management = payload
        .get("token_management")
        .and_then(|x| x.as_str())
        .expect("token_management present")
        .to_owned();

    let delete_uri = format!("/v1/connect/session/{sid}");
    let missing_token_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.as_str())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_token_resp.status(), StatusCode::UNAUTHORIZED);

    let delete_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.as_str())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    let delete_again = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.as_str())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_again.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn connect_session_status_requires_management_token() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0x34);
    let create_resp = app
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(create_resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = payload
        .get("sid")
        .and_then(|x| x.as_str())
        .expect("sid present");
    let token_management = payload
        .get("token_management")
        .and_then(|x| x.as_str())
        .expect("token_management present");

    let status_uri = format!("/v1/connect/status?sid={sid}");
    let missing_token_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(status_uri.as_str())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_token_resp.status(), StatusCode::UNAUTHORIZED);

    let status_resp = app
        .oneshot(
            Request::builder()
                .uri(status_uri.as_str())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(status_resp.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("session status should be JSON");
    assert_eq!(payload.get("sid").and_then(|x| x.as_str()), Some(sid));
    assert_eq!(
        payload.get("app_attached").and_then(|x| x.as_bool()),
        Some(false)
    );
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_session_delete_rejects_ws_attach() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_session_delete_rejects_ws_attach: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    // Use a second router handle for in-process REST calls.
    let app2 = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0x44);
    let create_resp = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(create_resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = payload
        .get("sid")
        .and_then(|x| x.as_str())
        .expect("sid present");
    assert_eq!(sid, sid_fixed);
    let token_app = payload
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");
    let token_management = payload
        .get("token_management")
        .and_then(|x| x.as_str())
        .expect("token_management");

    // Delete the session through REST and ensure it reports success.
    let delete_uri = format!("/v1/connect/session/{sid}");
    let delete_resp = app2
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(delete_uri.clone())
                .header(
                    axum::http::header::AUTHORIZATION,
                    format!("Bearer {token_management}"),
                )
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    // Attempt to attach over WS using the stale token; expect 401.
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("authorization header"),
    );
    match tokio_tungstenite::connect_async(request).await {
        Ok(_) => panic!("ws handshake should fail after session deletion"),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }
        Err(err) => panic!("unexpected ws failure: {err:?}"),
    }
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_handshake_succeeds_when_enabled() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    // Build enabled config and Torii router
    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();
    // Serve on an ephemeral port
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_handshake_succeeds_when_enabled: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    // Create a session via in-process router call to obtain tokens and sid
    let app2 = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0x52);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    // Attempt WS connect using the provided sid/token
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("authorization header"),
    );
    let (_ws, resp) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake ok");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_accepts_protocol_token() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::header};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_accepts_protocol_token: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0x62);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    let encoded = B64.encode(token_app.as_bytes());
    request.headers_mut().insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("iroha-connect.token.v1.{encoded}")
            .parse()
            .expect("protocol header"),
    );
    let (_ws, resp) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake ok");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_closes_on_role_direction_mismatch() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::{SinkExt, StreamExt};
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep, timeout};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_closes_on_role_direction_mismatch: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0x92);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Attach as app, then send a mismatched direction (WalletToApp).
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut request = url.into_client_request().expect("ws request");
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("authorization header"),
    );
    let (mut ws, resp) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake ok");
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let mismatch = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::WalletToApp,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
    };
    let payload = proto::encode_connect_frame_bare(&mismatch).expect("encode frame");
    ws.send(Message::Binary(payload.into()))
        .await
        .expect("send mismatch frame");

    let mut saw_connect_close = false;
    let mut saw_ws_close = false;
    for _ in 0..5 {
        let maybe_msg = timeout(Duration::from_millis(400), ws.next()).await;
        let Some(msg) = maybe_msg.unwrap_or(None) else {
            continue;
        };
        match msg {
            Ok(Message::Binary(bytes)) => {
                if let Ok(frame) = proto::decode_connect_frame_bare(&bytes) {
                    if let proto::FrameKind::Control(proto::ConnectControlV1::Close {
                        reason,
                        ..
                    }) = frame.kind
                    {
                        if reason == "connect_role_direction_mismatch" {
                            saw_connect_close = true;
                            break;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                saw_ws_close = true;
                break;
            }
            Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed) => {
                saw_ws_close = true;
                break;
            }
            _ => {}
        }
    }
    assert!(
        saw_connect_close || saw_ws_close,
        "expected websocket termination after role/direction mismatch"
    );

    // Poll status until mismatch closure is reflected.
    let mut mismatch_total = 0u64;
    let mut sessions_total = u64::MAX;
    for _ in 0..20 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&body).unwrap();
        mismatch_total = status_json
            .get("role_direction_mismatch_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        sessions_total = status_json
            .get("sessions_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(u64::MAX);
        if mismatch_total >= 1 && sessions_total == 0 {
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }
    assert!(mismatch_total >= 1, "mismatch counter should increment");
    assert_eq!(sessions_total, 0, "session should be terminated");
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_duplicate_frame_does_not_close_session() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::{SinkExt, StreamExt};
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_duplicate_frame_does_not_close_session: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();

    let (sid_fixed, req_body) = connect_session_request_body(0xA3);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");
    let token_wallet = v
        .get("token_wallet")
        .and_then(|x| x.as_str())
        .expect("token_wallet");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Connect app role.
    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    // Connect wallet role.
    let wallet_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=wallet");
    let mut wallet_req = wallet_url.into_client_request().expect("wallet ws request");
    wallet_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_wallet}")
            .parse()
            .expect("wallet authorization header"),
    );
    let (mut wallet_ws, wallet_resp) = tokio_tungstenite::connect_async(wallet_req)
        .await
        .expect("wallet ws handshake ok");
    assert_eq!(wallet_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 41 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    // Wallet should receive first frame.
    let first = timeout(Duration::from_millis(500), wallet_ws.next())
        .await
        .expect("wallet recv timeout")
        .expect("wallet recv closed")
        .expect("wallet recv error");
    let first_frame = match first {
        Message::Binary(bytes) => proto::decode_connect_frame_bare(&bytes).expect("decode first"),
        other => panic!("expected binary frame, got {other:?}"),
    };
    assert_eq!(first_frame.seq, 1);

    // Send duplicate seq=1; dedupe should drop it and keep session alive.
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode duplicate")
                .into(),
        ))
        .await
        .expect("send duplicate seq1");
    assert!(
        timeout(Duration::from_millis(200), wallet_ws.next())
            .await
            .is_err(),
        "duplicate frame should not be delivered to wallet"
    );
    assert!(
        timeout(Duration::from_millis(200), app_ws.next())
            .await
            .is_err(),
        "duplicate frame should not close app websocket"
    );

    let seq2 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 2,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 42 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq2)
                .expect("encode seq2")
                .into(),
        ))
        .await
        .expect("send seq2");
    let second = timeout(Duration::from_millis(500), wallet_ws.next())
        .await
        .expect("wallet recv seq2 timeout")
        .expect("wallet recv seq2 closed")
        .expect("wallet recv seq2 error");
    let second_frame = match second {
        Message::Binary(bytes) => proto::decode_connect_frame_bare(&bytes).expect("decode second"),
        other => panic!("expected binary frame, got {other:?}"),
    };
    assert_eq!(second_frame.seq, 2);

    let status = app2
        .oneshot(
            Request::builder()
                .uri(Uri::from_static("/v1/connect/status"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body = http_body_util::BodyExt::collect(status.into_body())
        .await
        .unwrap()
        .to_bytes();
    let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
    let dedupe_drops = status_json
        .get("dedupe_drops_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let sequence_violation_closes = status_json
        .get("sequence_violation_closes_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    assert!(dedupe_drops >= 1, "expected duplicate drop to be counted");
    assert_eq!(
        sequence_violation_closes, 0,
        "duplicate frame must not trigger sequence-violation close"
    );
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_broadcast_relay_updates_p2p_rebroadcast_counter() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "broadcast";
    cfg.torii.connect.p2p_ttl_hops = 1;
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_broadcast_relay_updates_p2p_rebroadcast_counter: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let (sid_fixed, req_body) = connect_session_request_body(0xB4);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Wait until async bus attachment reports active P2P relay wiring.
    let mut relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if relay_p2p_attached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(relay_p2p_attached, "connect relay should attach P2P bus");

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 7 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if rebroadcasts >= 1 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert!(rebroadcasts >= 1, "expected at least one p2p rebroadcast");
    assert_eq!(
        skipped, 0,
        "p2p attached relay should not count skipped sends"
    );
    assert_eq!(relay_effective_strategy, "broadcast");
    assert!(relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_broadcast_without_p2p_increments_skipped_rebroadcast_counter() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "broadcast";
    cfg.torii.connect.p2p_ttl_hops = 1;
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "skipping connect_ws_broadcast_without_p2p_increments_skipped_rebroadcast_counter: {err}"
            );
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let (sid_fixed, req_body) = connect_session_request_body(0xC5);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 8 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    let mut relay_p2p_attached = true;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(true);
        if skipped >= 1 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(rebroadcasts, 0);
    assert!(
        skipped >= 1,
        "expected missing-p2p rebroadcast skips to be counted"
    );
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(!relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_local_only_with_p2p_does_not_rebroadcast() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_strategy = "local_only";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_local_only_with_p2p_does_not_rebroadcast: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let (sid_fixed, req_body) = connect_session_request_body(0xD6);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Wait for async P2P bus attachment before sending frames.
    let mut relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if relay_p2p_attached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(relay_p2p_attached, "connect relay should attach P2P bus");

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 9 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if rebroadcasts > 0 || skipped > 0 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(rebroadcasts, 0);
    assert_eq!(skipped, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_relay_disabled_with_p2p_does_not_rebroadcast() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use futures::SinkExt;
    use iroha_torii_shared::connect as proto;
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let mut cfg = minimal_actual_config(true);
    cfg.torii.connect.relay_enabled = false;
    cfg.torii.connect.relay_strategy = "broadcast";
    let torii = build_torii(&cfg).with_p2p(iroha_core::IrohaNetwork::closed_for_tests());
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_relay_disabled_with_p2p_does_not_rebroadcast: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let app2 = torii.api_router_for_tests();
    let (sid_fixed, req_body) = connect_session_request_body(0xE7);
    let res = app2
        .clone()
        .oneshot(request_with_loopback_connect_info(
            Request::builder()
                .method("POST")
                .uri(Uri::from_static("/v1/connect/session"))
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(req_body))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let sid = v.get("sid").and_then(|x| x.as_str()).expect("sid");
    assert_eq!(sid, sid_fixed);
    let token_app = v
        .get("token_app")
        .and_then(|x| x.as_str())
        .expect("token_app");

    let mut sid_bytes = [0u8; 32];
    let sid_vec = B64.decode(sid).expect("decode sid");
    sid_bytes.copy_from_slice(&sid_vec);

    // Wait for async P2P bus attachment before sending frames.
    let mut relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if relay_p2p_attached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(relay_p2p_attached, "connect relay should attach P2P bus");

    let app_url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app");
    let mut app_req = app_url.into_client_request().expect("app ws request");
    app_req.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
        format!("Bearer {token_app}")
            .parse()
            .expect("app authorization header"),
    );
    let (mut app_ws, app_resp) = tokio_tungstenite::connect_async(app_req)
        .await
        .expect("app ws handshake ok");
    assert_eq!(app_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    let seq1 = proto::ConnectFrameV1 {
        sid: sid_bytes,
        dir: proto::Dir::AppToWallet,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 10 }),
    };
    app_ws
        .send(Message::Binary(
            proto::encode_connect_frame_bare(&seq1)
                .expect("encode seq1")
                .into(),
        ))
        .await
        .expect("send seq1");

    let mut rebroadcasts = 0u64;
    let mut skipped = 0u64;
    let mut relay_effective_strategy = String::new();
    relay_p2p_attached = false;
    for _ in 0..50 {
        let status = app2
            .clone()
            .oneshot(
                Request::builder()
                    .uri(Uri::from_static("/v1/connect/status"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = http_body_util::BodyExt::collect(status.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status_json: norito::json::Value = norito::json::from_slice(&status_body).unwrap();
        rebroadcasts = status_json
            .get("p2p_rebroadcasts_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        skipped = status_json
            .get("p2p_rebroadcast_skipped_total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        relay_effective_strategy = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_effective_strategy"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        relay_p2p_attached = status_json
            .get("policy")
            .and_then(|policy| policy.get("relay_p2p_attached"))
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if rebroadcasts > 0 || skipped > 0 {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(rebroadcasts, 0);
    assert_eq!(skipped, 0);
    assert_eq!(relay_effective_strategy, "local_only");
    assert!(relay_p2p_attached);
}

#[cfg(feature = "ws_integration_tests")]
#[tokio::test]
async fn connect_ws_rejects_query_token() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
    use tokio::net::TcpListener;

    let cfg = minimal_actual_config(true);
    let torii = build_torii(&cfg);
    let app = torii.api_router_for_tests();

    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping connect_ws_rejects_query_token: {err}");
            return;
        }
        Err(err) => panic!("failed to bind test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    spawn_test_server(listener, app);

    let sid = B64.encode([0x72u8; 32]);
    let url = format!("ws://{addr}/v1/connect/ws?sid={sid}&role=app&token=deadbeef");
    let err = tokio_tungstenite::connect_async(&url)
        .await
        .expect_err("ws handshake should reject query token");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

include!("connect_gating_disabled_ws_test.rs");

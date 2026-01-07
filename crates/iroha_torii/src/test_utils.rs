//! Test utilities for Torii integration tests.
//!
//! These helpers are intended for crate integration tests to avoid duplicating
//! queue-drain and state-apply boilerplate when exercising app API endpoints.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use iroha_config::parameters::{defaults, defaults::zk::fastpq};
use iroha_core::{
    block::{BlockBuilder, CommittedBlock},
    queue::{Queue, TransactionGuard},
    state::{State, StateBlock, StateReadOnly},
};
use iroha_crypto::Algorithm;
use iroha_data_model::{
    ChainId, account::AccountId, block::SignedBlock, content::ContentAuthMode,
    jurisdiction::JdgSignatureScheme, prelude::ExposedPrivateKey,
    sorafs::pricing::PricingScheduleRecord,
};
use nonzero_ext::nonzero;
/// Parameters for invoking a contract within Torii integration tests.
pub struct ContractCallOptions<'a> {
    /// Optional entry point function to call on the contract; defaults to main when `None`.
    pub entrypoint: Option<&'a str>,
    /// Serialized JSON payload passed to the contract call.
    pub payload: Option<&'a norito::json::Value>,
    /// Gas-paying asset identifier to attach to the request.
    pub gas_asset_id: Option<&'a str>,
    /// Upper bound on gas consumption for the call.
    pub gas_limit: Option<u64>,
}

/// Drain queued transactions and apply a single block at the given height.
///
/// Returns the number of applied transactions. Panics if any transaction fails
/// to apply (to keep tests concise and fail-fast).
pub fn apply_queued_in_one_block(
    state: &Arc<State>,
    queue: &Arc<Queue>,
    chain_id: &ChainId,
    expected_height: u64,
) -> usize {
    let max_txs_in_block = core::num::NonZeroUsize::new(1024).expect("nonzero");
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), max_txs_in_block, &mut guards);
    if guards.is_empty() {
        return 0;
    }

    let accepted: Vec<_> = guards
        .iter()
        .map(TransactionGuard::clone_accepted)
        .collect();
    let applied = accepted.len();

    let latest_block = state.view().latest_block();
    let leader = iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
    let new_block = BlockBuilder::new(accepted)
        .chain(0, latest_block.as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});

    debug_assert_eq!(
        new_block.header().height().get(),
        expected_height,
        "Unexpected block height when applying queued transactions",
    );

    let mut state_block = state.block(new_block.header());
    // Ensure stateless validation uses the expected chain id for these tests.
    state_block.chain_id = chain_id.clone();
    let valid_block = new_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});

    let committed_block = valid_block.commit_unchecked().unpack(|_| {});
    let block_ref = committed_block.as_ref();
    for (idx, tx) in block_ref.external_transactions().enumerate() {
        if let Some(error) = block_ref.error(idx) {
            panic!(
                "transaction at height {expected_height} with hash {} rejected: {error:?}",
                tx.hash()
            );
        }
    }
    finalize_committed_block(state, state_block, committed_block);

    applied
}

/// Repeatedly drain and apply queued transactions, incrementing block height
/// starting at `start_height`, until the queue is empty. Returns total applied.
pub fn drain_queue_and_apply_all(
    state: &Arc<State>,
    queue: &Arc<Queue>,
    chain_id: &ChainId,
    start_height: u64,
) -> usize {
    let mut total = 0usize;
    let mut h = start_height;
    loop {
        let n = apply_queued_in_one_block(state, queue, chain_id, h);
        if n == 0 {
            break;
        }
        total += n;
        h += 1;
    }
    total
}

/// Apply the bookkeeping for a fully committed block: update transaction heights,
/// advance block hashes, and persist the block into Kura.
pub fn finalize_committed_block(
    state: &Arc<State>,
    mut state_block: StateBlock<'_>,
    committed_block: CommittedBlock,
) {
    let _ = state_block.apply_without_execution(&committed_block, Vec::new());
    state_block.commit().unwrap();
    let signed_block: SignedBlock = committed_block.into();
    let _ = state.view().kura().store_block(Arc::new(signed_block));
}

/// Build a minimal IVM `.to` program containing a single HALT, with a V2 header.
pub fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000,
        abi_version,
    };
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

/// Compute the hex-encoded contract code hash for a `.to` program.
///
/// This matches Torii's deployment hashing (header + body).
pub fn body_code_hash_hex(code_bytes: &[u8]) -> String {
    let h = iroha_crypto::Hash::new(code_bytes);
    hex::encode(<[u8; 32]>::from(h))
}

/// Guard that overrides Torii's data directory with a temporary path for test isolation
/// and clears it on drop. The underlying directory is removed when the guard is dropped.
#[must_use]
pub struct TestDataDirGuard {
    temp_dir: tempfile::TempDir,
    override_guard: crate::data_dir::OverrideGuard,
}

impl TestDataDirGuard {
    /// Create a new temporary data directory and activate the override.
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("temp data dir");
        let override_guard = crate::data_dir::OverrideGuard::new(temp_dir.path());
        Self {
            temp_dir,
            override_guard,
        }
    }

    /// Access the filesystem path backing this guard.
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}

impl Default for TestDataDirGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Small container for random authority credentials used in tests.
pub struct AuthorityCreds {
    /// Generated account id for the authority
    pub account: AccountId,
    /// Private key corresponding to the generated account id
    pub private_key: ExposedPrivateKey,
}

/// Generate a random authority with matching private key (domain `wonderland`).
pub fn random_authority() -> AuthorityCreds {
    let kp = iroha_crypto::KeyPair::random();
    let account = AccountId::of(
        "wonderland".parse().expect("valid domain id"),
        kp.public_key().clone(),
    );
    AuthorityCreds {
        account,
        private_key: ExposedPrivateKey(kp.private_key().clone()),
    }
}

/// Build JSON string for deploy request body.
pub fn deploy_request_json(
    account: &AccountId,
    private_key: &ExposedPrivateKey,
    code_b64: &str,
) -> String {
    let value = crate::json_object(vec![
        crate::json_entry("authority", account.clone()),
        crate::json_entry("private_key", private_key.to_string()),
        crate::json_entry("code_b64", code_b64),
    ]);
    norito::json::to_json(&value).expect("serialize deploy request")
}

/// Build JSON string for activate-instance request body.
pub fn activate_instance_request_json(
    account: &AccountId,
    private_key: &ExposedPrivateKey,
    namespace: &str,
    contract_id: &str,
    code_hash_hex: &str,
) -> String {
    let value = crate::json_object(vec![
        crate::json_entry("authority", account.clone()),
        crate::json_entry("private_key", private_key.to_string()),
        crate::json_entry("namespace", namespace),
        crate::json_entry("contract_id", contract_id),
        crate::json_entry("code_hash", code_hash_hex),
    ]);
    norito::json::to_json(&value).expect("serialize activate request")
}

/// Build JSON string for contract call request body.
pub fn contract_call_request_json(
    account: &AccountId,
    private_key: &ExposedPrivateKey,
    namespace: &str,
    contract_id: &str,
    options: ContractCallOptions<'_>,
) -> String {
    let mut entries = vec![
        crate::json_entry("authority", account.clone()),
        crate::json_entry("private_key", private_key.to_string()),
        crate::json_entry("namespace", namespace),
        crate::json_entry("contract_id", contract_id),
    ];
    if let Some(ep) = options.entrypoint {
        entries.push(crate::json_entry("entrypoint", ep));
    }
    if let Some(value) = options.payload {
        entries.push(crate::json_entry("payload", value.clone()));
    }
    if let Some(asset) = options.gas_asset_id {
        entries.push(crate::json_entry("gas_asset_id", asset));
    }
    if let Some(limit) = options.gas_limit {
        entries.push(crate::json_entry("gas_limit", limit));
    }
    let value = crate::json_object(entries);
    norito::json::to_json(&value).expect("serialize contract call request")
}

/// Build JSON string for combined deploy+activate request body.
pub fn deploy_and_activate_request_json(
    account: &AccountId,
    private_key: &ExposedPrivateKey,
    namespace: &str,
    contract_id: &str,
    code_b64: &str,
) -> String {
    let value = crate::json_object(vec![
        crate::json_entry("authority", account.clone()),
        crate::json_entry("private_key", private_key.to_string()),
        crate::json_entry("namespace", namespace),
        crate::json_entry("contract_id", contract_id),
        crate::json_entry("code_b64", code_b64),
    ]);
    norito::json::to_json(&value).expect("serialize deploy+activate request")
}

/// Build a minimal actual configuration root suitable for constructing Kiso and Torii in tests.
///
/// The configuration mirrors the helpers used across integration tests and sets required
/// fields for `Genesis`, `Torii`, and `Sumeragi` to current structures.
pub fn mk_minimal_root_cfg() -> iroha_config::parameters::actual::Root {
    use iroha_config::{
        base::WithOrigin,
        parameters::{actual as A, defaults},
    };
    use iroha_crypto::{
        KeyPair,
        soranet::handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        },
        streaming::StreamingKeyMaterial,
    };
    use iroha_data_model::peer::Peer;
    use iroha_logger::Level;
    use iroha_primitives::addr::socket_addr;
    use nonzero_ext::nonzero;

    A::Root {
        common: A::Common {
            chain: ChainId::from("test-chain"),
            key_pair: KeyPair::random(),
            peer: Peer::new(
                socket_addr!(127.0.0.1:0),
                KeyPair::random().public_key().clone(),
            ),
            trusted_peers: WithOrigin::inline(A::TrustedPeers {
                myself: Peer::new(
                    socket_addr!(127.0.0.1:0),
                    KeyPair::random().public_key().clone(),
                ),
                others: iroha_primitives::unique_vec::UniqueVec::new(),
                pops: std::collections::BTreeMap::new(),
            }),
            default_account_domain_label: WithOrigin::inline(
                iroha_data_model::account::address::DEFAULT_DOMAIN_NAME_FALLBACK.to_owned(),
            ),
            chain_discriminant: WithOrigin::inline(defaults::common::chain_discriminant()),
        },
        network: A::Network {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            relay_mode: A::RelayMode::Disabled,
            relay_hub_address: None,
            relay_ttl: defaults::network::RELAY_TTL,
            soranet_handshake: A::SoranetHandshake {
                descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
                client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
                relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
                trust_gossip: defaults::network::TRUST_GOSSIP,
                kem_id: 1,
                sig_id: 1,
                resume_hash: None,
                pow: A::SoranetPow::default(),
            },
            soranet_privacy: A::SoranetPrivacy::default(),
            soranet_vpn: A::SoranetVpn::default(),
            lane_profile: A::LaneProfile::from_label(
                &defaults::network::lane_profile::default_label(),
            ),
            require_sm_handshake_match: true,
            require_sm_openssl_preview_match: true,
            idle_timeout: core::time::Duration::from_secs(5),
            peer_gossip_period: defaults::network::PEER_GOSSIP_PERIOD,
            trust_gossip: defaults::network::TRUST_GOSSIP,
            trust_decay_half_life: defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip: defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer: defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: defaults::network::TRUST_MIN_SCORE,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            tls_enabled: false,
            tls_listen_address: None,
            prefer_ws_fallback: false,
            p2p_queue_cap_high: nonzero!(128usize),
            p2p_queue_cap_low: nonzero!(512usize),
            p2p_post_queue_cap: nonzero!(128usize),
            happy_eyeballs_stagger: core::time::Duration::from_millis(100),
            addr_ipv6_first: false,
            max_incoming: None,
            max_total_connections: None,
            accept_rate_per_ip_per_sec: None,
            accept_burst_per_ip: None,
            max_accept_buckets: defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits: defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits: defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            public_key: KeyPair::random().public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: Vec::new(),
            bootstrap_max_bytes: defaults::genesis::BOOTSTRAP_MAX_BYTES.get(),
            bootstrap_response_throttle: defaults::genesis::BOOTSTRAP_RESPONSE_THROTTLE,
            bootstrap_request_timeout: defaults::genesis::BOOTSTRAP_REQUEST_TIMEOUT,
            bootstrap_retry_interval: defaults::genesis::BOOTSTRAP_RETRY_INTERVAL,
            bootstrap_max_attempts: defaults::genesis::BOOTSTRAP_MAX_ATTEMPTS,
            bootstrap_enabled: true,
        },
        torii: A::Torii {
            address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
            api_versions: defaults::torii::api_supported_versions(),
            api_version_default: defaults::torii::api_default_version(),
            api_min_proof_version: defaults::torii::api_min_proof_version(),
            api_version_sunset_unix: defaults::torii::API_SUNSET_UNIX,
            max_content_len: (1_048_576u64).into(),
            data_dir: defaults::torii::data_dir(),
            events_buffer_capacity: defaults::torii::events_buffer_capacity(),
            ws_message_timeout: Duration::from_millis(defaults::torii::WS_MESSAGE_TIMEOUT_MS),
            query_rate_per_authority_per_sec: None,
            query_burst_per_authority: None,
            deploy_rate_per_origin_per_sec: None,
            deploy_burst_per_origin: None,
            proof_api: iroha_config::parameters::actual::ProofApi {
                rate_per_minute: defaults::torii::PROOF_RATE_PER_MIN
                    .and_then(std::num::NonZeroU32::new),
                burst: defaults::torii::PROOF_BURST.and_then(std::num::NonZeroU32::new),
                max_body_bytes: defaults::torii::PROOF_MAX_BODY_BYTES,
                egress_bytes_per_sec: defaults::torii::PROOF_EGRESS_BYTES_PER_SEC
                    .and_then(std::num::NonZeroU64::new),
                egress_burst_bytes: defaults::torii::PROOF_EGRESS_BURST_BYTES
                    .and_then(std::num::NonZeroU64::new),
                max_list_limit: std::num::NonZeroU32::new(
                    defaults::torii::PROOF_MAX_LIST_LIMIT.max(1),
                )
                .expect("proof page limit must be non-zero"),
                request_timeout: std::time::Duration::from_millis(
                    defaults::torii::PROOF_REQUEST_TIMEOUT_MS,
                ),
                cache_max_age: std::time::Duration::from_secs(
                    defaults::torii::PROOF_CACHE_MAX_AGE_SECS,
                ),
                retry_after: std::time::Duration::from_secs(
                    defaults::torii::PROOF_RETRY_AFTER_SECS,
                ),
            },
            require_api_token: false,
            api_tokens: Vec::new(),
            api_fee_asset_id: None,
            api_fee_amount: None,
            api_fee_receiver: None,
            api_allow_cidrs: Vec::new(),
            peer_telemetry_urls: Vec::new(),
            peer_geo: A::ToriiPeerGeo::default(),
            soranet_privacy_ingest: A::SoranetPrivacyIngest::default(),
            strict_addresses: false,
            debug_match_filters: false,
            operator_auth: A::ToriiOperatorAuth::default(),
            preauth_max_connections: None,
            preauth_max_connections_per_ip: None,
            preauth_rate_per_ip_per_sec: None,
            preauth_burst_per_ip: None,
            preauth_temp_ban: None,
            preauth_allow_cidrs: Vec::new(),
            preauth_scheme_limits: Vec::new(),
            api_high_load_tx_threshold: None,
            api_high_load_stream_threshold: None,
            api_high_load_subscription_threshold: None,
            app_api: iroha_config::parameters::actual::AppApi {
                default_list_limit: std::num::NonZeroU32::new(
                    defaults::torii::APP_API_DEFAULT_LIST_LIMIT.max(1),
                )
                .expect("default list limit must be non-zero"),
                max_list_limit: std::num::NonZeroU32::new(
                    defaults::torii::APP_API_MAX_LIST_LIMIT.max(1),
                )
                .expect("max list limit must be non-zero"),
                max_fetch_size: std::num::NonZeroU32::new(
                    defaults::torii::APP_API_MAX_FETCH_SIZE.max(1),
                )
                .expect("max fetch size must be non-zero"),
                rate_limit_cost_per_row: std::num::NonZeroU32::new(
                    defaults::torii::APP_API_RATE_LIMIT_COST_PER_ROW.max(1),
                )
                .expect("rate limit cost must be non-zero"),
            },
            attachments_ttl_secs: 7 * 24 * 60 * 60,
            attachments_max_bytes: 4 * 1024 * 1024,
            attachments_per_tenant_max_count: defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT,
            attachments_per_tenant_max_bytes: defaults::torii::ATTACHMENTS_PER_TENANT_MAX_BYTES,
            attachments_allowed_mime_types: defaults::torii::attachments_allowed_mime_types(),
            attachments_max_expanded_bytes: defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES,
            attachments_max_archive_depth: defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH,
            attachments_sanitizer_mode:
                iroha_config::parameters::actual::AttachmentSanitizerMode::Subprocess,
            attachments_sanitize_timeout_ms: defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS,
            webhook: iroha_config::parameters::actual::Webhook {
                queue_capacity: defaults::torii::webhook_queue_capacity(),
                max_attempts: defaults::torii::webhook_max_attempts(),
                backoff_initial: std::time::Duration::from_millis(
                    defaults::torii::WEBHOOK_BACKOFF_INITIAL_MS,
                ),
                backoff_max: std::time::Duration::from_millis(
                    defaults::torii::WEBHOOK_BACKOFF_MAX_MS,
                ),
                connect_timeout: std::time::Duration::from_millis(
                    defaults::torii::WEBHOOK_CONNECT_TIMEOUT_MS,
                ),
                write_timeout: std::time::Duration::from_millis(
                    defaults::torii::WEBHOOK_WRITE_TIMEOUT_MS,
                ),
                read_timeout: std::time::Duration::from_millis(
                    defaults::torii::WEBHOOK_READ_TIMEOUT_MS,
                ),
            },
            push: iroha_config::parameters::actual::Push::default(),
            zk_prover_enabled: false,
            zk_prover_scan_period_secs: 30,
            zk_prover_reports_ttl_secs: 7 * 24 * 60 * 60,
            zk_prover_max_inflight: defaults::torii::ZK_PROVER_MAX_INFLIGHT,
            zk_prover_max_scan_bytes: defaults::torii::ZK_PROVER_MAX_SCAN_BYTES,
            zk_prover_max_scan_millis: defaults::torii::ZK_PROVER_MAX_SCAN_MILLIS,
            zk_prover_keys_dir: defaults::torii::zk_prover_keys_dir(),
            zk_prover_allowed_backends: defaults::torii::zk_prover_allowed_backends(),
            zk_prover_allowed_circuits: defaults::torii::zk_prover_allowed_circuits(),
            rbc_sampling: Default::default(),
            da_ingest: A::DaIngest::default(),
            connect: A::Connect {
                enabled: false,
                ws_max_sessions: defaults::connect::WS_MAX_SESSIONS,
                ws_per_ip_max_sessions: defaults::connect::WS_PER_IP_MAX_SESSIONS,
                ws_rate_per_ip_per_min: defaults::connect::WS_RATE_PER_IP_PER_MIN,
                session_ttl: defaults::connect::SESSION_TTL,
                frame_max_bytes: defaults::connect::FRAME_MAX_BYTES,
                session_buffer_max_bytes: defaults::connect::SESSION_BUFFER_MAX_BYTES,
                ping_interval: defaults::connect::PING_INTERVAL,
                ping_miss_tolerance: defaults::connect::PING_MISS_TOLERANCE,
                ping_min_interval: defaults::connect::PING_MIN_INTERVAL,
                dedupe_ttl: defaults::connect::DEDUPE_TTL,
                dedupe_cap: defaults::connect::DEDUPE_CAP,
                relay_enabled: defaults::connect::RELAY_ENABLED,
                relay_strategy: defaults::connect::RELAY_STRATEGY,
                p2p_ttl_hops: defaults::connect::P2P_TTL_HOPS,
            },
            iso_bridge: A::IsoBridge {
                enabled: false,
                dedupe_ttl_secs: defaults::torii::ISO_BRIDGE_DEDUPE_TTL_SECS,
                signer: None,
                account_aliases: Vec::new(),
                currency_assets: Vec::new(),
                reference_data: Default::default(),
            },
            sorafs_discovery: Default::default(),
            sorafs_storage: iroha_config::parameters::actual::SorafsStorage::default(),
            sorafs_quota: Default::default(),
            sorafs_alias_cache: Default::default(),
            sorafs_gateway: A::SorafsGateway {
                enforce_capabilities: false,
                enforce_admission: false,
                require_manifest_envelope: false,
                ..Default::default()
            },
            sorafs_por: Default::default(),
            transport: Default::default(),
            onboarding: None,
            offline_issuer: None,
        },
        kura: A::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: WithOrigin::inline(std::env::temp_dir()),
            max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: nonzero!(10usize),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: defaults::kura::FSYNC_MODE,
            fsync_interval: defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        },
        sumeragi: A::Sumeragi {
            debug_force_soft_fork: false,
            debug_disable_background_worker: false,
            debug_rbc_drop_every_nth_chunk: None,
            debug_rbc_shuffle_chunks: false,
            debug_rbc_duplicate_inits: false,
            debug_rbc_force_deliver_quorum_one: false,
            debug_rbc_corrupt_witness_ack: false,
            debug_rbc_corrupt_ready_signature: false,
            debug_rbc_drop_validator_mask: 0,
            debug_rbc_equivocate_chunk_mask: 0,
            debug_rbc_equivocate_validator_mask: 0,
            debug_rbc_conflicting_ready_mask: 0,
            debug_rbc_partial_chunk_mask: 0,
            kura_store_retry_interval: std::time::Duration::from_millis(
                defaults::sumeragi::KURA_STORE_RETRY_INTERVAL_MS,
            ),
            kura_store_retry_max_attempts: defaults::sumeragi::KURA_STORE_RETRY_MAX_ATTEMPTS,
            commit_inflight_timeout: std::time::Duration::from_millis(
                defaults::sumeragi::COMMIT_INFLIGHT_TIMEOUT_MS,
            ),
            missing_block_signer_fallback_attempts:
                defaults::sumeragi::MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS,
            membership_mismatch_alert_threshold:
                defaults::sumeragi::MEMBERSHIP_MISMATCH_ALERT_THRESHOLD,
            membership_mismatch_fail_closed: defaults::sumeragi::MEMBERSHIP_MISMATCH_FAIL_CLOSED,
            role: A::NodeRole::Validator,
            allow_view0_slack: false,
            collectors_k: 1,
            collectors_redundant_send_r: 1,
            rbc_pending_max_chunks: defaults::sumeragi::RBC_PENDING_MAX_CHUNKS,
            rbc_pending_max_bytes: defaults::sumeragi::RBC_PENDING_MAX_BYTES,
            rbc_pending_ttl: Duration::from_millis(defaults::sumeragi::RBC_PENDING_TTL_MS),
            block_max_transactions: defaults::sumeragi::BLOCK_MAX_TRANSACTIONS,
            block_max_payload_bytes: defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES,
            msg_channel_cap_votes: defaults::sumeragi::MSG_CHANNEL_CAP_VOTES,
            msg_channel_cap_block_payload: defaults::sumeragi::MSG_CHANNEL_CAP_BLOCK_PAYLOAD,
            msg_channel_cap_rbc_chunks: defaults::sumeragi::MSG_CHANNEL_CAP_RBC_CHUNKS,
            msg_channel_cap_blocks: defaults::sumeragi::MSG_CHANNEL_CAP_BLOCKS,
            control_msg_channel_cap: defaults::sumeragi::CONTROL_MSG_CHANNEL_CAP,
            consensus_mode: A::ConsensusMode::Permissioned,
            mode_flip_enabled: defaults::sumeragi::MODE_FLIP_ENABLED,
            commit_cert_history_cap: defaults::sumeragi::COMMIT_CERT_HISTORY_CAP,
            da_enabled: false,
            da_quorum_timeout_multiplier: defaults::sumeragi::DA_QUORUM_TIMEOUT_MULTIPLIER,
            da_availability_timeout_multiplier:
                defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_MULTIPLIER,
            da_availability_timeout_floor: Duration::from_millis(
                defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_FLOOR_MS,
            ),
            da_max_commitments_per_block: defaults::sumeragi::DA_MAX_COMMITMENTS_PER_BLOCK,
            da_max_proof_openings_per_block: defaults::sumeragi::DA_MAX_PROOF_OPENINGS_PER_BLOCK,
            proof_policy: A::ProofPolicy::Off,
            zk_finality_k: 0,
            require_precommit_qc: false,
            rbc_chunk_max_bytes: 32 * 1024,
            rbc_session_ttl: core::time::Duration::from_secs(10),
            rbc_store_max_sessions: defaults::sumeragi::RBC_STORE_MAX_SESSIONS,
            rbc_store_soft_sessions: defaults::sumeragi::RBC_STORE_SOFT_SESSIONS,
            rbc_store_max_bytes: defaults::sumeragi::RBC_STORE_MAX_BYTES,
            rbc_store_soft_bytes: defaults::sumeragi::RBC_STORE_SOFT_BYTES,
            rbc_disk_store_ttl: core::time::Duration::from_secs(
                defaults::sumeragi::RBC_DISK_STORE_TTL_SECS,
            ),
            rbc_disk_store_max_bytes: defaults::sumeragi::RBC_DISK_STORE_MAX_BYTES,
            key_activation_lead_blocks: defaults::sumeragi::KEY_ACTIVATION_LEAD_BLOCKS,
            key_overlap_grace_blocks: defaults::sumeragi::KEY_OVERLAP_GRACE_BLOCKS,
            key_expiry_grace_blocks: defaults::sumeragi::KEY_EXPIRY_GRACE_BLOCKS,
            key_require_hsm: defaults::sumeragi::KEY_REQUIRE_HSM,
            key_allowed_algorithms: defaults::sumeragi::key_allowed_algorithms()
                .into_iter()
                .collect::<BTreeSet<Algorithm>>(),
            key_allowed_hsm_providers: defaults::sumeragi::key_allowed_hsm_providers()
                .into_iter()
                .collect(),
            npos: A::SumeragiNpos::default(),
            use_stake_snapshot_roster: false,
            epoch_length_blocks: 0,
            vrf_commit_deadline_offset: 0,
            vrf_reveal_deadline_offset: 0,
            pacemaker_backoff_multiplier: 1,
            pacemaker_rtt_floor_multiplier: 1,
            pacemaker_max_backoff: core::time::Duration::from_secs(1),
            pacemaker_jitter_frac_permille: 0,
            adaptive_observability: A::AdaptiveObservability::default(),
            enable_bls: false,
        },
        block_sync: A::BlockSync {
            gossip_period: core::time::Duration::from_millis(200),
            gossip_size: nonzero!(32u32),
        },
        transaction_gossiper: A::TransactionGossiper {
            gossip_period: core::time::Duration::from_millis(200),
            gossip_size: nonzero!(32u32),
            dataspace: Default::default(),
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
            merkle_chunk_size_bytes: defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES,
            verification_public_key: None,
            signing_private_key: None,
        },
        telemetry_enabled: false,
        telemetry_profile: A::TelemetryProfile::Disabled,
        telemetry: None,
        telemetry_redaction: A::TelemetryRedaction::default(),
        telemetry_integrity: A::TelemetryIntegrity::default(),
        dev_telemetry: iroha_config::parameters::user::DevTelemetry {
            out_file: None,
            panic_on_duplicate_metrics: defaults::telemetry::PANIC_ON_DUPLICATE_METRICS,
        },
        pipeline: A::Pipeline {
            dynamic_prepass: false,
            access_set_cache_enabled: defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
            parallel_overlay: false,
            workers: defaults::pipeline::WORKERS,
            parallel_apply: true,
            ready_queue_heap: defaults::pipeline::READY_QUEUE_HEAP,
            gpu_key_bucket: defaults::pipeline::GPU_KEY_BUCKET,
            debug_trace_scheduler_inputs: defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
            debug_trace_tx_eval: defaults::pipeline::DEBUG_TRACE_TX_EVAL,
            signature_batch_max: defaults::pipeline::SIGNATURE_BATCH_MAX,
            signature_batch_max_ed25519: defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
            signature_batch_max_secp256k1: defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
            signature_batch_max_pqc: defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
            signature_batch_max_bls: defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
            cache_size: defaults::pipeline::CACHE_SIZE,
            ivm_cache_max_decoded_ops: defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
            ivm_cache_max_bytes: defaults::pipeline::IVM_CACHE_MAX_BYTES,
            ivm_prover_threads: defaults::pipeline::IVM_PROVER_THREADS,
            overlay_max_instructions: defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
            overlay_max_bytes: defaults::pipeline::OVERLAY_MAX_BYTES,
            overlay_chunk_instructions: defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
            gas: A::Gas {
                tech_account_id: defaults::pipeline::GAS_TECH_ACCOUNT_ID.to_string(),
                accepted_assets: Vec::new(),
                units_per_gas: Vec::new(),
            },
            ivm_max_cycles_upper_bound: defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
            ivm_max_decoded_instructions: defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
            ivm_max_decoded_bytes: defaults::pipeline::IVM_MAX_DECODED_BYTES,
            quarantine_max_txs_per_block: defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
            quarantine_tx_max_cycles: defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
            quarantine_tx_max_millis: defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
            query_default_cursor_mode: A::QueryCursorMode::Ephemeral,
            query_stored_min_gas_units: 0,
            amx_per_dataspace_budget_ms: defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
            amx_group_budget_ms: defaults::pipeline::AMX_GROUP_BUDGET_MS,
            amx_per_instruction_ns: defaults::pipeline::AMX_PER_INSTRUCTION_NS,
            amx_per_memory_access_ns: defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
            amx_per_syscall_ns: defaults::pipeline::AMX_PER_SYSCALL_NS,
        },
        tiered_state: A::TieredState {
            enabled: defaults::tiered_state::ENABLED,
            hot_retained_keys: defaults::tiered_state::HOT_RETAINED_KEYS,
            hot_retained_bytes: defaults::tiered_state::HOT_RETAINED_BYTES,
            hot_retained_grace_snapshots: defaults::tiered_state::HOT_RETAINED_GRACE_SNAPSHOTS,
            cold_store_root: None,
            max_snapshots: defaults::tiered_state::MAX_SNAPSHOTS,
            max_cold_bytes: defaults::tiered_state::MAX_COLD_BYTES,
        },
        compute: A::Compute {
            enabled: defaults::compute::ENABLED,
            namespaces: defaults::compute::default_namespaces()
                .into_iter()
                .collect(),
            default_ttl_slots: defaults::compute::default_ttl_slots(),
            max_ttl_slots: defaults::compute::max_ttl_slots(),
            max_request_bytes: defaults::compute::MAX_REQUEST_BYTES,
            max_response_bytes: defaults::compute::MAX_RESPONSE_BYTES,
            max_gas_per_call: defaults::compute::max_gas_per_call(),
            resource_profiles: defaults::compute::resource_profiles(),
            default_resource_profile: defaults::compute::default_resource_profile(),
            price_families: defaults::compute::price_families(),
            default_price_family: defaults::compute::default_price_family(),
            auth_policy: defaults::compute::default_auth_policy(),
            sandbox: defaults::compute::sandbox_rules(),
            economics: A::ComputeEconomics {
                max_cu_per_call: defaults::compute::max_cu_per_call(),
                max_amplification_ratio: defaults::compute::max_amplification_ratio(),
                fee_split: defaults::compute::fee_split(),
                sponsor_policy: defaults::compute::sponsor_policy(),
                price_bounds: defaults::compute::price_bounds(),
                price_risk_classes: defaults::compute::price_risk_classes(),
                price_family_baseline: defaults::compute::price_families(),
                price_amplifiers: defaults::compute::price_amplifiers(),
            },
            slo: A::ComputeSlo {
                max_inflight_per_route: defaults::compute::max_inflight_per_route(),
                queue_depth_per_route: defaults::compute::queue_depth_per_route(),
                max_requests_per_second: defaults::compute::max_requests_per_second(),
                target_p50_latency_ms: defaults::compute::target_p50_latency_ms(),
                target_p95_latency_ms: defaults::compute::target_p95_latency_ms(),
                target_p99_latency_ms: defaults::compute::target_p99_latency_ms(),
            },
        },
        content: A::Content {
            max_bundle_bytes: defaults::content::MAX_BUNDLE_BYTES,
            max_files: defaults::content::MAX_FILES,
            max_path_len: defaults::content::MAX_PATH_LEN,
            max_retention_blocks: defaults::content::MAX_RETENTION_BLOCKS,
            chunk_size_bytes: defaults::content::CHUNK_SIZE_BYTES,
            publish_allow_accounts: Vec::new(),
            limits: A::ContentLimits {
                max_requests_per_second: nonzero!(defaults::content::MAX_REQUESTS_PER_SECOND),
                request_burst: nonzero!(defaults::content::REQUEST_BURST),
                max_egress_bytes_per_second: NonZeroU64::new(
                    defaults::content::MAX_EGRESS_BYTES_PER_SECOND as u64,
                )
                .expect("default egress limit"),
                egress_burst_bytes: NonZeroU64::new(defaults::content::EGRESS_BURST_BYTES)
                    .expect("default egress burst"),
            },
            default_cache_max_age_secs: defaults::content::DEFAULT_CACHE_MAX_AGE_SECS,
            max_cache_max_age_secs: defaults::content::MAX_CACHE_MAX_AGE_SECS,
            immutable_bundles: defaults::content::IMMUTABLE_BUNDLES,
            default_auth_mode: ContentAuthMode::Public,
            slo: A::ContentSlo {
                target_p50_latency_ms: nonzero!(defaults::content::TARGET_P50_LATENCY_MS),
                target_p99_latency_ms: nonzero!(defaults::content::TARGET_P99_LATENCY_MS),
                target_availability_bps: nonzero!(defaults::content::TARGET_AVAILABILITY_BPS),
            },
            pow: A::ContentPow {
                difficulty_bits: defaults::content::POW_DIFFICULTY_BITS,
                header_name: defaults::content::default_pow_header(),
            },
            stripe_layout: defaults::content::default_stripe_layout(),
        },
        oracle: A::Oracle {
            history_depth: defaults::oracle::history_depth(),
            economics: A::OracleEconomics {
                reward_asset: defaults::oracle::reward_asset(),
                reward_pool: defaults::oracle::reward_pool(),
                reward_amount: defaults::oracle::reward_amount(),
                slash_asset: defaults::oracle::slash_asset(),
                slash_receiver: defaults::oracle::slash_receiver(),
                slash_outlier_amount: defaults::oracle::slash_outlier_amount(),
                slash_error_amount: defaults::oracle::slash_error_amount(),
                slash_no_show_amount: defaults::oracle::slash_no_show_amount(),
                dispute_bond_asset: defaults::oracle::dispute_bond_asset(),
                dispute_bond_amount: defaults::oracle::dispute_bond_amount(),
                dispute_reward_amount: defaults::oracle::dispute_reward_amount(),
                frivolous_slash_amount: defaults::oracle::frivolous_slash_amount(),
            },
            governance: A::OracleGovernance {
                intake_sla_blocks: defaults::oracle::intake_sla_blocks(),
                rules_sla_blocks: defaults::oracle::rules_sla_blocks(),
                cop_sla_blocks: defaults::oracle::cop_sla_blocks(),
                technical_sla_blocks: defaults::oracle::technical_sla_blocks(),
                policy_jury_sla_blocks: defaults::oracle::policy_jury_sla_blocks(),
                enact_sla_blocks: defaults::oracle::enact_sla_blocks(),
                intake_min_votes: defaults::oracle::intake_min_votes(),
                rules_min_votes: defaults::oracle::rules_min_votes(),
                cop_min_votes: A::OracleChangeThresholds {
                    low: defaults::oracle::cop_low_votes(),
                    medium: defaults::oracle::cop_medium_votes(),
                    high: defaults::oracle::cop_high_votes(),
                },
                technical_min_votes: defaults::oracle::technical_min_votes(),
                policy_jury_min_votes: A::OracleChangeThresholds {
                    low: defaults::oracle::policy_jury_low_votes(),
                    medium: defaults::oracle::policy_jury_medium_votes(),
                    high: defaults::oracle::policy_jury_high_votes(),
                },
            },
            twitter_binding: A::OracleTwitterBinding {
                feed_id: defaults::oracle::twitter_binding_feed_id(),
                pepper_id: defaults::oracle::twitter_binding_pepper_id(),
                max_ttl_ms: defaults::oracle::twitter_binding_max_ttl_ms(),
                min_ttl_ms: defaults::oracle::twitter_binding_min_ttl_ms(),
                min_update_spacing_ms: defaults::oracle::twitter_binding_min_update_spacing_ms(),
            },
        },
        hijiri: A::Hijiri::new(None),
        fraud_monitoring: iroha_config::parameters::actual::FraudMonitoring::new(
            defaults::fraud_monitoring::ENABLED,
            Vec::new(),
            defaults::fraud_monitoring::CONNECT_TIMEOUT,
            defaults::fraud_monitoring::REQUEST_TIMEOUT,
            defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS,
            None,
            Vec::new(),
        ),
        zk: A::Zk {
            halo2: A::Halo2 {
                enabled: false,
                curve: A::ZkCurve::Pallas,
                backend: A::Halo2Backend::Ipa,
                max_k: 16,
                verifier_budget_ms: 1000,
                verifier_max_batch: 8,
                ..A::Halo2::default()
            },
            fastpq: A::Fastpq {
                execution_mode: A::FastpqExecutionMode::Auto,
                device_class: None,
                chip_family: None,
                gpu_kind: None,
                metal_queue_fanout: None,
                metal_queue_column_threshold: None,
                poseidon_mode: A::FastpqPoseidonMode::Auto,
                metal_max_in_flight: fastpq::METAL_MAX_IN_FLIGHT,
                metal_threadgroup_width: fastpq::METAL_THREADGROUP_WIDTH,
                metal_trace: fastpq::METAL_TRACE,
                metal_debug_enum: fastpq::METAL_DEBUG_ENUM,
                metal_debug_fused: fastpq::METAL_DEBUG_FUSED,
            },
            root_history_cap: defaults::zk::ledger::ROOT_HISTORY_CAP,
            ballot_history_cap: defaults::zk::vote::BALLOT_HISTORY_CAP,
            empty_root_on_empty: defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY,
            merkle_depth: defaults::zk::ledger::EMPTY_ROOT_DEPTH,
            preverify_max_bytes: defaults::zk::preverify::MAX_BYTES,
            preverify_budget_bytes: defaults::zk::preverify::BUDGET_BYTES,
            proof_history_cap: defaults::zk::proof::RECORD_HISTORY_CAP,
            proof_retention_grace_blocks: defaults::zk::proof::RETENTION_GRACE_BLOCKS,
            proof_prune_batch: defaults::zk::proof::PRUNE_BATCH_SIZE,
            bridge_proof_max_range_len: defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
            bridge_proof_max_past_age_blocks: defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
            bridge_proof_max_future_drift_blocks:
                defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
            poseidon_params_id: defaults::confidential::POSEIDON_PARAMS_ID,
            pedersen_params_id: defaults::confidential::PEDERSEN_PARAMS_ID,
            kaigi_roster_join_vk: None,
            kaigi_roster_leave_vk: None,
            kaigi_usage_vk: None,
            max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
            max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
            max_commitments_per_tx: defaults::confidential::MAX_COMMITMENTS_PER_TX,
            max_confidential_ops_per_block: defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
            verify_timeout: defaults::confidential::VERIFY_TIMEOUT,
            max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
            max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
            max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
            max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
            max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
            reorg_depth_bound: defaults::confidential::REORG_DEPTH_BOUND,
            policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
            policy_transition_window_blocks:
                defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
            tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            tree_frontier_checkpoint_interval:
                defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
            registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
            registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
            registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
            gas: A::ConfidentialGas {
                proof_base: defaults::confidential::gas::PROOF_BASE,
                per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
                per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
                per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
                per_commitment: defaults::confidential::gas::PER_COMMITMENT,
            },
        },
        gov: A::Governance {
            vk_ballot: None,
            vk_tally: None,
            voting_asset_id: defaults::governance::VOTING_ASSET_ID
                .parse()
                .expect("valid default governance asset id"),
            citizenship_asset_id: defaults::governance::CITIZENSHIP_ASSET_ID
                .parse()
                .expect("valid default citizenship asset id"),
            citizenship_bond_amount: defaults::governance::CITIZENSHIP_BOND_AMOUNT,
            citizenship_escrow_account: defaults::governance::CITIZENSHIP_ESCROW_ACCOUNT
                .parse()
                .expect("valid default citizenship escrow account"),
            min_bond_amount: 150,
            bond_escrow_account: defaults::governance::BOND_ESCROW_ACCOUNT
                .parse()
                .expect("valid default governance bond escrow account"),
            slash_receiver_account: defaults::governance::SLASH_RECEIVER_ACCOUNT
                .parse()
                .expect("valid default governance slash receiver account"),
            slash_double_vote_bps: 0,
            slash_invalid_proof_bps: 0,
            slash_ineligible_proof_bps: 0,
            parliament_quorum_bps: defaults::governance::PARLIAMENT_QUORUM_BPS,
            citizen_service: A::CitizenServiceDiscipline {
                seat_cooldown_blocks: defaults::governance::citizen_service::SEAT_COOLDOWN_BLOCKS,
                max_seats_per_epoch: defaults::governance::citizen_service::MAX_SEATS_PER_EPOCH,
                free_declines_per_epoch:
                    defaults::governance::citizen_service::FREE_DECLINES_PER_EPOCH,
                decline_slash_bps: defaults::governance::citizen_service::DECLINE_SLASH_BPS,
                no_show_slash_bps: defaults::governance::citizen_service::NO_SHOW_SLASH_BPS,
                misconduct_slash_bps: defaults::governance::citizen_service::MISCONDUCT_SLASH_BPS,
                role_bond_multipliers: defaults::governance::citizen_service::role_bond_multipliers(
                ),
            },
            sorafs_pin_policy: A::SorafsPinPolicyConstraints::default(),
            alias_teu_minimum: defaults::governance::alias_teu_minimum(),
            alias_frontier_telemetry: defaults::governance::alias_frontier_telemetry(),
            debug_trace_pipeline: defaults::governance::DEBUG_TRACE_PIPELINE,
            jdg_signature_schemes: defaults::governance::jdg_signature_schemes()
                .into_iter()
                .map(|scheme| {
                    scheme
                        .parse::<JdgSignatureScheme>()
                        .expect("valid default JDG signature scheme")
                })
                .collect(),
            runtime_upgrade_provenance: A::RuntimeUpgradeProvenancePolicy::default(),
            sorafs_pricing: PricingScheduleRecord::launch_default(),
            sorafs_penalty: A::SorafsPenaltyPolicy::default(),
            sorafs_telemetry: A::SorafsTelemetryPolicy::default(),
            sorafs_provider_owners: BTreeMap::new(),
            conviction_step_blocks: 10,
            max_conviction: 4,
            min_enactment_delay: 0,
            window_span: 10,
            plain_voting_enabled: false,
            approval_threshold_q_num: 1,
            approval_threshold_q_den: 2,
            min_turnout: 0,
            parliament_committee_size: defaults::governance::PARLIAMENT_COMMITTEE_SIZE,
            parliament_term_blocks: defaults::governance::PARLIAMENT_TERM_BLOCKS,
            parliament_min_stake: defaults::governance::PARLIAMENT_MIN_STAKE,
            parliament_eligibility_asset_id: defaults::governance::PARLIAMENT_ELIGIBILITY_ASSET_ID
                .parse()
                .expect("valid default governance asset id"),
            parliament_alternate_size: defaults::governance::PARLIAMENT_ALTERNATE_SIZE,
            rules_committee_size: defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE,
            agenda_council_size: defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE,
            interest_panel_size: defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE,
            review_panel_size: defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE,
            policy_jury_size: defaults::governance::PARLIAMENT_POLICY_JURY_SIZE,
            oversight_committee_size: defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE,
            fma_committee_size: defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE,
            viral_incentives: A::ViralIncentives::default(),
            pipeline_study_sla_blocks: 0,
            pipeline_review_sla_blocks: 10,
            pipeline_decision_sla_blocks: 1,
            pipeline_enactment_sla_blocks: 20,
            pipeline_rules_sla_blocks: defaults::governance::PIPELINE_RULES_SLA_BLOCKS,
            pipeline_agenda_sla_blocks: defaults::governance::PIPELINE_AGENDA_SLA_BLOCKS,
        },
        norito: A::Norito {
            min_compress_bytes_cpu: defaults::norito::MIN_COMPRESS_BYTES_CPU,
            min_compress_bytes_gpu: defaults::norito::MIN_COMPRESS_BYTES_GPU,
            zstd_level_small: defaults::norito::ZSTD_LEVEL_SMALL,
            zstd_level_large: defaults::norito::ZSTD_LEVEL_LARGE,
            zstd_level_gpu: defaults::norito::ZSTD_LEVEL_GPU,
            large_threshold: defaults::norito::LARGE_THRESHOLD,
            enable_compact_seq_len_up_to: defaults::norito::ENABLE_COMPACT_SEQ_LEN_UP_TO,
            enable_varint_offsets_up_to: defaults::norito::ENABLE_VARINT_OFFSETS_UP_TO,
            allow_gpu_compression: defaults::norito::ALLOW_GPU_COMPRESSION,
            max_archive_len: defaults::norito::MAX_ARCHIVE_LEN,
            aos_ncb_small_n: defaults::norito::AOS_NCB_SMALL_N,
        },
        nts: A::Nts {
            sample_interval: defaults::time::NTS_SAMPLE_INTERVAL,
            sample_cap_per_round: defaults::time::NTS_SAMPLE_CAP_PER_ROUND,
            max_rtt_ms: defaults::time::NTS_MAX_RTT_MS,
            trim_percent: defaults::time::NTS_TRIM_PERCENT,
            per_peer_buffer: defaults::time::NTS_PER_PEER_BUFFER,
            smoothing_enabled: defaults::time::NTS_SMOOTHING_ENABLED,
            smoothing_alpha: defaults::time::NTS_SMOOTHING_ALPHA,
            max_adjust_ms_per_min: defaults::time::NTS_MAX_ADJUST_MS_PER_MIN,
            min_samples: defaults::time::NTS_MIN_SAMPLES,
            max_offset_ms: defaults::time::NTS_MAX_OFFSET_MS,
            max_confidence_ms: defaults::time::NTS_MAX_CONFIDENCE_MS,
            enforcement_mode: A::NtsEnforcementMode::Warn,
        },
        accel: A::Acceleration {
            enable_simd: false,
            enable_cuda: false,
            enable_metal: false,
            max_gpus: None,
            merkle_min_leaves_gpu: 1_000_000,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        },
        ivm: A::Ivm::default(),
        concurrency: A::Concurrency {
            scheduler_min_threads: 0,
            scheduler_max_threads: 0,
            rayon_global_threads: 0,
            scheduler_stack_bytes: defaults::concurrency::SCHEDULER_STACK_BYTES,
            prover_stack_bytes: defaults::concurrency::PROVER_STACK_BYTES,
            guest_stack_bytes: defaults::concurrency::GUEST_STACK_BYTES,
            gas_to_stack_multiplier: defaults::concurrency::GAS_TO_STACK_MULTIPLIER,
        },
        confidential: A::Confidential {
            enabled: defaults::confidential::ENABLED,
            assume_valid: defaults::confidential::ASSUME_VALID,
            verifier_backend: defaults::confidential::VERIFIER_BACKEND.to_string(),
            max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
            max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
            max_commitments_per_tx: defaults::confidential::MAX_COMMITMENTS_PER_TX,
            max_confidential_ops_per_block: defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
            verify_timeout: defaults::confidential::VERIFY_TIMEOUT,
            max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
            max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
            max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
            max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
            max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
            reorg_depth_bound: defaults::confidential::REORG_DEPTH_BOUND,
            policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
            policy_transition_window_blocks:
                defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
            tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            tree_frontier_checkpoint_interval:
                defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
            registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
            registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
            registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
            gas: A::ConfidentialGas {
                proof_base: defaults::confidential::gas::PROOF_BASE,
                per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
                per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
                per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
                per_commitment: defaults::confidential::gas::PER_COMMITMENT,
            },
        },
        crypto: A::Crypto::default(),
        settlement: A::Settlement::default(),
        streaming: A::Streaming {
            key_material: StreamingKeyMaterial::new(KeyPair::random())
                .expect("streaming key material"),
            session_store_dir: PathBuf::from(defaults::streaming::SESSION_STORE_DIR),
            feature_bits: defaults::streaming::FEATURE_BITS,
            soranet: A::StreamingSoranet::from_defaults(),
            sync: A::StreamingSync::from_defaults(),
            codec: A::StreamingCodec {
                cabac_mode: A::CabacMode::Disabled,
                trellis_block_sizes: defaults::streaming::codec::trellis_blocks(),
                rans_tables_path: defaults::streaming::codec::rans_tables_path(),
                entropy_mode: norito::streaming::EntropyMode::RansBundled,
                bundle_width: defaults::streaming::codec::bundle_width(),
                bundle_accel: A::BundleAcceleration::None,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, sync::Arc};

    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        queue::Queue,
        state::{State, StateReadOnly, World},
        tx::AcceptedTransaction,
    };
    use iroha_data_model::{
        ChainId, account::AccountId, metadata::Metadata, transaction::TransactionBuilder,
    };
    use iroha_primitives::json::Json;

    use super::{apply_queued_in_one_block, body_code_hash_hex, minimal_ivm_program};

    #[test]
    fn body_code_hash_hex_matches_full_bytes_hash() {
        let code = minimal_ivm_program(1);
        let expected = hex::encode(<[u8; 32]>::from(iroha_crypto::Hash::new(&code)));
        assert_eq!(body_code_hash_hex(&code), expected);
    }

    #[test]
    fn apply_queued_in_one_block_overrides_chain_id() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::default(), kura, query));
        let chain_id: ChainId = "chain".parse().expect("chain id");

        assert_ne!(&state.chain_id, &chain_id);

        let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            events,
        ));

        let keypair = iroha_crypto::KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain id"),
            keypair.public_key().clone(),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            "sumeragi_heartbeat".parse().expect("metadata key"),
            Json::new(true),
        );

        let tx = TransactionBuilder::new(chain_id.clone(), authority)
            .with_metadata(metadata)
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        queue.push(accepted, state.view()).expect("queue push");

        let applied = apply_queued_in_one_block(&state, &queue, &chain_id, 1);
        assert_eq!(applied, 1);
    }
}

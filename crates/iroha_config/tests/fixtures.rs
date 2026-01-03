#![allow(clippy::needless_raw_string_hashes, clippy::assertions_on_constants)] // triggered by `expect!` snapshots
//! Test fixtures exercising `iroha_config` parameter loading and validation.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Once,
    time::Duration,
};

use assertables::assert_contains;
use error_stack::{Report, ResultExt};
use expect_test::expect;
use iroha_config::parameters::user::ParseError;
#[allow(unused_imports)]
use iroha_config::parameters::{
    actual::{
        BlockSync, DaManifestPolicy, DataspaceGossip, DataspaceGossipFallback, FraudRiskBand,
        LaneProfile, OfflineProofMode, OperatorAuthLockout, OperatorTokenFallback,
        OperatorTokenSource, OracleChangeThresholds, OracleEconomics, OracleGovernance,
        OracleTwitterBinding, Root as Config, SoranetVpn, Streaming, StreamingSoranetAccessKind,
        StreamingSync, ToriiOperatorAuth, TransactionGossiper,
    },
    defaults,
    user::{Root as UserConfig, ToriiSoranetPrivacyIngest},
};
use iroha_config_base::{env::MockEnv, read::ConfigReader};
use iroha_data_model::{account::AccountId, name::Name};
use soranet_pq::MlKemSuite;
use thiserror::Error;
use url::Url;

fn fixtures_dir() -> PathBuf {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))
            .expect("tests run relative to crate root");
    });
    PathBuf::from("tests/fixtures")
}

fn parse_env(raw: impl AsRef<str>) -> HashMap<String, String> {
    raw.as_ref()
        .lines()
        .map(|line| {
            let mut items = line.split('=');
            let key = items
                .next()
                .expect("line should be in {key}={value} format");
            let value = items
                .next()
                .expect("line should be in {key}={value} format");
            (key.to_string(), value.to_string())
        })
        .collect()
}

fn test_env_from_file(p: impl AsRef<Path>) -> MockEnv {
    let contents = fs::read_to_string(p).expect("the path should be valid");
    let map = parse_env(contents);
    MockEnv::with_map(map)
}

fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[derive(Error, Debug)]
#[error("failed to load config from fixtures")]
struct FixtureConfigLoadError;

fn load_config_from_fixtures(path: impl AsRef<Path>) -> Result<Config, FixtureConfigLoadError> {
    let config = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join(path))
        .change_context(FixtureConfigLoadError)?
        .read_and_complete::<UserConfig>()
        .change_context(FixtureConfigLoadError)?
        .parse()
        .change_context(FixtureConfigLoadError)?;

    Ok(config)
}

#[allow(dead_code)]
fn load_user_config_from_fixtures(
    path: impl AsRef<Path>,
) -> Result<UserConfig, FixtureConfigLoadError> {
    ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join(path))
        .change_context(FixtureConfigLoadError)?
        .read_and_complete::<UserConfig>()
        .change_context(FixtureConfigLoadError)
}

/// This test not only asserts that the minimal set of fields is enough;
/// it also gives an insight into every single default value
#[test]
#[allow(clippy::too_many_lines)]
fn minimal_config_snapshot() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");

    // Snapshot updated to include new Sumeragi fields and other defaults
    expect![[r#"
        Root {
            common: Common {
                chain: ChainId(
                    "0",
                ),
                key_pair: KeyPair {
                    public_key: PublicKey(
                        bls_normal(
                            "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2",
                        ),
                    ),
                    private_key: "[REDACTED PrivateKey]",
                },
                peer: Peer {
                    address: Ipv4(
                        127.0.0.1:1337,
                    ),
                    id: ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2,
                },
                trusted_peers: WithOrigin {
                    value: TrustedPeers {
                        myself: Peer {
                            address: Ipv4(
                                127.0.0.1:1337,
                            ),
                            id: ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2,
                        },
                        others: UniqueVec(
                            [],
                        ),
                        pops: {},
                    },
                    origin: File {
                        id: ParameterId(trusted_peers),
                        path: "tests/fixtures/base_trusted_peers.toml",
                    },
                },
                default_account_domain_label: WithOrigin {
                    value: "default",
                    origin: Default {
                        id: ParameterId(default_account_domain_label),
                    },
                },
                chain_discriminant: WithOrigin {
                    value: 753,
                    origin: Default {
                        id: ParameterId(chain_discriminant),
                    },
                },
            },
            network: Network {
                address: WithOrigin {
                    value: Ipv4(
                        127.0.0.1:1337,
                    ),
                    origin: File {
                        id: ParameterId(network.address),
                        path: "tests/fixtures/base.toml",
                    },
                },
                public_address: WithOrigin {
                    value: Ipv4(
                        127.0.0.1:1337,
                    ),
                    origin: File {
                        id: ParameterId(network.public_address),
                        path: "tests/fixtures/base.toml",
                    },
                },
                relay_mode: Disabled,
                relay_hub_address: None,
                relay_ttl: 8,
                soranet_handshake: SoranetHandshake {
                    descriptor_commit: WithOrigin {
                        value_hex: "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
                        origin: Default {
                            id: ParameterId(network.soranet_handshake.descriptor_commit),
                        },
                    },
                    client_capabilities: WithOrigin {
                        value_hex: "010100020101010200020101010400030203010202000200047f100004deadbeef7f110004cafebabe",
                        origin: Default {
                            id: ParameterId(network.soranet_handshake.client_capabilities),
                        },
                    },
                    relay_capabilities: WithOrigin {
                        value_hex: "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f0104000302030102010001010202000200047f12000412345678",
                        origin: Default {
                            id: ParameterId(network.soranet_handshake.relay_capabilities),
                        },
                    },
                    trust_gossip: true,
                    kem_id: 1,
                    sig_id: 1,
                    resume_hash: None,
                    pow: SoranetPow { required: false, difficulty: 0, max_future_skew_secs: 300, min_ticket_ttl_secs: 30, ticket_ttl_secs: 60, revocation_store_capacity: 8192, revocation_max_ttl_secs: 900, revocation_store_path: ./storage/soranet/ticket_revocations.norito, puzzle: Some { memory_kib: 65536, time_cost: 2, lanes: 1 }, signed_ticket_public_key: None },
                },
                soranet_privacy: SoranetPrivacy {
                    bucket_secs: 60,
                    min_handshakes: 12,
                    flush_delay_buckets: 1,
                    force_flush_buckets: 6,
                    max_completed_buckets: 120,
                    max_share_lag_buckets: 12,
                    expected_shares: 2,
                    event_buffer_capacity: 4096,
                },
                soranet_vpn: SoranetVpn {
                    enabled: true,
                    cell_size_bytes: 1024,
                    flow_label_bits: 24,
                    cover_to_data_per_mille: 250,
                    max_cover_burst: 3,
                    heartbeat_ms: 500,
                    jitter_ms: 10,
                    padding_budget_ms: 15,
                    guard_refresh: 3600s,
                    lease: 600s,
                    dns_push_interval: 90s,
                    exit_class: "standard",
                    meter_family: "soranet.vpn.standard",
                },
                lane_profile: Core,
                require_sm_handshake_match: true,
                require_sm_openssl_preview_match: true,
                idle_timeout: 300s,
                peer_gossip_period: 1s,
                trust_gossip: true,
                trust_decay_half_life: 300s,
                trust_penalty_bad_gossip: 5,
                trust_penalty_unknown_peer: 3,
                trust_min_score: -20,
                dns_refresh_interval: None,
                dns_refresh_ttl: None,
                quic_enabled: false,
                tls_enabled: false,
                tls_listen_address: None,
                prefer_ws_fallback: false,
                p2p_queue_cap_high: 8192,
                p2p_queue_cap_low: 32768,
                p2p_post_queue_cap: 2048,
                happy_eyeballs_stagger: 100ms,
                addr_ipv6_first: false,
                max_incoming: None,
                max_total_connections: None,
                accept_rate_per_ip_per_sec: None,
                accept_burst_per_ip: None,
                max_accept_buckets: 4096,
                accept_bucket_idle: 600s,
                accept_prefix_v4_bits: 24,
                accept_prefix_v6_bits: 64,
                accept_rate_per_prefix_per_sec: None,
                accept_burst_per_prefix: None,
                low_priority_rate_per_sec: None,
                low_priority_burst: None,
                low_priority_bytes_per_sec: None,
                low_priority_bytes_burst: None,
                allowlist_only: false,
                allow_keys: [],
                deny_keys: [],
                allow_cidrs: [],
                deny_cidrs: [],
                disconnect_on_post_overflow: true,
                max_frame_bytes: 1048576,
                tcp_nodelay: true,
                tcp_keepalive: Some(
                    60s,
                ),
                max_frame_bytes_consensus: 1048576,
                max_frame_bytes_control: 131072,
                max_frame_bytes_block_sync: 1048576,
                max_frame_bytes_tx_gossip: 131072,
                max_frame_bytes_peer_gossip: 65536,
                max_frame_bytes_health: 32768,
                max_frame_bytes_other: 131072,
                tls_only_v1_3: false,
                quic_max_idle_timeout: None,
            },
            genesis: Genesis {
                public_key: PublicKey(
                    ed25519(
                        "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB",
                    ),
                ),
                file: None,
                manifest_json: None,
                expected_hash: None,
                bootstrap_allowlist: [],
                bootstrap_max_bytes: 16777216,
                bootstrap_response_throttle: 1s,
                bootstrap_request_timeout: 3s,
                bootstrap_retry_interval: 1s,
                bootstrap_max_attempts: 5,
                bootstrap_enabled: true,
            },
            torii: Torii {
                address: WithOrigin {
                    value: Ipv4(
                        127.0.0.1:8080,
                    ),
                    origin: File {
                        id: ParameterId(torii.address),
                        path: "tests/fixtures/base.toml",
                    },
                },
                api_versions: [
                    "1.0",
                    "1.1",
                ],
                api_version_default: "1.1",
                api_min_proof_version: "1.1",
                api_version_sunset_unix: Some(
                    1893456000,
                ),
                max_content_len: Bytes(
                    16777216,
                ),
                data_dir: "./storage/torii",
                query_rate_per_authority_per_sec: Some(
                    25,
                ),
                query_burst_per_authority: Some(
                    50,
                ),
                deploy_rate_per_origin_per_sec: Some(
                    4,
                ),
                deploy_burst_per_origin: Some(
                    8,
                ),
                require_api_token: false,
                api_tokens: [],
                api_fee_asset_id: None,
                api_fee_amount: None,
                api_fee_receiver: None,
                soranet_privacy_ingest: SoranetPrivacyIngest {
                    enabled: false,
                    require_token: true,
                    tokens: [],
                    rate_per_sec: Some(
                        8,
                    ),
                    burst: Some(
                        16,
                    ),
                    allow_cidrs: [],
                },
                api_allow_cidrs: [],
                peer_telemetry_urls: [],
                strict_addresses: true,
                debug_match_filters: false,
                operator_auth: ToriiOperatorAuth {
                    enabled: false,
                    require_mtls: false,
                    token_fallback: OperatorTokenFallback::Bootstrap,
                    token_source: OperatorTokenSource::OperatorTokens,
                    tokens: [],
                    rate_per_minute: Some(
                        30,
                    ),
                    burst: Some(
                        10,
                    ),
                    lockout: OperatorAuthLockout {
                        failures: Some(
                            5,
                        ),
                        window: 300s,
                        duration: 900s,
                    },
                    webauthn: None,
                },
                preauth_max_connections: None,
                preauth_max_connections_per_ip: None,
                preauth_rate_per_ip_per_sec: Some(
                    20,
                ),
                preauth_burst_per_ip: Some(
                    10,
                ),
                preauth_temp_ban: Some(
                    60s,
                ),
                preauth_allow_cidrs: [],
                preauth_scheme_limits: [],
                api_high_load_tx_threshold: None,
                api_high_load_stream_threshold: None,
                api_high_load_subscription_threshold: None,
                events_buffer_capacity: 10000,
                attachments_ttl_secs: 604800,
                attachments_max_bytes: 4194304,
                attachments_per_tenant_max_count: 128,
                attachments_per_tenant_max_bytes: 67108864,
                attachments_allowed_mime_types: [
                    "application/x-norito",
                    "application/json",
                    "text/json",
                    "application/x-zk1",
                ],
                attachments_max_expanded_bytes: 16777216,
                attachments_max_archive_depth: 2,
                attachments_sanitizer_mode: Subprocess,
                attachments_sanitize_timeout_ms: 1000,
                zk_prover_enabled: false,
                zk_prover_scan_period_secs: 30,
                zk_prover_reports_ttl_secs: 604800,
                zk_prover_max_inflight: 2,
                zk_prover_max_scan_bytes: 16777216,
                zk_prover_max_scan_millis: 2000,
                zk_prover_keys_dir: "./storage/torii/zk_prover/keys",
                zk_prover_allowed_backends: [
                    "halo2/",
                ],
                zk_prover_allowed_circuits: [],
                connect: Connect {
                    enabled: true,
                    ws_max_sessions: 10000,
                    ws_per_ip_max_sessions: 10,
                    ws_rate_per_ip_per_min: 120,
                    session_ttl: 300s,
                    frame_max_bytes: 64000,
                    session_buffer_max_bytes: 262144,
                    ping_interval: 30s,
                    ping_miss_tolerance: 3,
                    ping_min_interval: 15s,
                    dedupe_ttl: 120s,
                    dedupe_cap: 8192,
                    relay_enabled: true,
                    relay_strategy: "broadcast",
                    p2p_ttl_hops: 0,
                },
                iso_bridge: IsoBridge {
                    enabled: false,
                    dedupe_ttl_secs: 300,
                    signer: None,
                    account_aliases: [],
                    currency_assets: [],
                    reference_data: IsoReferenceData {
                        refresh_interval: 86400s,
                        isin_crosswalk_path: None,
                        bic_lei_path: None,
                        mic_directory_path: None,
                        cache_dir: None,
                    },
                },
                rbc_sampling: RbcSampling {
                    enabled: false,
                    max_samples_per_request: 3,
                    max_bytes_per_request: 262144,
                    daily_byte_budget: 4194304,
                    rate_per_minute: Some(
                        12,
                    ),
                },
                da_ingest: DaIngest {
                    replay_cache_capacity: 4096,
                    replay_cache_ttl: 900s,
                    replay_cache_max_sequence_lag: 4096,
                    replay_cache_store_dir: "./storage/da_replay",
                    manifest_store_dir: "./storage/da_manifests",
                    governance_metadata_key: None,
                    governance_metadata_key_label: None,
                    taikai_anchor: None,
                    replication_policy: DaReplicationPolicy {
                        default: RetentionPolicy {
                            hot_retention_secs: 21600,
                            cold_retention_secs: 2592000,
                            required_replicas: 3,
                            storage_class: Warm,
                            governance_tag: GovernanceTag(
                                "da.default",
                            ),
                        },
                        overrides: {
                            TaikaiSegment: RetentionPolicy {
                                hot_retention_secs: 86400,
                                cold_retention_secs: 1209600,
                                required_replicas: 5,
                                storage_class: Hot,
                                governance_tag: GovernanceTag(
                                    "da.taikai.live",
                                ),
                            },
                            NexusLaneSidecar: RetentionPolicy {
                                hot_retention_secs: 21600,
                                cold_retention_secs: 604800,
                                required_replicas: 4,
                                storage_class: Warm,
                                governance_tag: GovernanceTag(
                                    "da.sidecar",
                                ),
                            },
                            GovernanceArtifact: RetentionPolicy {
                                hot_retention_secs: 43200,
                                cold_retention_secs: 15552000,
                                required_replicas: 3,
                                storage_class: Cold,
                                governance_tag: GovernanceTag(
                                    "da.governance",
                                ),
                            },
                        },
                        taikai_availability: {
                            Hot: RetentionPolicy {
                                hot_retention_secs: 86400,
                                cold_retention_secs: 1209600,
                                required_replicas: 5,
                                storage_class: Hot,
                                governance_tag: GovernanceTag(
                                    "da.taikai.live",
                                ),
                            },
                            Warm: RetentionPolicy {
                                hot_retention_secs: 21600,
                                cold_retention_secs: 2592000,
                                required_replicas: 4,
                                storage_class: Warm,
                                governance_tag: GovernanceTag(
                                    "da.taikai.warm",
                                ),
                            },
                            Cold: RetentionPolicy {
                                hot_retention_secs: 3600,
                                cold_retention_secs: 15552000,
                                required_replicas: 3,
                                storage_class: Cold,
                                governance_tag: GovernanceTag(
                                    "da.taikai.archive",
                                ),
                            },
                        },
                    },
                    rent_policy: DaRentPolicyV1 {
                        version: 1,
                        base_rate_per_gib_month: XorAmount {
                            micro: 250000,
                        },
                        protocol_reserve_bps: 2000,
                        pdp_bonus_bps: 500,
                        potr_bonus_bps: 250,
                        egress_credit_per_gib: XorAmount {
                            micro: 1500,
                        },
                    },
                    telemetry_cluster_label: None,
                },
                sorafs_discovery: SorafsDiscovery {
                    discovery_enabled: true,
                    known_capabilities: [
                        "torii_gateway",
                        "chunk_range_fetch",
                        "vendor_reserved",
                    ],
                    admission: Some(
                        SorafsAdmission {
                            envelopes_dir: "tests/fixtures/sorafs_admission",
                        },
                    ),
                },
                sorafs_storage: SorafsStorage {
                    enabled: false,
                    data_dir: "./storage/sorafs",
                    max_capacity_bytes: Bytes(
                        107374182400,
                    ),
                    max_parallel_fetches: 32,
                    max_pins: 10000,
                    por_sample_interval_secs: 600,
                    alias: None,
                    adverts: SorafsAdvertOverrides {
                        stake_pointer: None,
                        availability: "hot",
                        max_latency_ms: 500,
                        topics: [
                            "sorafs.sf1.primary:global",
                        ],
                    },
                    metering_smoothing: SorafsMeteringSmoothing {
                        gib_hours_alpha: None,
                        por_success_alpha: None,
                    },
                    stream_tokens: SorafsTokenConfig {
                        enabled: false,
                        signing_key_path: None,
                        key_version: 1,
                        default_ttl_secs: 900,
                        default_max_streams: 4,
                        default_rate_limit_bytes: 8388608,
                        default_requests_per_minute: 120,
                    },
                    governance_dag_dir: None,
                    pin: SorafsStoragePin {
                        require_token: false,
                        tokens: {},
                        allow_cidrs: [],
                        rate_limit: SorafsGatewayRateLimit {
                            max_requests: None,
                            window: 1s,
                            ban: None,
                        },
                    },
                },
                sorafs_quota: SorafsQuota {
                    capacity_declaration: SorafsQuotaWindow {
                        max_events: Some(
                            4,
                        ),
                        window: 3600s,
                    },
                    capacity_telemetry: SorafsQuotaWindow {
                        max_events: Some(
                            12,
                        ),
                        window: 900s,
                    },
                    deal_telemetry: SorafsQuotaWindow {
                        max_events: Some(
                            60,
                        ),
                        window: 900s,
                    },
                    capacity_dispute: SorafsQuotaWindow {
                        max_events: Some(
                            2,
                        ),
                        window: 1800s,
                    },
                    storage_pin: SorafsQuotaWindow {
                        max_events: Some(
                            4,
                        ),
                        window: 3600s,
                    },
                    por_submission: SorafsQuotaWindow {
                        max_events: Some(
                            60,
                        ),
                        window: 900s,
                    },
                },
                sorafs_alias_cache: SorafsAliasCachePolicy {
                    positive_ttl: 600s,
                    refresh_window: 120s,
                    hard_expiry: 900s,
                    negative_ttl: 60s,
                    revocation_ttl: 300s,
                    rotation_max_age: 21600s,
                    successor_grace: 300s,
                    governance_grace: 0ns,
                },
                sorafs_gateway: SorafsGateway {
                    require_manifest_envelope: true,
                    enforce_admission: true,
                    enforce_capabilities: false,
                    salt_schedule_dir: None,
                    cdn_policy_path: None,
                    rate_limit: SorafsGatewayRateLimit {
                        max_requests: Some(
                            300,
                        ),
                        window: 60s,
                        ban: Some(
                            30s,
                        ),
                    },
                    denylist: SorafsGatewayDenylist {
                        path: None,
                        standard_ttl: 15552000s,
                        emergency_ttl: 2592000s,
                        emergency_review_window: 604800s,
                        require_governance_reference: true,
                    },
                    rollout_phase: Canary,
                    anonymity_policy: Some(
                        GuardPq,
                    ),
                    acme: SorafsGatewayAcme {
                        enabled: false,
                        account_email: None,
                        directory_url: "https://acme-v02.api.letsencrypt.org/directory",
                        hostnames: [],
                        dns_provider_id: None,
                        renewal_window: 2592000s,
                        retry_backoff: 1800s,
                        retry_jitter: 300s,
                        challenges: SorafsGatewayAcmeChallenges {
                            dns01: true,
                            tls_alpn_01: true,
                        },
                        ech_enabled: false,
                    },
                    direct_mode: None,
                },
                sorafs_por: SorafsPor {
                    enabled: false,
                    epoch_interval_secs: 3600,
                    response_window_secs: 900,
                    governance_dag_dir: "./storage/sorafs/governance",
                    randomness_seed: None,
                },
                transport: ToriiTransport {
                    norito_rpc: NoritoRpcTransport {
                        enabled: true,
                        require_mtls: false,
                        allowed_clients: [],
                        stage: Disabled,
                    },
                },
                proof_api: ProofApi {
                    rate_per_minute: Some(
                        120,
                    ),
                    burst: Some(
                        60,
                    ),
                    max_body_bytes: Bytes(
                        8388608,
                    ),
                    egress_bytes_per_sec: Some(
                        8388608,
                    ),
                    egress_burst_bytes: Some(
                        16777216,
                    ),
                    max_list_limit: 200,
                    request_timeout: 1s,
                    cache_max_age: 30s,
                    retry_after: 1s,
                },
                onboarding: None,
                offline_issuer: None,
                app_api: AppApi {
                    default_list_limit: 100,
                    max_list_limit: 500,
                    max_fetch_size: 500,
                    rate_limit_cost_per_row: 1,
                },
                webhook: Webhook {
                    queue_capacity: 10000,
                    max_attempts: 12,
                    backoff_initial: 1s,
                    backoff_max: 60s,
                    connect_timeout: 10s,
                    write_timeout: 10s,
                    read_timeout: 10s,
                },
                push: Push {
                    enabled: false,
                    rate_per_minute: Some(
                        60,
                    ),
                    burst: Some(
                        30,
                    ),
                    connect_timeout: 5s,
                    request_timeout: 10s,
                    max_topics_per_device: 32,
                    fcm_api_key: None,
                    apns_endpoint: None,
                    apns_auth_token: None,
                },
            },
            kura: Kura {
                init_mode: Strict,
                store_dir: WithOrigin {
                    value: "./storage",
                    origin: Default {
                        id: ParameterId(kura.store_dir),
                    },
                },
                blocks_in_memory: 1024,
                block_sync_roster_retention: 7200,
                roster_sidecar_retention: 512,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity: 256,
                fsync_mode: Batched,
                fsync_interval: 50ms,
            },
            sumeragi: Sumeragi {
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
                role: Validator,
                allow_view0_slack: false,
                collectors_k: 1,
                collectors_redundant_send_r: 1,
                block_max_transactions: None,
                block_max_payload_bytes: None,
                msg_channel_cap_votes: 8192,
                msg_channel_cap_block_payload: 128,
                msg_channel_cap_rbc_chunks: 1024,
                msg_channel_cap_blocks: 256,
                control_msg_channel_cap: 1024,
                consensus_mode: Permissioned,
                mode_flip_enabled: true,
                da_enabled: false,
                da_quorum_timeout_multiplier: 3,
                da_availability_timeout_multiplier: 2,
                da_availability_timeout_floor: 2s,
                kura_store_retry_interval: 1s,
                kura_store_retry_max_attempts: 5,
                commit_inflight_timeout: 30s,
                missing_block_signer_fallback_attempts: 1,
                membership_mismatch_alert_threshold: 1,
                membership_mismatch_fail_closed: false,
                da_max_commitments_per_block: 16,
                da_max_proof_openings_per_block: 128,
                proof_policy: Off,
                commit_cert_history_cap: 512,
                zk_finality_k: 0,
                require_execution_qc: false,
                require_precommit_qc: true,
                require_wsv_exec_qc: false,
                rbc_chunk_max_bytes: 65536,
                rbc_pending_max_chunks: 128,
                rbc_pending_max_bytes: 8388608,
                rbc_pending_ttl: 30s,
                rbc_session_ttl: 120s,
                rbc_store_max_sessions: 1024,
                rbc_store_soft_sessions: 768,
                rbc_store_max_bytes: 536870912,
                rbc_store_soft_bytes: 402653184,
                rbc_disk_store_ttl: 120s,
                rbc_disk_store_max_bytes: 536870912,
                key_activation_lead_blocks: 1,
                key_overlap_grace_blocks: 8,
                key_expiry_grace_blocks: 0,
                key_require_hsm: false,
                key_allowed_algorithms: {
                    BlsNormal,
                },
                key_allowed_hsm_providers: {
                    "pkcs11",
                    "softkey",
                    "yubihsm",
                },
                npos: SumeragiNpos {
                    block_time: 1s,
                    timeouts: SumeragiNposTimeouts {
                        propose: 350ms,
                        prevote: 450ms,
                        precommit: 550ms,
                        exec: 150ms,
                        witness: 150ms,
                        commit: 750ms,
                        da: 650ms,
                        aggregator: 120ms,
                    },
                    pacemaker_backoff_multiplier: 1,
                    pacemaker_rtt_floor_multiplier: 2,
                    pacemaker_max_backoff: 10s,
                    pacemaker_jitter_frac_permille: 0,
                    k_aggregators: 3,
                    redundant_send_r: 2,
                    vrf: SumeragiNposVrf {
                        commit_window_blocks: 100,
                        reveal_window_blocks: 40,
                    },
                    election: SumeragiNposElection {
                        max_validators: 128,
                        min_self_bond: 1000,
                        min_nomination_bond: 1,
                        max_nominator_concentration_pct: 25,
                        seat_band_pct: 5,
                        max_entity_correlation_pct: 25,
                        finality_margin_blocks: 8,
                    },
                    reconfig: SumeragiNposReconfig {
                        evidence_horizon_blocks: 7200,
                        activation_lag_blocks: 1,
                    },
                },
                use_stake_snapshot_roster: false,
                epoch_length_blocks: 3600,
                vrf_commit_deadline_offset: 100,
                vrf_reveal_deadline_offset: 140,
                pacemaker_backoff_multiplier: 1,
                pacemaker_rtt_floor_multiplier: 2,
                pacemaker_max_backoff: 10s,
                pacemaker_jitter_frac_permille: 0,
                enable_bls: true,
                adaptive_observability: AdaptiveObservability {
                    enabled: false,
                    qc_latency_alert_ms: 400,
                    da_reschedule_burst: 2,
                    pacemaker_extra_ms: 100,
                    collector_redundant_r: 3,
                    cooldown_ms: 5000,
                },
            },
            block_sync: BlockSync {
                gossip_period: 10s,
                gossip_size: 4,
            },
            transaction_gossiper: TransactionGossiper {
                gossip_period: 1s,
                gossip_size: 500,
                dataspace: DataspaceGossip {
                    drop_unknown_dataspace: false,
                    restricted_target_cap: None,
                    public_target_cap: Some(
                        16,
                    ),
                    public_target_reshuffle: 1s,
                    restricted_target_reshuffle: 1s,
                    restricted_fallback: Drop,
                    restricted_public_payload: Refuse,
                },
            },
            live_query_store: LiveQueryStore {
                idle_time: 10s,
                capacity: 128,
                capacity_per_user: 128,
            },
            logger: Logger {
                level: INFO,
                filter: None,
                format: Full,
                terminal_colors: false,
            },
            queue: Queue {
                capacity: 65536,
                capacity_per_user: 65536,
                transaction_time_to_live: 86400s,
            },
            nexus: Nexus {
                enabled: true,
                staking: NexusStaking {
                    public_validator_mode: StakeElected,
                    restricted_validator_mode: AdminManaged,
                    min_validator_stake: 1,
                    max_validators: 32,
                    unbonding_delay: 0ns,
                    withdraw_grace: 0ns,
                    max_slash_bps: 10000,
                    reward_dust_threshold: 0,
                    stake_asset_id: "xor#nexus",
                    stake_escrow_account_id: "gas@ivm",
                    slash_sink_account_id: "gas@ivm",
                },
                fees: NexusFees {
                    fee_asset_id: "xor#nexus",
                    fee_sink_account_id: "gas@ivm",
                    base_fee: 0,
                    per_byte_fee: 0,
                    per_instruction_fee: 0,
                    per_gas_unit_fee: 0,
                    sponsorship_enabled: false,
                    sponsor_max_fee: 0,
                },
                endorsement: NexusEndorsement {
                    committee_keys: [],
                    quorum: 0,
                },
                axt: NexusAxt {
                    slot_length_ms: 1,
                    max_clock_skew_ms: 0,
                    proof_cache_ttl_slots: 1,
                    replay_retention_slots: 128,
                },
                lane_relay_emergency: LaneRelayEmergency {
                    enabled: false,
                    multisig_threshold: 3,
                    multisig_members: 5,
                },
                lane_catalog: LaneCatalog {
                    lane_count: 1,
                    lanes: [
                        LaneConfig {
                            id: LaneId(
                                0,
                            ),
                            dataspace_id: DataSpaceId(
                                0,
                            ),
                            alias: "default",
                            description: None,
                            visibility: Public,
                            lane_type: None,
                            governance: None,
                            settlement: None,
                            storage: FullReplica,
                            proof_scheme: MerkleSha256,
                            metadata: {},
                        },
                    ],
                },
                lane_config: LaneConfig {
                    entries: [
                        LaneConfigEntry {
                            lane_id: LaneId(
                                0,
                            ),
                            shard_id: 0,
                            dataspace_id: DataSpaceId(
                                0,
                            ),
                            visibility: Public,
                            storage_profile: FullReplica,
                            proof_scheme: MerkleSha256,
                            alias: "default",
                            slug: "default",
                            kura_segment: "lane_000_default",
                            merge_segment: "lane_000_default_merge",
                            key_prefix: [
                                0,
                                0,
                                0,
                                0,
                            ],
                            manifest_policy: Strict,
                            confidential_compute: false,
                            confidential_policy: None,
                            confidential_access: [],
                        },
                    ],
                    by_id: {
                        LaneId(
                            0,
                        ): 0,
                    },
                },
                dataspace_catalog: DataSpaceCatalog {
                    entries: [
                        DataSpaceMetadata {
                            id: DataSpaceId(
                                0,
                            ),
                            alias: "global",
                            description: None,
                            fault_tolerance: 1,
                        },
                    ],
                },
                routing_policy: LaneRoutingPolicy {
                    default_lane: LaneId(
                        0,
                    ),
                    default_dataspace: DataSpaceId(
                        0,
                    ),
                    rules: [],
                },
                registry: LaneRegistry {
                    manifest_directory: None,
                    cache_directory: None,
                    poll_interval: 60s,
                },
                governance: GovernanceCatalog {
                    default_module: None,
                    modules: {},
                },
                compliance: LaneCompliance {
                    enabled: false,
                    audit_only: true,
                    policy_dir: None,
                },
                fusion: Fusion {
                    floor_teu: 4000,
                    exit_teu: 6000,
                    observation_slots: 2,
                    max_window_slots: 16,
                },
                commit: Commit {
                    window_slots: 2,
                },
                da: Da {
                    q_in_slot_total: 2048,
                    q_in_slot_per_ds_min: 8,
                    sample_size_base: 64,
                    sample_size_max: 96,
                    threshold_base: 43,
                    per_attester_shards: 25,
                    audit: DaAudit {
                        sample_size: 32,
                        window_count: 20,
                        interval: 600s,
                    },
                    recovery: DaRecovery {
                        request_timeout: 86400s,
                    },
                    rotation: DaRotation {
                        max_hits_per_window: 4,
                        window_slots: 64,
                        seed_tag: "iroha:da:rotate:v1\0",
                        latency_decay: 0.25,
                    },
                },
            },
            snapshot: Snapshot {
                mode: ReadWrite,
                create_every_ms: DurationMs(
                    600s,
                ),
                store_dir: WithOrigin {
                    value: "./storage/snapshot",
                    origin: Default {
                        id: ParameterId(snapshot.store_dir),
                    },
                },
                merkle_chunk_size_bytes: 1048576,
                verification_public_key: None,
                signing_private_key: None,
            },
            telemetry_enabled: true,
            telemetry_profile: Operator,
            telemetry: None,
            telemetry_redaction: TelemetryRedaction {
                mode: Strict,
                allowlist: [],
            },
            telemetry_integrity: TelemetryIntegrity {
                enabled: true,
                state_dir: None,
                signing_key: None,
                signing_key_id: None,
            },
            dev_telemetry: DevTelemetry {
                out_file: None,
                panic_on_duplicate_metrics: false,
            },
            pipeline: Pipeline {
                dynamic_prepass: true,
                access_set_cache_enabled: true,
                parallel_overlay: true,
                workers: 0,
                parallel_apply: true,
                ready_queue_heap: false,
                gpu_key_bucket: false,
                debug_trace_scheduler_inputs: false,
                debug_trace_tx_eval: false,
                signature_batch_max: 0,
                signature_batch_max_ed25519: 0,
                signature_batch_max_secp256k1: 0,
                signature_batch_max_pqc: 0,
                signature_batch_max_bls: 4,
                cache_size: 128,
                ivm_cache_max_decoded_ops: 8000000,
                ivm_cache_max_bytes: 67108864,
                ivm_prover_threads: 0,
                overlay_max_instructions: 0,
                overlay_max_bytes: 0,
                overlay_chunk_instructions: 256,
                gas: Gas {
                    tech_account_id: "gas@ivm",
                    accepted_assets: [],
                    units_per_gas: [],
                },
                ivm_max_cycles_upper_bound: 1000000,
                ivm_max_decoded_instructions: 1048576,
                ivm_max_decoded_bytes: 4194304,
                quarantine_max_txs_per_block: 0,
                quarantine_tx_max_cycles: 0,
                quarantine_tx_max_millis: 0,
                query_default_cursor_mode: Ephemeral,
                query_stored_min_gas_units: 0,
                amx_per_dataspace_budget_ms: 30,
                amx_group_budget_ms: 140,
                amx_per_instruction_ns: 50,
                amx_per_memory_access_ns: 80,
                amx_per_syscall_ns: 120,
            },
            tiered_state: TieredState {
                enabled: false,
                hot_retained_keys: 0,
                cold_store_root: None,
                max_snapshots: 2,
            },
            compute: Compute {
                enabled: false,
                namespaces: {
                    Name(
                        "compute",
                    ),
                },
                default_ttl_slots: 32,
                max_ttl_slots: 512,
                max_request_bytes: Bytes(
                    524288,
                ),
                max_response_bytes: Bytes(
                    524288,
                ),
                max_gas_per_call: 5000000,
                resource_profiles: {
                    Name(
                        "cpu-balanced",
                    ): ComputeResourceBudget {
                        max_cycles: 10000000,
                        max_memory_bytes: 268435456,
                        max_stack_bytes: 4194304,
                        max_io_bytes: 25165824,
                        max_egress_bytes: 12582912,
                        allow_gpu_hints: true,
                        allow_wasi: true,
                    },
                    Name(
                        "cpu-small",
                    ): ComputeResourceBudget {
                        max_cycles: 5000000,
                        max_memory_bytes: 134217728,
                        max_stack_bytes: 2097152,
                        max_io_bytes: 16777216,
                        max_egress_bytes: 8388608,
                        allow_gpu_hints: false,
                        allow_wasi: false,
                    },
                },
                default_resource_profile: Name(
                    "cpu-small",
                ),
                price_families: {
                    Name(
                        "default",
                    ): ComputePriceWeights {
                        cycles_per_unit: 1000000,
                        egress_bytes_per_unit: 1024,
                        unit_label: "cu",
                    },
                },
                default_price_family: Name(
                    "default",
                ),
                auth_policy: Either,
                sandbox: ComputeSandboxRules {
                    mode: IvmOnly,
                    randomness: SeededFromRequest,
                    storage: ReadOnly,
                    deny_nondeterministic_syscalls: true,
                    allow_gpu_hints: false,
                    allow_tee_hints: false,
                },
                economics: ComputeEconomics {
                    max_cu_per_call: 100000,
                    max_amplification_ratio: 16,
                    fee_split: ComputeFeeSplit {
                        burn_bps: 2000,
                        validators_bps: 6000,
                        providers_bps: 2000,
                    },
                    sponsor_policy: ComputeSponsorPolicy {
                        max_cu_per_call: 10000,
                        max_daily_cu: 100000,
                    },
                    price_bounds: {
                        Low: ComputePriceDeltaBounds {
                            max_cycles_delta_bps: 500,
                            max_egress_delta_bps: 500,
                        },
                        Balanced: ComputePriceDeltaBounds {
                            max_cycles_delta_bps: 1500,
                            max_egress_delta_bps: 1500,
                        },
                        High: ComputePriceDeltaBounds {
                            max_cycles_delta_bps: 3000,
                            max_egress_delta_bps: 3000,
                        },
                    },
                    price_risk_classes: {
                        Name(
                            "default",
                        ): Balanced,
                    },
                    price_family_baseline: {
                        Name(
                            "default",
                        ): ComputePriceWeights {
                            cycles_per_unit: 1000000,
                            egress_bytes_per_unit: 1024,
                            unit_label: "cu",
                        },
                    },
                    price_amplifiers: ComputePriceAmplifiers {
                        gpu_bps: 13000,
                        tee_bps: 15000,
                        best_effort_bps: 12500,
                    },
                },
                slo: ComputeSlo {
                    max_inflight_per_route: 32,
                    queue_depth_per_route: 512,
                    max_requests_per_second: 200,
                    target_p50_latency_ms: 25,
                    target_p95_latency_ms: 75,
                    target_p99_latency_ms: 120,
                },
            },
            content: Content {
                max_bundle_bytes: 1048576,
                max_files: 128,
                max_path_len: 256,
                max_retention_blocks: 10000,
                chunk_size_bytes: 65536,
                publish_allow_accounts: [],
                limits: ContentLimits {
                    max_requests_per_second: 200,
                    request_burst: 200,
                    max_egress_bytes_per_second: 16777216,
                    egress_burst_bytes: 8388608,
                },
                default_cache_max_age_secs: 300,
                max_cache_max_age_secs: 86400,
                immutable_bundles: true,
                default_auth_mode: Public,
                slo: ContentSlo {
                    target_p50_latency_ms: 50,
                    target_p99_latency_ms: 250,
                    target_availability_bps: 9990,
                },
                pow: ContentPow {
                    difficulty_bits: 0,
                    header_name: "x-iroha-pow",
                },
                stripe_layout: DaStripeLayout {
                    total_stripes: 1,
                    shards_per_stripe: 1,
                    row_parity_stripes: 0,
                },
            },
            oracle: Oracle {
                history_depth: 64,
                governance: OracleGovernance {
                    intake_sla_blocks: 12,
                    rules_sla_blocks: 24,
                    cop_sla_blocks: 36,
                    technical_sla_blocks: 36,
                    policy_jury_sla_blocks: 48,
                    enact_sla_blocks: 48,
                    intake_min_votes: 1,
                    rules_min_votes: 1,
                    cop_min_votes: OracleChangeThresholds {
                        low: 1,
                        medium: 2,
                        high: 3,
                    },
                    technical_min_votes: 2,
                    policy_jury_min_votes: OracleChangeThresholds {
                        low: 2,
                        medium: 3,
                        high: 4,
                    },
                },
                twitter_binding: OracleTwitterBinding {
                    feed_id: Name(
                        "twitter_follow_binding",
                    ),
                    pepper_id: "pepper-social-v1",
                    max_ttl_ms: 86400000,
                    min_ttl_ms: 300000,
                    min_update_spacing_ms: 30000,
                },
            },
            ivm: Ivm {
                memory_budget_profile: Name(
                    "cpu-small",
                ),
                banner: Banner {
                    show: true,
                    beep: true,
                },
            },
            norito: Norito {
                min_compress_bytes_cpu: 256,
                min_compress_bytes_gpu: 1048576,
                zstd_level_small: 1,
                zstd_level_large: 3,
                zstd_level_gpu: 1,
                large_threshold: 32768,
                enable_compact_seq_len_up_to: 18446744073709551615,
                enable_varint_offsets_up_to: 18446744073709551615,
                allow_gpu_compression: true,
                max_archive_len: 67108864,
                aos_ncb_small_n: 64,
            },
            hijiri: Hijiri {
                fee_policy: None,
            },
            fraud_monitoring: FraudMonitoring {
                enabled: false,
                service_endpoints: [],
                connect_timeout: 500ms,
                request_timeout: 1.5s,
                missing_assessment_grace: 0ns,
                required_minimum_band: None,
                attesters: [],
            },
            zk: Zk {
                halo2: Halo2 {
                    enabled: false,
                    curve: Pallas,
                    backend: Ipa,
                    max_k: 16,
                    verifier_budget_ms: 20,
                    verifier_max_batch: 16,
                    max_envelope_bytes: 1048576,
                    max_proof_bytes: 196608,
                    max_transcript_label_len: 64,
                    enforce_transcript_label_ascii: true,
                },
                fastpq: Fastpq {
                    execution_mode: Auto,
                    poseidon_mode: Auto,
                    device_class: None,
                    chip_family: None,
                    gpu_kind: None,
                    metal_queue_fanout: None,
                    metal_queue_column_threshold: None,
                    metal_max_in_flight: None,
                    metal_threadgroup_width: None,
                    metal_trace: false,
                    metal_debug_enum: false,
                    metal_debug_fused: false,
                },
                root_history_cap: 2048,
                ballot_history_cap: 1024,
                empty_root_on_empty: false,
                merkle_depth: 0,
                preverify_max_bytes: 1048576,
                preverify_budget_bytes: 0,
                proof_history_cap: 4096,
                proof_retention_grace_blocks: 256,
                proof_prune_batch: 512,
                bridge_proof_max_range_len: 4096,
                bridge_proof_max_past_age_blocks: 0,
                bridge_proof_max_future_drift_blocks: 0,
                poseidon_params_id: None,
                pedersen_params_id: None,
                kaigi_roster_join_vk: None,
                kaigi_roster_leave_vk: None,
                kaigi_usage_vk: None,
                max_proof_size_bytes: 262144,
                max_nullifiers_per_tx: 8,
                max_commitments_per_tx: 8,
                max_confidential_ops_per_block: 256,
                verify_timeout: 750ms,
                max_anchor_age_blocks: 10000,
                max_proof_bytes_block: 1048576,
                max_verify_calls_per_tx: 4,
                max_verify_calls_per_block: 128,
                max_public_inputs: 32,
                reorg_depth_bound: 10000,
                policy_transition_delay_blocks: 100,
                policy_transition_window_blocks: 200,
                tree_roots_history_len: 10000,
                tree_frontier_checkpoint_interval: 100,
                registry_max_vk_entries: 64,
                registry_max_params_entries: 32,
                registry_max_delta_per_block: 4,
                gas: ConfidentialGas {
                    proof_base: 250000,
                    per_public_input: 2000,
                    per_proof_byte: 5,
                    per_nullifier: 300,
                    per_commitment: 500,
                },
            },
            gov: Governance {
                vk_ballot: None,
                vk_tally: None,
                voting_asset_id: xor#sora,
                citizenship_asset_id: xor#sora,
                citizenship_bond_amount: 150,
                citizenship_escrow_account: 34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland,
                min_bond_amount: 150,
                bond_escrow_account: 34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland,
                slash_receiver_account: 34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland,
                slash_double_vote_bps: 2500,
                slash_invalid_proof_bps: 5000,
                slash_ineligible_proof_bps: 1500,
                alias_teu_minimum: 0,
                alias_frontier_telemetry: true,
                debug_trace_pipeline: false,
                jdg_signature_schemes: {
                    SimpleThreshold,
                },
                citizen_service: CitizenServiceDiscipline {
                    seat_cooldown_blocks: 10000,
                    max_seats_per_epoch: 1,
                    free_declines_per_epoch: 1,
                    decline_slash_bps: 250,
                    no_show_slash_bps: 1000,
                    misconduct_slash_bps: 5000,
                    role_bond_multipliers: {},
                },
                viral_incentives: ViralIncentives {
                    incentive_pool_account: 34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland,
                    escrow_account: 34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland,
                    reward_asset_definition_id: xor#sora,
                    follow_reward_amount: Numeric {
                        mantissa: 1,
                        scale: 0,
                    },
                    sender_bonus_amount: Numeric {
                        mantissa: 1,
                        scale: 1,
                    },
                    max_daily_claims_per_uaid: 1,
                    max_claims_per_binding: 1,
                    daily_budget: Numeric {
                        mantissa: 1000,
                        scale: 0,
                    },
                    halt: false,
                    deny_uaids: [],
                    deny_binding_digests: [],
                    promo_starts_at_ms: None,
                    promo_ends_at_ms: None,
                    campaign_cap: Numeric {
                        mantissa: 0,
                        scale: 0,
                    },
                },
                sorafs_pin_policy: SorafsPinPolicyConstraints {
                    min_replicas_floor: 1,
                    max_replicas_ceiling: None,
                    max_retention_epoch: None,
                    allowed_storage_classes: None,
                },
                sorafs_pricing: PricingScheduleRecord {
                    version: 1,
                    currency_code: "xor",
                    default_storage_class: Hot,
                    tiers: [
                        TierRate {
                            storage_class: Hot,
                            storage_price_nano_per_gib_month: 500000000,
                            egress_price_nano_per_gib: 50000000,
                        },
                        TierRate {
                            storage_class: Warm,
                            storage_price_nano_per_gib_month: 200000000,
                            egress_price_nano_per_gib: 20000000,
                        },
                        TierRate {
                            storage_class: Cold,
                            storage_price_nano_per_gib_month: 50000000,
                            egress_price_nano_per_gib: 10000000,
                        },
                    ],
                    collateral: CollateralPolicy {
                        multiplier_bps: 30000,
                        onboarding_discount_bps: 5000,
                        onboarding_period_secs: 2592000,
                    },
                    credit: CreditPolicy {
                        settlement_window_secs: 604800,
                        settlement_grace_secs: 172800,
                        low_balance_alert_bps: 2000,
                    },
                    discounts: DiscountSchedule {
                        loyalty_months_required: 12,
                        loyalty_discount_bps: 1000,
                        commitment_tiers: [
                            CommitmentDiscountTier {
                                minimum_commitment_gib_month: 500,
                                discount_bps: 500,
                            },
                            CommitmentDiscountTier {
                                minimum_commitment_gib_month: 2000,
                                discount_bps: 1500,
                            },
                        ],
                    },
                    notes: Some(
                        "Launch pricing schedule (0.50/0.20/0.05 XOR GiB·month; egress 0.05/0.02/0.01 XOR)",
                    ),
                },
                sorafs_penalty: SorafsPenaltyPolicy {
                    utilisation_floor_bps: 7500,
                    uptime_floor_bps: 9500,
                    por_success_floor_bps: 9700,
                    strike_threshold: 3,
                    penalty_bond_bps: 2500,
                    cooldown_windows: 2,
                    max_pdp_failures: 0,
                    max_potr_breaches: 0,
                },
                sorafs_telemetry: SorafsTelemetryPolicy {
                    require_submitter: true,
                    require_nonce: true,
                    max_window_gap: 21600s,
                    reject_zero_capacity: true,
                    submitters: [
                        34mSYn6ySFTASoiVzNGuyBkedDcPUhgmtD5UQyEPKXz7u1A1NpZLuc7sms8sFTrvTXHfgsaLr@sora,
                    ],
                    per_provider_submitters: {},
                },
                sorafs_provider_owners: {},
                conviction_step_blocks: 100,
                max_conviction: 6,
                min_enactment_delay: 20,
                window_span: 100,
                plain_voting_enabled: false,
                approval_threshold_q_num: 1,
                approval_threshold_q_den: 2,
                min_turnout: 0,
                parliament_committee_size: 21,
                parliament_term_blocks: 43200,
                parliament_min_stake: 1,
                parliament_eligibility_asset_id: SORA#stake,
                parliament_alternate_size: None,
                parliament_quorum_bps: 6667,
                rules_committee_size: 7,
                agenda_council_size: 9,
                interest_panel_size: 11,
                review_panel_size: 13,
                policy_jury_size: 25,
                oversight_committee_size: 7,
                fma_committee_size: 5,
                pipeline_study_sla_blocks: 20,
                pipeline_review_sla_blocks: 100,
                pipeline_decision_sla_blocks: 1,
                pipeline_enactment_sla_blocks: 200,
                pipeline_rules_sla_blocks: 20,
                pipeline_agenda_sla_blocks: 40,
            },
            nts: Nts {
                sample_interval: 5s,
                sample_cap_per_round: 8,
                max_rtt_ms: 500,
                trim_percent: 10,
                per_peer_buffer: 16,
                smoothing_enabled: false,
                smoothing_alpha: 0.2,
                max_adjust_ms_per_min: 50,
                min_samples: 3,
                max_offset_ms: 1000,
                max_confidence_ms: 500,
                enforcement_mode: Warn,
            },
            accel: Acceleration {
                enable_simd: true,
                enable_cuda: true,
                enable_metal: true,
                max_gpus: None,
                merkle_min_leaves_gpu: 8192,
                merkle_min_leaves_metal: None,
                merkle_min_leaves_cuda: None,
                prefer_cpu_sha2_max_leaves_aarch64: None,
                prefer_cpu_sha2_max_leaves_x86: None,
            },
            concurrency: Concurrency {
                scheduler_min_threads: 0,
                scheduler_max_threads: 0,
                rayon_global_threads: 0,
                scheduler_stack_bytes: 33554432,
                prover_stack_bytes: 33554432,
                guest_stack_bytes: 4194304,
                gas_to_stack_multiplier: 4,
            },
            confidential: Confidential {
                enabled: false,
                assume_valid: false,
                verifier_backend: "halo2-ipa-pallas",
                max_proof_size_bytes: 262144,
                max_nullifiers_per_tx: 8,
                max_commitments_per_tx: 8,
                max_confidential_ops_per_block: 256,
                verify_timeout: 750ms,
                max_anchor_age_blocks: 10000,
                max_proof_bytes_block: 1048576,
                max_verify_calls_per_tx: 4,
                max_verify_calls_per_block: 128,
                max_public_inputs: 32,
                reorg_depth_bound: 10000,
                policy_transition_delay_blocks: 100,
                policy_transition_window_blocks: 200,
                tree_roots_history_len: 10000,
                tree_frontier_checkpoint_interval: 100,
                registry_max_vk_entries: 64,
                registry_max_params_entries: 32,
                registry_max_delta_per_block: 4,
                gas: ConfidentialGas {
                    proof_base: 250000,
                    per_public_input: 2000,
                    per_proof_byte: 5,
                    per_nullifier: 300,
                    per_commitment: 500,
                },
            },
            crypto: Crypto {
                enable_sm_openssl_preview: false,
                sm_intrinsics: Auto,
                default_hash: "blake2b-256",
                allowed_signing: [
                    Ed25519,
                    Secp256k1,
                ],
                sm2_distid_default: "1234567812345678",
                allowed_curve_ids: [
                    1,
                    4,
                ],
            },
            settlement: Settlement {
                repo: Repo {
                    default_haircut_bps: 1500,
                    margin_frequency_secs: 86400,
                    eligible_collateral: [],
                    collateral_substitution_matrix: {},
                },
                offline: Offline {
                    hot_retention_blocks: 86400,
                    archive_batch_size: 128,
                    cold_retention_blocks: 0,
                    prune_batch_size: 128,
                    proof_mode: Optional,
                    max_receipt_age: 86400s,
                    android_trust_anchors: [],
                },
                router: Router {
                    twap_window: 60s,
                    epsilon_bps: 25,
                    buffer_alert_pct: 75,
                    buffer_throttle_pct: 25,
                    buffer_xor_only_pct: 10,
                    buffer_halt_pct: 2,
                    buffer_horizon_hours: 72,
                },
            },
            streaming: Streaming {
                key_material: StreamingKeyMaterial {
                    identity: KeyPair {
                        public_key: PublicKey(
                            ed25519(
                                "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB",
                            ),
                        ),
                        private_key: "[REDACTED PrivateKey]",
                    },
                    kyber_public: None,
                    kyber_secret: None,
                    kyber_fingerprint: None,
                    kem_suite: MlKem768,
                },
                session_store_dir: "./storage/streaming",
                feature_bits: 0,
                soranet: StreamingSoranet {
                    enabled: true,
                    exit_multiaddr: "/dns/torii/udp/9443/quic",
                    padding_budget_ms: Some(
                        25,
                    ),
                    access_kind: Authenticated,
                    channel_salt: "iroha.soranet.channel.seed.v1",
                    provision_spool_dir: "./storage/streaming/soranet_routes",
                },
                sync: StreamingSync {
                    enabled: false,
                    observe_only: true,
                    min_window_ms: 5000,
                    ewma_threshold_ms: 10,
                    hard_cap_ms: 12,
                },
                codec: StreamingCodec {
                    cabac_mode: Disabled,
                    trellis_block_sizes: [],
                    rans_tables_path: "codec/rans/tables/rans_seed0.toml",
                    entropy_mode: RansBundled,
                    bundle_width: 2,
                    bundle_accel: None,
                },
            },
        }"#]]
    .assert_eq(&format!("{config:#?}"));
}

#[test]
fn ivm_banner_defaults_enabled() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");

    assert!(config.ivm.banner.show, "banner should default to on");
    assert!(config.ivm.banner.beep, "beep should default to on");
}

#[test]
fn ivm_banner_override_applies() {
    let config =
        load_config_from_fixtures("ivm_banner_override.toml").expect("config should be valid");

    assert!(
        !config.ivm.banner.show,
        "override should disable banner rendering"
    );
    assert!(
        !config.ivm.banner.beep,
        "override should disable beep rendering"
    );
}

#[test]
fn ivm_memory_budget_profile_defaults_to_compute_profile() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");
    assert_eq!(
        config.ivm.memory_budget_profile,
        config.compute.default_resource_profile
    );
}

#[test]
fn ivm_memory_budget_profile_override_applies() {
    let config = load_config_from_fixtures("ivm_memory_budget_profile_override.toml")
        .expect("config should be valid");
    assert_eq!(
        config.ivm.memory_budget_profile,
        Name::from_str("cpu-balanced").expect("valid profile name")
    );
}

#[test]
fn torii_strict_addresses_enabled_by_default() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");
    assert!(
        config.torii.strict_addresses,
        "torii.strict_addresses should default to true so production clusters reject non-canonical aliases"
    );
}

#[test]
fn nexus_lane_requires_alias() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("   ".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_zero_axt_slot_length() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, NexusAxt};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        axt: NexusAxt {
            slot_length_ms: 0,
            max_clock_skew_ms:
                iroha_config::parameters::defaults::nexus::axt::CLOCK_SKEW_MS_DEFAULT,
            proof_cache_ttl_slots:
                iroha_config::parameters::defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots:
                iroha_config::parameters::defaults::nexus::axt::REPLAY_RETENTION_SLOTS,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_out_of_range_axt_slot_length() {
    let result = load_config_from_fixtures("bad.nexus_axt_slot_length_too_large.toml");
    assert!(
        result.is_err(),
        "slot length above guardrail must be rejected"
    );
}

#[test]
fn nexus_rejects_negative_axt_slot_length() {
    let result = load_config_from_fixtures("bad.nexus_axt_slot_length_negative.toml");
    assert!(result.is_err(), "negative slot length must be rejected");
}

#[test]
fn nexus_rejects_axt_clock_skew_above_slot_length() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, NexusAxt};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        axt: NexusAxt {
            slot_length_ms: 1_000,
            max_clock_skew_ms: 2_000,
            proof_cache_ttl_slots:
                iroha_config::parameters::defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots:
                iroha_config::parameters::defaults::nexus::axt::REPLAY_RETENTION_SLOTS,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_zero_axt_replay_retention_slots() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, NexusAxt};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        axt: NexusAxt {
            slot_length_ms: 1_000,
            max_clock_skew_ms: 0,
            proof_cache_ttl_slots:
                iroha_config::parameters::defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots: 0,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_out_of_range_axt_replay_retention_slots() {
    let result = load_config_from_fixtures("bad.nexus_axt_replay_retention_too_large.toml");
    assert!(
        result.is_err(),
        "replay retention above guardrail must be rejected"
    );
}

#[test]
fn nexus_axt_fields_load_from_fixture() {
    let config = load_config_from_fixtures("nexus_axt_full.toml").expect("config should be valid");
    assert_eq!(config.nexus.axt.slot_length_ms.get(), 1_000);
    assert_eq!(config.nexus.axt.max_clock_skew_ms, 250);
    assert_eq!(config.nexus.axt.proof_cache_ttl_slots.get(), 8);
    assert_eq!(config.nexus.axt.replay_retention_slots.get(), 256);
}

#[test]
fn nexus_multilane_requires_enable_flag() {
    let result = load_config_from_fixtures("bad.nexus_multilane_disabled.toml");
    let err = result.expect_err("multi-lane catalogs must require nexus.enabled");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("nexus.enabled"),
        "error should point at nexus.enabled being required (got {debug})"
    );
    assert!(
        debug.contains("multi-lane"),
        "error should mention multi-lane catalogs (got {debug})"
    );
}

#[test]
fn nexus_lane_overrides_rejected_when_disabled() {
    let result = load_config_from_fixtures("bad.nexus_lane_overrides_disabled.toml");
    let err = result.expect_err("lane overrides must be rejected when nexus is disabled");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("nexus.enabled"),
        "error should point at nexus.enabled being required (got {debug})"
    );
    assert!(
        debug.contains("single-lane"),
        "error should explain that overrides are ignored in single-lane mode (got {debug})"
    );
}

#[test]
fn nexus_lane_relay_emergency_requires_nexus_enabled() {
    use iroha_config::parameters::user::{LaneRelayEmergency, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: false,
        lane_relay_emergency: LaneRelayEmergency {
            enabled: true,
            ..LaneRelayEmergency::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("lane relay emergency should require nexus.enabled");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(
        debug,
        "nexus.lane_relay_emergency.enabled requires nexus.enabled = true"
    );
}

#[test]
fn nexus_lane_relay_emergency_rejects_zero_threshold() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, LaneRelayEmergency, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        lane_relay_emergency: LaneRelayEmergency {
            enabled: true,
            multisig_threshold: 0,
            multisig_members: 5,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("zero threshold must be rejected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(
        debug,
        "nexus.lane_relay_emergency.multisig_threshold must be > 0"
    );
}

#[test]
fn nexus_lane_relay_emergency_rejects_threshold_above_members() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, LaneRelayEmergency, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        lane_relay_emergency: LaneRelayEmergency {
            enabled: true,
            multisig_threshold: 6,
            multisig_members: 5,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("threshold above members must be rejected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(
        debug,
        "nexus.lane_relay_emergency.multisig_threshold 6 must be <= multisig_members 5"
    );
}

#[test]
fn nexus_profile_template_enables_multilane_defaults() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("defaults/nexus/config.toml");

    let config = ConfigReader::new()
        .read_toml_with_extends(&config_path)
        .change_context(FixtureConfigLoadError)
        .and_then(|reader| {
            reader
                .read_and_complete::<UserConfig>()
                .change_context(FixtureConfigLoadError)
        })
        .and_then(|user| user.parse().change_context(FixtureConfigLoadError))
        .expect("Nexus profile config should parse");

    assert!(
        config.nexus.enabled,
        "Nexus profile must set nexus.enabled = true"
    );
    assert_eq!(config.nexus.lane_catalog.lane_count().get(), 3);
    assert_eq!(
        config.nexus.dataspace_catalog.entries().len(),
        3,
        "profile should ship dataspace catalog entries for each lane"
    );
    let lane_aliases: Vec<_> = config
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| lane.alias.as_str())
        .collect();
    assert_eq!(lane_aliases, ["core", "governance", "zk"]);
    let dataspace_aliases: Vec<_> = config
        .nexus
        .dataspace_catalog
        .entries()
        .iter()
        .map(|entry| entry.alias.as_str())
        .collect();
    assert_eq!(dataspace_aliases, ["global", "governance", "zk"]);
    assert_eq!(config.nexus.routing_policy.rules.len(), 2);
    assert!(
        !config.nexus.lane_relay_emergency.enabled,
        "Nexus profile must leave lane relay emergency overrides disabled by default"
    );
    assert_eq!(
        config.nexus.lane_relay_emergency.multisig_threshold.get(),
        3
    );
    assert_eq!(config.nexus.lane_relay_emergency.multisig_members.get(), 5);
}

#[test]
fn lane_profile_home_applies_throttles() {
    let config = load_config_from_fixtures("home_lane_profile.toml")
        .expect("config should be valid with lane profile override");
    let expected_limits = LaneProfile::Home.derived_limits();
    let network = &config.network;

    assert_eq!(network.lane_profile, LaneProfile::Home);
    assert_eq!(network.max_incoming, expected_limits.max_incoming);
    assert_eq!(
        network.max_total_connections,
        expected_limits.max_total_connections
    );
    assert_eq!(
        network.low_priority_bytes_per_sec,
        expected_limits.low_priority_bytes_per_sec
    );
    assert_eq!(
        network.low_priority_rate_per_sec,
        expected_limits.low_priority_rate_per_sec
    );
}

#[test]
fn streaming_soranet_overrides_apply() {
    let config = load_config_from_fixtures("streaming_soranet_override.toml")
        .expect("config should load with soranet overrides");
    let soranet = &config.streaming.soranet;

    assert!(
        soranet.enabled,
        "override should keep SoraNet provisioning enabled"
    );
    assert_eq!(
        soranet.exit_multiaddr, "/dns/test-exit/quic",
        "override exit multiaddr should propagate"
    );
    assert_eq!(
        soranet.padding_budget_ms,
        Some(42),
        "override padding budget should propagate"
    );
    assert_eq!(
        soranet.access_kind,
        StreamingSoranetAccessKind::ReadOnly,
        "access policy override should convert into runtime enum"
    );
    assert_eq!(
        soranet.channel_salt, "custom.seed.v1",
        "channel salt override should propagate as provided string"
    );
}

#[test]
fn streaming_bundled_requires_build_flag() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("streaming_bundled.toml");
    result.expect("streaming_bundled config should load on bundled builds");
}

#[test]
fn streaming_bundle_width_above_tables_rejected() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("bad.streaming_bundle_width.toml");
    let err = result.expect_err("bundle width above available bundled tables must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("bundle_width"),
        "error should surface the bundle_width guard (got {debug})"
    );
    assert!(
        debug.contains("1..=3"),
        "error should report the available bundled width from the tables (got {debug})"
    );
}

#[test]
fn streaming_bundle_width_below_minimum_rejected() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("bad.streaming_bundle_width_small.toml");
    let err = result.expect_err("bundle width below minimum must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("bundle_width"),
        "error should surface the bundle_width guard (got {debug})"
    );
    assert!(
        debug.contains("at least 2"),
        "error should report the minimum bundled width requirement (got {debug})"
    );
}

#[test]
fn streaming_bundle_width_zero_rejected() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("bad.streaming_bundle_width_zero.toml");
    let err = result.expect_err("zero bundle width must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("bundle_width"),
        "error should surface the bundle_width guard (got {debug})"
    );
    assert!(
        debug.contains("1..=3"),
        "error should report the available bundled width from the tables (got {debug})"
    );
}

#[test]
fn streaming_invalid_kyber_suite_rejected() {
    let result = load_config_from_fixtures("bad.streaming_kyber_suite.toml");
    assert!(
        result.is_err(),
        "invalid streaming.kyber_suite must be rejected"
    );
}

#[test]
fn soranet_handshake_kem_suite_override() {
    let config = load_config_from_fixtures("soranet_handshake_kem_suite_override.toml")
        .expect("config should load with handshake override");
    assert_eq!(
        config.network.soranet_handshake.kem_id,
        MlKemSuite::MlKem512.kem_id(),
        "override should downshift the KEM suite id"
    );
}

#[test]
fn soranet_handshake_invalid_kem_suite_rejected() {
    let result = load_config_from_fixtures("bad.soranet_handshake_kem_suite.toml");
    assert!(
        result.is_err(),
        "invalid network.soranet_handshake.kem_suite must be rejected"
    );
}

#[test]
fn routing_policy_dataspace_resolution() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{
        DataSpaceDescriptor, LaneDescriptor, Nexus, RouteMatcher, RoutingPolicy, RoutingRule,
    };
    use iroha_config_base::util::Emitter;
    use iroha_data_model::nexus::DataSpaceId;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: None,
            description: None,
            fault_tolerance: None,
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("alpha".into()),
            rules: vec![RoutingRule {
                lane: Some(0),
                dataspace: Some("global".into()),
                matcher: RouteMatcher::default(),
            }],
        },
        ..Nexus::default()
    };

    let parsed = nexus
        .parse(&mut emitter)
        .expect("routing policy should parse");
    assert!(emitter.into_result().is_ok());

    assert_eq!(parsed.routing_policy.default_dataspace, DataSpaceId::new(1));
    assert_eq!(
        parsed.routing_policy.rules[0].dataspace,
        Some(DataSpaceId::GLOBAL)
    );
}

#[test]
fn dataspace_fault_tolerance_zero_rejected() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{DataSpaceDescriptor, LaneDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: None,
            description: None,
            fault_tolerance: Some(0),
        }],
        ..Nexus::default()
    };

    let parsed = nexus.parse(&mut emitter);
    assert!(parsed.is_none(), "fault_tolerance=0 must be rejected");
    let err = emitter.into_result().expect_err("parse error expected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "fault_tolerance must be >= 1");
}

#[test]
fn routing_policy_unknown_dataspace_rejected() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, RoutingPolicy};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("unknown".into()),
            ..RoutingPolicy::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn lane_registry_rejects_zero_poll_interval() {
    use std::{num::NonZeroU32, time::Duration};

    use iroha_config::parameters::user::{LaneDescriptor, LaneRegistryConfig, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        registry: LaneRegistryConfig {
            poll_interval_ms: Duration::from_secs(0).into(),
            ..LaneRegistryConfig::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn governance_default_module_must_exist() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{GovernanceCatalogConfig, LaneDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        governance: GovernanceCatalogConfig {
            default_module: Some("missing".into()),
            ..GovernanceCatalogConfig::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn governance_catalog_trims_and_parses_modules() {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use iroha_config::parameters::user::{
        GovernanceCatalogConfig, GovernanceModule, LaneDescriptor, Nexus,
    };
    use iroha_config_base::util::Emitter;

    let mut modules = BTreeMap::new();
    modules.insert(
        " parliament ".into(),
        GovernanceModule {
            module_type: Some(" council ".into()),
            params: {
                let mut params = BTreeMap::new();
                params.insert(" quorum ".into(), " 67 ".into());
                params
            },
        },
    );

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        governance: GovernanceCatalogConfig {
            default_module: Some("parliament".into()),
            modules,
        },
        ..Nexus::default()
    };

    let parsed = nexus.parse(&mut emitter).expect("governance should parse");
    assert!(emitter.into_result().is_ok());
    let catalog = parsed.governance;
    assert_eq!(catalog.default_module.as_deref(), Some("parliament"));
    let module = catalog
        .modules
        .get("parliament")
        .expect("module should be trimmed");
    assert_eq!(module.module_type.as_deref(), Some("council"));
    assert_eq!(module.params.get("quorum"), Some(&"67".to_string()));
}

#[test]
fn config_with_genesis() {
    let _config =
        load_config_from_fixtures("minimal_alone_with_genesis.toml").expect("should be valid");
}

#[test]
fn self_is_presented_in_trusted_peers() {
    let config =
        load_config_from_fixtures("minimal_alone_with_genesis.toml").expect("valid config");

    assert!(
        config
            .common
            .trusted_peers
            .value()
            .clone()
            .into_non_empty_vec()
            .contains(config.common.peer.id())
    );
}

#[test]
fn missing_fields() {
    let error = load_config_from_fixtures("bad.missing_fields.toml")
        .expect_err("should fail without missing fields");

    let msg = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(msg, "missing parameter: `chain`");
    assert_contains!(msg, "missing parameter: `public_key`");
    assert_contains!(msg, "missing parameter: `network.address`");
}

#[test]
fn extra_fields() {
    let error = load_config_from_fixtures("bad.extra_fields.toml")
        .expect_err("should fail with extra field");

    let msg = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(msg, "Found unrecognised parameters");
    assert_contains!(msg, "unknown parameter: `bar`");
    assert_contains!(msg, "unknown parameter: `foo`");
}

#[test]
fn ivm_memory_budget_profile_must_exist() {
    let error = load_config_from_fixtures("bad.ivm_memory_budget_profile.toml")
        .expect_err("should fail with unknown memory budget profile");
    let msg = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(msg, "ivm.memory_budget_profile");
    assert_contains!(msg, "compute.resource_profiles");
}

#[test]
fn sorafs_penalty_and_telemetry_roundtrip() {
    let config = load_config_from_fixtures("sorafs_penalty_and_telemetry.toml")
        .expect("config should parse with SoraFS governance overrides");

    let penalty = config.gov.sorafs_penalty;
    assert_eq!(penalty.utilisation_floor_bps, 7600);
    assert_eq!(penalty.uptime_floor_bps, 9650);
    assert_eq!(penalty.por_success_floor_bps, 9800);
    assert_eq!(penalty.strike_threshold, 4);
    assert_eq!(penalty.penalty_bond_bps, 1800);
    assert_eq!(penalty.cooldown_windows, 3);
    assert_eq!(penalty.max_pdp_failures, 1);
    assert_eq!(penalty.max_potr_breaches, 2);
    assert_eq!(penalty.cooldown_window_secs(1_800), 5_400);

    let telemetry = &config.gov.sorafs_telemetry;
    assert!(telemetry.require_submitter);
    assert!(telemetry.require_nonce);
    assert!(telemetry.reject_zero_capacity);
    assert_eq!(telemetry.max_window_gap, Duration::from_secs(7_200));
    let expected: Vec<_> = defaults::governance::sorafs_telemetry::submitters()
        .iter()
        .map(|id| AccountId::from_str(id).expect("default submitter must parse"))
        .collect();
    assert_eq!(telemetry.submitters, expected);
}

#[test]
fn sorafs_penalty_unknown_field_rejected() {
    let error = load_config_from_fixtures("bad.sorafs_penalty_unknown.toml")
        .expect_err("unknown penalty field should be rejected");

    let msg = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(
        msg,
        "unknown parameter: `gov.sorafs_penalty.unexpected_penalty_knob`"
    );
}

#[test]
fn sorafs_telemetry_unknown_field_rejected() {
    let error = load_config_from_fixtures("bad.sorafs_telemetry_unknown.toml")
        .expect_err("unknown telemetry field should be rejected");

    let msg = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(
        msg,
        "unknown parameter: `gov.sorafs_telemetry.unknown_submitter_field`"
    );
}

/// Aims the purpose of checking that every single provided env variable is consumed and parsed
/// into a valid config.
#[test]
fn full_envs_set_is_consumed() {
    let env = test_env_from_file(fixtures_dir().join("full.env"));

    // Read, complete, and fully parse into the actual config to ensure all
    // env-backed fields (including nested sections) are queried and consumed.
    let config = ConfigReader::new()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("should be fine to read user view")
        .parse()
        .expect("should parse into actual config");
    assert_eq!(
        config.streaming.key_material.identity().algorithm(),
        iroha_crypto::Algorithm::Ed25519
    );

    // Ensure every provided variable was consumed by the reader.
    assert_eq!(env.unvisited(), HashSet::new());
    // NOTE: The config now includes many additional env-backed knobs with defaults
    // (e.g., `PIPELINE_*`, `NORITO_*`, `ZK_*`, etc.). The reader probes them even
    // if not present in the environment. That makes `env.unknown()` non-empty in
    // this test scenario. We intentionally no longer assert on `unknown()` here.
}

#[test]
fn config_from_file_and_env() {
    let env = test_env_from_file(fixtures_dir().join("minimal_file_and_env.env"));

    ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_file_and_env.toml"))
        .expect("files are fine")
        .read_and_complete::<UserConfig>()
        .expect("should be fine")
        .parse()
        .expect("should be fine, again");
}

#[test]
fn full_config_parses_fine() {
    let cfg = load_config_from_fixtures("full.toml").expect("should be fine");
    let sorafs = &cfg.torii.sorafs_discovery;
    println!("sorafs parsed {sorafs:?}");
    assert!(
        sorafs.discovery_enabled,
        "torii.sorafs.discovery_enabled not parsed"
    );
    assert_eq!(
        sorafs.known_capabilities,
        vec![
            "torii_gateway".to_string(),
            "chunk_range_fetch".to_string(),
            "vendor_reserved".to_string()
        ]
    );
    let admission = sorafs
        .admission
        .as_ref()
        .expect("torii.sorafs.admission_envelopes_dir missing");
    assert_eq!(
        admission.envelopes_dir,
        PathBuf::from("tests/fixtures/sorafs_admission")
    );

    let alias_policy = cfg.torii.sorafs_alias_cache;
    assert_eq!(alias_policy.positive_ttl.as_secs(), 600);
    assert_eq!(alias_policy.refresh_window.as_secs(), 120);
    let storage = &cfg.torii.sorafs_storage;
    assert!(storage.enabled, "torii.sorafs.storage.enabled not parsed");
    assert_eq!(storage.data_dir, PathBuf::from("./storage/sorafs"));
    assert_eq!(storage.max_capacity_bytes.0, 107_374_182_400);
    assert_eq!(storage.max_parallel_fetches, 64);
    assert_eq!(storage.max_pins, 20000);
    assert_eq!(storage.por_sample_interval_secs, 900);
    assert_eq!(storage.alias.as_deref(), Some("tenant.alpha"));
    assert_eq!(
        storage.adverts.stake_pointer.as_deref(),
        Some("stake.pool.default")
    );
    assert_eq!(storage.adverts.availability, "warm");
    assert_eq!(storage.adverts.max_latency_ms, 750);
    assert_eq!(
        storage.adverts.topics,
        vec![
            "sorafs.sf1.primary:global".to_string(),
            "sorafs.sf1.backup:eu".to_string()
        ]
    );
}

#[test]
fn crypto_section_defaults_applied() {
    use iroha_crypto::Algorithm;

    let cfg = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("minimal config should be valid");
    let crypto = &cfg.crypto;

    assert_eq!(
        crypto.enable_sm_openssl_preview,
        defaults::crypto::ENABLE_SM_OPENSSL_PREVIEW
    );
    assert_eq!(crypto.default_hash, defaults::crypto::DEFAULT_HASH);
    assert_eq!(
        crypto.allowed_signing,
        vec![Algorithm::Ed25519, Algorithm::Secp256k1]
    );
    assert_eq!(
        crypto.sm2_distid_default,
        defaults::crypto::SM2_DISTID_DEFAULT
    );
    assert_eq!(crypto.allowed_curve_ids, vec![1, 4]);
}

#[test]
fn crypto_section_respects_env_overrides() {
    use iroha_crypto::Algorithm;

    let (default_hash, allowed_signing_env) = if cfg!(feature = "sm") {
        ("sm3-256", "ed25519,secp256k1,sm2")
    } else {
        ("blake2b-256", "ed25519,secp256k1")
    };

    let mut env = MockEnv::new()
        .set("CRYPTO_DEFAULT_HASH", default_hash)
        .set("CRYPTO_ALLOWED_SIGNING", allowed_signing_env)
        .set("CRYPTO_SM2_DISTID_DEFAULT", "CN12345678901234")
        .set("CRYPTO_CURVES_ALLOWED_IDS", "1,4");

    env = env.set(
        "CRYPTO_SM_OPENSSL_PREVIEW",
        if cfg!(feature = "sm-ffi-openssl") {
            "true"
        } else {
            "false"
        },
    );

    let cfg = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_with_trusted_peers.toml"))
        .expect("base file should be valid")
        .read_and_complete::<UserConfig>()
        .expect("user view with env overrides")
        .parse()
        .expect("actual config with env overrides");
    let crypto = &cfg.crypto;

    assert_eq!(crypto.default_hash, default_hash);
    assert_eq!(crypto.sm2_distid_default, "CN12345678901234");
    assert_eq!(
        crypto.enable_sm_openssl_preview,
        cfg!(feature = "sm-ffi-openssl")
    );

    #[cfg(feature = "sm")]
    assert_eq!(
        crypto.allowed_signing,
        vec![Algorithm::Ed25519, Algorithm::Secp256k1, Algorithm::Sm2]
    );

    #[cfg(not(feature = "sm"))]
    assert_eq!(
        crypto.allowed_signing,
        vec![Algorithm::Ed25519, Algorithm::Secp256k1]
    );

    assert_eq!(crypto.allowed_curve_ids, vec![1, 4]);
}

#[test]
fn fraud_monitoring_config_overrides_and_defaults() {
    let cfg = load_config_from_fixtures("fraud_monitoring.toml")
        .expect("fraud monitoring config should parse");
    let fraud = &cfg.fraud_monitoring;

    assert!(fraud.enabled);

    let endpoints: Vec<&str> = fraud.service_endpoints.iter().map(Url::as_str).collect();
    assert_eq!(
        endpoints,
        vec![
            "https://fraud.local/assess",
            "https://fraud.secondary/verify"
        ],
    );

    assert_eq!(
        fraud.connect_timeout,
        defaults::fraud_monitoring::CONNECT_TIMEOUT,
    );
    assert_eq!(fraud.request_timeout, Duration::from_millis(1_800));
    assert_eq!(fraud.missing_assessment_grace, Duration::from_secs(5),);
    assert_eq!(fraud.required_minimum_band, Some(FraudRiskBand::Medium));
}

#[test]
fn collectors_k_validation_propagates() {
    let env = MockEnv::new().set("SUMERAGI_COLLECTORS_K", "0");
    let report = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_with_trusted_peers.toml"))
        .expect("user config should load")
        .read_and_complete::<UserConfig>()
        .expect("user config view")
        .parse()
        .expect_err("parse should fail for invalid collectors_k");
    let message = format!("{report:?}");
    assert_contains!(message, "sumeragi.collectors_k must be greater than zero");
}

#[test]
fn da_timeout_multiplier_validation_propagates() {
    let env = MockEnv::new().set("SUMERAGI_DA_QUORUM_TIMEOUT_MULTIPLIER", "0");
    let report = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_with_trusted_peers.toml"))
        .expect("user config should load")
        .read_and_complete::<UserConfig>()
        .expect("user config view")
        .parse()
        .expect_err("parse should fail for invalid da_quorum_timeout_multiplier");
    let message = format!("{report:?}");
    assert_contains!(
        message,
        "sumeragi.da_quorum_timeout_multiplier must be greater than zero"
    );

    let env = MockEnv::new().set("SUMERAGI_DA_AVAILABILITY_TIMEOUT_MULTIPLIER", "0");
    let report = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_with_trusted_peers.toml"))
        .expect("user config should load")
        .read_and_complete::<UserConfig>()
        .expect("user config view")
        .parse()
        .expect_err("parse should fail for invalid da_availability_timeout_multiplier");
    let message = format!("{report:?}");
    assert_contains!(
        message,
        "sumeragi.da_availability_timeout_multiplier must be greater than zero"
    );
}

#[cfg(feature = "gost")]
#[test]
fn gost_config_rejects_tc26_consensus_keys() {
    let error = load_config_from_fixtures("gost_with_trusted_peers.toml")
        .expect_err("gost consensus keys should be rejected");
    let message = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(
        message,
        "public_key/private_key must be BLS-normal for consensus"
    );
}

#[test]
fn pipeline_workers_env_parses() {
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};
    use iroha_config_base::{env::MockEnv, read::ConfigReader};

    // Default: use minimal base file so required params are satisfied,
    // then ensure workers fall back to defaults (0 = auto)
    let cfg = ConfigReader::new()
        .with_env(MockEnv::new())
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user view")
        .parse();
    assert!(cfg.is_ok());

    // Override via env
    let env = MockEnv::new().set("PIPELINE_WORKERS", "7");
    let cfg2: Actual = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("read user config with env")
        .parse()
        .expect("parse actual config with env");
    assert_eq!(cfg2.pipeline.workers, 7);
}

#[test]
fn logger_level_env_accepts_lowercase() {
    use iroha_config::{
        logger::Level,
        parameters::{actual::Root as Actual, user::Root as User},
    };
    use iroha_config_base::{env::MockEnv, read::ConfigReader};

    let env = MockEnv::new().set("LOG_LEVEL", "info");
    let cfg: Actual = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user config with env")
        .parse()
        .expect("actual config with lowercase log level env");

    assert_eq!(cfg.logger.level, Level::INFO);
}

#[test]
fn sumeragi_timeout_defaults_target_one_second() {
    use defaults::sumeragi::npos;

    assert_eq!(npos::BLOCK_TIME_MS, 1_000);
    assert_eq!(npos::TIMEOUT_PROPOSE_MS, 350);
    assert_eq!(npos::TIMEOUT_PREVOTE_MS, 450);
    assert_eq!(npos::TIMEOUT_PRECOMMIT_MS, 550);
    assert_eq!(npos::TIMEOUT_COMMIT_MS, 750);
    assert_eq!(npos::TIMEOUT_DA_MS, 650);
}
// type alias used through fixtures for newer error-stack API
type Result<T, E> = core::result::Result<T, Report<E>>;

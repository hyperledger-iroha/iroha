//! Parameters default values

#![allow(
    clippy::doc_markdown,
    clippy::doc_link_with_quotes,
    clippy::assertions_on_constants
)]

use std::{
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    str::FromStr,
    time::Duration,
};

use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::{AccountId, curve::CurveId},
    asset::prelude::AssetDefinitionId,
    domain::DomainId,
    name::Name,
};
use iroha_primitives::numeric::Numeric;
use nonzero_ext::nonzero;

/// Cryptography configuration defaults.
pub mod crypto {
    use super::*;

    /// Preview toggle for OpenSSL-backed SM helpers.
    pub const ENABLE_SM_OPENSSL_PREVIEW: bool = false;
    /// Default SM intrinsic dispatch policy.
    pub const SM_INTRINSICS_POLICY: &str = "auto";
    /// Default hash algorithm identifier used when none is supplied.
    pub const DEFAULT_HASH: &str = "blake2b-256";
    /// Default signing algorithms permitted on the network.
    pub const ALLOWED_SIGNING: &[Algorithm] = &[Algorithm::Ed25519, Algorithm::Secp256k1];
    /// Default distinguishing identifier for SM2 signatures.
    pub const SM2_DISTID_DEFAULT: &str = "1234567812345678";
    /// Default set of curve identifiers permitted for account controllers.
    pub const ALLOWED_CURVE_IDS: &[u8] = &[CurveId::ED25519.as_u8(), CurveId::SECP256K1.as_u8()];

    /// Default hash algorithm identifier used when none is supplied.
    pub fn default_hash() -> String {
        DEFAULT_HASH.to_string()
    }

    /// Whether the OpenSSL-backed SM preview helpers are enabled by default.
    pub fn enable_sm_openssl_preview() -> bool {
        ENABLE_SM_OPENSSL_PREVIEW
    }

    /// Default SM intrinsic dispatch policy.
    pub fn sm_intrinsics_policy() -> String {
        SM_INTRINSICS_POLICY.to_owned()
    }

    /// Default set of signing algorithms permitted on the network.
    pub fn allowed_signing() -> Vec<Algorithm> {
        ALLOWED_SIGNING.to_vec()
    }

    /// Default set of signing algorithms expressed as strings (for env overrides).
    pub fn allowed_signing_env() -> Vec<String> {
        ALLOWED_SIGNING.iter().map(Algorithm::to_string).collect()
    }

    /// Default distinguishing identifier for SM2 signatures.
    pub fn sm2_distid_default() -> String {
        SM2_DISTID_DEFAULT.to_string()
    }

    /// Derive the curve capability list from the supplied signing algorithms.
    pub fn derive_curve_ids_from_algorithms(algorithms: &[Algorithm]) -> Vec<u8> {
        let mut ids: Vec<u8> = algorithms
            .iter()
            .filter_map(|algo| CurveId::try_from_algorithm(*algo).ok())
            .map(CurveId::as_u8)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Default set of curve identifiers permitted on the network.
    pub fn allowed_curve_ids() -> Vec<u8> {
        derive_curve_ids_from_algorithms(ALLOWED_SIGNING)
    }
}

/// Common configuration defaults shared across components.
pub mod common {
    /// Default domain label used when configuration omits the AccountAddress selector override.
    pub fn default_account_domain_label() -> String {
        iroha_data_model::account::address::DEFAULT_DOMAIN_NAME.to_owned()
    }

    /// Default chain discriminant / I105 network prefix (Sora Nexus global).
    pub const CHAIN_DISCRIMINANT: u16 = 0x02F1;

    /// Chain discriminant applied when configuration omits an override.
    pub const fn chain_discriminant() -> u16 {
        CHAIN_DISCRIMINANT
    }
}

/// IVM- and banner-related defaults.
pub mod ivm {
    use super::Name;

    /// Startup banner settings.
    pub mod banner {
        /// Show startup banners by default.
        pub const SHOW: bool = true;
        /// Play the startup beep by default (when built with the `beep` feature).
        pub const BEEP: bool = true;

        /// Whether the banner is shown by default.
        pub const fn show() -> bool {
            SHOW
        }

        /// Whether the startup beep is enabled by default.
        pub const fn beep() -> bool {
            BEEP
        }
    }

    /// Default compute resource profile used to cap IVM guest stack budgets.
    pub fn memory_budget_profile() -> Name {
        super::compute::default_resource_profile()
    }
}

/// Genesis bootstrap defaults.
pub mod genesis {
    use iroha_config_base::util::Bytes;

    use super::*;

    /// Maximum size (bytes) of a genesis payload served or accepted during bootstrap.
    pub const BOOTSTRAP_MAX_BYTES: Bytes<u64> = Bytes(16 * 1024 * 1024);
    /// Minimum interval between serving genesis responses to avoid abuse.
    pub const BOOTSTRAP_RESPONSE_THROTTLE: Duration = Duration::from_secs(1);
    /// Timeout for each bootstrap request round-trip (preflight or payload).
    pub const BOOTSTRAP_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
    /// Base retry/backoff interval between bootstrap attempts.
    pub const BOOTSTRAP_RETRY_INTERVAL: Duration = Duration::from_secs(1);
    /// Maximum number of bootstrap attempts before failing startup.
    pub const BOOTSTRAP_MAX_ATTEMPTS: u32 = 5;
}

/// Pending-transaction queue defaults used by consensus and Torii.
pub mod queue {
    use super::*;

    /// Maximum number of transactions the global queue holds concurrently.
    pub const CAPACITY: NonZeroUsize = nonzero!(2_usize.pow(16));
    /// Maximum number of transactions accepted per authority (prevents flooding).
    pub const CAPACITY_PER_USER: NonZeroUsize = nonzero!(2_usize.pow(16));
    /// Time-to-live for queued transactions before automatic eviction.
    pub const TRANSACTION_TIME_TO_LIVE: Duration = Duration::from_hours(24);
    /// Minimum interval between expired-transaction sweeps.
    pub const EXPIRED_CULL_INTERVAL: Duration = Duration::from_secs(1);
    /// Maximum number of entries scanned per expired-transaction sweep.
    pub const EXPIRED_CULL_BATCH: NonZeroUsize = nonzero!(256_usize);
}

/// Transaction admission defaults enforced at pipeline ingress.
pub mod transaction {
    use super::*;

    /// Maximum signatures accepted on a transaction payload.
    pub const fn max_signatures() -> NonZeroU64 {
        nonzero!(16_u64)
    }

    /// Maximum instructions allowed in a transaction payload.
    pub const fn max_instructions() -> NonZeroU64 {
        // `v1/contracts/call` wrapper transactions for dpn_sora_nexus can exceed 20k
        // decoded IVM instructions; keep a conservative margin above current payloads.
        nonzero!(50_000_u64)
    }

    /// Maximum Kotodama bytecode length (bytes) allowed during admission.
    pub const fn ivm_bytecode_size() -> NonZeroU64 {
        nonzero!(4 * 2_u64.pow(20))
    }
}

/// Compute lane defaults.
pub mod compute {
    use std::str::FromStr;

    use iroha_config_base::util::Bytes;
    use iroha_data_model::{
        compute::{
            ComputeAuthPolicy, ComputeFeeSplit, ComputePriceAmplifiers, ComputePriceDeltaBounds,
            ComputePriceRiskClass, ComputePriceWeights, ComputeRandomnessPolicy,
            ComputeResourceBudget, ComputeSandboxMode, ComputeSandboxRules, ComputeSponsorPolicy,
            ComputeStorageAccess,
        },
        name::Name,
    };

    use super::*;

    /// Whether the compute lane is enabled by default.
    pub const ENABLED: bool = false;
    /// Default TTL (slots) applied to compute calls.
    pub const fn default_ttl_slots() -> NonZeroU64 {
        nonzero!(32_u64)
    }
    /// Maximum TTL (slots) accepted for compute calls.
    pub const fn max_ttl_slots() -> NonZeroU64 {
        nonzero!(512_u64)
    }
    /// Maximum request payload size (bytes).
    pub const MAX_REQUEST_BYTES: Bytes<u64> = Bytes(512 * 1024);
    /// Maximum response payload size (bytes).
    pub const MAX_RESPONSE_BYTES: Bytes<u64> = Bytes(512 * 1024);
    /// Default per-call gas limit.
    pub const fn max_gas_per_call() -> NonZeroU64 {
        nonzero!(5_000_000_u64)
    }
    /// Maximum compute units that may be charged per call.
    pub const fn max_cu_per_call() -> NonZeroU64 {
        nonzero!(100_000_u64)
    }
    /// Maximum amplification ratio (response/ingress) permitted for compute calls.
    pub const fn max_amplification_ratio() -> NonZeroU32 {
        nonzero!(16_u32)
    }
    /// Maximum concurrent in-flight calls per route used by the gateway.
    pub const fn max_inflight_per_route() -> NonZeroUsize {
        nonzero!(32_usize)
    }
    /// Maximum queued requests per route (beyond in-flight).
    pub const fn queue_depth_per_route() -> NonZeroUsize {
        nonzero!(512_usize)
    }
    /// Maximum allowed requests per second (token-bucket rate limit).
    pub const fn max_requests_per_second() -> NonZeroU32 {
        nonzero!(200_u32)
    }
    /// Target p50 latency budget in milliseconds for compute calls.
    pub const fn target_p50_latency_ms() -> NonZeroU64 {
        nonzero!(25_u64)
    }
    /// Target p95 latency budget in milliseconds for compute calls.
    pub const fn target_p95_latency_ms() -> NonZeroU64 {
        nonzero!(75_u64)
    }
    /// Target p99 latency budget in milliseconds for compute calls.
    pub const fn target_p99_latency_ms() -> NonZeroU64 {
        nonzero!(120_u64)
    }

    fn name(value: &str) -> Name {
        Name::from_str(value).expect("default compute name")
    }

    /// Default namespace allowlist.
    pub fn default_namespaces() -> Vec<Name> {
        vec![name("compute")]
    }

    /// Default resource profile name.
    pub fn default_resource_profile() -> Name {
        name("cpu-small")
    }

    /// Default price family identifier.
    pub fn default_price_family() -> Name {
        name("default")
    }

    /// Default resource profiles shipped with the node.
    pub fn resource_profiles() -> BTreeMap<Name, ComputeResourceBudget> {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            default_resource_profile(),
            ComputeResourceBudget {
                max_cycles: nonzero!(5_000_000_u64),
                max_memory_bytes: nonzero!(128 * 1024 * 1024_u64),
                max_stack_bytes: nonzero!(2 * 1024 * 1024_u64),
                max_io_bytes: nonzero!(16 * 1024 * 1024_u64),
                max_egress_bytes: nonzero!(8 * 1024 * 1024_u64),
                allow_gpu_hints: false,
                allow_wasi: false,
            },
        );
        profiles.insert(
            name("cpu-balanced"),
            ComputeResourceBudget {
                max_cycles: nonzero!(10_000_000_u64),
                max_memory_bytes: nonzero!(256 * 1024 * 1024_u64),
                max_stack_bytes: nonzero!(4 * 1024 * 1024_u64),
                max_io_bytes: nonzero!(24 * 1024 * 1024_u64),
                max_egress_bytes: nonzero!(12 * 1024 * 1024_u64),
                allow_gpu_hints: true,
                allow_wasi: true,
            },
        );
        profiles
    }

    /// Default price families mapping cycles + egress into compute units.
    pub fn price_families() -> BTreeMap<Name, ComputePriceWeights> {
        let mut families = BTreeMap::new();
        families.insert(
            default_price_family(),
            ComputePriceWeights {
                cycles_per_unit: nonzero!(1_000_000_u64),
                egress_bytes_per_unit: nonzero!(1024_u64),
                unit_label: "cu".to_string(),
            },
        );
        families
    }

    /// Default risk class mapping for price families.
    pub fn price_risk_classes() -> BTreeMap<Name, ComputePriceRiskClass> {
        let mut classes = BTreeMap::new();
        classes.insert(default_price_family(), ComputePriceRiskClass::Balanced);
        classes
    }

    /// Default delta bounds applied per risk class (basis points).
    pub fn price_bounds() -> BTreeMap<ComputePriceRiskClass, ComputePriceDeltaBounds> {
        let mut bounds = BTreeMap::new();
        bounds.insert(
            ComputePriceRiskClass::Low,
            ComputePriceDeltaBounds {
                max_cycles_delta_bps: nonzero!(500_u16),
                max_egress_delta_bps: nonzero!(500_u16),
            },
        );
        bounds.insert(
            ComputePriceRiskClass::Balanced,
            ComputePriceDeltaBounds {
                max_cycles_delta_bps: nonzero!(1_500_u16),
                max_egress_delta_bps: nonzero!(1_500_u16),
            },
        );
        bounds.insert(
            ComputePriceRiskClass::High,
            ComputePriceDeltaBounds {
                max_cycles_delta_bps: nonzero!(3_000_u16),
                max_egress_delta_bps: nonzero!(3_000_u16),
            },
        );
        bounds
    }

    /// Default fee split applied to compute unit charges (basis points).
    pub fn fee_split() -> ComputeFeeSplit {
        ComputeFeeSplit {
            burn_bps: 2_000,
            validators_bps: 6_000,
            providers_bps: 2_000,
        }
    }

    /// Default sponsor policy caps for subsidised compute requests.
    pub fn sponsor_policy() -> ComputeSponsorPolicy {
        ComputeSponsorPolicy {
            max_cu_per_call: nonzero!(10_000_u64),
            max_daily_cu: nonzero!(100_000_u64),
        }
    }

    /// Price amplifiers applied for GPU/TEE/best-effort execution.
    pub fn price_amplifiers() -> ComputePriceAmplifiers {
        ComputePriceAmplifiers::default()
    }

    /// Default sandbox rules for compute execution.
    pub fn sandbox_rules() -> ComputeSandboxRules {
        ComputeSandboxRules {
            mode: ComputeSandboxMode::IvmOnly,
            randomness: ComputeRandomnessPolicy::SeededFromRequest,
            storage: ComputeStorageAccess::ReadOnly,
            deny_nondeterministic_syscalls: true,
            allow_gpu_hints: false,
            allow_tee_hints: false,
        }
    }

    /// Default authentication policy for compute routes.
    pub const fn default_auth_policy() -> ComputeAuthPolicy {
        ComputeAuthPolicy::Either
    }
}

/// Content lane defaults.
pub mod content {
    use iroha_data_model::da::prelude::DaStripeLayout;

    /// Maximum tarball size accepted for a single content bundle (bytes).
    pub const MAX_BUNDLE_BYTES: u64 = 1_048_576;
    /// Maximum number of files in a bundle.
    pub const MAX_FILES: u32 = 128;
    /// Maximum allowed path length per file.
    pub const MAX_PATH_LEN: u32 = 256;
    /// Maximum retention window (blocks) for an expiring bundle.
    pub const MAX_RETENTION_BLOCKS: u64 = 10_000;
    /// Chunk size (bytes) used when ingesting tarballs.
    pub const CHUNK_SIZE_BYTES: u32 = 64 * 1024;
    /// Default Cache-Control max-age for content bundles (seconds).
    pub const DEFAULT_CACHE_MAX_AGE_SECS: u32 = 300;
    /// Ceiling for Cache-Control max-age (seconds).
    pub const MAX_CACHE_MAX_AGE_SECS: u32 = 86_400;
    /// Force immutable cache-control by default.
    pub const IMMUTABLE_BUNDLES: bool = true;
    /// Maximum served requests per second for the content gateway.
    pub const MAX_REQUESTS_PER_SECOND: u32 = 200;
    /// Maximum served egress bytes per second for the content gateway.
    pub const MAX_EGRESS_BYTES_PER_SECOND: u32 = 16 * 1024 * 1024;
    /// Target p50 latency (milliseconds) for content responses.
    pub const TARGET_P50_LATENCY_MS: u32 = 50;
    /// Target p99 latency (milliseconds) for content responses.
    pub const TARGET_P99_LATENCY_MS: u32 = 250;
    /// Burst size for the content request token bucket.
    pub const REQUEST_BURST: u32 = 200;
    /// Burst size for the egress token bucket.
    pub const EGRESS_BURST_BYTES: u64 = 8 * 1024 * 1024;
    /// Target availability in basis points (10000 = 100%).
    pub const TARGET_AVAILABILITY_BPS: u32 = 9_990;
    /// Default PoW difficulty (leading zero bits) when enabled for content fetches.
    pub const POW_DIFFICULTY_BITS: u8 = 0;
    /// Default header name carrying PoW nonces.
    pub fn default_pow_header() -> String {
        "x-iroha-pow".to_string()
    }
    /// Default DA stripe layout used for content bundles.
    pub const fn default_stripe_layout() -> DaStripeLayout {
        DaStripeLayout {
            total_stripes: 1,
            shards_per_stripe: 1,
            row_parity_stripes: 0,
        }
    }
    /// Default auth mode string for content lane ("public" | "role:<role>" | "sponsor:<uaid>").
    pub fn default_auth_mode() -> String {
        "public".to_string()
    }
}

/// Oracle pipeline defaults.
pub mod oracle {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::prelude::Name;

    use super::*;

    fn deterministic_account(seed: &[u8], domain: &str) -> AccountId {
        let _domain_id = DomainId::from_str(domain).expect("default oracle domain");
        let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    /// Fixed reward amount for an inlier observation.
    pub fn reward_amount() -> Numeric {
        Numeric::from_str("1").expect("default oracle reward amount")
    }

    /// Maximum retained feed-history entries per oracle feed.
    pub const fn history_depth() -> NonZeroUsize {
        nonzero!(2_048usize)
    }

    /// Asset credited to providers when observations are accepted.
    pub fn reward_asset() -> AssetDefinitionId {
        AssetDefinitionId::from_str("xor#sora").expect("default oracle reward asset id")
    }

    /// Account debited to fund oracle provider rewards.
    pub fn reward_pool() -> AccountId {
        deterministic_account(b"oracle-reward-pool", "sora")
    }

    /// Asset debited when slashing oracle providers.
    pub fn slash_asset() -> AssetDefinitionId {
        AssetDefinitionId::from_str("xor#sora").expect("default oracle slash asset id")
    }

    /// Account credited when penalties are collected.
    pub fn slash_receiver() -> AccountId {
        deterministic_account(b"oracle-slash-receiver", "sora")
    }

    /// Penalty applied to outlier observations.
    pub fn slash_outlier_amount() -> Numeric {
        Numeric::from_str("1").expect("default oracle outlier penalty")
    }

    /// Penalty applied to explicit error observations.
    pub fn slash_error_amount() -> Numeric {
        Numeric::from_str("1").expect("default oracle error penalty")
    }

    /// Penalty applied when a provider misses a slot.
    pub fn slash_no_show_amount() -> Numeric {
        Numeric::from_str("1").expect("default oracle no-show penalty")
    }

    /// Bond asset required when opening a dispute.
    pub fn dispute_bond_asset() -> AssetDefinitionId {
        AssetDefinitionId::from_str("xor#sora").expect("default oracle dispute bond asset")
    }

    /// Bond amount required to open a dispute.
    pub fn dispute_bond_amount() -> Numeric {
        Numeric::from_str("2").expect("default oracle dispute bond amount")
    }

    /// Reward paid to a successful challenger.
    pub fn dispute_reward_amount() -> Numeric {
        Numeric::from_str("1").expect("default oracle dispute reward")
    }

    /// Penalty charged for frivolous disputes.
    pub fn frivolous_slash_amount() -> Numeric {
        Numeric::from_str("1").expect("default oracle frivolous penalty")
    }

    /// SLA (blocks) for intake stage.
    pub const fn intake_sla_blocks() -> u64 {
        12
    }

    /// SLA (blocks) for rules committee stage.
    pub const fn rules_sla_blocks() -> u64 {
        24
    }

    /// SLA (blocks) for COP review stage.
    pub const fn cop_sla_blocks() -> u64 {
        36
    }

    /// SLA (blocks) for technical audit stage.
    pub const fn technical_sla_blocks() -> u64 {
        36
    }

    /// SLA (blocks) for policy jury stage.
    pub const fn policy_jury_sla_blocks() -> u64 {
        48
    }

    /// SLA (blocks) for enactment stage.
    pub const fn enact_sla_blocks() -> u64 {
        48
    }

    /// Intake approvals required.
    pub const fn intake_min_votes() -> NonZeroUsize {
        nonzero!(1_usize)
    }

    /// Rules committee approvals required.
    pub const fn rules_min_votes() -> NonZeroUsize {
        nonzero!(1_usize)
    }

    /// COP approvals required for low-class changes.
    pub const fn cop_low_votes() -> NonZeroUsize {
        nonzero!(1_usize)
    }

    /// COP approvals required for medium-class changes.
    pub const fn cop_medium_votes() -> NonZeroUsize {
        nonzero!(2_usize)
    }

    /// COP approvals required for high-class changes.
    pub const fn cop_high_votes() -> NonZeroUsize {
        nonzero!(3_usize)
    }

    /// Technical audit approvals required.
    pub const fn technical_min_votes() -> NonZeroUsize {
        nonzero!(2_usize)
    }

    /// Policy jury approvals required for low-class changes.
    pub const fn policy_jury_low_votes() -> NonZeroUsize {
        nonzero!(2_usize)
    }

    /// Policy jury approvals required for medium-class changes.
    pub const fn policy_jury_medium_votes() -> NonZeroUsize {
        nonzero!(3_usize)
    }

    /// Policy jury approvals required for high-class changes.
    pub const fn policy_jury_high_votes() -> NonZeroUsize {
        nonzero!(4_usize)
    }

    /// Feed identifier expected for twitter follow attestations.
    pub fn twitter_binding_feed_id() -> Name {
        Name::from_str(iroha_data_model::oracle::TWITTER_FOLLOW_FEED_ID)
            .expect("default twitter binding feed id")
    }

    /// Pepper identifier used for keyed twitter hashes.
    pub fn twitter_binding_pepper_id() -> String {
        "pepper-social-v1".to_string()
    }

    /// Maximum TTL (milliseconds) accepted for twitter binding attestations.
    pub const fn twitter_binding_max_ttl_ms() -> u64 {
        86_400_000 // 24h
    }

    /// Minimum TTL (milliseconds) accepted for twitter binding attestations.
    pub const fn twitter_binding_min_ttl_ms() -> u64 {
        300_000 // 5 minutes
    }

    /// Minimum spacing (milliseconds) between attestations for the same binding hash.
    pub const fn twitter_binding_min_update_spacing_ms() -> u64 {
        30_000 // 30s
    }
}

/// Kura block-store defaults.
pub mod kura {
    use std::{num::NonZeroUsize, time::Duration};

    use nonzero_ext::nonzero;

    use iroha_config_base::util::Bytes;

    use crate::kura::FsyncMode;

    /// Directory for Kura storage relative to the node working directory.
    pub const STORE_DIR: &str = "./storage";
    /// Number of blocks cached in memory to accelerate lookups.
    pub const BLOCKS_IN_MEMORY: NonZeroUsize = nonzero!(1024_usize);
    /// Number of recent roster records retained for block-sync validation.
    pub const BLOCK_SYNC_ROSTER_RETENTION: NonZeroUsize = nonzero!(7_200_usize);
    /// Number of recent roster sidecars retained alongside the block store.
    pub const ROSTER_SIDECAR_RETENTION: NonZeroUsize = nonzero!(512_usize);
    /// Default number of merge-ledger entries cached in memory.
    pub const MERGE_LEDGER_CACHE_CAPACITY: usize = 256;
    /// Default fsync policy for block persistence.
    pub const FSYNC_MODE: FsyncMode = FsyncMode::Batched;
    /// Default batching interval for fsync operations.
    pub const FSYNC_INTERVAL: Duration = Duration::from_millis(50);
    /// Maximum on-disk footprint allowed for Kura (0 = unlimited).
    pub const MAX_DISK_USAGE_BYTES: Bytes<u64> = Bytes(0);
}

/// P2P networking defaults covering gossip, framing, and socket behavior.
pub mod network {
    use super::*;
    /// Default lane profile applied to networking presets.
    pub mod lane_profile {
        /// Default lane profile label for datacenter/validator deployments.
        pub const DEFAULT: &str = "core";
        /// Default lane profile label as owned string for config defaults.
        pub fn default_label() -> String {
            DEFAULT.to_string()
        }
        /// Core profile scheduler tick (milliseconds) for shaping calculations.
        pub const CORE_TICK_MS: u16 = 5;
        /// Core profile recommended MTU (bytes) for p2p traffic.
        pub const CORE_MTU_BYTES: u16 = 1500;
        /// Core profile target uplink budget (bits per second) used when deriving per-peer caps.
        pub const CORE_UPLINK_BPS: u64 = 1_200_000_000;
        /// Core profile constant-rate neighbor budget.
        pub const CORE_CONSTANT_NEIGHBORS: usize = 48;
        /// Core profile soft cap for total connections.
        pub const CORE_MAX_TOTAL_CONNECTIONS: usize = 120;
        /// Core profile soft cap for inbound connections.
        pub const CORE_MAX_INCOMING: usize = 48;

        /// Home profile scheduler tick (milliseconds) for shaping calculations.
        pub const HOME_TICK_MS: u16 = 10;
        /// Home profile recommended MTU (bytes) for p2p traffic.
        pub const HOME_MTU_BYTES: u16 = 1400;
        /// Home profile target uplink budget (bits per second) used when deriving per-peer caps.
        pub const HOME_UPLINK_BPS: u64 = 120_000_000;
        /// Home profile constant-rate neighbor budget.
        pub const HOME_CONSTANT_NEIGHBORS: usize = 12;
        /// Home profile soft cap for total connections.
        pub const HOME_MAX_TOTAL_CONNECTIONS: usize = 32;
        /// Home profile soft cap for inbound connections.
        pub const HOME_MAX_INCOMING: usize = 12;
    }

    /// Interval between transaction gossip batches.
    pub const TRANSACTION_GOSSIP_PERIOD: Duration = Duration::from_secs(1);
    /// Number of gossip ticks to wait before re-sending the same transactions.
    pub const TRANSACTION_GOSSIP_RESEND_TICKS: NonZeroU32 = nonzero!(3u32);
    /// Number of transactions gossiped per batch.
    pub const TRANSACTION_GOSSIP_SIZE: NonZeroU32 = nonzero!(500u32);
    /// Drop transaction gossip for dataspaces that are missing from the lane catalog instead of
    /// falling back to restricted targeting.
    pub const TX_GOSSIP_DROP_UNKNOWN_DATASPACE: bool = false;
    /// Optional cap on restricted-dataspace gossip targets (None = commit topology fanout).
    pub const TX_GOSSIP_RESTRICTED_TARGET_CAP: Option<NonZeroUsize> = None;
    /// Optional cap on public-dataspace gossip targets (None = broadcast; default = 16).
    pub const TX_GOSSIP_PUBLIC_TARGET_CAP: Option<NonZeroUsize> = Some(nonzero!(16_usize));
    /// Interval between reshuffles of public gossip target selection.
    pub const TX_GOSSIP_PUBLIC_TARGET_RESHUFFLE: Duration = TRANSACTION_GOSSIP_PERIOD;
    /// Interval between reshuffles of restricted gossip target selection.
    pub const TX_GOSSIP_RESTRICTED_TARGET_RESHUFFLE: Duration = TRANSACTION_GOSSIP_PERIOD;
    /// Fallback strategy for restricted gossip when no targets are available (`drop`|`public_overlay`).
    pub const TX_GOSSIP_RESTRICTED_FALLBACK: &str = "drop";
    /// Policy for handling restricted payloads when only the public overlay is available (`refuse`|`forward`).
    pub const TX_GOSSIP_RESTRICTED_PUBLIC_PAYLOAD: &str = "refuse";
    /// Interval between peer gossip batches.
    pub const PEER_GOSSIP_PERIOD: Duration = Duration::from_secs(1);
    /// Maximum interval between peer gossip batches (change-driven gossip backs off toward this).
    pub const PEER_GOSSIP_MAX_PERIOD: Duration = Duration::from_secs(30);

    /// Interval between block gossip batches.
    pub const BLOCK_GOSSIP_PERIOD: Duration = Duration::from_secs(10);
    /// Maximum interval between block gossip batches (idle backoff ceiling).
    pub const BLOCK_GOSSIP_MAX_PERIOD: Duration = Duration::from_secs(30);
    /// Number of blocks gossiped per batch.
    pub const BLOCK_GOSSIP_SIZE: NonZeroU32 = nonzero!(4u32);

    /// Trust decay half-life applied to gossip scores.
    pub const TRUST_DECAY_HALF_LIFE: Duration = Duration::from_secs(300);
    /// Whether trust gossip capability is advertised/enabled by default.
    pub const TRUST_GOSSIP: bool = true;
    /// Penalty applied for invalid/bad gossip (absolute score delta).
    pub const TRUST_PENALTY_BAD_GOSSIP: i32 = 5;
    /// Penalty applied when peers gossip about unknown/invalid peers.
    pub const TRUST_PENALTY_UNKNOWN_PEER: i32 = 3;
    /// Minimum trust score allowed before gossip is ignored.
    pub const TRUST_MIN_SCORE: i32 = -20;

    /// Idle timeout before disconnecting an inactive peer.
    ///
    /// Keep this comfortably above the typical integration-test runtime so peers do not churn
    /// before they exchange their first gossip/status messages.
    pub const IDLE_TIMEOUT: Duration = Duration::from_mins(5);
    /// Delay outbound peer dials after startup.
    pub const CONNECT_STARTUP_DELAY: Duration = Duration::from_millis(0);
    /// Timeout applied to an individual outbound dial attempt (TCP/TLS/QUIC/WS).
    pub const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
    /// Maximum age for deferred outbound frames queued while peer session is missing.
    pub const DEFERRED_SEND_TTL_MS: u64 = 1_500;
    /// Maximum deferred outbound frames retained per peer while session is missing.
    pub const DEFERRED_SEND_MAX_PER_PEER: usize = 256;
    /// Idle timeout before expiring accept throttle buckets.
    pub const ACCEPT_BUCKET_IDLE: Duration = Duration::from_mins(10);
    /// Maximum number of accept throttle buckets to retain.
    pub const MAX_ACCEPT_BUCKETS: NonZeroUsize = nonzero!(4096_usize);
    /// Prefix length used for IPv4 accept prefix buckets.
    pub const ACCEPT_PREFIX_V4_BITS: u8 = 24;
    /// Prefix length used for IPv6 accept prefix buckets.
    pub const ACCEPT_PREFIX_V6_BITS: u8 = 64;
    /// Default stagger between parallel dial attempts for multiple addresses (Happy Eyeballs)
    pub const HAPPY_EYEBALLS_STAGGER: Duration = Duration::from_millis(100);

    // QUIC datagram settings (best-effort gossip/health delivery).
    /// Whether QUIC DATAGRAM support is enabled when QUIC transport is in use.
    ///
    /// Datagrams are used only for best-effort topics; reliable topics keep using streams.
    pub const QUIC_DATAGRAMS_ENABLED: bool = true;
    /// Upper bound (bytes) for a single QUIC datagram payload.
    ///
    /// Chosen conservatively to avoid IP fragmentation on typical Internet paths.
    pub const QUIC_DATAGRAM_MAX_PAYLOAD_BYTES: NonZeroUsize = nonzero!(1200_usize);
    /// Total receive buffer reserved for QUIC datagrams (bytes).
    pub const QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES: NonZeroUsize = nonzero!(1_048_576_usize);
    /// Total send buffer reserved for QUIC datagrams (bytes).
    pub const QUIC_DATAGRAM_SEND_BUFFER_BYTES: NonZeroUsize = nonzero!(1_048_576_usize);

    /// Enable SCION-guided outbound peer dialing.
    ///
    /// When enabled and a route exists for a peer, the dialer attempts the SCION
    /// route first before legacy address dialing.
    pub const SCION_ENABLED: bool = false;
    /// Allow fallback to legacy dialing when a SCION route is missing or fails.
    pub const SCION_FALLBACK_TO_LEGACY: bool = true;

    // P2P bounded queue capacities (always enforced)
    // Defaults tuned for ~20,000 TPS environments: prioritize headroom for gossip/low-priority
    // traffic while keeping consensus/control queues responsive.
    /// Capacity for priority queues fed by high-importance messages.
    pub const P2P_QUEUE_CAP_HIGH: NonZeroUsize = nonzero!(8192_usize);
    /// Capacity for lower-importance queues (e.g., gossip bursts).
    pub const P2P_QUEUE_CAP_LOW: NonZeroUsize = nonzero!(32768_usize);
    /// Capacity for post-queue tasks (per topic).
    pub const P2P_POST_QUEUE_CAP: NonZeroUsize = nonzero!(2048_usize);
    /// Capacity for the inbound P2P subscriber queue feeding the node relay.
    pub const P2P_SUBSCRIBER_QUEUE_CAP: NonZeroUsize = nonzero!(8192_usize);

    /// Optional per-peer consensus ingress rate (msgs/sec). When None, consensus ingress limiting is disabled.
    ///
    /// Defaults tuned for liveness-sensitive traffic without allowing abusive bursts.
    pub const CONSENSUS_INGRESS_RATE_PER_SEC: Option<NonZeroU32> = Some(nonzero!(300_u32));
    /// Optional burst for consensus ingress rate limiting (msgs). Defaults to `rate` when None.
    pub const CONSENSUS_INGRESS_BURST: Option<NonZeroU32> = Some(nonzero!(300_u32));
    /// Optional per-peer consensus ingress bytes/sec budget. When None, bytes limiting is disabled.
    pub const CONSENSUS_INGRESS_BYTES_PER_SEC: Option<NonZeroU32> = Some(nonzero!(67_108_864_u32)); // 64 MiB/s
    /// Optional burst size in bytes for consensus ingress limiting. Defaults to `bytes_per_sec` when None.
    pub const CONSENSUS_INGRESS_BYTES_BURST: Option<NonZeroU32> = Some(nonzero!(134_217_728_u32)); // 128 MiB
    /// Optional per-peer critical consensus ingress rate (msgs/sec). When None, critical limiting is disabled.
    ///
    /// Critical traffic is liveness-sensitive (votes/QCs/VRF/RBC signals) and uses a dedicated cap.
    pub const CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC: Option<NonZeroU32> = Some(nonzero!(300_u32));
    /// Optional burst for critical consensus ingress rate limiting (msgs). Defaults to `rate` when None.
    pub const CONSENSUS_INGRESS_CRITICAL_BURST: Option<NonZeroU32> = Some(nonzero!(300_u32));
    /// Optional per-peer critical consensus ingress bytes/sec budget. When None, bytes limiting is disabled.
    pub const CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC: Option<NonZeroU32> =
        Some(nonzero!(134_217_728_u32)); // 128 MiB/s
    /// Optional burst size in bytes for critical consensus ingress limiting. Defaults to `bytes_per_sec` when None.
    pub const CONSENSUS_INGRESS_CRITICAL_BYTES_BURST: Option<NonZeroU32> =
        Some(nonzero!(268_435_456_u32)); // 256 MiB
    /// Maximum concurrent RBC sessions accepted per peer before throttling (0 disables).
    pub const CONSENSUS_INGRESS_RBC_SESSION_LIMIT: usize = 64;
    /// Drop threshold (per window) before temporarily suppressing consensus ingress.
    pub const CONSENSUS_INGRESS_PENALTY_THRESHOLD: u32 = 32;
    /// Window size (ms) for consensus ingress penalty tracking.
    pub const CONSENSUS_INGRESS_PENALTY_WINDOW_MS: u64 = 5_000;
    /// Cooldown (ms) applied after consensus ingress penalties trigger.
    pub const CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS: u64 = 10_000;

    // Optional DNS hostname refresh interval (None disables). Default 5 minutes.
    /// Interval between DNS resolution refreshes for peer hostnames.
    pub const DNS_REFRESH_INTERVAL: Duration = Duration::from_mins(5);

    // Disconnect peers when their per-topic bounded post channel overflows
    /// Whether to disconnect peers that overflow their per-topic queues.
    pub const DISCONNECT_ON_POST_OVERFLOW: bool = true;
    /// Default hop limit for relayed frames.
    pub const RELAY_TTL: u8 = 8;

    // Maximum allowed Norito frame size for peer messages (bytes)
    /// Maximum Norito frame size for peer messages in bytes (16 MiB default).
    pub const MAX_FRAME_BYTES: NonZeroUsize = nonzero!(16 * 1024 * 1024_usize); // 16 MiB default
    // Per-topic caps (defaults stricter than global except BlockSync)
    /// Maximum frame size for consensus control traffic.
    ///
    /// Consensus certificates and READY bundles can scale with validator set size, so keep
    /// this aligned with the global frame cap to avoid dropping liveness-critical frames.
    pub const MAX_FRAME_BYTES_CONSENSUS: NonZeroUsize = MAX_FRAME_BYTES;
    /// Maximum frame size for control-plane messages.
    pub const MAX_FRAME_BYTES_CONTROL: NonZeroUsize = nonzero!(131_072_usize);
    /// Maximum frame size for block sync / consensus payload traffic.
    pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = MAX_FRAME_BYTES; // allow full frame
    /// Maximum frame size for transaction gossip.
    pub const MAX_FRAME_BYTES_TX_GOSSIP: NonZeroUsize = nonzero!(262_144_usize); // 256 KiB
    /// Maximum frame size for peer gossip.
    pub const MAX_FRAME_BYTES_PEER_GOSSIP: NonZeroUsize = nonzero!(65_536_usize); // 64 KiB
    /// Maximum frame size for health-check channel messages.
    pub const MAX_FRAME_BYTES_HEALTH: NonZeroUsize = nonzero!(32_768_usize); // 32 KiB
    /// Maximum frame size for other miscellaneous topics.
    pub const MAX_FRAME_BYTES_OTHER: NonZeroUsize = nonzero!(131_072_usize); // 128 KiB
    // TCP options
    /// Whether to enable TCP_NODELAY for reduced latency.
    pub const TCP_NODELAY: bool = true;
    /// Default TCP keepalive (recommended for long-lived P2P sockets)
    pub const TCP_KEEPALIVE: Duration = Duration::from_mins(1);
    /// Require peers to advertise matching SM helper availability (`sm_enabled`) during handshake.
    pub const REQUIRE_SM_HANDSHAKE_MATCH: bool = true;
    /// Require peers to match the OpenSSL preview toggle (`sm_openssl_preview`) during handshake.
    pub const REQUIRE_SM_OPENSSL_PREVIEW_MATCH: bool = true;
    /// Default relay mode for P2P (disabled).
    pub const RELAY_MODE: &str = "disabled";
}

/// Snapshotting defaults for archival state dumps.
pub mod snapshot {
    use super::*;

    /// Directory for snapshot files relative to node root.
    pub const STORE_DIR: &str = "./storage/snapshot";
    // 10 mins
    /// Interval between automatic snapshot creation tasks.
    pub const CREATE_EVERY: Duration = Duration::from_mins(10);
    /// Chunk size used for snapshot Merkle metadata (default: 1 MiB).
    pub const MERKLE_CHUNK_SIZE_BYTES: NonZeroUsize = nonzero!(1_048_576_usize);
}

/// Norito streaming control-plane defaults.
pub mod streaming {
    use norito::streaming::CapabilityFlags;

    /// Directory for persisted streaming session snapshots relative to the node root.
    pub const SESSION_STORE_DIR: &str = "./storage/streaming";
    /// Feature bitmask advertised during capability negotiation (baseline feedback + privacy provider + bundled entropy).
    pub const FEATURE_BITS: u32 = 0b11 | CapabilityFlags::FEATURE_ENTROPY_BUNDLED;

    /// Defaults applied to SoraNet circuit integration for streaming routes.
    pub mod soranet {
        use iroha_config_base::util::Bytes;

        /// Enable automatic SoraNet provisioning for streaming routes by default.
        pub const ENABLED: bool = true;
        /// Default exit relay multiaddr used when none is provided in manifests.
        pub const EXIT_MULTIADDR: &str = "/dns/torii/udp/9443/quic";
        /// Default low-latency padding budget (milliseconds) applied to circuits.
        pub const PADDING_BUDGET_MS: u16 = 25;
        /// Access posture enforced by the exit relay (`authenticated` or `read-only`).
        pub const ACCESS_KIND: &str = "authenticated";
        /// Domain separator hashed into blinded channel identifiers when deriving defaults.
        pub const CHANNEL_SALT: &str = "iroha.soranet.channel.seed.v1";
        /// Directory used to spool SoraNet privacy route updates before relays ingest them.
        pub const PROVISION_SPOOL_DIR: &str = "./storage/streaming/soranet_routes";
        /// Maximum on-disk footprint for the SoraNet provisioning spool (0 = unlimited).
        pub const PROVISION_SPOOL_MAX_BYTES: Bytes<u64> = Bytes(0);
        /// Default segment window (inclusive) used when provisioning privacy routes.
        pub const PROVISION_WINDOW_SEGMENTS: u64 = 4;
        /// Maximum number of queued privacy-route provisioning jobs.
        pub const PROVISION_QUEUE_CAPACITY: u64 = 256;

        /// Convenience accessor returning the default padding budget as an `Option`.
        #[must_use]
        #[allow(clippy::unnecessary_wraps)]
        pub const fn padding_budget_ms() -> Option<u16> {
            Some(PADDING_BUDGET_MS)
        }
    }

    /// Defaults applied to SoraVPN local provisioning spools.
    pub mod soravpn {
        use iroha_config_base::util::Bytes;

        /// Directory used to spool SoraVPN route updates before VPN nodes ingest them.
        pub const PROVISION_SPOOL_DIR: &str = "./storage/streaming/soravpn_routes";
        /// Maximum on-disk footprint for the SoraVPN provision spool (0 = unlimited).
        pub const PROVISION_SPOOL_MAX_BYTES: Bytes<u64> = Bytes(0);
    }

    /// Defaults applied to the streaming audio/video sync enforcement gate.
    pub mod sync {
        /// Enable sync enforcement gate (disabled by default until rollout).
        pub const ENABLED: bool = false;
        /// Observe-only mode keeps logging metrics without rejecting segments.
        pub const OBSERVE_ONLY: bool = true;
        /// Minimum rolling window (milliseconds) required before enforcement.
        pub const MIN_WINDOW_MS: u16 = 5_000;
        /// Sustained EWMA drift threshold (milliseconds) that triggers rejection.
        pub const EWMA_THRESHOLD_MS: u16 = 10;
        /// Hard cap for any single frame drift (milliseconds).
        pub const HARD_CAP_MS: u16 = 12;
    }

    /// Codec gating defaults.
    pub mod codec {
        use std::path::PathBuf;

        /// Default CABAC runtime mode (`disabled`).
        pub const CABAC_MODE: &str = "disabled";
        /// Bundled entropy mode advertised by all supported builds (requires `ENABLE_RANS_BUNDLES=1`).
        pub const BUNDLED_ENTROPY_MODE: &str = "rans_bundled";
        /// Default bundle width used by the bundled-rANS encoder.
        pub const BUNDLE_WIDTH: u8 = 2;
        /// Default acceleration backend for bundle processing.
        pub const BUNDLE_ACCEL: &str = "none";

        /// Default trellis block sizes (empty until claim-avoidance ships).
        pub fn trellis_blocks() -> Vec<u16> {
            Vec::new()
        }

        /// Default deterministic rANS table artefact.
        pub fn rans_tables_path() -> PathBuf {
            PathBuf::from("codec/rans/tables/rans_seed0.toml")
        }

        /// Default entropy mode string (bundled rANS only).
        pub fn entropy_mode() -> String {
            assert!(
                norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
                "Bundled rANS is mandatory; rebuild with ENABLE_RANS_BUNDLES=1"
            );
            BUNDLED_ENTROPY_MODE.to_string()
        }

        /// Default bundle width for bundled rANS encoders.
        pub const fn bundle_width() -> u8 {
            BUNDLE_WIDTH
        }

        /// Default acceleration backend string for bundle execution.
        pub fn bundle_accel() -> String {
            BUNDLE_ACCEL.to_string()
        }
    }
}

/// SoraNet-specific defaults.
/// SoraFS storage defaults.
pub mod sorafs {
    /// Defaults governing the embedded SoraFS storage worker.
    pub mod storage {
        use std::path::PathBuf;

        use iroha_config_base::util::Bytes;

        /// Feature toggle for the embedded SoraFS storage worker.
        pub const ENABLED: bool = false;
        /// Default root directory for pinned chunks and manifest metadata.
        pub fn data_dir() -> PathBuf {
            PathBuf::from("./storage/sorafs")
        }
        /// Maximum on-disk capacity allocated to SoraFS (bytes).
        pub const MAX_CAPACITY_BYTES: Bytes<u64> = Bytes(100 * 1024 * 1024 * 1024);
        /// Maximum concurrent fetch operations served by the gateway.
        pub const MAX_PARALLEL_FETCHES: usize = 32;
        /// Maximum number of manifests pinned before the node applies back-pressure.
        pub const MAX_PINS: usize = 10_000;
        /// Background Proof-of-Retrievability sampling cadence (seconds).
        pub const POR_SAMPLE_INTERVAL_SECS: u64 = 600;
        /// Default telemetry alias advertised by the node.
        pub fn alias() -> Option<String> {
            None
        }
        /// Default SoraFS availability tier string.
        pub fn advert_availability() -> String {
            "hot".to_string()
        }
        /// Default upper bound on retrieval latency advertised in telemetry (milliseconds).
        pub const ADVERT_MAX_LATENCY_MS: u32 = 500;
        /// Default rendezvous topics advertised when none are provided.
        pub fn advert_topics() -> Vec<String> {
            vec!["sorafs.sf1.primary:global".to_string()]
        }
        /// Default filesystem directory for governance artefacts.
        pub fn governance_dir() -> Option<PathBuf> {
            None
        }

        /// Stream token issuance defaults.
        pub mod tokens {
            use std::path::PathBuf;

            /// Enable gateway-issued stream tokens.
            pub const ENABLED: bool = false;
            /// Default filesystem location for the Ed25519 signing key.
            pub fn signing_key_path() -> Option<PathBuf> {
                None
            }
            /// Token public-key version advertised to clients.
            pub const KEY_VERSION: u32 = 1;
            /// Default TTL applied to issued tokens (seconds).
            pub const DEFAULT_TTL_SECS: u64 = 900; // 15 minutes
            /// Default maximum concurrent streams permitted per token.
            pub const DEFAULT_MAX_STREAMS: u16 = 4;
            /// Default sustained throughput budget (bytes per second).
            pub const DEFAULT_RATE_LIMIT_BYTES: u64 = 8 * 1024 * 1024; // 8 MiB/s
            /// Default allowed requests per minute for token refresh.
            pub const DEFAULT_REQUESTS_PER_MINUTE: u32 = 120;
        }
    }

    /// Defaults for the SoraFS repair scheduler configuration.
    pub mod repair {
        use std::path::PathBuf;

        /// Enable the repair scheduler (disabled by default).
        pub const ENABLED: bool = false;
        /// Optional directory for durable repair state.
        pub fn state_dir() -> Option<PathBuf> {
            None
        }
        /// Default claim lease TTL for repair tickets (seconds).
        pub const CLAIM_TTL_SECS: u64 = 15 * 60;
        /// Heartbeat interval/TTL for active repair claims (seconds).
        pub const HEARTBEAT_INTERVAL_SECS: u64 = 60;
        /// Maximum number of repair attempts before escalation.
        pub const MAX_ATTEMPTS: u32 = 3;
        /// Concurrent repair workers per node.
        pub const WORKER_CONCURRENCY: usize = 4;
        /// Initial retry backoff for repair workers (seconds).
        pub const BACKOFF_INITIAL_SECS: u64 = 5;
        /// Maximum retry backoff for repair workers (seconds).
        pub const BACKOFF_MAX_SECS: u64 = 60;
        /// Default slash penalty for scheduler-generated repair proposals (nano-XOR).
        pub const DEFAULT_SLASH_PENALTY_NANO: u128 = 1_000_000_000;
    }

    /// Defaults for the SoraFS GC scheduler configuration.
    pub mod gc {
        use std::path::PathBuf;

        /// Enable the GC worker (disabled by default).
        pub const ENABLED: bool = false;
        /// Optional directory for durable GC state.
        pub fn state_dir() -> Option<PathBuf> {
            None
        }
        /// GC cadence (seconds).
        pub const INTERVAL_SECS: u64 = 15 * 60;
        /// Maximum deletions per GC run.
        pub const MAX_DELETIONS_PER_RUN: u32 = 500;
        /// Grace window for retention expiry (seconds).
        pub const RETENTION_GRACE_SECS: u64 = 24 * 60 * 60;
        /// Attempt a GC sweep before rejecting new pins when storage is full.
        pub const PRE_ADMISSION_SWEEP: bool = true;
    }

    /// Defaults for the Proof-of-Retrievability coordinator runtime.
    pub mod por {
        use std::path::PathBuf;

        /// Enable the PoR coordinator runtime.
        pub const ENABLED: bool = false;
        /// Length of a PoR epoch (seconds).
        pub const EPOCH_INTERVAL_SECS: u64 = 60 * 60;
        /// Response window allowed for proofs (seconds).
        pub const RESPONSE_WINDOW_SECS: u64 = 15 * 60;
        /// Default filesystem directory used to persist governance DAG payloads.
        pub fn governance_dir() -> PathBuf {
            PathBuf::from("./storage/sorafs/governance")
        }
        /// Optional deterministic randomness seed (hex). `None` falls back to runtime derivation.
        pub fn randomness_seed_hex() -> Option<String> {
            None
        }
    }

    /// Defaults for the SoraFS gateway policy and automation surface.
    pub mod gateway {
        /// Require clients to attach the manifest envelope before serving data.
        pub const REQUIRE_MANIFEST_ENVELOPE: bool = true;
        /// Enforce admission registry membership by default.
        pub const ENFORCE_ADMISSION: bool = true;
        /// Enforce advertised capabilities (e.g., chunk-range fetch) before serving data.
        pub const ENFORCE_CAPABILITIES: bool = false;

        /// Rate-limiting defaults applied to gateway clients.
        pub mod rate_limit {
            use std::time::Duration;

            /// Maximum requests permitted within the rolling window.
            pub const MAX_REQUESTS: Option<u32> = Some(300);
            /// Rolling window duration (seconds).
            pub const WINDOW: Duration = Duration::from_mins(1);
            /// Temporary ban duration applied after repeated violations.
            pub const BAN: Option<Duration> = Some(Duration::from_secs(30));
        }

        /// Denylist bootstrap defaults.
        pub mod denylist {
            use std::{path::PathBuf, time::Duration};

            /// Optional filesystem path to the denylist JSON file.
            #[must_use]
            pub fn path() -> Option<PathBuf> {
                None
            }

            /// Maximum TTL applied to standard denial entries when `expires_at` is omitted.
            pub const STANDARD_TTL: Duration = Duration::from_hours(24 * 180);
            /// Maximum TTL applied to emergency canons.
            pub const EMERGENCY_TTL: Duration = Duration::from_hours(24 * 30);
            /// Required review window for emergency canons.
            pub const EMERGENCY_REVIEW_WINDOW: Duration = Duration::from_hours(24 * 7);
            /// Permanent entries must cite a governance reference by default.
            pub const REQUIRE_GOVERNANCE_REFERENCE: bool = true;
        }

        /// Default staged anonymity policy applied to SoraNet transports.
        pub const DEFAULT_ANONYMITY_POLICY: &str = "anon-guard-pq";
        /// Default rollout phase label for the staged PQ activation.
        pub const DEFAULT_ROLLOUT_PHASE: &str = "canary";

        /// Returns the default rollout phase label.
        #[must_use]
        pub fn rollout_phase() -> String {
            DEFAULT_ROLLOUT_PHASE.to_string()
        }

        /// Returns the default staged anonymity policy label.
        #[must_use]
        #[allow(clippy::unnecessary_wraps)]
        pub fn anonymity_policy() -> Option<String> {
            Some(DEFAULT_ANONYMITY_POLICY.to_string())
        }

        /// ACME automation defaults.
        pub mod acme {
            use std::time::Duration;

            /// Enable ACME automation for TLS certificates.
            pub const ENABLED: bool = false;
            /// Default account email (unset by default).
            pub fn account_email() -> Option<String> {
                None
            }
            /// Default ACME directory URL (Let’s Encrypt production).
            pub fn directory_url() -> String {
                "https://acme-v02.api.letsencrypt.org/directory".to_string()
            }
            /// Default hostnames covered by the automation (empty list).
            pub fn hostnames() -> Vec<String> {
                Vec::new()
            }
            /// Optional DNS provider plug-in identifier.
            pub fn dns_provider_id() -> Option<String> {
                None
            }
            /// Renewal window applied before certificate expiry (seconds).
            pub const RENEWAL_WINDOW: Duration = Duration::from_hours(30 * 24);
            /// Backoff applied after automation failures (seconds).
            pub const RETRY_BACKOFF: Duration = Duration::from_mins(30);
            /// Maximum jitter applied to retry scheduling (seconds).
            pub const RETRY_JITTER: Duration = Duration::from_mins(5);
            /// Solve DNS-01 challenges by default.
            pub const DNS01: bool = true;
            /// Solve TLS-ALPN-01 challenges by default.
            pub const TLS_ALPN_01: bool = true;
            /// Initial ECH enabled state reported via telemetry.
            pub const ECH_ENABLED: bool = false;
        }
    }
}

/// Torii API defaults (HTTP + query service).
pub mod torii {
    use std::{
        num::{NonZeroU32, NonZeroUsize},
        path::PathBuf,
        time::Duration,
        vec::Vec,
    };

    use iroha_config_base::util::Bytes;
    use iroha_data_model::{
        da::types::{BlobClass, GovernanceTag, RetentionPolicy},
        sorafs::pin_registry::StorageClass as SorafsStorageClass,
    };
    use iroha_torii_shared::{
        API_MIN_PROOF_VERSION as SHARED_API_MIN_PROOF_VERSION,
        API_VERSION_DEFAULT as SHARED_API_VERSION_DEFAULT,
        API_VERSION_SUNSET_UNIX as SHARED_API_VERSION_SUNSET_UNIX,
        API_VERSION_SUPPORTED as SHARED_API_VERSION_SUPPORTED,
    };
    use nonzero_ext::nonzero;

    /// Maximum request payload size accepted by Torii (bytes).
    pub const MAX_CONTENT_LEN: Bytes<u64> = Bytes(2_u64.pow(20) * 16);
    /// Idle time before closing unused query subscriptions.
    pub const QUERY_IDLE_TIME: Duration = Duration::from_secs(10);
    /// Capacity of in-memory query result cache for all authorities.
    pub const QUERY_STORE_CAPACITY: NonZeroUsize = nonzero!(128usize);
    /// Per-authority allocation within the query result cache.
    pub const QUERY_STORE_CAPACITY_PER_USER: NonZeroUsize = nonzero!(128usize);
    // Default per-authority query rate (tokens/sec). Set low but permissive.
    // None disables limiting; Some enables it.
    // Chosen to be friendly under normal usage while protecting from bursty abuse.
    /// Default steady-state query rate tokens issued per authority every second.
    pub const QUERY_RATE_PER_AUTHORITY_PER_SEC: Option<u32> = Some(25);
    // Default burst capacity in tokens per authority.
    /// Maximum burst tokens accumulated per authority.
    pub const QUERY_BURST_PER_AUTHORITY: Option<u32> = Some(50);
    /// Default steady-state transaction submission rate tokens per authority every second.
    pub const TX_RATE_PER_AUTHORITY_PER_SEC: Option<u32> = Some(10_000);
    /// Default transaction submission burst tokens per authority.
    pub const TX_BURST_PER_AUTHORITY: Option<u32> = Some(20_000);
    /// Default steady-state deploy rate tokens issued per origin every second.
    pub const DEPLOY_RATE_PER_ORIGIN_PER_SEC: Option<u32> = Some(4);
    /// Maximum burst tokens accumulated per origin for deploy endpoints.
    pub const DEPLOY_BURST_PER_ORIGIN: Option<u32> = Some(8);
    /// Steady-state proof endpoint rate (requests per minute). None disables.
    pub const PROOF_RATE_PER_MIN: Option<u32> = Some(120);
    /// Burst tokens for proof endpoints (requests).
    pub const PROOF_BURST: Option<u32> = Some(60);
    /// Maximum proof request payload size (bytes).
    pub const PROOF_MAX_BODY_BYTES: Bytes<u64> = Bytes(8 * 1024 * 1024); // 8 MiB
    /// Steady-state egress budget for proof responses (bytes/sec). None disables.
    pub const PROOF_EGRESS_BYTES_PER_SEC: Option<u64> = Some(8 * 1024 * 1024); // 8 MiB/s
    /// Burst egress budget for proof responses (bytes).
    pub const PROOF_EGRESS_BURST_BYTES: Option<u64> = Some(16 * 1024 * 1024); // 16 MiB
    /// Maximum page size accepted by proof listing endpoints.
    pub const PROOF_MAX_LIST_LIMIT: u32 = 200;
    /// Wall-clock timeout applied to proof list/count handlers (milliseconds).
    pub const PROOF_REQUEST_TIMEOUT_MS: u64 = 1_000;
    /// Cache lifetime advertised for proof lookups (seconds).
    pub const PROOF_CACHE_MAX_AGE_SECS: u64 = 30;
    /// Retry hint advertised when proof endpoints are throttled (seconds).
    pub const PROOF_RETRY_AFTER_SECS: u64 = 1;
    /// Default global pre-auth connection cap (pre-RLIMIT clamp).
    pub const PREAUTH_MAX_CONNECTIONS: Option<NonZeroUsize> = Some(nonzero!(1024usize));
    /// Default per-IP pre-auth connection cap (None disables per-IP caps).
    pub const PREAUTH_MAX_CONNECTIONS_PER_IP: Option<NonZeroUsize> = None;
    /// SoraNet privacy ingestion defaults (disabled until explicitly configured).
    pub mod soranet_privacy_ingest {
        use super::*;

        /// Require an explicit allow-list and token before accepting privacy telemetry.
        pub const ENABLED: bool = false;
        /// Require a token header for privacy ingestion calls.
        pub const REQUIRE_TOKEN: bool = true;
        /// Requests per second budget for privacy ingest (None disables).
        pub const RATE_PER_SEC: Option<u32> = Some(8);
        /// Burst budget for privacy ingest (tokens).
        pub const BURST: Option<u32> = Some(16);

        /// Default token list (empty by default; must be configured).
        pub fn tokens() -> Vec<String> {
            Vec::new()
        }

        /// CIDR allow-list for privacy ingest (empty => deny).
        pub fn allow_cidrs() -> Vec<String> {
            Vec::new()
        }
    }

    /// Peer-telemetry geo lookup defaults (disabled unless explicitly enabled).
    pub mod peer_geo {
        use url::Url;

        /// Master enable switch for peer geo lookups.
        pub const ENABLED: bool = false;

        /// Optional geo endpoint (ip-api compatible). `None` uses the built-in default when enabled.
        pub fn endpoint() -> Option<Url> {
            None
        }
    }
    /// Offline certificate issuer defaults (only used when config is supplied).
    pub mod offline_issuer {
        /// Master enable switch for offline certificate issuer endpoints.
        pub const ENABLED: bool = true;

        /// Additional legacy operator private keys retained for build-claim compatibility.
        pub fn legacy_operator_private_keys() -> Vec<iroha_crypto::ExposedPrivateKey> {
            Vec::new()
        }

        /// Allowed controller allow-list (empty => allow all).
        pub fn allowed_controllers() -> Vec<String> {
            Vec::new()
        }
    }
    /// Operator request-signature defaults for Torii operator endpoints.
    pub mod operator_signatures {
        /// Master enable switch for operator signature authentication.
        pub const ENABLED: bool = true;
        /// Allow the node identity key (from `[common]`) to sign operator requests.
        pub const ALLOW_NODE_KEY: bool = true;
        /// Maximum allowed clock skew for signed operator requests (seconds).
        pub const MAX_CLOCK_SKEW_SECS: u64 = 60;
        /// TTL for operator nonces retained for replay detection (seconds).
        pub const NONCE_TTL_SECS: u64 = 300;
        /// Maximum number of nonces held in memory for replay detection.
        pub const REPLAY_CACHE_CAPACITY: usize = 10_000;

        /// Additional operator public keys allowed to sign requests (empty by default).
        pub fn allowed_public_keys() -> Vec<iroha_crypto::PublicKey> {
            Vec::new()
        }
    }
    /// Operator authentication defaults for Torii operator endpoints.
    pub mod operator_auth {
        /// Master enable switch for operator authentication.
        pub const ENABLED: bool = false;
        /// Require mTLS at the ingress tier before allowing operator endpoints.
        pub const REQUIRE_MTLS: bool = false;
        /// Token fallback mode (`disabled`, `bootstrap`, `always`).
        pub const TOKEN_FALLBACK: &str = "bootstrap";
        /// Token source selection (`operator`, `api`, `both`).
        pub const TOKEN_SOURCE: &str = "operator";
        /// Token allow-list for operator fallback (empty => none).
        pub fn tokens() -> Vec<String> {
            Vec::new()
        }
        /// Auth attempt rate (per minute). None disables.
        pub const RATE_PER_MIN: Option<u32> = Some(30);
        /// Burst budget for auth attempts (tokens).
        pub const BURST: Option<u32> = Some(10);
        /// Failures before applying a temporary lockout.
        pub const LOCKOUT_FAILURES: u32 = 5;
        /// Sliding window for lockout failure counts (seconds).
        pub const LOCKOUT_WINDOW_SECS: u64 = 300;
        /// Lockout duration once triggered (seconds).
        pub const LOCKOUT_DURATION_SECS: u64 = 900;

        /// WebAuthn configuration defaults.
        pub mod webauthn {
            /// Master enable switch for WebAuthn.
            pub const ENABLED: bool = true;
            /// Require user verification during assertions.
            pub const REQUIRE_USER_VERIFICATION: bool = true;
            /// Challenge TTL for WebAuthn ceremonies (seconds).
            pub const CHALLENGE_TTL_SECS: u64 = 120;
            /// Session token TTL after successful WebAuthn assertion (seconds).
            pub const SESSION_TTL_SECS: u64 = 900;

            /// Default RP name used in WebAuthn options.
            pub fn rp_name() -> String {
                "Iroha Operator".to_string()
            }

            /// Default user id encoded into WebAuthn options.
            pub fn user_id() -> String {
                "operator".to_string()
            }

            /// Default user name encoded into WebAuthn options.
            pub fn user_name() -> String {
                "operator".to_string()
            }

            /// Default user display name encoded into WebAuthn options.
            pub fn user_display_name() -> String {
                "Iroha Operator".to_string()
            }

            /// Allowed WebAuthn origins (empty => must be configured).
            pub fn origins() -> Vec<String> {
                Vec::new()
            }

            /// Allowed WebAuthn algorithms (COSE labels).
            pub fn allowed_algorithms() -> Vec<String> {
                vec!["es256".to_string(), "ed25519".to_string()]
            }
        }
    }
    /// Webhook destination security defaults for app-facing webhooks.
    pub mod webhook_security {
        /// Master enable switch for webhook destination guard rails.
        pub const ENABLED: bool = true;

        /// CIDR allow-list for webhook destinations (empty => only public IPs are allowed).
        pub fn allow_cidrs() -> Vec<String> {
            Vec::new()
        }
    }
    /// Capacity of the broadcast channel used for Torii events/SSE/webhooks.
    pub const EVENTS_BUFFER_CAPACITY: usize = 10_000;
    /// WebSocket message timeout for Torii event/block streams (milliseconds).
    pub const WS_MESSAGE_TIMEOUT_MS: u64 = 10_000;
    /// Default page size for app-facing list/query endpoints.
    pub const APP_API_DEFAULT_LIST_LIMIT: u32 = 100;
    /// Maximum page size accepted by app-facing list/query endpoints.
    pub const APP_API_MAX_LIST_LIMIT: u32 = 500;
    /// Maximum fetch size accepted by app-facing iterable queries.
    pub const APP_API_MAX_FETCH_SIZE: u32 = 500;
    /// Rate-limiter cost applied per requested row on app-facing endpoints.
    pub const APP_API_RATE_LIMIT_COST_PER_ROW: u32 = 1;
    /// Maximum pending webhook deliveries persisted on disk.
    pub const WEBHOOK_QUEUE_CAPACITY: usize = 10_000;
    /// Maximum delivery attempts before a payload is dropped.
    pub const WEBHOOK_MAX_ATTEMPTS: u32 = 12;
    /// Initial backoff delay (milliseconds) applied to webhook retries.
    pub const WEBHOOK_BACKOFF_INITIAL_MS: u64 = 1_000;
    /// Maximum backoff delay (milliseconds) applied to webhook retries.
    pub const WEBHOOK_BACKOFF_MAX_MS: u64 = 60_000;
    /// HTTP connect timeout (milliseconds) for webhook delivery.
    pub const WEBHOOK_CONNECT_TIMEOUT_MS: u64 = 10_000;
    /// HTTP write timeout (milliseconds) for webhook delivery.
    pub const WEBHOOK_WRITE_TIMEOUT_MS: u64 = 10_000;
    /// HTTP read timeout (milliseconds) for webhook delivery.
    pub const WEBHOOK_READ_TIMEOUT_MS: u64 = 10_000;
    /// Capacity helper for Torii events buffer.
    pub const fn events_buffer_capacity() -> NonZeroUsize {
        nonzero!(EVENTS_BUFFER_CAPACITY)
    }
    /// Queue capacity helper for webhook delivery worker.
    pub const fn webhook_queue_capacity() -> NonZeroUsize {
        nonzero!(WEBHOOK_QUEUE_CAPACITY)
    }
    /// Attempt cap helper for webhook delivery worker.
    pub const fn webhook_max_attempts() -> NonZeroU32 {
        nonzero!(WEBHOOK_MAX_ATTEMPTS)
    }
    /// Enable the push bridge (FCM/APNS). Disabled by default.
    pub const PUSH_ENABLED: bool = false;
    /// Optional steady-state rate (requests per minute) for push notifications. None disables.
    pub const PUSH_RATE_PER_MINUTE: Option<u32> = Some(60);
    /// Optional burst tokens for push notifications.
    pub const PUSH_BURST: Option<u32> = Some(30);
    /// HTTP connect timeout (milliseconds) for push delivery.
    pub const PUSH_CONNECT_TIMEOUT_MS: u64 = 5_000;
    /// HTTP request timeout (milliseconds) for push delivery.
    pub const PUSH_REQUEST_TIMEOUT_MS: u64 = 10_000;
    /// Maximum topics recorded per registered device.
    pub const PUSH_MAX_TOPICS_PER_DEVICE: usize = 32;
    /// Base directory for Torii persistence (attachments, webhooks, DA queues).
    pub fn data_dir() -> PathBuf {
        PathBuf::from("./storage/torii")
    }
    // API tokens are disabled by default.
    /// Whether Torii requires API tokens for authentication.
    pub const REQUIRE_API_TOKEN: bool = false;
    /// Steady-state rate for pre-authorization attempts per IP.
    pub const PREAUTH_RATE_PER_IP_PER_SEC: Option<u32> = Some(20);
    /// Burst tokens allowed for pre-authorization attempts per IP.
    pub const PREAUTH_BURST_PER_IP: Option<u32> = Some(10);
    /// Time to ban IPs that exceed pre-auth rate limits.
    pub const PREAUTH_BAN_DURATION: Duration = Duration::from_mins(1);
    /// Default TTL for app API ZK attachments (seconds)
    pub const ATTACHMENTS_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days
    /// Default maximum size per ZK attachment (bytes)
    pub const ATTACHMENTS_MAX_BYTES: u64 = 4 * 1024 * 1024; // 4 MiB
    /// Default maximum number of ZK attachments stored per tenant (0 = unlimited).
    pub const ATTACHMENTS_PER_TENANT_MAX_COUNT: u64 = 128;
    /// Default aggregate attachment bytes per tenant (0 = unlimited).
    pub const ATTACHMENTS_PER_TENANT_MAX_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB
    /// Default allowed MIME types for attachment payloads (post-sniff).
    #[must_use]
    pub fn attachments_allowed_mime_types() -> Vec<String> {
        vec![
            "application/x-norito".to_string(),
            "application/json".to_string(),
            "text/json".to_string(),
            "application/x-zk1".to_string(),
        ]
    }
    /// Default maximum expanded bytes when decompressing attachments.
    pub const ATTACHMENTS_MAX_EXPANDED_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB
    /// Default maximum archive depth when expanding attachments.
    pub const ATTACHMENTS_MAX_ARCHIVE_DEPTH: u32 = 2;
    /// Default attachment sanitization timeout (milliseconds).
    pub const ATTACHMENTS_SANITIZE_TIMEOUT_MS: u64 = 1_000;
    /// Attachment sanitizer execution mode (`subprocess` or `in_process`).
    pub const ATTACHMENTS_SANITIZER_MODE: &str = "subprocess";
    /// Background ZK prover worker enable flag (disabled by default)
    pub const ZK_PROVER_ENABLED: bool = false;
    /// Background ZK prover scan period (seconds)
    pub const ZK_PROVER_SCAN_PERIOD_SECS: u64 = 30;
    /// Background ZK prover reports retention TTL (seconds)
    pub const ZK_PROVER_REPORTS_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days
    /// Maximum number of attachments the background prover processes concurrently.
    pub const ZK_PROVER_MAX_INFLIGHT: usize = 2;
    /// Maximum aggregate attachment bytes processed per scan cycle.
    pub const ZK_PROVER_MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB
    /// Maximum wall-clock time (milliseconds) spent in a single scan cycle.
    pub const ZK_PROVER_MAX_SCAN_MILLIS: u64 = 2_000;
    /// Directory containing verifying key bytes for the background prover worker.
    #[must_use]
    pub fn zk_prover_keys_dir() -> PathBuf {
        data_dir().join("zk_prover").join("keys")
    }
    /// Maximum number of concurrent ZK IVM prove jobs handled by Torii.
    ///
    /// This limit applies to `POST /v1/zk/ivm/prove` (non-consensus helper).
    pub const ZK_IVM_PROVE_MAX_INFLIGHT: usize = 1;
    /// Maximum number of queued ZK IVM prove jobs accepted while inflight is saturated.
    ///
    /// This limit applies to `POST /v1/zk/ivm/prove` (non-consensus helper).
    pub const ZK_IVM_PROVE_MAX_QUEUE: usize = 16;
    /// TTL (seconds) for `/v1/zk/ivm/prove` job status entries.
    pub const ZK_IVM_PROVE_JOB_TTL_SECS: u64 = 30 * 60; // 30 minutes
    /// Maximum number of `/v1/zk/ivm/prove` job status entries retained in memory.
    pub const ZK_IVM_PROVE_JOB_MAX_ENTRIES: usize = 1_024;
    /// Allowlisted backend prefixes for the background prover worker.
    #[must_use]
    pub fn zk_prover_allowed_backends() -> Vec<String> {
        vec!["halo2/".to_string()]
    }
    /// Allowlisted circuit identifiers for the background prover worker.
    /// Empty list means "allow all circuits".
    #[must_use]
    pub fn zk_prover_allowed_circuits() -> Vec<String> {
        Vec::new()
    }
    /// Emit Torii filter debug traces (developer diagnostics only).
    pub const DEBUG_MATCH_FILTERS: bool = false;
    /// Default Torii API version used when clients omit the header.
    pub const API_DEFAULT_VERSION: &str = SHARED_API_VERSION_DEFAULT;
    /// Minimum API version required for proof/staking/fee endpoints.
    pub const API_MIN_PROOF_VERSION: &str = SHARED_API_MIN_PROOF_VERSION;
    /// Optional unix timestamp when the oldest supported version sunsets.
    pub const API_SUNSET_UNIX: Option<u64> = SHARED_API_VERSION_SUNSET_UNIX;
    /// Supported Torii API versions (oldest → newest).
    #[must_use]
    pub fn api_supported_versions() -> Vec<String> {
        SHARED_API_VERSION_SUPPORTED
            .iter()
            .map(|v| (*v).to_string())
            .collect()
    }
    /// Default Torii API version label as an owned string.
    #[must_use]
    pub fn api_default_version() -> String {
        API_DEFAULT_VERSION.to_string()
    }
    /// Minimum API version for protected surfaces as an owned string.
    #[must_use]
    pub fn api_min_proof_version() -> String {
        API_MIN_PROOF_VERSION.to_string()
    }
    /// RBC sampling endpoint disabled by default.
    pub const RBC_SAMPLING_ENABLED: bool = false;
    /// Maximum chunks sampled per request.
    pub const RBC_SAMPLING_MAX_SAMPLES_PER_REQUEST: u32 = 3;
    /// Maximum total chunk bytes returned per request.
    pub const RBC_SAMPLING_MAX_BYTES_PER_REQUEST: u64 = 256 * 1024; // 256 KiB
    /// Daily byte budget per caller for RBC sampling.
    pub const RBC_SAMPLING_DAILY_BYTE_BUDGET: u64 = 4 * 1024 * 1024; // 4 MiB
    /// Optional per-caller rate (requests per minute) for RBC sampling.
    pub const RBC_SAMPLING_RATE_PER_MIN: Option<u32> = Some(12);
    /// Replay cache capacity per `(lane, epoch)` window.
    pub const DA_REPLAY_CACHE_CAPACITY: NonZeroUsize = nonzero!(4096usize);
    /// Replay cache TTL (seconds) applied to observed manifests.
    pub const DA_REPLAY_CACHE_TTL_SECS: u64 = 15 * 60;
    /// Maximum sequence lag tolerated before rejecting manifests.
    pub const DA_REPLAY_CACHE_MAX_SEQUENCE_LAG: u64 = 4_096;
    /// Default directory for persisted DA replay cursors.
    pub fn da_replay_cache_store_dir() -> PathBuf {
        PathBuf::from("./storage/da_replay")
    }
    /// Default directory for queued DA manifests awaiting SoraFS orchestration.
    pub fn da_manifest_store_dir() -> PathBuf {
        PathBuf::from("./storage/da_manifests")
    }
    /// Governance metadata encryption disabled by default.
    pub fn da_governance_metadata_key() -> Option<[u8; 32]> {
        None
    }
    /// Default governance metadata key label (unused when encryption disabled).
    pub fn da_governance_metadata_key_label() -> Option<String> {
        None
    }
    /// Default replication/retention policy applied to DA blobs.
    pub fn da_replication_default_policy() -> RetentionPolicy {
        RetentionPolicy {
            hot_retention_secs: 6 * 60 * 60,
            cold_retention_secs: 30 * 24 * 60 * 60,
            required_replicas: 3,
            storage_class: SorafsStorageClass::Warm,
            governance_tag: GovernanceTag::new("da.default"),
        }
    }
    /// Per-class overrides layered on top of the default DA replication policy.
    pub fn da_replication_overrides() -> Vec<(BlobClass, RetentionPolicy)> {
        vec![
            (
                BlobClass::TaikaiSegment,
                RetentionPolicy {
                    hot_retention_secs: 24 * 60 * 60,
                    cold_retention_secs: 14 * 24 * 60 * 60,
                    required_replicas: 5,
                    storage_class: SorafsStorageClass::Hot,
                    governance_tag: GovernanceTag::new("da.taikai.live"),
                },
            ),
            (
                BlobClass::NexusLaneSidecar,
                RetentionPolicy {
                    hot_retention_secs: 6 * 60 * 60,
                    cold_retention_secs: 7 * 24 * 60 * 60,
                    required_replicas: 4,
                    storage_class: SorafsStorageClass::Warm,
                    governance_tag: GovernanceTag::new("da.sidecar"),
                },
            ),
            (
                BlobClass::GovernanceArtifact,
                RetentionPolicy {
                    hot_retention_secs: 12 * 60 * 60,
                    cold_retention_secs: 180 * 24 * 60 * 60,
                    required_replicas: 3,
                    storage_class: SorafsStorageClass::Cold,
                    governance_tag: GovernanceTag::new("da.governance"),
                },
            ),
        ]
    }

    /// Availability-class overrides layered on top of the Taikai policy.
    pub fn taikai_availability_overrides() -> Vec<(
        iroha_data_model::taikai::TaikaiAvailabilityClass,
        RetentionPolicy,
    )> {
        vec![
            (
                iroha_data_model::taikai::TaikaiAvailabilityClass::Hot,
                RetentionPolicy {
                    hot_retention_secs: 24 * 60 * 60,
                    cold_retention_secs: 14 * 24 * 60 * 60,
                    required_replicas: 5,
                    storage_class: SorafsStorageClass::Hot,
                    governance_tag: GovernanceTag::new("da.taikai.live"),
                },
            ),
            (
                iroha_data_model::taikai::TaikaiAvailabilityClass::Warm,
                RetentionPolicy {
                    hot_retention_secs: 6 * 60 * 60,
                    cold_retention_secs: 30 * 24 * 60 * 60,
                    required_replicas: 4,
                    storage_class: SorafsStorageClass::Warm,
                    governance_tag: GovernanceTag::new("da.taikai.warm"),
                },
            ),
            (
                iroha_data_model::taikai::TaikaiAvailabilityClass::Cold,
                RetentionPolicy {
                    hot_retention_secs: 60 * 60,
                    cold_retention_secs: 180 * 24 * 60 * 60,
                    required_replicas: 3,
                    storage_class: SorafsStorageClass::Cold,
                    governance_tag: GovernanceTag::new("da.taikai.archive"),
                },
            ),
        ]
    }

    /// Default rent base rate per GiB-month in micro-XOR.
    pub const DA_RENT_BASE_RATE_PER_GIB_MONTH_MICRO: u128 = 250_000;
    /// Default protocol reserve share in basis points.
    pub const DA_RENT_PROTOCOL_RESERVE_BPS: u16 = 2_000;
    /// Default PDP bonus share in basis points.
    pub const DA_RENT_PDP_BONUS_BPS: u16 = 500;
    /// Default PoTR bonus share in basis points.
    pub const DA_RENT_POTR_BONUS_BPS: u16 = 250;
    /// Default egress credit per GiB (micro-XOR).
    pub const DA_RENT_EGRESS_CREDIT_PER_GIB_MICRO: u128 = 1_500;

    /// Transport-specific defaults (Norito-RPC, future streaming surfaces, etc.).
    pub mod transport {
        /// Norito-RPC transport defaults surfaced via `torii.transport.norito_rpc`.
        pub mod norito_rpc {
            /// Enable Norito-RPC decoding by default so lab/devnet builds can exercise the transport.
            pub const ENABLED: bool = true;
            /// mTLS requirement flag is surfaced for operators; enforcement happens at the ingress layer.
            pub const REQUIRE_MTLS: bool = false;
            /// Default rollout stage label for Norito-RPC.
            pub const STAGE: &str = "disabled";

            /// Default allowlist of clients permitted to use Norito-RPC (empty = unrestricted).
            #[must_use]
            pub fn allowed_clients() -> Vec<String> {
                Vec::new()
            }
        }
    }

    /// MCP endpoint defaults surfaced via `torii.mcp`.
    pub mod mcp {
        /// Enable native Torii MCP server.
        pub const ENABLED: bool = false;
        /// Maximum accepted MCP request payload size (bytes).
        pub const MAX_REQUEST_BYTES: usize = 1_048_576; // 1 MiB
        /// Maximum number of tools returned per `tools/list` response page.
        pub const MAX_TOOLS_PER_LIST: usize = 500;
        /// Retention window for asynchronous MCP jobs.
        pub const ASYNC_JOB_TTL_SECS: u64 = 300;
        /// Maximum number of asynchronous MCP jobs retained in memory.
        pub const ASYNC_JOB_MAX_ENTRIES: usize = 2_000;
        /// Default MCP tool profile (`read_only`, `writer`, `operator`).
        pub const PROFILE: &str = "read_only";
        /// Expose operator-only routes in the MCP registry.
        pub const EXPOSE_OPERATOR_ROUTES: bool = false;
        /// Extra allow-list prefixes for MCP tool names (empty => profile-only).
        #[must_use]
        pub fn allow_tool_prefixes() -> Vec<String> {
            Vec::new()
        }
        /// Extra deny-list prefixes for MCP tool names.
        #[must_use]
        pub fn deny_tool_prefixes() -> Vec<String> {
            Vec::new()
        }
        /// Optional steady-state MCP request budget (requests/minute). None disables.
        pub const RATE_PER_MINUTE: Option<u32> = Some(240);
        /// Optional MCP request burst budget.
        pub const BURST: Option<u32> = Some(120);
    }
    /// Default poll interval (seconds) for Taikai anchor uploads.
    pub const DA_TAIKAI_ANCHOR_POLL_INTERVAL_SECS: u64 = 30;
    /// ISO 20022 bridge disabled by default.
    pub const ISO_BRIDGE_ENABLED: bool = false;
    /// ISO 20022 dedupe TTL (seconds).
    pub const ISO_BRIDGE_DEDUPE_TTL_SECS: u64 = 5 * 60; // 5 minutes
    /// ISO 20022 reference data refresh cadence (seconds).
    pub const ISO_BRIDGE_REFERENCE_REFRESH_SECS: u64 = 24 * 60 * 60; // 24 hours
    /// SoraFS discovery disabled by default (iroha2 builds).
    pub const SORAFS_DISCOVERY_ENABLED: bool = false;
    /// Maximum SoraFS capacity declarations per provider per hour.
    pub const SORAFS_QUOTA_DECLARATION_MAX_EVENTS: Option<u32> = Some(4);
    /// Rolling window (seconds) for SoraFS capacity declarations.
    pub const SORAFS_QUOTA_DECLARATION_WINDOW_SECS: u64 = 60 * 60;
    /// Maximum SoraFS storage pin submissions per provider per hour.
    pub const SORAFS_QUOTA_STORAGE_PIN_MAX_EVENTS: Option<u32> = Some(4);
    /// Rolling window (seconds) for SoraFS storage pin submissions.
    pub const SORAFS_QUOTA_STORAGE_PIN_WINDOW_SECS: u64 = 60 * 60;
    /// Maximum SoraFS capacity telemetry reports per provider per hour.
    pub const SORAFS_QUOTA_TELEMETRY_MAX_EVENTS: Option<u32> = Some(12);
    /// Rolling window (seconds) for SoraFS capacity telemetry.
    pub const SORAFS_QUOTA_TELEMETRY_WINDOW_SECS: u64 = 60 * 60;
    /// Maximum SoraFS deal telemetry submissions per deal per hour.
    pub const SORAFS_QUOTA_DEAL_TELEMETRY_MAX_EVENTS: Option<u32> = Some(60);
    /// Rolling window (seconds) for SoraFS deal telemetry.
    pub const SORAFS_QUOTA_DEAL_TELEMETRY_WINDOW_SECS: u64 = 60 * 60;
    /// Maximum SoraFS disputes per provider per day.
    pub const SORAFS_QUOTA_DISPUTE_MAX_EVENTS: Option<u32> = Some(2);
    /// Rolling window (seconds) for SoraFS disputes.
    pub const SORAFS_QUOTA_DISPUTE_WINDOW_SECS: u64 = 24 * 60 * 60;
    /// Maximum SoraFS PoR submissions per provider per hour.
    pub const SORAFS_QUOTA_POR_MAX_EVENTS: Option<u32> = Some(60);
    /// Rolling window (seconds) for SoraFS PoR submissions.
    pub const SORAFS_QUOTA_POR_WINDOW_SECS: u64 = 60 * 60;

    /// Alias cache positive TTL (seconds) applied by Torii gateways and SDK helpers.
    pub const SORAFS_ALIAS_POSITIVE_TTL_SECS: u64 = 10 * 60;
    /// Alias cache refresh window (seconds) before positive TTL elapses.
    pub const SORAFS_ALIAS_REFRESH_WINDOW_SECS: u64 = 2 * 60;
    /// Hard expiry (seconds) after which stale alias proofs are rejected even if refresh failed.
    pub const SORAFS_ALIAS_HARD_EXPIRY_SECS: u64 = 15 * 60;
    /// Alias cache negative TTL (seconds) for missing aliases.
    pub const SORAFS_ALIAS_NEGATIVE_TTL_SECS: u64 = 60;
    /// Alias cache TTL (seconds) for revoked aliases (`410 Gone` responses).
    pub const SORAFS_ALIAS_REVOCATION_TTL_SECS: u64 = 5 * 60;
    /// Maximum tolerated age (seconds) for alias proof bundles before rotation is required.
    pub const SORAFS_ALIAS_ROTATION_MAX_AGE_SECS: u64 = 6 * 60 * 60;
    /// Grace period (seconds) applied after an approved successor before refusing predecessor proofs.
    pub const SORAFS_ALIAS_SUCCESSOR_GRACE_SECS: u64 = 5 * 60;
    /// Grace period (seconds) applied to governance rotation events.
    pub const SORAFS_ALIAS_GOVERNANCE_GRACE_SECS: u64 = 0;

    /// Default set of capability names recognised by Torii's discovery cache.
    pub fn sorafs_known_capabilities() -> Vec<String> {
        vec!["torii_gateway".to_string(), "chunk_range_fetch".to_string()]
    }
}

/// Nexus lane/data-space defaults.
pub mod nexus {
    /// Enable multilane (Nexus/Iroha3) consensus by default.
    pub const ENABLED: bool = true;
    use super::*;

    /// AXT policy and runtime defaults.
    pub mod axt {
        /// Default slot length (in milliseconds) used when deriving AXT expiry slots from block timestamps.
        pub const SLOT_LENGTH_MS: u64 = 1;
        /// Minimum allowable slot length to keep expiry math meaningful.
        pub const MIN_SLOT_LENGTH_MS: u64 = 1;
        /// Maximum allowable slot length to avoid disabling expiry enforcement entirely.
        pub const MAX_SLOT_LENGTH_MS: u64 = 600_000;
        /// Default maximum tolerated clock skew (milliseconds) applied to expiry checks.
        pub const CLOCK_SKEW_MS_DEFAULT: u64 = 0;
        /// Upper bound on tolerated clock skew to prevent unbounded expiry extensions.
        pub const CLOCK_SKEW_MS_MAX: u64 = 60_000;
        /// Default number of slots to retain cached proofs (accepted or rejected).
        pub const PROOF_CACHE_TTL_SLOTS: u64 = 1;
        /// Maximum allowed proof cache TTL (slots) to bound replay surface.
        pub const PROOF_CACHE_TTL_SLOTS_MAX: u64 = 64;
        /// Default number of slots to retain handle usage for replay protection.
        pub const REPLAY_RETENTION_SLOTS: u64 = 128;
        /// Maximum allowed replay retention window (slots) to bound in-memory state.
        pub const REPLAY_RETENTION_SLOTS_MAX: u64 = 4_096;
    }

    /// Storage budget defaults for Nexus-enabled nodes.
    pub mod storage {
        use iroha_config_base::util::Bytes;

        /// Aggregate on-disk budget for Iroha3 storage (bytes).
        pub const MAX_DISK_USAGE_BYTES: Bytes<u64> = Bytes(256 * 1024 * 1024 * 1024);
        /// Block interval between disk budget enforcement scans (0 = every block).
        pub const BUDGET_ENFORCE_INTERVAL_BLOCKS: u64 = 10;
        /// WSV hot-tier deterministic payload size budget (bytes).
        pub const MAX_WSV_MEMORY_BYTES: Bytes<u64> = Bytes(8 * 1024 * 1024 * 1024);
        /// Budget share for Kura block storage (basis points).
        pub const KURA_BLOCKS_BPS: u16 = 3_000;
        /// Budget share for tiered-state cold snapshots (basis points).
        pub const WSV_SNAPSHOTS_BPS: u16 = 2_000;
        /// Budget share for SoraFS storage (basis points).
        pub const SORAFS_BPS: u16 = 4_000;
        /// Budget share for SoraNet route spools (basis points).
        pub const SORANET_SPOOL_BPS: u16 = 500;
        /// Budget share reserved for future SoraVPN storage (basis points).
        pub const SORAVPN_SPOOL_BPS: u16 = 500;
        /// Total basis points for storage budgeting.
        pub const BPS_TOTAL: u16 = 10_000;
    }

    /// Default number of execution lanes (single-lane placeholder).
    pub const LANE_COUNT: NonZeroU32 = nonzero!(1u32);
    /// Default alias assigned to the primary lane when no catalog entries are provided.
    pub const DEFAULT_LANE_ALIAS: &str = "default";
    /// Default alias assigned to the global data space when no catalog entries are provided.
    pub const DEFAULT_DATASPACE_ALIAS: &str = "universal";
    /// Default lane index used when routing policy omits an explicit value.
    pub const DEFAULT_ROUTING_LANE_INDEX: u32 = 0;

    /// Dataspace consensus defaults.
    pub mod dataspace {
        /// Default fault tolerance value (f) used to size per-dataspace committees (3f + 1).
        pub const FAULT_TOLERANCE: u32 = 1;
    }

    /// Lane-relay emergency override defaults.
    pub mod lane_relay_emergency {
        /// Emergency override disabled by default.
        pub const ENABLED: bool = false;
        /// Default multisig threshold required to authorize overrides.
        pub const MULTISIG_THRESHOLD: u16 = 3;
        /// Default multisig member count required to authorize overrides.
        pub const MULTISIG_MEMBERS: u16 = 5;
    }

    /// Lane registry defaults.
    pub mod registry {
        use std::time::Duration;

        /// Poll interval for refreshing manifests and governance bundles.
        pub const POLL_INTERVAL: Duration = Duration::from_mins(1);
    }

    /// Lane compliance configuration defaults.
    pub mod compliance {
        /// Compliance disabled by default.
        pub const ENABLED: bool = false;
        /// Audit-only mode enabled by default to allow dry-runs.
        pub const AUDIT_ONLY: bool = true;
    }

    /// Lane-fusion defaults governing low-load consolidation.
    pub mod fusion {
        /// TEU floor below which adjacent lanes should fuse after the observation window.
        pub const FLOOR_TEU: u32 = 4_000;
        /// TEU threshold that forces fused lanes to split back into independent pipelines.
        pub const EXIT_TEU: u32 = 6_000;
        /// Consecutive low-load slots required before fusion activates.
        pub const OBSERVATION_SLOTS: u16 = 2;
        /// Maximum number of slots a fused window may persist without re-evaluating load.
        pub const MAX_WINDOW_SLOTS: u16 = 16;
    }

    /// Deterministic lane autoscaling defaults.
    pub mod autoscale {
        /// Whether consensus-driven lane autoscaling is enabled.
        pub const ENABLED: bool = false;
        /// Minimum active lane count.
        pub const MIN_LANES: u32 = 1;
        /// Maximum active lane count.
        pub const MAX_LANES: u32 = 8;
        /// Target block interval used by the autoscaler (milliseconds).
        pub const TARGET_BLOCK_MS: u64 = 1_000;
        /// Scale-out latency ratio threshold versus target block interval.
        pub const SCALE_OUT_LATENCY_RATIO: f64 = 1.20;
        /// Scale-in latency ratio threshold versus target block interval.
        pub const SCALE_IN_LATENCY_RATIO: f64 = 0.80;
        /// Scale-out utilization ratio threshold.
        pub const SCALE_OUT_UTILIZATION_RATIO: f64 = 0.85;
        /// Scale-in utilization ratio threshold.
        pub const SCALE_IN_UTILIZATION_RATIO: f64 = 0.40;
        /// Number of recent blocks used for scale-out decisions.
        pub const SCALE_OUT_WINDOW_BLOCKS: u16 = 32;
        /// Number of recent blocks used for scale-in decisions.
        pub const SCALE_IN_WINDOW_BLOCKS: u16 = 96;
        /// Cooldown period in blocks after every transition.
        pub const COOLDOWN_BLOCKS: u16 = 64;
        /// Per-lane target throughput used to compute utilization (tx/s).
        pub const PER_LANE_TARGET_TPS: u32 = 50;
    }

    /// Commit & proof deadline defaults (Δ window).
    pub mod commit {
        /// Slot-bound deadline for proofs/DA bundles to arrive before a transaction aborts.
        pub const WINDOW_SLOTS: u16 = 2;
    }

    /// Public-lane staking defaults.
    pub mod staking {
        use std::time::Duration;

        use nonzero_ext::nonzero;

        use super::super::NonZeroU32;

        /// Minimum bonded stake required to register a validator (asset base units).
        pub const MIN_VALIDATOR_STAKE: u64 = 1;
        /// Maximum number of validators allowed per lane.
        pub const MAX_VALIDATORS: NonZeroU32 = nonzero!(32_u32);
        /// Minimum delay between scheduling and finalising unbonds.
        pub const UNBONDING_DELAY: Duration = Duration::from_secs(0);
        /// Grace window after `release_at_ms` for finalising withdrawals.
        pub const WITHDRAW_GRACE: Duration = Duration::from_secs(0);
        /// Maximum slash ratio (basis points, 10_000 = 100%).
        pub const MAX_SLASH_BPS: u16 = 10_000;
        /// Minimum reward amount (base units) that will be paid out; smaller amounts are skipped.
        pub const REWARD_DUST_THRESHOLD: u64 = 0;
        /// Asset definition used for staking bonds (defaults to the Nexus fee asset).
        pub const STAKE_ASSET_ID: &str = super::fees::FEE_ASSET_ID;
        /// Escrow account that custodies bonded stake.
        pub const STAKE_ESCROW_ACCOUNT_ID: &str = super::fees::FEE_SINK_ACCOUNT_ID;
        /// Account that receives slashed stake (treasury/burn sink).
        pub const SLASH_SINK_ACCOUNT_ID: &str = super::fees::FEE_SINK_ACCOUNT_ID;

        /// Asset definition used for staking bonds (defaults to the Nexus fee asset).
        pub fn stake_asset_id() -> String {
            STAKE_ASSET_ID.to_string()
        }

        /// Escrow account that custodies bonded stake.
        pub fn stake_escrow_account_id() -> String {
            STAKE_ESCROW_ACCOUNT_ID.to_string()
        }

        /// Account that receives slashed stake (treasury/burn sink).
        pub fn slash_sink_account_id() -> String {
            SLASH_SINK_ACCOUNT_ID.to_string()
        }
    }

    /// Universal fee schedule defaults.
    pub mod fees {
        /// Fee asset definition identifier (string form).
        pub const FEE_ASSET_ID: &str = "xor#nexus";
        /// Account that receives collected fees (string form).
        pub const FEE_SINK_ACCOUNT_ID: &str = super::pipeline::GAS_TECH_ACCOUNT_ID;
        /// Base fee charged per transaction (asset base units).
        pub const BASE_FEE: u64 = 0;
        /// Per-byte fee charged over the signed transaction payload (asset base units).
        pub const PER_BYTE_FEE: u64 = 0;
        /// Per-instruction fee charged for native ISI batches (asset base units).
        pub const PER_INSTRUCTION_FEE: u64 = 0;
        /// Per-gas-unit fee multiplier applied to measured gas usage (asset base units).
        pub const PER_GAS_UNIT_FEE: u64 = 0;
        /// Whether fee sponsorship is allowed.
        pub const SPONSORSHIP_ENABLED: bool = false;
        /// Maximum fee a sponsor can cover (asset base units, 0 = unlimited).
        pub const SPONSOR_MAX_FEE: u64 = 0;
    }

    /// Domain endorsement defaults.
    pub mod endorsement {
        /// Quorum required (committee signatures) to accept an endorsement (0 disables enforcement).
        pub const QUORUM: u16 = 0;
        /// Committee member key identifiers allowed to sign endorsements.
        pub fn committee_keys() -> Vec<String> {
            Vec::new()
        }
    }

    /// Data-availability sampling defaults.
    pub mod da {
        /// Total in-slot DA signatures budget per lane.
        pub const Q_IN_SLOT_TOTAL: u32 = 2_048;
        /// Minimum in-slot DA signatures per dataspace.
        pub const Q_IN_SLOT_PER_DS_MIN: u16 = 8;
        /// Baseline attester sample size (S) for VRF draws.
        pub const SAMPLE_SIZE_BASE: u16 = 64;
        /// Maximum attester sample size when adaptive scaling increases coverage.
        pub const SAMPLE_SIZE_MAX: u16 = 96;
        /// Threshold `T` (Ed25519 signatures) required off-path for DA certificates.
        pub const THRESHOLD_BASE: u16 = 43;
        /// Number of shards each attester must verify per slot.
        pub const PER_ATTESTER_SHARDS: u16 = 25;

        /// Rolling audit defaults ensuring long-term coverage.
        pub mod audit {
            use std::time::Duration;

            /// Signatures verified per audit window.
            pub const SAMPLE_SIZE: u16 = 32;
            /// Number of audit windows tracked before slashing for insufficient coverage.
            pub const WINDOW_COUNT: u16 = 20;
            /// Interval between audit windows.
            pub const INTERVAL: Duration = Duration::from_mins(10);
        }

        /// Recovery deadline defaults for missing DA proofs.
        pub mod recovery {
            use std::time::Duration;

            /// Deadline for supplying recovery proofs once requested.
            pub const REQUEST_TIMEOUT: Duration = Duration::from_hours(24);
        }

        /// Temporal diversity defaults for attester rotation.
        pub mod rotation {
            /// Maximum appearances of an attester within the rolling window.
            pub const MAX_HITS_PER_WINDOW: u16 = 4;
            /// Slot-width of the rolling window enforcing temporal diversity.
            pub const WINDOW_SLOTS: u16 = 64;
            /// Domain-separation tag used when deriving deterministic rotation seeds.
            pub const SEED_TAG: &str = "iroha:da:rotate:v1\0";
            /// Exponential decay applied to latency bias weights (0-1 inclusive).
            pub const LATENCY_DECAY: f64 = 0.25;
        }
    }
}

/// Iroha Connect defaults.
pub mod connect {
    use std::time::Duration;

    /// Enable Iroha Connect WS + P2P relay.
    pub const ENABLED: bool = true;
    /// Max concurrent WS sessions across roles.
    pub const WS_MAX_SESSIONS: usize = 10_000;
    /// Max concurrent WS sessions per remote IP.
    pub const WS_PER_IP_MAX_SESSIONS: usize = 10;
    /// Per-IP WS handshake rate (requests per minute).
    pub const WS_RATE_PER_IP_PER_MIN: u32 = 120;
    /// Session inactivity TTL (milliseconds).
    pub const SESSION_TTL: Duration = Duration::from_millis(300_000); // 5 minutes
    /// Maximum WS frame size accepted for Connect frames (bytes).
    pub const FRAME_MAX_BYTES: usize = 64_000;
    /// Maximum buffered payload per session (bytes) for pending delivery.
    pub const SESSION_BUFFER_MAX_BYTES: usize = 262_144; // 256 KiB
    /// Heartbeat ping interval (milliseconds).
    pub const PING_INTERVAL: Duration = Duration::from_millis(30_000);
    /// Minimum heartbeat interval allowed for browser transports (milliseconds).
    pub const PING_MIN_INTERVAL: Duration = Duration::from_millis(15_000);
    /// Number of consecutive missed heartbeats tolerated before disconnect.
    pub const PING_MISS_TOLERANCE: u32 = 3;
    /// Dedupe cache TTL (milliseconds) for (sid,dir,seq).
    pub const DEDUPE_TTL: Duration = Duration::from_millis(120_000); // 2 minutes
    /// Dedupe cache capacity (entries).
    pub const DEDUPE_CAP: usize = 8_192;
    /// Enable P2P re-broadcast relay.
    pub const RELAY_ENABLED: bool = true;
    /// Relay strategy string: "broadcast" or "local_only".
    pub const RELAY_STRATEGY: &str = "broadcast";
    /// Optional hop TTL for relay (0 disables, not enforced in v0 flood).
    pub const P2P_TTL_HOPS: u8 = 0;
}

/// External fraud-risk monitoring defaults.
pub mod fraud_monitoring {
    use std::time::Duration;

    /// Enable outbound fraud-monitoring requests.
    pub const ENABLED: bool = false;
    /// HTTP connect timeout for fraud-monitoring requests.
    pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
    /// Request timeout for fraud-monitoring calls.
    pub const REQUEST_TIMEOUT: Duration = Duration::from_millis(1_500);
    /// Maximum grace period to tolerate missing fraud assessments (seconds).
    pub const MISSING_ASSESSMENT_GRACE_SECS: u64 = 0;
}

/// Telemetry export defaults.
pub mod telemetry {
    use std::time::Duration;

    /// Default minimal retry period
    pub const MIN_RETRY_PERIOD: Duration = Duration::from_secs(1);
    /// Default maximum exponent for the retry delay
    pub const MAX_RETRY_DELAY_EXPONENT: u8 = 4;
    /// Master telemetry enable flag (on by default)
    pub const ENABLED: bool = true;
    /// Default telemetry capability profile applied when not overridden.
    pub const PROFILE: &str = "operator";
    /// Panic on duplicate metric registration (developer diagnostics only).
    pub const PANIC_ON_DUPLICATE_METRICS: bool = false;

    /// Telemetry redaction defaults.
    pub mod redaction {
        /// Default redaction mode for telemetry fields.
        pub const MODE: &str = "strict";
    }

    /// Telemetry integrity defaults.
    pub mod integrity {
        /// Enable hash-chained telemetry exports by default.
        pub const ENABLED: bool = true;
    }
}

/// Network Time Service (NTS) defaults.
pub mod time {
    use std::time::Duration;

    /// Sampling interval for peer time probes.
    pub const NTS_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
    /// Maximum peers to sample per round.
    pub const NTS_SAMPLE_CAP_PER_ROUND: usize = 8;
    /// Maximum acceptable round-trip time (milliseconds) for samples.
    pub const NTS_MAX_RTT_MS: u64 = 500;
    /// Trim percent for median aggregation (10% from each side).
    pub const NTS_TRIM_PERCENT: u8 = 10;
    /// Per-peer ring buffer capacity for samples.
    pub const NTS_PER_PEER_BUFFER: usize = 16;
    /// Enable EMA smoothing of network offset.
    pub const NTS_SMOOTHING_ENABLED: bool = false;
    /// EMA alpha in [0,1]; higher means more responsive.
    pub const NTS_SMOOTHING_ALPHA: f64 = 0.2;
    /// Maximum allowed adjustment per minute (ms) when smoothing.
    pub const NTS_MAX_ADJUST_MS_PER_MIN: u64 = 50;
    /// Minimum number of peer samples required for healthy NTS.
    pub const NTS_MIN_SAMPLES: usize = 3;
    /// Maximum absolute offset (ms) permitted before NTS is considered unhealthy (0 disables).
    pub const NTS_MAX_OFFSET_MS: u64 = 1_000;
    /// Maximum confidence (MAD) in ms permitted before NTS is considered unhealthy (0 disables).
    pub const NTS_MAX_CONFIDENCE_MS: u64 = 500;
    /// Enforcement mode for unhealthy NTS ("warn" or "reject").
    pub const NTS_ENFORCEMENT_MODE: &str = "warn";
}

/// Execution pipeline defaults (scheduler, overlay, batching).
pub mod pipeline {
    /// Enable dynamic prepass (IVM read-only run to derive access sets).
    pub const DYNAMIC_PREPASS: bool = true;
    /// Cache derived access sets by code hash/entrypoint for diagnostics.
    pub const ACCESS_SET_CACHE_ENABLED: bool = true;
    /// Enable parallel overlay construction.
    pub const PARALLEL_OVERLAY: bool = true;
    /// Number of worker threads for stateless/overlay pipeline stages (0 = auto).
    pub const WORKERS: usize = 0;
    /// Capacity for the stateless validation cache (0 = disabled).
    pub const STATELESS_CACHE_CAP: usize = 4_096;
    /// Enable per-layer parallel application of overlays.
    pub const PARALLEL_APPLY: bool = true;
    /// Use a binary-heap ready queue in the scheduler instead of stable sort.
    pub const READY_QUEUE_HEAP: bool = false;
    /// Enable GPU key bucketing for scheduler prepass when hardware is available.
    pub const GPU_KEY_BUCKET: bool = false;
    /// Emit scheduler input/output traces for deterministic tie-break debugging.
    pub const DEBUG_TRACE_SCHEDULER_INPUTS: bool = false;
    /// Emit transaction evaluation traces during overlay application (dev diagnostics).
    pub const DEBUG_TRACE_TX_EVAL: bool = false;
    /// Maximum instructions per overlayed transaction (0 = unlimited).
    pub const OVERLAY_MAX_INSTRUCTIONS: usize = 0;
    /// Maximum serialized overlay bytes per transaction (0 = unlimited).
    pub const OVERLAY_MAX_BYTES: u64 = 0;
    /// Instructions processed per overlay chunk during application.
    pub const OVERLAY_CHUNK_INSTRUCTIONS: usize = 256;
    /// IVM pre-decode cache size (number of decoded streams cached).
    pub const CACHE_SIZE: usize = 128;
    /// Hard cap on decoded instructions per cached entry (0 = unlimited; default tuned for safety).
    pub const IVM_CACHE_MAX_DECODED_OPS: usize = 8_000_000;
    /// Approximate byte budget for all cached pre-decode entries combined.
    pub const IVM_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB
    /// Rayon worker cap for prover/trace verification (0 = number of physical cores).
    pub const IVM_PROVER_THREADS: usize = 0;
    /// Historical aggregate signature batch size (0 disables batching).
    pub const SIGNATURE_BATCH_MAX: usize = 0;
    /// Ed25519-specific batch size (0 disables batching).
    pub const SIGNATURE_BATCH_MAX_ED25519: usize = 64;
    /// Secp256k1-specific batch size (0 disables batching).
    pub const SIGNATURE_BATCH_MAX_SECP256K1: usize = 16;
    /// PQC-specific batch size (0 disables batching).
    pub const SIGNATURE_BATCH_MAX_PQC: usize = 8;
    /// BLS-specific batch size (0 disables batching).
    pub const SIGNATURE_BATCH_MAX_BLS: usize = 16;
    /// Default gas-collection technical account identifier (encoded-only literal).
    pub const GAS_TECH_ACCOUNT_ID: &str = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
    /// Admission-time upper bound for `max_cycles` embedded in IVM bytecode headers.
    pub const IVM_MAX_CYCLES_UPPER_BOUND: u64 = 1_000_000;
    /// Maximum decoded Kotodama instructions accepted during admission (0 = unlimited).
    pub const IVM_MAX_DECODED_INSTRUCTIONS: u64 = 1_048_576;
    /// Maximum decoded byte length after Kotodama instruction expansion (0 = unlimited).
    pub const IVM_MAX_DECODED_BYTES: u64 = 4 * 1024 * 1024;
    /// Default cursor mode for query endpoints ("ephemeral" or "stored").
    pub const QUERY_DEFAULT_CURSOR_MODE: &str = "ephemeral";
    /// Maximum fetch size for iterable queries executed inside the IVM.
    pub const QUERY_MAX_FETCH_SIZE: u64 = super::torii::APP_API_MAX_FETCH_SIZE as u64;
    /// Maximum number of transactions allowed in the quarantine lane per block (0 = disabled).
    pub const QUARANTINE_MAX_TXS_PER_BLOCK: usize = 0;
    /// Per-transaction cycle cap enforced for the quarantine lane (0 = unlimited).
    pub const QUARANTINE_TX_MAX_CYCLES: u64 = 0;
    /// Per-transaction wall-clock budget (milliseconds) for the quarantine lane (0 = unlimited).
    pub const QUARANTINE_TX_MAX_MILLIS: u64 = 0;
    /// Minimum gas units required before stored cursor mode can be used (0 = disabled).
    pub const QUERY_STORED_MIN_GAS_UNITS: u64 = 0;
    /// AMX per-dataspace execution budget in milliseconds.
    pub const AMX_PER_DATASPACE_BUDGET_MS: u64 = 30;
    /// AMX group execution budget across dataspaces in milliseconds.
    pub const AMX_GROUP_BUDGET_MS: u64 = 140;
    /// Estimated nanoseconds per instruction used for AMX budgeting.
    pub const AMX_PER_INSTRUCTION_NS: u64 = 50;
    /// Estimated nanoseconds per memory access used for AMX budgeting.
    pub const AMX_PER_MEMORY_ACCESS_NS: u64 = 80;
    /// Estimated nanoseconds per syscall used for AMX budgeting.
    pub const AMX_PER_SYSCALL_NS: u64 = 120;

    /// Settings for admitting `Executable::IvmProved` (proof-carrying IVM overlays).
    pub mod ivm_proved {
        /// Whether `Executable::IvmProved` is accepted by the execution pipeline.
        ///
        /// Default is `false` until a full end-to-end IVM execution proof system is shipped.
        pub const ENABLED: bool = false;
        /// Skip deterministic replay for circuits that are known to prove full IVM execution semantics.
        pub const SKIP_REPLAY: bool = false;
    }
}

/// Tiered state backend defaults.
pub mod tiered_state {
    use iroha_config_base::util::Bytes;

    /// Disable tiered snapshots by default.
    pub const ENABLED: bool = false;
    /// Keep all keys hot unless explicitly configured.
    pub const HOT_RETAINED_KEYS: usize = 0;
    /// Hot-tier byte budget based on deterministic in-memory WSV sizing (0 = unlimited).
    pub const HOT_RETAINED_BYTES: Bytes<u64> = Bytes(0);
    /// Minimum snapshots to retain newly hot entries before demotion (0 = disabled).
    pub const HOT_RETAINED_GRACE_SNAPSHOTS: u64 = 1;
    /// Optional cold-tier byte budget across snapshots (0 = unlimited).
    pub const MAX_COLD_BYTES: Bytes<u64> = Bytes(0);
    /// Default on-disk root for tiered state snapshots.
    pub const DEFAULT_COLD_STORE_ROOT: &str = "./storage/tiered_state";
    /// Default on-disk root for DA-backed tiered state snapshots.
    pub const DEFAULT_DA_STORE_ROOT: &str = "./storage/da_wsv_snapshots";
    /// Retain the latest two snapshots when enabled.
    pub const MAX_SNAPSHOTS: usize = 2;
}

/// Concurrency defaults for thread pools and global Rayon.
pub mod concurrency {
    /// Minimum scheduler worker threads (0 = auto/physical cores)
    pub const SCHEDULER_MIN: usize = 0;
    /// Maximum scheduler worker threads (0 = auto/physical cores)
    pub const SCHEDULER_MAX: usize = 0;
    /// Global Rayon thread pool size (0 = auto/physical cores)
    pub const RAYON_GLOBAL: usize = 0;
    /// Default stack size (bytes) for scheduler worker threads.
    pub const SCHEDULER_STACK_BYTES: usize = 32 * 1024 * 1024;
    /// Default stack size (bytes) for prover worker threads.
    pub const PROVER_STACK_BYTES: usize = 32 * 1024 * 1024;
    /// Default guest stack size (bytes) for IVM instances.
    pub const GUEST_STACK_BYTES: u64 = 4 * 1024 * 1024;
    /// Maximum guest stack size (bytes) allowed by config (guard runaway reservations).
    pub const GUEST_STACK_BYTES_MAX: u64 = 64 * 1024 * 1024;
    /// Default gas→stack multiplier (bytes of stack available per unit of gas).
    pub const GAS_TO_STACK_MULTIPLIER: u64 = 4;
}

/// Norito codec defaults.
pub mod norito {
    /// Minimum payload size (bytes) to attempt CPU zstd.
    pub const MIN_COMPRESS_BYTES_CPU: usize = 256;
    /// Minimum payload size (bytes) to attempt GPU zstd when available.
    pub const MIN_COMPRESS_BYTES_GPU: usize = 1024 * 1024;
    /// zstd level for medium-size payloads.
    pub const ZSTD_LEVEL_SMALL: i32 = 1;
    /// zstd level for large payloads.
    pub const ZSTD_LEVEL_LARGE: i32 = 3;
    /// GPU zstd level (kept conservative to reduce latency).
    pub const ZSTD_LEVEL_GPU: i32 = 1;
    /// Size threshold distinguishing small vs large for CPU zstd level.
    pub const LARGE_THRESHOLD: usize = 32 * 1024;
    /// Allow GPU compression offload when compiled and available.
    pub const ALLOW_GPU_COMPRESSION: bool = true;
    /// Hard upper bound on Norito archive length after decompression (bytes).
    ///
    /// This limit is enforced before allocations to reject decompression bombs
    /// and other adversarial inputs that advertise extreme lengths. The default
    /// aligns with the RBC store cap to avoid rejecting persisted consensus
    /// payloads.
    pub const MAX_ARCHIVE_LEN: u64 = super::sumeragi::RBC_STORE_MAX_BYTES as u64; // 1 GiB
    /// Small-N threshold for AoS vs NCB adaptive selection in Norito columnar helpers.
    /// Inputs with `n <= AOS_NCB_SMALL_N` are encoded with a two-pass probe (AoS vs NCB, pick smaller).
    pub const AOS_NCB_SMALL_N: usize = 64;
}

/// Hardware acceleration defaults (Metal/CUDA usage in IVM and helpers).
pub mod accel {
    /// Enable SIMD acceleration (NEON/AVX/SSE) when available.
    pub const ENABLE_SIMD: bool = true;
    /// Enable CUDA backend when compiled and available.
    pub const ENABLE_CUDA: bool = true;
    /// Enable Metal backend on macOS when compiled and available.
    pub const ENABLE_METAL: bool = true;
    /// Maximum number of GPUs to initialize (0 = auto/no cap).
    pub const MAX_GPUS: usize = 0;
    /// Heuristic: minimum number of leaves to use GPU for Merkle leaves hashing.
    pub const MERKLE_MIN_LEAVES_GPU: usize = 8192;
}

/// Zero-knowledge subsystem defaults used by Torii and the host runtime.
pub mod zk {

    /// FASTPQ prover defaults.
    pub mod fastpq {
        /// Default execution mode for the FASTPQ prover (`auto`, `cpu`, or `gpu`).
        pub const EXECUTION_MODE: &str = "auto";
        /// Default Poseidon pipeline mode (mirrors execution mode unless overridden).
        pub const POSEIDON_MODE: &str = "auto";
        /// Optional override for the Metal command-buffer cap (None = derive automatically).
        pub const METAL_MAX_IN_FLIGHT: Option<usize> = None;
        /// Optional override for Metal threadgroup width (None = derive automatically).
        pub const METAL_THREADGROUP_WIDTH: Option<u64> = None;
        /// Whether to emit per-dispatch Metal kernel traces (off by default).
        pub const METAL_TRACE: bool = false;
        /// Whether to log Metal device enumeration details (off by default).
        pub const METAL_DEBUG_ENUM: bool = false;
        /// Whether to dump fused Poseidon pipeline failures (off by default).
        pub const METAL_DEBUG_FUSED: bool = false;
    }
    /// Halo2 verifier configuration for host-side proof checking.
    pub mod halo2 {
        /// Feature toggle for Halo2 verification in hosts.
        pub const ENABLED: bool = false;
        /// Default curve identifier used for Halo2 verification.
        pub const CURVE: &str = "toy_p61_additive";
        /// Backend implementation identifier (e.g., IPA).
        pub const BACKEND: &str = "ipa";
        /// Maximum circuit size expressed as `k` (2^k rows).
        pub const MAX_K: u32 = 16;
        /// Soft wall-clock budget for verification in milliseconds (DA proof bench: 8 MiB / 128 openings about 15 ms max).
        pub const VERIFIER_BUDGET_MS: u64 = 20; // soft budget
        /// Maximum batch size processed in a single verification call.
        pub const VERIFIER_MAX_BATCH: u32 = 16;
        /// Number of ZK lane verifier worker threads (0 = auto by available parallelism).
        pub const VERIFIER_WORKER_THREADS: usize = 0;
        /// Capacity of the ZK lane verifier ingress queue (0 = auto-derived).
        pub const VERIFIER_QUEUE_CAP: usize = 0;
        /// Maximum time spent waiting for ZK lane enqueue under saturation (ms).
        pub const VERIFIER_ENQUEUE_WAIT_MS: u64 = 25;
        /// Capacity of the important-task retry ring used by the ZK lane.
        pub const VERIFIER_RETRY_RING_CAP: usize = 2048;
        /// Maximum retry rounds for an item in the ZK lane retry ring.
        pub const VERIFIER_RETRY_MAX_ATTEMPTS: u32 = 3;
        /// Retry scheduler tick interval for the ZK lane (ms).
        pub const VERIFIER_RETRY_TICK_MS: u64 = 5;
        /// Maximum accepted Norito envelope payload length in bytes.
        pub const MAX_ENVELOPE_BYTES: usize = super::preverify::MAX_BYTES;
        /// Maximum accepted proof length in bytes after Norito encoding.
        pub const MAX_PROOF_BYTES: usize = 192 * 1024;
        /// Maximum accepted transcript label length in bytes.
        pub const MAX_TRANSCRIPT_LABEL_LEN: usize = 64;
        /// Whether transcript labels must be ASCII.
        pub const ENFORCE_TRANSCRIPT_LABEL_ASCII: bool = true;
    }
    /// Native STARK/FRI verifier configuration defaults.
    pub mod stark {
        /// Runtime toggle for STARK verification in hosts.
        ///
        /// Acceptance still requires binaries built with `zk-stark`; this default
        /// remains `false` so operators must explicitly opt in at runtime.
        pub const ENABLED: bool = false;
        /// Maximum accepted proof payload length (bytes).
        ///
        /// The native `stark/fri-v1/*` verifier enforces additional structural caps
        /// during decoding; this limit is an early, coarse safeguard.
        pub const MAX_PROOF_BYTES: usize = 1024 * 1024; // 1 MiB
    }
    /// Stateless pre-verification defaults.
    pub mod preverify {
        /// Maximum accepted proof size (bytes) for pre-verification.
        /// Larger proofs are rejected with `ProofTooBig` before any further checks.
        pub const MAX_BYTES: usize = 1024 * 1024; // 1 MiB
        /// Soft byte-budget for pre-verification work (0 = unlimited).
        /// If non-zero and the proof size exceeds this budget, `PreverifyBudgetExceeded` is returned.
        pub const BUDGET_BYTES: u64 = 0;
    }
    /// Shielded ledger/state defaults.
    pub mod ledger {
        /// Maximum number of recent Merkle roots to keep for shielded assets.
        /// Kept modest to bound memory while allowing lookback for typical proofs.
        pub const ROOT_HISTORY_CAP: usize = 2048;
        /// Whether to include an explicit empty-tree root in read APIs when an asset has no commitments.
        pub const EMPTY_ROOT_ON_EMPTY: bool = false;
        /// Default depth to use when computing the explicit empty-tree root.
        pub const EMPTY_ROOT_DEPTH: u8 = 0;
    }
    /// ZK voting/election defaults.
    pub mod vote {
        /// Maximum number of recent ballot ciphertexts to keep per election.
        pub const BALLOT_HISTORY_CAP: usize = 1024;
    }
    /// Proof registry defaults.
    pub mod proof {
        /// Maximum number of recent proof records to retain per backend (0 = unlimited).
        pub const RECORD_HISTORY_CAP: usize = 4096;
        /// Grace window (in blocks) to keep proof records even if over capacity.
        pub const RETENTION_GRACE_BLOCKS: u64 = 256;
        /// Maximum number of proof records pruned in a single enforcement pass (0 = unlimited).
        pub const PRUNE_BATCH_SIZE: usize = 512;
        /// Maximum length of a bridge proof range (`end_height - start_height + 1`, 0 = unlimited).
        pub const BRIDGE_MAX_RANGE_LEN: u64 = 4096;
        /// Maximum age (in blocks) a bridge proof's end height may trail the current block (0 = unlimited).
        pub const BRIDGE_MAX_PAST_AGE_BLOCKS: u64 = 0;
        /// Maximum future drift (in blocks) a bridge proof's end height may lead the current block (0 = unlimited).
        pub const BRIDGE_MAX_FUTURE_DRIFT_BLOCKS: u64 = 0;
    }
}

/// Sumeragi (consensus) defaults
pub mod sumeragi {
    use std::num::{NonZeroU64, NonZeroUsize};

    use iroha_crypto::Algorithm;
    use nonzero_ext::nonzero;

    /// Number of collectors to use (K). Default is 1 (single proxy tail).
    pub const COLLECTORS_K: usize = 1;
    /// Redundant send fanout (r): how many distinct collectors a validator sends to over time.
    /// Default targets 2f+1 for a 4-peer topology (r=3).
    pub const COLLECTORS_REDUNDANT_SEND_R: u8 = 3;
    /// Extra topology fanout alongside collector routing (0 = disabled).
    pub const COLLECTORS_PARALLEL_TOPOLOGY_FANOUT: usize = 1;
    /// Validator-set size threshold where deterministic active-subset fanout engages.
    pub const FANOUT_LARGE_SET_THRESHOLD: u32 = 256;
    /// Number of finalized blocks to inspect when scoring validator activity.
    pub const FANOUT_ACTIVITY_LOOKBACK_BLOCKS: u32 = 128;
    /// Optional cap on transactions per block (None = unlimited).
    pub const BLOCK_MAX_TRANSACTIONS: Option<NonZeroUsize> = None;
    /// Commit-time threshold (ms) for applying fast-finality proposal caps.
    pub const FAST_FINALITY_COMMIT_TIME_MS: u64 = 1_000;
    /// Optional cap on block gas limit when commit time is <= fast-finality threshold.
    pub const FAST_FINALITY_GAS_LIMIT_PER_BLOCK: Option<NonZeroU64> = None;
    /// Optional cap on payload bytes per block when RBC is disabled (None = unlimited).
    pub const BLOCK_MAX_PAYLOAD_BYTES: Option<NonZeroUsize> = None;
    /// Multiplier applied to the proposal queue scan budget (relative to max tx per block).
    pub const PROPOSAL_QUEUE_SCAN_MULTIPLIER: NonZeroUsize = nonzero!(4_usize);
    /// Maximum DA commitments (blobs) permitted in a single block.
    pub const DA_MAX_COMMITMENTS_PER_BLOCK: usize = 16;
    /// Maximum DA proof openings permitted in a single block (aggregate cap).
    pub const DA_MAX_PROOF_OPENINGS_PER_BLOCK: usize = 128;
    /// Default capacity for the vote message channel.
    pub const MSG_CHANNEL_CAP_VOTES: usize = 8_192;
    /// Default capacity for the block payload channel.
    pub const MSG_CHANNEL_CAP_BLOCK_PAYLOAD: usize = 128;
    /// Default capacity for the RBC chunk channel.
    pub const MSG_CHANNEL_CAP_RBC_CHUNKS: usize = 2_048;
    /// Default capacity for the fast-path block message channel (BlockCreated, FetchPendingBlock, RBC INIT, params).
    pub const MSG_CHANNEL_CAP_BLOCKS: usize = 256;
    /// Default capacity for Sumeragi control/background/lane channels.
    pub const CONTROL_MSG_CHANNEL_CAP: usize = 1_024;
    /// Cap (ms) on the worker loop's per-iteration time budget.
    pub const WORKER_ITERATION_BUDGET_CAP_MS: u64 = 2_000;
    /// Cap (ms) on worker mailbox draining per iteration.
    pub const WORKER_ITERATION_DRAIN_BUDGET_CAP_MS: u64 = 2_000;
    /// Cap (ms) on per-tick proposal/commit work (0 disables).
    pub const WORKER_TICK_WORK_BUDGET_CAP_MS: u64 = 500;
    /// Enable per-queue parallel ingress workers for the Sumeragi loop.
    pub const WORKER_PARALLEL_INGRESS: bool = true;
    /// Validation worker threads for pre-vote checks (0 = auto).
    pub const VALIDATION_WORKER_THREADS: usize = 0;
    /// Validation worker work-queue capacity per worker (0 = auto).
    pub const VALIDATION_WORK_QUEUE_CAP: usize = 0;
    /// Validation worker result-queue capacity (shared, 0 = auto).
    pub const VALIDATION_RESULT_QUEUE_CAP: usize = 0;
    /// Divisor used to derive queue-full inline-validation cutover from fast-timeout.
    pub const VALIDATION_QUEUE_FULL_INLINE_CUTOVER_DIVISOR: u32 = 2;
    /// QC verify worker threads (0 = auto).
    pub const QC_VERIFY_WORKER_THREADS: usize = 0;
    /// QC verify work queue capacity per worker (0 = auto).
    pub const QC_VERIFY_WORK_QUEUE_CAP: usize = 0;
    /// QC verify result queue capacity (shared, 0 = auto).
    pub const QC_VERIFY_RESULT_QUEUE_CAP: usize = 0;
    /// Cap on deferred vote-validation backlog before dropping inbound votes.
    pub const VALIDATION_PENDING_CAP: usize = 8_192;
    /// Vote burst cap used when block payload backlog is pending.
    pub const WORKER_VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG: usize = 8;
    /// Maximum urgent actor-gate streak before yielding to DA-critical work.
    pub const WORKER_MAX_URGENT_BEFORE_DA_CRITICAL: u32 = 8;
    /// Default runtime consensus mode: "permissioned".
    pub const CONSENSUS_MODE: &str = "permissioned";
    /// Default: allow runtime consensus mode flips driven by on-chain parameters.
    pub const MODE_FLIP_ENABLED: bool = true;
    /// Default: data availability (RBC + availability QC gating) disabled.
    pub const DA_ENABLED: bool = true;
    /// Multiplier for DA commit-quorum timeout (applied to block_time + 4 * commit_time).
    pub const DA_QUORUM_TIMEOUT_MULTIPLIER: u32 = 3;
    /// Multiplier for availability timeout in DA mode.
    pub const DA_AVAILABILITY_TIMEOUT_MULTIPLIER: u32 = 2;
    /// Floor (ms) for availability timeouts to avoid churn on tiny pipelines.
    pub const DA_AVAILABILITY_TIMEOUT_FLOOR_MS: u64 = 2_000;
    /// Default interval between kura persistence retry attempts (milliseconds).
    pub const KURA_STORE_RETRY_INTERVAL_MS: u64 = 1_000;
    /// Default maximum kura persistence retry attempts before aborting the block.
    pub const KURA_STORE_RETRY_MAX_ATTEMPTS: u32 = 5;
    /// Default timeout for inflight commit jobs before aborting (milliseconds).
    pub const COMMIT_INFLIGHT_TIMEOUT_MS: u64 = 30_000;
    /// Commit worker work-queue capacity.
    pub const COMMIT_WORK_QUEUE_CAP: usize = 1;
    /// Commit worker result-queue capacity.
    pub const COMMIT_RESULT_QUEUE_CAP: usize = 1;
    /// Default number of missing-block fetch attempts before falling back to the full topology.
    /// A value of 0 disables signer preference.
    pub const MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS: u32 = 1;
    /// Multiplier applied per retry attempt for missing-block fetch backoff (>=1).
    pub const RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER: u32 = 2;
    /// Ceiling for missing-block fetch retry backoff (milliseconds).
    pub const RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS: u64 = 5_000;
    /// Backlog-aware multiplier applied to quorum-reschedule grace windows.
    pub const VIEW_CHANGE_BACKLOG_EXTENSION_FACTOR: f64 = 1.5;
    /// Maximum additional quorum-reschedule grace window under backlog (milliseconds).
    pub const VIEW_CHANGE_BACKLOG_EXTENSION_CAP_MS: u64 = 200;
    /// TTL for deferred QC missing-payload recovery before escalation (milliseconds).
    pub const DEFERRED_QC_TTL_MS: u64 = 2_000;
    /// Deterministic per-height missing-block attempt cap before hard escalation.
    pub const MISSING_BLOCK_HEIGHT_ATTEMPT_CAP: u32 = 48;
    /// Deterministic per-height missing-block dwell cap before hard escalation (milliseconds).
    /// Defaults to 2 * commit_time with the default 1s commit timeout.
    pub const MISSING_BLOCK_HEIGHT_TTL_MS: u64 = 2_000;
    /// Deterministic per-height attempt cap used by bounded recovery.
    pub const RECOVERY_HEIGHT_ATTEMPT_CAP: u32 = 48;
    /// Deterministic per-height dwell window used by bounded recovery.
    /// Defaults to 2 * commit_time with the default 1s commit timeout.
    pub const RECOVERY_HEIGHT_WINDOW_MS: u64 = 2_000;
    /// Hash-miss threshold before escalating dependency recovery to range pull.
    pub const RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL: u32 = 3;
    /// Deterministic wait window before rotating after a missing-QC recovery attempt (milliseconds).
    pub const RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS: u64 = 1_200;
    /// Maximum forced self-proposal attempts allowed for a single (height, view).
    pub const RECOVERY_MAX_FORCED_PROPOSAL_ATTEMPTS_PER_VIEW: u32 = 1;
    /// Number of deterministic no-roster topology refresh retries allowed per view.
    pub const RECOVERY_NO_ROSTER_REFRESH_RETRY_PER_VIEW: u32 = 1;
    /// Rotate immediately when the missing-QC reacquire window is exhausted.
    pub const RECOVERY_ROTATE_AFTER_REACQUIRE_EXHAUSTED: bool = true;
    /// Number of views where no-roster fallback broadcasts remain allowed.
    pub const RECOVERY_NO_ROSTER_FALLBACK_VIEWS: u32 = 1;
    /// Sidecar mismatch retries before final-drop and canonical-only rebuild.
    pub const SIDECAR_MISMATCH_RETRY_CAP: u32 = 8;
    /// Sidecar mismatch TTL before final-drop (milliseconds).
    /// Defaults to 2 * commit_time with the default 1s commit timeout.
    pub const SIDECAR_MISMATCH_TTL_MS: u64 = 2_000;
    /// Number of hash misses before escalating missing dependencies to range pull.
    pub const RANGE_PULL_ESCALATION_AFTER_HASH_MISSES: u32 = 3;
    /// Height margin used to prune stale missing-block requests once head advances.
    pub const RECOVERY_MISSING_REQUEST_STALE_HEIGHT_MARGIN: u64 = 16;
    /// Maximum deferred block-sync updates retained in memory.
    pub const RECOVERY_PENDING_BLOCK_SYNC_CAP: usize = 256;
    /// Maximum cached proposal entries retained in memory.
    pub const RECOVERY_PENDING_PROPOSAL_CAP: usize = 128;
    /// Missing-block fetch attempts before switching from signer-preferred to aggressive topology.
    pub const RECOVERY_MISSING_FETCH_AGGRESSIVE_AFTER_ATTEMPTS: u32 = 2;
    /// Consecutive membership mismatches required before alerting.
    pub const MEMBERSHIP_MISMATCH_ALERT_THRESHOLD: u32 = 1;
    /// Whether to drop consensus messages from peers with repeated membership mismatches.
    pub const MEMBERSHIP_MISMATCH_FAIL_CLOSED: bool = false;
    /// Default: real BLS signing/verification enabled.
    pub const ENABLE_BLS: bool = true;
    /// Default RBC chunk maximum bytes per chunk.
    pub const RBC_CHUNK_MAX_BYTES: usize = 256 * 1024; // 256 KiB
    /// Optional fanout cap for RBC chunk broadcasts (None = auto based on topology).
    pub const RBC_CHUNK_FANOUT: Option<NonZeroUsize> = None;
    /// Default RBC session TTL (milliseconds) before pruning inactive sessions.
    pub const RBC_SESSION_TTL_MS: u64 = 120_000; // 2 minutes
    /// Maximum RBC sessions rebroadcast per tick to avoid payload storms.
    pub const RBC_REBROADCAST_SESSIONS_PER_TICK: usize = 8;
    /// Maximum RBC payload chunks broadcast per tick to avoid bursty floods.
    pub const RBC_PAYLOAD_CHUNKS_PER_TICK: usize = 64;
    /// Default maximum number of persisted RBC session summaries kept on disk.
    pub const RBC_STORE_MAX_SESSIONS: usize = 4096;
    /// Default soft quota for persisted RBC sessions. Back-pressure engages beyond this.
    pub const RBC_STORE_SOFT_SESSIONS: usize = (RBC_STORE_MAX_SESSIONS * 3) / 4;
    /// Default maximum total bytes of persisted RBC session payloads on disk.
    pub const RBC_STORE_MAX_BYTES: usize = 2 * 1024 * 1024 * 1024; // 2 GiB
    /// Default soft quota for persisted RBC payload bytes. Compaction triggers beyond this.
    pub const RBC_STORE_SOFT_BYTES: usize = (RBC_STORE_MAX_BYTES * 3) / 4;
    /// Default disk-backed RBC chunk retention TTL (milliseconds).
    pub const RBC_DISK_STORE_TTL_MS: u64 = RBC_SESSION_TTL_MS;
    /// Default maximum bytes allocated for disk-backed RBC chunks.
    pub const RBC_DISK_STORE_MAX_BYTES: u64 = RBC_STORE_MAX_BYTES as u64;
    /// Default maximum number of RBC chunks stashed before INIT per session.
    ///
    /// Keep this aligned with Sumeragi's maximum total chunks per RBC session (currently `1024`)
    /// so that lowering `rbc.chunk_max_bytes` does not inadvertently reduce the effective pending
    /// byte budget below [`RBC_PENDING_MAX_BYTES`].
    pub const RBC_PENDING_MAX_CHUNKS: usize = 1024;
    /// Default maximum pending RBC chunk bytes per session before INIT.
    pub const RBC_PENDING_MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
    /// Default maximum pending RBC sessions stashed before INIT.
    pub const RBC_PENDING_SESSION_LIMIT: usize = 256;
    /// Default TTL (milliseconds) for pending RBC stashes awaiting INIT.
    /// Align with session TTL so partitions don't evict pre-INIT evidence early.
    pub const RBC_PENDING_TTL_MS: u64 = RBC_SESSION_TTL_MS;
    /// Default: do not force RBC deliver quorum to 1.
    pub const DEBUG_RBC_FORCE_DELIVER_QUORUM_ONE: bool = false;
    /// Maximum height delta accepted for inbound consensus messages (0 disables future gating).
    pub const CONSENSUS_FUTURE_HEIGHT_WINDOW: u64 = 8;
    /// Maximum view delta accepted for inbound consensus messages (0 disables future gating).
    pub const CONSENSUS_FUTURE_VIEW_WINDOW: u64 = 8;
    /// Invalid signature count before temporarily suppressing a signer (0 disables).
    pub const INVALID_SIG_PENALTY_THRESHOLD: u32 = 3;
    /// Window (ms) for invalid signature penalty counting.
    pub const INVALID_SIG_PENALTY_WINDOW_MS: u64 = 5_000;
    /// Cooldown (ms) applied after invalid signature penalties trigger.
    pub const INVALID_SIG_PENALTY_COOLDOWN_MS: u64 = 15_000;
    /// Default cap for in-memory commit certificate history.
    pub const COMMIT_CERT_HISTORY_CAP: usize = 512;
    /// Default epoch length in blocks (NPoS).
    pub const EPOCH_LENGTH_BLOCKS: u64 = 3600;
    /// Default VRF commit deadline offset from epoch start (blocks).
    pub const VRF_COMMIT_DEADLINE_OFFSET: u64 = 100;
    /// Default VRF reveal deadline offset from epoch start (blocks).
    pub const VRF_REVEAL_DEADLINE_OFFSET: u64 = 140;
    /// Use stake snapshot provider for epoch validator roster (NPoS). Default: false.
    pub const USE_STAKE_SNAPSHOT_ROSTER: bool = false;
    /// Default proof policy: off (no additional validity gating beyond consensus).
    pub const PROOF_POLICY: &str = "off";
    /// Default zk parent-proving depth (0 disables zk finality gate).
    pub const ZK_FINALITY_K: u8 = 0;
    /// Default: do not require PrecommitQC unless explicitly enabled.
    pub const REQUIRE_PRECOMMIT_QC: bool = true;
    /// Enable adaptive observability/auto-mitigation hooks by default.
    pub const ADAPTIVE_OBSERVABILITY_ENABLED: bool = false;
    /// Threshold (ms) for observed QC latency before adaptive mitigation engages.
    pub const ADAPTIVE_QC_LATENCY_ALERT_MS: u64 = 400;
    /// Minimum number of DA reschedules observed between checks before mitigation engages.
    pub const ADAPTIVE_DA_RESCHEDULE_BURST: u64 = 2;
    /// Additional pacemaker interval (ms) applied when mitigation engages.
    pub const ADAPTIVE_PACEMAKER_EXTRA_MS: u64 = 100;
    /// Redundant collector fan-out cap when mitigation engages.
    pub const ADAPTIVE_COLLECTOR_REDUNDANT_R: u8 = 3;
    /// Cooldown (ms) before adaptive mitigation may re-apply or reset after a trigger.
    pub const ADAPTIVE_COOLDOWN_MS: u64 = 5_000;
    /// Number of recent blocks to sample for the pacing governor window.
    pub const PACING_GOVERNOR_WINDOW_BLOCKS: usize = 20;
    /// View-change pressure threshold (permille of view-change increments per block).
    pub const PACING_GOVERNOR_VIEW_CHANGE_PRESSURE_PERMILLE: u32 = 200;
    /// View-change pressure clear threshold (permille of view-change increments per block).
    pub const PACING_GOVERNOR_VIEW_CHANGE_CLEAR_PERMILLE: u32 = 50;
    /// Commit spacing pressure threshold (permille of target block time).
    pub const PACING_GOVERNOR_COMMIT_SPACING_PRESSURE_PERMILLE: u32 = 1_300;
    /// Commit spacing clear threshold (permille of target block time).
    pub const PACING_GOVERNOR_COMMIT_SPACING_CLEAR_PERMILLE: u32 = 1_100;
    /// Pacing factor step-up (basis points).
    pub const PACING_GOVERNOR_STEP_UP_BPS: u32 = 1_000;
    /// Pacing factor step-down (basis points).
    pub const PACING_GOVERNOR_STEP_DOWN_BPS: u32 = 100;
    /// Minimum pacing factor (basis points).
    pub const PACING_GOVERNOR_MIN_FACTOR_BPS: u32 = 10_000;
    /// Maximum pacing factor (basis points).
    pub const PACING_GOVERNOR_MAX_FACTOR_BPS: u32 = 20_000;
    /// Pacemaker backoff multiplier for view-change time increments (>=1).
    pub const PACEMAKER_BACKOFF_MULTIPLIER: u32 = 1;
    /// Pacemaker RTT floor multiplier applied to avg RTT (>=1).
    pub const PACEMAKER_RTT_FLOOR_MULTIPLIER: u32 = 2;
    /// Pacemaker maximum backoff cap in milliseconds.
    pub const PACEMAKER_MAX_BACKOFF_MS: u64 = 10_000; // 10 seconds to keep localnet responsive
    /// Pacemaker jitter band size as permille of the backoff window (0..=1000). 0 disables jitter.
    pub const PACEMAKER_JITTER_FRAC_PERMILLE: u32 = 0;
    /// Grace period (ms) before a pending block counts as stalled for pacemaker backpressure.
    pub const PACEMAKER_PENDING_STALL_GRACE_MS: u64 = 250;
    /// Allow fast quorum reschedules in DA mode when payloads are locally available.
    pub const PACEMAKER_DA_FAST_RESCHEDULE: bool = false;
    /// Soft limit for blocking pending blocks before pacemaker backpressure defers proposals.
    /// 0 keeps strict gating (any pending block defers).
    pub const PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT: usize = 1;
    /// Soft limit for unresolved RBC backlog sessions before pacemaker backpressure defers proposals.
    /// 0 keeps strict gating (any backlog session defers).
    pub const PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT: usize = 8;
    /// Soft limit for missing RBC chunks before pacemaker backpressure defers proposals.
    /// 0 keeps strict gating (any missing chunks defers).
    pub const PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT: usize = 256;
    /// Permissioned default block time (ms); keep aligned with on-chain defaults.
    pub const BLOCK_TIME_MS: u64 = 100;
    /// Base pacing factor for adaptive timing (basis points, 10_000 = 1.0x).
    pub const PACING_FACTOR_BPS: u32 = 10_000;
    /// Minimum lead time (blocks) between publishing a new consensus key and its activation.
    pub const KEY_ACTIVATION_LEAD_BLOCKS: u64 = 1;
    /// Grace/overlap window (blocks) during which both old and new keys remain valid.
    pub const KEY_OVERLAP_GRACE_BLOCKS: u64 = 8;
    /// Expiry grace window (blocks) after the declared expiry height.
    pub const KEY_EXPIRY_GRACE_BLOCKS: u64 = 0;
    /// Require HSM binding for consensus/committee keys by default.
    pub const KEY_REQUIRE_HSM: bool = false;
    /// Allowed consensus key algorithms (validator signatures use BLS-Normal).
    pub const KEY_ALLOWED_ALGOS: &[Algorithm] = &[Algorithm::BlsNormal];
    /// Allowed HSM providers for consensus keys.
    pub const KEY_ALLOWED_HSM_PROVIDERS: &[&str] = &["pkcs11", "yubihsm", "softkey"];
    /// Default list of allowed consensus key algorithms.
    pub fn key_allowed_algorithms() -> Vec<Algorithm> {
        KEY_ALLOWED_ALGOS.to_vec()
    }
    /// Default list of allowed HSM providers for consensus keys.
    pub fn key_allowed_hsm_providers() -> Vec<String> {
        KEY_ALLOWED_HSM_PROVIDERS
            .iter()
            .map(|s: &&str| (*s).to_string())
            .collect()
    }

    /// NPoS-specific defaults.
    pub mod npos {
        /// Target block time in milliseconds (1s).
        pub const BLOCK_TIME_MS: u64 = 1_000;
        /// Timeout for proposal broadcast/gather (ms).
        pub const TIMEOUT_PROPOSE_MS: u64 = 350;
        /// Timeout for prevote aggregation (ms).
        pub const TIMEOUT_PREVOTE_MS: u64 = 450;
        /// Timeout for precommit aggregation (ms).
        pub const TIMEOUT_PRECOMMIT_MS: u64 = 550;
        /// Timeout for execution QC aggregation (ms).
        pub const TIMEOUT_EXEC_MS: u64 = 150;
        /// Timeout for witness availability QC aggregation (ms).
        pub const TIMEOUT_WITNESS_MS: u64 = 150;
        /// Timeout for final commit confirmation (ms).
        pub const TIMEOUT_COMMIT_MS: u64 = 850;
        /// Timeout for data-availability quorum formation (ms).
        pub const TIMEOUT_DA_MS: u64 = 750;
        /// Timeout before gossip fanout to non-designated aggregators (ms).
        pub const TIMEOUT_AGG_MS: u64 = 120;
        /// Default number of aggregators (K) used in NPoS mode.
        pub const K_AGGREGATORS: usize = 3;
        /// Default redundant send fanout (r) for NPoS validators (2f+1 for 4 peers).
        pub const REDUNDANT_SEND_R: u8 = 3;
        /// VRF commitment window size in blocks from epoch start.
        pub const VRF_COMMIT_WINDOW_BLOCKS: u64 = 100;
        /// VRF reveal window size in blocks after the commit window closes.
        pub const VRF_REVEAL_WINDOW_BLOCKS: u64 = 40;
        /// Maximum validators to elect for a given epoch (0 = unlimited).
        pub const MAX_VALIDATORS: u32 = 128;
        /// Minimum self-bond required for validator eligibility (stake units).
        pub const MIN_SELF_BOND: u64 = 1_000;
        /// Minimum nomination bond required for delegators (stake units).
        pub const MIN_NOMINATION_BOND: u64 = 1;
        /// Maximum share of nominations from a single nominator (percentage 0-100).
        pub const MAX_NOMINATOR_CONCENTRATION_PCT: u8 = 25;
        /// Acceptable band for seat allocation variance (percentage 0-100).
        pub const SEAT_BAND_PCT: u8 = 5;
        /// Maximum correlated ownership across validators (percentage 0-100).
        pub const MAX_ENTITY_CORRELATION_PCT: u8 = 25;
        /// Number of blocks to retain evidence for governance actions.
        pub const RECONFIG_EVIDENCE_HORIZON_BLOCKS: u64 = 7_200;
        /// Activation lag in blocks for newly scheduled validator sets.
        pub const RECONFIG_ACTIVATION_LAG_BLOCKS: u64 = 1;
        /// Slashing delay in blocks before evidence penalties apply (3 days at 1s block time).
        pub const SLASHING_DELAY_BLOCKS: u64 = 259_200;
        /// Finality margin (blocks) required before activating a newly elected set.
        pub const FINALITY_MARGIN_BLOCKS: u64 = 8;
    }
}

/// Governance defaults (voting & parliament).
pub mod governance {
    use super::*;

    /// Default asset definition used for governance voting and bonds.
    pub const VOTING_ASSET_ID: &str = "xor#sora";
    /// Default asset definition used for citizenship bonding (mirrors voting asset).
    pub const CITIZENSHIP_ASSET_ID: &str = VOTING_ASSET_ID;
    /// Default public key used for governance escrow account derivation.
    pub const BOND_ESCROW_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    /// Default citizenship bond requirement (smallest units).
    pub const CITIZENSHIP_BOND_AMOUNT: u128 = 150;

    /// Default asset definition used for governance voting and bonds.
    pub fn voting_asset_id() -> String {
        VOTING_ASSET_ID.to_string()
    }

    fn account_id_from_public_key(public_key: &str) -> AccountId {
        let domain: DomainId = super::common::default_account_domain_label()
            .parse()
            .expect("default governance account domain");
        let public_key = public_key.parse().expect("default governance public key");
        let _ = domain;
        AccountId::new(public_key)
    }

    fn account_literal_from_account_id(account_id: &AccountId) -> String {
        // Configuration parsing accepts canonical I105 account literals.
        account_id
            .canonical_i105()
            .expect("default governance account literal")
    }

    fn account_literal_from_public_key(public_key: &str) -> String {
        account_literal_from_account_id(&account_id_from_public_key(public_key))
    }

    fn default_governance_account_id() -> AccountId {
        account_id_from_public_key(BOND_ESCROW_PUBLIC_KEY)
    }

    fn default_governance_account_literal() -> String {
        account_literal_from_account_id(&default_governance_account_id())
    }

    /// Default escrow account that custodies governance bonds.
    pub fn bond_escrow_account_id() -> AccountId {
        default_governance_account_id()
    }

    /// Default escrow account literal that custodies governance bonds.
    pub fn bond_escrow_account() -> String {
        default_governance_account_literal()
    }

    /// Default asset definition used for citizenship bonding.
    pub fn citizenship_asset_id() -> String {
        CITIZENSHIP_ASSET_ID.to_string()
    }

    /// Default escrow account that holds citizenship bonds.
    pub fn citizenship_escrow_account_id() -> AccountId {
        default_governance_account_id()
    }

    /// Default escrow account literal that holds citizenship bonds.
    pub fn citizenship_escrow_account() -> String {
        default_governance_account_literal()
    }

    /// Default receiver for slashed governance bonds.
    pub fn slash_receiver_account_id() -> AccountId {
        default_governance_account_id()
    }

    /// Default literal receiver for slashed governance bonds.
    pub fn slash_receiver_account() -> String {
        default_governance_account_literal()
    }

    /// Default citizenship bond requirement (smallest units).
    pub const fn citizenship_bond_amount() -> u128 {
        CITIZENSHIP_BOND_AMOUNT
    }

    /// Default minimum TEU balance required for alias admission.
    pub const ALIAS_TEU_MINIMUM: u128 = 0;
    /// Emit alias frontier telemetry by default.
    pub const ALIAS_FRONTIER_TELEMETRY: bool = true;
    /// Emit governance pipeline trace logs.
    pub const DEBUG_TRACE_PIPELINE: bool = false;
    /// Default JDG signature schemes accepted during attestation validation.
    pub const JDG_SIGNATURE_SCHEMES: &[&str] = &["simple_threshold"];
    /// Default runtime-upgrade provenance enforcement mode.
    pub const RUNTIME_UPGRADE_PROVENANCE_MODE: &str = "optional";
    /// Require SBOM digests for runtime-upgrade provenance.
    pub const RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SBOM: bool = false;
    /// Require SLSA attestation bytes for runtime-upgrade provenance.
    pub const RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SLSA: bool = false;
    /// Default signature threshold for runtime-upgrade provenance.
    pub const RUNTIME_UPGRADE_PROVENANCE_SIGNATURE_THRESHOLD: usize = 0;

    /// Default TEU minimum required for alias admission.
    pub const fn alias_teu_minimum() -> u128 {
        ALIAS_TEU_MINIMUM
    }

    /// Default toggle for emitting alias frontier telemetry.
    pub const fn alias_frontier_telemetry() -> bool {
        ALIAS_FRONTIER_TELEMETRY
    }

    /// Default JDG signature scheme allow-list.
    pub fn jdg_signature_schemes() -> Vec<String> {
        JDG_SIGNATURE_SCHEMES
            .iter()
            .copied()
            .map(str::to_string)
            .collect()
    }

    /// Default sortition council committee size.
    pub const PARLIAMENT_COMMITTEE_SIZE: usize = 21;
    /// Default term length for the council (blocks). ~12h at 1s blocks.
    pub const PARLIAMENT_TERM_BLOCKS: u64 = 43_200;
    /// Minimum stake required to qualify for council selection (smallest units).
    pub const PARLIAMENT_MIN_STAKE: u128 = 1;
    /// Default stake asset definition used for council eligibility.
    pub const PARLIAMENT_ELIGIBILITY_ASSET_ID: &str = "SORA#stake";
    /// Default alternates drawn per parliament term (None = committee size).
    pub const PARLIAMENT_ALTERNATE_SIZE: Option<usize> = None;
    /// Default council quorum requirement expressed in basis points (ceil-divided).
    pub const PARLIAMENT_QUORUM_BPS: u16 = 6_667;
    /// Default Rules Committee size.
    pub const PARLIAMENT_RULES_COMMITTEE_SIZE: usize = 7;
    /// Default Agenda Council size.
    pub const PARLIAMENT_AGENDA_COUNCIL_SIZE: usize = 9;
    /// Default Interest Panel size.
    pub const PARLIAMENT_INTEREST_PANEL_SIZE: usize = 11;
    /// Default Review Panel size.
    pub const PARLIAMENT_REVIEW_PANEL_SIZE: usize = 13;
    /// Default Policy Jury size (larger to cover high-impact items).
    pub const PARLIAMENT_POLICY_JURY_SIZE: usize = 25;
    /// Default Oversight Committee size.
    pub const PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE: usize = 7;
    /// Default MPC/FMA board size.
    pub const PARLIAMENT_FMA_COMMITTEE_SIZE: usize = 5;
    /// Default citizen service cooldown in blocks after accepting a seat.
    pub const CITIZEN_SEAT_COOLDOWN_BLOCKS: u64 = 10_000;
    /// Default maximum seats a single citizen may hold per epoch.
    pub const CITIZEN_MAX_SEATS_PER_EPOCH: u32 = 1;
    /// Default number of declines that do not trigger a slash per epoch.
    pub const CITIZEN_FREE_DECLINES_PER_EPOCH: u32 = 1;
    /// Slash applied when a citizen declines after exhausting the free budget (basis points).
    pub const CITIZEN_DECLINE_SLASH_BPS: u16 = 250;
    /// Slash applied when a citizen fails to appear for an assigned seat (basis points).
    pub const CITIZEN_NO_SHOW_SLASH_BPS: u16 = 1_000;
    /// Slash applied when misconduct is recorded for an assigned seat (basis points).
    pub const CITIZEN_MISCONDUCT_SLASH_BPS: u16 = 5_000;
    /// Default SLA (blocks) for opening a referendum after proposal submission.
    pub const PIPELINE_STUDY_SLA_BLOCKS: u64 = 20;
    /// Default SLA (blocks) for the referendum voting window.
    pub const PIPELINE_REVIEW_SLA_BLOCKS: u64 = 100;
    /// Default SLA (blocks) for recording the referendum decision.
    pub const PIPELINE_DECISION_SLA_BLOCKS: u64 = 1;
    /// Default SLA (blocks) to enact an approved proposal after decision.
    pub const PIPELINE_ENACTMENT_SLA_BLOCKS: u64 = 200;
    /// Default SLA (blocks) for rules committee approval.
    pub const PIPELINE_RULES_SLA_BLOCKS: u64 = 20;
    /// Default SLA (blocks) for agenda council scheduling.
    pub const PIPELINE_AGENDA_SLA_BLOCKS: u64 = 40;
    /// Default reward asset definition (mirrors governance voting asset).
    pub const VIRAL_REWARD_ASSET_ID: &str = VOTING_ASSET_ID;
    /// Default per-binding reward amount ("1" XOR).
    pub const VIRAL_FOLLOW_REWARD_AMOUNT: &str = "1";
    /// Default sender bonus amount ("0.1" XOR).
    pub const VIRAL_SENDER_BONUS_AMOUNT: &str = "0.1";
    /// Default maximum rewards a UAID may claim per day.
    pub const VIRAL_MAX_DAILY_CLAIMS_PER_UAID: u32 = 1;
    /// Default maximum rewards per binding (lifetime).
    pub const VIRAL_MAX_CLAIMS_PER_BINDING: u32 = 1;
    /// Default daily budget for viral rewards (in reward asset units).
    pub const VIRAL_DAILY_BUDGET: &str = "1000";

    /// Default incentive pool account identifier.
    pub fn viral_incentive_pool_account() -> String {
        slash_receiver_account()
    }

    /// Default escrow account identifier.
    pub fn viral_escrow_account() -> String {
        slash_receiver_account()
    }

    /// Default viral reward asset definition identifier.
    pub fn viral_reward_asset_id() -> String {
        VIRAL_REWARD_ASSET_ID.to_string()
    }

    /// Default viral follow reward amount.
    pub fn viral_follow_reward_amount() -> Numeric {
        Numeric::from_str(VIRAL_FOLLOW_REWARD_AMOUNT).expect("default viral follow reward amount")
    }

    /// Default sender bonus amount for first delivery.
    pub fn viral_sender_bonus_amount() -> Numeric {
        Numeric::from_str(VIRAL_SENDER_BONUS_AMOUNT).expect("default viral sender bonus amount")
    }

    /// Default daily viral reward budget.
    pub fn viral_daily_budget() -> Numeric {
        Numeric::from_str(VIRAL_DAILY_BUDGET).expect("default viral daily budget")
    }

    /// Optional promotion start timestamp (ms since Unix epoch).
    pub fn viral_promo_start_ms() -> Option<u64> {
        None
    }

    /// Optional promotion end timestamp (ms since Unix epoch).
    pub fn viral_promo_end_ms() -> Option<u64> {
        None
    }

    /// Aggregate campaign cap across the promo window (0 = unlimited).
    pub fn viral_campaign_cap() -> Numeric {
        Numeric::zero()
    }

    /// Default citizen service discipline parameters.
    pub mod citizen_service {
        use std::collections::BTreeMap;

        use crate::parameters::defaults::governance::{
            CITIZEN_DECLINE_SLASH_BPS, CITIZEN_FREE_DECLINES_PER_EPOCH,
            CITIZEN_MAX_SEATS_PER_EPOCH, CITIZEN_MISCONDUCT_SLASH_BPS, CITIZEN_NO_SHOW_SLASH_BPS,
            CITIZEN_SEAT_COOLDOWN_BLOCKS,
        };

        /// Default service cooldown (blocks) after accepting a seat.
        pub const SEAT_COOLDOWN_BLOCKS: u64 = CITIZEN_SEAT_COOLDOWN_BLOCKS;
        /// Default maximum seats a citizen may hold per epoch.
        pub const MAX_SEATS_PER_EPOCH: u32 = CITIZEN_MAX_SEATS_PER_EPOCH;
        /// Default number of free declines per epoch.
        pub const FREE_DECLINES_PER_EPOCH: u32 = CITIZEN_FREE_DECLINES_PER_EPOCH;
        /// Slash percentage applied to declines beyond the free budget (basis points).
        pub const DECLINE_SLASH_BPS: u16 = CITIZEN_DECLINE_SLASH_BPS;
        /// Slash percentage applied to no-show events (basis points).
        pub const NO_SHOW_SLASH_BPS: u16 = CITIZEN_NO_SHOW_SLASH_BPS;
        /// Slash percentage applied to misconduct events (basis points).
        pub const MISCONDUCT_SLASH_BPS: u16 = CITIZEN_MISCONDUCT_SLASH_BPS;

        /// Default role bond multipliers (empty map = multiplier of 1 for all roles).
        pub fn role_bond_multipliers() -> BTreeMap<String, u64> {
            BTreeMap::new()
        }
    }

    /// Default SoraFS pin policy constraints enforced by governance.
    pub mod sorafs_pin_policy {
        /// Minimum replicas floor required for approved manifests.
        pub const MIN_REPLICAS_FLOOR: u16 = 1;
        /// Maximum replicas ceiling allowed for approved manifests (inclusive).
        pub const MAX_REPLICAS_CEILING: Option<u16> = Some(5);
        /// Optional maximum retention epoch (inclusive); `None` disables the cap.
        pub const MAX_RETENTION_EPOCH: Option<u64> = None;
    }

    /// Default SoraFS under-delivery penalty policy thresholds.
    pub mod sorafs_penalty {
        /// Minimum utilisation ratio (basis points) required before counting a strike.
        pub const UTILISATION_FLOOR_BPS: u16 = 7_500;
        /// Minimum uptime success rate (basis points) before counting a strike.
        pub const UPTIME_FLOOR_BPS: u16 = 9_500;
        /// Minimum PoR success rate (basis points) before counting a strike.
        pub const POR_SUCCESS_FLOOR_BPS: u16 = 9_700;
        /// Number of consecutive strikes required before issuing a slash.
        pub const STRIKE_THRESHOLD: u32 = 3;
        /// Percentage of bonded collateral slashed when penalties trigger (basis points).
        pub const PENALTY_BOND_BPS: u16 = 2_500;
        /// Cooldown window count (in settlement windows) before another penalty may trigger.
        pub const COOLDOWN_WINDOWS: u32 = 2;
        /// Maximum PDP failures tolerated within a telemetry window before forcing a strike (0 = none).
        pub const MAX_PDP_FAILURES: u32 = 0;
        /// Maximum PoTR SLA breaches tolerated within a telemetry window before forcing a strike (0 = none).
        pub const MAX_POTR_BREACHES: u32 = 0;
    }

    /// Default SoraFS repair escalation governance policy.
    pub mod sorafs_repair_escalation {
        /// Minimum approval ratio required for escalation/slash decisions (basis points).
        pub const QUORUM_BPS: u16 = 6_667;
        /// Minimum number of distinct voters required to resolve a decision.
        pub const MINIMUM_VOTERS: u32 = 3;
        /// Dispute window in seconds after escalation before governance finalizes.
        pub const DISPUTE_WINDOW_SECS: u64 = 24 * 60 * 60;
        /// Appeal window in seconds after approval before a decision is final.
        pub const APPEAL_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;
        /// Maximum slash penalty allowed for repair escalation proposals (nano-XOR).
        pub const MAX_PENALTY_NANO: u128 =
            crate::parameters::defaults::sorafs::repair::DEFAULT_SLASH_PENALTY_NANO;
    }

    /// Default authentication and validation policy for SoraFS telemetry.
    pub mod sorafs_telemetry {
        /// Default telemetry submitter public key (development profile).
        pub const DEFAULT_SUBMITTER_PUBLIC_KEY: &str =
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C";

        /// Require telemetry submissions to originate from an authorised submitter list.
        pub const REQUIRE_SUBMITTER: bool = true;
        /// Require telemetry windows to carry a nonce for replay protection.
        /// Windows without a nonce are accepted only when this is false, but provided nonces
        /// are still checked for replay regardless.
        pub const REQUIRE_NONCE: bool = true;
        /// Maximum tolerated gap between accepted telemetry windows (seconds).
        pub const MAX_WINDOW_GAP_SECS: u64 = 6 * 60 * 60;
        /// Reject telemetry that reports zero capacity to avoid zero-fee windows.
        pub const REJECT_ZERO_CAPACITY: bool = true;

        /// Default authorised submitter accounts (development only).
        pub fn submitters() -> Vec<String> {
            vec![super::account_literal_from_public_key(
                DEFAULT_SUBMITTER_PUBLIC_KEY,
            )]
        }
    }

    /// Default governance bond slashing policy.
    pub mod slash_policy {
        /// Percentage of the locked bond slashed when a double-vote is detected (basis points).
        pub const DOUBLE_VOTE_BPS: u16 = 2_500;
        /// Percentage of the locked bond slashed for ineligible proofs (basis points).
        pub const INELIGIBLE_PROOF_BPS: u16 = 1_500;
        /// Percentage of the locked bond slashed for misconduct (basis points).
        pub const MISCONDUCT_BPS: u16 = 5_000;
        /// Appeal window (in blocks) after which restitution requests should be rejected by default.
        pub const RESTITUTION_WINDOW_BLOCKS: u64 = 7_200;
    }
}

/// Confidential asset/verifier defaults.
pub mod confidential {
    use super::*;

    /// Confidential features disabled by default.
    pub const ENABLED: bool = false;
    /// Observer-only assume-valid disabled by default.
    pub const ASSUME_VALID: bool = false;
    /// Default verifier backend identifier.
    pub const VERIFIER_BACKEND: &str = "halo2-ipa-pallas";
    /// Maximum confidential proof size (bytes).
    pub const MAX_PROOF_SIZE_BYTES: u32 = 262_144;
    /// Maximum nullifiers per transaction.
    pub const MAX_NULLIFIERS_PER_TX: u32 = 8;
    /// Maximum commitments per transaction.
    pub const MAX_COMMITMENTS_PER_TX: u32 = 8;
    /// Maximum confidential ops per block.
    pub const MAX_CONFIDENTIAL_OPS_PER_BLOCK: u32 = 256;
    /// Verifier timeout duration.
    pub const VERIFY_TIMEOUT: Duration = Duration::from_millis(750);
    /// Maximum anchor age in blocks.
    pub const MAX_ANCHOR_AGE_BLOCKS: u64 = 10_000;
    /// Maximum proof bytes per block.
    pub const MAX_PROOF_BYTES_BLOCK: u64 = 1_048_576;
    /// Maximum verification calls per transaction.
    pub const MAX_VERIFY_CALLS_PER_TX: u32 = 4;
    /// Maximum verification calls per block.
    pub const MAX_VERIFY_CALLS_PER_BLOCK: u32 = 128;
    /// Maximum public inputs per proof.
    pub const MAX_PUBLIC_INPUTS: u32 = 32;
    /// Reorg depth bound (must exceed anchor age).
    pub const REORG_DEPTH_BOUND: u64 = 10_000;
    /// Minimum delay between policy change request and activation.
    pub const POLICY_TRANSITION_DELAY_BLOCKS: u64 = 100;
    /// Grace window around policy activation for conversions.
    pub const POLICY_TRANSITION_WINDOW_BLOCKS: u64 = 200;
    /// Commitment tree root history length retained.
    pub const TREE_ROOTS_HISTORY_LEN: u64 = 10_000;
    /// Commitment tree frontier checkpoint interval.
    pub const TREE_FRONTIER_CHECKPOINT_INTERVAL: u64 = 100;
    /// Maximum verifier entries in registry.
    pub const REGISTRY_MAX_VK_ENTRIES: u32 = 64;
    /// Maximum parameter entries in registry.
    pub const REGISTRY_MAX_PARAMS_ENTRIES: u32 = 32;
    /// Maximum registry mutations per block.
    pub const REGISTRY_MAX_DELTA_PER_BLOCK: u32 = 4;
    /// Confidential verification gas schedule defaults.
    pub mod gas {
        /// Base verify cost for a confidential proof.
        pub const PROOF_BASE: u64 = 250_000;
        /// Cost per public input field element.
        pub const PER_PUBLIC_INPUT: u64 = 2_000;
        /// Cost per proof byte.
        pub const PER_PROOF_BYTE: u64 = 5;
        /// Cost per nullifier.
        pub const PER_NULLIFIER: u64 = 300;
        /// Cost per commitment.
        pub const PER_COMMITMENT: u64 = 500;
    }
    /// Default Poseidon parameter set identifier (if any) for confidential policies.
    pub const POSEIDON_PARAMS_ID: Option<u32> = None;
    /// Default Pedersen parameter set identifier (if any) for confidential policies.
    pub const PEDERSEN_PARAMS_ID: Option<u32> = None;
    /// Confidential ruleset version advertised during handshake.
    pub const RULES_VERSION: u32 = iroha_data_model::confidential::CONFIDENTIAL_RULES_VERSION;
}

/// SoraNet privacy telemetry defaults shared by relay runtimes.
pub mod soranet {
    /// Configuration defaults for privacy bucket aggregation.
    pub mod privacy {
        /// Width of each aggregation bucket (seconds).
        pub const BUCKET_SECS: u64 = 60;
        /// Minimum handshakes required before emitting counters.
        pub const MIN_HANDSHAKES: u64 = 12;
        /// Completed bucket delay before attempting a flush.
        pub const FLUSH_DELAY_BUCKETS: u64 = 1;
        /// Maximum bucket age before forcing a suppressed emit.
        pub const FORCE_FLUSH_BUCKETS: u64 = 6;
        /// Number of completed buckets retained for scraping.
        pub const MAX_COMPLETED_BUCKETS: usize = 120;
        /// Expected Prio shares required before combining contributions.
        pub const EXPECTED_SHARES: u16 = 2;
        /// Maximum bucket lag tolerated for collector shares before suppression.
        pub const MAX_SHARE_LAG_BUCKETS: u64 = 12;
        /// Capacity of the in-memory privacy event buffer.
        pub const EVENT_BUFFER_CAPACITY: usize = 4_096;
    }

    /// Defaults for the SoraNet VPN control plane and scheduler.
    pub mod vpn {
        use std::time::Duration;

        /// Enable the VPN tunnel by default.
        pub const ENABLED: bool = true;
        /// Fixed cell size (bytes).
        pub const CELL_SIZE_BYTES: u16 = 1_024;
        /// Flow label width (bits).
        pub const FLOW_LABEL_BITS: u8 = 24;
        /// Cover-to-data ratio expressed in permille.
        pub const COVER_TO_DATA_PER_MILLE: u16 = 250;
        /// Maximum burst of consecutive cover cells.
        pub const MAX_COVER_BURST: u16 = 3;
        /// Heartbeat cadence for keepalives (milliseconds).
        pub const HEARTBEAT_MS: u16 = 500;
        /// Maximum jitter applied to cover/keepalive slots (milliseconds).
        pub const JITTER_MS: u16 = 10;
        /// Padding budget advertised in headers (milliseconds).
        pub const PADDING_BUDGET_MS: u16 = 15;
        /// Guard/exit refresh cadence (seconds).
        pub const GUARD_REFRESH_SECS: Duration = Duration::from_secs(60 * 60);
        /// Control-plane lease duration (seconds).
        pub const LEASE_SECS: Duration = Duration::from_secs(10 * 60);
        /// DNS push interval (seconds).
        pub const DNS_PUSH_INTERVAL_SECS: Duration = Duration::from_secs(90);
        /// Default exit class label for billing/telemetry.
        pub const EXIT_CLASS: &str = "standard";
        /// Default meter family identifier.
        pub const METER_FAMILY: &str = "soranet.vpn.standard";

        /// Returns the default guard refresh cadence.
        #[must_use]
        pub const fn guard_refresh_secs() -> Duration {
            GUARD_REFRESH_SECS
        }

        /// Returns the default lease duration.
        #[must_use]
        pub const fn lease_secs() -> Duration {
            LEASE_SECS
        }

        /// Returns the default DNS push interval.
        #[must_use]
        pub const fn dns_push_interval_secs() -> Duration {
            DNS_PUSH_INTERVAL_SECS
        }

        /// Helper returning the guard refresh cadence in seconds.
        #[must_use]
        pub fn guard_refresh_secs_u64() -> u64 {
            GUARD_REFRESH_SECS.as_secs()
        }

        /// Helper returning the lease duration in seconds.
        #[must_use]
        pub fn lease_secs_u64() -> u64 {
            LEASE_SECS.as_secs()
        }

        /// Helper returning the DNS push interval in seconds.
        #[must_use]
        pub fn dns_push_interval_secs_u64() -> u64 {
            DNS_PUSH_INTERVAL_SECS.as_secs()
        }
    }
}

/// Settlement (repo and related) defaults.
pub mod settlement {
    /// Repo-specific defaults.
    pub mod repo {
        /// Default haircut applied to collateral (basis points).
        pub const DEFAULT_HAIRCUT_BPS: u16 = 1_500;
        /// Default cadence between mandatory margin checks (seconds).
        pub const DEFAULT_MARGIN_FREQUENCY_SECS: u64 = 86_400;
    }
    /// Offline settlement defaults.
    pub mod offline {
        /// Minimum number of blocks to retain settlement bundles in hot storage.
        pub const HOT_RETENTION_BLOCKS: u64 = 86_400;
        /// Maximum number of bundles to archive in a single retention pass.
        pub const ARCHIVE_BATCH_SIZE: usize = 128;
        /// Minimum number of blocks archived bundles remain available before pruning. Zero disables pruning.
        pub const COLD_RETENTION_BLOCKS: u64 = 0;
        /// Maximum number of archived bundles removed in a single prune pass.
        pub const PRUNE_BATCH_SIZE: usize = 128;
        /// Default aggregate-proof enforcement mode for offline bundles.
        pub const PROOF_MODE: &str = "optional";
        /// Default maximum age for offline receipts (ms). Zero disables age checks.
        pub const MAX_RECEIPT_AGE_MS: u64 = 86_400_000;
        /// Whether to skip platform attestation verification (for local testing only).
        pub const SKIP_PLATFORM_ATTESTATION: bool = false;
        /// Whether to skip build claim verification (for local testing only).
        pub const SKIP_BUILD_CLAIM_VERIFICATION: bool = false;
        /// Whether iOS App Attest signatures must verify only against raw `clientDataHash`.
        pub const APPLE_APP_ATTEST_STRICT_SIGNATURE: bool = false;
    }
    /// Router defaults (shadow price, guard rails).
    pub mod router {
        /// Default TWAP window used for conversion quotes (seconds).
        pub const TWAP_WINDOW_SECS: u64 = 60;
        /// Base epsilon margin applied to every quote (basis points).
        pub const EPSILON_BPS: u16 = 25;
        /// Default buffer coverage horizon (hours).
        pub const BUFFER_HORIZON_HOURS: u16 = 72;
        /// Alert threshold percentage for remaining buffer.
        pub const ALERT_PCT: u8 = 75;
        /// Throttle threshold percentage for remaining buffer.
        pub const THROTTLE_PCT: u8 = 25;
        /// XOR-only threshold percentage.
        pub const XOR_ONLY_PCT: u8 = 10;
        /// Halt threshold percentage.
        pub const HALT_PCT: u8 = 2;
    }
}

#[cfg(test)]
mod tests {
    use super::governance;

    #[test]
    fn jdg_signature_schemes_includes_simple_threshold() {
        let schemes = governance::jdg_signature_schemes();
        assert!(schemes.contains(&"simple_threshold".to_string()));
    }
}

//! "Actual" layer of Iroha configuration parameters. It contains strongly-typed validated
//! structures in a way that is efficient for Iroha internally.
#![allow(
    clippy::doc_markdown,
    clippy::doc_link_with_quotes,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::cast_lossless,
    clippy::manual_abs_diff,
    clippy::cast_possible_truncation,
    clippy::struct_field_names,
    clippy::missing_fields_in_debug,
    clippy::struct_excessive_bools,
    clippy::assertions_on_constants,
    clippy::too_many_arguments
)]

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use error_stack::{Report, ResultExt};
use iroha_config_base::{WithOrigin, read::ConfigReader, toml::TomlSource, util::Bytes};
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair, PrivateKey, PublicKey,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
    },
    streaming::StreamingKeyMaterial,
};
#[allow(unused_imports)]
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::prelude::AssetDefinitionId,
    block::BlockHeader,
    compute::{
        ComputeAuthPolicy, ComputeFeeSplit, ComputeGovernanceError, ComputePriceAmplifiers,
        ComputePriceDeltaBounds, ComputePriceRiskClass, ComputePriceWeights, ComputeResourceBudget,
        ComputeSandboxRules, ComputeSponsorPolicy,
    },
    content::ContentAuthMode,
    da::{
        commitment::DaProofScheme,
        confidential_compute::{ConfidentialComputeMechanism, ConfidentialComputePolicy},
        prelude::DaStripeLayout,
        types::{BlobClass, DaRentPolicyV1, RetentionPolicy},
    },
    domain::DomainId,
    hijiri::HijiriFeePolicy as ModelHijiriFeePolicy,
    jurisdiction::JdgSignatureScheme,
    name::Name,
    nexus::{
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneConfig as LaneConfigMetadata, LaneId, LaneStorageProfile, LaneVisibility, ShardId,
        UniversalAccountId,
    },
    oracle::KeyedHash,
    peer::{Peer, PeerId},
    sorafs::{
        capacity::ProviderId, pin_registry::StorageClass as SorafsStorageClass,
        pricing::PricingScheduleRecord,
    },
    taikai::TaikaiAvailabilityClass,
};
use iroha_primitives::{addr::SocketAddr, numeric::Numeric, unique_vec::UniqueVec};
use norito::streaming::EntropyMode;
use rust_decimal::Decimal;
use url::Url;
pub use user::{DevTelemetry, Logger, Snapshot};

use crate::{
    kura::{FsyncMode, InitMode},
    parameters::{defaults, user, user::ParseError},
};

type Result<T, E> = core::result::Result<T, Report<E>>;

/// Parsed configuration root used internally by Iroha services.
#[derive(Debug, Clone)]
pub struct Root {
    /// Common options shared across components.
    pub common: Common,
    /// Network configuration.
    pub network: Network,
    /// Genesis configuration.
    pub genesis: Genesis,
    /// Torii API configuration.
    pub torii: Torii,
    /// Block storage (Kura) configuration.
    pub kura: Kura,
    /// Consensus (Sumeragi) configuration.
    pub sumeragi: Sumeragi,
    /// Block synchronization parameters.
    pub block_sync: BlockSync,
    /// Transaction gossiping parameters.
    pub transaction_gossiper: TransactionGossiper,
    /// Live query store configuration.
    pub live_query_store: LiveQueryStore,
    /// Logger configuration.
    pub logger: Logger,
    /// Queue settings.
    pub queue: Queue,
    /// Nexus lane/data-space configuration.
    pub nexus: Nexus,
    /// Snapshot configuration.
    pub snapshot: Snapshot,
    /// Master telemetry switch (when false, all telemetry outputs are disabled at runtime).
    pub telemetry_enabled: bool,
    /// Active telemetry profile describing available capabilities.
    pub telemetry_profile: TelemetryProfile,
    /// Telemetry destination (if enabled).
    pub telemetry: Option<Telemetry>,
    /// Telemetry redaction policy.
    pub telemetry_redaction: TelemetryRedaction,
    /// Telemetry integrity policy.
    pub telemetry_integrity: TelemetryIntegrity,
    /// Developer telemetry settings.
    pub dev_telemetry: DevTelemetry,
    /// Pipeline execution settings.
    pub pipeline: Pipeline,
    /// Tiered state backend configuration.
    pub tiered_state: crate::parameters::actual::TieredState,
    /// Compute lane configuration.
    pub compute: Compute,
    /// Content lane configuration.
    pub content: Content,
    /// Oracle aggregation configuration.
    pub oracle: Oracle,
    /// IVM-related configuration (banner/beep).
    pub ivm: Ivm,
    /// Norito codec settings (serialization/compression heuristics).
    pub norito: Norito,
    /// Hijiri reputation system configuration.
    pub hijiri: Hijiri,
    /// Fraud monitoring configuration.
    pub fraud_monitoring: FraudMonitoring,
    /// Zero-knowledge proof system settings.
    pub zk: Zk,
    /// Governance settings (voting keys, policies).
    pub gov: Governance,
    /// Network Time Service parameters.
    pub nts: Nts,
    /// Hardware acceleration settings for IVM and helpers.
    pub accel: Acceleration,
    /// Concurrency settings for thread pools.
    pub concurrency: Concurrency,
    /// Confidential asset/verifier configuration.
    pub confidential: Confidential,
    /// Cryptography feature toggles and defaults.
    pub crypto: Crypto,
    /// Settlement configuration (repo and related agreements).
    pub settlement: Settlement,
    /// Streaming configuration (control-plane key material).
    pub streaming: Streaming,
}

/// See [`Root::from_toml_source`]
#[derive(thiserror::Error, Debug, Copy, Clone)]
#[error("Failed to read configuration from a given TOML source")]
pub struct FromTomlSourceError;

impl Root {
    /// A shorthand to read config from a single provided TOML.
    /// For testing purposes.
    /// # Errors
    /// If config reading/parsing fails.
    pub fn from_toml_source(src: TomlSource) -> Result<Self, FromTomlSourceError> {
        ConfigReader::new()
            .with_toml_source(src)
            .read_and_complete::<user::Root>()
            .change_context(FromTomlSourceError)?
            .parse()
            .change_context(FromTomlSourceError)
    }

    /// Check whether the configuration already enables Sora/Nexus-only features.
    #[must_use]
    pub fn uses_sora_features(&self) -> bool {
        let sorafs = self.torii.sorafs_storage.enabled
            || self.torii.sorafs_discovery.discovery_enabled
            || self.torii.sorafs_repair.enabled
            || self.torii.sorafs_gc.enabled;
        let multilane = self.uses_multilane_catalogs();

        sorafs || multilane
    }

    /// Detect whether the configuration declares multiple lanes/dataspaces or non-default routing.
    #[must_use]
    pub fn uses_multilane_catalogs(&self) -> bool {
        self.nexus.uses_multilane_catalogs()
    }

    /// Apply the bundled Sora Nexus profile (SoraFS + multi-lane defaults).
    pub fn apply_sora_profile(&mut self) {
        self.nexus.enabled = true;
        self.torii.sorafs_storage.enabled = true;
        self.torii.sorafs_discovery.discovery_enabled = true;
        self.sumeragi.da_enabled = true;
        if self.tiered_state.da_store_root.is_none() {
            self.tiered_state.da_store_root =
                Some(PathBuf::from(defaults::tiered_state::DEFAULT_DA_STORE_ROOT));
        }
        // Sora Nexus public dataspace always runs on the global NPoS ring.
        self.sumeragi.consensus_mode = ConsensusMode::Npos;

        let catalog = &self.nexus.lane_catalog;
        let is_default_catalog = catalog.lane_count().get() == 1
            && matches!(catalog.lanes(), [lane] if lane.id == LaneId::SINGLE && lane.alias == "default");
        let dataspace = &self.nexus.dataspace_catalog;
        let is_default_dataspace = matches!(dataspace.entries(), [entry]
            if entry.id == DataSpaceId::GLOBAL
                && entry.alias == defaults::nexus::DEFAULT_DATASPACE_ALIAS
        );
        let policy = &self.nexus.routing_policy;
        let is_default_policy = policy.default_lane == LaneId::SINGLE
            && policy.default_dataspace == DataSpaceId::GLOBAL
            && policy.rules.is_empty();

        if is_default_catalog && is_default_dataspace && is_default_policy {
            self.nexus.lane_catalog = sora_lane_catalog();
            self.nexus.lane_config = LaneConfig::from_catalog(&self.nexus.lane_catalog);
            self.nexus.dataspace_catalog = sora_dataspace_catalog();
            self.nexus.routing_policy = sora_routing_policy();
        }
    }

    /// Apply Nexus storage budgets to component-level caps.
    ///
    /// This is a best-effort configuration pass that only affects Nexus-enabled nodes.
    pub fn apply_storage_budget(&mut self) {
        if !self.nexus.enabled {
            return;
        }

        let max_wsv_mem = self.nexus.storage.max_wsv_memory_bytes.get();
        if max_wsv_mem > 0 {
            self.tiered_state.hot_retained_bytes =
                min_nonzero_bytes(self.tiered_state.hot_retained_bytes, max_wsv_mem);
            if !self.tiered_state.enabled {
                self.tiered_state.enabled = true;
            }
            if self.tiered_state.cold_store_root.is_none()
                && self.tiered_state.da_store_root.is_none()
            {
                self.tiered_state.cold_store_root = Some(PathBuf::from(
                    defaults::tiered_state::DEFAULT_COLD_STORE_ROOT,
                ));
            }
        }

        let max_disk = self.nexus.storage.max_disk_usage_bytes.get();
        if max_disk == 0 {
            return;
        }

        let weights = self.nexus.storage.disk_budget_weights;
        let total_bps = u64::from(weights.total_bps().max(1));
        let budget = |bps: u16| max_disk.saturating_mul(u64::from(bps)) / total_bps;

        let mut kura_budget = budget(weights.kura_blocks_bps);
        let wsv_budget = budget(weights.wsv_snapshots_bps);
        let sorafs_budget = budget(weights.sorafs_bps);
        let soranet_budget = budget(weights.soranet_spool_bps);
        let soravpn_budget = budget(weights.soravpn_spool_bps);

        let allocated = kura_budget
            .saturating_add(wsv_budget)
            .saturating_add(sorafs_budget)
            .saturating_add(soranet_budget)
            .saturating_add(soravpn_budget);
        let remainder = max_disk.saturating_sub(allocated);
        kura_budget = kura_budget.saturating_add(remainder);

        self.kura.max_disk_usage_bytes =
            min_nonzero_bytes(self.kura.max_disk_usage_bytes, kura_budget);
        self.tiered_state.max_cold_bytes =
            min_nonzero_bytes(self.tiered_state.max_cold_bytes, wsv_budget);
        self.torii.sorafs_storage.max_capacity_bytes =
            min_nonzero_bytes(self.torii.sorafs_storage.max_capacity_bytes, sorafs_budget);
        self.streaming.soranet.provision_spool_max_bytes = min_nonzero_bytes(
            self.streaming.soranet.provision_spool_max_bytes,
            soranet_budget,
        );
        self.streaming.soravpn.provision_spool_max_bytes = min_nonzero_bytes(
            self.streaming.soravpn.provision_spool_max_bytes,
            soravpn_budget,
        );
    }
}

fn min_nonzero_bytes(current: Bytes<u64>, limit: u64) -> Bytes<u64> {
    if limit == 0 {
        return current;
    }
    let current_val = current.get();
    if current_val == 0 {
        Bytes(limit)
    } else {
        Bytes(current_val.min(limit))
    }
}

pub(crate) fn sora_lane_catalog() -> LaneCatalog {
    let lane_count = NonZeroU32::new(3).expect("three lanes are non-zero");
    let lanes = vec![
        LaneConfigMetadata {
            id: LaneId::new(0),
            dataspace_id: DataSpaceId::GLOBAL,
            alias: "core".to_string(),
            description: Some("Primary execution lane".to_string()),
            visibility: LaneVisibility::Public,
            lane_type: Some("default_public".to_string()),
            governance: None,
            settlement: None,
            storage: LaneStorageProfile::FullReplica,
            proof_scheme: DaProofScheme::default(),
            metadata: BTreeMap::new(),
        },
        LaneConfigMetadata {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            alias: "governance".to_string(),
            description: Some("Governance & parliament traffic".to_string()),
            visibility: LaneVisibility::Restricted,
            lane_type: Some("governance".to_string()),
            governance: None,
            settlement: None,
            storage: LaneStorageProfile::FullReplica,
            proof_scheme: DaProofScheme::default(),
            metadata: BTreeMap::new(),
        },
        LaneConfigMetadata {
            id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(2),
            alias: "zk".to_string(),
            description: Some("Zero-knowledge attachments".to_string()),
            visibility: LaneVisibility::Restricted,
            lane_type: Some("attachments".to_string()),
            governance: None,
            settlement: None,
            storage: LaneStorageProfile::FullReplica,
            proof_scheme: DaProofScheme::default(),
            metadata: BTreeMap::new(),
        },
    ];
    LaneCatalog::new(lane_count, lanes).expect("static Sora lane catalog is valid")
}

pub(crate) fn sora_dataspace_catalog() -> DataSpaceCatalog {
    let entries = vec![
        DataSpaceMetadata {
            id: DataSpaceId::GLOBAL,
            alias: defaults::nexus::DEFAULT_DATASPACE_ALIAS.to_string(),
            description: Some("Single-lane data space".to_string()),
            fault_tolerance: defaults::nexus::dataspace::FAULT_TOLERANCE,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "governance".to_string(),
            description: Some("Governance proposals & manifests".to_string()),
            fault_tolerance: defaults::nexus::dataspace::FAULT_TOLERANCE,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(2),
            alias: "zk".to_string(),
            description: Some("Zero-knowledge proofs and attachments".to_string()),
            fault_tolerance: defaults::nexus::dataspace::FAULT_TOLERANCE,
        },
    ];
    DataSpaceCatalog::new(entries).expect("static Sora dataspace catalog is valid")
}

pub(crate) fn sora_routing_policy() -> LaneRoutingPolicy {
    LaneRoutingPolicy {
        default_lane: LaneId::new(0),
        default_dataspace: DataSpaceId::GLOBAL,
        rules: vec![
            LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(1)),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("governance".to_string()),
                    description: Some(
                        "Route governance instructions to the governance lane".to_string(),
                    ),
                },
            },
            LaneRoutingRule {
                lane: LaneId::new(2),
                dataspace: Some(DataSpaceId::new(2)),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("smartcontract::deploy".to_string()),
                    description: Some(
                        "Route contract deployments to the zk lane for proof tracking".to_string(),
                    ),
                },
            },
        ],
    }
}

#[cfg(test)]
mod sora_profile_tests {
    use std::num::NonZeroU32;

    use iroha_config_base::toml::TomlSource;
    use iroha_data_model::nexus::{LaneCatalog, LaneConfig as LaneConfigMetadata};
    use toml::Table;

    use super::*;

    const MINIMAL_CONFIG: &str = r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#;

    fn minimal_root() -> Root {
        let table: Table = toml::from_str(MINIMAL_CONFIG).expect("parse minimal config table");
        Root::from_toml_source(TomlSource::inline(table)).expect("load minimal config")
    }

    #[test]
    fn apply_sora_profile_enables_nexus_and_sets_catalogs_on_defaults() {
        let mut root = minimal_root();

        root.apply_sora_profile();

        assert!(root.nexus.enabled, "Sora profile must enable Nexus runtime");
        assert_eq!(
            root.sumeragi.consensus_mode,
            ConsensusMode::Npos,
            "Sora profile must force NPoS consensus"
        );
        assert!(
            root.sumeragi.da_enabled,
            "Sora profile must enable data availability"
        );
        assert_eq!(root.nexus.lane_catalog, sora_lane_catalog());
        assert_eq!(root.nexus.dataspace_catalog, sora_dataspace_catalog());
        assert_eq!(root.nexus.routing_policy, sora_routing_policy());
        assert_eq!(
            root.nexus.lane_config.entries().len(),
            root.nexus.lane_catalog.lanes().len()
        );
        assert_eq!(
            root.tiered_state
                .da_store_root
                .as_ref()
                .expect("DA store root should be defaulted"),
            &PathBuf::from(defaults::tiered_state::DEFAULT_DA_STORE_ROOT)
        );
    }

    #[test]
    fn has_lane_overrides_detects_single_lane_changes() {
        let mut root = minimal_root();
        root.nexus.enabled = false;
        root.nexus.lane_catalog = LaneCatalog::new(
            NonZeroU32::new(1).expect("nonzero lane count"),
            vec![LaneConfigMetadata {
                alias: "custom".to_string(),
                ..LaneConfigMetadata::default()
            }],
        )
        .expect("lane catalog");

        assert!(root.nexus.has_lane_overrides());
        assert!(
            !root.nexus.uses_multilane_catalogs(),
            "single-lane overrides should not be treated as multi-lane"
        );
    }

    #[test]
    fn apply_sora_profile_preserves_custom_catalogs_but_enables_flag() {
        let mut root = minimal_root();
        let custom_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                LaneConfigMetadata {
                    id: LaneId::new(0),
                    alias: "alpha".to_string(),
                    description: None,
                    ..LaneConfigMetadata::default()
                },
                LaneConfigMetadata {
                    id: LaneId::new(1),
                    alias: "beta".to_string(),
                    description: None,
                    ..LaneConfigMetadata::default()
                },
            ],
        )
        .expect("valid custom catalog");
        root.nexus.lane_config = LaneConfig::from_catalog(&custom_catalog);
        root.nexus.lane_catalog = custom_catalog.clone();

        root.apply_sora_profile();

        assert!(root.nexus.enabled, "Sora profile must enable Nexus runtime");
        assert_eq!(
            root.sumeragi.consensus_mode,
            ConsensusMode::Npos,
            "Sora profile must force NPoS consensus"
        );
        assert!(
            root.sumeragi.da_enabled,
            "Sora profile must enable data availability"
        );
        assert_eq!(
            root.tiered_state
                .da_store_root
                .as_ref()
                .expect("DA store root should be defaulted"),
            &PathBuf::from(defaults::tiered_state::DEFAULT_DA_STORE_ROOT)
        );
        assert_eq!(root.nexus.lane_catalog, custom_catalog);
        assert_eq!(
            root.nexus
                .lane_config
                .entry(LaneId::new(1))
                .expect("lane config should be preserved")
                .alias,
            "beta"
        );
    }

    #[test]
    fn apply_storage_budget_clamps_component_caps() {
        let mut root = minimal_root();
        root.nexus.enabled = true;
        root.nexus.storage.max_disk_usage_bytes = Bytes(1_000);
        root.nexus.storage.max_wsv_memory_bytes = Bytes(512);
        root.nexus.storage.disk_budget_weights = NexusStorageWeights {
            kura_blocks_bps: 5_000,
            wsv_snapshots_bps: 2_000,
            sorafs_bps: 2_000,
            soranet_spool_bps: 500,
            soravpn_spool_bps: 500,
        };
        root.tiered_state.enabled = false;
        root.tiered_state.cold_store_root = None;
        root.tiered_state.da_store_root = None;
        root.kura.max_disk_usage_bytes = Bytes(0);
        root.tiered_state.max_cold_bytes = Bytes(0);
        root.torii.sorafs_storage.max_capacity_bytes = Bytes(0);
        root.streaming.soranet.provision_spool_max_bytes = Bytes(0);
        root.streaming.soravpn.provision_spool_max_bytes = Bytes(0);

        root.apply_storage_budget();

        assert_eq!(root.kura.max_disk_usage_bytes.get(), 500);
        assert_eq!(root.tiered_state.max_cold_bytes.get(), 200);
        assert_eq!(root.torii.sorafs_storage.max_capacity_bytes.get(), 200);
        assert_eq!(root.streaming.soranet.provision_spool_max_bytes.get(), 50);
        assert_eq!(root.streaming.soravpn.provision_spool_max_bytes.get(), 50);
        assert!(root.tiered_state.enabled, "tiered state should be enabled");
        assert_eq!(root.tiered_state.hot_retained_bytes.get(), 512);
        assert!(root.tiered_state.da_store_root.is_none());
        assert_eq!(
            root.tiered_state
                .cold_store_root
                .as_ref()
                .expect("cold store root defaulted")
                .as_os_str(),
            defaults::tiered_state::DEFAULT_COLD_STORE_ROOT
        );
    }

    #[test]
    fn streaming_soravpn_defaults_match_constants() {
        let config = StreamingSoravpn::from_defaults();
        assert_eq!(
            config.provision_spool_dir,
            PathBuf::from(defaults::streaming::soravpn::PROVISION_SPOOL_DIR)
        );
        assert_eq!(
            config.provision_spool_max_bytes.get(),
            defaults::streaming::soravpn::PROVISION_SPOOL_MAX_BYTES.get()
        );
    }
}

/// Common options shared between multiple components.
#[derive(Debug, Clone)]
pub struct Common {
    /// Unique chain identifier.
    pub chain: ChainId,
    /// Key pair for signing transactions and blocks.
    pub key_pair: KeyPair,
    /// Local peer description.
    pub peer: Peer,
    /// Trusted peers including self.
    pub trusted_peers: WithOrigin<TrustedPeers>,
    /// Default implicit domain label applied to account addresses.
    pub default_account_domain_label: WithOrigin<String>,
    /// IH58 chain discriminant / network prefix applied when encoding addresses.
    pub chain_discriminant: WithOrigin<u16>,
}

/// Intrinsic dispatch policy applied to SM acceleration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmIntrinsicsPolicy {
    /// Runtime decides at startup based on hardware support (default).
    Auto,
    /// Intrinsics forced on by configuration.
    ForceEnable,
    /// Intrinsics forcibly disabled by configuration.
    ForceDisable,
}

impl SmIntrinsicsPolicy {
    /// Returns the string identifier used in configuration files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ForceEnable => "force-enable",
            Self::ForceDisable => "force-disable",
        }
    }
}

/// Cryptography defaults surfaced via configuration.
#[derive(Debug, Clone)]
pub struct Crypto {
    /// Toggle for the optional OpenSSL-backed SM preview helpers.
    pub enable_sm_openssl_preview: bool,
    /// Intrinsic dispatch policy applied to SM acceleration.
    pub sm_intrinsics: SmIntrinsicsPolicy,
    /// Default hash algorithm identifier (e.g., `blake2b-256`, `sm3-256`).
    pub default_hash: String,
    /// Signing algorithms allowed for transaction admission.
    pub allowed_signing: Vec<Algorithm>,
    /// Default distinguishing identifier applied when SM2 signatures omit it.
    pub sm2_distid_default: String,
    /// Curve identifiers (per the account curve registry) allowed for controllers.
    pub allowed_curve_ids: Vec<u8>,
}

impl Default for Crypto {
    fn default() -> Self {
        Self {
            enable_sm_openssl_preview: defaults::crypto::enable_sm_openssl_preview(),
            sm_intrinsics: SmIntrinsicsPolicy::Auto,
            default_hash: defaults::crypto::default_hash(),
            allowed_signing: defaults::crypto::allowed_signing(),
            sm2_distid_default: defaults::crypto::sm2_distid_default(),
            allowed_curve_ids: defaults::crypto::allowed_curve_ids(),
        }
    }
}

impl Crypto {
    /// Determine whether SM helper syscalls should be enabled for this configuration.
    #[must_use]
    pub fn sm_helpers_enabled(&self) -> bool {
        #[cfg(feature = "sm")]
        {
            self.allowed_signing
                .iter()
                .any(|algo| matches!(algo, Algorithm::Sm2))
        }
        #[cfg(not(feature = "sm"))]
        {
            let _ = self;
            false
        }
    }
}

/// Parsed SoraNet handshake configuration.
#[derive(Clone)]
pub struct SoranetHandshake {
    /// Descriptor commitment advertised by the node (32 bytes).
    pub descriptor_commit: WithOrigin<Vec<u8>>,
    /// Client capability TLVs serialized into bytes.
    pub client_capabilities: WithOrigin<Vec<u8>>,
    /// Relay capability TLVs serialized into bytes.
    pub relay_capabilities: WithOrigin<Vec<u8>>,
    /// Whether this node supports trust gossip exchange.
    pub trust_gossip: bool,
    /// Negotiated ML-KEM identifier.
    pub kem_id: u8,
    /// Negotiated signature suite identifier.
    pub sig_id: u8,
    /// Optional resume hash advertised to peers.
    pub resume_hash: Option<WithOrigin<Vec<u8>>>,
    /// PoW parameters for circuit admission.
    pub pow: SoranetPow,
}

/// Runtime knobs controlling the SoraNet privacy telemetry pipeline.
#[derive(Debug, Clone, Copy)]
pub struct SoranetPrivacy {
    /// Duration of each telemetry bucket in seconds.
    pub bucket_secs: u64,
    /// Minimum number of contributing handshakes required before publishing a bucket.
    pub min_handshakes: u64,
    /// Number of buckets to delay publication after the first contributor arrives.
    pub flush_delay_buckets: u64,
    /// Forced flush interval expressed in buckets.
    pub force_flush_buckets: u64,
    /// Maximum number of completed buckets retained in memory.
    pub max_completed_buckets: usize,
    /// Maximum bucket lag tolerated for collector shares before suppression.
    pub max_share_lag_buckets: u64,
    /// Expected number of collector shares contributing to a bucket.
    pub expected_shares: u16,
    /// Capacity for the in-memory event buffer feeding the aggregator.
    pub event_buffer_capacity: usize,
}

impl SoranetPrivacy {
    /// Default telemetry bucket width in seconds.
    pub const DEFAULT_BUCKET_SECS: u64 = defaults::soranet::privacy::BUCKET_SECS;
    /// Default minimum handshake contributors required before publishing.
    pub const DEFAULT_MIN_HANDSHAKES: u64 = defaults::soranet::privacy::MIN_HANDSHAKES;
    /// Default bucket delay before attempting a standard flush.
    pub const DEFAULT_FLUSH_DELAY_BUCKETS: u64 = defaults::soranet::privacy::FLUSH_DELAY_BUCKETS;
    /// Default forced flush interval expressed in buckets.
    pub const DEFAULT_FORCE_FLUSH_BUCKETS: u64 = defaults::soranet::privacy::FORCE_FLUSH_BUCKETS;
    /// Default number of completed buckets retained in memory.
    pub const DEFAULT_MAX_COMPLETED_BUCKETS: usize =
        defaults::soranet::privacy::MAX_COMPLETED_BUCKETS;
    /// Default maximum bucket lag tolerated for collector shares before suppression.
    pub const DEFAULT_MAX_SHARE_LAG_BUCKETS: u64 =
        defaults::soranet::privacy::MAX_SHARE_LAG_BUCKETS;
    /// Default expected PRIO share count.
    pub const DEFAULT_EXPECTED_SHARES: u16 = defaults::soranet::privacy::EXPECTED_SHARES;
    /// Default capacity of the in-memory privacy event buffer.
    pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize =
        defaults::soranet::privacy::EVENT_BUFFER_CAPACITY;
}

impl Default for SoranetPrivacy {
    fn default() -> Self {
        Self {
            bucket_secs: defaults::soranet::privacy::BUCKET_SECS,
            min_handshakes: defaults::soranet::privacy::MIN_HANDSHAKES,
            flush_delay_buckets: defaults::soranet::privacy::FLUSH_DELAY_BUCKETS,
            force_flush_buckets: defaults::soranet::privacy::FORCE_FLUSH_BUCKETS,
            max_completed_buckets: defaults::soranet::privacy::MAX_COMPLETED_BUCKETS,
            max_share_lag_buckets: defaults::soranet::privacy::MAX_SHARE_LAG_BUCKETS,
            expected_shares: defaults::soranet::privacy::EXPECTED_SHARES,
            event_buffer_capacity: defaults::soranet::privacy::EVENT_BUFFER_CAPACITY,
        }
    }
}

/// Derived VPN configuration for the native SoraNet tunnel.
#[derive(Debug, Clone)]
pub struct SoranetVpn {
    /// Whether the VPN surface is enabled.
    pub enabled: bool,
    /// Fixed cell size (bytes).
    pub cell_size_bytes: u16,
    /// Flow label width (bits).
    pub flow_label_bits: u8,
    /// Cover-to-data ratio (permille).
    pub cover_to_data_per_mille: u16,
    /// Maximum burst of consecutive cover cells.
    pub max_cover_burst: u16,
    /// Heartbeat cadence for keepalive cells (milliseconds).
    pub heartbeat_ms: u16,
    /// Maximum jitter applied to scheduled slots (milliseconds).
    pub jitter_ms: u16,
    /// Padding budget carried in cell headers (milliseconds).
    pub padding_budget_ms: u16,
    /// Guard/exit refresh cadence.
    pub guard_refresh: Duration,
    /// Control-plane lease duration.
    pub lease: Duration,
    /// DNS push interval.
    pub dns_push_interval: Duration,
    /// Exit class label used for billing/telemetry.
    pub exit_class: String,
    /// Meter family identifier for billing receipts.
    pub meter_family: String,
}

impl Default for SoranetVpn {
    fn default() -> Self {
        Self {
            enabled: defaults::soranet::vpn::ENABLED,
            cell_size_bytes: defaults::soranet::vpn::CELL_SIZE_BYTES,
            flow_label_bits: defaults::soranet::vpn::FLOW_LABEL_BITS,
            cover_to_data_per_mille: defaults::soranet::vpn::COVER_TO_DATA_PER_MILLE,
            max_cover_burst: defaults::soranet::vpn::MAX_COVER_BURST,
            heartbeat_ms: defaults::soranet::vpn::HEARTBEAT_MS,
            jitter_ms: defaults::soranet::vpn::JITTER_MS,
            padding_budget_ms: defaults::soranet::vpn::PADDING_BUDGET_MS,
            guard_refresh: defaults::soranet::vpn::guard_refresh_secs(),
            lease: defaults::soranet::vpn::lease_secs(),
            dns_push_interval: defaults::soranet::vpn::dns_push_interval_secs(),
            exit_class: defaults::soranet::vpn::EXIT_CLASS.to_string(),
            meter_family: defaults::soranet::vpn::METER_FAMILY.to_string(),
        }
    }
}

/// PoW admission parameters shared with peers.
#[derive(Debug, Clone)]
pub struct SoranetPow {
    /// Indicates whether PoW tickets are required for inbound circuits.
    pub required: bool,
    /// Required number of leading zero bits in the ticket digest.
    pub difficulty: u8,
    /// Maximum allowed ticket expiry skew relative to the relay clock.
    pub max_future_skew: Duration,
    /// Minimum lifetime a ticket must remain valid.
    pub min_ticket_ttl: Duration,
    /// Target lifetime used when minting tickets locally.
    pub ticket_ttl: Duration,
    /// Maximum revoked ticket entries to retain on disk.
    pub revocation_store_capacity: usize,
    /// Maximum TTL enforced for revoked entries.
    pub revocation_max_ttl: Duration,
    /// Filesystem path for the revocation snapshot.
    pub revocation_store_path: Cow<'static, str>,
    /// Optional puzzle parameters for Argon2-based challenges.
    pub puzzle: Option<SoranetPuzzle>,
    /// ML-DSA-44 public key used to verify signed PoW tickets.
    pub signed_ticket_public_key: Option<Vec<u8>>,
}

/// Argon2 puzzle parameters shared with peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoranetPuzzle {
    /// Memory cost expressed in kibibytes.
    pub memory_kib: NonZeroU32,
    /// Time cost (number of iterations).
    pub time_cost: NonZeroU32,
    /// Argon2 parallelism lanes.
    pub lanes: NonZeroU32,
}

impl SoranetPuzzle {
    /// Construct a puzzle with explicit parameters.
    pub const fn new(memory_kib: NonZeroU32, time_cost: NonZeroU32, lanes: NonZeroU32) -> Self {
        Self {
            memory_kib,
            time_cost,
            lanes,
        }
    }

    /// Default Argon2 puzzle used for handshake PoW challenges.
    pub const fn default_const() -> Self {
        Self {
            memory_kib: NonZeroU32::new(64 * 1024).unwrap(),
            time_cost: NonZeroU32::new(2).unwrap(),
            lanes: NonZeroU32::new(1).unwrap(),
        }
    }
}

impl Default for SoranetPuzzle {
    fn default() -> Self {
        Self::default_const()
    }
}

impl SoranetPow {
    /// Construct a PoW policy with explicit parameters.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        required: bool,
        difficulty: u8,
        max_future_skew: Duration,
        min_ticket_ttl: Duration,
        ticket_ttl: Duration,
        revocation_store_capacity: usize,
        revocation_max_ttl: Duration,
        revocation_store_path: Cow<'static, str>,
        puzzle: Option<SoranetPuzzle>,
    ) -> Self {
        Self {
            required,
            difficulty,
            max_future_skew,
            min_ticket_ttl,
            ticket_ttl,
            revocation_store_capacity,
            revocation_max_ttl,
            revocation_store_path,
            puzzle,
            signed_ticket_public_key: None,
        }
    }

    /// Default PoW admission policy applied when no override is supplied.
    pub const fn default_const() -> Self {
        Self {
            required: false,
            difficulty: 0,
            max_future_skew: Duration::from_secs(300),
            min_ticket_ttl: Duration::from_secs(30),
            ticket_ttl: Duration::from_secs(60),
            revocation_store_capacity: 8_192,
            revocation_max_ttl: Duration::from_secs(900),
            revocation_store_path: Cow::Borrowed("./storage/soranet/ticket_revocations.norito"),
            puzzle: Some(SoranetPuzzle::default_const()),
            signed_ticket_public_key: None,
        }
    }

    /// Attach the relay's signed-ticket verification key.
    #[must_use]
    pub fn with_signed_ticket_public_key(mut self, public_key: Option<Vec<u8>>) -> Self {
        self.signed_ticket_public_key = public_key;
        self
    }
}

impl Default for SoranetPow {
    fn default() -> Self {
        Self::default_const()
    }
}

struct HexWithOrigin<'a>(&'a WithOrigin<Vec<u8>>);

impl fmt::Debug for HexWithOrigin<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithOrigin")
            .field("value_hex", &hex::encode(self.0.value()))
            .field("origin", self.0.origin())
            .finish()
    }
}

impl fmt::Debug for SoranetHandshake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signed_ticket_key = self.pow.signed_ticket_public_key.as_ref().map_or_else(
            || "None".to_string(),
            |key| format!("Some(len={})", key.len()),
        );
        f.debug_struct("SoranetHandshake")
            .field("descriptor_commit", &HexWithOrigin(&self.descriptor_commit))
            .field(
                "client_capabilities",
                &HexWithOrigin(&self.client_capabilities),
            )
            .field(
                "relay_capabilities",
                &HexWithOrigin(&self.relay_capabilities),
            )
            .field("trust_gossip", &self.trust_gossip)
            .field("kem_id", &self.kem_id)
            .field("sig_id", &self.sig_id)
            .field("resume_hash", &self.resume_hash.as_ref().map(HexWithOrigin))
            .field(
                "pow",
                &format_args!(
                "SoranetPow {{ required: {}, difficulty: {}, max_future_skew_secs: {}, min_ticket_ttl_secs: {}, ticket_ttl_secs: {}, revocation_store_capacity: {}, revocation_max_ttl_secs: {}, revocation_store_path: {}, puzzle: {}, signed_ticket_public_key: {} }}",
                self.pow.required,
                self.pow.difficulty,
                self.pow.max_future_skew.as_secs(),
                    self.pow.min_ticket_ttl.as_secs(),
                    self.pow.ticket_ttl.as_secs(),
                    self.pow.revocation_store_capacity,
                    self.pow.revocation_max_ttl.as_secs(),
                    self.pow.revocation_store_path,
                    self.pow
                        .puzzle
                        .as_ref()
                        .map_or_else(
                            || "None".to_string(),
                            |puzzle| format!(
                            "Some {{ memory_kib: {}, time_cost: {}, lanes: {} }}",
                            puzzle.memory_kib.get(),
                            puzzle.time_cost.get(),
                            puzzle.lanes.get()
                        ),
                        ),
                    signed_ticket_key,
                ),
            )
            .finish()
    }
}

impl Default for SoranetHandshake {
    fn default() -> Self {
        Self {
            descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
            client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
            relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
            trust_gossip: true,
            kem_id: 1,
            sig_id: 1,
            resume_hash: None,
            pow: SoranetPow::default(),
        }
    }
}

/// Lane profile presets for shaping p2p behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneProfile {
    /// Datacenter/validator profile with generous defaults.
    Core,
    /// Constrained/home profile with tighter caps.
    Home,
}

impl LaneProfile {
    /// Resolve a profile label into a typed variant.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label.to_ascii_lowercase().as_str() {
            "home" => Self::Home,
            _ => Self::Core,
        }
    }

    /// Return the baked-in defaults for the profile.
    #[must_use]
    pub fn defaults(self) -> LaneProfileDefaults {
        match self {
            Self::Core => LaneProfileDefaults {
                tick_ms: defaults::network::lane_profile::CORE_TICK_MS,
                mtu_bytes: defaults::network::lane_profile::CORE_MTU_BYTES,
                uplink_bps: defaults::network::lane_profile::CORE_UPLINK_BPS,
                constant_neighbors: defaults::network::lane_profile::CORE_CONSTANT_NEIGHBORS,
                max_total_connections: defaults::network::lane_profile::CORE_MAX_TOTAL_CONNECTIONS,
                max_incoming: defaults::network::lane_profile::CORE_MAX_INCOMING,
            },
            Self::Home => LaneProfileDefaults {
                tick_ms: defaults::network::lane_profile::HOME_TICK_MS,
                mtu_bytes: defaults::network::lane_profile::HOME_MTU_BYTES,
                uplink_bps: defaults::network::lane_profile::HOME_UPLINK_BPS,
                constant_neighbors: defaults::network::lane_profile::HOME_CONSTANT_NEIGHBORS,
                max_total_connections: defaults::network::lane_profile::HOME_MAX_TOTAL_CONNECTIONS,
                max_incoming: defaults::network::lane_profile::HOME_MAX_INCOMING,
            },
        }
    }

    /// Render the canonical profile label.
    #[must_use]
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Home => "home",
        }
    }

    /// Derived shaping limits for the profile (caps applied only when present).
    #[must_use]
    pub fn derived_limits(self) -> LaneProfileLimits {
        match self {
            Self::Core => LaneProfileLimits {
                max_incoming: None,
                max_total_connections: None,
                low_priority_bytes_per_sec: None,
                low_priority_rate_per_sec: None,
            },
            Self::Home => {
                let defaults = self.defaults();
                let per_peer_bytes = defaults.per_peer_bytes_per_sec();
                let per_packet = defaults.mtu_payload_bytes();
                let rate = u64::from(per_peer_bytes).div_ceil(per_packet);
                let capped_rate = rate.clamp(1, u64::from(u32::MAX));
                LaneProfileLimits {
                    max_incoming: NonZeroUsize::new(defaults.max_incoming),
                    max_total_connections: NonZeroUsize::new(defaults.max_total_connections),
                    low_priority_bytes_per_sec: NonZeroU32::new(per_peer_bytes),
                    low_priority_rate_per_sec: NonZeroU32::new(
                        u32::try_from(capped_rate).expect("clamped to u32 range"),
                    ),
                }
            }
        }
    }
}

/// Resolved constants for a lane profile.
#[derive(Debug, Clone, Copy)]
pub struct LaneProfileDefaults {
    /// Scheduling tick used for shaping calculations.
    pub tick_ms: u16,
    /// MTU (bytes) assumed when computing message cadence.
    pub mtu_bytes: u16,
    /// Target uplink budget (bits per second).
    pub uplink_bps: u64,
    /// Constant-rate neighbor budget.
    pub constant_neighbors: usize,
    /// Soft cap for total connections.
    pub max_total_connections: usize,
    /// Soft cap for inbound connections.
    pub max_incoming: usize,
}

impl LaneProfileDefaults {
    /// Compute the per-peer byte budget per second based on the uplink split.
    #[must_use]
    pub fn per_peer_bytes_per_sec(self) -> u32 {
        let peers = u64::try_from(self.constant_neighbors.max(1)).expect("peer count fits in u64");
        let bytes = self.uplink_bps.saturating_div(8).saturating_div(peers);
        let capped_bytes = bytes.clamp(1, u64::from(u32::MAX));
        u32::try_from(capped_bytes).expect("clamped to u32 range")
    }

    /// Conservative payload size (bytes) after accounting for headers.
    #[must_use]
    pub fn mtu_payload_bytes(self) -> u64 {
        // Leave ~80 bytes for IP/UDP/TCP headers and crypto framing.
        u64::from(self.mtu_bytes.saturating_sub(80).max(512))
    }
}

/// Derived lane profile caps applied to networking defaults.
#[derive(Debug, Clone, Copy)]
pub struct LaneProfileLimits {
    /// Optional cap on inbound connections.
    pub max_incoming: Option<NonZeroUsize>,
    /// Optional cap on total connections.
    pub max_total_connections: Option<NonZeroUsize>,
    /// Optional per-peer low-priority byte budget.
    pub low_priority_bytes_per_sec: Option<NonZeroU32>,
    /// Optional per-peer low-priority message rate.
    pub low_priority_rate_per_sec: Option<NonZeroU32>,
}

/// Network options.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Network {
    /// Listening socket address.
    pub address: WithOrigin<SocketAddr>,
    /// Publicly advertised socket address.
    pub public_address: WithOrigin<SocketAddr>,
    /// Relay role (disabled/hub/spoke) for constrained topologies.
    pub relay_mode: RelayMode,
    /// Relay hub address to dial when in `spoke` mode.
    pub relay_hub_address: Option<SocketAddr>,
    /// Hop limit for relayed frames.
    pub relay_ttl: u8,
    /// SoraNet handshake capabilities to advertise.
    pub soranet_handshake: SoranetHandshake,
    /// Privacy telemetry configuration advertised to collectors.
    pub soranet_privacy: SoranetPrivacy,
    /// VPN tunnel configuration for the native SoraNet bridge.
    pub soranet_vpn: SoranetVpn,
    /// Lane profile preset controlling connection and throttle defaults.
    pub lane_profile: LaneProfile,
    /// Whether peers must match SM helper availability during handshake.
    pub require_sm_handshake_match: bool,
    /// Whether peers must match the OpenSSL preview toggle during handshake.
    pub require_sm_openssl_preview_match: bool,
    /// Idle connection timeout.
    pub idle_timeout: Duration,
    /// Delay outbound peer dials after startup.
    pub connect_startup_delay: Duration,
    /// Interval between peer gossip batches.
    pub peer_gossip_period: Duration,
    /// Maximum interval between peer gossip batches (idle backoff ceiling).
    pub peer_gossip_max_period: Duration,
    /// Whether to advertise and accept signed trust gossip frames.
    pub trust_gossip: bool,
    /// Half-life for peer trust decay (toward zero).
    pub trust_decay_half_life: Duration,
    /// Penalty applied for invalid/bad trust gossip.
    pub trust_penalty_bad_gossip: i32,
    /// Penalty applied when gossip mentions unknown/invalid peers.
    pub trust_penalty_unknown_peer: i32,
    /// Minimum score before trust gossip is ignored.
    pub trust_min_score: i32,
    /// Optional DNS hostname refresh interval (None disables).
    pub dns_refresh_interval: Option<Duration>,
    /// Optional TTL-based refresh for hostname-based peers.
    pub dns_refresh_ttl: Option<Duration>,
    /// Enable QUIC transport (feature-gated).
    pub quic_enabled: bool,
    /// Enable TLS-over-TCP transport for outbound dials (feature-gated).
    /// When enabled and built with the `iroha_p2p/p2p_tls` feature, the dialer will
    /// attempt to establish a TLS 1.3 session to the peer's host:port and run the
    /// existing signed application handshake over it. Falls back to plain TCP on
    /// failure.
    pub tls_enabled: bool,
    /// Optional TLS listener address for inbound TLS-over-TCP connections (feature-gated).
    /// If set (and `tls_enabled` is true), a TLS listener is started on this address.
    /// Plain TCP listener remains active on `address`.
    pub tls_listen_address: Option<WithOrigin<SocketAddr>>,
    /// Prefer WebSocket fallback for outbound dials when available (feature `p2p_ws`).
    /// Useful for constrained environments and tests that need deterministic WS dialing.
    pub prefer_ws_fallback: bool,
    /// Capacity for the high-priority network message queue (bounded mode only).
    pub p2p_queue_cap_high: NonZeroUsize,
    /// Capacity for the low-priority network message queue (bounded mode only).
    pub p2p_queue_cap_low: NonZeroUsize,
    /// Capacity for the per-peer post queue (bounded mode only).
    pub p2p_post_queue_cap: NonZeroUsize,
    /// Capacity for the inbound P2P subscriber queue feeding the node relay.
    pub p2p_subscriber_queue_cap: NonZeroUsize,
    /// Optional per-peer consensus ingress rate (msgs/sec). When None, ingress limiting is disabled.
    pub consensus_ingress_rate_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst for consensus ingress rate limiting. Defaults to `rate` when None.
    pub consensus_ingress_burst: Option<std::num::NonZeroU32>,
    /// Optional per-peer consensus ingress bytes/sec budget. When None, bytes limiting is disabled.
    pub consensus_ingress_bytes_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst (bytes) for consensus ingress bytes limiting. Defaults to `bytes_per_sec` when None.
    pub consensus_ingress_bytes_burst: Option<std::num::NonZeroU32>,
    /// Optional per-peer critical consensus ingress rate (msgs/sec). When None, critical limiting is disabled.
    pub consensus_ingress_critical_rate_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst for critical consensus ingress rate limiting. Defaults to `rate` when None.
    pub consensus_ingress_critical_burst: Option<std::num::NonZeroU32>,
    /// Optional per-peer critical consensus ingress bytes/sec budget. When None, bytes limiting is disabled.
    pub consensus_ingress_critical_bytes_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst (bytes) for critical consensus ingress bytes limiting. Defaults to `bytes_per_sec` when None.
    pub consensus_ingress_critical_bytes_burst: Option<std::num::NonZeroU32>,
    /// Maximum concurrent RBC sessions accepted per peer before throttling (0 disables).
    pub consensus_ingress_rbc_session_limit: usize,
    /// Drop threshold (per window) before temporarily suppressing consensus ingress.
    pub consensus_ingress_penalty_threshold: u32,
    /// Window for consensus ingress penalty tracking.
    pub consensus_ingress_penalty_window: Duration,
    /// Cooldown applied after consensus ingress penalties trigger.
    pub consensus_ingress_penalty_cooldown: Duration,
    /// Stagger between parallel dial attempts for multi-address peers.
    pub happy_eyeballs_stagger: Duration,
    /// Prefer IPv6 addresses over hostnames/IPv4 when dialing.
    pub addr_ipv6_first: bool,
    /// Maximum number of simultaneously accepted incoming connections.
    /// When `None`, incoming connections are not capped by count.
    pub max_incoming: Option<NonZeroUsize>,
    /// Maximum total number of connections (incoming + outgoing + in-flight accepts).
    /// When `None`, total connections are not capped by count.
    pub max_total_connections: Option<NonZeroUsize>,
    /// Optional per-IP(/24 for IPv4, /64 for IPv6) accept throttle, in accepts per second.
    /// When `None`, per-IP throttling is disabled.
    pub accept_rate_per_ip_per_sec: Option<std::num::NonZeroU32>,
    /// Optional accept token-bucket burst size per IP bucket.
    /// If `None`, a conservative burst equal to `accept_rate_per_ip_per_sec` is used.
    pub accept_burst_per_ip: Option<std::num::NonZeroU32>,
    /// Maximum number of accept throttle buckets retained (prefix + per-IP).
    pub max_accept_buckets: NonZeroUsize,
    /// Idle timeout before expiring accept throttle buckets.
    pub accept_bucket_idle: Duration,
    /// Prefix length applied to IPv4 prefix buckets.
    pub accept_prefix_v4_bits: u8,
    /// Prefix length applied to IPv6 prefix buckets.
    pub accept_prefix_v6_bits: u8,
    /// Optional prefix-level accept throttle, in accepts per second.
    pub accept_rate_per_prefix_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst size for prefix-level accept limiter.
    pub accept_burst_per_prefix: Option<std::num::NonZeroU32>,
    /// Optional per-peer Low-priority message rate (msgs/sec). When None, Low-priority rate limiting is disabled.
    pub low_priority_rate_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst for Low-priority token bucket. Defaults internally to `rate` if None.
    pub low_priority_burst: Option<std::num::NonZeroU32>,
    /// Optional per-peer Low-priority bytes budget (bytes/sec). When None, disabled.
    pub low_priority_bytes_per_sec: Option<std::num::NonZeroU32>,
    /// Optional burst in bytes for Low-priority bytes token bucket.
    pub low_priority_bytes_burst: Option<std::num::NonZeroU32>,
    /// Optional: Only allow connections (outbound/inbound) to peers whose public keys are explicitly listed.
    pub allowlist_only: bool,
    /// Optional allowlist of peer public keys.
    pub allow_keys: Vec<iroha_crypto::PublicKey>,
    /// Optional denylist of peer public keys.
    pub deny_keys: Vec<iroha_crypto::PublicKey>,
    /// Optional CIDR allowlist (IPv4/IPv6), e.g., "192.168.1.0/24", "`2001:db8::/32`".
    pub allow_cidrs: Vec<String>,
    /// Optional CIDR denylist.
    pub deny_cidrs: Vec<String>,
    /// Disconnect on per-peer post overflow (bounded per-topic channels)
    pub disconnect_on_post_overflow: bool,
    /// Maximum allowed frame size (bytes) for P2P messages
    pub max_frame_bytes: usize,
    /// `TCP_NODELAY` setting for TCP sockets
    pub tcp_nodelay: bool,
    /// TCP keepalive duration for sockets (if any)
    pub tcp_keepalive: Option<Duration>,
    /// Per-topic frame caps (bytes) for Consensus messages.
    pub max_frame_bytes_consensus: usize,
    /// Per-topic frame caps (bytes) for Control messages.
    pub max_frame_bytes_control: usize,
    /// Per-topic frame caps (bytes) for `BlockSync` messages.
    pub max_frame_bytes_block_sync: usize,
    /// Per-topic frame caps (bytes) for `TxGossip` messages.
    pub max_frame_bytes_tx_gossip: usize,
    /// Per-topic frame caps (bytes) for `PeerGossip` messages.
    pub max_frame_bytes_peer_gossip: usize,
    /// Per-topic frame caps (bytes) for Health messages.
    pub max_frame_bytes_health: usize,
    /// Per-topic frame caps (bytes) for Other messages.
    pub max_frame_bytes_other: usize,
    /// TLS policy: restrict to TLS 1.3 only when using TLS-over-TCP.
    pub tls_only_v1_3: bool,
    /// QUIC max idle timeout for stream inactivity (if QUIC is enabled).
    pub quic_max_idle_timeout: Option<Duration>,
}

/// P2P relay role for constrained deployments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayMode {
    /// Relay disabled; nodes connect directly to each other (default).
    Disabled,
    /// Relay hub; accept spokes and forward traffic.
    Hub,
    /// Relay spoke; dial only the hub and rely on forwarding.
    Spoke,
}

/// Hardware acceleration settings (actual layer).
#[derive(Debug, Clone, Copy)]
pub struct Acceleration {
    /// Enable SIMD acceleration (NEON/AVX/SSE) when available; when false, force scalar execution.
    pub enable_simd: bool,
    /// Enable CUDA backend when compiled and available.
    pub enable_cuda: bool,
    /// Enable Metal backend when compiled and available (macOS).
    pub enable_metal: bool,
    /// Maximum number of GPUs to initialize (None = auto/no cap).
    pub max_gpus: Option<usize>,
    /// Minimum number of leaves to use GPU for Merkle leaf hashing.
    pub merkle_min_leaves_gpu: usize,
    /// Backend-specific thresholds (None = inherit generic GPU threshold).
    pub merkle_min_leaves_metal: Option<usize>,
    /// Minimum leaves for CUDA to be used for Merkle leaf hashing (None = inherit GPU default).
    pub merkle_min_leaves_cuda: Option<usize>,
    /// Prefer CPU SHA2 for trees up to this many leaves (per-arch). If None, use defaults.
    pub prefer_cpu_sha2_max_leaves_aarch64: Option<usize>,
    /// Prefer CPU SHA2 threshold (`x86/x86_64`). If None, use defaults.
    pub prefer_cpu_sha2_max_leaves_x86: Option<usize>,
}

/// Execution mode for the FASTPQ prover backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastpqExecutionMode {
    /// Detect available accelerators at runtime and pick the best option.
    Auto,
    /// Force CPU execution even if accelerators are present.
    Cpu,
    /// Force GPU execution (falls back to CPU if kernels are unavailable at runtime).
    Gpu,
}

/// Poseidon pipeline override for the FASTPQ prover backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastpqPoseidonMode {
    /// Follow the global execution mode (default).
    Auto,
    /// Force CPU hashing even if FFT/LDE use the GPU.
    Cpu,
    /// Prefer GPU hashing even if the global execution mode falls back to CPU.
    Gpu,
}

/// FASTPQ prover configuration.
#[derive(Debug, Clone)]
pub struct Fastpq {
    /// Execution mode used when initialising the prover backend.
    pub execution_mode: FastpqExecutionMode,
    /// Poseidon pipeline override (defaults to the execution mode when `Auto`).
    pub poseidon_mode: FastpqPoseidonMode,
    /// Optional telemetry label describing the host/device class.
    pub device_class: Option<String>,
    /// Optional chip-family label used for telemetry slicing.
    pub chip_family: Option<String>,
    /// Optional GPU kind label (integrated vs. discrete) exposed via telemetry.
    pub gpu_kind: Option<String>,
    /// Optional Metal queue fan-out override (1–4 queues).
    pub metal_queue_fanout: Option<usize>,
    /// Optional Metal queue column threshold override (positive total columns).
    pub metal_queue_column_threshold: Option<u32>,
    /// Optional cap on concurrent Metal command buffers (None = heuristic).
    pub metal_max_in_flight: Option<usize>,
    /// Optional override for Metal threadgroup width (None = pipeline default).
    pub metal_threadgroup_width: Option<u64>,
    /// Enable per-dispatch Metal tracing (developer diagnostic; defaults off).
    pub metal_trace: bool,
    /// Emit verbose Metal device enumeration logs (developer diagnostic; defaults off).
    pub metal_debug_enum: bool,
    /// Emit verbose fused Poseidon failure diagnostics (developer diagnostic; defaults off).
    pub metal_debug_fused: bool,
}

/// Reference to a verifying key by backend and name.
#[derive(Debug, Clone)]
pub struct VerifyingKeyRef {
    /// Backend identifier of the verifying key backend.
    ///
    /// Examples: "halo2/ipa", "groth16/bn254". This string selects the
    /// verification scheme and the curve/domain parameters to use when
    /// validating proofs.
    pub backend: String,
    /// Human‑readable verifying‑key name within the backend namespace.
    ///
    /// This is a logical identifier (e.g., "`ballot_v1`") that maps to an
    /// on‑chain or out‑of‑band provisioned verifying key record for the given
    /// `backend`.
    pub name: String,
}

/// Citizen service discipline knobs applied to governance draws and reliability tracking.
#[derive(Debug, Clone)]
pub struct CitizenServiceDiscipline {
    /// Cooldown (blocks) enforced after a citizen accepts a seat.
    pub seat_cooldown_blocks: u64,
    /// Maximum seats a single citizen may occupy within one epoch.
    pub max_seats_per_epoch: u32,
    /// Declines permitted per epoch without slashing.
    pub free_declines_per_epoch: u32,
    /// Slash applied when declines exceed the free budget (basis points).
    pub decline_slash_bps: u16,
    /// Slash applied when a citizen fails to appear for an assigned seat (basis points).
    pub no_show_slash_bps: u16,
    /// Slash applied when misconduct is recorded for an assigned seat (basis points).
    pub misconduct_slash_bps: u16,
    /// Optional bond multipliers keyed by governance role name.
    pub role_bond_multipliers: BTreeMap<String, u64>,
}

impl CitizenServiceDiscipline {
    /// Lookup the bond multiplier for a specific governance role (defaults to 1).
    #[must_use]
    pub fn bond_multiplier_for_role(&self, role: &str) -> u64 {
        self.role_bond_multipliers.get(role).copied().unwrap_or(1)
    }

    /// Validate that configured percentages remain within basis-point bounds.
    pub fn assert_valid(&self) {
        for (label, value) in [
            ("citizen_decline_slash_bps", self.decline_slash_bps),
            ("citizen_no_show_slash_bps", self.no_show_slash_bps),
            ("citizen_misconduct_slash_bps", self.misconduct_slash_bps),
        ] {
            assert!(
                value <= 10_000,
                "{label} must not exceed 10_000 bps (found {value})"
            );
        }
    }
}

impl Default for CitizenServiceDiscipline {
    fn default() -> Self {
        Self {
            seat_cooldown_blocks: defaults::governance::citizen_service::SEAT_COOLDOWN_BLOCKS,
            max_seats_per_epoch: defaults::governance::citizen_service::MAX_SEATS_PER_EPOCH,
            free_declines_per_epoch: defaults::governance::citizen_service::FREE_DECLINES_PER_EPOCH,
            decline_slash_bps: defaults::governance::citizen_service::DECLINE_SLASH_BPS,
            no_show_slash_bps: defaults::governance::citizen_service::NO_SHOW_SLASH_BPS,
            misconduct_slash_bps: defaults::governance::citizen_service::MISCONDUCT_SLASH_BPS,
            role_bond_multipliers: defaults::governance::citizen_service::role_bond_multipliers(),
        }
    }
}

/// Viral incentive policy governing social reward flows.
#[derive(Debug, Clone)]
pub struct ViralIncentives {
    /// Account supplying reward payouts and sender bonuses.
    pub incentive_pool_account: AccountId,
    /// Account used to hold pending escrows for unbound handles.
    pub escrow_account: AccountId,
    /// Asset definition used for rewards/escrows.
    pub reward_asset_definition_id: AssetDefinitionId,
    /// Amount paid for a valid follow binding.
    pub follow_reward_amount: Numeric,
    /// Bonus paid back to the sender on first delivery.
    pub sender_bonus_amount: Numeric,
    /// Maximum rewards a UAID may claim per day.
    pub max_daily_claims_per_uaid: u32,
    /// Maximum rewards allowed per binding (lifetime).
    pub max_claims_per_binding: u32,
    /// Daily reward budget (spent + bonuses) in reward units.
    pub daily_budget: Numeric,
    /// When true, reward/escrow flows are halted.
    pub halt: bool,
    /// Denied UAIDs that cannot receive payouts.
    pub deny_uaids: Vec<UniversalAccountId>,
    /// Denied binding digests that cannot be rewarded.
    pub deny_binding_digests: Vec<Hash>,
    /// Optional promotion window start (Unix timestamp ms). `None` = always on.
    pub promo_starts_at_ms: Option<u64>,
    /// Optional promotion window end (Unix timestamp ms). `None` = unbounded.
    pub promo_ends_at_ms: Option<u64>,
    /// Aggregate campaign budget cap across the promo window (0 = unlimited).
    pub campaign_cap: Numeric,
}

impl Default for ViralIncentives {
    fn default() -> Self {
        Self {
            incentive_pool_account: defaults::governance::viral_incentive_pool_account()
                .parse()
                .expect("default viral incentive pool account"),
            escrow_account: defaults::governance::viral_escrow_account()
                .parse()
                .expect("default viral escrow account"),
            reward_asset_definition_id: defaults::governance::viral_reward_asset_id()
                .parse()
                .expect("default viral reward asset id"),
            follow_reward_amount: defaults::governance::viral_follow_reward_amount(),
            sender_bonus_amount: defaults::governance::viral_sender_bonus_amount(),
            max_daily_claims_per_uaid: defaults::governance::VIRAL_MAX_DAILY_CLAIMS_PER_UAID,
            max_claims_per_binding: defaults::governance::VIRAL_MAX_CLAIMS_PER_BINDING,
            daily_budget: defaults::governance::viral_daily_budget(),
            halt: false,
            deny_uaids: Vec::new(),
            deny_binding_digests: Vec::new(),
            promo_starts_at_ms: defaults::governance::viral_promo_start_ms(),
            promo_ends_at_ms: defaults::governance::viral_promo_end_ms(),
            campaign_cap: defaults::governance::viral_campaign_cap(),
        }
    }
}

/// Runtime-upgrade provenance enforcement modes (actual layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeUpgradeProvenanceMode {
    /// Provenance is optional; when provided it is verified.
    Optional,
    /// Provenance is required for runtime upgrade manifests.
    Required,
}

impl RuntimeUpgradeProvenanceMode {
    /// Return whether provenance is required.
    #[inline]
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

impl From<user::RuntimeUpgradeProvenanceMode> for RuntimeUpgradeProvenanceMode {
    fn from(mode: user::RuntimeUpgradeProvenanceMode) -> Self {
        match mode {
            user::RuntimeUpgradeProvenanceMode::Optional => Self::Optional,
            user::RuntimeUpgradeProvenanceMode::Required => Self::Required,
        }
    }
}

/// Runtime-upgrade provenance policy (actual layer).
#[derive(Debug, Clone)]
pub struct RuntimeUpgradeProvenancePolicy {
    /// Enforcement mode for provenance.
    pub mode: RuntimeUpgradeProvenanceMode,
    /// Require at least one SBOM digest entry when provenance is present/required.
    pub require_sbom: bool,
    /// Require a non-empty SLSA attestation when provenance is present/required.
    pub require_slsa: bool,
    /// Trusted signer public keys.
    pub trusted_signers: BTreeSet<PublicKey>,
    /// Minimum number of trusted signatures required.
    pub signature_threshold: usize,
}

impl Default for RuntimeUpgradeProvenancePolicy {
    fn default() -> Self {
        Self {
            mode: RuntimeUpgradeProvenanceMode::Optional,
            require_sbom: defaults::governance::RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SBOM,
            require_slsa: defaults::governance::RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SLSA,
            trusted_signers: BTreeSet::new(),
            signature_threshold:
                defaults::governance::RUNTIME_UPGRADE_PROVENANCE_SIGNATURE_THRESHOLD,
        }
    }
}

/// Governance configuration (actual layer).
#[derive(Debug, Clone)]
pub struct Governance {
    /// Optional default verifying key for ZK ballots (backend + name).
    pub vk_ballot: Option<VerifyingKeyRef>,
    /// Optional default verifying key for ZK tallies (backend + name).
    pub vk_tally: Option<VerifyingKeyRef>,
    /// Asset definition used to denominate governance bonds and voting locks.
    pub voting_asset_id: AssetDefinitionId,
    /// Asset definition used to denominate citizenship bonds.
    pub citizenship_asset_id: AssetDefinitionId,
    /// Minimum amount (in smallest units of `citizenship_asset_id`) required to register as a citizen.
    pub citizenship_bond_amount: u128,
    /// Escrow account that custody citizenship bonds until expiry or revocation.
    pub citizenship_escrow_account: AccountId,
    /// Minimum amount (in smallest units of `voting_asset_id`) required to submit a ballot.
    pub min_bond_amount: u128,
    /// Escrow account that custody governance bonds until expiry or slash.
    pub bond_escrow_account: AccountId,
    /// Account that receives slashed governance bonds (may mirror escrow).
    pub slash_receiver_account: AccountId,
    /// Slash percentage for double-vote attempts (basis points, 0–10_000).
    pub slash_double_vote_bps: u16,
    /// Slash percentage applied when ballot proofs are invalid (basis points).
    pub slash_invalid_proof_bps: u16,
    /// Slash percentage applied when eligibility proofs do not match (basis points).
    pub slash_ineligible_proof_bps: u16,
    /// Minimum TEU balance required to accept alias attestations.
    pub alias_teu_minimum: u128,
    /// Emit alias frontier telemetry and stats.
    pub alias_frontier_telemetry: bool,
    /// Emit debug tracing for governance pipeline progression.
    pub debug_trace_pipeline: bool,
    /// Allowed JDG signature schemes for attestation validation.
    pub jdg_signature_schemes: BTreeSet<JdgSignatureScheme>,
    /// Runtime upgrade provenance enforcement policy.
    pub runtime_upgrade_provenance: RuntimeUpgradeProvenancePolicy,
    /// Citizen service discipline knobs (cooldown/seat caps/slashing).
    pub citizen_service: CitizenServiceDiscipline,
    /// Viral incentive policy for social rewards.
    pub viral_incentives: ViralIncentives,
    /// SoraFS pin policy constraints enforced during manifest admission.
    pub sorafs_pin_policy: SorafsPinPolicyConstraints,
    /// SoraFS pricing schedule and credit policy.
    pub sorafs_pricing: PricingScheduleRecord,
    /// SoraFS under-delivery penalty policy applied to provider credits.
    pub sorafs_penalty: SorafsPenaltyPolicy,
    /// Repair escalation governance policy for SoraFS incidents.
    pub sorafs_repair_escalation: RepairEscalationPolicyV1,
    /// SoraFS telemetry authentication/replay safeguards.
    pub sorafs_telemetry: SorafsTelemetryPolicy,
    /// Static provider→owner bindings seeded at startup.
    pub sorafs_provider_owners: BTreeMap<ProviderId, AccountId>,
    /// Conviction step in blocks for plain (non‑ZK) voting. Duration/step yields extra weight.
    pub conviction_step_blocks: u64,
    /// Maximum conviction multiplier allowed in plain (non‑ZK) voting.
    pub max_conviction: u64,
    /// Minimum enactment delay (in blocks) for generating referendum windows.
    pub min_enactment_delay: u64,
    /// Referendum window span (in blocks); `h_end = h_start + span - 1`.
    pub window_span: u64,
    /// Allow non‑ZK quadratic voting (plain ballots). If false, plain ballots are rejected.
    pub plain_voting_enabled: bool,
    /// Approval threshold numerator (Q-format): approve / (approve + reject) >= num/den.
    pub approval_threshold_q_num: u64,
    /// Approval threshold denominator (Q-format).
    pub approval_threshold_q_den: u64,
    /// Minimum turnout required (approve + reject + abstain) to consider the referendum.
    pub min_turnout: u128,
    /// Sortition council committee size.
    pub parliament_committee_size: usize,
    /// Number of blocks per council term.
    pub parliament_term_blocks: u64,
    /// Minimum stake (in smallest units) required to qualify for sortition.
    pub parliament_min_stake: u128,
    /// Asset definition used to measure governance stake eligibility.
    pub parliament_eligibility_asset_id: AssetDefinitionId,
    /// Alternates drawn per term (None = committee size).
    pub parliament_alternate_size: Option<usize>,
    /// Quorum requirement for council approvals (basis points, ceil-divided).
    pub parliament_quorum_bps: u16,
    /// Rules Committee size.
    pub rules_committee_size: usize,
    /// Agenda Council size.
    pub agenda_council_size: usize,
    /// Interest Panel size.
    pub interest_panel_size: usize,
    /// Review Panel size.
    pub review_panel_size: usize,
    /// Policy Jury size.
    pub policy_jury_size: usize,
    /// Oversight Committee size.
    pub oversight_committee_size: usize,
    /// MPC/FMA board size.
    pub fma_committee_size: usize,
    /// Maximum blocks between proposal creation and referendum opening.
    pub pipeline_study_sla_blocks: u64,
    /// Maximum blocks allocated to the referendum voting window.
    pub pipeline_review_sla_blocks: u64,
    /// Maximum blocks allowed to compute/record the referendum decision.
    pub pipeline_decision_sla_blocks: u64,
    /// Maximum blocks allowed to enact an approved proposal after decision.
    pub pipeline_enactment_sla_blocks: u64,
    /// Maximum blocks allocated to rules committee approvals.
    pub pipeline_rules_sla_blocks: u64,
    /// Maximum blocks allocated to agenda council scheduling.
    pub pipeline_agenda_sla_blocks: u64,
}

impl Default for Governance {
    fn default() -> Self {
        Self {
            vk_ballot: None,
            vk_tally: None,
            voting_asset_id: defaults::governance::voting_asset_id()
                .parse()
                .expect("valid default voting asset id"),
            citizenship_asset_id: defaults::governance::citizenship_asset_id()
                .parse()
                .expect("valid default citizenship asset id"),
            citizenship_bond_amount: defaults::governance::citizenship_bond_amount(),
            citizenship_escrow_account: defaults::governance::citizenship_escrow_account()
                .parse()
                .expect("valid default citizenship escrow account id"),
            min_bond_amount: 150,
            bond_escrow_account: defaults::governance::bond_escrow_account()
                .parse()
                .expect("valid default bond escrow account id"),
            slash_receiver_account: defaults::governance::slash_receiver_account()
                .parse()
                .expect("valid default slash receiver account id"),
            slash_double_vote_bps: defaults::governance::slash_policy::DOUBLE_VOTE_BPS,
            slash_invalid_proof_bps: defaults::governance::slash_policy::MISCONDUCT_BPS,
            slash_ineligible_proof_bps: defaults::governance::slash_policy::INELIGIBLE_PROOF_BPS,
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
            runtime_upgrade_provenance: RuntimeUpgradeProvenancePolicy::default(),
            citizen_service: CitizenServiceDiscipline {
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
            viral_incentives: ViralIncentives::default(),
            sorafs_pin_policy: SorafsPinPolicyConstraints::default(),
            sorafs_pricing: PricingScheduleRecord::launch_default(),
            sorafs_penalty: SorafsPenaltyPolicy::default(),
            sorafs_repair_escalation: RepairEscalationPolicyV1::default(),
            sorafs_telemetry: SorafsTelemetryPolicy::default(),
            sorafs_provider_owners: BTreeMap::new(),
            conviction_step_blocks: 100,
            max_conviction: 6,
            min_enactment_delay: 20,
            window_span: 100,
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
            parliament_quorum_bps: defaults::governance::PARLIAMENT_QUORUM_BPS,
            rules_committee_size: defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE,
            agenda_council_size: defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE,
            interest_panel_size: defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE,
            review_panel_size: defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE,
            policy_jury_size: defaults::governance::PARLIAMENT_POLICY_JURY_SIZE,
            oversight_committee_size: defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE,
            fma_committee_size: defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE,
            pipeline_study_sla_blocks: defaults::governance::PIPELINE_STUDY_SLA_BLOCKS,
            pipeline_review_sla_blocks: defaults::governance::PIPELINE_REVIEW_SLA_BLOCKS,
            pipeline_decision_sla_blocks: defaults::governance::PIPELINE_DECISION_SLA_BLOCKS,
            pipeline_enactment_sla_blocks: defaults::governance::PIPELINE_ENACTMENT_SLA_BLOCKS,
            pipeline_rules_sla_blocks: defaults::governance::PIPELINE_RULES_SLA_BLOCKS,
            pipeline_agenda_sla_blocks: defaults::governance::PIPELINE_AGENDA_SLA_BLOCKS,
        }
    }
}

/// Concurrency controls for internal thread pools.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct Concurrency {
    /// Minimum scheduler worker threads (0 = auto/physical cores)
    pub scheduler_min_threads: usize,
    /// Maximum scheduler worker threads (0 = auto/physical cores)
    pub scheduler_max_threads: usize,
    /// Global Rayon thread pool size (0 = auto/physical cores)
    pub rayon_global_threads: usize,
    /// Stack size (bytes) for scheduler worker threads.
    pub scheduler_stack_bytes: usize,
    /// Stack size (bytes) for prover worker threads.
    pub prover_stack_bytes: usize,
    /// Guest stack size (bytes) for IVM instances.
    pub guest_stack_bytes: u64,
    /// Gas→stack multiplier (bytes of stack available per unit of gas).
    pub gas_to_stack_multiplier: u64,
}

impl Concurrency {
    /// Construct a concurrency configuration from repository defaults.
    #[must_use]
    pub const fn from_defaults() -> Self {
        Self {
            scheduler_min_threads: defaults::concurrency::SCHEDULER_MIN,
            scheduler_max_threads: defaults::concurrency::SCHEDULER_MAX,
            rayon_global_threads: defaults::concurrency::RAYON_GLOBAL,
            scheduler_stack_bytes: defaults::concurrency::SCHEDULER_STACK_BYTES,
            prover_stack_bytes: defaults::concurrency::PROVER_STACK_BYTES,
            guest_stack_bytes: defaults::concurrency::GUEST_STACK_BYTES,
            gas_to_stack_multiplier: defaults::concurrency::GAS_TO_STACK_MULTIPLIER,
        }
    }

    /// Validate stack sizes to ensure they are non-zero and within sane bounds.
    pub fn validate(&self) -> core::result::Result<(), Report<ParseError>> {
        let max_guest = defaults::concurrency::GUEST_STACK_BYTES_MAX;
        if self.guest_stack_bytes == 0 || self.guest_stack_bytes > max_guest {
            return Err(
                Report::new(ParseError::InvalidConcurrencyConfig).attach(format!(
                    "guest_stack_bytes must be in [1, {}], got {}",
                    max_guest, self.guest_stack_bytes
                )),
            );
        }
        if self.scheduler_stack_bytes == 0 || self.prover_stack_bytes == 0 {
            return Err(Report::new(ParseError::InvalidConcurrencyConfig)
                .attach("scheduler_stack_bytes and prover_stack_bytes must be non-zero"));
        }
        if self.gas_to_stack_multiplier == 0 {
            return Err(Report::new(ParseError::InvalidConcurrencyConfig)
                .attach("gas_to_stack_multiplier must be non-zero"));
        }
        Ok(())
    }
}

/// Governance configuration (actual layer).
/// Parsed genesis configuration
#[derive(Debug, Clone)]
pub struct Genesis {
    /// Genesis account public key
    pub public_key: PublicKey,
    /// Path to `GenesisBlock`.
    /// If it is none, the peer can only observe the genesis block.
    /// If it is some, the peer is responsible for submitting the genesis block.
    pub file: Option<WithOrigin<PathBuf>>,
    /// Optional path to genesis manifest JSON for validation at startup.
    pub manifest_json: Option<WithOrigin<PathBuf>>,
    /// Optional expected genesis block hash used during bootstrap preflight.
    pub expected_hash: Option<HashOf<BlockHeader>>,
    /// Optional peer allowlist permitted to serve genesis during bootstrap (falls back to trusted peers).
    pub bootstrap_allowlist: Vec<PeerId>,
    /// Maximum genesis payload size accepted/served during bootstrap (bytes).
    pub bootstrap_max_bytes: u64,
    /// Minimum interval between serving bootstrap responses.
    pub bootstrap_response_throttle: Duration,
    /// Per-attempt bootstrap request timeout (preflight + payload).
    pub bootstrap_request_timeout: Duration,
    /// Backoff between bootstrap attempts.
    pub bootstrap_retry_interval: Duration,
    /// Maximum bootstrap attempts before failing.
    pub bootstrap_max_attempts: u32,
    /// Whether to attempt bootstrap when local genesis is missing.
    pub bootstrap_enabled: bool,
}

/// Transaction queue settings.
#[derive(Debug, Clone, Copy)]
pub struct Queue {
    /// Maximum number of transactions allowed in the queue.
    pub capacity: NonZeroUsize,
    /// Per-user transaction limit in the queue.
    pub capacity_per_user: NonZeroUsize,
    /// Transaction time-to-live.
    pub transaction_time_to_live: Duration,
    /// Minimum interval between expired-transaction sweeps.
    pub expired_cull_interval: Duration,
}

/// Nexus staking configuration (public lanes).
#[derive(Debug, Clone)]
pub struct NexusStaking {
    /// Validator activation policy for public lanes.
    pub public_validator_mode: LaneValidatorMode,
    /// Validator activation policy for restricted/permissioned lanes.
    pub restricted_validator_mode: LaneValidatorMode,
    /// Minimum bonded stake required to register or bond as a validator (asset base units).
    pub min_validator_stake: u64,
    /// Maximum number of validators allowed per lane.
    pub max_validators: NonZeroU32,
    /// Minimum delay between scheduling and finalising an unbond (milliseconds).
    pub unbonding_delay: Duration,
    /// Grace window after `release_at_ms` during which withdrawals must be finalised (milliseconds).
    pub withdraw_grace: Duration,
    /// Maximum slash ratio allowed (basis points, 10_000 = 100%).
    pub max_slash_bps: u16,
    /// Minimum reward amount (base units) paid out; smaller amounts are skipped as dust.
    pub reward_dust_threshold: u64,
    /// Asset definition used for staking bonds (string form).
    pub stake_asset_id: String,
    /// Escrow account that holds bonded stake (string form).
    pub stake_escrow_account_id: String,
    /// Account that receives slashed stake (string form).
    pub slash_sink_account_id: String,
}

impl Default for NexusStaking {
    fn default() -> Self {
        Self {
            public_validator_mode: LaneValidatorMode::StakeElected,
            restricted_validator_mode: LaneValidatorMode::AdminManaged,
            min_validator_stake: defaults::nexus::staking::MIN_VALIDATOR_STAKE,
            max_validators: defaults::nexus::staking::MAX_VALIDATORS,
            unbonding_delay: defaults::nexus::staking::UNBONDING_DELAY,
            withdraw_grace: defaults::nexus::staking::WITHDRAW_GRACE,
            max_slash_bps: defaults::nexus::staking::MAX_SLASH_BPS,
            reward_dust_threshold: defaults::nexus::staking::REWARD_DUST_THRESHOLD,
            stake_asset_id: defaults::nexus::staking::stake_asset_id(),
            stake_escrow_account_id: defaults::nexus::staking::stake_escrow_account_id(),
            slash_sink_account_id: defaults::nexus::staking::slash_sink_account_id(),
        }
    }
}

impl NexusStaking {
    /// Resolve the validator activation policy for a lane using its configured visibility.
    #[must_use]
    pub fn validator_mode(&self, lane: LaneId, catalog: &LaneCatalog) -> LaneValidatorMode {
        let visibility = catalog
            .lanes()
            .iter()
            .find(|lane_meta| lane_meta.id == lane)
            .map_or(LaneVisibility::Public, |lane_meta| lane_meta.visibility);

        match visibility {
            LaneVisibility::Public => self.public_validator_mode,
            LaneVisibility::Restricted => self.restricted_validator_mode,
        }
    }
}

/// Nexus fee schedule for universal XOR-denominated charges.
#[derive(Debug, Clone)]
pub struct NexusFees {
    /// Asset definition used to collect fees (e.g., `xor#sora`).
    pub fee_asset_id: String,
    /// Account that receives collected fees.
    pub fee_sink_account_id: String,
    /// Base fee charged per transaction (asset base units).
    pub base_fee: u64,
    /// Per-byte fee charged over the signed transaction payload (asset base units).
    pub per_byte_fee: u64,
    /// Per-instruction fee charged for native ISI batches (asset base units).
    pub per_instruction_fee: u64,
    /// Per-gas-unit fee multiplier applied to measured gas usage (asset base units).
    pub per_gas_unit_fee: u64,
    /// Whether fee sponsorship is permitted.
    pub sponsorship_enabled: bool,
    /// Maximum fee a sponsor can cover per transaction (asset base units, 0 = unlimited).
    pub sponsor_max_fee: u64,
}

impl Default for NexusFees {
    fn default() -> Self {
        Self {
            fee_asset_id: defaults::nexus::fees::FEE_ASSET_ID.to_string(),
            fee_sink_account_id: defaults::nexus::fees::FEE_SINK_ACCOUNT_ID.to_string(),
            base_fee: defaults::nexus::fees::BASE_FEE,
            per_byte_fee: defaults::nexus::fees::PER_BYTE_FEE,
            per_instruction_fee: defaults::nexus::fees::PER_INSTRUCTION_FEE,
            per_gas_unit_fee: defaults::nexus::fees::PER_GAS_UNIT_FEE,
            sponsorship_enabled: defaults::nexus::fees::SPONSORSHIP_ENABLED,
            sponsor_max_fee: defaults::nexus::fees::SPONSOR_MAX_FEE,
        }
    }
}

/// Committee and quorum settings for protected-domain endorsements.
#[derive(Debug, Clone)]
pub struct NexusEndorsement {
    /// Committee member public keys allowed to sign endorsements (string form).
    pub committee_keys: Vec<String>,
    /// Quorum required to accept an endorsement (0 disables enforcement).
    pub quorum: u16,
}

impl Default for NexusEndorsement {
    fn default() -> Self {
        Self {
            committee_keys: defaults::nexus::endorsement::committee_keys(),
            quorum: defaults::nexus::endorsement::QUORUM,
        }
    }
}

/// Nexus configuration for AXT execution and expiry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NexusAxt {
    /// Slot length used when deriving AXT expiry slots from block timestamps.
    pub slot_length_ms: NonZeroU64,
    /// Maximum wall-clock skew tolerated when enforcing AXT expiry.
    pub max_clock_skew_ms: u64,
    /// Number of slots to retain cached proofs (accepted or rejected) for reuse/replay rejection.
    pub proof_cache_ttl_slots: NonZeroU64,
    /// Number of slots to retain handle usage for replay protection across restarts/peers.
    pub replay_retention_slots: NonZeroU64,
}

impl Default for NexusAxt {
    fn default() -> Self {
        Self {
            slot_length_ms: NonZeroU64::new(defaults::nexus::axt::SLOT_LENGTH_MS)
                .expect("default AXT slot length must be non-zero"),
            max_clock_skew_ms: defaults::nexus::axt::CLOCK_SKEW_MS_DEFAULT,
            proof_cache_ttl_slots: NonZeroU64::new(defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS)
                .expect("proof cache TTL must be non-zero"),
            replay_retention_slots: NonZeroU64::new(defaults::nexus::axt::REPLAY_RETENTION_SLOTS)
                .expect("replay retention window must be non-zero"),
        }
    }
}

/// Lane-relay emergency override configuration.
#[derive(Debug, Clone, Copy)]
pub struct LaneRelayEmergency {
    /// Whether emergency validator overrides are enabled.
    pub enabled: bool,
    /// Minimum multisig threshold required for override transactions.
    pub multisig_threshold: NonZeroU16,
    /// Minimum multisig member count required for override transactions.
    pub multisig_members: NonZeroU16,
}

impl Default for LaneRelayEmergency {
    fn default() -> Self {
        Self {
            enabled: defaults::nexus::lane_relay_emergency::ENABLED,
            multisig_threshold: NonZeroU16::new(
                defaults::nexus::lane_relay_emergency::MULTISIG_THRESHOLD,
            )
            .expect("default threshold must be non-zero"),
            multisig_members: NonZeroU16::new(
                defaults::nexus::lane_relay_emergency::MULTISIG_MEMBERS,
            )
            .expect("default member count must be non-zero"),
        }
    }
}

/// Storage budget configuration for Nexus-enabled nodes.
#[derive(Debug, Clone, Copy)]
pub struct NexusStorage {
    /// Aggregate on-disk storage budget (bytes).
    pub max_disk_usage_bytes: Bytes<u64>,
    /// WSV hot-tier deterministic payload size budget (bytes).
    pub max_wsv_memory_bytes: Bytes<u64>,
    /// Budget weights for dividing the disk cap across subsystems.
    pub disk_budget_weights: NexusStorageWeights,
}

impl Default for NexusStorage {
    fn default() -> Self {
        Self {
            max_disk_usage_bytes: defaults::nexus::storage::MAX_DISK_USAGE_BYTES,
            max_wsv_memory_bytes: defaults::nexus::storage::MAX_WSV_MEMORY_BYTES,
            disk_budget_weights: NexusStorageWeights::default(),
        }
    }
}

/// Basis-point budget weights for Nexus storage subsystems.
#[derive(Debug, Clone, Copy)]
pub struct NexusStorageWeights {
    /// Budget share for Kura block storage (basis points).
    pub kura_blocks_bps: u16,
    /// Budget share for tiered-state cold snapshots (basis points).
    pub wsv_snapshots_bps: u16,
    /// Budget share for SoraFS storage (basis points).
    pub sorafs_bps: u16,
    /// Budget share for SoraNet route spools (basis points).
    pub soranet_spool_bps: u16,
    /// Budget share reserved for future SoraVPN storage (basis points).
    pub soravpn_spool_bps: u16,
}

impl NexusStorageWeights {
    /// Total basis points across all weights.
    #[must_use]
    pub const fn total_bps(self) -> u32 {
        self.kura_blocks_bps as u32
            + self.wsv_snapshots_bps as u32
            + self.sorafs_bps as u32
            + self.soranet_spool_bps as u32
            + self.soravpn_spool_bps as u32
    }
}

impl Default for NexusStorageWeights {
    fn default() -> Self {
        Self {
            kura_blocks_bps: defaults::nexus::storage::KURA_BLOCKS_BPS,
            wsv_snapshots_bps: defaults::nexus::storage::WSV_SNAPSHOTS_BPS,
            sorafs_bps: defaults::nexus::storage::SORAFS_BPS,
            soranet_spool_bps: defaults::nexus::storage::SORANET_SPOOL_BPS,
            soravpn_spool_bps: defaults::nexus::storage::SORAVPN_SPOOL_BPS,
        }
    }
}

/// Nexus configuration describing lanes, data spaces, and routing policy.
#[derive(Debug, Clone)]
pub struct Nexus {
    /// Whether multilane (Nexus/Iroha3) features are enabled at runtime.
    pub enabled: bool,
    /// Storage budget configuration for Nexus-enabled nodes.
    pub storage: NexusStorage,
    /// Staking guardrails for public lanes.
    pub staking: NexusStaking,
    /// Universal fee schedule for Nexus transactions.
    pub fees: NexusFees,
    /// Domain endorsement controls.
    pub endorsement: NexusEndorsement,
    /// AXT execution and expiry configuration.
    pub axt: NexusAxt,
    /// Lane-relay emergency override configuration.
    pub lane_relay_emergency: LaneRelayEmergency,
    /// Validated lane catalog.
    pub lane_catalog: LaneCatalog,
    /// Derived storage/configuration geometry for lanes.
    pub lane_config: LaneConfig,
    /// Validated data-space catalog.
    pub dataspace_catalog: DataSpaceCatalog,
    /// Lane routing policy.
    pub routing_policy: LaneRoutingPolicy,
    /// Lane manifest registry configuration.
    pub registry: LaneRegistry,
    /// Governance module catalog.
    pub governance: GovernanceCatalog,
    /// Lane compliance policy configuration.
    pub compliance: LaneCompliance,
    /// Lane-fusion tuning.
    pub fusion: Fusion,
    /// Proof/commit deadline configuration.
    pub commit: Commit,
    /// Data-availability sampling configuration.
    pub da: Da,
}

#[allow(clippy::derivable_impls)]
impl Default for Nexus {
    fn default() -> Self {
        Self {
            enabled: false,
            storage: NexusStorage::default(),
            staking: NexusStaking::default(),
            fees: NexusFees::default(),
            endorsement: NexusEndorsement::default(),
            axt: NexusAxt::default(),
            lane_relay_emergency: LaneRelayEmergency::default(),
            lane_catalog: LaneCatalog::default(),
            lane_config: LaneConfig::default(),
            dataspace_catalog: DataSpaceCatalog::default(),
            routing_policy: LaneRoutingPolicy::default(),
            registry: LaneRegistry::default(),
            governance: GovernanceCatalog::default(),
            compliance: LaneCompliance::default(),
            fusion: Fusion::default(),
            commit: Commit::default(),
            da: Da::default(),
        }
    }
}

impl Nexus {
    /// Returns true when the catalog or routing policy deviates from the single-lane defaults.
    #[must_use]
    pub fn uses_multilane_catalogs(&self) -> bool {
        let policy = &self.routing_policy;
        let policy_is_default = policy.rules.is_empty()
            && policy.default_lane == LaneId::SINGLE
            && policy.default_dataspace == DataSpaceId::GLOBAL;
        let catalog_is_default = self.lane_catalog.lane_count().get() == 1
            && matches!(self.lane_catalog.lanes(), [lane] if lane.id == LaneId::SINGLE);
        let dataspace_is_default = matches!(
            self.dataspace_catalog.entries(),
            [entry] if entry.id == DataSpaceId::GLOBAL
        );

        !(policy_is_default && catalog_is_default && dataspace_is_default)
    }

    /// Returns true when any lane/dataspace/routing overrides are present (even in single-lane mode).
    #[must_use]
    pub fn has_lane_overrides(&self) -> bool {
        self.lane_catalog != LaneCatalog::default()
            || self.dataspace_catalog != DataSpaceCatalog::default()
            || self.routing_policy != LaneRoutingPolicy::default()
    }
}

/// Lane manifest registry configuration (placeholder).
#[derive(Debug, Clone)]
pub struct LaneRegistry {
    /// Optional directory containing lane manifest files.
    pub manifest_directory: Option<PathBuf>,
    /// Optional path used to cache downloaded manifests.
    pub cache_directory: Option<PathBuf>,
    /// Poll interval for refreshing manifests and governance data.
    pub poll_interval: Duration,
}

impl Default for LaneRegistry {
    fn default() -> Self {
        Self {
            manifest_directory: None,
            cache_directory: None,
            poll_interval: defaults::nexus::registry::POLL_INTERVAL,
        }
    }
}

/// Lane compliance policy configuration.
#[derive(Debug, Clone)]
pub struct LaneCompliance {
    /// Whether lane-level compliance checks are enabled.
    pub enabled: bool,
    /// When true, decisions are logged but not enforced.
    pub audit_only: bool,
    /// Optional directory containing Norito-encoded policy bundles.
    pub policy_dir: Option<PathBuf>,
}

impl Default for LaneCompliance {
    fn default() -> Self {
        Self {
            enabled: defaults::nexus::compliance::ENABLED,
            audit_only: defaults::nexus::compliance::AUDIT_ONLY,
            policy_dir: None,
        }
    }
}

/// Governance module catalog for lanes.
#[derive(Debug, Clone, Default)]
pub struct GovernanceCatalog {
    /// Default governance module identifier applied when a lane omits an override.
    pub default_module: Option<String>,
    /// Registered governance modules keyed by name.
    pub modules: BTreeMap<String, GovernanceModule>,
}

/// Governance module definition.
#[derive(Debug, Clone, Default)]
pub struct GovernanceModule {
    /// Module type (e.g., `parliament`, `stake_weighted`, `council_multisig`).
    pub module_type: Option<String>,
    /// Additional parameters defined by the module.
    pub params: BTreeMap<String, String>,
}

/// Confidential asset and verifier configuration.
#[derive(Debug, Clone)]
pub struct Confidential {
    /// Enables confidential asset features for this node.
    pub enabled: bool,
    /// Allows observer mode acceptance without verification.
    pub assume_valid: bool,
    /// Preferred verifier backend identifier.
    pub verifier_backend: String,
    /// Maximum proof size accepted from a single confidential operation.
    pub max_proof_size_bytes: u32,
    /// Maximum number of nullifiers per transaction.
    pub max_nullifiers_per_tx: u32,
    /// Maximum number of commitments per transaction.
    pub max_commitments_per_tx: u32,
    /// Maximum confidential operations per block.
    pub max_confidential_ops_per_block: u32,
    /// Verifier timeout.
    pub verify_timeout: Duration,
    /// Maximum anchor age in blocks.
    pub max_anchor_age_blocks: u64,
    /// Aggregate proof bytes allowed per block.
    pub max_proof_bytes_block: u64,
    /// Maximum verification calls per transaction.
    pub max_verify_calls_per_tx: u32,
    /// Maximum verification calls per block.
    pub max_verify_calls_per_block: u32,
    /// Maximum public inputs per proof.
    pub max_public_inputs: u32,
    /// Configured reorg depth bound.
    pub reorg_depth_bound: u64,
    /// Minimum delay between policy change request and activation.
    pub policy_transition_delay_blocks: u64,
    /// Grace window around policy activation.
    pub policy_transition_window_blocks: u64,
    /// Commitment tree root history length.
    pub tree_roots_history_len: u64,
    /// Frontier checkpoint interval.
    pub tree_frontier_checkpoint_interval: u64,
    /// Maximum verifier entries allowed in registry.
    pub registry_max_vk_entries: u32,
    /// Maximum parameter entries allowed in registry.
    pub registry_max_params_entries: u32,
    /// Maximum registry mutations per block.
    pub registry_max_delta_per_block: u32,
    /// Gas schedule applied to confidential proof verification.
    pub gas: ConfidentialGas,
}

/// Confidential verification gas schedule parameters.
#[derive(Debug, Clone, Copy)]
pub struct ConfidentialGas {
    /// Base cost charged for initiating a proof verification.
    pub proof_base: u64,
    /// Cost per public input included in the proof envelope.
    pub per_public_input: u64,
    /// Cost per proof byte.
    pub per_proof_byte: u64,
    /// Cost per nullifier consumed by the proof.
    pub per_nullifier: u64,
    /// Cost per commitment emitted by the proof.
    pub per_commitment: u64,
}

/// Declarative routing policy derived from configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneRoutingPolicy {
    /// Lane used when no rule matches.
    pub default_lane: LaneId,
    /// Dataspace used when a rule does not override it explicitly.
    pub default_dataspace: DataSpaceId,
    /// Ordered list of routing rules.
    pub rules: Vec<LaneRoutingRule>,
}

/// Individual routing rule targeting a lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneRoutingRule {
    /// Lane assigned when the matcher is satisfied.
    pub lane: LaneId,
    /// Optional dataspace override when the matcher is satisfied.
    pub dataspace: Option<DataSpaceId>,
    /// Selection criteria.
    pub matcher: LaneRoutingMatcher,
}

/// Matcher describing which transactions fall under a rule.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneRoutingMatcher {
    /// Optional authority/account string match.
    pub account: Option<String>,
    /// Optional instruction path match.
    pub instruction: Option<String>,
    /// Optional descriptive text.
    pub description: Option<String>,
}

/// Derived per-lane configuration used by state storage and tooling.
#[derive(Debug, Clone)]
pub struct LaneConfig {
    entries: Vec<LaneConfigEntry>,
    by_id: BTreeMap<LaneId, usize>,
}

impl Default for LaneConfig {
    fn default() -> Self {
        Self::from_catalog(&LaneCatalog::default())
    }
}

impl LaneConfig {
    /// Derive lane configuration metadata from the validated catalog.
    #[must_use]
    pub fn from_catalog(catalog: &LaneCatalog) -> Self {
        let mut entries = Vec::with_capacity(catalog.lanes().len());
        let mut by_id = BTreeMap::new();

        for lane in catalog.lanes() {
            let entry = LaneConfigEntry::from_metadata(lane);
            by_id.insert(entry.lane_id, entries.len());
            entries.push(entry);
        }

        Self { entries, by_id }
    }

    /// Iterate over all derived lane entries in catalog order.
    #[must_use]
    pub fn entries(&self) -> &[LaneConfigEntry] {
        &self.entries
    }

    /// Resolve the derived configuration entry for a specific lane.
    #[must_use]
    pub fn entry(&self, id: LaneId) -> Option<&LaneConfigEntry> {
        self.by_id.get(&id).and_then(|&idx| self.entries.get(idx))
    }

    /// Return the first catalog entry, representing the default/primary lane.
    ///
    /// # Panics
    /// Panics when the derived catalog is empty (should never happen because
    /// [`LaneCatalog::default`] always contains the single-lane placeholder).
    #[must_use]
    pub fn primary(&self) -> &LaneConfigEntry {
        self.entries
            .first()
            .expect("lane catalog must contain at least one entry")
    }

    /// Resolve the manifest enforcement policy for a lane.
    #[must_use]
    pub fn manifest_policy(&self, id: LaneId) -> DaManifestPolicy {
        self.entry(id)
            .map(|entry| entry.manifest_policy)
            .unwrap_or_default()
    }

    /// Resolve the shard mapping for all configured lanes.
    #[must_use]
    pub fn shard_mapping(&self) -> BTreeMap<LaneId, ShardId> {
        self.entries
            .iter()
            .map(|entry| (entry.lane_id, ShardId::new(entry.shard_id)))
            .collect()
    }

    /// Resolve the shard identifier for a lane (defaulting to the lane id).
    #[must_use]
    pub fn shard_id(&self, id: LaneId) -> u32 {
        self.entry(id)
            .map_or_else(|| id.as_u32(), |entry| entry.shard_id)
    }

    /// Return true when the lane is marked for confidential compute handling.
    #[must_use]
    pub fn is_confidential_compute(&self, id: LaneId) -> bool {
        self.entry(id)
            .is_some_and(|entry| entry.confidential_compute)
    }

    /// Resolve the confidential compute policy for a lane if configured.
    #[must_use]
    pub fn confidential_compute_policy(&self, id: LaneId) -> Option<&ConfidentialComputePolicy> {
        self.entry(id)
            .and_then(|entry| entry.confidential_policy.as_ref())
    }

    /// Declared access audience labels for a confidential compute lane.
    #[must_use]
    pub fn confidential_access(&self, id: LaneId) -> &[String] {
        self.entry(id)
            .map(|entry| entry.confidential_access.as_slice())
            .unwrap_or_default()
    }
}

/// Derived configuration for a single lane.
#[derive(Debug, Clone)]
pub struct LaneConfigEntry {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Shard identifier this lane maps to (defaults to the lane id).
    pub shard_id: u32,
    /// Dataspace identifier the lane belongs to.
    pub dataspace_id: DataSpaceId,
    /// Declarative visibility profile.
    pub visibility: LaneVisibility,
    /// Storage profile applied to the lane.
    pub storage_profile: LaneStorageProfile,
    /// Proof scheme expected for DA commitments on this lane.
    pub proof_scheme: DaProofScheme,
    /// Lane alias copied from metadata.
    pub alias: String,
    /// Normalised slug used for file names/metrics.
    pub slug: String,
    /// Kura segment namespace used for lane-local ledger storage.
    pub kura_segment: String,
    /// Kura segment name used for merge-ledger metadata.
    pub merge_segment: String,
    /// Deterministic key prefix applied to MV storage keys.
    pub key_prefix: [u8; 4],
    /// Manifest availability enforcement policy for this lane.
    pub manifest_policy: DaManifestPolicy,
    /// Whether the lane is marked for confidential compute handling.
    pub confidential_compute: bool,
    /// Confidential compute policy derived from lane metadata (if enabled).
    pub confidential_policy: Option<ConfidentialComputePolicy>,
    /// Declared audiences allowed to fetch confidential compute payloads.
    pub confidential_access: Vec<String>,
}

impl LaneConfigEntry {
    const MANIFEST_POLICY_METADATA_KEY: &'static str = "da_manifest_policy";
    const SHARD_ID_METADATA_KEY: &'static str = "da_shard_id";
    const CONFIDENTIAL_FLAG_KEY: &'static str = "confidential_compute";
    const CONFIDENTIAL_MECHANISM_KEY: &'static str = "confidential_mechanism";
    const CONFIDENTIAL_KEY_VERSION_KEY: &'static str = "confidential_key_version";
    const CONFIDENTIAL_ACCESS_KEY: &'static str = "confidential_access";

    fn from_metadata(meta: &LaneConfigMetadata) -> Self {
        let slug = Self::slugify(&meta.alias, meta.id);
        let lane_numeric = meta.id.as_u32();
        let key_prefix = lane_numeric.to_be_bytes();

        let kura_segment = format!("lane_{lane_numeric:03}_{slug}");
        let merge_segment = format!("lane_{lane_numeric:03}_{slug}_merge");
        let manifest_policy = meta
            .metadata
            .get(Self::MANIFEST_POLICY_METADATA_KEY)
            .and_then(|raw| DaManifestPolicy::from_metadata_value(raw))
            .unwrap_or_default();
        let shard_id = meta
            .metadata
            .get(Self::SHARD_ID_METADATA_KEY)
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(lane_numeric);
        let confidential_compute = meta
            .metadata
            .get(Self::CONFIDENTIAL_FLAG_KEY)
            .and_then(|raw| Self::parse_bool(raw.as_str()))
            .unwrap_or(false);
        let confidential_access = meta
            .metadata
            .get(Self::CONFIDENTIAL_ACCESS_KEY)
            .map(|raw| {
                raw.split(',')
                    .filter_map(|entry| {
                        let trimmed = entry.trim();
                        (!trimmed.is_empty()).then(|| trimmed.to_string())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mechanism = meta
            .metadata
            .get(Self::CONFIDENTIAL_MECHANISM_KEY)
            .and_then(|raw| ConfidentialComputeMechanism::from_metadata_value(raw.as_str()))
            .unwrap_or(ConfidentialComputeMechanism::Encryption);
        let key_version = meta
            .metadata
            .get(Self::CONFIDENTIAL_KEY_VERSION_KEY)
            .and_then(|raw| raw.parse::<u32>().ok());
        let confidential_policy = if confidential_compute {
            key_version.map(|key_version| {
                ConfidentialComputePolicy::new(mechanism, key_version, confidential_access.clone())
            })
        } else {
            None
        };

        Self {
            lane_id: meta.id,
            shard_id,
            dataspace_id: meta.dataspace_id,
            visibility: meta.visibility,
            storage_profile: meta.storage,
            proof_scheme: meta.proof_scheme,
            alias: meta.alias.clone(),
            slug,
            kura_segment,
            merge_segment,
            key_prefix,
            manifest_policy,
            confidential_compute,
            confidential_policy,
            confidential_access,
        }
    }

    fn parse_bool(raw: &str) -> Option<bool> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" | "on" => Some(true),
            "0" | "false" | "no" | "n" | "off" => Some(false),
            _ => None,
        }
    }

    fn slugify(alias: &str, lane_id: LaneId) -> String {
        let mut slug = String::with_capacity(alias.len());
        let mut underscore_written = false;

        for ch in alias.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
                underscore_written = false;
            } else if matches!(ch, '-' | ' ' | '_' | '.') {
                if !underscore_written {
                    slug.push('_');
                    underscore_written = true;
                }
            } else if !underscore_written {
                slug.push('_');
                underscore_written = true;
            }
        }

        let slug = slug.trim_matches('_').to_string();
        if slug.is_empty() {
            format!("lane{}", lane_id.as_u32())
        } else {
            slug
        }
    }

    /// Compute the canonical Kura segment directory for this lane.
    #[must_use]
    pub fn blocks_dir(&self, root: impl AsRef<Path>) -> PathBuf {
        root.as_ref().join("blocks").join(&self.kura_segment)
    }

    /// Compute the canonical merge-ledger log path for this lane.
    #[must_use]
    pub fn merge_log_path(&self, root: impl AsRef<Path>) -> PathBuf {
        root.as_ref()
            .join("merge_ledger")
            .join(format!("{}.log", self.merge_segment))
    }
}

/// Per-lane manifest availability enforcement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DaManifestPolicy {
    /// Missing manifests block commitment and proposal sealing.
    #[default]
    Strict,
    /// Missing manifests produce warnings but do not block commitment.
    AuditOnly,
}

impl DaManifestPolicy {
    fn from_metadata_value(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "strict" => Some(Self::Strict),
            "audit" | "audit_only" | "warn" | "warn_only" => Some(Self::AuditOnly),
            _ => None,
        }
    }
}

/// Lane-fusion tuning parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fusion {
    /// Sustained TEU floor (per lane) that triggers fusion when demand stays below it.
    pub floor_teu: u32,
    /// TEU threshold that forces fused lanes to split back to independent operation.
    pub exit_teu: u32,
    /// Number of consecutive slots that must satisfy the floor condition before fusing.
    pub observation_slots: NonZeroU16,
    /// Maximum number of slots a fused window can persist without re-evaluating load.
    pub max_window_slots: NonZeroU16,
}

impl Default for Fusion {
    fn default() -> Self {
        Self {
            floor_teu: defaults::nexus::fusion::FLOOR_TEU,
            exit_teu: defaults::nexus::fusion::EXIT_TEU,
            observation_slots: NonZeroU16::new(defaults::nexus::fusion::OBSERVATION_SLOTS)
                .expect("default observation slots > 0"),
            max_window_slots: NonZeroU16::new(defaults::nexus::fusion::MAX_WINDOW_SLOTS)
                .expect("default max window slots > 0"),
        }
    }
}

/// Proof/commit deadline configuration (Δ window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commit {
    /// Number of slots available for proofs/DA bundles to arrive before a transaction aborts.
    pub window_slots: NonZeroU16,
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            window_slots: NonZeroU16::new(defaults::nexus::commit::WINDOW_SLOTS)
                .expect("default commit window > 0"),
        }
    }
}

/// Data-availability sampling configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Da {
    /// Total in-slot DA signatures budget per lane.
    pub q_in_slot_total: NonZeroU32,
    /// Minimum in-slot DA signatures per dataspace.
    pub q_in_slot_per_ds_min: NonZeroU16,
    /// Baseline attester sample size (S).
    pub sample_size_base: NonZeroU16,
    /// Maximum attester sample size when scaling up coverage.
    pub sample_size_max: NonZeroU16,
    /// Threshold `T` applied off-path for DA certificates.
    pub threshold_base: NonZeroU16,
    /// Number of shards each attester must verify per slot.
    pub per_attester_shards: NonZeroU16,
    /// Rolling audit configuration.
    pub audit: DaAudit,
    /// Recovery deadline configuration.
    pub recovery: DaRecovery,
    /// Temporal diversity / attester rotation configuration.
    pub rotation: DaRotation,
}

impl Default for Da {
    fn default() -> Self {
        Self {
            q_in_slot_total: NonZeroU32::new(defaults::nexus::da::Q_IN_SLOT_TOTAL)
                .expect("default q_in_slot_total > 0"),
            q_in_slot_per_ds_min: NonZeroU16::new(defaults::nexus::da::Q_IN_SLOT_PER_DS_MIN)
                .expect("default q_in_slot_per_ds_min > 0"),
            sample_size_base: NonZeroU16::new(defaults::nexus::da::SAMPLE_SIZE_BASE)
                .expect("default sample_size_base > 0"),
            sample_size_max: NonZeroU16::new(defaults::nexus::da::SAMPLE_SIZE_MAX)
                .expect("default sample_size_max > 0"),
            threshold_base: NonZeroU16::new(defaults::nexus::da::THRESHOLD_BASE)
                .expect("default threshold_base > 0"),
            per_attester_shards: NonZeroU16::new(defaults::nexus::da::PER_ATTESTER_SHARDS)
                .expect("default per_attester_shards > 0"),
            audit: DaAudit::default(),
            recovery: DaRecovery::default(),
            rotation: DaRotation::default(),
        }
    }
}

/// Rolling audit configuration ensuring long-term DA coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaAudit {
    /// Signatures verified per audit window.
    pub sample_size: NonZeroU16,
    /// Number of audit windows retained before slashing for insufficient coverage.
    pub window_count: NonZeroU16,
    /// Duration of an audit window.
    pub interval: Duration,
}

impl Default for DaAudit {
    fn default() -> Self {
        Self {
            sample_size: NonZeroU16::new(defaults::nexus::da::audit::SAMPLE_SIZE)
                .expect("default audit sample size > 0"),
            window_count: NonZeroU16::new(defaults::nexus::da::audit::WINDOW_COUNT)
                .expect("default audit window count > 0"),
            interval: defaults::nexus::da::audit::INTERVAL,
        }
    }
}

/// Recovery deadline configuration for missing DA proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaRecovery {
    /// Deadline for providing recovery proofs once requested.
    pub request_timeout: Duration,
}

impl Default for DaRecovery {
    fn default() -> Self {
        Self {
            request_timeout: defaults::nexus::da::recovery::REQUEST_TIMEOUT,
        }
    }
}

/// Temporal diversity / attester rotation configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct DaRotation {
    /// Maximum appearances of an attester inside the rolling window.
    pub max_hits_per_window: NonZeroU16,
    /// Rolling window length (slots) for temporal diversity enforcement.
    pub window_slots: NonZeroU16,
    /// Domain-separation tag for deterministic rotation seed derivation.
    pub seed_tag: String,
    /// Latency-bias decay factor applied to attester weights.
    pub latency_decay: f64,
}

impl Default for DaRotation {
    fn default() -> Self {
        Self {
            max_hits_per_window: NonZeroU16::new(
                defaults::nexus::da::rotation::MAX_HITS_PER_WINDOW,
            )
            .expect("default rotation max hits > 0"),
            window_slots: NonZeroU16::new(defaults::nexus::da::rotation::WINDOW_SLOTS)
                .expect("default rotation window slots > 0"),
            seed_tag: defaults::nexus::da::rotation::SEED_TAG.to_string(),
            latency_decay: defaults::nexus::da::rotation::LATENCY_DECAY,
        }
    }
}

impl Fusion {
    /// Determine whether recent TEU demand satisfies the fusion criteria.
    #[must_use]
    pub fn should_fuse(&self, recent_teu: &[u64]) -> bool {
        let window = usize::from(self.observation_slots.get());
        if recent_teu.len() < window {
            return false;
        }
        recent_teu
            .iter()
            .rev()
            .take(window)
            .all(|&teu| teu <= u64::from(self.floor_teu))
    }

    /// Determine whether current TEU demand exceeds the split threshold.
    #[must_use]
    pub fn should_exit(&self, current_teu: u64) -> bool {
        current_teu > u64::from(self.exit_teu)
    }

    /// Maximum fused-window duration in slots before re-evaluating load.
    #[must_use]
    pub fn max_window_slots(&self) -> u16 {
        self.max_window_slots.get()
    }
}

impl Da {
    /// Maximum number of public dataspaces that can be served per slot under baseline sampling.
    #[must_use]
    pub fn max_public_dataspaces_per_slot(&self) -> u32 {
        let per_ds = u32::from(self.q_in_slot_per_ds_min.get()).max(1);
        self.q_in_slot_total.get() / per_ds
    }
}

impl DaRotation {
    /// Check if the observed attester occurrences within a window violate the configured cap.
    #[must_use]
    pub fn violates_temporal_diversity(
        &self,
        occurrences_within_window: u32,
        window_span_slots: u32,
    ) -> bool {
        let cap = u32::from(self.max_hits_per_window.get());
        let window_cap = u32::from(self.window_slots.get());
        occurrences_within_window > cap && window_span_slots >= window_cap
    }

    /// Expose the configured temporal window length (slots).
    #[must_use]
    pub fn window_slots(&self) -> u16 {
        self.window_slots.get()
    }

    /// Expose the configured hit cap within the temporal window.
    #[must_use]
    pub fn max_hits_per_window(&self) -> u16 {
        self.max_hits_per_window.get()
    }
}

/// Pipeline settings controlling execution behavior.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Pipeline {
    /// Enable dynamic prepass for IVM access-set derivation.
    pub dynamic_prepass: bool,
    /// Cache derived access sets for IVM manifests (advisory only).
    pub access_set_cache_enabled: bool,
    /// Enable parallel per-transaction overlay construction.
    pub parallel_overlay: bool,
    /// Number of worker threads for overlay construction (0 = auto).
    pub workers: usize,
    /// Enable parallel application of overlays (per conflict-free layer).
    pub parallel_apply: bool,
    /// Use BinaryHeap-based ready queue in the deterministic scheduler.
    /// Default is per-wave sort; this switch is for benchmarking and development.
    pub ready_queue_heap: bool,
    /// Optional GPU key bucketing (stable radix on (key, tx_idx, rw_flag)) for scheduler prepass.
    /// Deterministic CPU fallback is always available. Off by default.
    pub gpu_key_bucket: bool,
    /// Emit scheduler input/output traces for deterministic tie-break debugging.
    pub debug_trace_scheduler_inputs: bool,
    /// Emit transaction evaluation traces during overlay application (developer diagnostics).
    pub debug_trace_tx_eval: bool,
    /// Maximum size for deterministic signature micro-batches (0 disables; historical alias for Ed25519).
    pub signature_batch_max: usize,
    /// Per-scheme caps (0 disables) for signature batch verification.
    pub signature_batch_max_ed25519: usize,
    /// Maximum batch size for secp256k1 signatures (0 disables).
    pub signature_batch_max_secp256k1: usize,
    /// Maximum batch size for PQC signatures (0 disables).
    pub signature_batch_max_pqc: usize,
    /// Maximum batch size for BLS signatures (0 disables).
    pub signature_batch_max_bls: usize,
    /// IVM pre-decode cache capacity (decoded streams).
    pub cache_size: usize,
    /// Maximum decoded instructions retained per cached entry (0 = unlimited).
    pub ivm_cache_max_decoded_ops: usize,
    /// Approximate byte budget for cached pre-decode entries (bytes).
    pub ivm_cache_max_bytes: usize,
    /// Rayon worker cap for prover/trace verification (0 = physical cores).
    pub ivm_prover_threads: usize,
    /// Maximum instructions allowed per overlay (0 = unlimited).
    pub overlay_max_instructions: usize,
    /// Maximum serialized Norito bytes allowed per overlay (0 = unlimited).
    pub overlay_max_bytes: u64,
    /// Execute overlay instructions in chunks of this size (at least 1).
    pub overlay_chunk_instructions: usize,
    /// Gas fees configuration.
    pub gas: Gas,
    /// Admission-time ceiling for `ProgramMetadata.max_cycles` (0 disables the check).
    pub ivm_max_cycles_upper_bound: u64,
    /// Maximum decoded Kotodama instructions accepted during admission (0 = unlimited).
    pub ivm_max_decoded_instructions: u64,
    /// Maximum decoded Kotodama byte length accepted during admission (0 = unlimited).
    pub ivm_max_decoded_bytes: u64,
    /// Maximum transactions processed by the quarantine lane per block (0 = disabled).
    pub quarantine_max_txs_per_block: usize,
    /// Per-transaction cycle cap for quarantine lane (0 = unlimited).
    pub quarantine_tx_max_cycles: u64,
    /// Per-transaction wall-clock budget for quarantine lane in milliseconds (0 = unlimited).
    pub quarantine_tx_max_millis: u64,
    /// Default cursor mode for server-facing query endpoints.
    pub query_default_cursor_mode: QueryCursorMode,
    /// Maximum fetch size for iterable queries executed inside the IVM.
    pub query_max_fetch_size: u64,
    /// Minimum gas units required to use stored cursor mode (0 = disabled).
    pub query_stored_min_gas_units: u64,
    /// AMX per-dataspace execution budget in milliseconds.
    pub amx_per_dataspace_budget_ms: u64,
    /// AMX group execution budget across dataspaces in milliseconds.
    pub amx_group_budget_ms: u64,
    /// Estimated nanoseconds per instruction used for AMX budgeting.
    pub amx_per_instruction_ns: u64,
    /// Estimated nanoseconds per memory access used for AMX budgeting.
    pub amx_per_memory_access_ns: u64,
    /// Estimated nanoseconds per syscall used for AMX budgeting.
    pub amx_per_syscall_ns: u64,
}

/// Tiered state backend settings controlling hot/cold storage behaviour.
#[derive(Debug, Clone)]
pub struct TieredState {
    /// Enable tiered snapshots.
    pub enabled: bool,
    /// Maximum number of keys to keep hot (0 = unlimited).
    pub hot_retained_keys: usize,
    /// Hot-tier byte budget based on deterministic in-memory WSV sizing (0 = unlimited).
    /// Grace retention may temporarily exceed this budget.
    pub hot_retained_bytes: Bytes<u64>,
    /// Minimum snapshots to retain newly hot entries before demotion (0 = disabled).
    pub hot_retained_grace_snapshots: u64,
    /// Optional on-disk root for cold shards.
    pub cold_store_root: Option<PathBuf>,
    /// Optional on-disk root for DA-backed cold shards.
    pub da_store_root: Option<PathBuf>,
    /// Number of snapshots to retain on disk (0 = keep all).
    pub max_snapshots: usize,
    /// Optional cold-tier byte budget across snapshots (0 = unlimited).
    pub max_cold_bytes: Bytes<u64>,
}

/// Economics configuration for oracle slashing and rewards.
#[derive(Debug, Clone)]
pub struct OracleEconomics {
    /// Asset used to pay oracle rewards.
    pub reward_asset: AssetDefinitionId,
    /// Account that funds oracle rewards.
    pub reward_pool: AccountId,
    /// Fixed reward amount for an inlier observation.
    pub reward_amount: Numeric,
    /// Asset debited when applying penalties.
    pub slash_asset: AssetDefinitionId,
    /// Account credited with collected penalties.
    pub slash_receiver: AccountId,
    /// Penalty applied to outlier observations.
    pub slash_outlier_amount: Numeric,
    /// Penalty applied when a provider reports an error.
    pub slash_error_amount: Numeric,
    /// Penalty applied when a provider misses a slot.
    pub slash_no_show_amount: Numeric,
    /// Asset staked as a bond when opening disputes.
    pub dispute_bond_asset: AssetDefinitionId,
    /// Bond amount required to open a dispute.
    pub dispute_bond_amount: Numeric,
    /// Reward paid to successful challengers.
    pub dispute_reward_amount: Numeric,
    /// Penalty charged for frivolous disputes.
    pub frivolous_slash_amount: Numeric,
}

/// Approval thresholds for classed oracle governance stages.
#[derive(Debug, Clone, Copy)]
pub struct OracleChangeThresholds {
    /// Votes required for low-class changes.
    pub low: NonZeroUsize,
    /// Votes required for medium-class changes.
    pub medium: NonZeroUsize,
    /// Votes required for high-class changes.
    pub high: NonZeroUsize,
}

/// Governance configuration for oracle change proposals.
#[derive(Debug, Clone, Copy)]
pub struct OracleGovernance {
    /// SLA for the intake stage (blocks).
    pub intake_sla_blocks: u64,
    /// SLA for the rules committee stage (blocks).
    pub rules_sla_blocks: u64,
    /// SLA for the COP review stage (blocks).
    pub cop_sla_blocks: u64,
    /// SLA for the technical audit stage (blocks).
    pub technical_sla_blocks: u64,
    /// SLA for the policy jury stage (blocks).
    pub policy_jury_sla_blocks: u64,
    /// SLA for the enactment stage (blocks).
    pub enact_sla_blocks: u64,
    /// Intake approvals required.
    pub intake_min_votes: NonZeroUsize,
    /// Rules committee approvals required.
    pub rules_min_votes: NonZeroUsize,
    /// COP approvals keyed by change class.
    pub cop_min_votes: OracleChangeThresholds,
    /// Technical audit approvals required.
    pub technical_min_votes: NonZeroUsize,
    /// Policy jury approvals keyed by change class.
    pub policy_jury_min_votes: OracleChangeThresholds,
}

/// Twitter binding attestation guardrails.
#[derive(Debug, Clone)]
pub struct OracleTwitterBinding {
    /// Feed identifier expected for twitter follow attestations.
    pub feed_id: Name,
    /// Pepper identifier that must match the keyed hash.
    pub pepper_id: String,
    /// Maximum allowed TTL for attestations (milliseconds).
    pub max_ttl_ms: u64,
    /// Minimum allowed TTL for attestations (milliseconds).
    pub min_ttl_ms: u64,
    /// Minimum spacing between updates for the same binding hash (milliseconds).
    pub min_update_spacing_ms: u64,
}

/// Oracle aggregation configuration.
#[derive(Clone)]
pub struct Oracle {
    /// Maximum number of feed events retained per feed (oldest entries are pruned).
    pub history_depth: NonZeroUsize,
    /// Economic settings for oracle slashing/rewards.
    pub economics: OracleEconomics,
    /// Governance settings for oracle change proposals.
    pub governance: OracleGovernance,
    /// Guardrails for twitter follow binding attestations.
    pub twitter_binding: OracleTwitterBinding,
}

impl fmt::Debug for Oracle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Oracle")
            .field("history_depth", &self.history_depth)
            .field("governance", &self.governance)
            .field("twitter_binding", &self.twitter_binding)
            .finish()
    }
}

/// Compute lane configuration used by the gateway and scheduler.
#[derive(Debug, Clone)]
pub struct Compute {
    /// Whether compute is enabled.
    pub enabled: bool,
    /// Namespaces allowed for compute routes.
    pub namespaces: BTreeSet<Name>,
    /// Default TTL applied to calls (slots).
    pub default_ttl_slots: NonZeroU64,
    /// Maximum TTL accepted for calls (slots).
    pub max_ttl_slots: NonZeroU64,
    /// Maximum request payload size (bytes).
    pub max_request_bytes: Bytes<u64>,
    /// Maximum response payload size (bytes).
    pub max_response_bytes: Bytes<u64>,
    /// Per-call gas cap.
    pub max_gas_per_call: NonZeroU64,
    /// Resource profiles advertised by the node.
    pub resource_profiles: BTreeMap<Name, ComputeResourceBudget>,
    /// Default resource profile name.
    pub default_resource_profile: Name,
    /// Price families mapping cycles/egress into compute units.
    pub price_families: BTreeMap<Name, ComputePriceWeights>,
    /// Default price family identifier.
    pub default_price_family: Name,
    /// Authentication policy enforced when routes omit an override.
    pub auth_policy: ComputeAuthPolicy,
    /// Sandbox rules applied to compute execution.
    pub sandbox: ComputeSandboxRules,
    /// Economic settings for pricing, sponsorship, and governance bounds.
    pub economics: ComputeEconomics,
    /// Service-level objectives enforced by the gateway/scheduler.
    pub slo: ComputeSlo,
}

/// Content lane configuration.
#[derive(Debug, Clone)]
pub struct Content {
    /// Maximum tarball size accepted for a bundle (bytes).
    pub max_bundle_bytes: u64,
    /// Maximum number of files allowed in an archive.
    pub max_files: u32,
    /// Maximum allowed path length per entry.
    pub max_path_len: u32,
    /// Maximum retention window (blocks) for expiring bundles.
    pub max_retention_blocks: u64,
    /// Chunk size (bytes) used during ingestion.
    pub chunk_size_bytes: u32,
    /// Optional allow-list of accounts permitted to publish bundles.
    pub publish_allow_accounts: Vec<iroha_data_model::account::AccountId>,
    /// Rate/bandwidth limits for the content gateway.
    pub limits: ContentLimits,
    /// Default Cache-Control max-age (seconds) applied to bundles.
    pub default_cache_max_age_secs: u32,
    /// Upper bound for Cache-Control max-age (seconds).
    pub max_cache_max_age_secs: u32,
    /// Whether bundles are immutable by default.
    pub immutable_bundles: bool,
    /// Default read auth mode for bundles that omit an override.
    pub default_auth_mode: ContentAuthMode,
    /// Service-level objectives for the content gateway.
    pub slo: ContentSlo,
    /// Proof-of-work guard configuration for content fetches.
    pub pow: ContentPow,
    /// Default DA stripe layout applied to content bundles.
    pub stripe_layout: DaStripeLayout,
}

/// Content gateway SLO targets and rate limits.
#[derive(Debug, Clone, Copy)]
pub struct ContentSlo {
    /// Target p50 latency (milliseconds) for content responses.
    pub target_p50_latency_ms: NonZeroU32,
    /// Target p99 latency (milliseconds) for content responses.
    pub target_p99_latency_ms: NonZeroU32,
    /// Target availability (basis points) for content responses.
    pub target_availability_bps: NonZeroU32,
}

/// Rate/bandwidth limits for the content gateway.
#[derive(Debug, Clone, Copy)]
pub struct ContentLimits {
    /// Maximum requests per second accepted by the gateway.
    pub max_requests_per_second: NonZeroU32,
    /// Burst allowance for request tokens.
    pub request_burst: NonZeroU32,
    /// Maximum egress bytes per second served by the gateway.
    pub max_egress_bytes_per_second: NonZeroU64,
    /// Burst allowance for egress bytes.
    pub egress_burst_bytes: NonZeroU64,
}

/// Proof-of-work guard configuration for content fetches.
#[derive(Debug, Clone)]
pub struct ContentPow {
    /// Difficulty in leading zero bits (0 disables PoW).
    pub difficulty_bits: u8,
    /// Header name expected to carry PoW tokens.
    pub header_name: String,
}

impl Compute {
    /// Apply a governance price-family update with bounds enforcement.
    pub fn apply_price_update(
        &mut self,
        family: &Name,
        new_weights: ComputePriceWeights,
    ) -> core::result::Result<(), ComputeGovernanceError> {
        self.economics
            .apply_price_update(family, new_weights, &mut self.price_families)
    }
}

/// Compute SLO targets and caps.
#[derive(Debug, Clone, Copy)]
pub struct ComputeSlo {
    /// Maximum in-flight requests per route.
    pub max_inflight_per_route: NonZeroUsize,
    /// Maximum queued requests per route (beyond in-flight).
    pub queue_depth_per_route: NonZeroUsize,
    /// Maximum allowed requests per second (token-bucket).
    pub max_requests_per_second: NonZeroU32,
    /// Target p50 latency budget (milliseconds).
    pub target_p50_latency_ms: NonZeroU64,
    /// Target p95 latency budget (milliseconds).
    pub target_p95_latency_ms: NonZeroU64,
    /// Target p99 latency budget (milliseconds).
    pub target_p99_latency_ms: NonZeroU64,
}

/// Economic settings for compute pricing, sponsorship, and governance bounds.
#[derive(Debug, Clone)]
pub struct ComputeEconomics {
    /// Maximum compute units that may be charged per call.
    pub max_cu_per_call: NonZeroU64,
    /// Maximum amplification ratio (response/ingress) permitted for compute calls.
    pub max_amplification_ratio: NonZeroU32,
    /// Fee split across burn/validators/providers (basis points).
    pub fee_split: ComputeFeeSplit,
    /// Sponsor policy caps for subsidised calls.
    pub sponsor_policy: ComputeSponsorPolicy,
    /// Price delta bounds per risk class.
    pub price_bounds: BTreeMap<ComputePriceRiskClass, ComputePriceDeltaBounds>,
    /// Risk class mapping for price families.
    pub price_risk_classes: BTreeMap<Name, ComputePriceRiskClass>,
    /// Baseline price families used for governance delta calculations.
    pub price_family_baseline: BTreeMap<Name, ComputePriceWeights>,
    /// Multipliers applied for GPU/TEE/best-effort execution classes.
    pub price_amplifiers: ComputePriceAmplifiers,
}

impl ComputeEconomics {
    /// Apply a governance price update after validating bounds.
    pub fn apply_price_update(
        &self,
        family: &Name,
        new_weights: ComputePriceWeights,
        price_families: &mut BTreeMap<Name, ComputePriceWeights>,
    ) -> core::result::Result<(), ComputeGovernanceError> {
        let baseline = self.price_family_baseline.get(family).ok_or_else(|| {
            ComputeGovernanceError::UnknownPriceFamily {
                family: family.clone(),
            }
        })?;
        let risk_class = self.price_risk_classes.get(family).ok_or_else(|| {
            ComputeGovernanceError::MissingRiskClass {
                family: family.clone(),
            }
        })?;
        let bounds = self
            .price_bounds
            .get(risk_class)
            .ok_or_else(|| ComputeGovernanceError::MissingRiskBounds { class: *risk_class })?;

        if baseline.unit_label != new_weights.unit_label {
            return Err(ComputeGovernanceError::UnitLabelChanged {
                family: family.clone(),
                from: baseline.unit_label.clone(),
                to: new_weights.unit_label.clone(),
            });
        }

        let cycles_delta = delta_bps(new_weights.cycles_per_unit, baseline.cycles_per_unit);
        if cycles_delta > bounds.max_cycles_delta_bps.get() {
            return Err(ComputeGovernanceError::CyclesDeltaExceeded {
                family: family.clone(),
                delta_bps: cycles_delta,
                max_bps: bounds.max_cycles_delta_bps.get(),
            });
        }

        let egress_delta = delta_bps(
            new_weights.egress_bytes_per_unit,
            baseline.egress_bytes_per_unit,
        );
        if egress_delta > bounds.max_egress_delta_bps.get() {
            return Err(ComputeGovernanceError::EgressDeltaExceeded {
                family: family.clone(),
                delta_bps: egress_delta,
                max_bps: bounds.max_egress_delta_bps.get(),
            });
        }

        price_families.insert(family.clone(), new_weights);
        Ok(())
    }

    /// Validate that a requested sponsor allocation fits the configured caps.
    pub fn validate_sponsor_allocation(
        &self,
        requested_cu: u64,
    ) -> core::result::Result<(), ComputeGovernanceError> {
        self.validate_sponsor_allocation_with_usage(requested_cu, 0)
    }

    /// Validate sponsor allocation against per-call and daily caps.
    pub fn validate_sponsor_allocation_with_usage(
        &self,
        requested_cu: u64,
        consumed_today: u64,
    ) -> core::result::Result<(), ComputeGovernanceError> {
        let per_call_cap = self.sponsor_policy.max_cu_per_call.get();
        if requested_cu > per_call_cap {
            return Err(ComputeGovernanceError::SponsorCapExceeded {
                requested: requested_cu,
                limit: per_call_cap,
            });
        }
        let running = consumed_today.saturating_add(requested_cu);
        let daily_cap = self.sponsor_policy.max_daily_cu.get();
        if running > daily_cap {
            return Err(ComputeGovernanceError::SponsorCapExceeded {
                requested: running,
                limit: daily_cap,
            });
        }
        Ok(())
    }
}

fn delta_bps(value: NonZeroU64, baseline: NonZeroU64) -> u16 {
    let base = baseline.get() as u128;
    let value = value.get() as u128;
    let diff = if value > base {
        value - base
    } else {
        base - value
    };
    (diff.saturating_mul(10_000) / base) as u16
}

/// Cursor handling mode for server-facing iterable queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryCursorMode {
    /// Return only the first batch; do not store a server-side cursor.
    Ephemeral,
    /// Store a cursor in the LiveQueryStore and allow continuation.
    Stored,
}

/// Gas fees configuration: accepted assets, conversion mapping, and tech account.
#[derive(Debug, Clone)]
pub struct Gas {
    /// System-owned technical account that receives gas fee transfers.
    pub tech_account_id: String,
    /// Allowlist of accepted gas asset IDs.
    pub accepted_assets: Vec<String>,
    /// Deterministic conversion mapping (asset minimal units per one gas unit).
    #[allow(clippy::struct_field_names)]
    pub units_per_gas: Vec<GasRate>,
}

/// Governance-defined liquidity tiers for gas settlement routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GasLiquidity {
    /// Deep pools with negligible slippage.
    Tier1,
    /// Mid-depth pools with moderate slippage.
    #[default]
    Tier2,
    /// Thin pools or credit-constrained venues.
    Tier3,
}

impl FromStr for GasLiquidity {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tier1" | "tier1-deep" | "deep" => Ok(Self::Tier1),
            "tier2" | "tier2-medium" | "medium" => Ok(Self::Tier2),
            "tier3" | "tier3-thin" | "thin" => Ok(Self::Tier3),
            _ => Err(()),
        }
    }
}

/// Rolling-volatility classification for gas settlement routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GasVolatility {
    /// Normal trading conditions.
    #[default]
    Stable,
    /// Elevated but healthy volatility.
    Elevated,
    /// Dislocated markets requiring maximal margin.
    Dislocated,
}

impl FromStr for GasVolatility {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stable" | "calm" => Ok(Self::Stable),
            "elevated" | "stress" => Ok(Self::Elevated),
            "dislocated" | "extreme" => Ok(Self::Dislocated),
            _ => Err(()),
        }
    }
}

/// Deterministic gas conversion entry mapping an accepted asset to units per gas.
#[derive(Debug, Clone)]
pub struct GasRate {
    /// Asset ID string
    pub asset: String,
    /// Minimal units of the asset per one gas unit
    pub units_per_gas: u64,
    /// Time-weighted price of the gas asset denominated in local units per XOR.
    pub twap_local_per_xor: Decimal,
    /// Liquidity profile used to derive haircut tiers for the conversion path.
    pub liquidity: GasLiquidity,
    /// Volatility bucket derived from oracle inputs.
    pub volatility: GasVolatility,
}

/// Block storage (Kura) configuration.
#[derive(Debug, Clone)]
pub struct Kura {
    /// Initialization mode for block storage.
    pub init_mode: InitMode,
    /// Directory path for on-disk storage.
    pub store_dir: WithOrigin<PathBuf>,
    /// Maximum on-disk footprint for Kura (bytes, 0 = unlimited).
    pub max_disk_usage_bytes: Bytes<u64>,
    /// Number of recent blocks kept in memory.
    pub blocks_in_memory: NonZeroUsize,
    /// Number of recent roster records retained for block-sync validation.
    pub block_sync_roster_retention: NonZeroUsize,
    /// Number of recent roster sidecars retained alongside the block store.
    pub roster_sidecar_retention: NonZeroUsize,
    /// Whether to append new blocks as JSONL to `blocks.jsonl` under the active Kura lane.
    pub debug_output_new_blocks: bool,
    /// Maximum merge-ledger entries cached in memory (0 = default).
    pub merge_ledger_cache_capacity: usize,
    /// Fsync policy for block persistence.
    pub fsync_mode: FsyncMode,
    /// Interval used when batching fsync calls.
    pub fsync_interval: Duration,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            transaction_time_to_live: defaults::queue::TRANSACTION_TIME_TO_LIVE,
            capacity: defaults::queue::CAPACITY,
            capacity_per_user: defaults::queue::CAPACITY_PER_USER,
            expired_cull_interval: defaults::queue::EXPIRED_CULL_INTERVAL,
        }
    }
}

/// Node role in consensus participation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NodeRole {
    /// Full validator: proposes, votes, and finalizes blocks.
    Validator,
    /// Observer/sync-only: does not propose or vote; syncs blocks and serves queries.
    Observer,
}

/// Validator activation policy for a lane.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LaneValidatorMode {
    /// Validators are elected through the staking surface.
    StakeElected,
    /// Validators are administered directly without staking.
    AdminManaged,
}

impl LaneValidatorMode {
    /// Returns the canonical string representation for logs/telemetry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StakeElected => "stake_elected",
            Self::AdminManaged => "admin_managed",
        }
    }
}

/// Runtime consensus mode selection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConsensusMode {
    /// Classic permissioned Sumeragi flow
    Permissioned,
    /// Nominated Proof-of-Stake (NPoS) mode (consensus path)
    Npos,
}

/// Proof policy for consensus validity/finality.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProofPolicy {
    /// No additional proof gating beyond the standard single-chain QC.
    Off,
    /// Require zk parent-proving with depth `zk_finality_k`.
    ZkParent,
}

/// Adaptive observability configuration used to auto-tune consensus when telemetry detects slowness.
#[derive(Debug, Clone, Copy)]
pub struct AdaptiveObservability {
    /// Enable adaptive mitigation of collector fan-out and pacemaker intervals.
    pub enabled: bool,
    /// QC latency threshold (ms) that triggers mitigation.
    pub qc_latency_alert_ms: u64,
    /// Minimum missing-availability burst that triggers mitigation.
    pub da_reschedule_burst: u64,
    /// Additional pacemaker interval (ms) applied during mitigation.
    pub pacemaker_extra_ms: u64,
    /// Redundant collector fan-out cap during mitigation.
    pub collector_redundant_r: u8,
    /// Cooldown window (ms) before mitigation can re-trigger or reset.
    pub cooldown_ms: u64,
}

impl Default for AdaptiveObservability {
    fn default() -> Self {
        Self {
            enabled: defaults::sumeragi::ADAPTIVE_OBSERVABILITY_ENABLED,
            qc_latency_alert_ms: defaults::sumeragi::ADAPTIVE_QC_LATENCY_ALERT_MS,
            da_reschedule_burst: defaults::sumeragi::ADAPTIVE_DA_RESCHEDULE_BURST,
            pacemaker_extra_ms: defaults::sumeragi::ADAPTIVE_PACEMAKER_EXTRA_MS,
            collector_redundant_r: defaults::sumeragi::ADAPTIVE_COLLECTOR_REDUNDANT_R,
            cooldown_ms: defaults::sumeragi::ADAPTIVE_COOLDOWN_MS,
        }
    }
}

/// Consensus (Sumeragi) configuration.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Sumeragi {
    /// If true, force a soft fork for debugging purposes.
    pub debug_force_soft_fork: bool,
    /// Disable the dedicated consensus background post worker; actor sends inline instead (testing).
    pub debug_disable_background_worker: bool,
    /// Drop every Nth RBC chunk when acting as leader (adversarial testing).
    pub debug_rbc_drop_every_nth_chunk: Option<NonZeroU32>,
    /// Shuffle RBC chunk send order when broadcasting payloads (adversarial testing).
    pub debug_rbc_shuffle_chunks: bool,
    /// Broadcast a duplicate RBC init/chunk set for the next view (adversarial testing).
    pub debug_rbc_duplicate_inits: bool,
    /// Force RBC DELIVER quorum to 1 (adversarial testing).
    pub debug_rbc_force_deliver_quorum_one: bool,
    /// Corrupt witness availability acknowledgements emitted by this node (adversarial testing).
    pub debug_rbc_corrupt_witness_ack: bool,
    /// Corrupt RBC READY signatures emitted by this node (adversarial testing).
    pub debug_rbc_corrupt_ready_signature: bool,
    /// Bitmap of validator indexes (0-based) that should never receive RBC chunks from this node.
    /// Intended for deterministic adversarial tests; limited to the lower 64 validators.
    pub debug_rbc_drop_validator_mask: u64,
    /// Bitmap of chunk indexes (0-based) that should be equivocated when sent to targeted validators.
    /// Only the lower 64 chunk indexes are addressable; higher bits must remain zero.
    pub debug_rbc_equivocate_chunk_mask: u64,
    /// Bitmap of validator indexes (0-based) that should receive equivocated chunk payloads.
    /// Combined with `debug_rbc_equivocate_chunk_mask` to target specific peers/chunks.
    pub debug_rbc_equivocate_validator_mask: u64,
    /// Bitmap of validator indexes that should emit a conflicting RBC READY signature
    /// for adversarial testing. Limited to lower 64 validators.
    pub debug_rbc_conflicting_ready_mask: u64,
    /// Bitmap of chunk indexes withheld entirely from broadcast to simulate partial erasure.
    /// Limited to lower 64 chunk indexes; higher bits must remain zero.
    pub debug_rbc_partial_chunk_mask: u64,
    /// Node participation role (validator or observer).
    pub role: NodeRole,
    /// Allow SetBValidator votes in view 0 after local deadline widening.
    pub allow_view0_slack: bool,
    /// Number of collectors (K) used by validators and committers.
    pub collectors_k: usize,
    /// Redundant send fanout (r): number of distinct collectors a validator
    /// will target over time upon local timeouts.
    pub collectors_redundant_send_r: u8,
    /// Optional cap on transactions included in a single block (None = unlimited).
    pub block_max_transactions: Option<NonZeroUsize>,
    /// Optional cap on block payload bytes when RBC is disabled (None = unlimited).
    pub block_max_payload_bytes: Option<NonZeroUsize>,
    /// Multiplier applied to the proposal queue scan budget (relative to max tx per block).
    pub proposal_queue_scan_multiplier: NonZeroUsize,
    /// Capacity for the vote message channel.
    pub msg_channel_cap_votes: usize,
    /// Capacity for the block payload channel.
    pub msg_channel_cap_block_payload: usize,
    /// Capacity for the RBC chunk channel.
    pub msg_channel_cap_rbc_chunks: usize,
    /// Capacity for the block message channel (block sync updates, params, etc.).
    pub msg_channel_cap_blocks: usize,
    /// Capacity for Sumeragi control-message channel.
    pub control_msg_channel_cap: usize,
    /// Cap on the worker loop's per-iteration time budget.
    pub worker_iteration_budget_cap: Duration,
    /// Runtime consensus mode selection (permissioned vs npos).
    pub consensus_mode: ConsensusMode,
    /// Whether runtime consensus mode flips triggered by on-chain parameters are applied automatically.
    pub mode_flip_enabled: bool,
    /// Enable data availability for consensus (RBC + availability QC gating).
    pub da_enabled: bool,
    /// Multiplier for DA commit-quorum timeouts.
    pub da_quorum_timeout_multiplier: u32,
    /// Multiplier for availability timeouts in DA mode.
    pub da_availability_timeout_multiplier: u32,
    /// Floor for availability timeouts in DA mode.
    pub da_availability_timeout_floor: Duration,
    /// Interval between kura persistence retry attempts.
    pub kura_store_retry_interval: Duration,
    /// Maximum number of kura persistence retry attempts before aborting the block.
    pub kura_store_retry_max_attempts: u32,
    /// Timeout for inflight commit jobs before aborting.
    pub commit_inflight_timeout: Duration,
    /// Missing-block fetch attempts before falling back to the full commit topology.
    /// A value of 0 disables signer preference.
    pub missing_block_signer_fallback_attempts: u32,
    /// Consecutive membership mismatches required before alerting.
    pub membership_mismatch_alert_threshold: u32,
    /// Whether to drop consensus messages from peers with repeated membership mismatches.
    pub membership_mismatch_fail_closed: bool,
    /// Maximum height delta accepted for inbound consensus messages (0 disables future gating).
    pub consensus_future_height_window: u64,
    /// Maximum view delta accepted for inbound consensus messages (0 disables future gating).
    pub consensus_future_view_window: u64,
    /// Invalid signature count before temporarily suppressing a signer (0 disables).
    pub invalid_sig_penalty_threshold: u32,
    /// Window for invalid signature penalty counting.
    pub invalid_sig_penalty_window: Duration,
    /// Cooldown applied after invalid signature penalties trigger.
    pub invalid_sig_penalty_cooldown: Duration,
    /// Maximum DA commitments (blobs) permitted in a single block.
    pub da_max_commitments_per_block: usize,
    /// Maximum DA proof openings permitted in a single block (aggregate cap).
    pub da_max_proof_openings_per_block: usize,
    /// Proof policy selector (off/zk_parent).
    pub proof_policy: ProofPolicy,
    /// Cap for in-memory commit certificate history (used for status/finality proofs).
    pub commit_cert_history_cap: usize,
    /// Zk parent-proving depth (0 disables zk finality).
    pub zk_finality_k: u8,
    /// Require PrecommitQC for the candidate block before commit (consensus path).
    /// Defaults to false while the new voting path is being integrated.
    pub require_precommit_qc: bool,
    /// RBC chunk maximum bytes per chunk.
    pub rbc_chunk_max_bytes: usize,
    /// Optional fanout cap for RBC chunk broadcasts (None = auto based on topology).
    pub rbc_chunk_fanout: Option<NonZeroUsize>,
    /// Maximum pending RBC chunks stashed before INIT.
    pub rbc_pending_max_chunks: usize,
    /// Maximum pending RBC bytes per session before INIT.
    pub rbc_pending_max_bytes: usize,
    /// Maximum pending RBC sessions stashed before INIT.
    pub rbc_pending_session_limit: usize,
    /// TTL for pending RBC messages awaiting INIT.
    pub rbc_pending_ttl: Duration,
    /// RBC session TTL for pruning inactive sessions.
    pub rbc_session_ttl: Duration,
    /// Maximum RBC sessions rebroadcast per tick.
    pub rbc_rebroadcast_sessions_per_tick: usize,
    /// Maximum RBC payload chunks broadcast per tick.
    pub rbc_payload_chunks_per_tick: usize,
    /// Maximum number of persisted RBC session summaries retained on disk.
    pub rbc_store_max_sessions: usize,
    /// Soft quota for persisted RBC session summaries.
    pub rbc_store_soft_sessions: usize,
    /// Maximum total disk bytes allocated for persisted RBC session payloads.
    pub rbc_store_max_bytes: usize,
    /// Soft quota for persisted RBC session payload bytes.
    pub rbc_store_soft_bytes: usize,
    /// Disk-backed RBC chunk retention TTL.
    pub rbc_disk_store_ttl: Duration,
    /// Maximum bytes allocated for disk-backed RBC chunk persistence.
    pub rbc_disk_store_max_bytes: u64,
    /// Minimum lead time (blocks) between publishing a new consensus key and activation.
    pub key_activation_lead_blocks: u64,
    /// Overlap/grace window (blocks) permitting dual-signing during rotation.
    pub key_overlap_grace_blocks: u64,
    /// Expiry grace window (blocks) after declared expiry.
    pub key_expiry_grace_blocks: u64,
    /// Require HSM binding for consensus/committee keys.
    pub key_require_hsm: bool,
    /// Allowed algorithms for consensus/committee keys.
    pub key_allowed_algorithms: BTreeSet<Algorithm>,
    /// Allowed HSM providers for consensus/committee keys.
    pub key_allowed_hsm_providers: BTreeSet<String>,
    /// NPoS-specific consensus parameters.
    pub npos: SumeragiNpos,
    /// Use stake snapshot provider for epoch validator roster (NPoS)
    pub use_stake_snapshot_roster: bool,
    /// Epoch length in blocks (NPoS)
    pub epoch_length_blocks: u64,
    /// VRF commit deadline offset from epoch start (blocks)
    pub vrf_commit_deadline_offset: u64,
    /// VRF reveal deadline offset from epoch start (blocks)
    pub vrf_reveal_deadline_offset: u64,
    /// Pacemaker backoff multiplier for view-change increments (>=1)
    pub pacemaker_backoff_multiplier: u32,
    /// Pacemaker RTT floor multiplier (avg_rtt * multiplier)
    pub pacemaker_rtt_floor_multiplier: u32,
    /// Pacemaker maximum backoff cap
    pub pacemaker_max_backoff: Duration,
    /// Pacemaker jitter band (permille of window). 0 disables jitter.
    pub pacemaker_jitter_frac_permille: u32,
    /// Enable real BLS signing/verification for consensus votes (mandatory).
    pub enable_bls: bool,
    /// Adaptive observability/auto-mitigation knobs.
    pub adaptive_observability: AdaptiveObservability,
}

/// NPoS configuration bundle.
#[derive(Debug, Clone, Copy)]
pub struct SumeragiNpos {
    /// Target block time.
    pub block_time: Duration,
    /// Pacemaker per-phase timeouts.
    pub timeouts: SumeragiNposTimeouts,
    /// Pacemaker backoff multiplier applied to RTT estimates.
    pub pacemaker_backoff_multiplier: u32,
    /// Pacemaker RTT floor multiplier applied to RTT estimates.
    pub pacemaker_rtt_floor_multiplier: u32,
    /// Maximum pacemaker backoff window.
    pub pacemaker_max_backoff: Duration,
    /// Jitter band size as a permille of the backoff window.
    pub pacemaker_jitter_frac_permille: u32,
    /// Number of aggregators per round.
    pub k_aggregators: usize,
    /// Redundant send fanout for validators.
    pub redundant_send_r: u8,
    /// VRF commit/reveal windows.
    pub vrf: SumeragiNposVrf,
    /// Election policy knobs.
    pub election: SumeragiNposElection,
    /// Reconfiguration pipeline knobs.
    pub reconfig: SumeragiNposReconfig,
}

/// Pacemaker timeout durations.
#[derive(Debug, Clone, Copy)]
pub struct SumeragiNposTimeouts {
    /// Timeout allotted for the proposal broadcast stage.
    pub propose: Duration,
    /// Timeout allotted for prevote aggregation.
    pub prevote: Duration,
    /// Timeout allotted for precommit aggregation.
    pub precommit: Duration,
    /// Timeout allotted for execution QC aggregation.
    pub exec: Duration,
    /// Timeout allotted for witness availability QC aggregation.
    pub witness: Duration,
    /// Timeout allotted for final commit confirmation.
    pub commit: Duration,
    /// Timeout allotted for data-availability quorum formation.
    pub da: Duration,
    /// Timeout before validators widen redundant collector fanout.
    pub aggregator: Duration,
}

/// VRF window configuration.
#[derive(Debug, Clone, Copy)]
pub struct SumeragiNposVrf {
    /// Number of blocks from epoch start reserved for VRF commitments.
    pub commit_window_blocks: u64,
    /// Number of blocks after the commit window reserved for VRF reveals.
    pub reveal_window_blocks: u64,
}

/// Election policy configuration.
#[derive(Debug, Clone, Copy)]
pub struct SumeragiNposElection {
    /// Maximum validators allowed in an elected set (0 = unlimited).
    pub max_validators: u32,
    /// Minimum self-bond required for validator eligibility (stake units).
    pub min_self_bond: u64,
    /// Minimum nomination bond required for delegators (stake units).
    pub min_nomination_bond: u64,
    /// Maximum nomination share any single nominator may contribute (percentage 0-100).
    pub max_nominator_concentration_pct: u8,
    /// Acceptable variance band for seat allocation (percentage 0-100).
    pub seat_band_pct: u8,
    /// Maximum correlated ownership across validators (percentage 0-100).
    pub max_entity_correlation_pct: u8,
    /// Finality margin (blocks) required when activating a new set.
    pub finality_margin_blocks: u64,
}

/// Reconfiguration timing knobs.
#[derive(Debug, Clone, Copy)]
pub struct SumeragiNposReconfig {
    /// Number of blocks to retain governance evidence before pruning.
    pub evidence_horizon_blocks: u64,
    /// Activation lag in blocks before a newly scheduled validator set takes effect.
    pub activation_lag_blocks: u64,
    /// Slashing delay in blocks before evidence penalties apply.
    pub slashing_delay_blocks: u64,
}

impl Default for SumeragiNpos {
    fn default() -> Self {
        Self {
            block_time: Duration::from_millis(defaults::sumeragi::npos::BLOCK_TIME_MS),
            timeouts: SumeragiNposTimeouts::default(),
            pacemaker_backoff_multiplier: defaults::sumeragi::PACEMAKER_BACKOFF_MULTIPLIER,
            pacemaker_rtt_floor_multiplier: defaults::sumeragi::PACEMAKER_RTT_FLOOR_MULTIPLIER,
            pacemaker_max_backoff: Duration::from_millis(
                defaults::sumeragi::PACEMAKER_MAX_BACKOFF_MS,
            ),
            pacemaker_jitter_frac_permille: defaults::sumeragi::PACEMAKER_JITTER_FRAC_PERMILLE,
            k_aggregators: defaults::sumeragi::npos::K_AGGREGATORS,
            redundant_send_r: defaults::sumeragi::npos::REDUNDANT_SEND_R,
            vrf: SumeragiNposVrf::default(),
            election: SumeragiNposElection::default(),
            reconfig: SumeragiNposReconfig::default(),
        }
    }
}

impl Default for SumeragiNposTimeouts {
    fn default() -> Self {
        Self {
            propose: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_PROPOSE_MS),
            prevote: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_PREVOTE_MS),
            precommit: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_PRECOMMIT_MS),
            exec: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_EXEC_MS),
            witness: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_WITNESS_MS),
            commit: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_COMMIT_MS),
            da: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_DA_MS),
            aggregator: Duration::from_millis(defaults::sumeragi::npos::TIMEOUT_AGG_MS),
        }
    }
}

impl Default for SumeragiNposVrf {
    fn default() -> Self {
        Self {
            commit_window_blocks: defaults::sumeragi::npos::VRF_COMMIT_WINDOW_BLOCKS,
            reveal_window_blocks: defaults::sumeragi::npos::VRF_REVEAL_WINDOW_BLOCKS,
        }
    }
}

impl Default for SumeragiNposElection {
    fn default() -> Self {
        Self {
            max_validators: defaults::sumeragi::npos::MAX_VALIDATORS,
            min_self_bond: defaults::sumeragi::npos::MIN_SELF_BOND,
            min_nomination_bond: defaults::sumeragi::npos::MIN_NOMINATION_BOND,
            max_nominator_concentration_pct:
                defaults::sumeragi::npos::MAX_NOMINATOR_CONCENTRATION_PCT,
            seat_band_pct: defaults::sumeragi::npos::SEAT_BAND_PCT,
            max_entity_correlation_pct: defaults::sumeragi::npos::MAX_ENTITY_CORRELATION_PCT,
            finality_margin_blocks: defaults::sumeragi::npos::FINALITY_MARGIN_BLOCKS,
        }
    }
}

impl Default for SumeragiNposReconfig {
    fn default() -> Self {
        Self {
            evidence_horizon_blocks: defaults::sumeragi::npos::RECONFIG_EVIDENCE_HORIZON_BLOCKS,
            activation_lag_blocks: defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS,
            slashing_delay_blocks: defaults::sumeragi::npos::SLASHING_DELAY_BLOCKS,
        }
    }
}

/// Trusted peers configuration: the local peer and its peers.
#[derive(Debug, Clone)]
pub struct TrustedPeers {
    /// Local peer description.
    pub myself: Peer,
    /// Other trusted peers.
    pub others: UniqueVec<Peer>,
    /// Proof-of-Possession (PoP) for validators' BLS keys, keyed by public key.
    /// PoP entries must cover the full validator roster at config parse time; incomplete
    /// or invalid PoP maps are rejected. Runtime roster derivation still guards against
    /// missing or invalid PoPs to avoid divergent topologies.
    pub pops: std::collections::BTreeMap<PublicKey, Vec<u8>>,
}

impl TrustedPeers {
    /// Returns a list of trusted peers which is guaranteed to have at
    /// least one element - the id of the peer itself.
    pub fn into_non_empty_vec(self) -> UniqueVec<PeerId> {
        std::iter::once(self.myself)
            .chain(self.others)
            .map(|peer| peer.id().clone())
            .collect()
    }

    /// Tells whether a trusted peers list has some other peers except for the peer itself
    pub fn contains_other_trusted_peers(&self) -> bool {
        !self.others.is_empty()
    }
}

/// Live query store configuration.
#[derive(Debug, Clone, Copy)]
pub struct LiveQueryStore {
    /// Idle time before a live query is evicted.
    pub idle_time: Duration,
    /// Maximum number of live queries.
    pub capacity: NonZeroUsize,
    /// Per-user live query limit.
    pub capacity_per_user: NonZeroUsize,
}

impl Default for LiveQueryStore {
    fn default() -> Self {
        Self {
            idle_time: defaults::torii::QUERY_IDLE_TIME,
            capacity: defaults::torii::QUERY_STORE_CAPACITY,
            capacity_per_user: defaults::torii::QUERY_STORE_CAPACITY_PER_USER,
        }
    }
}

/// Block synchronization parameters.
#[derive(Debug, Clone, Copy)]
pub struct BlockSync {
    /// Block gossip interval.
    pub gossip_period: Duration,
    /// Maximum block gossip interval (idle backoff ceiling).
    pub gossip_max_period: Duration,
    /// Fanout cap for block-sync gossip (peer samples, block sync updates, availability votes, and NEW_VIEW gossip).
    pub gossip_size: NonZeroU32,
}

/// Dataspace-aware transaction gossip targeting policy.
#[derive(Debug, Clone, Copy)]
pub struct DataspaceGossip {
    /// Drop gossip for unknown dataspaces instead of falling back to restricted routing.
    pub drop_unknown_dataspace: bool,
    /// Optional cap on the number of peers targeted for restricted gossip (None = commit topology).
    pub restricted_target_cap: Option<NonZeroUsize>,
    /// Optional cap on the number of peers targeted for public gossip (None = broadcast).
    pub public_target_cap: Option<NonZeroUsize>,
    /// Interval between reshuffles of public gossip target selection.
    pub public_target_reshuffle: Duration,
    /// Interval between reshuffles of restricted gossip target selection.
    pub restricted_target_reshuffle: Duration,
    /// Fallback policy when restricted targets are unavailable.
    pub restricted_fallback: DataspaceGossipFallback,
    /// Policy for restricted payloads when only the public overlay is available.
    pub restricted_public_payload: RestrictedPublicPayload,
}

/// Fallback behaviour when restricted routing cannot determine targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataspaceGossipFallback {
    /// Drop the batch and retry later.
    Drop,
    /// Use the public overlay targets when commit topology is unavailable.
    UsePublicOverlay,
}

impl Default for DataspaceGossipFallback {
    fn default() -> Self {
        match defaults::network::TX_GOSSIP_RESTRICTED_FALLBACK {
            "public_overlay" => Self::UsePublicOverlay,
            _ => Self::Drop,
        }
    }
}

/// Action to take when restricted gossip can only target the public overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestrictedPublicPayload {
    /// Refuse to leak the payload onto the public overlay.
    Refuse,
    /// Forward to the public overlay (assumes operators provision their own payload protection).
    Forward,
}

impl Default for RestrictedPublicPayload {
    fn default() -> Self {
        match defaults::network::TX_GOSSIP_RESTRICTED_PUBLIC_PAYLOAD {
            "forward" => Self::Forward,
            _ => Self::Refuse,
        }
    }
}

impl Default for DataspaceGossip {
    fn default() -> Self {
        Self {
            drop_unknown_dataspace: defaults::network::TX_GOSSIP_DROP_UNKNOWN_DATASPACE,
            restricted_target_cap: defaults::network::TX_GOSSIP_RESTRICTED_TARGET_CAP,
            public_target_cap: defaults::network::TX_GOSSIP_PUBLIC_TARGET_CAP,
            public_target_reshuffle: defaults::network::TX_GOSSIP_PUBLIC_TARGET_RESHUFFLE,
            restricted_target_reshuffle: defaults::network::TX_GOSSIP_RESTRICTED_TARGET_RESHUFFLE,
            restricted_fallback: DataspaceGossipFallback::default(),
            restricted_public_payload: RestrictedPublicPayload::default(),
        }
    }
}

/// Transaction gossiping parameters.
#[derive(Debug, Clone, Copy)]
pub struct TransactionGossiper {
    /// Transaction gossip interval.
    pub gossip_period: Duration,
    /// Number of transactions per gossip message.
    pub gossip_size: NonZeroU32,
    /// Number of gossip periods to wait before re-sending the same transactions.
    pub gossip_resend_ticks: NonZeroU32,
    /// Dataspace-aware targeting options.
    pub dataspace: DataspaceGossip,
}

/// Proof endpoint DoS/backpressure policy.
#[derive(Debug, Clone, Copy)]
pub struct ProofApi {
    /// Rolling-window rate (requests per minute). None disables limiting.
    pub rate_per_minute: Option<NonZeroU32>,
    /// Burst tokens allowed within the window.
    pub burst: Option<NonZeroU32>,
    /// Maximum accepted proof request payload size.
    pub max_body_bytes: Bytes<u64>,
    /// Egress budget for proof responses (bytes/sec). None disables.
    pub egress_bytes_per_sec: Option<NonZeroU64>,
    /// Burst budget for proof responses (bytes).
    pub egress_burst_bytes: Option<NonZeroU64>,
    /// Maximum page size accepted by proof listings.
    pub max_list_limit: NonZeroU32,
    /// Wall-clock timeout applied to proof list/count handlers.
    pub request_timeout: Duration,
    /// Cache lifetime advertised for proof lookups.
    pub cache_max_age: Duration,
    /// Retry hint surfaced on throttling responses.
    pub retry_after: Duration,
}

/// Limits for app-facing list/query endpoints.
#[derive(Debug, Clone, Copy)]
pub struct AppApi {
    /// Default page size applied when clients omit `limit`.
    pub default_list_limit: NonZeroU32,
    /// Maximum page size accepted by app-facing list/query endpoints.
    pub max_list_limit: NonZeroU32,
    /// Maximum fetch size accepted by app-facing iterable queries.
    pub max_fetch_size: NonZeroU32,
    /// Rate-limiter cost applied per requested row when backpressure is enforced.
    pub rate_limit_cost_per_row: NonZeroU32,
}

/// Webhook delivery/backpressure configuration.
#[derive(Debug, Clone, Copy)]
pub struct Webhook {
    /// Maximum pending webhook deliveries persisted on disk.
    pub queue_capacity: NonZeroUsize,
    /// Maximum delivery attempts before a payload is dropped.
    pub max_attempts: NonZeroU32,
    /// Initial backoff delay applied to webhook retries.
    pub backoff_initial: Duration,
    /// Maximum backoff delay applied to webhook retries.
    pub backoff_max: Duration,
    /// HTTP connect timeout for webhook delivery.
    pub connect_timeout: Duration,
    /// HTTP write timeout for webhook delivery.
    pub write_timeout: Duration,
    /// HTTP read timeout for webhook delivery.
    pub read_timeout: Duration,
}

impl Default for Webhook {
    fn default() -> Self {
        Self {
            queue_capacity: NonZeroUsize::new(defaults::torii::WEBHOOK_QUEUE_CAPACITY)
                .expect("default webhook queue capacity non-zero"),
            max_attempts: NonZeroU32::new(defaults::torii::WEBHOOK_MAX_ATTEMPTS)
                .expect("default webhook max attempts non-zero"),
            backoff_initial: Duration::from_millis(defaults::torii::WEBHOOK_BACKOFF_INITIAL_MS),
            backoff_max: Duration::from_millis(defaults::torii::WEBHOOK_BACKOFF_MAX_MS),
            connect_timeout: Duration::from_millis(defaults::torii::WEBHOOK_CONNECT_TIMEOUT_MS),
            write_timeout: Duration::from_millis(defaults::torii::WEBHOOK_WRITE_TIMEOUT_MS),
            read_timeout: Duration::from_millis(defaults::torii::WEBHOOK_READ_TIMEOUT_MS),
        }
    }
}

/// Push notification delivery configuration (FCM/APNS).
#[derive(Debug, Clone)]
pub struct Push {
    /// Enable the push bridge (disabled by default).
    pub enabled: bool,
    /// Optional steady-state rate (requests/minute). None disables limiting.
    pub rate_per_minute: Option<NonZeroU32>,
    /// Optional burst tokens for push dispatch.
    pub burst: Option<NonZeroU32>,
    /// HTTP connect timeout used for push delivery.
    pub connect_timeout: Duration,
    /// HTTP request timeout used for push delivery.
    pub request_timeout: Duration,
    /// Maximum topics recorded per registered device.
    pub max_topics_per_device: NonZeroUsize,
    /// Optional FCM API key used for dispatch.
    pub fcm_api_key: Option<String>,
    /// Optional APNS endpoint base URL.
    pub apns_endpoint: Option<String>,
    /// Optional APNS auth token (e.g., JWT).
    pub apns_auth_token: Option<String>,
}

impl Default for Push {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::PUSH_ENABLED,
            rate_per_minute: defaults::torii::PUSH_RATE_PER_MINUTE
                .and_then(std::num::NonZeroU32::new),
            burst: defaults::torii::PUSH_BURST.and_then(std::num::NonZeroU32::new),
            connect_timeout: Duration::from_millis(defaults::torii::PUSH_CONNECT_TIMEOUT_MS),
            request_timeout: Duration::from_millis(defaults::torii::PUSH_REQUEST_TIMEOUT_MS),
            max_topics_per_device: NonZeroUsize::new(
                defaults::torii::PUSH_MAX_TOPICS_PER_DEVICE.max(1),
            )
            .expect("default push max topics non-zero"),
            fcm_api_key: None,
            apns_endpoint: None,
            apns_auth_token: None,
        }
    }
}

/// Torii API configuration.
#[derive(Debug, Clone)]
pub struct Torii {
    /// API listening address.
    pub address: WithOrigin<SocketAddr>,
    /// Supported Torii API versions (semantic `major.minor`, oldest → newest).
    pub api_versions: Vec<String>,
    /// Default API version assumed when the client omits the header.
    pub api_version_default: String,
    /// Minimum API version required for proof/staking/fee endpoints.
    pub api_min_proof_version: String,
    /// Optional unix timestamp when the oldest supported version sunsets.
    pub api_version_sunset_unix: Option<u64>,
    /// Maximum request body size.
    pub max_content_len: Bytes<u64>,
    /// Base directory for Torii persistence (attachments, webhooks, DA queues).
    pub data_dir: PathBuf,
    /// Optional per-authority query rate (tokens/sec). None disables limiting.
    pub query_rate_per_authority_per_sec: Option<NonZeroU32>,
    /// Optional per-authority burst capacity (tokens).
    pub query_burst_per_authority: Option<NonZeroU32>,
    /// Optional per-authority transaction submission rate (tokens/sec). None disables limiting.
    pub tx_rate_per_authority_per_sec: Option<NonZeroU32>,
    /// Optional per-authority transaction burst capacity (tokens).
    pub tx_burst_per_authority: Option<NonZeroU32>,
    /// Optional per-origin deploy rate (tokens/sec). None disables limiting.
    pub deploy_rate_per_origin_per_sec: Option<NonZeroU32>,
    /// Optional per-origin deploy burst capacity (tokens).
    pub deploy_burst_per_origin: Option<NonZeroU32>,
    /// Require a valid API token for app-facing endpoints.
    pub require_api_token: bool,
    /// Allowed API tokens (opaque strings). Empty means no tokens defined.
    pub api_tokens: Vec<String>,
    /// Optional fee policy: asset id (e.g., "rose#wonderland").
    pub api_fee_asset_id: Option<String>,
    /// Optional fee policy: fixed amount per request.
    pub api_fee_amount: Option<u64>,
    /// Optional fee policy: receiver account id (IH58 or `<alias|public_key>@domain`).
    pub api_fee_receiver: Option<String>,
    /// SoraNet privacy ingestion guard rails (auth/rate/namespace).
    pub soranet_privacy_ingest: SoranetPrivacyIngest,
    /// CIDR allowlist for bypassing API rate limits (IPv4/IPv6).
    pub api_allow_cidrs: Vec<String>,
    /// Optional Torii base URLs used to fetch peer telemetry metadata.
    pub peer_telemetry_urls: Vec<Url>,
    /// Peer telemetry geo lookup configuration.
    pub peer_geo: ToriiPeerGeo,
    /// Emit filter-match debug traces (developer diagnostics only).
    pub debug_match_filters: bool,
    /// Operator authentication policy for operator-facing endpoints.
    pub operator_auth: ToriiOperatorAuth,
    /// Maximum concurrent pre-auth connections (global).
    pub preauth_max_connections: Option<NonZeroUsize>,
    /// Maximum concurrent pre-auth connections per IP.
    pub preauth_max_connections_per_ip: Option<NonZeroUsize>,
    /// Pre-auth handshake rate per IP (tokens/sec).
    pub preauth_rate_per_ip_per_sec: Option<NonZeroU32>,
    /// Pre-auth handshake burst per IP (tokens).
    pub preauth_burst_per_ip: Option<NonZeroU32>,
    /// Optional temporary ban duration applied on repeated violations.
    pub preauth_temp_ban: Option<Duration>,
    /// CIDR allowlist for bypassing pre-auth limits.
    pub preauth_allow_cidrs: Vec<String>,
    /// Optional per-scheme pre-auth concurrency caps.
    pub preauth_scheme_limits: Vec<PreauthSchemeLimit>,
    /// Optional high-load threshold (queued txs) to enable rate limiting.
    /// When `None`, Torii uses an internal default.
    pub api_high_load_tx_threshold: Option<usize>,
    /// Optional high-load threshold for streaming endpoints (queued txs).
    /// When `None`, Torii uses a higher internal default.
    pub api_high_load_stream_threshold: Option<usize>,
    /// Optional high-load threshold for subscription WS endpoint specifically.
    /// When `None`, Torii uses the streaming default.
    pub api_high_load_subscription_threshold: Option<usize>,
    /// Capacity of the broadcast channel used for events/SSE/webhooks.
    pub events_buffer_capacity: NonZeroUsize,
    /// WebSocket message timeout for Torii event/block streams.
    pub ws_message_timeout: Duration,
    /// ZK attachments TTL (seconds) for app-facing attachments store.
    pub attachments_ttl_secs: u64,
    /// ZK attachments maximum allowed size per item (bytes).
    pub attachments_max_bytes: u64,
    /// Maximum number of attachments retained per tenant (0 = unlimited).
    pub attachments_per_tenant_max_count: u64,
    /// Maximum aggregate attachment bytes retained per tenant (0 = unlimited).
    pub attachments_per_tenant_max_bytes: u64,
    /// Allowed MIME types for attachment payloads (post-sniff).
    pub attachments_allowed_mime_types: Vec<String>,
    /// Maximum expanded bytes allowed when decompressing attachments.
    pub attachments_max_expanded_bytes: u64,
    /// Maximum nested archive depth allowed when decompressing attachments.
    pub attachments_max_archive_depth: u32,
    /// Execution mode for attachment sanitization.
    pub attachments_sanitizer_mode: AttachmentSanitizerMode,
    /// Attachment sanitization timeout (milliseconds).
    pub attachments_sanitize_timeout_ms: u64,
    /// Enable background ZK prover worker (non-consensus, app-facing only).
    pub zk_prover_enabled: bool,
    /// Scan period for the background prover worker (seconds).
    pub zk_prover_scan_period_secs: u64,
    /// Retention TTL for background ZK prover reports (seconds).
    pub zk_prover_reports_ttl_secs: u64,
    /// Maximum number of attachments processed concurrently by the prover worker.
    pub zk_prover_max_inflight: usize,
    /// Maximum aggregate attachment bytes processed per scan cycle.
    pub zk_prover_max_scan_bytes: u64,
    /// Maximum wall-clock time (milliseconds) spent in a single scan cycle.
    pub zk_prover_max_scan_millis: u64,
    /// Directory containing verifying key bytes for the background prover.
    pub zk_prover_keys_dir: PathBuf,
    /// Allowlisted backend prefixes for the background prover (empty = allow all).
    pub zk_prover_allowed_backends: Vec<String>,
    /// Allowlisted circuit identifiers for the background prover (empty = allow all).
    pub zk_prover_allowed_circuits: Vec<String>,
    /// Iroha Connect configuration.
    pub connect: Connect,
    /// ISO 20022 bridge configuration.
    pub iso_bridge: IsoBridge,
    /// RBC sampling endpoint configuration.
    pub rbc_sampling: RbcSampling,
    /// Data-availability ingest configuration.
    pub da_ingest: DaIngest,
    /// SoraFS discovery cache configuration.
    pub sorafs_discovery: SorafsDiscovery,
    /// Embedded SoraFS storage configuration.
    pub sorafs_storage: SorafsStorage,
    /// Repair scheduler configuration for SoraFS.
    pub sorafs_repair: SorafsRepair,
    /// GC scheduler configuration for SoraFS.
    pub sorafs_gc: SorafsGc,
    /// Quota configuration for SoraFS control-plane endpoints.
    pub sorafs_quota: SorafsQuota,
    /// Alias cache policy shared across gateways and SDKs.
    pub sorafs_alias_cache: SorafsAliasCachePolicy,
    /// Gateway policy and automation configuration for SoraFS delivery.
    pub sorafs_gateway: SorafsGateway,
    /// Proof-of-Retrievability coordinator configuration.
    pub sorafs_por: SorafsPor,
    /// Transport-specific configuration (Norito-RPC rollout, streaming knobs).
    pub transport: ToriiTransport,
    /// Proof endpoint DoS/backpressure policy.
    pub proof_api: ProofApi,
    /// Optional UAID onboarding authority configuration.
    pub onboarding: Option<ToriiOnboarding>,
    /// Optional offline certificate issuer configuration.
    pub offline_issuer: Option<ToriiOfflineIssuer>,
    /// App-facing query/backpressure limits.
    pub app_api: AppApi,
    /// Webhook delivery/backpressure configuration.
    pub webhook: Webhook,
    /// Push notification delivery configuration.
    pub push: Push,
}

/// Execution mode for attachment sanitization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentSanitizerMode {
    /// Run the sanitizer inside the Torii process.
    InProcess,
    /// Run the sanitizer in a dedicated subprocess.
    Subprocess,
}

impl AttachmentSanitizerMode {
    /// Render a stable label for config and telemetry.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::Subprocess => "subprocess",
        }
    }
}

/// Operator authentication configuration for Torii operator endpoints.
#[derive(Debug, Clone)]
pub struct ToriiOperatorAuth {
    /// Master enable switch for operator authentication.
    pub enabled: bool,
    /// Require mTLS at ingress before allowing operator endpoints.
    pub require_mtls: bool,
    /// Token fallback mode for operator auth.
    pub token_fallback: OperatorTokenFallback,
    /// Token source selection for operator auth.
    pub token_source: OperatorTokenSource,
    /// Token allow-list used for operator fallback.
    pub tokens: Vec<String>,
    /// Auth attempt rate (per minute). None disables.
    pub rate_per_minute: Option<NonZeroU32>,
    /// Auth attempt burst tokens. None disables.
    pub burst: Option<NonZeroU32>,
    /// Temporary lockout policy for repeated failures.
    pub lockout: OperatorAuthLockout,
    /// WebAuthn configuration (when enabled).
    pub webauthn: Option<OperatorWebAuthnConfig>,
}

impl Default for ToriiOperatorAuth {
    fn default() -> Self {
        let token_fallback = match defaults::torii::operator_auth::TOKEN_FALLBACK {
            "disabled" => OperatorTokenFallback::Disabled,
            "always" => OperatorTokenFallback::Always,
            _ => OperatorTokenFallback::Bootstrap,
        };
        let token_source = match defaults::torii::operator_auth::TOKEN_SOURCE {
            "api" => OperatorTokenSource::ApiTokens,
            "both" => OperatorTokenSource::Both,
            _ => OperatorTokenSource::OperatorTokens,
        };
        Self {
            enabled: defaults::torii::operator_auth::ENABLED,
            require_mtls: defaults::torii::operator_auth::REQUIRE_MTLS,
            token_fallback,
            token_source,
            tokens: defaults::torii::operator_auth::tokens(),
            rate_per_minute: defaults::torii::operator_auth::RATE_PER_MIN.and_then(NonZeroU32::new),
            burst: defaults::torii::operator_auth::BURST.and_then(NonZeroU32::new),
            lockout: OperatorAuthLockout::default(),
            webauthn: None,
        }
    }
}

/// Token fallback policy for operator auth.
#[derive(Debug, Clone, Copy)]
pub enum OperatorTokenFallback {
    /// Never accept tokens for operator auth.
    Disabled,
    /// Allow tokens only for bootstrap endpoints.
    Bootstrap,
    /// Allow tokens for all operator endpoints.
    Always,
}

impl OperatorTokenFallback {
    /// Render a stable label for telemetry and logging.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Bootstrap => "bootstrap",
            Self::Always => "always",
        }
    }
}

/// Token source selection for operator auth.
#[derive(Debug, Clone, Copy)]
pub enum OperatorTokenSource {
    /// Use the operator-specific token allow-list.
    OperatorTokens,
    /// Use Torii API tokens.
    ApiTokens,
    /// Accept both operator and Torii API tokens.
    Both,
}

impl OperatorTokenSource {
    /// Render a stable label for telemetry and logging.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OperatorTokens => "operator",
            Self::ApiTokens => "api",
            Self::Both => "both",
        }
    }
}

/// Lockout policy applied after repeated authentication failures.
#[derive(Debug, Clone, Copy)]
pub struct OperatorAuthLockout {
    /// Failures required to trigger a lockout (None disables lockouts).
    pub failures: Option<NonZeroU32>,
    /// Sliding window used to count failures.
    pub window: Duration,
    /// Lockout duration once triggered.
    pub duration: Duration,
}

impl Default for OperatorAuthLockout {
    fn default() -> Self {
        Self {
            failures: NonZeroU32::new(defaults::torii::operator_auth::LOCKOUT_FAILURES),
            window: Duration::from_secs(defaults::torii::operator_auth::LOCKOUT_WINDOW_SECS),
            duration: Duration::from_secs(defaults::torii::operator_auth::LOCKOUT_DURATION_SECS),
        }
    }
}

/// WebAuthn configuration required for operator auth.
#[derive(Debug, Clone)]
pub struct OperatorWebAuthnConfig {
    /// RP ID used for WebAuthn (domain).
    pub rp_id: String,
    /// RP display name used in WebAuthn options.
    pub rp_name: String,
    /// Allowed WebAuthn origins.
    pub origins: Vec<Url>,
    /// User identifier injected into WebAuthn registration options.
    pub user_id: Vec<u8>,
    /// User name injected into WebAuthn registration options.
    pub user_name: String,
    /// User display name injected into WebAuthn registration options.
    pub user_display_name: String,
    /// Challenge TTL for registration/assertion options.
    pub challenge_ttl: Duration,
    /// Session token TTL after successful assertion.
    pub session_ttl: Duration,
    /// Require user verification during assertions.
    pub require_user_verification: bool,
    /// Allowed WebAuthn algorithms.
    pub allowed_algorithms: Vec<OperatorWebAuthnAlgorithm>,
}

/// Supported WebAuthn algorithms for operator auth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorWebAuthnAlgorithm {
    /// COSE alg -7 (ES256 / P-256).
    Es256,
    /// COSE alg -8 (Ed25519).
    Ed25519,
}

impl OperatorWebAuthnAlgorithm {
    /// Return the COSE algorithm identifier.
    #[must_use]
    pub const fn cose_alg(self) -> i64 {
        match self {
            Self::Es256 => -7,
            Self::Ed25519 => -8,
        }
    }

    /// Render a stable label for telemetry and logging.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Es256 => "es256",
            Self::Ed25519 => "ed25519",
        }
    }
}

/// Peer telemetry geo lookup configuration.
#[derive(Debug, Clone)]
pub struct ToriiPeerGeo {
    /// Enable geo lookups for peer telemetry.
    pub enabled: bool,
    /// Optional geo endpoint (ip-api compatible).
    pub endpoint: Option<Url>,
}

impl Default for ToriiPeerGeo {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::peer_geo::ENABLED,
            endpoint: defaults::torii::peer_geo::endpoint(),
        }
    }
}

/// Ingress controls for SoraNet privacy telemetry endpoints.
#[derive(Debug, Clone)]
pub struct SoranetPrivacyIngest {
    /// Master enable switch for the `/v1/soranet/privacy/*` endpoints.
    pub enabled: bool,
    /// Require a token header before accepting telemetry.
    pub require_token: bool,
    /// Accepted token values.
    pub tokens: Vec<String>,
    /// Requests-per-second budget (None disables limiting).
    pub rate_per_sec: Option<NonZeroU32>,
    /// Burst capacity for the ingest limiter.
    pub burst: Option<NonZeroU32>,
    /// CIDR allow-list for trusted submitters; empty -> deny.
    pub allow_cidrs: Vec<String>,
}

impl Default for SoranetPrivacyIngest {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::soranet_privacy_ingest::ENABLED,
            require_token: defaults::torii::soranet_privacy_ingest::REQUIRE_TOKEN,
            tokens: defaults::torii::soranet_privacy_ingest::tokens(),
            rate_per_sec: defaults::torii::soranet_privacy_ingest::RATE_PER_SEC
                .and_then(std::num::NonZeroU32::new),
            burst: defaults::torii::soranet_privacy_ingest::BURST
                .and_then(std::num::NonZeroU32::new),
            allow_cidrs: defaults::torii::soranet_privacy_ingest::allow_cidrs(),
        }
    }
}

/// Transport-specific configuration exposed by Torii.
#[derive(Debug, Clone, Default)]
pub struct ToriiTransport {
    /// Norito-RPC rollout settings.
    pub norito_rpc: NoritoRpcTransport,
}

/// Norito-RPC transport configuration (stage, allowlist, toggles).
#[derive(Debug, Clone)]
pub struct NoritoRpcTransport {
    /// Master enable switch for Norito-RPC decoding.
    pub enabled: bool,
    /// Require mTLS at the ingress tier before allowing Norito-RPC (surfaced via `/rpc/capabilities`).
    pub require_mtls: bool,
    /// Explicit list of client tokens permitted during the `canary` stage.
    pub allowed_clients: Vec<String>,
    /// Current rollout stage label.
    pub stage: NoritoRpcStage,
}

/// Rollout stage for the Norito-RPC transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoritoRpcStage {
    /// Norito-RPC disabled outright (future default for prod until GA).
    #[default]
    Disabled,
    /// Canary stage: restricted to the configured allowlist.
    Canary,
    /// General availability: all authenticated clients may use Norito-RPC.
    Ga,
}

impl NoritoRpcStage {
    /// Parse a human-readable label into a stage variant.
    pub fn parse(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "disabled" => Some(Self::Disabled),
            "canary" => Some(Self::Canary),
            "ga" | "general" | "general_availability" => Some(Self::Ga),
            _ => None,
        }
    }

    /// Canonical label representation for serialization.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Canary => "canary",
            Self::Ga => "ga",
        }
    }
}

impl Default for NoritoRpcTransport {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::transport::norito_rpc::ENABLED,
            require_mtls: defaults::torii::transport::norito_rpc::REQUIRE_MTLS,
            allowed_clients: defaults::torii::transport::norito_rpc::allowed_clients(),
            stage: NoritoRpcStage::parse(defaults::torii::transport::norito_rpc::STAGE)
                .expect("default Norito-RPC stage label is valid"),
        }
    }
}

impl From<user::ToriiTransport> for ToriiTransport {
    fn from(value: user::ToriiTransport) -> Self {
        Self {
            norito_rpc: value.norito_rpc.into(),
        }
    }
}

impl From<user::ToriiNoritoRpcTransport> for NoritoRpcTransport {
    fn from(value: user::ToriiNoritoRpcTransport) -> Self {
        let stage = NoritoRpcStage::parse(&value.stage).unwrap_or_else(|| {
            panic!(
                "invalid torii.transport.norito_rpc.stage value `{}`. Expected disabled|canary|ga",
                value.stage
            )
        });
        Self {
            enabled: value.enabled,
            require_mtls: value.require_mtls,
            allowed_clients: value.allowed_clients,
            stage,
        }
    }
}

/// UAID onboarding authority wiring exposed to Torii.
#[derive(Debug, Clone)]
pub struct ToriiOnboarding {
    /// Account identifier that signs onboarding transactions.
    pub authority: AccountId,
    /// Private key corresponding to the onboarding authority.
    pub private_key: ExposedPrivateKey,
    /// Optional domain restriction for registered accounts.
    pub allowed_domain: Option<DomainId>,
}

/// Offline certificate issuer configuration exposed to Torii.
#[derive(Debug, Clone)]
pub struct ToriiOfflineIssuer {
    /// Private key used to sign offline wallet certificates.
    pub operator_private_key: ExposedPrivateKey,
    /// Allowed controller allow-list (empty => allow all).
    pub allowed_controllers: Vec<AccountId>,
}

/// Per-scheme cap applied by the Torii pre-auth gate.
#[derive(Debug, Clone)]
pub struct PreauthSchemeLimit {
    /// Scheme label (matches `ConnScheme::label()` in Torii).
    pub scheme: String,
    /// Maximum concurrent connections allowed for the scheme.
    pub max_connections: NonZeroUsize,
}

/// RBC sampling endpoint configuration.
#[derive(Debug, Copy, Clone)]
pub struct RbcSampling {
    /// Whether RBC sampling endpoints are active.
    pub enabled: bool,
    /// Maximum number of samples that a single request may return.
    pub max_samples_per_request: u32,
    /// Maximum number of bytes that a single request may return.
    pub max_bytes_per_request: u64,
    /// Daily aggregate byte budget for sampling responses.
    pub daily_byte_budget: u64,
    /// Optional per-minute request rate limit expressed as a non-zero value.
    pub rate_per_minute: Option<NonZeroU32>,
}

impl Default for RbcSampling {
    fn default() -> Self {
        Self {
            enabled: super::defaults::torii::RBC_SAMPLING_ENABLED,
            max_samples_per_request: super::defaults::torii::RBC_SAMPLING_MAX_SAMPLES_PER_REQUEST,
            max_bytes_per_request: super::defaults::torii::RBC_SAMPLING_MAX_BYTES_PER_REQUEST,
            daily_byte_budget: super::defaults::torii::RBC_SAMPLING_DAILY_BYTE_BUDGET,
            rate_per_minute: super::defaults::torii::RBC_SAMPLING_RATE_PER_MIN
                .and_then(std::num::NonZeroU32::new),
        }
    }
}

/// Replication policy applied to DA blobs based on their class.
#[derive(Debug, Clone)]
pub struct DaReplicationPolicy {
    default: RetentionPolicy,
    overrides: BTreeMap<BlobClass, RetentionPolicy>,
    taikai_availability: BTreeMap<TaikaiAvailabilityClass, RetentionPolicy>,
}

impl DaReplicationPolicy {
    /// Construct a policy from a default retention profile and class overrides.
    #[must_use]
    pub fn new(
        default: RetentionPolicy,
        overrides: BTreeMap<BlobClass, RetentionPolicy>,
        taikai_availability: BTreeMap<TaikaiAvailabilityClass, RetentionPolicy>,
    ) -> Self {
        Self {
            default,
            overrides,
            taikai_availability,
        }
    }

    fn retention_for_class(&self, class: BlobClass) -> &RetentionPolicy {
        self.overrides.get(&class).unwrap_or(&self.default)
    }

    fn retention_for_taikai(
        &self,
        availability: Option<TaikaiAvailabilityClass>,
    ) -> &RetentionPolicy {
        availability
            .and_then(|class| self.taikai_availability.get(&class))
            .unwrap_or_else(|| self.retention_for_class(BlobClass::TaikaiSegment))
    }

    /// Returns the enforced retention profile for the provided blob class.
    #[must_use]
    pub fn retention_for(
        &self,
        class: BlobClass,
        availability: Option<TaikaiAvailabilityClass>,
    ) -> &RetentionPolicy {
        if class == BlobClass::TaikaiSegment {
            return self.retention_for_taikai(availability);
        }
        self.retention_for_class(class)
    }

    /// Returns the enforced profile and whether the submitted policy mismatched it.
    #[must_use]
    pub fn enforce<'a>(
        &'a self,
        class: BlobClass,
        availability: Option<TaikaiAvailabilityClass>,
        submitted: &RetentionPolicy,
    ) -> (&'a RetentionPolicy, bool) {
        let expected = self.retention_for(class, availability);
        (expected, *submitted != *expected)
    }
}

impl Default for DaReplicationPolicy {
    fn default() -> Self {
        let default = super::defaults::torii::da_replication_default_policy();
        let overrides = super::defaults::torii::da_replication_overrides()
            .into_iter()
            .collect();
        let taikai_availability = super::defaults::torii::taikai_availability_overrides()
            .into_iter()
            .collect();
        Self {
            default,
            overrides,
            taikai_availability,
        }
    }
}

/// Data-availability ingest configuration.
#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)]
pub struct DaIngest {
    /// Maximum cached manifests tracked per `(lane, epoch)` window.
    pub replay_cache_capacity: NonZeroUsize,
    /// TTL applied to replay cache entries.
    pub replay_cache_ttl: Duration,
    /// Maximum sequence lag tolerated before rejecting manifests.
    pub replay_cache_max_sequence_lag: u64,
    /// Directory used to persist replay cursors across restarts.
    pub replay_cache_store_dir: PathBuf,
    /// Directory where canonical DA manifests are queued for SoraFS orchestration.
    pub manifest_store_dir: PathBuf,
    /// Symmetric key used to encrypt governance-only metadata entries.
    pub governance_metadata_key: Option<[u8; 32]>,
    /// Optional label advertised inside encrypted governance metadata envelopes.
    pub governance_metadata_key_label: Option<String>,
    /// Optional SoraNS anchor delivery configuration for Taikai envelopes.
    pub taikai_anchor: Option<DaTaikaiAnchor>,
    /// Replication policy enforced for each blob class.
    pub replication_policy: DaReplicationPolicy,
    /// Rent policy applied to DA submissions.
    pub rent_policy: DaRentPolicyV1,
    /// Optional telemetry cluster label used for Taikai ingest metrics.
    pub telemetry_cluster_label: Option<String>,
}

/// Configuration describing how Torii should publish Taikai artefacts to SoraNS.
#[derive(Debug, Clone)]
pub struct DaTaikaiAnchor {
    /// HTTP(S) endpoint that accepts Taikai envelope uploads.
    pub endpoint: Url,
    /// Optional bearer token supplied to the anchor service.
    pub api_token: Option<String>,
    /// Poll interval between spool scans.
    pub poll_interval: Duration,
}

impl Default for DaIngest {
    fn default() -> Self {
        Self {
            replay_cache_capacity: super::defaults::torii::DA_REPLAY_CACHE_CAPACITY,
            replay_cache_ttl: Duration::from_secs(super::defaults::torii::DA_REPLAY_CACHE_TTL_SECS),
            replay_cache_max_sequence_lag: super::defaults::torii::DA_REPLAY_CACHE_MAX_SEQUENCE_LAG,
            replay_cache_store_dir: super::defaults::torii::da_replay_cache_store_dir(),
            manifest_store_dir: super::defaults::torii::da_manifest_store_dir(),
            governance_metadata_key: defaults::torii::da_governance_metadata_key(),
            governance_metadata_key_label: defaults::torii::da_governance_metadata_key_label(),
            taikai_anchor: None,
            replication_policy: DaReplicationPolicy::default(),
            rent_policy: DaRentPolicyV1::default(),
            telemetry_cluster_label: None,
        }
    }
}

/// Torii-side SoraFS discovery configuration.
#[derive(Debug, Clone)]
pub struct SorafsDiscovery {
    /// Whether the discovery API is active.
    pub discovery_enabled: bool,
    /// Capability names recognised by the cache.
    pub known_capabilities: Vec<String>,
    /// Optional admission registry configuration.
    pub admission: Option<SorafsAdmission>,
}

impl Default for SorafsDiscovery {
    fn default() -> Self {
        Self {
            discovery_enabled: super::defaults::torii::SORAFS_DISCOVERY_ENABLED,
            known_capabilities: super::defaults::torii::sorafs_known_capabilities(),
            admission: None,
        }
    }
}

/// Governance admission registry configuration for SoraFS providers.
#[derive(Debug, Clone)]
pub struct SorafsAdmission {
    /// Directory containing governance-signed provider admission envelopes.
    pub envelopes_dir: PathBuf,
}

/// Repair scheduler configuration.
#[derive(Debug, Clone)]
pub struct SorafsRepair {
    /// Enable the repair scheduler.
    pub enabled: bool,
    /// Optional directory for durable repair state.
    pub state_dir: Option<PathBuf>,
    /// Claim TTL for repair tickets (seconds).
    pub claim_ttl_secs: u64,
    /// Heartbeat interval/TTL for active claims (seconds).
    pub heartbeat_interval_secs: u64,
    /// Maximum number of attempts before escalation.
    pub max_attempts: u32,
    /// Concurrent repair workers per node.
    pub worker_concurrency: usize,
    /// Initial retry backoff for failed repairs (seconds).
    pub backoff_initial_secs: u64,
    /// Maximum retry backoff for failed repairs (seconds).
    pub backoff_max_secs: u64,
    /// Default penalty used for scheduler-generated repair slash proposals (nano-XOR).
    pub default_slash_penalty_nano: u128,
}

impl Default for SorafsRepair {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::repair::ENABLED,
            state_dir: defaults::sorafs::repair::state_dir(),
            claim_ttl_secs: defaults::sorafs::repair::CLAIM_TTL_SECS,
            heartbeat_interval_secs: defaults::sorafs::repair::HEARTBEAT_INTERVAL_SECS,
            max_attempts: defaults::sorafs::repair::MAX_ATTEMPTS,
            worker_concurrency: defaults::sorafs::repair::WORKER_CONCURRENCY,
            backoff_initial_secs: defaults::sorafs::repair::BACKOFF_INITIAL_SECS,
            backoff_max_secs: defaults::sorafs::repair::BACKOFF_MAX_SECS,
            default_slash_penalty_nano: defaults::sorafs::repair::DEFAULT_SLASH_PENALTY_NANO,
        }
    }
}

/// GC scheduler configuration.
#[derive(Debug, Clone)]
pub struct SorafsGc {
    /// Enable the GC worker.
    pub enabled: bool,
    /// Optional directory for durable GC state.
    pub state_dir: Option<PathBuf>,
    /// GC cadence (seconds).
    pub interval_secs: u64,
    /// Maximum deletions per GC run.
    pub max_deletions_per_run: u32,
    /// Grace window for retention expiry (seconds).
    pub retention_grace_secs: u64,
    /// Attempt a GC sweep before rejecting new pins when storage is full.
    pub pre_admission_sweep: bool,
}

impl Default for SorafsGc {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::gc::ENABLED,
            state_dir: defaults::sorafs::gc::state_dir(),
            interval_secs: defaults::sorafs::gc::INTERVAL_SECS,
            max_deletions_per_run: defaults::sorafs::gc::MAX_DELETIONS_PER_RUN,
            retention_grace_secs: defaults::sorafs::gc::RETENTION_GRACE_SECS,
            pre_admission_sweep: defaults::sorafs::gc::PRE_ADMISSION_SWEEP,
        }
    }
}

/// Proof-of-Retrievability coordinator configuration.
#[derive(Debug, Clone)]
pub struct SorafsPor {
    /// Enable the coordinator runtime.
    pub enabled: bool,
    /// Duration of a PoR epoch (seconds).
    pub epoch_interval_secs: u64,
    /// Window granted to providers to submit proofs (seconds).
    pub response_window_secs: u64,
    /// Filesystem directory used to persist governance DAG payloads.
    pub governance_dag_dir: PathBuf,
    /// Optional deterministic randomness seed (32 bytes).
    pub randomness_seed: Option<[u8; 32]>,
}

impl Default for SorafsPor {
    fn default() -> Self {
        Self {
            enabled: super::defaults::sorafs::por::ENABLED,
            epoch_interval_secs: super::defaults::sorafs::por::EPOCH_INTERVAL_SECS,
            response_window_secs: super::defaults::sorafs::por::RESPONSE_WINDOW_SECS,
            governance_dag_dir: super::defaults::sorafs::por::governance_dir(),
            randomness_seed: super::defaults::sorafs::por::randomness_seed_hex()
                .and_then(|hex| hex::decode(hex).ok())
                .and_then(|bytes| bytes.try_into().ok()),
        }
    }
}

/// Embedded SoraFS storage configuration (Torii-owned).
#[derive(Debug, Clone)]
pub struct SorafsStorage {
    /// Whether the storage worker is enabled.
    pub enabled: bool,
    /// Root directory for chunk data, manifests, and telemetry artefacts.
    pub data_dir: PathBuf,
    /// Maximum on-disk footprint allocated to stored chunks.
    pub max_capacity_bytes: Bytes<u64>,
    /// Maximum number of concurrent fetch streams served.
    pub max_parallel_fetches: usize,
    /// Maximum number of pinned manifests accepted before back-pressure.
    pub max_pins: usize,
    /// Periodic Proof-of-Retrievability sampling cadence (seconds).
    pub por_sample_interval_secs: u64,
    /// Optional human-friendly alias advertised in telemetry.
    pub alias: Option<String>,
    /// Optional overrides applied when producing provider adverts.
    pub adverts: SorafsAdvertOverrides,
    /// Optional smoothing configuration applied to metering outputs.
    pub metering_smoothing: SorafsMeteringSmoothing,
    /// Stream-token issuance configuration for chunk-range gateways.
    pub stream_tokens: SorafsTokenConfig,
    /// Optional filesystem directory used to publish governance artefacts.
    pub governance_dag_dir: Option<PathBuf>,
    /// Authentication and rate limits for manifest pin submissions.
    pub pin: SorafsStoragePin,
}

/// Authentication and abuse controls for `/v1/sorafs/storage/pin`.
#[derive(Debug, Clone)]
pub struct SorafsStoragePin {
    /// Whether a bearer token is required to submit a pin request.
    pub require_token: bool,
    /// Static allow-list of bearer tokens accepted by the gateway.
    pub tokens: BTreeSet<String>,
    /// Optional CIDR allow-list limiting clients that may submit pin requests.
    pub allow_cidrs: Vec<String>,
    /// Per-client rate limits applied to pin submissions.
    pub rate_limit: SorafsGatewayRateLimit,
}

impl Default for SorafsStorage {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::storage::ENABLED,
            data_dir: defaults::sorafs::storage::data_dir(),
            max_capacity_bytes: defaults::sorafs::storage::MAX_CAPACITY_BYTES,
            max_parallel_fetches: defaults::sorafs::storage::MAX_PARALLEL_FETCHES,
            max_pins: defaults::sorafs::storage::MAX_PINS,
            por_sample_interval_secs: defaults::sorafs::storage::POR_SAMPLE_INTERVAL_SECS,
            alias: defaults::sorafs::storage::alias(),
            adverts: SorafsAdvertOverrides::default(),
            metering_smoothing: SorafsMeteringSmoothing::default(),
            stream_tokens: SorafsTokenConfig::default(),
            governance_dag_dir: defaults::sorafs::storage::governance_dir(),
            pin: SorafsStoragePin::default(),
        }
    }
}

impl Default for SorafsStoragePin {
    fn default() -> Self {
        Self {
            require_token: false,
            tokens: BTreeSet::new(),
            allow_cidrs: Vec::new(),
            rate_limit: SorafsGatewayRateLimit::disabled(),
        }
    }
}

/// Under-delivery penalty policy enforced for SoraFS providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SorafsPenaltyPolicy {
    /// Minimum utilisation ratio (basis points) required before a strike is counted.
    pub utilisation_floor_bps: u16,
    /// Minimum uptime success rate (basis points) required before a strike is counted.
    pub uptime_floor_bps: u16,
    /// Minimum Proof-of-Retrievability success rate (basis points) required before a strike.
    pub por_success_floor_bps: u16,
    /// Number of consecutive strikes required before applying a penalty.
    pub strike_threshold: u32,
    /// Fraction of bonded collateral removed when a penalty triggers (basis points).
    pub penalty_bond_bps: u16,
    /// Cooldown window count (settlement windows) enforced between penalties.
    pub cooldown_windows: u32,
    /// Maximum PDP failures tolerated within a telemetry window before forcing a strike (0 = none).
    pub max_pdp_failures: u32,
    /// Maximum PoTR SLA breaches tolerated within a telemetry window before forcing a strike (0 = none).
    pub max_potr_breaches: u32,
}

impl SorafsPenaltyPolicy {
    /// Compute the cooldown interval in seconds based on the configured settlement window.
    #[must_use]
    pub fn cooldown_window_secs(&self, settlement_window_secs: u64) -> u64 {
        settlement_window_secs.saturating_mul(u64::from(self.cooldown_windows))
    }
}

impl Default for SorafsPenaltyPolicy {
    fn default() -> Self {
        Self {
            utilisation_floor_bps: defaults::governance::sorafs_penalty::UTILISATION_FLOOR_BPS,
            uptime_floor_bps: defaults::governance::sorafs_penalty::UPTIME_FLOOR_BPS,
            por_success_floor_bps: defaults::governance::sorafs_penalty::POR_SUCCESS_FLOOR_BPS,
            strike_threshold: defaults::governance::sorafs_penalty::STRIKE_THRESHOLD,
            penalty_bond_bps: defaults::governance::sorafs_penalty::PENALTY_BOND_BPS,
            cooldown_windows: defaults::governance::sorafs_penalty::COOLDOWN_WINDOWS,
            max_pdp_failures: defaults::governance::sorafs_penalty::MAX_PDP_FAILURES,
            max_potr_breaches: defaults::governance::sorafs_penalty::MAX_POTR_BREACHES,
        }
    }
}

/// Governance policy for repair escalation and slashing decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairEscalationPolicyV1 {
    /// Approval quorum (basis points) required for escalation/slash decisions.
    pub quorum_bps: u16,
    /// Minimum number of distinct voters required.
    pub minimum_voters: u32,
    /// Dispute window in seconds after escalation before governance finalizes.
    pub dispute_window_secs: u64,
    /// Appeal window in seconds after approval before a decision is final.
    pub appeal_window_secs: u64,
    /// Maximum slash penalty allowed for repair escalations (nano-XOR).
    pub max_penalty_nano: u128,
}

impl Default for RepairEscalationPolicyV1 {
    fn default() -> Self {
        Self {
            quorum_bps: defaults::governance::sorafs_repair_escalation::QUORUM_BPS,
            minimum_voters: defaults::governance::sorafs_repair_escalation::MINIMUM_VOTERS,
            dispute_window_secs:
                defaults::governance::sorafs_repair_escalation::DISPUTE_WINDOW_SECS,
            appeal_window_secs: defaults::governance::sorafs_repair_escalation::APPEAL_WINDOW_SECS,
            max_penalty_nano: defaults::governance::sorafs_repair_escalation::MAX_PENALTY_NANO,
        }
    }
}

/// Telemetry authentication and replay policy for SoraFS capacity windows.
#[derive(Debug, Clone)]
pub struct SorafsTelemetryPolicy {
    /// Require telemetry submissions to originate from a configured allow-list.
    pub require_submitter: bool,
    /// Require a replay nonce on each telemetry window.
    pub require_nonce: bool,
    /// Maximum tolerated gap between consecutive telemetry windows.
    pub max_window_gap: Duration,
    /// Reject zero-capacity telemetry windows.
    pub reject_zero_capacity: bool,
    /// Accounts permitted to submit telemetry when `require_submitter` is true.
    pub submitters: Vec<AccountId>,
    /// Per-provider submitter overrides; when present, this list is enforced instead of the global list.
    pub per_provider_submitters: BTreeMap<ProviderId, Vec<AccountId>>,
}

impl Default for SorafsTelemetryPolicy {
    fn default() -> Self {
        Self {
            require_submitter: defaults::governance::sorafs_telemetry::REQUIRE_SUBMITTER,
            require_nonce: defaults::governance::sorafs_telemetry::REQUIRE_NONCE,
            max_window_gap: Duration::from_secs(
                defaults::governance::sorafs_telemetry::MAX_WINDOW_GAP_SECS,
            ),
            reject_zero_capacity: defaults::governance::sorafs_telemetry::REJECT_ZERO_CAPACITY,
            submitters: defaults::governance::sorafs_telemetry::submitters()
                .iter()
                .map(|id| {
                    id.parse()
                        .expect("default SoraFS telemetry submitter account id")
                })
                .collect(),
            per_provider_submitters: BTreeMap::new(),
        }
    }
}

/// Optional smoothing parameters for metering outputs.
#[derive(Debug, Clone, Copy, Default)]
pub struct SorafsMeteringSmoothing {
    /// Alpha applied to the GiB·hour exponential moving average.
    pub gib_hours_alpha: Option<f64>,
    /// Alpha applied to the PoR-success exponential moving average.
    pub por_success_alpha: Option<f64>,
}

/// Stream-token issuance configuration for chunk-range gateways.
#[derive(Debug, Clone)]
pub struct SorafsTokenConfig {
    /// Enable stream-token issuance.
    pub enabled: bool,
    /// Filesystem path to the Ed25519 signing key.
    pub signing_key_path: Option<PathBuf>,
    /// Public-key version advertised in issued tokens.
    pub key_version: u32,
    /// Default TTL applied to tokens (seconds).
    pub default_ttl_secs: u64,
    /// Default concurrent-stream budget encoded per token.
    pub default_max_streams: u16,
    /// Default sustained throughput per token (bytes per second).
    pub default_rate_limit_bytes: u64,
    /// Default refresh budget (requests per minute).
    pub default_requests_per_minute: u32,
}

impl Default for SorafsTokenConfig {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::storage::tokens::ENABLED,
            signing_key_path: defaults::sorafs::storage::tokens::signing_key_path(),
            key_version: defaults::sorafs::storage::tokens::KEY_VERSION,
            default_ttl_secs: defaults::sorafs::storage::tokens::DEFAULT_TTL_SECS,
            default_max_streams: defaults::sorafs::storage::tokens::DEFAULT_MAX_STREAMS,
            default_rate_limit_bytes: defaults::sorafs::storage::tokens::DEFAULT_RATE_LIMIT_BYTES,
            default_requests_per_minute:
                defaults::sorafs::storage::tokens::DEFAULT_REQUESTS_PER_MINUTE,
        }
    }
}

/// Per-action quota configuration for SoraFS control-plane endpoints.
#[derive(Debug, Clone, Copy)]
pub struct SorafsQuotaWindow {
    /// Maximum events permitted within the rolling window. `None` disables the quota.
    pub max_events: Option<NonZeroU32>,
    /// Rolling window duration.
    pub window: Duration,
}

impl Default for SorafsQuotaWindow {
    fn default() -> Self {
        Self {
            max_events: None,
            window: Duration::from_secs(1),
        }
    }
}

/// Consolidated quota configuration for SoraFS control-plane endpoints.
#[derive(Debug, Clone, Copy)]
pub struct SorafsQuota {
    /// Quota applied to capacity declaration submissions.
    pub capacity_declaration: SorafsQuotaWindow,
    /// Quota applied to capacity telemetry reports.
    pub capacity_telemetry: SorafsQuotaWindow,
    /// Quota applied to deal telemetry submissions.
    pub deal_telemetry: SorafsQuotaWindow,
    /// Quota applied to capacity disputes raised against providers.
    pub capacity_dispute: SorafsQuotaWindow,
    /// Quota applied to manifest pin submissions.
    pub storage_pin: SorafsQuotaWindow,
    /// Quota applied to proof-of-retrievability submissions.
    pub por_submission: SorafsQuotaWindow,
}

impl Default for SorafsQuota {
    fn default() -> Self {
        Self {
            capacity_declaration: SorafsQuotaWindow {
                max_events: defaults::torii::SORAFS_QUOTA_DECLARATION_MAX_EVENTS
                    .and_then(NonZeroU32::new),
                window: Duration::from_secs(defaults::torii::SORAFS_QUOTA_DECLARATION_WINDOW_SECS),
            },
            capacity_telemetry: SorafsQuotaWindow {
                max_events: defaults::torii::SORAFS_QUOTA_TELEMETRY_MAX_EVENTS
                    .and_then(NonZeroU32::new),
                window: Duration::from_secs(defaults::torii::SORAFS_QUOTA_TELEMETRY_WINDOW_SECS),
            },
            deal_telemetry: SorafsQuotaWindow {
                max_events: defaults::torii::SORAFS_QUOTA_DEAL_TELEMETRY_MAX_EVENTS
                    .and_then(NonZeroU32::new),
                window: Duration::from_secs(
                    defaults::torii::SORAFS_QUOTA_DEAL_TELEMETRY_WINDOW_SECS,
                ),
            },
            capacity_dispute: SorafsQuotaWindow {
                max_events: defaults::torii::SORAFS_QUOTA_DISPUTE_MAX_EVENTS
                    .and_then(NonZeroU32::new),
                window: Duration::from_secs(defaults::torii::SORAFS_QUOTA_DISPUTE_WINDOW_SECS),
            },
            storage_pin: SorafsQuotaWindow {
                max_events: defaults::torii::SORAFS_QUOTA_STORAGE_PIN_MAX_EVENTS
                    .and_then(NonZeroU32::new),
                window: Duration::from_secs(defaults::torii::SORAFS_QUOTA_STORAGE_PIN_WINDOW_SECS),
            },
            por_submission: SorafsQuotaWindow {
                max_events: defaults::torii::SORAFS_QUOTA_POR_MAX_EVENTS.and_then(NonZeroU32::new),
                window: Duration::from_secs(defaults::torii::SORAFS_QUOTA_POR_WINDOW_SECS),
            },
        }
    }
}

/// Alias cache policy shared by Torii gateways and client helpers.
#[derive(Debug, Clone, Copy)]
pub struct SorafsAliasCachePolicy {
    /// Positive TTL for cached alias proofs.
    pub positive_ttl: Duration,
    /// Refresh window applied before the positive TTL elapses.
    pub refresh_window: Duration,
    /// Hard expiry after which stale proofs are rejected.
    pub hard_expiry: Duration,
    /// Negative cache TTL for missing aliases.
    pub negative_ttl: Duration,
    /// TTL for revoked aliases (responses returning `410 Gone`).
    pub revocation_ttl: Duration,
    /// Maximum tolerated age for alias proof bundles before rotation is required.
    pub rotation_max_age: Duration,
    /// Grace period applied after an approved successor before predecessor proofs are refused.
    pub successor_grace: Duration,
    /// Grace period applied to governance rotation events.
    pub governance_grace: Duration,
}

impl Default for SorafsAliasCachePolicy {
    fn default() -> Self {
        Self {
            positive_ttl: Duration::from_secs(defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
            refresh_window: Duration::from_secs(defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
            hard_expiry: Duration::from_secs(defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
            negative_ttl: Duration::from_secs(defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
            revocation_ttl: Duration::from_secs(defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
            rotation_max_age: Duration::from_secs(
                defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS,
            ),
            successor_grace: Duration::from_secs(
                defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS,
            ),
            governance_grace: Duration::from_secs(
                defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS,
            ),
        }
    }
}

/// Staged anonymity rollout policy for SoraNet transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum SorafsAnonymityStage {
    /// Require at least one PQ-capable guard (Stage A).
    GuardPq,
    /// Prefer PQ-capable relays for a super-majority (Stage B).
    MajorityPq,
    /// Enforce PQ-only SoraNet paths (Stage C).
    StrictPq,
}

impl SorafsAnonymityStage {
    /// Parses a policy label into the corresponding stage.
    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        let normalised = label.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "anon_guard_pq" | "anon-guard-pq" | "stage_a" | "stage-a" | "stagea" => {
                Some(Self::GuardPq)
            }
            "anon_majority_pq" | "anon-majority-pq" | "stage_b" | "stage-b" | "stageb" => {
                Some(Self::MajorityPq)
            }
            "anon_strict_pq" | "anon-strict-pq" | "stage_c" | "stage-c" | "stagec" => {
                Some(Self::StrictPq)
            }
            _ => None,
        }
    }

    /// Returns the canonical label for the stage.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::GuardPq => "anon-guard-pq",
            Self::MajorityPq => "anon-majority-pq",
            Self::StrictPq => "anon-strict-pq",
        }
    }
}

/// High-level rollout phase controlling the staged PQ activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SorafsRolloutPhase {
    /// Canary phase – default to Stage A (guard PQ required).
    #[default]
    Canary,
    /// Ramp phase – default to Stage B (majority PQ preferred).
    Ramp,
    /// Default phase – default to Stage C (strict PQ).
    Default,
}

impl SorafsRolloutPhase {
    /// Parses a rollout phase label into the corresponding variant.
    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        let normalised = label.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "canary" | "stage_a" | "stage-a" | "stagea" => Some(Self::Canary),
            "ramp" | "stage_b" | "stage-b" | "stageb" | "majority" => Some(Self::Ramp),
            "default" | "stable" | "ga" | "stage_c" | "stage-c" | "stagec" => Some(Self::Default),
            _ => None,
        }
    }

    /// Returns the canonical label for the rollout phase.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Canary => "canary",
            Self::Ramp => "ramp",
            Self::Default => "default",
        }
    }

    /// Returns the anonymity stage associated with the rollout phase.
    #[must_use]
    pub fn default_anonymity_policy(self) -> SorafsAnonymityStage {
        match self {
            Self::Canary => SorafsAnonymityStage::GuardPq,
            Self::Ramp => SorafsAnonymityStage::MajorityPq,
            Self::Default => SorafsAnonymityStage::StrictPq,
        }
    }
}

/// Gateway policy configuration for SoraFS delivery.
#[derive(Debug, Clone)]
pub struct SorafsGateway {
    /// Require clients to attach the manifest envelope.
    pub require_manifest_envelope: bool,
    /// Enforce admission registry membership for providers.
    pub enforce_admission: bool,
    /// Enforce advertised capabilities (e.g., chunk-range fetch) before serving data.
    pub enforce_capabilities: bool,
    /// Directory containing SoraNet salt announcements (Norito JSON).
    pub salt_schedule_dir: Option<PathBuf>,
    /// Optional CDN policy payload (GarCdnPolicyV1) loaded from disk.
    pub cdn_policy_path: Option<PathBuf>,
    /// Client-facing rate limit configuration.
    pub rate_limit: SorafsGatewayRateLimit,
    /// Denylist bootstrap configuration.
    pub denylist: SorafsGatewayDenylist,
    /// High-level rollout phase controlling default anonymity policy.
    pub rollout_phase: SorafsRolloutPhase,
    /// Optional staged anonymity policy override.
    pub anonymity_policy: Option<SorafsAnonymityStage>,
    /// ACME automation configuration.
    pub acme: SorafsGatewayAcme,
    /// Optional direct-mode override configuration.
    pub direct_mode: Option<SorafsGatewayDirectMode>,
}

impl Default for SorafsGateway {
    fn default() -> Self {
        Self {
            require_manifest_envelope: defaults::sorafs::gateway::REQUIRE_MANIFEST_ENVELOPE,
            enforce_admission: defaults::sorafs::gateway::ENFORCE_ADMISSION,
            enforce_capabilities: defaults::sorafs::gateway::ENFORCE_CAPABILITIES,
            salt_schedule_dir: None,
            cdn_policy_path: None,
            rate_limit: SorafsGatewayRateLimit::default(),
            denylist: SorafsGatewayDenylist::default(),
            rollout_phase: SorafsRolloutPhase::default(),
            anonymity_policy: Some(
                SorafsAnonymityStage::parse(defaults::sorafs::gateway::DEFAULT_ANONYMITY_POLICY)
                    .unwrap_or_else(|| SorafsRolloutPhase::default().default_anonymity_policy()),
            ),
            acme: SorafsGatewayAcme::default(),
            direct_mode: None,
        }
    }
}

impl SorafsGateway {
    /// Returns the effective anonymity policy, falling back to the rollout phase when unset.
    #[must_use]
    pub fn effective_anonymity_policy(&self) -> SorafsAnonymityStage {
        self.anonymity_policy
            .unwrap_or_else(|| self.rollout_phase.default_anonymity_policy())
    }
}

/// Rolling-window rate limit applied to gateway clients.
#[derive(Debug, Clone, Copy)]
pub struct SorafsGatewayRateLimit {
    /// Maximum requests permitted within the window.
    pub max_requests: Option<NonZeroU32>,
    /// Duration of the accounting window.
    pub window: Duration,
    /// Optional temporary ban duration.
    pub ban: Option<Duration>,
}

impl Default for SorafsGatewayRateLimit {
    fn default() -> Self {
        Self {
            max_requests: defaults::sorafs::gateway::rate_limit::MAX_REQUESTS
                .and_then(NonZeroU32::new),
            window: defaults::sorafs::gateway::rate_limit::WINDOW,
            ban: defaults::sorafs::gateway::rate_limit::BAN,
        }
    }
}

impl SorafsGatewayRateLimit {
    /// Convenience helper returning a disabled rate limit.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            max_requests: None,
            window: Duration::from_secs(1),
            ban: None,
        }
    }
}

/// Configuration for bootstrapping gateway denylists.
#[derive(Debug, Clone)]
pub struct SorafsGatewayDenylist {
    /// Optional filesystem path to a JSON denylist.
    pub path: Option<PathBuf>,
    /// Maximum TTL applied to standard entries when `expires_at` is omitted.
    pub standard_ttl: Duration,
    /// Maximum TTL applied to emergency entries.
    pub emergency_ttl: Duration,
    /// Review window enforced for emergency canons.
    pub emergency_review_window: Duration,
    /// Require governance references for permanent entries.
    pub require_governance_reference: bool,
}

impl Default for SorafsGatewayDenylist {
    fn default() -> Self {
        Self {
            path: defaults::sorafs::gateway::denylist::path(),
            standard_ttl: defaults::sorafs::gateway::denylist::STANDARD_TTL,
            emergency_ttl: defaults::sorafs::gateway::denylist::EMERGENCY_TTL,
            emergency_review_window: defaults::sorafs::gateway::denylist::EMERGENCY_REVIEW_WINDOW,
            require_governance_reference:
                defaults::sorafs::gateway::denylist::REQUIRE_GOVERNANCE_REFERENCE,
        }
    }
}

/// Challenge toggles for ACME automation.
#[derive(Debug, Clone, Copy)]
pub struct SorafsGatewayAcmeChallenges {
    /// Whether DNS-01 challenges should be solved.
    pub dns01: bool,
    /// Whether TLS-ALPN-01 challenges should be solved.
    pub tls_alpn_01: bool,
}

impl Default for SorafsGatewayAcmeChallenges {
    fn default() -> Self {
        Self {
            dns01: defaults::sorafs::gateway::acme::DNS01,
            tls_alpn_01: defaults::sorafs::gateway::acme::TLS_ALPN_01,
        }
    }
}

/// ACME automation settings for TLS/ECH management.
#[derive(Debug, Clone)]
pub struct SorafsGatewayAcme {
    /// Enable ACME automation.
    pub enabled: bool,
    /// Account email registered with the ACME provider.
    pub account_email: Option<String>,
    /// ACME directory URL.
    pub directory_url: String,
    /// Hostnames covered by certificate orders.
    pub hostnames: Vec<String>,
    /// Identifier of the DNS provider used for DNS-01 challenges.
    pub dns_provider_id: Option<String>,
    /// Renewal window applied before certificate expiry.
    pub renewal_window: Duration,
    /// Base backoff applied after failures.
    pub retry_backoff: Duration,
    /// Maximum jitter applied to retry scheduling.
    pub retry_jitter: Duration,
    /// Challenge toggles to exercise.
    pub challenges: SorafsGatewayAcmeChallenges,
    /// Initial ECH enabled state exposed via telemetry.
    pub ech_enabled: bool,
}

impl Default for SorafsGatewayAcme {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::gateway::acme::ENABLED,
            account_email: defaults::sorafs::gateway::acme::account_email(),
            directory_url: defaults::sorafs::gateway::acme::directory_url(),
            hostnames: defaults::sorafs::gateway::acme::hostnames(),
            dns_provider_id: defaults::sorafs::gateway::acme::dns_provider_id(),
            renewal_window: defaults::sorafs::gateway::acme::RENEWAL_WINDOW,
            retry_backoff: defaults::sorafs::gateway::acme::RETRY_BACKOFF,
            retry_jitter: defaults::sorafs::gateway::acme::RETRY_JITTER,
            challenges: SorafsGatewayAcmeChallenges::default(),
            ech_enabled: defaults::sorafs::gateway::acme::ECH_ENABLED,
        }
    }
}

/// Optional direct-mode override details for gateway configuration.
#[derive(Debug, Clone)]
pub struct SorafsGatewayDirectMode {
    /// Provider identifier associated with the direct-mode override (hex).
    pub provider_id_hex: String,
    /// Chain id associated with the override.
    pub chain_id: String,
    /// Canonical hostname derived from governance inputs.
    pub canonical_host: String,
    /// Vanity hostname exposed for direct-mode tooling.
    pub vanity_host: String,
    /// Direct-CAR endpoint bound to the canonical host.
    pub direct_car_canonical: String,
    /// Direct-CAR endpoint bound to the vanity host.
    pub direct_car_vanity: String,
    /// Manifest digest tied to the override.
    pub manifest_digest_hex: String,
}

/// Optional overrides for provider advert telemetry generated by the storage worker.
#[derive(Debug, Clone)]
pub struct SorafsAdvertOverrides {
    /// Optional governance stake pointer advertised alongside the provider ID.
    pub stake_pointer: Option<String>,
    /// Availability tier advertised in QoS hints.
    pub availability: String,
    /// Maximum retrieval latency (milliseconds) advertised in QoS hints.
    pub max_latency_ms: u32,
    /// Rendezvous topics broadcast for discovery.
    pub topics: Vec<String>,
}

impl Default for SorafsAdvertOverrides {
    fn default() -> Self {
        Self {
            stake_pointer: None,
            availability: defaults::sorafs::storage::advert_availability(),
            max_latency_ms: defaults::sorafs::storage::ADVERT_MAX_LATENCY_MS,
            topics: defaults::sorafs::storage::advert_topics(),
        }
    }
}

/// Governance-defined constraints enforced on SoraFS pin policies.
#[derive(Debug, Clone)]
pub struct SorafsPinPolicyConstraints {
    /// Minimum number of replicas required for an approved manifest.
    pub min_replicas_floor: u16,
    /// Optional ceiling on requested replicas.
    pub max_replicas_ceiling: Option<u16>,
    /// Optional maximum retention epoch (inclusive).
    pub max_retention_epoch: Option<u64>,
    /// Allowed storage classes for replicas; `None` permits any class.
    pub allowed_storage_classes: Option<BTreeSet<SorafsStorageClass>>,
}

impl Default for SorafsPinPolicyConstraints {
    fn default() -> Self {
        Self {
            min_replicas_floor: super::defaults::governance::sorafs_pin_policy::MIN_REPLICAS_FLOOR,
            max_replicas_ceiling:
                super::defaults::governance::sorafs_pin_policy::MAX_REPLICAS_CEILING,
            max_retention_epoch:
                super::defaults::governance::sorafs_pin_policy::MAX_RETENTION_EPOCH,
            allowed_storage_classes: None,
        }
    }
}

/// Iroha Connect configuration.
#[derive(Debug, Clone, Copy)]
pub struct Connect {
    /// Enable Iroha Connect WS + P2P relay.
    pub enabled: bool,
    /// Max concurrent WS sessions across roles.
    pub ws_max_sessions: usize,
    /// Max concurrent WS sessions per remote IP (0 disables the per-IP cap).
    pub ws_per_ip_max_sessions: usize,
    /// Per-IP WS handshake rate (requests per minute, 0 disables rate limiting).
    pub ws_rate_per_ip_per_min: u32,
    /// Session inactivity TTL.
    pub session_ttl: Duration,
    /// Maximum WS frame size accepted for Connect frames (bytes).
    pub frame_max_bytes: usize,
    /// Maximum buffered payload per session (bytes) for pending delivery.
    pub session_buffer_max_bytes: usize,
    /// Heartbeat ping interval.
    pub ping_interval: Duration,
    /// Number of consecutive missed pongs tolerated before disconnect.
    pub ping_miss_tolerance: u32,
    /// Minimum heartbeat interval enforced for browser transports.
    pub ping_min_interval: Duration,
    /// Dedupe cache TTL.
    pub dedupe_ttl: Duration,
    /// Dedupe cache capacity (entries).
    pub dedupe_cap: usize,
    /// Enable P2P re-broadcast relay.
    pub relay_enabled: bool,
    /// Relay strategy string ("broadcast" for now).
    pub relay_strategy: &'static str,
    /// Optional hop TTL for relay (0 = disabled).
    pub p2p_ttl_hops: u8,
}

/// ISO 20022 bridge configuration.
#[derive(Debug, Clone)]
pub struct IsoBridge {
    /// Enable ISO 20022 ingestion endpoints.
    pub enabled: bool,
    /// TTL for deduplication records (seconds).
    pub dedupe_ttl_secs: u64,
    /// Optional signer configuration when enabled.
    pub signer: Option<IsoBridgeSigner>,
    /// Mapping of IBANs to on-ledger account identifiers.
    pub account_aliases: Vec<IsoAccountAlias>,
    /// Mapping of currency codes to asset definitions.
    pub currency_assets: Vec<IsoCurrencyAsset>,
    /// Reference-data ingestion and refresh settings.
    pub reference_data: IsoReferenceData,
}

/// Signing configuration for ISO bridge transactions.
#[derive(Debug, Clone)]
pub struct IsoBridgeSigner {
    /// Account identifier used as the transaction authority.
    pub account_id: String,
    /// Private key used to sign generated transactions.
    pub private_key: PrivateKey,
}

/// Account alias mapping (IBAN -> AccountId).
#[derive(Debug, Clone)]
pub struct IsoAccountAlias {
    /// External IBAN representation.
    pub iban: String,
    /// Account identifier (IH58 or `<alias|public_key>@domain`).
    pub account_id: String,
}

/// Currency to asset definition mapping.
#[derive(Debug, Clone)]
pub struct IsoCurrencyAsset {
    /// ISO 4217 currency code (e.g., `USD`).
    pub currency: String,
    /// Asset definition identifier (e.g., `usd#payments`).
    pub asset_definition: String,
}

/// Reference data inputs (ISIN/CUSIP, BIC↔LEI, MIC).
#[derive(Debug, Clone)]
pub struct IsoReferenceData {
    /// Refresh cadence for reference snapshot ingestion.
    pub refresh_interval: Duration,
    /// Optional path to an ANNA/CUSIP crosswalk snapshot.
    pub isin_crosswalk_path: Option<PathBuf>,
    /// Optional path to a BIC↔LEI mapping snapshot.
    pub bic_lei_path: Option<PathBuf>,
    /// Optional path to a MIC directory snapshot.
    pub mic_directory_path: Option<PathBuf>,
    /// Directory where loaded snapshots and provenance metadata should be cached.
    pub cache_dir: Option<PathBuf>,
}

impl Default for IsoReferenceData {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(
                super::defaults::torii::ISO_BRIDGE_REFERENCE_REFRESH_SECS,
            ),
            isin_crosswalk_path: None,
            bic_lei_path: None,
            mic_directory_path: None,
            cache_dir: None,
        }
    }
}

/// Zero-knowledge proof configuration namespace.
#[derive(Debug, Clone)]
pub struct Zk {
    /// Halo2 (transparent) verification settings.
    pub halo2: Halo2,
    /// FASTPQ prover settings.
    pub fastpq: Fastpq,
    /// Cap on the number of recent shielded Merkle roots kept per asset.
    pub root_history_cap: usize,
    /// Cap on the number of recent ballot ciphertexts kept per election.
    pub ballot_history_cap: usize,
    /// When an asset has no commitments, include an explicit empty-tree root in read APIs.
    pub empty_root_on_empty: bool,
    /// Depth to use when computing the explicit empty-tree root.
    pub merkle_depth: u8,
    /// Maximum accepted proof size for stateless pre-verification (bytes).
    pub preverify_max_bytes: usize,
    /// Soft byte-budget for stateless pre-verification (0 = unlimited).
    pub preverify_budget_bytes: u64,
    /// Maximum number of recent proof records to retain per backend (0 = unlimited).
    pub proof_history_cap: usize,
    /// Minimum number of blocks to retain proof records regardless of cap (age-based pruning).
    pub proof_retention_grace_blocks: u64,
    /// Maximum number of proof records pruned per enforcement pass (0 = unlimited).
    pub proof_prune_batch: usize,
    /// Maximum length of a bridge proof range (`end_height - start_height + 1`, 0 = unlimited).
    pub bridge_proof_max_range_len: u64,
    /// Maximum age (in blocks) a bridge proof's end height may trail the current block (0 = unlimited).
    pub bridge_proof_max_past_age_blocks: u64,
    /// Maximum future drift (in blocks) a bridge proof's end height may lead the current block (0 = unlimited).
    pub bridge_proof_max_future_drift_blocks: u64,
    /// Poseidon parameter set identifier to embed into policies (if any).
    pub poseidon_params_id: Option<u32>,
    /// Pedersen parameter set identifier to embed into policies (if any).
    pub pedersen_params_id: Option<u32>,
    /// Optional verifying key reference used for Kaigi roster join proofs.
    pub kaigi_roster_join_vk: Option<VerifyingKeyRef>,
    /// Optional verifying key reference used for Kaigi roster leave proofs.
    pub kaigi_roster_leave_vk: Option<VerifyingKeyRef>,
    /// Optional verifying key reference used for Kaigi usage commitment proofs.
    pub kaigi_usage_vk: Option<VerifyingKeyRef>,
    /// Maximum proof size accepted from a single confidential operation.
    pub max_proof_size_bytes: u32,
    /// Maximum number of nullifiers a transaction may consume.
    pub max_nullifiers_per_tx: u32,
    /// Maximum number of commitments a transaction may create.
    pub max_commitments_per_tx: u32,
    /// Maximum confidential operations allowed in a block.
    pub max_confidential_ops_per_block: u32,
    /// Verifier timeout for confidential proofs.
    pub verify_timeout: Duration,
    /// Maximum age (in blocks) for anchors referenced by confidential proofs.
    pub max_anchor_age_blocks: u64,
    /// Aggregate proof bytes allowed per block.
    pub max_proof_bytes_block: u64,
    /// Maximum verification calls allowed per transaction.
    pub max_verify_calls_per_tx: u32,
    /// Maximum verification calls allowed per block.
    pub max_verify_calls_per_block: u32,
    /// Maximum public inputs accepted per proof.
    pub max_public_inputs: u32,
    /// Configured reorg depth bound for retaining commitment tree checkpoints.
    pub reorg_depth_bound: u64,
    /// Minimum delay (in blocks) between policy change request and activation.
    pub policy_transition_delay_blocks: u64,
    /// Grace window (in blocks) around policy activation for conversions.
    pub policy_transition_window_blocks: u64,
    /// Commitment tree root history length to retain.
    pub tree_roots_history_len: u64,
    /// Interval (in blocks) between frontier checkpoints.
    pub tree_frontier_checkpoint_interval: u64,
    /// Maximum active verifier entries allowed in registry.
    pub registry_max_vk_entries: u32,
    /// Maximum active parameter sets allowed in registry.
    pub registry_max_params_entries: u32,
    /// Maximum number of registry mutations allowed per block.
    pub registry_max_delta_per_block: u32,
    /// Gas schedule applied to confidential verification.
    pub gas: ConfidentialGas,
}

/// CABAC runtime mode compiled into the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CabacMode {
    /// CABAC code paths are disabled; only rANS is available.
    #[default]
    Disabled,
    /// CABAC code paths are compiled but negotiated adaptively per manifest.
    Adaptive,
    /// CABAC is forced on regardless of manifest preferences.
    Forced,
}

/// Execution backend for bundled rANS acceleration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BundleAcceleration {
    /// Always use the portable scalar implementation.
    #[default]
    None,
    /// Force CPU SIMD paths (AVX2/NEON) when bundled mode is enabled.
    CpuSimd,
    /// Route bundle kernels through the GPU acceleration hooks.
    Gpu,
}

/// Build-time codec toggles used by the runtime.
#[derive(Debug, Clone)]
pub struct StreamingCodec {
    /// CABAC runtime mode (disabled/adaptive/forced).
    pub cabac_mode: CabacMode,
    /// Trellis-enabled block sizes (empty until claim-avoidance ships).
    pub trellis_block_sizes: Vec<u16>,
    /// Path to the deterministic SignedRansTablesV1 artefact.
    pub rans_tables_path: PathBuf,
    /// Entropy mode advertised by manifests/headers.
    pub entropy_mode: EntropyMode,
    /// Bundle width used when `entropy_mode` enables bundled rANS.
    pub bundle_width: u8,
    /// Preferred acceleration backend for bundle execution.
    pub bundle_accel: BundleAcceleration,
}

impl StreamingCodec {
    /// Construct codec toggles from repository defaults.
    #[must_use]
    pub fn from_defaults() -> Self {
        assert!(
            norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
            "Bundled rANS is mandatory; rebuild with ENABLE_RANS_BUNDLES=1"
        );
        Self {
            cabac_mode: CabacMode::Disabled,
            trellis_block_sizes: defaults::streaming::codec::trellis_blocks(),
            rans_tables_path: defaults::streaming::codec::rans_tables_path(),
            entropy_mode: EntropyMode::RansBundled,
            bundle_width: defaults::streaming::codec::bundle_width(),
            bundle_accel: BundleAcceleration::None,
        }
    }
}

impl Default for StreamingCodec {
    fn default() -> Self {
        Self::from_defaults()
    }
}

/// Norito streaming configuration used by the runtime.
#[derive(Debug, Clone)]
pub struct Streaming {
    /// Node-owned key material used for streaming control-plane handshakes.
    pub key_material: StreamingKeyMaterial,
    /// Directory where streaming session snapshots are persisted.
    pub session_store_dir: PathBuf,
    /// Feature bitmask advertised during capability negotiation.
    pub feature_bits: u32,
    /// Default SoraNet integration parameters applied to streaming privacy routes.
    pub soranet: StreamingSoranet,
    /// SoraVPN provisioning spool settings for streaming routes.
    pub soravpn: StreamingSoravpn,
    /// Audio/video sync enforcement policy.
    pub sync: StreamingSync,
    /// Codec toggles (CABAC gating, trellis scopes, rANS artefact path).
    pub codec: StreamingCodec,
}

/// Runtime representation of the audio/video sync enforcement gate.
#[derive(Debug, Clone, Copy)]
pub struct StreamingSync {
    /// Enable the sync enforcement gate.
    pub enabled: bool,
    /// Observe-only mode logs violations without rejection.
    pub observe_only: bool,
    /// Minimum diagnostic window (milliseconds) required before enforcement.
    pub min_window_ms: u16,
    /// Sustained EWMA drift threshold (milliseconds).
    pub ewma_threshold_ms: u16,
    /// Hard cap for any single frame drift (milliseconds).
    pub hard_cap_ms: u16,
}

impl StreamingSync {
    /// Construct sync enforcement defaults from repository constants.
    #[must_use]
    pub fn from_defaults() -> Self {
        Self {
            enabled: defaults::streaming::sync::ENABLED,
            observe_only: defaults::streaming::sync::OBSERVE_ONLY,
            min_window_ms: defaults::streaming::sync::MIN_WINDOW_MS,
            ewma_threshold_ms: defaults::streaming::sync::EWMA_THRESHOLD_MS,
            hard_cap_ms: defaults::streaming::sync::HARD_CAP_MS,
        }
    }
}

impl Default for StreamingSync {
    fn default() -> Self {
        Self::from_defaults()
    }
}

/// SoraNet bridge defaults applied when provisioning streaming privacy routes.
#[derive(Debug, Clone)]
pub struct StreamingSoranet {
    /// Enable automatic SoraNet provisioning for streaming routes.
    pub enabled: bool,
    /// Exit relay multiaddr used when manifests omit explicit routing metadata.
    pub exit_multiaddr: String,
    /// Optional padding budget (milliseconds) applied to low-latency circuits.
    pub padding_budget_ms: Option<u16>,
    /// Access policy enforced by exit relays when bridging to Torii.
    pub access_kind: StreamingSoranetAccessKind,
    /// Domain-separated salt used to derive blinded channel identifiers.
    pub channel_salt: String,
    /// Filesystem spool where privacy-route updates are staged for exit relays.
    pub provision_spool_dir: PathBuf,
    /// Maximum on-disk footprint for the SoraNet provision spool (0 = unlimited).
    pub provision_spool_max_bytes: Bytes<u64>,
    /// Segment window (inclusive) used when provisioning privacy routes.
    pub provision_window_segments: u64,
    /// Maximum number of queued privacy-route provisioning jobs.
    pub provision_queue_capacity: u64,
}

impl StreamingSoranet {
    /// Construct SoraNet defaults using the repository constants.
    #[must_use]
    pub fn from_defaults() -> Self {
        Self {
            enabled: defaults::streaming::soranet::ENABLED,
            exit_multiaddr: defaults::streaming::soranet::EXIT_MULTIADDR.to_owned(),
            padding_budget_ms: defaults::streaming::soranet::padding_budget_ms(),
            access_kind: defaults::streaming::soranet::ACCESS_KIND
                .parse::<StreamingSoranetAccessKind>()
                .unwrap_or(StreamingSoranetAccessKind::Authenticated),
            channel_salt: defaults::streaming::soranet::CHANNEL_SALT.to_owned(),
            provision_spool_dir: PathBuf::from(defaults::streaming::soranet::PROVISION_SPOOL_DIR),
            provision_spool_max_bytes: defaults::streaming::soranet::PROVISION_SPOOL_MAX_BYTES,
            provision_window_segments: defaults::streaming::soranet::PROVISION_WINDOW_SEGMENTS,
            provision_queue_capacity: defaults::streaming::soranet::PROVISION_QUEUE_CAPACITY,
        }
    }
}

/// SoraVPN provisioning spool settings for streaming routes.
#[derive(Debug, Clone)]
pub struct StreamingSoravpn {
    /// Filesystem spool where SoraVPN route updates are staged for local VPN nodes.
    pub provision_spool_dir: PathBuf,
    /// Maximum on-disk footprint for the SoraVPN provision spool (0 = unlimited).
    pub provision_spool_max_bytes: Bytes<u64>,
}

impl StreamingSoravpn {
    /// Construct SoraVPN defaults using the repository constants.
    #[must_use]
    pub fn from_defaults() -> Self {
        Self {
            provision_spool_dir: PathBuf::from(defaults::streaming::soravpn::PROVISION_SPOOL_DIR),
            provision_spool_max_bytes: defaults::streaming::soravpn::PROVISION_SPOOL_MAX_BYTES,
        }
    }
}

/// Access stance enforced by exit relays when bridging SoraNet circuits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingSoranetAccessKind {
    /// Exit relays forward read-only content without viewer authentication.
    ReadOnly,
    /// Exit relays require viewer authentication/tickets.
    Authenticated,
}

impl StreamingSoranetAccessKind {
    /// Parse an access kind label (e.g., `authenticated`, `read-only`).
    pub fn parse_label(label: &str) -> Option<Self> {
        match label.to_ascii_lowercase().as_str() {
            "authenticated" | "auth" => Some(Self::Authenticated),
            "read-only" | "readonly" | "read_only" | "ro" => Some(Self::ReadOnly),
            _ => None,
        }
    }

    /// Render the access kind using the canonical label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Authenticated => "authenticated",
        }
    }
}

impl FromStr for StreamingSoranetAccessKind {
    type Err = ();

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        Self::parse_label(s).ok_or(())
    }
}

/// Settlement configuration (repo legs, default policies).
#[derive(Debug, Clone, Default)]
pub struct Settlement {
    /// Repo defaults.
    pub repo: Repo,
    /// Offline settlement retention policy.
    pub offline: Offline,
    /// Router configuration for XOR conversion.
    pub router: Router,
}

/// Repo governance defaults surfaced via configuration.
#[derive(Debug, Clone)]
pub struct Repo {
    /// Default haircut, in basis points.
    pub default_haircut_bps: u16,
    /// Margin frequency expressed in seconds.
    pub margin_frequency_secs: u64,
    /// Whitelisted collateral definitions accepted by default.
    pub eligible_collateral: Vec<AssetDefinitionId>,
    /// Matrix describing which collateral definitions may substitute for a recorded pledge.
    pub collateral_substitution_matrix: BTreeMap<AssetDefinitionId, Vec<AssetDefinitionId>>,
}

impl Default for Repo {
    fn default() -> Self {
        Self {
            default_haircut_bps: defaults::settlement::repo::DEFAULT_HAIRCUT_BPS,
            margin_frequency_secs: defaults::settlement::repo::DEFAULT_MARGIN_FREQUENCY_SECS,
            eligible_collateral: Vec::new(),
            collateral_substitution_matrix: BTreeMap::new(),
        }
    }
}

/// Offline settlement retention policy parameters.
#[derive(Debug, Clone)]
pub struct Offline {
    /// Minimum number of blocks to keep settlement bundles in hot storage.
    pub hot_retention_blocks: u64,
    /// Maximum number of bundles to archive per retention pass.
    pub archive_batch_size: usize,
    /// Minimum number of blocks archived bundles remain available before pruning (0 disables pruning).
    pub cold_retention_blocks: u64,
    /// Maximum number of archived bundles pruned per retention pass.
    pub prune_batch_size: usize,
    /// Aggregate-proof enforcement mode for offline bundles.
    pub proof_mode: OfflineProofMode,
    /// Maximum age for offline receipts (0 disables age checks).
    pub max_receipt_age: Duration,
    /// Optional DER-encoded trust anchors appended to the built-in Android root set.
    pub android_trust_anchors: Vec<Vec<u8>>,
}

impl Default for Offline {
    fn default() -> Self {
        Self {
            hot_retention_blocks: defaults::settlement::offline::HOT_RETENTION_BLOCKS,
            archive_batch_size: defaults::settlement::offline::ARCHIVE_BATCH_SIZE,
            cold_retention_blocks: defaults::settlement::offline::COLD_RETENTION_BLOCKS,
            prune_batch_size: defaults::settlement::offline::PRUNE_BATCH_SIZE,
            proof_mode: OfflineProofMode::Optional,
            max_receipt_age: Duration::from_millis(
                defaults::settlement::offline::MAX_RECEIPT_AGE_MS,
            ),
            android_trust_anchors: Vec::new(),
        }
    }
}

/// Offline aggregate-proof enforcement modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineProofMode {
    /// Accept bundles without aggregate proofs (proofs verified when present).
    Optional,
    /// Require bundles to carry aggregate proofs.
    Required,
}

impl OfflineProofMode {
    /// Canonical config label for this mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Optional => "optional",
            Self::Required => "required",
        }
    }
}

/// Router configuration controlling shadow-price and buffer guard rails.
#[derive(Debug, Clone, Copy)]
pub struct Router {
    /// TWAP window used during settlement conversion.
    pub twap_window: Duration,
    /// Base epsilon safety margin (basis points).
    pub epsilon_bps: u16,
    /// Buffer alert threshold percentage.
    pub buffer_alert_pct: u8,
    /// Buffer throttle threshold percentage.
    pub buffer_throttle_pct: u8,
    /// Buffer XOR-only threshold percentage.
    pub buffer_xor_only_pct: u8,
    /// Buffer halt threshold percentage.
    pub buffer_halt_pct: u8,
    /// Buffer coverage horizon (hours).
    pub buffer_horizon_hours: u16,
}

impl Default for Router {
    fn default() -> Self {
        Self {
            twap_window: Duration::from_secs(defaults::settlement::router::TWAP_WINDOW_SECS),
            epsilon_bps: defaults::settlement::router::EPSILON_BPS,
            buffer_alert_pct: defaults::settlement::router::ALERT_PCT,
            buffer_throttle_pct: defaults::settlement::router::THROTTLE_PCT,
            buffer_xor_only_pct: defaults::settlement::router::XOR_ONLY_PCT,
            buffer_halt_pct: defaults::settlement::router::HALT_PCT,
            buffer_horizon_hours: defaults::settlement::router::BUFFER_HORIZON_HOURS,
        }
    }
}

/// Supported curves for Halo2 verification.
#[derive(Debug, Clone, Copy)]
pub enum ZkCurve {
    /// Toy additive prime field backend (testing only).
    Pallas,
    /// Pasta (Pallas/Vesta) — reserved for future backends.
    Pasta,
    /// Goldilocks multiplicative backend.
    Goldilocks,
    /// BN254 — reserved for future backends.
    Bn254,
}

/// Halo2 transparent backend kind.
#[derive(Debug, Clone, Copy)]
pub enum Halo2Backend {
    /// Inner-Product Argument (transparent PCS).
    Ipa,
}

/// Halo2 transparent verification settings.
#[derive(Debug, Clone, Copy)]
pub struct Halo2 {
    /// Enable Halo2 verification in hosts.
    pub enabled: bool,
    /// Selected curve backend.
    pub curve: ZkCurve,
    /// Transparent PCS backend.
    pub backend: Halo2Backend,
    /// Maximum circuit size exponent (N = 2^k) accepted for verification.
    pub max_k: u32,
    /// Soft time budget for a single verification (ms).
    pub verifier_budget_ms: u64,
    /// Maximum number of proofs allowed in a batch verification.
    pub verifier_max_batch: u32,
    /// Maximum accepted Norito envelope payload length (bytes).
    pub max_envelope_bytes: usize,
    /// Maximum accepted proof payload length (bytes).
    pub max_proof_bytes: usize,
    /// Maximum allowed transcript label length (bytes).
    pub max_transcript_label_len: usize,
    /// Require transcript labels to be ASCII.
    pub enforce_transcript_label_ascii: bool,
}

impl Default for Halo2 {
    fn default() -> Self {
        Self {
            enabled: crate::parameters::defaults::zk::halo2::ENABLED,
            curve: ZkCurve::Pallas,
            backend: Halo2Backend::Ipa,
            max_k: crate::parameters::defaults::zk::halo2::MAX_K,
            verifier_budget_ms: crate::parameters::defaults::zk::halo2::VERIFIER_BUDGET_MS,
            verifier_max_batch: crate::parameters::defaults::zk::halo2::VERIFIER_MAX_BATCH,
            max_envelope_bytes: crate::parameters::defaults::zk::halo2::MAX_ENVELOPE_BYTES,
            max_proof_bytes: crate::parameters::defaults::zk::halo2::MAX_PROOF_BYTES,
            max_transcript_label_len:
                crate::parameters::defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN,
            enforce_transcript_label_ascii:
                crate::parameters::defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII,
        }
    }
}

/// Telemetry capability flags derived from a [`TelemetryProfile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryCapabilities {
    metrics: bool,
    expensive_metrics: bool,
    developer_outputs: bool,
}

impl TelemetryCapabilities {
    /// Construct a capability set from individual flags.
    #[must_use]
    pub const fn new(metrics: bool, expensive_metrics: bool, developer_outputs: bool) -> Self {
        Self {
            metrics,
            expensive_metrics,
            developer_outputs,
        }
    }

    /// Return a capability set with all flags disabled.
    #[must_use]
    pub const fn disabled() -> Self {
        Self::new(false, false, false)
    }

    /// Return whether lightweight metrics instrumentation is enabled.
    #[inline]
    #[must_use]
    pub const fn metrics_enabled(self) -> bool {
        self.metrics
    }

    /// Return whether expensive/prometheus metrics instrumentation is enabled.
    #[inline]
    #[must_use]
    pub const fn expensive_metrics_enabled(self) -> bool {
        self.expensive_metrics
    }

    /// Return whether developer-only telemetry outputs are enabled.
    #[inline]
    #[must_use]
    pub const fn developer_outputs_enabled(self) -> bool {
        self.developer_outputs
    }

    /// Override the metrics-enabled flag while preserving other capabilities.
    #[must_use]
    pub const fn with_metrics(self, metrics: bool) -> Self {
        Self::new(metrics, self.expensive_metrics, self.developer_outputs)
    }

    /// Combine two capability sets using logical OR for each flag.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            metrics: self.metrics || other.metrics,
            expensive_metrics: self.expensive_metrics || other.expensive_metrics,
            developer_outputs: self.developer_outputs || other.developer_outputs,
        }
    }
}

/// Telemetry profiles describing high-level capability bundles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryProfile {
    /// Telemetry is disabled entirely.
    Disabled,
    /// Enable lightweight operator metrics and status endpoints.
    Operator,
    /// Enable operator metrics plus expensive instrumentation (e.g., Prometheus exporters).
    Extended,
    /// Enable operator metrics plus developer-only sinks (JSON/file outputs, tracing bridges).
    Developer,
    /// Enable all telemetry capabilities supported by the build.
    Full,
}

impl TelemetryProfile {
    /// Compute the capability set implied by this profile.
    #[must_use]
    pub const fn capabilities(self) -> TelemetryCapabilities {
        match self {
            Self::Disabled => TelemetryCapabilities::disabled(),
            Self::Operator => TelemetryCapabilities::new(true, false, false),
            Self::Extended => TelemetryCapabilities::new(true, true, false),
            Self::Developer => TelemetryCapabilities::new(true, false, true),
            Self::Full => TelemetryCapabilities::new(true, true, true),
        }
    }

    /// Return `true` when this profile disables all telemetry outputs.
    #[inline]
    #[must_use]
    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }
}

impl From<user::TelemetryProfile> for TelemetryProfile {
    fn from(profile: user::TelemetryProfile) -> Self {
        match profile {
            user::TelemetryProfile::Disabled => Self::Disabled,
            user::TelemetryProfile::Operator => Self::Operator,
            user::TelemetryProfile::Extended => Self::Extended,
            user::TelemetryProfile::Developer => Self::Developer,
            user::TelemetryProfile::Full => Self::Full,
        }
    }
}

impl Default for TelemetryCapabilities {
    fn default() -> Self {
        Self::disabled()
    }
}

impl From<TelemetryProfile> for TelemetryCapabilities {
    fn from(profile: TelemetryProfile) -> Self {
        profile.capabilities()
    }
}

/// Telemetry redaction modes (runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryRedactionMode {
    /// Redact all sensitive fields; ignore allow-list entries.
    Strict,
    /// Redact sensitive fields unless explicitly allow-listed.
    Allowlist,
    /// Disable telemetry redaction (developer-only).
    Disabled,
}

impl TelemetryRedactionMode {
    /// Return whether redaction is disabled.
    #[inline]
    #[must_use]
    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    /// Return whether allow-list entries may bypass keyword redaction.
    #[inline]
    #[must_use]
    pub const fn allowlist_enabled(self) -> bool {
        matches!(self, Self::Allowlist)
    }
}

impl From<user::TelemetryRedactionMode> for TelemetryRedactionMode {
    fn from(mode: user::TelemetryRedactionMode) -> Self {
        match mode {
            user::TelemetryRedactionMode::Strict => Self::Strict,
            user::TelemetryRedactionMode::Allowlist => Self::Allowlist,
            user::TelemetryRedactionMode::Disabled => Self::Disabled,
        }
    }
}

/// Telemetry redaction policy.
#[derive(Debug, Clone)]
pub struct TelemetryRedaction {
    /// Redaction mode.
    pub mode: TelemetryRedactionMode,
    /// Allow-list of field names exempted from keyword redaction.
    pub allowlist: Vec<String>,
}

impl Default for TelemetryRedaction {
    fn default() -> Self {
        Self {
            mode: TelemetryRedactionMode::Strict,
            allowlist: Vec::new(),
        }
    }
}

/// Telemetry integrity policy (hash chaining + optional signing key).
#[derive(Debug, Clone)]
pub struct TelemetryIntegrity {
    /// Enable hash-chained telemetry exports.
    pub enabled: bool,
    /// Optional directory for integrity state snapshots.
    pub state_dir: Option<PathBuf>,
    /// Optional signing key for keyed hashes.
    pub signing_key: Option<[u8; 32]>,
    /// Optional key identifier for rotation workflows.
    pub signing_key_id: Option<String>,
}

impl Default for TelemetryIntegrity {
    fn default() -> Self {
        Self {
            enabled: defaults::telemetry::integrity::ENABLED,
            state_dir: None,
            signing_key: None,
            signing_key_id: None,
        }
    }
}

/// Complete configuration needed to start regular telemetry.
#[derive(Debug, Clone)]
pub struct Telemetry {
    /// Telemetry sink name.
    pub name: String,
    /// Telemetry endpoint URL.
    pub url: Url,
    /// Minimum retry period on failure.
    pub min_retry_period: Duration,
    /// Exponent for exponential backoff upper bound.
    pub max_retry_delay_exponent: u8,
    /// Optional Telegram bot key for alerts.
    pub telegram_bot_key: Option<String>,
    /// Optional Telegram chat ID for alerts.
    pub telegram_chat_id: Option<String>,
    /// Optional minimum level for Telegram alerts (e.g., "WARN", "ERROR").
    pub telegram_min_level: Option<String>,
    /// Optional list of target prefixes to include (e.g., ["p2p", "network"]). If empty or None, include all.
    pub telegram_targets: Option<Vec<String>>,
    /// Optional alerts rate limit (messages per minute).
    pub telegram_rate_per_minute: Option<NonZeroU32>,
    /// Include a metrics snapshot in alerts.
    pub telegram_include_metrics: bool,
    /// Optional metrics URL to poll (e.g., http://127.0.0.1:8080/metrics).
    pub telegram_metrics_url: Option<Url>,
    /// Optional metrics poll period.
    pub telegram_metrics_period: Option<Duration>,
    /// Optional allow-list of `msg` kinds to send.
    pub telegram_allow_kinds: Option<Vec<String>>,
    /// Optional deny-list of `msg` kinds to suppress.
    pub telegram_deny_kinds: Option<Vec<String>>,
}

/// Network Time Service (NTS) configuration.
#[derive(Debug, Clone, Copy)]
pub struct Nts {
    /// Sampling interval for peer time probes.
    pub sample_interval: Duration,
    /// Maximum peers to sample per round.
    pub sample_cap_per_round: usize,
    /// Maximum acceptable round-trip time (milliseconds) for samples.
    pub max_rtt_ms: u64,
    /// Trim percent for median aggregation (0–45 allowed; 10 typical).
    pub trim_percent: u8,
    /// Per-peer ring buffer capacity for samples.
    pub per_peer_buffer: usize,
    /// Enable EMA smoothing of network offset.
    pub smoothing_enabled: bool,
    /// EMA alpha in [0,1]; higher means more responsive.
    pub smoothing_alpha: f64,
    /// Maximum allowed adjustment per minute (ms) when smoothing.
    pub max_adjust_ms_per_min: u64,
    /// Minimum number of peer samples required before NTS is considered healthy.
    pub min_samples: usize,
    /// Maximum absolute offset (ms) allowed before NTS is considered unhealthy (0 disables).
    pub max_offset_ms: u64,
    /// Maximum confidence (MAD) in ms allowed before NTS is considered unhealthy (0 disables).
    pub max_confidence_ms: u64,
    /// Enforcement mode for unhealthy NTS.
    pub enforcement_mode: NtsEnforcementMode,
}

/// Enforcement modes for unhealthy NTS during admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtsEnforcementMode {
    /// Log unhealthy NTS status but accept time-sensitive transactions.
    Warn,
    /// Reject time-sensitive transactions when NTS is unhealthy.
    Reject,
}

impl NtsEnforcementMode {
    /// Canonical config label for this mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Reject => "reject",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use iroha_data_model::nexus::{
        LaneCatalog, LaneConfig as LaneConfigMetadata, LaneId, LaneVisibility,
    };
    use iroha_primitives::{addr::socket_addr, unique_vec};

    use super::*;

    fn dummy_peer(port: u16) -> Peer {
        Peer::new(
            socket_addr!(127.0.0.1:port),
            KeyPair::random().into_parts().0,
        )
    }

    #[test]
    fn torii_operator_auth_defaults_match_expected() {
        let auth = ToriiOperatorAuth::default();
        assert!(matches!(
            auth.token_fallback,
            OperatorTokenFallback::Bootstrap
        ));
        assert!(matches!(
            auth.token_source,
            OperatorTokenSource::OperatorTokens
        ));
    }

    #[test]
    fn concurrency_validate_rejects_zero_stacks() {
        let mut cfg = Concurrency::from_defaults();
        cfg.scheduler_stack_bytes = 0;

        let err = cfg.validate().expect_err("zero stack should be invalid");
        assert!(matches!(
            err.current_context(),
            ParseError::InvalidConcurrencyConfig
        ));
    }

    #[test]
    fn concurrency_validate_rejects_zero_gas_stack_multiplier() {
        let mut cfg = Concurrency::from_defaults();
        cfg.gas_to_stack_multiplier = 0;

        let err = cfg
            .validate()
            .expect_err("zero multiplier should be invalid");
        assert!(matches!(
            err.current_context(),
            ParseError::InvalidConcurrencyConfig
        ));
    }

    #[test]
    fn concurrency_validate_rejects_excessive_guest_stack() {
        let mut cfg = Concurrency::from_defaults();
        cfg.guest_stack_bytes = defaults::concurrency::GUEST_STACK_BYTES_MAX + 1;

        let err = cfg
            .validate()
            .expect_err("guest stack beyond max must fail");
        assert!(matches!(
            err.current_context(),
            ParseError::InvalidConcurrencyConfig
        ));
    }

    #[test]
    fn concurrency_validate_accepts_defaults() {
        assert!(Concurrency::from_defaults().validate().is_ok());
    }

    #[test]
    fn lane_config_uses_metadata_shard_id() {
        let mut metadata = BTreeMap::new();
        metadata.insert("da_shard_id".to_string(), "9".to_string());
        let catalog = LaneCatalog::new(
            NonZeroU32::new(6).expect("lane count"),
            vec![LaneConfigMetadata {
                id: LaneId::new(5),
                alias: "lane5".into(),
                metadata,
                ..LaneConfigMetadata::default()
            }],
        )
        .expect("lane catalog");

        let config = LaneConfig::from_catalog(&catalog);
        let entry = config.entry(LaneId::new(5)).expect("lane entry");
        assert_eq!(entry.shard_id, 9);
        assert_eq!(config.shard_id(LaneId::new(5)), 9);
    }

    #[test]
    fn shard_mapping_exposes_lane_binding() {
        let mut metadata = BTreeMap::new();
        metadata.insert("da_shard_id".to_string(), "7".to_string());
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("lane count"),
            vec![
                LaneConfigMetadata {
                    id: LaneId::new(0),
                    alias: "lane1".into(),
                    metadata,
                    ..LaneConfigMetadata::default()
                },
                LaneConfigMetadata {
                    id: LaneId::new(1),
                    alias: "lane2".into(),
                    ..LaneConfigMetadata::default()
                },
            ],
        )
        .expect("lane catalog");

        let config = LaneConfig::from_catalog(&catalog);
        assert_eq!(config.shard_id(LaneId::new(0)), 7);
        assert_eq!(config.shard_id(LaneId::new(1)), 1);
    }

    #[test]
    fn shard_defaults_to_lane_id_when_metadata_missing() {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(4).expect("lane count"),
            vec![LaneConfigMetadata {
                id: LaneId::new(3),
                alias: "lane3".into(),
                ..LaneConfigMetadata::default()
            }],
        )
        .expect("lane catalog");

        let config = LaneConfig::from_catalog(&catalog);
        let entry = config.entry(LaneId::new(3)).expect("lane entry");
        assert_eq!(entry.shard_id, 3);
    }

    #[test]
    fn sorafs_anonymity_stage_parses_aliases() {
        assert_eq!(
            SorafsAnonymityStage::parse("stage_a"),
            Some(SorafsAnonymityStage::GuardPq)
        );
        assert_eq!(
            SorafsAnonymityStage::parse("anon-majority-pq"),
            Some(SorafsAnonymityStage::MajorityPq)
        );
        assert_eq!(
            SorafsAnonymityStage::parse("Stage-C"),
            Some(SorafsAnonymityStage::StrictPq)
        );
        assert_eq!(SorafsAnonymityStage::parse("anon-unknown"), None);
        assert_eq!(SorafsAnonymityStage::parse("stage_0"), None);
        assert_eq!(SorafsAnonymityStage::parse("anon_compatible"), None);
        assert_eq!(SorafsAnonymityStage::parse("unknown-stage"), None);
    }

    #[test]
    fn sorafs_anonymity_stage_labels_are_canonical() {
        assert_eq!(SorafsAnonymityStage::GuardPq.label(), "anon-guard-pq");
        assert_eq!(SorafsAnonymityStage::MajorityPq.label(), "anon-majority-pq");
        assert_eq!(SorafsAnonymityStage::StrictPq.label(), "anon-strict-pq");
    }

    #[test]
    fn sorafs_rollout_phase_parses_aliases() {
        assert_eq!(
            SorafsRolloutPhase::parse("stage-a"),
            Some(SorafsRolloutPhase::Canary)
        );
        assert_eq!(
            SorafsRolloutPhase::parse("majority"),
            Some(SorafsRolloutPhase::Ramp)
        );
        assert_eq!(
            SorafsRolloutPhase::parse("GA"),
            Some(SorafsRolloutPhase::Default)
        );
        assert_eq!(SorafsRolloutPhase::parse("unknown-rollout"), None);
    }

    #[test]
    fn sorafs_gateway_effective_anonymity_policy_respects_phase_fallback() {
        let mut gateway = SorafsGateway::default();
        assert_eq!(
            gateway.effective_anonymity_policy(),
            SorafsAnonymityStage::GuardPq
        );

        gateway.anonymity_policy = None;
        gateway.rollout_phase = SorafsRolloutPhase::Default;
        assert_eq!(
            gateway.effective_anonymity_policy(),
            SorafsAnonymityStage::StrictPq
        );

        gateway.anonymity_policy = Some(SorafsAnonymityStage::MajorityPq);
        assert_eq!(
            gateway.effective_anonymity_policy(),
            SorafsAnonymityStage::MajorityPq
        );
    }

    #[test]
    fn streaming_codec_default_entropy_mode_matches_build_flag() {
        assert!(
            norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
            "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
        );
        let codec = StreamingCodec::from_defaults();
        assert_eq!(codec.entropy_mode, EntropyMode::RansBundled);
    }

    #[test]
    fn streaming_default_entropy_string_tracks_build_flag() {
        assert!(
            norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
            "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
        );
        let default = defaults::streaming::codec::entropy_mode();
        assert_eq!(
            default,
            defaults::streaming::codec::BUNDLED_ENTROPY_MODE,
            "string helper should mirror bundled availability"
        );
    }

    #[test]
    fn soranet_pow_defaults_are_const_initializable() {
        const CONST_POW: SoranetPow = SoranetPow::default_const();
        const CONST_PUZZLE: SoranetPuzzle = SoranetPuzzle::default_const();

        let runtime = SoranetPow::default();
        assert_eq!(CONST_POW.required, runtime.required);
        assert_eq!(CONST_POW.difficulty, runtime.difficulty);
        assert_eq!(CONST_POW.max_future_skew, runtime.max_future_skew);
        assert_eq!(CONST_POW.min_ticket_ttl, runtime.min_ticket_ttl);
        assert_eq!(CONST_POW.ticket_ttl, runtime.ticket_ttl);
        assert_eq!(
            CONST_POW.revocation_store_capacity,
            runtime.revocation_store_capacity
        );
        assert_eq!(CONST_POW.revocation_max_ttl, runtime.revocation_max_ttl);
        assert_eq!(
            CONST_POW.revocation_store_path,
            runtime.revocation_store_path
        );
        assert_eq!(CONST_POW.puzzle.is_some(), runtime.puzzle.is_some());

        let runtime_puzzle = runtime.puzzle.expect("puzzle present by default");
        assert_eq!(CONST_PUZZLE.memory_kib, runtime_puzzle.memory_kib);
        assert_eq!(CONST_PUZZLE.time_cost, runtime_puzzle.time_cost);
        assert_eq!(CONST_PUZZLE.lanes, runtime_puzzle.lanes);
    }

    #[test]
    fn no_trusted_peers() {
        let value = TrustedPeers {
            myself: dummy_peer(80),
            others: unique_vec![],
            pops: std::collections::BTreeMap::default(),
        };
        assert!(!value.contains_other_trusted_peers());
    }

    #[test]
    fn one_trusted_peer() {
        let value = TrustedPeers {
            myself: dummy_peer(80),
            others: unique_vec![dummy_peer(81)],
            pops: std::collections::BTreeMap::default(),
        };
        assert!(value.contains_other_trusted_peers());
    }

    #[test]
    fn many_trusted_peers() {
        let value = TrustedPeers {
            myself: dummy_peer(80),
            others: unique_vec![dummy_peer(1), dummy_peer(2), dummy_peer(3), dummy_peer(4),],
            pops: std::collections::BTreeMap::default(),
        };
        assert!(value.contains_other_trusted_peers());
    }

    #[test]
    fn telemetry_profile_capabilities_match_expectations() {
        let disabled = TelemetryProfile::Disabled.capabilities();
        assert!(!disabled.metrics_enabled());
        assert!(!disabled.expensive_metrics_enabled());
        assert!(!disabled.developer_outputs_enabled());

        let operator = TelemetryProfile::Operator.capabilities();
        assert!(operator.metrics_enabled());
        assert!(!operator.expensive_metrics_enabled());
        assert!(!operator.developer_outputs_enabled());

        let full = TelemetryProfile::Full.capabilities();
        assert!(full.metrics_enabled());
        assert!(full.expensive_metrics_enabled());
        assert!(full.developer_outputs_enabled());

        let combined = TelemetryCapabilities::from(TelemetryProfile::Developer)
            .union(TelemetryCapabilities::from(TelemetryProfile::Extended));
        assert!(combined.metrics_enabled());
        assert!(combined.expensive_metrics_enabled());
        assert!(combined.developer_outputs_enabled());
    }

    #[test]
    fn telemetry_profile_from_user_enum_round_trips() {
        use super::user;

        assert_eq!(
            TelemetryProfile::from(user::TelemetryProfile::Operator),
            TelemetryProfile::Operator
        );
        assert_eq!(
            TelemetryProfile::from(user::TelemetryProfile::Extended),
            TelemetryProfile::Extended
        );
        assert_eq!(
            TelemetryProfile::from(user::TelemetryProfile::Developer),
            TelemetryProfile::Developer
        );
        assert_eq!(
            TelemetryProfile::from(user::TelemetryProfile::Full),
            TelemetryProfile::Full
        );
    }

    #[test]
    fn fraud_monitoring_new_dedup_and_defaults() {
        use url::Url;
        let url = Url::parse("https://risk.example/api").expect("url");
        let cfg = FraudMonitoring::new(
            true,
            vec![url.clone(), url.clone()],
            Duration::from_millis(0),
            Duration::from_millis(0),
            5,
            Some(FraudRiskBand::High),
            Vec::new(),
        );
        assert_eq!(cfg.service_endpoints.len(), 1);
        assert_eq!(cfg.service_endpoints[0], url);
        assert_eq!(
            cfg.connect_timeout,
            defaults::fraud_monitoring::CONNECT_TIMEOUT
        );
        assert_eq!(
            cfg.request_timeout,
            defaults::fraud_monitoring::REQUEST_TIMEOUT
        );
        assert_eq!(cfg.missing_assessment_grace, Duration::from_secs(5));
        assert_eq!(cfg.required_minimum_band, Some(FraudRiskBand::High));
    }

    #[test]
    fn fraud_monitoring_default_matches_defaults() {
        let cfg = FraudMonitoring::default();
        assert!(!cfg.enabled);
        assert!(cfg.service_endpoints.is_empty());
        assert_eq!(
            cfg.connect_timeout,
            defaults::fraud_monitoring::CONNECT_TIMEOUT
        );
        assert_eq!(
            cfg.request_timeout,
            defaults::fraud_monitoring::REQUEST_TIMEOUT
        );
        assert_eq!(
            cfg.missing_assessment_grace,
            Duration::from_secs(defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS,)
        );
        assert!(cfg.required_minimum_band.is_none());
        assert!(cfg.attesters.is_empty());
    }

    #[test]
    fn lane_config_derives_storage_geometry() {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                LaneConfigMetadata::default(),
                LaneConfigMetadata {
                    id: LaneId::new(1),
                    alias: "Public Lane ①".to_string(),
                    lane_type: Some("default_public".to_string()),
                    governance: Some("parliament".to_string()),
                    ..LaneConfigMetadata::default()
                },
            ],
        )
        .expect("catalog");

        let config = LaneConfig::from_catalog(&catalog);
        let entries = config.entries();
        assert_eq!(entries.len(), 2);

        let default_entry = config.entry(LaneId::SINGLE).expect("default lane exists");
        assert_eq!(default_entry.alias, "default");
        assert_eq!(default_entry.slug, "default");
        assert_eq!(default_entry.kura_segment, "lane_000_default");
        assert_eq!(default_entry.merge_segment, "lane_000_default_merge");
        assert_eq!(
            default_entry.key_prefix,
            LaneId::SINGLE.as_u32().to_be_bytes()
        );
        assert_eq!(default_entry.dataspace_id, DataSpaceId::GLOBAL);
        assert_eq!(default_entry.visibility, LaneVisibility::Public);
        assert_eq!(
            default_entry.storage_profile,
            LaneStorageProfile::FullReplica
        );

        let public_entry = config.entry(LaneId::new(1)).expect("lane 1 exists");
        assert_eq!(public_entry.alias, "Public Lane ①");
        assert_eq!(public_entry.slug, "public_lane");
        assert_eq!(public_entry.kura_segment, "lane_001_public_lane");
        assert_eq!(public_entry.merge_segment, "lane_001_public_lane_merge");
        assert_eq!(
            public_entry.key_prefix,
            LaneId::new(1).as_u32().to_be_bytes()
        );
        assert_eq!(public_entry.dataspace_id, DataSpaceId::GLOBAL);
        assert_eq!(public_entry.visibility, LaneVisibility::Public);
        assert_eq!(
            public_entry.storage_profile,
            LaneStorageProfile::FullReplica
        );
    }
}

/// IVM/runtime presentation toggles.
#[derive(Debug, Clone)]
pub struct Ivm {
    /// Compute resource profile name used to cap IVM guest stack budgets.
    pub memory_budget_profile: Name,
    /// Banner/presentation toggles surfaced during startup.
    pub banner: Banner,
}

impl Ivm {
    /// Construct an `Ivm` configuration from repository defaults.
    #[must_use]
    pub fn from_defaults() -> Self {
        Self {
            memory_budget_profile: defaults::ivm::memory_budget_profile(),
            banner: Banner::from_defaults(),
        }
    }
}

impl Default for Ivm {
    fn default() -> Self {
        Self::from_defaults()
    }
}

/// Startup banner settings.
#[derive(Debug, Clone, Copy)]
pub struct Banner {
    /// Whether to print the Norito/IVM startup banner on daemon launch.
    pub show: bool,
    /// Whether to play the retro startup tune when compiled with the `beep` feature.
    pub beep: bool,
}

impl Banner {
    /// Construct banner settings from repository defaults.
    #[must_use]
    pub const fn from_defaults() -> Self {
        Self {
            show: defaults::ivm::banner::show(),
            beep: defaults::ivm::banner::beep(),
        }
    }
}

impl Default for Banner {
    fn default() -> Self {
        Self::from_defaults()
    }
}

/// Norito codec configuration (actual layer).
///
/// These knobs describe the canonical serialization layout and compression
/// thresholds baked into the Norito codec. Runtime overrides are ignored in
/// favour of the compiled profile; configuration values should match the
/// defaults unless the node ships a custom Norito build.
#[derive(Debug, Clone, Copy)]
pub struct Norito {
    /// Minimum payload size in bytes before attempting CPU Zstd compression.
    pub min_compress_bytes_cpu: usize,
    /// Minimum payload size in bytes before attempting GPU Zstd compression
    /// when a GPU backend is compiled and available.
    pub min_compress_bytes_gpu: usize,
    /// Zstd compression level for small/medium payloads.
    pub zstd_level_small: i32,
    /// Zstd compression level for large payloads.
    pub zstd_level_large: i32,
    /// Zstd compression level for GPU offload.
    pub zstd_level_gpu: i32,
    /// Size threshold that separates small and large CPU payloads for level selection.
    pub large_threshold: usize,
    /// Allow GPU compression offload when compiled and available.
    pub allow_gpu_compression: bool,
    /// Maximum allowed Norito archive length in bytes (0 = unlimited).
    pub max_archive_len: u64,
    /// Small-N threshold for AoS vs NCB adaptive selection in Norito columnar helpers.
    pub aos_ncb_small_n: usize,
}

/// Hijiri configuration (actual layer).
#[derive(Debug, Clone)]
pub struct Hijiri {
    /// Optional fee policy derived from Hijiri risk scores.
    pub fee_policy: Option<ModelHijiriFeePolicy>,
}

impl Hijiri {
    /// Construct a new Hijiri configuration.
    pub fn new(fee_policy: Option<ModelHijiriFeePolicy>) -> Self {
        Self { fee_policy }
    }
}

/// Severity bands reported by the fraud-monitoring service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FraudRiskBand {
    /// Lowest severity rating produced by the external assessor.
    Low,
    /// Medium severity rating indicating elevated but tolerable risk.
    Medium,
    /// High severity rating signalling transactions that should be halted.
    High,
    /// Critical severity threshold reserved for outright blocking decisions.
    Critical,
}

impl FraudRiskBand {
    /// Return the canonical lowercase string representation expected by config files.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl core::fmt::Display for FraudRiskBand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl core::str::FromStr for FraudRiskBand {
    type Err = ();

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(()),
        }
    }
}

impl From<super::user::FraudRiskBand> for FraudRiskBand {
    fn from(value: super::user::FraudRiskBand) -> Self {
        match value {
            super::user::FraudRiskBand::Low => Self::Low,
            super::user::FraudRiskBand::Medium => Self::Medium,
            super::user::FraudRiskBand::High => Self::High,
            super::user::FraudRiskBand::Critical => Self::Critical,
        }
    }
}

/// Registered assessment attester (engine id + public key).
#[derive(Debug, Clone)]
pub struct FraudAttester {
    /// Deterministic identifier of the scoring engine / attester.
    pub engine_id: String,
    /// Public key used to verify assessment signatures.
    pub public_key: PublicKey,
}

impl FraudAttester {
    /// Create a trimmed engine identifier for metrics/logging.
    #[must_use]
    pub fn engine_label(&self) -> &str {
        let trimmed = self.engine_id.trim();
        if trimmed.is_empty() {
            "unknown"
        } else {
            trimmed
        }
    }
}

/// Strongly typed fraud-monitoring configuration used by the host.
#[derive(Debug, Clone)]
pub struct FraudMonitoring {
    /// Master switch controlling whether fraud assessments gate admission.
    pub enabled: bool,
    /// Ordered list of HTTP endpoints queried for fraud assessments.
    pub service_endpoints: Vec<Url>,
    /// Timeout applied to the initial TCP connection attempt.
    pub connect_timeout: Duration,
    /// Timeout applied to the full HTTP request, including body transfer.
    pub request_timeout: Duration,
    /// Grace period after which missing assessments trigger warnings or fallback.
    pub missing_assessment_grace: Duration,
    /// Minimum severity band required for admission; `None` disables gating.
    pub required_minimum_band: Option<FraudRiskBand>,
    /// Registered assessment attesters whose signatures must validate assessments.
    pub attesters: Vec<FraudAttester>,
}

impl FraudMonitoring {
    #[allow(clippy::too_many_arguments)]
    /// Construct a [`FraudMonitoring`] instance from validated user parameters.
    pub fn new(
        enabled: bool,
        service_endpoints: Vec<Url>,
        connect_timeout: Duration,
        request_timeout: Duration,
        missing_assessment_grace_secs: u64,
        required_minimum_band: Option<FraudRiskBand>,
        attesters: Vec<FraudAttester>,
    ) -> Self {
        let mut deduped = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for endpoint in service_endpoints {
            let key = endpoint.as_str().to_owned();
            if seen.insert(key) {
                deduped.push(endpoint);
            }
        }
        let connect_timeout = if connect_timeout.is_zero() {
            defaults::fraud_monitoring::CONNECT_TIMEOUT
        } else {
            connect_timeout
        };
        let request_timeout = if request_timeout.is_zero() {
            defaults::fraud_monitoring::REQUEST_TIMEOUT
        } else {
            request_timeout
        };
        let missing_assessment_grace = Duration::from_secs(missing_assessment_grace_secs);
        let mut attesters: Vec<FraudAttester> = attesters
            .into_iter()
            .filter_map(|mut attester| {
                let trimmed = attester.engine_id.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let mut normalized = trimmed.to_ascii_lowercase();
                if normalized.is_empty() {
                    normalized = trimmed.to_string();
                }
                attester.engine_id = normalized;
                Some(attester)
            })
            .collect();
        attesters.sort_by(|a, b| a.engine_id.cmp(&b.engine_id));
        attesters.dedup_by(|a, b| a.engine_id == b.engine_id);
        Self {
            enabled,
            service_endpoints: deduped,
            connect_timeout,
            request_timeout,
            missing_assessment_grace,
            required_minimum_band,
            attesters,
        }
    }

    /// Return the registered attester for the provided engine identifier.
    #[must_use]
    pub fn attester(&self, engine_id: &str) -> Option<&FraudAttester> {
        let key = engine_id.trim();
        if key.is_empty() {
            return None;
        }
        self.attesters
            .iter()
            .find(|attester| attester.engine_id == key)
    }
}

impl Default for FraudMonitoring {
    fn default() -> Self {
        Self {
            enabled: defaults::fraud_monitoring::ENABLED,
            service_endpoints: Vec::new(),
            connect_timeout: defaults::fraud_monitoring::CONNECT_TIMEOUT,
            request_timeout: defaults::fraud_monitoring::REQUEST_TIMEOUT,
            missing_assessment_grace: Duration::from_secs(
                defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS,
            ),
            required_minimum_band: None,
            attesters: Vec::new(),
        }
    }
}

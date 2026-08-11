//! Iroha node executable, startup wiring, and command-line interface.

/// Feature-isolated real-network consensus fault-injection control.
#[cfg(feature = "test-network-message-control")]
mod consensus_message_control;
/// Iroha server command-line interface and node bootstrap entrypoint.
mod i18n;
/// Deployment-injected factory for the supervised private Musubi publication service.
pub mod musubi_publication_service;
/// Asynchronous Nexus DPN fee settlement relay.
mod nexus_fee_relay_worker;
/// Platform-fixed local runtime-provider broker used by the stock launcher.
mod runtime_provider_broker;
/// Deployment-owned runtime-provider registry boundary for the standard launcher.
pub mod runtime_provider_registry;
/// Exact external credential boundary for authenticated Hugging Face inference.
pub mod soracloud_hf_credential;
/// Embedded Soracloud runtime-manager reconciliation.
#[cfg(feature = "embedded-soracloud-runtime")]
#[path = "soracloud_runtime.rs"]
mod soracloud_runtime;
/// No-op Soracloud runtime used when the full embedded runtime is disabled.
#[cfg(not(feature = "embedded-soracloud-runtime"))]
#[path = "soracloud_runtime_stub.rs"]
mod soracloud_runtime;
/// Exact external signer boundary for Soracloud runtime mutations.
pub mod soracloud_runtime_signer;
/// Supervised committed `SoraFS` hedging/billing projector and delivery worker.
pub mod sorafs_hedging_billing_runtime;
/// Fail-closed config-bound `SoraFS` `PoP` runtime construction.
pub mod sorafs_pop_runtime;
/// Supervised finalized-PoR reputation reconciliation and optional archive compaction.
pub mod sorafs_por_replay_archive_runtime;
/// Immutable finalized-ledger query adapter for provider ingest.
pub mod sorafs_provider_ingest_finalized_query;
/// Supervised finalized-ledger `SoraFS` provider-ingest worker.
pub mod sorafs_provider_ingest_runtime;
/// Immutable finalized-ledger query adapter for the reputation runtime.
pub mod sorafs_reputation_finalized_query;
/// Supervised committed `SoraFS` reputation projector and publisher.
pub mod sorafs_reputation_runtime;
/// Supervised finalized reserve-event transparency ingestion.
pub mod sorafs_reserve_transparency_runtime;
/// Qualified stream-token gateway admission and durable callback reconciliation.
mod sorafs_stream_token_gateway_runtime;
/// Bounded local readers for startup trust-root artifacts.
mod startup_artifact;
/// Native Falcon-backed standalone Taira Bootle/Lantern issuer broker.
#[cfg(feature = "daemon")]
pub mod taira_bootle_lantern_broker;

pub use runtime_provider_broker::{
    BootleLanternIssuanceBrokerBackendErrorV1, BootleLanternIssuanceBrokerBackendV1,
    RuntimeProviderBrokerBackendRegistryV1, RuntimeProviderBrokerBackendsV1,
    RuntimeProviderBrokerDeploymentV1, RuntimeProviderBrokerExecutableArgsV1,
    RuntimeProviderBrokerExecutableErrorV1, RuntimeProviderBrokerExecutableV1,
    RuntimeProviderBrokerLauncherErrorV1, RuntimeProviderBrokerLifecycleV1,
    RuntimeProviderBrokerReadinessErrorV1, RuntimeProviderBrokerServerErrorV1,
    StockGovernanceDagServiceRuntimeProviderRegistryV1,
    load_runtime_provider_broker_catalog_file_v1, serve_runtime_provider_broker_v1,
    serve_runtime_provider_broker_with_fallible_readiness_v1,
    serve_runtime_provider_broker_with_lifecycle_v1,
};
pub use runtime_provider_registry::{
    IrohaRuntimeProviderBindingV1, IrohaRuntimeProviderBindingsV1,
    IrohaRuntimeProviderCatalogErrorV1, IrohaRuntimeProviderRegistryErrorV1,
    IrohaRuntimeProviderRegistryV1, IrohaRuntimeProviderSlotV1,
    RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1,
};
#[cfg(target_os = "windows")]
use std::os::windows::{ffi::OsStrExt, fs::MetadataExt as _};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    convert::TryFrom,
    env,
    ffi::OsString,
    fs,
    future::Future,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::soracloud_runtime::{
    QueuedSoracloudRuntimeMutationSink, SoracloudRuntimeManager, SoracloudRuntimeManagerHandle,
};
use clap::Parser;
use error_stack::{Report, ResultExt};
use eyre::Result as EyreResult;
use fastpq_prover::MetalOverrides;
use iroha_config::{
    base::{WithOrigin, read::ConfigReader, toml::TomlSource, util::Emitter},
    parameters::{
        actual::{
            FastpqExecutionMode, FastpqPoseidonMode, NexusStorageBudgetComponent,
            NexusStorageFilesystemBudget, Root as Config,
        },
        user::Root as UserConfig,
    },
    snapshot::Mode as SnapshotMode,
};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::{StateTelemetry, StreamingTelemetry};
use iroha_core::{
    IrohaNetwork,
    block::ValidBlock,
    compliance::LaneComplianceEngine,
    gossiper::{TransactionGossiper, TransactionGossiperHandle},
    governance::manifest::{
        GovernanceGuardError, LaneManifestRegistry, LaneManifestRegistryHandle,
    },
    kiso::KisoHandle,
    kura::Kura,
    panic_hook,
    peers_gossiper::{PeersGossiper, PeersGossiperHandle},
    query::store::LiveQueryStore,
    queue::{ConfigLaneRouter, LaneRouter, Queue, SingleLaneRouter},
    smartcontracts::isi::Registrable as _,
    snapshot::{
        SnapshotMaker, TryReadError as TryReadSnapshotError,
        try_read_snapshot_with_bootstrap_policy,
    },
    state::{State, World, WorldReadOnly as _},
    streaming::{FilesystemSoranetProvisioner, ManifestPublisher, run_ticket_event_listener},
    sumeragi::{
        GenesisWithPubKey, InboundBlockMessage, LaneRelayMessage,
        ProductionTwoStageRelayRetryTraceProjection, SumeragiHandle, SumeragiIngressDisposition,
        SumeragiStartArgs, V2StartupReplayInventoryGuard, VotingBlock,
        check_production_two_stage_relay_retry_transition, filter_validators_from_trusted,
        network_topology::Topology,
    },
};
use iroha_crypto::Algorithm;
use iroha_data_model::query::{self as dm_query, ErasedIterQuery};
use iroha_data_model::{
    isi::RegisterPeerWithPop,
    parameter::system::{
        ConsensusHandshakeMetadata, confidential_metadata, consensus_metadata, crypto_metadata,
    },
};
use iroha_data_model::{prelude::*, transaction::Executable};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, Supervisor};
use iroha_genesis::{
    GenesisBlock, ManifestCrypto, RawGenesisTransaction, compute_genesis_vk_set_hash,
    init_instruction_registry as init_genesis_instruction_registry,
};
use iroha_logger::actor::LoggerHandle;
use iroha_p2p::ClassifyTopic;
use iroha_primitives::addr::SocketAddr;
use iroha_primitives::erasure::rs16;
use iroha_primitives::json::Json;
use iroha_primitives::time::TimeSource;
#[cfg(feature = "telemetry")]
use iroha_telemetry::metrics::set_duplicate_metrics_panic;
use iroha_torii::Torii;
pub use iroha_version::BuildLine;
use norito::{codec::Encode, derive::JsonDeserialize, streaming::CapabilityFlags};
use parking_lot::deadlock;
use tokio::{
    sync::{Semaphore, broadcast, mpsc, oneshot},
    task,
};

use startup_artifact::{
    INTEGRITY_BOUND_CONFIG_MAX_BYTES_V1, read_bounded_startup_artifact, read_genesis_manifest,
    read_genesis_unlocked,
};

const NODE_RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
/// Build-time source identity embedded for release artifact validation.
const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");

fn startup_trace_enabled() -> bool {
    env::var_os("IROHA_STARTUP_TRACE").is_some()
}

fn log_startup_trace(stage: &'static str, started_at: Instant) {
    if startup_trace_enabled() {
        iroha_logger::info!(
            stage,
            elapsed_ms = started_at.elapsed().as_millis(),
            "startup trace"
        );
    }
}

fn torii_receipt_signer_or_derived(
    receipt_signer: Option<KeyPair>,
    node_key_pair: &KeyPair,
) -> Result<KeyPair, iroha_crypto::Error> {
    if let Some(receipt_signer) = receipt_signer {
        return Ok(receipt_signer);
    }

    // A receipt key must survive process restarts because the durable DA log verifies historical
    // receipts during startup. Derive a separate key from the node identity instead of generating
    // an ephemeral key; the BLAKE3 KDF context prevents the derived key from being reused as the
    // node key or by another protocol.
    let (node_algorithm, mut node_secret) = node_key_pair.private_key().try_to_bytes()?;
    let mut key_deriver = blake3::Hasher::new_derive_key("iroha:torii:receipt-signer:secp256k1:v1");
    key_deriver.update(node_algorithm.as_static_str().as_bytes());
    key_deriver.update(&[0]);
    key_deriver.update(&node_secret);
    let mut seed = [0_u8; 32];
    key_deriver.finalize_xof().fill(&mut seed);
    node_secret.fill(0);
    let key = iroha_crypto::KeyPair::try_from_seed(seed.to_vec(), Algorithm::Secp256k1);
    seed.fill(0);
    let key = key?;
    let algorithm = key
        .public_key()
        .try_algorithm()
        .map_or("malformed", |algorithm| algorithm.as_static_str());
    iroha_logger::info!(
        algorithm,
        "torii receipt signer not configured; derived a stable domain-separated key from the node identity"
    );
    Ok(key)
}

type ConsensusHandshakeMeta = ConsensusHandshakeMetadata;

fn parse_handshake_meta_str(raw: &str) -> Result<ConsensusHandshakeMeta, norito::Error> {
    let metadata: ConsensusHandshakeMeta =
        norito::json::from_str(raw).map_err(norito::Error::from)?;
    metadata
        .validate()
        .map_err(|error| norito::Error::Message(error.clone()))?;
    Ok(metadata)
}

fn parse_manifest_crypto_str(raw: &str) -> Result<ManifestCrypto, norito::Error> {
    norito::json::from_str(raw).map_err(norito::Error::from)
}

fn parse_confidential_registry_meta_str(
    raw: &str,
) -> Result<ConfidentialRegistryMeta, norito::Error> {
    norito::json::from_str(raw).map_err(norito::Error::from)
}

fn decode_crypto_manifest_meta(payload: &Json) -> Result<ManifestCrypto, norito::Error> {
    match parse_manifest_crypto_str(payload.get()) {
        Ok(meta) => Ok(meta),
        Err(error) => {
            let preview: String = payload.get().chars().take(256).collect();
            tracing::warn!(?error, preview = %preview, "failed to decode crypto_manifest_meta payload");
            Err(norito::Error::Message(
                "failed to decode crypto_manifest_meta payload".to_string(),
            ))
        }
    }
}

fn decode_confidential_registry_meta(
    payload: &Json,
) -> Result<ConfidentialRegistryMeta, norito::Error> {
    parse_confidential_registry_meta_str(payload.get()).map_err(|_| {
        norito::Error::Message("failed to decode confidential_registry_root payload".to_string())
    })
}

fn confidential_handshake_policy_digest(
    digest: iroha_data_model::confidential::ConfidentialFeatureDigest,
) -> iroha_data_model::confidential::ConfidentialFeatureDigest {
    iroha_data_model::confidential::ConfidentialFeatureDigest::new(
        None,
        None,
        None,
        digest.conf_rules_version,
        digest.zk_policy_hash,
    )
}

fn decode_consensus_handshake_meta(
    payload: &Json,
) -> Result<ConsensusHandshakeMeta, norito::Error> {
    parse_handshake_meta_str(payload.get()).map_err(|_| {
        norito::Error::Message("failed to decode consensus_handshake_meta payload".to_string())
    })
}

type SharedSoraFsProviderCache = Arc<tokio::sync::RwLock<iroha_torii::sorafs::ProviderAdvertCache>>;

#[derive(Debug)]
enum SharedSoraFsProviderCacheError {
    AdmissionPolicyRequired,
    MalformedCouncilKey {
        index: usize,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    UnsupportedCouncilKeyAlgorithm {
        index: usize,
        algorithm: iroha_crypto::Algorithm,
    },
    InvalidCouncilKeyLength {
        index: usize,
        actual: usize,
    },
    InvalidCouncilPolicy(sorafs_manifest::ProviderAdmissionCouncilPolicyError),
    AdmissionRegistry(iroha_torii::sorafs::AdmissionRegistryError),
    UnknownCapability(String),
    DuplicateCapability(String),
    EmptyCapabilities,
    ReplayCheckpoint {
        path: PathBuf,
        source: iroha_torii::sorafs::ReplayCheckpointError,
    },
}

impl core::fmt::Display for SharedSoraFsProviderCacheError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AdmissionPolicyRequired => formatter.write_str(
                "SoraFS discovery requires sorafs.discovery.admission.envelopes_dir, trusted_council_keys, and signature_threshold",
            ),
            Self::MalformedCouncilKey { index, source } => write!(
                formatter,
                "SoraFS admission council key at index {index} is malformed: {source}"
            ),
            Self::UnsupportedCouncilKeyAlgorithm { index, algorithm } => write!(
                formatter,
                "SoraFS admission council key at index {index} uses {algorithm:?}; Ed25519 is required"
            ),
            Self::InvalidCouncilKeyLength { index, actual } => write!(
                formatter,
                "SoraFS admission council key at index {index} has {actual} bytes; 32 are required"
            ),
            Self::InvalidCouncilPolicy(source) => {
                write!(formatter, "invalid SoraFS admission council policy: {source}")
            }
            Self::AdmissionRegistry(source) => {
                write!(formatter, "failed to load SoraFS provider admission registry: {source}")
            }
            Self::UnknownCapability(name) => write!(
                formatter,
                "unknown SoraFS capability `{name}` in torii.sorafs.known_capabilities"
            ),
            Self::DuplicateCapability(name) => write!(
                formatter,
                "duplicate SoraFS capability `{name}` in torii.sorafs.known_capabilities"
            ),
            Self::EmptyCapabilities => formatter.write_str(
                "torii.sorafs.known_capabilities must include at least one capability",
            ),
            Self::ReplayCheckpoint { path, source } => write!(
                formatter,
                "failed to load SoraFS provider replay checkpoint {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SharedSoraFsProviderCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedCouncilKey { source, .. } => Some(source.as_ref()),
            Self::InvalidCouncilPolicy(source) => Some(source),
            Self::AdmissionRegistry(source) => Some(source),
            Self::ReplayCheckpoint { source, .. } => Some(source),
            Self::AdmissionPolicyRequired
            | Self::UnsupportedCouncilKeyAlgorithm { .. }
            | Self::InvalidCouncilKeyLength { .. }
            | Self::UnknownCapability(_)
            | Self::DuplicateCapability(_)
            | Self::EmptyCapabilities => None,
        }
    }
}

fn build_shared_sorafs_provider_cache(
    config: &Config,
) -> Result<Option<SharedSoraFsProviderCache>, SharedSoraFsProviderCacheError> {
    let discovery = &config.torii.sorafs_discovery;
    if !discovery.discovery_enabled {
        return Ok(None);
    }

    let admission_cfg = discovery
        .admission
        .as_ref()
        .ok_or(SharedSoraFsProviderCacheError::AdmissionPolicyRequired)?;

    let trusted_council_keys = admission_cfg
        .trusted_council_keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let (algorithm, payload) = key.try_to_bytes().map_err(|source| {
                SharedSoraFsProviderCacheError::MalformedCouncilKey {
                    index,
                    source: Box::new(source),
                }
            })?;
            if algorithm != iroha_crypto::Algorithm::Ed25519 {
                return Err(
                    SharedSoraFsProviderCacheError::UnsupportedCouncilKeyAlgorithm {
                        index,
                        algorithm,
                    },
                );
            }
            <[u8; 32]>::try_from(payload).map_err(|_| {
                SharedSoraFsProviderCacheError::InvalidCouncilKeyLength {
                    index,
                    actual: payload.len(),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let policy = sorafs_manifest::ProviderAdmissionCouncilPolicy::new(
        trusted_council_keys,
        admission_cfg.signature_threshold.get(),
    )
    .map_err(SharedSoraFsProviderCacheError::InvalidCouncilPolicy)?;

    let mut capabilities = Vec::new();
    for name in &discovery.known_capabilities {
        let capability = iroha_torii::sorafs::parse_capability_name(name)
            .ok_or_else(|| SharedSoraFsProviderCacheError::UnknownCapability(name.clone()))?;
        if capabilities.contains(&capability) {
            return Err(SharedSoraFsProviderCacheError::DuplicateCapability(
                name.clone(),
            ));
        }
        capabilities.push(capability);
    }

    if capabilities.is_empty() {
        return Err(SharedSoraFsProviderCacheError::EmptyCapabilities);
    }

    let admission = Arc::new(
        iroha_torii::sorafs::AdmissionRegistry::load_from_dir(&admission_cfg.envelopes_dir, policy)
            .map_err(SharedSoraFsProviderCacheError::AdmissionRegistry)?,
    );

    let replay_checkpoint_path = if discovery.replay_checkpoint_path.is_absolute() {
        discovery.replay_checkpoint_path.clone()
    } else {
        config
            .torii
            .data_dir
            .join(&discovery.replay_checkpoint_path)
    };
    let cache = iroha_torii::sorafs::ProviderAdvertCache::new_persistent(
        capabilities,
        admission,
        replay_checkpoint_path.clone(),
        discovery.replay_checkpoint_max_entries,
    )
    .map_err(|source| SharedSoraFsProviderCacheError::ReplayCheckpoint {
        path: replay_checkpoint_path,
        source,
    })?;

    Ok(Some(Arc::new(tokio::sync::RwLock::new(cache))))
}

include!("main/shared_sorafs_provider_cache_tests.rs");

#[cfg(test)]
mod handshake_payload_tests {
    use super::*;
    use iroha_genesis::{GenesisBuilder, ManifestCrypto};
    use std::path::PathBuf;

    fn handshake_payload_from_genesis() -> Json {
        let chain = iroha_data_model::ChainId::from("handshake-meta-test");
        let manifest = GenesisBuilder::new_without_executor(chain, PathBuf::from("."))
            .build_raw()
            .with_consensus_meta();
        let keypair = iroha_crypto::KeyPair::random();
        let genesis_block = manifest
            .build_and_sign(&keypair)
            .expect("sign genesis with meta");

        for tx in genesis_block.0.external_transactions() {
            if let Executable::Instructions(batch) = tx.instructions() {
                for instr in batch {
                    if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                        && let Parameter::Custom(custom) = set_param.inner()
                        && custom.id() == &consensus_metadata::handshake_meta_id()
                    {
                        return custom.payload().clone();
                    }
                }
            }
        }
        panic!("handshake payload not found");
    }

    #[test]
    fn decode_consensus_meta_rejects_nested_json_string_payload() {
        let payload = handshake_payload_from_genesis();
        let meta = decode_consensus_handshake_meta(&payload).expect("decode normal payload");
        assert_eq!(
            meta.mode,
            iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned
        );

        let stringified =
            Json::new(norito::json::to_json(&payload).expect("stringify handshake payload"));
        let err =
            decode_consensus_handshake_meta(&stringified).expect_err("nested payload must fail");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_consensus_meta_rejects_garbage() {
        let bad = Json::from_norito_value_ref(&norito::json::Value::String("not json".into()))
            .expect("construct bad json");
        let err = decode_consensus_handshake_meta(&bad).expect_err("garbage must fail");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_consensus_meta_rejects_mangled_json() {
        let mangled = Json::from_norito_value_ref(&norito::json::Value::String(
            r#"{mode"Permissioned",bls_domain"bls-iroha2:permissioned-sumeragi:v2",consensus_fingerprint"0x632eaff6fe3054ca279416357baae5ff7f28144b3bc6a83921f68d466c4ec0ab"}"#.to_string(),
        ))
        .expect("construct mangled payload");
        let err = decode_consensus_handshake_meta(&mangled).expect_err("mangled payload must fail");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_consensus_meta_rejects_unprefixed_hex_and_uppercase_tokens() {
        let fingerprint = "632eaff6fe3054ca279416357baae5ff7f28144b3bc6a83921f68d466c4ec0ab";
        let raw = format!(
            "MODE=PERMISSIONED bls_domain=bls-iroha2:permissioned-sumeragi:v2 consensus_fingerprint={fingerprint}"
        );
        let payload = Json::from(raw.as_str());
        let err =
            decode_consensus_handshake_meta(&payload).expect_err("non-JSON payload must fail");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_crypto_manifest_meta_rejects_nested_json_string_payload() {
        let manifest = ManifestCrypto::default();
        let payload = Json::new(manifest.clone());
        let decoded = decode_crypto_manifest_meta(&payload).expect("decode normal payload");
        assert_eq!(decoded, manifest);

        let stringified =
            Json::new(norito::json::to_json(&payload).expect("stringify manifest payload"));
        let err = decode_crypto_manifest_meta(&stringified)
            .expect_err("nested string payload must be rejected");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_crypto_manifest_meta_rejects_raw_quoted_legacy_payload() {
        let raw = r#""{"allowed_curve_ids":[1,3,4],"allowed_signing":["ed25519","secp256k1","bls_normal"],"default_hash":"blake2b-256","sm2_distid_default":"1234567812345678","sm_openssl_preview":false}""#;
        assert!(
            Json::from_raw_json(raw.to_owned()).is_err(),
            "raw-quoted compatibility payload must be rejected at construction"
        );
    }

    #[test]
    fn decode_crypto_manifest_meta_rejects_backslash_escaped_object_payload() {
        let raw = r#"{\"allowed_curve_ids\":[1,3,4],\"allowed_signing\":[\"ed25519\",\"secp256k1\",\"bls_normal\"],\"default_hash\":\"blake2b-256\",\"sm2_distid_default\":\"1234567812345678\",\"sm_openssl_preview\":false}"#;
        assert!(
            Json::from_raw_json(raw.to_owned()).is_err(),
            "backslash-escaped compatibility payload must be rejected at construction"
        );
    }

    #[test]
    fn decode_crypto_manifest_meta_rejects_mangled_key_value_separators() {
        let raw = r#"{"allowed_curve_ids""[1,3,4],"allowed_signing""["ed25519","secp256k1","bls_normal"],"default_hash""blake2b-256","sm2_distid_default""1234567812345678","sm_openssl_preview"false}"#;
        assert!(
            Json::from_raw_json(raw.to_owned()).is_err(),
            "mangled compatibility payload must be rejected at construction"
        );
    }

    #[test]
    fn decode_confidential_registry_meta_handles_normal_json() {
        let hash = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let payload = Json::from_raw_json(format!("{{\"vk_set_hash\":\"{hash}\"}}"))
            .expect("valid confidential registry JSON");
        let decoded =
            decode_confidential_registry_meta(&payload).expect("decode confidential payload");
        assert_eq!(decoded.vk_set_hash.as_deref(), Some(hash));
    }

    #[test]
    fn decode_confidential_registry_meta_rejects_mangled_key_value_separators() {
        let hash = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(
            Json::from_raw_json(format!("{{\"vk_set_hash\"\"{hash}\"}}")).is_err(),
            "mangled compatibility payload must fail at construction"
        );
    }

    #[test]
    fn parse_confidential_registry_hash_treats_json_null_as_absent() {
        let payload = Json::from_raw_json("{\"vk_set_hash\":null}".to_string())
            .expect("valid null confidential registry JSON");
        let decoded =
            parse_confidential_registry_hash(&payload).expect("decode null confidential payload");
        assert_eq!(decoded, None);
    }

    #[test]
    fn confidential_handshake_digest_excludes_height_dependent_registry_fields() {
        let digest = iroha_data_model::confidential::ConfidentialFeatureDigest::new(
            Some([1; 32]),
            Some(7),
            Some(9),
            Some(1),
            Some([2; 32]),
        );

        let handshake = confidential_handshake_policy_digest(digest);

        assert_eq!(handshake.vk_set_hash, None);
        assert_eq!(handshake.poseidon_params_id, None);
        assert_eq!(handshake.pedersen_params_id, None);
        assert_eq!(handshake.conf_rules_version, Some(1));
        assert_eq!(handshake.zk_policy_hash, Some([2; 32]));
    }
}

#[derive(Debug, JsonDeserialize)]
struct ConfidentialRegistryMeta {
    #[norito(default)]
    vk_set_hash: Option<String>,
}

#[cfg(feature = "beep")]
use ivm::IVM;
use ivm::set_banner_enabled;

/// Detect if the current terminal supports ANSI colors.
pub fn is_coloring_supported() -> bool {
    supports_color::on(supports_color::Stream::Stdout).is_some()
}

fn default_terminal_colors_str() -> clap::builder::OsStr {
    is_coloring_supported().to_string().into()
}

/// Initialize the global query registry used to decode iterable queries.
///
/// Iroha transports iterable queries as type-erased `QueryBox` values. The
/// receiving side needs a registry to deserialize them back into an erased
/// representation carrying predicate/selector info. Register all supported
/// iterable query item types here.
fn init_query_registry() {
    use iroha_data_model as dm;

    dm_query::set_query_registry(dm::query_registry![
        ErasedIterQuery<dm::domain::Domain>,
        ErasedIterQuery<dm::account::Account>,
        ErasedIterQuery<dm::asset::value::Asset>,
        ErasedIterQuery<dm::asset::definition::AssetDefinition>,
        ErasedIterQuery<dm::nft::Nft>,
        ErasedIterQuery<dm::role::Role>,
        ErasedIterQuery<dm::role::RoleId>,
        ErasedIterQuery<dm::peer::PeerId>,
        ErasedIterQuery<dm::trigger::TriggerId>,
        ErasedIterQuery<dm::trigger::Trigger>,
        ErasedIterQuery<dm_query::CommittedTransaction>,
        ErasedIterQuery<dm::block::SignedBlock>,
        ErasedIterQuery<dm::block::BlockHeader>,
    ]);
}

#[cfg(feature = "telemetry")]
fn init_global_metrics_handle(
    panic_on_duplicate_metrics: bool,
) -> Arc<iroha_telemetry::metrics::Metrics> {
    set_duplicate_metrics_panic(panic_on_duplicate_metrics);
    iroha_telemetry::metrics::global().map_or_else(
        || {
            let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
            match iroha_telemetry::metrics::install_global(Arc::clone(&metrics)) {
                Ok(()) => metrics,
                Err(_) => iroha_telemetry::metrics::global_or_default(),
            }
        },
        Arc::clone,
    )
}

fn nexus_topology_is_custom(nexus: &iroha_config::parameters::actual::Nexus) -> bool {
    nexus.uses_multilane_catalogs()
}

fn should_use_config_router(nexus: &iroha_config::parameters::actual::Nexus) -> bool {
    nexus.enabled && nexus_topology_is_custom(nexus)
}

fn ensure_manifest_crypto_matches(
    manifest: &RawGenesisTransaction,
    config: &Config,
) -> Result<(), String> {
    ensure_crypto_snapshot_matches_config(manifest.crypto(), config)
}

fn ensure_crypto_snapshot_matches_config(
    manifest_crypto: &ManifestCrypto,
    config: &Config,
) -> Result<(), String> {
    manifest_crypto
        .validate()
        .map_err(|err| format!("Invalid crypto section in genesis manifest: {err:?}"))?;

    let mut manifest_allowed = manifest_crypto.allowed_signing.clone();
    manifest_allowed.sort();
    manifest_allowed.dedup();

    let mut config_allowed = config.crypto.allowed_signing.clone();
    config_allowed.sort();
    config_allowed.dedup();

    let hashes_match = manifest_crypto
        .default_hash
        .eq_ignore_ascii_case(&config.crypto.default_hash);

    let distid_match = manifest_crypto.sm2_distid_default == config.crypto.sm2_distid_default;
    let manifest_sm_helpers = manifest_crypto
        .allowed_signing
        .iter()
        .any(|algo| algo.as_static_str().eq_ignore_ascii_case("sm2"));
    let config_sm_helpers = config.crypto.sm_helpers_enabled();
    let preview_match =
        manifest_crypto.sm_openssl_preview == config.crypto.enable_sm_openssl_preview;
    let mut manifest_curves =
        iroha_config::parameters::actual::Crypto::from(manifest_crypto.clone()).allowed_curve_ids;
    manifest_curves.sort_unstable();
    manifest_curves.dedup();

    let mut config_curves = config.crypto.allowed_curve_ids.clone();
    config_curves.sort_unstable();
    config_curves.dedup();

    if !hashes_match
        || manifest_allowed != config_allowed
        || !distid_match
        || manifest_sm_helpers != config_sm_helpers
        || !preview_match
        || manifest_curves != config_curves
    {
        return Err(format!(
            "Genesis manifest crypto mismatch: manifest {{ sm_helpers_enabled: {}, sm_openssl_preview: {}, default_hash: {}, allowed_signing: {:?}, allowed_curve_ids: {:?}, sm2_distid_default: {} }} != config {{ sm_helpers_enabled: {}, sm_openssl_preview: {}, default_hash: {}, allowed_signing: {:?}, allowed_curve_ids: {:?}, sm2_distid_default: {} }}",
            manifest_sm_helpers,
            manifest_crypto.sm_openssl_preview,
            manifest_crypto.default_hash,
            manifest_allowed,
            manifest_curves,
            manifest_crypto.sm2_distid_default,
            config_sm_helpers,
            config.crypto.enable_sm_openssl_preview,
            config.crypto.default_hash,
            config_allowed,
            config_curves,
            config.crypto.sm2_distid_default,
        ));
    }

    Ok(())
}

/// Ensure operator signature policy includes the node identity when requested by config.
fn ensure_operator_node_key_allowlisted(config: &mut Config) {
    if !config.torii.operator_signatures.allow_node_key {
        return;
    }

    let node_public_key = config.common.key_pair.public_key().clone();
    if config
        .torii
        .operator_signatures
        .allowed_public_keys
        .iter()
        .all(|key| key != &node_public_key)
    {
        config
            .torii
            .operator_signatures
            .allowed_public_keys
            .push(node_public_key);
    }
}

#[cfg(feature = "beep")]
fn startup_beep(enable_beep: bool) -> bool {
    if !enable_beep {
        return false;
    }

    IVM::beep_music();
    const SHA256_ABC_EXPECTED: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    let _ = SHA256_ABC_EXPECTED;
    true
}

/// Iroha server CLI
#[derive(clap::Args, Clone, Debug)]
pub struct StartupArgs {
    /// Validate configuration and any locally available genesis, then exit
    /// without binding network or Torii sockets.
    #[arg(long)]
    pub check_config: bool,
    /// Fully qualify the configured Kagemusha catalog and publish its canonical
    /// cold-start seal at this root-owned path.
    #[arg(
        long,
        value_name = "PATH",
        value_hint(clap::ValueHint::FilePath),
        requires = "check_config"
    )]
    pub write_kagemusha_catalog_qualification_seal: Option<PathBuf>,
    /// Enables trace logs of configuration reading & parsing.
    ///
    /// Might be useful for configuration troubleshooting.
    #[arg(long, env)]
    pub trace_config: bool,
    /// Require the configuration file bytes to match this lowercase or uppercase
    /// 64-digit BLAKE3 digest.
    ///
    /// Integrity-bound files are parsed from the exact bytes that were hashed
    /// and must be flattened (the `extends` directive is not accepted).
    #[arg(long, value_name = "HEX", requires = "config")]
    pub config_blake3: Option<String>,
}

/// Complete command-line arguments for the Iroha server.
#[derive(Parser, Debug)]
#[command(
    name = "iroha3d",
    version = env!("CARGO_PKG_VERSION"),
    author
)]
pub struct Args {
    /// Path to the configuration file
    #[arg(long, short, value_name("PATH"), value_hint(clap::ValueHint::FilePath))]
    pub config: Option<PathBuf>,
    /// Optional path to genesis manifest JSON for consensus validation
    #[arg(long, value_name = "PATH", value_hint(clap::ValueHint::FilePath))]
    pub genesis_manifest_json: Option<PathBuf>,
    /// Startup and configuration-validation switches.
    #[command(flatten)]
    pub startup: StartupArgs,
    /// Whether to enable ANSI-colored output or not
    ///
    /// By default, Iroha determines whether the terminal supports colors or not.
    ///
    /// In order to disable this flag explicitly, pass `--terminal-colors=false`.
    #[arg(
        long,
        env,
        default_missing_value("true"),
        default_value(default_terminal_colors_str()),
        action(clap::ArgAction::Set),
        require_equals(true),
        num_args(0..=1),
    )]
    pub terminal_colors: bool,
    /// Override system language for messages
    #[arg(long)]
    pub language: Option<String>,
    /// Enable Sora Nexus feature profile (`SoraFS`, `SoraNet` handshake, multi-lane consensus)
    #[arg(long, env = "IROHA_SORA_PROFILE")]
    pub sora: bool,
    /// Override FASTPQ prover execution mode (`cpu` or `gpu`).
    #[arg(
        long = "fastpq-execution-mode",
        value_name = "MODE",
        value_parser = parse_fastpq_execution_mode
    )]
    pub fastpq_execution_mode: Option<FastpqExecutionMode>,
    /// Override the FASTPQ Poseidon pipeline mode (`cpu` or `gpu`).
    #[arg(
        long = "fastpq-poseidon-mode",
        value_name = "MODE",
        value_parser = parse_fastpq_poseidon_mode
    )]
    pub fastpq_poseidon_mode: Option<FastpqPoseidonMode>,
    /// Override the FASTPQ telemetry device-class label (e.g., `apple-m4`, `xeon-rtx-sm80`).
    #[arg(long = "fastpq-device-class", value_name = "LABEL")]
    pub fastpq_device_class: Option<String>,
    /// Override the FASTPQ chip-family label (e.g., `m4`, `xeon-icelake`).
    #[arg(long = "fastpq-chip-family", value_name = "LABEL")]
    pub fastpq_chip_family: Option<String>,
    /// Override the FASTPQ GPU-kind label (e.g., `integrated`, `discrete`).
    #[arg(long = "fastpq-gpu-kind", value_name = "LABEL")]
    pub fastpq_gpu_kind: Option<String>,
}

/// Top-level standard-launcher failure category.
#[derive(Clone, Copy, Debug)]
pub enum MainError {
    /// Configuration tracing could not be initialized.
    TraceConfigSetup,
    /// Static configuration, genesis, or runtime-provider resolution failed.
    Config,
    /// Global logging could not be initialized.
    Logger,
    /// One or more node subsystems failed during startup.
    IrohaStart,
    /// A supervised node subsystem failed while the daemon was running.
    IrohaRun,
}

/// Read-only Torii adapter that refuses committed reputation reads whenever
/// the supervised daemon is not ready.
#[derive(Debug, Clone)]
struct ReadyReputationCommittedReaderV1 {
    runtime: sorafs_reputation_runtime::ReputationRuntimeHandleV1,
}

impl ReadyReputationCommittedReaderV1 {
    fn ensure_ready(&self) -> Result<(), sorafs_node::reputation::runtime::ReputationRuntimeError> {
        if self.runtime.status()?.ready {
            return Ok(());
        }
        let failure =
            sorafs_node::reputation::runtime::ReputationExternalFailureV1::try_new([0x52; 32])?;
        Err(sorafs_node::reputation::runtime::ReputationRuntimeError::External(failure))
    }
}

impl sorafs_node::reputation::runtime::ReputationCommittedReadApiV1
    for ReadyReputationCommittedReaderV1
{
    fn committed_read_projection(
        &self,
    ) -> Result<
        sorafs_node::reputation::runtime::ReputationCommittedReadProjectionV1,
        sorafs_node::reputation::runtime::ReputationRuntimeError,
    > {
        self.ensure_ready()?;
        self.runtime.committed_read_projection()
    }

    fn committed_snapshot_by_id(
        &self,
        snapshot_id: [u8; 16],
    ) -> Result<
        Option<sorafs_manifest::ReputationSnapshotV1>,
        sorafs_node::reputation::runtime::ReputationRuntimeError,
    > {
        self.ensure_ready()?;
        self.runtime.committed_snapshot_by_id(snapshot_id)
    }

    fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<
        Vec<sorafs_manifest::ReputationSnapshotEventV1>,
        sorafs_node::reputation::runtime::ReputationRuntimeError,
    > {
        self.ensure_ready()?;
        self.runtime.committed_events_after(sequence)
    }
}

/// [Orchestrator](https://en.wikipedia.org/wiki/Orchestration_%28computing%29)
/// of the system. It configures, coordinates and manages transactions
/// and queries processing, work of consensus and storage.
pub struct Iroha {
    /// Kura — block storage
    kura: Arc<Kura>,
    /// State of blockchain
    state: Arc<State>,
    /// Embedded Soracloud runtime-manager handle.
    soracloud_runtime: SoracloudRuntimeManagerHandle,
    /// Streaming session manager
    streaming: iroha_core::streaming::StreamingHandle,
    /// P2P network handle used for outbound control frames (e.g., streaming manifests).
    network: IrohaNetwork,
    /// Supervised committed reputation runtime status/metrics handle.
    sorafs_reputation_runtime: Option<sorafs_reputation_runtime::ReputationRuntimeHandleV1>,
    /// Supervised committed hedging/billing runtime status/metrics handle.
    sorafs_hedging_billing_runtime:
        Option<sorafs_hedging_billing_runtime::HedgingBillingRuntimeHandleV1>,
    /// Supervised finalized-ledger provider-ingest status/metrics handle.
    sorafs_provider_ingest_runtime:
        Option<sorafs_provider_ingest_runtime::ProviderIngestRuntimeHandleV1>,
    /// Daemon-owned archive-only provider-ingest finalized query.
    sorafs_provider_ingest_finalized_query: Option<
        Arc<sorafs_provider_ingest_finalized_query::ArchivedProviderIngestFinalizedLedgerV1>,
    >,
    /// Inert take-once tenure joining the exact prepared signed capture reader
    /// to the embedded SoraFS storage/outbox incarnation.
    #[allow(dead_code)]
    sorafs_provider_ingest_completed_musubi_capture:
        Option<sorafs_node::ProviderIngestCompletedMusubiCaptureCoordinatorV1>,
}

include!("main/runtime_deps.rs");

/// Error(s) that might occur while starting [`Iroha`]
#[derive(Debug, Copy, Clone)]
pub enum StartError {
    /// Failed to start the P2P network layer
    StartP2p,
    /// Failed to initialize block storage (Kura)
    InitKura,
    /// Failed to start development telemetry
    StartDevTelemetry,
    /// Failed to start telemetry subsystem
    StartTelemetry,
    /// Failed to listen for OS shutdown signals
    ListenOsSignal,
    /// Failed to start the Torii API server
    StartTorii,
}

#[derive(Debug, Copy, Clone)]
enum GenesisManifestError {
    ConsensusFingerprintMismatch,
}

impl core::fmt::Display for GenesisManifestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ConsensusFingerprintMismatch => {
                write!(f, "Genesis manifest consensus_fingerprint mismatch")
            }
        }
    }
}

impl std::error::Error for GenesisManifestError {}

impl std::fmt::Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = match self {
            MainError::TraceConfigSetup => "error.trace_config_setup",
            MainError::Config => "error.config",
            MainError::Logger => "error.logger",
            MainError::IrohaStart => "error.start",
            MainError::IrohaRun => "error.run",
        };
        write!(f, "{}", i18n::t(key))
    }
}

impl std::error::Error for MainError {}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = match self {
            StartError::StartP2p => "error.start_p2p",
            StartError::InitKura => "error.init_kura",
            StartError::StartDevTelemetry => "error.start_dev_telemetry",
            StartError::StartTelemetry => "error.start_telemetry",
            StartError::ListenOsSignal => "error.listen_os_signal",
            StartError::StartTorii => "error.start_torii",
        };
        write!(f, "{}", i18n::t(key))
    }
}

impl std::error::Error for StartError {}

struct NetworkRelay {
    sumeragi: SumeragiHandle,
    tx_gossiper: TransactionGossiperHandle,
    peers_gossiper: PeersGossiperHandle,
    network: IrohaNetwork,
    streaming: iroha_core::streaming::StreamingHandle,
    consensus_ingress: ConsensusIngressLimiter,
    low_priority_ingress: LowPriorityIngressLimiter,
    #[cfg(feature = "test-network-message-control")]
    test_message_control: Option<Arc<SumeragiMessageController>>,
}

#[cfg(feature = "test-network-message-control")]
type SumeragiMessageController = consensus_message_control::Controller<
    iroha_p2p::network::NetworkReplyRoute,
    HeldSumeragiRelayOwnership,
>;

struct NetworkRelayShared {
    sumeragi: SumeragiHandle,
    tx_gossiper: TransactionGossiperHandle,
    peers_gossiper: PeersGossiperHandle,
    network: IrohaNetwork,
    streaming: iroha_core::streaming::StreamingHandle,
    consensus_ingress: Mutex<ConsensusIngressLimiter>,
    low_priority_ingress: Mutex<LowPriorityIngressLimiter>,
    #[cfg(feature = "test-network-message-control")]
    test_message_control: Option<Arc<SumeragiMessageController>>,
}

type RelayWorkItem = iroha_p2p::peer::message::PeerMessage<iroha_core::NetworkMessage>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SumeragiRelayClass {
    V2,
    Lane,
}

/// Checked capacity witness joining the two upstream authenticated-source
/// credit lanes to the daemon's per-class relay corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SumeragiRelayCapacityGeometry {
    network_per_lane: usize,
    authenticated_source_count: usize,
    daemon_per_source: usize,
    class_capacity: usize,
    retained_capacity: usize,
}

impl SumeragiRelayCapacityGeometry {
    fn checked(
        subscriber_base: usize,
        network_per_lane: usize,
        authenticated_source_count: usize,
    ) -> Option<Self> {
        if subscriber_base == 0 || network_per_lane == 0 || authenticated_source_count == 0 {
            return None;
        }
        let daemon_per_source = 2_usize.checked_mul(network_per_lane)?;
        let aggregate_authenticated_sources =
            authenticated_source_count.checked_mul(daemon_per_source)?;
        let baseline = 2_usize.checked_mul(subscriber_base)?;
        let class_capacity = baseline.max(aggregate_authenticated_sources);
        let retained_capacity = 2_usize.checked_mul(class_capacity)?;
        Some(Self {
            network_per_lane,
            authenticated_source_count,
            daemon_per_source,
            class_capacity,
            retained_capacity,
        })
    }

    const fn daemon_source_capacity_matches_two_upstream_lanes(self) -> bool {
        self.network_per_lane != 0
            && self.daemon_per_source != 0
            && matches!(
                self.network_per_lane.checked_mul(2),
                Some(capacity) if capacity == self.daemon_per_source
            )
    }

    const fn class_corridor_covers_authenticated_sources(self) -> bool {
        self.authenticated_source_count != 0
            && self.class_capacity != 0
            && matches!(
                self.authenticated_source_count
                    .checked_mul(self.daemon_per_source),
                Some(aggregate) if aggregate <= self.class_capacity
            )
            && matches!(
                self.class_capacity.checked_mul(2),
                Some(capacity) if capacity == self.retained_capacity
            )
    }
}

/// Terminal disposition of one exact daemon relay occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SumeragiRelayTerminalOutcome {
    /// Ordinary ingress accepted, coalesced, or found the occurrence obsolete.
    Delivered,
    /// Its authenticated reply authority retired before ingress succeeded.
    Retired,
    /// Preparation or downstream semantic admission failed.
    Failed,
}

impl SumeragiRelayClass {
    const fn label(self) -> &'static str {
        match self {
            Self::V2 => "v2",
            Self::Lane => "lane",
        }
    }
}

struct SumeragiRelayWorkItem {
    work: RelayWorkItem,
    #[cfg(feature = "test-network-message-control")]
    skip_test_control: bool,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
}

/// One relay occurrence after exact layered ownership has been established.
struct CreditedSumeragiRelayWorkItem {
    work: SumeragiRelayWorkItem,
    ownership: SumeragiRelayIngressOwnership,
}

/// Ownership crossing the relay-dispatch channel.
///
/// Fresh network work retains its upstream credit inside `work` and adds the
/// daemon permit here. A controlled release instead carries the exact combined
/// token which was moved into the Hold queue; it never reacquires either layer.
enum SumeragiRelayIngressOwnership {
    Fresh {
        source: SumeragiRelaySource,
        geometry: SumeragiRelayCapacityGeometry,
        source_credit: tokio::sync::OwnedSemaphorePermit,
    },
    #[cfg(feature = "test-network-message-control")]
    Rehydrated(SumeragiRelayRetention),
}

impl SumeragiRelayWorkItem {
    fn live(work: RelayWorkItem) -> Self {
        Self {
            work,
            #[cfg(feature = "test-network-message-control")]
            skip_test_control: false,
            completion: None,
        }
    }

    #[cfg(feature = "test-network-message-control")]
    fn released(
        work: RelayWorkItem,
        completion: oneshot::Sender<SumeragiRelayTerminalOutcome>,
    ) -> Self {
        Self {
            work,
            skip_test_control: true,
            completion: Some(completion),
        }
    }
}

trait SumeragiReplyRouteLiveness {
    fn is_active(&self) -> bool;
}

impl SumeragiReplyRouteLiveness for iroha_p2p::network::NetworkReplyRoute {
    fn is_active(&self) -> bool {
        iroha_p2p::network::NetworkReplyRoute::is_active(self)
    }
}

fn sumeragi_reply_route_terminal_if_inactive<R: SumeragiReplyRouteLiveness>(
    reply_route: &R,
) -> Option<SumeragiRelayTerminalOutcome> {
    (!reply_route.is_active()).then_some(SumeragiRelayTerminalOutcome::Retired)
}

#[cfg(feature = "test-network-message-control")]
struct SumeragiRelayPreparationParts<R, O = ()> {
    peer: Peer,
    authenticated_via: PeerId,
    message: iroha_core::NetworkMessage,
    size_bytes: usize,
    reply_route: Option<R>,
    ownership: O,
}

#[cfg(feature = "test-network-message-control")]
enum SumeragiRelayPreparationBoundary<R, O = ()> {
    Held,
    Dropped(O),
    Prepared(SumeragiRelayPreparationParts<R, O>),
    RetiredInactiveReplyRoute(O),
    Rejected {
        error: SumeragiRelayPreparationBoundaryError,
        ownership: Option<O>,
    },
}

#[cfg(feature = "test-network-message-control")]
enum SumeragiRelayPreparationBoundaryError {
    InvalidPassDisposition,
    Controller(consensus_message_control::ControlError),
    MissingReplyRoute,
}

#[cfg(feature = "test-network-message-control")]
fn prepare_sumeragi_relay_work_boundary<R: SumeragiReplyRouteLiveness, O>(
    controller: Option<&consensus_message_control::Controller<R, O>>,
    skip_test_control: bool,
    parts: SumeragiRelayPreparationParts<R, O>,
) -> SumeragiRelayPreparationBoundary<R, O> {
    let SumeragiRelayPreparationParts {
        peer,
        authenticated_via,
        message,
        size_bytes,
        reply_route,
        ownership,
    } = parts;
    let admitted = if !skip_test_control && let Some(controller) = controller {
        controller.admit_with_reply_route_and_ownership(
            peer,
            &authenticated_via,
            message,
            size_bytes,
            reply_route,
            Some(ownership),
        )
    } else {
        Ok((
            consensus_message_control::Admission::Pass,
            Some((peer, message, size_bytes, reply_route, Some(ownership))),
        ))
    };
    let (peer, message, size_bytes, reply_route, ownership) = match admitted {
        Ok((consensus_message_control::Admission::Held, _)) => {
            return SumeragiRelayPreparationBoundary::Held;
        }
        Ok((consensus_message_control::Admission::Consumed, Some(message))) => {
            let (_, _, _, _, ownership) = message;
            let Some(ownership) = ownership else {
                return SumeragiRelayPreparationBoundary::Rejected {
                    error: SumeragiRelayPreparationBoundaryError::InvalidPassDisposition,
                    ownership: None,
                };
            };
            return SumeragiRelayPreparationBoundary::Dropped(ownership);
        }
        Ok((consensus_message_control::Admission::Consumed, None)) => {
            return SumeragiRelayPreparationBoundary::Rejected {
                error: SumeragiRelayPreparationBoundaryError::InvalidPassDisposition,
                ownership: None,
            };
        }
        Ok((consensus_message_control::Admission::Pass, Some(message))) => message,
        Ok((consensus_message_control::Admission::Pass, None)) => {
            return SumeragiRelayPreparationBoundary::Rejected {
                error: SumeragiRelayPreparationBoundaryError::InvalidPassDisposition,
                ownership: None,
            };
        }
        Err(error) => {
            return SumeragiRelayPreparationBoundary::Rejected {
                error: SumeragiRelayPreparationBoundaryError::Controller(error),
                ownership: None,
            };
        }
    };
    let Some(ownership) = ownership else {
        return SumeragiRelayPreparationBoundary::Rejected {
            error: SumeragiRelayPreparationBoundaryError::InvalidPassDisposition,
            ownership: None,
        };
    };
    let Some(reply_route_ref) = reply_route.as_ref() else {
        return SumeragiRelayPreparationBoundary::Rejected {
            error: SumeragiRelayPreparationBoundaryError::MissingReplyRoute,
            ownership: Some(ownership),
        };
    };
    if !reply_route_ref.is_active() {
        return SumeragiRelayPreparationBoundary::RetiredInactiveReplyRoute(ownership);
    }
    SumeragiRelayPreparationBoundary::Prepared(SumeragiRelayPreparationParts {
        peer,
        authenticated_via,
        message,
        size_bytes,
        reply_route,
        ownership,
    })
}

#[cfg(feature = "test-network-message-control")]
trait AuthenticatedSumeragiRelayWork {
    fn authenticated_via(&self) -> &PeerId;
}

#[cfg(feature = "test-network-message-control")]
impl AuthenticatedSumeragiRelayWork for RelayWorkItem {
    fn authenticated_via(&self) -> &PeerId {
        iroha_p2p::peer::message::PeerMessage::authenticated_via(self)
    }
}

#[cfg(feature = "test-network-message-control")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeldSumeragiReentryFailure {
    UnsupportedMessage,
    MissingReplyRoute,
    MissingOwnership,
    RouteMismatch,
    AuthenticatedViaMismatch,
}

#[cfg(feature = "test-network-message-control")]
impl HeldSumeragiReentryFailure {
    const fn label(self) -> &'static str {
        match self {
            Self::UnsupportedMessage => "unsupported_message",
            Self::MissingReplyRoute => "missing_reply_route",
            Self::MissingOwnership => "missing_ownership",
            Self::RouteMismatch => "route_mismatch",
            Self::AuthenticatedViaMismatch => "authenticated_via_mismatch",
        }
    }
}

#[cfg(feature = "test-network-message-control")]
enum HeldSumeragiReentry<W, O> {
    Ready {
        sequence: u64,
        class: SumeragiRelayClass,
        work: W,
        ownership: O,
    },
    RetireStale {
        sequence: u64,
        ownership: O,
    },
    Reject {
        sequence: u64,
        reason: HeldSumeragiReentryFailure,
        ownership: Option<O>,
    },
}

#[cfg(feature = "test-network-message-control")]
fn rehydrate_held_sumeragi_relay_work<R, W, O>(
    held: consensus_message_control::HeldMessage<R, O>,
    reattach: impl FnOnce(Peer, iroha_core::NetworkMessage, usize, R) -> Result<W, R>,
) -> HeldSumeragiReentry<W, O>
where
    R: SumeragiReplyRouteLiveness,
    W: AuthenticatedSumeragiRelayWork,
{
    let consensus_message_control::HeldMessage {
        sequence,
        peer,
        authenticated_via,
        message,
        size_bytes,
        reply_route,
        ownership,
    } = held;
    let Some(ownership) = ownership else {
        return HeldSumeragiReentry::Reject {
            sequence,
            reason: HeldSumeragiReentryFailure::MissingOwnership,
            ownership: None,
        };
    };
    let Some(class) = sumeragi_relay_class(&message) else {
        return HeldSumeragiReentry::Reject {
            sequence,
            reason: HeldSumeragiReentryFailure::UnsupportedMessage,
            ownership: Some(ownership),
        };
    };
    let Some(reply_route) = reply_route else {
        return HeldSumeragiReentry::Reject {
            sequence,
            reason: HeldSumeragiReentryFailure::MissingReplyRoute,
            ownership: Some(ownership),
        };
    };
    if !reply_route.is_active() {
        return HeldSumeragiReentry::RetireStale {
            sequence,
            ownership,
        };
    }
    let work = match reattach(peer, message, size_bytes, reply_route) {
        Ok(work) => work,
        Err(reply_route) if !reply_route.is_active() => {
            return HeldSumeragiReentry::RetireStale {
                sequence,
                ownership,
            };
        }
        Err(_) => {
            return HeldSumeragiReentry::Reject {
                sequence,
                reason: HeldSumeragiReentryFailure::RouteMismatch,
                ownership: Some(ownership),
            };
        }
    };
    if work.authenticated_via() != &authenticated_via {
        return HeldSumeragiReentry::Reject {
            sequence,
            reason: HeldSumeragiReentryFailure::AuthenticatedViaMismatch,
            ownership: Some(ownership),
        };
    }
    HeldSumeragiReentry::Ready {
        sequence,
        class,
        work,
        ownership,
    }
}

#[derive(Clone)]
struct SumeragiRelayIngress {
    v2: mpsc::Sender<CreditedSumeragiRelayWorkItem>,
    lane: mpsc::Sender<CreditedSumeragiRelayWorkItem>,
    source_credits: SumeragiRelaySourceCredits,
}

impl SumeragiRelayIngress {
    async fn send(
        &self,
        class: SumeragiRelayClass,
        work: SumeragiRelayWorkItem,
    ) -> Result<(), CreditedSumeragiRelayWorkItem> {
        let source = SumeragiRelaySource {
            class,
            via: work.work.authenticated_via().clone(),
        };
        let Some(source_credit) = self.source_credits.try_acquire(&source) else {
            sumeragi_relay_source_credit_invariant_fatal(class, work);
        };
        let credited = CreditedSumeragiRelayWorkItem {
            work,
            ownership: SumeragiRelayIngressOwnership::Fresh {
                source,
                geometry: self.source_credits.geometry(),
                source_credit,
            },
        };
        let result = match class {
            SumeragiRelayClass::V2 => self.v2.send(credited).await,
            SumeragiRelayClass::Lane => self.lane.send(credited).await,
        };
        result.map_err(|error| error.0)
    }

    #[cfg(feature = "test-network-message-control")]
    async fn send_rehydrated(
        &self,
        class: SumeragiRelayClass,
        work: SumeragiRelayWorkItem,
        ownership: SumeragiRelayRetention,
    ) -> Result<(), CreditedSumeragiRelayWorkItem> {
        let source = SumeragiRelaySource {
            class,
            via: work.work.authenticated_via().clone(),
        };
        if !sumeragi_rehydrated_ownership_matches(
            &source,
            self.source_credits.geometry(),
            &ownership.source,
            ownership.geometry,
        ) {
            let _exact_held_ownership = (work, ownership);
            iroha_logger::error!(
                class = class.label(),
                "controlled Sumeragi release changed its authenticated ownership key; stopping"
            );
            std::process::exit(1);
        }
        let credited = CreditedSumeragiRelayWorkItem {
            work,
            ownership: SumeragiRelayIngressOwnership::Rehydrated(ownership),
        };
        let result = match class {
            SumeragiRelayClass::V2 => self.v2.send(credited).await,
            SumeragiRelayClass::Lane => self.lane.send(credited).await,
        };
        result.map_err(|error| error.0)
    }
}

#[cfg(any(test, feature = "test-network-message-control"))]
fn sumeragi_rehydrated_ownership_matches(
    expected_source: &SumeragiRelaySource,
    expected_geometry: SumeragiRelayCapacityGeometry,
    retained_source: &SumeragiRelaySource,
    retained_geometry: SumeragiRelayCapacityGeometry,
) -> bool {
    retained_source == expected_source && retained_geometry == expected_geometry
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SumeragiRelaySource {
    class: SumeragiRelayClass,
    via: PeerId,
}

#[derive(Clone)]
struct SumeragiRelaySourceCredits {
    geometry: SumeragiRelayCapacityGeometry,
    by_source: Arc<Mutex<BTreeMap<SumeragiRelaySource, Weak<Semaphore>>>>,
}

impl SumeragiRelaySourceCredits {
    fn new(geometry: SumeragiRelayCapacityGeometry) -> Self {
        assert!(geometry.daemon_source_capacity_matches_two_upstream_lanes());
        assert!(geometry.class_corridor_covers_authenticated_sources());
        Self {
            geometry,
            by_source: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    const fn geometry(&self) -> SumeragiRelayCapacityGeometry {
        self.geometry
    }

    fn semaphore(&self, source: &SumeragiRelaySource) -> Arc<Semaphore> {
        let mut by_source = self
            .by_source
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        by_source.retain(|_, semaphore| semaphore.strong_count() != 0);
        let existing = by_source.get(source).and_then(Weak::upgrade);
        existing.unwrap_or_else(|| {
            let semaphore = Arc::new(Semaphore::new(self.geometry.daemon_per_source));
            by_source.insert(source.clone(), Arc::downgrade(&semaphore));
            semaphore
        })
    }

    #[cfg(test)]
    async fn acquire(&self, source: &SumeragiRelaySource) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore(source)
            .acquire_owned()
            .await
            .expect("daemon Sumeragi source-credit semaphores are never closed")
    }

    fn try_acquire(
        &self,
        source: &SumeragiRelaySource,
    ) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.semaphore(source).try_acquire_owned().ok()
    }

    #[cfg(test)]
    fn live_sources(&self) -> usize {
        self.by_source
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .filter(|semaphore| semaphore.strong_count() != 0)
            .count()
    }

    #[cfg(all(test, feature = "test-network-message-control"))]
    fn available_permits(&self, source: &SumeragiRelaySource) -> usize {
        self.semaphore(source).available_permits()
    }
}

enum PreparedSumeragiRelayItem {
    Block(Box<InboundBlockMessage>),
    Lane(Box<LaneRelayMessage>),
}

/// Complete layered ownership retained from daemon admission to a terminal outcome.
struct SumeragiRelayRetention {
    /// Exact class and authenticated source owning both retained layers.
    source: SumeragiRelaySource,
    /// Exact checked corridor geometry under which this occurrence was admitted.
    geometry: SumeragiRelayCapacityGeometry,
    _p2p: iroha_p2p::peer::message::PeerMessageRetentionGuard,
    _daemon_source_credit: tokio::sync::OwnedSemaphorePermit,
}

#[cfg(feature = "test-network-message-control")]
struct HeldSumeragiRelayOwnership {
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
}

struct PreparedSumeragiRelayWork {
    source: SumeragiRelaySource,
    item: PreparedSumeragiRelayItem,
    /// Original exact route retained independently of every ingress item variant.
    reply_route: iroha_p2p::network::NetworkReplyRoute,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    retry_eligible_at: Instant,
}

/// Bounded round-robin owner for retryable work, keyed by class and authenticated source.
struct FairRetainedQueue<K, T> {
    capacity: usize,
    source_capacity: usize,
    len: usize,
    lanes: BTreeMap<K, VecDeque<T>>,
    ready: VecDeque<K>,
}

struct FairRetainedSelection<K, T> {
    trace: FairRetainedSelectionTrace<K>,
    item: T,
}

struct FairRetainedSelectionTrace<K> {
    source: K,
    selected_eligible: bool,
    ready_sources_before: usize,
    selected_source_rank_before: usize,
    source_depth_before: usize,
    selected_item_rank_before: usize,
    total_depth_before: usize,
}

enum FairRetainedPushError<T> {
    Full(T),
    SourceFull(T),
}

#[cfg(test)]
impl<T> FairRetainedPushError<T> {
    fn into_item(self) -> T {
        match self {
            Self::Full(item) | Self::SourceFull(item) => item,
        }
    }
}

impl<K: Clone + Ord, T> FairRetainedQueue<K, T> {
    fn new(capacity: usize, source_capacity: usize) -> Self {
        assert!(capacity > 0, "retained Sumeragi capacity must be non-zero");
        assert!(
            source_capacity > 0 && source_capacity < capacity,
            "retained per-source capacity must be non-zero and leave a source reserve"
        );
        Self {
            capacity,
            source_capacity,
            len: 0,
            lanes: BTreeMap::new(),
            ready: VecDeque::new(),
        }
    }

    fn has_capacity(&self) -> bool {
        self.len < self.capacity
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push(&mut self, key: K, item: T) -> Result<(), FairRetainedPushError<T>> {
        if !self.has_capacity() {
            return Err(FairRetainedPushError::Full(item));
        }
        let lane = self.lanes.entry(key.clone()).or_default();
        if lane.len() >= self.source_capacity {
            return Err(FairRetainedPushError::SourceFull(item));
        }
        if lane.is_empty() {
            self.ready.push_back(key);
        }
        lane.push_back(item);
        self.len += 1;
        debug_assert!(self.len <= self.capacity);
        Ok(())
    }

    #[cfg(test)]
    fn pop(&mut self) -> Option<T> {
        self.pop_if(|_| true)
    }

    #[cfg(test)]
    fn pop_if(&mut self, mut eligible: impl FnMut(&T) -> bool) -> Option<T> {
        self.pop_if_with_trace(&mut eligible)
            .map(|selection| selection.item)
    }

    fn pop_if_with_trace(
        &mut self,
        mut eligible: impl FnMut(&T) -> bool,
    ) -> Option<FairRetainedSelection<K, T>> {
        let turns = self.ready.len();
        let total_depth_before = self.len;
        for selected_source_rank_before in 0..turns {
            let key = self
                .ready
                .pop_front()
                .expect("snapshotted retained source must remain ready");
            let (source_depth_before, eligible_index) = self
                .lanes
                .get(&key)
                .map(|lane| (lane.len(), lane.iter().position(&mut eligible)))
                .expect("ready retained source must own a lane");
            let Some(eligible_index) = eligible_index else {
                self.ready.push_back(key);
                continue;
            };
            let lane = self
                .lanes
                .get_mut(&key)
                .expect("ready retained source must own a lane");
            let item = lane
                .remove(eligible_index)
                .expect("selected retained source item must remain queued");
            self.len = self
                .len
                .checked_sub(1)
                .expect("retained queue length cannot underflow");
            if lane.is_empty() {
                self.lanes.remove(&key);
            } else {
                self.ready.push_back(key.clone());
            }
            return Some(FairRetainedSelection {
                item,
                trace: FairRetainedSelectionTrace {
                    source: key,
                    selected_eligible: true,
                    ready_sources_before: turns,
                    selected_source_rank_before,
                    source_depth_before,
                    selected_item_rank_before: eligible_index,
                    total_depth_before,
                },
            });
        }
        None
    }
}

enum PrepareSumeragiRelayResult {
    Prepared(Box<PreparedSumeragiRelayWork>),
    /// Test control atomically owns the occurrence until an explicit release.
    #[cfg(feature = "test-network-message-control")]
    Held,
    /// Test control either owns the exact token or has already released it at
    /// a terminal drop/rejection boundary.
    #[cfg(feature = "test-network-message-control")]
    Controlled {
        outcome: SumeragiRelayTerminalOutcome,
        completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    },
    Terminal {
        outcome: SumeragiRelayTerminalOutcome,
        retention_guard: SumeragiRelayRetention,
        completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    },
}

enum SumeragiRelayAttempt {
    Terminal {
        outcome: SumeragiRelayTerminalOutcome,
        retention_guard: SumeragiRelayRetention,
        completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    },
    Retry(PreparedSumeragiRelayWork),
    Fatal {
        source: SumeragiRelaySource,
        reason: SumeragiRelayFatalReason,
        exact_item: Option<PreparedSumeragiRelayItem>,
        reply_route: iroha_p2p::network::NetworkReplyRoute,
        retention_guard: SumeragiRelayRetention,
        completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SumeragiRelayFatalReason {
    IngressClosed,
    FailStop,
    BlockingTaskPanicked,
    BlockingTaskCancelled,
    BlockingTaskFailed,
}

impl SumeragiRelayFatalReason {
    const fn label(self) -> &'static str {
        match self {
            Self::IngressClosed => "closed",
            Self::FailStop => "fail_stop",
            Self::BlockingTaskPanicked => "blocking_task_panicked",
            Self::BlockingTaskCancelled => "blocking_task_cancelled",
            Self::BlockingTaskFailed => "blocking_task_failed",
        }
    }
}

/// Maximum number of consecutive high-priority messages before yielding to an ordinary lane.
const RELAY_HIGH_BURST: usize = 32;
/// Upper bound between attempts while serialized Sumeragi ingress remains temporarily unavailable.
const SUMERAGI_RELAY_RETRY_CADENCE: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsensusIngressDropReason {
    Rate,
    Bytes,
    Penalty,
}

impl ConsensusIngressDropReason {
    fn label(self) -> &'static str {
        match self {
            Self::Rate => "rate",
            Self::Bytes => "bytes",
            Self::Penalty => "penalty",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LowPriorityIngressDropReason {
    Rate,
    Bytes,
}

impl LowPriorityIngressDropReason {
    fn label(self) -> &'static str {
        match self {
            Self::Rate => "rate",
            Self::Bytes => "bytes",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BucketConfig {
    rate_per_sec: std::num::NonZeroU32,
    burst: std::num::NonZeroU32,
}

#[derive(Clone, Copy, Debug)]
struct PenaltyConfig {
    threshold: u32,
    window: Duration,
    cooldown: Duration,
}

struct ConsensusIngressLimiter {
    msg_rate: Option<BucketConfig>,
    bytes_rate: Option<BucketConfig>,
    bulk_msg_rate: Option<BucketConfig>,
    bulk_bytes_rate: Option<BucketConfig>,
    critical_msg_rate: Option<BucketConfig>,
    critical_bytes_rate: Option<BucketConfig>,
    penalty: PenaltyConfig,
    peers: HashMap<PeerId, PeerIngressState>,
}

struct PeerIngressState {
    msg_bucket: Option<TokenBucket>,
    bytes_bucket: Option<TokenBucket>,
    bulk_msg_bucket: Option<TokenBucket>,
    bulk_bytes_bucket: Option<TokenBucket>,
    critical_msg_bucket: Option<TokenBucket>,
    critical_bytes_bucket: Option<TokenBucket>,
    penalty: PenaltyTracker,
}

struct LowPriorityIngressLimiter {
    msg_rate: Option<BucketConfig>,
    bytes_rate: Option<BucketConfig>,
    peers: HashMap<PeerId, LowPriorityPeerState>,
}

struct LowPriorityPeerState {
    msg_bucket: Option<TokenBucket>,
    bytes_bucket: Option<TokenBucket>,
}

#[derive(Debug)]
struct PenaltyTracker {
    threshold: u32,
    window: Duration,
    cooldown: Duration,
    window_start: Option<Instant>,
    count: u32,
    cooldown_until: Option<Instant>,
}

#[derive(Debug)]
struct TokenBucket {
    rate_per_sec: f64,
    capacity: f64,
    tokens: f64,
    last_refill: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IngressRateClass {
    Limited,
    Bulk,
    Critical,
}

#[derive(Clone, Copy, Debug)]
struct IngressPolicy {
    rate_class: Option<IngressRateClass>,
    apply_penalty: bool,
}

impl IngressPolicy {
    const fn limited() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Limited),
            apply_penalty: true,
        }
    }

    const fn bulk() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Bulk),
            apply_penalty: false,
        }
    }

    const fn critical() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Critical),
            apply_penalty: false,
        }
    }
}

impl BucketConfig {
    fn scaled(self, factor: u32) -> Self {
        let factor = factor.max(1);
        let rate = self.rate_per_sec.get().saturating_mul(factor);
        let burst = self.burst.get().saturating_mul(factor);
        Self {
            rate_per_sec: std::num::NonZeroU32::new(rate).unwrap_or(self.rate_per_sec),
            burst: std::num::NonZeroU32::new(burst).unwrap_or(self.burst),
        }
    }
}

impl ConsensusIngressLimiter {
    fn ingress_policy(msg: &iroha_core::NetworkMessage) -> IngressPolicy {
        use iroha_core::sumeragi::message::BlockMessage;

        match msg {
            iroha_core::NetworkMessage::SumeragiBlock(block) => match block.as_ref().as_ref() {
                BlockMessage::KuraReplicaAdvert(_) => IngressPolicy::limited(),
                BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_)
                | BlockMessage::LaneBlockCertificate(_)
                | BlockMessage::LaneHistoricalRecoveryRequest(_)
                | BlockMessage::LaneExecutablePayload(_)
                | BlockMessage::LaneBlockNewViewVote(_)
                | BlockMessage::LaneBlockNewViewCertificate(_) => IngressPolicy::critical(),
                BlockMessage::LaneHistoricalRecoveryResponse(_) => IngressPolicy::bulk(),
                BlockMessage::V2(message) => {
                    use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

                    match &message.payload {
                        ConsensusMessageV2Payload::CertifiedBodyResponse(_) => {
                            IngressPolicy::bulk()
                        }
                        ConsensusMessageV2Payload::PayloadChunk(_) => IngressPolicy::critical(),
                        ConsensusMessageV2Payload::Proposal(_)
                        | ConsensusMessageV2Payload::Vote(_)
                        | ConsensusMessageV2Payload::QuorumCertificate(_)
                        | ConsensusMessageV2Payload::TimeoutVote(_)
                        | ConsensusMessageV2Payload::TimeoutCertificate(_)
                        | ConsensusMessageV2Payload::PayloadManifest(_)
                        | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                        | ConsensusMessageV2Payload::CommitCertificateRequest(_)
                        | ConsensusMessageV2Payload::CommitCertificateResponse(_)
                        | ConsensusMessageV2Payload::VrfCommit(_)
                        | ConsensusMessageV2Payload::VrfReveal(_) => IngressPolicy::critical(),
                    }
                }
                // All other block messages are decode-only v1 artifacts. The relay
                // rejects them before ingress accounting; keeping this fallback
                // separate prevents archival types from shaping live v2 limits.
                _ => IngressPolicy::limited(),
            },
            iroha_core::NetworkMessage::SumeragiControlFlow(_)
            | iroha_core::NetworkMessage::LaneDrainVote(_)
            | iroha_core::NetworkMessage::NativeAmx(_) => IngressPolicy::critical(),
            iroha_core::NetworkMessage::CertifiedMergeSidecar(message) => match message.as_ref() {
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Request(_)
                | iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Close(_) => {
                    IngressPolicy::limited()
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::CloseAck(_)
                | iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::GenerationHint(_) => {
                    IngressPolicy::critical()
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Chunk(_) => {
                    IngressPolicy::bulk()
                }
            },
            iroha_core::NetworkMessage::BlockSync(_) => IngressPolicy::bulk(),
            _ => IngressPolicy::limited(),
        }
    }

    fn from_config(
        network: &iroha_config::parameters::actual::Network,
        signed_block_cadence: Duration,
    ) -> Self {
        let msg_rate = network
            .consensus_ingress_rate_per_sec
            .map(|rate| BucketConfig {
                rate_per_sec: rate,
                burst: network.consensus_ingress_burst.unwrap_or(rate),
            });
        let bytes_rate = network
            .consensus_ingress_bytes_per_sec
            .map(|rate| BucketConfig {
                rate_per_sec: rate,
                burst: network.consensus_ingress_bytes_burst.unwrap_or(rate),
            });
        let bulk_scale = Self::bulk_scale_factor(signed_block_cadence);
        let bulk_msg_rate = msg_rate.map(|cfg| cfg.scaled(bulk_scale));
        let bulk_bytes_rate = bytes_rate.map(|cfg| cfg.scaled(bulk_scale));
        let critical_msg_rate =
            network
                .consensus_ingress_critical_rate_per_sec
                .map(|rate| BucketConfig {
                    rate_per_sec: rate,
                    burst: network.consensus_ingress_critical_burst.unwrap_or(rate),
                });
        let critical_bytes_rate = network
            .consensus_ingress_critical_bytes_per_sec
            .map(|rate| BucketConfig {
                rate_per_sec: rate,
                burst: network
                    .consensus_ingress_critical_bytes_burst
                    .unwrap_or(rate),
            });
        let penalty = PenaltyConfig {
            threshold: network.consensus_ingress_penalty_threshold,
            window: network.consensus_ingress_penalty_window,
            cooldown: network.consensus_ingress_penalty_cooldown,
        };
        Self::new(
            msg_rate,
            bytes_rate,
            bulk_msg_rate,
            bulk_bytes_rate,
            critical_msg_rate,
            critical_bytes_rate,
            penalty,
        )
    }

    fn bulk_scale_factor(block_time: Duration) -> u32 {
        let base_ms =
            u128::from(iroha_config::parameters::defaults::sumeragi::BLOCK_CADENCE_MS).max(1);
        let block_ms = block_time.as_millis().max(1);
        let scale = base_ms.div_ceil(block_ms);
        u32::try_from(scale).unwrap_or(u32::MAX).max(1)
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        msg_rate: Option<BucketConfig>,
        bytes_rate: Option<BucketConfig>,
        bulk_msg_rate: Option<BucketConfig>,
        bulk_bytes_rate: Option<BucketConfig>,
        critical_msg_rate: Option<BucketConfig>,
        critical_bytes_rate: Option<BucketConfig>,
        penalty: PenaltyConfig,
    ) -> Self {
        Self {
            msg_rate,
            bytes_rate,
            bulk_msg_rate,
            bulk_bytes_rate,
            critical_msg_rate,
            critical_bytes_rate,
            penalty,
            peers: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn should_drop(
        &mut self,
        peer: &Peer,
        msg: &iroha_core::NetworkMessage,
        size_bytes: usize,
    ) -> Option<ConsensusIngressDropReason> {
        self.should_drop_from(peer.id(), msg, size_bytes)
    }

    fn should_drop_from(
        &mut self,
        authenticated_via: &PeerId,
        msg: &iroha_core::NetworkMessage,
        size_bytes: usize,
    ) -> Option<ConsensusIngressDropReason> {
        if matches!(msg, iroha_core::NetworkMessage::LaneDrainVote(_))
            && size_bytes > iroha_core::MAX_LANE_DRAIN_VOTE_WIRE_BYTES
        {
            return Some(ConsensusIngressDropReason::Bytes);
        }
        if matches!(
            msg,
            iroha_core::NetworkMessage::SumeragiBlock(block)
                if matches!(
                    block.as_ref().as_ref(),
                    iroha_core::sumeragi::message::BlockMessage::KuraReplicaAdvert(_)
                )
        ) && size_bytes > iroha_core::MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES
        {
            return Some(ConsensusIngressDropReason::Bytes);
        }
        let policy = Self::ingress_policy(msg);
        let apply_penalty = policy.apply_penalty
            || (policy.rate_class == Some(IngressRateClass::Critical)
                && self.critical_msg_rate.is_none()
                && self.critical_bytes_rate.is_none());
        let now = Instant::now();
        let entry = self
            .peers
            .entry(authenticated_via.clone())
            .or_insert_with(|| {
                PeerIngressState::new(
                    now,
                    self.msg_rate,
                    self.bytes_rate,
                    self.bulk_msg_rate,
                    self.bulk_bytes_rate,
                    self.critical_msg_rate,
                    self.critical_bytes_rate,
                    self.penalty,
                )
            });
        if apply_penalty && entry.penalty.is_suppressed(now) {
            return Some(ConsensusIngressDropReason::Penalty);
        }
        if let Some(rate_class) = policy.rate_class {
            if let Some(bucket) = entry.msg_bucket_for(rate_class)
                && !bucket.allow(1.0, now)
            {
                if apply_penalty {
                    entry.penalty.note_violation(now);
                }
                return Some(ConsensusIngressDropReason::Rate);
            }
            let size_bytes_f64 = f64::from(u32::try_from(size_bytes).unwrap_or(u32::MAX));
            if let Some(bucket) = entry.bytes_bucket_for(rate_class)
                && !bucket.allow(size_bytes_f64, now)
            {
                if apply_penalty {
                    entry.penalty.note_violation(now);
                }
                return Some(ConsensusIngressDropReason::Bytes);
            }
        }
        None
    }
}

impl PeerIngressState {
    #[allow(clippy::too_many_arguments)]
    fn new(
        now: Instant,
        msg_rate: Option<BucketConfig>,
        bytes_rate: Option<BucketConfig>,
        bulk_msg_rate: Option<BucketConfig>,
        bulk_bytes_rate: Option<BucketConfig>,
        critical_msg_rate: Option<BucketConfig>,
        critical_bytes_rate: Option<BucketConfig>,
        penalty: PenaltyConfig,
    ) -> Self {
        Self {
            msg_bucket: msg_rate.map(|cfg| TokenBucket::new(cfg, now)),
            bytes_bucket: bytes_rate.map(|cfg| TokenBucket::new(cfg, now)),
            bulk_msg_bucket: bulk_msg_rate.map(|cfg| TokenBucket::new(cfg, now)),
            bulk_bytes_bucket: bulk_bytes_rate.map(|cfg| TokenBucket::new(cfg, now)),
            critical_msg_bucket: critical_msg_rate.map(|cfg| TokenBucket::new(cfg, now)),
            critical_bytes_bucket: critical_bytes_rate.map(|cfg| TokenBucket::new(cfg, now)),
            penalty: PenaltyTracker::new(penalty),
        }
    }

    fn msg_bucket_for(&mut self, class: IngressRateClass) -> Option<&mut TokenBucket> {
        match class {
            IngressRateClass::Limited => self.msg_bucket.as_mut(),
            IngressRateClass::Bulk => self.bulk_msg_bucket.as_mut(),
            IngressRateClass::Critical => {
                if self.critical_msg_bucket.is_some() {
                    self.critical_msg_bucket.as_mut()
                } else {
                    self.msg_bucket.as_mut()
                }
            }
        }
    }

    fn bytes_bucket_for(&mut self, class: IngressRateClass) -> Option<&mut TokenBucket> {
        match class {
            IngressRateClass::Limited => self.bytes_bucket.as_mut(),
            IngressRateClass::Bulk => self.bulk_bytes_bucket.as_mut(),
            IngressRateClass::Critical => {
                if self.critical_bytes_bucket.is_some() {
                    self.critical_bytes_bucket.as_mut()
                } else {
                    self.bytes_bucket.as_mut()
                }
            }
        }
    }
}

impl LowPriorityIngressLimiter {
    fn from_config(network: &iroha_config::parameters::actual::Network) -> Self {
        let msg_rate = network.low_priority_rate_per_sec.map(|rate| BucketConfig {
            rate_per_sec: rate,
            burst: network.low_priority_burst.unwrap_or(rate),
        });
        let bytes_rate = network.low_priority_bytes_per_sec.map(|rate| BucketConfig {
            rate_per_sec: rate,
            burst: network.low_priority_bytes_burst.unwrap_or(rate),
        });
        Self::new(msg_rate, bytes_rate)
    }

    fn new(msg_rate: Option<BucketConfig>, bytes_rate: Option<BucketConfig>) -> Self {
        Self {
            msg_rate,
            bytes_rate,
            peers: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn should_drop(
        &mut self,
        peer: &Peer,
        size_bytes: usize,
    ) -> Option<LowPriorityIngressDropReason> {
        self.should_drop_from(peer.id(), size_bytes)
    }

    fn should_drop_from(
        &mut self,
        authenticated_via: &PeerId,
        size_bytes: usize,
    ) -> Option<LowPriorityIngressDropReason> {
        if self.msg_rate.is_none() && self.bytes_rate.is_none() {
            return None;
        }
        let now = Instant::now();
        let entry = self
            .peers
            .entry(authenticated_via.clone())
            .or_insert_with(|| LowPriorityPeerState::new(now, self.msg_rate, self.bytes_rate));
        if let Some(bucket) = entry.msg_bucket.as_mut()
            && !bucket.allow(1.0, now)
        {
            return Some(LowPriorityIngressDropReason::Rate);
        }
        let size_bytes_f64 = f64::from(u32::try_from(size_bytes).unwrap_or(u32::MAX));
        if let Some(bucket) = entry.bytes_bucket.as_mut()
            && !bucket.allow(size_bytes_f64, now)
        {
            return Some(LowPriorityIngressDropReason::Bytes);
        }
        None
    }
}

impl LowPriorityPeerState {
    fn new(now: Instant, msg_rate: Option<BucketConfig>, bytes_rate: Option<BucketConfig>) -> Self {
        Self {
            msg_bucket: msg_rate.map(|cfg| TokenBucket::new(cfg, now)),
            bytes_bucket: bytes_rate.map(|cfg| TokenBucket::new(cfg, now)),
        }
    }
}

impl PenaltyTracker {
    fn new(config: PenaltyConfig) -> Self {
        Self {
            threshold: config.threshold,
            window: config.window,
            cooldown: config.cooldown,
            window_start: None,
            count: 0,
            cooldown_until: None,
        }
    }

    fn is_suppressed(&mut self, now: Instant) -> bool {
        if self.threshold == 0 {
            return false;
        }
        if let Some(until) = self.cooldown_until {
            if now < until {
                return true;
            }
            self.cooldown_until = None;
        }
        false
    }

    fn note_violation(&mut self, now: Instant) {
        if self.threshold == 0 {
            return;
        }
        let window_expired = self
            .window_start
            .is_none_or(|start| now.saturating_duration_since(start) > self.window);
        if window_expired {
            self.window_start = Some(now);
            self.count = 0;
        }
        self.count = self.count.saturating_add(1);
        if self.count >= self.threshold {
            self.count = 0;
            self.window_start = Some(now);
            if !self.cooldown.is_zero() {
                self.cooldown_until = Some(now.checked_add(self.cooldown).unwrap_or(now));
            }
        }
    }
}

impl TokenBucket {
    fn new(config: BucketConfig, now: Instant) -> Self {
        let capacity = f64::from(config.burst.get());
        Self {
            rate_per_sec: f64::from(config.rate_per_sec.get()),
            capacity,
            tokens: capacity,
            last_refill: now,
        }
    }

    fn allow(&mut self, cost: f64, now: Instant) -> bool {
        if cost <= 0.0 {
            return true;
        }
        self.refill(now);
        if cost > self.capacity {
            return false;
        }
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill);
        if elapsed.is_zero() {
            return;
        }
        let added = elapsed.as_secs_f64() * self.rate_per_sec;
        self.tokens = (self.tokens + added).min(self.capacity);
        self.last_refill = now;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelayReceiverKind {
    High,
    Payload,
    Chunk,
    Low,
}

impl RelayReceiverKind {
    const fn label(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Payload => "payload",
            Self::Chunk => "chunk",
            Self::Low => "low",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelayIngressLoopExit {
    ReceiverClosed(RelayReceiverKind),
    WorkerClosed(RelayReceiverKind),
}

enum RelayReceiverEvent<T> {
    Message(RelayReceiverKind, T),
    Closed,
    AllClosed,
}

const RELAY_ORDINARY_RECEIVERS: [RelayReceiverKind; 3] = [
    RelayReceiverKind::Payload,
    RelayReceiverKind::Chunk,
    RelayReceiverKind::Low,
];

/// Fair, priority-aware receiver set shared by the relay ingress and worker stages.
///
/// The high-priority lane may run for at most `high_burst` consecutive admissions while any
/// ordinary lane is ready. Ordinary lanes are selected with a persistent round-robin cursor, so a
/// continuously ready payload lane cannot starve chunk or low-priority work. Closed lanes remain
/// eligible until Tokio reports that their buffered messages have been drained.
struct OrdinaryRelayReceiverState {
    payload_open: bool,
    chunk_open: bool,
    low_open: bool,
}

struct FairRelayReceivers<T> {
    high: mpsc::Receiver<T>,
    payload: mpsc::Receiver<T>,
    chunk: mpsc::Receiver<T>,
    low: mpsc::Receiver<T>,
    high_open: bool,
    ordinary_state: OrdinaryRelayReceiverState,
    consecutive_high: usize,
    next_ordinary: usize,
    high_burst: usize,
}

impl<T: Send> FairRelayReceivers<T> {
    fn new(
        high: mpsc::Receiver<T>,
        payload: mpsc::Receiver<T>,
        chunk: mpsc::Receiver<T>,
        low: mpsc::Receiver<T>,
        high_burst: usize,
    ) -> Self {
        assert!(high_burst > 0, "relay high-priority burst must be non-zero");
        Self {
            high,
            payload,
            chunk,
            low,
            high_open: true,
            ordinary_state: OrdinaryRelayReceiverState {
                payload_open: true,
                chunk_open: true,
                low_open: true,
            },
            consecutive_high: 0,
            next_ordinary: 0,
            high_burst,
        }
    }

    async fn recv(&mut self) -> Option<(RelayReceiverKind, T)> {
        loop {
            match self.recv_event().await {
                RelayReceiverEvent::Message(kind, message) => return Some((kind, message)),
                RelayReceiverEvent::Closed => {}
                RelayReceiverEvent::AllClosed => return None,
            }
        }
    }

    async fn recv_event(&mut self) -> RelayReceiverEvent<T> {
        std::future::poll_fn(|cx| self.poll_recv_event(cx)).await
    }

    fn poll_recv_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<RelayReceiverEvent<T>> {
        let ordinary_due = self.consecutive_high >= self.high_burst;
        if ordinary_due && let Some(event) = self.poll_ordinary(cx) {
            return std::task::Poll::Ready(event);
        }

        if let Some(event) = Self::poll_receiver(
            RelayReceiverKind::High,
            &mut self.high,
            &mut self.high_open,
            cx,
        ) {
            if matches!(&event, RelayReceiverEvent::Message(_, _)) {
                self.consecutive_high = self.consecutive_high.saturating_add(1);
            }
            return std::task::Poll::Ready(event);
        }

        if !ordinary_due && let Some(event) = self.poll_ordinary(cx) {
            return std::task::Poll::Ready(event);
        }

        if self.high_open
            || self.ordinary_state.payload_open
            || self.ordinary_state.chunk_open
            || self.ordinary_state.low_open
        {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(RelayReceiverEvent::AllClosed)
        }
    }

    fn poll_ordinary(&mut self, cx: &mut std::task::Context<'_>) -> Option<RelayReceiverEvent<T>> {
        for offset in 0..RELAY_ORDINARY_RECEIVERS.len() {
            let index = (self.next_ordinary + offset) % RELAY_ORDINARY_RECEIVERS.len();
            let kind = RELAY_ORDINARY_RECEIVERS[index];
            let event = match kind {
                RelayReceiverKind::Payload => Self::poll_receiver(
                    kind,
                    &mut self.payload,
                    &mut self.ordinary_state.payload_open,
                    cx,
                ),
                RelayReceiverKind::Chunk => Self::poll_receiver(
                    kind,
                    &mut self.chunk,
                    &mut self.ordinary_state.chunk_open,
                    cx,
                ),
                RelayReceiverKind::Low => {
                    Self::poll_receiver(kind, &mut self.low, &mut self.ordinary_state.low_open, cx)
                }
                RelayReceiverKind::High => unreachable!("high lane is not an ordinary relay lane"),
            };
            if let Some(event) = event {
                if matches!(&event, RelayReceiverEvent::Message(_, _)) {
                    self.consecutive_high = 0;
                    self.next_ordinary = (index + 1) % RELAY_ORDINARY_RECEIVERS.len();
                }
                return Some(event);
            }
        }
        None
    }

    fn poll_receiver(
        kind: RelayReceiverKind,
        receiver: &mut mpsc::Receiver<T>,
        open: &mut bool,
        cx: &mut std::task::Context<'_>,
    ) -> Option<RelayReceiverEvent<T>> {
        if !*open {
            return None;
        }
        match std::pin::Pin::new(receiver).poll_recv(cx) {
            std::task::Poll::Ready(Some(message)) => {
                Some(RelayReceiverEvent::Message(kind, message))
            }
            std::task::Poll::Ready(None) => {
                *open = false;
                Some(RelayReceiverEvent::Closed)
            }
            std::task::Poll::Pending => None,
        }
    }
}

fn sumeragi_relay_class(message: &iroha_core::NetworkMessage) -> Option<SumeragiRelayClass> {
    use iroha_core::NetworkMessage::*;
    use iroha_core::sumeragi::message::BlockMessage;

    match message {
        SumeragiBlock(block) => match block.as_ref().as_ref() {
            BlockMessage::V2(_) | BlockMessage::KuraReplicaAdvert(_) => {
                Some(SumeragiRelayClass::V2)
            }
            BlockMessage::LaneBlockProposal(_)
            | BlockMessage::LaneExecutablePayload(_)
            | BlockMessage::LaneBlockNewViewVote(_)
            | BlockMessage::LaneBlockNewViewCertificate(_)
            | BlockMessage::LaneBlockVote(_)
            | BlockMessage::LaneBlockQc(_)
            | BlockMessage::LaneBlockCertificate(_)
            | BlockMessage::LaneHistoricalRecoveryRequest(_)
            | BlockMessage::LaneHistoricalRecoveryResponse(_) => Some(SumeragiRelayClass::Lane),
            _ => None,
        },
        LaneRelay(_)
        | MergeCommitteeSignature(_)
        | LaneDrainVote(_)
        | CertifiedMergeSidecar(_)
        | NativeAmx(_) => Some(SumeragiRelayClass::Lane),
        _ => None,
    }
}

fn obsolete_sumeragi_relay_terminal_meta(
    message: &iroha_core::NetworkMessage,
) -> Option<(
    &'static str,
    Option<u64>,
    Option<u64>,
    SumeragiRelayTerminalOutcome,
)> {
    NetworkRelayShared::retired_sumeragi_message_meta(message)
        .map(|(kind, height, view)| (kind, height, view, SumeragiRelayTerminalOutcome::Delivered))
}

fn certified_merge_sidecar_ingress_reply_route(
    _message: &iroha_core::merge_sidecar::CertifiedMergeSidecarMessage,
    reply_route: iroha_p2p::network::NetworkReplyRoute,
) -> iroha_p2p::network::NetworkReplyRoute {
    reply_route
}

struct RetainedSumeragiRelayParts {
    peer: Peer,
    authenticated_via: PeerId,
    message: iroha_core::NetworkMessage,
    size_bytes: usize,
    reply_route: Option<iroha_p2p::network::NetworkReplyRoute>,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    #[cfg(feature = "test-network-message-control")]
    skip_test_control: bool,
}

struct AuthorizedSumeragiRelayParts {
    peer: Peer,
    authenticated_via: PeerId,
    message: iroha_core::NetworkMessage,
    size_bytes: usize,
    reply_route: iroha_p2p::network::NetworkReplyRoute,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
}

struct SumeragiRelayBuildContext {
    peer: Peer,
    authenticated_via: PeerId,
    size_bytes: usize,
    reply_route: iroha_p2p::network::NetworkReplyRoute,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
}

impl AuthorizedSumeragiRelayParts {
    fn into_message_and_context(self) -> (iroha_core::NetworkMessage, SumeragiRelayBuildContext) {
        (
            self.message,
            SumeragiRelayBuildContext {
                peer: self.peer,
                authenticated_via: self.authenticated_via,
                size_bytes: self.size_bytes,
                reply_route: self.reply_route,
                retention_guard: self.retention_guard,
                completion: self.completion,
            },
        )
    }
}

impl SumeragiRelayBuildContext {
    fn terminal(self, outcome: SumeragiRelayTerminalOutcome) -> PrepareSumeragiRelayResult {
        PrepareSumeragiRelayResult::Terminal {
            outcome,
            retention_guard: self.retention_guard,
            completion: self.completion,
        }
    }

    fn prepared(
        self,
        class: SumeragiRelayClass,
        item: PreparedSumeragiRelayItem,
    ) -> PrepareSumeragiRelayResult {
        let source = self.retention_guard.source.clone();
        debug_assert_eq!(source.class, class);
        debug_assert_eq!(source.via, self.authenticated_via);
        PrepareSumeragiRelayResult::Prepared(Box::new(PreparedSumeragiRelayWork {
            source,
            item,
            reply_route: self.reply_route,
            retention_guard: self.retention_guard,
            completion: self.completion,
            retry_eligible_at: Instant::now(),
        }))
    }
}

fn retain_sumeragi_relay_parts(work: CreditedSumeragiRelayWorkItem) -> RetainedSumeragiRelayParts {
    let CreditedSumeragiRelayWorkItem {
        work: relay_work,
        ownership,
    } = work;
    let SumeragiRelayWorkItem {
        work,
        completion,
        #[cfg(feature = "test-network-message-control")]
        skip_test_control,
        ..
    } = relay_work;
    let (peer, authenticated_via, message, size_bytes, reply_route, p2p_retention_guard) =
        work.into_parts_with_reply_route();
    let ownership_source = match &ownership {
        SumeragiRelayIngressOwnership::Fresh { source, .. } => source,
        #[cfg(feature = "test-network-message-control")]
        SumeragiRelayIngressOwnership::Rehydrated(retention) => &retention.source,
    };
    if ownership_source.via != authenticated_via
        || sumeragi_relay_class(&message) != Some(ownership_source.class)
    {
        let _exact_owned_occurrence = (
            peer,
            authenticated_via,
            message,
            size_bytes,
            reply_route,
            p2p_retention_guard,
            ownership,
            completion,
        );
        iroha_logger::error!(
            "retained Sumeragi occurrence changed its class or authenticated owner; stopping"
        );
        std::process::exit(1);
    }
    let retention_guard = match ownership {
        SumeragiRelayIngressOwnership::Fresh {
            source,
            geometry,
            source_credit,
        } => SumeragiRelayRetention {
            source,
            geometry,
            _p2p: p2p_retention_guard,
            _daemon_source_credit: source_credit,
        },
        #[cfg(feature = "test-network-message-control")]
        SumeragiRelayIngressOwnership::Rehydrated(retention) => {
            drop(p2p_retention_guard);
            retention
        }
    };
    RetainedSumeragiRelayParts {
        peer,
        authenticated_via,
        message,
        size_bytes,
        reply_route,
        retention_guard,
        completion,
        #[cfg(feature = "test-network-message-control")]
        skip_test_control,
    }
}

#[cfg(feature = "test-network-message-control")]
fn log_sumeragi_relay_preparation_boundary_error(error: &SumeragiRelayPreparationBoundaryError) {
    match error {
        SumeragiRelayPreparationBoundaryError::InvalidPassDisposition => {
            iroha_logger::error!(
                "test-network consensus controller returned an invalid disposition; rejecting the exact inbound copy"
            );
        }
        SumeragiRelayPreparationBoundaryError::Controller(error) => {
            iroha_logger::error!(
                reason = error.code(),
                "test-network consensus controller rejected admission; rejecting the exact inbound copy"
            );
        }
        SumeragiRelayPreparationBoundaryError::MissingReplyRoute => {
            iroha_logger::error!(
                "rejecting retained Sumeragi message without authenticated reply authority"
            );
        }
    }
}

fn prepare_sumeragi_block_relay_item(
    context: SumeragiRelayBuildContext,
    data: Arc<iroha_core::sumeragi::message::BlockMessageWire>,
) -> PrepareSumeragiRelayResult {
    let (kind, height, view) = NetworkRelayShared::block_message_meta(data.as_ref().as_ref());
    iroha_logger::debug!(
        peer = %context.peer,
        ?height,
        ?view,
        size_bytes = context.size_bytes,
        kind,
        "retained relay received Sumeragi v2 message"
    );
    let message = Arc::unwrap_or_clone(data).into_message();
    let class = if matches!(
        message,
        iroha_core::sumeragi::message::BlockMessage::V2(_)
            | iroha_core::sumeragi::message::BlockMessage::KuraReplicaAdvert(_)
    ) {
        SumeragiRelayClass::V2
    } else {
        SumeragiRelayClass::Lane
    };
    if let iroha_core::sumeragi::message::BlockMessage::KuraReplicaAdvert(advert) = &message
        && (context.peer.id() != &context.authenticated_via || context.peer.id() != &advert.keeper)
    {
        iroha_logger::debug!(
            peer = %context.peer,
            via = %context.authenticated_via,
            keeper = %advert.keeper,
            "rejecting Kura replica advert without a direct authenticated keeper route"
        );
        return context.terminal(SumeragiRelayTerminalOutcome::Failed);
    }
    match InboundBlockMessage::try_from_transport_with_reply_route(
        message,
        context.peer.id().clone(),
        context.authenticated_via.clone(),
        context.reply_route.clone(),
    ) {
        Ok(inbound) => context.prepared(class, PreparedSumeragiRelayItem::Block(Box::new(inbound))),
        Err(iroha_p2p::network::NetworkReplyRouteError::Inactive) => {
            context.terminal(SumeragiRelayTerminalOutcome::Retired)
        }
        Err(error) => {
            iroha_logger::error!(
                ?error,
                peer = %context.peer,
                "authenticated Sumeragi reply capability failed ingress validation"
            );
            context.terminal(SumeragiRelayTerminalOutcome::Failed)
        }
    }
}

fn prepare_sumeragi_lane_relay_item(
    context: SumeragiRelayBuildContext,
    message: iroha_core::NetworkMessage,
) -> PrepareSumeragiRelayResult {
    use iroha_core::NetworkMessage::{
        CertifiedMergeSidecar, LaneDrainVote, LaneRelay, MergeCommitteeSignature, NativeAmx,
    };

    let peer_id = context.peer.id().clone();
    let item = match message {
        LaneRelay(envelope) => LaneRelayMessage::Envelope(*envelope),
        MergeCommitteeSignature(signature) => {
            LaneRelayMessage::MergeSignature(Arc::unwrap_or_clone(signature))
        }
        CertifiedMergeSidecar(message) => {
            let reply_route = certified_merge_sidecar_ingress_reply_route(
                message.as_ref(),
                context.reply_route.clone(),
            );
            LaneRelayMessage::CertifiedMergeSidecar {
                sender: peer_id.clone(),
                reply_route: Some(reply_route),
                message: Arc::unwrap_or_clone(message),
            }
        }
        NativeAmx(message) => LaneRelayMessage::NativeAmx {
            sender: peer_id.clone(),
            reply_route: Some(context.reply_route.clone()),
            message: Arc::unwrap_or_clone(message),
        },
        LaneDrainVote(vote) => {
            let vote = *vote;
            if vote.signer != peer_id {
                iroha_logger::debug!(
                    peer = %context.peer,
                    signer = %vote.signer,
                    "rejecting lane-drain vote whose signed identity differs from its authenticated sender"
                );
                return context.terminal(SumeragiRelayTerminalOutcome::Failed);
            }
            LaneRelayMessage::DrainVote {
                sender: peer_id,
                vote,
            }
        }
        _ => {
            iroha_logger::error!(
                peer = %context.peer,
                "non-Sumeragi message reached the retained Sumeragi dispatcher"
            );
            return context.terminal(SumeragiRelayTerminalOutcome::Failed);
        }
    };
    context.prepared(
        SumeragiRelayClass::Lane,
        PreparedSumeragiRelayItem::Lane(Box::new(item)),
    )
}

impl NetworkRelayShared {
    fn authorize_sumeragi_relay_parts(
        &self,
        parts: RetainedSumeragiRelayParts,
    ) -> Result<AuthorizedSumeragiRelayParts, Box<PrepareSumeragiRelayResult>> {
        let RetainedSumeragiRelayParts {
            peer,
            authenticated_via,
            message,
            size_bytes,
            reply_route,
            retention_guard,
            completion,
            ..
        } = parts;
        let reply_route = match reply_route {
            None => {
                iroha_logger::error!(
                    %peer,
                    "rejecting retained Sumeragi message without authenticated reply authority"
                );
                return Err(Box::new(PrepareSumeragiRelayResult::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Failed,
                    retention_guard,
                    completion,
                }));
            }
            Some(reply_route) if !reply_route.is_active() => {
                iroha_logger::debug!(
                    %peer,
                    "retiring Sumeragi occurrence whose authenticated reply tenure ended before retained ingress"
                );
                return Err(Box::new(PrepareSumeragiRelayResult::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Retired,
                    retention_guard,
                    completion,
                }));
            }
            Some(reply_route) => reply_route,
        };
        if let Some((kind, height, view, outcome)) = obsolete_sumeragi_relay_terminal_meta(&message)
        {
            iroha_logger::debug!(
                %peer,
                ?height,
                ?view,
                kind,
                "rejecting retired Sumeragi message before retained ingress"
            );
            return Err(Box::new(PrepareSumeragiRelayResult::Terminal {
                outcome,
                retention_guard,
                completion,
            }));
        }
        if !self.consensus_ingress_allows(&authenticated_via, &message, size_bytes) {
            return Err(Box::new(PrepareSumeragiRelayResult::Terminal {
                outcome: SumeragiRelayTerminalOutcome::Failed,
                retention_guard,
                completion,
            }));
        }
        Ok(AuthorizedSumeragiRelayParts {
            peer,
            authenticated_via,
            message,
            size_bytes,
            reply_route,
            retention_guard,
            completion,
        })
    }

    #[cfg(feature = "test-network-message-control")]
    fn apply_sumeragi_relay_test_control(
        &self,
        parts: RetainedSumeragiRelayParts,
    ) -> Result<RetainedSumeragiRelayParts, Box<PrepareSumeragiRelayResult>> {
        let RetainedSumeragiRelayParts {
            peer,
            authenticated_via,
            message,
            size_bytes,
            reply_route,
            retention_guard,
            completion,
            skip_test_control,
        } = parts;
        match prepare_sumeragi_relay_work_boundary(
            self.test_message_control.as_deref(),
            skip_test_control,
            SumeragiRelayPreparationParts {
                peer,
                authenticated_via: authenticated_via.clone(),
                message,
                size_bytes,
                reply_route,
                ownership: HeldSumeragiRelayOwnership {
                    retention_guard,
                    completion,
                },
            },
        ) {
            SumeragiRelayPreparationBoundary::Held => {
                Err(Box::new(PrepareSumeragiRelayResult::Held))
            }
            SumeragiRelayPreparationBoundary::Dropped(ownership) => {
                Err(Box::new(PrepareSumeragiRelayResult::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Delivered,
                    retention_guard: ownership.retention_guard,
                    completion: ownership.completion,
                }))
            }
            SumeragiRelayPreparationBoundary::Prepared(parts) => {
                let SumeragiRelayPreparationParts {
                    peer,
                    message,
                    size_bytes,
                    reply_route,
                    ownership,
                    ..
                } = parts;
                Ok(RetainedSumeragiRelayParts {
                    peer,
                    authenticated_via,
                    message,
                    size_bytes,
                    reply_route,
                    retention_guard: ownership.retention_guard,
                    completion: ownership.completion,
                    skip_test_control: true,
                })
            }
            SumeragiRelayPreparationBoundary::RetiredInactiveReplyRoute(ownership) => {
                iroha_logger::debug!(
                    "retiring Sumeragi occurrence whose authenticated reply tenure ended before retained ingress"
                );
                Err(Box::new(PrepareSumeragiRelayResult::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Retired,
                    retention_guard: ownership.retention_guard,
                    completion: ownership.completion,
                }))
            }
            SumeragiRelayPreparationBoundary::Rejected { error, ownership } => {
                log_sumeragi_relay_preparation_boundary_error(&error);
                let result = ownership.map_or(
                    PrepareSumeragiRelayResult::Controlled {
                        outcome: SumeragiRelayTerminalOutcome::Failed,
                        completion: None,
                    },
                    |ownership| PrepareSumeragiRelayResult::Terminal {
                        outcome: SumeragiRelayTerminalOutcome::Failed,
                        retention_guard: ownership.retention_guard,
                        completion: ownership.completion,
                    },
                );
                Err(Box::new(result))
            }
        }
    }

    fn prepare_sumeragi_relay_work(
        &self,
        work: CreditedSumeragiRelayWorkItem,
    ) -> PrepareSumeragiRelayResult {
        let parts = retain_sumeragi_relay_parts(work);
        #[cfg(feature = "test-network-message-control")]
        let parts = match self.apply_sumeragi_relay_test_control(parts) {
            Ok(parts) => parts,
            Err(result) => return *result,
        };
        let authorized = match self.authorize_sumeragi_relay_parts(parts) {
            Ok(authorized) => authorized,
            Err(result) => return *result,
        };
        let (message, context) = authorized.into_message_and_context();
        match message {
            iroha_core::NetworkMessage::SumeragiBlock(data) => {
                prepare_sumeragi_block_relay_item(context, data)
            }
            message => prepare_sumeragi_lane_relay_item(context, message),
        }
    }
}

fn finish_sumeragi_block_ingress_attempt(
    source: SumeragiRelaySource,
    reply_route: iroha_p2p::network::NetworkReplyRoute,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
    disposition: SumeragiIngressDisposition<InboundBlockMessage>,
) -> SumeragiRelayAttempt {
    match disposition {
        SumeragiIngressDisposition::Accepted
        | SumeragiIngressDisposition::Coalesced
        | SumeragiIngressDisposition::Obsolete => SumeragiRelayAttempt::Terminal {
            outcome: SumeragiRelayTerminalOutcome::Delivered,
            retention_guard,
            completion,
        },
        SumeragiIngressDisposition::Rejected(_) => SumeragiRelayAttempt::Terminal {
            outcome: if reply_route.is_active() {
                SumeragiRelayTerminalOutcome::Failed
            } else {
                SumeragiRelayTerminalOutcome::Retired
            },
            retention_guard,
            completion,
        },
        SumeragiIngressDisposition::Retry(inbound) => {
            if !reply_route.is_active() {
                return SumeragiRelayAttempt::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Retired,
                    retention_guard,
                    completion,
                };
            }
            SumeragiRelayAttempt::Retry(PreparedSumeragiRelayWork {
                source,
                item: PreparedSumeragiRelayItem::Block(Box::new(inbound)),
                reply_route,
                retention_guard,
                completion,
                retry_eligible_at: Instant::now() + SUMERAGI_RELAY_RETRY_CADENCE,
            })
        }
        SumeragiIngressDisposition::Closed(inbound) => SumeragiRelayAttempt::Fatal {
            source,
            reason: SumeragiRelayFatalReason::IngressClosed,
            exact_item: Some(PreparedSumeragiRelayItem::Block(Box::new(inbound))),
            reply_route,
            retention_guard,
            completion,
        },
        SumeragiIngressDisposition::FailStop(inbound) => SumeragiRelayAttempt::Fatal {
            source,
            reason: SumeragiRelayFatalReason::FailStop,
            exact_item: Some(PreparedSumeragiRelayItem::Block(Box::new(inbound))),
            reply_route,
            retention_guard,
            completion,
        },
    }
}

#[cfg(test)]
fn attempt_sumeragi_block_relay_work_for_test(
    sumeragi: &SumeragiHandle,
    work: PreparedSumeragiRelayWork,
) -> SumeragiRelayAttempt {
    let PreparedSumeragiRelayWork {
        source,
        item,
        reply_route,
        retention_guard,
        completion,
        retry_eligible_at: _,
    } = work;
    let PreparedSumeragiRelayItem::Block(inbound) = item else {
        panic!("block-attempt fixture requires a block relay item");
    };
    let disposition = sumeragi.try_incoming_block_message_owned(*inbound);
    finish_sumeragi_block_ingress_attempt(
        source,
        reply_route,
        retention_guard,
        completion,
        disposition,
    )
}

fn attempt_sumeragi_lane_relay_work(
    shared: &NetworkRelayShared,
    source: SumeragiRelaySource,
    message: LaneRelayMessage,
    reply_route: iroha_p2p::network::NetworkReplyRoute,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
) -> SumeragiRelayAttempt {
    if !reply_route.is_active() {
        return SumeragiRelayAttempt::Terminal {
            outcome: SumeragiRelayTerminalOutcome::Retired,
            retention_guard,
            completion,
        };
    }
    match shared.sumeragi.try_incoming_lane_relay_owned(message) {
        SumeragiIngressDisposition::Accepted
        | SumeragiIngressDisposition::Coalesced
        | SumeragiIngressDisposition::Obsolete => SumeragiRelayAttempt::Terminal {
            outcome: SumeragiRelayTerminalOutcome::Delivered,
            retention_guard,
            completion,
        },
        SumeragiIngressDisposition::Rejected(_) => SumeragiRelayAttempt::Terminal {
            outcome: if reply_route.is_active() {
                SumeragiRelayTerminalOutcome::Failed
            } else {
                SumeragiRelayTerminalOutcome::Retired
            },
            retention_guard,
            completion,
        },
        SumeragiIngressDisposition::Retry(message) => {
            if !reply_route.is_active() {
                return SumeragiRelayAttempt::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Retired,
                    retention_guard,
                    completion,
                };
            }
            SumeragiRelayAttempt::Retry(PreparedSumeragiRelayWork {
                source,
                item: PreparedSumeragiRelayItem::Lane(Box::new(message)),
                reply_route,
                retention_guard,
                completion,
                retry_eligible_at: Instant::now() + SUMERAGI_RELAY_RETRY_CADENCE,
            })
        }
        SumeragiIngressDisposition::Closed(message) => SumeragiRelayAttempt::Fatal {
            source,
            reason: SumeragiRelayFatalReason::IngressClosed,
            exact_item: Some(PreparedSumeragiRelayItem::Lane(Box::new(message))),
            reply_route,
            retention_guard,
            completion,
        },
        SumeragiIngressDisposition::FailStop(message) => SumeragiRelayAttempt::Fatal {
            source,
            reason: SumeragiRelayFatalReason::FailStop,
            exact_item: Some(PreparedSumeragiRelayItem::Lane(Box::new(message))),
            reply_route,
            retention_guard,
            completion,
        },
    }
}

async fn attempt_sumeragi_relay_work(
    shared: &NetworkRelayShared,
    work: PreparedSumeragiRelayWork,
    shutdown_signal: &ShutdownSignal,
) -> SumeragiRelayAttempt {
    let PreparedSumeragiRelayWork {
        source,
        item,
        reply_route,
        retention_guard,
        completion,
        retry_eligible_at: _,
    } = work;
    let class = source.class;
    if sumeragi_reply_route_terminal_if_inactive(&reply_route).is_some() {
        return SumeragiRelayAttempt::Terminal {
            outcome: SumeragiRelayTerminalOutcome::Retired,
            retention_guard,
            completion,
        };
    }
    match item {
        PreparedSumeragiRelayItem::Block(inbound) => {
            let sumeragi = shared.sumeragi.clone();
            let ingress_reply_route = reply_route.clone();
            match tokio::task::spawn_blocking(move || {
                ingress_reply_route
                    .is_active()
                    .then(|| sumeragi.try_incoming_block_message_owned(*inbound))
            })
            .await
            {
                Ok(None) => SumeragiRelayAttempt::Terminal {
                    outcome: SumeragiRelayTerminalOutcome::Retired,
                    retention_guard,
                    completion,
                },
                Ok(Some(disposition)) => finish_sumeragi_block_ingress_attempt(
                    source,
                    reply_route,
                    retention_guard,
                    completion,
                    disposition,
                ),
                Err(error) => {
                    let reason = if error.is_panic() {
                        SumeragiRelayFatalReason::BlockingTaskPanicked
                    } else if error.is_cancelled() {
                        SumeragiRelayFatalReason::BlockingTaskCancelled
                    } else {
                        SumeragiRelayFatalReason::BlockingTaskFailed
                    };
                    if sumeragi_relay_failure_disposition(reason, shutdown_signal.is_sent())
                        == SumeragiRelayFailureDisposition::RetireForShutdown
                    {
                        iroha_logger::debug!(
                            ?error,
                            class = class.label(),
                            reason = reason.label(),
                            "blocking Sumeragi ingress stopped during node shutdown"
                        );
                    } else {
                        iroha_logger::error!(
                            ?error,
                            class = class.label(),
                            reason = reason.label(),
                            "blocking Sumeragi ingress failed; exact message ownership is unrecoverable"
                        );
                    }
                    SumeragiRelayAttempt::Fatal {
                        source,
                        reason,
                        exact_item: None,
                        reply_route,
                        retention_guard,
                        completion,
                    }
                }
            }
        }
        PreparedSumeragiRelayItem::Lane(message) => attempt_sumeragi_lane_relay_work(
            shared,
            source,
            *message,
            reply_route,
            retention_guard,
            completion,
        ),
    }
}

fn try_recv_sumeragi_relay_work(
    v2: &mut mpsc::Receiver<CreditedSumeragiRelayWorkItem>,
    lane: &mut mpsc::Receiver<CreditedSumeragiRelayWorkItem>,
    next: &mut SumeragiRelayClass,
    v2_open: &mut bool,
    lane_open: &mut bool,
) -> Option<CreditedSumeragiRelayWorkItem> {
    for class in [
        *next,
        match *next {
            SumeragiRelayClass::V2 => SumeragiRelayClass::Lane,
            SumeragiRelayClass::Lane => SumeragiRelayClass::V2,
        },
    ] {
        let result = match class {
            SumeragiRelayClass::V2 if *v2_open => v2.try_recv(),
            SumeragiRelayClass::Lane if *lane_open => lane.try_recv(),
            _ => continue,
        };
        match result {
            Ok(work) => {
                *next = match class {
                    SumeragiRelayClass::V2 => SumeragiRelayClass::Lane,
                    SumeragiRelayClass::Lane => SumeragiRelayClass::V2,
                };
                return Some(work);
            }
            Err(mpsc::error::TryRecvError::Empty) => {}
            Err(mpsc::error::TryRecvError::Disconnected) => match class {
                SumeragiRelayClass::V2 => *v2_open = false,
                SumeragiRelayClass::Lane => *lane_open = false,
            },
        }
    }
    None
}

fn finish_sumeragi_relay_terminal(
    outcome: SumeragiRelayTerminalOutcome,
    retention_guard: SumeragiRelayRetention,
    completion: Option<oneshot::Sender<SumeragiRelayTerminalOutcome>>,
) {
    if let Some(completion) = completion {
        let _ = completion.send(outcome);
    }
    drop(retention_guard);
}

fn retain_sumeragi_work(
    retained: &mut FairRetainedQueue<SumeragiRelaySource, PreparedSumeragiRelayWork>,
    work: PreparedSumeragiRelayWork,
) -> Result<(), Box<PreparedSumeragiRelayWork>> {
    if let Some(outcome) = sumeragi_reply_route_terminal_if_inactive(&work.reply_route) {
        let PreparedSumeragiRelayWork {
            item,
            reply_route,
            retention_guard,
            completion,
            ..
        } = work;
        if let Some(completion) = completion {
            let _ = completion.send(outcome);
        }
        drop(item);
        drop(reply_route);
        drop(retention_guard);
        return Ok(());
    }
    let source = work.source.clone();
    retained.push(source, work).map_err(|error| {
        Box::new(match error {
            FairRetainedPushError::Full(work) | FairRetainedPushError::SourceFull(work) => work,
        })
    })
}

fn sumeragi_relay_retry_is_eligible(work: &PreparedSumeragiRelayWork, now: Instant) -> bool {
    work.retry_eligible_at <= now
}

enum SumeragiRelayRetryRetentionError {
    Capacity(Box<PreparedSumeragiRelayWork>),
    RefinementViolation,
}

fn sumeragi_relay_retain_retry(
    retained: &mut FairRetainedQueue<SumeragiRelaySource, PreparedSumeragiRelayWork>,
    selection: &FairRetainedSelectionTrace<SumeragiRelaySource>,
    work: PreparedSumeragiRelayWork,
) -> Result<(), SumeragiRelayRetryRetentionError> {
    let retry_route_active = work.reply_route.is_active();
    if !retry_route_active {
        return retain_sumeragi_work(retained, work)
            .map_err(SumeragiRelayRetryRetentionError::Capacity);
    }

    let retry_source = work.source.clone();
    let retry_route = work.reply_route.clone();
    let retry_geometry = work.retention_guard.geometry;
    let source_depth_before_reinsert = retained.lanes.get(&retry_source).map_or(0, VecDeque::len);
    if !retained.has_capacity() || source_depth_before_reinsert >= retained.source_capacity {
        return Err(SumeragiRelayRetryRetentionError::Capacity(Box::new(work)));
    }
    let source_rank_before_reinsert = retained
        .ready
        .iter()
        .position(|source| source == &retry_source);
    let ready_sources_after = if source_depth_before_reinsert == 0 {
        retained.ready.len().checked_add(1)
    } else {
        Some(retained.ready.len())
    }
    .ok_or(SumeragiRelayRetryRetentionError::RefinementViolation)?;
    let selected_source_rank_after = if source_depth_before_reinsert == 0 {
        retained.ready.len()
    } else {
        source_rank_before_reinsert.ok_or(SumeragiRelayRetryRetentionError::RefinementViolation)?
    };
    let source_depth_after = source_depth_before_reinsert
        .checked_add(1)
        .ok_or(SumeragiRelayRetryRetentionError::RefinementViolation)?;
    let total_depth_after = retained
        .len
        .checked_add(1)
        .ok_or(SumeragiRelayRetryRetentionError::RefinementViolation)?;
    let prospective = ProductionTwoStageRelayRetryTraceProjection {
        daemon_source_capacity_matches_two_upstream_lanes: retry_geometry
            .daemon_source_capacity_matches_two_upstream_lanes(),
        class_corridor_covers_authenticated_sources: retry_geometry
            .class_corridor_covers_authenticated_sources(),
        authenticated_source_matches_resource_owner: selection.source == retry_source
            && retry_route.is_authenticated_via(&selection.source.via),
        retry_route_same_delivery: true,
        retry_route_active,
        selected_eligible: selection.selected_eligible,
        ready_sources_before: u64::try_from(selection.ready_sources_before)
            .expect("retained ready-source count must fit u64"),
        selected_source_rank_before: u64::try_from(selection.selected_source_rank_before)
            .expect("retained source rank must fit u64"),
        ready_sources_after: u64::try_from(ready_sources_after)
            .expect("retained ready-source count must fit u64"),
        selected_source_rank_after: u64::try_from(selected_source_rank_after)
            .expect("retained source rank must fit u64"),
        source_depth_before: u64::try_from(selection.source_depth_before)
            .expect("retained source depth must fit u64"),
        selected_item_rank_before: u64::try_from(selection.selected_item_rank_before)
            .expect("retained item rank must fit u64"),
        source_depth_after: u64::try_from(source_depth_after)
            .expect("retained source depth must fit u64"),
        selected_item_rank_after: u64::try_from(source_depth_before_reinsert)
            .expect("retained item rank must fit u64"),
        total_depth_before: u64::try_from(selection.total_depth_before)
            .expect("retained total depth must fit u64"),
        total_depth_after: u64::try_from(total_depth_after)
            .expect("retained total depth must fit u64"),
        source_capacity: u64::try_from(retained.source_capacity)
            .expect("retained source capacity must fit u64"),
        total_capacity: u64::try_from(retained.capacity)
            .expect("retained total capacity must fit u64"),
    };
    let Some(checked_transition) = check_production_two_stage_relay_retry_transition(prospective)
    else {
        return Err(SumeragiRelayRetryRetentionError::RefinementViolation);
    };
    let prospective = checked_transition.into_projection();
    retained.push(retry_source.clone(), work).map_err(|error| {
        SumeragiRelayRetryRetentionError::Capacity(Box::new(match error {
            FairRetainedPushError::Full(work) | FairRetainedPushError::SourceFull(work) => work,
        }))
    })?;

    let selected_source_rank_after = retained
        .ready
        .iter()
        .position(|source| source == &retry_source);
    let retry_lane = retained.lanes.get(&retry_source);
    let source_depth_after = retry_lane.map(VecDeque::len);
    let selected_item_rank_after = retry_lane.and_then(|lane| {
        lane.iter()
            .rposition(|candidate| candidate.reply_route.same_delivery(&retry_route))
    });
    let observed = ProductionTwoStageRelayRetryTraceProjection {
        daemon_source_capacity_matches_two_upstream_lanes: retry_geometry
            .daemon_source_capacity_matches_two_upstream_lanes(),
        class_corridor_covers_authenticated_sources: retry_geometry
            .class_corridor_covers_authenticated_sources(),
        authenticated_source_matches_resource_owner: selection.source == retry_source
            && retry_route.is_authenticated_via(&selection.source.via),
        retry_route_same_delivery: selected_item_rank_after.is_some(),
        retry_route_active,
        selected_eligible: selection.selected_eligible,
        ready_sources_before: u64::try_from(selection.ready_sources_before)
            .expect("retained ready-source count must fit u64"),
        selected_source_rank_before: u64::try_from(selection.selected_source_rank_before)
            .expect("retained source rank must fit u64"),
        ready_sources_after: u64::try_from(retained.ready.len())
            .expect("retained ready-source count must fit u64"),
        selected_source_rank_after: selected_source_rank_after
            .and_then(|rank| u64::try_from(rank).ok())
            .unwrap_or(u64::MAX),
        source_depth_before: u64::try_from(selection.source_depth_before)
            .expect("retained source depth must fit u64"),
        selected_item_rank_before: u64::try_from(selection.selected_item_rank_before)
            .expect("retained item rank must fit u64"),
        source_depth_after: source_depth_after
            .and_then(|depth| u64::try_from(depth).ok())
            .unwrap_or(u64::MAX),
        selected_item_rank_after: selected_item_rank_after
            .and_then(|rank| u64::try_from(rank).ok())
            .unwrap_or(u64::MAX),
        total_depth_before: u64::try_from(selection.total_depth_before)
            .expect("retained total depth must fit u64"),
        total_depth_after: u64::try_from(retained.len).expect("retained total depth must fit u64"),
        source_capacity: u64::try_from(retained.source_capacity)
            .expect("retained source capacity must fit u64"),
        total_capacity: u64::try_from(retained.capacity)
            .expect("retained total capacity must fit u64"),
    };
    if observed == prospective {
        Ok(())
    } else {
        Err(SumeragiRelayRetryRetentionError::RefinementViolation)
    }
}

fn sumeragi_relay_source_capacity(
    class_capacity: usize,
    daemon_source_capacity: usize,
) -> Option<usize> {
    (daemon_source_capacity != 0 && daemon_source_capacity <= class_capacity)
        .then_some(daemon_source_capacity)
}

fn new_sumeragi_relay_retained_queue(
    retained_capacity: usize,
    source_capacity: usize,
) -> FairRetainedQueue<SumeragiRelaySource, PreparedSumeragiRelayWork> {
    FairRetainedQueue::new(retained_capacity, source_capacity)
}

fn sumeragi_relay_source_credits(
    geometry: SumeragiRelayCapacityGeometry,
) -> SumeragiRelaySourceCredits {
    SumeragiRelaySourceCredits::new(geometry)
}

fn sumeragi_relay_dispatcher_capacity(class_capacity: usize) -> Option<usize> {
    class_capacity
        .checked_mul(2)
        .filter(|capacity| *capacity >= 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SumeragiRelayDispatcherLifecycle {
    Running,
    GracefulShutdown,
    UnexpectedClosure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SumeragiRelayFailureDisposition {
    RetireForShutdown,
    FailStop,
}

fn sumeragi_relay_dispatcher_lifecycle(
    shutdown_requested: bool,
    v2_open: bool,
    lane_open: bool,
    retained_empty: bool,
) -> SumeragiRelayDispatcherLifecycle {
    // Do not close the receiver merely because shutdown was requested: detached
    // worker generations still own senders and interpret receiver loss as a
    // live-node fail-stop. Their closure plus an empty exact queue is the
    // dispatcher subtree's teardown acknowledgement.
    if v2_open || lane_open || !retained_empty {
        SumeragiRelayDispatcherLifecycle::Running
    } else if shutdown_requested {
        SumeragiRelayDispatcherLifecycle::GracefulShutdown
    } else {
        SumeragiRelayDispatcherLifecycle::UnexpectedClosure
    }
}

fn sumeragi_relay_failure_disposition(
    reason: SumeragiRelayFatalReason,
    shutdown_requested: bool,
) -> SumeragiRelayFailureDisposition {
    if shutdown_requested
        && matches!(
            reason,
            SumeragiRelayFatalReason::IngressClosed
                | SumeragiRelayFatalReason::BlockingTaskCancelled
        )
    {
        SumeragiRelayFailureDisposition::RetireForShutdown
    } else {
        SumeragiRelayFailureDisposition::FailStop
    }
}

fn sumeragi_relay_class_capacity(class_capacity: usize) -> Option<usize> {
    (class_capacity != 0).then_some(class_capacity)
}

fn sumeragi_relay_closed_fatal(
    class: SumeragiRelayClass,
    work: CreditedSumeragiRelayWorkItem,
) -> ! {
    let _exact_unadmitted_item = work;
    iroha_logger::error!(
        class = class.label(),
        "retained Sumeragi dispatcher closed; failing stop with exact ownership"
    );
    std::process::exit(1)
}

fn sumeragi_relay_source_credit_invariant_fatal(
    class: SumeragiRelayClass,
    work: SumeragiRelayWorkItem,
) -> ! {
    let _exact_unadmitted_item = work;
    iroha_logger::error!(
        class = class.label(),
        "upstream-owned Sumeragi item could not acquire its matched daemon source credit; stopping"
    );
    std::process::exit(1)
}

fn sumeragi_relay_retention_invariant_fatal(
    reason: &'static str,
    work: PreparedSumeragiRelayWork,
) -> ! {
    let class = work.source.class;
    let _exact_retained_item = work;
    iroha_logger::error!(
        class = class.label(),
        reason,
        "retained Sumeragi ownership invariant failed; stopping with exact ownership"
    );
    std::process::exit(1)
}

fn sumeragi_relay_retained_refinement_fatal(reason: &'static str) -> ! {
    iroha_logger::error!(
        reason,
        "retained Sumeragi retry refinement failed; stopping with exact queue ownership"
    );
    std::process::exit(1)
}

async fn attempt_retained_sumeragi_relay_work(
    shared: &NetworkRelayShared,
    retained: &mut FairRetainedQueue<SumeragiRelaySource, PreparedSumeragiRelayWork>,
    shutdown_signal: &ShutdownSignal,
) -> bool {
    let now = Instant::now();
    let Some(selection) =
        retained.pop_if_with_trace(|work| sumeragi_relay_retry_is_eligible(work, now))
    else {
        return false;
    };
    let FairRetainedSelection { trace, item } = selection;
    match attempt_sumeragi_relay_work(shared, item, shutdown_signal).await {
        SumeragiRelayAttempt::Terminal {
            outcome,
            retention_guard,
            completion,
        } => finish_sumeragi_relay_terminal(outcome, retention_guard, completion),
        SumeragiRelayAttempt::Retry(work) => {
            match sumeragi_relay_retain_retry(retained, &trace, work) {
                Ok(()) => {}
                Err(SumeragiRelayRetryRetentionError::Capacity(work)) => {
                    sumeragi_relay_retention_invariant_fatal("retry_reinsertion_capacity", *work);
                }
                Err(SumeragiRelayRetryRetentionError::RefinementViolation) => {
                    sumeragi_relay_retained_refinement_fatal("two_stage_retry_source_fairness");
                }
            }
        }
        SumeragiRelayAttempt::Fatal {
            source,
            reason,
            exact_item,
            reply_route,
            retention_guard,
            completion,
        } => {
            if sumeragi_relay_failure_disposition(reason, shutdown_signal.is_sent())
                == SumeragiRelayFailureDisposition::RetireForShutdown
            {
                let _exact_shutdown_item = (exact_item, reply_route, retention_guard, completion);
                iroha_logger::debug!(
                    class = source.class.label(),
                    via = %source.via,
                    reason = reason.label(),
                    "retiring retained Sumeragi ingress during node shutdown"
                );
                return true;
            }
            iroha_logger::error!(
                class = source.class.label(),
                via = %source.via,
                reason = reason.label(),
                "retained Sumeragi ingress entered fail-stop mode"
            );
            std::process::exit(1);
        }
    }
    true
}

async fn run_sumeragi_relay_dispatcher(
    shared: Arc<NetworkRelayShared>,
    mut v2_rx: mpsc::Receiver<CreditedSumeragiRelayWorkItem>,
    mut lane_rx: mpsc::Receiver<CreditedSumeragiRelayWorkItem>,
    retained_capacity: usize,
    source_capacity: usize,
    shutdown_signal: ShutdownSignal,
) {
    let mut retained = new_sumeragi_relay_retained_queue(retained_capacity, source_capacity);
    let mut next = SumeragiRelayClass::V2;
    let mut v2_open = true;
    let mut lane_open = true;
    loop {
        let mut admitted_input = false;
        if retained.has_capacity()
            && let Some(work) = try_recv_sumeragi_relay_work(
                &mut v2_rx,
                &mut lane_rx,
                &mut next,
                &mut v2_open,
                &mut lane_open,
            )
        {
            admitted_input = true;
            match shared.prepare_sumeragi_relay_work(work) {
                PrepareSumeragiRelayResult::Prepared(work) => {
                    if let Err(work) = retain_sumeragi_work(&mut retained, *work) {
                        sumeragi_relay_retention_invariant_fatal(
                            "initial_retained_capacity",
                            *work,
                        );
                    }
                }
                #[cfg(feature = "test-network-message-control")]
                PrepareSumeragiRelayResult::Held => {}
                #[cfg(feature = "test-network-message-control")]
                PrepareSumeragiRelayResult::Controlled {
                    outcome,
                    completion,
                } => {
                    if let Some(completion) = completion {
                        let _ = completion.send(outcome);
                    }
                }
                PrepareSumeragiRelayResult::Terminal {
                    outcome,
                    retention_guard,
                    completion,
                } => finish_sumeragi_relay_terminal(outcome, retention_guard, completion),
            }
        }

        let attempted =
            attempt_retained_sumeragi_relay_work(&shared, &mut retained, &shutdown_signal).await;

        match sumeragi_relay_dispatcher_lifecycle(
            shutdown_signal.is_sent(),
            v2_open,
            lane_open,
            retained.is_empty(),
        ) {
            SumeragiRelayDispatcherLifecycle::Running => {}
            SumeragiRelayDispatcherLifecycle::GracefulShutdown => {
                iroha_logger::debug!(
                    "retained Sumeragi relay drained after shutdown; dispatcher exiting"
                );
                return;
            }
            SumeragiRelayDispatcherLifecycle::UnexpectedClosure => {
                iroha_logger::error!("retained Sumeragi relay channels closed");
                std::process::exit(1);
            }
        }
        if !admitted_input && !attempted {
            tokio::time::sleep(SUMERAGI_RELAY_RETRY_CADENCE).await;
        }
    }
}

fn spawn_sumeragi_relay_dispatcher(
    shared: Arc<NetworkRelayShared>,
    geometry: SumeragiRelayCapacityGeometry,
    shutdown_signal: ShutdownSignal,
) -> SumeragiRelayIngress {
    assert!(geometry.daemon_source_capacity_matches_two_upstream_lanes());
    assert!(geometry.class_corridor_covers_authenticated_sources());
    let class_capacity = sumeragi_relay_class_capacity(geometry.class_capacity)
        .expect("Sumeragi relay class capacity must be non-zero at startup");
    let retained_capacity = sumeragi_relay_dispatcher_capacity(class_capacity)
        .filter(|capacity| *capacity == geometry.retained_capacity)
        .expect("Sumeragi relay retained capacity changed after startup validation");
    let source_capacity =
        sumeragi_relay_source_capacity(class_capacity, geometry.daemon_per_source).expect(
            "Sumeragi relay corridor cannot reserve the exact authenticated-source geometry",
        );
    let source_credits = sumeragi_relay_source_credits(geometry);
    let (v2_tx, v2_rx) = mpsc::channel(class_capacity);
    let (lane_tx, lane_rx) = mpsc::channel(class_capacity);
    tokio::spawn(run_sumeragi_relay_dispatcher(
        shared,
        v2_rx,
        lane_rx,
        retained_capacity,
        source_capacity,
        shutdown_signal,
    ));

    SumeragiRelayIngress {
        v2: v2_tx,
        lane: lane_tx,
        source_credits,
    }
}

fn spawn_network_relay_worker(
    shared: &Arc<NetworkRelayShared>,
    sumeragi_ingress: &SumeragiRelayIngress,
    worker_limit: usize,
    work_high_cap: usize,
    work_payload_cap: usize,
    work_chunk_cap: usize,
    work_low_cap: usize,
) -> (
    mpsc::Sender<RelayWorkItem>,
    mpsc::Sender<RelayWorkItem>,
    mpsc::Sender<RelayWorkItem>,
    mpsc::Sender<RelayWorkItem>,
) {
    let (work_high_tx, work_high_rx) = mpsc::channel::<RelayWorkItem>(work_high_cap);
    let (work_payload_tx, work_payload_rx) = mpsc::channel::<RelayWorkItem>(work_payload_cap);
    let (work_chunk_tx, work_chunk_rx) = mpsc::channel::<RelayWorkItem>(work_chunk_cap);
    let (work_low_tx, work_low_rx) = mpsc::channel::<RelayWorkItem>(work_low_cap);
    let worker_sem = Arc::new(tokio::sync::Semaphore::new(worker_limit));
    let shared_for_workers = Arc::clone(shared);
    let worker_sem_for_workers = Arc::clone(&worker_sem);
    let sumeragi_ingress = sumeragi_ingress.clone();
    tokio::spawn(async move {
        let mut receivers = FairRelayReceivers::new(
            work_high_rx,
            work_payload_rx,
            work_chunk_rx,
            work_low_rx,
            RELAY_HIGH_BURST,
        );
        while let Some((_kind, msg)) = receivers.recv().await {
            if let Some(class) = sumeragi_relay_class(&msg.payload) {
                if let Err(work) = sumeragi_ingress
                    .send(class, SumeragiRelayWorkItem::live(msg))
                    .await
                {
                    sumeragi_relay_closed_fatal(class, work);
                }
                continue;
            }
            let Ok(permit) = worker_sem_for_workers.clone().acquire_owned().await else {
                let _exact_unadmitted_item = msg;
                iroha_logger::error!(
                    "network relay worker semaphore closed while exact work was retained"
                );
                std::process::exit(1);
            };
            let shared = Arc::clone(&shared_for_workers);
            tokio::spawn(async move {
                let (peer, authenticated_via, payload, payload_bytes, _retention_guard) =
                    msg.into_parts();
                shared
                    .handle_message(peer, authenticated_via, payload, payload_bytes)
                    .await;
                drop(permit);
            });
        }
    });

    (work_high_tx, work_payload_tx, work_chunk_tx, work_low_tx)
}

async fn forward_relay_lane(
    mut receiver: mpsc::Receiver<RelayWorkItem>,
    tx: &mpsc::Sender<RelayWorkItem>,
    sumeragi_ingress: Option<&SumeragiRelayIngress>,
    kind: RelayReceiverKind,
    fail_stop_on_close: bool,
) -> RelayIngressLoopExit {
    while let Some(msg) = receiver.recv().await {
        if let Some((message_kind, height, view)) =
            NetworkRelayShared::retired_sumeragi_message_meta(&msg.payload)
        {
            iroha_logger::debug!(
                peer = %msg.peer,
                ?height,
                ?view,
                message_kind,
                "rejecting retired Sumeragi v1 message before relay preprocessing"
            );
            continue;
        }

        if let Some(class) = sumeragi_relay_class(&msg.payload)
            && let Some(sumeragi_ingress) = sumeragi_ingress
        {
            if let Err(work) = sumeragi_ingress
                .send(class, SumeragiRelayWorkItem::live(msg))
                .await
            {
                sumeragi_relay_closed_fatal(class, work);
            }
            continue;
        }

        // `send` retains the exact owned item while this lane's bounded worker queue is full.
        // Each lane has an independent forwarder, so backpressure in one class cannot obstruct
        // admission in the other classes. The worker-side scheduler supplies bounded service
        // fairness after admission.
        if let Err(error) = tx.send(msg).await {
            let _exact_unadmitted_item = error.0;
            if fail_stop_on_close {
                iroha_logger::error!(
                    queue = kind.label(),
                    "relay worker queue closed while exact work was retained; stopping"
                );
                std::process::exit(1);
            }
            return RelayIngressLoopExit::WorkerClosed(kind);
        }
    }

    if fail_stop_on_close {
        iroha_logger::error!(
            receiver = kind.label(),
            "relay subscriber channel closed; stopping before sibling owners can be cancelled"
        );
        std::process::exit(1);
    }
    RelayIngressLoopExit::ReceiverClosed(kind)
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
async fn drive_network_relay_ingress(
    high_receiver: mpsc::Receiver<RelayWorkItem>,
    payload_receiver: mpsc::Receiver<RelayWorkItem>,
    chunk_receiver: mpsc::Receiver<RelayWorkItem>,
    low_receiver: mpsc::Receiver<RelayWorkItem>,
    work_high_tx: &mpsc::Sender<RelayWorkItem>,
    work_payload_tx: &mpsc::Sender<RelayWorkItem>,
    work_chunk_tx: &mpsc::Sender<RelayWorkItem>,
    work_low_tx: &mpsc::Sender<RelayWorkItem>,
) -> RelayIngressLoopExit {
    drive_network_relay_ingress_inner(
        high_receiver,
        payload_receiver,
        chunk_receiver,
        low_receiver,
        work_high_tx,
        work_payload_tx,
        work_chunk_tx,
        work_low_tx,
        None,
        false,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn drive_network_relay_ingress_inner(
    high_receiver: mpsc::Receiver<RelayWorkItem>,
    payload_receiver: mpsc::Receiver<RelayWorkItem>,
    chunk_receiver: mpsc::Receiver<RelayWorkItem>,
    low_receiver: mpsc::Receiver<RelayWorkItem>,
    work_high_tx: &mpsc::Sender<RelayWorkItem>,
    work_payload_tx: &mpsc::Sender<RelayWorkItem>,
    work_chunk_tx: &mpsc::Sender<RelayWorkItem>,
    work_low_tx: &mpsc::Sender<RelayWorkItem>,
    sumeragi_ingress: Option<&SumeragiRelayIngress>,
    fail_stop_on_close: bool,
) -> RelayIngressLoopExit {
    // These four futures are independent bounded corridors. `select!` returns on the first
    // closed ingress or worker lane, which preserves the relay's fail-stop restart contract.
    tokio::select! {
        exit = forward_relay_lane(
            high_receiver,
            work_high_tx,
            sumeragi_ingress,
            RelayReceiverKind::High,
            fail_stop_on_close,
        ) => exit,
        exit = forward_relay_lane(
            payload_receiver,
            work_payload_tx,
            sumeragi_ingress,
            RelayReceiverKind::Payload,
            fail_stop_on_close,
        ) => exit,
        exit = forward_relay_lane(
            chunk_receiver,
            work_chunk_tx,
            sumeragi_ingress,
            RelayReceiverKind::Chunk,
            fail_stop_on_close,
        ) => exit,
        exit = forward_relay_lane(
            low_receiver,
            work_low_tx,
            sumeragi_ingress,
            RelayReceiverKind::Low,
            fail_stop_on_close,
        ) => exit,
    }
}

fn high_priority_relay_filter() -> iroha_p2p::network::SubscriberFilter {
    use iroha_p2p::network::{SubscriberFilter, message::Topic};

    SubscriberFilter::topics([Topic::ConsensusSafety, Topic::Consensus, Topic::Control])
}

impl NetworkRelay {
    fn into_shared(self) -> NetworkRelayShared {
        NetworkRelayShared {
            sumeragi: self.sumeragi,
            tx_gossiper: self.tx_gossiper,
            peers_gossiper: self.peers_gossiper,
            network: self.network,
            streaming: self.streaming,
            consensus_ingress: Mutex::new(self.consensus_ingress),
            low_priority_ingress: Mutex::new(self.low_priority_ingress),
            #[cfg(feature = "test-network-message-control")]
            test_message_control: self.test_message_control,
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn run(self, shutdown_signal: ShutdownSignal) {
        use iroha_p2p::network::{SubscriberFilter, message::Topic};

        let shared = Arc::new(self.into_shared());
        let base_cap = shared.network.subscriber_queue_cap().get();
        let network_per_lane = shared.network.authenticated_source_credit_capacity().get();
        let authenticated_source_count = shared.network.reply_route_source_capacity();
        let sumeragi_geometry = SumeragiRelayCapacityGeometry::checked(
            base_cap,
            network_per_lane,
            authenticated_source_count,
        )
        .expect(
            "Sumeragi relay corridor cannot reserve both upstream lanes for every authenticated source",
        );
        let sumeragi_ingress = spawn_sumeragi_relay_dispatcher(
            Arc::clone(&shared),
            sumeragi_geometry,
            shutdown_signal,
        );
        #[cfg(feature = "test-network-message-control")]
        if let Some(controller) = shared.test_message_control.clone() {
            let sumeragi_ingress = sumeragi_ingress.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if let Err(error) = controller.poll_command() {
                        iroha_logger::error!(
                            reason = error.code(),
                            "test-network consensus command watcher failed closed"
                        );
                        std::process::exit(1);
                    }
                    loop {
                        let held = match controller.next_release() {
                            Ok(Some(held)) => held,
                            Ok(None) => break,
                            Err(error) => {
                                iroha_logger::error!(
                                    reason = error.code(),
                                    "test-network consensus release queue failed closed"
                                );
                                std::process::exit(1);
                            }
                        };
                        let (sequence, class, released, ownership) =
                            match rehydrate_held_sumeragi_relay_work(
                                held,
                                |peer, message, size_bytes, reply_route| {
                                    let mut released =
                                        RelayWorkItem::new(peer, message, size_bytes);
                                    released.reattach_reply_route(reply_route)?;
                                    Ok(released)
                                },
                            ) {
                                HeldSumeragiReentry::Ready {
                                    sequence,
                                    class,
                                    work,
                                    ownership,
                                } => (sequence, class, work, ownership),
                                HeldSumeragiReentry::RetireStale {
                                    sequence,
                                    ownership,
                                } => {
                                    iroha_logger::debug!(
                                        sequence,
                                        "retiring held consensus occurrence whose authenticated connection tenure ended; a later authenticated retransmission remains authoritative"
                                    );
                                    if let Err(error) = controller.complete_release(
                                        sequence,
                                        consensus_message_control::ReleaseOutcome::Retired,
                                    ) {
                                        iroha_logger::error!(
                                            reason = error.code(),
                                            "test-network consensus stale-occurrence retirement failed closed"
                                        );
                                        std::process::exit(1);
                                    }
                                    if let Some(completion) = ownership.completion {
                                        let _ =
                                            completion.send(SumeragiRelayTerminalOutcome::Retired);
                                    }
                                    drop(ownership.retention_guard);
                                    continue;
                                }
                                HeldSumeragiReentry::Reject {
                                    sequence,
                                    reason,
                                    ownership,
                                } => {
                                    iroha_logger::error!(
                                        sequence,
                                        reason = reason.label(),
                                        "test-network consensus release rehydration failed closed"
                                    );
                                    let completion_result = controller.complete_release(
                                        sequence,
                                        consensus_message_control::ReleaseOutcome::Failed,
                                    );
                                    if let Some(ownership) = ownership {
                                        if let Some(completion) = ownership.completion {
                                            let _ = completion
                                                .send(SumeragiRelayTerminalOutcome::Failed);
                                        }
                                        drop(ownership.retention_guard);
                                    }
                                    if let Err(error) = completion_result {
                                        iroha_logger::error!(
                                            reason = error.code(),
                                            "test-network consensus release rejection failed closed"
                                        );
                                    }
                                    std::process::exit(1);
                                }
                            };
                        let HeldSumeragiRelayOwnership {
                            retention_guard,
                            completion: held_completion,
                        } = ownership;
                        let (completion_tx, completion_rx) = oneshot::channel();
                        let work = SumeragiRelayWorkItem::released(released, completion_tx);
                        if let Err(work) = sumeragi_ingress
                            .send_rehydrated(class, work, retention_guard)
                            .await
                        {
                            sumeragi_relay_closed_fatal(class, work);
                        }
                        let outcome = completion_rx
                            .await
                            .unwrap_or(SumeragiRelayTerminalOutcome::Failed);
                        let release_outcome = match outcome {
                            SumeragiRelayTerminalOutcome::Delivered => {
                                consensus_message_control::ReleaseOutcome::Delivered
                            }
                            SumeragiRelayTerminalOutcome::Retired => {
                                consensus_message_control::ReleaseOutcome::Retired
                            }
                            SumeragiRelayTerminalOutcome::Failed => {
                                consensus_message_control::ReleaseOutcome::Failed
                            }
                        };
                        if let Err(error) = controller.complete_release(sequence, release_outcome) {
                            iroha_logger::error!(
                                sequence,
                                reason = error.code(),
                                "test-network consensus release delivery failed closed"
                            );
                            std::process::exit(1);
                        }
                        if let Some(completion) = held_completion {
                            let _ = completion.send(outcome);
                        }
                    }
                }
            });
        }
        let high_cap = base_cap.saturating_mul(4).max(base_cap);
        let payload_cap = base_cap.saturating_mul(2).max(base_cap);
        let chunk_cap = base_cap;
        let low_cap = base_cap;
        let work_high_cap = high_cap.saturating_mul(2);
        let work_payload_cap = payload_cap.saturating_mul(2);
        let work_chunk_cap = chunk_cap;
        let work_low_cap = low_cap;
        let worker_limit = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .clamp(1, 8);

        let high_filter = high_priority_relay_filter();
        let payload_filter = SubscriberFilter::topics([Topic::ConsensusPayload, Topic::BlockSync]);
        let chunk_filter = SubscriberFilter::topics([Topic::ConsensusChunk]);
        let low_filter = SubscriberFilter::topics([
            Topic::TxGossip,
            Topic::TxGossipRestricted,
            Topic::PeerGossip,
            Topic::TrustGossip,
            Topic::Health,
            Topic::Other,
        ]);

        loop {
            let (high_sender, high_receiver) = mpsc::channel(high_cap);
            let (payload_sender, payload_receiver) = mpsc::channel(payload_cap);
            let (chunk_sender, chunk_receiver) = mpsc::channel(chunk_cap);
            let (low_sender, low_receiver) = mpsc::channel(low_cap);
            let (work_high_tx, work_payload_tx, work_chunk_tx, work_low_tx) =
                spawn_network_relay_worker(
                    &shared,
                    &sumeragi_ingress,
                    worker_limit,
                    work_high_cap,
                    work_payload_cap,
                    work_chunk_cap,
                    work_low_cap,
                );

            let mut high_sender = Some(high_sender);
            let mut payload_sender = Some(payload_sender);
            let mut chunk_sender = Some(chunk_sender);
            let mut low_sender = Some(low_sender);
            if let Some(sender) = high_sender.take() {
                match shared
                    .network
                    .subscribe_to_peers_messages_with_filter(sender, high_filter.clone())
                {
                    Ok(()) => {
                        iroha_logger::info!("registered high-priority relay subscriber");
                    }
                    Err(returned) => {
                        iroha_logger::warn!("retrying high-priority P2P subscriber registration");
                        high_sender = Some(returned);
                    }
                }
            }

            if let Some(sender) = payload_sender.take() {
                match shared
                    .network
                    .subscribe_to_peers_messages_with_filter(sender, payload_filter.clone())
                {
                    Ok(()) => {
                        iroha_logger::info!("registered payload relay subscriber");
                    }
                    Err(returned) => {
                        iroha_logger::warn!("retrying payload P2P subscriber registration");
                        payload_sender = Some(returned);
                    }
                }
            }

            if let Some(sender) = chunk_sender.take() {
                match shared
                    .network
                    .subscribe_to_peers_messages_with_filter(sender, chunk_filter.clone())
                {
                    Ok(()) => {
                        iroha_logger::info!("registered chunk relay subscriber");
                    }
                    Err(returned) => {
                        iroha_logger::warn!("retrying chunk P2P subscriber registration");
                        chunk_sender = Some(returned);
                    }
                }
            }

            if let Some(sender) = low_sender.take() {
                match shared
                    .network
                    .subscribe_to_peers_messages_with_filter(sender, low_filter.clone())
                {
                    Ok(()) => {
                        iroha_logger::info!("registered low-priority relay subscriber");
                    }
                    Err(returned) => {
                        iroha_logger::warn!("retrying low-priority P2P subscriber registration");
                        low_sender = Some(returned);
                    }
                }
            }

            if high_sender.is_none()
                && payload_sender.is_none()
                && chunk_sender.is_none()
                && low_sender.is_none()
            {
                let exit = drive_network_relay_ingress_inner(
                    high_receiver,
                    payload_receiver,
                    chunk_receiver,
                    low_receiver,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
                    Some(&sumeragi_ingress),
                    true,
                )
                .await;
                match exit {
                    RelayIngressLoopExit::ReceiverClosed(kind) => {
                        iroha_logger::warn!(
                            receiver = kind.label(),
                            "relay subscriber channel closed; restarting subscriptions"
                        );
                    }
                    RelayIngressLoopExit::WorkerClosed(kind) => {
                        iroha_logger::warn!(
                            queue = kind.label(),
                            "relay worker queue closed; restarting dispatcher"
                        );
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl NetworkRelayShared {
    fn consensus_ingress_allows(
        &self,
        authenticated_via: &PeerId,
        msg: &iroha_core::NetworkMessage,
        size_bytes: usize,
    ) -> bool {
        use iroha_core::NetworkMessage::*;

        if !matches!(
            msg,
            SumeragiBlock(_) | SumeragiControlFlow(_) | LaneDrainVote(_) | CertifiedMergeSidecar(_)
        ) {
            return true;
        }
        let reason = {
            let mut limiter = self
                .consensus_ingress
                .lock()
                .expect("consensus ingress mutex poisoned");
            limiter.should_drop_from(authenticated_via, msg, size_bytes)
        };
        let Some(reason) = reason else {
            return true;
        };

        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global()
            && let Some(topic) = Self::consensus_ingress_topic_label(msg)
        {
            metrics
                .consensus_ingress_drop_total
                .with_label_values(&[topic, reason.label()])
                .inc();
        }
        let (kind, height, view) = match msg {
            SumeragiBlock(data) => Self::block_message_meta(data.as_ref().as_ref()),
            SumeragiControlFlow(data) => Self::control_flow_meta(data.as_ref()),
            LaneDrainVote(vote) => (
                "LaneDrainVote",
                Some(vote.body.intent.close_global_height),
                None,
            ),
            CertifiedMergeSidecar(data) => match data.as_ref() {
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Request(_) => {
                    ("CertifiedMergeSidecarRequest", None, None)
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Close(_) => {
                    ("CertifiedMergeSidecarClose", None, None)
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::CloseAck(_) => {
                    ("CertifiedMergeSidecarCloseAck", None, None)
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::GenerationHint(_) => {
                    ("CertifiedMergeSidecarGenerationHint", None, None)
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Chunk(_) => {
                    ("CertifiedMergeSidecarChunk", None, None)
                }
            },
            _ => ("Other", None, None),
        };
        iroha_logger::debug!(
            via = %authenticated_via,
            ?height,
            ?view,
            size_bytes,
            kind,
            reason = reason.label(),
            "dropping inbound consensus message due to ingress limits"
        );
        false
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_message(
        &self,
        peer: Peer,
        authenticated_via: PeerId,
        msg: iroha_core::NetworkMessage,
        size_bytes: usize,
    ) {
        #[cfg(feature = "test-network-message-control")]
        let (peer, msg, size_bytes) = if let Some(controller) = &self.test_message_control {
            match controller.admit(peer, &authenticated_via, msg, size_bytes) {
                Ok((
                    consensus_message_control::Admission::Consumed
                    | consensus_message_control::Admission::Held,
                    _,
                )) => return,
                Ok((consensus_message_control::Admission::Pass, Some(message))) => message,
                Ok((consensus_message_control::Admission::Pass, None)) => {
                    iroha_logger::error!(
                        "test-network consensus controller returned an invalid pass disposition; rejecting the inbound copy"
                    );
                    return;
                }
                Err(error) => {
                    iroha_logger::error!(
                        reason = error.code(),
                        "test-network consensus controller rejected admission; rejecting the inbound copy"
                    );
                    return;
                }
            }
        } else {
            (peer, msg, size_bytes)
        };

        let _ = self
            .handle_message_after_test_control(peer, authenticated_via, msg, size_bytes)
            .await;
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_message_after_test_control(
        &self,
        peer: Peer,
        authenticated_via: PeerId,
        msg: iroha_core::NetworkMessage,
        size_bytes: usize,
    ) -> bool {
        use iroha_core::NetworkMessage::*;

        if let Some((kind, height, view)) = Self::retired_sumeragi_message_meta(&msg) {
            iroha_logger::debug!(
                %peer,
                ?height,
                ?view,
                kind,
                "rejecting retired Sumeragi v1 message before ingress accounting"
            );
            return false;
        }

        if !self.consensus_ingress_allows(&authenticated_via, &msg, size_bytes) {
            return false;
        }

        if Self::should_apply_low_priority_ingress(&msg) {
            let reason = {
                let mut limiter = self
                    .low_priority_ingress
                    .lock()
                    .expect("low-priority ingress mutex poisoned");
                limiter.should_drop_from(&authenticated_via, size_bytes)
            };
            if let Some(reason) = reason {
                iroha_logger::debug!(
                    %peer,
                    via = %authenticated_via,
                    size_bytes,
                    reason = reason.label(),
                    "dropping inbound low-priority message due to ingress limits"
                );
                return false;
            }
        }

        match msg {
            SumeragiBlock(_)
            | SumeragiControlFlow(_)
            | LaneRelay(_)
            | MergeCommitteeSignature(_)
            | LaneDrainVote(_)
            | CertifiedMergeSidecar(_)
            | NativeAmx(_) => {
                iroha_logger::error!(
                    %peer,
                    via = %authenticated_via,
                    "Sumeragi message bypassed the retained dispatcher; rejecting without panicking"
                );
                return false;
            }
            StreamingControl(frame) => {
                if let Err(err) = self.streaming.process_control_frame(&peer, frame.as_ref()) {
                    iroha_logger::warn!(%peer, ?err, "Failed to process streaming control frame");
                }
            }
            BlockSync(_) => {
                iroha_logger::debug!(
                    %peer,
                    via = %authenticated_via,
                    "retired v1 block sync bypassed preprocessing; rejecting"
                );
                return false;
            }
            TransactionGossiper(data) => {
                iroha_logger::debug!(
                    %peer,
                    txs = data.txs.len(),
                    "relay received transaction gossip"
                );
                self.tx_gossiper.gossip(data);
            }
            PeersGossiper(data) => self.peers_gossiper.gossip(*data, peer),
            PeerTrustGossip(data) => self.peers_gossiper.gossip_trust(*data, peer),
            msg @ (SoracloudLocalReadProxyRequest(_)
            | SoracloudLocalReadProxyResponse(_)
            | ToriiProxyRequest(_)
            | ToriiProxyResponse(_)
            | QueuePlanAdmissionPublication(_)
            | Health
            | Connect(_)) => {
                debug_assert!(Self::is_handled_by_dedicated_subscriber(&msg));
                // Health frames are handled elsewhere. Connect, Soracloud local-read proxy,
                // and Torii proxy frames go to Torii via its own subscriber tasks when those
                // surfaces are enabled.
            }
            TimePing(p) => {
                iroha_core::time::handle_message(
                    peer,
                    iroha_core::NetworkMessage::TimePing(p),
                    &self.network,
                )
                .await;
            }
            TimePong(p) => {
                iroha_core::time::handle_message(
                    peer,
                    iroha_core::NetworkMessage::TimePong(p),
                    &self.network,
                )
                .await;
            }
        }
        true
    }

    fn is_handled_by_dedicated_subscriber(msg: &iroha_core::NetworkMessage) -> bool {
        msg.is_torii_proxy_control_message()
            || matches!(
                msg,
                iroha_core::NetworkMessage::Health | iroha_core::NetworkMessage::Connect(_)
            )
    }

    fn should_apply_low_priority_ingress(msg: &iroha_core::NetworkMessage) -> bool {
        use iroha_p2p::network::message::Topic;

        matches!(
            msg.topic(),
            Topic::TxGossip
                | Topic::TxGossipRestricted
                | Topic::PeerGossip
                | Topic::TrustGossip
                | Topic::Health
                | Topic::Other
        ) || matches!(msg, iroha_core::NetworkMessage::StreamingControl(_))
    }

    fn retired_sumeragi_message_meta(
        msg: &iroha_core::NetworkMessage,
    ) -> Option<(&'static str, Option<u64>, Option<u64>)> {
        match msg {
            iroha_core::NetworkMessage::SumeragiBlock(block)
                if !block.as_ref().as_ref().is_authoritative_v2_ingress() =>
            {
                Some(Self::block_message_meta(block.as_ref().as_ref()))
            }
            iroha_core::NetworkMessage::SumeragiControlFlow(message) => {
                Some(Self::control_flow_meta(message.as_ref()))
            }
            iroha_core::NetworkMessage::BlockSync(_) => Some(("BlockSyncV1", None, None)),
            _ => None,
        }
    }

    #[cfg(feature = "telemetry")]
    fn consensus_ingress_topic_label(msg: &iroha_core::NetworkMessage) -> Option<&'static str> {
        use iroha_p2p::network::message::Topic;

        if matches!(msg, iroha_core::NetworkMessage::LaneDrainVote(_)) {
            return Some("LaneDrainVote");
        }
        match msg.topic() {
            Topic::ConsensusPayload => Some("ConsensusPayload"),
            Topic::ConsensusChunk => Some("ConsensusChunk"),
            _ => None,
        }
    }

    fn block_message_meta(
        msg: &iroha_core::sumeragi::message::BlockMessage,
    ) -> (&'static str, Option<u64>, Option<u64>) {
        use iroha_core::sumeragi::message::BlockMessage::*;

        match msg {
            BlockCreated(_)
            | BlockSyncUpdate(_)
            | QcVote(_)
            | Qc(_)
            | VrfCommit(_)
            | VrfReveal(_)
            | ExecWitness(_)
            | RbcInit(_)
            | RbcInitRequest(_)
            | RbcChunk(_)
            | RbcChunkCompact(_)
            | RbcChunkRequest(_)
            | RbcReady(_)
            | RbcDeliver(_)
            | FetchBlockBody(_)
            | BlockBodyResponse(_)
            | CertifiedBlockFetch(_)
            | FetchPendingBlock(_)
            | ProposalHint(_)
            | Proposal(_) => Self::legacy_block_message_meta(msg),
            LaneBlockProposal(_)
            | LaneExecutablePayload(_)
            | LaneBlockNewViewVote(_)
            | LaneBlockNewViewCertificate(_)
            | LaneBlockVote(_)
            | LaneBlockQc(_)
            | LaneBlockCertificate(_)
            | LaneHistoricalRecoveryRequest(_)
            | LaneHistoricalRecoveryResponse(_) => Self::lane_block_message_meta(msg),
            KuraReplicaAdvert(advert) => ("KuraReplicaAdvert", Some(advert.height), None),
            V2(message) => Self::v2_block_message_meta(&message.payload),
        }
    }

    fn legacy_block_message_meta(
        msg: &iroha_core::sumeragi::message::BlockMessage,
    ) -> (&'static str, Option<u64>, Option<u64>) {
        use iroha_core::sumeragi::message::BlockMessage::*;

        match msg {
            BlockCreated(block) => {
                let header = block.block.header();
                (
                    "BlockCreated",
                    Some(header.height().get()),
                    Some(header.view_change_index()),
                )
            }
            BlockSyncUpdate(block) => {
                let header = block.block.header();
                (
                    "BlockSyncUpdate",
                    Some(header.height().get()),
                    Some(header.view_change_index()),
                )
            }
            QcVote(vote) => {
                let label = match vote.phase {
                    iroha_core::sumeragi::consensus::Phase::Prepare => "PrepareVote",
                    iroha_core::sumeragi::consensus::Phase::Commit => "QcVote",
                    iroha_core::sumeragi::consensus::Phase::NewView => "NewViewVote",
                };
                (label, Some(vote.height), Some(vote.view))
            }
            Qc(cert) => {
                let label = match cert.phase {
                    iroha_core::sumeragi::consensus::Phase::Prepare => "PrepareCert",
                    iroha_core::sumeragi::consensus::Phase::Commit => "CommitCert",
                    iroha_core::sumeragi::consensus::Phase::NewView => "NewViewCert",
                };
                (label, Some(cert.height), Some(cert.view))
            }
            VrfCommit(_) => ("VrfCommit", None, None),
            VrfReveal(_) => ("VrfReveal", None, None),
            ExecWitness(witness) => ("ExecWitness", Some(witness.height), Some(witness.view)),
            RbcInit(init) => ("RbcInit", Some(init.height), Some(init.view)),
            RbcInitRequest(request) => ("RbcInitRequest", Some(request.height), Some(request.view)),
            RbcChunk(chunk) => ("RbcChunk", Some(chunk.height), Some(chunk.view)),
            RbcChunkCompact(chunk) => (
                "RbcChunk",
                Some(u64::from(chunk.height)),
                Some(u64::from(chunk.view)),
            ),
            RbcChunkRequest(request) => {
                ("RbcChunkRequest", Some(request.height), Some(request.view))
            }
            RbcReady(ready) => ("RbcReady", Some(ready.height), Some(ready.view)),
            RbcDeliver(deliver) => ("RbcDeliver", Some(deliver.height), Some(deliver.view)),
            FetchBlockBody(request) => ("FetchBlockBody", Some(request.height), Some(request.view)),
            BlockBodyResponse(response) => (
                "BlockBodyResponse",
                Some(response.height),
                Some(response.view),
            ),
            CertifiedBlockFetch(fetch) => match fetch {
                iroha_core::sumeragi::message::CertifiedBlockFetch::Request(request) => (
                    "CertifiedBlockFetchRequest",
                    Some(request.height),
                    Some(request.view),
                ),
                iroha_core::sumeragi::message::CertifiedBlockFetch::Response(response) => (
                    "CertifiedBlockFetchResponse",
                    Some(response.height),
                    Some(response.view),
                ),
                iroha_core::sumeragi::message::CertifiedBlockFetch::Proof(proof) => (
                    "CertifiedBlockFetchProof",
                    Some(proof.height),
                    Some(proof.view),
                ),
                iroha_core::sumeragi::message::CertifiedBlockFetch::Body(body) => (
                    "CertifiedBlockFetchBody",
                    Some(body.height),
                    Some(body.view),
                ),
            },
            FetchPendingBlock(_request) => ("FetchPendingBlock", None, None),
            ProposalHint(hint) => ("ProposalHint", Some(hint.height), Some(hint.view)),
            Proposal(proposal) => (
                "Proposal",
                Some(proposal.header.height),
                Some(proposal.header.view),
            ),
            _ => unreachable!("legacy metadata helper received a non-legacy block message"),
        }
    }

    fn lane_block_message_meta(
        msg: &iroha_core::sumeragi::message::BlockMessage,
    ) -> (&'static str, Option<u64>, Option<u64>) {
        use iroha_core::sumeragi::message::BlockMessage::*;

        match msg {
            LaneBlockProposal(proposal) => (
                "LaneBlockProposal",
                Some(proposal.descriptor.lane_block_height),
                Some(proposal.descriptor.lane_block_view),
            ),
            LaneExecutablePayload(payload) => (
                "LaneExecutablePayload",
                Some(payload.origin_proposal.descriptor.lane_block_height),
                Some(payload.origin_proposal.descriptor.lane_block_view),
            ),
            LaneBlockNewViewVote(vote) => (
                "LaneBlockNewViewVote",
                Some(vote.body.lane_block_height),
                Some(vote.body.target_view),
            ),
            LaneBlockNewViewCertificate(certificate) => (
                "LaneBlockNewViewCertificate",
                Some(certificate.body.lane_block_height),
                Some(certificate.body.target_view),
            ),
            LaneBlockVote(vote) => {
                let label = match vote.body.phase {
                    iroha_core::sumeragi::consensus::Phase::Prepare => "LaneBlockPrepareVote",
                    iroha_core::sumeragi::consensus::Phase::Commit => "LaneBlockVote",
                    iroha_core::sumeragi::consensus::Phase::NewView => "LaneBlockNewViewVote",
                };
                (
                    label,
                    Some(vote.body.lane_block_height),
                    Some(vote.body.lane_block_view),
                )
            }
            LaneBlockQc(qc) => {
                let label = match qc.body.phase {
                    iroha_core::sumeragi::consensus::Phase::Prepare => "LaneBlockPrepareCert",
                    iroha_core::sumeragi::consensus::Phase::Commit => "LaneBlockCert",
                    iroha_core::sumeragi::consensus::Phase::NewView => "LaneBlockNewViewCert",
                };
                (
                    label,
                    Some(qc.body.lane_block_height),
                    Some(qc.body.lane_block_view),
                )
            }
            LaneBlockCertificate(certificate) => (
                "LaneBlockCertificate",
                Some(certificate.proposal.descriptor.lane_block_height),
                Some(certificate.proposal.descriptor.lane_block_view),
            ),
            LaneHistoricalRecoveryRequest(request) => {
                let proposal = request.proposal();
                (
                    "LaneHistoricalRecoveryRequest",
                    proposal
                        .map(|proposal| proposal.descriptor.lane_block_height)
                        .or_else(|| {
                            let source_height = request.source_height();
                            (source_height != 0).then_some(source_height)
                        }),
                    proposal.map(|proposal| proposal.descriptor.lane_block_view),
                )
            }
            LaneHistoricalRecoveryResponse(response) => match &response.payload {
                iroha_core::sumeragi::message::LaneHistoricalRecoveryPayloadV1::CanonicalBlock {
                    block,
                    ..
                } => (
                    "LaneHistoricalRecoveryResponse",
                    Some(block.header().height().get()),
                    Some(block.header().view_change_index()),
                ),
                iroha_core::sumeragi::message::LaneHistoricalRecoveryPayloadV1::AutonomousPayload {
                    payload,
                    ..
                } => (
                    "LaneHistoricalRecoveryResponse",
                    Some(payload.origin_proposal.descriptor.lane_block_height),
                    Some(payload.origin_proposal.descriptor.lane_block_view),
                ),
                iroha_core::sumeragi::message::LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
                    finality_artifact,
                    ..
                } => (
                    "LaneHistoricalRecoveryResponse",
                    Some(finality_artifact.height),
                    Some(finality_artifact.commit_qc.round.view),
                ),
            },
            _ => unreachable!("lane metadata helper received a non-lane block message"),
        }
    }

    fn v2_block_message_meta(
        payload: &iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload,
    ) -> (&'static str, Option<u64>, Option<u64>) {
        use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

        match payload {
            ConsensusMessageV2Payload::Proposal(value) => (
                "SumeragiV2Proposal",
                Some(value.round.height),
                Some(value.round.view),
            ),
            ConsensusMessageV2Payload::Vote(value) => {
                let label = match value.phase {
                    iroha_data_model::block::consensus_v2::GlobalPhase::Prepare => {
                        "SumeragiV2PrepareVote"
                    }
                    iroha_data_model::block::consensus_v2::GlobalPhase::Commit => {
                        "SumeragiV2CommitVote"
                    }
                };
                (label, Some(value.round.height), Some(value.round.view))
            }
            ConsensusMessageV2Payload::QuorumCertificate(value) => {
                let label = match value.phase {
                    iroha_data_model::block::consensus_v2::GlobalPhase::Prepare => {
                        "SumeragiV2PrepareCertificate"
                    }
                    iroha_data_model::block::consensus_v2::GlobalPhase::Commit => {
                        "SumeragiV2CommitCertificate"
                    }
                };
                (label, Some(value.round.height), Some(value.round.view))
            }
            ConsensusMessageV2Payload::TimeoutVote(value) => (
                "SumeragiV2TimeoutVote",
                Some(value.round.height),
                Some(value.round.view),
            ),
            ConsensusMessageV2Payload::TimeoutCertificate(value) => (
                "SumeragiV2TimeoutCertificate",
                Some(value.round.height),
                Some(value.round.view),
            ),
            ConsensusMessageV2Payload::PayloadManifest(value) => (
                "SumeragiV2PayloadManifest",
                Some(value.round.height),
                Some(value.round.view),
            ),
            ConsensusMessageV2Payload::PayloadChunk(_) => ("SumeragiV2PayloadChunk", None, None),
            ConsensusMessageV2Payload::CertifiedBodyRequest(value) => (
                "SumeragiV2CertifiedBodyRequest",
                Some(value.round.height),
                Some(value.round.view),
            ),
            ConsensusMessageV2Payload::CertifiedBodyResponse(value) => (
                "SumeragiV2CertifiedBodyResponse",
                Some(value.manifest.round.height),
                Some(value.manifest.round.view),
            ),
            ConsensusMessageV2Payload::CommitCertificateRequest(request) => (
                "SumeragiV2CommitCertificateRequest",
                Some(request.height),
                None,
            ),
            ConsensusMessageV2Payload::CommitCertificateResponse(response) => (
                "SumeragiV2CommitCertificateResponse",
                Some(response.certificate.round.height),
                Some(response.certificate.round.view),
            ),
            ConsensusMessageV2Payload::VrfCommit(_) => ("SumeragiV2VrfCommit", None, None),
            ConsensusMessageV2Payload::VrfReveal(_) => ("SumeragiV2VrfReveal", None, None),
        }
    }

    fn control_flow_meta(
        msg: &iroha_core::sumeragi::message::ControlFlow,
    ) -> (&'static str, Option<u64>, Option<u64>) {
        use iroha_core::sumeragi::message::ControlFlow::*;
        match msg {
            Evidence(_) => ("Evidence", None, None),
        }
    }
}

#[cfg(test)]
mod network_relay_tests {
    use std::{num::NonZeroU64, time::Duration};

    use iroha_core::{
        MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES, MAX_LANE_DRAIN_VOTE_WIRE_BYTES,
        lane_consensus::{
            LaneBlockNewViewBodyV1, LaneBlockNewViewCertificateV1, LaneBlockNewViewVoteV1,
            LaneDrainVoteV1, LaneExecutablePayloadV1,
        },
        sumeragi::{
            consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1, LaneBlockQcV1, Phase},
            message::{
                BlockMessage, BlockMessageWire, KURA_REPLICA_ADVERT_VERSION_V1, KuraReplicaAdvertV1,
            },
        },
        torii_proxy::{
            TORII_PROXY_REQUEST_VERSION_V5, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV4, ToriiProxyRequestV5,
            ToriiProxyResponseFormatV1, ToriiProxyResponseV1, ToriiReadEndpointV1,
            ToriiReadProxyRequestV1, ToriiRouteHintV1,
        },
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        block::{
            BlockHeader,
            consensus_v2::{
                self, CommitCertificateRequest, ConsensusMessageV2, ConsensusMessageV2Payload,
                HeightContextId, PROTOCOL_VERSION,
            },
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        nexus::{DataSpaceId, LaneId},
        peer::{Peer, PeerId},
    };

    use super::{
        BucketConfig, ConsensusIngressDropReason, ConsensusIngressLimiter, IngressRateClass,
        LowPriorityIngressDropReason, LowPriorityIngressLimiter, NetworkRelayShared, PenaltyConfig,
        SumeragiRelayClass, SumeragiRelayTerminalOutcome, obsolete_sumeragi_relay_terminal_meta,
        sumeragi_relay_class,
    };

    #[cfg(feature = "test-network-message-control")]
    #[derive(Clone)]
    struct TestReplyRoute {
        semantic_target: PeerId,
        authenticated_via: PeerId,
        active: std::sync::Arc<std::sync::atomic::AtomicBool>,
        tenure: std::sync::Arc<()>,
    }

    #[cfg(feature = "test-network-message-control")]
    impl TestReplyRoute {
        fn new(semantic_target: PeerId, authenticated_via: PeerId) -> Self {
            Self {
                semantic_target,
                authenticated_via,
                active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
                tenure: std::sync::Arc::new(()),
            }
        }

        fn same_tenure(&self, other: &Self) -> bool {
            std::sync::Arc::ptr_eq(&self.tenure, &other.tenure)
        }

        fn cancel(&self) {
            self.active
                .store(false, std::sync::atomic::Ordering::Release);
        }
    }

    #[cfg(feature = "test-network-message-control")]
    impl super::SumeragiReplyRouteLiveness for TestReplyRoute {
        fn is_active(&self) -> bool {
            self.active.load(std::sync::atomic::Ordering::Acquire)
        }
    }

    #[cfg(feature = "test-network-message-control")]
    struct TestRelayWork {
        peer: Peer,
        authenticated_via: PeerId,
        message: iroha_core::NetworkMessage,
        size_bytes: usize,
        reply_route: TestReplyRoute,
    }

    #[cfg(feature = "test-network-message-control")]
    impl super::AuthenticatedSumeragiRelayWork for TestRelayWork {
        fn authenticated_via(&self) -> &PeerId {
            &self.authenticated_via
        }
    }

    #[cfg(feature = "test-network-message-control")]
    fn reattach_test_reply_route(
        peer: Peer,
        message: iroha_core::NetworkMessage,
        size_bytes: usize,
        reply_route: TestReplyRoute,
    ) -> Result<TestRelayWork, TestReplyRoute> {
        if !super::SumeragiReplyRouteLiveness::is_active(&reply_route)
            || &reply_route.semantic_target != peer.id()
        {
            return Err(reply_route);
        }
        Ok(TestRelayWork {
            peer,
            authenticated_via: reply_route.authenticated_via.clone(),
            message,
            size_bytes,
            reply_route,
        })
    }

    #[cfg(feature = "test-network-message-control")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_control_hold_release_preserves_live_route_and_retires_canceled_reentry() {
        use super::{
            HeldSumeragiReentry, HeldSumeragiReentryFailure, SumeragiRelayPreparationBoundary,
            SumeragiRelayPreparationBoundaryError, SumeragiRelayPreparationParts,
            SumeragiRelayTerminalOutcome, prepare_sumeragi_relay_work_boundary,
            rehydrate_held_sumeragi_relay_work, sumeragi_reply_route_terminal_if_inactive,
        };

        let (_control_dir, controller) =
            super::consensus_message_control::Controller::<TestReplyRoute>::for_tests();
        let semantic_peer = sample_peer();
        let authenticated_via = sample_peer().id().clone();
        assert_ne!(semantic_peer.id(), &authenticated_via);

        controller.drain_subsequent_messages_for_tests();
        let live_route = TestReplyRoute::new(semantic_peer.id().clone(), authenticated_via.clone());
        let expected_live_route = live_route.clone();
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                false,
                SumeragiRelayPreparationParts {
                    peer: semantic_peer.clone(),
                    authenticated_via: authenticated_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 101,
                    reply_route: Some(live_route),
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Held
        ));
        let held = controller
            .next_release()
            .expect("take live held release")
            .expect("live occurrence must be queued for release");
        assert_eq!(held.peer, semantic_peer);
        assert_eq!(held.authenticated_via, authenticated_via);
        assert!(
            held.reply_route
                .as_ref()
                .is_some_and(|route| route.same_tenure(&expected_live_route))
        );
        let (live_sequence, live_work, live_ownership) =
            match rehydrate_held_sumeragi_relay_work(held, reattach_test_reply_route) {
                HeldSumeragiReentry::Ready {
                    sequence,
                    work,
                    ownership,
                    ..
                } => (sequence, work, ownership),
                HeldSumeragiReentry::RetireStale { .. } => panic!("live route was retired"),
                HeldSumeragiReentry::Reject { reason, .. } => {
                    panic!("live route was rejected: {reason:?}")
                }
            };
        assert_eq!(live_work.authenticated_via, authenticated_via);
        assert!(live_work.reply_route.same_tenure(&expected_live_route));
        let live_reentry = prepare_sumeragi_relay_work_boundary(
            Some(&controller),
            true,
            SumeragiRelayPreparationParts {
                peer: live_work.peer,
                authenticated_via: live_work.authenticated_via,
                message: live_work.message,
                size_bytes: live_work.size_bytes,
                reply_route: Some(live_work.reply_route),
                ownership: live_ownership,
            },
        );
        let live_prepared = match live_reentry {
            SumeragiRelayPreparationBoundary::Prepared(parts) => parts,
            SumeragiRelayPreparationBoundary::Held => panic!("release was held twice"),
            SumeragiRelayPreparationBoundary::Dropped(_) => panic!("release was dropped"),
            SumeragiRelayPreparationBoundary::RetiredInactiveReplyRoute(_) => {
                panic!("live release was retired during re-entry")
            }
            SumeragiRelayPreparationBoundary::Rejected { .. } => {
                panic!("live release was rejected during re-entry")
            }
        };
        assert_eq!(live_prepared.authenticated_via, authenticated_via);
        assert!(
            live_prepared
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_tenure(&expected_live_route))
        );
        controller
            .complete_release(
                live_sequence,
                super::consensus_message_control::ReleaseOutcome::Delivered,
            )
            .expect("complete live held release");

        controller.drain_subsequent_messages_for_tests();
        let held_cancel_route =
            TestReplyRoute::new(semantic_peer.id().clone(), authenticated_via.clone());
        let cancel_while_held = held_cancel_route.clone();
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                false,
                SumeragiRelayPreparationParts {
                    peer: semantic_peer.clone(),
                    authenticated_via: authenticated_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 102,
                    reply_route: Some(held_cancel_route),
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Held
        ));
        cancel_while_held.cancel();
        let held = controller
            .next_release()
            .expect("take canceled held release")
            .expect("canceled occurrence remains explicitly terminal");
        let held_sequence = held.sequence;
        assert!(matches!(
            rehydrate_held_sumeragi_relay_work(held, reattach_test_reply_route),
            HeldSumeragiReentry::RetireStale { sequence, .. } if sequence == held_sequence
        ));
        controller
            .complete_release(
                held_sequence,
                super::consensus_message_control::ReleaseOutcome::Retired,
            )
            .expect("retire occurrence canceled while held");

        controller.drain_subsequent_messages_for_tests();
        let cancelable_route =
            TestReplyRoute::new(semantic_peer.id().clone(), authenticated_via.clone());
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                false,
                SumeragiRelayPreparationParts {
                    peer: semantic_peer.clone(),
                    authenticated_via: authenticated_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 103,
                    reply_route: Some(cancelable_route),
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Held
        ));
        let held = controller
            .next_release()
            .expect("take cancelable held release")
            .expect("cancelable occurrence must be queued for release");
        let (canceled_sequence, canceled_work, canceled_ownership) =
            match rehydrate_held_sumeragi_relay_work(held, reattach_test_reply_route) {
                HeldSumeragiReentry::Ready {
                    sequence,
                    work,
                    ownership,
                    ..
                } => (sequence, work, ownership),
                HeldSumeragiReentry::RetireStale { .. } => {
                    panic!("route was canceled before the intended race boundary")
                }
                HeldSumeragiReentry::Reject { reason, .. } => {
                    panic!("cancelable route was rejected before cancellation: {reason:?}")
                }
            };
        canceled_work.reply_route.cancel();
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                true,
                SumeragiRelayPreparationParts {
                    peer: canceled_work.peer,
                    authenticated_via: canceled_work.authenticated_via,
                    message: canceled_work.message,
                    size_bytes: canceled_work.size_bytes,
                    reply_route: Some(canceled_work.reply_route),
                    ownership: canceled_ownership,
                },
            ),
            SumeragiRelayPreparationBoundary::RetiredInactiveReplyRoute(_)
        ));
        controller
            .complete_release(
                canceled_sequence,
                super::consensus_message_control::ReleaseOutcome::Retired,
            )
            .expect("retire canceled held occurrence");
        assert!(
            controller
                .next_release()
                .expect("inspect post-retirement queue")
                .is_none()
        );

        let protected_peer = sample_peer();
        let rejected_via = sample_peer().id().clone();
        let independent_peer = sample_peer();
        let independent_via = sample_peer().id().clone();
        assert_ne!(protected_peer.id(), &rejected_via);
        assert_ne!(independent_peer.id(), &independent_via);
        assert_ne!(rejected_via, independent_via);

        controller.drain_subsequent_messages_for_tests();
        let protected_route =
            TestReplyRoute::new(protected_peer.id().clone(), rejected_via.clone());
        let expected_protected_route = protected_route.clone();
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                false,
                SumeragiRelayPreparationParts {
                    peer: protected_peer.clone(),
                    authenticated_via: rejected_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 201,
                    reply_route: Some(protected_route),
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Held
        ));
        let independent_route =
            TestReplyRoute::new(independent_peer.id().clone(), independent_via.clone());
        let expected_independent_route = independent_route.clone();
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                false,
                SumeragiRelayPreparationParts {
                    peer: independent_peer.clone(),
                    authenticated_via: independent_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 202,
                    reply_route: Some(independent_route),
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Held
        ));

        let missing_ownership_route =
            TestReplyRoute::new(protected_peer.id().clone(), rejected_via.clone());
        let expected_missing_ownership_route = missing_ownership_route.clone();
        assert!(matches!(
            rehydrate_held_sumeragi_relay_work(
                super::consensus_message_control::HeldMessage {
                    sequence: 10_001,
                    peer: protected_peer.clone(),
                    authenticated_via: rejected_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 301,
                    reply_route: Some(missing_ownership_route),
                    ownership: None::<u64>,
                },
                reattach_test_reply_route,
            ),
            HeldSumeragiReentry::Reject {
                sequence: 10_001,
                reason: HeldSumeragiReentryFailure::MissingOwnership,
                ownership: None,
            }
        ));
        assert!(super::SumeragiReplyRouteLiveness::is_active(
            &expected_missing_ownership_route
        ));

        let unsupported_route =
            TestReplyRoute::new(protected_peer.id().clone(), rejected_via.clone());
        let expected_unsupported_route = unsupported_route.clone();
        assert!(matches!(
            rehydrate_held_sumeragi_relay_work(
                super::consensus_message_control::HeldMessage {
                    sequence: 10_002,
                    peer: protected_peer.clone(),
                    authenticated_via: rejected_via.clone(),
                    message: limited_msg(),
                    size_bytes: 302,
                    reply_route: Some(unsupported_route),
                    ownership: Some(42),
                },
                reattach_test_reply_route,
            ),
            HeldSumeragiReentry::Reject {
                sequence: 10_002,
                reason: HeldSumeragiReentryFailure::UnsupportedMessage,
                ownership: Some(42),
            }
        ));
        assert!(super::SumeragiReplyRouteLiveness::is_active(
            &expected_unsupported_route
        ));

        assert!(matches!(
            rehydrate_held_sumeragi_relay_work(
                super::consensus_message_control::HeldMessage::<TestReplyRoute, u64> {
                    sequence: 10_003,
                    peer: protected_peer.clone(),
                    authenticated_via: rejected_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 303,
                    reply_route: None,
                    ownership: Some(43),
                },
                reattach_test_reply_route,
            ),
            HeldSumeragiReentry::Reject {
                sequence: 10_003,
                reason: HeldSumeragiReentryFailure::MissingReplyRoute,
                ownership: Some(43),
            }
        ));

        let wrong_target = sample_peer();
        assert_ne!(protected_peer.id(), wrong_target.id());
        let mismatched_route = TestReplyRoute::new(wrong_target.id().clone(), rejected_via.clone());
        let expected_mismatched_route = mismatched_route.clone();
        assert!(matches!(
            rehydrate_held_sumeragi_relay_work(
                super::consensus_message_control::HeldMessage {
                    sequence: 10_004,
                    peer: protected_peer.clone(),
                    authenticated_via: rejected_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 304,
                    reply_route: Some(mismatched_route),
                    ownership: Some(44),
                },
                reattach_test_reply_route,
            ),
            HeldSumeragiReentry::Reject {
                sequence: 10_004,
                reason: HeldSumeragiReentryFailure::RouteMismatch,
                ownership: Some(44),
            }
        ));
        assert!(super::SumeragiReplyRouteLiveness::is_active(
            &expected_mismatched_route
        ));

        let substituted_via = sample_peer().id().clone();
        assert_ne!(rejected_via, substituted_via);
        let substituted_route =
            TestReplyRoute::new(protected_peer.id().clone(), substituted_via.clone());
        let expected_substituted_route = substituted_route.clone();
        assert!(matches!(
            rehydrate_held_sumeragi_relay_work(
                super::consensus_message_control::HeldMessage {
                    sequence: 10_005,
                    peer: protected_peer.clone(),
                    authenticated_via: rejected_via.clone(),
                    message: v2_vote_msg(),
                    size_bytes: 305,
                    reply_route: Some(substituted_route),
                    ownership: Some(45),
                },
                reattach_test_reply_route,
            ),
            HeldSumeragiReentry::Reject {
                sequence: 10_005,
                reason: HeldSumeragiReentryFailure::AuthenticatedViaMismatch,
                ownership: Some(45),
            }
        ));
        assert!(super::SumeragiReplyRouteLiveness::is_active(
            &expected_substituted_route
        ));
        assert_eq!(
            expected_substituted_route.authenticated_via,
            substituted_via
        );

        for (expected_peer, expected_via, expected_route, expected_size) in [
            (protected_peer, rejected_via, expected_protected_route, 201),
            (
                independent_peer,
                independent_via,
                expected_independent_route,
                202,
            ),
        ] {
            let held = controller
                .next_release()
                .expect("release capability after rejected reentry probes")
                .expect("rejected reentry must not consume a queued capability");
            assert_eq!(held.peer, expected_peer);
            assert_eq!(held.authenticated_via, expected_via);
            assert_eq!(held.size_bytes, expected_size);
            assert!(
                held.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_tenure(&expected_route))
            );
            let sequence = match rehydrate_held_sumeragi_relay_work(held, reattach_test_reply_route)
            {
                HeldSumeragiReentry::Ready {
                    sequence,
                    class: super::SumeragiRelayClass::V2,
                    work,
                    ownership: (),
                } => {
                    assert_eq!(work.peer, expected_peer);
                    assert_eq!(work.authenticated_via, expected_via);
                    assert_eq!(work.size_bytes, expected_size);
                    assert!(work.reply_route.same_tenure(&expected_route));
                    sequence
                }
                HeldSumeragiReentry::Ready { .. }
                | HeldSumeragiReentry::RetireStale { .. }
                | HeldSumeragiReentry::Reject { .. } => {
                    panic!("queued capability changed during rejected reentry probes")
                }
            };
            controller
                .complete_release(
                    sequence,
                    super::consensus_message_control::ReleaseOutcome::Delivered,
                )
                .expect("complete unchanged queued capability");
        }
        assert!(
            controller
                .next_release()
                .expect("inspect queue after independent source delivery")
                .is_none()
        );

        let retry_route =
            TestReplyRoute::new(semantic_peer.id().clone(), authenticated_via.clone());
        assert_eq!(
            sumeragi_reply_route_terminal_if_inactive(&retry_route),
            None,
            "the live route may own a retry"
        );
        retry_route.cancel();
        assert_eq!(
            sumeragi_reply_route_terminal_if_inactive(&retry_route),
            Some(SumeragiRelayTerminalOutcome::Retired),
            "cancellation before retry requeue is terminal retirement"
        );

        let fresh_route =
            TestReplyRoute::new(semantic_peer.id().clone(), authenticated_via.clone());
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                false,
                SumeragiRelayPreparationParts {
                    peer: semantic_peer,
                    authenticated_via,
                    message: v2_vote_msg(),
                    size_bytes: 104,
                    reply_route: Some(fresh_route),
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Prepared(_)
        ));
        assert!(matches!(
            prepare_sumeragi_relay_work_boundary(
                Some(&controller),
                true,
                SumeragiRelayPreparationParts {
                    peer: sample_peer(),
                    authenticated_via: sample_peer().id().clone(),
                    message: v2_vote_msg(),
                    size_bytes: 105,
                    reply_route: None,
                    ownership: (),
                },
            ),
            SumeragiRelayPreparationBoundary::Rejected {
                error: SumeragiRelayPreparationBoundaryError::MissingReplyRoute,
                ..
            }
        ));
    }

    #[test]
    fn block_message_blocking_ingress_policy_admits_only_authoritative_v2() {
        assert!(
            BlockMessage::LaneBlockProposal(sample_lane_block_proposal())
                .requires_blocking_ingress()
        );
        assert!(
            BlockMessage::LaneExecutablePayload(sample_lane_executable_payload())
                .requires_blocking_ingress()
        );
        assert!(
            BlockMessage::LaneBlockNewViewVote(sample_lane_block_new_view_vote())
                .requires_blocking_ingress()
        );
        assert!(
            BlockMessage::LaneBlockNewViewCertificate(sample_lane_block_new_view_certificate())
                .requires_blocking_ingress()
        );
        assert!(
            BlockMessage::LaneBlockVote(sample_lane_block_vote(Phase::Prepare))
                .requires_blocking_ingress()
        );
        assert!(
            BlockMessage::LaneBlockQc(sample_lane_block_qc(Phase::Commit))
                .requires_blocking_ingress()
        );
        assert!(v2_payload_chunk_block_message().requires_blocking_ingress());
        assert!(sumeragi_v2_commit_certificate_request().requires_blocking_ingress());
        assert!(kura_replica_advert_block_message().requires_blocking_ingress());
    }

    fn kura_replica_advert_block_message() -> BlockMessage {
        let keeper = PeerId::new(KeyPair::random().public_key().clone());
        BlockMessage::KuraReplicaAdvert(KuraReplicaAdvertV1 {
            version: KURA_REPLICA_ADVERT_VERSION_V1,
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"relay-kura-advert-test")),
            ),
            height: 11,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"advert-block")),
            executed_block_wire_len: 1024,
            executed_block_wire_hash: Hash::new(b"advert-wire"),
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(b"advert-finality")),
            keeper_index: 0,
            keeper,
            signature: vec![0xA5; 96],
        })
    }

    #[test]
    fn kura_replica_advert_is_fixed_small_limited_v2_relay_work() {
        let message = kura_replica_advert_block_message();
        let network_message = sumeragi_msg(message.clone());
        let policy = ConsensusIngressLimiter::ingress_policy(&network_message);
        assert_eq!(policy.rate_class, Some(IngressRateClass::Limited));
        assert!(policy.apply_penalty);
        assert_eq!(
            sumeragi_relay_class(&network_message),
            Some(SumeragiRelayClass::V2)
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&message),
            ("KuraReplicaAdvert", Some(11), None)
        );

        let peer = sample_peer();
        let mut limiter = ConsensusIngressLimiter::new(
            None,
            None,
            None,
            None,
            None,
            None,
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );
        assert_eq!(
            limiter.should_drop(
                &peer,
                &network_message,
                MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES + 1,
            ),
            Some(ConsensusIngressDropReason::Bytes)
        );
    }

    #[test]
    fn obsolete_sumeragi_relay_message_completes_as_delivered() {
        assert_eq!(
            obsolete_sumeragi_relay_terminal_meta(&retired_vrf_commit_msg())
                .map(|(_, _, _, outcome)| outcome),
            Some(SumeragiRelayTerminalOutcome::Delivered)
        );
        assert!(obsolete_sumeragi_relay_terminal_meta(&v2_vote_msg()).is_none());
    }

    #[test]
    fn sumeragi_v2_ingress_policy_and_metadata_match_payload_kind() {
        let chunk = v2_payload_chunk_block_message();
        let chunk_policy = ConsensusIngressLimiter::ingress_policy(&sumeragi_msg(chunk.clone()));
        assert_eq!(chunk_policy.rate_class, Some(IngressRateClass::Critical));
        assert_eq!(
            NetworkRelayShared::block_message_meta(&chunk),
            ("SumeragiV2PayloadChunk", None, None)
        );

        let request = sumeragi_v2_commit_certificate_request();
        let request_policy =
            ConsensusIngressLimiter::ingress_policy(&sumeragi_msg(request.clone()));
        assert_eq!(request_policy.rate_class, Some(IngressRateClass::Critical));
        assert_eq!(
            NetworkRelayShared::block_message_meta(&request),
            ("SumeragiV2CommitCertificateRequest", Some(9), None)
        );
    }

    pub fn sample_peer() -> Peer {
        let keypair = KeyPair::random();
        Peer::new(
            "127.0.0.1:0".parse().expect("socket address"),
            keypair.public_key().clone(),
        )
    }

    fn nz_u32(value: u32) -> std::num::NonZeroU32 {
        std::num::NonZeroU32::new(value).expect("non-zero")
    }

    pub fn sumeragi_msg(msg: BlockMessage) -> iroha_core::NetworkMessage {
        iroha_core::NetworkMessage::SumeragiBlock(std::sync::Arc::new(BlockMessageWire::new(msg)))
    }

    fn lane_drain_vote_msg() -> iroha_core::NetworkMessage {
        let keypair = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
            .expect("generate lane-drain ingress fixture keypair");
        let signer = PeerId::new(keypair.public_key().clone());
        let validator_set = vec![signer.clone()];
        let body = LaneDrainCertificateBodyV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                network_id: NetworkId::from_genesis_hash(dummy_block_hash(0xA1)),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(7),
                lane_incarnation: Hash::new(b"irohad-lane-drain-incarnation"),
                close_global_height: 12,
                initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(7),
                    Hash::new(b"irohad-lane-drain-incarnation"),
                    4,
                    Some(Hash::new(b"irohad-lane-drain-initial")),
                ),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                LaneId::new(3),
                DataSpaceId::new(7),
                Hash::new(b"irohad-lane-drain-incarnation"),
                5,
                Some(Hash::new(b"irohad-lane-drain-final")),
            ),
        };
        let proof_of_possession = iroha_crypto::bls_normal_pop_prove(keypair.private_key())
            .expect("derive lane-drain fixture proof of possession");
        let bls_signature =
            iroha_crypto::Signature::try_new(keypair.private_key(), &body.signature_preimage())
                .expect("sign lane-drain ingress fixture")
                .payload()
                .to_vec();
        iroha_core::NetworkMessage::LaneDrainVote(Box::new(LaneDrainVoteV1 {
            body,
            signer,
            proof_of_possession,
            bls_signature,
        }))
    }

    pub fn sample_v2_round(height: u64, view: u64) -> consensus_v2::ConsensusRound {
        consensus_v2::ConsensusRound {
            context_id: consensus_v2::HeightContextId(HashOf::from_untyped_unchecked(
                Hash::prehashed([0x61; 32]),
            )),
            height,
            view,
        }
    }

    pub fn sample_v2_subject() -> consensus_v2::BlockSubject {
        consensus_v2::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x62; 32])),
            payload_hash: Hash::prehashed([0x63; 32]),
        }
    }

    fn v2_vote_block_message() -> BlockMessage {
        BlockMessage::V2(consensus_v2::ConsensusMessageV2::new(
            consensus_v2::ConsensusMessageV2Payload::Vote(consensus_v2::Vote {
                round: sample_v2_round(5, 7),
                proposal_round: sample_v2_round(5, 7),
                phase: consensus_v2::GlobalPhase::Prepare,
                subject: sample_v2_subject(),
                execution_commitment:
                    consensus_v2::ExecutionCommitment::without_topups_or_merge_carrier(
                        Hash::prehashed([0x64; 32]),
                        Hash::prehashed([0x65; 32]),
                        Hash::prehashed([0x66; 32]),
                        1,
                        Hash::prehashed([0x67; 32]),
                    ),
                signer: 0,
                signature: vec![0x64],
            }),
        ))
    }

    fn v2_payload_chunk_block_message() -> BlockMessage {
        BlockMessage::V2(consensus_v2::ConsensusMessageV2::new(
            consensus_v2::ConsensusMessageV2Payload::PayloadChunk(consensus_v2::PayloadChunk {
                manifest_hash: HashOf::<consensus_v2::PayloadManifest>::from_untyped_unchecked(
                    Hash::prehashed([0x65; 32]),
                ),
                index: 0,
                bytes: vec![0x66],
                sender: 0,
                signature: vec![0x67],
            }),
        ))
    }

    fn sample_v2_manifest() -> consensus_v2::PayloadManifest {
        consensus_v2::PayloadManifest {
            round: sample_v2_round(5, 7),
            subject: sample_v2_subject(),
            payload_size_bytes: 4,
            layout: consensus_v2::SumeragiV2GenesisContextParameters::recommended().da_layout,
            chunk_hashes: vec![Hash::new(b"body")],
            chunk_root: Hash::new(b"chunk-root"),
        }
    }

    fn v2_proposal_msg() -> iroha_core::NetworkMessage {
        let manifest = sample_v2_manifest();
        sumeragi_msg(BlockMessage::V2(ConsensusMessageV2::new(
            ConsensusMessageV2Payload::Proposal(consensus_v2::Proposal {
                round: manifest.round,
                proposer: 0,
                subject: manifest.subject,
                manifest,
                justification: consensus_v2::ProposalJustification::ParentCommit(
                    consensus_v2::ParentCommitJustification { certificate: None },
                ),
                signature: vec![0x68],
            }),
        )))
    }

    fn v2_certified_body_response_msg() -> iroha_core::NetworkMessage {
        sumeragi_msg(BlockMessage::V2(ConsensusMessageV2::new(
            ConsensusMessageV2Payload::CertifiedBodyResponse(consensus_v2::CertifiedBodyResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"irohad-certified-body-request",
                )),
                manifest: sample_v2_manifest(),
                body: b"body".to_vec(),
                responder: 0,
                signature: vec![0x69],
            }),
        )))
    }

    fn certified_merge_sidecar_control_messages() -> (
        iroha_core::NetworkMessage,
        iroha_core::NetworkMessage,
        iroha_core::NetworkMessage,
    ) {
        use iroha_core::merge_sidecar::{
            CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarCloseAckV1,
            CertifiedMergeSidecarCloseV1, CertifiedMergeSidecarGenerationHintV1,
            CertifiedMergeSidecarMessage, CertifiedMergeSidecarServiceGenerationV1,
            CertifiedMergeSidecarStreamEpochV1,
        };

        let requester = PeerId::new(KeyPair::random().public_key().clone());
        let responder = PeerId::new(KeyPair::random().public_key().clone());
        let stream_epoch = CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("sidecar control stream epoch is non-zero"),
        );
        let service_generation = CertifiedMergeSidecarServiceGenerationV1::INITIAL;
        let close_id = Hash::prehashed([0x6C; 32]);
        let close = iroha_core::NetworkMessage::CertifiedMergeSidecar(std::sync::Arc::new(
            CertifiedMergeSidecarMessage::Close(CertifiedMergeSidecarCloseV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation,
                stream_epoch,
                closed_through: 7,
                close_id,
                requester: requester.clone(),
                responder: responder.clone(),
            }),
        ));
        let ack = iroha_core::NetworkMessage::CertifiedMergeSidecar(std::sync::Arc::new(
            CertifiedMergeSidecarMessage::CloseAck(CertifiedMergeSidecarCloseAckV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation,
                stream_epoch,
                closed_through: 7,
                close_id,
                requester: requester.clone(),
                responder: responder.clone(),
            }),
        ));
        let hint = iroha_core::NetworkMessage::CertifiedMergeSidecar(std::sync::Arc::new(
            CertifiedMergeSidecarMessage::GenerationHint(CertifiedMergeSidecarGenerationHintV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                observed_generation: service_generation,
                current_generation: CertifiedMergeSidecarServiceGenerationV1(
                    NonZeroU64::new(2).expect("sidecar control successor generation is non-zero"),
                ),
                observed_message_hash: Hash::new(b"sidecar control observed message"),
                hint_id: Hash::new(b"sidecar control generation Hint"),
                requester,
                responder,
            }),
        ));
        (close, ack, hint)
    }

    fn limited_msg() -> iroha_core::NetworkMessage {
        iroha_core::NetworkMessage::Health
    }

    fn sumeragi_v2_commit_certificate_request() -> BlockMessage {
        let requester = PeerId::new(KeyPair::random().public_key().clone());
        BlockMessage::V2(ConsensusMessageV2::new(
            ConsensusMessageV2Payload::CommitCertificateRequest(CommitCertificateRequest {
                protocol_version: PROTOCOL_VERSION,
                network_id: NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                        b"irohad-v2-context-genesis",
                    )),
                ),
                context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"irohad-v2-context",
                ))),
                height: 9,
                requester,
                signature: vec![0xCC],
            }),
        ))
    }

    pub fn v2_vote_msg() -> iroha_core::NetworkMessage {
        sumeragi_msg(v2_vote_block_message())
    }

    fn retired_vrf_commit_msg() -> iroha_core::NetworkMessage {
        sumeragi_msg(BlockMessage::VrfCommit(
            iroha_data_model::block::consensus::VrfCommit {
                epoch: 9,
                commitment: [0x91; 32],
                signer: 1,
                bls_sig: vec![0x92],
            },
        ))
    }

    fn sample_lane_block_proposal() -> LaneBlockProposalV1 {
        let validator_set = vec![PeerId::new(KeyPair::random().public_key().clone())];
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation: Hash::new(b"irohad-lane-fixture-incarnation"),
            proposal_height: 5,
            previous_lane_block_height: 4,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x50; 32])),
            lane_block_height: 5,
            lane_block_view: 7,
            subject_hash: Hash::prehashed([0x51; 32]),
            payload_ownership_hash: Hash::prehashed([0x52; 32]),
            rbc_instance_hash: Hash::prehashed([0x53; 32]),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::prehashed([0x54; 32])],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "nexus:lane-block:test".to_owned(),
            descriptor_hash: Hash::prehashed([0x55; 32]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0x56; 32]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn sample_lane_executable_payload() -> LaneExecutablePayloadV1 {
        let origin_proposal = sample_lane_block_proposal();
        let producer = origin_proposal.descriptor.validator_set[0].clone();
        LaneExecutablePayloadV1 {
            version: 2,
            network_id: NetworkId::from_genesis_hash(dummy_block_hash(0xA1)),
            epoch: 3,
            origin_proposal,
            entrypoint_hashes: Vec::new(),
            entrypoints: Vec::new(),
            reservation_keys: Vec::new(),
            routing_plans: Vec::new(),
            native_amx_receipts: Vec::new(),
            payload_hash: Hash::new(b"irohad-lane-payload"),
            producer,
            producer_signature: vec![0xAA],
        }
    }

    fn sample_lane_block_new_view_body() -> LaneBlockNewViewBodyV1 {
        let proposal = sample_lane_block_proposal();
        let descriptor = &proposal.descriptor;
        LaneBlockNewViewBodyV1 {
            version: 1,
            network_id: NetworkId::from_genesis_hash(dummy_block_hash(0xA1)),
            epoch: 3,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            from_view: descriptor.lane_block_view,
            target_view: descriptor.lane_block_view + 1,
            locked_proposal_hash: proposal.proposal_hash,
            locked_descriptor_hash: descriptor.descriptor_hash,
            executable_payload_hash: Hash::new(b"irohad-lane-new-view-payload"),
            validator_set_hash_version: descriptor.validator_set_hash_version,
            validator_set_hash: descriptor.validator_set_hash,
            validator_count: descriptor.validator_count,
            min_quorum: descriptor.min_quorum,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
        }
    }

    fn sample_lane_block_new_view_vote() -> LaneBlockNewViewVoteV1 {
        LaneBlockNewViewVoteV1 {
            body: sample_lane_block_new_view_body(),
            signer: PeerId::new(KeyPair::random().public_key().clone()),
            bls_signature: vec![0xCC],
        }
    }

    fn sample_lane_block_new_view_certificate() -> LaneBlockNewViewCertificateV1 {
        LaneBlockNewViewCertificateV1 {
            body: sample_lane_block_new_view_body(),
            validator_set: vec![PeerId::new(KeyPair::random().public_key().clone())],
            signers_bitmap: vec![0b1],
            bls_aggregate_signature: vec![0xDD],
        }
    }

    fn sample_lane_block_vote(phase: Phase) -> iroha_core::lane_consensus::LaneBlockVoteV1 {
        let proposal = sample_lane_block_proposal();
        iroha_core::lane_consensus::LaneBlockVoteV1 {
            body: proposal.vote_body(phase),
            payload_availability_vote: None,
            signer: proposal.descriptor.validator_set[0].clone(),
            bls_signature: vec![0xAA],
        }
    }

    fn sample_lane_block_qc(phase: Phase) -> LaneBlockQcV1 {
        let proposal = sample_lane_block_proposal();
        LaneBlockQcV1 {
            body: proposal.vote_body(phase),
            validator_set_hash_version: proposal.descriptor.validator_set_hash_version,
            validator_set_hash: proposal.descriptor.validator_set_hash,
            validator_set: proposal.descriptor.validator_set.clone(),
            signers_bitmap: vec![0b1],
            bls_aggregate_signature: vec![0xBB],
            payload_availability_qc: None,
        }
    }

    fn lane_block_proposal_msg() -> iroha_core::NetworkMessage {
        sumeragi_msg(BlockMessage::LaneBlockProposal(sample_lane_block_proposal()))
    }

    fn lane_block_vote_msg(phase: Phase) -> iroha_core::NetworkMessage {
        sumeragi_msg(BlockMessage::LaneBlockVote(sample_lane_block_vote(phase)))
    }

    fn lane_block_qc_msg(phase: Phase) -> iroha_core::NetworkMessage {
        sumeragi_msg(BlockMessage::LaneBlockQc(sample_lane_block_qc(phase)))
    }

    fn torii_proxy_request_msg() -> iroha_core::NetworkMessage {
        iroha_core::NetworkMessage::ToriiProxyRequest(Arc::new(ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::prehashed([0x41; 32]),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV4::Read(ToriiReadProxyRequestV1 {
                endpoint: ToriiReadEndpointV1::AccountsList,
                expected_route: ToriiRouteHintV1 {
                    lane_id: LaneId::SINGLE,
                    dataspace_id: DataSpaceId::UNIVERSAL,
                },
                path_args: Vec::new(),
                query_string: None,
                body: Vec::new(),
                response_format: ToriiProxyResponseFormatV1::Json,
            }),
        }))
    }

    fn torii_proxy_response_msg() -> iroha_core::NetworkMessage {
        iroha_core::NetworkMessage::ToriiProxyResponse(Box::new(ToriiProxyResponseV1 {
            schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
            request_id: Hash::prehashed([0x42; 32]),
            response: ToriiProxyHttpResponseV1 {
                status_code: 200,
                headers: Vec::new(),
                body: Vec::new(),
            },
        }))
    }

    #[test]
    fn consensus_ingress_rate_limit_drops_burst() {
        let peer = sample_peer();
        let msg = limited_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            None,
            None,
            None,
            None,
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &msg, 32), None);
        assert_eq!(
            limiter.should_drop(&peer, &msg, 32),
            Some(ConsensusIngressDropReason::Rate)
        );
    }

    #[test]
    fn consensus_ingress_critical_bypasses_limited_bucket() {
        let peer = sample_peer();
        let msg = limited_msg();
        let vote = v2_vote_msg();
        let proposal = v2_proposal_msg();
        let chunk = sumeragi_msg(v2_payload_chunk_block_message());
        let request = sumeragi_msg(sumeragi_v2_commit_certificate_request());
        let lane_proposal = lane_block_proposal_msg();
        let lane_vote = lane_block_vote_msg(Phase::Prepare);
        let lane_qc = lane_block_qc_msg(Phase::Commit);
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            None,
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(32),
                burst: nz_u32(32),
            }),
            None,
            PenaltyConfig {
                threshold: 1,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(10),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &msg, 1), None);
        assert_eq!(
            limiter.should_drop(&peer, &msg, 1),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(limiter.should_drop(&peer, &vote, 1), None);
        assert_eq!(limiter.should_drop(&peer, &proposal, 1), None);
        assert_eq!(limiter.should_drop(&peer, &chunk, 1), None);
        assert_eq!(limiter.should_drop(&peer, &request, 1), None);
        assert_eq!(limiter.should_drop(&peer, &lane_proposal, 1), None);
        assert_eq!(limiter.should_drop(&peer, &lane_vote, 1), None);
        assert_eq!(limiter.should_drop(&peer, &lane_qc, 1), None);
    }

    #[test]
    fn consensus_ingress_proposal_uses_critical_bucket() {
        let peer = sample_peer();
        let msg = limited_msg();
        let proposal = v2_proposal_msg();
        let lane_proposal = lane_block_proposal_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            None,
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(20),
                burst: nz_u32(20),
            }),
            None,
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &msg, 1), None);
        assert_eq!(
            limiter.should_drop(&peer, &msg, 1),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(limiter.should_drop(&peer, &proposal, 1), None);
        assert_eq!(limiter.should_drop(&peer, &lane_proposal, 1), None);
    }

    #[test]
    fn consensus_ingress_v2_uses_critical_bucket() {
        let policy = ConsensusIngressLimiter::ingress_policy(&v2_vote_msg());

        assert_eq!(policy.rate_class, Some(IngressRateClass::Critical));
        assert!(!policy.apply_penalty);
    }

    #[test]
    fn block_message_meta_labels_lane_block_messages() {
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockProposal(
                sample_lane_block_proposal()
            )),
            ("LaneBlockProposal", Some(5), Some(7))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneExecutablePayload(
                sample_lane_executable_payload()
            )),
            ("LaneExecutablePayload", Some(5), Some(7))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockNewViewVote(
                sample_lane_block_new_view_vote()
            )),
            ("LaneBlockNewViewVote", Some(5), Some(8))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockNewViewCertificate(
                sample_lane_block_new_view_certificate()
            )),
            ("LaneBlockNewViewCertificate", Some(5), Some(8))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockVote(
                sample_lane_block_vote(Phase::Prepare)
            )),
            ("LaneBlockPrepareVote", Some(5), Some(7))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockVote(
                sample_lane_block_vote(Phase::Commit)
            )),
            ("LaneBlockVote", Some(5), Some(7))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockQc(
                sample_lane_block_qc(Phase::Prepare)
            )),
            ("LaneBlockPrepareCert", Some(5), Some(7))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&BlockMessage::LaneBlockQc(
                sample_lane_block_qc(Phase::Commit)
            )),
            ("LaneBlockCert", Some(5), Some(7))
        );
    }

    #[test]
    fn block_message_meta_reports_v2_round_when_available() {
        assert_eq!(
            NetworkRelayShared::block_message_meta(&v2_vote_block_message()),
            ("SumeragiV2PrepareVote", Some(5), Some(7))
        );
        assert_eq!(
            NetworkRelayShared::block_message_meta(&v2_payload_chunk_block_message()),
            ("SumeragiV2PayloadChunk", None, None)
        );
    }

    #[test]
    fn autonomous_lane_messages_use_expected_ingress_buckets() {
        let payload = sumeragi_msg(BlockMessage::LaneExecutablePayload(
            sample_lane_executable_payload(),
        ));
        let new_view_vote = sumeragi_msg(BlockMessage::LaneBlockNewViewVote(
            sample_lane_block_new_view_vote(),
        ));
        let new_view_certificate = sumeragi_msg(BlockMessage::LaneBlockNewViewCertificate(
            sample_lane_block_new_view_certificate(),
        ));
        let drain_vote = lane_drain_vote_msg();

        assert_eq!(
            ConsensusIngressLimiter::ingress_policy(&payload).rate_class,
            Some(IngressRateClass::Critical)
        );
        assert_eq!(
            ConsensusIngressLimiter::ingress_policy(&new_view_vote).rate_class,
            Some(IngressRateClass::Critical)
        );
        assert_eq!(
            ConsensusIngressLimiter::ingress_policy(&new_view_certificate).rate_class,
            Some(IngressRateClass::Critical)
        );
        assert_eq!(
            ConsensusIngressLimiter::ingress_policy(&drain_vote).rate_class,
            Some(IngressRateClass::Critical)
        );
    }

    #[test]
    fn certified_merge_sidecar_close_is_limited_but_responder_controls_are_critical() {
        let (close, close_ack, generation_hint) = certified_merge_sidecar_control_messages();

        let close_policy = ConsensusIngressLimiter::ingress_policy(&close);
        assert_eq!(close_policy.rate_class, Some(IngressRateClass::Limited));
        assert!(close_policy.apply_penalty);

        for message in [&close_ack, &generation_hint] {
            let policy = ConsensusIngressLimiter::ingress_policy(message);
            assert_eq!(policy.rate_class, Some(IngressRateClass::Critical));
            assert!(!policy.apply_penalty);
        }
    }

    #[test]
    fn certified_merge_sidecar_messages_preserve_ingress_reply_route() {
        use iroha_core::merge_sidecar::{
            CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
            CertifiedMergeSidecarMessage, CertifiedMergeSidecarRequestV1,
            CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
            CertifiedMergeSidecarStreamEpochV1,
        };
        use iroha_p2p::network::NetworkReplyRouteTestFixture;

        let (close, close_ack, generation_hint) = certified_merge_sidecar_control_messages();
        let into_sidecar = |message| {
            let iroha_core::NetworkMessage::CertifiedMergeSidecar(message) = message else {
                panic!("fixture must contain a certified merge-sidecar message");
            };
            std::sync::Arc::unwrap_or_clone(message)
        };
        let close = into_sidecar(close);
        let close_ack = into_sidecar(close_ack);
        let generation_hint = into_sidecar(generation_hint);

        let requester = PeerId::new(KeyPair::random().public_key().clone());
        let responder = PeerId::new(KeyPair::random().public_key().clone());
        let stream_epoch = CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("sidecar stream epoch is non-zero"),
        );
        let entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"sidecar route entry"));
        let reference_digest = Hash::new(b"sidecar route reference");
        let mut request = CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch,
            semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(
                NonZeroU64::new(1).expect("sidecar semantic sequence is non-zero"),
            ),
            closed_through: 0,
            request_id: Hash::prehashed([0; Hash::LENGTH]),
            entry_hash,
            encoded_len: 1,
            epoch_id: 1,
            reference_digest,
            requester: requester.clone(),
            responder: responder.clone(),
        };
        request.request_id = request.canonical_request_id();
        let chunk = CertifiedMergeSidecarChunkV1 {
            version: request.version,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            semantic_sequence: request.semantic_sequence,
            request_id: request.request_id,
            entry_hash: request.entry_hash,
            encoded_len: request.encoded_len,
            epoch_id: request.epoch_id,
            reference_digest: request.reference_digest,
            requester,
            responder,
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![0xA5],
        };

        let authenticated_via = PeerId::new(KeyPair::random().public_key().clone());
        let semantic_sender = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(authenticated_via);
        for message in [
            CertifiedMergeSidecarMessage::Request(request),
            close,
            close_ack,
            generation_hint,
            CertifiedMergeSidecarMessage::Chunk(chunk),
        ] {
            let route = routes.mint(semantic_sender.clone());
            let retained =
                super::certified_merge_sidecar_ingress_reply_route(&message, route.clone());
            assert!(retained.same_delivery(&route));
        }
    }

    #[test]
    fn lane_drain_vote_cannot_bypass_critical_rate_or_wire_size_limits() {
        let peer = sample_peer();
        let vote = lane_drain_vote_msg();
        let encoded_len = norito::to_bytes(&vote)
            .expect("encode lane-drain vote")
            .len();
        assert!(encoded_len <= MAX_LANE_DRAIN_VOTE_WIRE_BYTES);

        let mut unconfigured = ConsensusIngressLimiter::new(
            None,
            None,
            None,
            None,
            None,
            None,
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );
        assert_eq!(
            unconfigured.should_drop(&peer, &vote, MAX_LANE_DRAIN_VOTE_WIRE_BYTES + 1),
            Some(ConsensusIngressDropReason::Bytes),
            "the hard cap must apply even when configurable byte buckets are disabled"
        );

        let mut rate_limited = ConsensusIngressLimiter::new(
            None,
            None,
            None,
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );
        assert_eq!(rate_limited.should_drop(&peer, &vote, encoded_len), None);
        assert_eq!(
            rate_limited.should_drop(&peer, &vote, encoded_len),
            Some(ConsensusIngressDropReason::Rate)
        );
    }

    #[test]
    fn consensus_ingress_critical_rate_limit_drops_burst() {
        let peer = sample_peer();
        let vote = v2_vote_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            None,
            None,
            None,
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            PenaltyConfig {
                threshold: 1,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(10),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &vote, 1), None);
        assert_eq!(
            limiter.should_drop(&peer, &vote, 1),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(
            limiter.should_drop(&peer, &vote, 1),
            Some(ConsensusIngressDropReason::Rate)
        );
    }

    #[test]
    fn consensus_ingress_bulk_messages_use_bulk_bucket() {
        fn assert_bulk(peer: &Peer, msg: &iroha_core::NetworkMessage) {
            let standard = limited_msg();
            let mut limiter = ConsensusIngressLimiter::new(
                Some(BucketConfig {
                    rate_per_sec: nz_u32(1),
                    burst: nz_u32(1),
                }),
                None,
                Some(BucketConfig {
                    rate_per_sec: nz_u32(2),
                    burst: nz_u32(2),
                }),
                None,
                None,
                None,
                PenaltyConfig {
                    threshold: 0,
                    window: Duration::from_secs(1),
                    cooldown: Duration::from_secs(1),
                },
            );

            assert_eq!(limiter.should_drop(peer, &standard, 1), None);
            assert_eq!(
                limiter.should_drop(peer, &standard, 1),
                Some(ConsensusIngressDropReason::Rate)
            );
            assert_eq!(limiter.should_drop(peer, msg, 1), None);
            assert_eq!(limiter.should_drop(peer, msg, 1), None);
            assert_eq!(
                limiter.should_drop(peer, msg, 1),
                Some(ConsensusIngressDropReason::Rate)
            );
        }

        let peer = sample_peer();
        assert_bulk(&peer, &v2_certified_body_response_msg());
    }

    #[test]
    fn consensus_ingress_bytes_limit_drops_oversize() {
        let peer = sample_peer();
        let msg = limited_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(10),
                burst: nz_u32(10),
            }),
            None,
            None,
            None,
            None,
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );

        assert_eq!(
            limiter.should_drop(&peer, &msg, 20),
            Some(ConsensusIngressDropReason::Bytes)
        );
        assert_eq!(limiter.should_drop(&peer, &msg, 5), None);
    }

    #[test]
    fn low_priority_ingress_rate_limit_drops_burst() {
        let peer = sample_peer();
        let mut limiter = LowPriorityIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
        );

        assert_eq!(limiter.should_drop(&peer, 32), None);
        assert_eq!(
            limiter.should_drop(&peer, 32),
            Some(LowPriorityIngressDropReason::Rate)
        );
    }

    #[test]
    fn low_priority_ingress_bytes_limit_drops_oversize() {
        let peer = sample_peer();
        let mut limiter = LowPriorityIngressLimiter::new(
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
        );

        assert_eq!(
            limiter.should_drop(&peer, 2),
            Some(LowPriorityIngressDropReason::Bytes)
        );
        assert_eq!(limiter.should_drop(&peer, 1), None);
    }

    #[test]
    fn dedicated_subscriber_message_set_includes_torii_proxy_frames() {
        let request = torii_proxy_request_msg();
        let response = torii_proxy_response_msg();

        assert!(NetworkRelayShared::is_handled_by_dedicated_subscriber(
            &request
        ));
        assert!(NetworkRelayShared::is_handled_by_dedicated_subscriber(
            &response
        ));
        assert!(!NetworkRelayShared::is_handled_by_dedicated_subscriber(
            &v2_vote_msg()
        ));
    }

    #[test]
    fn consensus_ingress_penalty_skips_critical_messages() {
        let peer = sample_peer();
        let msg = limited_msg();
        let vote = v2_vote_msg();
        let proposal = v2_proposal_msg();
        let chunk = sumeragi_msg(v2_payload_chunk_block_message());
        let request = sumeragi_msg(sumeragi_v2_commit_certificate_request());
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            None,
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(10),
                burst: nz_u32(10),
            }),
            None,
            PenaltyConfig {
                threshold: 1,
                window: Duration::from_secs(5),
                cooldown: Duration::from_secs(30),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &msg, 8), None);
        assert_eq!(
            limiter.should_drop(&peer, &msg, 8),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(limiter.should_drop(&peer, &vote, 8), None);
        assert_eq!(limiter.should_drop(&peer, &proposal, 8), None);
        assert_eq!(limiter.should_drop(&peer, &chunk, 8), None);
        assert_eq!(limiter.should_drop(&peer, &request, 8), None);
    }

    #[test]
    fn consensus_ingress_penalty_skips_bulk_messages() {
        let peer = sample_peer();
        let bulk = v2_certified_body_response_msg();
        let standard = limited_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            Some(BucketConfig {
                rate_per_sec: nz_u32(10),
                burst: nz_u32(10),
            }),
            None,
            PenaltyConfig {
                threshold: 1,
                window: Duration::from_secs(5),
                cooldown: Duration::from_secs(30),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &bulk, 8), None);
        assert_eq!(
            limiter.should_drop(&peer, &bulk, 8),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(limiter.should_drop(&peer, &standard, 8), None);
    }

    #[test]
    fn consensus_ingress_critical_fallback_applies_penalty_when_unset() {
        let peer = sample_peer();
        let vote = v2_vote_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            None,
            None,
            None,
            None,
            PenaltyConfig {
                threshold: 1,
                window: Duration::from_secs(5),
                cooldown: Duration::from_secs(30),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &vote, 1), None);
        assert_eq!(
            limiter.should_drop(&peer, &vote, 1),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(
            limiter.should_drop(&peer, &vote, 1),
            Some(ConsensusIngressDropReason::Penalty)
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn consensus_ingress_topic_label_tracks_payload_topics() {
        let payload = v2_certified_body_response_msg();
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&payload),
            Some("ConsensusPayload")
        );

        let chunk = sumeragi_msg(v2_payload_chunk_block_message());
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&chunk),
            Some("ConsensusChunk")
        );

        let vote = v2_vote_msg();
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&vote),
            None
        );

        let drain_vote = lane_drain_vote_msg();
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&drain_vote),
            Some("LaneDrainVote")
        );
    }

    #[test]
    fn consensus_ingress_penalty_suppresses_after_threshold() {
        let peer = sample_peer();
        let msg = limited_msg();
        let mut limiter = ConsensusIngressLimiter::new(
            Some(BucketConfig {
                rate_per_sec: nz_u32(1),
                burst: nz_u32(1),
            }),
            None,
            None,
            None,
            None,
            None,
            PenaltyConfig {
                threshold: 2,
                window: Duration::from_secs(5),
                cooldown: Duration::from_secs(30),
            },
        );

        assert_eq!(limiter.should_drop(&peer, &msg, 8), None);
        assert_eq!(
            limiter.should_drop(&peer, &msg, 8),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(
            limiter.should_drop(&peer, &msg, 8),
            Some(ConsensusIngressDropReason::Rate)
        );
        assert_eq!(
            limiter.should_drop(&peer, &msg, 8),
            Some(ConsensusIngressDropReason::Penalty)
        );
    }
}

fn snapshot_read_error_is_recoverable(error: &TryReadSnapshotError) -> bool {
    snapshot_read_error_is_recoverable_for_bootstrap(error, false)
}

fn snapshot_mode_allows_restore(mode: SnapshotMode) -> bool {
    !matches!(mode, SnapshotMode::Disabled)
}

fn snapshot_failure_allows_empty_state_fallback(
    error: &TryReadSnapshotError,
    provisional_imported_prefix: bool,
) -> bool {
    !provisional_imported_prefix && snapshot_read_error_is_recoverable(error)
}

fn preflight_empty_state_snapshot_fallback(
    kura: &Kura,
    network_id: &NetworkId,
    configured_lane_catalog: &iroha_data_model::nexus::LaneCatalog,
) -> ReportResult<(), StartError> {
    State::preflight_configured_primary_geometry_replay(
        kura,
        network_id,
        configured_lane_catalog,
    )
    .map_err(|error| Report::new(error).change_context(StartError::InitKura))
    .map_err(|report| {
        report.attach(
            "cannot rebuild from an empty state because retained Kura geometry no longer reaches the configured-primary replay floor",
        )
    })
}

fn snapshot_read_error_is_recoverable_for_bootstrap(
    error: &TryReadSnapshotError,
    hard_fork_snapshot_bootstrap: bool,
) -> bool {
    match error {
        TryReadSnapshotError::IO(_, _)
        | TryReadSnapshotError::NetworkIdMismatch { .. }
        | TryReadSnapshotError::ZkConfigInstall(_) => false,
        TryReadSnapshotError::MismatchedHeight { .. } => hard_fork_snapshot_bootstrap,
        _ => true,
    }
}

fn refresh_block_count_after_snapshot_load(
    block_count: &mut iroha_core::kura::BlockCount,
    committed_height: usize,
    kura: &Kura,
) -> Result<(), String> {
    let durable_height = kura
        .exact_durable_blocks_count()
        .map_err(|error| format!("failed to read exact post-snapshot Kura height: {error}"))?;
    let logical_height = kura.blocks_count();
    if durable_height != logical_height {
        return Err(format!(
            "post-snapshot Kura durable height {durable_height} differs from logical height {logical_height}"
        ));
    }
    if committed_height > durable_height {
        return Err(format!(
            "post-snapshot State height {committed_height} exceeds reconciled Kura height {durable_height}"
        ));
    }

    if block_count.0 != durable_height {
        iroha_logger::warn!(
            committed_height,
            previous_block_count = block_count.0,
            durable_height,
            "Replacing startup block count with the exact post-snapshot Kura height"
        );
    }
    block_count.0 = durable_height;
    Ok(())
}

fn authorize_kura_runtime_start(
    provisional_imported_prefix: bool,
    authenticated_snapshot_bootstrap: bool,
) -> Result<(), &'static str> {
    match (
        provisional_imported_prefix,
        authenticated_snapshot_bootstrap,
    ) {
        (true, false) => Err(
            "Kura has a provisional imported prefix but no authenticated snapshot lineage is installed",
        ),
        (false, true) => Err(
            "an authenticated snapshot lineage is installed without a provisional imported prefix",
        ),
        (true, true) | (false, false) => Ok(()),
    }
}

fn apply_state_runtime_config_before_snapshot_auth(state: &mut State, config: &Config) {
    // These fields are process-local execution policy and do not touch Kura-owned geometry.
    // Settlement must be installed before replay because historical
    // top-up/redemption transitions resolve their deterministic execution
    // state and any explicitly referenced proof release through
    // `State::settlement`. This ordering is replay correctness, not an offline
    // capability or node-readiness gate.
    state.set_crypto(config.crypto.clone());
    state.set_pipeline(config.pipeline.clone());
    state.set_oracle(config.oracle.clone());
    state.set_fraud_monitoring(config.fraud_monitoring.clone());
    state.set_gov(config.gov.clone());
    state.content = config.content.clone();
    state.set_settlement(config.settlement.clone());
}

fn apply_state_geometry_config_before_kura_replay(
    state: &mut State,
    config: &Config,
) -> ReportResult<(), StartError> {
    let restored_runtime = state
        .nexus_runtime_restored_from_snapshot()
        .then(|| state.nexus_snapshot());
    if restored_runtime.is_none() {
        state
            .prepare_configured_primary_geometry_anchor(&config.nexus.configured_lane_catalog)
            .map_err(|err| Report::new(err).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach("failed to anchor authenticated primary lane geometry at startup")
            })?;
        state
            .restore_kura_lane_segments_before_startup_replay()
            .map_err(|err| Report::new(err).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach("failed to restore primary geometry before startup replay")
            })?;
    } else {
        state
            .prepare_restored_configured_primary_geometry_anchor(
                &config.nexus.configured_lane_catalog,
            )
            .map_err(|err| Report::new(err).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach(
                    "failed to anchor snapshot-authenticated primary lane geometry at startup",
                )
            })?;
        state
            .restore_kura_lane_segments_from_nexus()
            .map_err(|err| Report::new(err).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach("failed to restore snapshot Nexus lane storage at startup")
            })?;
    }
    let nexus = nexus_config_for_startup_replay(config.nexus.clone(), restored_runtime.as_ref());
    state
        .set_nexus_from_config(nexus)
        .map_err(|err| Report::new(err).change_context(StartError::InitKura))
        .map_err(|report| {
            report.attach("failed to apply Nexus lane catalog/lifecycle at startup")
        })?;
    Ok(())
}

fn install_zk_config_before_kura_replay(
    state: &mut State,
    config: &Config,
) -> ReportResult<(), StartError> {
    state
        .set_zk(config.zk.clone())
        .map_err(|error| Report::new(error).change_context(StartError::InitKura))
}

fn nexus_config_for_startup_replay(
    mut configured: iroha_config::parameters::actual::Nexus,
    restored: Option<&iroha_config::parameters::actual::Nexus>,
) -> iroha_config::parameters::actual::Nexus {
    let Some(restored) = restored else {
        return configured;
    };
    // A snapshot is a committed WSV checkpoint. Preserve its effective lane
    // topology and autoscale cooldown while refreshing all static policy knobs
    // from local configuration. Silently replacing either stateful value here
    // can recreate a retired lane or permit an immediate duplicate transition.
    configured.lane_catalog = restored.lane_catalog.clone();
    configured.lane_config = restored.lane_config.clone();
    configured.autoscale.last_transition_height = restored.autoscale.last_transition_height;
    configured
}

/// Return the effective post-replay Nexus configuration used by runtime
/// admission, routing, and manifest surfaces.
///
/// Replay can commit manual or autoscale lane lifecycle transitions, so the
/// process configuration is no longer authoritative at this boundary.
fn nexus_for_runtime_surfaces(state: &State) -> iroha_config::parameters::actual::Nexus {
    state.nexus_snapshot()
}

/// Freeze the exact manifest source snapshot used while reconstructing State from Kura.
///
/// Replay executes ordinary transaction admission, so its registry must cover the same active
/// catalog as the configured Nexus geometry. Loading this snapshot before replay also makes later
/// catalog rebinding independent of filesystem changes during startup.
fn freeze_lane_manifests_for_startup_replay(
    nexus: &iroha_config::parameters::actual::Nexus,
) -> Result<LaneManifestRegistryHandle, GovernanceGuardError> {
    let registry = if nexus.enabled {
        LaneManifestRegistry::from_config(&nexus.lane_catalog, &nexus.governance, &nexus.registry)
    } else {
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance)
    };
    registry.validate_active_coverage()?;
    Ok(Arc::new(registry))
}

/// Rebind the frozen startup sources to the effective catalog produced by replay.
fn rebind_frozen_lane_manifests_after_startup_replay(
    frozen: &LaneManifestRegistryHandle,
    nexus: &iroha_config::parameters::actual::Nexus,
) -> Result<LaneManifestRegistryHandle, GovernanceGuardError> {
    let rebound = frozen.rebind(&nexus.lane_catalog, &nexus.governance);
    rebound.validate_active_coverage()?;
    Ok(Arc::new(rebound))
}

#[cfg(test)]
mod snapshot_read_error_tests {
    use super::*;
    use std::num::NonZeroUsize;

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;

    fn dummy_block_hash(byte: u8) -> HashOf<BlockHeader> {
        let mut bytes = [0u8; Hash::LENGTH];
        bytes[0] = byte;
        HashOf::from_untyped_unchecked(Hash::prehashed(bytes))
    }

    #[test]
    fn snapshot_read_error_classifies_fatal_errors() {
        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::NotFound
        ));

        let io = std::io::Error::other("boom");
        assert!(!snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::IO(io, std::path::PathBuf::from("snapshot.data"))
        ));

        assert!(!snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::NetworkIdMismatch {
                expected: NetworkId::from_genesis_hash(dummy_block_hash(1)),
                actual: NetworkId::from_genesis_hash(dummy_block_hash(2)),
            }
        ));

        let incompatible_zk = TryReadSnapshotError::ZkConfigInstall(
            iroha_core::state::ZkConfigInstallError::InvalidSccpPendingUsage {
                usage: iroha_data_model::bridge::SccpOutboundPendingUsageV1 {
                    message_count: 0,
                    payload_bytes: 1,
                },
            },
        );
        assert!(!snapshot_read_error_is_recoverable_for_bootstrap(
            &incompatible_zk,
            false,
        ));
        assert!(!snapshot_read_error_is_recoverable_for_bootstrap(
            &incompatible_zk,
            true,
        ));
    }

    #[test]
    fn snapshot_integrity_errors_are_recoverable() {
        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::ChecksumMismatch {
                expected: "deadbeef".into(),
                actual: "beadfeed".into(),
            }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::ChecksumMissing(std::path::PathBuf::from("snapshot.sha256"))
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::SignatureMissing(std::path::PathBuf::from("snapshot.sig"))
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::SignatureMalformed("bad sig".into())
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::SignatureInvalid("invalid sig".into())
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleMissing(std::path::PathBuf::from("snapshot.merkle.json"))
        ));

        let json_err = norito::json::from_str::<norito::json::Value>("not json").unwrap_err();
        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::Serialization(json_err)
        ));

        let json_err = norito::json::from_str::<norito::json::Value>("not json").unwrap_err();
        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleMetadata(json_err)
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleMetadataMalformed("bad merkle".into())
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleMismatch {
                expected: "deadbeef".into(),
                actual: "beadfeed".into(),
            }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleChunkSizeMismatch {
                expected: NonZeroUsize::new(1).unwrap(),
                actual: NonZeroUsize::new(2).unwrap(),
            }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleLengthMismatch {
                expected: 10,
                actual: 11,
            }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MerkleProofInvalid {
                chunk: 0,
                reason: "bad proof".into(),
            }
        ));
    }

    #[test]
    fn snapshot_bootstrap_recovery_obeys_height_and_import_policy() {
        let mismatched_height = TryReadSnapshotError::MismatchedHeight {
            snapshot_height: 2,
            kura_height: 1,
        };
        assert!(!snapshot_read_error_is_recoverable_for_bootstrap(
            &mismatched_height,
            false,
        ));
        assert!(snapshot_read_error_is_recoverable_for_bootstrap(
            &mismatched_height,
            true,
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MissingBlock { height: 1 }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MissingSpaceDirectoryManifestSection {
                snapshot_height: 608
            }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MismatchedHash {
                height: 1,
                snapshot_block_hash: dummy_block_hash(1),
                kura_block_hash: dummy_block_hash(2),
            }
        ));
    }

    #[test]
    fn provisional_imported_prefix_makes_every_snapshot_failure_fatal() {
        let failures = vec![
            TryReadSnapshotError::NotFound,
            TryReadSnapshotError::ChecksumMismatch {
                expected: "expected".to_owned(),
                actual: "corrupt".to_owned(),
            },
            TryReadSnapshotError::SignatureInvalid("forged signature".to_owned()),
            TryReadSnapshotError::InvalidSnapshotBootstrap(
                "substituted retained lineage".to_owned(),
            ),
            TryReadSnapshotError::MissingBlock { height: 2 },
        ];
        for failure in &failures {
            assert!(
                !snapshot_failure_allows_empty_state_fallback(failure, true),
                "provisional imported history must never fall back after {failure}"
            );
        }
        assert!(snapshot_failure_allows_empty_state_fallback(
            &TryReadSnapshotError::NotFound,
            false,
        ));
        assert!(snapshot_failure_allows_empty_state_fallback(
            &TryReadSnapshotError::SignatureInvalid("ordinary corrupt snapshot".to_owned()),
            false,
        ));
    }

    #[test]
    fn empty_state_fallback_preflight_propagates_geometry_failure_as_kura_start_error() {
        let kura = Kura::blank_kura_for_testing();
        let error = preflight_empty_state_snapshot_fallback(
            kura.as_ref(),
            &NetworkId::from_genesis_hash(dummy_block_hash(0x33)),
            &iroha_data_model::nexus::LaneCatalog::default(),
        )
        .expect_err("missing authenticated geometry baseline must reject empty-state fallback");

        assert!(matches!(error.current_context(), StartError::InitKura));
        const FALLBACK_CONTEXT: &str = "cannot rebuild from an empty state because retained Kura \
            geometry no longer reaches the configured-primary replay floor";
        assert!(
            error.frames().any(|frame| {
                frame
                    .downcast_ref::<&str>()
                    .is_some_and(|context| *context == FALLBACK_CONTEXT)
            }),
            "fallback rejection must retain its startup-boundary attachment: {error:?}"
        );
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("no authenticated Kura catalog baseline"),
            "fallback rejection must retain the exact geometry cause: {rendered}"
        );
    }

    #[test]
    fn disabled_snapshot_mode_skips_restore() {
        assert!(snapshot_mode_allows_restore(SnapshotMode::ReadWrite));
        assert!(snapshot_mode_allows_restore(SnapshotMode::Readonly));
        assert!(!snapshot_mode_allows_restore(SnapshotMode::Disabled));
    }

    #[test]
    fn refresh_block_count_after_snapshot_load_uses_snapshot_height_when_ahead() {
        let mut block_count = iroha_core::kura::BlockCount(0);
        let kura = Kura::blank_kura_for_testing();
        let hashes = [dummy_block_hash(1), dummy_block_hash(2)];
        kura.extend_hash_only_suffix_from_verified_snapshot(&hashes)
            .expect("publish reconciled snapshot hashes");

        refresh_block_count_after_snapshot_load(&mut block_count, 2, kura.as_ref())
            .expect("refresh exact reconciled height");

        assert_eq!(block_count.0, 2);
    }

    #[test]
    fn refresh_block_count_after_snapshot_load_uses_exact_higher_kura_height() {
        let mut block_count = iroha_core::kura::BlockCount(9);
        let kura = Kura::blank_kura_for_testing();
        let hashes = (1_u8..=5).map(dummy_block_hash).collect::<Vec<_>>();
        kura.extend_hash_only_suffix_from_verified_snapshot(&hashes)
            .expect("publish reconciled Kura suffix");

        refresh_block_count_after_snapshot_load(&mut block_count, 2, kura.as_ref())
            .expect("replace stale count with exact durable Kura height");

        assert_eq!(block_count.0, 5);
    }

    #[test]
    fn refresh_block_count_rejects_state_ahead_of_reconciled_kura() {
        let mut block_count = iroha_core::kura::BlockCount(1);
        let kura = Kura::blank_kura_for_testing();
        kura.extend_hash_only_suffix_from_verified_snapshot(&[dummy_block_hash(1)])
            .expect("publish one durable hash");

        assert!(
            refresh_block_count_after_snapshot_load(&mut block_count, 2, kura.as_ref()).is_err()
        );
        assert_eq!(block_count.0, 1, "failed refresh preserves the prior count");
    }

    #[test]
    fn audited_kura_runtime_and_height_refresh_require_authenticated_snapshot_root() {
        let mut block_count = iroha_core::kura::BlockCount(0);
        let kura = Kura::blank_kura_for_testing();
        let hashes = (1_u8..=7).map(dummy_block_hash).collect::<Vec<_>>();
        kura.extend_hash_only_suffix_from_verified_snapshot(&hashes)
            .expect("publish reconciled snapshot hashes");
        assert!(authorize_kura_runtime_start(true, false).is_err());
        assert_eq!(block_count.0, 0, "failed auth cannot promote Kura height");
        authorize_kura_runtime_start(true, true)
            .expect("authenticated snapshot authorizes Kura runtime start");
        refresh_block_count_after_snapshot_load(&mut block_count, 7, kura.as_ref())
            .expect("authenticated exact height refresh");
        assert_eq!(block_count.0, 7);

        authorize_kura_runtime_start(false, false).expect("ordinary startup has no lineage");
        assert!(authorize_kura_runtime_start(false, true).is_err());

        // A normally signed carried lineage is valid after the one-time digest policy is disabled;
        // authorization follows the provisional Kura marker, not the current policy toggle.
        authorize_kura_runtime_start(true, true)
            .expect("policy-disabled carried lineage authenticates a provisional prefix");
    }

    #[test]
    fn provisional_imported_prefix_does_not_require_a_stored_genesis_body() {
        let kura = Kura::blank_kura_for_testing();
        let genesis_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA7; Hash::LENGTH]));
        kura.extend_hash_only_suffix_from_verified_snapshot(&[genesis_hash])
            .expect("publish hash-only fixture");
        let count = iroha_core::kura::BlockCount(1);

        assert!(
            read_stored_genesis_block(kura.as_ref(), count, true)
                .expect("provisional prefix uses its signed snapshot trust source")
                .is_none()
        );
        assert!(
            read_stored_genesis_block(kura.as_ref(), count, false).is_err(),
            "ordinary startup must not silently accept a missing genesis body"
        );
    }

    #[test]
    fn startup_nexus_merge_preserves_snapshot_topology_and_cooldown_only() {
        use std::num::{NonZeroU32, NonZeroU64};

        use iroha_config::parameters::actual::LaneConfig as RuntimeLaneConfig;
        use iroha_data_model::nexus::{LaneCatalog, LaneConfig, LaneId};

        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane namespace"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "snapshot-lane".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("snapshot catalog");
        let mut restored = iroha_config::parameters::actual::Nexus {
            lane_config: RuntimeLaneConfig::from_catalog(&catalog),
            lane_catalog: catalog.clone(),
            configured_lane_catalog: catalog.clone(),
            ..Default::default()
        };
        restored.autoscale.last_transition_height = 17;
        restored.autoscale.target_block_ms = NonZeroU64::new(777).expect("nonzero target");

        let mut configured = iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        };
        configured.autoscale.target_block_ms = NonZeroU64::new(321).expect("nonzero target");
        let merged = nexus_config_for_startup_replay(configured, Some(&restored));

        assert!(merged.enabled);
        assert_eq!(merged.lane_catalog, catalog);
        assert_eq!(merged.lane_config.entries().len(), 2);
        assert_eq!(merged.autoscale.last_transition_height, 17);
        assert_eq!(merged.autoscale.target_block_ms.get(), 321);
        assert_eq!(
            merged.configured_lane_catalog,
            iroha_data_model::nexus::LaneCatalog::default(),
            "snapshot topology must not replace the process-configured baseline"
        );
    }

    #[test]
    fn startup_nexus_merge_uses_config_for_legacy_snapshot() {
        let mut configured = iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        };
        configured.autoscale.last_transition_height = 9;

        let merged = nexus_config_for_startup_replay(configured, None);

        assert!(merged.enabled);
        assert_eq!(
            merged.lane_catalog,
            iroha_data_model::nexus::LaneCatalog::default()
        );
        assert_eq!(merged.autoscale.last_transition_height, 9);
    }

    #[test]
    fn runtime_surfaces_use_post_replay_lane_catalog() {
        use std::num::NonZeroU32;

        use iroha_data_model::nexus::{LaneCatalog, LaneConfig, LaneId};

        let configured = iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        };
        let mut replayed = configured.clone();
        replayed.lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "replayed-runtime-lane".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("valid replayed lane catalog");

        let mut state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state
            .set_nexus(replayed)
            .expect("install replayed Nexus topology");

        let runtime = nexus_for_runtime_surfaces(&state);
        assert_ne!(runtime.lane_catalog, configured.lane_catalog);
        assert!(
            runtime
                .lane_catalog
                .lanes()
                .iter()
                .any(|lane| lane.id == LaneId::new(1)),
            "runtime queue and manifest setup must see the replayed lane"
        );
    }

    #[test]
    fn startup_replay_installs_default_lane_manifest_snapshot_before_validation() {
        use iroha_core::governance::manifest::{GovernanceGuardReason, LaneManifestRegistry};
        use iroha_data_model::nexus::LaneId;

        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state
            .lane_manifests
            .read()
            .ensure_lane_ready(LaneId::SINGLE)
            .expect("the test constructor binds the default lane manifest");
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::empty()));
        let absent = state
            .lane_manifests
            .read()
            .ensure_lane_ready(LaneId::SINGLE)
            .expect_err("the modeled pre-replay snapshot has no bound manifest catalog");
        assert_eq!(absent.reason(), GovernanceGuardReason::UnknownLane);

        let nexus = iroha_config::parameters::actual::Nexus::default();
        let frozen = freeze_lane_manifests_for_startup_replay(&nexus)
            .expect("default lane is ready in the frozen startup registry");
        state.install_lane_manifests(&frozen);

        state
            .lane_manifests
            .read()
            .ensure_lane_ready(LaneId::SINGLE)
            .expect("atomic replay sees the configured default lane");
    }

    #[test]
    fn startup_replay_manifest_freeze_fails_closed_for_missing_governance_source() {
        use std::num::NonZeroU32;

        use iroha_core::governance::manifest::GovernanceGuardReason;
        use iroha_data_model::nexus::{LaneCatalog, LaneConfig};

        let governed_lane = LaneConfig {
            governance: Some("parliament".to_owned()),
            ..LaneConfig::default()
        };
        let catalog = LaneCatalog::new(
            NonZeroU32::new(1).expect("non-zero lane namespace"),
            vec![governed_lane],
        )
        .expect("single governed lane catalog");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        };
        nexus.lane_catalog = catalog.clone();
        nexus.configured_lane_catalog = catalog;

        let error = freeze_lane_manifests_for_startup_replay(&nexus)
            .expect_err("governed replay lane without a frozen manifest must reject startup");

        assert_eq!(error.reason(), GovernanceGuardReason::MissingManifest);
    }

    #[test]
    fn post_replay_manifest_rebind_does_not_rescan_changed_sources() {
        let manifest_dir = tempfile::tempdir().expect("create manifest source directory");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        };
        nexus.registry.manifest_directory = Some(manifest_dir.path().to_path_buf());
        let frozen = freeze_lane_manifests_for_startup_replay(&nexus)
            .expect("ungoverned default lane is ready without a manifest");
        assert!(!frozen.has_manifest_source_alias("default"));

        std::fs::write(manifest_dir.path().join("default.manifest.json"), b"{}")
            .expect("replace manifest source set after the startup freeze");
        let rebound = rebind_frozen_lane_manifests_after_startup_replay(&frozen, &nexus)
            .expect("frozen source set deterministically rebinds");

        assert!(!rebound.has_manifest_source_alias("default"));
        assert_eq!(
            rebound.consensus_policy_digest(),
            frozen.consensus_policy_digest(),
            "post-replay rebinding must preserve the pre-replay source snapshot"
        );
        let rescanned = LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &nexus.governance,
            &nexus.registry,
        );
        assert!(rescanned.has_manifest_source_alias("default"));
        assert_ne!(
            rescanned.consensus_policy_digest(),
            frozen.consensus_policy_digest(),
            "the adversarial file change would affect a forbidden second scan"
        );
    }
}

fn validate_reputation_runtime_provider_presence(
    runtime_enabled: bool,
    provider_presence: [bool; 4],
) -> Result<(), &'static str> {
    if runtime_enabled {
        match provider_presence {
            [true, true, true, true] => Ok(()),
            [false, _, _, _] => Err(
                "enabled SoraFS reputation runtime requires an injected monotonic journal-checkpoint provider",
            ),
            [true, false, _, _] => Err(
                "enabled SoraFS reputation runtime requires an injected authenticated journal-transaction submitter",
            ),
            [true, true, false, _] => Err(
                "enabled SoraFS reputation runtime requires an injected external threshold signer",
            ),
            [true, true, true, false] => Err(
                "enabled SoraFS reputation runtime requires an injected authenticated Governance DAG adapter",
            ),
        }
    } else if provider_presence == [false; 4] {
        Ok(())
    } else {
        Err("disabled SoraFS reputation runtime rejects unexpected runtime providers")
    }
}

fn validate_governance_dag_service_publisher_binding(
    storage: &iroha_config::parameters::actual::SorafsStorage,
    runtime_signer: Option<&dyn sorafs_node::GovernanceDagRuntimeSigner>,
) -> Result<(), sorafs_node::GovernanceDagServiceError> {
    if !storage.governance_dag_service.enabled {
        return Ok(());
    }

    let producer_public_key_hex = storage
        .governance_dag_publisher_public_key_hex
        .as_deref()
        .ok_or_else(|| {
            sorafs_node::GovernanceDagServiceError::Config(
                "enabled Governance DAG service requires the embedded producer public-key binding"
                    .to_owned(),
            )
        })?;
    let service_public_key_hex = storage
        .governance_dag_service
        .publisher_public_key_hex
        .as_deref()
        .ok_or_else(|| {
            sorafs_node::GovernanceDagServiceError::Config(
                "enabled Governance DAG service requires its publisher public key".to_owned(),
            )
        })?;
    if service_public_key_hex != producer_public_key_hex {
        return Err(sorafs_node::GovernanceDagServiceError::Config(
            "Governance DAG service publisher public key does not match the embedded producer configuration"
                .to_owned(),
        ));
    }

    let runtime_public_key_hex = runtime_signer
        .map(|signer| hex::encode(signer.public_key()))
        .ok_or_else(|| {
            sorafs_node::GovernanceDagServiceError::Config(
                "enabled Governance DAG service requires the embedded producer runtime signer"
                    .to_owned(),
            )
        })?;
    if runtime_public_key_hex != producer_public_key_hex {
        return Err(sorafs_node::GovernanceDagServiceError::Config(
            "Governance DAG service publisher public key does not match the embedded producer runtime signer"
                .to_owned(),
        ));
    }
    Ok(())
}

fn resolve_governance_dag_service_launch(
    storage: &iroha_config::parameters::actual::SorafsStorage,
    runtime_deps: &IrohaRuntimeDeps,
) -> Result<
    Option<(
        iroha_config::parameters::actual::SorafsGovernanceDagServiceView,
        sorafs_node::GovernanceDagServiceRuntimeProviders,
    )>,
    sorafs_node::GovernanceDagServiceError,
> {
    let public_service_provider_presence = [
        runtime_deps
            .sorafs_governance_dag_ipfs_authenticator
            .is_some(),
        runtime_deps
            .sorafs_governance_dag_head_authenticator
            .is_some(),
    ];
    validate_governance_dag_service_publisher_binding(
        storage,
        runtime_deps.sorafs_governance_dag_signer.as_deref(),
    )?;
    if !storage.governance_dag_service.enabled {
        if public_service_provider_presence
            .into_iter()
            .any(|present| present)
        {
            return Err(sorafs_node::GovernanceDagServiceError::Config(
                "disabled Governance DAG service rejects unexpected public-service runtime providers"
                    .to_owned(),
            ));
        }
        return Ok(None);
    }

    let mut providers = sorafs_node::GovernanceDagServiceRuntimeProviders::default();
    if let Some(authenticator) = runtime_deps
        .sorafs_governance_dag_ipfs_authenticator
        .as_ref()
    {
        providers = providers.with_ipfs_authenticator(Arc::clone(authenticator));
    }
    if let Some(authenticator) = runtime_deps
        .sorafs_governance_dag_head_authenticator
        .as_ref()
    {
        providers = providers.with_head_authenticator(Arc::clone(authenticator));
    }
    if let Some(checkpoint_store) = runtime_deps.sorafs_governance_dag_checkpoint_store.as_ref() {
        providers = providers.with_checkpoint_store(Arc::clone(checkpoint_store));
    }
    let view = iroha_config::parameters::actual::SorafsGovernanceDagServiceView {
        source_dir: storage.governance_dag_dir.clone(),
        producer_publisher_peer_id: storage.governance_dag_publisher_peer_id.clone(),
        producer_signer_handle: storage.governance_dag_signer_handle.clone(),
        producer_signer_revision: storage.governance_dag_signer_revision,
        producer_signer_policy_digest: storage.governance_dag_signer_policy_digest,
        producer_publisher_public_key_hex: storage.governance_dag_publisher_public_key_hex.clone(),
        service: storage.governance_dag_service.clone(),
    };
    sorafs_node::validate_governance_dag_service_runtime_providers(&view, &providers)?;
    Ok(Some((view, providers)))
}

fn validate_reputation_archive_presence(
    runtime_enabled: bool,
    archive_present: bool,
) -> Result<(), &'static str> {
    match (runtime_enabled, archive_present) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => Err(
            "enabled SoraFS reputation runtime requires its daemon-owned archive before Sumeragi startup",
        ),
        (false, true) => {
            Err("disabled SoraFS reputation runtime rejects an unexpected Sumeragi archive")
        }
    }
}

fn validate_provider_ingest_archive_presence(
    runtime_enabled: bool,
    archive_present: bool,
) -> Result<(), &'static str> {
    match (runtime_enabled, archive_present) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => Err(
            "enabled SoraFS provider-ingest runtime requires its daemon-owned archive before Sumeragi startup",
        ),
        (false, true) => {
            Err("disabled SoraFS provider-ingest runtime rejects an unexpected finalized archive")
        }
    }
}

fn validate_provider_attestation_journal_activation(
    journal_configured: bool,
) -> Result<(), &'static str> {
    // TODO: Replace this gate with the supervised capture child only after its
    // scanner is sealed to the concrete finalized archive and the local store
    // initialization, durable time, approval-signer, and authenticated
    // inventory boundaries are activation-qualified.
    if journal_configured {
        Err(
            "SoraFS provider-attestation journal capture is not yet activation-qualified; the concrete finalized-archive scanner, bounded store initialization, rollback-resistant time, approval signer, and authenticated inventory must be wired before enabling it",
        )
    } else {
        Ok(())
    }
}

fn validate_sorafs_native_signer_role_presence(
    role: &'static str,
    required: bool,
    configured: bool,
    injected: bool,
) -> Result<(), String> {
    match (required, configured) {
        (true, false) => {
            return Err(format!(
                "required SoraFS {role} signer role is missing its configured binding for storage-enabled durable drain or role generation"
            ));
        }
        (false, true) => {
            return Err(format!(
                "inactive SoraFS {role} signer role rejects a configured binding without storage-enabled durable drain or role generation"
            ));
        }
        _ => {}
    }
    match (configured, injected) {
        (true, false) => Err(format!(
            "configured SoraFS {role} signer role is missing its runtime provider"
        )),
        (false, true) => Err(format!(
            "unconfigured SoraFS {role} signer role rejects an injected runtime provider"
        )),
        _ => Ok(()),
    }
}

const fn sorafs_native_signer_role_required(
    storage_enabled: bool,
    role_generation_enabled: bool,
) -> bool {
    storage_enabled || role_generation_enabled
}

fn validate_sorafs_native_signer_provider_presence(
    config: &Config,
    runtime_deps: &IrohaRuntimeDeps,
) -> Result<(), String> {
    let configured = &config.torii.sorafs_storage.native_transaction_signers;
    validate_sorafs_native_signer_role_presence(
        "proof_outcome",
        sorafs_native_signer_role_required(config.torii.sorafs_storage.enabled, false),
        configured.proof_outcome.is_some(),
        runtime_deps.sorafs_proof_outcome_signer.is_some(),
    )?;
    validate_sorafs_native_signer_role_presence(
        "repair",
        sorafs_native_signer_role_required(
            config.torii.sorafs_storage.enabled,
            config.torii.sorafs_repair.enabled,
        ),
        configured.repair.is_some(),
        runtime_deps.sorafs_repair_transaction_signer.is_some(),
    )?;
    validate_sorafs_native_signer_role_presence(
        "reserve",
        sorafs_native_signer_role_required(
            config.torii.sorafs_storage.enabled,
            config.torii.sorafs_storage.reserve_worker.enabled,
        ),
        configured.reserve.is_some(),
        runtime_deps.sorafs_reserve_transaction_signer.is_some(),
    )?;
    validate_sorafs_native_signer_role_presence(
        "orderbook",
        sorafs_native_signer_role_required(
            config.torii.sorafs_storage.enabled,
            config.torii.sorafs_storage.orderbook_worker.enabled,
        ),
        configured.orderbook.is_some(),
        runtime_deps.sorafs_orderbook_transaction_signer.is_some(),
    )
}

fn qualify_soracloud_runtime_signer_for_startup(
    production_mode: bool,
    configured: Option<&iroha_config::parameters::actual::SoracloudRuntimeMutationSignerBinding>,
    runtime_deps: &mut IrohaRuntimeDeps,
) -> Result<(), &'static str> {
    let injected = runtime_deps.soracloud_runtime_mutation_signer.take();
    match (configured, injected) {
        (None, None) if production_mode => {
            Err("production mode requires an exact configured signer binding")
        }
        (None, None) => Ok(()),
        (None, Some(_)) => Err("an unrequested signer provider was injected"),
        (Some(_), None) => Err("the configured signer provider is missing"),
        (Some(configured), Some(provider)) => {
            let binding =
                soracloud_runtime_signer::SoracloudRuntimeSignerBindingV1::try_from_config(
                    configured,
                )
                .map_err(|_| "configured signer binding is invalid")?;
            runtime_deps.soracloud_runtime_mutation_signer = Some(
                soracloud_runtime_signer::qualify_soracloud_runtime_mutation_signer_v1(
                    binding, provider,
                )
                .map_err(
                    |_| "injected signer is substituted, stale, revoked, test-only, or unavailable",
                )?,
            );
            Ok(())
        }
    }
}

impl Iroha {
    /// Starts Iroha with all its subsystems.
    ///
    /// Returns iroha itself and a future of system shutdown.
    ///
    /// # Errors
    /// - Reading telemetry configs
    /// - Telemetry setup
    /// - Initialization of the Sumeragi v2 reducer via [`SumeragiStartArgs`] and [`Kura`]
    pub async fn start(
        config: Config,
        genesis: Option<GenesisBlock>,
        logger: LoggerHandle,
        shutdown_signal: ShutdownSignal,
    ) -> ReportResult<
        (
            Self,
            impl Future<Output = iroha_futures::supervisor::Result<()>>,
        ),
        StartError,
    > {
        Box::pin(Self::start_with_runtime_deps(
            config,
            genesis,
            logger,
            shutdown_signal,
            IrohaRuntimeDeps::default(),
            None,
        ))
        .await
    }

    /// Starts Iroha with deployment-owned, runtime-only service dependencies.
    ///
    /// The standard daemon entry point does not adapt the validator node key
    /// into `SoraFS` proof-outcome, repair, reserve/rent, orderbook, moderation,
    /// or Soracloud mutation/provenance authority roles. Those signers, moderation durable
    /// handoffs, and all hedging/billing query, verification, HSM,
    /// publication, acknowledgement, and witness adapters must be supplied by
    /// an injecting launcher; enabling the dependent path without one fails
    /// closed. A private Musubi publication runner is likewise assembled only
    /// by an explicitly injected late-bound factory and joins this node's supervisor.
    /// The reputation queue submitter is a separately injected deployment
    /// boundary. The Torii proxy bridge signer remains a separate native node
    /// role. Any configured signed Governance DAG producer requires a sealed
    /// monotonic checkpoint store; enabling its public service additionally
    /// requires separately qualified IPFS/head authenticators. The exact
    /// historical reputation query is daemon-owned and backed only by the
    /// configured Kura-authenticated archive.
    ///
    /// # Errors
    /// - Reading telemetry configs
    /// - Telemetry setup
    /// - Initialization of the Sumeragi v2 reducer via [`SumeragiStartArgs`] and [`Kura`]
    #[allow(clippy::too_many_lines)]
    #[iroha_logger::log(name = "start", skip_all)] // This is actually easier to understand as a linear sequence of init statements.
    pub(crate) async fn start_with_runtime_deps(
        mut config: Config,
        genesis: Option<GenesisBlock>,
        logger: LoggerHandle,
        shutdown_signal: ShutdownSignal,
        mut runtime_deps: IrohaRuntimeDeps,
        musubi_publication_factory: Option<
            Box<dyn musubi_publication_service::MusubiPublicationPrivateServiceFactoryV1>,
        >,
    ) -> ReportResult<
        (
            Self,
            impl Future<Output = iroha_futures::supervisor::Result<()>>,
        ),
        StartError,
    > {
        validate_provider_attestation_journal_activation(
            config
                .torii
                .sorafs_storage
                .provider_ingest_runtime
                .as_ref()
                .is_some_and(|runtime| runtime.provider_attestation_journal.is_some()),
        )
        .map_err(|message| Report::new(StartError::StartTorii).attach(message))?;
        validate_sorafs_native_signer_provider_presence(&config, &runtime_deps).map_err(
            |error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed SoraFS native signer startup presence preflight: {error}"
                ))
            },
        )?;
        qualify_soracloud_runtime_signer_for_startup(
            config.soracloud_runtime.production_mode,
            config.soracloud_runtime.submission.signer.as_ref(),
            &mut runtime_deps,
        )
        .map_err(|error| {
            Report::new(StartError::StartTorii).attach(format!(
                "failed to qualify Soracloud runtime mutation signer: {error}"
            ))
        })?;
        let sorafs_governance_dag_service_launch =
            resolve_governance_dag_service_launch(&config.torii.sorafs_storage, &runtime_deps)
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to qualify Governance DAG service runtime providers: {error}"
                    ))
                })?;
        validate_reputation_runtime_provider_presence(
            config.torii.sorafs_storage.reputation_runtime.is_some(),
            [
                runtime_deps
                    .sorafs_reputation_journal_checkpoint_provider
                    .is_some(),
                runtime_deps
                    .sorafs_reputation_journal_transaction_submitter
                    .is_some(),
                runtime_deps.sorafs_reputation_threshold_signer.is_some(),
                runtime_deps.sorafs_reputation_governance_dag.is_some(),
            ],
        )
        .map_err(|message| Report::new(StartError::StartTorii).attach(message))?;
        let sorafs_provider_ingest_preflight = match config
            .torii
            .sorafs_storage
            .provider_ingest_runtime
            .as_ref()
        {
            Some(provider_ingest_config) => {
                let provider_id = config
                    .torii
                    .sorafs_storage
                    .provider_id
                    .ok_or_else(|| {
                        Report::new(StartError::StartTorii).attach(
                            "enabled SoraFS provider-ingest runtime requires the exact configured storage provider identity",
                        )
                    })?;
                let authenticated_source = runtime_deps
                    .sorafs_provider_ingest_authenticated_source
                    .clone()
                    .ok_or_else(|| {
                        Report::new(StartError::StartTorii).attach(
                            "enabled SoraFS provider-ingest runtime requires an injected authenticated governed source-fetch adapter",
                        )
                    })?;
                let signer_resolver = runtime_deps
                    .sorafs_provider_ingest_signer_resolver
                    .clone()
                    .ok_or_else(|| {
                        Report::new(StartError::StartTorii).attach(
                            "enabled SoraFS provider-ingest runtime requires an injected governance-aware HSM/KMS signer resolver",
                        )
                    })?;
                let checkpoint_runtime = runtime_deps
                    .sorafs_provider_ingest_checkpoint_runtime
                    .clone()
                    .ok_or_else(|| {
                        Report::new(StartError::StartTorii).attach(
                            "enabled SoraFS provider-ingest runtime requires an injected sealed monotonic checkpoint provider",
                        )
                    })?;
                if provider_ingest_config
                    .finalized_archive
                    .retention_authority
                    .is_some()
                    != runtime_deps
                        .sorafs_provider_ingest_retention_authority
                        .is_some()
                {
                    return Err(Report::new(StartError::StartTorii).attach(
                        "SoraFS provider-ingest finalized-archive retention requires exact configured/injected sealed authority presence",
                    ));
                }
                Some(
                    sorafs_provider_ingest_runtime::preflight_runtime_adapters(
                        provider_ingest_config,
                        provider_id,
                        sorafs_provider_ingest_runtime::ProviderIngestRuntimeAdaptersV1::new(
                            authenticated_source,
                            signer_resolver,
                        ),
                        checkpoint_runtime,
                    )
                    .await
                    .map_err(|error| {
                        Report::new(StartError::StartTorii).attach(format!(
                            "failed state-free SoraFS provider-ingest runtime adapter preflight: {error:#}"
                        ))
                    })?,
                )
            }
            None => {
                if runtime_deps
                    .sorafs_provider_ingest_authenticated_source
                    .is_some()
                    || runtime_deps
                        .sorafs_provider_ingest_signer_resolver
                        .is_some()
                    || runtime_deps
                        .sorafs_provider_ingest_checkpoint_runtime
                        .is_some()
                    || runtime_deps
                        .sorafs_provider_ingest_retention_authority
                        .is_some()
                {
                    return Err(Report::new(StartError::StartTorii).attach(
                        "disabled SoraFS provider-ingest runtime rejects unexpected runtime providers",
                    ));
                }
                None
            }
        };
        let mut supervisor = Supervisor::new();
        let startup_trace_started_at = Instant::now();
        log_startup_trace("irohad.start.enter", startup_trace_started_at);
        let sorafs_pop_credentials = sorafs_pop_runtime::build(
            config.torii.sorafs_storage.pop_credentials.as_ref(),
            runtime_deps.sorafs_pop_credential_provider_registry.clone(),
        )
        .map_err(|error| {
            Report::new(StartError::StartTorii).attach(format!(
                "failed to initialise config-bound SoraFS PoP runtime: {error}"
            ))
        })?;

        // Log detailed backtraces if a lock-order deadlock occurs so we can
        // diagnose stalls during long-running scenarios (e.g., integration tests).
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(Duration::from_secs(10));
                let deadlocks = deadlock::check_deadlock();
                if deadlocks.is_empty() {
                    continue;
                }
                for (i, threads) in deadlocks.iter().enumerate() {
                    iroha_logger::error!(
                        deadlock_index = i,
                        thread_count = threads.len(),
                        "deadlock detected"
                    );
                    for thr in threads {
                        iroha_logger::error!(
                            deadlock_index = i,
                            thread = ?thr.thread_id(),
                            backtrace = ?thr.backtrace(),
                            "deadlocked thread backtrace"
                        );
                    }
                }
            }
        });

        let (kura, mut block_count) =
            Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
                &config.kura,
                &config.nexus.lane_config,
                &config.nexus.configured_lane_catalog,
                &config.snapshot.bootstrap,
                &config.sumeragi.limits,
            )
            .map_err(|err| {
                let resolved = config.kura.store_dir.resolve_relative_path();
                Report::new(err).attach(format!(
                    "failed to initialize Kura for store_dir {} (raw {})",
                    resolved.display(),
                    config.kura.store_dir.value().display(),
                ))
            })
            .change_context(StartError::InitKura)?;
        kura.bind_local_peer_id(PeerId::new(config.common.key_pair.public_key().clone()))
            .map_err(|error| {
                Report::new(error).attach(
                    "failed to bind the configured node identity to Kura before runtime start",
                )
            })
            .change_context(StartError::InitKura)?;
        let provisional_imported_prefix = kura.provisional_snapshot_bootstrap_pending();
        kura.configure_fastpq_proof_sidecar_limits(&config.zk.fastpq);

        let (live_query_store, child) =
            LiveQueryStore::from_config(config.live_query_store, supervisor.shutdown_signal())
                .start();
        supervisor.monitor(child);

        let telemetry_profile = if config.telemetry_enabled {
            config.telemetry_profile
        } else {
            iroha_config::parameters::actual::TelemetryProfile::Disabled
        };
        let telemetry_capabilities = telemetry_profile.capabilities();

        #[cfg(feature = "telemetry")]
        let (metrics, state_telemetry, streaming_telemetry) = {
            let metrics =
                init_global_metrics_handle(config.dev_telemetry.panic_on_duplicate_metrics);
            let state = StateTelemetry::from_privacy_parameters(
                Arc::clone(&metrics),
                telemetry_capabilities.expensive_metrics_enabled(),
                &config.network.soranet_privacy,
            );
            state.set_nexus_enabled(config.nexus.enabled);
            let streaming = if telemetry_capabilities.metrics_enabled() {
                Some(StreamingTelemetry::new(
                    Arc::clone(&metrics),
                    telemetry_capabilities.metrics_enabled(),
                ))
            } else {
                None
            };
            (metrics, state, streaming)
        };

        let verification_key = config
            .snapshot
            .verification_public_key
            .as_ref()
            .unwrap_or_else(|| config.common.key_pair.public_key());
        let signing_key = config.snapshot.signing_private_key.as_ref().map_or_else(
            || config.common.key_pair.clone(),
            |key| iroha_crypto::KeyPair::from(key.clone()),
        );

        let genesis = load_deferred_normal_startup_genesis(
            provisional_imported_prefix,
            genesis,
            config.genesis.file.as_ref(),
        )?;
        // Resolve the trust source before reading a genesis body from Kura. The on-disk block may
        // satisfy an already resolved exact hash, but it can never choose its own trust anchor.
        let startup_trust_root = StartupTrustRoot::resolve(
            provisional_imported_prefix,
            &config.genesis.public_key,
            config.genesis.expected_hash,
            genesis.as_ref(),
        )?;
        let stored_genesis_block =
            read_stored_genesis_block(kura.as_ref(), block_count, provisional_imported_prefix)?;
        let effective_genesis = stored_genesis_block.as_ref().or(genesis.as_ref());
        let signed_genesis_context = match &startup_trust_root {
            StartupTrustRoot::Genesis(anchor) => {
                let genesis_to_verify = effective_genesis.ok_or_else(|| {
                    Report::new(StartError::InitKura).attach(
                        "normal startup has an exact genesis trust anchor but no local or stored signed genesis body; peer genesis retrieval is not supported",
                    )
                })?;
                anchor.verify(genesis_to_verify)?;
                Some(
                    signed_v2_genesis_context_metadata(genesis_to_verify)
                        .map_err(|error| Report::new(StartError::InitKura).attach(error))?,
                )
            }
            StartupTrustRoot::AuthenticatedSnapshotPending => None,
        };

        let effective_genesis_public_key = config.genesis.public_key.clone();

        let mut loaded_state_from_snapshot = false;
        let snapshot_result = if snapshot_mode_allows_restore(config.snapshot.mode) {
            try_read_snapshot_with_bootstrap_policy(
                config.snapshot.store_dir.resolve_relative_path(),
                &kura,
                || live_query_store.clone(),
                block_count,
                config.snapshot.merkle_chunk_size_bytes,
                config.snapshot.max_payload_bytes,
                config.snapshot.resources,
                verification_key,
                &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                &config.zk,
                &config.snapshot.bootstrap,
                #[cfg(feature = "telemetry")]
                state_telemetry.clone(),
            )
        } else {
            iroha_logger::info!("Snapshot restore is disabled by configuration");
            Err(TryReadSnapshotError::NotFound)
        };
        let mut state = match snapshot_result {
            Ok(state) => {
                iroha_logger::info!(
                    at_height = state.committed_height(),
                    "Successfully loaded the state from a snapshot"
                );
                loaded_state_from_snapshot = true;
                refresh_block_count_after_snapshot_load(
                    &mut block_count,
                    state.committed_height(),
                    kura.as_ref(),
                )
                .map_err(|error| Report::new(StartError::InitKura).attach(error))?;
                state
            }
            Err(TryReadSnapshotError::NotFound) if !provisional_imported_prefix => {
                preflight_empty_state_snapshot_fallback(
                    kura.as_ref(),
                    &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    &config.nexus.configured_lane_catalog,
                )?;
                iroha_logger::info!("Didn't find a state snapshot; creating an empty state");
                let genesis_public_key = effective_genesis_public_key.clone();
                let mut world = World::with(
                    [genesis_domain(genesis_public_key.clone())],
                    [genesis_account(genesis_public_key)],
                    [],
                );
                if let Some(genesis_block) = stored_genesis_block.as_ref().or(genesis.as_ref()) {
                    iroha_core::sns::seed_genesis_alias_bootstrap(
                        &mut world,
                        &genesis_block.0,
                        &config.nexus.dataspace_catalog,
                    );
                }
                State::try_new_with_chain_and_network_id(
                    world,
                    Arc::clone(&kura),
                    live_query_store.clone(),
                    config.common.chain.clone(),
                    NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    #[cfg(feature = "telemetry")]
                    state_telemetry.clone(),
                )
                .map_err(|error| Report::new(error).change_context(StartError::InitKura))?
            }
            Err(error)
                if snapshot_failure_allows_empty_state_fallback(
                    &error,
                    provisional_imported_prefix,
                ) =>
            {
                iroha_logger::warn!(
                    ?error,
                    "Failed to load state snapshot; checking whether Kura can rebuild from an empty state"
                );
                preflight_empty_state_snapshot_fallback(
                    kura.as_ref(),
                    &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    &config.nexus.configured_lane_catalog,
                )?;
                iroha_logger::warn!(
                    "Kura retains the configured-primary replay floor; rebuilding state from blocks"
                );
                let genesis_public_key = effective_genesis_public_key.clone();
                let mut world = World::with(
                    [genesis_domain(genesis_public_key.clone())],
                    [genesis_account(genesis_public_key)],
                    [],
                );
                if let Some(genesis_block) = stored_genesis_block.as_ref().or(genesis.as_ref()) {
                    iroha_core::sns::seed_genesis_alias_bootstrap(
                        &mut world,
                        &genesis_block.0,
                        &config.nexus.dataspace_catalog,
                    );
                }
                State::try_new_with_chain_and_network_id(
                    world,
                    Arc::clone(&kura),
                    live_query_store.clone(),
                    config.common.chain.clone(),
                    NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    #[cfg(feature = "telemetry")]
                    state_telemetry.clone(),
                )
                .map_err(|error| Report::new(error).change_context(StartError::InitKura))?
            }
            Err(error) => {
                return Err(Report::new(error).change_context(StartError::InitKura));
            }
        };
        #[cfg(feature = "telemetry")]
        {
            kura.attach_telemetry(state.telemetry.clone());
        }
        let expected_network_id = NetworkId::from_genesis_hash(config.genesis.expected_hash);
        if state.network_id != expected_network_id {
            return Err(Report::new(StartError::InitKura).attach(format!(
                "restored state network id {} differs from genesis.expected_hash-derived id {}",
                state.network_id, expected_network_id
            )));
        }
        // Thread the display/configuration label into state for legacy VRF prehash binding.
        state.chain_id = config.common.chain.clone();
        install_configured_kagemusha_release_catalog(&mut state, &config)
            .map_err(|error| Report::new(StartError::InitKura).attach(error))?;
        if !loaded_state_from_snapshot {
            // Snapshot candidates install this at their post-decode,
            // pre-reconciliation boundary. Fresh and Kura-rebuilt state has no
            // snapshot boundary, so install it exactly once here before replay.
            install_zk_config_before_kura_replay(&mut state, &config)?;
        }
        apply_state_runtime_config_before_snapshot_auth(&mut state, &config);
        if !provisional_imported_prefix {
            apply_state_geometry_config_before_kura_replay(&mut state, &config)?;
            // Kura authenticates canonical replay evidence before State exists. Geometry setup
            // above then publishes the configured lane directories, so refresh only that
            // auxiliary metadata before planning. Otherwise the new lane paths invalidate the
            // reusable inventory and force a second complete historical finality audit.
            kura.refresh_v2_startup_replay_auxiliary_binding()
                .map_err(|error| Report::new(error).change_context(StartError::InitKura))?;
        }

        // Resolve the complete replay boundary before selecting any consensus trust source. A
        // signed snapshot lineage is authoritative only when the typed imported prefix and exact
        // Kura boundary have already been authenticated.
        let mut v2_replay_plan = iroha_core::sumeragi::plan_v2_startup_replay(kura.as_ref())
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        // Planning installs O(H) metadata used to avoid another historical
        // finality scan. Own its cleanup across every remaining startup error
        // and transfer that ownership to Sumeragi recovery below.
        let startup_replay_inventory_guard = V2StartupReplayInventoryGuard::new(Arc::clone(&kura));
        v2_replay_plan
            .validate_restored_state_height(state.committed_height())
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        if v2_replay_plan.durable_height() != block_count.0 {
            return Err(Report::new(StartError::InitKura).attach(format!(
                "Sumeragi v2 startup replay plan height {} differs from Kura block count {}",
                v2_replay_plan.durable_height(),
                block_count.0
            )));
        }
        let mut snapshot_startup_authorization =
            iroha_core::sumeragi::authenticate_v2_snapshot_startup(
                kura.as_ref(),
                &state,
                &v2_replay_plan,
            )
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        let authenticated_snapshot_mode = snapshot_startup_authorization
            .as_ref()
            .map(iroha_core::sumeragi::AuthenticatedV2SnapshotStartup::mode);
        let authenticated_snapshot_bootstrap = state.authenticated_snapshot_v2_bootstrap().cloned();
        let snapshot_bootstrap_active = authenticated_snapshot_bootstrap.is_some();
        authorize_kura_runtime_start(provisional_imported_prefix, snapshot_bootstrap_active)
            .map_err(|error| Report::new(StartError::InitKura).attach(error))?;
        let (signed_consensus_mode, signed_v2_genesis_context) = match (
            authenticated_snapshot_mode,
            authenticated_snapshot_bootstrap.as_ref(),
        ) {
            (Some(mode), Some(bootstrap)) if mode == bootstrap.context.mode => (
                mode,
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters {
                    da_layout: bootstrap.context.da_layout,
                    nexus_amx_context_hash: *bootstrap.context.nexus_amx_context_hash.as_ref(),
                    execution_policy_hash: *bootstrap.context.execution_policy_hash.as_ref(),
                },
            ),
            (Some(_), Some(_)) => {
                return Err(Report::new(StartError::InitKura).attach(
                    "authenticated snapshot mode differs from its retained bootstrap lineage",
                ));
            }
            (None, None) => {
                let (mode, context) = signed_genesis_context.ok_or_else(|| {
                        Report::new(StartError::InitKura).attach(
                            "startup has neither authenticated snapshot lineage nor signed genesis metadata",
                        )
                    })?;
                (mode, context)
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(Report::new(StartError::InitKura)
                    .attach("snapshot lineage and typed imported-prefix authentication disagree"));
            }
        };
        if config.nexus.enabled
            && signed_consensus_mode != iroha_data_model::block::consensus_v2::ConsensusMode::Npos
        {
            return Err(Report::new(StartError::InitKura)
                .attach("Nexus requires the authenticated Sumeragi v2 mode to be NPoS"));
        }

        if !snapshot_bootstrap_active && block_count.0 > 0 {
            match kura
                .v2_finality_artifact(1)
                .map_err(|error| Report::new(StartError::InitKura).attach(error))?
            {
                Some(artifact) => {
                    let signed_genesis = effective_genesis.ok_or_else(|| {
                        Report::new(StartError::InitKura).attach(
                            "stored height-one finality has no signed genesis authority root",
                        )
                    })?;
                    iroha_core::sumeragi::validate_signed_genesis_v2_authority(
                        signed_genesis,
                        &artifact.height_context,
                        &artifact.validator_set_pops,
                    )
                    .map_err(|error| {
                        Report::new(StartError::InitKura).attach(format!(
                            "stored Sumeragi v2 genesis finality authority is not rooted in signed genesis: {error}"
                        ))
                    })?;
                    let context = &artifact.height_context;
                    if context.network_id != expected_network_id
                        || context.height != 1
                        || context.mode != signed_consensus_mode
                        || context.da_layout != signed_v2_genesis_context.da_layout
                        || context.nexus_amx_context_hash
                            != iroha_crypto::Hash::prehashed(
                                signed_v2_genesis_context.nexus_amx_context_hash,
                            )
                        || context.execution_policy_hash
                            != iroha_crypto::Hash::prehashed(
                                signed_v2_genesis_context.execution_policy_hash,
                            )
                    {
                        return Err(Report::new(StartError::InitKura).attach(
                            "stored Sumeragi v2 genesis finality context differs from signed genesis metadata",
                        ));
                    }
                }
                None if block_count.0 == 1 => {
                    iroha_logger::warn!(
                        "genesis block is durable without its v2 finality sidecar; safety-WAL recovery must complete it before ingress"
                    );
                }
                None => {
                    return Err(Report::new(StartError::InitKura).attach(
                        "stored Sumeragi v2 chain is missing its genesis finality artifact",
                    ));
                }
            }
        }

        if provisional_imported_prefix {
            // Check the complete pre-finalization body range before permitting deferred recovery
            // to mutate Kura. Deferred stage/manifest/finality recovery can change the replay
            // boundary, so this plan is deliberately not retained for execution below.
            let prefinalization_state_height = state.committed_height();
            let prefinalization_replay_height = v2_replay_plan.complete_prefix_height();
            if prefinalization_replay_height > prefinalization_state_height {
                iroha_core::state::preflight_v2_replay_body_availability(
                    kura.as_ref(),
                    &state,
                    prefinalization_state_height.saturating_add(1),
                    prefinalization_replay_height,
                )
                .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
            }
            let authorization = snapshot_startup_authorization.take().ok_or_else(|| {
                Report::new(StartError::InitKura)
                    .attach("provisional imported prefix has no consumable snapshot authorization")
            })?;
            kura.finalize_authenticated_snapshot_bootstrap(authorization)
                .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
            state
                .rehydrate_deferred_startup_journals_after_snapshot_authentication()
                .map_err(|err| Report::new(err).change_context(StartError::InitKura))?;
            refresh_block_count_after_snapshot_load(
                &mut block_count,
                state.committed_height(),
                kura.as_ref(),
            )
            .map_err(|error| Report::new(StartError::InitKura).attach(error))?;

            // Finalization runs deferred commit-manifest, retained-stage, finality, and carrier
            // recovery. Recompute from the resulting durable image and reauthenticate the exact
            // snapshot boundary; executing the pre-finalization plan would create a TOCTOU trust
            // gap whenever deferred recovery completed or rejected replay metadata.
            v2_replay_plan = iroha_core::sumeragi::plan_v2_startup_replay(kura.as_ref())
                .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
            v2_replay_plan
                .validate_restored_state_height(state.committed_height())
                .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
            if v2_replay_plan.durable_height() != block_count.0 {
                return Err(Report::new(StartError::InitKura).attach(format!(
                    "post-finalization Sumeragi v2 startup replay plan height {} differs from Kura block count {}",
                    v2_replay_plan.durable_height(),
                    block_count.0
                )));
            }
            iroha_core::sumeragi::authenticate_v2_snapshot_replay_boundary(
                kura.as_ref(),
                &state,
                &v2_replay_plan,
            )
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        }
        if kura.provisional_snapshot_bootstrap_pending() {
            return Err(Report::new(StartError::InitKura).attach(
                "Kura imported-prefix storage remains provisional after authorization finalization",
            ));
        }

        // Generic state replay is authorized only through the contiguous prefix whose every
        // full-body height is checkpoint-, manifest-, and cryptographic-finality-complete. The
        // post-finalization preflight covers the complete recomputed range before geometry or WSV
        // mutation. One Kura-first durable tip remains outside that prefix and resumes only
        // through v2 Apply.
        let state_height = state.committed_height();
        let generic_replay_height = v2_replay_plan.complete_prefix_height();
        let generic_replay_start = state_height.saturating_add(1);
        if provisional_imported_prefix {
            apply_state_geometry_config_before_kura_replay(&mut state, &config)?;
        }
        // Transaction validation during replay consults the lane registry. Freeze and install the
        // configured source set before the first replay transition; installing it only after
        // replay leaves even the default lane absent on snapshot-free restart.
        let replay_nexus = nexus_for_runtime_surfaces(&state);
        let frozen_startup_lane_manifests = freeze_lane_manifests_for_startup_replay(&replay_nexus)
            .map_err(|error| Report::new(error).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach("lane manifest registry is not ready before atomic Kura replay")
            })?;
        state.install_lane_manifests(&frozen_startup_lane_manifests);
        if generic_replay_height > state_height {
            iroha_logger::info!(
                start_height = generic_replay_start,
                generic_replay_height,
                pending_v2_tip = ?v2_replay_plan.pending_tip_height(),
                "Replaying authenticated complete Kura prefix"
            );
            iroha_core::state::replay_blocks_from_kura_range(
                &kura,
                &mut state,
                generic_replay_start,
                generic_replay_height,
            )
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        }
        // No Kura writer is live while trust selection or replay can still fail. Only the fully
        // authenticated and replayed state may publish the canonical writer thread.
        let child = Kura::start(kura.clone(), supervisor.shutdown_signal())
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        supervisor.monitor(child);
        // Delay Arc wrapping until after we tweak state with config

        let (events_sender, _) = broadcast::channel(config.torii.events_buffer_capacity.get());
        // Register pipeline events sender for ZK lane reporting
        iroha_core::pipeline::zk_lane::register_events_sender(events_sender.clone());
        // Kura replay can advance consensus-owned Nexus topology beyond the
        // process configuration (manual lifecycle transactions and autoscale
        // transitions both do so). Seed every admission/manifest surface from
        // the effective replayed state, otherwise a restarted node briefly
        // routes with the stale startup catalog and never installs state-side
        // manifest bindings for restored lanes.
        let runtime_nexus = nexus_for_runtime_surfaces(&state);
        let router: Arc<dyn LaneRouter> = if should_use_config_router(&runtime_nexus) {
            Arc::new(ConfigLaneRouter::new(
                runtime_nexus.routing_policy.clone(),
                runtime_nexus.dataspace_catalog.clone(),
                runtime_nexus.lane_catalog.clone(),
            ))
        } else {
            Arc::new(SingleLaneRouter::new())
        };
        let queue_limits = iroha_core::queue::QueueLimits::from_nexus(&runtime_nexus);
        let lane_catalog = Arc::new(runtime_nexus.lane_catalog.clone());
        let dataspace_catalog = Arc::new(runtime_nexus.dataspace_catalog.clone());
        let governance_catalog = Arc::new(runtime_nexus.governance.clone());
        let registry_cfg = runtime_nexus.registry.clone();
        let lane_compliance = if runtime_nexus.compliance.enabled {
            let dir = runtime_nexus
                .compliance
                .policy_dir
                .as_ref()
                .ok_or_else(|| {
                    Report::new(StartError::InitKura)
                        .attach("lane compliance enabled but no policy_dir configured")
                })?;
            let engine =
                LaneComplianceEngine::from_directory(dir, runtime_nexus.compliance.audit_only)
                    .map_err(|err| Report::new(err).change_context(StartError::InitKura))?;
            engine
                .validate_active_catalog(lane_catalog.as_ref())
                .map_err(|err| Report::new(err).change_context(StartError::InitKura))?;
            Some(Arc::new(engine))
        } else {
            None
        };
        let queue = Arc::new(Queue::from_config_with_router_limits_and_catalogs(
            config.queue,
            events_sender.clone(),
            router.clone(),
            queue_limits,
            &lane_catalog,
            &dataspace_catalog,
            lane_compliance.clone(),
        ));
        state.install_lane_compliance_engine(lane_compliance.clone());
        #[cfg(feature = "telemetry")]
        let mut lane_manifest_task = None;
        #[cfg(not(feature = "telemetry"))]
        let mut lane_manifest_task = None;
        // Replay may have committed lane lifecycle transitions. Rebind the same immutable source
        // snapshot used by replay to the effective catalog, rather than rescanning mutable files
        // at a second startup boundary.
        let lane_manifests = rebind_frozen_lane_manifests_after_startup_replay(
            &frozen_startup_lane_manifests,
            &runtime_nexus,
        )
        .map_err(|error| Report::new(error).change_context(StartError::InitKura))
        .map_err(|report| {
            report.attach("lane manifest registry is not ready after atomic Kura replay")
        })?;
        queue.install_lane_manifests_with_state(&lane_manifests, &state);
        state
            .telemetry
            .set_lane_manifest_registry(Arc::clone(&lane_manifests));
        if runtime_nexus.enabled {
            for status in lane_manifests.missing_entries() {
                iroha_logger::warn!(
                    lane = %status.alias,
                    "governance manifest missing; rejecting transactions routed to this lane until a manifest is provisioned"
                );
            }
            #[cfg(feature = "telemetry")]
            {
                let queue_task = Arc::clone(&queue);
                let telemetry_task = state.telemetry.clone();
                let governance_task = Arc::clone(&governance_catalog);
                let registry_cfg_task = registry_cfg.clone();
                lane_manifest_task = Some((
                    queue_task,
                    telemetry_task,
                    governance_task,
                    registry_cfg_task,
                ));
            }
            #[cfg(not(feature = "telemetry"))]
            {
                let queue_task = Arc::clone(&queue);
                let governance_task = Arc::clone(&governance_catalog);
                let registry_cfg_task = registry_cfg.clone();
                lane_manifest_task = Some((queue_task, governance_task, registry_cfg_task));
            }
        }
        // Independent lane producers transfer FIFO ownership before they
        // publish any payload bytes. Install and replay that durable ownership
        // journal before the mandatory ordinary queue-plan journal can reinsert
        // pending transactions.
        let lane_reservation_journal_path = config
            .kura
            .store_dir
            .resolve_relative_path()
            .join("lane_queue_reservations.norito");
        let lane_reservation_replay = queue
            .install_lane_reservation_journal(
                &lane_reservation_journal_path,
                config.queue.plan_journal_max_bytes,
            )
            .map_err(|err| {
                Report::new(StartError::InitKura).attach(format!(
                    "failed to open lane queue reservation journal {}: {err}",
                    lane_reservation_journal_path.display()
                ))
            })?;
        iroha_logger::info!(
            path = %lane_reservation_journal_path.display(),
            restored = lane_reservation_replay.restored,
            awaiting_transaction_replay = lane_reservation_replay.awaiting_transaction_replay,
            commit_barriers = lane_reservation_replay.commit_barriers,
            release_barriers = lane_reservation_replay.release_barriers,
            completed_releases = lane_reservation_replay.completed_releases,
            "lane queue reservation journal installed"
        );

        let journal_path = config
            .kura
            .store_dir
            .resolve_relative_path()
            .join("queue_plan_journal.norito");
        let replayable = queue
            .install_plan_journal(&journal_path, config.queue.plan_journal_max_bytes, true)
            .map_err(|err| {
                Report::new(StartError::InitKura).attach(format!(
                    "failed to open queue plan journal {}: {err}",
                    journal_path.display()
                ))
            })?;
        let replay_summary = queue.replay_plan_journal(&state).map_err(|err| {
            Report::new(StartError::InitKura).attach(format!(
                "failed to replay queue plan journal {}: {err}",
                journal_path.display()
            ))
        })?;
        iroha_logger::info!(
            path = %journal_path.display(),
            replayable,
            records = replay_summary.records,
            replayed = replay_summary.replayed,
            tombstoned_committed = replay_summary.tombstoned_committed,
            tombstoned_expired = replay_summary.tombstoned_expired,
            "queue plan journal installed"
        );

        let compliance_policy_digest = state
            .lane_compliance_engine()
            .map(|engine| engine.consensus_policy_digest());
        let lane_manifest_policy_digest =
            Some(state.lane_manifests.read().consensus_policy_digest());
        let config_caps = build_consensus_config_caps(
            &state.nexus_snapshot(),
            compliance_policy_digest,
            lane_manifest_policy_digest,
        )?;
        let proto = iroha_core::sumeragi::consensus::PROTO_VERSION;

        // Peer admission is frozen by exactly one authenticated startup root. Ordinary startup
        // uses signed genesis metadata. An audited hash-only import instead derives the handshake
        // from the authenticated snapshot WSV and its frozen v2 mode; it must not depend on an
        // unavailable legacy genesis body.
        let (
            computed_mode_tag,
            _computed_bls_domain,
            consensus_caps,
            signed_block_cadence_ms,
            confidential_features,
        ) = {
            let view = state.view();
            let height = u64::try_from(view.block_hashes().len()).expect("height fits into u64");
            let confidential_features = iroha_core::state::compute_confidential_feature_digest(
                view.world(),
                &view.zk,
                view.sccp_registry.as_ref(),
                height,
            );
            let (mode_tag, bls_domain, caps, block_cadence_ms) = if snapshot_bootstrap_active {
                let (mode_tag, bls_domain, caps) = compute_consensus_handshake_caps(
                    view.world(),
                    height,
                    &config,
                    &config_caps,
                    signed_consensus_mode,
                    signed_v2_genesis_context,
                )?;
                (
                    mode_tag,
                    bls_domain,
                    caps,
                    view.world()
                        .parameters()
                        .sumeragi()
                        .block_cadence_ms()
                        .get(),
                )
            } else {
                consensus_caps_from_genesis(
                        effective_genesis.expect("normal startup has signed genesis metadata"),
                        &config_caps,
                        &config.sumeragi,
                    )
                    .ok_or_else(|| {
                        Report::new(StartError::InitKura).attach(
                            "signed genesis does not contain one canonical Sumeragi v2 handshake context",
                        )
                    })?
            };
            (
                mode_tag,
                bls_domain,
                caps,
                block_cadence_ms,
                confidential_features,
            )
        };
        let authenticated_block_cadence = Duration::from_millis(signed_block_cadence_ms);
        if state.committed_height() > 0
            && state.sumeragi_block_cadence() != authenticated_block_cadence
        {
            return Err(Report::new(StartError::InitKura).attach(format!(
                "committed state cadence {:?} differs from authenticated startup cadence {:?}",
                state.sumeragi_block_cadence(),
                authenticated_block_cadence,
            )));
        }
        iroha_logger::info!(
            mode=%consensus_caps.mode.tag(),
            proto=%consensus_caps.proto_version,
            fingerprint=%format!("0x{}", hex::encode(consensus_caps.consensus_fingerprint)),
            "Consensus handshake caps"
        );
        let mut staged_v2_genesis = None;
        if !snapshot_bootstrap_active {
            verify_genesis_metadata(
                effective_genesis.expect("normal startup has signed genesis metadata"),
                &config,
                &consensus_caps,
                &computed_mode_tag,
                proto,
            )
            .map_err(|error| Report::new(StartError::InitKura).attach(error))?;
        }

        // If a genesis manifest JSON is provided via CLI, validate consensus fields.
        let cfg_manifest = config
            .genesis
            .manifest_json
            .as_ref()
            .map(WithOrigin::resolve_relative_path);
        if !snapshot_bootstrap_active && let Some(json_path) = cfg_manifest {
            let manifest = read_genesis_manifest(&json_path)?;
            if let Err(err) = ensure_manifest_crypto_matches(&manifest, &config) {
                return Err(Report::new(StartError::InitKura).attach(format!(
                    "Genesis manifest crypto settings do not match node configuration: {err}"
                )));
            } else if genesis.is_none() {
                config.crypto = manifest.crypto().clone().into();
            }

            let expected = match signed_consensus_mode {
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned => {
                    iroha_core::sumeragi::consensus::PERMISSIONED_TAG
                }
                iroha_data_model::block::consensus_v2::ConsensusMode::Npos => {
                    iroha_core::sumeragi::consensus::NPOS_TAG
                }
            };
            let got = match manifest.consensus_mode() {
                iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => {
                    iroha_core::sumeragi::consensus::PERMISSIONED_TAG
                }
                iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => {
                    iroha_core::sumeragi::consensus::NPOS_TAG
                }
            };
            if got != expected {
                return Err(Report::new(StartError::InitKura).attach(format!(
                    "Genesis manifest consensus_mode mismatch: manifest `{got}`, expected `{expected}`"
                )));
            }

            if manifest.wire_protocol_version() != proto {
                return Err(Report::new(StartError::InitKura).attach(format!(
                    "Genesis manifest wire_protocol_version is not v{proto}"
                )));
            }

            if let Some(fingerprint) = manifest.consensus_fingerprint()
                && fingerprint.into_bytes() != consensus_caps.consensus_fingerprint
            {
                return Err(
                    Report::new(GenesisManifestError::ConsensusFingerprintMismatch)
                        .change_context(StartError::InitKura),
                );
            }
        }

        let confidential_caps = iroha_p2p::ConfidentialHandshakeCaps {
            enabled: config.confidential.enabled,
            assume_valid: config.confidential.assume_valid,
            verifier_backend: config.confidential.verifier_backend.clone(),
            features: Some(confidential_handshake_policy_digest(confidential_features)),
        };
        let crypto_caps = iroha_p2p::CryptoHandshakeCaps {
            sm_enabled: config.crypto.sm_helpers_enabled(),
            sm_openssl_preview: config.crypto.enable_sm_openssl_preview,
            require_sm_handshake_match: config.network.require_sm_handshake_match,
            require_sm_openssl_preview_match: config.network.require_sm_openssl_preview_match,
        };
        let configured_validator_dial_roster: BTreeSet<_> =
            filter_validators_from_trusted(config.common.trusted_peers.value())
                .into_iter()
                .collect();
        let initial_trusted_sources = config
            .common
            .trusted_peers
            .value()
            .others
            .iter()
            .map(|peer| peer.id().clone())
            .collect();
        let p2p_identity_keys = iroha_p2p::P2pIdentityKeys::new(
            config.common.key_pair.clone(),
            config.common.soranet_transport_key_pair.clone(),
        )
        .attach_with(|| config.network.address.clone().into_attachment())
        .change_context(StartError::StartP2p)?;
        let (network, child) = IrohaNetwork::start_with_crypto_and_initial_authorities(
            p2p_identity_keys,
            config.network.clone(),
            expected_network_id,
            Some(consensus_caps.clone()),
            Some(confidential_caps),
            Some(crypto_caps),
            initial_trusted_sources,
            configured_validator_dial_roster.iter().cloned().collect(),
            supervisor.shutdown_signal(),
        )
        .await
        .attach_with(|| config.network.address.clone().into_attachment())
        .change_context(StartError::StartP2p)?;
        supervisor.monitor(child);

        if !snapshot_bootstrap_active && let Some(genesis_block) = genesis.as_ref() {
            // On non-empty storage, avoid re-validating the provided genesis signature.
            // Instead, ensure the optional provided payload matches the genesis already
            // persisted at height 1 and continue replay from stored data.
            if let Some(stored_genesis) = stored_genesis_block.as_ref() {
                let stored_hash = stored_genesis.0.hash();
                let provided_hash = genesis_block.0.hash();
                if stored_hash != provided_hash {
                    return Err(Report::new(StartError::InitKura).attach(format!(
                        "provided genesis does not match stored genesis (stored={stored_hash}, provided={provided_hash})",
                    )));
                }
                iroha_logger::info!(
                    hash = %stored_hash,
                    "non-empty block store detected; using stored genesis for restart",
                );
            } else {
                let (fresh_mode_tag, _fresh_bls_domain, fresh_caps, fresh_block_cadence_ms) =
                    consensus_caps_from_genesis(genesis_block, &config_caps, &config.sumeragi)
                        .ok_or_else(|| {
                            Report::new(StartError::InitKura).attach(
                        "fresh genesis is missing required signed Sumeragi v2 consensus metadata",
                    )
                        })?;
                if fresh_block_cadence_ms != signed_block_cadence_ms {
                    return Err(Report::new(StartError::InitKura).attach(
                        "fresh signed genesis cadence differs from the handshake opened for bootstrap",
                    ));
                }
                verify_genesis_metadata(
                    genesis_block,
                    &config,
                    &fresh_caps,
                    &fresh_mode_tag,
                    proto,
                )
                .map_err(|error| Report::new(StartError::InitKura).attach(error))?;
                if fresh_caps.consensus_fingerprint != consensus_caps.consensus_fingerprint
                    || fresh_caps.mode != consensus_caps.mode
                    || fresh_caps.proto_version != consensus_caps.proto_version
                {
                    return Err(Report::new(StartError::InitKura).attach(
                        "fresh signed genesis consensus metadata differs from the peer handshake opened for bootstrap",
                    ));
                }
                let genesis_account = AccountId::new(effective_genesis_public_key.clone());
                if let Err(err) =
                    iroha_core::validate_genesis_block(&genesis_block.0, &genesis_account)
                {
                    let err_display = err.to_string();
                    iroha_logger::error!(
                        error = %err,
                        "Invalid genesis block rejected during validation"
                    );
                    return Err(Report::new(err)
                        .attach(format!(
                            "Invalid genesis block rejected during validation: {err_display}"
                        ))
                        .change_context(StartError::InitKura));
                }

                // Execute genesis in a disposable state overlay. The signed
                // RegisterPeerWithPop set is the only allowed height-one voting
                // topology; plain peer registrations are observers.
                let signed_voters = iroha_core::sumeragi::signed_genesis_voting_peers(
                    genesis_block,
                )
                .map_err(|error| {
                    Report::new(StartError::InitKura).attach(format!(
                        "invalid signed Sumeragi v2 genesis roster: {error}"
                    ))
                })?;
                let topology = Topology::new(signed_voters);
                let time_source = TimeSource::new_system();
                let mut voting_block: Option<VotingBlock> = None;
                let committed_height_before_staging = state.committed_height();
                let block_count_before_staging = block_count;
                let validation = ValidBlock::validate_signed_genesis_keep_voting_block(
                    genesis_block.0.clone(),
                    &topology,
                    &genesis_account,
                    &time_source,
                    &state,
                    &mut voting_block,
                    signed_consensus_mode,
                )
                .unpack(|_| {});
                match validation {
                    Ok((_valid_block, state_block)) => {
                        let staged_block_cadence_ms = state_block
                            .world()
                            .parameters()
                            .sumeragi()
                            .block_cadence_ms()
                            .get();
                        if staged_block_cadence_ms != fresh_block_cadence_ms {
                            return Err(Report::new(StartError::InitKura).attach(format!(
                                "staged genesis cadence {staged_block_cadence_ms} ms differs from authenticated signed cadence {fresh_block_cadence_ms} ms",
                            )));
                        }
                        let (mode, signed_parameters) =
                            signed_v2_genesis_context_metadata(genesis_block)
                                .map_err(|error| Report::new(StartError::InitKura).attach(error))?;
                        staged_v2_genesis = Some(
                            iroha_core::sumeragi::freeze_staged_genesis_v2(
                                genesis_block,
                                &state_block,
                                mode,
                                signed_parameters,
                            )
                            .map_err(|error| {
                                Report::new(StartError::InitKura).attach(format!(
                                    "failed to freeze staged Sumeragi v2 genesis: {error}"
                                ))
                            })?,
                        );
                        // Dropping StateBlock discards the overlay. Consensus
                        // must persist a CommitQC decision before applying it.
                        drop(state_block);
                        if state.committed_height() != committed_height_before_staging
                            || block_count.0 != block_count_before_staging.0
                        {
                            return Err(Report::new(StartError::InitKura).attach(
                                "fresh genesis staging mutated committed state before consensus",
                            ));
                        }
                        iroha_logger::info!(
                            context_id = ?staged_v2_genesis
                                .as_ref()
                                .expect("just staged")
                                .context()
                                .id(),
                            "Validated and staged genesis without committing state or Kura"
                        );
                    }
                    Err((_failed_block, err)) => {
                        let err_display = err.to_string();
                        iroha_logger::error!(
                            error = %err,
                            "Genesis block execution failed during validation"
                        );
                        return Err(Report::new(err)
                            .attach(format!(
                                "Genesis block execution failed during validation: {err_display}"
                            ))
                            .change_context(StartError::InitKura));
                    }
                }
            }
        } else if !snapshot_bootstrap_active && block_count.0 == 0 {
            return Err(Report::new(StartError::InitKura)
                .attach("missing genesis file for empty storage; provide `--genesis.file`"));
        }

        let snapshot_file = config
            .streaming
            .session_store_dir
            .clone()
            .join("sessions.norito");

        let mut streaming = iroha_core::streaming::StreamingHandle::with_key_material(
            config.streaming.key_material.clone(),
        )
        .with_capabilities(CapabilityFlags::from_bits(config.streaming.feature_bits));
        streaming
            .apply_codec_config(&config.streaming.codec)
            .map_err(|err| Report::new(err).change_context(StartError::StartP2p))?;
        streaming.apply_crypto_config(&config.crypto);
        streaming.set_soranet_config(&config.streaming.soranet);
        streaming.apply_sync_config(&config.streaming.sync);
        #[cfg(feature = "telemetry")]
        if let Some(ref telemetry_handle) = streaming_telemetry {
            streaming = streaming.with_telemetry(telemetry_handle.clone());
        }
        configure_soranet_transport(&mut streaming, &config.streaming.soranet)?;
        streaming.set_snapshot_path(snapshot_file.clone());

        let snapshot_encryption_key =
            iroha_core::streaming::snapshot_session_key(&config.streaming.key_material);

        streaming
            .set_snapshot_encryption_key(&snapshot_encryption_key)
            .map_err(Report::from)
            .change_context(StartError::StartP2p)
            .map_err(|report| report.attach("failed to configure streaming snapshot encryption"))?;

        if let Err(err) = streaming.load_snapshots() {
            iroha_logger::warn!(?err, "Failed to load streaming session snapshots");
        }
        log_startup_trace("irohad.streaming.ready", startup_trace_started_at);

        iroha_core::streaming::set_global_handle(streaming.clone());

        let streaming_events_handle = streaming.clone();
        let ticket_events_rx = events_sender.subscribe();
        supervisor.monitor(tokio::spawn(async move {
            run_ticket_event_listener(streaming_events_handle, ticket_events_rx).await;
        }));

        #[cfg(feature = "telemetry")]
        start_telemetry(&logger, &config, &mut supervisor).await?;
        #[cfg(feature = "telemetry")]
        log_startup_trace("irohad.telemetry.ready", startup_trace_started_at);

        // Thread the remaining runtime preferences from config into state. ZK
        // configuration was installed once before Kura replay so hydrated SCCP
        // usage was checked against the actual configured caps at startup.
        // Use cloned config values to keep `config` borrowable later.
        let tiered_state_cfg = config.tiered_state.clone();
        let pipeline_cfg = config.pipeline.clone();
        let sumeragi_cfg = config.sumeragi.clone();
        let fraud_cfg = config.fraud_monitoring.clone();
        let zk_cfg = config.zk.clone();
        let gov_cfg = config.gov.clone();
        let oracle_cfg = config.oracle.clone();
        let streaming_soranet_spool_dir = config.streaming.soranet.provision_spool_dir.clone();
        let streaming_soravpn_spool_dir = config.streaming.soravpn.provision_spool_dir.clone();
        let merge_cache_capacity = config.kura.merge_ledger_cache_capacity;
        state
            .set_tiered_backend(&tiered_state_cfg)
            .map_err(|err| Report::new(err).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach("failed to restore effective Nexus tiered lane geometry")
            })?;
        state.set_pipeline(pipeline_cfg);
        state.set_sumeragi_parameters(&sumeragi_cfg);
        state.set_oracle(oracle_cfg);
        state.set_streaming_storage_paths(streaming_soranet_spool_dir, streaming_soravpn_spool_dir);
        state.set_fraud_monitoring(fraud_cfg);
        // Settlement runtime state was installed before Kura replay. Preserve
        // its lazily derived escrow bindings instead of replacing the replayed
        // snapshot after Kura has started.
        state.set_gov(gov_cfg);
        state.set_merge_ledger_cache_capacity(merge_cache_capacity);
        log_startup_trace(
            "irohad.state.runtime_config_applied",
            startup_trace_started_at,
        );
        // Recovery: scan recent persisted pipeline sidecars and log DAG fingerprint mismatches (best-effort).
        #[cfg(feature = "dag-recovery-verify")]
        {
            use iroha_core::pipeline::access::{IvmStrategy, derive_for_transaction};
            use nonzero_ext::nonzero;
            use sha2::{Digest, Sha256};

            // Choose strategy based on configured pipeline prepass
            let view = state.query_view();
            let dyn_pre = state.pipeline_snapshot().dynamic_prepass;
            let strategy = if dyn_pre {
                IvmStrategy::DynamicThenConservative
            } else {
                IvmStrategy::Conservative
            };

            // Deterministic fingerprint over interned access ids + call hashes
            fn fp_from_access(
                key_count: usize,
                access: &[iroha_core::pipeline::access::AccessSet],
                call_hashes: &[iroha_crypto::HashOf<
                    iroha_data_model::transaction::signed::TransactionEntrypoint,
                >],
            ) -> [u8; 32] {
                use std::collections::BTreeMap;
                let mut map: BTreeMap<&str, u32> = BTreeMap::new();
                for aset in access.iter() {
                    for k in aset.read_keys.iter() {
                        map.entry(k.as_str()).or_insert(u32::MAX);
                    }
                    for k in aset.write_keys.iter() {
                        map.entry(k.as_str()).or_insert(u32::MAX);
                    }
                }
                let mut next: u32 = 0;
                for v in map.values_mut() {
                    *v = next;
                    next = next.saturating_add(1);
                }
                let mut hasher = Sha256::new();
                hasher.update(&(key_count as u64).to_le_bytes());
                for aset in access.iter() {
                    hasher.update(&(aset.read_keys.len() as u64).to_le_bytes());
                    for k in aset.read_keys.iter() {
                        let id = *map.get(k.as_str()).expect("interned");
                        hasher.update(&id.to_le_bytes());
                    }
                    hasher.update(&(aset.write_keys.len() as u64).to_le_bytes());
                    for k in aset.write_keys.iter() {
                        let id = *map.get(k.as_str()).expect("interned");
                        hasher.update(&id.to_le_bytes());
                    }
                }
                for ch in call_hashes.iter() {
                    hasher.update(ch.as_ref());
                }
                hasher.finalize().into()
            }

            // Scan recent blocks for persisted sidecars and compare fingerprints
            let scan_n: usize = 16;
            let total = block_count.0;
            let start = total.saturating_sub(scan_n) + 1;
            for h in start..=total {
                if let Some(sidecar) = kura.read_pipeline_metadata(h as u64) {
                    let exp = sidecar.dag.fingerprint;
                    if let Some(height) = std::num::NonZeroUsize::new(h) {
                        if let Some(block) = kura.get_block(height) {
                            let txs: Vec<&iroha_data_model::transaction::SignedTransaction> =
                                block.external_transactions().collect();
                            let access: Vec<_> = txs
                                .iter()
                                .map(|tx| derive_for_transaction(tx, Some(&view), strategy))
                                .collect();
                            use std::collections::BTreeSet;
                            let mut keys = BTreeSet::new();
                            for aset in access.iter() {
                                for k in aset.read_keys.iter() {
                                    keys.insert(k.as_str());
                                }
                                for k in aset.write_keys.iter() {
                                    keys.insert(k.as_str());
                                }
                            }
                            let key_count = keys.len();
                            let call_hashes: Vec<_> =
                                txs.iter().map(|tx| tx.hash_as_entrypoint()).collect();
                            let got = fp_from_access(key_count, &access, &call_hashes);
                            if got != exp {
                                iroha_logger::warn!(
                                    height = h,
                                    expected=%hex::encode(exp),
                                    actual=%hex::encode(got),
                                    "startup: pipeline DAG fingerprint mismatch (persisted vs recomputed)"
                                );
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(feature = "dag-recovery-verify"))]
        {
            // Recovery sidecar scan is optional and only used for diagnostics; keep it lightweight
            let scan_n: usize = 16;
            let total = block_count.0;
            let start = total.saturating_sub(scan_n) + 1;
            for h in start..=total {
                if kura.read_pipeline_metadata(h as u64).is_some() {
                    iroha_logger::debug!(height = h, "found pipeline recovery sidecar");
                }
            }
        }
        let state = Arc::new(state);
        #[cfg(feature = "telemetry")]
        if let Some((queue_task, telemetry_task, governance_task, registry_cfg_task)) =
            lane_manifest_task
        {
            let state_task = Arc::clone(&state);
            tokio::spawn(async move {
                queue_task
                    .watch_lane_manifests_task(
                        Some(telemetry_task),
                        governance_task,
                        registry_cfg_task,
                        Some(state_task),
                    )
                    .await;
            });
        }
        #[cfg(not(feature = "telemetry"))]
        if let Some((queue_task, governance_task, registry_cfg_task)) = lane_manifest_task {
            let state_task = Arc::clone(&state);
            tokio::spawn(async move {
                queue_task
                    .watch_lane_manifests_task(
                        None,
                        governance_task,
                        registry_cfg_task,
                        Some(state_task),
                    )
                    .await;
            });
        }

        #[cfg(feature = "telemetry")]
        let telemetry = {
            let (metrics_reporter, child) = iroha_core::telemetry::start(
                metrics,
                Arc::clone(&state),
                kura.clone(),
                queue.clone(),
                network.online_peers_receiver(),
                config.common.peer.id.clone(),
                TimeSource::new_system(),
                telemetry_capabilities.metrics_enabled(),
            );
            supervisor.monitor(child);

            metrics_reporter
        };

        let (peers_gossiper, child) = PeersGossiper::start(
            config.common.peer.id.clone(),
            expected_network_id,
            config.common.trusted_peers.value().clone(),
            configured_validator_dial_roster,
            config.common.key_pair.clone(),
            config.network.peer_gossip_period,
            config.network.peer_gossip_max_period,
            signed_consensus_mode,
            config.network.trust_decay_half_life,
            config.network.trust_penalty_bad_gossip,
            config.network.trust_penalty_unknown_peer,
            config.network.trust_min_score,
            network.clone(),
            supervisor.shutdown_signal(),
        );
        supervisor.monitor(child);
        log_startup_trace("irohad.peers_gossiper.ready", startup_trace_started_at);

        #[cfg(feature = "telemetry")]
        let torii_telemetry =
            iroha_torii::MaybeTelemetry::from_profile(Some(telemetry.clone()), telemetry_profile);
        #[cfg(not(feature = "telemetry"))]
        let torii_telemetry = iroha_torii::MaybeTelemetry::from_profile(None, telemetry_profile);

        // The feature-isolated receiver controller must pin and acknowledge its
        // revision-1 rules before Sumeragi can enter the first post-genesis round.
        #[cfg(feature = "test-network-message-control")]
        let test_message_control = match consensus_message_control::Controller::from_env() {
            Ok(controller) => controller.map(Arc::new),
            Err(error) => {
                iroha_logger::error!(
                    reason = error.code(),
                    "test-network consensus message controller failed closed before Sumeragi startup"
                );
                std::process::exit(1);
            }
        };

        let mut prepared_sorafs_provider_ingest_archive = if let Some(provider_ingest_config) =
            config.torii.sorafs_storage.provider_ingest_runtime.as_ref()
        {
            let provider_id = config
                    .torii
                    .sorafs_storage
                    .provider_id
                    .ok_or_else(|| {
                        Report::new(StartError::StartP2p).attach(
                            "enabled provider-ingest archive requires the exact configured storage provider identity",
                        )
                    })?;
            let prepared =
                    sorafs_provider_ingest_finalized_query::prepare_provider_ingest_finalized_archive_v1(
                        &provider_ingest_config.finalized_archive,
                        NetworkId::from_genesis_hash(config.genesis.expected_hash),
                        provider_id,
                        &config.kura.store_dir.resolve_relative_path(),
                        &state,
                        &kura,
                        &v2_replay_plan,
                        runtime_deps
                            .sorafs_provider_ingest_retention_authority
                            .clone(),
                    )
                    .map_err(|error| {
                        Report::new(StartError::StartP2p).attach(format!(
                            "failed to qualify the daemon-owned finalized provider-ingest archive before Sumeragi startup: {error}"
                        ))
                    })?;
            if let Some(retention) = prepared.retention_authority() {
                iroha_logger::info!(
                    authority_handle = retention.authority().handle(),
                    authority_revision = retention.binding().qualification().revision(),
                    "qualified deployment-owned provider-ingest archive retention authority"
                );
            }
            match prepared.startup_mode() {
                sorafs_provider_ingest_finalized_query::ProviderIngestFinalizedArchiveStartupModeV1::BootstrapAwaitingGenesisCapture => {
                    iroha_logger::info!(
                        state_height = 0,
                        kura_tip_height = 0,
                        "opened empty daemon-owned finalized provider-ingest archive; genesis capture will establish its first anchor"
                    );
                }
                sorafs_provider_ingest_finalized_query::ProviderIngestFinalizedArchiveStartupModeV1::Qualified {
                    reconciliation,
                    live_qualification,
                } => {
                    if reconciliation.activation_floor_created() {
                        iroha_logger::warn!(
                            activation_floor_height =
                                reconciliation.qualification().activation_floor().height,
                            "finalized provider-ingest archive established an explicit activation floor; earlier historical coverage is unavailable"
                        );
                    }
                    iroha_logger::info!(
                        activation_floor_height = live_qualification.activation_floor().height,
                        archive_tip_height = live_qualification.archive_tip().height,
                        kura_tip_height = live_qualification.kura_tip_height(),
                        lag_blocks = live_qualification.lag_blocks(),
                        "qualified daemon-owned finalized provider-ingest archive"
                    );
                }
                sorafs_provider_ingest_finalized_query::ProviderIngestFinalizedArchiveStartupModeV1::PendingTipReplay {
                    pending_tip_height,
                    qualification,
                    activation_floor_created,
                } => {
                    if *activation_floor_created
                        && let Some(qualification) = qualification
                    {
                        iroha_logger::warn!(
                            activation_floor_height = qualification.activation_floor().height,
                            "pending-tip provider-ingest recovery established an explicit activation floor; earlier historical coverage is unavailable"
                        );
                    }
                    if let Some(qualification) = qualification {
                        iroha_logger::info!(
                            pending_tip_height = *pending_tip_height,
                            activation_floor_height = qualification.activation_floor().height,
                            archive_tip_height = qualification.archive_tip().height,
                            kura_tip_height = qualification.kura_tip_height(),
                            lag_blocks = qualification.lag_blocks(),
                            "qualified daemon-owned finalized provider-ingest archive for exact pending-tip replay"
                        );
                    } else {
                        iroha_logger::info!(
                            pending_tip_height = *pending_tip_height,
                            state_height = 0,
                            "opened empty daemon-owned finalized provider-ingest archive for pending genesis replay"
                        );
                    }
                }
            }
            Some(prepared)
        } else {
            None
        };
        validate_provider_ingest_archive_presence(
            config
                .torii
                .sorafs_storage
                .provider_ingest_runtime
                .is_some(),
            prepared_sorafs_provider_ingest_archive.is_some(),
        )
        .map_err(|message| Report::new(StartError::StartP2p).attach(message))?;
        let sorafs_provider_ingest_finalized_query = prepared_sorafs_provider_ingest_archive
            .as_ref()
            .map(|prepared| Arc::clone(prepared.query()));
        let sorafs_provider_ingest_runtime_query = prepared_sorafs_provider_ingest_archive
            .as_ref()
            .map(|prepared| Arc::clone(prepared.runtime_query()));

        let sorafs_reputation_retention_authority =
            runtime_deps.sorafs_reputation_retention_authority.clone();
        if config.torii.sorafs_storage.reputation_runtime.is_none()
            && sorafs_reputation_retention_authority.is_some()
        {
            return Err(Report::new(StartError::StartP2p).attach(
                "disabled SoraFS reputation runtime rejects an unexpected finalized-archive retention authority",
            ));
        }
        let prepared_sorafs_reputation_archive = if let Some(reputation_config) =
            config.torii.sorafs_storage.reputation_runtime.as_ref()
        {
            let prepared =
                    sorafs_reputation_finalized_query::prepare_reputation_finalized_archive_v1(
                        reputation_config,
                        &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                        &state,
                        &kura,
                        &v2_replay_plan,
                        sorafs_reputation_retention_authority.clone(),
                    )
                    .map_err(|error| {
                        Report::new(StartError::StartP2p).attach(format!(
                            "failed to qualify the daemon-owned finalized reputation archive before Sumeragi startup: {error}"
                        ))
                    })?;
            if let Some(retention) = prepared.retention_authority() {
                iroha_logger::info!(
                    authority_handle = retention.authority().handle(),
                    authority_revision = retention.binding().qualification().revision(),
                    "qualified deployment-owned finalized reputation archive retention authority"
                );
            }
            match prepared.startup_mode() {
                sorafs_reputation_finalized_query::ReputationFinalizedArchiveStartupModeV1::BootstrapAwaitingGenesisCapture => {
                    iroha_logger::info!(
                        state_height = 0,
                        kura_tip_height = 0,
                        "opened empty daemon-owned finalized reputation archive; genesis capture will establish its first anchor"
                    );
                }
                sorafs_reputation_finalized_query::ReputationFinalizedArchiveStartupModeV1::Qualified {
                    reconciliation,
                    live_qualification,
                } => {
                    if reconciliation.activation_floor_created() {
                        iroha_logger::warn!(
                            activation_floor_height =
                                reconciliation.qualification().activation_floor().height,
                            "finalized reputation archive established an explicit activation floor; earlier historical coverage is unavailable"
                        );
                    }
                    iroha_logger::info!(
                        activation_floor_height = live_qualification.activation_floor().height,
                        archive_tip_height = live_qualification.archive_tip().height,
                        kura_tip_height = live_qualification.kura_tip_height(),
                        lag_blocks = live_qualification.lag_blocks(),
                        "qualified daemon-owned finalized reputation archive"
                    );
                }
                sorafs_reputation_finalized_query::ReputationFinalizedArchiveStartupModeV1::PendingTipReplay {
                    pending_tip_height,
                    qualification,
                    activation_floor_created,
                } => {
                    if *activation_floor_created
                        && let Some(qualification) = qualification
                    {
                        iroha_logger::warn!(
                            activation_floor_height = qualification.activation_floor().height,
                            "pending-tip reputation recovery established an explicit activation floor; earlier historical coverage is unavailable"
                        );
                    }
                    if let Some(qualification) = qualification {
                        iroha_logger::info!(
                            pending_tip_height = *pending_tip_height,
                            activation_floor_height = qualification.activation_floor().height,
                            archive_tip_height = qualification.archive_tip().height,
                            kura_tip_height = qualification.kura_tip_height(),
                            lag_blocks = qualification.lag_blocks(),
                            "qualified daemon-owned finalized reputation archive for exact pending-tip replay"
                        );
                    } else {
                        iroha_logger::info!(
                            pending_tip_height = *pending_tip_height,
                            state_height = 0,
                            "opened empty daemon-owned finalized reputation archive for pending genesis replay"
                        );
                    }
                }
            }
            Some(prepared)
        } else {
            None
        };
        validate_reputation_archive_presence(
            config.torii.sorafs_storage.reputation_runtime.is_some(),
            prepared_sorafs_reputation_archive.is_some(),
        )
        .map_err(|message| Report::new(StartError::StartP2p).attach(message))?;

        let genesis_for_consensus = if snapshot_bootstrap_active || stored_genesis_block.is_some() {
            None
        } else {
            genesis
        };
        let sumeragi_cfg = config.sumeragi.clone();
        log_startup_trace("irohad.sumeragi.starting", startup_trace_started_at);
        let (sumeragi, child) = SumeragiStartArgs {
            config: sumeragi_cfg.clone(),
            common_config: config.common.clone(),
            events_sender: events_sender.clone(),
            state: state.clone(),
            queue: queue.clone(),
            kura: kura.clone(),
            provider_ingest_finalized_archive: prepared_sorafs_provider_ingest_archive
                .as_ref()
                .map(|prepared| Arc::clone(prepared.archive())),
            reputation_finalized_archive: prepared_sorafs_reputation_archive
                .as_ref()
                .map(|prepared| Arc::clone(prepared.archive())),
            startup_replay_plan: v2_replay_plan,
            startup_replay_inventory_guard,
            network: network.clone(),
            max_frame_bytes: config.network.max_frame_bytes,
            max_frame_bytes_consensus: config.network.max_frame_bytes_consensus,
            max_frame_bytes_control: config.network.max_frame_bytes_control,
            max_frame_bytes_block_sync: config.network.max_frame_bytes_block_sync,
            outbound_frame_queue_max_high_bytes: config
                .network
                .p2p_outbound_frame_queue_max_high_bytes
                .get(),
            genesis_network: GenesisWithPubKey {
                genesis: genesis_for_consensus,
                public_key: effective_genesis_public_key.clone(),
                block_cadence: authenticated_block_cadence,
                v2_bootstrap: staged_v2_genesis,
            },
        }
        .start(supervisor.shutdown_signal())
        .map_err(|error| {
            Report::new(StartError::StartP2p)
                .attach(format!("failed to start Sumeragi v2 reducer: {error:#}"))
        })?;
        supervisor.monitor(child);
        log_startup_trace("irohad.sumeragi.started", startup_trace_started_at);

        let trusted = config.common.trusted_peers.value();
        let self_peer_id = trusted.myself.id().clone();
        let trusted_peers: BTreeSet<_> = std::iter::once(self_peer_id.clone())
            .chain(trusted.others.iter().map(|peer| peer.id().clone()))
            .collect();
        let max_peer_id = trusted_peers
            .iter()
            .max_by_key(|peer_id| peer_id.encoded_len())
            .cloned()
            .unwrap_or_else(|| self_peer_id.clone());

        let (tx_gossiper, child) = TransactionGossiper::from_config(
            config.transaction_gossiper,
            &config.network,
            self_peer_id,
            max_peer_id,
            network.clone(),
            Arc::clone(&queue),
            Arc::clone(&state),
        )
        .start(supervisor.shutdown_signal());
        supervisor.monitor(child);

        if let Some(snapshot_maker) =
            SnapshotMaker::from_config(&config.snapshot, Arc::clone(&state), signing_key)
        {
            supervisor.monitor(snapshot_maker.start(supervisor.shutdown_signal()));
        }

        let sorafs_storage_config =
            sorafs_node::config::StorageConfig::from(&config.torii.sorafs_storage);
        let sorafs_repair_config =
            sorafs_node::config::RepairConfig::from(&config.torii.sorafs_repair);
        let sorafs_gc_config = sorafs_node::config::GcConfig::from(&config.torii.sorafs_gc);
        let bootle_lantern_issuance_provider_registry = runtime_deps
            .bootle_lantern_issuance_provider_registry
            .clone();
        let moderation_quarantine_key_wrapper =
            runtime_deps.moderation_quarantine_key_wrapper.clone();
        let privacy_cycle_prf_provider = runtime_deps.privacy_cycle_prf_provider.clone();
        let privacy_release_anchor = runtime_deps.privacy_release_anchor.clone();
        let transparency_leader_lease_provider =
            runtime_deps.transparency_leader_lease_provider.clone();
        let sorafs_fenced_transparency_publisher =
            runtime_deps.sorafs_fenced_transparency_publisher.clone();
        let sorafs_fenced_transparency_head_reader =
            runtime_deps.sorafs_fenced_transparency_head_reader.clone();
        let sorafs_governance_dag_signer = runtime_deps.sorafs_governance_dag_signer.clone();
        let sorafs_governance_dag_checkpoint_store =
            runtime_deps.sorafs_governance_dag_checkpoint_store.clone();
        let sorafs_stream_token_signer = runtime_deps.sorafs_stream_token_signer.clone();
        let sorafs_stream_token_gateway_admission =
            runtime_deps.sorafs_stream_token_gateway_admission.clone();
        let sorafs_appeal_finance_runtime_signers =
            runtime_deps.sorafs_appeal_finance_runtime_signers.clone();
        let sorafs_appeal_finance_checkpoint_runtime = runtime_deps
            .sorafs_appeal_finance_checkpoint_runtime
            .clone();
        let sorafs_proof_outcome_signer = runtime_deps.sorafs_proof_outcome_signer.clone();
        let sorafs_repair_transaction_signer =
            runtime_deps.sorafs_repair_transaction_signer.clone();
        let sorafs_reserve_transaction_signer =
            runtime_deps.sorafs_reserve_transaction_signer.clone();
        let sorafs_orderbook_transaction_signer =
            runtime_deps.sorafs_orderbook_transaction_signer.clone();
        let soracloud_runtime_mutation_signer =
            runtime_deps.soracloud_runtime_mutation_signer.clone();
        let soracloud_hf_inference_credential_provider = runtime_deps
            .soracloud_hf_inference_credential_provider
            .clone();
        let sorafs_moderation_transaction_signer =
            runtime_deps.sorafs_moderation_transaction_signer.clone();
        let sorafs_moderation_settlement_handoff =
            runtime_deps.sorafs_moderation_settlement_handoff.clone();
        let sorafs_moderation_publication_handoff =
            runtime_deps.sorafs_moderation_publication_handoff.clone();
        let sorafs_moderation_panel_notification =
            runtime_deps.sorafs_moderation_panel_notification.clone();
        let sorafs_moderation_panel_notification_archive = runtime_deps
            .sorafs_moderation_panel_notification_archive
            .clone();
        let sorafs_moderation_checkpoint_store =
            runtime_deps.sorafs_moderation_checkpoint_store.clone();
        let sorafs_evidence_viewer_webauthn = runtime_deps.sorafs_evidence_viewer_webauthn.clone();
        let sorafs_evidence_viewer_grants = runtime_deps.sorafs_evidence_viewer_grants.clone();
        let sorafs_evidence_viewer_receipt_signer =
            runtime_deps.sorafs_evidence_viewer_receipt_signer.clone();
        let sorafs_evidence_viewer_erasure = runtime_deps.sorafs_evidence_viewer_erasure.clone();
        let sorafs_evidence_viewer_checkpoint_store =
            runtime_deps.sorafs_evidence_viewer_checkpoint_store.clone();
        let sorafs_evidence_viewer_compaction_archive = runtime_deps
            .sorafs_evidence_viewer_compaction_archive
            .clone();
        let sorafs_evidence_viewer_transparency_publisher = runtime_deps
            .sorafs_evidence_viewer_transparency_publisher
            .clone();
        let sorafs_potr_runtime_signer_roles =
            runtime_deps.sorafs_potr_runtime_signer_roles.clone();
        let sorafs_gateway_acme_client = runtime_deps.sorafs_gateway_acme_client.clone();
        let sorafs_reputation_journal_transaction_submitter_override = runtime_deps
            .sorafs_reputation_journal_transaction_submitter
            .clone();
        let sorafs_reputation_journal_checkpoint_provider = runtime_deps
            .sorafs_reputation_journal_checkpoint_provider
            .clone();
        let sorafs_reputation_threshold_signer =
            runtime_deps.sorafs_reputation_threshold_signer.clone();
        let sorafs_reputation_governance_dag =
            runtime_deps.sorafs_reputation_governance_dag.clone();
        let sorafs_reputation_config = config.torii.sorafs_storage.reputation_runtime.clone();
        let sorafs_reserve_transparency_config = config
            .torii
            .sorafs_storage
            .reserve_transparency_runtime
            .clone();
        let sorafs_hedging_billing_finalized_query =
            runtime_deps.sorafs_hedging_billing_finalized_query.clone();
        let sorafs_hedging_billing_journal_verifier =
            runtime_deps.sorafs_hedging_billing_journal_verifier.clone();
        let sorafs_billing_statement_signer = runtime_deps.sorafs_billing_statement_signer.clone();
        let sorafs_billing_statement_publisher =
            runtime_deps.sorafs_billing_statement_publisher.clone();
        let sorafs_billing_acknowledgement_authority = runtime_deps
            .sorafs_billing_acknowledgement_authority
            .clone();
        let sorafs_hedging_billing_epoch_witness_store = runtime_deps
            .sorafs_hedging_billing_epoch_witness_store
            .clone();
        let sorafs_hedging_billing_config =
            config.torii.sorafs_storage.hedging_billing_runtime.clone();
        let sorafs_provider_ingest_config =
            config.torii.sorafs_storage.provider_ingest_runtime.clone();
        let sorafs_provider_ingest_checkpoint_runtime = sorafs_provider_ingest_preflight
            .as_ref()
            .map(|preflight| preflight.checkpoint_runtime());
        let sorafs_por_finalized_replay_archive =
            runtime_deps.sorafs_por_finalized_replay_archive.clone();
        let sorafs_gateway_compliance_feed_transport = runtime_deps
            .sorafs_gateway_compliance_feed_transport
            .clone();
        match (
            config.torii.sorafs_gateway.compliance.as_ref(),
            sorafs_gateway_compliance_feed_transport.as_ref(),
        ) {
            (Some(_), None) => {
                return Err(Report::new(StartError::StartTorii).attach(
                    "enabled SoraFS gateway compliance requires the exact deployment-owned authenticated feed transport",
                ));
            }
            (None, Some(_)) => {
                return Err(Report::new(StartError::StartTorii).attach(
                    "disabled SoraFS gateway compliance rejects an unexpected feed transport",
                ));
            }
            (Some(_), Some(_)) | (None, None) => {}
        }
        match (
            config.torii.sorafs_gateway.acme.provider.as_ref(),
            sorafs_gateway_acme_client.as_ref(),
        ) {
            (Some(_), None) => {
                return Err(Report::new(StartError::StartTorii).attach(
                    "configured SoraFS gateway ACME automation requires the exact deployment-owned ACME client",
                ));
            }
            (None, Some(_)) => {
                return Err(Report::new(StartError::StartTorii).attach(
                    "unconfigured SoraFS gateway ACME automation rejects an unexpected client",
                ));
            }
            (Some(_), Some(_)) | (None, None) => {}
        }
        let sorafs_runtime_deps = sorafs_node::NodeRuntimeDeps::default();
        let sorafs_runtime_deps =
            if let Some(key_wrapper) = moderation_quarantine_key_wrapper.as_ref() {
                sorafs_runtime_deps.with_moderation_quarantine_key_wrapper(Arc::clone(key_wrapper))
            } else {
                sorafs_runtime_deps
            };
        let sorafs_runtime_deps = if let Some(provider) = privacy_cycle_prf_provider.as_ref() {
            sorafs_runtime_deps.with_privacy_cycle_prf_provider(Arc::clone(provider))
        } else {
            sorafs_runtime_deps
        };
        let sorafs_runtime_deps = if let Some(anchor) = privacy_release_anchor.as_ref() {
            sorafs_runtime_deps.with_privacy_release_anchor(Arc::clone(anchor))
        } else {
            sorafs_runtime_deps
        };
        let sorafs_runtime_deps =
            if let Some(provider) = transparency_leader_lease_provider.as_ref() {
                sorafs_runtime_deps.with_transparency_leader_lease_provider(Arc::clone(provider))
            } else {
                sorafs_runtime_deps
            };
        let sorafs_runtime_deps =
            if let Some(publisher) = sorafs_fenced_transparency_publisher.as_ref() {
                sorafs_runtime_deps.with_fenced_transparency_publisher(Arc::clone(publisher))
            } else {
                sorafs_runtime_deps
            };
        let sorafs_runtime_deps =
            if let Some(reader) = sorafs_fenced_transparency_head_reader.as_ref() {
                sorafs_runtime_deps.with_fenced_transparency_head_reader(Arc::clone(reader))
            } else {
                sorafs_runtime_deps
            };
        let sorafs_runtime_deps = if let Some(signer) = sorafs_governance_dag_signer.as_ref() {
            sorafs_runtime_deps.with_governance_dag_signer(Arc::clone(signer))
        } else {
            sorafs_runtime_deps
        };
        let sorafs_runtime_deps = if let Some(checkpoint_store) =
            sorafs_governance_dag_checkpoint_store.as_ref()
        {
            sorafs_runtime_deps.with_governance_dag_checkpoint_store(Arc::clone(checkpoint_store))
        } else {
            sorafs_runtime_deps
        };
        let sorafs_runtime_deps =
            if let Some(runtime) = sorafs_provider_ingest_checkpoint_runtime.as_ref() {
                sorafs_runtime_deps.with_provider_ingest_checkpoint_runtime(Arc::clone(runtime))
            } else {
                sorafs_runtime_deps
            };
        let sorafs_runtime_deps =
            if let Some(archive) = sorafs_por_finalized_replay_archive.as_ref() {
                sorafs_runtime_deps.with_por_finalized_replay_archive(Arc::clone(archive))
            } else {
                sorafs_runtime_deps
            };
        let mut sorafs_node = sorafs_node::NodeHandle::try_new_with_policies_and_runtime_deps(
            sorafs_storage_config,
            sorafs_repair_config,
            sorafs_gc_config,
            sorafs_runtime_deps,
        )
        .map_err(|err| {
            Report::new(StartError::StartTorii).attach(format!(
                "failed to initialise embedded SoraFS runtime: {err}"
            ))
        })?;
        let sorafs_provider_ingest_completed_musubi_capture = match (
            prepared_sorafs_provider_ingest_archive.as_mut(),
            sorafs_provider_ingest_config.as_ref(),
        ) {
            (Some(prepared), Some(provider_ingest_config)) => Some(
                sorafs_provider_ingest_runtime::compose_inert_completed_musubi_capture_coordinator_v1(
                    &sorafs_node,
                    prepared,
                    NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    provider_ingest_config.max_page_rows,
                )
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to reserve the inert completed-Musubi capture coordinator: {error:#}"
                    ))
                })?,
            ),
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(Report::new(StartError::StartTorii).attach(
                    "provider-ingest archive and runtime configuration diverged before inert completed-Musubi capture composition",
                ));
            }
        };
        if let Some((view, providers)) = sorafs_governance_dag_service_launch {
            let runner = sorafs_node::prepare_governance_dag_service_from_view(view, providers)
                .await
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to prepare the supervised Governance DAG service: {error}"
                    ))
                })?;
            sorafs_node
                .install_governance_dag_mirror_read_handle(runner.mirror_read_handle())
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to install the supervised Governance DAG mirror reader: {error}"
                    ))
                })?;
            let service_shutdown = supervisor.shutdown_signal();
            let child = tokio::spawn(async move {
                if let Err(error) = runner
                    .run_until(async move { service_shutdown.receive().await })
                    .await
                {
                    panic!("supervised Governance DAG service failed: {error}");
                }
            });
            supervisor.monitor(Child::new(
                child,
                OnShutdown::Wait(NODE_RUNTIME_SHUTDOWN_TIMEOUT),
            ));
        }
        let shared_sorafs_cache = build_shared_sorafs_provider_cache(&config)
            .map_err(Report::new)
            .change_context(StartError::StartTorii)?;

        let sorafs_provider_ingest_runtime = if let Some(provider_ingest_config) =
            sorafs_provider_ingest_config
        {
            let preflight = sorafs_provider_ingest_preflight.ok_or_else(|| {
                Report::new(StartError::StartTorii).attach(
                    "enabled SoraFS provider-ingest runtime has no state-free qualified adapter token",
                )
            })?;
            let (handle, child) = sorafs_provider_ingest_runtime::start(
                provider_ingest_config,
                sorafs_provider_ingest_runtime::ProviderIngestRuntimeStartArgsV1::new(
                    NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    Arc::clone(&state),
                    Arc::clone(&queue),
                    sorafs_node.clone(),
                    Arc::clone(
                        sorafs_provider_ingest_runtime_query
                            .as_ref()
                            .ok_or_else(|| {
                                Report::new(StartError::StartTorii).attach(
                                    "enabled SoraFS provider-ingest runtime has no archive-only finalized query installed in Sumeragi",
                                )
                            })?,
                    ),
                ),
                preflight,
                supervisor.shutdown_signal(),
            )
            .await
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to initialise SoraFS provider-ingest runtime: {error:#}"
                ))
            })?;
            supervisor.monitor(child);
            Some(handle)
        } else {
            None
        };
        let sorafs_reputation_runtime = if let Some(reputation_config) =
            sorafs_reputation_config.as_ref()
        {
            let trust_policy = sorafs_node.reputation_trust_policy().ok_or_else(|| {
                Report::new(StartError::StartTorii).attach(
                    "enabled committed SoraFS reputation runtime requires the configured canonical reputation trust policy",
                )
            })?;
            let prepared_archive = prepared_sorafs_reputation_archive.as_ref().ok_or_else(|| {
                Report::new(StartError::StartTorii).attach(
                    "enabled committed SoraFS reputation runtime has no daemon-owned finalized archive installed in Sumeragi",
                )
            })?;
            let reputation_archive_activation = prepared_archive.activation().clone();
            let reputation_archive_active = reputation_archive_activation
                .activation_ready()
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "daemon-owned finalized reputation archive activation gate failed before adapter construction: {error}"
                    ))
                })?;
            let query_qualification =
                sorafs_reputation_runtime::finalized_query_qualification_v1(
                    reputation_config,
                    &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    trust_policy.as_ref(),
                )
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to derive the daemon-owned finalized reputation query qualification: {error:#}"
                    ))
                })?;
            let finalized_query: Arc<
                dyn sorafs_node::reputation::runtime::ReputationFinalizedQueryV1,
            > = Arc::new(
                sorafs_reputation_finalized_query::ArchivedReputationFinalizedQueryV1::try_new(
                    reputation_config.finalized_query_handle.clone(),
                    query_qualification,
                    Arc::clone(prepared_archive.archive()),
                )
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to construct the daemon-owned finalized reputation query adapter: {error:#}"
                    ))
                })?,
            );
            let journal_transaction_submitter: Arc<
                dyn sorafs_node::reputation::runtime::ReputationJournalTransactionSubmitterV1,
            > = sorafs_reputation_journal_transaction_submitter_override.ok_or_else(|| {
                Report::new(StartError::StartTorii).attach(
                    "enabled committed SoraFS reputation runtime requires an injected authenticated journal-transaction submitter; adapting the validator node key is forbidden",
                )
            })?;
            let retention_control: Option<
                Arc<
                    dyn sorafs_reputation_finalized_query::ReputationFinalizedArchiveRetentionControlV1,
                >,
            > = prepared_archive.retention_controller().map(|control| {
                let control: Arc<
                    dyn sorafs_reputation_finalized_query::ReputationFinalizedArchiveRetentionControlV1,
                > = Arc::new(control);
                control
            });
            let dependencies = sorafs_reputation_runtime::ReputationRuntimeDependenciesV1::require(
                Some(Arc::clone(&finalized_query)),
                sorafs_reputation_journal_checkpoint_provider,
                Some(journal_transaction_submitter),
                sorafs_reputation_threshold_signer,
                sorafs_reputation_governance_dag,
                retention_control,
            )
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "enabled committed SoraFS reputation runtime has incomplete runtime-only dependencies: {error:#}"
                ))
            })?;
            let (handle, child) = if reputation_archive_active {
                sorafs_reputation_runtime::start(
                    reputation_config,
                    &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    trust_policy.as_ref(),
                    dependencies,
                    supervisor.shutdown_signal(),
                )
            } else {
                let activation_probe: sorafs_reputation_runtime::ReputationRuntimeActivationProbeV1 =
                    Arc::new(move || {
                        reputation_archive_activation
                            .activation_ready()
                            .map_err(eyre::Report::new)
                    });
                sorafs_reputation_runtime::start_deferred(
                    reputation_config,
                    &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    trust_policy.as_ref(),
                    dependencies,
                    activation_probe,
                    supervisor.shutdown_signal(),
                )
            }
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to initialise committed SoraFS reputation runtime: {error:#}"
                ))
            })?;
            supervisor.monitor(child);
            if let Some(scanner_config) = sorafs_reserve_transparency_config.as_ref() {
                let child = sorafs_reserve_transparency_runtime::start(
                    scanner_config,
                    &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                    query_qualification,
                    finalized_query,
                    Arc::clone(&state),
                    sorafs_node.clone(),
                    supervisor.shutdown_signal(),
                )
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to initialise finalized reserve transparency scanner: {error:#}"
                    ))
                })?;
                supervisor.monitor(child);
            }
            Some(handle)
        } else {
            if sorafs_reserve_transparency_config.is_some() {
                return Err(Report::new(StartError::StartTorii).attach(
                    "enabled finalized reserve transparency scanner requires the committed reputation runtime and immutable finalized archive",
                ));
            }
            None
        };
        if let (Some(reputation_runtime), Some(reputation_config)) = (
            sorafs_reputation_runtime.as_ref(),
            sorafs_reputation_config.as_ref(),
        ) {
            let admission: Arc<
                dyn sorafs_node::reputation::runtime::ReputationNativeOutcomeAdmissionApiV1,
            > = Arc::new(reputation_runtime.clone());
            let child = if sorafs_node.config().por_replay_archive_policy().is_some() {
                sorafs_por_replay_archive_runtime::start(
                    sorafs_node.clone(),
                    admission,
                    supervisor.shutdown_signal(),
                )
            } else {
                // Reuse the validated native page-item bound as the maximum
                // number of retained PoR terminals admitted by one reputation
                // worker tick. Replay archival is independent and optional.
                sorafs_por_replay_archive_runtime::start_reputation_reconciliation(
                    sorafs_node.clone(),
                    admission,
                    reputation_config.poll_interval,
                    reputation_config.page_items,
                    supervisor.shutdown_signal(),
                )
            }
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to initialise finalized PoR reputation worker: {error}"
                ))
            })?;
            supervisor.monitor(child);
        } else if sorafs_node.config().por_replay_archive_policy().is_some() {
            return Err(Report::new(StartError::StartTorii).attach(
                "enabled finalized PoR replay archival requires the committed reputation runtime",
            ));
        }
        let stream_token_reputation_admission = sorafs_reputation_runtime.as_ref().map(|runtime| {
            let admission: Arc<
                dyn sorafs_node::reputation::runtime::ReputationNativeOutcomeAdmissionApiV1,
            > = Arc::new(runtime.clone());
            admission
        });
        let sorafs_stream_token_admission_capture =
            sorafs_stream_token_gateway_runtime::prepare_capture(
                &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                &config.torii.sorafs_storage.stream_tokens,
                config
                    .torii
                    .sorafs_gateway
                    .compliance
                    .as_ref()
                    .map(|compliance| compliance.gateway_id.as_str()),
                sorafs_stream_token_gateway_admission,
                stream_token_reputation_admission,
            )
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to initialise qualified stream-token gateway admission: {error:#}"
                ))
            })?;
        if let Some(capture) = sorafs_stream_token_admission_capture.as_ref() {
            let poll_interval = sorafs_reputation_config
                .as_ref()
                .ok_or_else(|| {
                    Report::new(StartError::StartTorii).attach(
                        "enabled stream-token gateway admission requires reputation reconciliation policy",
                    )
                })?
                .poll_interval;
            let child = sorafs_stream_token_gateway_runtime::start_reconciler(
                Arc::clone(capture),
                poll_interval,
                supervisor.shutdown_signal(),
            )
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to supervise stream-token gateway reconciliation: {error:#}"
                ))
            })?;
            supervisor.monitor(child);
        }
        let sorafs_hedging_billing_runtime = if let Some(hedging_billing_config) =
            sorafs_hedging_billing_config
        {
            let feed_policy = sorafs_node
                .hedging_feed_trust_policy()
                .ok_or_else(|| {
                    Report::new(StartError::StartTorii).attach(
                        "enabled committed SoraFS hedging/billing runtime requires the configured canonical hedging-feed trust policy",
                    )
                })?;
            let service_policy = sorafs_node::load_hedging_billing_service_policy(
                &hedging_billing_config.service_policy_path,
                hedging_billing_config.service_policy_digest,
            )
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to load committed SoraFS hedging/billing service policy: {error}"
                ))
            })?;
            let dependencies =
                sorafs_hedging_billing_runtime::HedgingBillingRuntimeDependenciesV1::require(
                    sorafs_hedging_billing_finalized_query,
                    sorafs_hedging_billing_journal_verifier,
                    sorafs_billing_statement_signer,
                    sorafs_billing_statement_publisher,
                    sorafs_billing_acknowledgement_authority,
                    sorafs_hedging_billing_epoch_witness_store,
                )
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "enabled committed SoraFS hedging/billing runtime has incomplete runtime-only dependencies: {error:#}"
                    ))
                })?;
            let (handle, child) = sorafs_hedging_billing_runtime::start(
                hedging_billing_config,
                &NetworkId::from_genesis_hash(config.genesis.expected_hash),
                service_policy,
                &feed_policy,
                dependencies,
                supervisor.shutdown_signal(),
            )
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to initialise committed SoraFS hedging/billing runtime: {error:#}"
                ))
            })?;
            supervisor.monitor(child);
            Some(handle)
        } else {
            None
        };
        let sorafs_hedging_billing_torii_runtime: Option<
            Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingRuntimeApiV1>,
        > = sorafs_hedging_billing_runtime.as_ref().map(|runtime| {
            let runtime: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingRuntimeApiV1> =
                Arc::new(runtime.clone());
            runtime
        });
        if config.nexus.relay_worker.enabled {
            let relay_worker_authority =
                AccountId::new(config.common.key_pair.public_key().clone());
            let relay_worker_storage_root = config
                .kura
                .store_dir
                .resolve_relative_path()
                .join("nexus_fee_relay_worker");
            let relay_worker = nexus_fee_relay_worker::NexusFeeRelayWorker::new(
                config.nexus.relay_worker.clone(),
                nexus_fee_relay_worker::NexusFeeRelayWorkerContext {
                    storage_root: relay_worker_storage_root,
                    queue: Arc::clone(&queue),
                    state: Arc::clone(&state),
                    sumeragi: sumeragi.clone(),
                    authority: relay_worker_authority,
                    key_pair: config.common.key_pair.clone(),
                    fastpq: config.zk.fastpq.clone(),
                },
            )
            .map_err(|error| {
                Report::new(StartError::InitKura)
                    .attach(format!("failed to start Nexus fee relay worker: {error:?}"))
            })?;
            supervisor.monitor(relay_worker.start(supervisor.shutdown_signal()));
        }
        let local_validator_account_id = soracloud_runtime_mutation_signer.as_ref().map_or_else(
            || {
                AccountId::new(
                    config
                        .common
                        .trusted_peers
                        .value()
                        .myself
                        .id()
                        .public_key()
                        .clone(),
                )
            },
            |signer| signer.authority(),
        );
        let local_peer_id = config.common.trusted_peers.value().myself.id().to_string();
        let runtime_manager = SoracloudRuntimeManager::new(
            soracloud_runtime::SoracloudRuntimeManagerConfig::from_runtime_config(
                &config.soracloud_runtime,
            )
            .with_local_host_identity(local_validator_account_id, local_peer_id),
            Arc::clone(&state),
        )
        .with_sorafs_node(sorafs_node.clone())
        .with_remote_stream_token_operator_from_config(&config);
        let runtime_manager = if let Some(provider) = soracloud_hf_inference_credential_provider {
            runtime_manager.with_hf_inference_credential_provider(provider)
        } else {
            runtime_manager
        };
        let runtime_manager = if let Some(signer) = soracloud_runtime_mutation_signer {
            let runtime_mutation_sink = Arc::new(
                QueuedSoracloudRuntimeMutationSink::new(
                    Arc::clone(&queue),
                    Arc::clone(&state),
                    signer,
                    config.soracloud_runtime.submission.clone(),
                )
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to construct qualified Soracloud runtime mutation sink: {error:#}"
                    ))
                })?,
            );
            runtime_manager.with_mutation_sink(runtime_mutation_sink)
        } else {
            runtime_manager
        };
        let runtime_manager = if let Some(cache) = shared_sorafs_cache.clone() {
            runtime_manager.with_sorafs_provider_cache(cache)
        } else {
            runtime_manager
        };
        let (soracloud_runtime, child) = runtime_manager
            .start(supervisor.shutdown_signal())
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to initialise embedded Soracloud runtime manager: {error:#}"
                ))
            })?;
        state.set_soracloud_runtime(Some(Arc::new(soracloud_runtime.clone())));
        supervisor.monitor(child);
        ensure_operator_node_key_allowlisted(&mut config);
        let (kiso, child) = KisoHandle::start(config.clone());
        supervisor.monitor(child);
        // Subscribe before Torii exposes the configuration update endpoint.
        // A watch update published after Kiso starts is then retained even if
        // the relay task has not begun polling yet.
        let config_update_receivers = ConfigUpdateReceivers {
            log_level: kiso.subscribe_on_logger_updates().await.map_err(|error| {
                Report::new(StartError::StartTorii)
                    .attach(format!("failed to subscribe to logger updates: {error}"))
            })?,
            acl: kiso
                .subscribe_on_network_acl_updates()
                .await
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to subscribe to network ACL updates: {error}"
                    ))
                })?,
            handshake: kiso
                .subscribe_on_soranet_handshake_updates()
                .await
                .map_err(|error| {
                    Report::new(StartError::StartTorii).attach(format!(
                        "failed to subscribe to SoraNet handshake updates: {error}"
                    ))
                })?,
        };

        let receipt_signer = torii_receipt_signer_or_derived(
            config.torii.receipt_signer.clone(),
            &config.common.key_pair,
        )
        .map_err(|err| {
            Report::new(StartError::StartTorii)
                .attach(format!("failed to derive Torii receipt signer: {err}"))
        })?;
        let vpn_relay_trust = if config.network.soranet_vpn.enabled {
            let vpn = &config.network.soranet_vpn;
            let snapshot_path = vpn.guard_directory_path.as_ref().ok_or_else(|| {
                Report::new(StartError::StartTorii)
                    .attach("VPN guard directory path missing after configuration validation")
            })?;
            let snapshot =
                iroha_crypto::soranet::directory::read_guard_directory_snapshot_file(snapshot_path)
                    .map_err(|error| {
                        Report::new(StartError::StartTorii).attach(format!(
                            "failed to read VPN guard directory {}: {error}",
                            snapshot_path.display()
                        ))
                    })?;
            let expected_digest = vpn.guard_directory_digest.ok_or_else(|| {
                Report::new(StartError::StartTorii)
                    .attach("VPN guard directory digest missing after configuration validation")
            })?;
            let relay_id = vpn.relay_id.ok_or_else(|| {
                Report::new(StartError::StartTorii)
                    .attach("VPN relay identity missing after configuration validation")
            })?;
            let at_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| {
                    Report::new(StartError::StartTorii)
                        .attach(format!("system clock precedes Unix epoch: {error}"))
                })?
                .as_secs()
                .try_into()
                .map_err(|_| {
                    Report::new(StartError::StartTorii).attach("current Unix time exceeds i64::MAX")
                })?;
            let trust = iroha_torii::VpnRelayTrust::from_guard_directory_at(
                &snapshot,
                expected_digest,
                relay_id,
                at_unix,
            )
            .map_err(|error| Report::new(StartError::StartTorii).attach(error))?;
            Some(trust)
        } else {
            None
        };
        let musubi_publication_context =
            musubi_publication_service::MusubiPublicationPrivateServiceContextV1::new(
                NetworkId::from_genesis_hash(config.genesis.expected_hash),
                Arc::clone(&state),
                Arc::clone(&queue),
                sorafs_node.clone(),
            );
        let runtime_deps = iroha_torii::ToriiRuntimeDeps::new(torii_telemetry)
            .with_soracloud_runtime(Arc::new(soracloud_runtime.clone()))
            .with_soracloud_hf_config(config.soracloud_runtime.hf.clone())
            .with_sorafs_node(sorafs_node)
            .with_torii_proxy_bridge_signer(config.common.key_pair.clone())
            .with_vpn_helper_ticket_secret(config.network.soranet_vpn.helper_ticket_secret)
            .with_vpn_relay_trust(vpn_relay_trust);
        let runtime_deps = if let Some(registry) = bootle_lantern_issuance_provider_registry {
            runtime_deps.with_bootle_lantern_issuance_provider_registry(registry)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(runtime) = sorafs_reputation_runtime.as_ref() {
            let reader: Arc<dyn sorafs_node::reputation::runtime::ReputationCommittedReadApiV1> =
                Arc::new(ReadyReputationCommittedReaderV1 {
                    runtime: runtime.clone(),
                });
            runtime_deps.with_sorafs_reputation_committed_reader(reader)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_proof_outcome_signer {
            runtime_deps.with_sorafs_proof_outcome_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_repair_transaction_signer {
            runtime_deps.with_sorafs_repair_transaction_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_reserve_transaction_signer {
            runtime_deps.with_sorafs_reserve_transaction_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_orderbook_transaction_signer {
            runtime_deps.with_sorafs_orderbook_transaction_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_moderation_transaction_signer {
            runtime_deps.with_sorafs_moderation_transaction_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_stream_token_signer {
            runtime_deps.with_sorafs_stream_token_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(capture) = sorafs_stream_token_admission_capture {
            runtime_deps.with_sorafs_stream_token_admission_capture(capture)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(boundary) = sorafs_moderation_settlement_handoff {
            runtime_deps.with_sorafs_moderation_settlement_handoff(boundary)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(boundary) = sorafs_moderation_publication_handoff {
            runtime_deps.with_sorafs_moderation_publication_handoff(boundary)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(boundary) = sorafs_moderation_panel_notification {
            runtime_deps.with_sorafs_moderation_panel_notification(boundary)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(archive) = sorafs_moderation_panel_notification_archive {
            runtime_deps.with_sorafs_moderation_panel_notification_archive(archive)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(checkpoint_store) = sorafs_moderation_checkpoint_store {
            runtime_deps.with_sorafs_moderation_checkpoint_store(checkpoint_store)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(runtime) = sorafs_appeal_finance_checkpoint_runtime {
            runtime_deps.with_sorafs_appeal_finance_checkpoint_runtime(runtime)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(boundary) = sorafs_evidence_viewer_webauthn {
            runtime_deps.with_sorafs_evidence_viewer_webauthn(boundary)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(boundary) = sorafs_evidence_viewer_grants {
            runtime_deps.with_sorafs_evidence_viewer_grants(boundary)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signer) = sorafs_evidence_viewer_receipt_signer {
            runtime_deps.with_sorafs_evidence_viewer_receipt_signer(signer)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(boundary) = sorafs_evidence_viewer_erasure {
            runtime_deps.with_sorafs_evidence_viewer_erasure(boundary)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(checkpoint_store) = sorafs_evidence_viewer_checkpoint_store {
            runtime_deps.with_sorafs_evidence_viewer_checkpoint_store(checkpoint_store)
        } else {
            runtime_deps
        };
        let runtime_deps =
            if let Some(compaction_archive) = sorafs_evidence_viewer_compaction_archive {
                runtime_deps.with_sorafs_evidence_viewer_compaction_archive(compaction_archive)
            } else {
                runtime_deps
            };
        let runtime_deps = if let Some(publisher) = sorafs_evidence_viewer_transparency_publisher {
            runtime_deps.with_sorafs_evidence_viewer_transparency_publisher(publisher)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(runtime) = sorafs_pop_credentials {
            runtime_deps.with_sorafs_pop_credentials(runtime)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(cache) = shared_sorafs_cache {
            runtime_deps.with_sorafs_cache(cache)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(key_wrapper) = moderation_quarantine_key_wrapper {
            runtime_deps.with_sorafs_moderation_quarantine_key_wrapper(key_wrapper)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(client) = sorafs_gateway_acme_client {
            runtime_deps.with_sorafs_gateway_acme_client(client)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(transport) = sorafs_gateway_compliance_feed_transport {
            runtime_deps.with_sorafs_gateway_compliance_feed_transport(transport)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(signers) = sorafs_appeal_finance_runtime_signers {
            runtime_deps.with_sorafs_appeal_finance_runtime_signers(signers)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(roles) = sorafs_potr_runtime_signer_roles {
            runtime_deps.with_sorafs_potr_runtime_signer_roles(roles)
        } else {
            runtime_deps
        };
        let runtime_deps = if let Some(runtime) = sorafs_hedging_billing_torii_runtime {
            runtime_deps.with_sorafs_hedging_billing_runtime(runtime)
        } else {
            runtime_deps
        };
        let queue_backpressure = queue.backpressure_handle();
        // Start proof lanes before Torii begins accepting submissions so one-time GPU setup happens
        // during node startup instead of the first hot-path transaction burst.
        if let Some((_h, child)) = iroha_core::pipeline::zk_lane::start(&zk_cfg.halo2) {
            supervisor.monitor(Child::new(child, OnShutdown::Wait(Duration::from_secs(1))));
        }
        if let Some((_h, child)) = iroha_core::fastpq::lane::start_with_backpressure(
            &zk_cfg.fastpq,
            Some(queue_backpressure),
            Some(kura.clone()),
        ) {
            supervisor.monitor(Child::new(child, OnShutdown::Wait(Duration::from_secs(1))));
        }
        let torii = Torii::new_with_handle(
            config.common.chain.clone(),
            NetworkId::from_genesis_hash(config.genesis.expected_hash),
            kiso.clone(),
            config.torii,
            queue,
            events_sender,
            live_query_store,
            kura.clone(),
            state.clone(),
            receipt_signer,
            include!("main/online_peers_provider.rs"),
            Some(sumeragi.clone()),
            runtime_deps,
        );
        let torii = torii.with_p2p(network.clone());
        let torii = torii.with_local_peer_id(config.common.peer.id.clone());
        let torii_run = torii.start(supervisor.shutdown_signal());
        let shutdown_on_failure = supervisor.shutdown_signal();
        supervisor.monitor(Child::new(
            tokio::spawn(async move {
                if let Err(err) = Box::pin(torii_run).await {
                    iroha_logger::error!(?err, "Torii failed to terminate gracefully");
                    shutdown_on_failure.send();
                    std::process::exit(1);
                } else {
                    iroha_logger::debug!("Torii exited normally");
                }
            }),
            OnShutdown::Wait(Duration::from_secs(5)),
        ));

        let network_relay_shutdown = supervisor.shutdown_signal();
        supervisor.monitor(task::spawn(
            NetworkRelay {
                sumeragi,
                tx_gossiper,
                peers_gossiper,
                network: network.clone(),
                streaming: streaming.clone(),
                consensus_ingress: ConsensusIngressLimiter::from_config(
                    &config.network,
                    Duration::from_millis(signed_block_cadence_ms),
                ),
                low_priority_ingress: LowPriorityIngressLimiter::from_config(&config.network),
                #[cfg(feature = "test-network-message-control")]
                test_message_control,
            }
            .run(network_relay_shutdown),
        ));
        // Start Network Time Service sampler with config parameters
        let (_nts_peers_tx, nts_peers_rx) =
            tokio::sync::watch::channel(std::collections::BTreeSet::new());
        iroha_core::time::start_with_params(
            network.clone(),
            nts_peers_rx,
            iroha_core::time::Params::from(&config.nts),
        );
        // Observer nodes are configured with `NodeRole::Observer`; Sumeragi suppresses
        // local consensus emissions in that case, so observers follow the chain and
        // serve queries without proposing or voting. Validators retain the full duties.

        let net_for_relay = network.clone();
        // The relay retains `kiso`, so its watch senders cannot close through ordinary
        // handle loss. Any pre-shutdown return means the supervised configuration
        // authority failed; restarting only this relay would preserve stale ACL or
        // handshake policy and is therefore deliberately not recoverable.
        let confidential_gas = config.confidential.gas;
        supervisor.monitor(tokio::task::spawn(async move {
            if let Err(err) = config_updates_relay(
                kiso,
                logger,
                net_for_relay,
                confidential_gas,
                config_update_receivers,
            )
            .await
            {
                iroha_logger::error!(?err, "Config updates relay exited");
            }
        }));

        supervisor
            .setup_shutdown_on_os_signals()
            .change_context(StartError::ListenOsSignal)?;

        let (_availability, publication_child) = musubi_publication_service::
            build_and_start_injected_musubi_publication_private_service_v1(
                musubi_publication_factory,
                musubi_publication_context,
                supervisor.shutdown_signal(),
            )
            .map_err(|error| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to assemble private Musubi publication service: {error}"
                ))
            })?;
        if let Some(child) = publication_child {
            supervisor.monitor(child);
        }

        supervisor.shutdown_on_external_signal(shutdown_signal);

        Ok((
            Self {
                kura,
                state,
                soracloud_runtime,
                streaming: streaming.clone(),
                network: network.clone(),
                sorafs_reputation_runtime,
                sorafs_hedging_billing_runtime,
                sorafs_provider_ingest_runtime,
                sorafs_provider_ingest_finalized_query,
                sorafs_provider_ingest_completed_musubi_capture,
            },
            async move {
                supervisor.start().await?;
                iroha_logger::info!("Iroha shutdown normally");
                Ok(())
            },
        ))
    }

    /// Read-only handle to the world state view.
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }

    /// Access to the block storage handle.
    pub fn kura(&self) -> &Arc<Kura> {
        &self.kura
    }

    /// Access the daemon-owned archive-only provider-ingest finalized query.
    pub fn sorafs_provider_ingest_finalized_query(
        &self,
    ) -> Option<&Arc<sorafs_provider_ingest_finalized_query::ArchivedProviderIngestFinalizedLedgerV1>>
    {
        self.sorafs_provider_ingest_finalized_query.as_ref()
    }

    /// Access the embedded Soracloud runtime-manager handle.
    pub fn soracloud_runtime(&self) -> &SoracloudRuntimeManagerHandle {
        &self.soracloud_runtime
    }

    /// Streaming handle used for Torii and telemetry ingress.
    pub fn streaming(&self) -> iroha_core::streaming::StreamingHandle {
        self.streaming.clone()
    }

    /// Access the supervised committed `SoraFS` reputation status/metrics handle.
    #[must_use]
    pub fn sorafs_reputation_runtime(
        &self,
    ) -> Option<&sorafs_reputation_runtime::ReputationRuntimeHandleV1> {
        self.sorafs_reputation_runtime.as_ref()
    }

    /// Access the supervised committed `SoraFS` hedging/billing status and
    /// metrics handle.
    #[must_use]
    pub fn sorafs_hedging_billing_runtime(
        &self,
    ) -> Option<&sorafs_hedging_billing_runtime::HedgingBillingRuntimeHandleV1> {
        self.sorafs_hedging_billing_runtime.as_ref()
    }

    /// Access the supervised finalized-ledger `SoraFS` provider-ingest status
    /// and metrics handle.
    #[must_use]
    pub fn sorafs_provider_ingest_runtime(
        &self,
    ) -> Option<&sorafs_provider_ingest_runtime::ProviderIngestRuntimeHandleV1> {
        self.sorafs_provider_ingest_runtime.as_ref()
    }

    /// Construct a manifest publisher for the active network.
    pub fn manifest_publisher(&self) -> ManifestPublisher<IrohaNetwork> {
        ManifestPublisher::new(self.streaming.clone(), self.network.clone())
    }
}

fn configure_soranet_transport(
    streaming: &mut iroha_core::streaming::StreamingHandle,
    soranet: &iroha_config::parameters::actual::StreamingSoranet,
) -> ReportResult<(), StartError> {
    if !soranet.enabled {
        streaming.set_soranet_transport(None);
        return Ok(());
    }

    let spool_dir = soranet.provision_spool_dir.clone();
    fs::create_dir_all(&spool_dir).map_err(|err| {
        Report::new(err)
            .change_context(StartError::StartP2p)
            .attach(format!(
                "failed to initialize SoraNet provision spool directory {}",
                spool_dir.display()
            ))
    })?;

    let mut provisioner =
        FilesystemSoranetProvisioner::new(spool_dir, soranet.provision_spool_max_bytes.get());
    #[cfg(feature = "telemetry")]
    if let Some(telemetry) = streaming.telemetry_handle() {
        provisioner = provisioner.with_telemetry(telemetry);
    }
    streaming.set_soranet_transport(Some(Arc::new(provisioner)));
    Ok(())
}

#[cfg(feature = "telemetry")]
async fn start_telemetry(
    logger: &LoggerHandle,
    config: &Config,
    supervisor: &mut Supervisor,
) -> ReportResult<(), StartError> {
    const MSG_SUBSCRIBE: &str = "unable to subscribe to the channel";
    const MSG_START_TASK: &str = "unable to start the task";

    let telemetry_profile = if config.telemetry_enabled {
        config.telemetry_profile
    } else {
        iroha_config::parameters::actual::TelemetryProfile::Disabled
    };
    let telemetry_capabilities = telemetry_profile.capabilities();

    if !telemetry_capabilities.metrics_enabled() {
        iroha_logger::info!(
            ?telemetry_profile,
            "Telemetry metrics disabled by profile; skipping sinks",
        );
        return Ok(());
    }

    #[cfg(feature = "dev-telemetry")]
    {
        if telemetry_capabilities.developer_outputs_enabled() {
            if let Some(out_file) = &config.dev_telemetry.out_file {
                let receiver = logger
                    .subscribe_on_telemetry(iroha_logger::telemetry::Channel::Future)
                    .await
                    .change_context(StartError::StartDevTelemetry)
                    .attach(MSG_SUBSCRIBE)?;
                let handle = iroha_telemetry::dev::start_file_output(
                    out_file.resolve_relative_path(),
                    receiver,
                )
                .await
                .map_err(|err| {
                    Report::new(StartError::StartDevTelemetry)
                        .attach(MSG_START_TASK)
                        .attach(err)
                })?;
                supervisor.monitor(handle);
            }
        } else {
            iroha_logger::debug!(
                ?telemetry_profile,
                "Developer telemetry outputs disabled by profile",
            );
        }
    }

    if let Some(telemetry_cfg) = &config.telemetry {
        let receiver = logger
            .subscribe_on_telemetry(iroha_logger::telemetry::Channel::Regular)
            .await
            .change_context(StartError::StartTelemetry)
            .attach(MSG_SUBSCRIBE)?;
        let handle = iroha_telemetry::ws::start(
            telemetry_cfg.clone(),
            config.telemetry_integrity.clone(),
            receiver,
        )
        .await
        .map_err(|err| {
            Report::new(StartError::StartTelemetry)
                .attach(MSG_START_TASK)
                .attach(err)
        })?;
        supervisor.monitor(handle);
        #[cfg(feature = "telegram-alerts")]
        if telemetry_capabilities.developer_outputs_enabled()
            && telemetry_cfg.telegram_bot_key.is_some()
            && telemetry_cfg.telegram_chat_id.is_some()
        {
            let chain_id_str = config.common.chain.to_string();
            let receiver = logger
                .subscribe_on_telemetry(iroha_logger::telemetry::Channel::Regular)
                .await
                .change_context(StartError::StartTelemetry)
                .attach(MSG_SUBSCRIBE)?;
            let metrics_url = telemetry_cfg.telegram_metrics_url.clone().or_else(|| {
                let addr = config.torii.address.value().to_string();
                url::Url::parse(&format!("http://{}/metrics", addr)).ok()
            });
            let mut cfg = telemetry_cfg.clone();
            cfg.telegram_metrics_url = metrics_url;
            match iroha_telemetry::telegram::start_with_context(cfg, Some(chain_id_str), receiver)
                .await
            {
                Ok(h) => supervisor.monitor(h),
                Err(e) => iroha_logger::warn!(%e, "Failed to start Telegram alerts"),
            }
        }
        iroha_logger::info!("Telemetry started");
        Ok(())
    } else {
        iroha_logger::info!("Telemetry not started due to absent configuration");
        Ok(())
    }
}

/// Relays local configuration-actor updates to local runtime components.
///
/// Security-sensitive handshake policy is never accepted from, or propagated
/// to, remote peers. Every node derives it exclusively from its own
/// operator-controlled configuration.
struct ConfigUpdateReceivers {
    log_level: tokio::sync::watch::Receiver<iroha_config::parameters::actual::Logger>,
    acl: tokio::sync::watch::Receiver<iroha_config::client_api::NetworkAcl>,
    handshake: tokio::sync::watch::Receiver<iroha_config::parameters::actual::SoranetHandshake>,
}

#[allow(clippy::too_many_lines)]
async fn config_updates_relay(
    _kiso: KisoHandle,
    logger: LoggerHandle,
    network: iroha_core::IrohaNetwork,
    confidential_gas: iroha_config::parameters::actual::ConfidentialGas,
    config_update_receivers: ConfigUpdateReceivers,
) -> EyreResult<()> {
    #[cfg(not(feature = "telemetry"))]
    let _ = confidential_gas;

    let mut log_level_update = config_update_receivers.log_level;
    let mut acl_update = config_update_receivers.acl;
    let mut handshake_update = config_update_receivers.handshake;
    #[cfg(feature = "telemetry")]
    let confidential_metrics_handle = iroha_telemetry::metrics::global().cloned();
    #[cfg(feature = "telemetry")]
    if let Some(metrics) = confidential_metrics_handle.as_ref() {
        metrics.set_confidential_gas_schedule(&confidential_gas);
        let digest = ivm::gas::schedule_hash();
        metrics.set_ivm_gas_schedule_hash(digest.as_ref());
    }
    // The network actor was constructed from this same current snapshot. Wait
    // for an actual Kiso change before rebuilding the runtime handshake state;
    // replaying the initial value would try to reopen its already-locked
    // persistent ticket-revocation store.

    // See https://github.com/tokio-rs/tokio/issues/5616 and
    // https://github.com/rust-lang/rust-clippy/issues/10636
    #[cfg(feature = "telemetry")]
    #[allow(clippy::redundant_pub_crate)]
    loop {
        tokio::select! {
            result = log_level_update.changed() => {
                if let Ok(()) = result {
                    let value = log_level_update.borrow_and_update().clone();
                    if let Err(error) = logger.reload_level(value.resolve_filter()).await {
                        iroha_logger::error!("Failed to reload log level: {error}");
                    }
                } else {
                    iroha_logger::debug!("Exiting config updates relay (log level channel closed)");
                    break;
                }
            },
            result = acl_update.changed() => {
                if let Ok(()) = result {
                    let value = acl_update.borrow_and_update().clone();
                    let update = iroha_p2p::network::message::UpdateAcl {
                        allowlist_only: value.allowlist_only.unwrap_or(false),
                        allow_keys: value.allow_keys.clone().unwrap_or_default(),
                        deny_keys: value.deny_keys.clone().unwrap_or_default(),
                        allow_cidrs: value.allow_cidrs.clone().unwrap_or_default(),
                        deny_cidrs: value.deny_cidrs.clone().unwrap_or_default(),
                    };
                    network.update_acl(update);
                } else {
                    iroha_logger::debug!("Exiting config updates relay (ACL channel closed)");
                    break;
                }
            },
            result = handshake_update.changed() => {
                if let Ok(()) = result {
                    let value = handshake_update.borrow_and_update().clone();
                    network.update_soranet_handshake(value);
                } else {
                    iroha_logger::debug!("Exiting config updates relay (handshake channel closed)");
                    break;
                }
            },
        };
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(clippy::redundant_pub_crate)]
    loop {
        tokio::select! {
            result = log_level_update.changed() => {
                if let Ok(()) = result {
                    let value = log_level_update.borrow_and_update().clone();
                    if let Err(error) = logger.reload_level(value.resolve_filter()).await {
                        iroha_logger::error!("Failed to reload log level: {error}");
                    }
                } else {
                    iroha_logger::debug!("Exiting config updates relay (log level channel closed)");
                    break;
                }
            },
            result = acl_update.changed() => {
                if let Ok(()) = result {
                    let value = acl_update.borrow_and_update().clone();
                    let update = iroha_p2p::network::message::UpdateAcl {
                        allowlist_only: value.allowlist_only.unwrap_or(false),
                        allow_keys: value.allow_keys.clone().unwrap_or_default(),
                        deny_keys: value.deny_keys.clone().unwrap_or_default(),
                        allow_cidrs: value.allow_cidrs.clone().unwrap_or_default(),
                        deny_cidrs: value.deny_cidrs.clone().unwrap_or_default(),
                    };
                    network.update_acl(update);
                } else {
                    iroha_logger::debug!("Exiting config updates relay (ACL channel closed)");
                    break;
                }
            },
            result = handshake_update.changed() => {
                if let Ok(()) = result {
                    let value = handshake_update.borrow_and_update().clone();
                    network.update_soranet_handshake(value);
                } else {
                    iroha_logger::debug!("Exiting config updates relay (handshake channel closed)");
                    break;
                }
            },
        };
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StartupTrustRoot {
    /// Normal startup is bound to one exact signed genesis instance.
    Genesis(ResolvedGenesisTrustAnchor),
    /// An audited imported snapshot must authenticate its independent lineage before replay.
    AuthenticatedSnapshotPending,
}

impl StartupTrustRoot {
    fn resolve(
        authenticated_snapshot_pending: bool,
        public_key: &PublicKey,
        configured_hash: HashOf<BlockHeader>,
        local_genesis: Option<&GenesisBlock>,
    ) -> ReportResult<Self, StartError> {
        if authenticated_snapshot_pending {
            return Ok(Self::AuthenticatedSnapshotPending);
        }
        ResolvedGenesisTrustAnchor::resolve(public_key, configured_hash, local_genesis)
            .map(Self::Genesis)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedGenesisTrustAnchor {
    public_key: PublicKey,
    consensus_header_hash: HashOf<BlockHeader>,
}

fn load_deferred_normal_startup_genesis(
    authenticated_snapshot_pending: bool,
    genesis: Option<GenesisBlock>,
    signed_file: Option<&WithOrigin<PathBuf>>,
) -> ReportResult<Option<GenesisBlock>, StartError> {
    if authenticated_snapshot_pending || genesis.is_some() {
        return Ok(genesis);
    }
    let Some(signed_file) = signed_file else {
        return Ok(None);
    };
    read_genesis(&signed_file.resolve_relative_path())
        .attach(signed_file.clone().into_attachment().display_path())
        .change_context(StartError::InitKura)
        .map(Some)
}

impl ResolvedGenesisTrustAnchor {
    fn resolve(
        public_key: &PublicKey,
        configured_hash: HashOf<BlockHeader>,
        local_genesis: Option<&GenesisBlock>,
    ) -> ReportResult<Self, StartError> {
        if let Some(local) = local_genesis.map(|genesis| genesis.0.hash())
            && configured_hash != local
        {
            return Err(Report::new(StartError::InitKura).attach(format!(
                "local signed genesis hash {local} differs from configured genesis.expected_hash {configured_hash}"
            )));
        }
        let anchor = Self {
            public_key: public_key.clone(),
            consensus_header_hash: configured_hash,
        };
        if let Some(local_genesis) = local_genesis {
            anchor.verify(local_genesis)?;
        }
        Ok(anchor)
    }

    fn verify(&self, block: &GenesisBlock) -> ReportResult<(), StartError> {
        let embedded_key = genesis_public_key_from_genesis_block(block)?;
        if embedded_key != self.public_key {
            return Err(Report::new(StartError::InitKura).attach(format!(
                "genesis authority `{embedded_key}` does not match configured genesis.public_key `{}`",
                self.public_key
            )));
        }

        let block_hash = block.0.hash();
        if block_hash != self.consensus_header_hash {
            return Err(Report::new(StartError::InitKura).attach(format!(
                "genesis hash {block_hash} does not match the resolved genesis trust-anchor hash {}",
                self.consensus_header_hash
            )));
        }

        let mut signatures = block.0.signatures();
        let signature = signatures.next().ok_or_else(|| {
            Report::new(StartError::InitKura)
                .attach("genesis block has no configured-authority signature")
        })?;
        if signature.index() != 0 || signatures.next().is_some() {
            return Err(Report::new(StartError::InitKura)
                .attach("genesis block must have exactly one signature at index 0"));
        }
        signature
            .signature()
            .verify_hash(&self.public_key, block_hash)
            .map_err(|error| {
                Report::new(StartError::InitKura).attach(format!(
                    "genesis block signature does not verify against configured genesis.public_key `{}`: {error}",
                    self.public_key
                ))
            })?;

        Ok(())
    }
}

fn read_stored_genesis_block(
    kura: &Kura,
    block_count: iroha_core::kura::BlockCount,
    provisional_imported_prefix: bool,
) -> ReportResult<Option<GenesisBlock>, StartError> {
    if block_count.0 == 0 {
        return Ok(None);
    }
    let nz = std::num::NonZeroUsize::new(1).expect("nonzero");
    if provisional_imported_prefix {
        if kura.get_durable_block_hash(nz).is_none() {
            return Err(Report::new(StartError::InitKura)
                .attach("provisional imported prefix has no durable height-one hash anchor"));
        }
        return Ok(None);
    }
    let Some(stored) = kura.get_block(nz) else {
        return Err(Report::new(StartError::InitKura)
            .attach("non-empty block store is missing genesis block at height 1"));
    };
    Ok(Some(GenesisBlock((*stored).clone())))
}

fn genesis_public_key_from_genesis_block(
    block: &GenesisBlock,
) -> ReportResult<PublicKey, StartError> {
    let first = block.0.external_transactions().next().ok_or_else(|| {
        Report::new(StartError::InitKura).attach("stored genesis block contains no transactions")
    })?;
    let authority = first.authority();
    authority.try_signatory().cloned().ok_or_else(|| {
        Report::new(StartError::InitKura)
            .attach("stored genesis transaction authority is not a single-key account")
    })
}

fn genesis_account(public_key: PublicKey) -> Account {
    let genesis_account_id = AccountId::new(public_key);
    Account::new(genesis_account_id.clone()).build(&genesis_account_id)
}

fn genesis_domain(public_key: PublicKey) -> Domain {
    let genesis_account_id = AccountId::new(public_key);
    Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id)
}

#[cfg(test)]
mod genesis_key_tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_genesis::GenesisBuilder;
    use std::path::PathBuf;

    fn signed_genesis(keypair: &KeyPair) -> GenesisBlock {
        GenesisBuilder::new_without_executor(
            ChainId::from("configured-genesis-trust-anchor-test"),
            PathBuf::from("."),
        )
        .build_raw()
        .build_and_sign(keypair)
        .expect("build signed genesis")
    }

    fn signed_genesis_with_marker(keypair: &KeyPair, marker: &str) -> GenesisBlock {
        GenesisBuilder::new_without_executor(
            ChainId::from("configured-genesis-trust-anchor-test"),
            PathBuf::from("."),
        )
        .append_instruction(iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            marker.to_owned(),
        ))
        .build_raw()
        .build_and_sign(keypair)
        .expect("build marked signed genesis")
    }

    fn write_signed_genesis(path: &Path, genesis: &GenesisBlock) {
        let bytes = {
            let _registry_guard = instruction_registry_test_guard();
            init_genesis_instruction_registry();
            genesis.0.encode_wire().expect("encode signed genesis")
        };
        fs::write(path, bytes).expect("write signed genesis");
    }

    #[test]
    fn derives_genesis_pubkey_from_block_authority() {
        let chain = ChainId::from("derive-genesis-pubkey-test");
        let manifest = GenesisBuilder::new_without_executor(chain, PathBuf::from(".")).build_raw();
        let keypair = iroha_crypto::KeyPair::random();
        let genesis_block = manifest
            .build_and_sign(&keypair)
            .expect("build genesis block");
        let derived =
            genesis_public_key_from_genesis_block(&genesis_block).expect("derive genesis pubkey");
        assert_eq!(&derived, keypair.public_key());
    }

    #[test]
    fn genesis_domain_owner_matches_genesis_authority() {
        let keypair = iroha_crypto::KeyPair::random();
        let expected_owner = AccountId::new(keypair.public_key().clone());
        let domain = genesis_domain(keypair.public_key().clone());

        assert_eq!(domain.owned_by(), &expected_owner);
    }

    #[test]
    fn resolved_genesis_trust_anchor_accepts_matching_block() {
        let keypair = KeyPair::random();
        let genesis = signed_genesis(&keypair);
        let anchor = ResolvedGenesisTrustAnchor {
            public_key: keypair.public_key().clone(),
            consensus_header_hash: genesis.0.hash(),
        };

        anchor
            .verify(&genesis)
            .expect("matching configured genesis trust anchor should verify");
    }

    #[test]
    fn resolved_genesis_trust_anchor_rejects_wrong_public_key() {
        let signer = KeyPair::random();
        let genesis = signed_genesis(&signer);
        let anchor = ResolvedGenesisTrustAnchor {
            public_key: KeyPair::random().public_key().clone(),
            consensus_header_hash: genesis.0.hash(),
        };

        let error = anchor
            .verify(&genesis)
            .expect_err("configured public-key mismatch must reject genesis");
        assert!(matches!(error.current_context(), StartError::InitKura));
        assert!(
            format!("{error:?}").contains("does not match configured genesis.public_key"),
            "unexpected mismatch diagnostic: {error:?}"
        );
    }

    #[test]
    fn resolved_genesis_trust_anchor_rejects_wrong_hash() {
        let keypair = KeyPair::random();
        let genesis = signed_genesis(&keypair);
        let wrong_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
        assert_ne!(genesis.0.hash(), wrong_hash);
        let anchor = ResolvedGenesisTrustAnchor {
            public_key: keypair.public_key().clone(),
            consensus_header_hash: wrong_hash,
        };

        let error = anchor
            .verify(&genesis)
            .expect_err("configured genesis hash mismatch must reject genesis");
        assert!(matches!(error.current_context(), StartError::InitKura));
        assert!(
            format!("{error:?}").contains("does not match the resolved genesis trust-anchor hash"),
            "unexpected mismatch diagnostic: {error:?}"
        );
    }

    #[test]
    fn startup_trust_root_requires_local_artifact_to_match_configured_hash() {
        let keypair = KeyPair::random();
        let genesis = signed_genesis(&keypair);

        let root = StartupTrustRoot::resolve(
            false,
            keypair.public_key(),
            genesis.0.hash(),
            Some(&genesis),
        )
        .expect("the local signed genesis matches the independently configured hash");
        let StartupTrustRoot::Genesis(anchor) = root else {
            panic!("normal startup must resolve a genesis root");
        };

        assert_eq!(anchor.consensus_header_hash, genesis.0.hash());
        anchor.verify(&genesis).expect("resolved anchor verifies");
    }

    #[test]
    fn normal_startup_loads_genesis_deferred_by_snapshot_policy() {
        let keypair = KeyPair::random();
        let genesis = signed_genesis(&keypair);
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("genesis.nrt");
        write_signed_genesis(&path, &genesis);
        let signed_file = WithOrigin::inline(path);

        let loaded = load_deferred_normal_startup_genesis(false, None, Some(&signed_file))
            .expect("normal startup reads the protected local artifact")
            .expect("normal startup loads a genesis block");

        assert_eq!(loaded.0.hash(), genesis.0.hash());
    }

    #[test]
    fn provisional_snapshot_startup_does_not_read_deferred_genesis() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let missing = WithOrigin::inline(temp.path().join("missing-genesis.nrt"));

        let loaded = load_deferred_normal_startup_genesis(true, None, Some(&missing))
            .expect("provisional snapshot startup defers to authenticated snapshot lineage");

        assert!(loaded.is_none());
    }

    #[test]
    fn snapshot_startup_selects_the_independent_pending_trust_root() {
        let keypair = KeyPair::random();
        let configured_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x5A; 32]));

        let root = StartupTrustRoot::resolve(true, keypair.public_key(), configured_hash, None)
            .expect("an authenticated provisional snapshot is an independent trust root");

        assert!(matches!(
            root,
            StartupTrustRoot::AuthenticatedSnapshotPending
        ));
    }

    #[test]
    fn resolver_rejects_config_and_local_hash_disagreement() {
        let keypair = KeyPair::random();
        let genesis = signed_genesis(&keypair);
        let configured_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xB6; 32]));
        assert_ne!(configured_hash, genesis.0.hash());

        let error =
            StartupTrustRoot::resolve(false, keypair.public_key(), configured_hash, Some(&genesis))
                .expect_err("two exact genesis sources must agree");

        assert!(matches!(error.current_context(), StartError::InitKura));
        assert!(
            format!("{error:?}").contains("differs from configured genesis.expected_hash"),
            "unexpected conflicting-anchor diagnostic: {error:?}"
        );
    }

    #[test]
    fn resolver_accepts_matching_config_and_local_hashes() {
        let keypair = KeyPair::random();
        let genesis = signed_genesis(&keypair);

        let root = StartupTrustRoot::resolve(
            false,
            keypair.public_key(),
            genesis.0.hash(),
            Some(&genesis),
        )
        .expect("matching configured and local hashes resolve one exact anchor");
        let StartupTrustRoot::Genesis(anchor) = root else {
            panic!("normal startup must resolve a genesis root");
        };

        assert_eq!(anchor.consensus_header_hash, genesis.0.hash());
    }

    #[test]
    fn configured_anchor_rejects_alternate_same_key_same_chain_genesis_with_local_body() {
        let keypair = KeyPair::random();
        let trusted = signed_genesis_with_marker(&keypair, "trusted genesis");
        let alternate = signed_genesis_with_marker(&keypair, "alternate genesis");
        assert_ne!(trusted.0.hash(), alternate.0.hash());

        let root = StartupTrustRoot::resolve(
            false,
            keypair.public_key(),
            trusted.0.hash(),
            Some(&trusted),
        )
        .expect("the local trusted genesis matches the configured exact anchor");
        let StartupTrustRoot::Genesis(anchor) = root else {
            panic!("normal startup must resolve a genesis root");
        };
        let error = anchor
            .verify(&alternate)
            .expect_err("same signer and chain must not authorize another genesis instance");

        assert!(matches!(error.current_context(), StartError::InitKura));
        assert!(
            format!("{error:?}").contains("does not match the resolved genesis trust-anchor hash"),
            "unexpected alternate-genesis diagnostic: {error:?}"
        );
    }

    #[test]
    fn configured_hash_anchor_rejects_alternate_same_key_same_chain_genesis() {
        let keypair = KeyPair::random();
        let trusted = signed_genesis_with_marker(&keypair, "trusted genesis");
        let alternate = signed_genesis_with_marker(&keypair, "alternate genesis");
        assert_ne!(trusted.0.hash(), alternate.0.hash());

        let root = StartupTrustRoot::resolve(false, keypair.public_key(), trusted.0.hash(), None)
            .expect("configured expected hash resolves an exact anchor");
        let StartupTrustRoot::Genesis(anchor) = root else {
            panic!("normal startup must resolve a genesis root");
        };
        let error = anchor
            .verify(&alternate)
            .expect_err("the configured hash must reject another genesis from the same signer");

        assert!(matches!(error.current_context(), StartError::InitKura));
        assert!(
            format!("{error:?}").contains("does not match the resolved genesis trust-anchor hash"),
            "unexpected alternate-genesis diagnostic: {error:?}"
        );
    }
}

/// Errors raised while reading configuration and genesis data.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Failed to read configuration from disk or environment.
    ReadConfig,
    /// Configuration contents failed validation.
    ParseConfig,
    /// Failed to load the genesis file.
    ReadGenesis,
    #[cfg(feature = "dev-telemetry")]
    /// Telemetry output path resolved to root or empty.
    TelemetryOutFileIsRootOrEmpty,
    #[cfg(feature = "dev-telemetry")]
    /// Telemetry output path pointed to a directory.
    TelemetryOutFileIsDir,
    /// Network and Torii addresses conflict.
    SameNetworkAndToriiAddrs,
    /// Invalid directory path supplied in configuration.
    InvalidDirPath,
    /// Confidential features are disabled for a validator build.
    ConfidentialDisabledForValidator,
    /// Confidential assume-valid was enabled for a validator build.
    ConfidentialAssumeValidForValidator,
    /// Encrypted P2P frame cap exceeds the deterministic runtime buffer limit.
    NetworkFrameSizeExceedsRuntimeLimit {
        /// Configured encrypted-frame cap in bytes.
        configured: usize,
    },
    /// A topic plaintext cap exceeds the payload carried by the encrypted frame cap.
    NetworkTopicFrameSizeExceedsPlaintextLimit {
        /// Canonical configuration path for the topic cap.
        path: &'static str,
        /// Configured topic plaintext cap in bytes.
        configured: usize,
        /// Maximum plaintext bytes carried by the configured encrypted frame cap.
        plaintext_ceiling: usize,
        /// Configured encrypted frame cap from which the plaintext ceiling is derived.
        encrypted_cap: usize,
    },
    /// Failed to bind a configured address.
    CannotBindAddress {
        /// Address that could not be bound.
        addr: SocketAddr,
    },
    /// Multi-lane Nexus catalogs require the Nexus runtime to be enabled.
    NexusMultilaneDisabled,
    /// Joining Sora profile is mandatory but missing.
    SoraProfileRequired,
    #[cfg(not(feature = "embedded-soracloud-runtime"))]
    /// Production Soracloud runtime was requested from a binary lacking support.
    SoracloudRuntimeFeatureRequired,
    /// Embedded SoraFS storage was enabled without governed gateway compliance.
    SorafsStorageComplianceRequired,
    /// SoraFS gateway automation was enabled while embedded storage was disabled.
    SorafsGatewayRequiresStorage,
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReadConfig => write!(
                f,
                "Error occurred while reading configuration from file(s) and environment"
            ),
            Self::ParseConfig => {
                write!(f, "Error occurred while validating configuration integrity")
            }
            Self::ReadGenesis => write!(f, "Error occurred while reading genesis block"),
            #[cfg(feature = "dev-telemetry")]
            Self::TelemetryOutFileIsRootOrEmpty => {
                write!(f, "Telemetry output file path is root or empty")
            }
            #[cfg(feature = "dev-telemetry")]
            Self::TelemetryOutFileIsDir => {
                write!(f, "Telemetry output file path is a directory")
            }
            Self::SameNetworkAndToriiAddrs => write!(
                f,
                "Torii and Network addresses are the same, but should be different"
            ),
            Self::InvalidDirPath => write!(f, "Invalid directory path found"),
            Self::ConfidentialDisabledForValidator => write!(
                f,
                "validator nodes must enable confidential verification (`confidential.enabled = true`)"
            ),
            Self::ConfidentialAssumeValidForValidator => write!(
                f,
                "validator nodes cannot enable confidential observer mode (`confidential.assume_valid = false` required)"
            ),
            Self::NetworkFrameSizeExceedsRuntimeLimit { configured } => write!(
                f,
                "network.max_frame_bytes ({configured}) exceeds the deterministic encrypted P2P runtime limit of {} bytes (the wire uses a u32 body-length prefix)",
                iroha_p2p::MAX_ENCRYPTED_FRAME_BYTES,
            ),
            Self::NetworkTopicFrameSizeExceedsPlaintextLimit {
                path,
                configured,
                plaintext_ceiling,
                encrypted_cap,
            } => write!(
                f,
                "{path} ({configured}) exceeds the AEAD-specific plaintext ceiling of {plaintext_ceiling} bytes derived from network.max_frame_bytes ({encrypted_cap})",
            ),
            Self::CannotBindAddress { addr } => {
                write!(f, "Network error: cannot listen to address `{addr}`")
            }
            Self::NexusMultilaneDisabled => write!(
                f,
                "`nexus.enabled` must be set to true when lane catalogs/dataspaces or routing rules are configured"
            ),
            Self::SoraProfileRequired => {
                write!(
                    f,
                    "Sora Nexus features require `iroha3d --sora`; remove the Sora-only config overrides or rerun with the flag"
                )
            }
            #[cfg(not(feature = "embedded-soracloud-runtime"))]
            Self::SoracloudRuntimeFeatureRequired => write!(
                f,
                "`soracloud_runtime.production_mode = true` requires building iroha3d with the `embedded-soracloud-runtime` feature"
            ),
            Self::SorafsStorageComplianceRequired => write!(
                f,
                "sorafs.storage.enabled requires the governed sorafs.gateway.compliance controller"
            ),
            Self::SorafsGatewayRequiresStorage => write!(
                f,
                "SoraFS gateway ACME/compliance configuration requires sorafs.storage.enabled"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Render the IVM scheduler banner line for a given core count.
fn scheduler_banner_line(core_count: usize) -> String {
    let count = core_count.max(1);
    let core_label = if count == 1 { "core" } else { "cores" };
    format!("Using {count} {core_label}")
}

/// Translate FASTPQ Metal-related configuration into overrides understood by the prover.
fn fastpq_metal_overrides_from_config(
    config: &iroha_config::parameters::actual::Fastpq,
) -> MetalOverrides {
    MetalOverrides {
        max_in_flight: config.metal_max_in_flight,
        threadgroup_size: config.metal_threadgroup_width,
        dispatch_trace: config.metal_trace,
        debug_enum: config.metal_debug_enum,
        debug_fused: config.metal_debug_fused,
    }
}

/// Apply concurrency settings (IVM scheduler + Rayon) derived from configuration.
fn apply_concurrency_config(concurrency: &iroha_config::parameters::actual::Concurrency) {
    let stack_outcome = ivm::apply_stack_sizes(
        concurrency.scheduler_stack_bytes,
        concurrency.prover_stack_bytes,
    );
    iroha_core::sumeragi::set_sumeragi_stack_size_bytes(concurrency.sumeragi_stack_bytes);
    if stack_outcome.scheduler_clamped || stack_outcome.prover_clamped {
        iroha_logger::warn!(
            requested_scheduler_bytes = stack_outcome.requested_scheduler_bytes,
            requested_prover_bytes = stack_outcome.requested_prover_bytes,
            scheduler_bytes = stack_outcome.scheduler_bytes,
            prover_bytes = stack_outcome.prover_bytes,
            min_stack_bytes = ivm::MIN_STACK_BYTES,
            max_stack_bytes = ivm::MAX_STACK_BYTES,
            "Stack size overrides were clamped to the supported range"
        );
    }
    let min = concurrency.scheduler_min_threads;
    let max = concurrency.scheduler_max_threads;
    ivm::set_scheduler_thread_limits(
        if min == 0 { None } else { Some(min) },
        if max == 0 { None } else { Some(max) },
    );
    let (effective_min, _effective_max) = ivm::parallel::default_scheduler_limits();
    println!("{}", scheduler_banner_line(effective_min));
    if concurrency.rayon_global_threads > 0
        && let Err(err) = ivm::init_global_rayon(concurrency.rayon_global_threads)
    {
        iroha_telemetry::metrics::record_stack_pool_fallback();
        iroha_logger::warn!(
            threads = %concurrency.rayon_global_threads,
            ?err,
            "Failed to set IVM Rayon global pool with the requested stack size; using existing pool"
        );
    }
}

#[allow(clippy::too_many_lines)]
/// Read the configuration and then a genesis block if specified.
///
/// The returned configuration is **not** validated; call [`validate_config`] after
/// setting up logging to check for potential issues.
///
/// # Errors
/// - If failed to read the config
/// - If failed to load the genesis block
pub fn read_config_and_genesis(
    args: &Args,
) -> ReportResult<(Config, Option<GenesisBlock>), ConfigError> {
    let mut config = ConfigReader::new();

    if let Some(path) = &args.config {
        config = if let Some(expected) = args.startup.config_blake3.as_deref() {
            if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(Report::new(ConfigError::ReadConfig)
                    .attach("`--config-blake3` must contain exactly 64 hexadecimal digits"));
            }
            let raw = read_bounded_startup_artifact(
                path,
                INTEGRITY_BOUND_CONFIG_MAX_BYTES_V1,
                "integrity-bound configuration",
            )
            .change_context(ConfigError::ReadConfig)
            .attach_with(|| {
                format!(
                    "failed to read integrity-bound configuration {}",
                    path.display()
                )
            })?;
            let observed = blake3::hash(&raw).to_hex().to_string();
            if !expected.eq_ignore_ascii_case(&observed) {
                return Err(Report::new(ConfigError::ReadConfig).attach(format!(
                    "integrity-bound configuration {} has BLAKE3 {observed}, expected {expected}",
                    path.display()
                )));
            }
            let raw = std::str::from_utf8(&raw).map_err(|error| {
                Report::new(ConfigError::ReadConfig).attach(format!(
                    "integrity-bound configuration {} is not UTF-8: {error}",
                    path.display()
                ))
            })?;
            let table = raw.parse::<toml::Table>().map_err(|error| {
                Report::new(ConfigError::ReadConfig).attach(format!(
                    "failed to parse integrity-bound configuration {}: {error}",
                    path.display()
                ))
            })?;
            if table.contains_key("extends") {
                return Err(Report::new(ConfigError::ReadConfig).attach(format!(
                    "integrity-bound configuration {} must be flattened and cannot use `extends`",
                    path.display()
                )));
            }
            config.with_toml_source(TomlSource::new(path.clone(), table))
        } else {
            config
                .read_toml_with_extends(path)
                .change_context(ConfigError::ReadConfig)?
        };
    }

    let sorafs_storage_enabled_is_explicit =
        config.contains_toml_parameter(["sorafs", "storage", "enabled"]);
    let sorafs_discovery_enabled_is_explicit =
        config.contains_toml_parameter(["sorafs", "discovery", "discovery_enabled"]);
    let mut config = UserConfig::read_and_complete(config)
        .change_context(ConfigError::ReadConfig)?
        .parse()
        .change_context(ConfigError::ParseConfig)?;
    if let Some(path) = args.genesis_manifest_json.as_ref() {
        config.genesis.manifest_json = Some(WithOrigin::inline(path.clone()));
    }

    if args.sora {
        let configured_sorafs_storage_enabled = config.torii.sorafs_storage.enabled;
        let configured_sorafs_discovery_enabled = config.torii.sorafs_discovery.discovery_enabled;
        config.apply_sora_profile();
        if sorafs_storage_enabled_is_explicit {
            config.torii.sorafs_storage.enabled = configured_sorafs_storage_enabled;
        }
        if sorafs_discovery_enabled_is_explicit {
            config.torii.sorafs_discovery.discovery_enabled = configured_sorafs_discovery_enabled;
        }
    }

    let sorafs_enabled = config.torii.sorafs_storage.enabled
        || config.torii.sorafs_discovery.discovery_enabled
        || config.torii.sorafs_repair.enabled
        || config.torii.sorafs_gc.enabled;
    let nexus_requires_router = nexus_topology_is_custom(&config.nexus);
    let nexus_lane_overrides = config.nexus.has_lane_overrides();
    let requires_sora_profile = sorafs_enabled || nexus_requires_router || nexus_lane_overrides;

    if nexus_requires_router && !config.nexus.enabled {
        return Err(Report::new(ConfigError::NexusMultilaneDisabled).attach(
            format!(
                "Multi-lane catalogs or routing rules detected (lane_count = {}); set `nexus.enabled = true` in config or rerun with `--sora` to apply the Nexus profile",
                config.nexus.lane_catalog.lane_count()
            ),
        ));
    }
    if nexus_lane_overrides && !config.nexus.enabled {
        return Err(Report::new(ConfigError::NexusMultilaneDisabled).attach(
            "Nexus lane/dataspace/routing overrides require `nexus.enabled = true`; Iroha 2 runs strictly single-lane",
        ));
    }

    if !args.sora && requires_sora_profile {
        let mut sora_features = Vec::new();
        if sorafs_enabled {
            sora_features.push("SoraFS");
        }
        if nexus_requires_router {
            sora_features.push("multi-lane routing");
        }
        if nexus_lane_overrides {
            sora_features.push("nexus lane configuration");
        }

        let detail = sora_features.join(", ");
        return Err(
            Report::new(ConfigError::SoraProfileRequired).attach(format!(
                "Detected Sora Nexus features enabled without `--sora`: {detail}"
            )),
        );
    }

    config.apply_storage_budget();
    let storage_budget_filesystems = reconcile_nexus_storage_budget(&mut config)?;
    warn_if_nexus_storage_budget_exceeds_available(&config, &storage_budget_filesystems);

    if let Some(mode) = args.fastpq_execution_mode {
        config.zk.fastpq.execution_mode = mode;
    }
    if let Some(mode) = args.fastpq_poseidon_mode {
        config.zk.fastpq.poseidon_mode = mode;
    }
    if let Some(device_class) = args.fastpq_device_class.as_deref() {
        let trimmed = device_class.trim();
        if trimmed.is_empty() {
            config.zk.fastpq.device_class = None;
        } else {
            config.zk.fastpq.device_class = Some(trimmed.to_owned());
        }
    }
    if let Some(chip_family) = args.fastpq_chip_family.as_deref() {
        let trimmed = chip_family.trim();
        if trimmed.is_empty() {
            config.zk.fastpq.chip_family = None;
        } else {
            config.zk.fastpq.chip_family = Some(trimmed.to_owned());
        }
    }
    if let Some(gpu_kind) = args.fastpq_gpu_kind.as_deref() {
        let trimmed = gpu_kind.trim();
        if trimmed.is_empty() {
            config.zk.fastpq.gpu_kind = None;
        } else {
            config.zk.fastpq.gpu_kind = Some(trimmed.to_owned());
        }
    }

    if let Err(err) =
        fastpq_prover::apply_metal_overrides(fastpq_metal_overrides_from_config(&config.zk.fastpq))
    {
        iroha_logger::warn!(
            target: "fastpq",
            %err,
            "failed to apply FASTPQ Metal overrides"
        );
    }
    #[cfg(feature = "fastpq-gpu")]
    preflight_fastpq_bn254_poseidon_words(&config.zk.fastpq);

    apply_concurrency_config(&config.concurrency);

    // Apply Norito settings immediately so subsequent Norito decode/encode (e.g., genesis)
    // uses the configured archive bounds and GPU offload policy.
    apply_norito_config(&config);

    // Apply hardware acceleration configuration for IVM (Metal/CUDA). Defaults enable all
    // available hardware; config can cap GPUs or disable specific backends. This does not
    // change outputs, only performance characteristics.
    apply_ivm_acceleration_config(&config.accel);
    rs16::set_simd_enabled(config.accel.enable_simd);

    iroha_data_model::account::address::set_chain_discriminant(
        *config.common.chain_discriminant.value(),
    );

    let genesis =
        read_genesis_for_snapshot_policy(&config.snapshot.bootstrap, config.genesis.file.as_ref())?;

    config.logger.terminal_colors = args.terminal_colors;

    Ok((config, genesis))
}

fn read_genesis_for_snapshot_policy(
    policy: &iroha_config::parameters::actual::SnapshotBootstrapPolicy,
    signed_file: Option<&WithOrigin<PathBuf>>,
) -> ReportResult<Option<GenesisBlock>, ConfigError> {
    policy
        .validate()
        .map_err(|error| Report::new(ConfigError::ParseConfig).attach(error))?;
    if policy.enabled {
        return Ok(None);
    }
    let Some(signed_file) = signed_file else {
        return Ok(None);
    };
    let genesis = read_genesis(&signed_file.resolve_relative_path())
        .attach(signed_file.clone().into_attachment().display_path())?;
    Ok(Some(genesis))
}

#[cfg(test)]
mod snapshot_bootstrap_genesis_tests {
    use super::*;

    fn enabled_policy() -> iroha_config::parameters::actual::SnapshotBootstrapPolicy {
        iroha_config::parameters::actual::SnapshotBootstrapPolicy {
            enabled: true,
            audited_sha256: Some("ab".repeat(32)),
            audited_height: Some(7),
        }
    }

    #[test]
    fn enabled_audited_snapshot_does_not_read_missing_or_invalid_genesis() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let invalid = temp.path().join("invalid-genesis.nrt");
        fs::write(&invalid, b"not a signed genesis").expect("write invalid fixture");
        let invalid = WithOrigin::inline(invalid);
        assert!(
            read_genesis_for_snapshot_policy(&enabled_policy(), Some(&invalid))
                .expect("enabled snapshot bootstrap ignores legacy genesis bytes")
                .is_none()
        );

        let missing = WithOrigin::inline(temp.path().join("missing-genesis.nrt"));
        assert!(
            read_genesis_for_snapshot_policy(&enabled_policy(), Some(&missing))
                .expect("enabled snapshot bootstrap does not require a legacy genesis file")
                .is_none()
        );
    }

    #[test]
    fn disabled_or_partial_snapshot_policy_never_bypasses_genesis_validation() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let invalid = temp.path().join("invalid-genesis.nrt");
        fs::write(&invalid, b"not a signed genesis").expect("write invalid fixture");
        let invalid = WithOrigin::inline(invalid);
        assert!(
            read_genesis_for_snapshot_policy(
                &iroha_config::parameters::actual::SnapshotBootstrapPolicy::default(),
                Some(&invalid),
            )
            .is_err(),
            "disabled policy must decode and reject invalid configured genesis"
        );

        let partial = iroha_config::parameters::actual::SnapshotBootstrapPolicy {
            enabled: true,
            audited_sha256: None,
            audited_height: Some(7),
        };
        assert!(
            read_genesis_for_snapshot_policy(&partial, Some(&invalid)).is_err(),
            "partial authorization must fail before it can suppress genesis validation"
        );
        let disabled_with_authority = iroha_config::parameters::actual::SnapshotBootstrapPolicy {
            enabled: false,
            audited_sha256: Some("ab".repeat(32)),
            audited_height: Some(7),
        };
        assert!(
            read_genesis_for_snapshot_policy(&disabled_with_authority, Some(&invalid)).is_err(),
            "disabled policy must reject dangling audited authorization fields"
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StorageBudgetFilesystemProbe {
    filesystem_id: String,
    path: PathBuf,
    total_bytes: u64,
    available_bytes: u64,
    managed_bytes: u64,
    components: Vec<NexusStorageBudgetComponent>,
    managed_roots: Vec<PathBuf>,
    derived_budget_bytes: Option<u64>,
}

fn reconcile_nexus_storage_budget(
    config: &mut Config,
) -> ReportResult<Vec<StorageBudgetFilesystemProbe>, ConfigError> {
    if !config.nexus.enabled {
        return Ok(Vec::new());
    }

    if config.nexus.storage.local_budget_bytes.is_some() {
        return probe_nexus_storage_filesystems(config);
    }

    let mut filesystems = probe_nexus_storage_filesystems(config)?;
    let filesystem_budgets = derive_runtime_nexus_storage_budget(&filesystems)?;
    let aggregate_budget_bytes = config
        .apply_derived_storage_budget(&filesystem_budgets)
        .map_err(|error| {
            Report::new(ConfigError::ParseConfig).attach(format!(
                "failed to apply runtime Nexus storage budget: {error}"
            ))
        })?;
    for (filesystem, budget) in filesystems.iter_mut().zip(&filesystem_budgets) {
        filesystem.derived_budget_bytes = Some(budget.budget_bytes.get());
        iroha_logger::info!(
            filesystem_id = %filesystem.filesystem_id,
            path = %filesystem.path.display(),
            components = ?nexus_storage_component_labels(&filesystem.components),
            total_bytes = filesystem.total_bytes,
            available_bytes = filesystem.available_bytes,
            managed_bytes = filesystem.managed_bytes,
            budget_bytes = budget.budget_bytes.get(),
            "derived runtime-only Nexus storage budget"
        );
    }
    iroha_logger::info!(
        aggregate_budget_bytes = aggregate_budget_bytes.get(),
        filesystem_groups = filesystem_budgets.len(),
        "activated runtime-only Nexus storage budget; operator configuration remains unchanged"
    );
    Ok(filesystems)
}

fn probe_nexus_storage_filesystems(
    config: &Config,
) -> ReportResult<Vec<StorageBudgetFilesystemProbe>, ConfigError> {
    let mut groups = BTreeMap::<String, StorageBudgetFilesystemProbe>::new();

    for (component, root) in effective_nexus_storage_component_roots(config) {
        let lexical_root = normalize_budget_probe_path(root).ok_or_else(|| {
            Report::new(ConfigError::ParseConfig).attach(format!(
                "failed to resolve Nexus storage root for component `{}` against the current directory",
                component.as_str()
            ))
        })?;
        let resolved_root = resolve_budget_probe_root(&lexical_root, component)?;
        let probe_path = resolved_root.probe_path;
        let filesystem_id = filesystem_identity(&probe_path).ok_or_else(|| {
            filesystem_probe_config_error(format!(
                "failed to determine the filesystem identity for `{}` (component `{}`)",
                probe_path.display(),
                component.as_str()
            ))
        })?;
        let (available_bytes, total_bytes) = filesystem_space(&probe_path).ok_or_else(|| {
            filesystem_probe_config_error(format!(
                "failed to determine filesystem capacity for `{}` (component `{}`)",
                probe_path.display(),
                component.as_str()
            ))
        })?;
        let managed_root = resolved_root.managed_root;

        groups
            .entry(filesystem_id.clone())
            .and_modify(|group| {
                group.available_bytes = group.available_bytes.min(available_bytes);
                group.total_bytes = group.total_bytes.min(total_bytes);
                if !group.components.contains(&component) {
                    group.components.push(component);
                }
                if let Some(managed_root) = managed_root.as_ref()
                    && !group.managed_roots.contains(managed_root)
                {
                    group.managed_roots.push(managed_root.clone());
                }
            })
            .or_insert_with(|| StorageBudgetFilesystemProbe {
                filesystem_id,
                path: probe_path,
                total_bytes,
                available_bytes,
                managed_bytes: 0,
                components: vec![component],
                managed_roots: managed_root.into_iter().collect(),
                derived_budget_bytes: None,
            });
    }

    let mut groups: Vec<_> = groups.into_values().collect();
    for group in &mut groups {
        group.components.sort_unstable();
        deduplicate_managed_roots(&mut group.managed_roots);
        group.managed_bytes = group.managed_roots.iter().try_fold(0_u64, |total, root| {
            let bytes = managed_root_size(root, &group.filesystem_id).map_err(|error| {
                Report::new(ConfigError::ParseConfig).attach(format!(
                    "failed to measure Nexus managed storage root `{}`: {error}",
                    root.display()
                ))
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                Report::new(ConfigError::ParseConfig).attach(format!(
                    "managed Nexus storage byte count overflowed for filesystem `{}`",
                    group.filesystem_id
                ))
            })
        })?;
    }
    groups.sort_by_key(|group| {
        group.components.first().map_or(usize::MAX, |component| {
            nexus_storage_component_order(*component)
        })
    });
    Ok(groups)
}

fn effective_nexus_storage_component_roots(
    config: &Config,
) -> Vec<(NexusStorageBudgetComponent, PathBuf)> {
    let mut roots = vec![(
        NexusStorageBudgetComponent::Kura,
        config.kura.store_dir.resolve_relative_path(),
    )];

    let tiered_state_root = config
        .tiered_state
        .da_store_root
        .clone()
        .or_else(|| config.tiered_state.cold_store_root.clone())
        .or_else(|| {
            (config.nexus.storage.max_wsv_memory_bytes.get() > 0).then(|| {
                PathBuf::from(
                    iroha_config::parameters::defaults::tiered_state::DEFAULT_COLD_STORE_ROOT,
                )
            })
        });
    if let Some(tiered_state_root) = tiered_state_root {
        roots.push((NexusStorageBudgetComponent::WsvCold, tiered_state_root));
    }

    roots.push((
        NexusStorageBudgetComponent::Sorafs,
        config.torii.sorafs_storage.data_dir.clone(),
    ));
    roots.push((
        NexusStorageBudgetComponent::SoranetSpool,
        config.streaming.soranet.provision_spool_dir.clone(),
    ));
    roots.push((
        NexusStorageBudgetComponent::SoravpnSpool,
        config.streaming.soravpn.provision_spool_dir.clone(),
    ));
    roots
}

fn derive_runtime_nexus_storage_budget(
    filesystems: &[StorageBudgetFilesystemProbe],
) -> ReportResult<Vec<NexusStorageFilesystemBudget>, ConfigError> {
    let mut filesystem_budgets = Vec::with_capacity(filesystems.len());
    for filesystem in filesystems {
        let total_bytes = u128::from(filesystem.total_bytes);
        let headroom_bps = u128::from(
            iroha_config::parameters::defaults::nexus::storage::AUTO_STORAGE_HEADROOM_BPS,
        );
        let bps_total = u128::from(iroha_config::parameters::defaults::nexus::storage::BPS_TOTAL);
        let reserve_bytes = total_bytes
            .checked_mul(headroom_bps)
            .and_then(|value| value.checked_add(bps_total.saturating_sub(1)))
            .map(|value| value / bps_total)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| {
                Report::new(ConfigError::ParseConfig).attach(format!(
                    "failed to derive Nexus storage headroom for filesystem `{}`",
                    filesystem.filesystem_id
                ))
            })?;
        let usable_bytes = filesystem
            .managed_bytes
            .checked_add(filesystem.available_bytes)
            .ok_or_else(|| {
                Report::new(ConfigError::ParseConfig).attach(format!(
                    "managed plus available bytes overflowed for Nexus storage filesystem `{}`",
                    filesystem.filesystem_id
                ))
            })?;
        let budget_bytes = usable_bytes
            .checked_sub(reserve_bytes)
            .and_then(NonZeroU64::new)
            .ok_or_else(|| {
                Report::new(ConfigError::ParseConfig).attach(format!(
                    "filesystem `{}` has no safe non-zero Nexus storage budget after reserving {reserve_bytes} bytes of headroom; configure nexus.storage.local_budget_bytes explicitly only after freeing space",
                    filesystem.filesystem_id
                ))
            })?;
        if budget_bytes.get() < filesystem.managed_bytes {
            return Err(Report::new(ConfigError::ParseConfig).attach(format!(
                "filesystem `{}` derived budget {} is below the {} managed bytes after reserving {reserve_bytes} bytes of headroom; free space before startup or configure nexus.storage.local_budget_bytes deliberately",
                filesystem.filesystem_id,
                budget_bytes.get(),
                filesystem.managed_bytes
            )));
        }
        filesystem_budgets.push(NexusStorageFilesystemBudget {
            budget_bytes,
            components: filesystem.components.clone(),
        });
    }

    if filesystem_budgets.is_empty() {
        return Err(Report::new(ConfigError::ParseConfig)
            .attach("runtime Nexus storage derivation produced no filesystem budget groups"));
    }
    Ok(filesystem_budgets)
}

fn warn_if_nexus_storage_budget_exceeds_available(
    config: &Config,
    filesystems: &[StorageBudgetFilesystemProbe],
) {
    if filesystems.is_empty() || !config.nexus.enabled {
        return;
    }

    if filesystems
        .iter()
        .all(|filesystem| filesystem.derived_budget_bytes.is_some())
    {
        return;
    }

    for filesystem in filesystems {
        let Some(assigned_budget) = operator_explicit_budget_shortfall(config, filesystem) else {
            continue;
        };
        iroha_logger::warn!(
            filesystem_id = %filesystem.filesystem_id,
            path = %filesystem.path.display(),
            components = ?nexus_storage_component_labels(&filesystem.components),
            assigned_budget_bytes = assigned_budget,
            managed_bytes = filesystem.managed_bytes,
            available_bytes = filesystem.available_bytes,
            "effective operator-configured Nexus storage caps exceed safe available disk space on a filesystem"
        );
    }
}

fn operator_explicit_budget_shortfall(
    config: &Config,
    filesystem: &StorageBudgetFilesystemProbe,
) -> Option<u64> {
    let assigned_budget = effective_assigned_budget_for_filesystem(config, filesystem);
    let required_growth = assigned_budget.saturating_sub(filesystem.managed_bytes);
    (required_growth > filesystem.available_bytes).then_some(assigned_budget)
}

fn effective_assigned_budget_for_filesystem(
    config: &Config,
    filesystem: &StorageBudgetFilesystemProbe,
) -> u64 {
    filesystem
        .components
        .iter()
        .fold(0_u64, |total, component| {
            let component_budget = match component {
                NexusStorageBudgetComponent::Kura => config.kura.max_disk_usage_bytes.get(),
                NexusStorageBudgetComponent::WsvCold => config.tiered_state.max_cold_bytes.get(),
                NexusStorageBudgetComponent::Sorafs => {
                    config.torii.sorafs_storage.max_capacity_bytes.get()
                }
                NexusStorageBudgetComponent::SoranetSpool => {
                    config.streaming.soranet.provision_spool_max_bytes.get()
                }
                NexusStorageBudgetComponent::SoravpnSpool => {
                    config.streaming.soravpn.provision_spool_max_bytes.get()
                }
            };
            total.saturating_add(component_budget)
        })
}

fn nexus_storage_component_labels(components: &[NexusStorageBudgetComponent]) -> Vec<&'static str> {
    components
        .iter()
        .map(|component| component.as_str())
        .collect()
}

fn nexus_storage_component_order(component: NexusStorageBudgetComponent) -> usize {
    NexusStorageBudgetComponent::ORDER
        .iter()
        .position(|ordered| ordered == &component)
        .unwrap_or(usize::MAX)
}

fn normalize_budget_probe_path(path: PathBuf) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => {
                normalized.push(std::path::MAIN_SEPARATOR_STR);
            }
            std::path::Component::Normal(segment) => normalized.push(segment),
        }
    }
    Some(normalized)
}

fn nearest_existing_ancestor(path: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut current = path.to_path_buf();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => return Ok(Some(current)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if !current.pop() {
            return Ok(None);
        }
    }
}

#[derive(Debug)]
struct ResolvedStorageBudgetRoot {
    probe_path: PathBuf,
    managed_root: Option<PathBuf>,
}

fn resolve_budget_probe_root(
    lexical_root: &Path,
    component: NexusStorageBudgetComponent,
) -> ReportResult<ResolvedStorageBudgetRoot, ConfigError> {
    let existing_ancestor = nearest_existing_ancestor(lexical_root)
        .map_err(|error| {
            filesystem_probe_config_error(format!(
                "failed to inspect Nexus storage root `{}` (component `{}`): {error}",
                lexical_root.display(),
                component.as_str()
            ))
        })?
        .ok_or_else(|| {
            filesystem_probe_config_error(format!(
                "failed to find an existing ancestor for Nexus storage root `{}` (component `{}`)",
                lexical_root.display(),
                component.as_str()
            ))
        })?;
    let existing_metadata = fs::symlink_metadata(&existing_ancestor).map_err(|error| {
        filesystem_probe_config_error(format!(
            "failed to recheck Nexus storage ancestor `{}` (component `{}`): {error}",
            existing_ancestor.display(),
            component.as_str()
        ))
    })?;
    let root_exists = existing_ancestor == lexical_root;
    if root_exists && metadata_is_symlink_or_reparse(&existing_metadata) {
        return Err(filesystem_probe_config_error(format!(
            "Nexus storage root `{}` (component `{}`) must not be a symbolic link or reparse point",
            lexical_root.display(),
            component.as_str()
        )));
    }

    let canonical_ancestor = fs::canonicalize(&existing_ancestor).map_err(|error| {
        filesystem_probe_config_error(format!(
            "failed to canonicalize Nexus storage ancestor `{}` (component `{}`): {error}",
            existing_ancestor.display(),
            component.as_str()
        ))
    })?;
    let canonical_metadata = fs::metadata(&canonical_ancestor).map_err(|error| {
        filesystem_probe_config_error(format!(
            "failed to inspect canonical Nexus storage ancestor `{}` (component `{}`): {error}",
            canonical_ancestor.display(),
            component.as_str()
        ))
    })?;
    if !canonical_metadata.is_dir() {
        return Err(filesystem_probe_config_error(format!(
            "Nexus storage ancestor `{}` (component `{}`) is not a directory",
            canonical_ancestor.display(),
            component.as_str()
        )));
    }

    let missing_suffix = lexical_root
        .strip_prefix(&existing_ancestor)
        .expect("nearest ancestor must prefix the normalized storage root");
    let canonical_root = normalize_budget_probe_path(canonical_ancestor.join(missing_suffix))
        .ok_or_else(|| {
            filesystem_probe_config_error(format!(
                "failed to normalize canonical Nexus storage root `{}` (component `{}`)",
                lexical_root.display(),
                component.as_str()
            ))
        })?;
    let probe_path = if root_exists {
        canonical_root.clone()
    } else {
        canonical_ancestor
    };
    Ok(ResolvedStorageBudgetRoot {
        probe_path,
        managed_root: root_exists.then_some(canonical_root),
    })
}

fn metadata_is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(target_os = "windows"))]
    false
}

fn deduplicate_managed_roots(roots: &mut Vec<PathBuf>) {
    roots.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    let mut unique = Vec::<PathBuf>::with_capacity(roots.len());
    for root in std::mem::take(roots) {
        if !unique.iter().any(|ancestor| root.starts_with(ancestor)) {
            unique.push(root);
        }
    }
    *roots = unique;
}

fn managed_root_size(path: &Path, expected_filesystem_id: &str) -> std::io::Result<u64> {
    managed_root_size_with_identity(path, expected_filesystem_id, filesystem_identity)
}

fn managed_root_size_with_identity<F>(
    path: &Path,
    expected_filesystem_id: &str,
    identity: F,
) -> std::io::Result<u64>
where
    F: Fn(&Path) -> Option<String>,
{
    let metadata = fs::symlink_metadata(path)?;
    if metadata_is_symlink_or_reparse(&metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "managed storage root must not be a symbolic link or reparse point",
        ));
    }
    if identity(path).as_deref() != Some(expected_filesystem_id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "managed storage root moved to a different filesystem during probing",
        ));
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0_u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_metadata = fs::symlink_metadata(&entry_path)?;
            if metadata_is_symlink_or_reparse(&entry_metadata) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "managed storage tree contains symbolic link or reparse point `{}`",
                        entry_path.display()
                    ),
                ));
            }
            let entry_filesystem_id = identity(&entry_path).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "failed to determine filesystem identity for managed storage entry `{}`",
                        entry_path.display()
                    ),
                )
            })?;
            if entry_filesystem_id != expected_filesystem_id {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "managed storage entry `{}` crosses from filesystem `{expected_filesystem_id}` to `{entry_filesystem_id}`",
                        entry_path.display()
                    ),
                ));
            }
            if entry_metadata.is_dir() {
                stack.push(entry_path);
            } else if entry_metadata.is_file() {
                total = total.checked_add(entry_metadata.len()).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "managed storage byte count overflowed u64",
                    )
                })?;
            }
        }
    }
    let final_metadata = fs::symlink_metadata(path)?;
    if metadata_is_symlink_or_reparse(&final_metadata)
        || identity(path).as_deref() != Some(expected_filesystem_id)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "managed storage root changed identity during measurement",
        ));
    }
    Ok(total)
}

fn filesystem_probe_config_error(detail: String) -> Report<ConfigError> {
    let report = Report::new(ConfigError::ParseConfig).attach(detail);
    #[cfg(not(any(unix, target_os = "windows")))]
    let report = report.attach("Nexus storage filesystem probing is unsupported on this platform");
    report
}

#[cfg(unix)]
fn filesystem_identity(path: &Path) -> Option<String> {
    let stats = rustix::fs::stat(path).ok()?;
    Some(format!("dev:{}", stats.st_dev))
}

#[cfg(target_os = "windows")]
fn filesystem_identity(path: &Path) -> Option<String> {
    let volume_mount_point = windows_volume_mount_point(path)?;
    let volume_name = windows_volume_name_for_mount_point(&volume_mount_point)?;
    Some(normalize_windows_volume_identity(&volume_name))
}

#[cfg(unix)]
fn filesystem_space(path: &Path) -> Option<(u64, u64)> {
    let stats = rustix::fs::statvfs(path).ok()?;
    let fragment_size = stats.f_frsize.max(stats.f_bsize);
    let available_bytes = stats.f_bavail.checked_mul(fragment_size)?;
    let total_bytes = stats.f_blocks.checked_mul(fragment_size)?;
    Some((available_bytes, total_bytes))
}

#[cfg(target_os = "windows")]
fn filesystem_space(path: &Path) -> Option<(u64, u64)> {
    let wide_path = windows_wide_path(path);
    let mut free_bytes_available = 0_u64;
    let mut total_bytes = 0_u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide_path.as_ptr(),
            &mut free_bytes_available,
            &mut total_bytes,
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some((free_bytes_available, total_bytes))
}

#[cfg(not(any(unix, target_os = "windows")))]
fn filesystem_space(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(not(any(unix, target_os = "windows")))]
fn filesystem_identity(_path: &Path) -> Option<String> {
    None
}

#[cfg(any(target_os = "windows", test))]
fn normalize_windows_volume_mount_point(volume_mount_point: &str) -> String {
    let mut normalized = volume_mount_point.replace('/', "\\");
    if !normalized.ends_with('\\') {
        normalized.push('\\');
    }
    normalized
}

#[cfg(any(target_os = "windows", test))]
fn normalize_windows_volume_identity(volume_name: &str) -> String {
    let mut normalized = normalize_windows_volume_mount_point(volume_name);
    normalized.make_ascii_lowercase();
    format!("volume:{normalized}")
}

#[cfg(any(target_os = "windows", test))]
fn windows_string_from_wide_buffer(buffer: &[u16]) -> Option<String> {
    let end = buffer.iter().position(|&unit| unit == 0)?;
    Some(String::from_utf16_lossy(&buffer[..end]))
}

#[cfg(target_os = "windows")]
const WINDOWS_FILESYSTEM_PROBE_BUFFER_LEN: usize = 32_768;

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn GetDiskFreeSpaceExW(
        lp_directory_name: *const u16,
        lp_free_bytes_available_to_caller: *mut u64,
        lp_total_number_of_bytes: *mut u64,
        lp_total_number_of_free_bytes: *mut u64,
    ) -> i32;
    fn GetVolumeNameForVolumeMountPointW(
        lpsz_volume_mount_point: *const u16,
        lpsz_volume_name: *mut u16,
        cch_buffer_length: u32,
    ) -> i32;
    fn GetVolumePathNameW(
        lpsz_file_name: *const u16,
        lpsz_volume_path_name: *mut u16,
        cch_buffer_length: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
fn windows_wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn windows_query_volume_string<F>(mut query: F) -> Option<String>
where
    F: FnMut(*mut u16, u32) -> i32,
{
    let mut buffer = vec![0_u16; WINDOWS_FILESYSTEM_PROBE_BUFFER_LEN];
    (query(buffer.as_mut_ptr(), buffer.len() as u32) != 0)
        .then(|| windows_string_from_wide_buffer(&buffer))
        .flatten()
}

#[cfg(target_os = "windows")]
fn windows_volume_mount_point(path: &Path) -> Option<String> {
    let wide_path = windows_wide_path(path);
    windows_query_volume_string(|buffer, len| unsafe {
        GetVolumePathNameW(wide_path.as_ptr(), buffer, len)
    })
    .map(|mount_point| normalize_windows_volume_mount_point(&mount_point))
}

#[cfg(target_os = "windows")]
fn windows_volume_name_for_mount_point(volume_mount_point: &str) -> Option<String> {
    let wide_mount_point = windows_wide_string(volume_mount_point);
    windows_query_volume_string(|buffer, len| unsafe {
        GetVolumeNameForVolumeMountPointW(wide_mount_point.as_ptr(), buffer, len)
    })
}

pub(crate) fn apply_ivm_acceleration_config(
    accel: &iroha_config::parameters::actual::Acceleration,
) {
    let ivm_cfg = ivm::AccelerationConfig {
        enable_simd: accel.enable_simd,
        enable_metal: accel.enable_metal,
        enable_cuda: accel.enable_cuda,
        max_gpus: accel.max_gpus,
        merkle_min_leaves_gpu: Some(accel.merkle_min_leaves_gpu),
        merkle_min_leaves_metal: accel.merkle_min_leaves_metal,
        merkle_min_leaves_cuda: accel.merkle_min_leaves_cuda,
        prefer_cpu_sha2_max_leaves_aarch64: accel.prefer_cpu_sha2_max_leaves_aarch64,
        prefer_cpu_sha2_max_leaves_x86: accel.prefer_cpu_sha2_max_leaves_x86,
    };
    ivm::set_acceleration_config(ivm_cfg);
}

#[cfg(test)]
mod build_line_tests {
    use super::{resolve_build_line_from_env, *};
    use iroha_config_base::toml::TomlSource;

    #[test]
    fn soracloud_runtime_manager_corridor_has_no_local_key_fallback() {
        let source = include_str!("main.rs");
        let start = source
            .find("let local_validator_account_id =")
            .expect("Soracloud identity corridor");
        let end = source[start..]
            .find("let runtime_manager = if let Some(signer)")
            .map(|offset| start + offset)
            .expect("Soracloud sink injection corridor");
        let corridor = &source[start..end];
        assert!(
            !corridor.contains("config.common.key_pair"),
            "Soracloud mutation authority must never fall back to the process-local node key"
        );
    }

    #[test]
    fn soracloud_production_fails_before_startup_without_signer_binding() {
        let mut dependencies = IrohaRuntimeDeps::default();
        assert_eq!(
            qualify_soracloud_runtime_signer_for_startup(true, None, &mut dependencies),
            Err("production mode requires an exact configured signer binding")
        );
        assert!(
            qualify_soracloud_runtime_signer_for_startup(false, None, &mut dependencies).is_ok(),
            "unbound non-production mode must remain read-only"
        );
        assert!(dependencies.is_empty());
    }

    #[test]
    fn native_signer_presence_gate_accepts_only_exact_enabled_and_disabled_shapes() {
        assert!(
            validate_sorafs_native_signer_role_presence("proof_outcome", true, true, true).is_ok()
        );
        assert!(
            validate_sorafs_native_signer_role_presence("proof_outcome", false, false, false)
                .is_ok()
        );
    }

    #[test]
    fn storage_enabled_requires_native_signers_independently_of_generation_flags() {
        assert!(sorafs_native_signer_role_required(true, false));
        assert!(sorafs_native_signer_role_required(true, true));
        assert!(sorafs_native_signer_role_required(false, true));
        assert!(!sorafs_native_signer_role_required(false, false));
    }

    #[test]
    fn native_signer_presence_gate_rejects_enabled_missing_and_disabled_injected_roles() {
        assert_eq!(
            validate_sorafs_native_signer_role_presence("proof_outcome", true, false, false),
            Err(
                "required SoraFS proof_outcome signer role is missing its configured binding for storage-enabled durable drain or role generation"
                    .to_owned(),
            )
        );
        assert_eq!(
            validate_sorafs_native_signer_role_presence(
                "proof_outcome",
                false,
                false,
                true
            ),
            Err(
                "unconfigured SoraFS proof_outcome signer role rejects an injected runtime provider"
                    .to_owned()
            )
        );
    }

    #[test]
    fn native_signer_presence_gate_rejects_configured_missing_and_disabled_binding_roles() {
        assert_eq!(
            validate_sorafs_native_signer_role_presence("proof_outcome", true, true, false),
            Err(
                "configured SoraFS proof_outcome signer role is missing its runtime provider"
                    .to_owned()
            )
        );
        assert_eq!(
            validate_sorafs_native_signer_role_presence("proof_outcome", false, true, true),
            Err(
                "inactive SoraFS proof_outcome signer role rejects a configured binding without storage-enabled durable drain or role generation"
                    .to_owned(),
            )
        );
    }
    use iroha_crypto::Hash;
    use iroha_data_model::nexus::{DataSpaceId, LaneCatalog, LaneConfig, LaneId};
    use std::{io::Write, num::NonZeroU32, path::Path};
    use tempfile::NamedTempFile;
    use toml::Table;

    pub fn minimal_config_table() -> Table {
        toml::from_str(
            r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers_pop = [
  { public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#,
        )
        .expect("minimal config")
    }

    pub fn multilane_config_table(enabled: bool) -> Table {
        toml::from_str(&format!(
            r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers_pop = [
  {{ public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }}
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"

[nexus]
enabled = {enabled}
lane_count = 2

[[nexus.lane_catalog]]
index = 0
alias = "core"
metadata = {{}}

[[nexus.lane_catalog]]
index = 1
alias = "zk"
metadata = {{}}
"#
        ))
        .expect("multilane config")
    }

    fn single_lane_override_config_table() -> Table {
        toml::from_str(
            r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers_pop = [
  { public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"

[nexus]
enabled = false
lane_count = 1

[[nexus.lane_catalog]]
index = 0
alias = "custom"
description = "lane overrides should be rejected when nexus is disabled"
metadata = {}
"#,
        )
        .expect("single-lane override config")
    }

    const NEXUS_DEFAULTS_BLAKE2B: &str =
        "3a7d7f8a8a20880b05d9473ee7d08492afead4a3160949ef9da9aa0e152628c7";

    fn file_blake2b_hex(path: &Path) -> String {
        let bytes = std::fs::read(path).expect("read file");
        Hash::new(bytes).to_string()
    }

    pub(super) fn load_unprovisioned_profile_for_inspection(path: &Path) -> Config {
        let source = std::fs::read_to_string(path).expect("read checked-in signing profile");
        let mut table: Table = toml::from_str(&source).expect("parse signing profile TOML");
        let expected_hash = table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .and_then(|genesis| genesis.get_mut("expected_hash"))
            .expect("signing profile genesis.expected_hash placeholder");
        assert_eq!(
            expected_hash.as_str(),
            Some("REPLACE_WITH_GENESIS_EXPECTED_HASH"),
            "checked-in signing profiles must remain explicitly unprovisioned"
        );
        // This hash exists only so tests can inspect unrelated typed fields. It never enters a
        // runtime config and the returned config must not be used to start a node.
        *expected_hash = toml::Value::String(
            Hash::new(b"irohad non-runtime signing-profile inspection").to_string(),
        );
        Config::from_toml_source(TomlSource::inline(table))
            .expect("resolve signing profile for non-runtime inspection")
    }

    #[test]
    fn build_line_env_override_takes_precedence() {
        assert_eq!(
            resolve_build_line_from_env(Some("iroha2".to_owned()), "iroha3d"),
            BuildLine::Iroha2
        );
        assert_eq!(
            resolve_build_line_from_env(Some("iroha3".to_owned()), "iroha3d"),
            BuildLine::Iroha3
        );
        assert_eq!(
            resolve_build_line_from_env(Some("unknown".to_owned()), "iroha3d"),
            BuildLine::Iroha3
        );
    }

    #[test]
    fn operator_signatures_allowlist_adds_node_key_when_enabled() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        let node_public_key = config.common.key_pair.public_key().clone();
        config.torii.operator_signatures.allow_node_key = true;
        config.torii.operator_signatures.allowed_public_keys.clear();

        ensure_operator_node_key_allowlisted(&mut config);

        assert!(
            config
                .torii
                .operator_signatures
                .allowed_public_keys
                .contains(&node_public_key),
            "node public key should be allow-listed when allow_node_key is enabled"
        );
    }

    #[test]
    fn operator_signatures_allowlist_keeps_node_key_unique() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        let node_public_key = config.common.key_pair.public_key().clone();
        config.torii.operator_signatures.allow_node_key = true;
        config
            .torii
            .operator_signatures
            .allowed_public_keys
            .push(node_public_key.clone());

        ensure_operator_node_key_allowlisted(&mut config);

        let count = config
            .torii
            .operator_signatures
            .allowed_public_keys
            .iter()
            .filter(|key| *key == &node_public_key)
            .count();
        assert_eq!(count, 1, "node public key should not be duplicated");
    }

    #[test]
    fn operator_signatures_allowlist_respects_disabled_node_key_flag() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        config.torii.operator_signatures.allow_node_key = false;
        config.torii.operator_signatures.allowed_public_keys.clear();

        ensure_operator_node_key_allowlisted(&mut config);

        assert!(
            config
                .torii
                .operator_signatures
                .allowed_public_keys
                .is_empty(),
            "allow-list should remain unchanged when allow_node_key is disabled"
        );
    }

    #[test]
    fn iroha2_disarms_soranet_streaming() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        config.streaming.soranet.enabled = true;
        enforce_build_line(BuildLine::Iroha2, &mut config).expect("should sanitize");
        assert!(!config.streaming.soranet.enabled);
    }

    #[test]
    fn iroha2_disarms_nexus_flag_without_multilane() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        config.nexus.enabled = true;
        enforce_build_line(BuildLine::Iroha2, &mut config).expect("nexus flag should be disarmed");
        assert!(!config.nexus.enabled);
    }

    #[test]
    fn iroha2_disarms_sorafs_switches() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        config.torii.sorafs_storage.enabled = true;
        config.torii.sorafs_discovery.discovery_enabled = true;

        enforce_build_line(BuildLine::Iroha2, &mut config).expect("should sanitize");

        assert!(!config.torii.sorafs_storage.enabled);
        assert!(!config.torii.sorafs_discovery.discovery_enabled);
    }

    #[test]
    fn iroha2_rejects_multilane_catalog() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    dataspace_id: DataSpaceId::UNIVERSAL,
                    alias: "governance".to_string(),
                    description: Some("governance lane".to_string()),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("catalog");
        config.nexus.lane_catalog = catalog.clone();
        config.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog);

        let err = enforce_build_line(BuildLine::Iroha2, &mut config).expect_err("must fail");
        let rendered = format!("{err:?}");
        assert!(rendered.contains("Nexus"));
    }

    #[test]
    fn iroha2_rejects_lane_overrides_without_nexus() {
        let err = Config::from_toml_source(TomlSource::inline(single_lane_override_config_table()))
            .expect_err("lane overrides should be rejected when nexus is disabled");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("nexus.enabled"),
            "error should point at the required nexus flag: {rendered}"
        );
        assert!(
            rendered.contains("single-lane"),
            "error should mention single-lane boundary: {rendered}"
        );
    }

    #[test]
    fn sora_profile_enables_nexus_and_catalog() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        assert!(config.nexus.enabled);

        config.apply_sora_profile();

        assert!(config.nexus.enabled);
        assert_eq!(config.nexus.lane_catalog.lane_count().get(), 3);
        assert_eq!(config.nexus.lane_config.entries().len(), 3);
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
        assert_eq!(dataspace_aliases, ["universal", "governance", "zk"]);
        assert!(nexus_topology_is_custom(&config.nexus));
        assert!(should_use_config_router(&config.nexus));
    }

    #[test]
    fn config_router_requires_enabled_flag() {
        let err = Config::from_toml_source(TomlSource::inline(multilane_config_table(false)))
            .expect_err("multilane config should be rejected without nexus flag");
        assert!(
            format!("{err:?}").contains("nexus.enabled"),
            "missing nexus-enabled hint: {err:?}"
        );

        let config = Config::from_toml_source(TomlSource::inline(multilane_config_table(true)))
            .expect("enabled multilane config");

        assert!(nexus_topology_is_custom(&config.nexus));
        assert!(should_use_config_router(&config.nexus));
    }

    #[test]
    fn nexus_profile_defaults_enable_flag() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");
        assert!(
            Config::from_toml_source(
                TomlSource::from_file(path.clone()).expect("read nexus defaults config")
            )
            .is_err(),
            "the unprovisioned profile must not be a runnable node config"
        );
        let config = load_unprovisioned_profile_for_inspection(&path);

        assert!(config.nexus.enabled);
        assert_eq!(config.nexus.dataspace_catalog.entries().len(), 3);
        assert!(nexus_topology_is_custom(&config.nexus));
        assert!(should_use_config_router(&config.nexus));
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
        assert_eq!(dataspace_aliases, ["universal", "governance", "zk"]);
    }

    #[test]
    fn nexus_profile_hash_matches_template() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");
        let hash = file_blake2b_hex(&path);
        assert_eq!(hash, NEXUS_DEFAULTS_BLAKE2B);
    }

    #[test]
    fn sora_flag_enables_nexus_profile() {
        let mut config_file = NamedTempFile::new().expect("create temp config");
        let toml_value = toml::Value::Table(minimal_config_table());
        config_file
            .write_all(
                toml::to_string(&toml_value)
                    .expect("render config")
                    .as_bytes(),
            )
            .expect("write config");

        let args = parse_args_from([
            "iroha3d",
            "--sora",
            "--config",
            config_file
                .path()
                .to_str()
                .expect("temp config path to string"),
        ]);

        let (config, _) = read_config_and_genesis(&args).expect("parse config with --sora");

        let mut expected =
            Config::from_toml_source(TomlSource::inline(minimal_config_table())).expect("default");
        expected.apply_sora_profile();

        assert!(config.nexus.enabled);
        assert_eq!(config.nexus.lane_catalog, expected.nexus.lane_catalog);
        assert_eq!(
            config.nexus.dataspace_catalog,
            expected.nexus.dataspace_catalog
        );
        assert_eq!(config.nexus.routing_policy, expected.nexus.routing_policy);
        assert!(should_use_config_router(&config.nexus));
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
        assert_eq!(dataspace_aliases, ["universal", "governance", "zk"]);
    }

    #[test]
    fn sora_flag_preserves_explicitly_disabled_sorafs_storage() {
        let mut config_file = NamedTempFile::new().expect("create temp config");
        let mut table = minimal_config_table();
        iroha_config::base::toml::Writer::new(&mut table)
            .write(["sorafs", "storage", "enabled"], false);
        config_file
            .write_all(
                toml::to_string(&toml::Value::Table(table))
                    .expect("render config")
                    .as_bytes(),
            )
            .expect("write config");

        let args = parse_args_from([
            "iroha3d",
            "--sora",
            "--config",
            config_file
                .path()
                .to_str()
                .expect("temp config path to string"),
        ]);

        let (config, _) =
            read_config_and_genesis(&args).expect("parse config with explicit storage opt-out");

        assert!(config.nexus.enabled);
        assert!(
            !config.torii.sorafs_storage.enabled,
            "--sora must not override an explicit operator storage opt-out"
        );
    }

    #[test]
    fn single_lane_config_preserves_defaults_without_sora_flag() {
        let mut config_file = NamedTempFile::new().expect("create temp config");
        let toml_value = toml::Value::Table(minimal_config_table());
        config_file
            .write_all(
                toml::to_string(&toml_value)
                    .expect("render config")
                    .as_bytes(),
            )
            .expect("write config");

        let args = parse_args_from([
            "iroha3d",
            "--config",
            config_file
                .path()
                .to_str()
                .expect("temp config path to string"),
        ]);

        let (config, _) = read_config_and_genesis(&args).expect("parse config without --sora");

        assert!(config.nexus.enabled);
        assert_eq!(config.nexus.lane_catalog.lane_count().get(), 1);
        assert!(!nexus_topology_is_custom(&config.nexus));
        assert!(!should_use_config_router(&config.nexus));
    }

    #[test]
    fn multilane_config_requires_nexus_enabled_flag() {
        let mut config_file = NamedTempFile::new().expect("create temp config");
        let toml_value = toml::Value::Table(multilane_config_table(false));
        config_file
            .write_all(
                toml::to_string(&toml_value)
                    .expect("render config")
                    .as_bytes(),
            )
            .expect("write config");

        let args = parse_args_from([
            "iroha3d",
            "--config",
            config_file
                .path()
                .to_str()
                .expect("temp config path to string"),
        ]);

        let err =
            read_config_and_genesis(&args).expect_err("must reject disabled multilane config");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("nexus.enabled"),
            "missing nexus-enabled hint: {rendered}"
        );
    }
}

#[cfg(test)]
mod accel_tests {
    fn sha256_abc_digest() -> [u8; 32] {
        let mut state = [
            0x6a09_e667_u32,
            0xbb67_ae85,
            0x3c6e_f372,
            0xa54f_f53a,
            0x510e_527f,
            0x9b05_688c,
            0x1f83_d9ab,
            0x5be0_cd19,
        ];
        let mut block = [0u8; 64];
        block[0] = b'a';
        block[1] = b'b';
        block[2] = b'c';
        block[3] = 0x80;
        block[63] = 24;
        ivm::sha256_compress(&mut state, &block);
        let mut digest = [0u8; 32];
        for (i, w) in state.iter().enumerate() {
            digest[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
        }
        digest
    }

    const SHA256_ABC_EXPECTED: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];

    struct AccelTestGuard {
        original_config: ivm::AccelerationConfig,
        original_simd_override: Option<ivm::SimdChoice>,
        _simd_lock: std::sync::MutexGuard<'static, ()>,
    }

    impl AccelTestGuard {
        fn new() -> Self {
            let simd_lock = ivm::forced_simd_test_lock();
            let original_config = ivm::acceleration_config();
            let original_simd_override = ivm::set_forced_simd(None);

            Self {
                original_config,
                original_simd_override,
                _simd_lock: simd_lock,
            }
        }
    }

    impl Drop for AccelTestGuard {
        fn drop(&mut self) {
            ivm::set_acceleration_config(self.original_config);
            ivm::set_forced_simd(self.original_simd_override);
        }
    }

    #[test]
    fn accel_config_disables_cuda_parity_holds() {
        let _guard = AccelTestGuard::new();
        ivm::reset_cuda_backend_for_tests();
        let accel = iroha_config::parameters::actual::Acceleration {
            enable_simd: true,
            enable_cuda: false,
            enable_metal: true,
            max_gpus: None,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        };
        super::apply_ivm_acceleration_config(&accel);
        assert!(!ivm::cuda_available(), "CUDA should be disabled by config");
        if ivm::cuda_disabled() || ivm::cuda_available() {
            assert!(ivm::cuda_disabled(), "cuda_disabled flag should be set");
        }
        let mut state = [
            0x6a09_e667_u32,
            0xbb67_ae85,
            0x3c6e_f372,
            0xa54f_f53a,
            0x510e_527f,
            0x9b05_688c,
            0x1f83_d9ab,
            0x5be0_cd19,
        ];
        let mut block = [0u8; 64];
        block[0] = b'a';
        block[1] = b'b';
        block[2] = b'c';
        block[3] = 0x80;
        block[63] = 24;
        assert!(
            !ivm::sha256_compress_cuda(&mut state, &block),
            "CUDA helper should report false when disabled"
        );
        assert_eq!(sha256_abc_digest(), SHA256_ABC_EXPECTED);

        let restore = iroha_config::parameters::actual::Acceleration {
            enable_simd: true,
            enable_cuda: true,
            enable_metal: true,
            max_gpus: None,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        };
        super::apply_ivm_acceleration_config(&restore);
        ivm::reset_cuda_backend_for_tests();
    }

    #[test]
    fn accel_config_disables_simd_parity_holds() {
        let _guard = AccelTestGuard::new();
        let original = ivm::acceleration_config();
        let accel = iroha_config::parameters::actual::Acceleration {
            enable_simd: false,
            enable_cuda: true,
            enable_metal: true,
            max_gpus: None,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        };
        super::apply_ivm_acceleration_config(&accel);

        let status = ivm::acceleration_runtime_status();
        assert!(
            !status.simd.configured && !status.simd.available,
            "SIMD backend should be marked unavailable when disabled"
        );
        let result_scalar = ivm::vadd32([9, 8, 7, 6], [1, 2, 3, 4]);

        let restore = iroha_config::parameters::actual::Acceleration {
            enable_simd: true,
            enable_cuda: original.enable_cuda,
            enable_metal: original.enable_metal,
            max_gpus: original.max_gpus,
            merkle_min_leaves_gpu: original.merkle_min_leaves_gpu.unwrap_or(0),
            merkle_min_leaves_metal: original.merkle_min_leaves_metal,
            merkle_min_leaves_cuda: original.merkle_min_leaves_cuda,
            prefer_cpu_sha2_max_leaves_aarch64: original.prefer_cpu_sha2_max_leaves_aarch64,
            prefer_cpu_sha2_max_leaves_x86: original.prefer_cpu_sha2_max_leaves_x86,
        };
        super::apply_ivm_acceleration_config(&restore);
        let status_enabled = ivm::acceleration_runtime_status();
        assert!(status_enabled.simd.configured);
        let result_simd = ivm::vadd32([9, 8, 7, 6], [1, 2, 3, 4]);
        assert_eq!(
            result_scalar, result_simd,
            "SIMD disablement must not change vector results"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn accel_config_disables_metal_parity_holds() {
        let _guard = AccelTestGuard::new();
        ivm::reset_metal_backend_for_tests();
        if !ivm::metal_available() {
            return;
        }
        ivm::release_metal_state();
        let pre_compiles = ivm::bit_pipe_compile_count();
        let accel = iroha_config::parameters::actual::Acceleration {
            enable_simd: true,
            enable_cuda: true,
            enable_metal: false,
            max_gpus: None,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        };
        super::apply_ivm_acceleration_config(&accel);
        assert!(
            !ivm::metal_available(),
            "Metal should be disabled by config"
        );
        assert!(
            ivm::metal_disabled(),
            "Metal forced-disabled flag should be set"
        );
        let result = ivm::vadd32([1, 2, 3, 4], [4, 3, 2, 1]);
        assert_eq!(result, [5, 5, 5, 5]);
        assert_eq!(
            ivm::bit_pipe_compile_count(),
            pre_compiles,
            "Metal pipelines must not compile when disabled"
        );
        assert_eq!(sha256_abc_digest(), SHA256_ABC_EXPECTED);

        let restore = iroha_config::parameters::actual::Acceleration {
            enable_simd: true,
            enable_cuda: true,
            enable_metal: true,
            max_gpus: None,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        };
        super::apply_ivm_acceleration_config(&restore);
        ivm::reset_metal_backend_for_tests();
    }
}

fn log_config_warning(message: &str) {
    iroha_logger::warn!(target: "config", "{message}");
}

#[cfg(test)]
static INSTRUCTION_REGISTRY_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
fn instruction_registry_test_guard() -> std::sync::MutexGuard<'static, ()> {
    // `iroha_data_model` is linked into this unit-test binary as a normal
    // dependency, so its instruction registry is process-global. Serialize the
    // tests that intentionally clear the registry with the helpers that decode
    // genesis, otherwise parallel test threads can observe an empty registry.
    INSTRUCTION_REGISTRY_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn read_genesis(path: &Path) -> ReportResult<GenesisBlock, ConfigError> {
    #[cfg(test)]
    let _registry_guard = instruction_registry_test_guard();

    read_genesis_unlocked(path)
}

fn resolve_norito_max_archive_len(cfg: &Config) -> u64 {
    let requested = cfg.norito.max_archive_len;
    let max_frame_bytes = u64::try_from(cfg.network.max_frame_bytes).unwrap_or(u64::MAX);
    let resolved = requested.max(max_frame_bytes);

    if resolved != requested {
        iroha_logger::warn!(
            target: "config",
            requested,
            max_frame_bytes,
            resolved,
            "Norito max_archive_len too small for the configured network frame; increasing it so accepted frames remain decodable"
        );
    }

    resolved
}

/// Apply Norito operational configuration from config.
fn apply_norito_config(cfg: &Config) {
    let max_archive_len = resolve_norito_max_archive_len(cfg);
    norito::core::set_max_archive_len(max_archive_len);
    norito::core::hw::set_gpu_compression_allowed(cfg.norito.allow_gpu_compression);
}

fn validate_config(config: &Config) -> ReportResult<(), ConfigError> {
    validate_network_frame_runtime_limit(config)?;

    let mut emitter = Emitter::new();

    validate_config_io(&mut emitter, config);
    validate_config_runtime(&mut emitter, config);

    finish_config_validation(emitter)
}

/// Validate configuration without probing or binding any listening socket.
fn validate_config_offline(config: &Config) -> ReportResult<(), ConfigError> {
    validate_network_frame_runtime_limit(config)?;

    let mut emitter = Emitter::new();

    validate_config_static_io(&mut emitter, config);
    validate_config_runtime(&mut emitter, config);

    finish_config_validation(emitter)
}

/// Reject frame caps that cannot be encoded before any validation probes bind sockets.
fn validate_network_frame_runtime_limit(config: &Config) -> ReportResult<(), ConfigError> {
    let configured = config.network.max_frame_bytes;
    if configured > iroha_p2p::MAX_ENCRYPTED_FRAME_BYTES {
        return Err(Report::new(
            ConfigError::NetworkFrameSizeExceedsRuntimeLimit { configured },
        ));
    }

    let plaintext_ceiling = iroha_p2p::frame_plaintext_cap(configured);
    for (path, topic_cap) in [
        (
            "network.max_frame_bytes_consensus",
            config.network.max_frame_bytes_consensus,
        ),
        (
            "network.max_frame_bytes_control",
            config.network.max_frame_bytes_control,
        ),
        (
            "network.max_frame_bytes_block_sync",
            config.network.max_frame_bytes_block_sync,
        ),
        (
            "network.max_frame_bytes_tx_gossip",
            config.network.max_frame_bytes_tx_gossip,
        ),
        (
            "network.max_frame_bytes_peer_gossip",
            config.network.max_frame_bytes_peer_gossip,
        ),
        (
            "network.max_frame_bytes_health",
            config.network.max_frame_bytes_health,
        ),
        (
            "network.max_frame_bytes_other",
            config.network.max_frame_bytes_other,
        ),
    ] {
        if topic_cap > plaintext_ceiling {
            return Err(Report::new(
                ConfigError::NetworkTopicFrameSizeExceedsPlaintextLimit {
                    path,
                    configured: topic_cap,
                    plaintext_ceiling,
                    encrypted_cap: configured,
                },
            ));
        }
    }
    Ok(())
}

fn finish_config_validation(emitter: Emitter<ConfigError>) -> ReportResult<(), ConfigError> {
    if let Err(report) = emitter.into_result() {
        let mut collected: Vec<ConfigError> = report
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ConfigError>())
            .cloned()
            .collect();

        if let Some(mut aggregated) = collected.pop().map(Report::new) {
            while let Some(error) = collected.pop() {
                aggregated = aggregated.change_context(error);
            }
            return Err(aggregated.change_context(ConfigError::ParseConfig));
        }

        return Err(Report::new(ConfigError::ParseConfig));
    }

    Ok(())
}

fn validate_config_io(emitter: &mut Emitter<ConfigError>, config: &Config) {
    // These cause race condition in tests, due to them actually binding TCP listeners
    // Since these validations are primarily for the convenience of the end user,
    // it seems a fine compromise to run it only in release mode
    #[cfg(not(test))]
    {
        validate_try_bind_address(emitter, &config.network.address);
        validate_try_bind_address(emitter, &config.torii.address);
    }

    validate_config_static_io(emitter, config);
}

fn validate_config_static_io(emitter: &mut Emitter<ConfigError>, config: &Config) {
    validate_directory_path(emitter, &config.kura.store_dir);
    // maybe validate only if snapshot mode is enabled
    validate_directory_path(emitter, &config.snapshot.store_dir);

    if config.network.address.value() == config.torii.address.value() {
        emitter.emit(
            Report::new(ConfigError::SameNetworkAndToriiAddrs)
                .attach(config.network.address.clone().into_attachment())
                .attach(config.torii.address.clone().into_attachment()),
        );
    }
}

fn validate_config_runtime(emitter: &mut Emitter<ConfigError>, config: &Config) {
    #[cfg(not(feature = "embedded-soracloud-runtime"))]
    if config.soracloud_runtime.production_mode {
        emitter.emit(Report::new(ConfigError::SoracloudRuntimeFeatureRequired));
    }

    let sorafs_storage_enabled = config.torii.sorafs_storage.enabled;
    let sorafs_gateway_compliance_enabled = config.torii.sorafs_gateway.compliance.is_some();
    if sorafs_storage_enabled && !sorafs_gateway_compliance_enabled {
        emitter.emit(Report::new(ConfigError::SorafsStorageComplianceRequired));
    }
    if !sorafs_storage_enabled
        && (config.torii.sorafs_gateway.acme.enabled || sorafs_gateway_compliance_enabled)
    {
        emitter.emit(Report::new(ConfigError::SorafsGatewayRequiresStorage));
    }

    /// Warnings about unused configuration options are logged via the standard
    /// logger so that they are visible alongside other diagnostic messages.
    #[cfg(not(feature = "telemetry"))]
    if config.telemetry.is_some() {
        log_config_warning(
            "`telemetry` config is specified, but ignored, because Iroha is compiled without `telemetry` feature enabled",
        );
    }

    #[cfg(not(feature = "dev-telemetry"))]
    if config.dev_telemetry.out_file.is_some() {
        log_config_warning(
            "`dev_telemetry.out_file` config is specified, but ignored, because Iroha is compiled without `dev-telemetry` feature enabled",
        );
    }

    #[cfg(feature = "dev-telemetry")]
    if let Some(path) = &config.dev_telemetry.out_file {
        if path.value().parent().is_none() {
            emitter.emit(
                Report::new(ConfigError::TelemetryOutFileIsRootOrEmpty)
                    .attach(path.clone().into_attachment().display_path()),
            );
        }
        if path.value().is_dir() {
            emitter.emit(
                Report::new(ConfigError::TelemetryOutFileIsDir)
                    .attach(path.clone().into_attachment().display_path()),
            );
        }
    }

    if config.sumeragi.role == iroha_config::parameters::actual::NodeRole::Validator {
        if !config.confidential.enabled {
            emitter.emit(
                Report::new(ConfigError::ConfidentialDisabledForValidator).attach(
                    "validators must enable confidential verification or downgrade the node role to `Observer`",
                ),
            );
        }
        if config.confidential.assume_valid {
            emitter.emit(
                Report::new(ConfigError::ConfidentialAssumeValidForValidator).attach(
                    "validators cannot run with confidential observer mode; set `confidential.assume_valid = false`",
                ),
            );
        }
    }
}

fn validate_directory_path(emitter: &mut Emitter<ConfigError>, path: &WithOrigin<PathBuf>) {
    #[derive(Debug)]
    struct InvalidDirPathError {
        path: PathBuf,
    }

    impl core::fmt::Display for InvalidDirPathError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                f,
                "expected path to be either non-existing or a directory, but it points to an existing file: {}",
                self.path.display()
            )
        }
    }

    impl std::error::Error for InvalidDirPathError {}

    if path.value().is_file() {
        emitter.emit(
            Report::new(InvalidDirPathError {
                path: path.value().clone(),
            })
            .attach(path.clone().into_attachment().display_path())
            .change_context(ConfigError::InvalidDirPath),
        );
    }
}

#[cfg(not(test))]
fn validate_try_bind_address(_emitter: &mut Emitter<ConfigError>, value: &WithOrigin<SocketAddr>) {
    use std::net::TcpListener;

    if let Err(err) = TcpListener::bind(value.value()) {
        iroha_logger::warn!(addr = %value.value(), raw = ?err.raw_os_error(), err = ?err, "Skipping bind validation after failure");
    }
}

/// Configures globals of [`error_stack::Report`]
fn configure_reports(args: &Args) {
    use std::panic::Location;

    use error_stack::{Report, fmt::ColorMode};

    Report::set_color_mode(if args.terminal_colors {
        ColorMode::Color
    } else {
        ColorMode::None
    });

    // neither devs nor users benefit from it
    Report::install_debug_hook::<Location>(|_, _| {});
}

const BUILD_LINE_ENV: &str = "IROHA_BUILD_LINE";

fn resolve_build_line_from_env(env_value: Option<String>, bin_name: &str) -> BuildLine {
    if let Some(val) = env_value {
        match val.trim().to_ascii_lowercase().as_str() {
            "iroha2" | "i2" | "2" => return BuildLine::Iroha2,
            "iroha3" | "i3" | "3" => return BuildLine::Iroha3,
            other => iroha_logger::warn!(
                target: "config",
                ?other,
                "Ignoring invalid {BUILD_LINE_ENV} override (expected iroha2/iroha3); falling back to binary name"
            ),
        }
    }
    BuildLine::from_bin_name(bin_name)
}

/// Run the stock daemon launcher for one build line.
///
/// The stock binaries resolve supported runtime-only bindings through a
/// platform-fixed local broker running under the same effective service UID.
/// The broker is not contacted when the validated configuration contains no
/// runtime-provider bindings.
pub fn main_entry(default_build_line: BuildLine) {
    let _ = std::hint::black_box(BUILD_SOURCE_ID);
    let build_line = resolve_build_line_from_env(
        env::var(BUILD_LINE_ENV).ok(),
        default_build_line.daemon_bin(),
    );
    if let Err(report) = run_main(build_line, None, None) {
        eprintln!("{report:?}");
        std::process::exit(1);
    }
}

/// Run the standard CLI launcher with a deployment-owned provider registry.
///
/// An external deployment binary can call this function after constructing a
/// registry backed by PKCS#11, managed KMS, `WebAuthn`, authenticated HTTPS, or
/// another reviewed runtime. Registry selection is an explicit launcher
/// decision; no environment or config value dynamically loads executable
/// provider code.
///
/// # Errors
///
/// Returns a launcher error if configuration, provider resolution, subsystem
/// startup, or supervised execution fails.
pub fn run_with_runtime_provider_registry(
    default_build_line: BuildLine,
    registry: &dyn IrohaRuntimeProviderRegistryV1,
) -> ReportResult<(), MainError> {
    let build_line = resolve_build_line_from_env(
        env::var(BUILD_LINE_ENV).ok(),
        default_build_line.daemon_bin(),
    );
    run_main(build_line, Some(registry), None)
}

/// Run the standard CLI launcher with a deployment-owned private Musubi publication factory.
///
/// The caller supplies a one-shot factory which receives the exact daemon-owned finalized-state,
/// transaction-queue, and SoraFS handles only after trusted startup replay. The factory must
/// assemble the complete private HTTPS runner, including its durable clock and journal, receipt
/// signer, and admitted SoraFS backends. The launcher transfers that opaque factory into the
/// daemon supervisor without requiring an unrelated runtime-provider registry. It never exposes
/// the private routes through Torii or reads service credentials from argv or node configuration.
/// An unexpected private-runner exit is fatal to the same supervisor that owns the node.
///
/// # Errors
///
/// Returns a launcher error if configuration, subsystem startup, or supervised execution fails.
pub fn run_with_musubi_publication(
    default_build_line: BuildLine,
    factory: Box<dyn musubi_publication_service::MusubiPublicationPrivateServiceFactoryV1>,
) -> ReportResult<(), MainError> {
    let build_line = resolve_build_line_from_env(
        env::var(BUILD_LINE_ENV).ok(),
        default_build_line.daemon_bin(),
    );
    run_main(build_line, None, Some(factory))
}

/// Run the standard CLI launcher with deployment-owned runtime providers and a private Musubi
/// publication factory.
///
/// The one-shot factory receives the exact daemon-owned finalized-state, transaction-queue, and
/// SoraFS handles only after trusted startup replay. It must assemble the complete private HTTPS
/// runner, including its durable clock and journal, receipt signer, and admitted SoraFS backends,
/// while retaining every credential inside deployment-owned adapters. The launcher only transfers
/// that opaque factory into the daemon supervisor; it never exposes the private routes through
/// Torii or reads service credentials from argv or node configuration. An unexpected private-runner
/// exit is fatal to the same supervisor that owns the node.
///
/// # Errors
///
/// Returns a launcher error if configuration, provider resolution, subsystem startup, or
/// supervised execution fails.
pub fn run_with_runtime_provider_registry_and_musubi_publication(
    default_build_line: BuildLine,
    registry: &dyn IrohaRuntimeProviderRegistryV1,
    factory: Box<dyn musubi_publication_service::MusubiPublicationPrivateServiceFactoryV1>,
) -> ReportResult<(), MainError> {
    let build_line = resolve_build_line_from_env(
        env::var(BUILD_LINE_ENV).ok(),
        default_build_line.daemon_bin(),
    );
    run_main(build_line, Some(registry), Some(factory))
}

fn parse_fastpq_execution_mode(value: &str) -> Result<FastpqExecutionMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Ok(FastpqExecutionMode::Cpu),
        "gpu" => Ok(FastpqExecutionMode::Gpu),
        _ => Err("expected MODE to be one of: cpu, gpu".to_string()),
    }
}

fn parse_fastpq_poseidon_mode(value: &str) -> Result<FastpqPoseidonMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Ok(FastpqPoseidonMode::Cpu),
        "gpu" => Ok(FastpqPoseidonMode::Gpu),
        _ => Err("expected MODE to be one of: cpu, gpu".to_string()),
    }
}

fn parse_args() -> Args {
    parse_args_from(env::args_os())
}

fn parse_args_from<I, T>(args: I) -> Args
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut iter = args.into_iter().map(Into::into);
    let mut filtered = Vec::new();
    if let Some(binary) = iter.next() {
        filtered.push(binary);
    } else {
        filtered.push(OsString::from("iroha3d"));
    }
    filtered.extend(iter.filter_map(|arg| {
        let display = arg.to_string_lossy();
        let trimmed = display.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.len() == display.len() {
            return Some(arg);
        }
        match display {
            Cow::Borrowed(_) => Some(OsString::from(trimmed)),
            Cow::Owned(_) => Some(arg),
        }
    }));
    Args::parse_from(filtered)
}

#[cfg(feature = "telemetry")]
#[derive(Clone)]
struct FastpqDeviceLabels {
    device_class: Arc<str>,
    chip_family: Arc<str>,
    gpu_kind: Arc<str>,
}

#[cfg(feature = "telemetry")]
impl FastpqDeviceLabels {
    fn from_config(config: &iroha_config::parameters::actual::Fastpq) -> Self {
        Self {
            device_class: normalize_fastpq_label(config.device_class.clone(), "unknown"),
            chip_family: normalize_fastpq_label(config.chip_family.clone(), "unknown"),
            gpu_kind: normalize_fastpq_label(config.gpu_kind.clone(), "unknown"),
        }
    }
}

#[cfg(feature = "telemetry")]
fn normalize_fastpq_label(label: Option<String>, fallback: &str) -> Arc<str> {
    label
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
        .map_or_else(|| Arc::from(fallback), Arc::from)
}

#[cfg(feature = "telemetry")]
fn install_fastpq_execution_mode_probe(labels: &FastpqDeviceLabels) {
    let telemetry_labels = labels.clone();
    fastpq_prover::set_execution_mode_observer(move |requested, resolved, backend| {
        let backend_label = backend.map_or("none", |kind| kind.as_str());
        let metrics = iroha_telemetry::metrics::global_or_default();
        metrics.record_fastpq_execution_mode(
            requested.as_str(),
            resolved.as_str(),
            backend_label,
            telemetry_labels.device_class.as_ref(),
            telemetry_labels.chip_family.as_ref(),
            telemetry_labels.gpu_kind.as_ref(),
        );
    });
}

#[cfg(feature = "fastpq-gpu")]
fn preflight_fastpq_bn254_poseidon_words(config: &iroha_config::parameters::actual::Fastpq) {
    if !fastpq_poseidon_word_preflight_enabled(config) {
        iroha_logger::debug!(
            target: "fastpq",
            "BN254 Poseidon word-batch GPU preflight skipped by FASTPQ config"
        );
        return;
    }

    if fastpq_prover::preflight_bn254_poseidon_word_batches() {
        iroha_logger::info!(
            target: "fastpq",
            "BN254 Poseidon word-batch GPU preflight passed"
        );
    } else {
        iroha_logger::debug!(
            target: "fastpq",
            "BN254 Poseidon word-batch GPU preflight unavailable; scalar fallback remains active"
        );
    }
}

#[cfg(feature = "fastpq-gpu")]
fn fastpq_poseidon_word_preflight_enabled(
    config: &iroha_config::parameters::actual::Fastpq,
) -> bool {
    match config.poseidon_mode {
        FastpqPoseidonMode::Cpu => false,
        FastpqPoseidonMode::Gpu => true,
    }
}

#[cfg(feature = "telemetry")]
fn install_fastpq_poseidon_probe(labels: &FastpqDeviceLabels) {
    let telemetry_labels = labels.clone();
    fastpq_prover::set_poseidon_pipeline_observer(move |policy, path, _backend| {
        let metrics = iroha_telemetry::metrics::global_or_default();
        metrics.record_fastpq_poseidon_mode(
            policy.requested().as_str(),
            policy.resolved().as_str(),
            path,
            telemetry_labels.device_class.as_ref(),
            telemetry_labels.chip_family.as_ref(),
            telemetry_labels.gpu_kind.as_ref(),
        );
    });
}

#[cfg(feature = "telemetry")]
fn install_fastpq_gpu_event_probe(labels: &FastpqDeviceLabels) {
    let telemetry_labels = labels.clone();
    fastpq_prover::set_poseidon_gpu_event_observer(move |accelerator, event, reason, backend| {
        let gpu_kind =
            backend.map_or_else(|| telemetry_labels.gpu_kind.as_ref(), |kind| kind.as_str());
        let metrics = iroha_telemetry::metrics::global_or_default();
        match event {
            "disabled" => metrics.inc_fastpq_gpu_disable(
                accelerator,
                reason,
                telemetry_labels.device_class.as_ref(),
                telemetry_labels.chip_family.as_ref(),
                gpu_kind,
            ),
            "sampled_parity_failure" => metrics.inc_fastpq_gpu_parity_failure(
                accelerator,
                reason,
                telemetry_labels.device_class.as_ref(),
                telemetry_labels.chip_family.as_ref(),
                gpu_kind,
            ),
            _ => {}
        }
    });
}

#[cfg(all(feature = "telemetry", feature = "fastpq-gpu", target_os = "macos"))]
fn install_fastpq_queue_probe(labels: FastpqDeviceLabels) {
    use fastpq_prover::{
        enable_lde_host_stats, enable_queue_depth_stats, snapshot_queue_depth_stats,
        take_lde_host_stats,
    };
    use iroha_telemetry::metrics::{
        FastpqMetalQueueLaneSample, FastpqMetalQueueSample, global_or_default,
    };
    use std::{sync::Arc, thread, time::Duration};

    enable_queue_depth_stats(true);
    enable_lde_host_stats(true);
    let labels = Arc::new(labels);

    thread::Builder::new()
        .name("fastpq-queue-telemetry".into())
        .spawn(move || {
            let metrics = global_or_default();
            let mut lane_buffer = Vec::new();
            loop {
                thread::sleep(Duration::from_secs(5));
                if let Some(stats) = snapshot_queue_depth_stats() {
                    lane_buffer.clear();
                    for lane in &stats.queues {
                        lane_buffer.push(FastpqMetalQueueLaneSample {
                            index: lane.index as usize,
                            dispatch_count: u64::from(lane.dispatch_count),
                            max_in_flight: u64::from(lane.max_in_flight),
                            busy_ms: lane.busy_ms,
                            overlap_ms: lane.overlap_ms,
                        });
                    }
                    let sample = FastpqMetalQueueSample {
                        limit: u64::from(stats.limit),
                        max_in_flight: u64::from(stats.max_in_flight),
                        dispatch_count: u64::from(stats.dispatch_count),
                        window_ms: stats.window_ms,
                        busy_ms: stats.busy_ms,
                        overlap_ms: stats.overlap_ms,
                        lanes: &lane_buffer,
                    };
                    metrics.record_fastpq_metal_queue_stats(
                        labels.device_class.as_ref(),
                        labels.chip_family.as_ref(),
                        labels.gpu_kind.as_ref(),
                        &sample,
                    );
                }
                while let Some(stats) = take_lde_host_stats() {
                    metrics.record_fastpq_zero_fill(
                        labels.device_class.as_ref(),
                        labels.chip_family.as_ref(),
                        labels.gpu_kind.as_ref(),
                        stats.zero_fill_ms,
                        stats.zero_fill_bytes as u64,
                    );
                }
            }
        })
        .expect("spawn FASTPQ Metal queue telemetry thread");
}

fn run_main(
    build_line: BuildLine,
    runtime_provider_registry: Option<&dyn IrohaRuntimeProviderRegistryV1>,
    musubi_publication_factory: Option<
        Box<dyn musubi_publication_service::MusubiPublicationPrivateServiceFactoryV1>,
    >,
) -> ReportResult<(), MainError> {
    let args = parse_args();

    let lang = i18n::detect_language(args.language.as_deref());
    i18n::init(lang);

    configure_reports(&args);

    if args.startup.trace_config {
        iroha_config::enable_tracing()
            .change_context(MainError::TraceConfigSetup)
            .attach("was enabled by `--trace-config` argument")?;
    }

    // Ensure the instruction registry is initialized **before** we attempt to
    // read and decode the genesis block. Without this call, decoding the
    // embedded `InstructionBox` values would panic with "instruction registry
    // is not initialized".
    init_genesis_instruction_registry();
    init_query_registry();

    let (mut config, genesis) =
        read_config_and_genesis(&args).change_context(MainError::Config).attach_with(|| {
            args.config.as_ref().map_or_else(
                || "`--config` arg was not set, therefore configuration relies fully on environment variables".to_owned(),
                |path| format!("config path is specified by `--config` arg: {}", path.display()),
            )
        })?;

    enforce_build_line(build_line, &mut config)?;
    if args.startup.check_config {
        let qualification_seal_target = args
            .startup
            .write_kagemusha_catalog_qualification_seal
            .as_deref()
            .map(|path| QualificationSealPublicationTarget::prepare(&config, path))
            .transpose()
            .map_err(|error| {
                Report::new(MainError::Config).attach(format!(
                    "invalid Kagemusha catalog qualification seal output: {error}"
                ))
            })?;
        let qualification_seal = validate_config_for_check(
            &config,
            genesis.as_ref(),
            qualification_seal_target.is_some(),
        )?;
        if let Some(target) = qualification_seal_target {
            let seal = qualification_seal.ok_or_else(|| {
                Report::new(MainError::Config)
                    .attach("Kagemusha catalog qualification completed without producing a seal")
            })?;
            let output_path = target.path().to_owned();
            target.publish_and_verify(&config, &seal).map_err(|error| {
                Report::new(MainError::Config).attach(format!(
                    "failed to publish Kagemusha catalog qualification seal: {error}"
                ))
            })?;
            println!(
                "Published: canonical Kagemusha catalog qualification seal at {}",
                output_path.display()
            );
        }
        if genesis.is_some() {
            println!("Ready: configuration and available genesis are valid");
        } else {
            println!(
                "Pending: static configuration is valid; genesis/bootstrap state is not locally available"
            );
        }
        return Ok(());
    }
    // Resolve deployment-owned executable providers only after the complete
    // static configuration has passed the same offline checks as
    // `--check-config`, and before Tokio or node-owned durable state starts.
    validate_config_offline(&config).change_context(MainError::Config)?;
    let stock_runtime_provider_registry = if runtime_provider_registry.is_none() {
        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .map_err(Report::new)
            .change_context(MainError::Config)
            .attach("failed to project deployment runtime-provider bindings")?;
        (!bindings.is_empty())
            .then(runtime_provider_broker::StockRuntimeProviderBrokerRegistryV1::new)
    } else {
        None
    };
    let runtime_provider_registry = runtime_provider_registry.or_else(|| {
        stock_runtime_provider_registry.as_ref().map(|registry| {
            let registry: &dyn IrohaRuntimeProviderRegistryV1 = registry;
            registry
        })
    });
    let runtime_deps =
        runtime_provider_registry::resolve_runtime_deps(&config, runtime_provider_registry)
            .map_err(Report::new)
            .change_context(MainError::Config)
            .attach("failed to resolve deployment runtime-provider bindings")?;
    iroha_logger::info!(
        target: "config",
        build_line = %build_line,
        protocol_version = u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION),
        "Resolved build line; Sumeragi v2 data availability is mandatory"
    );

    #[cfg(feature = "telemetry")]
    let fastpq_device_labels = FastpqDeviceLabels::from_config(&config.zk.fastpq);
    #[cfg(feature = "telemetry")]
    install_fastpq_execution_mode_probe(&fastpq_device_labels);
    #[cfg(feature = "telemetry")]
    install_fastpq_poseidon_probe(&fastpq_device_labels);
    #[cfg(feature = "telemetry")]
    install_fastpq_gpu_event_probe(&fastpq_device_labels);
    #[cfg(all(feature = "telemetry", feature = "fastpq-gpu", target_os = "macos"))]
    install_fastpq_queue_probe(fastpq_device_labels.clone());

    // Concurrency configuration: set global Rayon pool and IVM scheduler limits.
    let min = if config.concurrency.scheduler_min_threads == 0 {
        // auto (physical cores) — defer to IVM internals
        0
    } else {
        config.concurrency.scheduler_min_threads
    };
    let max = if config.concurrency.scheduler_max_threads == 0 {
        // auto
        0
    } else {
        config.concurrency.scheduler_max_threads
    };
    // Build Tokio runtime with a conservative number of worker threads to avoid
    // oversubscription with the IVM scheduler. Keep a slightly higher minimum
    // to prevent HTTP/p2p tasks from starving under consensus body/chunk load, and use the
    // available parallelism as the auto baseline instead of a fixed floor.
    let auto_budget = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4);
    let budget = if max > 0 {
        max
    } else if min > 0 {
        min
    } else {
        auto_budget
    };
    let tokio_workers = budget.clamp(4, 16);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(tokio_workers)
        .thread_stack_size(config.concurrency.tokio_stack_bytes)
        .enable_all()
        .build()
        .map_err(Report::from)
        .change_context(MainError::IrohaStart)?;

    let result = rt.block_on(run_node(
        config,
        genesis,
        runtime_deps,
        musubi_publication_factory,
    ));
    rt.shutdown_timeout(NODE_RUNTIME_SHUTDOWN_TIMEOUT);
    result
}

/// Pinned, validated destination for one root-owned catalog qualification seal.
///
/// Preparation happens before the expensive qualification pass and keeps the
/// destination directory open until publication. The final name must not
/// exist: qualification seals are immutable release artifacts and are never
/// replaced in place.
struct QualificationSealPublicationTarget {
    path: PathBuf,
    #[cfg(unix)]
    parent: fs::File,
    #[cfg(unix)]
    file_name: OsString,
    #[cfg(unix)]
    expected_uid: u32,
}

#[cfg(target_os = "macos")]
const QUALIFICATION_SEAL_MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[cfg(target_os = "macos")]
fn run_bounded_macos_acl_command(
    program: &str,
    option: &str,
    path: &Path,
    label: &str,
) -> Result<std::process::Output, String> {
    let output = std::process::Command::new(program)
        .arg(option)
        .arg(path)
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .map_err(|error| {
            format!(
                "failed to run macOS ACL command for {label} `{}`: {error}",
                path.display()
            )
        })?;
    if output.stdout.len() > QUALIFICATION_SEAL_MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
        || output.stderr.len() > QUALIFICATION_SEAL_MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
    {
        return Err(format!(
            "macOS ACL command output exceeded its bound for {label} `{}`",
            path.display()
        ));
    }
    if !output.status.success() {
        return Err(format!(
            "macOS ACL command failed for {label} `{}`: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

#[cfg(target_os = "macos")]
fn require_no_macos_extended_acl(path: &Path, label: &str) -> Result<(), String> {
    let output = run_bounded_macos_acl_command("/bin/ls", "-ldeq", path, label)?;
    let newline_count = output.stdout.iter().filter(|byte| **byte == b'\n').count();
    if !output.stderr.is_empty() || !output.stdout.ends_with(b"\n") || newline_count != 1 {
        return Err(format!(
            "{label} `{}` must not have an extended ACL",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn clear_macos_extended_acl(path: &Path, label: &str) -> Result<(), String> {
    let output = run_bounded_macos_acl_command("/bin/chmod", "-N", path, label)?;
    if !output.stdout.is_empty() || !output.stderr.is_empty() {
        return Err(format!(
            "macOS ACL removal produced unexpected output for {label} `{}`",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn same_qualification_seal_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(target_os = "macos")]
fn require_acl_free_pinned_path(opened: &fs::File, path: &Path, label: &str) -> Result<(), String> {
    let opened_before = opened
        .metadata()
        .map_err(|error| format!("failed to inspect pinned {label}: {error}"))?;
    let path_before = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
    if !same_qualification_seal_metadata(&opened_before, &path_before) {
        return Err(format!(
            "{label} `{}` no longer identifies the pinned inode",
            path.display()
        ));
    }
    require_no_macos_extended_acl(path, label)?;
    let opened_after = opened
        .metadata()
        .map_err(|error| format!("failed to re-inspect pinned {label}: {error}"))?;
    let path_after = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to re-inspect {label} `{}` after ACL validation: {error}",
            path.display()
        )
    })?;
    if !same_qualification_seal_metadata(&opened_before, &opened_after)
        || !same_qualification_seal_metadata(&opened_after, &path_after)
    {
        return Err(format!(
            "{label} `{}` changed during ACL validation",
            path.display()
        ));
    }
    Ok(())
}

impl QualificationSealPublicationTarget {
    fn prepare(config: &Config, requested_path: &Path) -> Result<Self, String> {
        let configured_path = config
            .settlement
            .offline
            .kagemusha_catalog_qualification_seal_path
            .as_deref()
            .ok_or_else(|| {
                "`--write-kagemusha-catalog-qualification-seal` requires settlement.offline.kagemusha_catalog_qualification_seal_path"
                    .to_owned()
            })?;
        validate_canonical_absolute_path(configured_path, "configured qualification seal path")?;
        validate_canonical_absolute_path(requested_path, "requested qualification seal path")?;
        if requested_path != configured_path {
            return Err(format!(
                "requested qualification seal path `{}` does not exactly match configured path `{}`",
                requested_path.display(),
                configured_path.display()
            ));
        }
        validate_qualification_seal_directory_separation(config, requested_path)?;

        #[cfg(unix)]
        {
            let effective_uid = rustix::process::geteuid().as_raw();
            if effective_uid != 0 {
                return Err(format!(
                    "qualification seal publication requires effective uid 0, got {effective_uid}"
                ));
            }
            Self::prepare_for_owner(requested_path, 0)
        }
        #[cfg(not(unix))]
        {
            let _ = requested_path;
            Err(
                "root-owned atomic qualification seal publication is unsupported on this platform"
                    .to_owned(),
            )
        }
    }

    #[must_use]
    fn path(&self) -> &Path {
        &self.path
    }

    fn publish_and_verify(
        self,
        config: &Config,
        seal: &iroha_core::smartcontracts::isi::offline::KagemushaCatalogQualificationSealV1,
    ) -> Result<(), String> {
        #[cfg(unix)]
        {
            let canonical_bytes = seal.canonical_bytes()?;
            self.publish_bytes_and_verify(&canonical_bytes, |final_path| {
                if final_path
                    != config
                        .settlement
                        .offline
                        .kagemusha_catalog_qualification_seal_path
                        .as_deref()
                        .expect("publication target requires a configured seal path")
                {
                    return Err(
                        "published qualification seal path no longer matches configuration"
                            .to_owned(),
                    );
                }
                load_configured_kagemusha_release_catalog(config).map(|_| ())
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (config, seal);
            Err(
                "root-owned atomic qualification seal publication is unsupported on this platform"
                    .to_owned(),
            )
        }
    }

    #[cfg(unix)]
    fn prepare_for_owner(path: &Path, expected_uid: u32) -> Result<Self, String> {
        use std::os::unix::fs::MetadataExt as _;

        validate_canonical_absolute_path(path, "qualification seal path")?;
        let file_name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "qualification seal path must end in a file name".to_owned())?
            .to_owned();
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| "qualification seal path must have an absolute parent".to_owned())?;

        let mut ancestors = parent_path.ancestors().collect::<Vec<_>>();
        ancestors.reverse();
        for ancestor in ancestors {
            let metadata = fs::symlink_metadata(ancestor).map_err(|error| {
                format!(
                    "failed to inspect qualification seal parent `{}`: {error}",
                    ancestor.display()
                )
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "qualification seal parent chain contains a symlink or non-directory `{}`",
                    ancestor.display()
                ));
            }
            if metadata.uid() != 0 && metadata.uid() != expected_uid {
                return Err(format!(
                    "qualification seal parent `{}` is owned by untrusted uid {}",
                    ancestor.display(),
                    metadata.uid()
                ));
            }
            if metadata.mode() & 0o022 != 0 {
                return Err(format!(
                    "qualification seal parent `{}` is group- or world-writable",
                    ancestor.display()
                ));
            }
            #[cfg(target_os = "macos")]
            {
                require_no_macos_extended_acl(ancestor, "qualification seal parent")?;
                let after = fs::symlink_metadata(ancestor).map_err(|error| {
                    format!(
                        "failed to re-inspect qualification seal parent `{}` after ACL validation: {error}",
                        ancestor.display()
                    )
                })?;
                if !same_qualification_seal_metadata(&metadata, &after) {
                    return Err(format!(
                        "qualification seal parent `{}` changed during ACL validation",
                        ancestor.display()
                    ));
                }
            }
        }

        let direct_parent = fs::symlink_metadata(parent_path).map_err(|error| {
            format!(
                "failed to inspect qualification seal destination directory `{}`: {error}",
                parent_path.display()
            )
        })?;
        if direct_parent.uid() != expected_uid {
            return Err(format!(
                "qualification seal destination directory `{}` must be owned by uid {expected_uid}",
                parent_path.display()
            ));
        }

        let parent = fs::File::from(
            rustix::fs::open(
                parent_path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|error| {
                format!(
                    "failed to pin qualification seal destination directory `{}`: {error}",
                    parent_path.display()
                )
            })?,
        );
        let opened = parent.metadata().map_err(|error| {
            format!("failed to inspect pinned qualification seal destination directory: {error}")
        })?;
        let current = fs::symlink_metadata(parent_path).map_err(|error| {
            format!("failed to re-inspect qualification seal destination directory: {error}")
        })?;
        if !opened.is_dir()
            || opened.dev() != current.dev()
            || opened.ino() != current.ino()
            || opened.uid() != expected_uid
            || opened.mode() & 0o022 != 0
        {
            return Err(
                "qualification seal destination directory changed or became untrusted while opening"
                    .to_owned(),
            );
        }
        #[cfg(target_os = "macos")]
        require_acl_free_pinned_path(
            &parent,
            parent_path,
            "qualification seal destination directory",
        )?;
        match rustix::fs::statat(&parent, &file_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => {
                return Err(format!(
                    "qualification seal destination already exists and will not be replaced: {}",
                    path.display()
                ));
            }
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect qualification seal destination `{}`: {error}",
                    path.display()
                ));
            }
        }

        Ok(Self {
            path: path.to_owned(),
            parent,
            file_name,
            expected_uid,
        })
    }

    #[cfg(unix)]
    #[allow(clippy::too_many_lines)]
    fn publish_bytes_and_verify(
        self,
        canonical_bytes: &[u8],
        verify_final: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        use std::{
            io::{Read as _, Seek as _, SeekFrom, Write as _},
            os::unix::fs::MetadataExt as _,
        };

        if canonical_bytes.is_empty() {
            return Err("canonical qualification seal bytes must not be empty".to_owned());
        }
        self.verify_parent_identity()?;

        let mut nonce = [0_u8; 16];
        rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut nonce).map_err(|error| {
            format!(
                "operating-system randomness unavailable for qualification seal staging: {error}"
            )
        })?;
        let staging_name =
            OsString::from(format!(".irohad-kagemusha-seal-{}.tmp", hex::encode(nonce)));
        #[cfg(target_os = "macos")]
        let staging_path = self
            .path
            .parent()
            .expect("prepared qualification seal path has a parent")
            .join(&staging_name);
        let mut staging = fs::File::from(
            rustix::fs::openat(
                &self.parent,
                &staging_name,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::from_raw_mode(0o600),
            )
            .map_err(|error| {
                format!("failed to create exclusive qualification seal staging file: {error}")
            })?,
        );

        let staged_result = (|| -> Result<(u64, u64), String> {
            staging.write_all(canonical_bytes).map_err(|error| {
                format!("failed to write qualification seal staging file: {error}")
            })?;
            staging.flush().map_err(|error| {
                format!("failed to flush qualification seal staging file: {error}")
            })?;
            staging.sync_all().map_err(|error| {
                format!("failed to sync qualification seal staging file: {error}")
            })?;
            rustix::fs::fchmod(&staging, rustix::fs::Mode::from_raw_mode(0o444)).map_err(
                |error| {
                    format!("failed to make qualification seal staging file immutable: {error}")
                },
            )?;
            staging.sync_all().map_err(|error| {
                format!("failed to sync immutable qualification seal staging file: {error}")
            })?;

            let before_acl_clear = staging.metadata().map_err(|error| {
                format!("failed to inspect qualification seal staging file: {error}")
            })?;
            let named_before_acl_clear = rustix::fs::statat(
                &self.parent,
                &staging_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| {
                format!(
                    "failed to bind qualification seal staging name before ACL removal: {error}"
                )
            })?;
            if u64::try_from(named_before_acl_clear.st_dev).ok() != Some(before_acl_clear.dev())
                || u64::try_from(named_before_acl_clear.st_ino).ok() != Some(before_acl_clear.ino())
                || u64::try_from(named_before_acl_clear.st_nlink).ok() != Some(1)
            {
                return Err("qualification seal staging name changed before ACL removal".to_owned());
            }
            #[cfg(target_os = "macos")]
            {
                clear_macos_extended_acl(&staging_path, "qualification seal staging file")?;
                require_acl_free_pinned_path(
                    &staging,
                    &staging_path,
                    "qualification seal staging file",
                )?;
            }

            let metadata = staging.metadata().map_err(|error| {
                format!("failed to inspect ACL-free qualification seal staging file: {error}")
            })?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || metadata.uid() != self.expected_uid
                || metadata.mode() & 0o7777 != 0o444
                || metadata.len()
                    != u64::try_from(canonical_bytes.len())
                        .map_err(|_| "qualification seal length does not fit u64".to_owned())?
            {
                return Err(
                    "qualification seal staging file ownership, mode, links, or length is invalid"
                        .to_owned(),
                );
            }
            let named = rustix::fs::statat(
                &self.parent,
                &staging_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| format!("failed to bind qualification seal staging name: {error}"))?;
            if u64::try_from(named.st_dev).ok() != Some(metadata.dev())
                || u64::try_from(named.st_ino).ok() != Some(metadata.ino())
                || u64::try_from(named.st_nlink).ok() != Some(1)
            {
                return Err(
                    "qualification seal staging name no longer identifies the opened file"
                        .to_owned(),
                );
            }

            staging.seek(SeekFrom::Start(0)).map_err(|error| {
                format!("failed to rewind qualification seal staging file: {error}")
            })?;
            let mut readback = Vec::with_capacity(canonical_bytes.len());
            staging.read_to_end(&mut readback).map_err(|error| {
                format!("failed to read back qualification seal staging file: {error}")
            })?;
            if readback != canonical_bytes {
                return Err(
                    "qualification seal staging file did not round-trip canonical bytes".to_owned(),
                );
            }
            self.verify_parent_identity()?;
            self.parent.sync_all().map_err(|error| {
                format!("failed to sync qualification seal destination before publication: {error}")
            })?;
            match rustix::fs::statat(
                &self.parent,
                &self.file_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Err(error) if error == rustix::io::Errno::NOENT => {}
                Ok(_) => {
                    return Err(format!(
                        "qualification seal destination appeared during qualification and will not be replaced: {}",
                        self.path.display()
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "failed to recheck qualification seal destination: {error}"
                    ));
                }
            }
            Ok((metadata.dev(), metadata.ino()))
        })();

        let (staged_device, staged_inode) = match staged_result {
            Ok(identity) => identity,
            Err(error) => {
                drop(staging);
                let _ =
                    rustix::fs::unlinkat(&self.parent, &staging_name, rustix::fs::AtFlags::empty());
                let _ = self.parent.sync_all();
                return Err(error);
            }
        };

        if let Err(error) = rustix::fs::renameat_with(
            &self.parent,
            &staging_name,
            &self.parent,
            &self.file_name,
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            drop(staging);
            let _ = rustix::fs::unlinkat(&self.parent, &staging_name, rustix::fs::AtFlags::empty());
            let _ = self.parent.sync_all();
            return Err(format!(
                "failed to publish qualification seal without replacement: {error}"
            ));
        }

        let final_result = (|| -> Result<(), String> {
            let published = rustix::fs::statat(
                &self.parent,
                &self.file_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|error| format!("failed to inspect published qualification seal: {error}"))?;
            if u64::try_from(published.st_dev).ok() != Some(staged_device)
                || u64::try_from(published.st_ino).ok() != Some(staged_inode)
                || u64::try_from(published.st_nlink).ok() != Some(1)
                || u32::try_from(published.st_uid).ok() != Some(self.expected_uid)
                || u32::try_from(published.st_mode).map_or(true, |mode| mode & 0o7777 != 0o444)
            {
                return Err(
                    "published qualification seal does not match the staged immutable inode"
                        .to_owned(),
                );
            }
            #[cfg(target_os = "macos")]
            require_acl_free_pinned_path(&staging, &self.path, "published qualification seal")?;
            self.parent.sync_all().map_err(|error| {
                format!("failed to sync qualification seal destination after publication: {error}")
            })?;
            verify_final(&self.path)?;
            self.verify_parent_identity()
        })();

        if let Err(error) = final_result {
            let cleanup = match rustix::fs::statat(
                &self.parent,
                &self.file_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Ok(current)
                    if u64::try_from(current.st_dev).ok() == Some(staged_device)
                        && u64::try_from(current.st_ino).ok() == Some(staged_inode) =>
                {
                    rustix::fs::unlinkat(
                        &self.parent,
                        &self.file_name,
                        rustix::fs::AtFlags::empty(),
                    )
                    .map(|()| true)
                    .map_err(|cleanup_error| cleanup_error.to_string())
                }
                Ok(_) => Err(
                    "refused to remove a final path that no longer identifies the published inode"
                        .to_owned(),
                ),
                Err(cleanup_error) if cleanup_error == rustix::io::Errno::NOENT => Ok(false),
                Err(cleanup_error) => Err(cleanup_error.to_string()),
            };
            let sync = self
                .parent
                .sync_all()
                .map_err(|sync_error| sync_error.to_string());
            return match (cleanup, sync) {
                (Ok(true), Ok(())) => Err(format!(
                    "{error}; removed the newly published seal because final verification failed"
                )),
                (Ok(false), Ok(())) => Err(format!(
                    "{error}; the newly published seal was already absent after final verification failed"
                )),
                (cleanup, sync) => Err(format!(
                    "{error}; failed to durably remove the newly published seal after verification failure (unlink={cleanup:?}, sync={sync:?})"
                )),
            };
        }

        Ok(())
    }

    #[cfg(unix)]
    fn verify_parent_identity(&self) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt as _;

        let parent_path = self
            .path
            .parent()
            .expect("prepared qualification seal path has a parent");
        let opened = self.parent.metadata().map_err(|error| {
            format!("failed to inspect pinned qualification seal destination: {error}")
        })?;
        let current = fs::symlink_metadata(parent_path).map_err(|error| {
            format!("failed to re-inspect qualification seal destination path: {error}")
        })?;
        if current.file_type().is_symlink()
            || !current.is_dir()
            || opened.dev() != current.dev()
            || opened.ino() != current.ino()
            || opened.uid() != self.expected_uid
            || opened.mode() & 0o022 != 0
        {
            return Err(
                "qualification seal destination directory changed identity or trust attributes"
                    .to_owned(),
            );
        }
        #[cfg(target_os = "macos")]
        require_acl_free_pinned_path(
            &self.parent,
            parent_path,
            "qualification seal destination directory",
        )?;
        Ok(())
    }
}

fn validate_canonical_absolute_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(format!(
            "{label} must not contain `.` or `..` components: {}",
            path.display()
        ));
    }
    Ok(())
}

fn validate_qualification_seal_directory_separation(
    config: &Config,
    seal_path: &Path,
) -> Result<(), String> {
    let seal_parent = seal_path
        .parent()
        .ok_or_else(|| "qualification seal path must have a parent directory".to_owned())?;
    let policy_parent = config
        .settlement
        .offline
        .kagemusha_release_policy_path
        .as_deref()
        .and_then(Path::parent);
    let artifact_dir = config.settlement.offline.kagemusha_artifact_dir.as_deref();
    let executable_parent = env::current_exe()
        .map_err(|error| format!("failed to resolve current executable path: {error}"))?
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| "current executable has no parent directory".to_owned())?;
    for (label, source_dir) in [
        ("release-policy parent", policy_parent),
        ("artifact directory", artifact_dir),
        (
            "current-executable parent",
            Some(executable_parent.as_path()),
        ),
    ] {
        let Some(source_dir) = source_dir else {
            continue;
        };
        if seal_parent.starts_with(source_dir) || source_dir.starts_with(seal_parent) {
            return Err(format!(
                "qualification seal directory `{}` must be separate from the Kagemusha {label} `{}` because publication changes directory identity",
                seal_parent.display(),
                source_dir.display()
            ));
        }
    }
    Ok(())
}

fn validate_config_for_check(
    config: &Config,
    genesis: Option<&GenesisBlock>,
    build_kagemusha_qualification_seal: bool,
) -> ReportResult<
    Option<iroha_core::smartcontracts::isi::offline::KagemushaCatalogQualificationSealV1>,
    MainError,
> {
    validate_config_offline(config).change_context(MainError::Config)?;
    IrohaRuntimeProviderBindingsV1::try_from_config(config)
        .map_err(Report::new)
        .change_context(MainError::Config)
        .attach("failed to validate the public runtime-provider binding catalog")?;

    if build_kagemusha_qualification_seal && genesis.is_none() {
        return Err(Report::new(MainError::Config).attach(
            "`--write-kagemusha-catalog-qualification-seal` requires locally available genesis so the seal is published only after full Kagemusha release and genesis validation",
        ));
    }

    let (catalog, qualification_seal) = if build_kagemusha_qualification_seal {
        let (catalog, seal) = load_and_build_configured_kagemusha_catalog_qualification_seal(
            config,
        )
        .map_err(|error| {
            Report::new(MainError::Config).attach(format!(
                "failed to fully qualify the Kagemusha V4 release catalog: {error}"
            ))
        })?;
        (catalog, Some(seal))
    } else {
        let catalog = load_configured_kagemusha_release_catalog(config).map_err(|error| {
            Report::new(MainError::Config).attach(format!(
                "failed to load the configured Kagemusha V4 release catalog: {error}"
            ))
        })?;
        (catalog, None)
    };

    let Some(genesis) = genesis else {
        // Runtime startup can use an exact configured genesis hash with the
        // canonical body already persisted in Kura, or an authenticated
        // provisional snapshot. Static validation and catalog authentication
        // are complete even though body-level validation is pending.
        return Ok(None);
    };

    let configured_key = &config.genesis.public_key;
    let embedded_key =
        genesis_public_key_from_genesis_block(genesis).change_context(MainError::Config)?;
    if &embedded_key != configured_key {
        return Err(Report::new(MainError::Config).attach(format!(
            "genesis authority `{embedded_key}` does not match configured genesis.public_key `{configured_key}`"
        )));
    }

    if genesis.0.hash() != config.genesis.expected_hash {
        return Err(Report::new(MainError::Config).attach(format!(
            "local genesis hash {} does not match configured genesis.expected_hash {}",
            genesis.0.hash(),
            config.genesis.expected_hash,
        )));
    }

    let genesis_account = AccountId::new(embedded_key);
    iroha_core::validate_genesis_block(&genesis.0, &genesis_account)
        .map_err(Report::new)
        .change_context(MainError::Config)?;

    let (signed_mode, signed_parameters) = signed_v2_genesis_context_metadata(genesis)
        .map_err(|error| Report::new(MainError::Config).attach(error))?;
    let config_caps =
        build_consensus_config_caps(&config.nexus, None, None).change_context(MainError::Config)?;
    let (mode_tag, _bls_domain, consensus_caps, block_cadence_ms) =
        consensus_caps_from_genesis(genesis, &config_caps, &config.sumeragi).ok_or_else(|| {
            Report::new(MainError::Config).attach(
                "local genesis does not contain one valid canonical Sumeragi v2 handshake context",
            )
        })?;
    verify_genesis_metadata(
        genesis,
        config,
        &consensus_caps,
        &mode_tag,
        iroha_core::sumeragi::consensus::PROTO_VERSION,
    )?;
    validate_genesis_execution_offline(
        config,
        genesis,
        &genesis_account,
        signed_mode,
        signed_parameters,
        block_cadence_ms,
        catalog,
    )?;

    Ok(qualification_seal)
}

fn load_configured_kagemusha_release_catalog(
    config: &Config,
) -> Result<iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4, String> {
    let offline = &config.settlement.offline;
    iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4::from_offline_config(
        offline,
    )
    .map_err(|error| format!("failed to authenticate Kagemusha V4 release catalog: {error}"))
}

fn load_and_build_configured_kagemusha_catalog_qualification_seal(
    config: &Config,
) -> Result<
    (
        iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4,
        iroha_core::smartcontracts::isi::offline::KagemushaCatalogQualificationSealV1,
    ),
    String,
> {
    match (
        config
            .settlement
            .offline
            .kagemusha_release_policy_path
            .as_deref(),
        config
            .settlement
            .offline
            .kagemusha_artifact_dir
            .as_deref(),
    ) {
        (Some(policy_path), Some(artifact_dir)) => {
            iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4::load_and_build_qualification_seal(
                policy_path,
                artifact_dir,
                config.settlement.offline.kagemusha_max_decoded_bytes,
            )
            .map_err(|error| {
                format!("failed to authenticate the complete Kagemusha V4 release catalog: {error}")
            })
        }
        (None, None) => Err(
            "cannot qualify a Kagemusha V4 catalog without a release policy and artifact directory"
                .to_owned(),
        ),
        _ => Err(
            "Kagemusha V4 release policy and artifact directory must be configured together"
                .to_owned(),
        ),
    }
}

fn install_configured_kagemusha_release_catalog(
    state: &mut State,
    config: &Config,
) -> Result<(), String> {
    state.set_kagemusha_release_catalog(load_configured_kagemusha_release_catalog(config)?);
    Ok(())
}

struct DisposableValidationRoot {
    path: PathBuf,
}

impl DisposableValidationRoot {
    fn create() -> std::io::Result<Self> {
        let parent = env::temp_dir();
        for _ in 0..32 {
            let mut nonce = [0_u8; 16];
            rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut nonce)
                .map_err(|error| {
                    std::io::Error::other(format!(
                        "operating-system randomness unavailable for temporary validation storage: {error}"
                    ))
                })?;
            let path = parent.join(format!(
                "irohad-check-config-{}-{}",
                std::process::id(),
                hex::encode(nonce)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "failed to allocate a unique temporary validation directory after 32 attempts",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DisposableValidationRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            iroha_logger::warn!(
                path = %self.path.display(),
                ?error,
                "failed to remove disposable check-config storage"
            );
        }
    }
}

fn open_disposable_validation_kura(
    config: &Config,
    validation_root: &DisposableValidationRoot,
) -> ReportResult<Arc<Kura>, MainError> {
    let mut kura_config = config.kura.clone();
    kura_config.store_dir = WithOrigin::inline(validation_root.path().join("kura"));
    let (kura, block_count) =
        Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
            &kura_config,
            &config.nexus.lane_config,
            &config.nexus.configured_lane_catalog,
            &iroha_config::parameters::actual::SnapshotBootstrapPolicy::default(),
            &config.sumeragi.limits,
        )
        .map_err(|error| {
            Report::new(MainError::Config).attach(format!(
                "failed to initialize disposable Kura for genesis validation: {error}"
            ))
        })?;
    if block_count.0 != 0 {
        return Err(Report::new(MainError::Config).attach(format!(
            "disposable genesis validation storage was not empty ({} blocks)",
            block_count.0
        )));
    }
    Ok(kura)
}

/// Execute a locally available genesis against a disposable state overlay.
///
/// This mirrors the fresh-node staging boundary closely enough to catch instruction-order,
/// catalog, permission, and other world-state failures while keeping the configured Kura and
/// every listening socket untouched.
fn validate_genesis_execution_offline(
    config: &Config,
    genesis: &GenesisBlock,
    genesis_authority: &AccountId,
    signed_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
    signed_parameters: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
    expected_block_cadence_ms: u64,
    kagemusha_release_catalog: iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4,
) -> ReportResult<(), MainError> {
    let validation_root = DisposableValidationRoot::create().map_err(|error| {
        Report::new(MainError::Config).attach(format!(
            "failed to create disposable storage for genesis validation: {error}"
        ))
    })?;
    let kura = open_disposable_validation_kura(config, &validation_root)?;

    let mut world = World::with(
        [genesis_domain(config.genesis.public_key.clone())],
        [genesis_account(config.genesis.public_key.clone())],
        [],
    );
    iroha_core::sns::seed_genesis_alias_bootstrap(
        &mut world,
        &genesis.0,
        &config.nexus.dataspace_catalog,
    );
    let mut state = State::try_new_with_chain_and_network_id(
        world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        config.common.chain.clone(),
        NetworkId::from_genesis_hash(config.genesis.expected_hash),
        #[cfg(feature = "telemetry")]
        StateTelemetry::default(),
    )
    .map_err(|error| {
        Report::new(MainError::Config).attach(format!(
            "failed to initialize disposable world state for genesis validation: {error}"
        ))
    })?;
    state.set_kagemusha_release_catalog(kagemusha_release_catalog);
    install_zk_config_before_kura_replay(&mut state, config).change_context(MainError::Config)?;
    apply_state_runtime_config_before_snapshot_auth(&mut state, config);
    apply_state_geometry_config_before_kura_replay(&mut state, config)
        .change_context(MainError::Config)?;
    let replay_nexus = nexus_for_runtime_surfaces(&state);
    let frozen_lane_manifests =
        freeze_lane_manifests_for_startup_replay(&replay_nexus).map_err(|error| {
            Report::new(MainError::Config).attach(format!(
                "lane manifest registry is not ready for Kagemusha release and genesis validation: {error}"
            ))
        })?;
    state.install_lane_manifests(&frozen_lane_manifests);

    let signed_voters =
        iroha_core::sumeragi::signed_genesis_voting_peers(genesis).map_err(|error| {
            Report::new(MainError::Config).attach(format!(
                "invalid signed Sumeragi v2 genesis roster: {error}"
            ))
        })?;
    let topology = Topology::new(signed_voters);
    let mut voting_block: Option<VotingBlock> = None;
    let (_valid, staged) = ValidBlock::validate_signed_genesis_keep_voting_block(
        genesis.0.clone(),
        &topology,
        genesis_authority,
        &TimeSource::new_system(),
        &state,
        &mut voting_block,
        signed_mode,
    )
    .unpack(|_| {})
    .map_err(|(_failed_block, error)| {
        Report::new(MainError::Config)
            .attach(format!("genesis instruction execution failed: {error}"))
    })?;

    let staged_block_cadence_ms = staged
        .world()
        .parameters()
        .sumeragi()
        .block_cadence_ms()
        .get();
    if staged_block_cadence_ms != expected_block_cadence_ms {
        return Err(Report::new(MainError::Config).attach(format!(
            "staged genesis cadence {staged_block_cadence_ms} ms differs from authenticated signed cadence {expected_block_cadence_ms} ms"
        )));
    }
    iroha_core::sumeragi::freeze_staged_genesis_v2(
        genesis,
        &staged,
        signed_mode,
        signed_parameters,
    )
    .map_err(|error| {
        Report::new(MainError::Config).attach(format!(
            "failed to freeze staged Sumeragi v2 genesis: {error}"
        ))
    })?;

    Ok(())
}

fn enforce_build_line(build_line: BuildLine, config: &mut Config) -> ReportResult<(), MainError> {
    if build_line.is_iroha3() {
        return Ok(());
    }

    let mut disarmed = Vec::new();
    if config.streaming.soranet.enabled {
        config.streaming.soranet.enabled = false;
        disarmed.push("streaming.soranet.enabled");
    }
    if config.torii.sorafs_storage.enabled {
        config.torii.sorafs_storage.enabled = false;
        disarmed.push("torii.sorafs_storage.enabled");
    }
    if config.torii.sorafs_discovery.discovery_enabled {
        config.torii.sorafs_discovery.discovery_enabled = false;
        disarmed.push("torii.sorafs_discovery.discovery_enabled");
    }

    let sora_features = config.uses_sora_features();
    let mut fatal = Vec::new();
    if sora_features {
        fatal.push("Nexus/multi-dataspace/SoraFS runtime");
    }

    if config.nexus.enabled && !sora_features {
        config.nexus.enabled = false;
        disarmed.push("nexus.enabled");
    }

    if !fatal.is_empty() {
        return Err(Report::new(MainError::Config).attach(format!(
            "Iroha 2 build forbids Nexus/Sora features; disable the following: {}",
            fatal.join(", ")
        )));
    }

    if !disarmed.is_empty() {
        eprintln!(
            "Iroha 2 build disabled Sora-only features at startup: {}",
            disarmed.join(", ")
        );
    }

    Ok(())
}

fn parse_confidential_registry_hash(payload: &Json) -> ReportResult<Option<[u8; 32]>, MainError> {
    let meta = decode_confidential_registry_meta(payload).map_err(|err| {
        Report::new(MainError::Config).attach(format!(
            "failed to decode confidential_registry_root payload: {err}"
        ))
    })?;
    if let Some(hash_str) = meta.vk_set_hash {
        let trimmed = hash_str.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
            return Ok(None);
        }
        let body = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if body.len() != 64 || !body.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(Report::new(MainError::Config).attach(format!(
                "confidential_registry_root.vk_set_hash must be 32-byte hex, got `{hash_str}`"
            )));
        }
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(body, &mut bytes).map_err(|err| {
            Report::new(MainError::Config).attach(format!(
                "failed to decode confidential_registry_root.vk_set_hash `{hash_str}`: {err}"
            ))
        })?;
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

fn build_consensus_config_caps(
    nexus: &iroha_config::parameters::actual::Nexus,
    compliance_policy_digest: Option<[u8; 32]>,
    lane_manifest_policy_digest: Option<[u8; 32]>,
) -> ReportResult<iroha_p2p::ConsensusConfigCaps, StartError> {
    let nexus_policy_digest =
        iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
            nexus,
            compliance_policy_digest,
            lane_manifest_policy_digest,
        )
        .map_err(|err| {
            Report::new(StartError::StartP2p).attach(format!(
                "failed to construct Nexus consensus-policy digest: {err}"
            ))
        })?;

    Ok(iroha_p2p::ConsensusConfigCaps {
        // The signed genesis/world projection replaces this bootstrap value
        // before the handshake is exposed to peers.
        v2_config_fingerprint: [0; 32],
        execution_policy_hash: [0; 32],
        nexus_policy_digest,
        ivm_gas_schedule_hash: ivm::gas::schedule_hash().into(),
    })
}

fn consensus_caps_from_genesis(
    genesis: &GenesisBlock,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    sumeragi: &iroha_config::parameters::actual::Sumeragi,
) -> Option<(String, String, iroha_p2p::ConsensusHandshakeCaps, u64)> {
    let mut params = iroha_data_model::parameter::Parameters::default();
    let mut handshake_entries = Vec::new();

    for tx in genesis.0.external_transactions() {
        if let Executable::Instructions(batch) = tx.instructions() {
            for instr in batch {
                if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>() {
                    if let iroha_data_model::parameter::Parameter::Custom(custom) =
                        set_param.inner()
                        && custom.id() == &consensus_metadata::handshake_meta_id()
                        && let Ok(meta) = decode_consensus_handshake_meta(custom.payload())
                    {
                        handshake_entries.push(meta);
                    }
                    params.set_parameter(set_param.inner().clone());
                }
            }
        }
    }

    let [entry] = handshake_entries.as_slice() else {
        return None;
    };
    if entry.wire_protocol_version
        != u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)
    {
        return None;
    }
    let (expected_mode, expected_domain) = match entry.mode {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => (
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            iroha_data_model::block::consensus_v2::PERMISSIONED_BLS_DOMAIN,
        ),
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => (
            iroha_data_model::block::consensus_v2::ConsensusMode::Npos,
            iroha_data_model::block::consensus_v2::NPOS_BLS_DOMAIN,
        ),
    };
    params.sumeragi.block_cadence_ms = entry.block_cadence_ms;

    let (mode_tag, consensus_params, computed_fingerprint) =
        consensus_entry_caps(entry, &params).ok()?;
    if entry.consensus_fingerprint.into_bytes() != computed_fingerprint {
        return None;
    }
    let mut config_caps = *config_caps;
    config_caps.execution_policy_hash = entry.sumeragi_v2.execution_policy_hash;
    config_caps.v2_config_fingerprint = sumeragi
        .v2_config(
            Duration::from_millis(consensus_params.block_cadence_ms.get()),
            expected_mode,
        )
        .ok()?
        .fingerprint()
        .into();

    Some((
        mode_tag.clone(),
        expected_domain.to_owned(),
        iroha_p2p::ConsensusHandshakeCaps {
            mode: expected_mode,
            proto_version: iroha_core::sumeragi::consensus::PROTO_VERSION,
            consensus_fingerprint: computed_fingerprint,
            config: config_caps,
        },
        consensus_params.block_cadence_ms.get(),
    ))
}

fn signed_v2_genesis_context_metadata(
    genesis: &GenesisBlock,
) -> core::result::Result<
    (
        iroha_data_model::block::consensus_v2::ConsensusMode,
        iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
    ),
    String,
> {
    let mut metadata_entries = Vec::new();
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(
                "Sumeragi v2 genesis metadata must be carried by instruction batches".to_owned(),
            );
        };
        for set_parameter in instructions
            .iter()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        {
            let Parameter::Custom(custom) = set_parameter.inner() else {
                continue;
            };
            if custom.id() == &consensus_metadata::handshake_meta_id() {
                metadata_entries.push(
                    decode_consensus_handshake_meta(custom.payload())
                        .map_err(|error| error.to_string())?,
                );
            }
        }
    }
    let [metadata] = metadata_entries.as_slice() else {
        return Err(format!(
            "Sumeragi v2 genesis requires exactly one signed handshake metadata entry, found {}",
            metadata_entries.len()
        ));
    };
    let expected_protocol = u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION);
    if metadata.wire_protocol_version != expected_protocol {
        return Err(format!(
            "Sumeragi v2 genesis requires wire_protocol_version = {expected_protocol}, got {}",
            metadata.wire_protocol_version
        ));
    }
    metadata.validate()?;
    let mode = match metadata.mode {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => {
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned
        }
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => {
            iroha_data_model::block::consensus_v2::ConsensusMode::Npos
        }
    };
    Ok((mode, metadata.sumeragi_v2))
}

fn consensus_entry_caps(
    entry: &ConsensusHandshakeMeta,
    params: &iroha_data_model::parameter::Parameters,
) -> EyreResult<(
    String,
    iroha_data_model::block::consensus::ConsensusGenesisParams,
    [u8; 32],
)> {
    let (mode, mode_tag) = match entry.mode {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => (
            iroha_data_model::block::consensus_v2::ConsensusMode::Npos,
            iroha_core::sumeragi::consensus::NPOS_TAG,
        ),
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => (
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            iroha_core::sumeragi::consensus::PERMISSIONED_TAG,
        ),
    };
    let mut params = params.clone();
    params.sumeragi.block_cadence_ms = entry.block_cadence_ms;
    let consensus_params =
        iroha_core::sumeragi::consensus::consensus_genesis_params_from_parameters(
            mode,
            &params,
            entry.sumeragi_v2,
        )
        .map_err(|error| eyre::eyre!(error))?;

    let fingerprint = iroha_core::sumeragi::consensus::compute_consensus_parameters_fingerprint(
        &consensus_params,
    )
    .map_err(|error| eyre::eyre!(error))?;

    Ok((mode_tag.to_string(), consensus_params, fingerprint))
}

fn compute_consensus_handshake_caps(
    world: &impl iroha_core::state::WorldReadOnly,
    height: u64,
    config: &Config,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    frozen_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
    signed_v2_context: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
) -> ReportResult<(String, String, iroha_p2p::ConsensusHandshakeCaps), StartError> {
    iroha_core::sumeragi::consensus::compute_consensus_handshake_caps_from_world(
        world,
        height,
        &config.sumeragi,
        config_caps,
        frozen_mode,
        signed_v2_context,
    )
    .map_err(|error| {
        Report::new(StartError::StartP2p)
            .attach(format!("invalid shared Sumeragi v2 configuration: {error}"))
    })
}

#[allow(clippy::too_many_lines)]
fn verify_genesis_metadata(
    genesis: &GenesisBlock,
    config: &Config,
    consensus_caps: &iroha_p2p::ConsensusHandshakeCaps,
    mode_tag: &str,
    proto_version: u32,
) -> ReportResult<(), MainError> {
    let mut instructions: Vec<InstructionBox> = Vec::new();
    for tx in genesis.0.external_transactions() {
        match tx.instructions() {
            Executable::Instructions(batch) => {
                instructions.extend(batch.iter().cloned());
            }
            Executable::ContractCall(_) => {
                return Err(Report::new(MainError::Config).attach(
                    "genesis transaction payload contains contract calls; expected instruction batches",
                ));
            }
            Executable::Ivm(_) => {
                return Err(Report::new(MainError::Config).attach(
                    "genesis transaction payload contains raw IVM bytecode; expected instruction batches",
                ));
            }
            Executable::IvmProved(_) => {
                return Err(Report::new(MainError::Config).attach(
                    "genesis transaction payload contains proved IVM bytecode; expected instruction batches",
                ));
            }
            Executable::Batch(_) => {
                return Err(Report::new(MainError::Config).attach(
                    "genesis transaction payload contains a mixed executable batch; expected instruction batches",
                ));
            }
        }
    }

    let mut handshake_entries = Vec::new();
    for set_param in instructions
        .iter()
        .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
    {
        if let Parameter::Custom(custom) = set_param.inner()
            && custom.id() == &consensus_metadata::handshake_meta_id()
        {
            let meta: ConsensusHandshakeMeta = decode_consensus_handshake_meta(custom.payload())
                .map_err(|err| {
                    Report::new(MainError::Config).attach(format!(
                        "failed to decode consensus_handshake_meta payload: {err}"
                    ))
                })?;
            handshake_entries.push(meta);
        }
    }
    if handshake_entries.is_empty() {
        return Err(Report::new(MainError::Config).attach(
            "genesis block missing consensus_handshake_meta parameter; regenerate genesis with consensus metadata populated",
        ));
    }

    let expected_mode = if mode_tag == iroha_core::sumeragi::consensus::PERMISSIONED_TAG {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned
    } else if mode_tag == iroha_core::sumeragi::consensus::NPOS_TAG {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos
    } else {
        return Err(Report::new(MainError::Config)
            .attach(format!("unknown consensus mode tag `{mode_tag}`")));
    };

    let expected_fp_hex = hex::encode(consensus_caps.consensus_fingerprint);
    let mut matched_meta: Option<ConsensusHandshakeMeta> = None;
    for meta in &handshake_entries {
        if meta.mode != expected_mode {
            continue;
        }
        if meta.wire_protocol_version != proto_version {
            continue;
        }
        if meta.consensus_fingerprint.into_bytes() == consensus_caps.consensus_fingerprint {
            matched_meta = Some(*meta);
            break;
        }
    }
    let Some(matched_meta) = matched_meta else {
        let entries_summary = handshake_entries
            .iter()
            .map(|meta| {
                format!(
                    "{{mode={:?}, block_cadence_ms={}, wire_protocol_version={}, fingerprint=0x{}}}",
                    meta.mode,
                    meta.block_cadence_ms,
                    meta.wire_protocol_version,
                    hex::encode(meta.consensus_fingerprint.into_bytes())
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Report::new(MainError::Config).attach(format!(
            "none of the consensus_handshake_meta entries match the authenticated startup context (expected consensus_mode `{expected_mode:?}`, proto v{proto_version}, fingerprint 0x{expected_fp_hex}`); entries observed: {entries_summary}"
        )));
    };

    let mut params = iroha_data_model::parameter::Parameters::default();
    for set_param in instructions
        .iter()
        .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
    {
        params.set_parameter(set_param.inner().clone());
    }
    params.sumeragi.block_cadence_ms = matched_meta.block_cadence_ms;

    let crypto_manifest_payload = instructions
        .iter()
        .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
        .find_map(|set| {
            if let Parameter::Custom(custom) = set.inner()
                && custom.id() == &crypto_metadata::manifest_meta_id()
            {
                Some(custom.payload())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            Report::new(MainError::Config).attach(
                "genesis block missing crypto_manifest_meta parameter; regenerate genesis with crypto metadata populated",
            )
        })?;
    let manifest_crypto: ManifestCrypto = decode_crypto_manifest_meta(crypto_manifest_payload)
        .map_err(|err| {
            Report::new(MainError::Config).attach(format!(
                "failed to decode crypto_manifest_meta payload: {err}"
            ))
        })?;
    ensure_crypto_snapshot_matches_config(&manifest_crypto, config)
        .map_err(|err| Report::new(MainError::Config).attach(err))?;

    let mode = if mode_tag == iroha_core::sumeragi::consensus::NPOS_TAG {
        iroha_data_model::block::consensus_v2::ConsensusMode::Npos
    } else {
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned
    };
    let consensus_params =
        iroha_core::sumeragi::consensus::consensus_genesis_params_from_parameters(
            mode,
            &params,
            matched_meta.sumeragi_v2,
        )
        .map_err(|error| Report::new(MainError::Config).attach(error))?;
    let computed_fp = iroha_core::sumeragi::consensus::compute_consensus_parameters_fingerprint(
        &consensus_params,
    )
    .map_err(|error| Report::new(MainError::Config).attach(error))?;
    if computed_fp != matched_meta.consensus_fingerprint.into_bytes() {
        return Err(Report::new(MainError::Config).attach(format!(
            "consensus_handshake_meta fingerprint 0x{} does not match parameters encoded in genesis (computed 0x{})",
            hex::encode(matched_meta.consensus_fingerprint.into_bytes()),
            hex::encode(computed_fp)
        )));
    }

    let expected_vk_hash = compute_genesis_vk_set_hash(instructions.iter()).map_err(|err| {
        Report::new(MainError::Config).attach(format!(
            "failed to evaluate confidential registry instructions in genesis: {err}"
        ))
    })?;
    let registry_payload = instructions
        .iter()
        .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
        .find_map(|set| {
            if let Parameter::Custom(custom) = set.inner()
                && custom.id() == &confidential_metadata::registry_root_id()
            {
                Some(custom.payload())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            Report::new(MainError::Config).attach(
                "genesis block missing confidential_registry_root parameter; regenerate genesis with confidential metadata populated",
            )
        })?;
    let declared_vk_hash = parse_confidential_registry_hash(registry_payload)?;
    if declared_vk_hash != expected_vk_hash {
        let declared = declared_vk_hash.map_or_else(
            || "null".to_string(),
            |hash| format!("0x{}", hex::encode(hash)),
        );
        let expected = expected_vk_hash.map_or_else(
            || "null".to_string(),
            |hash| format!("0x{}", hex::encode(hash)),
        );
        return Err(Report::new(MainError::Config).attach(format!(
            "genesis confidential registry root mismatch: manifest {declared} vs expected {expected}"
        )));
    }

    let mut genesis_peers: BTreeMap<PeerId, RegisterPeerWithPop> = BTreeMap::new();
    for register in instructions
        .iter()
        .filter_map(|instr| instr.as_any().downcast_ref::<RegisterPeerWithPop>())
    {
        if genesis_peers
            .insert(register.peer.clone(), register.clone())
            .is_some()
        {
            return Err(Report::new(MainError::Config).attach(format!(
                "genesis registers peer {} multiple times",
                register.peer
            )));
        }
    }

    let trusted = config.common.trusted_peers.value();
    let expected_validators = filter_validators_from_trusted(trusted);
    if expected_validators.is_empty() {
        if !genesis_peers.is_empty() {
            return Err(Report::new(MainError::Config).attach(format!(
                "genesis encodes {} validator(s) with PoP but configuration filters them all out",
                genesis_peers.len()
            )));
        }
        return Ok(());
    }

    for peer_id in expected_validators {
        let entry = genesis_peers
            .remove(&peer_id)
            .or_else(|| {
                trusted
                    .pops
                    .get(peer_id.public_key())
                    .map(|pop| RegisterPeerWithPop::new(peer_id.clone(), pop.clone()))
            })
            .ok_or_else(|| {
                Report::new(MainError::Config).attach(format!(
                    "genesis lacks RegisterPeerWithPop for validator {peer_id}"
                ))
            })?;

        let bls_pk = peer_id.public_key();
        match bls_pk.try_algorithm() {
            Ok(Algorithm::BlsNormal) => {}
            Ok(_) => {
                return Err(Report::new(MainError::Config)
                    .attach(format!("trusted peer {peer_id} must use a BLS-normal key")));
            }
            Err(err) => {
                return Err(Report::new(MainError::Config).attach(format!(
                    "trusted peer {peer_id} has malformed public key: {err}"
                )));
            }
        }
        if let Some(expected_pop) = trusted.pops.get(bls_pk) {
            if &entry.pop != expected_pop {
                return Err(Report::new(MainError::Config).attach(format!(
                    "genesis PoP for peer {peer_id} does not match configuration"
                )));
            }
        } else if !trusted.pops.is_empty() {
            return Err(Report::new(MainError::Config).attach(format!(
                "trusted peer {peer_id} missing PoP in configuration"
            )));
        }
        if let Err(err) = iroha_crypto::bls_normal_pop_verify(bls_pk, &entry.pop) {
            return Err(Report::new(MainError::Config).attach(format!(
                "genesis PoP for peer {peer_id} failed verification: {err}"
            )));
        }
    }

    if !genesis_peers.is_empty() {
        let extras = genesis_peers
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Report::new(MainError::Config).attach(format!(
            "genesis encodes unexpected validators with PoP: {extras}"
        )));
    }

    Ok(())
}

async fn run_node(
    config: Config,
    genesis: Option<GenesisBlock>,
    runtime_deps: IrohaRuntimeDeps,
    musubi_publication_factory: Option<
        Box<dyn musubi_publication_service::MusubiPublicationPrivateServiceFactoryV1>,
    >,
) -> ReportResult<(), MainError> {
    let logger = iroha_logger::init_global(config.logger.clone()).map_err(|err| {
        // https://github.com/hashintel/hash/issues/4295
        Report::new(MainError::Logger).attach(err)
    })?;
    validate_config(&config).change_context(MainError::Config)?;

    set_banner_enabled(config.ivm.banner.show);

    // Print a retro Norito banner with applied settings when enabled.
    if config.ivm.banner.show {
        log_norito_banner(&config);
    }

    iroha_logger::info!(
        version = env!("CARGO_PKG_VERSION"),
        git_commit_sha = VERGEN_GIT_SHA,
        build_features = VERGEN_CARGO_FEATURES,
        peer = %config.common.peer,
        chain = %config.common.chain,
        listening_on = %config.torii.address.value(),
        "{}",
        i18n::t("info.welcome"),
    );

    if genesis.is_some() {
        iroha_logger::debug!("Submitting genesis.");
    }

    #[cfg(feature = "beep")]
    startup_beep(config.ivm.banner.beep);

    let shutdown_on_panic = ShutdownSignal::new();
    let default_hook = std::panic::take_hook();
    let signal_clone = shutdown_on_panic.clone();
    std::panic::set_hook(Box::new(move |info| {
        let suppressed_by_panic_hook = panic_hook::is_suppressed();
        let suppressed_by_norito_decode = norito::decode_panic_suppressed();
        if suppressed_by_panic_hook || suppressed_by_norito_decode {
            let panic_file = info.location().map(std::panic::Location::file);
            let panic_line = info.location().map(std::panic::Location::line);
            iroha_logger::warn!(
                suppressed_by_panic_hook,
                suppressed_by_norito_decode,
                ?panic_file,
                ?panic_line,
                "Panic occurred with shutdown suppression active; skipping shutdown signal"
            );
        } else {
            iroha_logger::error!("Panic occurred, shutting down Iroha gracefully...");
            signal_clone.send();
        }
        default_hook(info);
    }));

    let start = Iroha::start_with_runtime_deps(
        config,
        genesis,
        logger,
        shutdown_on_panic,
        runtime_deps,
        musubi_publication_factory,
    );
    let (_iroha, supervisor_fut) = Box::pin(start)
        .await
        .change_context(MainError::IrohaStart)?;
    supervisor_fut.await.change_context(MainError::IrohaRun)
}

/// Print a startup banner with applied Norito codec settings in a retro style.
fn log_norito_banner(cfg: &Config) {
    // Snapshot core settings
    let n = &cfg.norito;
    let gpu_allowed = n.allow_gpu_compression;
    let gpu_probe_status = if gpu_allowed { "deferred" } else { "disabled" };

    // UTF‑8 box drawing and kana render nicely in modern terminals.
    let art = r"
╔══════════════════════════════════════════════════════════════════════╗
║  ⛩  ノ  リ  ト   N O R I T O   ⛩     「速く、正しく、そして同じ結果」║
╠══════════════════════════════════════════════════════════════════════╣
║              ┌────────────── イロハ ──────────────┐                  ║
║              │      ────┬──────────────┬────      │                  ║
║              │          │  ノ  リ  ト  │          │                  ║
║              │      ────┴──────────────┴────      │                  ║
║              └────────────────────────────────────┘                  ║
╚══════════════════════════════════════════════════════════════════════╝
";

    // Compose settings block
    let msg = format!(
        "\n{}\nNorito settings:\n  - max_archive_len: {}\n  - gpu_offload_allowed: {}\n  - gpu_backend_probe: {}\n",
        art,
        resolve_norito_max_archive_len(cfg),
        gpu_allowed,
        gpu_probe_status,
    );

    iroha_logger::info!(target: "norito", "{}", msg);
}

#[cfg(test)]
mod tests {
    use super::build_line_tests::{
        load_unprovisioned_profile_for_inspection, minimal_config_table, multilane_config_table,
    };
    #[allow(unused_imports)]
    use super::*;
    use iroha_config_base::toml::TomlSource;

    const GOVERNANCE_DAG_PUBLISHER_HANDLE: &str = "pkcs11:governance-dag-publisher";
    const GOVERNANCE_DAG_PUBLISHER_PEER_ID: &str = "governance-dag-publisher";
    const GOVERNANCE_DAG_PUBLISHER_POLICY_DIGEST: [u8; 32] = [0xA5; 32];
    const GOVERNANCE_DAG_CHECKPOINT_STORE_HANDLE: &str =
        "sealed:governance-dag:producer-checkpoint";
    const GOVERNANCE_DAG_CHECKPOINT_STORE_POLICY_DIGEST: [u8; 32] = [0xA6; 32];

    #[derive(Debug)]
    struct GovernanceDagPublisherBindingSigner {
        key_pair: iroha_crypto::KeyPair,
    }

    impl GovernanceDagPublisherBindingSigner {
        fn from_seed(seed: u8) -> Self {
            Self {
                key_pair: iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("derive Governance DAG publisher key"),
            }
        }

        fn public_key_bytes(&self) -> [u8; 32] {
            self.key_pair
                .public_key()
                .to_bytes()
                .1
                .try_into()
                .expect("Ed25519 public key has 32 bytes")
        }
    }

    include!("main_tests/governance_dag_publisher_binding_signer.rs");

    #[derive(Debug)]
    struct GovernanceDagCheckpointBindingStore;

    impl sorafs_node::GovernanceDagSealedCheckpointStore for GovernanceDagCheckpointBindingStore {
        fn handle(&self) -> &str {
            GOVERNANCE_DAG_CHECKPOINT_STORE_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    GOVERNANCE_DAG_CHECKPOINT_STORE_POLICY_DIGEST,
                ),
            )
        }

        fn load(
            &self,
            _slot: sorafs_node::GovernanceDagSealedStateSlot,
        ) -> Result<Option<sorafs_node::GovernanceDagSealedStateRecord>, String> {
            Ok(None)
        }

        fn compare_and_swap(
            &self,
            _slot: sorafs_node::GovernanceDagSealedStateSlot,
            _expected_revision: Option<[u8; 32]>,
            _next: sorafs_node::GovernanceDagSealedStateRecord,
        ) -> Result<(), String> {
            Ok(())
        }

        fn delete(
            &self,
            _slot: sorafs_node::GovernanceDagSealedStateSlot,
            _expected_revision: [u8; 32],
        ) -> Result<(), String> {
            Ok(())
        }
    }

    fn governance_dag_publisher_public_key(seed: u8) -> [u8; 32] {
        GovernanceDagPublisherBindingSigner::from_seed(seed).public_key_bytes()
    }

    fn governance_dag_service_storage(
        public_key: [u8; 32],
    ) -> iroha_config::parameters::actual::SorafsStorage {
        let public_key_hex = hex::encode(public_key);
        let mut storage = iroha_config::parameters::actual::SorafsStorage::default();
        storage.enabled = true;
        storage.governance_dag_dir =
            Some(std::path::PathBuf::from("/var/lib/iroha/sorafs/governance"));
        storage.governance_dag_publisher_peer_id =
            Some(GOVERNANCE_DAG_PUBLISHER_PEER_ID.to_owned());
        storage.governance_dag_signer_handle = Some(GOVERNANCE_DAG_PUBLISHER_HANDLE.to_owned());
        storage.governance_dag_signer_revision = Some(1);
        storage.governance_dag_signer_policy_digest = Some(GOVERNANCE_DAG_PUBLISHER_POLICY_DIGEST);
        storage.governance_dag_service.enabled = true;
        storage.governance_dag_service.checkpoint_store_handle =
            Some(GOVERNANCE_DAG_CHECKPOINT_STORE_HANDLE.to_owned());
        storage.governance_dag_service.checkpoint_store_revision = Some(1);
        storage
            .governance_dag_service
            .checkpoint_store_policy_digest = Some(GOVERNANCE_DAG_CHECKPOINT_STORE_POLICY_DIGEST);
        storage.governance_dag_publisher_public_key_hex = Some(public_key_hex.clone());
        storage.governance_dag_service.publisher_public_key_hex = Some(public_key_hex);
        storage
    }

    #[cfg(target_os = "macos")]
    struct MacosAclGuard {
        path: PathBuf,
    }

    #[cfg(target_os = "macos")]
    impl Drop for MacosAclGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/bin/chmod")
                .arg("-N")
                .arg(&self.path)
                .status();
        }
    }

    #[cfg(target_os = "macos")]
    fn add_macos_acl(path: &Path, entry: &str) -> MacosAclGuard {
        let output = std::process::Command::new("/bin/chmod")
            .arg("+a")
            .arg(entry)
            .arg(path)
            .output()
            .expect("run macOS chmod");
        assert!(
            output.status.success(),
            "chmod +a failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        MacosAclGuard {
            path: path.to_path_buf(),
        }
    }

    #[test]
    fn high_priority_relay_subscribes_to_consensus_safety() {
        use iroha_p2p::network::{SubscriberFilter, message::Topic};

        assert_eq!(
            high_priority_relay_filter(),
            SubscriberFilter::topics([Topic::ConsensusSafety, Topic::Consensus, Topic::Control,])
        );
    }

    #[test]
    fn standard_launcher_does_not_derive_six_sorafs_authority_signers_from_node_key() {
        let dependencies = IrohaRuntimeDeps::default();

        assert!(dependencies.sorafs_stream_token_signer.is_none());
        assert!(dependencies.sorafs_proof_outcome_signer.is_none());
        assert!(dependencies.sorafs_repair_transaction_signer.is_none());
        assert!(dependencies.sorafs_reserve_transaction_signer.is_none());
        assert!(dependencies.sorafs_orderbook_transaction_signer.is_none());
        assert!(dependencies.sorafs_moderation_transaction_signer.is_none());
        assert!(dependencies.sorafs_moderation_settlement_handoff.is_none());
        assert!(dependencies.sorafs_moderation_publication_handoff.is_none());
        assert!(dependencies.sorafs_moderation_panel_notification.is_none());
        assert!(
            dependencies
                .sorafs_governance_dag_ipfs_authenticator
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_governance_dag_head_authenticator
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_governance_dag_checkpoint_store
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_appeal_finance_checkpoint_runtime
                .is_none()
        );
        assert!(dependencies.sorafs_evidence_viewer_webauthn.is_none());
        assert!(dependencies.sorafs_evidence_viewer_grants.is_none());
        assert!(dependencies.sorafs_evidence_viewer_receipt_signer.is_none());
        assert!(dependencies.sorafs_evidence_viewer_erasure.is_none());
        assert!(
            dependencies
                .sorafs_evidence_viewer_checkpoint_store
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_evidence_viewer_compaction_archive
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_moderation_panel_notification_archive
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_pop_credential_provider_registry
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_provider_ingest_checkpoint_runtime
                .is_none()
        );
        assert!(
            dependencies
                .sorafs_provider_ingest_retention_authority
                .is_none()
        );
        assert!(dependencies.sorafs_reputation_retention_authority.is_none());
    }

    #[test]
    fn governance_dag_service_accepts_exact_embedded_publisher_binding() {
        let signer = Arc::new(GovernanceDagPublisherBindingSigner::from_seed(0x51));
        let public_key = signer.public_key_bytes();
        let storage = governance_dag_service_storage(public_key);
        let runtime_deps = IrohaRuntimeDeps::default().with_sorafs_governance_dag_signer(signer);

        validate_governance_dag_service_publisher_binding(
            &storage,
            runtime_deps.sorafs_governance_dag_signer.as_deref(),
        )
        .expect("service, producer configuration, and runtime signer must cross-bind");
        assert_eq!(
            storage.governance_dag_signer_handle.as_deref(),
            Some(GOVERNANCE_DAG_PUBLISHER_HANDLE)
        );
        assert_eq!(storage.governance_dag_signer_revision, Some(1));
        assert_eq!(
            storage.governance_dag_signer_policy_digest,
            Some(GOVERNANCE_DAG_PUBLISHER_POLICY_DIGEST)
        );
        assert_eq!(
            storage.governance_dag_publisher_peer_id.as_deref(),
            Some(GOVERNANCE_DAG_PUBLISHER_PEER_ID)
        );
        assert!(storage.governance_dag_dir.is_some());
    }

    #[test]
    fn disabled_governance_dag_service_leaves_shared_checkpoint_store_for_local_producer() {
        let signer = Arc::new(GovernanceDagPublisherBindingSigner::from_seed(0x57));
        let mut storage = governance_dag_service_storage(signer.public_key_bytes());
        storage.governance_dag_service.enabled = false;
        storage.governance_dag_service.publisher_public_key_hex = None;
        let runtime_deps = IrohaRuntimeDeps::default()
            .with_sorafs_governance_dag_signer(signer)
            .with_sorafs_governance_dag_checkpoint_store(Arc::new(
                GovernanceDagCheckpointBindingStore,
            ));

        let launch = resolve_governance_dag_service_launch(&storage, &runtime_deps)
            .expect("disabled public service must leave producer-owned roles available");

        assert!(
            launch.is_none(),
            "disabled public service must not start a supervised service"
        );
        assert!(
            runtime_deps
                .sorafs_governance_dag_checkpoint_store
                .is_some(),
            "the same resolved checkpoint store remains available for NodeRuntimeDeps"
        );
    }

    #[test]
    fn governance_dag_publisher_binding_signer_produces_valid_ed25519_signatures() {
        let signer = GovernanceDagPublisherBindingSigner::from_seed(0x50);
        let payload =
            sorafs_node::governance_dag_key_transition_signing_payload_v1(1, 2, [0x50; 32])
                .expect("governance key-transition payload");
        let purpose = sorafs_node::GovernanceDagSigningPurposeV1::KeyTransition;
        let signature = sorafs_node::GovernanceDagRuntimeSigner::sign(&signer, purpose, &payload)
            .expect("sign payload");
        let repeated = sorafs_node::GovernanceDagRuntimeSigner::sign(&signer, purpose, &payload)
            .expect("repeat signing");

        assert_eq!(signature, repeated);
        assert_eq!(
            sorafs_node::GovernanceDagRuntimeSigner::handle(&signer),
            GOVERNANCE_DAG_PUBLISHER_HANDLE
        );
        iroha_crypto::Signature::from_bytes(&signature)
            .verify(signer.key_pair.public_key(), &payload)
            .expect("deterministic Governance DAG signature must verify");
    }

    #[test]
    fn governance_dag_service_rejects_substituted_publisher_configuration() {
        let signer = GovernanceDagPublisherBindingSigner::from_seed(0x52);
        let producer_public_key = signer.public_key_bytes();
        let mut storage = governance_dag_service_storage(producer_public_key);
        storage.governance_dag_service.publisher_public_key_hex =
            Some(hex::encode(governance_dag_publisher_public_key(0x53)));

        let error = validate_governance_dag_service_publisher_binding(&storage, Some(&signer))
            .expect_err("substituted service publisher key must fail before state access");
        assert!(
            error
                .to_string()
                .contains("does not match the embedded producer configuration")
        );
    }

    #[test]
    fn governance_dag_service_rejects_missing_or_substituted_runtime_signer() {
        let public_key = governance_dag_publisher_public_key(0x54);
        let storage = governance_dag_service_storage(public_key);

        let missing = resolve_governance_dag_service_launch(&storage, &IrohaRuntimeDeps::default())
            .expect_err("missing embedded producer signer must fail before state access");
        assert!(
            missing
                .to_string()
                .contains("requires the embedded producer runtime signer")
        );

        let substituted = GovernanceDagPublisherBindingSigner::from_seed(0x55);
        let error = validate_governance_dag_service_publisher_binding(&storage, Some(&substituted))
            .expect_err("substituted runtime signer must fail before state access");
        assert!(
            error
                .to_string()
                .contains("does not match the embedded producer runtime signer")
        );
    }

    #[test]
    fn governance_dag_service_rejects_missing_embedded_producer_configuration() {
        let signer = GovernanceDagPublisherBindingSigner::from_seed(0x56);
        let public_key = signer.public_key_bytes();
        let mut storage = governance_dag_service_storage(public_key);
        storage.governance_dag_publisher_public_key_hex = None;

        let error = validate_governance_dag_service_publisher_binding(&storage, Some(&signer))
            .expect_err("missing embedded producer binding must fail before state access");
        assert!(
            error
                .to_string()
                .contains("requires the embedded producer public-key binding")
        );
    }

    #[test]
    fn repository_iroha3_dev_default_config_requests_no_runtime_providers() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../defaults/kagami/iroha3-dev/config.toml");
        let config = load_unprovisioned_profile_for_inspection(&path);
        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project default provider bindings");

        assert!(
            bindings.is_empty(),
            "stock newcomer defaults must not require a deployment registry"
        );
    }

    #[test]
    fn reputation_runtime_provider_presence_fails_closed_in_both_modes() {
        assert!(validate_reputation_runtime_provider_presence(false, [false; 4]).is_ok());
        for provider_presence in [
            [true, false, false, false],
            [false, true, false, false],
            [false, false, true, false],
            [false, false, false, true],
        ] {
            assert_eq!(
                validate_reputation_runtime_provider_presence(false, provider_presence),
                Err("disabled SoraFS reputation runtime rejects unexpected runtime providers")
            );
        }
        assert!(
            validate_reputation_runtime_provider_presence(true, [true; 4]).is_ok(),
            "enabled reputation runtime passes presence validation before exact qualification"
        );
        for (provider_presence, expected_error) in [
            (
                [false, true, true, true],
                "enabled SoraFS reputation runtime requires an injected monotonic journal-checkpoint provider",
            ),
            (
                [true, false, true, true],
                "enabled SoraFS reputation runtime requires an injected authenticated journal-transaction submitter",
            ),
            (
                [true, true, false, true],
                "enabled SoraFS reputation runtime requires an injected external threshold signer",
            ),
            (
                [true, true, true, false],
                "enabled SoraFS reputation runtime requires an injected authenticated Governance DAG adapter",
            ),
        ] {
            assert_eq!(
                validate_reputation_runtime_provider_presence(true, provider_presence),
                Err(expected_error)
            );
        }
    }

    #[test]
    fn reputation_archive_presence_matches_runtime_enablement() {
        assert!(validate_reputation_archive_presence(false, false).is_ok());
        assert!(validate_reputation_archive_presence(true, true).is_ok());
        assert_eq!(
            validate_reputation_archive_presence(true, false),
            Err(
                "enabled SoraFS reputation runtime requires its daemon-owned archive before Sumeragi startup"
            )
        );
        assert_eq!(
            validate_reputation_archive_presence(false, true),
            Err("disabled SoraFS reputation runtime rejects an unexpected Sumeragi archive")
        );
    }

    #[test]
    fn provider_ingest_archive_presence_matches_runtime_enablement() {
        assert!(validate_provider_ingest_archive_presence(false, false).is_ok());
        assert!(validate_provider_ingest_archive_presence(true, true).is_ok());
        assert_eq!(
            validate_provider_ingest_archive_presence(true, false),
            Err(
                "enabled SoraFS provider-ingest runtime requires its daemon-owned archive before Sumeragi startup"
            )
        );
        assert_eq!(
            validate_provider_ingest_archive_presence(false, true),
            Err("disabled SoraFS provider-ingest runtime rejects an unexpected finalized archive")
        );
    }

    #[test]
    fn provider_attestation_journal_remains_fail_closed_until_activation_is_qualified() {
        assert!(validate_provider_attestation_journal_activation(false).is_ok());
        assert_eq!(
            validate_provider_attestation_journal_activation(true),
            Err(
                "SoraFS provider-attestation journal capture is not yet activation-qualified; the concrete finalized-archive scanner, bounded store initialization, rollback-resistant time, approval signer, and authenticated inventory must be wired before enabling it"
            )
        );

        let startup = include_str!("main.rs")
            .split_once("pub(crate) async fn start_with_runtime_deps")
            .expect("runtime-dependency startup entry")
            .1;
        let activation_gate = startup
            .find("validate_provider_attestation_journal_activation")
            .expect("provider-attestation activation gate");
        let supervisor = startup
            .find("let mut supervisor = Supervisor::new()")
            .expect("supervisor construction");
        assert!(
            activation_gate < supervisor,
            "an unqualified capture request must fail before any supervised child starts"
        );
    }

    #[test]
    fn provider_ingest_archive_is_qualified_and_installed_before_runtime_startup() {
        let source = include_str!("main.rs");
        let adapter_preflight = source
            .find("let sorafs_provider_ingest_preflight = match")
            .expect("provider-ingest state-free adapter preflight");
        let preparation = source
            .find("prepare_provider_ingest_finalized_archive_v1")
            .expect("provider-ingest archive preparation");
        let sumeragi_start = source
            .find("let (sumeragi, child) = SumeragiStartArgs")
            .expect("Sumeragi startup");
        assert!(
            adapter_preflight < preparation && preparation < sumeragi_start,
            "external adapter preflight and archive qualification must fail closed before consensus starts"
        );
        let node_state_open = source
            .find(
                "let sorafs_node = sorafs_node::NodeHandle::try_new_with_policies_and_runtime_deps",
            )
            .expect("embedded SoraFS durable-state startup");
        let checkpoint_injection = source
            .find("with_provider_ingest_checkpoint_runtime")
            .expect("provider-ingest checkpoint injection");
        assert!(
            adapter_preflight < checkpoint_injection && checkpoint_injection < node_state_open,
            "provider-ingest external adapters must be qualified before the checkpoint provider can reach NodeHandle or initialize the outbox"
        );
        let provider_runtime_start = source
            .find("let sorafs_provider_ingest_runtime = if let Some")
            .expect("provider-ingest runtime startup");
        assert!(
            sumeragi_start < provider_runtime_start,
            "the commit-capturing archive must be installed before its runtime reader starts"
        );
        let sumeragi_wiring = &source[sumeragi_start..provider_runtime_start];
        assert!(
            sumeragi_wiring.contains("provider_ingest_finalized_archive:"),
            "Sumeragi startup must receive the exact prepared provider-ingest archive"
        );
        let runtime_wiring = &source[provider_runtime_start..];
        assert!(
            runtime_wiring.contains("sorafs_provider_ingest_finalized_query"),
            "provider ingest must consume the archive-only finalized query"
        );
        assert!(
            runtime_wiring.contains("sorafs_provider_ingest_preflight"),
            "provider ingest must consume the opaque state-free preflight token"
        );
    }

    #[test]
    fn enabled_reputation_runtime_has_no_validator_key_submitter_fallback() {
        let reputation_startup = include_str!("main.rs")
            .split_once(
                "let sorafs_reputation_runtime = if let Some(reputation_config) = \
                 sorafs_reputation_config",
            )
            .expect("reputation startup branch")
            .1
            .split_once("let sorafs_hedging_billing_runtime")
            .expect("reputation startup branch boundary")
            .0;
        assert!(
            !reputation_startup.contains("config.common.key_pair"),
            "the standard launcher must never adapt the validator key into reputation authority"
        );
        assert!(
            reputation_startup
                .contains("sorafs_reputation_journal_transaction_submitter_override.ok_or_else"),
            "enabled reputation startup must explicitly require the injected submitter"
        );
        assert!(
            reputation_startup.contains("sorafs_reputation_journal_checkpoint_provider"),
            "enabled reputation startup must pass the injected monotonic journal checkpoint provider"
        );
    }

    #[test]
    fn reputation_runtime_defers_assembly_until_archive_activation() {
        let source = include_str!("main.rs");
        let reputation_startup = source
            .split_once(
                "let sorafs_reputation_runtime = if let Some(reputation_config) = \
                 sorafs_reputation_config",
            )
            .expect("reputation startup branch")
            .1
            .split_once("let sorafs_hedging_billing_runtime")
            .expect("reputation startup branch boundary")
            .0;
        assert!(
            reputation_startup
                .contains("let reputation_archive_active = reputation_archive_activation")
                && reputation_startup.contains(".activation_ready()"),
            "reputation startup must evaluate the prepared archive activation gate"
        );
        assert!(
            reputation_startup.contains("sorafs_reputation_runtime::start_deferred"),
            "an unactivated archive must use nonblocking deferred runtime assembly"
        );
        assert!(
            reputation_startup.contains("ReputationRuntimeActivationProbeV1"),
            "deferred assembly must retain the exact prepared activation probe"
        );
    }

    #[test]
    fn configured_appeal_finance_roles_have_exact_ordered_bindings() {
        use iroha_config::parameters::actual::{
            SorafsAppealFinanceCheckpointBinding, SorafsAppealFinanceSignerBinding,
        };
        use iroha_crypto::KeyPair;

        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("resolve repository default config");
        let signer_a = KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::Ed25519)
            .expect("derive first appeal-finance signer");
        let signer_b = KeyPair::try_from_seed(vec![0xA2; 32], Algorithm::Ed25519)
            .expect("derive rotated appeal-finance signer");
        let checkpoint = KeyPair::try_from_seed(vec![0xA3; 32], Algorithm::Ed25519)
            .expect("derive independent checkpoint signer");
        let appeal_finance = &mut config.torii.sorafs_appeal_finance_settlement;
        appeal_finance.submitter_signers = vec![
            SorafsAppealFinanceSignerBinding {
                handle: "pkcs11:appeal-finance-a".to_owned(),
                authority: AccountId::new(signer_a.public_key().clone()),
                public_key: signer_a.public_key().clone(),
                revision: 7,
                policy_digest: [0xA7; 32],
                valid_from_block_height: 1,
                revoked_at_block_height: Some(10),
            },
            SorafsAppealFinanceSignerBinding {
                handle: "pkcs11:appeal-finance-b".to_owned(),
                authority: AccountId::new(signer_b.public_key().clone()),
                public_key: signer_b.public_key().clone(),
                revision: 8,
                policy_digest: [0xA8; 32],
                valid_from_block_height: 10,
                revoked_at_block_height: None,
            },
        ];
        appeal_finance.checkpoint_provider = Some(SorafsAppealFinanceCheckpointBinding {
            handle: "kms:appeal-finance-checkpoint".to_owned(),
            public_key: checkpoint.public_key().clone(),
            revision: 3,
            policy_digest: [0xA3; 32],
        });

        let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project configured appeal-finance provider bindings");
        let observed: Vec<_> = bindings
            .iter()
            .map(|binding| {
                (
                    binding.slot(),
                    binding.handle().to_owned(),
                    binding.revision(),
                    binding.policy_digest(),
                )
            })
            .collect();

        assert_eq!(
            observed,
            vec![
                (
                    IrohaRuntimeProviderSlotV1::AppealFinanceTransactionSigner,
                    "pkcs11:appeal-finance-a".to_owned(),
                    Some(7),
                    Some([0xA7; 32]),
                ),
                (
                    IrohaRuntimeProviderSlotV1::AppealFinanceTransactionSigner,
                    "pkcs11:appeal-finance-b".to_owned(),
                    Some(8),
                    Some([0xA8; 32]),
                ),
                (
                    IrohaRuntimeProviderSlotV1::AppealFinanceCheckpoint,
                    "kms:appeal-finance-checkpoint".to_owned(),
                    Some(3),
                    Some([0xA3; 32]),
                ),
            ]
        );
        assert_eq!(
            bindings
                .iter()
                .filter(|binding| {
                    binding.slot() == IrohaRuntimeProviderSlotV1::AppealFinanceTransactionSigner
                })
                .count(),
            2,
            "multiple bindings for the appeal-finance signer role are intentional"
        );
    }

    #[test]
    fn standard_launcher_resolves_and_forwards_deployment_runtime_dependencies() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let run_main_source = compact_source
            .split_once("fnrun_main(")
            .expect("run_main source")
            .1
            .split_once("fnvalidate_config_for_check(")
            .expect("run_main source boundary")
            .0;
        let run_node_source = compact_source
            .split_once("asyncfnrun_node(")
            .expect("run_node source")
            .1
            .split_once("fnlog_norito_banner(")
            .expect("run_node source boundary")
            .0;

        assert!(
            run_main_source.contains(
                "runtime_provider_registry::resolve_runtime_deps(&config,runtime_provider_registry)"
            ),
            "standard CLI startup must resolve the sanitized deployment registry request"
        );
        assert!(
            run_main_source.contains(
                "(!bindings.is_empty()).then(runtime_provider_broker::StockRuntimeProviderBrokerRegistryV1::new)"
            ),
            "the stock broker registry must be instantiated only for a non-empty binding catalog"
        );
        assert!(
            run_main_source.contains(
                "runtime_provider_registry.or_else(||{stock_runtime_provider_registry.as_ref()"
            ),
            "an explicitly injected deployment registry must remain authoritative"
        );
        assert!(
            run_main_source.contains(
                "rt.block_on(run_node(config,genesis,runtime_deps,musubi_publication_factory))"
            ),
            "standard CLI startup must forward the resolved dependency set and private publication factory"
        );
        assert!(
            run_node_source.contains(
                "Iroha::start_with_runtime_deps(config,genesis,logger,shutdown_on_panic,runtime_deps,musubi_publication_factory)"
            ),
            "daemon startup must consume the resolved dependency set and private publication factory"
        );
        assert!(
            !run_main_source
                .contains("letruntime_deps=IrohaRuntimeDeps::default();rt.block_on(run_node("),
            "standard CLI startup must not replace registry output with Default"
        );

        let validation = run_main_source
            .find("validate_config_offline(&config).change_context(MainError::Config)?")
            .expect("offline validation in run_main");
        let binding_projection = run_main_source
            .find("IrohaRuntimeProviderBindingsV1::try_from_config(&config)")
            .expect("runtime-provider binding projection in run_main");
        let stock_broker = run_main_source
            .find("StockRuntimeProviderBrokerRegistryV1::new")
            .expect("stock runtime-provider broker construction in run_main");
        let resolution = run_main_source
            .find(
                "runtime_provider_registry::resolve_runtime_deps(&config,runtime_provider_registry)",
            )
            .expect("provider resolution in run_main");
        let runtime_start = run_main_source
            .find("tokio::runtime::Builder::new_multi_thread()")
            .expect("Tokio runtime construction in run_main");
        assert!(
            validation < binding_projection
                && binding_projection < stock_broker
                && stock_broker < resolution
                && resolution < runtime_start,
            "fixed-binding preflight must precede broker construction, and provider resolution must precede Tokio/node startup"
        );
    }

    #[test]
    fn explicit_musubi_private_factory_is_late_bound_and_supervised_fail_closed() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        assert!(
            compact_source.contains("run_main(build_line,None,None)"),
            "the stock launcher must not construct or start a private publication service"
        );
        assert!(
            compact_source.contains("run_main(build_line,Some(registry),None)"),
            "the existing custom registry launcher must preserve fail-closed publication defaults"
        );
        assert!(
            compact_source.contains("run_main(build_line,None,Some(factory))"),
            "the standalone publication launcher must not require an unrelated provider registry"
        );
        assert!(
            compact_source.contains("run_main(build_line,Some(registry),Some(factory))"),
            "the combined custom launcher must inject both deployment-owned dependencies and the late-bound publication factory"
        );

        let startup_source = compact_source
            .split_once("pub(crate)asyncfnstart_with_runtime_deps(")
            .expect("start_with_runtime_deps source")
            .1
            .split_once("fnvalidate_membership_snapshot_against_live_peers(")
            .expect("start_with_runtime_deps source boundary")
            .0;
        let sorafs_node_ready = startup_source
            .find("sorafs_node::NodeHandle::try_new_with_policies_and_runtime_deps(")
            .expect("SoraFS node construction");
        let publication_context = startup_source
            .find(
                "musubi_publication_service::MusubiPublicationPrivateServiceContextV1::new(NetworkId::from_genesis_hash(config.genesis.expected_hash),Arc::clone(&state),Arc::clone(&queue),sorafs_node.clone())",
            )
            .expect("private publication context construction");
        let torii_runtime_deps = startup_source
            .find("letruntime_deps=iroha_torii::ToriiRuntimeDeps::new(torii_telemetry)")
            .expect("Torii runtime dependency construction");
        let signal_setup = startup_source
            .find("supervisor.setup_shutdown_on_os_signals()")
            .expect("OS signal setup");
        let publication_start = startup_source
            .find(
                "musubi_publication_service::build_and_start_injected_musubi_publication_private_service_v1(musubi_publication_factory,musubi_publication_context,supervisor.shutdown_signal(),)",
            )
            .expect("late-bound publication service startup");
        let publication_monitor = startup_source
            .find("ifletSome(child)=publication_child{supervisor.monitor(child);}")
            .expect("publication child supervision");
        let external_signal = startup_source
            .find("supervisor.shutdown_on_external_signal(shutdown_signal)")
            .expect("external shutdown signal hookup");
        assert!(
            sorafs_node_ready < publication_context
                && publication_context < torii_runtime_deps
                && torii_runtime_deps < signal_setup
                && signal_setup < publication_start
                && publication_start < publication_monitor
                && publication_monitor < external_signal,
            "the private factory must receive ready daemon handles after replay, start only after fallible signal setup, and join the node supervisor"
        );
    }

    #[test]
    fn standard_launcher_source_forbids_six_node_key_sorafs_signer_adaptations() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for builder in [
            "with_sorafs_stream_token_signer",
            "with_sorafs_proof_outcome_signer",
            "with_sorafs_repair_transaction_signer",
            "with_sorafs_reserve_transaction_signer",
            "with_sorafs_orderbook_transaction_signer",
            "with_sorafs_moderation_transaction_signer",
        ] {
            let forbidden = [".", builder, "(Arc::new(config.common.key_pair.clone()))"].concat();
            assert!(
                !compact_source.contains(&forbidden),
                "standard launcher must not adapt the node key through {builder}"
            );
        }
    }

    #[test]
    fn standard_launcher_forwards_external_sorafs_runtime_dependencies() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for (field, builder) in [
            (
                "sorafs_appeal_finance_checkpoint_runtime",
                "with_sorafs_appeal_finance_checkpoint_runtime",
            ),
            (
                "sorafs_evidence_viewer_webauthn",
                "with_sorafs_evidence_viewer_webauthn",
            ),
            (
                "sorafs_moderation_panel_notification",
                "with_sorafs_moderation_panel_notification",
            ),
            (
                "sorafs_moderation_panel_notification_archive",
                "with_sorafs_moderation_panel_notification_archive",
            ),
            (
                "sorafs_evidence_viewer_grants",
                "with_sorafs_evidence_viewer_grants",
            ),
            (
                "sorafs_evidence_viewer_receipt_signer",
                "with_sorafs_evidence_viewer_receipt_signer",
            ),
            (
                "sorafs_evidence_viewer_erasure",
                "with_sorafs_evidence_viewer_erasure",
            ),
            (
                "sorafs_evidence_viewer_checkpoint_store",
                "with_sorafs_evidence_viewer_checkpoint_store",
            ),
            (
                "sorafs_evidence_viewer_compaction_archive",
                "with_sorafs_evidence_viewer_compaction_archive",
            ),
            (
                "sorafs_evidence_viewer_transparency_publisher",
                "with_sorafs_evidence_viewer_transparency_publisher",
            ),
            (
                "sorafs_gateway_acme_client",
                "with_sorafs_gateway_acme_client",
            ),
            (
                "sorafs_gateway_compliance_feed_transport",
                "with_sorafs_gateway_compliance_feed_transport",
            ),
        ] {
            let clone_from_external = ["runtime_deps.", field, ".clone()"].concat();
            let forward_to_torii = [".", builder, "("].concat();
            assert!(
                compact_source.contains(&clone_from_external),
                "standard launcher must clone external dependency `{field}` before Torii dependency assembly"
            );
            assert!(
                compact_source.contains(&forward_to_torii),
                "standard launcher must forward `{field}` through `{builder}`"
            );
        }
        assert!(
            compact_source.contains("runtime_deps.sorafs_pop_credential_provider_registry.clone()"),
            "standard launcher must clone the deployment-owned PoP provider registry"
        );
        assert!(
            compact_source.contains("sorafs_pop_runtime::build("),
            "standard launcher must build the config-bound PoP runtime"
        );
        assert!(
            compact_source.contains(".with_sorafs_pop_credentials("),
            "standard launcher must forward only the qualified PoP runtime to Torii"
        );
        let forbidden_gateway_fallback =
            ["ProductionGatewayComplianceFeedTransport", "::try_new"].concat();
        assert!(
            !compact_source.contains(&forbidden_gateway_fallback),
            "standard launcher must not replace a missing deployment-owned compliance transport with an in-process fallback"
        );
        assert!(
            compact_source.contains(
                "enabledSoraFSgatewaycompliancerequirestheexactdeployment-ownedauthenticatedfeedtransport"
            ),
            "enabled gateway compliance must fail before Torii startup when its deployment transport is absent"
        );
        assert!(
            compact_source.contains(
                "configuredSoraFSgatewayACMEautomationrequirestheexactdeployment-ownedACMEclient"
            ),
            "configured ACME automation must fail before Torii startup when its deployment client is absent"
        );
        assert!(
            compact_source
                .contains("disabledSoraFSgatewaycompliancerejectsanunexpectedfeedtransport"),
            "disabled gateway compliance must reject an injected transport"
        );
        assert!(
            compact_source
                .contains("unconfiguredSoraFSgatewayACMEautomationrejectsanunexpectedclient"),
            "unconfigured ACME automation must reject an injected client"
        );
    }

    #[test]
    fn standard_launcher_forwards_and_supervises_por_replay_archival() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let clone = compact_source
            .find("runtime_deps.sorafs_por_finalized_replay_archive.clone()")
            .expect("launcher clones the deployment-owned PoR replay archive");
        let inject = compact_source
            .find(".with_por_finalized_replay_archive(Arc::clone(archive))")
            .expect("launcher injects the archive into NodeRuntimeDeps");
        let node = compact_source
            .find("sorafs_node::NodeHandle::try_new_with_policies_and_runtime_deps(")
            .expect("launcher constructs the archive-aware node");
        let reputation = compact_source
            .find("letsorafs_reputation_runtime=ifletSome(reputation_config)")
            .expect("launcher starts the committed reputation runtime");
        let worker = compact_source
            .find("sorafs_por_replay_archive_runtime::start(")
            .expect("launcher starts the supervised reconciliation/compaction worker");
        let reputation_only_worker = compact_source
            .find("sorafs_por_replay_archive_runtime::start_reputation_reconciliation(")
            .expect("launcher starts PoR reputation reconciliation without optional archival");

        assert!(
            clone < inject
                && inject < node
                && node < reputation
                && reputation < worker
                && reputation < reputation_only_worker,
            "the exact archive must enter the node before routes are built, and bounded compaction must start only after durable reputation admission"
        );
        assert!(compact_source.contains(
            "dynsorafs_node::reputation::runtime::ReputationNativeOutcomeAdmissionApiV1"
        ));
        assert!(
            compact_source.contains("reputation_config.poll_interval,reputation_config.page_items"),
            "the reputation-only callback worker must use validated bounded runtime policy"
        );
    }

    #[test]
    fn standard_launcher_forwards_both_fenced_privacy_roles_to_the_node() {
        let source = include_str!("main.rs");
        let launcher_start = source
            .find("pub(crate) async fn start_with_runtime_deps(")
            .expect("standard launcher function");
        let launcher_end = source[launcher_start..]
            .find("\n    /// Read-only handle to the world state view.")
            .map(|offset| launcher_start + offset)
            .expect("end of standard launcher function");
        let compact_source: String = source[launcher_start..launcher_end]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for field in [
            "sorafs_fenced_transparency_publisher",
            "sorafs_fenced_transparency_head_reader",
            "sorafs_governance_dag_signer",
            "sorafs_governance_dag_checkpoint_store",
        ] {
            assert!(
                compact_source.contains(&["runtime_deps.", field, ".clone()"].concat()),
                "standard launcher must clone deployment-owned `{field}`"
            );
        }
        let writer = compact_source
            .find(".with_fenced_transparency_publisher(Arc::clone(publisher))")
            .expect("launcher forwards the fused privacy writer");
        let reader = compact_source
            .find(".with_fenced_transparency_head_reader(Arc::clone(reader))")
            .expect("launcher forwards the authenticated head reader");
        let signer = compact_source
            .find(".with_governance_dag_signer(Arc::clone(signer))")
            .expect("launcher forwards the signed Governance DAG publisher");
        let checkpoint_store = compact_source
            .find(".with_governance_dag_checkpoint_store(Arc::clone(checkpoint_store))")
            .expect("launcher forwards the sealed Governance DAG producer checkpoint store");
        let node = compact_source
            .find("sorafs_node::NodeHandle::try_new_with_policies_and_runtime_deps(")
            .expect("launcher constructs the embedded SoraFS node");
        assert!(
            writer < reader
                && reader < signer
                && signer < checkpoint_store
                && checkpoint_store < node,
            "the fused privacy role pair, signed Governance publisher, and sealed producer store must enter NodeRuntimeDeps before node construction"
        );
        let torii_deps = compact_source
            .find("let runtime_deps=iroha_torii::ToriiRuntimeDeps::new(")
            .expect("launcher assembles Torii dependencies");
        let torii_start = compact_source[torii_deps..]
            .find("let torii=Torii::new_with_handle(")
            .map(|offset| torii_deps + offset)
            .expect("launcher constructs Torii");
        assert!(
            !compact_source[torii_deps..torii_start]
                .contains("with_sorafs_governance_dag_checkpoint_store"),
            "irohad must retain the raw producer checkpoint store inside its prebuilt node instead of injecting it into Torii a second time"
        );
    }

    include!("main/governance_dag_launcher_tests.rs");

    #[test]
    fn standard_launcher_binds_kura_local_peer_before_start() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let construct = compact_source
            .find("Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(")
            .expect("standard launcher constructs Kura");
        let bind = compact_source
            .find(
                "kura.bind_local_peer_id(PeerId::new(config.common.key_pair.public_key().clone()))",
            )
            .expect("standard launcher binds the configured node identity to Kura");
        let start = compact_source
            .find("Kura::start(kura.clone(),supervisor.shutdown_signal())")
            .expect("standard launcher starts Kura");
        assert!(
            construct < bind && bind < start,
            "Kura eviction authority must bind the configured local PeerId exactly once before its runtime starts"
        );
    }

    #[test]
    fn standard_launcher_pop_runtime_uses_only_config_and_the_injected_registry() {
        let compact_source: String = include_str!("main.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let corridor = compact_source
            .split_once("letsorafs_pop_credentials=sorafs_pop_runtime::build(")
            .map(|(_, suffix)| suffix)
            .expect("standard launcher must build the PoP runtime");
        let corridor = corridor
            .split_once("//Logdetailedbacktraces")
            .map(|(corridor, _)| corridor)
            .expect("PoP startup corridor must end before daemon subsystem startup");
        assert!(
            corridor.starts_with(
                "config.torii.sorafs_storage.pop_credentials.as_ref(),runtime_deps.sorafs_pop_credential_provider_registry.clone(),)"
            ),
            "PoP startup must use only public config and the injected registry"
        );
        for forbidden in [
            "config.common.key_pair",
            "private_key",
            "std::env",
            "env::",
            "std::fs",
            "fs::",
            "SystemTime",
            "PopCredentialRuntimeSecretsV1",
            "PopCredentialToriiRuntimeV1::open",
        ] {
            assert!(
                !corridor.contains(forbidden),
                "standard PoP startup corridor must not contain `{forbidden}`"
            );
        }
    }

    mod scheduler_banner {
        use super::*;

        #[test]
        fn formats_core_count() {
            assert_eq!(scheduler_banner_line(1), "Using 1 core");
            assert_eq!(scheduler_banner_line(4), "Using 4 cores");
        }

        #[test]
        fn clamps_zero_to_one_core() {
            assert_eq!(scheduler_banner_line(0), "Using 1 core");
        }
    }

    mod replay_startup_config {
        use super::*;

        #[test]
        fn installs_actual_zk_and_settlement_config_before_kura_replay() {
            let config_table = toml::toml! {
                chain = "00000000-0000-0000-0000-000000000000"
                public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"

                [network]
                address = "addr:127.0.0.1:1337#8F78"
                public_address = "addr:127.0.0.1:1337#8F78"

                [genesis]
                public_key = "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4"
                file = "./genesis.signed.nrt"
                expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

                [streaming]
                identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
                identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"

                [torii]
                address = "addr:127.0.0.1:8080#8942"
            };
            let mut config = ConfigReader::new()
                .with_toml_source(TomlSource::inline(config_table))
                .read_and_complete::<UserConfig>()
                .expect("sample config should be readable")
                .parse()
                .expect("sample config should parse");
            config.zk.sccp.max_pending_outbound_messages =
                std::num::NonZeroU64::new(7).expect("nonzero message cap");
            config.zk.sccp.max_pending_outbound_payload_bytes =
                std::num::NonZeroU64::new(11).expect("nonzero byte cap");
            let offline_asset_definition_id = AssetDefinitionId::derive_from_components(
                iroha_data_model::domain::DomainId::try_new("boi", "is")
                    .expect("offline asset domain"),
                "ds".parse().expect("offline asset name"),
            );
            let offline_escrow_account_id = iroha_test_samples::ALICE_ID.clone();
            config.settlement.offline.escrow_accounts.insert(
                offline_asset_definition_id.clone(),
                offline_escrow_account_id.clone(),
            );

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);

            install_zk_config_before_kura_replay(&mut state, &config)
                .expect("fresh state accepts actual ZK configuration");
            apply_state_runtime_config_before_snapshot_auth(&mut state, &config);

            let installed = state.zk_snapshot();
            assert_eq!(
                installed.sccp.max_pending_outbound_messages,
                config.zk.sccp.max_pending_outbound_messages
            );
            assert_eq!(
                installed.sccp.max_pending_outbound_payload_bytes,
                config.zk.sccp.max_pending_outbound_payload_bytes
            );
            assert_eq!(
                state
                    .settlement()
                    .offline
                    .escrow_accounts
                    .get(&offline_asset_definition_id),
                Some(&offline_escrow_account_id),
                "the exact offline escrow catalog must be installed before Kura replay",
            );
        }
    }

    mod fastpq_overrides {
        use super::*;
        use iroha_config::parameters::actual::{Fastpq, FastpqExecutionMode, FastpqPoseidonMode};

        #[test]
        fn maps_metal_overrides_from_config() {
            let cfg = Fastpq {
                execution_mode: FastpqExecutionMode::Cpu,
                poseidon_mode: FastpqPoseidonMode::Cpu,
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
                metal_max_in_flight: Some(8),
                metal_threadgroup_width: Some(128),
                metal_trace: true,
                metal_debug_enum: true,
                metal_debug_fused: false,
            };

            let overrides = fastpq_metal_overrides_from_config(&cfg);
            assert_eq!(overrides.max_in_flight, Some(8));
            assert_eq!(overrides.threadgroup_size, Some(128));
            assert!(overrides.dispatch_trace);
            assert!(overrides.debug_enum);
            assert!(!overrides.debug_fused);
        }

        #[cfg(feature = "fastpq-gpu")]
        #[test]
        fn poseidon_word_preflight_respects_fastpq_config() {
            let mut cfg = Fastpq {
                execution_mode: FastpqExecutionMode::Cpu,
                poseidon_mode: FastpqPoseidonMode::Cpu,
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
                metal_trace: false,
                metal_debug_enum: false,
                metal_debug_fused: false,
            };

            assert!(!fastpq_poseidon_word_preflight_enabled(&cfg));
            cfg.poseidon_mode = FastpqPoseidonMode::Gpu;
            assert!(fastpq_poseidon_word_preflight_enabled(&cfg));
            cfg.poseidon_mode = FastpqPoseidonMode::Cpu;
            assert!(!fastpq_poseidon_word_preflight_enabled(&cfg));
        }
    }

    mod torii_receipt_signer_selection {
        use super::*;

        #[test]
        fn default_is_stable_and_domain_separated_from_node_identity() {
            let node = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let first = torii_receipt_signer_or_derived(None, &node)
                .expect("checked receipt signer derivation should succeed");
            let second = torii_receipt_signer_or_derived(None, &node)
                .expect("checked receipt signer derivation should be repeatable");
            assert_eq!(first.algorithm(), Algorithm::Secp256k1);
            assert_eq!(first.public_key(), second.public_key());
            assert_ne!(first.public_key(), node.public_key());
        }

        #[test]
        fn different_node_identities_derive_different_receipt_signers() {
            let first_node = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let second_node = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let first = torii_receipt_signer_or_derived(None, &first_node)
                .expect("first checked receipt signer derivation should succeed");
            let second = torii_receipt_signer_or_derived(None, &second_node)
                .expect("second checked receipt signer derivation should succeed");
            assert_ne!(first.public_key(), second.public_key());
        }

        #[test]
        fn preserves_configured_receipt_signer() {
            let configured = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let unrelated_node = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let signer = torii_receipt_signer_or_derived(Some(configured.clone()), &unrelated_node)
                .expect("configured receipt signer should not require derivation");
            assert_eq!(signer.public_key(), configured.public_key());
            assert_eq!(signer.algorithm(), Algorithm::Ed25519);
        }
    }

    mod relay_ingress {
        use super::*;
        use iroha_core::torii_proxy::{
            TORII_PROXY_REQUEST_VERSION_V5, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV4, ToriiProxyRequestV5,
            ToriiProxyResponseFormatV1, ToriiProxyResponseV1, ToriiReadEndpointV1,
            ToriiReadProxyRequestV1, ToriiRouteHintV1,
        };
        use iroha_crypto::Hash;
        use iroha_data_model::nexus::{DataSpaceId, LaneId};

        #[test]
        fn torii_proxy_frames_are_not_low_priority() {
            let route = ToriiRouteHintV1 {
                lane_id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(0),
            };
            let request =
                iroha_core::NetworkMessage::ToriiProxyRequest(Arc::new(ToriiProxyRequestV5 {
                    schema_version: TORII_PROXY_REQUEST_VERSION_V5,
                    request_id: Hash::new(b"torii-proxy-request"),
                    hop_count: 1,
                    max_hops: 3,
                    visited_peer_ids: Vec::new(),
                    request: ToriiProxyRequestKindV4::Read(ToriiReadProxyRequestV1 {
                        endpoint: ToriiReadEndpointV1::AccountsList,
                        expected_route: route,
                        path_args: Vec::new(),
                        query_string: None,
                        body: Vec::new(),
                        response_format: ToriiProxyResponseFormatV1::Json,
                    }),
                }));
            let response =
                iroha_core::NetworkMessage::ToriiProxyResponse(Box::new(ToriiProxyResponseV1 {
                    schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
                    request_id: Hash::new(b"torii-proxy-response"),
                    response: ToriiProxyHttpResponseV1 {
                        status_code: 200,
                        headers: Vec::new(),
                        body: Vec::new(),
                    },
                }));

            assert!(!NetworkRelayShared::should_apply_low_priority_ingress(
                &request
            ));
            assert!(!NetworkRelayShared::should_apply_low_priority_ingress(
                &response
            ));
        }
    }

    mod norito_archive_len {
        use super::*;

        fn base_config() -> Config {
            let table = toml::toml! {
                chain = "00000000-0000-0000-0000-000000000000"
                public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
                trusted_peers_pop = [
                  { public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }
                ]

                [network]
                address = "addr:127.0.0.1:1337#8F78"
                public_address = "addr:127.0.0.1:1337#8F78"

                [torii]
                address = "addr:127.0.0.1:8080#8942"

                [genesis]
                public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

                [streaming]
                identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
                identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
            };

            Config::from_toml_source(TomlSource::inline(table)).expect("base config")
        }

        #[test]
        fn resolves_to_network_frame_when_larger() {
            let mut config = base_config();
            config.norito.max_archive_len = 32 * 1024 * 1024;
            config.network.max_frame_bytes = 128 * 1024 * 1024;

            let resolved = resolve_norito_max_archive_len(&config);

            assert_eq!(resolved, 128 * 1024 * 1024);
        }

        #[test]
        fn preserves_requested_when_already_largest() {
            let mut config = base_config();
            config.norito.max_archive_len = 256 * 1024 * 1024;
            config.network.max_frame_bytes = 64 * 1024 * 1024;

            let resolved = resolve_norito_max_archive_len(&config);

            assert_eq!(resolved, 256 * 1024 * 1024);
        }
    }

    mod consensus_ingress_limits {
        use super::*;
        use std::num::NonZeroU32;

        #[test]
        fn bulk_scale_factor_scales_for_faster_block_time() {
            let scale = ConsensusIngressLimiter::bulk_scale_factor(Duration::from_millis(50));
            assert_eq!(scale, 20);
        }

        #[test]
        fn bulk_scale_factor_clamps_for_slower_pipelines() {
            let scale = ConsensusIngressLimiter::bulk_scale_factor(Duration::from_secs(5));
            assert_eq!(scale, 1);
        }

        #[test]
        fn bucket_config_scaled_multiplies_rate_and_burst() {
            let cfg = BucketConfig {
                rate_per_sec: NonZeroU32::new(2).expect("non-zero"),
                burst: NonZeroU32::new(3).expect("non-zero"),
            };
            let scaled = cfg.scaled(2);
            assert_eq!(scaled.rate_per_sec.get(), 4);
            assert_eq!(scaled.burst.get(), 6);
        }
    }

    mod relay_fairness {
        use super::*;
        use crate::network_relay_tests::{
            sample_peer, sample_v2_round, sample_v2_subject, sumeragi_msg, v2_vote_msg,
        };
        use iroha_core::sumeragi::message::BlockMessage;
        use iroha_data_model::block::consensus_v2;
        use iroha_p2p::network::{NetworkReplyRoute, NetworkReplyRouteTestFixture};
        use tokio::sync::mpsc;

        fn relay_source(class: SumeragiRelayClass) -> SumeragiRelaySource {
            SumeragiRelaySource {
                class,
                via: PeerId::new(KeyPair::random().public_key().clone()),
            }
        }

        fn relay_geometry(
            subscriber_base: usize,
            network_per_lane: usize,
            authenticated_source_count: usize,
        ) -> SumeragiRelayCapacityGeometry {
            SumeragiRelayCapacityGeometry::checked(
                subscriber_base,
                network_per_lane,
                authenticated_source_count,
            )
            .expect("test relay geometry must be exactly representable")
        }

        fn relay_with_upstream_source_credit(
            authenticated_peer: Peer,
            semantic_peer: Option<Peer>,
            payload_bytes: usize,
            upstream: &Arc<Semaphore>,
        ) -> RelayWorkItem {
            let mut relay = RelayWorkItem::new(authenticated_peer, v2_vote_msg(), payload_bytes);
            if let Some(semantic_peer) = semantic_peer {
                relay.peer = semantic_peer;
            }
            relay.retain_authenticated_source_credit(
                Arc::clone(upstream)
                    .try_acquire_owned()
                    .expect("upstream source credit remains"),
            );
            relay
        }

        struct DaemonCreditSourceFixture {
            authenticated_via: PeerId,
            responsive_via: PeerId,
            upstream: Arc<Semaphore>,
            responsive_upstream: Arc<Semaphore>,
        }

        fn enqueue_daemon_credit_source_fixture(
            high: &mpsc::Sender<RelayWorkItem>,
            network_per_lane: usize,
        ) -> DaemonCreditSourceFixture {
            let via = sample_peer();
            let authenticated_via = via.id().clone();
            let upstream = Arc::new(Semaphore::new(network_per_lane));
            for seed in 0..8_u64 {
                let key =
                    KeyPair::from_seed((seed + 1).to_le_bytes().repeat(4), Algorithm::Ed25519);
                let semantic_peer = Peer::new(
                    "127.0.0.1:1".parse().expect("semantic origin address"),
                    key.public_key().clone(),
                );
                high.try_send(relay_with_upstream_source_credit(
                    via.clone(),
                    Some(semantic_peer),
                    1,
                    &upstream,
                ))
                .expect("the same high-priority lane retains A1 through A8");
            }
            high.try_send(relay_with_upstream_source_credit(via, None, 9, &upstream))
                .expect("A9 remains behind A1..A8 on the same high-priority lane");

            let responsive = sample_peer();
            let responsive_via = responsive.id().clone();
            let responsive_upstream = Arc::new(Semaphore::new(1));
            high.try_send(relay_with_upstream_source_credit(
                responsive,
                None,
                10,
                &responsive_upstream,
            ))
            .expect("B sits behind blocked A9 on the exact same priority lane");
            DaemonCreditSourceFixture {
                authenticated_via,
                responsive_via,
                upstream,
                responsive_upstream,
            }
        }

        async fn observe_daemon_credit_delivery_ranks(
            receiver: &mut mpsc::Receiver<CreditedSumeragiRelayWorkItem>,
            authenticated_via: &PeerId,
            responsive_via: &PeerId,
            expected_count: usize,
        ) -> (Option<usize>, Option<usize>) {
            let mut responsive_rank = None;
            let mut ninth_rank = None;
            for rank in 1..=expected_count {
                let credited = receiver
                    .recv()
                    .await
                    .expect("exact credited work remains queued");
                if credited.work.work.authenticated_via() == responsive_via {
                    responsive_rank = Some(rank);
                }
                if credited.work.work.authenticated_via() == authenticated_via
                    && credited.work.work.payload_bytes == 9
                {
                    ninth_rank = Some(rank);
                }
                drop(credited);
            }
            (responsive_rank, ninth_rank)
        }

        fn indexed_v2_vote_block_message(height: u64, marker: u8) -> BlockMessage {
            BlockMessage::V2(consensus_v2::ConsensusMessageV2::new(
                consensus_v2::ConsensusMessageV2Payload::Vote(consensus_v2::Vote {
                    round: sample_v2_round(height, 7),
                    proposal_round: sample_v2_round(height, 7),
                    phase: consensus_v2::GlobalPhase::Prepare,
                    subject: sample_v2_subject(),
                    execution_commitment:
                        consensus_v2::ExecutionCommitment::without_topups_or_merge_carrier(
                            Hash::prehashed([marker; 32]),
                            Hash::prehashed([marker.wrapping_add(1); 32]),
                            Hash::prehashed([marker.wrapping_add(2); 32]),
                            1,
                            Hash::prehashed([marker.wrapping_add(3); 32]),
                        ),
                    signer: 0,
                    signature: vec![marker],
                }),
            ))
        }

        async fn prepared_v2_relay_work(
            peer: Peer,
            message: BlockMessage,
            fixture: &mut NetworkReplyRouteTestFixture,
            source_credits: &SumeragiRelaySourceCredits,
            payload_bytes: usize,
        ) -> (
            PreparedSumeragiRelayWork,
            oneshot::Receiver<SumeragiRelayTerminalOutcome>,
            NetworkReplyRoute,
        ) {
            let route = fixture.mint(peer.id().clone());
            let mut relay = RelayWorkItem::new(peer, sumeragi_msg(message), payload_bytes);
            relay
                .reattach_reply_route(route.clone())
                .expect("fixture route must match the exact relay occurrence");
            let upstream = Arc::new(Semaphore::new(1));
            relay.retain_authenticated_source_credit(
                upstream
                    .try_acquire_owned()
                    .expect("upstream fixture source credit remains"),
            );
            let (peer, authenticated_via, payload, _, reply_route, p2p_guard) =
                relay.into_parts_with_reply_route();
            let iroha_core::NetworkMessage::SumeragiBlock(message) = payload else {
                panic!("v2 relay fixture must preserve a Sumeragi block");
            };
            let reply_route = reply_route.expect("v2 relay fixture retains its exact route");
            let inbound = InboundBlockMessage::try_from_transport_with_reply_route(
                Arc::unwrap_or_clone(message).into_message(),
                peer.id().clone(),
                authenticated_via.clone(),
                reply_route.clone(),
            )
            .expect("fixture route must pass real ingress capability validation");
            let source = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: authenticated_via,
            };
            let daemon_source_credit = source_credits.acquire(&source).await;
            let (completion, outcome) = oneshot::channel();
            (
                PreparedSumeragiRelayWork {
                    source: source.clone(),
                    item: PreparedSumeragiRelayItem::Block(Box::new(inbound)),
                    reply_route,
                    retention_guard: SumeragiRelayRetention {
                        source,
                        geometry: source_credits.geometry(),
                        _p2p: p2p_guard,
                        _daemon_source_credit: daemon_source_credit,
                    },
                    completion: Some(completion),
                    retry_eligible_at: Instant::now(),
                },
                outcome,
                route,
            )
        }

        fn exact_retry(attempt: SumeragiRelayAttempt) -> PreparedSumeragiRelayWork {
            match attempt {
                SumeragiRelayAttempt::Retry(work) => work,
                SumeragiRelayAttempt::Terminal { .. } | SumeragiRelayAttempt::Fatal { .. } => {
                    panic!("full real ingress must return the exact retry owner")
                }
            }
        }

        fn finish_delivered_attempt(attempt: SumeragiRelayAttempt) {
            match attempt {
                SumeragiRelayAttempt::Terminal {
                    outcome,
                    retention_guard,
                    completion,
                } => {
                    assert_eq!(outcome, SumeragiRelayTerminalOutcome::Delivered);
                    finish_sumeragi_relay_terminal(outcome, retention_guard, completion);
                }
                SumeragiRelayAttempt::Retry(_) => {
                    panic!("open real ingress unexpectedly retained the selected owner for retry")
                }
                SumeragiRelayAttempt::Fatal { reason, .. } => {
                    panic!(
                        "open real ingress failed the selected owner: {}",
                        reason.label()
                    )
                }
            }
        }

        fn retry_next_exact_relay(
            retained: &mut FairRetainedQueue<SumeragiRelaySource, PreparedSumeragiRelayWork>,
            expected_source: &SumeragiRelaySource,
            handle: &SumeragiHandle,
            expected_route: &NetworkReplyRoute,
        ) {
            let selection = retained
                .pop_if_with_trace(|_| true)
                .expect("expected source owns the next outer service rank");
            assert_eq!(&selection.trace.source, expected_source);
            let FairRetainedSelection { trace, item } = selection;
            let retry = exact_retry(attempt_sumeragi_block_relay_work_for_test(handle, item));
            assert!(
                sumeragi_relay_retain_retry(retained, &trace, retry).is_ok(),
                "the exact retry must remain independently owned"
            );
            assert!(
                retained
                    .lanes
                    .get(expected_source)
                    .expect("source lane remains retained")
                    .iter()
                    .any(|work| work.reply_route.same_delivery(expected_route))
            );
        }

        fn finish_next_delivered_relay(
            retained: &mut FairRetainedQueue<SumeragiRelaySource, PreparedSumeragiRelayWork>,
            expected_source: &SumeragiRelaySource,
            handle: &SumeragiHandle,
            expected_route: Option<&NetworkReplyRoute>,
        ) {
            let selection = retained
                .pop_if_with_trace(|_| true)
                .expect("expected exact relay remains queued");
            assert_eq!(&selection.trace.source, expected_source);
            if let Some(expected_route) = expected_route {
                assert!(selection.item.reply_route.same_delivery(expected_route));
            }
            finish_delivered_attempt(attempt_sumeragi_block_relay_work_for_test(
                handle,
                selection.item,
            ));
        }

        fn fill_real_inner_ingress(handle: &SumeragiHandle, capacity: usize) {
            for offset in 0..capacity {
                let offset = u8::try_from(offset).expect("fixture offset fits u8");
                assert!(matches!(
                    handle.try_incoming_block_message_owned(InboundBlockMessage::new(
                        indexed_v2_vote_block_message(
                            40 + u64::from(offset),
                            0x40_u8.checked_add(offset).expect("fixture marker fits u8"),
                        ),
                        None,
                    )),
                    SumeragiIngressDisposition::Accepted
                ));
            }
        }

        fn drain_real_inner_ingress(
            harness: &iroha_core::sumeragi::SumeragiIngressTestHarness,
            count: usize,
        ) {
            for _ in 0..count {
                drop(
                    harness
                        .pop_block()
                        .expect("release one exact inner prefill owner"),
                );
            }
        }

        #[test]
        fn retained_sumeragi_geometry_is_checked() {
            let zero_geometry = SumeragiRelayCapacityGeometry {
                network_per_lane: 0,
                authenticated_source_count: 0,
                daemon_per_source: 0,
                class_capacity: 0,
                retained_capacity: 0,
            };
            assert!(!zero_geometry.daemon_source_capacity_matches_two_upstream_lanes());
            assert!(!zero_geometry.class_corridor_covers_authenticated_sources());
            assert_eq!(SumeragiRelayCapacityGeometry::checked(0, 1, 1), None);
            assert_eq!(SumeragiRelayCapacityGeometry::checked(1, 0, 1), None);
            assert_eq!(SumeragiRelayCapacityGeometry::checked(1, 1, 0), None);
            assert_eq!(
                SumeragiRelayCapacityGeometry::checked(1, 1, 4),
                Some(SumeragiRelayCapacityGeometry {
                    network_per_lane: 1,
                    authenticated_source_count: 4,
                    daemon_per_source: 2,
                    class_capacity: 8,
                    retained_capacity: 16,
                })
            );
            assert_eq!(
                SumeragiRelayCapacityGeometry::checked(usize::MAX, 1, 1),
                None
            );
            assert_eq!(
                SumeragiRelayCapacityGeometry::checked(1, usize::MAX, 1),
                None
            );
            assert_eq!(
                SumeragiRelayCapacityGeometry::checked(1, 1, usize::MAX),
                None
            );
            assert_eq!(sumeragi_relay_class_capacity(0), None);
            assert_eq!(sumeragi_relay_class_capacity(1), Some(1));
            assert_eq!(sumeragi_relay_dispatcher_capacity(1), Some(2));
            assert_eq!(sumeragi_relay_dispatcher_capacity(usize::MAX), None);

            let geometry = relay_geometry(1, 1, 4);
            let source = relay_source(SumeragiRelayClass::V2);
            assert!(sumeragi_rehydrated_ownership_matches(
                &source, geometry, &source, geometry
            ));
            let mut substituted_geometry = geometry;
            substituted_geometry.class_capacity += 1;
            assert!(!sumeragi_rehydrated_ownership_matches(
                &source,
                geometry,
                &source,
                substituted_geometry,
            ));
            assert!(!sumeragi_rehydrated_ownership_matches(
                &source,
                geometry,
                &relay_source(SumeragiRelayClass::V2),
                geometry,
            ));
        }

        #[test]
        fn retained_sumeragi_dispatcher_only_exits_after_drained_shutdown() {
            assert_eq!(
                sumeragi_relay_dispatcher_lifecycle(false, false, false, true),
                SumeragiRelayDispatcherLifecycle::UnexpectedClosure
            );
            assert_eq!(
                sumeragi_relay_dispatcher_lifecycle(true, false, false, true),
                SumeragiRelayDispatcherLifecycle::GracefulShutdown
            );
            assert_eq!(
                sumeragi_relay_dispatcher_lifecycle(true, true, false, true),
                SumeragiRelayDispatcherLifecycle::Running
            );
            assert_eq!(
                sumeragi_relay_dispatcher_lifecycle(true, false, false, false),
                SumeragiRelayDispatcherLifecycle::Running
            );
            assert_eq!(
                sumeragi_relay_failure_disposition(SumeragiRelayFatalReason::IngressClosed, false),
                SumeragiRelayFailureDisposition::FailStop
            );
            assert_eq!(
                sumeragi_relay_failure_disposition(SumeragiRelayFatalReason::IngressClosed, true),
                SumeragiRelayFailureDisposition::RetireForShutdown
            );
            assert_eq!(
                sumeragi_relay_failure_disposition(
                    SumeragiRelayFatalReason::BlockingTaskCancelled,
                    true
                ),
                SumeragiRelayFailureDisposition::RetireForShutdown
            );
            for reason in [
                SumeragiRelayFatalReason::FailStop,
                SumeragiRelayFatalReason::BlockingTaskPanicked,
                SumeragiRelayFatalReason::BlockingTaskFailed,
            ] {
                assert_eq!(
                    sumeragi_relay_failure_disposition(reason, true),
                    SumeragiRelayFailureDisposition::FailStop
                );
            }
        }

        #[test]
        fn retained_sumeragi_queue_rotates_across_sources_and_classes() {
            let v2_a = relay_source(SumeragiRelayClass::V2);
            let v2_b = relay_source(SumeragiRelayClass::V2);
            let lane_a = SumeragiRelaySource {
                class: SumeragiRelayClass::Lane,
                via: v2_a.via.clone(),
            };
            let lane_b = SumeragiRelaySource {
                class: SumeragiRelayClass::Lane,
                via: v2_b.via.clone(),
            };
            let mut queue = FairRetainedQueue::new(8, 4);

            assert!(queue.push(v2_a.clone(), 1).is_ok());
            assert!(queue.push(v2_a, 2).is_ok());
            assert!(queue.push(lane_a, 3).is_ok());
            assert!(queue.push(v2_b, 4).is_ok());
            assert!(queue.push(lane_b, 5).is_ok());

            assert_eq!(
                [
                    queue.pop(),
                    queue.pop(),
                    queue.pop(),
                    queue.pop(),
                    queue.pop(),
                ],
                [Some(1), Some(3), Some(4), Some(5), Some(2)]
            );
        }

        #[test]
        fn source_saturation_returns_exact_copy_and_preserves_responsive_reserve() {
            let saturated = relay_source(SumeragiRelayClass::V2);
            let responsive = relay_source(SumeragiRelayClass::V2);
            let mut queue = FairRetainedQueue::new(8, 4);

            for item in 0..4 {
                assert!(queue.push(saturated.clone(), item).is_ok());
            }
            let exact = queue
                .push(saturated.clone(), 4)
                .expect_err("primary source capacity is isolated")
                .into_item();
            assert_eq!(exact, 4, "source pressure must return the exact copy");
            assert!(
                queue.push(responsive, 99).is_ok(),
                "a saturated source must leave isolated admission for a responsive source"
            );

            let retrying = queue
                .pop()
                .expect("saturated source owns first service rank");
            assert_eq!(retrying, 0);
            assert!(queue.push(saturated, retrying).is_ok());
            assert_eq!(
                queue.pop(),
                Some(99),
                "responsive work must be serviced after one retrying source turn"
            );
        }

        #[test]
        fn retry_ineligible_source_cannot_block_responsive_service() {
            let stalled = relay_source(SumeragiRelayClass::V2);
            let responsive = relay_source(SumeragiRelayClass::V2);
            let now = Instant::now();
            let mut queue = FairRetainedQueue::new(8, 4);
            assert!(
                queue
                    .push(stalled, (1_u8, now + Duration::from_secs(60)))
                    .is_ok(),
                "enqueue retry-ineligible source"
            );
            assert!(
                queue.push(responsive, (2_u8, now)).is_ok(),
                "enqueue responsive source"
            );

            assert_eq!(
                queue.pop_if(|(_, eligible_at)| *eligible_at <= now),
                Some((2, now)),
                "one complete fair scan must bypass a retry-ineligible source"
            );
        }

        #[tokio::test]
        async fn daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner() {
            // Each authenticated source owns 16 credits in each of the safety
            // and shared-high upstream lanes. The daemon therefore reserves
            // 32, while its 128-slot class corridor covers all four sources.
            // The historical daemon-local cap of eight made A9 wait and
            // blocked B behind it on this same lane.
            let network_per_lane = 64_usize.div_ceil(4);
            let geometry = relay_geometry(64, network_per_lane, 4);
            assert_eq!(geometry.daemon_per_source, 32);
            assert_eq!(
                sumeragi_relay_source_capacity(geometry.class_capacity, geometry.daemon_per_source),
                Some(geometry.daemon_per_source)
            );
            let (v2, mut v2_rx) = mpsc::channel(network_per_lane + 2);
            let (lane, _lane_rx) = mpsc::channel(1);
            let ingress = SumeragiRelayIngress {
                v2,
                lane,
                source_credits: SumeragiRelaySourceCredits::new(geometry),
            };
            let (high_tx, high_rx) = mpsc::channel(network_per_lane + 2);
            let (work_high_tx, _work_high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(1);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            let (work_payload_tx, _work_payload_rx) = mpsc::channel(1);
            let (work_chunk_tx, _work_chunk_rx) = mpsc::channel(1);
            let (work_low_tx, _work_low_rx) = mpsc::channel(1);

            let sources = enqueue_daemon_credit_source_fixture(&high_tx, network_per_lane);
            drop(high_tx);

            let forward_ingress = ingress.clone();
            let forwarder = tokio::spawn(async move {
                drive_network_relay_ingress_inner(
                    high_rx,
                    payload_rx,
                    chunk_rx,
                    low_rx,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
                    Some(&forward_ingress),
                    false,
                )
                .await
            });
            let _open_sibling_inputs = (payload_tx, chunk_tx, low_tx);

            tokio::time::timeout(Duration::from_millis(250), async {
                while v2_rx.len() != 10 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("matched daemon geometry must pass same-lane A1 through A9 and then B");

            assert_eq!(ingress.source_credits.live_sources(), 2);
            let (responsive_rank, ninth_rank) = observe_daemon_credit_delivery_ranks(
                &mut v2_rx,
                &sources.authenticated_via,
                &sources.responsive_via,
                10,
            )
            .await;
            assert_eq!(
                responsive_rank,
                Some(10),
                "B must progress immediately after the preceding same-lane A9"
            );
            assert_eq!(
                ninth_rank,
                Some(9),
                "the exact A9 owner must not be reordered"
            );
            assert!(matches!(
                forwarder.await.expect("same-lane forwarder must not panic"),
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High)
            ));
            assert_eq!(sources.upstream.available_permits(), network_per_lane);
            assert_eq!(sources.responsive_upstream.available_permits(), 1);
        }

        #[tokio::test]
        async fn same_source_safety_and_shared_high_credits_cross_daemon_without_head_of_line_wait()
        {
            let geometry = relay_geometry(1, 1, 1);
            assert_eq!(geometry.daemon_per_source, 2);
            assert_eq!(geometry.class_capacity, 2);
            let (v2, mut v2_rx) = mpsc::channel(geometry.class_capacity);
            let (lane, _lane_rx) = mpsc::channel(1);
            let ingress = SumeragiRelayIngress {
                v2,
                lane,
                source_credits: SumeragiRelaySourceCredits::new(geometry),
            };
            let peer = sample_peer();
            let safety_upstream = Arc::new(Semaphore::new(1));
            let shared_high_upstream = Arc::new(Semaphore::new(1));
            let mut safety = RelayWorkItem::new(peer.clone(), v2_vote_msg(), 1);
            safety.retain_authenticated_source_credit(
                Arc::clone(&safety_upstream)
                    .try_acquire_owned()
                    .expect("safety source credit remains"),
            );
            let mut shared_high = RelayWorkItem::new(peer, v2_vote_msg(), 2);
            shared_high.retain_authenticated_source_credit(
                Arc::clone(&shared_high_upstream)
                    .try_acquire_owned()
                    .expect("shared-high source credit remains"),
            );

            let (high_tx, high_rx) = mpsc::channel(2);
            high_tx
                .try_send(safety)
                .expect("safety occurrence enters the shared daemon input");
            high_tx
                .try_send(shared_high)
                .expect("shared-high occurrence enters behind safety");
            drop(high_tx);
            let (work_tx, _work_rx) = mpsc::channel(1);
            let forward_ingress = ingress.clone();
            let forwarder = tokio::spawn(async move {
                forward_relay_lane(
                    high_rx,
                    &work_tx,
                    Some(&forward_ingress),
                    RelayReceiverKind::High,
                    false,
                )
                .await
            });

            tokio::time::timeout(Duration::from_millis(250), async {
                while v2_rx.len() != 2 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("both independent upstream lane owners must cross the daemon corridor");
            assert_eq!(safety_upstream.available_permits(), 0);
            assert_eq!(shared_high_upstream.available_permits(), 0);
            let first = v2_rx.recv().await.expect("safety owner remains exact");
            let second = v2_rx.recv().await.expect("shared-high owner remains exact");
            assert_eq!(
                (
                    first.work.work.payload_bytes,
                    second.work.work.payload_bytes
                ),
                (1, 2)
            );
            drop((first, second));
            assert_eq!(safety_upstream.available_permits(), 1);
            assert_eq!(shared_high_upstream.available_permits(), 1);
            assert!(matches!(
                forwarder
                    .await
                    .expect("same-source forwarder must not panic"),
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High)
            ));
        }

        #[tokio::test]
        async fn base_one_four_sources_reserve_both_upstream_lanes_without_head_of_line_wait() {
            let geometry = relay_geometry(1, 1, 4);
            assert_eq!(geometry.daemon_per_source, 2);
            assert_eq!(geometry.class_capacity, 8);
            assert_eq!(geometry.retained_capacity, 16);
            let (v2, mut v2_rx) = mpsc::channel(geometry.class_capacity);
            let (lane, _lane_rx) = mpsc::channel(1);
            let ingress = SumeragiRelayIngress {
                v2,
                lane,
                source_credits: SumeragiRelaySourceCredits::new(geometry),
            };
            let (high_tx, high_rx) = mpsc::channel(geometry.class_capacity);
            let mut sources = Vec::new();
            for source_index in 0..4_usize {
                let peer = sample_peer();
                let via = peer.id().clone();
                let safety_upstream = Arc::new(Semaphore::new(1));
                let shared_high_upstream = Arc::new(Semaphore::new(1));
                for (lane_index, upstream) in [
                    Arc::clone(&safety_upstream),
                    Arc::clone(&shared_high_upstream),
                ]
                .into_iter()
                .enumerate()
                {
                    let payload_bytes = source_index * 2 + lane_index + 1;
                    let mut work = RelayWorkItem::new(peer.clone(), v2_vote_msg(), payload_bytes);
                    work.retain_authenticated_source_credit(
                        upstream
                            .try_acquire_owned()
                            .expect("independent upstream source credit remains"),
                    );
                    high_tx
                        .try_send(work)
                        .expect("all eight exact owners fit the same high lane");
                }
                sources.push((via, safety_upstream, shared_high_upstream));
            }
            drop(high_tx);
            let (work_tx, _work_rx) = mpsc::channel(1);
            let forward_ingress = ingress.clone();
            let forwarder = tokio::spawn(async move {
                forward_relay_lane(
                    high_rx,
                    &work_tx,
                    Some(&forward_ingress),
                    RelayReceiverKind::High,
                    false,
                )
                .await
            });

            tokio::time::timeout(Duration::from_millis(250), async {
                while v2_rx.len() != geometry.class_capacity {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("aggregate class geometry must admit both lanes for all four sources");
            assert_eq!(ingress.source_credits.live_sources(), 4);
            for (_, safety, shared_high) in &sources {
                assert_eq!(safety.available_permits(), 0);
                assert_eq!(shared_high.available_permits(), 0);
            }

            let mut delivered_by_source = BTreeMap::<PeerId, usize>::new();
            while let Some(credited) = v2_rx.recv().await {
                *delivered_by_source
                    .entry(credited.work.work.authenticated_via().clone())
                    .or_default() += 1;
                drop(credited);
                if delivered_by_source.values().sum::<usize>() == geometry.class_capacity {
                    break;
                }
            }
            assert_eq!(delivered_by_source.len(), 4);
            assert!(delivered_by_source.values().all(|count| *count == 2));
            for (_, safety, shared_high) in &sources {
                assert_eq!(safety.available_permits(), 1);
                assert_eq!(shared_high.available_permits(), 1);
            }
            assert!(matches!(
                forwarder
                    .await
                    .expect("four-source forwarder must not panic"),
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High)
            ));
        }

        #[cfg(feature = "test-network-message-control")]
        #[tokio::test]
        async fn hold_release_same_source_reconnect_retires_old_delivery_without_rebinding_new_route()
         {
            let (_control_dir, controller) = crate::consensus_message_control::Controller::<
                NetworkReplyRoute,
                HeldSumeragiRelayOwnership,
            >::for_tests();
            let source_credits = SumeragiRelaySourceCredits::new(relay_geometry(1, 1, 1));

            controller.drain_subsequent_messages_for_tests();
            let peer = sample_peer();
            let mut fixture =
                NetworkReplyRouteTestFixture::with_source_capacity(peer.id().clone(), 4);
            let upstream = Arc::new(Semaphore::new(2));

            let old_route = fixture.mint(peer.id().clone());
            let mut old_relay = RelayWorkItem::new(peer.clone(), v2_vote_msg(), 101);
            old_relay
                .reattach_reply_route(old_route.clone())
                .expect("old fixture route reattaches to its exact occurrence");
            old_relay.retain_authenticated_source_credit(
                Arc::clone(&upstream)
                    .try_acquire_owned()
                    .expect("old upstream source credit remains"),
            );
            let (
                old_peer,
                old_authenticated_via,
                old_message,
                old_size_bytes,
                old_reply_route,
                old_p2p_guard,
            ) = old_relay.into_parts_with_reply_route();
            let source = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: old_authenticated_via.clone(),
            };
            let old_daemon_credit = source_credits
                .try_acquire(&source)
                .expect("old daemon source credit remains");
            let (old_completion, old_outcome) = oneshot::channel();
            assert!(matches!(
                prepare_sumeragi_relay_work_boundary(
                    Some(&controller),
                    false,
                    SumeragiRelayPreparationParts {
                        peer: old_peer,
                        authenticated_via: old_authenticated_via,
                        message: old_message,
                        size_bytes: old_size_bytes,
                        reply_route: old_reply_route,
                        ownership: HeldSumeragiRelayOwnership {
                            retention_guard: SumeragiRelayRetention {
                                source: source.clone(),
                                geometry: source_credits.geometry(),
                                _p2p: old_p2p_guard,
                                _daemon_source_credit: old_daemon_credit,
                            },
                            completion: Some(old_completion),
                        },
                    },
                ),
                SumeragiRelayPreparationBoundary::Held
            ));

            let reconnect_route = fixture.mint(peer.id().clone());
            assert!(old_route.same_source(&reconnect_route));
            assert!(!old_route.same_tenure(&reconnect_route));
            assert_eq!(
                reconnect_route.source_update_from(&old_route),
                Ok(iroha_p2p::network::NetworkReplyRouteSourceUpdate::Reconnected)
            );
            let mut reconnect_relay = RelayWorkItem::new(peer, v2_vote_msg(), 102);
            reconnect_relay
                .reattach_reply_route(reconnect_route.clone())
                .expect("reconnect fixture route reattaches to its exact occurrence");
            reconnect_relay.retain_authenticated_source_credit(
                Arc::clone(&upstream)
                    .try_acquire_owned()
                    .expect("reconnect upstream source credit remains"),
            );
            let (
                reconnect_peer,
                reconnect_authenticated_via,
                reconnect_message,
                reconnect_size_bytes,
                reconnect_reply_route,
                reconnect_p2p_guard,
            ) = reconnect_relay.into_parts_with_reply_route();
            let reconnect_source = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: reconnect_authenticated_via.clone(),
            };
            assert_eq!(reconnect_source, source);
            let reconnect_daemon_credit = source_credits
                .try_acquire(&reconnect_source)
                .expect("reconnect daemon source credit remains");
            let (reconnect_completion, reconnect_outcome) = oneshot::channel();
            assert!(matches!(
                prepare_sumeragi_relay_work_boundary(
                    Some(&controller),
                    false,
                    SumeragiRelayPreparationParts {
                        peer: reconnect_peer,
                        authenticated_via: reconnect_authenticated_via,
                        message: reconnect_message,
                        size_bytes: reconnect_size_bytes,
                        reply_route: reconnect_reply_route,
                        ownership: HeldSumeragiRelayOwnership {
                            retention_guard: SumeragiRelayRetention {
                                source: reconnect_source.clone(),
                                geometry: source_credits.geometry(),
                                _p2p: reconnect_p2p_guard,
                                _daemon_source_credit: reconnect_daemon_credit,
                            },
                            completion: Some(reconnect_completion),
                        },
                    },
                ),
                SumeragiRelayPreparationBoundary::Held
            ));
            assert_eq!(upstream.available_permits(), 0);
            assert_eq!(source_credits.available_permits(&source), 0);

            assert!(fixture.retire(&old_route));
            assert!(!old_route.is_active());
            assert!(reconnect_route.is_active());

            let held_old = controller
                .next_release()
                .expect("take old held release")
                .expect("old held occurrence is releasable");
            assert!(
                held_old
                    .reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&old_route))
            );
            assert!(
                !held_old
                    .reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_tenure(&reconnect_route))
            );
            let mut old_reattach_called = false;
            let (old_sequence, old_ownership) = match rehydrate_held_sumeragi_relay_work(
                held_old,
                |peer, message, size_bytes, route| {
                    old_reattach_called = true;
                    let mut released = RelayWorkItem::new(peer, message, size_bytes);
                    released.reattach_reply_route(route)?;
                    Ok(released)
                },
            ) {
                HeldSumeragiReentry::RetireStale {
                    sequence,
                    ownership,
                } => (sequence, ownership),
                HeldSumeragiReentry::Ready { .. } | HeldSumeragiReentry::Reject { .. } => {
                    panic!("the held old tenure must retire instead of using the reconnect route")
                }
            };
            assert!(
                !old_reattach_called,
                "stale held work must retire before any route reattachment"
            );
            controller
                .complete_release(
                    old_sequence,
                    consensus_message_control::ReleaseOutcome::Retired,
                )
                .expect("record old-tenure retirement before releasing its exact token");
            finish_sumeragi_relay_terminal(
                SumeragiRelayTerminalOutcome::Retired,
                old_ownership.retention_guard,
                old_ownership.completion,
            );
            assert_eq!(
                old_outcome.await.expect("old completion remains live"),
                SumeragiRelayTerminalOutcome::Retired
            );
            assert_eq!(upstream.available_permits(), 1);
            assert_eq!(source_credits.available_permits(&source), 1);

            let held_reconnect = controller
                .next_release()
                .expect("take reconnect held release")
                .expect("reconnect held occurrence is releasable");
            assert!(
                held_reconnect
                    .reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&reconnect_route))
            );
            assert!(
                !held_reconnect
                    .reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_tenure(&old_route))
            );
            let (reconnect_sequence, released, reconnect_ownership) =
                match rehydrate_held_sumeragi_relay_work(
                    held_reconnect,
                    |peer, message, size_bytes, route| {
                        let mut released = RelayWorkItem::new(peer, message, size_bytes);
                        released.reattach_reply_route(route)?;
                        Ok(released)
                    },
                ) {
                    HeldSumeragiReentry::Ready {
                        sequence,
                        class: SumeragiRelayClass::V2,
                        work,
                        ownership,
                    } => (sequence, work, ownership),
                    HeldSumeragiReentry::Ready { .. }
                    | HeldSumeragiReentry::RetireStale { .. }
                    | HeldSumeragiReentry::Reject { .. } => {
                        panic!("the live reconnect must rehydrate with its own exact route")
                    }
                };
            assert!(
                released
                    .reply_route()
                    .is_some_and(|route| route.same_delivery(&reconnect_route))
            );
            assert_eq!(released.authenticated_via(), &source.via);
            drop(released);
            controller
                .complete_release(
                    reconnect_sequence,
                    consensus_message_control::ReleaseOutcome::Delivered,
                )
                .expect("record reconnect delivery before releasing its exact token");
            finish_sumeragi_relay_terminal(
                SumeragiRelayTerminalOutcome::Delivered,
                reconnect_ownership.retention_guard,
                reconnect_ownership.completion,
            );
            assert_eq!(
                reconnect_outcome
                    .await
                    .expect("reconnect completion remains live"),
                SumeragiRelayTerminalOutcome::Delivered
            );
            assert_eq!(upstream.available_permits(), 2);
            assert_eq!(source_credits.available_permits(&source), 2);
            assert!(
                controller
                    .next_release()
                    .expect("completed drain remains healthy")
                    .is_none()
            );
        }

        #[cfg(feature = "test-network-message-control")]
        #[tokio::test]
        async fn hold_release_preserves_exact_layered_ownership_until_recorded_terminal() {
            let (_control_dir, controller) = crate::consensus_message_control::Controller::<
                NetworkReplyRoute,
                HeldSumeragiRelayOwnership,
            >::for_tests();
            let source_credits = SumeragiRelaySourceCredits::new(relay_geometry(1, 1, 1));

            controller.drain_subsequent_messages_for_tests();
            let peer = sample_peer();
            let mut fixture =
                NetworkReplyRouteTestFixture::with_source_capacity(peer.id().clone(), 4);
            let route = fixture.mint(peer.id().clone());
            let upstream = Arc::new(Semaphore::new(1));
            let mut relay = RelayWorkItem::new(peer, v2_vote_msg(), 101);
            relay
                .reattach_reply_route(route)
                .expect("fixture route reattaches to its exact occurrence");
            relay.retain_authenticated_source_credit(
                Arc::clone(&upstream)
                    .try_acquire_owned()
                    .expect("one upstream credit remains"),
            );
            let (peer, authenticated_via, message, size_bytes, reply_route, p2p_guard) =
                relay.into_parts_with_reply_route();
            let source = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: authenticated_via.clone(),
            };
            let daemon_credit = source_credits
                .try_acquire(&source)
                .expect("one matched daemon credit remains");
            let (completion, mut outcome) = oneshot::channel();
            assert!(matches!(
                prepare_sumeragi_relay_work_boundary(
                    Some(&controller),
                    false,
                    SumeragiRelayPreparationParts {
                        peer,
                        authenticated_via,
                        message,
                        size_bytes,
                        reply_route,
                        ownership: HeldSumeragiRelayOwnership {
                            retention_guard: SumeragiRelayRetention {
                                source: source.clone(),
                                geometry: source_credits.geometry(),
                                _p2p: p2p_guard,
                                _daemon_source_credit: daemon_credit,
                            },
                            completion: Some(completion),
                        },
                    },
                ),
                SumeragiRelayPreparationBoundary::Held
            ));
            assert_eq!(upstream.available_permits(), 0);
            assert_eq!(source_credits.available_permits(&source), 1);
            assert!(matches!(
                outcome.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ));

            let held = controller
                .next_release()
                .expect("take held release")
                .expect("held occurrence is releasable");
            let (sequence, released, ownership) = match rehydrate_held_sumeragi_relay_work(
                held,
                |peer, message, size_bytes, route| {
                    let mut released = RelayWorkItem::new(peer, message, size_bytes);
                    released.reattach_reply_route(route)?;
                    Ok(released)
                },
            ) {
                HeldSumeragiReentry::Ready {
                    sequence,
                    class: SumeragiRelayClass::V2,
                    work,
                    ownership,
                } => (sequence, work, ownership),
                HeldSumeragiReentry::Ready { .. }
                | HeldSumeragiReentry::RetireStale { .. }
                | HeldSumeragiReentry::Reject { .. } => {
                    panic!("live held occurrence must rehydrate with exact ownership")
                }
            };
            assert_eq!(upstream.available_permits(), 0);
            assert_eq!(source_credits.available_permits(&source), 1);

            let (v2, mut v2_rx) = mpsc::channel(2);
            let (lane, _lane_rx) = mpsc::channel(2);
            let ingress = SumeragiRelayIngress {
                v2,
                lane,
                source_credits: source_credits.clone(),
            };
            let HeldSumeragiRelayOwnership {
                retention_guard,
                completion,
            } = ownership;
            assert!(
                ingress
                    .send_rehydrated(
                        SumeragiRelayClass::V2,
                        SumeragiRelayWorkItem::live(released),
                        retention_guard,
                    )
                    .await
                    .is_ok(),
                "rehydration reuses the held daemon owner without reacquiring"
            );
            let credited = v2_rx.recv().await.expect("rehydrated item remains exact");
            let SumeragiRelayIngressOwnership::Rehydrated(retention_guard) = credited.ownership
            else {
                panic!("held work must retain its original layered token");
            };
            assert_eq!(upstream.available_permits(), 0);
            assert_eq!(source_credits.available_permits(&source), 1);
            controller
                .complete_release(
                    sequence,
                    consensus_message_control::ReleaseOutcome::Delivered,
                )
                .expect("record delivery before releasing the exact token");
            finish_sumeragi_relay_terminal(
                SumeragiRelayTerminalOutcome::Delivered,
                retention_guard,
                completion,
            );
            assert_eq!(
                outcome.await.expect("original completion remains live"),
                SumeragiRelayTerminalOutcome::Delivered
            );
            assert_eq!(upstream.available_permits(), 1);
            assert_eq!(source_credits.available_permits(&source), 2);

            controller.drain_subsequent_messages_for_tests();
            let peer = sample_peer();
            let mut fixture =
                NetworkReplyRouteTestFixture::with_source_capacity(peer.id().clone(), 4);
            let route = fixture.mint(peer.id().clone());
            let cancel_while_held = route.clone();
            let upstream = Arc::new(Semaphore::new(1));
            let mut relay = RelayWorkItem::new(peer, v2_vote_msg(), 102);
            relay
                .reattach_reply_route(route)
                .expect("second fixture route reattaches");
            relay.retain_authenticated_source_credit(
                Arc::clone(&upstream)
                    .try_acquire_owned()
                    .expect("second upstream credit remains"),
            );
            let (peer, authenticated_via, message, size_bytes, reply_route, p2p_guard) =
                relay.into_parts_with_reply_route();
            let source = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: authenticated_via.clone(),
            };
            let daemon_credit = source_credits
                .try_acquire(&source)
                .expect("second daemon credit remains");
            let (completion, outcome) = oneshot::channel();
            assert!(matches!(
                prepare_sumeragi_relay_work_boundary(
                    Some(&controller),
                    false,
                    SumeragiRelayPreparationParts {
                        peer,
                        authenticated_via,
                        message,
                        size_bytes,
                        reply_route,
                        ownership: HeldSumeragiRelayOwnership {
                            retention_guard: SumeragiRelayRetention {
                                source: source.clone(),
                                geometry: source_credits.geometry(),
                                _p2p: p2p_guard,
                                _daemon_source_credit: daemon_credit,
                            },
                            completion: Some(completion),
                        },
                    },
                ),
                SumeragiRelayPreparationBoundary::Held
            ));
            assert!(fixture.retire(&cancel_while_held));
            let held = controller
                .next_release()
                .expect("take canceled release")
                .expect("canceled occurrence remains explicitly releasable");
            let (sequence, ownership) = match rehydrate_held_sumeragi_relay_work(
                held,
                |peer, message, size_bytes, route| {
                    let mut released = RelayWorkItem::new(peer, message, size_bytes);
                    released.reattach_reply_route(route)?;
                    Ok(released)
                },
            ) {
                HeldSumeragiReentry::RetireStale {
                    sequence,
                    ownership,
                } => (sequence, ownership),
                HeldSumeragiReentry::Ready { .. } | HeldSumeragiReentry::Reject { .. } => {
                    panic!("canceled held route must retire with its exact token")
                }
            };
            assert_eq!(upstream.available_permits(), 0);
            assert_eq!(source_credits.available_permits(&source), 1);
            controller
                .complete_release(sequence, consensus_message_control::ReleaseOutcome::Retired)
                .expect("record retirement before releasing the exact token");
            finish_sumeragi_relay_terminal(
                SumeragiRelayTerminalOutcome::Retired,
                ownership.retention_guard,
                ownership.completion,
            );
            assert_eq!(
                outcome.await.expect("retired completion remains live"),
                SumeragiRelayTerminalOutcome::Retired
            );
            assert_eq!(upstream.available_permits(), 1);
            assert_eq!(source_credits.available_permits(&source), 2);
        }

        #[tokio::test]
        async fn saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits() {
            let (v2_tx, mut v2_rx) = mpsc::channel(1);
            let (lane_tx, _lane_rx) = mpsc::channel(1);
            let sumeragi_ingress = SumeragiRelayIngress {
                v2: v2_tx,
                lane: lane_tx,
                source_credits: SumeragiRelaySourceCredits::new(relay_geometry(1, 1, 1)),
            };
            let peer = sample_peer();
            assert!(
                sumeragi_ingress
                    .send(
                        SumeragiRelayClass::V2,
                        SumeragiRelayWorkItem::live(RelayWorkItem::new(
                            peer.clone(),
                            v2_vote_msg(),
                            11,
                        )),
                    )
                    .await
                    .is_ok(),
                "prefill retained v2 ingress"
            );

            let (high_tx, high_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            let (work_high_tx, _work_high_rx) = mpsc::channel(1);
            let (work_low_tx, mut work_low_rx) = mpsc::channel(1);
            high_tx
                .try_send(RelayWorkItem::new(peer.clone(), v2_vote_msg(), 22))
                .expect("enqueue v2 item behind retained saturation");
            low_tx
                .try_send(RelayWorkItem::new(
                    peer,
                    iroha_core::NetworkMessage::Health,
                    33,
                ))
                .expect("enqueue ordinary relay work");

            let high_ingress = sumeragi_ingress.clone();
            let high_forwarder = tokio::spawn(async move {
                forward_relay_lane(
                    high_rx,
                    &work_high_tx,
                    Some(&high_ingress),
                    RelayReceiverKind::High,
                    false,
                )
                .await
            });
            let low_ingress = sumeragi_ingress.clone();
            let low_forwarder = tokio::spawn(async move {
                forward_relay_lane(
                    low_rx,
                    &work_low_tx,
                    Some(&low_ingress),
                    RelayReceiverKind::Low,
                    false,
                )
                .await
            });

            let ordinary = tokio::time::timeout(Duration::from_millis(100), work_low_rx.recv())
                .await
                .expect("ordinary relay lane must remain schedulable")
                .expect("ordinary worker queue remains open");
            assert_eq!(ordinary.payload_bytes, 33);
            let worker_permits = Arc::new(tokio::sync::Semaphore::new(2));
            let _all_permits = worker_permits
                .try_acquire_many_owned(2)
                .expect("retained Sumeragi send must not consume normal worker permits");

            let first = v2_rx.recv().await.expect("prefilled retained item");
            assert_eq!(first.work.work.payload_bytes, 11);
            drop(first);
            let second = tokio::time::timeout(Duration::from_millis(100), v2_rx.recv())
                .await
                .expect("released daemon credit must unblock the exact high-lane owner")
                .expect("retained ingress remains open");
            assert_eq!(second.work.work.payload_bytes, 22);
            drop(second);

            drop((high_tx, low_tx));
            assert!(matches!(
                high_forwarder.await.expect("high forwarder must not panic"),
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High)
            ));
            assert!(matches!(
                low_forwarder.await.expect("low forwarder must not panic"),
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Low)
            ));
        }

        #[tokio::test]
        async fn real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank() {
            const INNER_CAPACITY: usize = 3;
            let harness = iroha_core::sumeragi::SumeragiIngressTestHarness::new(INNER_CAPACITY);
            let handle = harness.handle();
            fill_real_inner_ingress(&handle, INNER_CAPACITY);

            let peer_a = sample_peer();
            let peer_b = sample_peer();
            let source_a = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: peer_a.id().clone(),
            };
            let source_b = SumeragiRelaySource {
                class: SumeragiRelayClass::V2,
                via: peer_b.id().clone(),
            };
            let source_credits = SumeragiRelaySourceCredits::new(relay_geometry(2, 2, 1));
            let mut fixture_a = NetworkReplyRouteTestFixture::new(peer_a.id().clone());
            let mut fixture_b = NetworkReplyRouteTestFixture::new(peer_b.id().clone());
            let (a1, mut a1_outcome, a1_route) = prepared_v2_relay_work(
                peer_a.clone(),
                indexed_v2_vote_block_message(41, 0x41),
                &mut fixture_a,
                &source_credits,
                41,
            )
            .await;
            let (a2, a2_outcome, a2_route) = prepared_v2_relay_work(
                peer_a,
                indexed_v2_vote_block_message(42, 0x42),
                &mut fixture_a,
                &source_credits,
                42,
            )
            .await;
            let (b, b_outcome, b_route) = prepared_v2_relay_work(
                peer_b,
                indexed_v2_vote_block_message(43, 0x43),
                &mut fixture_b,
                &source_credits,
                43,
            )
            .await;

            let mut retained = FairRetainedQueue::new(8, 4);
            assert!(retained.push(source_a.clone(), a1).is_ok(), "retain A1");
            assert!(retained.push(source_a.clone(), a2).is_ok(), "retain A2");
            assert!(retained.push(source_b.clone(), b).is_ok(), "retain B");

            retry_next_exact_relay(&mut retained, &source_a, &handle, &a1_route);
            retry_next_exact_relay(&mut retained, &source_b, &handle, &b_route);
            assert!(matches!(
                a1_outcome.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            ));

            drain_real_inner_ingress(&harness, INNER_CAPACITY);

            finish_next_delivered_relay(&mut retained, &source_a, &handle, Some(&a2_route));
            drop(harness.pop_block().expect("release accepted A2"));

            finish_next_delivered_relay(&mut retained, &source_b, &handle, Some(&b_route));
            drop(harness.pop_block().expect("release accepted B"));

            finish_next_delivered_relay(&mut retained, &source_a, &handle, Some(&a1_route));
            drop(harness.pop_block().expect("release accepted A1"));

            assert_eq!(
                a2_outcome.await.expect("A2 completion remains connected"),
                SumeragiRelayTerminalOutcome::Delivered
            );
            assert_eq!(
                b_outcome.await.expect("B completion remains connected"),
                SumeragiRelayTerminalOutcome::Delivered
            );
            assert_eq!(
                a1_outcome.await.expect("A1 completion remains connected"),
                SumeragiRelayTerminalOutcome::Delivered
            );
            assert!(retained.is_empty());
        }

        #[tokio::test]
        async fn worker_scheduler_bounds_every_ordinary_lane_under_continuous_high_backlog() {
            let ordinary_lane_count = RELAY_ORDINARY_RECEIVERS.len();
            let high_messages = RELAY_HIGH_BURST * ordinary_lane_count;
            let service_bound = (RELAY_HIGH_BURST + 1) * ordinary_lane_count;
            let (high_tx, high_rx) = mpsc::channel(high_messages);
            let (payload_tx, payload_rx) = mpsc::channel(1);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);

            for id in 0..high_messages {
                high_tx
                    .try_send(u16::try_from(id).expect("test message id fits in u16"))
                    .expect("high-priority backlog fits its queue");
            }
            payload_tx.try_send(10_001).expect("enqueue payload work");
            chunk_tx.try_send(10_002).expect("enqueue chunk work");
            low_tx.try_send(10_003).expect("enqueue low work");

            let mut receivers =
                FairRelayReceivers::new(high_rx, payload_rx, chunk_rx, low_rx, RELAY_HIGH_BURST);
            let mut payload_rank = None;
            let mut chunk_rank = None;
            let mut low_rank = None;
            let mut high_served = 0;

            for rank in 1..=service_bound {
                let (kind, _message) = receivers.recv().await.expect("queued relay work");
                match kind {
                    RelayReceiverKind::High => high_served += 1,
                    RelayReceiverKind::Payload => {
                        payload_rank.get_or_insert(rank);
                    }
                    RelayReceiverKind::Chunk => {
                        chunk_rank.get_or_insert(rank);
                    }
                    RelayReceiverKind::Low => {
                        low_rank.get_or_insert(rank);
                    }
                }
            }

            assert_eq!(payload_rank, Some(RELAY_HIGH_BURST + 1));
            assert_eq!(chunk_rank, Some(2 * (RELAY_HIGH_BURST + 1)));
            assert_eq!(low_rank, Some(service_bound));
            assert_eq!(high_served, high_messages);

            // Keep all senders alive through the assertions: the bound above is caused by the
            // scheduler, not by high-lane closure or exhaustion detection.
            drop((high_tx, payload_tx, chunk_tx, low_tx));
        }

        #[tokio::test]
        async fn shared_scheduler_bounds_chunk_and_low_under_continuous_payload_backlog() {
            let (_high_tx, high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(64);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            for id in 0..64 {
                payload_tx.try_send(id).expect("enqueue payload work");
            }
            chunk_tx.try_send(64).expect("enqueue chunk work");
            low_tx.try_send(65).expect("enqueue low work");
            let mut receivers =
                FairRelayReceivers::new(high_rx, payload_rx, chunk_rx, low_rx, RELAY_HIGH_BURST);

            let mut order = Vec::new();
            for _ in 0..4 {
                let (kind, _message) = receivers.recv().await.expect("queued relay work");
                order.push(kind);
            }

            assert_eq!(
                order,
                [
                    RelayReceiverKind::Payload,
                    RelayReceiverKind::Chunk,
                    RelayReceiverKind::Low,
                    RelayReceiverKind::Payload,
                ]
            );
        }

        #[tokio::test]
        async fn worker_scheduler_drains_closed_lanes_and_stops_only_when_all_are_closed() {
            let (high_tx, high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(1);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            high_tx.try_send(1).expect("enqueue high work");
            chunk_tx.try_send(2).expect("enqueue chunk work");
            drop((high_tx, payload_tx, chunk_tx, low_tx));
            let mut receivers = FairRelayReceivers::new(high_rx, payload_rx, chunk_rx, low_rx, 1);

            assert_eq!(receivers.recv().await, Some((RelayReceiverKind::High, 1)));
            assert_eq!(receivers.recv().await, Some((RelayReceiverKind::Chunk, 2)));
            assert_eq!(receivers.recv().await, None);
        }

        #[tokio::test]
        async fn relay_ingress_forwards_chunk_and_low_with_continuous_payload_backlog() {
            let (high_tx, high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(64);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            let (work_high_tx, _work_high_rx) = mpsc::channel(1);
            let (work_payload_tx, _work_payload_rx) = mpsc::channel(1);
            let (work_chunk_tx, mut work_chunk_rx) = mpsc::channel(1);
            let (work_low_tx, mut work_low_rx) = mpsc::channel(1);
            let keypair = KeyPair::random();
            let peer = Peer::new(
                "127.0.0.1:0".parse().expect("socket address"),
                keypair.public_key().clone(),
            );
            let work = || RelayWorkItem::new(peer.clone(), iroha_core::NetworkMessage::Health, 0);
            for _ in 0..64 {
                payload_tx
                    .try_send(work())
                    .expect("payload backlog fits its ingress queue");
            }
            chunk_tx.try_send(work()).expect("enqueue chunk work");
            low_tx.try_send(work()).expect("enqueue low work");

            let ingress = tokio::spawn(async move {
                drive_network_relay_ingress(
                    high_rx,
                    payload_rx,
                    chunk_rx,
                    low_rx,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
                )
                .await
            });

            tokio::time::timeout(Duration::from_millis(100), work_chunk_rx.recv())
                .await
                .expect("chunk lane must reach its worker queue within the service bound")
                .expect("chunk worker queue remains open");
            tokio::time::timeout(Duration::from_millis(100), work_low_rx.recv())
                .await
                .expect("low lane must reach its worker queue within the service bound")
                .expect("low worker queue remains open");

            drop(high_tx);
            let exit = tokio::time::timeout(Duration::from_millis(100), ingress)
                .await
                .expect("closing one ingress lane must stop the ingress loop")
                .expect("ingress task must not panic");
            assert_eq!(
                exit,
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High)
            );

            drop((payload_tx, chunk_tx, low_tx));
        }

        #[tokio::test]
        async fn relay_ingress_retains_exact_item_while_worker_lane_is_full() {
            let (high_tx, high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(1);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            let (work_high_tx, _work_high_rx) = mpsc::channel(1);
            let (work_payload_tx, mut work_payload_rx) = mpsc::channel(1);
            let (work_chunk_tx, _work_chunk_rx) = mpsc::channel(1);
            let (work_low_tx, _work_low_rx) = mpsc::channel(1);
            let keypair = KeyPair::random();
            let peer = Peer::new(
                "127.0.0.1:0".parse().expect("socket address"),
                keypair.public_key().clone(),
            );

            work_payload_tx
                .try_send(RelayWorkItem::new(
                    peer.clone(),
                    iroha_core::NetworkMessage::Health,
                    1,
                ))
                .expect("prefill payload worker queue");
            payload_tx
                .try_send(RelayWorkItem::new(
                    peer,
                    iroha_core::NetworkMessage::Health,
                    99,
                ))
                .expect("enqueue retained payload item");

            let ingress = tokio::spawn(async move {
                drive_network_relay_ingress(
                    high_rx,
                    payload_rx,
                    chunk_rx,
                    low_rx,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
                )
                .await
            });

            let first = work_payload_rx
                .recv()
                .await
                .expect("prefilled item remains");
            assert_eq!(first.payload_bytes, 1);
            let retained = tokio::time::timeout(Duration::from_millis(100), work_payload_rx.recv())
                .await
                .expect("retained item must be admitted after capacity opens")
                .expect("payload worker queue remains open");
            assert_eq!(retained.payload_bytes, 99);

            drop(high_tx);
            let exit = tokio::time::timeout(Duration::from_millis(100), ingress)
                .await
                .expect("closing one ingress lane must stop the ingress loop")
                .expect("ingress task must not panic");
            assert_eq!(
                exit,
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High)
            );
            drop((payload_tx, chunk_tx, low_tx));
        }

        #[tokio::test]
        async fn relay_ingress_requests_restart_when_receiver_closes() {
            let (_high_tx, high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(1);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            let (work_high_tx, _work_high_rx) = mpsc::channel(1);
            let (work_payload_tx, _work_payload_rx) = mpsc::channel(1);
            let (work_chunk_tx, _work_chunk_rx) = mpsc::channel(1);
            let (work_low_tx, _work_low_rx) = mpsc::channel(1);

            drop(payload_tx);
            let exit = tokio::time::timeout(
                Duration::from_millis(100),
                drive_network_relay_ingress(
                    high_rx,
                    payload_rx,
                    chunk_rx,
                    low_rx,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
                ),
            )
            .await
            .expect("relay ingress should notice closed receiver");

            assert_eq!(
                exit,
                RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Payload)
            );

            drop(chunk_tx);
            drop(low_tx);
        }

        #[tokio::test]
        async fn relay_ingress_requests_restart_when_worker_queue_closes() {
            let (_high_tx, high_rx) = mpsc::channel(1);
            let (payload_tx, payload_rx) = mpsc::channel(1);
            let (chunk_tx, chunk_rx) = mpsc::channel(1);
            let (low_tx, low_rx) = mpsc::channel(1);
            let (work_high_tx, _work_high_rx) = mpsc::channel(1);
            let (work_payload_tx, work_payload_rx) = mpsc::channel(1);
            let (work_chunk_tx, _work_chunk_rx) = mpsc::channel(1);
            let (work_low_tx, _work_low_rx) = mpsc::channel(1);

            drop(work_payload_rx);
            let keypair = KeyPair::random();
            payload_tx
                .try_send(RelayWorkItem::new(
                    Peer::new(
                        "127.0.0.1:0".parse().expect("socket address"),
                        keypair.public_key().clone(),
                    ),
                    iroha_core::NetworkMessage::Health,
                    0,
                ))
                .expect("enqueue payload message");

            let exit = tokio::time::timeout(
                Duration::from_millis(100),
                drive_network_relay_ingress(
                    high_rx,
                    payload_rx,
                    chunk_rx,
                    low_rx,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
                ),
            )
            .await
            .expect("relay ingress should notice closed worker queue");

            assert_eq!(
                exit,
                RelayIngressLoopExit::WorkerClosed(RelayReceiverKind::Payload)
            );

            drop(chunk_tx);
            drop(low_tx);
        }
    }

    #[cfg(feature = "telemetry")]
    mod metrics_bootstrap {
        #[allow(unused_imports)]
        use super::*;
        use serial_test::serial;
        use std::sync::Arc;

        #[test]
        #[serial]
        fn init_global_metrics_handle_is_idempotent() {
            let first = super::init_global_metrics_handle(false);
            let second = super::init_global_metrics_handle(false);
            assert!(Arc::ptr_eq(&first, &second));
        }
    }

    mod cli_args {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn whitespace_only_arguments_are_ignored() {
            let parsed = parse_args_from(vec![
                OsString::from("iroha3d"),
                OsString::from(" "),
                OsString::from("--trace-config"),
            ]);

            assert!(parsed.startup.trace_config);
        }

        #[test]
        fn surrounding_whitespace_is_trimmed() {
            let parsed = parse_args_from(vec![
                OsString::from("iroha3d"),
                OsString::from("   --trace-config  "),
            ]);

            assert!(parsed.startup.trace_config);
        }

        #[test]
        fn meaningful_arguments_are_preserved() {
            let parsed = parse_args_from(vec![
                OsString::from("iroha3d"),
                OsString::from("--config"),
                OsString::from("config.toml"),
            ]);

            assert_eq!(
                parsed.config,
                Some(PathBuf::from("config.toml")),
                "config argument should remain untouched"
            );
        }
    }

    mod manifest_crypto_checks {
        use std::sync::Arc;

        use super::*;
        use iroha_config::base::toml::TomlSource;
        use iroha_core::{kura::Kura, query::store::LiveQueryStore};
        use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry, ManifestCrypto};

        fn sample_manifest() -> RawGenesisTransaction {
            GenesisBuilder::new_without_executor(ChainId::from("test-chain"), PathBuf::from("."))
                .build_raw()
        }

        fn sample_config_table() -> toml::Table {
            toml::toml! {
                chain = "00000000-0000-0000-0000-000000000000"
                public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
                trusted_peers_pop = [
                  { public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }
                ]

                [network]
                address = "addr:127.0.0.1:1337#8F78"
                public_address = "addr:127.0.0.1:1337#8F78"

                [genesis]
                public_key = "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4"
                file = "./genesis.signed.nrt"
                expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

                [streaming]
                identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
                identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"

                [torii]
                address = "addr:127.0.0.1:8080#8942"

                [logger]
                format = "pretty"
            }
        }

        fn sample_config() -> Config {
            ConfigReader::new()
                .with_toml_source(TomlSource::inline(sample_config_table()))
                .read_and_complete::<UserConfig>()
                .expect("sample config should be readable")
                .parse()
                .expect("sample config should parse")
        }

        fn configure_exact_moderation_strict_ingress(config: &mut Config) {
            let qualification = iroha_torii::sorafs::moderation_runtime::
                torii_moderation_strict_ingress_qualification_v1();
            let authority = AccountId::new(config.common.key_pair.public_key().clone());
            config.torii.sorafs_storage.moderation_orchestrator = Some(
                iroha_config::parameters::actual::SorafsModerationOrchestrator {
                    checkpoint_path: "/var/lib/iroha/sorafs/moderation.to".into(),
                    checkpoint_store_handle: "sealed:moderation:checkpoint-primary".into(),
                    checkpoint_store_revision: 1,
                    checkpoint_store_policy_digest: [0x81; 32],
                    checkpoint_store_attestation_public_key: [
                        0x3d, 0x40, 0x17, 0xc3, 0xe8, 0x43, 0x89, 0x5a, 0x92, 0xb7, 0x0a,
                        0xa7, 0x4d, 0x1b, 0x7e, 0xbc, 0x9c, 0x98, 0x2c, 0xcf, 0x2e, 0xc4,
                        0x96, 0x8c, 0xc0, 0xcd, 0x55, 0xf1, 0x2a, 0xf4, 0x66, 0x0c,
                    ],
                    maintenance_authority: authority,
                    transaction_signer_handle: "hsm:moderation:signer-primary".into(),
                    transaction_signer_revision: 1,
                    transaction_signer_policy_digest: [0x82; 32],
                    strict_ingress_handle: iroha_torii::sorafs::moderation_runtime::
                        TORII_MODERATION_STRICT_INGRESS_HANDLE_V1.into(),
                    strict_ingress_revision: qualification.revision(),
                    strict_ingress_policy_digest: qualification.policy_digest(),
                    settlement_handoff_handle: "queue:moderation:settlement-primary".into(),
                    settlement_handoff_revision: 1,
                    settlement_handoff_policy_digest: [0x83; 32],
                    publication_handoff_handle: "dag:moderation:publication-primary".into(),
                    publication_handoff_revision: 1,
                    publication_handoff_policy_digest: [0x84; 32],
                    panel_notification_handle: "queue:moderation:notification-primary".into(),
                    panel_notification_revision: 1,
                    panel_notification_policy_digest: [0x85; 32],
                    panel_notification_archive_handle:
                        "object-lock:moderation:notification-receipts-primary".into(),
                    panel_notification_archive_revision: 1,
                    panel_notification_archive_policy_digest: [0x86; 32],
                    panel_notification_archive_id: [0x87; 32],
                    panel_notification_archive_bootstrap_public_key: [
                        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe,
                        0xd3, 0xc9, 0x64, 0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6,
                        0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
                    ],
                    panel_notification_archive_public_key: [
                        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe,
                        0xd3, 0xc9, 0x64, 0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6,
                        0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
                    ],
                    panel_notification_archive_predecessor_revocation_generation: None,
                    panel_notification_archive_predecessor_authorization_signature: None,
                    panel_notification_archive_new_key_possession_signature: None,
                    max_cases: 8,
                    max_events: 16,
                    max_outbox_entries: 8,
                    max_idempotency_records: 16,
                    max_handoffs: 8,
                    max_submit_attempts: 2,
                    checkpoint_max_bytes: iroha_config_base::util::Bytes(1024 * 1024),
                    panel_notification_archive_max_bytes: iroha_config_base::util::Bytes(
                        5 * 1024 * 1024,
                    ),
                    worker_interval: std::time::Duration::from_secs(1),
                    maintenance_batch_limit: 4,
                },
            );
        }

        fn genesis_staging_state_for_test(
            config: &Config,
            genesis: &GenesisBlock,
        ) -> (State, Arc<Kura>) {
            let authority = AccountId::new(config.genesis.public_key.clone());
            let mut world = World::with(
                [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority)],
                [Account::new(authority.clone()).build(&authority)],
                [],
            );
            iroha_core::sns::seed_genesis_alias_bootstrap(
                &mut world,
                &genesis.0,
                &config.nexus.dataspace_catalog,
            );
            let kura = Kura::blank_kura_for_testing();
            let mut state = State::new_with_chain_for_testing(
                world,
                Arc::clone(&kura),
                LiveQueryStore::start_test(),
                config.common.chain.clone(),
            );
            state.set_pipeline(config.pipeline.clone());
            state.set_oracle(config.oracle.clone());
            state.set_fraud_monitoring(config.fraud_monitoring.clone());
            state.set_gov(config.gov.clone());
            state.content = config.content.clone();
            state.set_settlement(config.settlement.clone());
            state.set_kagemusha_release_catalog(
                iroha_core::smartcontracts::isi::offline::KagemushaReleaseCatalogV4::from_offline_config(
                    &config.settlement.offline,
                )
                .expect("test Kagemusha release policy must be valid"),
            );
            state
                .set_zk(config.zk.clone())
                .expect("test ZK config must be valid");
            state
                .set_nexus(config.nexus.clone())
                .expect("test Nexus config must be valid");
            state.set_crypto(config.crypto.clone());
            let nexus = state.nexus_snapshot();
            let lane_manifests = Arc::new(
                LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
            );
            state.install_lane_manifests(&lane_manifests);
            (state, kura)
        }

        fn staged_context_hashes_for_test(
            genesis: &RawGenesisTransaction,
            genesis_authority: &KeyPair,
            config: &Config,
        ) -> (Hash, Hash) {
            let provisional = genesis
                .clone()
                .with_consensus_meta()
                .build_and_sign(genesis_authority)
                .expect("sign provisional genesis fixture");
            let authority = AccountId::new(genesis_authority.public_key().clone());
            let voters = iroha_core::sumeragi::signed_genesis_voting_peers(&provisional)
                .expect("provisional fixture voting roster");
            let topology = Topology::new(voters);
            let (mode, _) =
                signed_v2_genesis_context_metadata(&provisional).expect("signed v2 metadata");
            let (state, _kura) = genesis_staging_state_for_test(config, &provisional);
            let mut voting_block = None;
            let (_valid, staged) = ValidBlock::validate_signed_genesis_keep_voting_block(
                provisional.0,
                &topology,
                &authority,
                &TimeSource::new_system(),
                &state,
                &mut voting_block,
                mode,
            )
            .unpack(|_| {})
            .unwrap_or_else(|(block, error)| {
                let transaction_errors = (0..block.external_transactions().count())
                    .filter_map(|index| {
                        block
                            .error(index)
                            .map(|reason| format!("transaction[{index}]: {reason:?}"))
                    })
                    .collect::<Vec<_>>();
                panic!(
                    "provisional genesis fixture must stage before context binding: {error}; {}",
                    transaction_errors.join("; ")
                );
            });
            (
                iroha_core::sumeragi::staged_genesis_nexus_amx_context_hash(&staged),
                iroha_core::sumeragi::staged_genesis_execution_policy_hash(&staged)
                    .expect("derive staged execution-policy identity"),
            )
        }

        fn bind_staged_context_for_test(
            genesis: RawGenesisTransaction,
            genesis_authority: &KeyPair,
            config: &Config,
        ) -> GenesisBlock {
            let (nexus_amx_hash, execution_policy_hash) =
                staged_context_hashes_for_test(&genesis, genesis_authority, config);
            let mut parameters = genesis.sumeragi_v2_context_parameters();
            parameters.nexus_amx_context_hash = nexus_amx_hash.into();
            parameters.execution_policy_hash = execution_policy_hash.into();
            genesis
                .with_sumeragi_v2_context_parameters(parameters)
                .with_consensus_meta()
                .build_and_sign(genesis_authority)
                .expect("sign context-bound genesis fixture")
        }

        #[test]
        fn manifest_crypto_matches_config() {
            let manifest = sample_manifest();
            let config = sample_config();
            ensure_manifest_crypto_matches(&manifest, &config)
                .expect("expected manifest and config to match");
        }

        #[test]
        fn detects_hash_mismatch() {
            let manifest = sample_manifest();
            let mut config = sample_config();
            config.crypto.default_hash = "sm3-256".to_owned();
            let err = ensure_manifest_crypto_matches(&manifest, &config)
                .expect_err("hash mismatch should be detected");
            assert!(
                err.contains("default_hash"),
                "error should mention hash: {err}"
            );
        }

        #[test]
        fn detects_allowed_signing_mismatch() {
            let mut manifest = sample_manifest();
            let crypto = ManifestCrypto {
                allowed_signing: vec![Algorithm::Ed25519, Algorithm::Sm2],
                default_hash: "sm3-256".to_owned(),
                ..Default::default()
            };
            manifest = manifest.into_builder().with_crypto(crypto).build_raw();

            let config = sample_config();
            let err = ensure_manifest_crypto_matches(&manifest, &config)
                .expect_err("allowed signing mismatch should be detected");
            assert!(
                err.contains("allowed_signing"),
                "error should mention allowed_signing mismatch: {err}"
            );
        }

        #[test]
        fn detects_allowed_curve_ids_mismatch() {
            let manifest = sample_manifest();
            let mut config = sample_config();
            config.crypto.allowed_curve_ids.push(2);

            let err = ensure_manifest_crypto_matches(&manifest, &config)
                .expect_err("curve id mismatch should be detected");
            assert!(
                err.contains("allowed_curve_ids"),
                "error should mention allowed_curve_ids mismatch: {err}"
            );
        }

        #[test]
        fn verify_genesis_metadata_rejects_crypto_mismatch_in_block() -> eyre::Result<()> {
            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();
            let mut config = sample_config();
            let genesis_keys = config.common.key_pair.clone();
            let chain = config.common.chain.clone();
            let manifest = GenesisBuilder::new_without_executor(chain.clone(), PathBuf::from("."))
                .build_raw()
                .with_consensus_meta();
            let genesis_block = manifest.build_and_sign(&genesis_keys)?;

            let mut instructions = Vec::new();
            for tx in genesis_block.0.external_transactions() {
                if let Executable::Instructions(batch) = tx.instructions() {
                    instructions.extend(batch.iter().cloned());
                }
            }

            let handshake_meta = instructions
                .iter()
                .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
                .find_map(|set| {
                    if let Parameter::Custom(custom) = set.inner()
                        && custom.id() == &consensus_metadata::handshake_meta_id()
                    {
                        decode_consensus_handshake_meta(custom.payload()).ok()
                    } else {
                        None
                    }
                })
                .expect("handshake meta should be present in genesis");
            let (mode, mode_tag) = match handshake_meta.mode {
                iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => (
                    iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
                    iroha_core::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
                ),
                iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => (
                    iroha_data_model::block::consensus_v2::ConsensusMode::Npos,
                    iroha_core::sumeragi::consensus::NPOS_TAG.to_string(),
                ),
            };
            let proto = handshake_meta.wire_protocol_version;
            let consensus_fingerprint = handshake_meta.consensus_fingerprint.into_bytes();
            let config_caps = build_consensus_config_caps(&config.nexus, None, None)
                .map_err(|err| eyre::eyre!(format!("{err:?}")))?;
            let consensus_caps = iroha_p2p::ConsensusHandshakeCaps {
                mode,
                proto_version: proto,
                consensus_fingerprint,
                config: config_caps,
            };

            config.genesis.public_key = genesis_keys.public_key().clone();
            config.common.chain = chain;
            config.common.key_pair = genesis_keys.clone();
            config.crypto.allowed_signing = vec![Algorithm::Ed25519];

            let err =
                verify_genesis_metadata(&genesis_block, &config, &consensus_caps, &mode_tag, proto)
                    .expect_err("crypto mismatch should be detected");
            let report = format!("{err:?}");
            assert!(
                report.contains("crypto manifest") || report.contains("crypto mismatch"),
                "unexpected error: {report}"
            );

            Ok(())
        }

        #[test]
        fn genesis_validation_accepts_bls_controllers_when_crypto_config_applied() {
            use std::sync::Arc;

            use iroha_core::{block::ValidBlock, kura::Kura, query::store::LiveQueryStore};
            use iroha_data_model::{account::curve::CurveId, prelude::*};
            use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();

            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
            let genesis_account_id = SAMPLE_GENESIS_ACCOUNT_ID.clone();
            let domain_id: DomainId =
                DomainId::try_new("wonderland", "universal").expect("valid domain id");
            let bls_keypair = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let bls_account_id = AccountId::new(bls_keypair.public_key().clone());

            let tx = TransactionBuilder::new_genesis(
                genesis_account_id.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([
                InstructionBox::from(Register::domain(Domain::new(domain_id.clone()))),
                InstructionBox::from(Register::account(Account::new(bls_account_id.clone()))),
            ])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            let block = SignedBlock::genesis(
                vec![tx],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                None,
                None,
            );

            let world = World::with(
                [genesis_domain(
                    SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
                )],
                [genesis_account(
                    SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
                )],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let nexus = state.nexus_snapshot();
            let lane_manifests = Arc::new(
                LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
            );
            state.install_lane_manifests(&lane_manifests);

            let mut crypto = iroha_config::parameters::actual::Crypto::default();
            if !crypto.allowed_signing.contains(&Algorithm::BlsNormal) {
                crypto.allowed_signing.push(Algorithm::BlsNormal);
            }
            crypto.allowed_signing.sort();
            crypto.allowed_signing.dedup();
            let mut curve_ids = crypto
                .allowed_signing
                .iter()
                .filter_map(|algo| CurveId::try_from_algorithm(*algo).ok())
                .map(CurveId::as_u8)
                .collect::<Vec<_>>();
            curve_ids.sort_unstable();
            curve_ids.dedup();
            crypto.allowed_curve_ids = curve_ids;
            state.set_crypto(crypto);

            let topology = Topology::new(vec![PeerId::new(
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
            )]);
            let time_source = TimeSource::new_system();
            let mut voting_block = None;
            let result = ValidBlock::validate_keep_voting_block(
                block,
                &topology,
                &genesis_account_id,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            if let Err((block, error)) = result {
                let results = block
                    .results()
                    .map(|result| format!("{result:?}"))
                    .collect::<Vec<_>>();
                panic!(
                    "genesis validation should accept BLS controllers when crypto config allows \
                     it: {error:?}; transaction results: {results:?}"
                );
            }
        }

        #[test]
        fn fresh_v2_genesis_staging_does_not_commit_state_or_kura() {
            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();

            let chain_id = ChainId::from("fresh-v2-genesis-staging-test");
            let genesis_authority = iroha_crypto::KeyPair::try_from_seed(
                b"fresh-v2-genesis-authority".to_vec(),
                Algorithm::Ed25519,
            )
            .expect("deterministic genesis authority");
            let voter_keys = (0_u8..4)
                .map(|index| {
                    iroha_crypto::KeyPair::try_from_seed(
                        vec![0x70 + index; 32],
                        Algorithm::BlsNormal,
                    )
                    .expect("deterministic BLS voter")
                })
                .collect::<Vec<_>>();
            let topology = voter_keys
                .iter()
                .map(|key| {
                    let pop =
                        iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("BLS PoP");
                    GenesisTopologyEntry::new(PeerId::new(key.public_key().clone()), pop)
                })
                .collect::<Vec<_>>();
            let raw_genesis = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
                .set_topology(topology)
                .build_raw();

            let authority_id = AccountId::new(genesis_authority.public_key().clone());
            let mut config = sample_config();
            config.common.chain = chain_id.clone();
            config.genesis.public_key = genesis_authority.public_key().clone();
            if !config
                .crypto
                .allowed_signing
                .contains(&Algorithm::BlsNormal)
            {
                config.crypto.allowed_signing.push(Algorithm::BlsNormal);
            }
            let genesis = bind_staged_context_for_test(raw_genesis, &genesis_authority, &config);
            let (state, _kura) = genesis_staging_state_for_test(&config, &genesis);

            let voters = iroha_core::sumeragi::signed_genesis_voting_peers(&genesis)
                .expect("signed voting roster");
            let topology = Topology::new(voters);
            let before_height = state.committed_height();
            let before_hashes = state.committed_block_hashes_snapshot();
            let mut voting_block = None;
            let (mode, signed_parameters) =
                signed_v2_genesis_context_metadata(&genesis).expect("signed v2 metadata");
            let (_valid, staged) = ValidBlock::validate_signed_genesis_keep_voting_block(
                genesis.0.clone(),
                &topology,
                &authority_id,
                &TimeSource::new_system(),
                &state,
                &mut voting_block,
                mode,
            )
            .unpack(|_| {})
            .expect("genesis executes in staging overlay");
            let bootstrap = iroha_core::sumeragi::freeze_staged_genesis_v2(
                &genesis,
                &staged,
                mode,
                signed_parameters,
            )
            .expect("freeze staged height context");
            assert_eq!(bootstrap.context().height, 1);
            assert_eq!(bootstrap.context().roster.len(), voter_keys.len());
            assert!(
                bootstrap
                    .context()
                    .roster
                    .iter()
                    .all(|entry| entry.power == 1)
            );
            drop(staged);

            assert_eq!(state.committed_height(), before_height);
            assert_eq!(state.committed_block_hashes_snapshot(), before_hashes);
        }

        struct OfflineSemanticGenesisFixture {
            config: Config,
            genesis: GenesisBlock,
            authority: AccountId,
            mode: iroha_data_model::block::consensus_v2::ConsensusMode,
            parameters: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
            cadence_ms: u64,
        }

        fn offline_semantic_genesis_fixture(
            extra_instructions: impl IntoIterator<Item = InstructionBox>,
        ) -> OfflineSemanticGenesisFixture {
            let mut config = sample_config();
            let chain_id = ChainId::from("offline-genesis-validation-test");
            let genesis_authority = iroha_crypto::KeyPair::try_from_seed(
                b"offline-genesis-validation-authority".to_vec(),
                Algorithm::Ed25519,
            )
            .expect("deterministic genesis authority");
            let topology = (0_u8..4)
                .map(|index| {
                    let key = iroha_crypto::KeyPair::try_from_seed(
                        vec![0x50 + index; 32],
                        Algorithm::BlsNormal,
                    )
                    .expect("deterministic BLS validator");
                    let pop = iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("BLS proof of possession");
                    GenesisTopologyEntry::new(PeerId::new(key.public_key().clone()), pop)
                })
                .collect();
            let authority = AccountId::new(genesis_authority.public_key().clone());

            config.common.chain = chain_id.clone();
            config.genesis.public_key = genesis_authority.public_key().clone();
            if !config
                .crypto
                .allowed_signing
                .contains(&Algorithm::BlsNormal)
            {
                config.crypto.allowed_signing.push(Algorithm::BlsNormal);
            }
            let base_genesis =
                GenesisBuilder::new_without_executor(chain_id, ".").set_topology(topology);
            let base_raw = base_genesis.build_raw();
            let (context_hash, execution_policy_hash) =
                staged_context_hashes_for_test(&base_raw, &genesis_authority, &config);
            let mut parameters = base_raw.sumeragi_v2_context_parameters();
            parameters.nexus_amx_context_hash = context_hash.into();
            parameters.execution_policy_hash = execution_policy_hash.into();
            let mut builder = base_raw
                .into_builder()
                .with_sumeragi_v2_context_parameters(parameters);
            // These fixtures add ordinary world-state instructions only; they
            // deliberately do not alter the signed Nexus/AMX projection.
            for instruction in extra_instructions {
                builder = builder.append_instruction(instruction);
            }
            let genesis = builder
                .build_and_sign(&genesis_authority)
                .expect("signed genesis fixture");
            let (mode, parameters) =
                signed_v2_genesis_context_metadata(&genesis).expect("signed v2 metadata");
            let config_caps = build_consensus_config_caps(&config.nexus, None, None)
                .expect("default consensus config caps");
            let (_, _, _, cadence_ms) =
                consensus_caps_from_genesis(&genesis, &config_caps, &config.sumeragi)
                    .expect("canonical genesis consensus metadata");
            OfflineSemanticGenesisFixture {
                config,
                genesis,
                authority,
                mode,
                parameters,
                cadence_ms,
            }
        }

        #[test]
        fn check_config_offline_executes_available_genesis() {
            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();
            let fixture = offline_semantic_genesis_fixture([]);

            validate_genesis_execution_offline(
                &fixture.config,
                &fixture.genesis,
                &fixture.authority,
                fixture.mode,
                fixture.parameters,
                fixture.cadence_ms,
                load_configured_kagemusha_release_catalog(&fixture.config)
                    .expect("an omitted release cache uses an empty catalog"),
            )
            .expect("valid genesis should execute in the disposable overlay");
        }

        #[test]
        fn check_config_installs_offline_catalog_before_genesis_activation() {
            let source = include_str!("main.rs");
            let check_path = source
                .split_once("fn validate_genesis_execution_offline(")
                .expect("offline genesis validator")
                .1
                .split_once("fn enforce_build_line(")
                .expect("end offline genesis validator")
                .0;
            let install = check_path
                .find("state.set_kagemusha_release_catalog(kagemusha_release_catalog)")
                .expect("check-config catalog installation");
            let execute = check_path
                .find("ValidBlock::validate_signed_genesis_keep_voting_block(")
                .expect("offline genesis execution");
            let freeze = check_path
                .find("freeze_staged_genesis_v2(")
                .expect("staged genesis freeze");
            assert!(install < execute);
            assert!(execute < freeze);
            assert!(
                !check_path.contains("ensure_mandatory_offline"),
                "offline support must not introduce a genesis readiness gate"
            );

            let runtime_path = source
                .split_once("pub async fn start_with_runtime_deps(")
                .expect("runtime startup")
                .1
                .split_once("// Resolve the complete replay boundary")
                .expect("end runtime catalog setup")
                .0;
            assert!(
                runtime_path
                    .contains("install_configured_kagemusha_release_catalog(&mut state, &config)")
            );
        }

        #[test]
        fn check_config_accepts_taira_without_offline_backend_settings() {
            let mut config = sample_config();
            config.common.chain = ChainId::from("taira");
            config.confidential.enabled = true;
            config.confidential.assume_valid = false;

            validate_config_for_check(&config, None, false)
                .expect("Taira has universal offline primitives without backend enablement");
        }

        #[test]
        fn check_config_qualifies_the_fixed_moderation_strict_ingress() {
            let mut exact = sample_config();
            configure_exact_moderation_strict_ingress(&mut exact);
            assert!(
                validate_config_for_check(&exact, None, false)
                    .expect("exact fixed moderation ingress must pass static check-config")
                    .is_none()
            );

            for (mutation, expected) in [
                (0, "runtime-provider binding is substituted"),
                (1, "runtime-provider binding is stale or revoked"),
            ] {
                let mut invalid = exact.clone();
                let moderation = invalid
                    .torii
                    .sorafs_storage
                    .moderation_orchestrator
                    .as_mut()
                    .expect("configured moderation runtime");
                if mutation == 0 {
                    moderation.strict_ingress_handle =
                        "torii.sorafs.moderation-strict-ingress.secondary".into();
                } else {
                    moderation.strict_ingress_revision += 1;
                }
                let report = validate_config_for_check(&invalid, None, false)
                    .expect_err("invalid fixed ingress binding must fail check-config");
                assert!(format!("{report:#}").contains(expected));
            }
        }

        #[test]
        fn configured_kagemusha_catalog_loader_is_optional_for_every_asset() {
            let mut config = sample_config();
            config.settlement.offline.kagemusha_release_policy_path = None;
            config.settlement.offline.kagemusha_artifact_dir = None;
            assert!(
                load_configured_kagemusha_release_catalog(&config)
                    .expect("an omitted verifier cache uses an empty catalog")
                    .is_empty()
            );

            config.settlement.offline.escrow_accounts.insert(
                iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                    iroha_data_model::domain::DomainId::try_new("offline", "universal")
                        .expect("offline asset domain"),
                    "cash".parse().expect("offline asset name"),
                ),
                iroha_test_samples::ALICE_ID.clone(),
            );
            assert!(
                load_configured_kagemusha_release_catalog(&config)
                    .expect("runtime escrow state must not require a process-local catalog")
                    .is_empty()
            );

            config.settlement.offline.kagemusha_release_policy_path =
                Some(PathBuf::from("/tmp/policy.norito"));
            assert!(
                load_configured_kagemusha_release_catalog(&config)
                    .err()
                    .expect("catalog paths must be configured together")
                    .contains("configured together")
            );
        }

        #[test]
        fn configured_kagemusha_catalog_loader_uses_seal_without_fallback() {
            let mut config = sample_config();
            config.settlement.offline.kagemusha_release_policy_path =
                Some(PathBuf::from("/missing/policy.norito"));
            config.settlement.offline.kagemusha_artifact_dir =
                Some(PathBuf::from("/missing/artifacts"));
            config
                .settlement
                .offline
                .kagemusha_catalog_qualification_seal_path =
                Some(PathBuf::from("/missing/catalog-seal.norito"));

            let error = load_configured_kagemusha_release_catalog(&config)
                .err()
                .expect("configured seal must select the fail-closed sealed loader");
            assert!(
                error.contains("sealed Kagemusha V4 release catalog without fallback"),
                "unexpected sealed catalog error: {error}"
            );
        }

        #[test]
        fn qualification_seal_check_requires_local_genesis() {
            let config = sample_config();

            let error = validate_config_for_check(&config, None, true).err().expect(
                "seal publication must wait for full Kagemusha release and genesis validation",
            );
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains("requires locally available genesis"),
                "unexpected missing-genesis error: {rendered}"
            );
        }

        #[test]
        fn check_config_offline_rejects_genesis_instruction_failure() {
            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();
            let duplicate_domain =
                DomainId::try_new("duplicate", "universal").expect("valid domain id");
            let instructions = [
                Register::domain(Domain::new(duplicate_domain.clone())).into(),
                Register::domain(Domain::new(duplicate_domain)).into(),
            ];
            let fixture = offline_semantic_genesis_fixture(instructions);

            let error = validate_genesis_execution_offline(
                &fixture.config,
                &fixture.genesis,
                &fixture.authority,
                fixture.mode,
                fixture.parameters,
                fixture.cadence_ms,
                load_configured_kagemusha_release_catalog(&fixture.config)
                    .expect("an omitted release cache uses an empty catalog"),
            )
            .expect_err("duplicate genesis registration must fail semantic execution");
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains("genesis instruction execution failed"),
                "unexpected offline validation error: {rendered}"
            );
        }

        #[test]
        fn consensus_config_caps_use_canonical_v2_fields() {
            let config = sample_config();
            let caps = build_consensus_config_caps(&config.nexus, None, None)
                .expect("config caps should build");
            let expected_nexus_policy_digest = iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
                &config.nexus,
                None,
                None,
            )
            .expect("default Nexus config should produce a policy digest");

            assert_eq!(caps.v2_config_fingerprint, [0; 32]);
            assert_eq!(caps.nexus_policy_digest, expected_nexus_policy_digest);
            assert_eq!(
                caps.ivm_gas_schedule_hash,
                <[u8; 32]>::from(ivm::gas::schedule_hash())
            );
        }

        #[test]
        fn consensus_caps_use_frozen_height_context_mode() {
            use iroha_core::{kura::Kura, query::store::LiveQueryStore};

            let config = sample_config();
            let config_caps = build_consensus_config_caps(&config.nexus, None, None)
                .expect("config caps should build");
            let signed_context = iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended();

            let permissioned_state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let permissioned_world = permissioned_state.world_view();
            let (mode_tag_perm, bls_perm, caps_perm) = compute_consensus_handshake_caps(
                &permissioned_world,
                u64::try_from(permissioned_state.committed_height()).unwrap_or(u64::MAX),
                &config,
                &config_caps,
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
                signed_context,
            )
            .expect("valid permissioned v2 handshake config");
            assert_eq!(
                mode_tag_perm,
                iroha_core::sumeragi::consensus::PERMISSIONED_TAG
            );
            assert_eq!(bls_perm, "bls-iroha2:permissioned-sumeragi:v2");
            assert_eq!(
                caps_perm.config.nexus_policy_digest, config_caps.nexus_policy_digest,
                "the Nexus policy digest remains an independent admission gate",
            );
            assert_ne!(
                caps_perm.config.v2_config_fingerprint, [0; 32],
                "the handshake must replace the pre-world placeholder with the canonical v2 fingerprint",
            );
            let permissioned_fp = caps_perm.consensus_fingerprint;
            let permissioned_v2_config_fp = caps_perm.config.v2_config_fingerprint;

            let npos_world = World::new();
            {
                let mut block = npos_world.block();
                let npos = iroha_data_model::parameter::system::SumeragiNposParameters::default();
                block
                    .parameters
                    .get_mut()
                    .set_parameter(Parameter::Custom(npos.into_custom_parameter()));
                block.commit();
            }
            let npos_state = State::new_for_testing(
                npos_world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let npos_world = npos_state.world_view();
            let (mode_tag_npos, bls_npos, caps_npos) = compute_consensus_handshake_caps(
                &npos_world,
                u64::try_from(npos_state.committed_height()).unwrap_or(u64::MAX),
                &config,
                &config_caps,
                iroha_data_model::block::consensus_v2::ConsensusMode::Npos,
                signed_context,
            )
            .expect("valid NPoS v2 handshake config");
            assert_eq!(mode_tag_npos, iroha_core::sumeragi::consensus::NPOS_TAG);
            assert_eq!(bls_npos, "bls-iroha2:npos-sumeragi:v2");
            assert_eq!(
                caps_npos.config.nexus_policy_digest,
                config_caps.nexus_policy_digest,
            );
            assert_ne!(
                permissioned_v2_config_fp, caps_npos.config.v2_config_fingerprint,
                "the canonical shared-config fingerprint must bind the active v2 mode",
            );
            assert_ne!(
                permissioned_fp, caps_npos.consensus_fingerprint,
                "the frozen height-context mode must change the consensus fingerprint"
            );
        }

        #[test]
        fn verify_genesis_metadata_rejects_consensus_mode_mismatch() -> eyre::Result<()> {
            use iroha_data_model::parameter::system::SumeragiConsensusMode;

            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();
            let config = sample_config();
            let genesis_keys = config.common.key_pair.clone();
            let chain = config.common.chain.clone();
            let permissioned_genesis =
                GenesisBuilder::new_without_executor(chain.clone(), PathBuf::from("."))
                    .build_raw()
                    .with_consensus_meta()
                    .build_and_sign(&genesis_keys)?;
            let npos_genesis =
                GenesisBuilder::new_without_executor(chain.clone(), PathBuf::from("."))
                    .append_parameter(Parameter::Custom(
                        iroha_data_model::parameter::system::SumeragiNposParameters::default()
                            .into_custom_parameter(),
                    ))
                    .build_raw()
                    .with_consensus_mode(SumeragiConsensusMode::Npos)
                    .with_consensus_meta()
                    .build_and_sign(&genesis_keys)?;

            let config_caps = build_consensus_config_caps(&config.nexus, None, None)
                .map_err(|err| eyre::eyre!(format!("{err:?}")))?;
            let (mode_tag, _bls_domain, consensus_caps, _) =
                consensus_caps_from_genesis(&permissioned_genesis, &config_caps, &config.sumeragi)
                    .expect("permissioned signed genesis must produce canonical v2 caps");

            let proto = iroha_core::sumeragi::consensus::PROTO_VERSION;
            let err =
                verify_genesis_metadata(&npos_genesis, &config, &consensus_caps, &mode_tag, proto)
                    .expect_err("signed genesis mode mismatch should be detected");
            assert!(
                format!("{err:?}").contains("consensus_mode"),
                "error should mention consensus_mode mismatch: {err:?}"
            );

            Ok(())
        }

        #[test]
        fn verify_genesis_metadata_rejects_fingerprint_mismatch() -> eyre::Result<()> {
            use iroha_core::{kura::Kura, query::store::LiveQueryStore};

            let _registry_guard = instruction_registry_test_guard();
            iroha_genesis::init_instruction_registry();
            let mut config = sample_config();
            let genesis_keys = config.common.key_pair.clone();
            let chain = config.common.chain.clone();

            // Build a canonical manifest with consensus metadata, then tamper with the advertised
            // fingerprint so genesis validation should fail.
            let manifest = GenesisBuilder::new_without_executor(chain, PathBuf::from("."))
                .build_raw()
                .with_consensus_meta();
            let mut manifest_value =
                norito::json::value::to_value(&manifest).expect("serialize manifest");
            if let Some(obj) = manifest_value.as_object_mut() {
                obj.insert(
                    "consensus_fingerprint".to_owned(),
                    norito::json::Value::String(
                        "0x00000000000000000000000000000000000000000000000000000000000000ff"
                            .to_owned(),
                    ),
                );
            } else {
                panic!("manifest must serialize as a JSON object");
            }
            let tampered: RawGenesisTransaction =
                norito::json::value::from_value(manifest_value).expect("decode tampered manifest");
            let genesis_block = tampered.build_and_sign(&genesis_keys)?;

            let config_caps = build_consensus_config_caps(&config.nexus, None, None)
                .map_err(|err| eyre::eyre!(format!("{err:?}")))?;
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let world = state.world_view();
            let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
            let (mode_tag, _bls_domain, consensus_caps) = compute_consensus_handshake_caps(
                &world,
                height,
                &config,
                &config_caps,
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
                iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
            )
            .expect("valid v2 handshake config");

            // Diverge the runtime chain after computing consensus caps to force a
            // fingerprint mismatch without altering the embedded handshake metadata.
            config.common.chain = ChainId::from("fingerprint-mismatch");
            let proto = iroha_core::sumeragi::consensus::PROTO_VERSION;
            let err =
                verify_genesis_metadata(&genesis_block, &config, &consensus_caps, &mode_tag, proto)
                    .expect_err("tampered fingerprint should be rejected");
            assert!(
                format!("{err:?}")
                    .to_ascii_lowercase()
                    .contains("fingerprint"),
                "expected fingerprint mismatch error, got {err:?}"
            );

            Ok(())
        }

        #[cfg(feature = "sm")]
        #[test]
        fn manifest_crypto_applies_without_genesis_block() -> eyre::Result<()> {
            let genesis_keys = KeyPair::random();
            let mut config_table = config_factory(genesis_keys.public_key());
            iroha_config::base::toml::Writer::new(&mut config_table)
                .write(["kura", "store_dir"], "./storage")
                .write(["snapshot", "store_dir"], "./snapshots")
                .write(["dev_telemetry", "out_file"], "./telemetry.log");
            if let Some(genesis_table) = config_table
                .get_mut("genesis")
                .and_then(toml::Value::as_table_mut)
            {
                genesis_table.remove("file");
            }

            let mut manifest_crypto = ManifestCrypto::default();
            manifest_crypto.default_hash = "sm3-256".to_owned();
            manifest_crypto.allowed_signing = vec![Algorithm::Ed25519, Algorithm::Sm2];
            manifest_crypto.sm2_distid_default = "CN1234567812345678".to_owned();

            let manifest = GenesisBuilder::new_without_executor(
                ChainId::from("test-chain"),
                PathBuf::from("."),
            )
            .with_crypto(manifest_crypto)
            .build_raw();

            let temp_dir = tempfile::tempdir()?;
            let config_path = temp_dir.path().join("config.toml");
            let manifest_path = temp_dir.path().join("manifest.json");

            std::fs::write(&config_path, toml::to_string(&config_table)?)?;
            std::fs::write(&manifest_path, norito::json::to_vec(&manifest)?)?;

            let (config, genesis) = read_config_and_genesis(&Args {
                config: Some(config_path),
                genesis_manifest_json: Some(manifest_path),
                startup: StartupArgs {
                    check_config: false,
                    write_kagemusha_catalog_qualification_seal: None,
                    trace_config: false,
                    config_blake3: None,
                },
                terminal_colors: false,
                language: None,
                sora: false,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
                fastpq_device_class: None,
                fastpq_chip_family: None,
                fastpq_gpu_kind: None,
            })
            .map_err(|report| eyre::eyre!("{report:?}"))?;

            assert!(genesis.is_none());
            assert!(config.crypto.default_hash.eq_ignore_ascii_case("sm3-256"));
            assert!(config.crypto.allowed_signing.contains(&Algorithm::Sm2));
            assert_eq!(config.crypto.sm2_distid_default, "CN1234567812345678");

            Ok(())
        }
    }

    mod config_integration {
        use assertables::assert_contains;
        use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, bls_normal_pop_prove};
        use iroha_genesis::GenesisBuilder;
        use iroha_primitives::addr::socket_addr;
        use path_absolutize::Absolutize as _;

        #[allow(unused_imports)]
        use super::*;

        fn config_factory(genesis_public_key: &PublicKey) -> toml::Table {
            let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let pubkey = keypair.public_key().clone();
            let privkey = keypair.private_key().clone();
            let pop = bls_normal_pop_prove(&privkey).expect("pop prove");

            let mut table = toml::Table::new();
            iroha_config::base::toml::Writer::new(&mut table)
                .write("chain", "0")
                .write("public_key", pubkey.to_string())
                // Use `ExposedPrivateKey`'s Display impl to emit the actual hex instead of
                // the redacted placeholder provided by `PrivateKey::Display`.
                .write("private_key", ExposedPrivateKey(privkey).to_string())
                .write(
                    "soranet_transport_public_key",
                    "ed0120D9F6AEF1813164294D1D9C0662FEB9C7F7861B4DFFE385680331093DA4ABD10B",
                )
                .write(
                    "soranet_transport_private_key",
                    "802620134C4527B3852AE2218A8F079B301C651EAD8C7567B96BD7A9BE8DB366E46B89",
                )
                .write(
                    ["network", "address"],
                    socket_addr!(127.0.0.1:1337).to_literal(),
                )
                .write(
                    ["network", "public_address"],
                    socket_addr!(127.0.0.1:1337).to_literal(),
                )
                .write(
                    ["torii", "address"],
                    socket_addr!(127.0.0.1:8080).to_literal(),
                )
                .write(
                    ["streaming", "identity_public_key"],
                    "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB",
                )
                .write(
                    ["streaming", "identity_private_key"],
                    "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F",
                )
                .write(["confidential", "enabled"], true)
                .write(["confidential", "assume_valid"], false)
                .write(["genesis", "public_key"], genesis_public_key.to_string());
            let mut pop_entry = toml::Table::new();
            pop_entry.insert(
                "public_key".to_string(),
                toml::Value::String(pubkey.to_string()),
            );
            pop_entry.insert("pop_hex".to_string(), toml::Value::String(hex::encode(pop)));
            table.insert(
                "trusted_peers_pop".to_string(),
                toml::Value::Array(vec![toml::Value::Table(pop_entry)]),
            );
            table
        }

        fn config_test_args(config_path: PathBuf, genesis_manifest_json: Option<PathBuf>) -> Args {
            Args {
                config: Some(config_path),
                genesis_manifest_json,
                startup: StartupArgs {
                    check_config: false,
                    write_kagemusha_catalog_qualification_seal: None,
                    trace_config: false,
                    config_blake3: None,
                },
                terminal_colors: false,
                language: None,
                sora: false,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
                fastpq_device_class: None,
                fastpq_chip_family: None,
                fastpq_gpu_kind: None,
            }
        }

        #[test]
        fn integrity_bound_config_is_hashed_and_parsed_from_one_buffer() -> eyre::Result<()> {
            let genesis_key_pair = KeyPair::random();
            let config = config_factory(genesis_key_pair.public_key());
            let raw = toml::to_string(&config)?;
            let dir = tempfile::tempdir()?;
            let config_path = dir.path().join("config.toml");
            std::fs::write(&config_path, raw.as_bytes())?;
            let expected = blake3::hash(raw.as_bytes()).to_hex().to_string();
            let mut args = config_test_args(config_path.clone(), None);
            args.startup.config_blake3 = Some(expected);

            read_config_and_genesis(&args)
                .map_err(|report| eyre::eyre!("valid integrity-bound config failed: {report:?}"))?;

            std::fs::write(&config_path, format!("{raw}\n# changed after admission\n"))?;
            let error = read_config_and_genesis(&args)
                .expect_err("changed integrity-bound config must fail closed");
            assert!(
                format!("{error:?}").contains("has BLAKE3"),
                "unexpected integrity error: {error:?}"
            );
            Ok(())
        }

        #[test]
        fn integrity_bound_config_rejects_extends() -> eyre::Result<()> {
            let genesis_key_pair = KeyPair::random();
            let mut config = config_factory(genesis_key_pair.public_key());
            config.insert(
                "extends".to_owned(),
                toml::Value::String("base.toml".to_owned()),
            );
            let raw = toml::to_string(&config)?;
            let dir = tempfile::tempdir()?;
            let config_path = dir.path().join("config.toml");
            std::fs::write(&config_path, raw.as_bytes())?;
            let mut args = config_test_args(config_path, None);
            args.startup.config_blake3 = Some(blake3::hash(raw.as_bytes()).to_hex().to_string());

            let error = read_config_and_genesis(&args)
                .expect_err("integrity-bound config must not resolve external extends");
            assert!(
                format!("{error:?}").contains("must be flattened"),
                "unexpected extends error: {error:?}"
            );
            Ok(())
        }

        fn load_config_with_overrides<F>(
            mut adjust: F,
        ) -> eyre::Result<(Config, tempfile::TempDir, PathBuf)>
        where
            F: FnMut(&mut toml::Table, &KeyPair),
        {
            let genesis_key_pair = KeyPair::random();
            let raw = GenesisBuilder::new_without_executor(ChainId::from("chain"), ".").build_raw();
            iroha_genesis::init_instruction_registry();
            let genesis = raw
                .build_and_sign(&genesis_key_pair)
                .expect("build genesis");

            let mut config = config_factory(genesis_key_pair.public_key());
            iroha_config::base::toml::Writer::new(&mut config)
                .write(["genesis", "file"], "./genesis/genesis.signed.nrt")
                .write(["kura", "store_dir"], "../storage")
                .write(["snapshot", "store_dir"], "../snapshots")
                .write(["dev_telemetry", "out_file"], "../logs/telemetry");

            adjust(&mut config, &genesis_key_pair);

            let dir = tempfile::tempdir()?;
            let config_dir = dir.path().join("config");
            let genesis_dir = config_dir.join("genesis");
            std::fs::create_dir_all(&genesis_dir)?;

            let config_path = config_dir.join("config.toml");
            let genesis_path = genesis_dir.join("genesis.signed.nrt");
            let executor_path = genesis_dir.join("executor.to");

            std::fs::write(&config_path, toml::to_string(&config)?)?;
            std::fs::write(&genesis_path, genesis.0.encode_wire()?)?;
            std::fs::write(&executor_path, "")?;

            let (config, _genesis) =
                read_config_and_genesis(&config_test_args(config_path.clone(), None))
                    .map_err(|report| eyre::eyre!("{report:?}"))?;

            Ok((config, dir, config_path))
        }

        #[test]
        fn cli_genesis_manifest_path_overrides_config() -> eyre::Result<()> {
            let (_config, dir, config_path) = load_config_with_overrides(|table, _| {
                iroha_config::base::toml::Writer::new(table)
                    .write(["genesis", "manifest_json"], "./stale-manifest.json");
            })?;
            let manifest_path = dir.path().join("bound-genesis.json");
            std::fs::write(&manifest_path, b"{}")?;

            let (config, _genesis) = read_config_and_genesis(&config_test_args(
                config_path,
                Some(manifest_path.clone()),
            ))
            .map_err(|report| eyre::eyre!("{report:?}"))?;

            assert_eq!(
                config
                    .genesis
                    .manifest_json
                    .as_ref()
                    .expect("CLI manifest override should be retained")
                    .resolve_relative_path(),
                manifest_path
            );
            Ok(())
        }

        fn parse_config_with_overrides<F>(
            mut adjust: F,
        ) -> eyre::Result<(Config, tempfile::TempDir, PathBuf)>
        where
            F: FnMut(&mut toml::Table, &KeyPair),
        {
            let genesis_key_pair = KeyPair::random();
            let mut config = config_factory(genesis_key_pair.public_key());
            iroha_config::base::toml::Writer::new(&mut config)
                .write(["kura", "store_dir"], "../storage")
                .write(["snapshot", "store_dir"], "../snapshots")
                .write(["dev_telemetry", "out_file"], "../logs/telemetry");

            adjust(&mut config, &genesis_key_pair);

            let dir = tempfile::tempdir()?;
            let config_dir = dir.path().join("config");
            std::fs::create_dir_all(&config_dir)?;

            let config_path = config_dir.join("config.toml");
            std::fs::write(&config_path, toml::to_string(&config)?)?;

            let mut reader = ConfigReader::new();
            reader = reader
                .read_toml_with_extends(&config_path)
                .map_err(|report| eyre::eyre!("{report:?}"))?;
            let config = reader
                .read_and_complete::<UserConfig>()
                .map_err(|report| eyre::eyre!("{report:?}"))?
                .parse()
                .map_err(|report| eyre::eyre!("{report:?}"))?;

            Ok((config, dir, config_path))
        }

        #[test]
        fn relative_file_paths_resolution() -> eyre::Result<()> {
            // Given

            let genesis_key_pair = KeyPair::random();
            let raw = GenesisBuilder::new_without_executor(ChainId::from("chain"), ".").build_raw();
            iroha_genesis::init_instruction_registry();
            let genesis = raw
                .build_and_sign(&genesis_key_pair)
                .expect("build genesis");

            let mut config = config_factory(genesis_key_pair.public_key());
            iroha_config::base::toml::Writer::new(&mut config)
                .write(["genesis", "file"], "./genesis/genesis.signed.nrt")
                .write(["kura", "store_dir"], "../storage")
                .write(["snapshot", "store_dir"], "../snapshots")
                .write(["dev_telemetry", "out_file"], "../logs/telemetry");

            let dir = tempfile::tempdir()?;
            let genesis_path = dir.path().join("config/genesis/genesis.signed.nrt");
            let executor_path = dir.path().join("config/genesis/executor.to");
            let config_path = dir.path().join("config/config.toml");
            std::fs::create_dir(dir.path().join("config"))?;
            std::fs::create_dir(dir.path().join("config/genesis"))?;
            std::fs::write(config_path, toml::to_string(&config)?)?;
            std::fs::write(genesis_path, genesis.0.encode_wire()?)?;
            std::fs::write(executor_path, "")?;

            let config_path = dir.path().join("config/config.toml");

            // When

            let (config, genesis) = read_config_and_genesis(&Args {
                config: Some(config_path),
                genesis_manifest_json: None,
                startup: StartupArgs {
                    check_config: false,
                    write_kagemusha_catalog_qualification_seal: None,
                    trace_config: false,
                    config_blake3: None,
                },
                terminal_colors: false,
                language: None,
                sora: false,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
                fastpq_device_class: None,
                fastpq_chip_family: None,
                fastpq_gpu_kind: None,
            })
            .map_err(|report| eyre::eyre!("{report:?}"))?;
            validate_config(&config).map_err(|report| eyre::eyre!("{report:?}"))?;

            // Then

            // No need to check whether genesis.file is resolved - if not, genesis wouldn't be read
            assert!(genesis.is_some());

            assert_eq!(
                config.kura.store_dir.resolve_relative_path().absolutize()?,
                dir.path().join("storage")
            );
            assert_eq!(
                config
                    .snapshot
                    .store_dir
                    .resolve_relative_path()
                    .absolutize()?,
                dir.path().join("snapshots")
            );
            assert_eq!(
                config
                    .dev_telemetry
                    .out_file
                    .expect("dev telemetry should be set")
                    .resolve_relative_path()
                    .absolutize()?,
                dir.path().join("logs/telemetry")
            );

            Ok(())
        }

        fn storage_budget_probe(
            total_bytes: u64,
            available_bytes: u64,
            managed_bytes: u64,
        ) -> StorageBudgetFilesystemProbe {
            StorageBudgetFilesystemProbe {
                filesystem_id: "dev:test".to_owned(),
                path: PathBuf::from("/tmp/nexus-storage"),
                total_bytes,
                available_bytes,
                managed_bytes,
                components: vec![NexusStorageBudgetComponent::Kura],
                managed_roots: Vec::new(),
                derived_budget_bytes: None,
            }
        }

        #[test]
        fn runtime_derived_budget_does_not_mutate_operator_configuration() -> eyre::Result<()> {
            let (mut config, _dir, config_path) =
                parse_config_with_overrides(|table, _genesis_key| {
                    iroha_config::base::toml::Writer::new(table).write(["nexus", "enabled"], true);
                })?;
            let original_config = std::fs::read_to_string(&config_path)?;
            assert!(config.nexus.storage.local_budget_bytes.is_none());
            assert!(config.nexus.storage.effective_local_budget_bytes.is_none());

            let filesystem_budget = NexusStorageFilesystemBudget {
                budget_bytes: NonZeroU64::new(800).expect("non-zero budget"),
                components: vec![NexusStorageBudgetComponent::Kura],
            };
            let aggregate = config
                .apply_derived_storage_budget(&[filesystem_budget])
                .expect("valid filesystem budget");

            assert_eq!(aggregate.get(), 800);
            assert!(config.nexus.storage.local_budget_bytes.is_none());
            assert_eq!(
                config
                    .nexus
                    .storage
                    .effective_local_budget_bytes
                    .map(iroha_config::base::util::Bytes::get),
                Some(800)
            );
            assert_eq!(config.kura.max_disk_usage_bytes.get(), 800);
            assert_eq!(std::fs::read_to_string(config_path)?, original_config);
            Ok(())
        }

        #[cfg(unix)]
        #[test]
        fn runtime_reconciliation_keeps_read_only_key_config_bytes_mode_and_inode()
        -> eyre::Result<()> {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            let (_parsed, _dir, config_path) =
                parse_config_with_overrides(|table, _genesis_key| {
                    iroha_config::base::toml::Writer::new(table).write(["nexus", "enabled"], true);
                })?;
            std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o400))?;
            let original_bytes = std::fs::read(&config_path)?;
            let original_metadata = std::fs::metadata(&config_path)?;
            assert_eq!(original_metadata.mode() & 0o777, 0o400);

            let (config, _genesis) = read_config_and_genesis(&Args {
                config: Some(config_path.clone()),
                genesis_manifest_json: None,
                startup: StartupArgs {
                    check_config: false,
                    write_kagemusha_catalog_qualification_seal: None,
                    trace_config: false,
                    config_blake3: None,
                },
                terminal_colors: false,
                language: None,
                sora: true,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
                fastpq_device_class: None,
                fastpq_chip_family: None,
                fastpq_gpu_kind: None,
            })
            .map_err(|report| eyre::eyre!("{report:?}"))?;
            assert!(
                config.nexus.storage.effective_local_budget_bytes.is_some(),
                "startup must complete runtime reconciliation"
            );

            let final_metadata = std::fs::metadata(&config_path)?;
            assert_eq!(std::fs::read(&config_path)?, original_bytes);
            assert_eq!(final_metadata.mode(), original_metadata.mode());
            assert_eq!(final_metadata.ino(), original_metadata.ino());
            Ok(())
        }

        #[test]
        fn operator_local_budget_initializes_the_effective_budget() -> eyre::Result<()> {
            let (config, _dir, config_path) =
                parse_config_with_overrides(|table, _genesis_key| {
                    iroha_config::base::toml::Writer::new(table)
                        .write(["nexus", "enabled"], true)
                        .write(["nexus", "storage", "local_budget_bytes"], 4_096_i64);
                })?;

            assert_eq!(
                config
                    .nexus
                    .storage
                    .local_budget_bytes
                    .map(iroha_config::base::util::Bytes::get),
                Some(4_096)
            );
            assert_eq!(
                config
                    .nexus
                    .storage
                    .effective_local_budget_bytes
                    .map(iroha_config::base::util::Bytes::get),
                Some(4_096)
            );

            let persisted: toml::Value = toml::from_str(&std::fs::read_to_string(config_path)?)?;
            let storage = persisted
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|nexus| nexus.get("storage"))
                .and_then(toml::Value::as_table)
                .expect("storage table");
            assert_eq!(
                storage
                    .get("local_budget_bytes")
                    .and_then(toml::Value::as_integer),
                Some(4_096)
            );
            assert!(storage.get("effective_local_budget_bytes").is_none());
            assert!(storage.get("auto_default").is_none());
            assert!(storage.get("max_disk_usage_bytes").is_none());
            Ok(())
        }

        #[test]
        fn runtime_budget_uses_checked_capacity_minus_ceil_headroom() {
            let probe = storage_budget_probe(1_001, 301, 0);
            let budgets =
                derive_runtime_nexus_storage_budget(&[probe]).expect("safe derived budget");
            assert_eq!(budgets.len(), 1);
            assert_eq!(budgets[0].budget_bytes.get(), 100);
        }

        #[test]
        fn runtime_budget_is_stable_across_restart_usage_splits() {
            let before_restart = storage_budget_probe(1_000, 700, 100);
            let after_restart = storage_budget_probe(1_000, 300, 500);

            let before = derive_runtime_nexus_storage_budget(&[before_restart])
                .expect("safe pre-restart budget");
            let after = derive_runtime_nexus_storage_budget(&[after_restart])
                .expect("safe post-restart budget");

            assert_eq!(before[0].budget_bytes, after[0].budget_bytes);
            assert_eq!(before[0].budget_bytes.get(), 600);
        }

        #[test]
        fn runtime_budget_rejects_existing_usage_above_the_safe_cap() {
            let probe = storage_budget_probe(1_000, 100, 150);
            let error = derive_runtime_nexus_storage_budget(&[probe])
                .expect_err("auto derivation must not activate a cap below managed usage");
            let diagnostic = format!("{error:?}");
            assert!(
                diagnostic.contains("below the 150 managed bytes"),
                "{diagnostic}"
            );
        }

        #[test]
        fn runtime_budget_rejects_zero_underflow_and_overflow() {
            for probe in [
                storage_budget_probe(1_000, 200, 0),
                storage_budget_probe(1_000, 199, 0),
                storage_budget_probe(1_000, 1, u64::MAX),
            ] {
                assert!(
                    derive_runtime_nexus_storage_budget(&[probe]).is_err(),
                    "invalid capacity arithmetic must fail closed"
                );
            }
        }

        #[cfg(unix)]
        include!("main/runtime_budget_and_config_tests.rs");
    }

    include!("main/startup_tail_tests.rs");
}

/// Result type returned by daemon launcher and startup operations.
pub type ReportResult<T, E> = core::result::Result<T, Report<E>>;
const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(value) => value,
    None => "unknown",
};

const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {
    Some(value) => value,
    None => "unknown",
};

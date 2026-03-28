#[allow(
    dead_code,
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::option_if_let_else,
    clippy::or_fun_call,
    clippy::explicit_auto_deref,
    clippy::unused_async,
    clippy::unnecessary_wraps,
    clippy::too_many_lines,
    clippy::if_not_else
)]
mod genesis_bootstrap;
/// Iroha server command-line interface and node bootstrap entrypoint.
mod i18n;
/// Embedded Soracloud runtime-manager reconciliation.
#[cfg(feature = "embedded-soracloud-runtime")]
#[path = "soracloud_runtime.rs"]
mod soracloud_runtime;
/// No-op Soracloud runtime used when the full embedded runtime is disabled.
#[cfg(not(feature = "embedded-soracloud-runtime"))]
#[path = "soracloud_runtime_stub.rs"]
mod soracloud_runtime;

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    convert::TryFrom,
    env,
    ffi::OsString,
    fs,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::genesis_bootstrap::GenesisBootstrapper;
use crate::soracloud_runtime::{
    QueuedSoracloudRuntimeMutationSink, SoracloudRuntimeManager, SoracloudRuntimeManagerHandle,
};
use clap::Parser;
use error_stack::{Report, ResultExt};
use eyre::Result as EyreResult;
use fastpq_prover::MetalOverrides;
use iroha_config::{
    base::{WithOrigin, read::ConfigReader, util::Emitter},
    parameters::{
        actual::{FastpqExecutionMode, FastpqPoseidonMode, Root as Config},
        user::Root as UserConfig,
    },
};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::{StateTelemetry, StreamingTelemetry};
use iroha_core::{
    IrohaNetwork,
    block::ValidBlock,
    block_sync::{BlockSynchronizer, BlockSynchronizerHandle},
    compliance::LaneComplianceEngine,
    gossiper::{TransactionGossiper, TransactionGossiperHandle},
    governance::manifest::LaneManifestRegistry,
    kiso::KisoHandle,
    kura::Kura,
    panic_hook,
    peers_gossiper::{PeersGossiper, PeersGossiperHandle},
    query::store::LiveQueryStore,
    queue::{ConfigLaneRouter, LaneRouter, Queue, SingleLaneRouter},
    smartcontracts::isi::Registrable as _,
    snapshot::{SnapshotMaker, TryReadError as TryReadSnapshotError, try_read_snapshot},
    state::{State, World, WorldReadOnly},
    streaming::{FilesystemSoranetProvisioner, ManifestPublisher, run_ticket_event_listener},
    sumeragi::{
        GenesisWithPubKey, RbcStoreConfig, SumeragiHandle, SumeragiStartArgs, VotingBlock,
        filter_validators_from_trusted, network_topology::Topology,
    },
};
use iroha_crypto::Algorithm;
use iroha_data_model::nexus::{PublicLaneValidatorRecord, PublicLaneValidatorStatus};
use iroha_data_model::query::{self as dm_query, ErasedIterQuery};
use iroha_data_model::{block::decode_framed_signed_block, prelude::*, transaction::Executable};
use iroha_data_model::{
    isi::RegisterPeerWithPop,
    parameter::system::{
        SumeragiNposParameters, confidential_metadata, consensus_metadata, crypto_metadata,
    },
};
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
use iroha_version::BuildLine;
use mv::storage::StorageReadOnly;
use norito::{codec::Encode, derive::JsonDeserialize, streaming::CapabilityFlags};
use parking_lot::deadlock;
use tokio::{
    sync::{broadcast, mpsc},
    task,
};

#[derive(Clone, Debug, JsonDeserialize)]
struct ConsensusHandshakeMeta {
    mode: String,
    bls_domain: String,
    wire_proto_versions: Vec<u32>,
    consensus_fingerprint: String,
}

fn parse_handshake_meta_str(raw: &str) -> Result<ConsensusHandshakeMeta, norito::Error> {
    norito::json::from_str(raw).map_err(norito::Error::from)
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

fn decode_consensus_handshake_meta(
    payload: &Json,
) -> Result<ConsensusHandshakeMeta, norito::Error> {
    parse_handshake_meta_str(payload.get()).map_err(|_| {
        norito::Error::Message("failed to decode consensus_handshake_meta payload".to_string())
    })
}

fn build_shared_sorafs_provider_cache(
    config: &Config,
) -> Option<Arc<tokio::sync::RwLock<iroha_torii::sorafs::ProviderAdvertCache>>> {
    let discovery = &config.torii.sorafs_discovery;
    if !discovery.discovery_enabled {
        return None;
    }

    let Some(admission_cfg) = discovery.admission.as_ref() else {
        if config.torii.sorafs_gateway.enforce_admission {
            iroha_logger::warn!(
                "torii.sorafs.admission_envelopes_dir not configured; shared SoraFS discovery cache disabled"
            );
        }
        return None;
    };

    let admission =
        match iroha_torii::sorafs::AdmissionRegistry::load_from_dir(&admission_cfg.envelopes_dir) {
            Ok(registry) => Arc::new(registry),
            Err(err) => {
                iroha_logger::error!(
                    ?err,
                    dir = ?admission_cfg.envelopes_dir,
                    "failed to load shared SoraFS provider admission registry"
                );
                return None;
            }
        };

    let mut capabilities = Vec::new();
    for name in &discovery.known_capabilities {
        match iroha_torii::sorafs::parse_capability_name(name) {
            Some(capability) => capabilities.push(capability),
            None => {
                panic!("unknown SoraFS capability `{name}` in torii.sorafs.known_capabilities");
            }
        }
    }

    assert!(
        !capabilities.is_empty(),
        "torii.sorafs.known_capabilities must include at least one capability"
    );

    Some(Arc::new(tokio::sync::RwLock::new(
        iroha_torii::sorafs::ProviderAdvertCache::new(capabilities, admission),
    )))
}

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
        assert_eq!(meta.mode, "Permissioned");

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
            r#"{mode"Permissioned",bls_domain"bls-iroha2:permissioned-sumeragi:v1",consensus_fingerprint"0x632eaff6fe3054ca279416357baae5ff7f28144b3bc6a83921f68d466c4ec0ab"}"#.to_string(),
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
            "MODE=PERMISSIONED bls_domain=bls-iroha2:permissioned-sumeragi:v1 consensus_fingerprint={fingerprint}"
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
        let payload = Json::from_string_unchecked(raw.to_owned());
        let err = decode_crypto_manifest_meta(&payload)
            .expect_err("raw-quoted compatibility payload must be rejected");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_crypto_manifest_meta_rejects_backslash_escaped_object_payload() {
        let raw = r#"{\"allowed_curve_ids\":[1,3,4],\"allowed_signing\":[\"ed25519\",\"secp256k1\",\"bls_normal\"],\"default_hash\":\"blake2b-256\",\"sm2_distid_default\":\"1234567812345678\",\"sm_openssl_preview\":false}"#;
        let payload = Json::from_string_unchecked(raw.to_owned());
        let err = decode_crypto_manifest_meta(&payload)
            .expect_err("backslash-escaped compatibility payload must be rejected");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_crypto_manifest_meta_rejects_mangled_key_value_separators() {
        let raw = r#"{"allowed_curve_ids""[1,3,4],"allowed_signing""["ed25519","secp256k1","bls_normal"],"default_hash""blake2b-256","sm2_distid_default""1234567812345678","sm_openssl_preview"false}"#;
        let payload = Json::from_string_unchecked(raw.to_owned());
        let err = decode_crypto_manifest_meta(&payload)
            .expect_err("mangled compatibility payload must be rejected");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decode_confidential_registry_meta_handles_normal_json() {
        let hash = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let payload = Json::from_string_unchecked(format!("{{\"vk_set_hash\":\"{hash}\"}}"));
        let decoded =
            decode_confidential_registry_meta(&payload).expect("decode confidential payload");
        assert_eq!(decoded.vk_set_hash.as_deref(), Some(hash));
    }

    #[test]
    fn decode_confidential_registry_meta_rejects_mangled_key_value_separators() {
        let hash = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let payload = Json::from_string_unchecked(format!("{{\"vk_set_hash\"\"{hash}\"}}"));
        let err = decode_confidential_registry_meta(&payload)
            .expect_err("mangled compatibility payload must fail");
        assert!(
            err.to_string().contains("failed to decode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_confidential_registry_hash_treats_json_null_as_absent() {
        let payload = Json::from_string_unchecked("{\"vk_set_hash\":null}".to_string());
        let decoded =
            parse_confidential_registry_hash(&payload).expect("decode null confidential payload");
        assert_eq!(decoded, None);
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

fn read_genesis_manifest(path: &Path) -> ReportResult<RawGenesisTransaction, StartError> {
    let bytes = std::fs::read(path)
        .change_context(StartError::InitKura)
        .attach_with(|| format!("failed to read genesis manifest JSON at {}", path.display()))?;
    norito::json::from_slice(&bytes).map_err(|err| {
        Report::new(StartError::InitKura).attach(format!(
            "failed to parse genesis manifest JSON at {}: {err}",
            path.display()
        ))
    })
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
#[derive(Parser, Debug)]
#[command(
    name = "irohad",
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
    /// Enables trace logs of configuration reading & parsing.
    ///
    /// Might be useful for configuration troubleshooting.
    #[arg(long, env)]
    pub trace_config: bool,
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
    /// Override FASTPQ prover execution mode (`auto`, `cpu`, or `gpu`).
    #[arg(
        long = "fastpq-execution-mode",
        value_name = "MODE",
        value_parser = parse_fastpq_execution_mode
    )]
    pub fastpq_execution_mode: Option<FastpqExecutionMode>,
    /// Override the FASTPQ Poseidon pipeline mode (`auto`, `cpu`, or `gpu`).
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

#[derive(Debug)]
enum MainError {
    TraceConfigSetup,
    Config,
    Logger,
    IrohaStart,
    IrohaRun,
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
}

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
    block_sync: BlockSynchronizerHandle,
    tx_gossiper: TransactionGossiperHandle,
    peers_gossiper: PeersGossiperHandle,
    network: IrohaNetwork,
    streaming: iroha_core::streaming::StreamingHandle,
    kiso: KisoHandle,
    #[allow(dead_code)]
    suppress_pow_broadcast: Arc<AtomicBool>,
    pow_update_version: Arc<AtomicU64>,
    consensus_ingress: ConsensusIngressLimiter,
    low_priority_ingress: LowPriorityIngressLimiter,
}

struct NetworkRelayShared {
    sumeragi: SumeragiHandle,
    block_sync: BlockSynchronizerHandle,
    tx_gossiper: TransactionGossiperHandle,
    peers_gossiper: PeersGossiperHandle,
    network: IrohaNetwork,
    streaming: iroha_core::streaming::StreamingHandle,
    kiso: KisoHandle,
    suppress_pow_broadcast: Arc<AtomicBool>,
    pow_update_version: Arc<AtomicU64>,
    consensus_ingress: Mutex<ConsensusIngressLimiter>,
    low_priority_ingress: Mutex<LowPriorityIngressLimiter>,
}

type RelayWorkItem = iroha_p2p::peer::message::PeerMessage<iroha_core::NetworkMessage>;

/// Maximum number of consecutive high-priority messages before yielding to low-priority work.
const RELAY_HIGH_BURST: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsensusIngressDropReason {
    Rate,
    Bytes,
    RbcSessionLimit,
    Penalty,
}

impl ConsensusIngressDropReason {
    fn label(self) -> &'static str {
        match self {
            Self::Rate => "rate",
            Self::Bytes => "bytes",
            Self::RbcSessionLimit => "rbc_session_limit",
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
    rbc_session_limit: usize,
    rbc_session_ttl: Duration,
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
    rbc_sessions: HashMap<iroha_core::sumeragi::rbc_store::SessionKey, Instant>,
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
    apply_rbc_session_limit: bool,
}

impl IngressPolicy {
    const fn limited() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Limited),
            apply_penalty: true,
            apply_rbc_session_limit: true,
        }
    }

    const fn bulk() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Bulk),
            apply_penalty: false,
            apply_rbc_session_limit: true,
        }
    }

    const fn critical() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Critical),
            apply_penalty: false,
            apply_rbc_session_limit: false,
        }
    }

    const fn critical_with_rbc_sessions() -> Self {
        Self {
            rate_class: Some(IngressRateClass::Critical),
            apply_penalty: false,
            apply_rbc_session_limit: true,
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
                BlockMessage::QcVote(_)
                | BlockMessage::Qc(_)
                | BlockMessage::VrfCommit(_)
                | BlockMessage::VrfReveal(_)
                | BlockMessage::FetchBlockBody(_)
                | BlockMessage::FetchPendingBlock(_)
                | BlockMessage::ProposalHint(_)
                | BlockMessage::Proposal(_)
                | BlockMessage::BlockCreated(_) => IngressPolicy::critical(),
                BlockMessage::RbcInit(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_) => IngressPolicy::critical_with_rbc_sessions(),
                BlockMessage::RbcChunk(_)
                | BlockMessage::RbcChunkCompact(_)
                | BlockMessage::BlockBodyResponse(_) => IngressPolicy::bulk(),
                BlockMessage::ConsensusParams(_) => IngressPolicy::limited(),
                BlockMessage::BlockSyncUpdate(_) | BlockMessage::ExecWitness(_) => {
                    IngressPolicy::bulk()
                }
            },
            iroha_core::NetworkMessage::SumeragiControlFlow(_) => IngressPolicy::critical(),
            iroha_core::NetworkMessage::BlockSync(_) => IngressPolicy::bulk(),
            _ => IngressPolicy::limited(),
        }
    }

    fn from_config(
        network: &iroha_config::parameters::actual::Network,
        sumeragi: &iroha_config::parameters::actual::Sumeragi,
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
        let bulk_scale = Self::bulk_scale_factor(Duration::from_millis(
            iroha_config::parameters::defaults::sumeragi::BLOCK_TIME_MS,
        ));
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
        let rbc_session_limit =
            Self::resolve_rbc_session_limit(network.consensus_ingress_rbc_session_limit, sumeragi);
        Self::new(
            msg_rate,
            bytes_rate,
            bulk_msg_rate,
            bulk_bytes_rate,
            critical_msg_rate,
            critical_bytes_rate,
            rbc_session_limit,
            sumeragi.rbc.session_ttl,
            penalty,
        )
    }

    fn bulk_scale_factor(block_time: Duration) -> u32 {
        let base_ms =
            u128::from(iroha_config::parameters::defaults::sumeragi::BLOCK_TIME_MS).max(1);
        let block_ms = block_time.as_millis().max(1);
        let scale = base_ms.div_ceil(block_ms);
        u32::try_from(scale).unwrap_or(u32::MAX).max(1)
    }

    fn resolve_rbc_session_limit(
        configured: usize,
        sumeragi: &iroha_config::parameters::actual::Sumeragi,
    ) -> usize {
        if configured == 0 || sumeragi.rbc.session_ttl.is_zero() {
            return configured;
        }
        let block_time = std::time::Duration::from_millis(
            iroha_config::parameters::defaults::sumeragi::BLOCK_TIME_MS,
        );
        Self::rbc_session_limit_from_ttl(configured, sumeragi.rbc.session_ttl, block_time)
    }

    fn rbc_session_limit_from_ttl(configured: usize, ttl: Duration, block_time: Duration) -> usize {
        // Scale the cap to cover fast pipelines without dropping in-flight RBC sessions.
        if configured == 0 || ttl.is_zero() {
            return configured;
        }
        let block_ms = block_time.as_millis().max(1);
        let ttl_ms = ttl.as_millis().max(1);
        let expected = usize::try_from(ttl_ms / block_ms).unwrap_or(usize::MAX);
        let padded = expected.saturating_add(1).saturating_mul(2).max(1);
        configured.max(padded)
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        msg_rate: Option<BucketConfig>,
        bytes_rate: Option<BucketConfig>,
        bulk_msg_rate: Option<BucketConfig>,
        bulk_bytes_rate: Option<BucketConfig>,
        critical_msg_rate: Option<BucketConfig>,
        critical_bytes_rate: Option<BucketConfig>,
        rbc_session_limit: usize,
        rbc_session_ttl: Duration,
        penalty: PenaltyConfig,
    ) -> Self {
        Self {
            msg_rate,
            bytes_rate,
            bulk_msg_rate,
            bulk_bytes_rate,
            critical_msg_rate,
            critical_bytes_rate,
            rbc_session_limit,
            rbc_session_ttl,
            penalty,
            peers: HashMap::new(),
        }
    }

    fn should_drop(
        &mut self,
        peer: &Peer,
        msg: &iroha_core::NetworkMessage,
        size_bytes: usize,
    ) -> Option<ConsensusIngressDropReason> {
        let policy = Self::ingress_policy(msg);
        let apply_penalty = policy.apply_penalty
            || (policy.rate_class == Some(IngressRateClass::Critical)
                && self.critical_msg_rate.is_none()
                && self.critical_bytes_rate.is_none());
        let now = Instant::now();
        let entry = self.peers.entry(peer.id().clone()).or_insert_with(|| {
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
        if policy.apply_rbc_session_limit
            && let Some(key) = Self::rbc_session_key(msg)
            && self.rbc_session_limit > 0
        {
            entry.prune_rbc_sessions(now, self.rbc_session_ttl);
            if !entry.rbc_sessions.contains_key(&key)
                && entry.rbc_sessions.len() >= self.rbc_session_limit
            {
                if apply_penalty {
                    entry.penalty.note_violation(now);
                }
                return Some(ConsensusIngressDropReason::RbcSessionLimit);
            }
            entry.rbc_sessions.insert(key, now);
        }
        None
    }

    fn rbc_session_key(
        msg: &iroha_core::NetworkMessage,
    ) -> Option<iroha_core::sumeragi::rbc_store::SessionKey> {
        use iroha_core::sumeragi::message::BlockMessage::*;

        let iroha_core::NetworkMessage::SumeragiBlock(block) = msg else {
            return None;
        };
        match block.as_ref().as_ref() {
            RbcInit(init) => Some((init.block_hash, init.height, init.view)),
            RbcChunk(chunk) => Some((chunk.block_hash, chunk.height, chunk.view)),
            RbcChunkCompact(chunk) => Some((
                chunk.block_hash,
                u64::from(chunk.height),
                u64::from(chunk.view),
            )),
            RbcReady(ready) => Some((ready.block_hash, ready.height, ready.view)),
            RbcDeliver(deliver) => Some((deliver.block_hash, deliver.height, deliver.view)),
            _ => None,
        }
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
            rbc_sessions: HashMap::new(),
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

    fn prune_rbc_sessions(&mut self, now: Instant, ttl: Duration) {
        if ttl.is_zero() {
            self.rbc_sessions.clear();
            return;
        }
        self.rbc_sessions
            .retain(|_, seen| now.saturating_duration_since(*seen) <= ttl);
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

    fn should_drop(
        &mut self,
        peer: &Peer,
        size_bytes: usize,
    ) -> Option<LowPriorityIngressDropReason> {
        if self.msg_rate.is_none() && self.bytes_rate.is_none() {
            return None;
        }
        let now = Instant::now();
        let entry = self
            .peers
            .entry(peer.id().clone())
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

fn try_recv_after_burst<T>(
    receiver: &mut mpsc::Receiver<T>,
    high_budget: &mut usize,
    high_burst: usize,
) -> Option<T> {
    if *high_budget != 0 {
        return None;
    }
    let msg = receiver.try_recv().ok();
    *high_budget = high_burst;
    msg
}

impl NetworkRelay {
    fn into_shared(self) -> NetworkRelayShared {
        NetworkRelayShared {
            sumeragi: self.sumeragi,
            block_sync: self.block_sync,
            tx_gossiper: self.tx_gossiper,
            peers_gossiper: self.peers_gossiper,
            network: self.network,
            streaming: self.streaming,
            kiso: self.kiso,
            suppress_pow_broadcast: self.suppress_pow_broadcast,
            pow_update_version: self.pow_update_version,
            consensus_ingress: Mutex::new(self.consensus_ingress),
            low_priority_ingress: Mutex::new(self.low_priority_ingress),
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn run(self) {
        use iroha_p2p::network::{SubscriberFilter, message::Topic};

        let shared = Arc::new(self.into_shared());
        let base_cap = shared.network.subscriber_queue_cap().get();
        let high_cap = base_cap.saturating_mul(4).max(base_cap);
        let payload_cap = base_cap.saturating_mul(2).max(base_cap);
        let chunk_cap = base_cap;
        let low_cap = base_cap;
        let (high_sender, mut high_receiver) = mpsc::channel(high_cap);
        let (payload_sender, mut payload_receiver) = mpsc::channel(payload_cap);
        let (chunk_sender, mut chunk_receiver) = mpsc::channel(chunk_cap);
        let (low_sender, mut low_receiver) = mpsc::channel(low_cap);
        let work_high_cap = high_cap.saturating_mul(2);
        let work_payload_cap = payload_cap.saturating_mul(2);
        let work_chunk_cap = chunk_cap;
        let work_low_cap = low_cap;
        let (work_high_tx, mut work_high_rx) = mpsc::channel::<RelayWorkItem>(work_high_cap);
        let (work_payload_tx, mut work_payload_rx) =
            mpsc::channel::<RelayWorkItem>(work_payload_cap);
        let (work_chunk_tx, mut work_chunk_rx) = mpsc::channel::<RelayWorkItem>(work_chunk_cap);
        let (work_low_tx, mut work_low_rx) = mpsc::channel::<RelayWorkItem>(work_low_cap);
        let worker_limit = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .clamp(1, 8);
        let worker_sem = Arc::new(tokio::sync::Semaphore::new(worker_limit));
        let shared_for_workers = Arc::clone(&shared);
        let worker_sem_for_workers = Arc::clone(&worker_sem);
        tokio::spawn(async move {
            loop {
                let msg = tokio::select! {
                    biased;
                    Some(msg) = work_high_rx.recv() => Some(msg),
                    Some(msg) = work_payload_rx.recv() => Some(msg),
                    Some(msg) = work_chunk_rx.recv() => Some(msg),
                    Some(msg) = work_low_rx.recv() => Some(msg),
                    else => None,
                };
                let Some(msg) = msg else {
                    break;
                };
                let permit = match worker_sem_for_workers.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => break,
                };
                let shared = Arc::clone(&shared_for_workers);
                tokio::spawn(async move {
                    shared
                        .handle_message(msg.peer, msg.payload, msg.payload_bytes)
                        .await;
                    drop(permit);
                });
            }
        });

        let high_filter = SubscriberFilter::topics([Topic::Consensus, Topic::Control]);
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

        let mut high_sender = Some(high_sender);
        let mut payload_sender = Some(payload_sender);
        let mut chunk_sender = Some(chunk_sender);
        let mut low_sender = Some(low_sender);
        loop {
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
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Ensure payload, chunk, and low-priority queues make progress without stalling consensus traffic.
        let mut high_budget = RELAY_HIGH_BURST;
        let mut high_drops: u64 = 0;
        let mut payload_drops: u64 = 0;
        let mut chunk_drops: u64 = 0;
        let mut low_drops: u64 = 0;
        loop {
            if let Some(msg) =
                try_recv_after_burst(&mut payload_receiver, &mut high_budget, RELAY_HIGH_BURST)
            {
                match work_payload_tx.try_send(msg) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(msg)) => {
                        payload_drops = payload_drops.saturating_add(1);
                        if payload_drops == 1 || payload_drops.is_multiple_of(1024) {
                            iroha_logger::warn!(
                                peer = %msg.peer,
                                topic = ?msg.payload.topic(),
                                drops = payload_drops,
                                "relay work queue full; dropping payload message"
                            );
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                }
                continue;
            }
            if let Some(msg) =
                try_recv_after_burst(&mut chunk_receiver, &mut high_budget, RELAY_HIGH_BURST)
            {
                match work_chunk_tx.try_send(msg) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(msg)) => {
                        chunk_drops = chunk_drops.saturating_add(1);
                        if chunk_drops == 1 || chunk_drops.is_multiple_of(1024) {
                            iroha_logger::warn!(
                                peer = %msg.peer,
                                topic = ?msg.payload.topic(),
                                drops = chunk_drops,
                                "relay work queue full; dropping chunk message"
                            );
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                }
                continue;
            }
            if let Some(msg) =
                try_recv_after_burst(&mut low_receiver, &mut high_budget, RELAY_HIGH_BURST)
            {
                match work_low_tx.try_send(msg) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(msg)) => {
                        low_drops = low_drops.saturating_add(1);
                        if low_drops == 1 || low_drops.is_multiple_of(1024) {
                            iroha_logger::warn!(
                                peer = %msg.peer,
                                topic = ?msg.payload.topic(),
                                drops = low_drops,
                                "relay work queue full; dropping low-priority message"
                            );
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                }
                continue;
            }
            tokio::select! {
                biased;
                Some(msg) = high_receiver.recv() => {
                    high_budget = high_budget.saturating_sub(1);
                    match work_high_tx.try_send(msg) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(msg)) => {
                            high_drops = high_drops.saturating_add(1);
                            if high_drops == 1 || high_drops.is_multiple_of(1024) {
                                iroha_logger::warn!(
                                    peer = %msg.peer,
                                    topic = ?msg.payload.topic(),
                                    drops = high_drops,
                                    "relay work queue full; dropping high-priority message"
                                );
                            }
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                Some(msg) = payload_receiver.recv() => {
                    high_budget = RELAY_HIGH_BURST;
                    match work_payload_tx.try_send(msg) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(msg)) => {
                            payload_drops = payload_drops.saturating_add(1);
                            if payload_drops == 1 || payload_drops.is_multiple_of(1024) {
                                iroha_logger::warn!(
                                    peer = %msg.peer,
                                    topic = ?msg.payload.topic(),
                                    drops = payload_drops,
                                    "relay work queue full; dropping payload message"
                                );
                            }
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                Some(msg) = chunk_receiver.recv() => {
                    high_budget = RELAY_HIGH_BURST;
                    match work_chunk_tx.try_send(msg) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(msg)) => {
                            chunk_drops = chunk_drops.saturating_add(1);
                            if chunk_drops == 1 || chunk_drops.is_multiple_of(1024) {
                                iroha_logger::warn!(
                                    peer = %msg.peer,
                                    topic = ?msg.payload.topic(),
                                    drops = chunk_drops,
                                    "relay work queue full; dropping chunk message"
                                );
                            }
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                Some(msg) = low_receiver.recv() => {
                    high_budget = RELAY_HIGH_BURST;
                    match work_low_tx.try_send(msg) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(msg)) => {
                            low_drops = low_drops.saturating_add(1);
                            if low_drops == 1 || low_drops.is_multiple_of(1024) {
                                iroha_logger::warn!(
                                    peer = %msg.peer,
                                    topic = ?msg.payload.topic(),
                                    drops = low_drops,
                                    "relay work queue full; dropping low-priority message"
                                );
                            }
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                else => {
                    break;
                }
            }
        }
        iroha_logger::debug!("Exiting the network relay");
    }
}

impl NetworkRelayShared {
    #[allow(clippy::too_many_lines)]
    async fn handle_message(&self, peer: Peer, msg: iroha_core::NetworkMessage, size_bytes: usize) {
        use iroha_core::NetworkMessage::*;

        if matches!(
            &msg,
            SumeragiBlock(_) | SumeragiControlFlow(_) | BlockSync(_)
        ) {
            let reason = {
                let mut limiter = self
                    .consensus_ingress
                    .lock()
                    .expect("consensus ingress mutex poisoned");
                limiter.should_drop(&peer, &msg, size_bytes)
            };
            if let Some(reason) = reason {
                #[cfg(feature = "telemetry")]
                if let Some(metrics) = iroha_telemetry::metrics::global()
                    && let Some(topic) = Self::consensus_ingress_topic_label(&msg)
                {
                    metrics
                        .consensus_ingress_drop_total
                        .with_label_values(&[topic, reason.label()])
                        .inc();
                }
                let (kind, height, view) = match &msg {
                    SumeragiBlock(data) => Self::block_message_meta(data.as_ref().as_ref()),
                    SumeragiControlFlow(data) => Self::control_flow_meta(data.as_ref()),
                    BlockSync(data) => {
                        let label = match data.as_ref() {
                            iroha_core::block_sync::message::Message::GetBlocksAfter(_) => {
                                "BlockSyncRequest"
                            }
                            iroha_core::block_sync::message::Message::ShareBlocks(_) => {
                                "BlockSyncResponse"
                            }
                        };
                        (label, None, None)
                    }
                    _ => ("Other", None, None),
                };
                iroha_logger::debug!(
                    %peer,
                    ?height,
                    ?view,
                    size_bytes,
                    kind,
                    reason = reason.label(),
                    "dropping inbound consensus message due to ingress limits"
                );
                return;
            }
        }

        if Self::should_apply_low_priority_ingress(&msg) {
            let reason = {
                let mut limiter = self
                    .low_priority_ingress
                    .lock()
                    .expect("low-priority ingress mutex poisoned");
                limiter.should_drop(&peer, size_bytes)
            };
            if let Some(reason) = reason {
                iroha_logger::debug!(
                    %peer,
                    size_bytes,
                    reason = reason.label(),
                    "dropping inbound low-priority message due to ingress limits"
                );
                return;
            }
        }

        match msg {
            SumeragiBlock(data) => {
                let (kind, height, view) = Self::block_message_meta(data.as_ref().as_ref());
                iroha_logger::debug!(
                    %peer,
                    ?height,
                    ?view,
                    size_bytes,
                    kind,
                    "relay received sumeragi block message"
                );
                let sender = peer.id().clone();
                let sumeragi = self.sumeragi.clone();
                let msg = (*data).into_message();
                if sumeragi_block_message_requires_blocking(&msg) {
                    let handle = tokio::task::spawn_blocking(move || {
                        sumeragi.incoming_block_message_from(sender, msg);
                    });
                    if let Err(err) = handle.await {
                        iroha_logger::warn!(?err, "blocking sumeragi ingress task aborted");
                    }
                } else {
                    let _ = sumeragi.try_incoming_block_message_from(sender, msg);
                }
            }
            SumeragiControlFlow(data) => {
                let (kind, height, view) = Self::control_flow_meta(data.as_ref());
                iroha_logger::debug!(
                    %peer,
                    ?height,
                    ?view,
                    size_bytes,
                    kind,
                    "relay received sumeragi control-flow message"
                );
                let _ = self
                    .sumeragi
                    .try_incoming_consensus_control_flow_message(*data);
            }
            LaneRelay(envelope) => {
                let _ = self.sumeragi.try_incoming_lane_relay(*envelope);
            }
            MergeCommitteeSignature(signature) => {
                let _ = self.sumeragi.try_incoming_merge_signature(*signature);
            }
            StreamingControl(frame) => {
                if let Err(err) = self.streaming.process_control_frame(&peer, frame.as_ref()) {
                    iroha_logger::warn!(%peer, ?err, "Failed to process streaming control frame");
                }
            }
            BlockSync(data) => {
                let Some(block_sync) = Self::sanitize_block_sync_message(&peer, *data) else {
                    return;
                };
                match &block_sync {
                    iroha_core::block_sync::message::Message::GetBlocksAfter(get) => {
                        iroha_logger::debug!(
                            %peer,
                            from = %get.peer_id,
                            prev = ?get.prev_hash,
                            latest = ?get.latest_hash,
                            seen = get.seen_blocks.len(),
                            "relay received block sync request"
                        );
                    }
                    iroha_core::block_sync::message::Message::ShareBlocks(share) => {
                        iroha_logger::debug!(
                            %peer,
                            from = %share.peer_id,
                            count = share.blocks.len(),
                            first_height = share.blocks.first().map(|b| b.header().height().get()),
                            last_height = share.blocks.last().map(|b| b.header().height().get()),
                            "relay received block sync response"
                        );
                    }
                }
                self.block_sync.message(block_sync);
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
            SoranetPowConfig(bytes) => {
                self.apply_remote_pow_update(&bytes).await;
            }
            SoracloudLocalReadProxyRequest(_)
            | SoracloudLocalReadProxyResponse(_)
            | ToriiProxyRequest(_)
            | ToriiProxyResponse(_)
            | GenesisRequest(_)
            | GenesisResponse(_)
            | Health
            | Connect(_) => {
                // Genesis bootstrap is handled by the dedicated bootstrapper listener.
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

    #[cfg(feature = "telemetry")]
    fn consensus_ingress_topic_label(msg: &iroha_core::NetworkMessage) -> Option<&'static str> {
        use iroha_p2p::network::message::Topic;

        match msg.topic() {
            Topic::ConsensusPayload => Some("ConsensusPayload"),
            Topic::ConsensusChunk => Some("ConsensusChunk"),
            Topic::BlockSync => Some("BlockSync"),
            _ => None,
        }
    }

    fn sanitize_block_sync_message(
        peer: &Peer,
        msg: iroha_core::block_sync::message::Message,
    ) -> Option<iroha_core::block_sync::message::Message> {
        use iroha_core::block_sync::message::Message::{GetBlocksAfter, ShareBlocks};

        match msg {
            GetBlocksAfter(mut get) => {
                if get.peer_id != *peer.id() {
                    iroha_logger::warn!(
                        %peer,
                        declared = %get.peer_id,
                        "dropping block sync request with mismatched peer_id"
                    );
                    return None;
                }
                get.peer_id = peer.id().clone();
                Some(GetBlocksAfter(get))
            }
            ShareBlocks(mut share) => {
                if share.peer_id != *peer.id() {
                    iroha_logger::warn!(
                        %peer,
                        declared = %share.peer_id,
                        "dropping block sync response with mismatched peer_id"
                    );
                    return None;
                }
                share.peer_id = peer.id().clone();
                Some(ShareBlocks(share))
            }
        }
    }

    fn block_message_meta(
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
            ConsensusParams(_) => ("ConsensusParams", None, None),
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
            RbcChunk(chunk) => ("RbcChunk", Some(chunk.height), Some(chunk.view)),
            RbcChunkCompact(chunk) => (
                "RbcChunk",
                Some(u64::from(chunk.height)),
                Some(u64::from(chunk.view)),
            ),
            RbcReady(ready) => ("RbcReady", Some(ready.height), Some(ready.view)),
            RbcDeliver(deliver) => ("RbcDeliver", Some(deliver.height), Some(deliver.view)),
            FetchBlockBody(request) => ("FetchBlockBody", Some(request.height), Some(request.view)),
            BlockBodyResponse(response) => (
                "BlockBodyResponse",
                Some(response.height),
                Some(response.view),
            ),
            FetchPendingBlock(_request) => ("FetchPendingBlock", None, None),
            ProposalHint(hint) => ("ProposalHint", Some(hint.height), Some(hint.view)),
            Proposal(proposal) => (
                "Proposal",
                Some(proposal.header.height),
                Some(proposal.header.view),
            ),
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

    fn pow_summary_matches_broadcast(
        current: &iroha_config::client_api::SoranetHandshakePowSummary,
        update: &iroha_core::SoranetPowConfigBroadcast,
    ) -> bool {
        let puzzle_matches = match (current.puzzle, update.puzzle) {
            (None, None) => true,
            (Some(current), Some(update)) => {
                current.memory_kib == update.memory_kib
                    && current.time_cost == update.time_cost
                    && current.lanes == update.lanes
            }
            _ => false,
        };
        current.required == update.required
            && current.difficulty == update.difficulty
            && current.max_future_skew_secs == update.max_future_skew_secs
            && current.min_ticket_ttl_secs == update.min_ticket_ttl_secs
            && current.ticket_ttl_secs == update.ticket_ttl_secs
            && puzzle_matches
    }

    async fn apply_remote_pow_update(&self, bytes: &[u8]) {
        iroha_logger::debug!(payload_len = bytes.len(), "Received PoW update payload");
        let Ok(update) = norito::json::from_slice::<iroha_core::SoranetPowConfigBroadcast>(bytes)
        else {
            iroha_logger::warn!("Failed to decode SoraNet PoW config broadcast; ignoring");
            return;
        };
        let mut logger = iroha_config::client_api::Logger {
            level: iroha_logger::Level::INFO,
            filter: None,
        };
        let mut matches_current = false;
        match self.kiso.get_dto().await {
            Ok(dto) => {
                matches_current = Self::pow_summary_matches_broadcast(
                    &dto.network.soranet_handshake.pow,
                    &update,
                );
                logger = dto.logger;
            }
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    "Falling back to INFO logger while applying remote PoW update"
                );
            }
        };

        let observed_version = self.pow_update_version.load(Ordering::SeqCst);
        if update.version < observed_version {
            iroha_logger::debug!(
                incoming_version = update.version,
                local_version = observed_version,
                "Ignoring stale PoW update version"
            );
            return;
        }
        if update.version == observed_version {
            if !matches_current {
                iroha_logger::warn!(
                    incoming_version = update.version,
                    local_version = observed_version,
                    "Ignoring conflicting PoW update with equal version"
                );
            }
            iroha_logger::debug!(
                incoming_version = update.version,
                local_version = observed_version,
                "PoW update version already applied; skipping"
            );
            return;
        }
        if matches_current {
            let _ = self.pow_update_version.compare_exchange(
                observed_version,
                update.version,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            iroha_logger::debug!(
                incoming_version = update.version,
                local_version = observed_version,
                "PoW config already matches; advancing version only"
            );
            return;
        }
        if self
            .pow_update_version
            .compare_exchange(
                observed_version,
                update.version,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            iroha_logger::debug!(
                incoming_version = update.version,
                local_version = self.pow_update_version.load(Ordering::SeqCst),
                "Skipping PoW update after local version changed concurrently"
            );
            return;
        }

        let puzzle = match update.puzzle {
            Some(p) => Some(iroha_config::client_api::SoranetHandshakePuzzleUpdate {
                enabled: Some(true),
                memory_kib: Some(p.memory_kib),
                time_cost: Some(p.time_cost),
                lanes: Some(p.lanes),
            }),
            None => Some(iroha_config::client_api::SoranetHandshakePuzzleUpdate {
                enabled: Some(false),
                memory_kib: None,
                time_cost: None,
                lanes: None,
            }),
        };
        // Remote updates should not trigger another rebroadcast from this peer.
        self.suppress_pow_broadcast.store(true, Ordering::SeqCst);
        if let Err(err) = self
            .kiso
            .update_with_dto(iroha_config::client_api::ConfigUpdateDTO {
                logger,
                network_acl: None,
                network: None,
                confidential_gas: None,
                soranet_handshake: Some(iroha_config::client_api::SoranetHandshakeUpdate {
                    descriptor_commit_hex: None,
                    client_capabilities_hex: None,
                    relay_capabilities_hex: None,
                    kem_id: None,
                    sig_id: None,
                    resume_hash_hex: None,
                    pow: Some(iroha_config::client_api::SoranetHandshakePowUpdate {
                        required: Some(update.required),
                        difficulty: Some(update.difficulty),
                        max_future_skew_secs: Some(update.max_future_skew_secs),
                        min_ticket_ttl_secs: Some(update.min_ticket_ttl_secs),
                        ticket_ttl_secs: Some(update.ticket_ttl_secs),
                        puzzle,
                        signed_ticket_public_key_hex: None,
                    }),
                }),
                transport: None,
                compute_pricing: None,
            })
            .await
        {
            self.suppress_pow_broadcast.store(false, Ordering::SeqCst);
            let _ = self.pow_update_version.compare_exchange(
                update.version,
                observed_version,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            iroha_logger::warn!(?err, "Failed to apply remote PoW configuration update");
        }
    }
}

#[cfg(test)]
fn enqueue_sumeragi_block_message<F>(msg: iroha_core::sumeragi::message::BlockMessage, enqueue: F)
where
    F: FnOnce(iroha_core::sumeragi::message::BlockMessage) + Send + 'static,
{
    enqueue(msg);
}

fn sumeragi_block_message_requires_blocking(
    msg: &iroha_core::sumeragi::message::BlockMessage,
) -> bool {
    use iroha_core::sumeragi::message::BlockMessage;

    matches!(
        msg,
        BlockMessage::BlockSyncUpdate(_)
            | BlockMessage::BlockCreated(_)
            | BlockMessage::Proposal(_)
            | BlockMessage::QcVote(_)
            | BlockMessage::Qc(_)
            | BlockMessage::RbcInit(_)
            | BlockMessage::RbcReady(_)
            | BlockMessage::RbcDeliver(_)
    )
}

#[cfg(test)]
mod network_relay_tests {
    use std::{borrow::Cow, collections::BTreeSet, time::Duration};

    use iroha_config::{
        client_api::{SoranetHandshakePowSummary, SoranetHandshakePuzzleSummary},
        parameters::actual::{SoranetPow, SoranetPuzzle},
    };
    use iroha_core::{
        SoranetPowConfigBroadcast, SoranetPuzzleConfigBroadcast,
        block::BlockBuilder,
        block_sync::message::{GetBlocksAfter, Message as BlockSyncMessage, ShareBlocks},
        sumeragi::{
            consensus::{
                ConsensusBlockHeader, Evidence, EvidenceKind, EvidencePayload, ExecWitness,
                ExecWitnessMsg, Phase, Proposal, QcHeaderRef, RbcInit,
            },
            message::{
                BlockMessage, BlockMessageWire, BlockSyncUpdate, ConsensusParamsAdvert, ControlFlow,
            },
        },
        tx::AcceptedTransaction,
    };
    use iroha_crypto::{Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        AccountId, ChainId, Level,
        block::{BlockHeader, BlockSignature, SignedBlock},
        isi::Log,
        peer::{Peer, PeerId},
        transaction::TransactionBuilder,
    };

    use super::{
        BucketConfig, ConsensusIngressDropReason, ConsensusIngressLimiter,
        LowPriorityIngressDropReason, LowPriorityIngressLimiter, NetworkRelayShared, PenaltyConfig,
        enqueue_sumeragi_block_message, pow_update_payload,
        sumeragi_block_message_requires_blocking,
    };

    fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let keypair = KeyPair::random();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    #[test]
    fn relay_enqueue_drops_when_queue_is_full() {
        let (tx, rx) = std::sync::mpsc::sync_channel(0);
        let keypair = KeyPair::random();
        let new_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .sign(keypair.private_key())
            .unpack(|_| {});
        let signed = iroha_data_model::block::SignedBlock::from(new_block);
        let update = BlockSyncUpdate::from(&signed);
        let msg = BlockMessage::BlockSyncUpdate(update);

        enqueue_sumeragi_block_message(msg, move |msg| {
            let _ = tx.try_send(msg);
        });

        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty | std::sync::mpsc::TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn sumeragi_block_message_requires_blocking_matches_expected_variants() {
        let signed = signed_block_for_test();
        let created =
            BlockMessage::BlockCreated(iroha_core::sumeragi::message::BlockCreated::from(&signed));
        assert!(sumeragi_block_message_requires_blocking(&created));

        let init = BlockMessage::RbcInit(RbcInit {
            block_hash: signed.hash(),
            height: signed.header().height().get(),
            view: signed.header().view_change_index(),
            epoch: 0,
            roster: vec![PeerId::new(KeyPair::random().public_key().clone())],
            roster_hash: Hash::prehashed([0x11; 32]),
            total_chunks: 1,
            chunk_digests: vec![[0x22; 32]],
            payload_hash: Hash::prehashed([0x33; 32]),
            chunk_root: Hash::prehashed([0x44; 32]),
            block_header: signed.header().clone(),
            leader_signature: signed
                .signatures()
                .next()
                .expect("signed block has signature")
                .clone(),
        });
        assert!(sumeragi_block_message_requires_blocking(&init));

        let advert = ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        let params = BlockMessage::ConsensusParams(advert);
        assert!(!sumeragi_block_message_requires_blocking(&params));
    }

    #[test]
    fn pow_broadcast_match_detects_exact_match() {
        let summary = SoranetHandshakePowSummary {
            required: true,
            difficulty: 7,
            max_future_skew_secs: 900,
            min_ticket_ttl_secs: 120,
            ticket_ttl_secs: 240,
            puzzle: Some(SoranetHandshakePuzzleSummary {
                memory_kib: 131_072,
                time_cost: 3,
                lanes: 2,
            }),
            signed_ticket_public_key_hex: None,
        };
        let broadcast = SoranetPowConfigBroadcast {
            version: 1,
            required: true,
            difficulty: 7,
            max_future_skew_secs: 900,
            min_ticket_ttl_secs: 120,
            ticket_ttl_secs: 240,
            puzzle: Some(SoranetPuzzleConfigBroadcast {
                memory_kib: 131_072,
                time_cost: 3,
                lanes: 2,
            }),
        };

        assert!(NetworkRelayShared::pow_summary_matches_broadcast(
            &summary, &broadcast
        ));
    }

    #[test]
    fn pow_broadcast_match_rejects_puzzle_mismatch() {
        let summary = SoranetHandshakePowSummary {
            required: true,
            difficulty: 7,
            max_future_skew_secs: 900,
            min_ticket_ttl_secs: 120,
            ticket_ttl_secs: 240,
            puzzle: Some(SoranetHandshakePuzzleSummary {
                memory_kib: 131_072,
                time_cost: 3,
                lanes: 2,
            }),
            signed_ticket_public_key_hex: None,
        };
        let broadcast = SoranetPowConfigBroadcast {
            version: 1,
            required: true,
            difficulty: 7,
            max_future_skew_secs: 900,
            min_ticket_ttl_secs: 120,
            ticket_ttl_secs: 240,
            puzzle: None,
        };

        assert!(!NetworkRelayShared::pow_summary_matches_broadcast(
            &summary, &broadcast
        ));
    }

    #[test]
    fn pow_update_payload_skips_when_pow_disabled() {
        let pow = SoranetPow::default();
        assert!(pow_update_payload(&pow, 1).is_none());
    }

    #[test]
    fn pow_update_payload_encodes_expected_fields() {
        let mut pow = SoranetPow::default();
        pow.required = true;
        pow.difficulty = 7;
        pow.max_future_skew = Duration::from_secs(900);
        pow.min_ticket_ttl = Duration::from_secs(120);
        pow.ticket_ttl = Duration::from_secs(240);
        pow.puzzle = Some(SoranetPuzzle::new(nz_u32(131_072), nz_u32(3), nz_u32(2)));

        let payload = pow_update_payload(&pow, 42).expect("payload");
        let decoded: SoranetPowConfigBroadcast =
            norito::json::from_slice(&payload).expect("decode payload");

        assert_eq!(decoded.version, 42);
        assert!(decoded.required);
        assert_eq!(decoded.difficulty, 7);
        assert_eq!(decoded.max_future_skew_secs, 900);
        assert_eq!(decoded.min_ticket_ttl_secs, 120);
        assert_eq!(decoded.ticket_ttl_secs, 240);
        let puzzle = decoded.puzzle.expect("puzzle included");
        assert_eq!(puzzle.memory_kib, 131_072);
        assert_eq!(puzzle.time_cost, 3);
        assert_eq!(puzzle.lanes, 2);
    }

    fn sample_peer() -> Peer {
        let keypair = KeyPair::random();
        Peer::new(
            "127.0.0.1:0".parse().expect("socket address"),
            keypair.public_key().clone(),
        )
    }

    fn nz_u32(value: u32) -> std::num::NonZeroU32 {
        std::num::NonZeroU32::new(value).expect("non-zero")
    }

    fn sumeragi_msg(msg: BlockMessage) -> iroha_core::NetworkMessage {
        iroha_core::NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(msg)))
    }

    fn consensus_params_msg() -> iroha_core::NetworkMessage {
        let advert = ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        sumeragi_msg(BlockMessage::ConsensusParams(advert))
    }

    fn signed_block_for_test() -> SignedBlock {
        let keypair = KeyPair::random();
        let new_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .sign(keypair.private_key())
            .unpack(|_| {});
        SignedBlock::from(new_block)
    }

    fn block_created_msg() -> iroha_core::NetworkMessage {
        let signed = signed_block_for_test();
        let created =
            BlockMessage::BlockCreated(iroha_core::sumeragi::message::BlockCreated::from(&signed));
        sumeragi_msg(created)
    }

    fn proposal_hint_msg() -> iroha_core::NetworkMessage {
        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x20; 32]));
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x21; 32]));
        let hint = iroha_core::sumeragi::message::ProposalHint {
            block_hash,
            height: 2,
            view: 0,
            highest_qc: QcHeaderRef {
                height: 1,
                view: 0,
                epoch: 0,
                subject_block_hash: parent_hash,
                phase: Phase::Commit,
            },
        };
        sumeragi_msg(BlockMessage::ProposalHint(hint))
    }

    fn sample_proposal() -> Proposal {
        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32]));
        Proposal {
            header: ConsensusBlockHeader {
                parent_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 2,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 1,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: parent_hash,
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::new(b"payload"),
        }
    }

    fn proposal_msg() -> iroha_core::NetworkMessage {
        let proposal = sample_proposal();
        sumeragi_msg(BlockMessage::Proposal(proposal))
    }

    fn exec_witness_msg() -> iroha_core::NetworkMessage {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x23; 32]));
        let msg = ExecWitnessMsg {
            block_hash,
            height: 2,
            view: 0,
            epoch: 0,
            witness: ExecWitness::default(),
        };
        sumeragi_msg(BlockMessage::ExecWitness(msg))
    }

    fn control_flow_msg() -> iroha_core::NetworkMessage {
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal: sample_proposal(),
                reason: "invalid proposal".to_owned(),
            },
        };
        iroha_core::NetworkMessage::SumeragiControlFlow(Box::new(ControlFlow::Evidence(evidence)))
    }

    fn block_sync_update_msg() -> iroha_core::NetworkMessage {
        let signed = signed_block_for_test();
        let update = BlockSyncUpdate::from(&signed);
        sumeragi_msg(BlockMessage::BlockSyncUpdate(update))
    }

    fn block_sync_msg(peer: &Peer) -> iroha_core::NetworkMessage {
        let msg = BlockSyncMessage::GetBlocksAfter(GetBlocksAfter::new(
            peer.id().clone(),
            None,
            None,
            BTreeSet::new(),
        ));
        iroha_core::NetworkMessage::BlockSync(Box::new(msg))
    }

    fn qc_vote_msg() -> iroha_core::NetworkMessage {
        use iroha_core::sumeragi::consensus::{Phase, Vote};

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2; 32]));
        let vote = Vote {
            phase: Phase::Commit,
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
            parent_state_root: Hash::prehashed([0; 32]),
            post_state_root: Hash::prehashed([0; 32]),
        };
        sumeragi_msg(BlockMessage::QcVote(vote))
    }

    fn qc_msg() -> iroha_core::NetworkMessage {
        use iroha_core::sumeragi::consensus::{Phase, Qc, QcAggregate};

        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([3; 32]));
        let validator = PeerId::new(KeyPair::random().public_key().clone());
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::prehashed([0x10; 32]),
            post_state_root: Hash::prehashed([0x11; 32]),
            height: 1,
            view: 0,
            epoch: 0,
            mode_tag: iroha_core::sumeragi::consensus::PERMISSIONED_TAG.to_owned(),
            highest_qc: None,
            validator_set_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0x12; 32])),
            validator_set_hash_version: 1,
            validator_set: vec![validator],
            aggregate: QcAggregate {
                signers_bitmap: vec![0b1],
                bls_aggregate_signature: vec![0xAA],
            },
        };
        sumeragi_msg(BlockMessage::Qc(qc))
    }

    fn vrf_commit_msg() -> iroha_core::NetworkMessage {
        let commit = iroha_core::sumeragi::consensus::VrfCommit {
            epoch: 1,
            commitment: [0x13; 32],
            signer: 0,
        };
        sumeragi_msg(BlockMessage::VrfCommit(commit))
    }

    fn vrf_reveal_msg() -> iroha_core::NetworkMessage {
        let reveal = iroha_core::sumeragi::consensus::VrfReveal {
            epoch: 1,
            reveal: [0x14; 32],
            signer: 0,
        };
        sumeragi_msg(BlockMessage::VrfReveal(reveal))
    }

    fn rbc_init_msg(tag: u8, height: u64, view: u64) -> iroha_core::NetworkMessage {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([tag; 32]));
        let block_header = BlockHeader::new(
            std::num::NonZeroU64::new(height).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            view,
        );
        let leader_key = KeyPair::random();
        let (_, leader_private) = leader_key.into_parts();
        let leader_signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(&leader_private, block_header.hash()),
        );
        let init = iroha_core::sumeragi::consensus::RbcInit {
            block_hash,
            height,
            view,
            epoch: 0,
            roster: Vec::new(),
            roster_hash: Hash::prehashed([0x11; 32]),
            total_chunks: 1,
            chunk_digests: vec![[0x22; 32]],
            payload_hash: Hash::prehashed([0x33; 32]),
            chunk_root: Hash::prehashed([0x44; 32]),
            block_header,
            leader_signature,
        };
        sumeragi_msg(BlockMessage::RbcInit(init))
    }

    fn rbc_ready_msg(tag: u8, height: u64, view: u64) -> iroha_core::NetworkMessage {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([tag; 32]));
        let ready = iroha_core::sumeragi::consensus::RbcReady {
            block_hash,
            height,
            view,
            epoch: 0,
            roster_hash: Hash::prehashed([0x21; 32]),
            chunk_root: Hash::prehashed([0x22; 32]),
            sender: 0,
            signature: vec![0x23],
        };
        sumeragi_msg(BlockMessage::RbcReady(ready))
    }

    fn rbc_deliver_msg(tag: u8, height: u64, view: u64) -> iroha_core::NetworkMessage {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([tag; 32]));
        let deliver = iroha_core::sumeragi::consensus::RbcDeliver {
            block_hash,
            height,
            view,
            epoch: 0,
            roster_hash: Hash::prehashed([0x31; 32]),
            chunk_root: Hash::prehashed([0x32; 32]),
            sender: 0,
            signature: vec![0x33],
            ready_signatures: Vec::new(),
        };
        sumeragi_msg(BlockMessage::RbcDeliver(deliver))
    }

    fn rbc_chunk_msg(tag: u8, height: u64, view: u64) -> iroha_core::NetworkMessage {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([tag; 32]));
        let chunk = iroha_core::sumeragi::consensus::RbcChunk {
            block_hash,
            height,
            view,
            epoch: 0,
            idx: 0,
            bytes: vec![0xCD; 4],
        };
        sumeragi_msg(BlockMessage::RbcChunk(chunk))
    }

    #[test]
    fn block_sync_request_drops_mismatched_peer_id() {
        let peer = sample_peer();
        let other = PeerId::new(KeyPair::random().public_key().clone());
        assert_ne!(peer.id(), &other);

        let msg = BlockSyncMessage::GetBlocksAfter(GetBlocksAfter::new(
            other,
            None,
            None,
            BTreeSet::new(),
        ));

        assert!(NetworkRelayShared::sanitize_block_sync_message(&peer, msg).is_none());
    }

    #[test]
    fn block_sync_response_drops_mismatched_peer_id() {
        let peer = sample_peer();
        let other = PeerId::new(KeyPair::random().public_key().clone());
        assert_ne!(peer.id(), &other);

        let msg = BlockSyncMessage::ShareBlocks(ShareBlocks::new(
            Vec::new(),
            other,
            Vec::new(),
            Vec::new(),
        ));

        assert!(NetworkRelayShared::sanitize_block_sync_message(&peer, msg).is_none());
    }

    #[test]
    fn block_sync_message_keeps_matching_peer_id() {
        let peer = sample_peer();
        let msg = BlockSyncMessage::GetBlocksAfter(GetBlocksAfter::new(
            peer.id().clone(),
            None,
            None,
            BTreeSet::new(),
        ));

        let sanitized =
            NetworkRelayShared::sanitize_block_sync_message(&peer, msg).expect("expected message");
        let BlockSyncMessage::GetBlocksAfter(get) = sanitized else {
            panic!("unexpected block sync variant");
        };
        assert_eq!(get.peer_id, *peer.id());
    }

    #[test]
    fn consensus_ingress_rate_limit_drops_burst() {
        let peer = sample_peer();
        let msg = consensus_params_msg();
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
            0,
            Duration::from_secs(1),
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
        let msg = consensus_params_msg();
        let vote = qc_vote_msg();
        let qc = qc_msg();
        let commit = vrf_commit_msg();
        let reveal = vrf_reveal_msg();
        let hint = proposal_hint_msg();
        let proposal = proposal_msg();
        let control_flow = control_flow_msg();
        let init = rbc_init_msg(0x01, 2, 0);
        let chunk = rbc_chunk_msg(0x01, 2, 0);
        let ready = rbc_ready_msg(0x01, 2, 0);
        let deliver = rbc_deliver_msg(0x01, 2, 0);
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
            0,
            Duration::from_secs(1),
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
        assert_eq!(limiter.should_drop(&peer, &qc, 1), None);
        assert_eq!(limiter.should_drop(&peer, &commit, 1), None);
        assert_eq!(limiter.should_drop(&peer, &reveal, 1), None);
        assert_eq!(limiter.should_drop(&peer, &hint, 1), None);
        assert_eq!(limiter.should_drop(&peer, &proposal, 1), None);
        assert_eq!(limiter.should_drop(&peer, &control_flow, 1), None);
        assert_eq!(limiter.should_drop(&peer, &init, 1), None);
        assert_eq!(limiter.should_drop(&peer, &chunk, 1), None);
        assert_eq!(limiter.should_drop(&peer, &ready, 1), None);
        assert_eq!(limiter.should_drop(&peer, &deliver, 1), None);
    }

    #[test]
    fn consensus_ingress_proposal_uses_critical_bucket() {
        let peer = sample_peer();
        let msg = consensus_params_msg();
        let hint = proposal_hint_msg();
        let proposal = proposal_msg();
        let created = block_created_msg();
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
            0,
            Duration::from_secs(1),
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
        assert_eq!(limiter.should_drop(&peer, &hint, 1), None);
        assert_eq!(limiter.should_drop(&peer, &proposal, 1), None);
        assert_eq!(limiter.should_drop(&peer, &created, 1), None);
    }

    #[test]
    fn consensus_ingress_critical_rate_limit_drops_burst() {
        let peer = sample_peer();
        let vote = qc_vote_msg();
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
            0,
            Duration::from_secs(1),
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
    fn consensus_ingress_block_sync_uses_bulk_bucket() {
        let peer = sample_peer();
        let msg = consensus_params_msg();
        let sync = block_sync_msg(&peer);
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
            0,
            Duration::from_secs(1),
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
        assert_eq!(limiter.should_drop(&peer, &sync, 1), None);
        assert_eq!(limiter.should_drop(&peer, &sync, 1), None);
        assert_eq!(
            limiter.should_drop(&peer, &sync, 1),
            Some(ConsensusIngressDropReason::Rate)
        );
    }

    #[test]
    fn consensus_ingress_bulk_messages_use_bulk_bucket() {
        fn assert_bulk(peer: &Peer, msg: &iroha_core::NetworkMessage) {
            let standard = consensus_params_msg();
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
                0,
                Duration::from_secs(1),
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
        assert_bulk(&peer, &block_sync_update_msg());
        assert_bulk(&peer, &exec_witness_msg());
        assert_bulk(&peer, &rbc_chunk_msg(0x01, 2, 0));
    }

    #[test]
    fn consensus_ingress_bytes_limit_drops_oversize() {
        let peer = sample_peer();
        let msg = consensus_params_msg();
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
            0,
            Duration::from_secs(1),
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
    fn consensus_ingress_rbc_session_limit_counts_unique_sessions() {
        let peer = sample_peer();
        let mut limiter = ConsensusIngressLimiter::new(
            None,
            None,
            None,
            None,
            None,
            None,
            1,
            Duration::from_secs(60),
            PenaltyConfig {
                threshold: 0,
                window: Duration::from_secs(1),
                cooldown: Duration::from_secs(1),
            },
        );

        let first = rbc_init_msg(0x01, 2, 0);
        let same_session = rbc_chunk_msg(0x01, 2, 0);
        let second = rbc_init_msg(0x02, 3, 0);
        let ready_new = rbc_ready_msg(0x03, 4, 0);

        assert_eq!(limiter.should_drop(&peer, &first, 128), None);
        assert_eq!(limiter.should_drop(&peer, &same_session, 64), None);
        assert_eq!(
            limiter.should_drop(&peer, &second, 128),
            Some(ConsensusIngressDropReason::RbcSessionLimit)
        );
        assert_eq!(
            limiter.should_drop(&peer, &ready_new, 64),
            Some(ConsensusIngressDropReason::RbcSessionLimit)
        );
    }

    #[test]
    fn consensus_ingress_penalty_skips_critical_messages() {
        let peer = sample_peer();
        let msg = consensus_params_msg();
        let vote = qc_vote_msg();
        let qc = qc_msg();
        let commit = vrf_commit_msg();
        let reveal = vrf_reveal_msg();
        let hint = proposal_hint_msg();
        let proposal = proposal_msg();
        let control_flow = control_flow_msg();
        let chunk = rbc_chunk_msg(0x01, 2, 0);
        let ready = rbc_ready_msg(0x01, 2, 0);
        let deliver = rbc_deliver_msg(0x01, 2, 0);
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
            0,
            Duration::from_secs(1),
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
        assert_eq!(limiter.should_drop(&peer, &qc, 8), None);
        assert_eq!(limiter.should_drop(&peer, &commit, 8), None);
        assert_eq!(limiter.should_drop(&peer, &reveal, 8), None);
        assert_eq!(limiter.should_drop(&peer, &hint, 8), None);
        assert_eq!(limiter.should_drop(&peer, &proposal, 8), None);
        assert_eq!(limiter.should_drop(&peer, &control_flow, 8), None);
        assert_eq!(limiter.should_drop(&peer, &chunk, 8), None);
        assert_eq!(limiter.should_drop(&peer, &ready, 8), None);
        assert_eq!(limiter.should_drop(&peer, &deliver, 8), None);
    }

    #[test]
    fn consensus_ingress_penalty_skips_bulk_messages() {
        let peer = sample_peer();
        let bulk = block_sync_update_msg();
        let standard = consensus_params_msg();
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
            0,
            Duration::from_secs(1),
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
        let vote = qc_vote_msg();
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
            0,
            Duration::from_secs(1),
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
        let payload = block_sync_update_msg();
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&payload),
            Some("ConsensusPayload")
        );

        let chunk = rbc_chunk_msg(0x01, 1, 0);
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&chunk),
            Some("ConsensusChunk")
        );

        let peer = sample_peer();
        let block_sync = block_sync_msg(&peer);
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&block_sync),
            Some("BlockSync")
        );

        let vote = qc_vote_msg();
        assert_eq!(
            NetworkRelayShared::consensus_ingress_topic_label(&vote),
            None
        );
    }

    #[test]
    fn consensus_ingress_penalty_suppresses_after_threshold() {
        let peer = sample_peer();
        let msg = consensus_params_msg();
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
            0,
            Duration::from_secs(1),
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
    match error {
        TryReadSnapshotError::NotFound => true,
        TryReadSnapshotError::IO(_, _) => false,
        TryReadSnapshotError::ChainIdMismatch { .. } => false,
        _ => true,
    }
}

#[cfg(test)]
mod snapshot_read_error_tests {
    use super::*;
    use std::num::NonZeroUsize;

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;

    #[test]
    fn snapshot_read_error_is_recoverable_classifies_errors() {
        fn dummy_block_hash(byte: u8) -> HashOf<BlockHeader> {
            let mut bytes = [0u8; Hash::LENGTH];
            bytes[0] = byte;
            HashOf::from_untyped_unchecked(Hash::prehashed(bytes))
        }

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::NotFound
        ));

        let io = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        assert!(!snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::IO(io, std::path::PathBuf::from("snapshot.data"))
        ));

        assert!(!snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::ChainIdMismatch {
                expected: ChainId::from("expected-chain"),
                actual: ChainId::from("actual-chain"),
            }
        ));

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

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MismatchedHeight {
                snapshot_height: 2,
                kura_height: 1,
            }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MissingBlock { height: 1 }
        ));

        assert!(snapshot_read_error_is_recoverable(
            &TryReadSnapshotError::MismatchedHash {
                height: 1,
                snapshot_block_hash: dummy_block_hash(1),
                kura_block_hash: dummy_block_hash(2),
            }
        ));
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
    /// - Initialization of [`Sumeragi`](iroha_core::sumeragi::main_loop::Sumeragi) and [`Kura`]
    #[allow(clippy::too_many_lines)]
    #[iroha_logger::log(name = "start", skip_all)] // This is actually easier to understand as a linear sequence of init statements.
    pub async fn start(
        mut config: Config,
        mut genesis: Option<GenesisBlock>,
        logger: LoggerHandle,
        shutdown_signal: ShutdownSignal,
    ) -> ReportResult<
        (
            Self,
            impl Future<Output = iroha_futures::supervisor::Result<()>>,
        ),
        StartError,
    > {
        let mut supervisor = Supervisor::new();

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

        let (kura, mut block_count) = Kura::new(&config.kura, &config.nexus.lane_config)
            .map_err(|err| {
                let resolved = config.kura.store_dir.resolve_relative_path();
                Report::new(err).attach(format!(
                    "failed to initialize Kura for store_dir {} (raw {})",
                    resolved.display(),
                    config.kura.store_dir.value().display(),
                ))
            })
            .change_context(StartError::InitKura)?;
        let child = Kura::start(kura.clone(), supervisor.shutdown_signal());
        supervisor.monitor(child);

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
            |key| iroha_crypto::KeyPair::from(key.0.clone()),
        );

        let stored_genesis_block = read_stored_genesis_block(kura.as_ref(), block_count)?;
        let stored_genesis_hash = stored_genesis_block.as_ref().map(|block| block.0.hash());

        let effective_genesis_public_key = if let Some(stored_genesis) =
            stored_genesis_block.as_ref()
        {
            let stored_key = genesis_public_key_from_genesis_block(stored_genesis)?;
            if stored_key != config.genesis.public_key {
                iroha_logger::warn!(
                    configured = %config.genesis.public_key,
                    stored = %stored_key,
                    "genesis.public_key does not match stored genesis; using stored key for restart"
                );
            }
            if let (Some(config_hash), Some(stored_hash)) = (
                config.genesis.expected_hash.as_ref(),
                stored_genesis_hash.as_ref(),
            ) {
                if config_hash != stored_hash {
                    iroha_logger::warn!(
                        configured = ?config_hash,
                        stored = ?stored_hash,
                        "genesis.expected_hash does not match stored genesis; ignoring for restart"
                    );
                }
            }
            stored_key
        } else {
            config.genesis.public_key.clone()
        };

        let mut state = match try_read_snapshot(
            config.snapshot.store_dir.resolve_relative_path(),
            &kura,
            || live_query_store.clone(),
            block_count,
            config.snapshot.merkle_chunk_size_bytes,
            verification_key,
            &config.common.chain,
            #[cfg(feature = "telemetry")]
            state_telemetry.clone(),
        ) {
            Ok(state) => {
                iroha_logger::info!(
                    at_height = state.committed_height(),
                    "Successfully loaded the state from a snapshot"
                );
                state
            }
            Err(TryReadSnapshotError::NotFound) => {
                iroha_logger::info!("Didn't find a state snapshot; creating an empty state");
                let genesis_public_key = effective_genesis_public_key.clone();
                let mut world = World::with(
                    [genesis_domain(genesis_public_key.clone())],
                    [genesis_account(genesis_public_key)],
                    [],
                );
                if let Some(genesis_block) = stored_genesis_block.as_ref().or(genesis.as_ref()) {
                    iroha_core::sns::seed_genesis_alias_bootstrap(&mut world, &genesis_block.0);
                }
                State::new(
                    world,
                    Arc::clone(&kura),
                    live_query_store.clone(),
                    #[cfg(feature = "telemetry")]
                    state_telemetry,
                )
            }
            Err(error) if snapshot_read_error_is_recoverable(&error) => {
                iroha_logger::warn!(
                    ?error,
                    "Failed to load state snapshot; rebuilding state by replaying Kura blocks"
                );
                let genesis_public_key = effective_genesis_public_key.clone();
                let mut world = World::with(
                    [genesis_domain(genesis_public_key.clone())],
                    [genesis_account(genesis_public_key)],
                    [],
                );
                if let Some(genesis_block) = stored_genesis_block.as_ref().or(genesis.as_ref()) {
                    iroha_core::sns::seed_genesis_alias_bootstrap(&mut world, &genesis_block.0);
                }
                State::new(
                    world,
                    Arc::clone(&kura),
                    live_query_store.clone(),
                    #[cfg(feature = "telemetry")]
                    state_telemetry,
                )
            }
            Err(error) => {
                return Err(Report::new(error).change_context(StartError::InitKura));
            }
        };
        #[cfg(feature = "telemetry")]
        {
            kura.attach_telemetry(state.telemetry.clone());
        }
        // Thread chain id into state for VRF prehash binding.
        state.chain_id = config.common.chain.clone();
        // Apply crypto config before replay/genesis validation so allowed_signing is respected.
        state.set_crypto(config.crypto.clone());
        state
            .set_nexus(config.nexus.clone())
            .map_err(|err| Report::new(err).change_context(StartError::InitKura))
            .map_err(|report| {
                report.attach("failed to apply Nexus lane catalog/lifecycle at startup")
            })?;

        let state_height = state.committed_height();
        if block_count.0 > state_height {
            let start_height = state_height.saturating_add(1);
            iroha_logger::info!(
                start_height,
                block_count = block_count.0,
                "Replaying stored blocks to catch up with Kura"
            );
            let trusted = config.common.trusted_peers.value();
            let mut commit_topology = filter_validators_from_trusted(trusted);
            if commit_topology.is_empty() {
                commit_topology = trusted.clone().into_non_empty_vec().into_iter().collect();
            }
            let topology = Topology::new(commit_topology);
            iroha_core::state::replay_blocks_from_kura_range(
                &kura,
                &mut state,
                &topology,
                start_height,
                block_count.0,
                config.sumeragi.consensus_mode,
            )
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
        }
        // Delay Arc wrapping until after we tweak state with config

        let (events_sender, _) = broadcast::channel(config.torii.events_buffer_capacity.get());
        // Register pipeline events sender for ZK lane reporting
        iroha_core::pipeline::zk_lane::register_events_sender(events_sender.clone());
        let router: Arc<dyn LaneRouter> = if should_use_config_router(&config.nexus) {
            Arc::new(ConfigLaneRouter::new(
                config.nexus.routing_policy.clone(),
                config.nexus.dataspace_catalog.clone(),
                config.nexus.lane_catalog.clone(),
            ))
        } else {
            Arc::new(SingleLaneRouter::new())
        };
        let queue_limits = iroha_core::queue::QueueLimits::from_nexus(&config.nexus);
        let lane_catalog = Arc::new(config.nexus.lane_catalog.clone());
        let dataspace_catalog = Arc::new(config.nexus.dataspace_catalog.clone());
        let governance_catalog = Arc::new(config.nexus.governance.clone());
        let registry_cfg = config.nexus.registry.clone();
        let lane_compliance = if config.nexus.compliance.enabled {
            let dir = config.nexus.compliance.policy_dir.as_ref().ok_or_else(|| {
                Report::new(StartError::InitKura)
                    .attach("lane compliance enabled but no policy_dir configured")
            })?;
            let engine =
                LaneComplianceEngine::from_directory(dir, config.nexus.compliance.audit_only)
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
        if config.nexus.enabled {
            let lane_manifests = Arc::new(LaneManifestRegistry::from_config(
                lane_catalog.as_ref(),
                governance_catalog.as_ref(),
                &registry_cfg,
            ));
            queue.install_lane_manifests(&lane_manifests);
            state.install_lane_manifests(&lane_manifests);
            state
                .telemetry
                .set_lane_manifest_registry(Arc::clone(&lane_manifests));
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
        } else {
            let empty_registry = Arc::new(LaneManifestRegistry::empty());
            queue.install_lane_manifests(&empty_registry);
            state.install_lane_manifests(&empty_registry);
            state
                .telemetry
                .set_lane_manifest_registry(Arc::clone(&empty_registry));
        }

        let config_caps = build_consensus_config_caps(&config.sumeragi)?;
        let consensus_caps_override = if block_count.0 == 0 {
            genesis.as_ref().and_then(|block| {
                consensus_caps_from_genesis(
                    block,
                    &config.common.chain,
                    &config_caps,
                    &config.sumeragi,
                )
            })
        } else {
            None
        };
        let proto = iroha_core::sumeragi::consensus::PROTO_VERSION;

        // Compute consensus handshake caps for gating peers
        // Use WSV Sumeragi parameters (canonical JSON) so fingerprint is stable across peers
        let (computed_mode_tag, computed_bls_domain, consensus_caps, confidential_features) = {
            let world = state.world_view();
            let height = u64::try_from(state.committed_height()).expect("height fits into u64");
            let zk = state.zk_snapshot();
            let confidential_features =
                iroha_core::state::compute_confidential_feature_digest(&world, &zk, height);
            let (mode_tag, bls_domain, caps) = compute_consensus_handshake_caps(
                &world,
                height,
                &config,
                &config_caps,
                consensus_caps_override.clone(),
            );
            (mode_tag, bls_domain, caps, confidential_features)
        };
        iroha_logger::info!(
            mode=%consensus_caps.mode_tag,
            proto=%consensus_caps.proto_version,
            fingerprint=%format!("0x{}", hex::encode(consensus_caps.consensus_fingerprint)),
            "Consensus handshake caps"
        );

        if let Some(genesis_block) = genesis.as_ref() {
            let metadata_genesis = stored_genesis_block.as_ref().unwrap_or(genesis_block);
            verify_genesis_metadata(
                metadata_genesis,
                &config,
                &consensus_caps,
                &computed_mode_tag,
                &computed_bls_domain,
                proto,
            )
            .map_err(|err| {
                iroha_logger::error!(?err, "genesis consensus metadata validation failed");
                Report::new(StartError::InitKura).attach(err)
            })?;
        }

        // If a genesis manifest JSON is provided via CLI, validate consensus fields.
        let cfg_manifest = config
            .genesis
            .manifest_json
            .as_ref()
            .map(WithOrigin::resolve_relative_path);
        if let Some(json_path) = cfg_manifest {
            let manifest = read_genesis_manifest(&json_path)?;
            if let Err(err) = ensure_manifest_crypto_matches(&manifest, &config) {
                return Err(Report::new(StartError::InitKura).attach(format!(
                    "Genesis manifest crypto settings do not match node configuration: {err}"
                )));
            } else if genesis.is_none() {
                config.crypto = manifest.crypto().clone().into();
            }

            if let Some(mode) = manifest.consensus_mode() {
                let expected = match config.sumeragi.consensus_mode {
                    iroha_config::parameters::actual::ConsensusMode::Permissioned => {
                        iroha_core::sumeragi::consensus::PERMISSIONED_TAG
                    }
                    iroha_config::parameters::actual::ConsensusMode::Npos => {
                        iroha_core::sumeragi::consensus::NPOS_TAG
                    }
                };
                let got = match mode {
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
            }

            if !manifest.wire_proto_versions().is_empty()
                && !manifest.wire_proto_versions().contains(&proto)
            {
                return Err(Report::new(StartError::InitKura).attach(format!(
                    "Genesis manifest wire_proto_versions does not include v{proto}"
                )));
            }

            if let Some(fp_s) = manifest.consensus_fingerprint() {
                let fp_clean = fp_s.trim_start_matches("0x");
                if let Ok(bytes) = hex::decode(fp_clean)
                    && bytes.len() == 32
                    && bytes[..] != consensus_caps.consensus_fingerprint
                {
                    return Err(
                        Report::new(GenesisManifestError::ConsensusFingerprintMismatch)
                            .change_context(StartError::InitKura),
                    );
                }
            }
        }

        let bootstrap_allowlist: HashSet<PeerId> = if config.genesis.bootstrap_allowlist.is_empty()
        {
            config
                .common
                .trusted_peers
                .value()
                .clone()
                .into_non_empty_vec()
                .into_iter()
                .collect()
        } else {
            config.genesis.bootstrap_allowlist.iter().cloned().collect()
        };

        let confidential_caps = iroha_p2p::ConfidentialHandshakeCaps {
            enabled: config.confidential.enabled,
            assume_valid: config.confidential.assume_valid,
            verifier_backend: config.confidential.verifier_backend.clone(),
            features: Some(confidential_features),
        };
        let crypto_caps = iroha_p2p::CryptoHandshakeCaps {
            sm_enabled: config.crypto.sm_helpers_enabled(),
            sm_openssl_preview: config.crypto.enable_sm_openssl_preview,
            require_sm_handshake_match: config.network.require_sm_handshake_match,
            require_sm_openssl_preview_match: config.network.require_sm_openssl_preview_match,
        };
        let (network, child) = IrohaNetwork::start_with_crypto(
            config.common.key_pair.clone(),
            config.network.clone(),
            // Bind handshake to chain id when supported by the p2p layer
            Some(config.common.chain.clone()),
            Some(consensus_caps),
            Some(confidential_caps),
            Some(crypto_caps),
            supervisor.shutdown_signal(),
        )
        .await
        .attach_with(|| config.network.address.clone().into_attachment())
        .change_context(StartError::StartP2p)?;
        supervisor.monitor(child);

        // Bootstrapper orchestrates request/response handling for genesis.
        let bootstrap_genesis_config = if let Some(stored_hash) = stored_genesis_hash.clone() {
            let mut cfg = config.genesis.clone();
            cfg.public_key = effective_genesis_public_key.clone();
            cfg.expected_hash = Some(stored_hash);
            cfg
        } else {
            config.genesis.clone()
        };
        let bootstrapper = GenesisBootstrapper::new(
            &bootstrap_genesis_config,
            network.clone(),
            config.common.chain.clone(),
        );
        let trusted = config.common.trusted_peers.value().clone();
        let peer_seed: Vec<(PeerId, SocketAddr)> = std::iter::once(trusted.myself)
            .chain(trusted.others.into_iter())
            .map(|peer| (peer.id().clone(), peer.address.clone()))
            .collect();
        bootstrapper.seed_topology(&peer_seed);
        bootstrapper.spawn_listener().await;

        // Allow peers to fetch genesis if we already have it locally (storage or file).
        if let Some(stored_genesis) = stored_genesis_block.as_ref() {
            if let Err(err) = bootstrapper.set_payload(stored_genesis).await {
                iroha_logger::warn!(
                    ?err,
                    "failed to register stored genesis payload for bootstrap"
                );
            }
        } else if let Some(genesis_block) = genesis.as_ref() {
            if let Err(err) = bootstrapper.set_payload(genesis_block).await {
                iroha_logger::warn!(
                    ?err,
                    "failed to register local genesis payload for bootstrap"
                );
            }
        }

        // If we are starting from empty storage without a local genesis file, try bootstrapping
        // from trusted peers before failing fast.
        if genesis.is_none() && block_count.0 == 0 {
            if config.genesis.bootstrap_enabled {
                let candidates: Vec<PeerId> = bootstrap_allowlist
                    .iter()
                    .filter(|peer| *peer != config.common.peer.id())
                    .cloned()
                    .collect();
                if candidates.is_empty() {
                    iroha_logger::warn!(
                        "genesis bootstrap skipped: no trusted peers available to request genesis"
                    );
                } else {
                    let expected_hash = config.genesis.expected_hash;
                    let genesis_account = AccountId::new(effective_genesis_public_key.clone());
                    match bootstrapper
                        .fetch_genesis(&candidates, &genesis_account, expected_hash)
                        .await
                    {
                        Ok(fetched) => {
                            let path = config
                                .kura
                                .store_dir
                                .resolve_relative_path()
                                .join("genesis.bootstrap.nrt");
                            if let Err(err) = fs::create_dir_all(
                                path.parent().expect("genesis bootstrap path has parent"),
                            ) {
                                iroha_logger::warn!(
                                    ?err,
                                    path = %path.display(),
                                    "failed to create bootstrap genesis directory"
                                );
                            } else if let Err(err) = fs::write(&path, &fetched.bytes) {
                                iroha_logger::warn!(
                                    ?err,
                                    path = %path.display(),
                                    "failed to persist bootstrapped genesis payload"
                                );
                            } else {
                                iroha_logger::info!(
                                    path = %path.display(),
                                    "persisted bootstrapped genesis payload"
                                );
                                config.genesis.file = Some(WithOrigin::inline(path.clone()));
                            }
                            if let Err(err) = bootstrapper.set_payload(&fetched.block).await {
                                iroha_logger::warn!(
                                    ?err,
                                    "failed to register bootstrapped genesis payload"
                                );
                            }
                            genesis = Some(fetched.block);
                        }
                        Err(err) => {
                            iroha_logger::warn!(
                                %err,
                                timeout_ms = config.genesis.bootstrap_request_timeout.as_millis(),
                                "genesis bootstrap failed"
                            );
                        }
                    }
                }
            } else {
                iroha_logger::warn!(
                    "genesis bootstrap is disabled and no local genesis is available; startup will fail"
                );
            }
        }

        if let Some(genesis_block) = genesis.as_ref() {
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
                let genesis_account = AccountId::new(effective_genesis_public_key.clone());
                if let Err(err) = iroha_core::validate_genesis_block(
                    &genesis_block.0,
                    &genesis_account,
                    &config.common.chain,
                ) {
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

                // Ensure genesis instructions execute successfully before starting consensus.
                // Use the configured trusted peers as the initial commit topology when the
                // state hasn't recorded one yet (fresh storage). This ensures the genesis
                // application seeds the validator roster with the full cluster rather than
                // a single local peer, which would otherwise isolate the node and stall
                // consensus.
                let commit_topology: Vec<_> = {
                    let peers = state.commit_topology_snapshot();
                    if peers.is_empty() {
                        let mut validator_roster =
                            filter_validators_from_trusted(config.common.trusted_peers.value());
                        if validator_roster.is_empty() {
                            validator_roster = config
                                .common
                                .trusted_peers
                                .value()
                                .clone()
                                .into_non_empty_vec()
                                .into_iter()
                                .collect();
                        }
                        let me = config.common.peer.id();
                        if !validator_roster.iter().any(|p| p == me) {
                            validator_roster.push(me.clone());
                        }
                        if validator_roster.is_empty() {
                            validator_roster.push(me.clone());
                        }
                        validator_roster
                    } else {
                        peers
                    }
                };
                let topology = Topology::new(commit_topology.clone());
                let time_source = TimeSource::new_system();
                let mut voting_block: Option<VotingBlock> = None;
                let validation = ValidBlock::validate_keep_voting_block(
                    genesis_block.0.clone(),
                    &topology,
                    &config.common.chain,
                    &genesis_account,
                    &time_source,
                    &state,
                    &mut voting_block,
                    false,
                )
                .unpack(|_| {});
                match validation {
                    Ok((valid_block, mut state_block)) => {
                        let committed_block = valid_block.commit_unchecked().unpack(|_| {});
                        let events = state_block
                            .apply_without_execution(&committed_block, commit_topology.clone());
                        state_block.commit().map_err(|err| {
                            Report::new(StartError::InitKura)
                                .attach(format!("failed to commit injected genesis state: {err}"))
                        })?;
                        let params_snapshot = {
                            let world = state.world_view();
                            let params = world.parameters();
                            (
                                params.block().max_transactions().get(),
                                params.smart_contract().execution_depth(),
                                params.executor().execution_depth(),
                            )
                        };
                        iroha_logger::debug!(
                            height = committed_block.as_ref().header().height().get(),
                            max_tx = params_snapshot.0,
                            sc_depth = params_snapshot.1,
                            exec_depth = params_snapshot.2,
                            "genesis parameters after commit"
                        );
                        if matches!(
                            config.sumeragi.consensus_mode,
                            iroha_config::parameters::actual::ConsensusMode::Npos
                        ) {
                            let (active_bls, active_total, pending, total) = {
                                let world = state.world_view();
                                npos_validator_status_counts(
                                    world
                                        .public_lane_validators()
                                        .iter()
                                        .map(|(_, record)| record),
                                )
                            };
                            if active_bls == 0 {
                                let stake_asset_id = config.nexus.staking.stake_asset_id.as_str();
                                iroha_logger::error!(
                                    active_bls,
                                    active_total,
                                    pending,
                                    total,
                                    stake_asset_id,
                                    "NPoS genesis did not activate any BLS validators"
                                );
                                let err_msg = format!(
                                    "NPoS genesis did not activate any BLS validators (active_total={active_total}, pending={pending}, total={total}). Ensure genesis registers validators with PoPs and stakes {stake_asset_id} for each topology peer (for example via `kagami localnet` or `kagami genesis sign --topology ... --peer-pop ...`)."
                                );
                                return Err(Report::new(StartError::InitKura).attach(err_msg));
                            }
                        }
                        for event in events {
                            if let Err(err) = events_sender.send(event) {
                                iroha_logger::debug!(
                                    ?err,
                                    "failed to send pipeline event while applying genesis"
                                );
                            }
                        }
                        kura.store_block(committed_block.clone())
                            .map_err(|err| Report::new(err).change_context(StartError::InitKura))?;
                        let genesis_height = committed_block.as_ref().header().height().get();
                        let height_as_usize = usize::try_from(genesis_height).unwrap_or(usize::MAX);
                        block_count.0 = block_count.0.max(height_as_usize);
                        iroha_logger::info!(
                            height = genesis_height,
                            "Applied genesis block to local storage"
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
        } else if block_count.0 == 0 {
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

        iroha_core::streaming::set_global_handle(streaming.clone());

        let streaming_events_handle = streaming.clone();
        let ticket_events_rx = events_sender.subscribe();
        supervisor.monitor(tokio::spawn(async move {
            run_ticket_event_listener(streaming_events_handle, ticket_events_rx).await;
        }));

        #[cfg(feature = "telemetry")]
        start_telemetry(&logger, &config, &mut supervisor).await?;

        // Thread tiered state, pipeline, and ZK (Halo2) preferences from config into runtime state.
        // Use cloned config values to keep `config` borrowable later.
        let tiered_state_cfg = config.tiered_state.clone();
        let pipeline_cfg = config.pipeline.clone();
        let sumeragi_cfg = config.sumeragi.clone();
        let fraud_cfg = config.fraud_monitoring.clone();
        let zk_cfg = config.zk.clone();
        let settlement_cfg = config.settlement.clone();
        let gov_cfg = config.gov.clone();
        let oracle_cfg = config.oracle.clone();
        let streaming_cfg = config.streaming.clone();
        let merge_cache_capacity = config.kura.merge_ledger_cache_capacity;
        state.set_tiered_backend(&tiered_state_cfg);
        state.set_pipeline(pipeline_cfg);
        state.set_sumeragi_parameters(&sumeragi_cfg);
        state.set_sumeragi_pacing_governor(sumeragi_cfg.pacing_governor);
        state.set_oracle(oracle_cfg);
        state.set_streaming(streaming_cfg);
        state.set_fraud_monitoring(fraud_cfg);
        state.set_zk(zk_cfg.clone());
        state.set_settlement(settlement_cfg);
        state.set_gov(gov_cfg);
        state.set_merge_ledger_cache_capacity(merge_cache_capacity);
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
            config.common.trusted_peers.value().clone(),
            config.common.key_pair.clone(),
            config.network.peer_gossip_period,
            config.network.peer_gossip_max_period,
            config.sumeragi.consensus_mode,
            config.network.trust_decay_half_life,
            config.network.trust_penalty_bad_gossip,
            config.network.trust_penalty_unknown_peer,
            config.network.trust_min_score,
            network.clone(),
            supervisor.shutdown_signal(),
        );
        supervisor.monitor(child);

        // Background poster worker for Sumeragi frames; overflow falls back to inline posts.
        let background_post_tx = if config.sumeragi.debug.disable_background_worker {
            None
        } else {
            let bg_cap = config.sumeragi.queues.control.max(1);
            let (bg_tx, bg_rx) =
                std::sync::mpsc::sync_channel::<iroha_core::sumeragi::BackgroundPost>(bg_cap);
            {
                let network_for_worker = network.clone();
                #[cfg(feature = "telemetry")]
                let telemetry_for_worker = telemetry.clone();
                std::thread::Builder::new()
                    .name("sumeragi-post".to_string())
                    .spawn(move || {
                        while let Ok(task) = bg_rx.recv() {
                            match task {
                                iroha_core::sumeragi::BackgroundPost::Post {
                                    peer,
                                    msg,
                                    enqueued_at,
                                } => {
                                    #[cfg(feature = "telemetry")]
                                    telemetry_for_worker.note_consensus_message_sent(&msg);
                                    let priority = msg.priority();
                                    let post = iroha_p2p::Post {
                                        data: iroha_core::NetworkMessage::SumeragiBlock(Box::new(
                                            msg,
                                        )),
                                        peer_id: peer.clone(),
                                        priority,
                                    };
                                    network_for_worker.post(post);
                                    #[cfg(feature = "telemetry")]
                                    {
                                        telemetry_for_worker.dec_bg_post_queue_depth();
                                        telemetry_for_worker
                                            .dec_bg_post_queue_depth_for_peer(&peer);
                                        let age_ms = enqueued_at.elapsed().as_secs_f64() * 1000.0;
                                        telemetry_for_worker.observe_bg_post_age_ms("Post", age_ms);
                                    }
                                }
                                iroha_core::sumeragi::BackgroundPost::PostControlFlow {
                                    peer,
                                    frame,
                                    enqueued_at,
                                } => {
                                    let post = iroha_p2p::Post {
                                        data: iroha_core::NetworkMessage::SumeragiControlFlow(
                                            Box::new(frame),
                                        ),
                                        peer_id: peer.clone(),
                                        priority: iroha_p2p::Priority::High,
                                    };
                                    network_for_worker.post(post);
                                    #[cfg(feature = "telemetry")]
                                    {
                                        telemetry_for_worker.dec_bg_post_queue_depth();
                                        telemetry_for_worker
                                            .dec_bg_post_queue_depth_for_peer(&peer);
                                        let age_ms = enqueued_at.elapsed().as_secs_f64() * 1000.0;
                                        telemetry_for_worker
                                            .observe_bg_post_age_ms("PostControlFlow", age_ms);
                                    }
                                }
                                iroha_core::sumeragi::BackgroundPost::Broadcast {
                                    msg,
                                    enqueued_at,
                                } => {
                                    #[cfg(feature = "telemetry")]
                                    telemetry_for_worker.note_consensus_message_sent(&msg);
                                    let priority = msg.priority();
                                    let b = iroha_p2p::Broadcast {
                                        data: iroha_core::NetworkMessage::SumeragiBlock(Box::new(
                                            msg,
                                        )),
                                        priority,
                                    };
                                    network_for_worker.broadcast(b);
                                    #[cfg(feature = "telemetry")]
                                    {
                                        telemetry_for_worker.dec_bg_post_queue_depth();
                                        let age_ms = enqueued_at.elapsed().as_secs_f64() * 1000.0;
                                        telemetry_for_worker
                                            .observe_bg_post_age_ms("Broadcast", age_ms);
                                    }
                                }
                                iroha_core::sumeragi::BackgroundPost::BroadcastControlFlow {
                                    frame,
                                    enqueued_at,
                                } => {
                                    let b = iroha_p2p::Broadcast {
                                        data: iroha_core::NetworkMessage::SumeragiControlFlow(
                                            Box::new(frame),
                                        ),
                                        priority: iroha_p2p::Priority::High,
                                    };
                                    network_for_worker.broadcast(b);
                                    #[cfg(feature = "telemetry")]
                                    {
                                        telemetry_for_worker.dec_bg_post_queue_depth();
                                        let age_ms = enqueued_at.elapsed().as_secs_f64() * 1000.0;
                                        telemetry_for_worker
                                            .observe_bg_post_age_ms("BroadcastControlFlow", age_ms);
                                    }
                                }
                            }
                        }
                    })
                    .expect("spawn sumeragi-post worker");
            }
            Some(bg_tx)
        };

        #[cfg(feature = "telemetry")]
        let torii_telemetry =
            iroha_torii::MaybeTelemetry::from_profile(Some(telemetry.clone()), telemetry_profile);
        #[cfg(not(feature = "telemetry"))]
        let torii_telemetry = iroha_torii::MaybeTelemetry::from_profile(None, telemetry_profile);

        let rbc_store_dir = config
            .kura
            .store_dir
            .resolve_relative_path()
            .join("rbc_sessions");

        let genesis_for_consensus = if stored_genesis_block.is_some() {
            None
        } else {
            genesis
        };
        let sumeragi_cfg = config.sumeragi.clone();
        let (sumeragi, child) = SumeragiStartArgs {
            config: sumeragi_cfg.clone(),
            common_config: config.common.clone(),
            consensus_frame_cap: config.network.max_frame_bytes_consensus,
            consensus_payload_frame_cap: config.network.max_frame_bytes_block_sync,
            events_sender: events_sender.clone(),
            state: state.clone(),
            queue: queue.clone(),
            kura: kura.clone(),
            network: network.clone(),
            peers_gossiper: peers_gossiper.clone(),
            genesis_network: GenesisWithPubKey {
                genesis: genesis_for_consensus,
                public_key: effective_genesis_public_key.clone(),
            },
            block_count,
            block_sync_gossip_limit: usize::try_from(config.block_sync.gossip_size.get())
                .unwrap_or(usize::MAX),
            #[cfg(feature = "telemetry")]
            telemetry: telemetry.clone(),
            epoch_roster_provider: if matches!(
                sumeragi_cfg.consensus_mode,
                iroha_config::parameters::actual::ConsensusMode::Npos
            ) && sumeragi_cfg.npos.use_stake_snapshot_roster
            {
                // Placeholder: map current WSV peers to contiguous indices.
                let world = state.world_view();
                let peers: Vec<PeerId> = world.peers().to_vec();
                Some(std::sync::Arc::new(
                    iroha_core::sumeragi::WsvEpochRosterAdapter::new(peers),
                ))
            } else {
                None
            },
            rbc_store: Some({
                let disk_max_bytes =
                    usize::try_from(sumeragi_cfg.rbc.disk_store_max_bytes).unwrap_or(usize::MAX);
                let max_bytes = sumeragi_cfg.rbc.store_max_bytes.min(disk_max_bytes);
                RbcStoreConfig {
                    dir: rbc_store_dir.clone(),
                    max_sessions: sumeragi_cfg.rbc.store_max_sessions,
                    soft_sessions: sumeragi_cfg.rbc.store_soft_sessions,
                    max_bytes,
                    soft_bytes: sumeragi_cfg.rbc.store_soft_bytes,
                    ttl: sumeragi_cfg.rbc.disk_store_ttl,
                }
            }),
            background_post_tx,
            da_spool_dir: config.torii.da_ingest.manifest_store_dir.clone(),
        }
        .start(supervisor.shutdown_signal());
        supervisor.monitor(child);

        let block_sync_frame_cap = {
            let global_plaintext = iroha_p2p::frame_plaintext_cap(config.network.max_frame_bytes);
            config
                .network
                .max_frame_bytes_block_sync
                .min(global_plaintext)
        };
        #[cfg(feature = "telemetry")]
        let block_sync_telemetry = Some(telemetry.clone());
        #[cfg(not(feature = "telemetry"))]
        let block_sync_telemetry = None;
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
        let (block_sync, child) = BlockSynchronizer::from_config(
            &config.block_sync,
            sumeragi.clone(),
            kura.clone(),
            config.common.peer.clone(),
            trusted_peers,
            network.clone(),
            Arc::clone(&state),
            block_sync_telemetry,
            config.sumeragi.consensus_mode,
            config.network.relay_ttl,
            block_sync_frame_cap,
        )
        .start(supervisor.shutdown_signal());
        supervisor.monitor(child);

        let (tx_gossiper, child) = TransactionGossiper::from_config(
            config.common.chain.clone(),
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

        let sorafs_node = sorafs_node::NodeHandle::new_with_policies(
            sorafs_node::config::StorageConfig::from(&config.torii.sorafs_storage),
            sorafs_node::config::RepairConfig::from_repair_and_policy(
                &config.torii.sorafs_repair,
                &state.gov.sorafs_repair_escalation,
            ),
            sorafs_node::config::GcConfig::from(&config.torii.sorafs_gc),
        );
        let shared_sorafs_cache = build_shared_sorafs_provider_cache(&config);

        let chain_id = Arc::new(config.common.chain.clone());
        let local_validator_account_id =
            AccountId::new(config.common.key_pair.public_key().clone());
        let local_peer_id = config.common.trusted_peers.value().myself.id().to_string();
        let runtime_mutation_sink = Arc::new(QueuedSoracloudRuntimeMutationSink::new(
            Arc::clone(&chain_id),
            Arc::clone(&queue),
            Arc::clone(&state),
            local_validator_account_id.clone(),
            config.common.key_pair.clone(),
        ));
        let runtime_manager = SoracloudRuntimeManager::new(
            soracloud_runtime::SoracloudRuntimeManagerConfig::from_runtime_config(
                &config.soracloud_runtime,
            )
            .with_local_host_identity(local_validator_account_id, local_peer_id),
            Arc::clone(&state),
        )
        .with_mutation_sink(runtime_mutation_sink)
        .with_sorafs_node(sorafs_node.clone());
        let runtime_manager = if let Some(cache) = shared_sorafs_cache.clone() {
            runtime_manager.with_sorafs_provider_cache(cache)
        } else {
            runtime_manager
        };
        let (soracloud_runtime, child) = runtime_manager.start(supervisor.shutdown_signal());
        state.set_soracloud_runtime(Some(Arc::new(soracloud_runtime.clone())));
        supervisor.monitor(child);

        ensure_operator_node_key_allowlisted(&mut config);
        let (kiso, child) = KisoHandle::start(config.clone());
        supervisor.monitor(child);

        let receipt_signer = config.torii.receipt_signer.clone().unwrap_or_else(|| {
            let key = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
            iroha_logger::info!(
                algorithm = ?key.public_key().algorithm(),
                "torii receipt signer not configured; generated ephemeral key"
            );
            key
        });
        let runtime_deps = iroha_torii::ToriiRuntimeDeps::new(torii_telemetry)
            .with_soracloud_runtime(Arc::new(soracloud_runtime.clone()))
            .with_sorafs_node(sorafs_node);
        let runtime_deps = if let Some(cache) = shared_sorafs_cache {
            runtime_deps.with_sorafs_cache(cache)
        } else {
            runtime_deps
        };
        let torii = Torii::new_with_handle(
            config.common.chain.clone(),
            kiso.clone(),
            config.torii,
            queue,
            events_sender,
            live_query_store,
            kura.clone(),
            state.clone(),
            receipt_signer,
            iroha_torii::OnlinePeersProvider::new(network.online_peers_receiver()),
            Some(sumeragi.clone()),
            runtime_deps,
        );
        let torii = torii.with_rbc_store_dir(rbc_store_dir.clone());
        let torii = torii.with_p2p(network.clone());
        let torii = torii.with_local_peer_id(config.common.peer.id.clone());
        let torii_run = torii.start(supervisor.shutdown_signal());
        let shutdown_on_failure = supervisor.shutdown_signal();
        supervisor.monitor(Child::new(
            tokio::spawn(async move {
                if let Err(err) = torii_run.await {
                    iroha_logger::error!(?err, "Torii failed to terminate gracefully");
                    shutdown_on_failure.send();
                    std::process::exit(1);
                } else {
                    iroha_logger::debug!("Torii exited normally");
                }
            }),
            OnShutdown::Wait(Duration::from_secs(5)),
        ));

        let suppress_pow_broadcast = Arc::new(AtomicBool::new(false));
        let pow_update_version = Arc::new(AtomicU64::new(1));
        supervisor.monitor(task::spawn(
            NetworkRelay {
                sumeragi,
                block_sync,
                tx_gossiper,
                peers_gossiper,
                network: network.clone(),
                streaming: streaming.clone(),
                kiso: kiso.clone(),
                suppress_pow_broadcast: Arc::clone(&suppress_pow_broadcast),
                pow_update_version: Arc::clone(&pow_update_version),
                consensus_ingress: ConsensusIngressLimiter::from_config(
                    &config.network,
                    &config.sumeragi,
                ),
                low_priority_ingress: LowPriorityIngressLimiter::from_config(&config.network),
            }
            .run(),
        ));
        // Start ZK lane (non-forking trace verification) if enabled in config
        if let Some((_h, child)) = iroha_core::pipeline::zk_lane::start(&zk_cfg.halo2) {
            supervisor.monitor(Child::new(child, OnShutdown::Wait(Duration::from_secs(1))));
        }
        // Start FASTPQ prover lane (background STARK generation) if the backend initialises.
        if let Some((_h, child)) = iroha_core::fastpq::lane::start(&zk_cfg.fastpq) {
            supervisor.monitor(Child::new(child, OnShutdown::Wait(Duration::from_secs(1))));
        }
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
        let suppress_pow_broadcast_for_relay = suppress_pow_broadcast.clone();
        let pow_update_version_for_relay = pow_update_version.clone();
        supervisor.monitor(tokio::task::spawn(async move {
            if let Err(err) = config_updates_relay(
                kiso,
                logger,
                net_for_relay,
                suppress_pow_broadcast_for_relay,
                pow_update_version_for_relay,
            )
            .await
            {
                iroha_logger::error!(?err, "Config updates relay exited");
            }
        }));

        supervisor
            .setup_shutdown_on_os_signals()
            .change_context(StartError::ListenOsSignal)?;

        supervisor.shutdown_on_external_signal(shutdown_signal);

        Ok((
            Self {
                kura,
                state,
                soracloud_runtime,
                streaming: streaming.clone(),
                network: network.clone(),
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

    /// Access the embedded Soracloud runtime-manager handle.
    pub fn soracloud_runtime(&self) -> &SoracloudRuntimeManagerHandle {
        &self.soracloud_runtime
    }

    /// Streaming handle used for Torii and telemetry ingress.
    pub fn streaming(&self) -> iroha_core::streaming::StreamingHandle {
        self.streaming.clone()
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

/// Spawns a task which subscribes on updates from the configuration actor
/// and broadcasts them further to interested actors. This way, neither the config actor nor other ones know
/// about each other, achieving loose coupling of code and system.
#[allow(clippy::too_many_lines)]
async fn config_updates_relay(
    kiso: KisoHandle,
    logger: LoggerHandle,
    network: iroha_core::IrohaNetwork,
    suppress_pow_broadcast: Arc<AtomicBool>,
    pow_update_version: Arc<AtomicU64>,
) -> EyreResult<()> {
    let mut log_level_update = kiso.subscribe_on_logger_updates().await?;
    let mut acl_update = kiso.subscribe_on_network_acl_updates().await?;
    let mut handshake_update = kiso.subscribe_on_soranet_handshake_updates().await?;
    let mut online_peers_update = network.online_peers_receiver();
    let mut known_peers: HashSet<PeerId> = online_peers_update
        .borrow()
        .iter()
        .map(|peer| peer.id().clone())
        .collect();
    #[cfg(feature = "telemetry")]
    let mut confidential_gas_update = kiso.subscribe_on_confidential_gas_updates().await?;
    #[cfg(feature = "telemetry")]
    let confidential_metrics_handle = iroha_telemetry::metrics::global().cloned();
    #[cfg(feature = "telemetry")]
    if let (Some(metrics), gas) = (
        confidential_metrics_handle.as_ref(),
        *confidential_gas_update.borrow(),
    ) {
        metrics.set_confidential_gas_schedule(&gas);
    }
    #[cfg(feature = "telemetry")]
    if let Some(metrics) = confidential_metrics_handle.as_ref() {
        let digest = ivm::gas::schedule_hash();
        metrics.set_ivm_gas_schedule_hash(digest.as_ref());
    }
    // Emit the current handshake configuration immediately so runtime components inherit puzzle settings.
    let initial_handshake = handshake_update.borrow().clone();
    network.update_soranet_handshake(initial_handshake.clone());
    // Broadcast the baseline PoW/puzzle policy before any runtime updates so new peers inherit
    // the consensus-backed guard rails even if they join before the first config change.
    let initial_pow_version = pow_update_version.load(Ordering::SeqCst);
    let mut pow_payload = pow_update_payload(&initial_handshake.pow, initial_pow_version);
    let pow_broadcast_generation = Arc::new(AtomicU64::new(0));
    if let Some(payload) = pow_payload.clone() {
        broadcast_pow_payload(payload, &network, &pow_broadcast_generation);
    }

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
                    network.update_soranet_handshake(value.clone());
                    let was_suppressed =
                        suppress_pow_broadcast.swap(false, Ordering::SeqCst);
                    let next_version = if was_suppressed {
                        pow_update_version.load(Ordering::SeqCst)
                    } else {
                        pow_update_version
                            .fetch_add(1, Ordering::SeqCst)
                            .saturating_add(1)
                    };
                    pow_payload = pow_update_payload(&value.pow, next_version);
                    if was_suppressed {
                        // A fresh config landed from a remote peer; stop stale retry loops.
                        bump_pow_broadcast_generation(&pow_broadcast_generation);
                    } else if let Some(payload) = pow_payload.clone() {
                        broadcast_pow_payload(payload, &network, &pow_broadcast_generation);
                    } else {
                        // PoW disabled: cancel any in-flight retries of older payloads.
                        bump_pow_broadcast_generation(&pow_broadcast_generation);
                    }
                } else {
                    iroha_logger::debug!("Exiting config updates relay (handshake channel closed)");
                    break;
                }
            },
            result = online_peers_update.changed() => {
                if let Ok(()) = result {
                    let snapshot = online_peers_update.borrow();
                    let mut current = HashSet::with_capacity(snapshot.len());
                    for peer in snapshot.iter() {
                        let peer_id = peer.id().clone();
                        if !known_peers.contains(&peer_id) {
                            if let Some(payload) = pow_payload.as_ref() {
                                network.post(iroha_p2p::Post {
                                    data: iroha_core::NetworkMessage::SoranetPowConfig(
                                        payload.clone()
                                    ),
                                    peer_id: peer_id.clone(),
                                    priority: iroha_p2p::Priority::High,
                                });
                            }
                        }
                        current.insert(peer_id);
                    }
                    known_peers = current;
                } else {
                    iroha_logger::debug!(
                        "Exiting config updates relay (online peers channel closed)"
                    );
                    break;
                }
            },
            result = confidential_gas_update.changed() => {
                if let Ok(()) = result {
                    if let Some(metrics) = confidential_metrics_handle.as_ref() {
                        let gas = *confidential_gas_update.borrow_and_update();
                        metrics.set_confidential_gas_schedule(&gas);
                    }
                } else {
                    iroha_logger::debug!("Exiting config updates relay (confidential gas channel closed)");
                    break;
                }
            }
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
                    network.update_soranet_handshake(value.clone());
                    let was_suppressed =
                        suppress_pow_broadcast.swap(false, Ordering::SeqCst);
                    let next_version = if was_suppressed {
                        pow_update_version.load(Ordering::SeqCst)
                    } else {
                        pow_update_version
                            .fetch_add(1, Ordering::SeqCst)
                            .saturating_add(1)
                    };
                    pow_payload = pow_update_payload(&value.pow, next_version);
                    if was_suppressed {
                        // A fresh config landed from a remote peer; stop stale retry loops.
                        bump_pow_broadcast_generation(&pow_broadcast_generation);
                    } else if let Some(payload) = pow_payload.clone() {
                        broadcast_pow_payload(payload, &network, &pow_broadcast_generation);
                    } else {
                        // PoW disabled: cancel any in-flight retries of older payloads.
                        bump_pow_broadcast_generation(&pow_broadcast_generation);
                    }
                } else {
                    iroha_logger::debug!("Exiting config updates relay (handshake channel closed)");
                    break;
                }
            },
            result = online_peers_update.changed() => {
                if let Ok(()) = result {
                    let snapshot = online_peers_update.borrow();
                    let mut current = HashSet::with_capacity(snapshot.len());
                    for peer in snapshot.iter() {
                        let peer_id = peer.id().clone();
                        if !known_peers.contains(&peer_id) {
                            if let Some(payload) = pow_payload.as_ref() {
                                network.post(iroha_p2p::Post {
                                    data: iroha_core::NetworkMessage::SoranetPowConfig(
                                        payload.clone()
                                    ),
                                    peer_id: peer_id.clone(),
                                    priority: iroha_p2p::Priority::High,
                                });
                            }
                        }
                        current.insert(peer_id);
                    }
                    known_peers = current;
                } else {
                    iroha_logger::debug!(
                        "Exiting config updates relay (online peers channel closed)"
                    );
                    break;
                }
            }
        };
    }

    Ok(())
}

fn pow_update_payload(
    pow: &iroha_config::parameters::actual::SoranetPow,
    version: u64,
) -> Option<Vec<u8>> {
    if !pow.required {
        return None;
    }
    let broadcast = iroha_core::SoranetPowConfigBroadcast {
        version,
        required: pow.required,
        difficulty: pow.difficulty,
        max_future_skew_secs: pow.max_future_skew.as_secs(),
        min_ticket_ttl_secs: pow.min_ticket_ttl.as_secs(),
        ticket_ttl_secs: pow.ticket_ttl.as_secs(),
        puzzle: pow
            .puzzle
            .map(|p| iroha_core::SoranetPuzzleConfigBroadcast {
                memory_kib: p.memory_kib.get(),
                time_cost: p.time_cost.get(),
                lanes: p.lanes.get(),
            }),
    };
    let payload = norito::json::to_json(&broadcast)
        .expect("broadcast is serializable")
        .into_bytes();
    Some(payload)
}

fn bump_pow_broadcast_generation(generation: &AtomicU64) {
    generation.fetch_add(1, Ordering::SeqCst);
}

fn broadcast_pow_payload(
    payload: Vec<u8>,
    network: &iroha_core::IrohaNetwork,
    generation: &Arc<AtomicU64>,
) {
    // Bump generation so any in-flight payload attempt is considered stale.
    generation.fetch_add(1, Ordering::SeqCst);

    network.broadcast(iroha_p2p::Broadcast {
        data: iroha_core::NetworkMessage::SoranetPowConfig(payload),
        priority: iroha_p2p::Priority::High,
    });
}

fn read_stored_genesis_block(
    kura: &Kura,
    block_count: iroha_core::kura::BlockCount,
) -> ReportResult<Option<GenesisBlock>, StartError> {
    if block_count.0 == 0 {
        return Ok(None);
    }

    let nz = std::num::NonZeroUsize::new(1).expect("nonzero");
    let stored = kura.get_block(nz).ok_or_else(|| {
        Report::new(StartError::InitKura)
            .attach("non-empty block store is missing genesis block at height 1")
    })?;
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
    Account::new(genesis_account_id.to_account_id(iroha_genesis::GENESIS_DOMAIN_ID.clone()))
        .build(&genesis_account_id)
}

fn genesis_domain(public_key: PublicKey) -> Domain {
    let genesis_account_id = AccountId::new(public_key);
    Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id)
}

#[cfg(test)]
mod genesis_key_tests {
    use super::*;
    use iroha_genesis::GenesisBuilder;
    use std::path::PathBuf;

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
    /// Genesis roster contained only a single peer.
    LonePeer,
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
    /// Failed to bind a configured address.
    CannotBindAddress {
        /// Address that could not be bound.
        addr: SocketAddr,
    },
    /// Multi-lane Nexus catalogs require the Nexus runtime to be enabled.
    NexusMultilaneDisabled,
    /// Joining Sora profile is mandatory but missing.
    SoraProfileRequired,
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
            Self::LonePeer => write!(f, "The network consists from this one peer only"),
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
                    "Sora Nexus features require `irohad --sora`; remove the Sora-only config overrides or rerun with the flag"
                )
            }
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

fn ivm_stack_budget_bytes(config: &Config) -> u64 {
    config
        .compute
        .resource_profiles
        .get(&config.ivm.memory_budget_profile)
        .map(|budget| budget.max_stack_bytes.get())
        .expect("ivm.memory_budget_profile missing from compute.resource_profiles")
}

/// Apply concurrency settings (IVM scheduler + Rayon) derived from configuration.
fn apply_concurrency_config(
    concurrency: &iroha_config::parameters::actual::Concurrency,
    stack_budget_bytes: u64,
) {
    let stack_outcome = ivm::apply_stack_sizes(
        concurrency.scheduler_stack_bytes,
        concurrency.prover_stack_bytes,
        concurrency.guest_stack_bytes,
        stack_budget_bytes,
    );
    if stack_outcome.scheduler_clamped
        || stack_outcome.prover_clamped
        || stack_outcome.guest_clamped
        || stack_outcome.budget_clamped
    {
        iroha_logger::warn!(
            requested_scheduler_bytes = stack_outcome.requested_scheduler_bytes,
            requested_prover_bytes = stack_outcome.requested_prover_bytes,
            requested_guest_bytes = stack_outcome.requested_guest_bytes,
            requested_budget_bytes = stack_outcome.requested_budget_bytes,
            scheduler_bytes = stack_outcome.scheduler_bytes,
            prover_bytes = stack_outcome.prover_bytes,
            guest_bytes = stack_outcome.guest_bytes,
            budget_bytes = stack_outcome.budget_bytes,
            min_stack_bytes = ivm::MIN_STACK_BYTES,
            max_stack_bytes = ivm::MAX_STACK_BYTES,
            "Stack size overrides were clamped to the supported range"
        );
    }
    ivm::set_gas_to_stack_multiplier(concurrency.gas_to_stack_multiplier);
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
        config = config
            .read_toml_with_extends(path)
            .change_context(ConfigError::ReadConfig)?;
    }

    let mut config = config
        .read_and_complete::<UserConfig>()
        .change_context(ConfigError::ReadConfig)?
        .parse()
        .change_context(ConfigError::ParseConfig)?;

    if args.sora {
        config.apply_sora_profile();
    }
    config.apply_storage_budget();

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

    let stack_budget_bytes = ivm_stack_budget_bytes(&config);
    apply_concurrency_config(&config.concurrency, stack_budget_bytes);

    // Apply Norito settings immediately so subsequent Norito decode/encode (e.g., genesis)
    // uses the configured heuristics and GPU offload policy.
    apply_norito_config(&config);

    // Apply hardware acceleration configuration for IVM (Metal/CUDA). Defaults enable all
    // available hardware; config can cap GPUs or disable specific backends. This does not
    // change outputs, only performance characteristics.
    apply_ivm_acceleration_config(&config.accel);
    rs16::set_simd_enabled(config.accel.enable_simd);

    iroha_data_model::account::address::set_default_domain_name(
        config.common.default_account_domain_label.value().clone(),
    )
    .map_err(|err| {
        Report::new(ConfigError::ParseConfig).attach(format!(
            "invalid default account domain label `{}`: {err}",
            config.common.default_account_domain_label.value()
        ))
    })?;
    iroha_data_model::account::address::set_chain_discriminant(
        *config.common.chain_discriminant.value(),
    );

    let genesis = if let Some(signed_file) = &config.genesis.file {
        let genesis = read_genesis(&signed_file.resolve_relative_path())
            .attach(signed_file.clone().into_attachment().display_path())?;
        Some(genesis)
    } else {
        None
    };

    config.logger.terminal_colors = args.terminal_colors;

    Ok((config, genesis))
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
    use iroha_crypto::Hash;
    use iroha_data_model::nexus::{DataSpaceId, LaneCatalog, LaneConfig, LaneId};
    use std::{io::Write, num::NonZeroU32, path::Path};
    use tempfile::NamedTempFile;
    use toml::Table;

    fn minimal_config_table() -> Table {
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
        "7f66b7a2fd69ea5ab9eacce7c3315a4a3b2fb1a5f110115f68594a54d94174a7";

    fn file_blake2b_hex(path: &Path) -> String {
        let bytes = std::fs::read(path).expect("read file");
        Hash::new(bytes).to_string()
    }

    #[test]
    fn build_line_env_override_takes_precedence() {
        assert_eq!(
            resolve_build_line_from_env(Some("iroha2".to_owned()), "irohad"),
            BuildLine::Iroha2
        );
        assert_eq!(
            resolve_build_line_from_env(Some("iroha3".to_owned()), "irohad"),
            BuildLine::Iroha3
        );
        assert_eq!(
            resolve_build_line_from_env(Some("unknown".to_owned()), "irohad"),
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
    fn iroha2_preserves_da_flag() {
        let mut enabled_config =
            Config::from_toml_source(TomlSource::inline(minimal_config_table()))
                .expect("default config");
        enabled_config.sumeragi.da.enabled = true;

        enforce_build_line(BuildLine::Iroha2, &mut enabled_config)
            .expect("iroha2 should keep DA configurable");

        assert!(enabled_config.sumeragi.da.enabled);

        let mut disabled_config =
            Config::from_toml_source(TomlSource::inline(minimal_config_table()))
                .expect("default config");
        disabled_config.sumeragi.da.enabled = false;

        enforce_build_line(BuildLine::Iroha2, &mut disabled_config)
            .expect("iroha2 should keep DA configurable");

        assert!(!disabled_config.sumeragi.da.enabled);
    }

    #[test]
    fn iroha3_rejects_da_disabled() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        config.sumeragi.da.enabled = false;

        let err = enforce_build_line(BuildLine::Iroha3, &mut config)
            .expect_err("iroha3 should reject DA-disabled config");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("sumeragi.da.enabled"),
            "error should point at sumeragi.da.enabled: {rendered}"
        );
        assert!(
            !config.sumeragi.da.enabled,
            "DA override should not mutate the config"
        );
    }

    #[test]
    fn iroha3_rejects_permissioned_consensus_with_nexus_enabled() {
        let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
            .expect("default config");
        config.sumeragi.da.enabled = true;
        config.nexus.enabled = true;
        config.sumeragi.consensus_mode =
            iroha_config::parameters::actual::ConsensusMode::Permissioned;

        let err = enforce_build_line(BuildLine::Iroha3, &mut config)
            .expect_err("iroha3 should reject permissioned consensus with nexus");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("NPoS"),
            "error should mention NPoS requirement: {rendered}"
        );
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
                    dataspace_id: DataSpaceId::GLOBAL,
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
        let config = Config::from_toml_source(
            TomlSource::from_file(path).expect("read nexus defaults config"),
        )
        .expect("parse nexus defaults");

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
            "irohad",
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
            "irohad",
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
            "irohad",
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

    #[test]
    fn accel_config_disables_cuda_parity_holds() {
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

fn read_genesis(path: &Path) -> ReportResult<GenesisBlock, ConfigError> {
    const PANIC_HELP: &str = concat!(
        "Genesis decode panicked. A common cause is an invalid `Name` (identifiers ",
        "must not contain whitespace or the characters `@`, `#`, `$`). ",
        "Please sanitize identifiers in your genesis and re-sign the file."
    );

    // Ensure the instruction registry is populated before attempting to
    // decode the genesis block. Tests may invoke this function directly
    // without calling `init_genesis_instruction_registry` beforehand, which
    // would otherwise cause a panic when deserializing `InstructionBox`
    // values.
    init_genesis_instruction_registry();
    init_query_registry();

    let bytes = std::fs::read(path).change_context(ConfigError::ReadGenesis)?;

    // Norito decoding may panic inside data-model validators (e.g., `Name`) if
    // the encoded genesis contains invalid identifiers. Catch panics to provide
    // a clear diagnostic instead of aborting the process.
    let decoded = std::panic::catch_unwind(|| decode_framed_signed_block(&bytes));

    match decoded {
        Ok(Ok(genesis)) => Ok(GenesisBlock(genesis)),
        Ok(Err(versioned_err)) => Err(versioned_err).change_context(ConfigError::ReadGenesis),
        Err(_panic) => Err(Report::new(ConfigError::ReadGenesis).attach(PANIC_HELP)),
    }
}

fn resolve_norito_max_archive_len(cfg: &Config) -> u64 {
    let requested = cfg.norito.max_archive_len;
    let rbc_store_max = u64::try_from(cfg.sumeragi.rbc.store_max_bytes).unwrap_or(u64::MAX);
    let max_frame_bytes = u64::try_from(cfg.network.max_frame_bytes).unwrap_or(u64::MAX);
    let resolved = requested.max(rbc_store_max).max(max_frame_bytes);

    if resolved != requested {
        iroha_logger::warn!(
            target: "config",
            requested,
            rbc_store_max,
            max_frame_bytes,
            resolved,
            "Norito max_archive_len too small for configured RBC store or network frame; increasing to keep consensus payloads decodable"
        );
    }

    resolved
}

/// Apply Norito codec configuration (heuristics + GPU offload gate) from config.
fn apply_norito_config(cfg: &Config) {
    // Capture requested heuristics to detect configuration drift. The codec uses
    // a fixed canonical profile; runtime overrides are no longer supported.
    let requested = norito::core::heuristics::Heuristics {
        min_compress_bytes_cpu: cfg.norito.min_compress_bytes_cpu,
        min_compress_bytes_gpu: cfg.norito.min_compress_bytes_gpu,
        zstd_level_small: cfg.norito.zstd_level_small,
        zstd_level_large: cfg.norito.zstd_level_large,
        zstd_level_gpu: cfg.norito.zstd_level_gpu,
        large_threshold: cfg.norito.large_threshold,
        aos_ncb_small_n: cfg.norito.aos_ncb_small_n,
        ..norito::core::heuristics::Heuristics::canonical()
    };
    let canonical = norito::core::heuristics::Heuristics::canonical();
    if requested != canonical {
        iroha_logger::warn!(
            target: "config",
            ?requested,
            ?canonical,
            "Norito heuristics overrides detected in config; ignoring overrides and using canonical codec profile"
        );
    }
    let max_archive_len = resolve_norito_max_archive_len(cfg);
    norito::core::set_max_archive_len(max_archive_len);
    // Gate GPU compression offload for deterministic profiles if desired.
    norito::core::hw::set_gpu_compression_allowed(cfg.norito.allow_gpu_compression);
}

/// Enforce build-line specific Sumeragi DA/RBC policy:
/// - Iroha 2 honours the configured flags (defaults keep DA/RBC off).
/// - Iroha 3 always requires DA with RBC.
fn enforce_da_rbc_policy(build_line: BuildLine, config: &Config) -> ReportResult<(), MainError> {
    if build_line.is_iroha3() && !config.sumeragi.da.enabled {
        return Err(Report::new(MainError::Config)
            .attach("Iroha 3 requires DA/RBC; set sumeragi.da.enabled=true in the configuration"));
    }
    Ok(())
}

fn validate_config(config: &Config) -> ReportResult<(), ConfigError> {
    let mut emitter = Emitter::new();

    validate_config_io(&mut emitter, config);
    validate_config_runtime(&mut emitter, config);

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
    validate_directory_path(emitter, &config.kura.store_dir);
    // maybe validate only if snapshot mode is enabled
    validate_directory_path(emitter, &config.snapshot.store_dir);

    if config.genesis.file.is_none()
        && !config
            .common
            .trusted_peers
            .value()
            .contains_other_trusted_peers()
    {
        emitter.emit(Report::new(ConfigError::LonePeer).attach("\
            Reason: the network consists from this one peer only (no `trusted_peers` provided).\n\
            Since `genesis.file` is not set, there is no way to receive the genesis block.\n\
            Either provide the genesis by setting `genesis.file` configuration parameter,\n\
            or increase the number of trusted peers in the network using `trusted_peers` configuration parameter.\
        ").attach(config.common.trusted_peers.clone().into_attachment().display_as_debug()));
    }

    if config.network.address.value() == config.torii.address.value() {
        emitter.emit(
            Report::new(ConfigError::SameNetworkAndToriiAddrs)
                .attach(config.network.address.clone().into_attachment())
                .attach(config.torii.address.clone().into_attachment()),
        );
    }
}

fn validate_config_runtime(emitter: &mut Emitter<ConfigError>, config: &Config) {
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

    if config.compute.enabled {
        let guest_stack = config.concurrency.guest_stack_bytes;
        let budget_stack = config
            .compute
            .resource_profiles
            .get(&config.ivm.memory_budget_profile)
            .map_or_else(|| guest_stack.max(1), |budget| budget.max_stack_bytes.get());
        if guest_stack < budget_stack {
            log_config_warning(&format!(
                "concurrency.guest_stack_bytes ({guest_stack}) is smaller than ivm.memory_budget_profile `{}` max_stack_bytes ({budget_stack}); guest stack limits will be clamped to the smaller value",
                config.ivm.memory_budget_profile
            ));
        } else if guest_stack != budget_stack {
            log_config_warning(&format!(
                "concurrency.guest_stack_bytes ({guest_stack}) differs from ivm.memory_budget_profile `{}` max_stack_bytes ({budget_stack}); effective stacks use the minimum of the caps",
                config.ivm.memory_budget_profile
            ));
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

/// Resolve the build line from an explicit env override or the binary name.
fn resolve_build_line() -> BuildLine {
    resolve_build_line_from_env(env::var(BUILD_LINE_ENV).ok(), env!("CARGO_BIN_NAME"))
}

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

fn main() {
    let build_line = resolve_build_line();
    if let Err(report) = run_main(build_line) {
        eprintln!("{report:?}");
        std::process::exit(1);
    }
}

fn parse_fastpq_execution_mode(value: &str) -> Result<FastpqExecutionMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(FastpqExecutionMode::Auto),
        "cpu" => Ok(FastpqExecutionMode::Cpu),
        "gpu" => Ok(FastpqExecutionMode::Gpu),
        _ => Err("expected MODE to be one of: auto, cpu, gpu".to_string()),
    }
}

fn parse_fastpq_poseidon_mode(value: &str) -> Result<FastpqPoseidonMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(FastpqPoseidonMode::Auto),
        "cpu" => Ok(FastpqPoseidonMode::Cpu),
        "gpu" => Ok(FastpqPoseidonMode::Gpu),
        _ => Err("expected MODE to be one of: auto, cpu, gpu".to_string()),
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
        filtered.push(OsString::from("irohad"));
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
                            index: lane.index,
                            dispatch_count: lane.dispatch_count,
                            max_in_flight: lane.max_in_flight,
                            busy_ms: lane.busy_ms,
                            overlap_ms: lane.overlap_ms,
                        });
                    }
                    let sample = FastpqMetalQueueSample {
                        limit: stats.limit,
                        max_in_flight: stats.max_in_flight,
                        dispatch_count: stats.dispatch_count,
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

fn run_main(build_line: BuildLine) -> ReportResult<(), MainError> {
    let args = parse_args();

    let lang = i18n::detect_language(args.language.as_deref());
    i18n::init(lang);

    configure_reports(&args);

    if args.trace_config {
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
    iroha_logger::info!(
        target: "config",
        build_line = %build_line,
        da_enabled = config.sumeragi.da.enabled,
        "Resolved build line and consensus DA policy"
    );

    #[cfg(feature = "telemetry")]
    let fastpq_device_labels = FastpqDeviceLabels::from_config(&config.zk.fastpq);
    #[cfg(feature = "telemetry")]
    install_fastpq_execution_mode_probe(&fastpq_device_labels);
    #[cfg(feature = "telemetry")]
    install_fastpq_poseidon_probe(&fastpq_device_labels);
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
    // to prevent HTTP/p2p tasks from starving under RBC/DA load, and use the
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
        .enable_all()
        .build()
        .map_err(Report::from)
        .change_context(MainError::IrohaStart)?;

    rt.block_on(run_node(config, genesis))
}

fn enforce_build_line(build_line: BuildLine, config: &mut Config) -> ReportResult<(), MainError> {
    enforce_da_rbc_policy(build_line, config)?;

    if build_line.is_iroha3() {
        if config.nexus.enabled
            && config.sumeragi.consensus_mode
                != iroha_config::parameters::actual::ConsensusMode::Npos
        {
            return Err(Report::new(MainError::Config).attach(
                "Nexus requires the global NPoS validator set; set sumeragi.consensus_mode = \"npos\" or disable nexus.enabled",
            ));
        }
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
    sumeragi: &iroha_config::parameters::actual::Sumeragi,
) -> ReportResult<iroha_p2p::ConsensusConfigCaps, StartError> {
    let collectors_k = u16::try_from(sumeragi.collectors.k).map_err(|_| {
        Report::new(StartError::StartP2p)
            .attach("sumeragi.collectors.k exceeds handshake limits (must fit into u16)")
    })?;
    let rbc_chunk_max_bytes = u64::try_from(sumeragi.rbc.chunk_max_bytes).map_err(|_| {
        Report::new(StartError::StartP2p).attach(
            "sumeragi.advanced.rbc.chunk_max_bytes exceeds handshake limits (must fit into u64)",
        )
    })?;
    let rbc_store_max_bytes = u64::try_from(sumeragi.rbc.store_max_bytes).map_err(|_| {
        Report::new(StartError::StartP2p).attach(
            "sumeragi.advanced.rbc.store_max_bytes exceeds handshake limits (must fit into u64)",
        )
    })?;
    let rbc_store_soft_bytes = u64::try_from(sumeragi.rbc.store_soft_bytes).map_err(|_| {
        Report::new(StartError::StartP2p).attach(
            "sumeragi.advanced.rbc.store_soft_bytes exceeds handshake limits (must fit into u64)",
        )
    })?;
    let rbc_store_max_sessions = u32::try_from(sumeragi.rbc.store_max_sessions).map_err(|_| {
        Report::new(StartError::StartP2p).attach(
            "sumeragi.advanced.rbc.store_max_sessions exceeds handshake limits (must fit into u32)",
        )
    })?;
    let rbc_store_soft_sessions =
        u32::try_from(sumeragi.rbc.store_soft_sessions).map_err(|_| {
            Report::new(StartError::StartP2p).attach(
                "sumeragi.advanced.rbc.store_soft_sessions exceeds handshake limits (must fit into u32)",
            )
        })?;
    let rbc_session_ttl_ms = u64::try_from(sumeragi.rbc.session_ttl.as_millis()).map_err(|_| {
        Report::new(StartError::StartP2p).attach(
            "sumeragi.advanced.rbc.session_ttl exceeds handshake limits (must fit into u64 milliseconds)",
        )
    })?;

    Ok(iroha_p2p::ConsensusConfigCaps {
        collectors_k,
        redundant_send_r: sumeragi.collectors.redundant_send_r,
        da_enabled: sumeragi.da.enabled,
        rbc_chunk_max_bytes,
        rbc_session_ttl_ms,
        rbc_store_max_sessions,
        rbc_store_soft_sessions,
        rbc_store_max_bytes,
        rbc_store_soft_bytes,
    })
}

fn consensus_caps_from_genesis(
    genesis: &GenesisBlock,
    chain_id: &ChainId,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    sumeragi: &iroha_config::parameters::actual::Sumeragi,
) -> Option<(String, String, iroha_p2p::ConsensusHandshakeCaps)> {
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

    let entry = handshake_entries
        .iter()
        .filter(|entry| {
            entry
                .wire_proto_versions
                .contains(&iroha_core::sumeragi::consensus::PROTO_VERSION)
        })
        .find_map(|entry| {
            let advertised = parse_consensus_handshake_fingerprint(&entry.consensus_fingerprint)?;
            let (_, _, fingerprint) = consensus_entry_caps(chain_id, entry, &params, sumeragi);
            (advertised == fingerprint).then_some(entry)
        })
        .or_else(|| {
            handshake_entries.iter().find(|entry| {
                entry
                    .wire_proto_versions
                    .contains(&iroha_core::sumeragi::consensus::PROTO_VERSION)
            })
        })
        .or_else(|| handshake_entries.first())?;

    let (mode_tag, _consensus_params, computed_fingerprint) =
        consensus_entry_caps(chain_id, entry, &params, sumeragi);

    Some((
        mode_tag.clone(),
        entry.bls_domain.clone(),
        iroha_p2p::ConsensusHandshakeCaps {
            mode_tag,
            proto_version: iroha_core::sumeragi::consensus::PROTO_VERSION,
            consensus_fingerprint: computed_fingerprint,
            config: *config_caps,
        },
    ))
}

fn parse_consensus_handshake_fingerprint(raw: &str) -> Option<[u8; 32]> {
    let mut bytes = [0_u8; 32];
    let fingerprint = raw.trim_start_matches("0x");
    hex::decode_to_slice(fingerprint, &mut bytes).ok()?;
    Some(bytes)
}

fn consensus_entry_caps(
    chain_id: &ChainId,
    entry: &ConsensusHandshakeMeta,
    params: &iroha_data_model::parameter::Parameters,
    sumeragi: &iroha_config::parameters::actual::Sumeragi,
) -> (
    String,
    iroha_data_model::block::consensus::ConsensusGenesisParams,
    [u8; 32],
) {
    let mode_tag = match entry.mode.as_str() {
        "Npos" => iroha_core::sumeragi::consensus::NPOS_TAG,
        _ => iroha_core::sumeragi::consensus::PERMISSIONED_TAG,
    };
    let use_npos = mode_tag == iroha_core::sumeragi::consensus::NPOS_TAG;

    let npos_payload = params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter);
    let epoch_length_blocks = if use_npos {
        npos_payload.as_ref().map_or(
            sumeragi.npos.epoch_length_blocks,
            SumeragiNposParameters::epoch_length_blocks,
        )
    } else {
        0
    };
    let npos_timeouts = if use_npos {
        let min_finality_ms = params.sumeragi().min_finality_ms.max(1);
        let mut block_time_ms = params.sumeragi().block_time_ms.max(1);
        if block_time_ms < min_finality_ms {
            block_time_ms = min_finality_ms;
        }
        Some(
            sumeragi
                .npos
                .timeouts_overrides
                .resolve(Duration::from_millis(block_time_ms)),
        )
    } else {
        None
    };
    let npos_params = if use_npos {
        let timeouts = npos_timeouts.expect("timeouts computed for NPoS");
        let duration_ms = |value: Duration| -> u64 {
            let ms = value.as_millis();
            u64::try_from(ms).expect("NPoS timeout exceeds millisecond range")
        };
        Some(match npos_payload {
            Some(npos) => iroha_data_model::block::consensus::NposGenesisParams {
                block_time_ms: params.sumeragi().block_time_ms,
                timeout_propose_ms: duration_ms(timeouts.propose),
                timeout_prevote_ms: duration_ms(timeouts.prevote),
                timeout_precommit_ms: duration_ms(timeouts.precommit),
                timeout_commit_ms: duration_ms(timeouts.commit),
                timeout_da_ms: duration_ms(timeouts.da),
                timeout_aggregator_ms: duration_ms(timeouts.aggregator),
                k_aggregators: npos.k_aggregators(),
                redundant_send_r: npos.redundant_send_r(),
                epoch_seed: npos.epoch_seed(),
                vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                max_validators: npos.max_validators(),
                min_self_bond: npos.min_self_bond(),
                min_nomination_bond: npos.min_nomination_bond(),
                max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                seat_band_pct: npos.seat_band_pct,
                max_entity_correlation_pct: npos.max_entity_correlation_pct,
                finality_margin_blocks: npos.finality_margin_blocks(),
                evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                activation_lag_blocks: npos.activation_lag_blocks(),
                slashing_delay_blocks: npos.slashing_delay_blocks(),
            },
            None => {
                let chain_hash = iroha_crypto::Hash::new(chain_id.clone().into_inner().as_bytes());
                let epoch_seed: [u8; 32] = chain_hash.into();
                iroha_data_model::block::consensus::NposGenesisParams {
                    block_time_ms: params.sumeragi().block_time_ms,
                    timeout_propose_ms: duration_ms(timeouts.propose),
                    timeout_prevote_ms: duration_ms(timeouts.prevote),
                    timeout_precommit_ms: duration_ms(timeouts.precommit),
                    timeout_commit_ms: duration_ms(timeouts.commit),
                    timeout_da_ms: duration_ms(timeouts.da),
                    timeout_aggregator_ms: duration_ms(timeouts.aggregator),
                    k_aggregators: u16::try_from(sumeragi.collectors.k)
                        .expect("sumeragi.collectors.k must fit into u16"),
                    redundant_send_r: sumeragi.collectors.redundant_send_r,
                    epoch_seed,
                    vrf_commit_window_blocks: sumeragi.npos.vrf.commit_window_blocks,
                    vrf_reveal_window_blocks: sumeragi.npos.vrf.reveal_window_blocks,
                    max_validators: sumeragi.npos.election.max_validators,
                    min_self_bond: sumeragi.npos.election.min_self_bond,
                    min_nomination_bond: sumeragi.npos.election.min_nomination_bond,
                    max_nominator_concentration_pct: sumeragi
                        .npos
                        .election
                        .max_nominator_concentration_pct,
                    seat_band_pct: sumeragi.npos.election.seat_band_pct,
                    max_entity_correlation_pct: sumeragi.npos.election.max_entity_correlation_pct,
                    finality_margin_blocks: sumeragi.npos.election.finality_margin_blocks,
                    evidence_horizon_blocks: sumeragi.npos.reconfig.evidence_horizon_blocks,
                    activation_lag_blocks: sumeragi.npos.reconfig.activation_lag_blocks,
                    slashing_delay_blocks: sumeragi.npos.reconfig.slashing_delay_blocks,
                }
            }
        })
    } else {
        None
    };

    let consensus_params = iroha_data_model::block::consensus::ConsensusGenesisParams {
        min_finality_ms: params.sumeragi().min_finality_ms,
        block_time_ms: params.sumeragi().block_time_ms,
        commit_time_ms: params.sumeragi().commit_time_ms,
        max_clock_drift_ms: params.sumeragi().max_clock_drift_ms,
        collectors_k: params.sumeragi().collectors_k,
        redundant_send_r: params.sumeragi().collectors_redundant_send_r,
        block_max_transactions: params.block().max_transactions().get(),
        da_enabled: params.sumeragi().da_enabled,
        epoch_length_blocks,
        bls_domain: entry.bls_domain.clone(),
        npos: npos_params,
    };

    let fingerprint = iroha_core::sumeragi::consensus::compute_consensus_fingerprint_from_params(
        chain_id,
        &consensus_params,
        &mode_tag,
    );

    (mode_tag.to_string(), consensus_params, fingerprint)
}

fn compute_consensus_handshake_caps(
    world: &impl WorldReadOnly,
    height: u64,
    config: &Config,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    override_caps: Option<(String, String, iroha_p2p::ConsensusHandshakeCaps)>,
) -> (String, String, iroha_p2p::ConsensusHandshakeCaps) {
    if let Some((mode_tag, bls_domain, caps)) = override_caps {
        return (mode_tag, bls_domain, caps);
    }

    iroha_core::sumeragi::consensus::compute_consensus_handshake_caps_from_world(
        world,
        height,
        &config.common,
        &config.sumeragi,
        config_caps,
    )
}

fn npos_validator_status_counts<'a>(
    validators: impl IntoIterator<Item = &'a PublicLaneValidatorRecord> + 'a,
) -> (usize, usize, usize, usize) {
    let mut active_bls = 0usize;
    let mut active_total = 0usize;
    let mut pending = 0usize;
    let mut total = 0usize;
    for record in validators {
        total = total.saturating_add(1);
        match record.status {
            PublicLaneValidatorStatus::Active => {
                active_total = active_total.saturating_add(1);
                if let Some(pk) = record.validator.try_signatory()
                    && pk.algorithm() == Algorithm::BlsNormal
                {
                    active_bls = active_bls.saturating_add(1);
                }
            }
            PublicLaneValidatorStatus::PendingActivation(_) => {
                pending = pending.saturating_add(1);
            }
            _ => {}
        }
    }
    (active_bls, active_total, pending, total)
}

#[allow(clippy::too_many_lines)]
fn verify_genesis_metadata(
    genesis: &GenesisBlock,
    config: &Config,
    consensus_caps: &iroha_p2p::ConsensusHandshakeCaps,
    mode_tag: &str,
    bls_domain: &str,
    proto_version: u32,
) -> ReportResult<(), MainError> {
    let mut instructions: Vec<InstructionBox> = Vec::new();
    for tx in genesis.0.external_transactions() {
        match tx.instructions() {
            Executable::Instructions(batch) => {
                instructions.extend(batch.iter().cloned());
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
        "Permissioned"
    } else if mode_tag == iroha_core::sumeragi::consensus::NPOS_TAG {
        "Npos"
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
        if meta.bls_domain != bls_domain {
            continue;
        }
        if !meta.wire_proto_versions.contains(&proto_version) {
            continue;
        }
        let meta_fp_clean = meta.consensus_fingerprint.trim_start_matches("0x");
        if meta_fp_clean.eq_ignore_ascii_case(&expected_fp_hex) {
            matched_meta = Some(meta.clone());
            break;
        }
    }
    let Some(matched_meta) = matched_meta else {
        let entries_summary = handshake_entries
            .iter()
            .map(|meta| {
                format!(
                    "{{mode={}, bls_domain={}, wire_proto_versions={:?}, fingerprint={}}}",
                    meta.mode,
                    meta.bls_domain,
                    meta.wire_proto_versions,
                    meta.consensus_fingerprint
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Report::new(MainError::Config).attach(format!(
            "none of the consensus_handshake_meta entries match runtime settings (expected consensus_mode `{expected_mode}`, bls_domain `{bls_domain}`, proto v{proto_version}, fingerprint 0x{expected_fp_hex}`); entries observed: {entries_summary}"
        )));
    };

    let mut params = iroha_data_model::parameter::Parameters::default();
    for set_param in instructions
        .iter()
        .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
    {
        params.set_parameter(set_param.inner().clone());
    }

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

    let use_npos = mode_tag == iroha_core::sumeragi::consensus::NPOS_TAG;
    let npos_payload = params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter);
    let epoch_length_blocks = if use_npos {
        npos_payload.as_ref().map_or(
            config.sumeragi.npos.epoch_length_blocks,
            SumeragiNposParameters::epoch_length_blocks,
        )
    } else {
        0
    };
    let npos_params = if use_npos {
        let min_finality_ms = params.sumeragi().min_finality_ms.max(1);
        let mut block_time_ms = params.sumeragi().block_time_ms.max(1);
        if block_time_ms < min_finality_ms {
            block_time_ms = min_finality_ms;
        }
        let timeouts = config
            .sumeragi
            .npos
            .timeouts_overrides
            .resolve(Duration::from_millis(block_time_ms));
        let duration_ms = |value: Duration| -> u64 {
            let ms = value.as_millis();
            u64::try_from(ms).expect("NPoS timeout exceeds millisecond range")
        };
        Some(match npos_payload {
            Some(npos) => iroha_data_model::block::consensus::NposGenesisParams {
                block_time_ms: params.sumeragi().block_time_ms,
                timeout_propose_ms: duration_ms(timeouts.propose),
                timeout_prevote_ms: duration_ms(timeouts.prevote),
                timeout_precommit_ms: duration_ms(timeouts.precommit),
                timeout_commit_ms: duration_ms(timeouts.commit),
                timeout_da_ms: duration_ms(timeouts.da),
                timeout_aggregator_ms: duration_ms(timeouts.aggregator),
                k_aggregators: npos.k_aggregators(),
                redundant_send_r: npos.redundant_send_r(),
                epoch_seed: npos.epoch_seed(),
                vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                max_validators: npos.max_validators(),
                min_self_bond: npos.min_self_bond(),
                min_nomination_bond: npos.min_nomination_bond(),
                max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                seat_band_pct: npos.seat_band_pct(),
                max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                finality_margin_blocks: npos.finality_margin_blocks(),
                evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                activation_lag_blocks: npos.activation_lag_blocks(),
                slashing_delay_blocks: npos.slashing_delay_blocks(),
            },
            None => {
                let chain_hash =
                    iroha_crypto::Hash::new(config.common.chain.clone().into_inner().as_bytes());
                let epoch_seed: [u8; 32] = chain_hash.into();
                iroha_data_model::block::consensus::NposGenesisParams {
                    block_time_ms: params.sumeragi().block_time_ms,
                    timeout_propose_ms: duration_ms(timeouts.propose),
                    timeout_prevote_ms: duration_ms(timeouts.prevote),
                    timeout_precommit_ms: duration_ms(timeouts.precommit),
                    timeout_commit_ms: duration_ms(timeouts.commit),
                    timeout_da_ms: duration_ms(timeouts.da),
                    timeout_aggregator_ms: duration_ms(timeouts.aggregator),
                    k_aggregators: u16::try_from(config.sumeragi.collectors.k)
                        .expect("sumeragi.collectors.k must fit into u16"),
                    redundant_send_r: config.sumeragi.collectors.redundant_send_r,
                    epoch_seed,
                    vrf_commit_window_blocks: config.sumeragi.npos.vrf.commit_window_blocks,
                    vrf_reveal_window_blocks: config.sumeragi.npos.vrf.reveal_window_blocks,
                    max_validators: config.sumeragi.npos.election.max_validators,
                    min_self_bond: config.sumeragi.npos.election.min_self_bond,
                    min_nomination_bond: config.sumeragi.npos.election.min_nomination_bond,
                    max_nominator_concentration_pct: config
                        .sumeragi
                        .npos
                        .election
                        .max_nominator_concentration_pct,
                    seat_band_pct: config.sumeragi.npos.election.seat_band_pct,
                    max_entity_correlation_pct: config
                        .sumeragi
                        .npos
                        .election
                        .max_entity_correlation_pct,
                    finality_margin_blocks: config.sumeragi.npos.election.finality_margin_blocks,
                    evidence_horizon_blocks: config.sumeragi.npos.reconfig.evidence_horizon_blocks,
                    activation_lag_blocks: config.sumeragi.npos.reconfig.activation_lag_blocks,
                    slashing_delay_blocks: config.sumeragi.npos.reconfig.slashing_delay_blocks,
                }
            }
        })
    } else {
        None
    };
    let consensus_params = iroha_data_model::block::consensus::ConsensusGenesisParams {
        min_finality_ms: params.sumeragi().min_finality_ms,
        block_time_ms: params.sumeragi().block_time_ms,
        commit_time_ms: params.sumeragi().commit_time_ms,
        max_clock_drift_ms: params.sumeragi().max_clock_drift_ms,
        collectors_k: params.sumeragi().collectors_k,
        redundant_send_r: params.sumeragi().collectors_redundant_send_r,
        block_max_transactions: params.block().max_transactions().get(),
        da_enabled: params.sumeragi().da_enabled,
        epoch_length_blocks,
        bls_domain: matched_meta.bls_domain.clone(),
        npos: npos_params,
    };
    let computed_fp = iroha_core::sumeragi::consensus::compute_consensus_fingerprint_from_params(
        &config.common.chain,
        &consensus_params,
        mode_tag,
    );
    let meta_fp_clean = matched_meta.consensus_fingerprint.trim_start_matches("0x");
    let meta_fp_bytes = hex::decode(meta_fp_clean).map_err(|err| {
        Report::new(MainError::Config).attach(format!(
            "failed to decode consensus_handshake_meta fingerprint `{}`: {err}",
            matched_meta.consensus_fingerprint
        ))
    })?;
    if meta_fp_bytes.len() != 32 {
        return Err(Report::new(MainError::Config).attach(format!(
            "consensus_handshake_meta fingerprint must be 32 bytes, got `{}`",
            matched_meta.consensus_fingerprint
        )));
    }
    if computed_fp != meta_fp_bytes.as_slice() {
        return Err(Report::new(MainError::Config).attach(format!(
            "consensus_handshake_meta fingerprint 0x{} does not match parameters encoded in genesis (computed 0x{})",
            matched_meta.consensus_fingerprint,
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
        if bls_pk.algorithm() != Algorithm::BlsNormal {
            return Err(Report::new(MainError::Config)
                .attach(format!("trusted peer {peer_id} must use a BLS-normal key")));
        }
        let expected_pop = trusted.pops.get(bls_pk).ok_or_else(|| {
            Report::new(MainError::Config).attach(format!(
                "trusted peer {peer_id} missing PoP in configuration"
            ))
        })?;
        if &entry.pop != expected_pop {
            return Err(Report::new(MainError::Config).attach(format!(
                "genesis PoP for peer {peer_id} does not match configuration"
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

async fn run_node(config: Config, genesis: Option<GenesisBlock>) -> ReportResult<(), MainError> {
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
        if panic_hook::is_suppressed() || norito::decode_panic_suppressed() {
            iroha_logger::warn!(
                "Panic occurred with shutdown suppression active; skipping shutdown signal"
            );
        } else {
            iroha_logger::error!("Panic occurred, shutting down Iroha gracefully...");
            signal_clone.send();
        }
        default_hook(info);
    }));

    let start = Iroha::start(config, genesis, logger, shutdown_on_panic);
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
    let gpu_available = norito::core::hw::has_gpu_compression();

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
        "\n{}\nNorito settings:\n  - min_compress_bytes_cpu: {}\n  - min_compress_bytes_gpu: {}\n  - zstd_level_small: {}\n  - zstd_level_large: {}\n  - zstd_level_gpu: {}\n  - large_threshold: {}\n  - gpu_offload_allowed: {}\n  - gpu_backend_available: {}\n",
        art,
        n.min_compress_bytes_cpu,
        n.min_compress_bytes_gpu,
        n.zstd_level_small,
        n.zstd_level_large,
        n.zstd_level_gpu,
        n.large_threshold,
        gpu_allowed,
        gpu_available,
    );

    iroha_logger::info!(target: "norito", "{}", msg);
}

#[cfg(test)]
mod tests {
    use super::build_line_tests::multilane_config_table;
    #[allow(unused_imports)]
    use super::*;
    use iroha_config_base::toml::TomlSource;

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

    mod fastpq_overrides {
        use super::*;
        use iroha_config::parameters::actual::{Fastpq, FastpqExecutionMode, FastpqPoseidonMode};

        #[test]
        fn maps_metal_overrides_from_config() {
            let cfg = Fastpq {
                execution_mode: FastpqExecutionMode::Auto,
                poseidon_mode: FastpqPoseidonMode::Auto,
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
    }

    mod relay_ingress {
        use super::*;
        use iroha_core::torii_proxy::{
            TORII_PROXY_REQUEST_VERSION_V1, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV1, ToriiProxyRequestV1,
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
            let request = iroha_core::NetworkMessage::ToriiProxyRequest(Box::new(
                ToriiProxyRequestV1 {
                    schema_version: TORII_PROXY_REQUEST_VERSION_V1,
                    request_id: Hash::new(b"torii-proxy-request"),
                    hop_count: 0,
                    request: ToriiProxyRequestKindV1::Read(ToriiReadProxyRequestV1 {
                        endpoint: ToriiReadEndpointV1::AccountsList,
                        expected_route: route,
                        path_args: Vec::new(),
                        query_string: None,
                        body: Vec::new(),
                        response_format: ToriiProxyResponseFormatV1::Json,
                    }),
                },
            ));
            let response = iroha_core::NetworkMessage::ToriiProxyResponse(Box::new(
                ToriiProxyResponseV1 {
                    schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
                    request_id: Hash::new(b"torii-proxy-response"),
                    response: ToriiProxyHttpResponseV1 {
                        status_code: 200,
                        headers: Vec::new(),
                        body: Vec::new(),
                    },
                },
            ));

            assert!(!NetworkRelayShared::should_apply_low_priority_ingress(&request));
            assert!(!NetworkRelayShared::should_apply_low_priority_ingress(&response));
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

                [streaming]
                identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
                identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
            };

            Config::from_toml_source(TomlSource::inline(table)).expect("base config")
        }

        #[test]
        fn resolves_to_rbc_store_max_when_larger() {
            let mut config = base_config();
            config.norito.max_archive_len = 32 * 1024 * 1024;
            config.sumeragi.rbc.store_max_bytes = 128 * 1024 * 1024;
            config.network.max_frame_bytes = 64 * 1024 * 1024;

            let resolved = resolve_norito_max_archive_len(&config);

            assert_eq!(resolved, 128 * 1024 * 1024);
        }

        #[test]
        fn preserves_requested_when_already_largest() {
            let mut config = base_config();
            config.norito.max_archive_len = 256 * 1024 * 1024;
            config.sumeragi.rbc.store_max_bytes = 128 * 1024 * 1024;
            config.network.max_frame_bytes = 64 * 1024 * 1024;

            let resolved = resolve_norito_max_archive_len(&config);

            assert_eq!(resolved, 256 * 1024 * 1024);
        }
    }

    mod consensus_ingress_limits {
        use super::*;
        use std::num::NonZeroU32;

        #[test]
        fn rbc_session_limit_scales_with_ttl_and_block_time() {
            let limit = ConsensusIngressLimiter::rbc_session_limit_from_ttl(
                64,
                Duration::from_secs(120),
                Duration::from_secs(1),
            );
            assert_eq!(limit, 242);
        }

        #[test]
        fn rbc_session_limit_respects_explicit_upper_bound() {
            let limit = ConsensusIngressLimiter::rbc_session_limit_from_ttl(
                512,
                Duration::from_secs(120),
                Duration::from_secs(1),
            );
            assert_eq!(limit, 512);
        }

        #[test]
        fn rbc_session_limit_disables_when_configured_zero() {
            let limit = ConsensusIngressLimiter::rbc_session_limit_from_ttl(
                0,
                Duration::from_secs(120),
                Duration::from_secs(1),
            );
            assert_eq!(limit, 0);
        }

        #[test]
        fn bulk_scale_factor_scales_for_faster_block_time() {
            let scale = ConsensusIngressLimiter::bulk_scale_factor(Duration::from_millis(50));
            assert_eq!(scale, 2);
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

    mod npos_validator_counts {
        use super::*;
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            account::AccountId,
            metadata::Metadata,
            nexus::{LaneId, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
        };
        use iroha_primitives::numeric::Numeric;

        fn record_with_status(
            status: PublicLaneValidatorStatus,
            algorithm: Algorithm,
        ) -> PublicLaneValidatorRecord {
            let keypair = KeyPair::random_with_algorithm(algorithm);
            let account_id = AccountId::new(keypair.public_key().clone());
            let stake = Numeric::from(10_u64);
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: account_id.clone(),
                stake_account: account_id,
                total_stake: stake.clone(),
                self_stake: stake,
                metadata: Metadata::default(),
                status,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            }
        }

        #[test]
        fn tracks_active_bls_validators() {
            let active_bls =
                record_with_status(PublicLaneValidatorStatus::Active, Algorithm::BlsNormal);
            let active_ed =
                record_with_status(PublicLaneValidatorStatus::Active, Algorithm::Ed25519);
            let pending = record_with_status(
                PublicLaneValidatorStatus::PendingActivation(0),
                Algorithm::BlsNormal,
            );

            let (active_bls_count, active_total, pending_count, total) =
                npos_validator_status_counts([&active_bls, &active_ed, &pending]);

            assert_eq!(active_bls_count, 1);
            assert_eq!(active_total, 2);
            assert_eq!(pending_count, 1);
            assert_eq!(total, 3);
        }
    }

    mod relay_fairness {
        use super::*;
        use tokio::sync::mpsc;
        use tokio::sync::mpsc::error::TryRecvError;

        #[test]
        fn try_recv_after_burst_skips_when_budget_remaining() {
            let (tx, mut rx) = mpsc::channel(1);
            tx.try_send(7).expect("send low message");
            let mut budget = 1;

            let msg = try_recv_after_burst(&mut rx, &mut budget, 4);

            assert!(msg.is_none());
            assert_eq!(budget, 1);
            assert!(matches!(rx.try_recv(), Ok(7)));
        }

        #[test]
        fn try_recv_after_burst_consumes_when_due() {
            let (tx, mut rx) = mpsc::channel(1);
            tx.try_send(9).expect("send low message");
            let mut budget = 0;

            let msg = try_recv_after_burst(&mut rx, &mut budget, 4);

            assert!(matches!(msg, Some(9)));
            assert_eq!(budget, 4);
            assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
        }

        #[test]
        fn try_recv_after_burst_resets_budget_when_empty() {
            let (_tx, mut rx) = mpsc::channel::<u8>(1);
            let mut budget = 0;

            let msg = try_recv_after_burst(&mut rx, &mut budget, 4);

            assert!(msg.is_none());
            assert_eq!(budget, 4);
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
                OsString::from("irohad"),
                OsString::from(" "),
                OsString::from("--trace-config"),
            ]);

            assert!(parsed.trace_config);
        }

        #[test]
        fn surrounding_whitespace_is_trimmed() {
            let parsed = parse_args_from(vec![
                OsString::from("irohad"),
                OsString::from("   --trace-config  "),
            ]);

            assert!(parsed.trace_config);
        }

        #[test]
        fn meaningful_arguments_are_preserved() {
            let parsed = parse_args_from(vec![
                OsString::from("irohad"),
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
        use super::*;
        use iroha_config::base::toml::TomlSource;
        use iroha_genesis::{GenesisBuilder, ManifestCrypto};

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
            let mode_tag = match handshake_meta.mode.as_str() {
                "Permissioned" => iroha_core::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
                "Npos" => iroha_core::sumeragi::consensus::NPOS_TAG.to_string(),
                other => panic!("unexpected mode {other}"),
            };
            let proto = handshake_meta
                .wire_proto_versions
                .first()
                .copied()
                .unwrap_or(iroha_core::sumeragi::consensus::PROTO_VERSION);
            let fp_bytes = hex::decode(
                handshake_meta
                    .consensus_fingerprint
                    .trim_start_matches("0x"),
            )?;
            assert_eq!(fp_bytes.len(), 32, "fingerprint must be 32 bytes");
            let mut consensus_fingerprint = [0u8; 32];
            consensus_fingerprint.copy_from_slice(&fp_bytes);
            let config_caps = build_consensus_config_caps(&config.sumeragi)
                .map_err(|err| eyre::eyre!(format!("{err:?}")))?;
            let consensus_caps = iroha_p2p::ConsensusHandshakeCaps {
                mode_tag: mode_tag.clone(),
                proto_version: proto,
                consensus_fingerprint,
                config: config_caps,
            };

            config.genesis.public_key = genesis_keys.public_key().clone();
            config.common.chain = chain;
            config.common.key_pair = genesis_keys.clone();
            config.crypto.allowed_signing = vec![Algorithm::Ed25519];

            let err = verify_genesis_metadata(
                &genesis_block,
                &config,
                &consensus_caps,
                &mode_tag,
                &handshake_meta.bls_domain,
                proto,
            )
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

            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
            let genesis_account_id = SAMPLE_GENESIS_ACCOUNT_ID.clone();
            let domain_id: DomainId = "wonderland".parse().expect("valid domain id");
            let bls_keypair = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let bls_account_id = AccountId::new(bls_keypair.public_key().clone());

            let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
                .with_instructions([
                    InstructionBox::from(Register::domain(Domain::new(domain_id.clone()))),
                    InstructionBox::from(Register::account(Account::new(
                        bls_account_id.to_account_id(domain_id),
                    ))),
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
                &chain_id,
                &genesis_account_id,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            assert!(
                result.is_ok(),
                "genesis validation should accept BLS controllers when crypto config allows it"
            );
        }

        #[test]
        fn consensus_caps_follow_activation_height() {
            use iroha_core::{kura::Kura, query::store::LiveQueryStore};

            let config = sample_config();

            // Before activation: next_mode staged but height below the cutover keeps permissioned caps.
            let world = World::new();
            {
                let mut block = world.block();
                let params = block.parameters.get_mut();
                params.sumeragi.next_mode =
                    Some(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
                params.sumeragi.mode_activation_height = Some(5);
                block.commit();
            }
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let world = state.world_view();
            let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
            let config_caps =
                build_consensus_config_caps(&config.sumeragi).expect("config caps should build");
            let (mode_tag_perm, bls_perm, caps_perm) =
                compute_consensus_handshake_caps(&world, height, &config, &config_caps, None);
            assert_eq!(
                mode_tag_perm,
                iroha_core::sumeragi::consensus::PERMISSIONED_TAG
            );
            assert_eq!(bls_perm, "bls-iroha2:permissioned-sumeragi:v1");
            let permissioned_fp = caps_perm.consensus_fingerprint;

            // After activation: height at or beyond the cutover switches to the staged NPoS mode.
            let world = World::new();
            {
                let mut block = world.block();
                let params = block.parameters.get_mut();
                params.sumeragi.next_mode =
                    Some(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
                params.sumeragi.mode_activation_height = Some(0);
                block.commit();
            }
            let state = State::new_for_testing(world, kura, LiveQueryStore::start_test());
            let world = state.world_view();
            let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
            let (mode_tag_npos, bls_npos, caps_npos) =
                compute_consensus_handshake_caps(&world, height, &config, &config_caps, None);
            assert_eq!(mode_tag_npos, iroha_core::sumeragi::consensus::NPOS_TAG);
            assert_eq!(bls_npos, "bls-iroha2:npos-sumeragi:v1");
            assert_ne!(
                permissioned_fp, caps_npos.consensus_fingerprint,
                "mode cutover should change consensus fingerprint"
            );
        }

        #[test]
        fn verify_genesis_metadata_rejects_consensus_mode_mismatch() -> eyre::Result<()> {
            use iroha_core::{kura::Kura, query::store::LiveQueryStore};
            use iroha_data_model::parameter::system::SumeragiConsensusMode;

            iroha_genesis::init_instruction_registry();
            let mut config = sample_config();
            config.sumeragi.consensus_mode =
                iroha_config::parameters::actual::ConsensusMode::Permissioned;
            let genesis_keys = config.common.key_pair.clone();
            let chain = config.common.chain.clone();
            let manifest = GenesisBuilder::new_without_executor(chain.clone(), PathBuf::from("."))
                .build_raw()
                .with_consensus_mode(SumeragiConsensusMode::Npos)
                .with_consensus_meta();
            let genesis_block = manifest.build_and_sign(&genesis_keys)?;

            let config_caps = build_consensus_config_caps(&config.sumeragi)
                .map_err(|err| eyre::eyre!(format!("{err:?}")))?;
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let world = state.world_view();
            let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
            let (mode_tag, bls_domain, consensus_caps) =
                compute_consensus_handshake_caps(&world, height, &config, &config_caps, None);

            let proto = iroha_core::sumeragi::consensus::PROTO_VERSION;
            let err = verify_genesis_metadata(
                &genesis_block,
                &config,
                &consensus_caps,
                &mode_tag,
                &bls_domain,
                proto,
            )
            .expect_err("consensus mode mismatch should be detected");
            assert!(
                format!("{err:?}").contains("consensus_mode"),
                "error should mention consensus_mode mismatch: {err:?}"
            );

            Ok(())
        }

        #[test]
        fn verify_genesis_metadata_rejects_fingerprint_mismatch() -> eyre::Result<()> {
            use iroha_core::{kura::Kura, query::store::LiveQueryStore};

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

            let config_caps = build_consensus_config_caps(&config.sumeragi)
                .map_err(|err| eyre::eyre!(format!("{err:?}")))?;
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let world = state.world_view();
            let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
            let (mode_tag, bls_domain, consensus_caps) =
                compute_consensus_handshake_caps(&world, height, &config, &config_caps, None);

            // Diverge the runtime chain after computing consensus caps to force a
            // fingerprint mismatch without altering the embedded handshake metadata.
            config.common.chain = ChainId::from("fingerprint-mismatch");
            let proto = iroha_core::sumeragi::consensus::PROTO_VERSION;
            let err = verify_genesis_metadata(
                &genesis_block,
                &config,
                &consensus_caps,
                &mode_tag,
                &bls_domain,
                proto,
            )
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
                terminal_colors: false,
                trace_config: false,
                language: None,
                sora: false,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
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

        fn load_config_with_overrides<F>(mut adjust: F) -> eyre::Result<(Config, tempfile::TempDir)>
        where
            F: FnMut(&mut toml::Table, &KeyPair),
        {
            let genesis_key_pair = KeyPair::random();
            let manifest_json = norito::json!({
                "chain": "chain",
                "ivm_dir": ".",
                "consensus_mode": "Permissioned",
                "transactions": [
                    {
                        "parameters": {
                            "sumeragi": {
                                "block_time_ms": 1000,
                                "commit_time_ms": 1000
                            }
                        },
                        "instructions": [],
                        "ivm_triggers": [],
                        "topology": []
                    }
                ]
            });
            let raw: iroha_genesis::RawGenesisTransaction =
                norito::json::value::from_value(manifest_json).expect("manifest json");
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

            let (config, _genesis) = read_config_and_genesis(&Args {
                config: Some(config_path),
                genesis_manifest_json: None,
                terminal_colors: false,
                trace_config: false,
                language: None,
                sora: false,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
                fastpq_device_class: None,
                fastpq_chip_family: None,
                fastpq_gpu_kind: None,
            })
            .map_err(|report| eyre::eyre!("{report:?}"))?;

            Ok((config, dir))
        }

        #[test]
        fn relative_file_paths_resolution() -> eyre::Result<()> {
            // Given

            let genesis_key_pair = KeyPair::random();
            let manifest_json = norito::json!({
                "chain": "chain",
                "ivm_dir": ".",
                "consensus_mode": "Permissioned",
                "transactions": [
                    {
                        "parameters": {
                            "sumeragi": {
                                "block_time_ms": 1000,
                                "commit_time_ms": 1000
                            }
                        },
                        "instructions": [],
                        "ivm_triggers": [],
                        "topology": []
                    }
                ]
            });
            let raw: iroha_genesis::RawGenesisTransaction =
                norito::json::value::from_value(manifest_json).expect("manifest json");
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
                terminal_colors: false,
                trace_config: false,
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

        #[test]
        fn fails_with_no_trusted_peers_and_submit_role() -> eyre::Result<()> {
            // Given

            let genesis_key_pair = KeyPair::random();
            let mut config = config_factory(genesis_key_pair.public_key());
            iroha_config::base::toml::Writer::new(&mut config);

            let dir = tempfile::tempdir()?;
            std::fs::write(dir.path().join("config.toml"), toml::to_string(&config)?)?;
            std::fs::write(dir.path().join("executor.to"), "")?;
            let config_path = dir.path().join("config.toml");

            // When

            let (config, _genesis) = read_config_and_genesis(&Args {
                config: Some(config_path),
                genesis_manifest_json: None,
                terminal_colors: false,
                trace_config: false,
                language: None,
                sora: false,
                fastpq_execution_mode: None,
                fastpq_poseidon_mode: None,
                fastpq_device_class: None,
                fastpq_chip_family: None,
                fastpq_gpu_kind: None,
            })
            .map_err(|report| eyre::eyre!("{report:?}"))?;

            // Then

            let report = validate_config(&config).unwrap_err();

            assert_contains!(
                format!("{report:#}"),
                "The network consists from this one peer only"
            );

            Ok(())
        }

        #[test]
        fn validate_config_io_flags_lone_peer_and_address_conflict() -> eyre::Result<()> {
            let (config, _dir) = load_config_with_overrides(|table, _genesis_key| {
                if let Some(genesis_table) =
                    table.get_mut("genesis").and_then(toml::Value::as_table_mut)
                {
                    genesis_table.remove("file");
                }
                iroha_config::base::toml::Writer::new(table).write(
                    ["torii", "address"],
                    socket_addr!(127.0.0.1:1337).to_literal(),
                );
            })?;

            let mut emitter = Emitter::new();
            validate_config_io(&mut emitter, &config);
            let report = emitter
                .into_result()
                .expect_err("expected validation errors");
            let report_text = format!("{report:#}");
            assert_contains!(report_text, "The network consists from this one peer only");
            assert_contains!(
                report_text,
                "Torii and Network addresses are the same, but should be different"
            );

            Ok(())
        }

        #[test]
        fn stack_budget_mismatch_warns_but_allows_config() -> eyre::Result<()> {
            let (config, _dir) = load_config_with_overrides(|table, _genesis_key| {
                let mut cpu_balanced = toml::Table::new();
                cpu_balanced.insert("max_cycles".to_owned(), toml::Value::Integer(10_000_000));
                cpu_balanced.insert(
                    "max_memory_bytes".to_owned(),
                    toml::Value::Integer(256 * 1024 * 1024),
                );
                cpu_balanced.insert(
                    "max_stack_bytes".to_owned(),
                    toml::Value::Integer(8 * 1024 * 1024),
                );
                cpu_balanced.insert(
                    "max_io_bytes".to_owned(),
                    toml::Value::Integer(24 * 1024 * 1024),
                );
                cpu_balanced.insert(
                    "max_egress_bytes".to_owned(),
                    toml::Value::Integer(12 * 1024 * 1024),
                );
                cpu_balanced.insert("allow_gpu_hints".to_owned(), toml::Value::Boolean(true));
                cpu_balanced.insert("allow_wasi".to_owned(), toml::Value::Boolean(true));

                let mut profiles = toml::Table::new();
                profiles.insert("cpu-balanced".to_owned(), toml::Value::Table(cpu_balanced));

                iroha_config::base::toml::Writer::new(table)
                    .write(["compute", "enabled"], true)
                    .write(
                        ["compute", "resource_profiles"],
                        toml::Value::Table(profiles),
                    )
                    .write(["compute", "default_resource_profile"], "cpu-balanced")
                    .write(["ivm", "memory_budget_profile"], "cpu-balanced")
                    .write(["concurrency", "guest_stack_bytes"], 4_i64 * 1024 * 1024);
            })?;

            validate_config(&config).map_err(|report| eyre::eyre!("{report:?}"))?;

            Ok(())
        }

        #[test]
        fn validator_requires_confidential_enabled() -> eyre::Result<()> {
            let (config, _dir) = load_config_with_overrides(|table, _genesis_key| {
                iroha_config::base::toml::Writer::new(table)
                    .write(["sumeragi", "role"], "validator")
                    .write(["confidential", "enabled"], false)
                    .write(["confidential", "assume_valid"], false);
            })?;

            let report = validate_config(&config).unwrap_err();
            assert_contains!(
                format!("{report:#}"),
                "validator nodes must enable confidential verification"
            );

            Ok(())
        }

        #[test]
        fn validate_config_runtime_rejects_validator_confidential_disabled() -> eyre::Result<()> {
            let (config, _dir) = load_config_with_overrides(|table, _genesis_key| {
                iroha_config::base::toml::Writer::new(table)
                    .write(["sumeragi", "role"], "validator")
                    .write(["confidential", "enabled"], false)
                    .write(["confidential", "assume_valid"], false);
            })?;

            let mut emitter = Emitter::new();
            validate_config_runtime(&mut emitter, &config);
            let report = emitter
                .into_result()
                .expect_err("expected validation errors");
            assert_contains!(
                format!("{report:#}"),
                "validator nodes must enable confidential verification"
            );

            Ok(())
        }

        #[test]
        fn validator_cannot_assume_valid_confidential() -> eyre::Result<()> {
            let (config, _dir) = load_config_with_overrides(|table, _genesis_key| {
                iroha_config::base::toml::Writer::new(table)
                    .write(["sumeragi", "role"], "validator")
                    .write(["confidential", "enabled"], true)
                    .write(["confidential", "assume_valid"], true);
            })?;

            let report = validate_config(&config).unwrap_err();
            assert_contains!(
                format!("{report:#}"),
                "validator nodes cannot enable confidential observer mode"
            );

            Ok(())
        }
    }

    #[test]
    #[allow(clippy::bool_assert_comparison)] // for expressiveness
    fn default_args() {
        let args = Args::try_parse_from(["test"]).unwrap();

        assert_eq!(args.terminal_colors, is_coloring_supported());
    }

    #[test]
    #[allow(clippy::bool_assert_comparison)] // for expressiveness
    fn terminal_colors_works_as_expected() -> eyre::Result<()> {
        fn try_with(arg: &str) -> eyre::Result<bool> {
            Ok(Args::try_parse_from(["test", arg])?.terminal_colors)
        }

        assert_eq!(
            Args::try_parse_from(["test"])?.terminal_colors,
            is_coloring_supported()
        );
        assert_eq!(try_with("--terminal-colors")?, true);
        assert_eq!(try_with("--terminal-colors=false")?, false);
        assert_eq!(try_with("--terminal-colors=true")?, true);
        assert!(try_with("--terminal-colors=random").is_err());

        Ok(())
    }

    #[test]
    fn user_provided_config_path_works() {
        let args = Args::try_parse_from(["test", "--config", "/home/custom/file.json"]).unwrap();

        assert_eq!(args.config, Some(PathBuf::from("/home/custom/file.json")));
    }

    #[test]
    fn user_can_provide_any_extension() {
        let _args = Args::try_parse_from(["test", "--config", "file.toml.but.not"])
            .expect("should allow doing this as well");
    }

    #[test]
    fn config_router_disabled_for_single_lane_defaults() {
        let nexus = iroha_config::parameters::actual::Nexus::default();
        assert!(!should_use_config_router(&nexus));
    }

    #[test]
    fn config_router_enabled_when_lane_catalog_expands() {
        use iroha_data_model::nexus::{LaneCatalog, LaneConfig};
        use std::num::NonZeroU32;

        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "lane-1".to_owned(),
                    description: None,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog),
            lane_catalog,
            ..Default::default()
        };

        assert!(should_use_config_router(&nexus));
    }

    #[test]
    fn multilane_config_requires_nexus_enabled_flag() {
        let err = Config::from_toml_source(TomlSource::inline(multilane_config_table(false)))
            .expect_err("multi-lane catalog must require nexus.enabled");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("nexus.enabled"),
            "error should mention nexus.enabled, got: {rendered}"
        );
    }

    #[test]
    fn multilane_config_parses_when_enabled_flag_set() {
        let config = Config::from_toml_source(TomlSource::inline(multilane_config_table(true)))
            .expect("multi-lane config with nexus enabled should parse");
        assert!(config.nexus.enabled);
        assert_eq!(config.nexus.lane_catalog.lane_count().get(), 2);
        assert_eq!(config.nexus.lane_config.entries().len(), 2);
    }

    #[test]
    fn read_genesis_handles_decode_failure() {
        // Create a bogus genesis file and ensure we return an error instead of panicking.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.genesis.signed.nrt");
        std::fs::write(&path, [0u8, 1u8, 2u8, 3u8]).unwrap();

        let res = read_genesis(&path);
        assert!(res.is_err());
    }

    #[test]
    fn read_genesis_initializes_instruction_registry() {
        use iroha_data_model::isi::{InstructionRegistry, set_instruction_registry};

        // Start with an empty registry to simulate uninitialized state.
        set_instruction_registry(InstructionRegistry::new());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.genesis.signed.nrt");
        std::fs::write(&path, [0u8, 1u8, 2u8, 3u8]).unwrap();

        // `read_genesis` should initialize the registry internally and simply
        // return a decode error for the bogus file instead of panicking.
        let res = read_genesis(&path);
        assert!(res.is_err());
    }

    #[cfg(feature = "beep")]
    #[test]
    fn startup_beep_respects_config_flag() {
        assert!(
            !startup_beep(false),
            "beep disabled by config flag should no-op"
        );
        assert!(
            startup_beep(true),
            "beep enabled by config flag should play once"
        );
    }

    mod soranet_transport {
        use iroha_config::parameters::actual;
        use tempfile::tempdir;

        #[test]
        fn configure_soranet_transport_creates_spool_directory() {
            let temp = tempdir().expect("create temp dir");
            let spool_dir = temp.path().join("spool");

            let mut soranet = actual::StreamingSoranet::from_defaults();
            soranet.enabled = true;
            soranet.provision_spool_dir = spool_dir.clone();

            let mut handle = iroha_core::streaming::StreamingHandle::new();
            super::super::configure_soranet_transport(&mut handle, &soranet)
                .expect("soranet transport configuration should succeed");

            assert!(
                spool_dir.is_dir(),
                "expected configure_soranet_transport to create the spool directory"
            );
        }

        #[test]
        fn configure_soranet_transport_noop_when_disabled() {
            let temp = tempdir().expect("create temp dir");
            let spool_dir = temp.path().join("disabled");

            let mut soranet = actual::StreamingSoranet::from_defaults();
            soranet.enabled = false;
            soranet.provision_spool_dir = spool_dir.clone();

            let mut handle = iroha_core::streaming::StreamingHandle::new();
            super::super::configure_soranet_transport(&mut handle, &soranet)
                .expect("disabled soranet transport should not fail");

            assert!(
                !spool_dir.exists(),
                "disabled configuration must not create the spool directory"
            );
        }
    }
}

type ReportResult<T, E> = core::result::Result<T, Report<E>>;
const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(value) => value,
    None => "unknown",
};

const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {
    Some(value) => value,
    None => "unknown",
};

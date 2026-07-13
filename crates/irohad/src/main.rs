/// Feature-isolated real-network consensus fault-injection control.
#[cfg(feature = "test-network-message-control")]
mod consensus_message_control;
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
/// Asynchronous Nexus DPN fee settlement relay.
mod nexus_fee_relay_worker;
/// Embedded Soracloud runtime-manager reconciliation.
#[cfg(feature = "embedded-soracloud-runtime")]
#[path = "soracloud_runtime.rs"]
mod soracloud_runtime;
/// No-op Soracloud runtime used when the full embedded runtime is disabled.
#[cfg(not(feature = "embedded-soracloud-runtime"))]
#[path = "soracloud_runtime_stub.rs"]
mod soracloud_runtime;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
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
        actual::{
            FastpqExecutionMode, FastpqPoseidonMode, NexusStorageAutoDefault,
            NexusStorageAutoDefaultFilesystemGroup, NexusStorageBudgetComponent,
            NexusStorageBudgetSource, Root as Config,
        },
        user::Root as UserConfig,
    },
};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::{StateTelemetry, StreamingTelemetry};
use iroha_core::{
    IrohaNetwork,
    block::ValidBlock,
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
    snapshot::{
        SnapshotMaker, TryReadError as TryReadSnapshotError,
        try_read_snapshot_with_bootstrap_policy,
    },
    state::{State, World},
    streaming::{FilesystemSoranetProvisioner, ManifestPublisher, run_ticket_event_listener},
    sumeragi::{
        GenesisWithPubKey, SumeragiHandle, SumeragiStartArgs, VotingBlock,
        filter_validators_from_trusted, network_topology::Topology,
    },
};
use iroha_crypto::Algorithm;
use iroha_data_model::query::{self as dm_query, ErasedIterQuery};
use iroha_data_model::{block::decode_framed_signed_block, prelude::*, transaction::Executable};
use iroha_data_model::{
    isi::RegisterPeerWithPop,
    parameter::system::{
        ConsensusHandshakeMetadata, confidential_metadata, consensus_metadata, crypto_metadata,
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

const NODE_RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
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

fn torii_receipt_signer_or_ephemeral(
    receipt_signer: Option<KeyPair>,
) -> Result<KeyPair, iroha_crypto::Error> {
    if let Some(receipt_signer) = receipt_signer {
        return Ok(receipt_signer);
    }

    let key = iroha_crypto::KeyPair::try_random_with_algorithm(Algorithm::Secp256k1)?;
    let algorithm = key
        .public_key()
        .try_algorithm()
        .map_or("malformed", |algorithm| algorithm.as_static_str());
    iroha_logger::info!(
        algorithm,
        "torii receipt signer not configured; generated ephemeral secp256k1 key"
    );
    Ok(key)
}

type ConsensusHandshakeMeta = ConsensusHandshakeMetadata;

fn parse_handshake_meta_str(raw: &str) -> Result<ConsensusHandshakeMeta, norito::Error> {
    let metadata: ConsensusHandshakeMeta =
        norito::json::from_str(raw).map_err(norito::Error::from)?;
    metadata
        .validate()
        .map_err(|error| norito::Error::Message(error.to_owned()))?;
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

fn build_shared_sorafs_provider_cache(
    config: &Config,
) -> Option<Arc<tokio::sync::RwLock<iroha_torii::sorafs::ProviderAdvertCache>>> {
    let discovery = &config.torii.sorafs_discovery;
    if !discovery.discovery_enabled {
        return None;
    }

    let Some(admission_cfg) = discovery.admission.as_ref() else {
        panic!(
            "SoraFS discovery requires sorafs.discovery.admission.envelopes_dir, trusted_council_keys, and signature_threshold"
        );
    };

    let trusted_council_keys = admission_cfg
        .trusted_council_keys
        .iter()
        .map(|key| {
            let (algorithm, payload) = key
                .try_to_bytes()
                .unwrap_or_else(|err| panic!("invalid SoraFS admission council key: {err}"));
            assert_eq!(
                algorithm,
                iroha_crypto::Algorithm::Ed25519,
                "SoraFS admission council keys must use Ed25519"
            );
            <[u8; 32]>::try_from(payload)
                .unwrap_or_else(|_| panic!("SoraFS admission council keys must be 32 bytes"))
        })
        .collect::<Vec<_>>();
    let policy = sorafs_manifest::ProviderAdmissionCouncilPolicy::new(
        trusted_council_keys,
        admission_cfg.signature_threshold.get(),
    )
    .unwrap_or_else(|err| panic!("invalid SoraFS admission council policy: {err}"));
    let admission = Arc::new(
        iroha_torii::sorafs::AdmissionRegistry::load_from_dir(&admission_cfg.envelopes_dir, policy)
            .unwrap_or_else(|err| {
                panic!(
                    "failed to load shared SoraFS provider admission registry {}: {err}",
                    admission_cfg.envelopes_dir.display()
                )
            }),
    );

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
    tx_gossiper: TransactionGossiperHandle,
    peers_gossiper: PeersGossiperHandle,
    network: IrohaNetwork,
    streaming: iroha_core::streaming::StreamingHandle,
    kiso: KisoHandle,
    suppress_pow_broadcast: Arc<AtomicBool>,
    pow_update_version: Arc<AtomicU64>,
    consensus_ingress: Mutex<ConsensusIngressLimiter>,
    low_priority_ingress: Mutex<LowPriorityIngressLimiter>,
    #[cfg(feature = "test-network-message-control")]
    test_message_control: Option<Arc<consensus_message_control::Controller>>,
}

type RelayWorkItem = iroha_p2p::peer::message::PeerMessage<iroha_core::NetworkMessage>;

/// Maximum number of consecutive high-priority messages before yielding to low-priority work.
const RELAY_HIGH_BURST: usize = 32;

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
                BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_) => IngressPolicy::critical(),
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
                        | ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
                            IngressPolicy::critical()
                        }
                    }
                }
                // All other block messages are decode-only v1 artifacts. The relay
                // rejects them before ingress accounting; keeping this fallback
                // separate prevents archival types from shaping live v2 limits.
                _ => IngressPolicy::limited(),
            },
            iroha_core::NetworkMessage::SumeragiControlFlow(_) => IngressPolicy::critical(),
            iroha_core::NetworkMessage::CertifiedMergeSidecar(message) => match message.as_ref() {
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Request(_) => {
                    IngressPolicy::limited()
                }
                iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Chunk(_) => {
                    IngressPolicy::bulk()
                }
            },
            iroha_core::NetworkMessage::NativeAmx(_) => IngressPolicy::critical(),
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

#[derive(Debug, PartialEq, Eq)]
enum RelayBurstRecv<T> {
    Skip,
    Message(T),
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelayIngressLoopExit {
    ReceiverClosed(RelayReceiverKind),
    WorkerClosed(RelayReceiverKind),
}

fn try_recv_after_burst<T>(
    receiver: &mut mpsc::Receiver<T>,
    high_budget: &mut usize,
    high_burst: usize,
) -> RelayBurstRecv<T> {
    if *high_budget != 0 {
        return RelayBurstRecv::Skip;
    }
    *high_budget = high_burst;
    match receiver.try_recv() {
        Ok(msg) => RelayBurstRecv::Message(msg),
        Err(mpsc::error::TryRecvError::Empty) => RelayBurstRecv::Skip,
        Err(mpsc::error::TryRecvError::Disconnected) => RelayBurstRecv::Disconnected,
    }
}

fn spawn_network_relay_worker(
    shared: Arc<NetworkRelayShared>,
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
    let (work_high_tx, mut work_high_rx) = mpsc::channel::<RelayWorkItem>(work_high_cap);
    let (work_payload_tx, mut work_payload_rx) = mpsc::channel::<RelayWorkItem>(work_payload_cap);
    let (work_chunk_tx, mut work_chunk_rx) = mpsc::channel::<RelayWorkItem>(work_chunk_cap);
    let (work_low_tx, mut work_low_rx) = mpsc::channel::<RelayWorkItem>(work_low_cap);
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

    (work_high_tx, work_payload_tx, work_chunk_tx, work_low_tx)
}

fn try_enqueue_relay_work(
    tx: &mpsc::Sender<RelayWorkItem>,
    msg: RelayWorkItem,
    kind: RelayReceiverKind,
    drops: &mut u64,
) -> Result<(), RelayIngressLoopExit> {
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
        return Ok(());
    }
    match tx.try_send(msg) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(msg)) => {
            *drops = drops.saturating_add(1);
            if *drops == 1 || (*drops).is_multiple_of(1024) {
                iroha_logger::warn!(
                    peer = %msg.peer,
                    topic = ?msg.payload.topic(),
                    drops = *drops,
                    queue = kind.label(),
                    "relay work queue full; dropping message"
                );
            }
            Ok(())
        }
        Err(mpsc::error::TrySendError::Closed(_)) => Err(RelayIngressLoopExit::WorkerClosed(kind)),
    }
}

#[allow(clippy::too_many_arguments)]
async fn drive_network_relay_ingress(
    mut high_receiver: mpsc::Receiver<RelayWorkItem>,
    mut payload_receiver: mpsc::Receiver<RelayWorkItem>,
    mut chunk_receiver: mpsc::Receiver<RelayWorkItem>,
    mut low_receiver: mpsc::Receiver<RelayWorkItem>,
    work_high_tx: &mpsc::Sender<RelayWorkItem>,
    work_payload_tx: &mpsc::Sender<RelayWorkItem>,
    work_chunk_tx: &mpsc::Sender<RelayWorkItem>,
    work_low_tx: &mpsc::Sender<RelayWorkItem>,
) -> RelayIngressLoopExit {
    let mut high_budget = RELAY_HIGH_BURST;
    let mut high_drops: u64 = 0;
    let mut payload_drops: u64 = 0;
    let mut chunk_drops: u64 = 0;
    let mut low_drops: u64 = 0;

    loop {
        match try_recv_after_burst(&mut payload_receiver, &mut high_budget, RELAY_HIGH_BURST) {
            RelayBurstRecv::Message(msg) => {
                if let Err(exit) = try_enqueue_relay_work(
                    work_payload_tx,
                    msg,
                    RelayReceiverKind::Payload,
                    &mut payload_drops,
                ) {
                    return exit;
                }
                continue;
            }
            RelayBurstRecv::Disconnected => {
                return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Payload);
            }
            RelayBurstRecv::Skip => {}
        }
        match try_recv_after_burst(&mut chunk_receiver, &mut high_budget, RELAY_HIGH_BURST) {
            RelayBurstRecv::Message(msg) => {
                if let Err(exit) = try_enqueue_relay_work(
                    work_chunk_tx,
                    msg,
                    RelayReceiverKind::Chunk,
                    &mut chunk_drops,
                ) {
                    return exit;
                }
                continue;
            }
            RelayBurstRecv::Disconnected => {
                return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Chunk);
            }
            RelayBurstRecv::Skip => {}
        }
        match try_recv_after_burst(&mut low_receiver, &mut high_budget, RELAY_HIGH_BURST) {
            RelayBurstRecv::Message(msg) => {
                if let Err(exit) =
                    try_enqueue_relay_work(work_low_tx, msg, RelayReceiverKind::Low, &mut low_drops)
                {
                    return exit;
                }
                continue;
            }
            RelayBurstRecv::Disconnected => {
                return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Low);
            }
            RelayBurstRecv::Skip => {}
        }
        tokio::select! {
            biased;
            msg = high_receiver.recv() => {
                let Some(msg) = msg else {
                    return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::High);
                };
                high_budget = high_budget.saturating_sub(1);
                if let Err(exit) = try_enqueue_relay_work(
                    work_high_tx,
                    msg,
                    RelayReceiverKind::High,
                    &mut high_drops,
                ) {
                    return exit;
                }
            }
            msg = payload_receiver.recv() => {
                let Some(msg) = msg else {
                    return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Payload);
                };
                high_budget = RELAY_HIGH_BURST;
                if let Err(exit) = try_enqueue_relay_work(
                    work_payload_tx,
                    msg,
                    RelayReceiverKind::Payload,
                    &mut payload_drops,
                ) {
                    return exit;
                }
            }
            msg = chunk_receiver.recv() => {
                let Some(msg) = msg else {
                    return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Chunk);
                };
                high_budget = RELAY_HIGH_BURST;
                if let Err(exit) = try_enqueue_relay_work(
                    work_chunk_tx,
                    msg,
                    RelayReceiverKind::Chunk,
                    &mut chunk_drops,
                ) {
                    return exit;
                }
            }
            msg = low_receiver.recv() => {
                let Some(msg) = msg else {
                    return RelayIngressLoopExit::ReceiverClosed(RelayReceiverKind::Low);
                };
                high_budget = RELAY_HIGH_BURST;
                if let Err(exit) = try_enqueue_relay_work(
                    work_low_tx,
                    msg,
                    RelayReceiverKind::Low,
                    &mut low_drops,
                ) {
                    return exit;
                }
            }
        }
    }
}

impl NetworkRelay {
    fn into_shared(self) -> NetworkRelayShared {
        #[cfg(feature = "test-network-message-control")]
        let test_message_control = match consensus_message_control::Controller::from_env() {
            Ok(controller) => controller.map(Arc::new),
            Err(error) => {
                iroha_logger::error!(
                    reason = error.code(),
                    "test-network consensus message controller failed closed during startup"
                );
                std::process::exit(1);
            }
        };
        NetworkRelayShared {
            sumeragi: self.sumeragi,
            tx_gossiper: self.tx_gossiper,
            peers_gossiper: self.peers_gossiper,
            network: self.network,
            streaming: self.streaming,
            kiso: self.kiso,
            suppress_pow_broadcast: self.suppress_pow_broadcast,
            pow_update_version: self.pow_update_version,
            consensus_ingress: Mutex::new(self.consensus_ingress),
            low_priority_ingress: Mutex::new(self.low_priority_ingress),
            #[cfg(feature = "test-network-message-control")]
            test_message_control,
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn run(self) {
        use iroha_p2p::network::{SubscriberFilter, message::Topic};

        let shared = Arc::new(self.into_shared());
        #[cfg(feature = "test-network-message-control")]
        if let Some(controller) = shared.test_message_control.clone() {
            let shared = Arc::downgrade(&shared);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    let Some(shared) = shared.upgrade() else {
                        return;
                    };
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
                        let delivered = shared
                            .handle_message_after_test_control(
                                held.peer,
                                held.message,
                                held.size_bytes,
                            )
                            .await;
                        if let Err(error) = controller.complete_release(held.sequence, delivered) {
                            iroha_logger::error!(
                                sequence = held.sequence,
                                reason = error.code(),
                                "test-network consensus release delivery failed closed"
                            );
                            std::process::exit(1);
                        }
                    }
                }
            });
        }
        let base_cap = shared.network.subscriber_queue_cap().get();
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

        loop {
            let (high_sender, high_receiver) = mpsc::channel(high_cap);
            let (payload_sender, payload_receiver) = mpsc::channel(payload_cap);
            let (chunk_sender, chunk_receiver) = mpsc::channel(chunk_cap);
            let (low_sender, low_receiver) = mpsc::channel(low_cap);
            let (work_high_tx, work_payload_tx, work_chunk_tx, work_low_tx) =
                spawn_network_relay_worker(
                    Arc::clone(&shared),
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
                let exit = drive_network_relay_ingress(
                    high_receiver,
                    payload_receiver,
                    chunk_receiver,
                    low_receiver,
                    &work_high_tx,
                    &work_payload_tx,
                    &work_chunk_tx,
                    &work_low_tx,
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
    #[allow(clippy::too_many_lines)]
    async fn handle_message(&self, peer: Peer, msg: iroha_core::NetworkMessage, size_bytes: usize) {
        #[cfg(feature = "test-network-message-control")]
        let (peer, msg, size_bytes) = if let Some(controller) = &self.test_message_control {
            match controller.admit(peer, msg, size_bytes) {
                Ok((consensus_message_control::Admission::Consumed, _)) => return,
                Ok((consensus_message_control::Admission::Pass, Some(message))) => message,
                Ok((consensus_message_control::Admission::Pass, None)) => {
                    iroha_logger::error!(
                        "test-network consensus controller returned an invalid pass disposition"
                    );
                    std::process::exit(1);
                }
                Err(error) => {
                    iroha_logger::error!(
                        reason = error.code(),
                        "test-network consensus controller failed closed during admission"
                    );
                    std::process::exit(1);
                }
            }
        } else {
            (peer, msg, size_bytes)
        };

        let _ = self
            .handle_message_after_test_control(peer, msg, size_bytes)
            .await;
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_message_after_test_control(
        &self,
        peer: Peer,
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

        if matches!(
            &msg,
            SumeragiBlock(_) | SumeragiControlFlow(_) | CertifiedMergeSidecar(_)
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
                    CertifiedMergeSidecar(data) => match data.as_ref() {
                        iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Request(_) => {
                            ("CertifiedMergeSidecarRequest", None, None)
                        }
                        iroha_core::merge_sidecar::CertifiedMergeSidecarMessage::Chunk(_) => {
                            ("CertifiedMergeSidecarChunk", None, None)
                        }
                    },
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
                return false;
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
                return false;
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
                if msg.requires_blocking_ingress() {
                    let handle = tokio::task::spawn_blocking(move || {
                        sumeragi.incoming_block_message_from(sender, msg);
                    });
                    if let Err(err) = handle.await {
                        iroha_logger::warn!(?err, "blocking sumeragi ingress task aborted");
                        return false;
                    }
                } else {
                    return sumeragi.try_incoming_block_message_from(sender, msg);
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
            CertifiedMergeSidecar(message) => {
                let _ = self
                    .sumeragi
                    .try_incoming_certified_merge_sidecar(peer.id().clone(), *message);
            }
            NativeAmx(message) => {
                let _ = self
                    .sumeragi
                    .try_incoming_native_amx(peer.id().clone(), *message);
            }
            StreamingControl(frame) => {
                if let Err(err) = self.streaming.process_control_frame(&peer, frame.as_ref()) {
                    iroha_logger::warn!(%peer, ?err, "Failed to process streaming control frame");
                }
            }
            BlockSync(_) => unreachable!("retired v1 block sync is rejected before dispatch"),
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
            msg @ (SoracloudLocalReadProxyRequest(_)
            | SoracloudLocalReadProxyResponse(_)
            | ToriiProxyRequest(_)
            | ToriiProxyResponse(_)
            | GenesisRequest(_)
            | GenesisResponse(_)
            | Health
            | Connect(_)) => {
                debug_assert!(Self::is_handled_by_dedicated_subscriber(&msg));
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
        true
    }

    fn is_handled_by_dedicated_subscriber(msg: &iroha_core::NetworkMessage) -> bool {
        msg.is_torii_proxy_control_message()
            || matches!(
                msg,
                iroha_core::NetworkMessage::GenesisRequest(_)
                    | iroha_core::NetworkMessage::GenesisResponse(_)
                    | iroha_core::NetworkMessage::Health
                    | iroha_core::NetworkMessage::Connect(_)
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
        use iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload;

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
            LaneBlockProposal(proposal) => (
                "LaneBlockProposal",
                Some(proposal.descriptor.lane_block_height),
                Some(proposal.descriptor.lane_block_view),
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
            KuraReplicaAdvert(advert) => ("KuraReplicaAdvert", Some(advert.height), None),
            V2(message) => match &message.payload {
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
                ConsensusMessageV2Payload::PayloadChunk(_) => {
                    ("SumeragiV2PayloadChunk", None, None)
                }
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
            },
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

#[cfg(test)]
mod network_relay_tests {
    use std::time::Duration;

    use iroha_config::{
        client_api::{SoranetHandshakePowSummary, SoranetHandshakePuzzleSummary},
        parameters::actual::{SoranetPow, SoranetPuzzle},
    };
    use iroha_core::{
        SoranetPowConfigBroadcast, SoranetPuzzleConfigBroadcast,
        sumeragi::{
            consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1, LaneBlockQcV1, Phase},
            message::{BlockMessage, BlockMessageWire},
        },
        torii_proxy::{
            TORII_PROXY_REQUEST_VERSION_V2, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV1, ToriiProxyRequestV2,
            ToriiProxyResponseFormatV1, ToriiProxyResponseV1, ToriiReadEndpointV1,
            ToriiReadProxyRequestV1, ToriiRouteHintV1,
        },
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus_v2::{
                self, CommitCertificateRequest, ConsensusMessageV2, ConsensusMessageV2Payload,
                HeightContextId, PROTOCOL_VERSION,
            },
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        nexus::{DataSpaceId, LaneId},
        peer::{Peer, PeerId},
    };

    use super::{
        BucketConfig, ConsensusIngressDropReason, ConsensusIngressLimiter, IngressRateClass,
        LowPriorityIngressDropReason, LowPriorityIngressLimiter, NetworkRelayShared, PenaltyConfig,
        enqueue_sumeragi_block_message, pow_update_payload,
    };

    #[test]
    fn relay_enqueue_drops_when_queue_is_full() {
        let (tx, rx) = std::sync::mpsc::sync_channel(0);
        let msg = v2_vote_block_message();

        enqueue_sumeragi_block_message(msg, move |msg| {
            let _ = tx.try_send(msg);
        });

        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty | std::sync::mpsc::TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn block_message_blocking_ingress_policy_admits_only_authoritative_v2() {
        assert!(
            BlockMessage::LaneBlockProposal(sample_lane_block_proposal())
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

        assert!(
            NetworkRelayShared::retired_sumeragi_message_meta(&retired_vrf_commit_msg()).is_some()
        );
        assert!(NetworkRelayShared::retired_sumeragi_message_meta(&v2_vote_msg()).is_none());
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

    fn sample_v2_round(height: u64, view: u64) -> consensus_v2::ConsensusRound {
        consensus_v2::ConsensusRound {
            context_id: consensus_v2::HeightContextId(HashOf::from_untyped_unchecked(
                Hash::prehashed([0x61; 32]),
            )),
            height,
            view,
        }
    }

    fn sample_v2_subject() -> consensus_v2::BlockSubject {
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
                phase: consensus_v2::GlobalPhase::Prepare,
                subject: sample_v2_subject(),
                execution_commitment: consensus_v2::ExecutionCommitment::without_topups(
                    Hash::prehashed([0x64; 32]),
                    Hash::prehashed([0x65; 32]),
                    Hash::prehashed([0x66; 32]),
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

    fn limited_msg() -> iroha_core::NetworkMessage {
        iroha_core::NetworkMessage::Health
    }

    fn sumeragi_v2_commit_certificate_request() -> BlockMessage {
        let requester = PeerId::new(KeyPair::random().public_key().clone());
        BlockMessage::V2(ConsensusMessageV2::new(
            ConsensusMessageV2Payload::CommitCertificateRequest(CommitCertificateRequest {
                protocol_version: PROTOCOL_VERSION,
                chain_id: "00000000-0000-0000-0000-000000000000"
                    .parse()
                    .expect("valid chain id"),
                context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"irohad-v2-context",
                ))),
                height: 9,
                requester,
                signature: vec![0xCC],
            }),
        ))
    }

    fn v2_vote_msg() -> iroha_core::NetworkMessage {
        sumeragi_msg(v2_vote_block_message())
    }

    fn retired_vrf_commit_msg() -> iroha_core::NetworkMessage {
        sumeragi_msg(BlockMessage::VrfCommit(
            iroha_data_model::consensus::VrfCommit {
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
            validator_set_hash: proposal.descriptor.validator_set_hash.clone(),
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
        iroha_core::NetworkMessage::ToriiProxyRequest(Box::new(ToriiProxyRequestV2 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V2,
            request_id: Hash::prehashed([0x41; 32]),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV1::Read(ToriiReadProxyRequestV1 {
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

fn snapshot_failure_allows_empty_state_fallback(
    error: &TryReadSnapshotError,
    provisional_imported_prefix: bool,
) -> bool {
    !provisional_imported_prefix && snapshot_read_error_is_recoverable(error)
}

fn snapshot_read_error_is_recoverable_for_bootstrap(
    error: &TryReadSnapshotError,
    hard_fork_snapshot_bootstrap: bool,
) -> bool {
    match error {
        TryReadSnapshotError::NotFound => true,
        TryReadSnapshotError::IO(_, _) => false,
        TryReadSnapshotError::ChainIdMismatch { .. } => false,
        TryReadSnapshotError::ZkConfigInstall(_) => false,
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
    state.set_crypto(config.crypto.clone());
    state.set_pipeline(config.pipeline.clone());
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

fn apply_state_config_before_kura_replay(
    state: &mut State,
    config: &Config,
) -> ReportResult<(), StartError> {
    apply_state_runtime_config_before_snapshot_auth(state, config);
    apply_state_geometry_config_before_kura_replay(state, config)
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
    fn snapshot_read_error_is_recoverable_classifies_errors() {
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
        let mut restored = iroha_config::parameters::actual::Nexus::default();
        restored.lane_config = RuntimeLaneConfig::from_catalog(&catalog);
        restored.lane_catalog = catalog.clone();
        restored.configured_lane_catalog = catalog.clone();
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
        let startup_trace_started_at = Instant::now();
        log_startup_trace("irohad.start.enter", startup_trace_started_at);

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
            Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap(
                &config.kura,
                &config.nexus.lane_config,
                &config.nexus.configured_lane_catalog,
                &config.snapshot.bootstrap,
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
            |key| iroha_crypto::KeyPair::from(key.0.clone()),
        );

        let stored_genesis_block =
            read_stored_genesis_block(kura.as_ref(), block_count, provisional_imported_prefix)?;
        let stored_genesis_hash = stored_genesis_block.as_ref().map(|block| block.0.hash());
        if !provisional_imported_prefix
            && block_count.0 == 0
            && stored_genesis_block.is_none()
            && genesis.is_none()
        {
            return Err(Report::new(StartError::InitKura).attach(
                "fresh Sumeragi v2 startup requires a local signed genesis; peer bootstrap cannot authenticate the genesis-selected handshake context",
            ));
        }
        let effective_genesis = stored_genesis_block.as_ref().or(genesis.as_ref());
        if effective_genesis.is_none() && !provisional_imported_prefix {
            return Err(Report::new(StartError::InitKura).attach(
                "Sumeragi v2 requires signed genesis metadata unless an imported snapshot prefix is awaiting signed-lineage authentication",
            ));
        }
        let signed_genesis_context = if provisional_imported_prefix {
            None
        } else {
            effective_genesis
                .map(signed_v2_genesis_context_metadata)
                .transpose()
                .map_err(|error| Report::new(StartError::InitKura).attach(error))?
        };

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

        let mut loaded_state_from_snapshot = false;
        let mut state = match try_read_snapshot_with_bootstrap_policy(
            config.snapshot.store_dir.resolve_relative_path(),
            &kura,
            || live_query_store.clone(),
            block_count,
            config.snapshot.merkle_chunk_size_bytes,
            config.snapshot.max_payload_bytes,
            verification_key,
            &config.common.chain,
            &config.zk,
            &config.snapshot.bootstrap,
            #[cfg(feature = "telemetry")]
            state_telemetry.clone(),
        ) {
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
                State::try_new_with_chain(
                    world,
                    Arc::clone(&kura),
                    live_query_store.clone(),
                    config.common.chain.clone(),
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
                    "Failed to load state snapshot; rebuilding state by replaying Kura blocks"
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
                State::try_new_with_chain(
                    world,
                    Arc::clone(&kura),
                    live_query_store.clone(),
                    config.common.chain.clone(),
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
        // Thread chain id into state for VRF prehash binding.
        state.chain_id = config.common.chain.clone();
        if !loaded_state_from_snapshot {
            // Snapshot candidates install this at their post-decode,
            // pre-reconciliation boundary. Fresh and Kura-rebuilt state has no
            // snapshot boundary, so install it exactly once here before replay.
            install_zk_config_before_kura_replay(&mut state, &config)?;
        }
        apply_state_runtime_config_before_snapshot_auth(&mut state, &config);
        if !provisional_imported_prefix {
            apply_state_geometry_config_before_kura_replay(&mut state, &config)?;
        }

        // Resolve the complete replay boundary before selecting any consensus trust source. A
        // signed snapshot lineage is authoritative only when the typed imported prefix and exact
        // Kura boundary have already been authenticated.
        let mut v2_replay_plan = iroha_core::sumeragi::plan_v2_startup_replay(kura.as_ref())
            .map_err(|err| Report::new(StartError::InitKura).attach(err))?;
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
            (Some(mode), Some(bootstrap)) if mode == bootstrap.context.mode => (mode, None),
            (Some(_), Some(_)) => {
                return Err(Report::new(StartError::InitKura).attach(
                    "authenticated snapshot mode differs from its retained bootstrap lineage",
                ));
            }
            (None, None) => {
                let (mode, context) = signed_genesis_context.clone().ok_or_else(|| {
                        Report::new(StartError::InitKura).attach(
                            "startup has neither authenticated snapshot lineage nor signed genesis metadata",
                        )
                    })?;
                (mode, Some(context))
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
            let signed_v2_genesis_context = signed_v2_genesis_context
                .as_ref()
                .expect("normal startup selected signed genesis metadata");
            match kura
                .v2_finality_artifact(1)
                .map_err(|error| Report::new(StartError::InitKura).attach(error))?
            {
                Some(artifact) => {
                    let context = &artifact.height_context;
                    if context.chain_id != config.common.chain
                        || context.height != 1
                        || context.mode != signed_consensus_mode
                        || context.da_layout != signed_v2_genesis_context.da_layout
                        || context.nexus_amx_context_hash
                            != iroha_crypto::Hash::prehashed(
                                signed_v2_genesis_context.nexus_amx_context_hash,
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
        if generic_replay_height > state_height {
            iroha_logger::info!(
                start_height = generic_replay_start,
                generic_replay_height,
                pending_v2_tip = ?v2_replay_plan.pending_tip_height(),
                "Replaying authenticated complete Kura prefix"
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
                generic_replay_start,
                generic_replay_height,
                signed_consensus_mode,
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
        if runtime_nexus.enabled {
            let lane_manifests = Arc::new(LaneManifestRegistry::from_config(
                lane_catalog.as_ref(),
                governance_catalog.as_ref(),
                &registry_cfg,
            ));
            lane_manifests
                .validate_active_coverage()
                .map_err(|err| Report::new(err).change_context(StartError::InitKura))?;
            queue.install_lane_manifests_with_state(&lane_manifests, &state);
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
            let empty_registry = Arc::new(
                LaneManifestRegistry::empty()
                    .rebind(lane_catalog.as_ref(), governance_catalog.as_ref()),
            );
            queue.install_lane_manifests_with_state(&empty_registry, &state);
            state
                .telemetry
                .set_lane_manifest_registry(Arc::clone(&empty_registry));
        }
        // Independent lane producers transfer FIFO ownership before they
        // publish any payload bytes. Install and replay that durable ownership
        // journal before the ordinary queue-plan journal can reinsert pending
        // transactions, regardless of whether plan journaling is enabled.
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
            "lane queue reservation journal installed"
        );

        if config.queue.plan_journal_enabled {
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
                tombstoned_stale = replay_summary.tombstoned_stale,
                tombstoned_malformed = replay_summary.tombstoned_malformed,
                rejected = replay_summary.rejected,
                "queue plan journal installed"
            );
        } else {
            queue
                .finalize_plan_journal_startup_disabled()
                .map_err(|err| {
                    Report::new(StartError::InitKura).attach(format!(
                        "failed to finalize disabled queue plan journal startup: {err}"
                    ))
                })?;
        }

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
            computed_bls_domain,
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
                    None,
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
                        &config.common.chain,
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
        iroha_logger::info!(
            mode=%consensus_caps.mode_tag,
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
            features: Some(confidential_handshake_policy_digest(confidential_features)),
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
            Some(consensus_caps.clone()),
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

        // Audited snapshot bootstrap is a complete alternative trust root and must not advertise
        // an unrelated local legacy genesis file as if it authenticated the imported history.
        if snapshot_bootstrap_active {
            iroha_logger::info!(
                "authenticated snapshot bootstrap: legacy genesis request serving is disabled"
            );
        } else if let Some(stored_genesis) = stored_genesis_block.as_ref() {
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
                    consensus_caps_from_genesis(
                        genesis_block,
                        &config.common.chain,
                        &config_caps,
                        &config.sumeragi,
                    )
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
                    || fresh_caps.mode_tag != consensus_caps.mode_tag
                    || fresh_caps.proto_version != consensus_caps.proto_version
                {
                    return Err(Report::new(StartError::InitKura).attach(
                        "fresh signed genesis consensus metadata differs from the peer handshake opened for bootstrap",
                    ));
                }
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
                    Ok((_valid_block, state_block)) => {
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
        let settlement_cfg = config.settlement.clone();
        let gov_cfg = config.gov.clone();
        let oracle_cfg = config.oracle.clone();
        let streaming_cfg = config.streaming.clone();
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
        state.set_streaming(streaming_cfg);
        state.set_fraud_monitoring(fraud_cfg);
        state.set_settlement(settlement_cfg);
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
            config.common.trusted_peers.value().clone(),
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
            network: network.clone(),
            genesis_network: GenesisWithPubKey {
                genesis: genesis_for_consensus,
                public_key: effective_genesis_public_key.clone(),
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

        let sorafs_node = sorafs_node::NodeHandle::try_new_with_policies(
            sorafs_node::config::StorageConfig::from(&config.torii.sorafs_storage),
            sorafs_node::config::RepairConfig::from_repair_and_policy(
                &config.torii.sorafs_repair,
                &state.gov.sorafs_repair_escalation,
            ),
            sorafs_node::config::GcConfig::from(&config.torii.sorafs_gc),
        )
        .map_err(|err| {
            Report::new(StartError::StartTorii).attach(format!(
                "failed to initialise embedded SoraFS runtime: {err}"
            ))
        })?;
        let shared_sorafs_cache = build_shared_sorafs_provider_cache(&config);

        let chain_id = Arc::new(config.common.chain.clone());
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
                relay_worker_storage_root,
                Arc::clone(&chain_id),
                Arc::clone(&queue),
                Arc::clone(&state),
                sumeragi.clone(),
                relay_worker_authority,
                config.common.key_pair.clone(),
                config.zk.fastpq.clone(),
            )
            .map_err(|error| {
                Report::new(StartError::InitKura)
                    .attach(format!("failed to start Nexus fee relay worker: {error:?}"))
            })?;
            supervisor.monitor(relay_worker.start(supervisor.shutdown_signal()));
        }
        let local_validator_account_id =
            AccountId::new(config.common.key_pair.public_key().clone());
        let local_peer_id = config.common.trusted_peers.value().myself.id().to_string();
        let runtime_mutation_sink = Arc::new(QueuedSoracloudRuntimeMutationSink::new(
            Arc::clone(&chain_id),
            Arc::clone(&queue),
            Arc::clone(&state),
            local_validator_account_id.clone(),
            config.common.key_pair.clone(),
            config.soracloud_runtime.submission.gas_asset_id.clone(),
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

        let receipt_signer = torii_receipt_signer_or_ephemeral(config.torii.receipt_signer.clone())
            .map_err(|err| {
                Report::new(StartError::StartTorii).attach(format!(
                    "failed to generate ephemeral Torii receipt signer: {err}"
                ))
            })?;
        let runtime_deps = iroha_torii::ToriiRuntimeDeps::new(torii_telemetry)
            .with_soracloud_runtime(Arc::new(soracloud_runtime.clone()))
            .with_soracloud_hf_config(config.soracloud_runtime.hf.clone())
            .with_sorafs_node(sorafs_node)
            .with_torii_proxy_bridge_signer(config.common.key_pair.clone())
            .with_vpn_helper_ticket_secret(config.network.soranet_vpn.helper_ticket_secret);
        let runtime_deps = if let Some(cache) = shared_sorafs_cache {
            runtime_deps.with_sorafs_cache(cache)
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
                tx_gossiper,
                peers_gossiper,
                network: network.clone(),
                streaming: streaming.clone(),
                kiso: kiso.clone(),
                suppress_pow_broadcast: Arc::clone(&suppress_pow_broadcast),
                pow_update_version: Arc::clone(&pow_update_version),
                consensus_ingress: ConsensusIngressLimiter::from_config(
                    &config.network,
                    Duration::from_millis(signed_block_cadence_ms),
                ),
                low_priority_ingress: LowPriorityIngressLimiter::from_config(&config.network),
            }
            .run(),
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
    /// Failed to persist configuration updates back to disk.
    WriteConfig,
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
    /// Nexus auto-derived storage defaults require a writable config path.
    NexusStorageBudgetPersistenceRequired,
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReadConfig => write!(
                f,
                "Error occurred while reading configuration from file(s) and environment"
            ),
            Self::WriteConfig => write!(f, "Error occurred while writing configuration to disk"),
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
            Self::NexusStorageBudgetPersistenceRequired => write!(
                f,
                "Nexus auto-derived storage defaults require a writable configuration file path"
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
    iroha_core::sumeragi::set_sumeragi_stack_size_bytes(concurrency.sumeragi_stack_bytes);
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

    let storage_budget_filesystems =
        reconcile_nexus_storage_budget(&mut config, args.config.as_deref())?;
    config.apply_storage_budget();
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

    let stack_budget_bytes = ivm_stack_budget_bytes(&config);
    apply_concurrency_config(&config.concurrency, stack_budget_bytes);

    // Apply Norito settings immediately so subsequent Norito decode/encode (e.g., genesis)
    // uses the configured archive bounds and GPU offload policy.
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
    available_bytes: u64,
    components: Vec<NexusStorageBudgetComponent>,
}

fn reconcile_nexus_storage_budget(
    config: &mut Config,
    config_path: Option<&Path>,
) -> ReportResult<Vec<StorageBudgetFilesystemProbe>, ConfigError> {
    if !config.nexus.enabled {
        return Ok(Vec::new());
    }

    match config.nexus.storage.budget_source {
        NexusStorageBudgetSource::Unset => {
            let config_path = require_nexus_storage_budget_config_path(
                config_path,
                "nexus.storage.local_budget_bytes is unset and the daemon must persist an auto-derived filesystem budget before startup can continue",
            )?;
            let filesystems = probe_nexus_storage_filesystems(config)?;
            let auto_default = derive_auto_default_nexus_storage_budget(&filesystems);
            persist_nexus_auto_storage_budget(config_path, &auto_default)?;
            activate_auto_default_nexus_storage_budget(config, auto_default.clone());
            iroha_logger::info!(
                config_path = %config_path.display(),
                aggregate_budget_bytes = auto_default.aggregate_budget_bytes,
                filesystem_groups = auto_default.filesystem_groups.len(),
                "persisted first-run nexus.storage.local_budget_bytes and auto-derived filesystem metadata into config"
            );
            Ok(filesystems)
        }
        NexusStorageBudgetSource::OperatorExplicit => {
            match probe_nexus_storage_filesystems(config) {
                Ok(filesystems) => Ok(filesystems),
                Err(error) => {
                    iroha_logger::warn!(
                        ?error,
                        "failed to probe Nexus storage filesystems for warning-only budget checks; continuing with operator-explicit budget"
                    );
                    Ok(Vec::new())
                }
            }
        }
        NexusStorageBudgetSource::AutoDerived => {
            let filesystems = probe_nexus_storage_filesystems(config)?;
            let Some(auto_default) = config.nexus.storage.auto_default.clone() else {
                return Ok(filesystems);
            };
            if storage_layout_matches_auto_default(&filesystems, &auto_default) {
                return Ok(filesystems);
            }

            let config_path = require_nexus_storage_budget_config_path(
                config_path,
                "the Nexus storage filesystem layout changed and the persisted auto-derived budget metadata must be rewritten",
            )?;
            let regenerated = derive_auto_default_nexus_storage_budget(&filesystems);
            persist_nexus_auto_storage_budget(config_path, &regenerated)?;
            activate_auto_default_nexus_storage_budget(config, regenerated.clone());
            iroha_logger::info!(
                config_path = %config_path.display(),
                aggregate_budget_bytes = regenerated.aggregate_budget_bytes,
                filesystem_groups = regenerated.filesystem_groups.len(),
                "regenerated nexus.storage.auto_default after the storage filesystem layout changed"
            );
            Ok(filesystems)
        }
    }
}

fn require_nexus_storage_budget_config_path<'a>(
    config_path: Option<&'a Path>,
    detail: &'static str,
) -> ReportResult<&'a Path, ConfigError> {
    config_path.ok_or_else(|| {
        Report::new(ConfigError::NexusStorageBudgetPersistenceRequired).attach(detail)
    })
}

fn probe_nexus_storage_filesystems(
    config: &Config,
) -> ReportResult<Vec<StorageBudgetFilesystemProbe>, ConfigError> {
    let mut groups = BTreeMap::<String, StorageBudgetFilesystemProbe>::new();

    for (component, root) in effective_nexus_storage_component_roots(config) {
        let normalized_root = normalize_budget_probe_path(root).ok_or_else(|| {
            Report::new(ConfigError::ParseConfig).attach(format!(
                "failed to resolve Nexus storage root for component `{}` against the current directory",
                component.as_str()
            ))
        })?;
        let probe_path = nearest_existing_ancestor(&normalized_root).ok_or_else(|| {
            Report::new(ConfigError::ParseConfig).attach(format!(
                "failed to find an existing ancestor for Nexus storage root `{}` (component `{}`)",
                normalized_root.display(),
                component.as_str()
            ))
        })?;
        let filesystem_id = filesystem_identity(&probe_path).ok_or_else(|| {
            filesystem_probe_config_error(format!(
                "failed to determine the filesystem identity for `{}` (component `{}`)",
                probe_path.display(),
                component.as_str()
            ))
        })?;
        let available_bytes = filesystem_available_bytes(&probe_path).ok_or_else(|| {
            filesystem_probe_config_error(format!(
                "failed to determine available free space for `{}` (component `{}`)",
                probe_path.display(),
                component.as_str()
            ))
        })?;

        groups
            .entry(filesystem_id.clone())
            .and_modify(|group| {
                if !group.components.contains(&component) {
                    group.components.push(component);
                }
            })
            .or_insert_with(|| StorageBudgetFilesystemProbe {
                filesystem_id,
                path: probe_path,
                available_bytes,
                components: vec![component],
            });
    }

    let mut groups: Vec<_> = groups.into_values().collect();
    for group in &mut groups {
        group.components.sort_unstable();
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

fn derive_auto_default_nexus_storage_budget(
    filesystems: &[StorageBudgetFilesystemProbe],
) -> NexusStorageAutoDefault {
    let filesystem_groups: Vec<_> = filesystems
        .iter()
        .map(|filesystem| NexusStorageAutoDefaultFilesystemGroup {
            filesystem_id: filesystem.filesystem_id.clone(),
            budget_bytes: filesystem
                .available_bytes
                .saturating_mul(80)
                .saturating_div(100),
            components: filesystem.components.clone(),
        })
        .collect();
    let aggregate_budget_bytes = filesystem_groups.iter().fold(0_u64, |total, filesystem| {
        total.saturating_add(filesystem.budget_bytes)
    });

    NexusStorageAutoDefault {
        version: NexusStorageAutoDefault::VERSION,
        aggregate_budget_bytes,
        filesystem_groups,
    }
}

fn storage_layout_matches_auto_default(
    filesystems: &[StorageBudgetFilesystemProbe],
    auto_default: &NexusStorageAutoDefault,
) -> bool {
    let mut current_signature: Vec<_> = filesystems
        .iter()
        .map(|filesystem| {
            (
                filesystem.filesystem_id.clone(),
                filesystem.components.clone(),
            )
        })
        .collect();
    let mut persisted_signature: Vec<_> = auto_default
        .filesystem_groups
        .iter()
        .map(|filesystem| {
            (
                filesystem.filesystem_id.clone(),
                filesystem.components.clone(),
            )
        })
        .collect();

    current_signature.sort_by(|left, right| {
        left.1
            .first()
            .map_or(usize::MAX, |component| {
                nexus_storage_component_order(*component)
            })
            .cmp(&right.1.first().map_or(usize::MAX, |component| {
                nexus_storage_component_order(*component)
            }))
            .then_with(|| left.0.cmp(&right.0))
    });
    persisted_signature.sort_by(|left, right| {
        left.1
            .first()
            .map_or(usize::MAX, |component| {
                nexus_storage_component_order(*component)
            })
            .cmp(&right.1.first().map_or(usize::MAX, |component| {
                nexus_storage_component_order(*component)
            }))
            .then_with(|| left.0.cmp(&right.0))
    });

    current_signature == persisted_signature
}

fn activate_auto_default_nexus_storage_budget(
    config: &mut Config,
    auto_default: NexusStorageAutoDefault,
) {
    config.nexus.storage.max_disk_usage_bytes =
        iroha_config::base::util::Bytes(auto_default.aggregate_budget_bytes);
    config.nexus.storage.budget_source = NexusStorageBudgetSource::AutoDerived;
    config.nexus.storage.auto_default = Some(auto_default);
}

fn persist_nexus_auto_storage_budget(
    config_path: &Path,
    auto_default: &NexusStorageAutoDefault,
) -> ReportResult<(), ConfigError> {
    let config_text = fs::read_to_string(config_path)
        .attach(format!(
            "read config `{}` before persisting storage budget",
            config_path.display()
        ))
        .change_context(ConfigError::WriteConfig)?;
    let mut config_table: toml::Table = toml::from_str(&config_text)
        .attach(format!(
            "parse config `{}` before persisting storage budget",
            config_path.display()
        ))
        .change_context(ConfigError::WriteConfig)?;
    let persisted_budget = nexus_storage_budget_toml_integer(
        auto_default.aggregate_budget_bytes,
        "aggregate Nexus storage budget",
        config_path,
    )?;
    iroha_config::base::toml::Writer::new(&mut config_table)
        .write(["nexus", "storage", "local_budget_bytes"], persisted_budget);
    let auto_default_table = nexus_storage_auto_default_to_toml(auto_default, config_path)?;
    iroha_config::base::toml::Writer::new(&mut config_table).write(
        ["nexus", "storage", "auto_default"],
        toml::Value::Table(auto_default_table),
    );
    let encoded = toml::to_string(&config_table)
        .attach(format!(
            "encode config `{}` after persisting storage budget",
            config_path.display()
        ))
        .change_context(ConfigError::WriteConfig)?;
    write_bytes_atomic(config_path, encoded.as_bytes())
        .attach(format!(
            "write config `{}` after persisting storage budget",
            config_path.display()
        ))
        .change_context(ConfigError::WriteConfig)?;
    Ok(())
}

fn nexus_storage_auto_default_to_toml(
    auto_default: &NexusStorageAutoDefault,
    config_path: &Path,
) -> ReportResult<toml::Table, ConfigError> {
    let mut table = toml::Table::new();
    table.insert(
        "version".to_string(),
        toml::Value::Integer(i64::from(auto_default.version)),
    );
    table.insert(
        "aggregate_budget_bytes".to_string(),
        toml::Value::Integer(nexus_storage_budget_toml_integer(
            auto_default.aggregate_budget_bytes,
            "aggregate Nexus storage budget",
            config_path,
        )?),
    );

    let filesystem_groups = auto_default
        .filesystem_groups
        .iter()
        .map(|filesystem| {
            let mut filesystem_table = toml::Table::new();
            filesystem_table.insert(
                "filesystem_id".to_string(),
                toml::Value::String(filesystem.filesystem_id.clone()),
            );
            filesystem_table.insert(
                "budget_bytes".to_string(),
                toml::Value::Integer(nexus_storage_budget_toml_integer(
                    filesystem.budget_bytes,
                    "filesystem Nexus storage budget",
                    config_path,
                )?),
            );
            filesystem_table.insert(
                "components".to_string(),
                toml::Value::Array(
                    filesystem
                        .components
                        .iter()
                        .map(|component| toml::Value::String(component.as_str().to_owned()))
                        .collect(),
                ),
            );
            Ok(toml::Value::Table(filesystem_table))
        })
        .collect::<ReportResult<Vec<_>, ConfigError>>()?;
    table.insert(
        "filesystem_groups".to_string(),
        toml::Value::Array(filesystem_groups),
    );

    Ok(table)
}

fn nexus_storage_budget_toml_integer(
    budget_bytes: u64,
    label: &'static str,
    config_path: &Path,
) -> ReportResult<i64, ConfigError> {
    i64::try_from(budget_bytes)
        .attach(format!(
            "{label} {budget_bytes} does not fit into TOML integer range for `{}`",
            config_path.display()
        ))
        .change_context(ConfigError::WriteConfig)
}

fn warn_if_nexus_storage_budget_exceeds_available(
    config: &Config,
    filesystems: &[StorageBudgetFilesystemProbe],
) {
    if filesystems.is_empty() || !config.nexus.enabled {
        return;
    }

    match config.nexus.storage.budget_source {
        NexusStorageBudgetSource::Unset => {}
        NexusStorageBudgetSource::AutoDerived => {
            let Some(auto_default) = config.nexus.storage.auto_default.as_ref() else {
                return;
            };
            for filesystem in filesystems {
                let Some(budget_bytes) = auto_default_budget_shortfall(auto_default, filesystem)
                else {
                    continue;
                };
                iroha_logger::warn!(
                    filesystem_id = %filesystem.filesystem_id,
                    path = %filesystem.path.display(),
                    components = ?nexus_storage_component_labels(&filesystem.components),
                    budget_bytes,
                    available_bytes = filesystem.available_bytes,
                    "stored auto-derived Nexus filesystem budget exceeds currently available free disk space"
                );
            }
        }
        NexusStorageBudgetSource::OperatorExplicit => {
            for filesystem in filesystems {
                let Some(assigned_budget) = operator_explicit_budget_shortfall(config, filesystem)
                else {
                    continue;
                };
                iroha_logger::warn!(
                    filesystem_id = %filesystem.filesystem_id,
                    path = %filesystem.path.display(),
                    components = ?nexus_storage_component_labels(&filesystem.components),
                    assigned_budget_bytes = assigned_budget,
                    available_bytes = filesystem.available_bytes,
                    "effective operator-configured Nexus storage caps exceed currently available free disk space on a filesystem"
                );
            }
        }
    }
}

fn auto_default_budget_shortfall(
    auto_default: &NexusStorageAutoDefault,
    filesystem: &StorageBudgetFilesystemProbe,
) -> Option<u64> {
    let stored_group = auto_default.filesystem_groups.iter().find(|stored_group| {
        stored_group.filesystem_id == filesystem.filesystem_id
            && stored_group.components == filesystem.components
    })?;
    (stored_group.budget_bytes > filesystem.available_bytes).then_some(stored_group.budget_bytes)
}

fn operator_explicit_budget_shortfall(
    config: &Config,
    filesystem: &StorageBudgetFilesystemProbe,
) -> Option<u64> {
    let assigned_budget = effective_assigned_budget_for_filesystem(config, filesystem);
    (assigned_budget > filesystem.available_bytes).then_some(assigned_budget)
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
    if path.is_absolute() {
        Some(path)
    } else {
        std::env::current_dir().ok().map(|cwd| cwd.join(path))
    }
}

fn nearest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path must have a parent")
    })?;
    fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
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
fn filesystem_available_bytes(path: &Path) -> Option<u64> {
    let stats = rustix::fs::statvfs(path).ok()?;
    let fragment_size = stats.f_frsize.max(stats.f_bsize);
    Some(stats.f_bavail.saturating_mul(fragment_size))
}

#[cfg(target_os = "windows")]
fn filesystem_available_bytes(path: &Path) -> Option<u64> {
    let wide_path = windows_wide_path(path);
    let mut free_bytes_available = 0_u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide_path.as_ptr(),
            &mut free_bytes_available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free_bytes_available)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn filesystem_available_bytes(_path: &Path) -> Option<u64> {
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
        "5434666dee1a353467a927189b27422a9c85366a14134ba54b3be83a1beed13d";

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

fn read_genesis_unlocked(path: &Path) -> ReportResult<GenesisBlock, ConfigError> {
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
        let gpu_kind = backend.map_or(telemetry_labels.gpu_kind.as_ref(), |kind| kind.as_str());
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
        .enable_all()
        .build()
        .map_err(Report::from)
        .change_context(MainError::IrohaStart)?;

    let result = rt.block_on(run_node(config, genesis));
    rt.shutdown_timeout(NODE_RUNTIME_SHUTDOWN_TIMEOUT);
    result
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
        nexus_policy_digest,
    })
}

fn consensus_caps_from_genesis(
    genesis: &GenesisBlock,
    chain_id: &ChainId,
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
        consensus_entry_caps(chain_id, entry, &params).ok()?;
    if entry.consensus_fingerprint.into_bytes() != computed_fingerprint {
        return None;
    }
    let mut config_caps = *config_caps;
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
            mode_tag,
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
    chain_id: &ChainId,
    entry: &ConsensusHandshakeMeta,
    params: &iroha_data_model::parameter::Parameters,
) -> Result<(
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
        .map_err(|error| eyre!(error))?;

    let fingerprint = iroha_core::sumeragi::consensus::compute_consensus_fingerprint_from_params(
        chain_id,
        &consensus_params,
    )
    .map_err(|error| eyre!(error))?;

    Ok((mode_tag.to_string(), consensus_params, fingerprint))
}

fn compute_consensus_handshake_caps(
    world: &impl iroha_core::state::WorldReadOnly,
    height: u64,
    config: &Config,
    config_caps: &iroha_p2p::ConsensusConfigCaps,
    frozen_mode: iroha_data_model::block::consensus_v2::ConsensusMode,
    signed_v2_context: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
    override_caps: Option<(String, String, iroha_p2p::ConsensusHandshakeCaps)>,
) -> ReportResult<(String, String, iroha_p2p::ConsensusHandshakeCaps), StartError> {
    if let Some((mode_tag, bls_domain, caps)) = override_caps {
        return Ok((mode_tag, bls_domain, caps));
    }

    iroha_core::sumeragi::consensus::compute_consensus_handshake_caps_from_world(
        world,
        height,
        &config.common,
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
            matched_meta = Some(meta.clone());
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
    let computed_fp = iroha_core::sumeragi::consensus::compute_consensus_fingerprint_from_params(
        &config.common.chain,
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
        let suppressed_by_panic_hook = panic_hook::is_suppressed();
        let suppressed_by_norito_decode = norito::decode_panic_suppressed();
        if suppressed_by_panic_hook || suppressed_by_norito_decode {
            let panic_file = info.location().map(|location| location.file());
            let panic_line = info.location().map(|location| location.line());
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

    mod replay_startup_config {
        use super::*;

        #[test]
        fn installs_actual_zk_config_for_fresh_state_before_kura_replay() {
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

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);

            install_zk_config_before_kura_replay(&mut state, &config)
                .expect("fresh state accepts actual ZK configuration");

            let installed = state.zk_snapshot();
            assert_eq!(
                installed.sccp.max_pending_outbound_messages,
                config.zk.sccp.max_pending_outbound_messages
            );
            assert_eq!(
                installed.sccp.max_pending_outbound_payload_bytes,
                config.zk.sccp.max_pending_outbound_payload_bytes
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
        fn defaults_to_ephemeral_secp256k1() {
            let signer = torii_receipt_signer_or_ephemeral(None)
                .expect("checked ephemeral receipt signer generation should succeed");
            assert_eq!(signer.algorithm(), Algorithm::Secp256k1);
        }

        #[test]
        fn preserves_configured_receipt_signer() {
            let configured = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let signer = torii_receipt_signer_or_ephemeral(Some(configured.clone()))
                .expect("configured receipt signer should not require randomness");
            assert_eq!(signer.public_key(), configured.public_key());
            assert_eq!(signer.algorithm(), Algorithm::Ed25519);
        }
    }

    mod relay_ingress {
        use super::*;
        use iroha_core::torii_proxy::{
            TORII_PROXY_REQUEST_VERSION_V2, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV1, ToriiProxyRequestV2,
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
                iroha_core::NetworkMessage::ToriiProxyRequest(Box::new(ToriiProxyRequestV2 {
                    schema_version: TORII_PROXY_REQUEST_VERSION_V2,
                    request_id: Hash::new(b"torii-proxy-request"),
                    hop_count: 1,
                    max_hops: 3,
                    visited_peer_ids: Vec::new(),
                    request: ToriiProxyRequestKindV1::Read(ToriiReadProxyRequestV1 {
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

            assert_eq!(msg, RelayBurstRecv::Skip);
            assert_eq!(budget, 1);
            assert!(matches!(rx.try_recv(), Ok(7)));
        }

        #[test]
        fn try_recv_after_burst_consumes_when_due() {
            let (tx, mut rx) = mpsc::channel(1);
            tx.try_send(9).expect("send low message");
            let mut budget = 0;

            let msg = try_recv_after_burst(&mut rx, &mut budget, 4);

            assert!(matches!(msg, RelayBurstRecv::Message(9)));
            assert_eq!(budget, 4);
            assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
        }

        #[test]
        fn try_recv_after_burst_resets_budget_when_empty() {
            let (_tx, mut rx) = mpsc::channel::<u8>(1);
            let mut budget = 0;

            let msg = try_recv_after_burst(&mut rx, &mut budget, 4);

            assert_eq!(msg, RelayBurstRecv::Skip);
            assert_eq!(budget, 4);
        }

        #[test]
        fn try_recv_after_burst_reports_disconnected_receiver() {
            let (tx, mut rx) = mpsc::channel::<u8>(1);
            drop(tx);
            let mut budget = 0;

            let msg = try_recv_after_burst(&mut rx, &mut budget, 4);

            assert_eq!(msg, RelayBurstRecv::Disconnected);
            assert_eq!(budget, 4);
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
                .try_send(RelayWorkItem {
                    peer: Peer::new(
                        "127.0.0.1:0".parse().expect("socket address"),
                        keypair.public_key().clone(),
                    ),
                    payload: iroha_core::NetworkMessage::Health,
                    payload_bytes: 0,
                })
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
            let mode_tag = match handshake_meta.mode {
                iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => {
                    iroha_core::sumeragi::consensus::PERMISSIONED_TAG.to_string()
                }
                iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => {
                    iroha_core::sumeragi::consensus::NPOS_TAG.to_string()
                }
            };
            let proto = handshake_meta
                .wire_protocol_version
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
            let config_caps = build_consensus_config_caps(&config.nexus, None, None)
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

            let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
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
        fn fresh_v2_genesis_staging_does_not_commit_state_or_kura() {
            use std::sync::Arc;

            use iroha_core::{block::ValidBlock, kura::Kura, query::store::LiveQueryStore};

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
            let genesis = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
                .set_topology(topology)
                .build_and_sign(&genesis_authority)
                .expect("signed genesis");

            let authority_id = AccountId::new(genesis_authority.public_key().clone());
            let world = World::with(
                [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority_id)],
                [Account::new(authority_id.clone()).build(&authority_id)],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let mut state = State::new_with_chain_for_testing(
                world,
                Arc::clone(&kura),
                LiveQueryStore::start_test(),
                chain_id.clone(),
            );
            state.set_pipeline(iroha_config::parameters::actual::Pipeline::default());
            state
                .set_nexus(iroha_config::parameters::actual::Nexus::default())
                .expect("default Nexus config");
            let nexus = state.nexus_snapshot();
            let lane_manifests = Arc::new(
                LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
            );
            state.install_lane_manifests(&lane_manifests);
            let mut crypto = iroha_config::parameters::actual::Crypto::default();
            if !crypto.allowed_signing.contains(&Algorithm::BlsNormal) {
                crypto.allowed_signing.push(Algorithm::BlsNormal);
            }
            state.set_crypto(crypto);

            let voters = iroha_core::sumeragi::signed_genesis_voting_peers(&genesis)
                .expect("signed voting roster");
            let topology = Topology::new(voters);
            let before_height = state.committed_height();
            let before_hashes = state.committed_block_hashes_snapshot();
            let mut voting_block = None;
            let (_valid, staged) = ValidBlock::validate_keep_voting_block(
                genesis.0.clone(),
                &topology,
                &chain_id,
                &authority_id,
                &TimeSource::new_system(),
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {})
            .expect("genesis executes in staging overlay");
            let (mode, signed_parameters) =
                signed_v2_genesis_context_metadata(&genesis).expect("signed v2 metadata");
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
            let (mode_tag, _bls_domain, consensus_caps, _) = consensus_caps_from_genesis(
                &permissioned_genesis,
                &chain,
                &config_caps,
                &config.sumeragi,
            )
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

        fn load_config_with_overrides<F>(
            mut adjust: F,
        ) -> eyre::Result<(Config, tempfile::TempDir, PathBuf)>
        where
            F: FnMut(&mut toml::Table, &KeyPair),
        {
            let genesis_key_pair = KeyPair::random();
            let chain_discriminant = iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT;
            let manifest_json = norito::json!({
                "chain": "chain",
                "chain_discriminant": chain_discriminant,
                "executor": null,
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
                config: Some(config_path.clone()),
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

            Ok((config, dir, config_path))
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
            let chain_discriminant = iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT;
            let manifest_json = norito::json!({
                "chain": "chain",
                "chain_discriminant": chain_discriminant,
                "executor": null,
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
        fn read_config_persists_first_run_nexus_storage_budget() -> eyre::Result<()> {
            let (config, _dir, config_path) = load_config_with_overrides(|table, _genesis_key| {
                iroha_config::base::toml::Writer::new(table).write(["nexus", "enabled"], true);
            })?;

            assert_eq!(
                config.nexus.storage.budget_source,
                NexusStorageBudgetSource::AutoDerived
            );

            let effective_budget = config.nexus.storage.max_disk_usage_bytes.get();
            let auto_default = config
                .nexus
                .storage
                .auto_default
                .as_ref()
                .expect("config auto_default");
            assert_eq!(auto_default.aggregate_budget_bytes, effective_budget);
            assert_eq!(auto_default.sum_budget_bytes(), effective_budget);

            let persisted: toml::Value =
                toml::from_str(&std::fs::read_to_string(&config_path)?).expect("persisted config");
            let persisted_budget = persisted
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|nexus| nexus.get("storage"))
                .and_then(toml::Value::as_table)
                .and_then(|storage| storage.get("local_budget_bytes"))
                .and_then(toml::Value::as_integer)
                .expect("persisted local budget");
            assert_eq!(persisted_budget, i64::try_from(effective_budget)?);
            let persisted_auto_default = persisted
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|nexus| nexus.get("storage"))
                .and_then(toml::Value::as_table)
                .and_then(|storage| storage.get("auto_default"))
                .and_then(toml::Value::as_table)
                .expect("persisted auto_default");
            let persisted_auto_aggregate = persisted_auto_default
                .get("aggregate_budget_bytes")
                .and_then(toml::Value::as_integer)
                .expect("persisted auto aggregate");
            assert_eq!(persisted_auto_aggregate, persisted_budget);
            let filesystem_groups = persisted_auto_default
                .get("filesystem_groups")
                .and_then(toml::Value::as_array)
                .expect("persisted filesystem groups");
            assert!(
                !filesystem_groups.is_empty(),
                "auto-derived metadata must persist at least one filesystem group"
            );

            let persisted_once = std::fs::read_to_string(&config_path)?;
            let (_config_again, _genesis_again) = read_config_and_genesis(&Args {
                config: Some(config_path.clone()),
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
            assert_eq!(std::fs::read_to_string(&config_path)?, persisted_once);

            Ok(())
        }

        #[test]
        fn read_config_does_not_persist_local_budget_when_legacy_alias_is_present()
        -> eyre::Result<()> {
            let (config, _dir, config_path) = load_config_with_overrides(|table, _genesis_key| {
                iroha_config::base::toml::Writer::new(table)
                    .write(["nexus", "enabled"], true)
                    .write(["nexus", "storage", "max_disk_usage_bytes"], 4_096_i64);
            })?;

            assert_eq!(
                config.nexus.storage.budget_source,
                NexusStorageBudgetSource::OperatorExplicit
            );
            assert_eq!(config.nexus.storage.max_disk_usage_bytes.get(), 4_096);

            let persisted: toml::Value =
                toml::from_str(&std::fs::read_to_string(&config_path)?).expect("persisted config");
            let storage = persisted
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|nexus| nexus.get("storage"))
                .and_then(toml::Value::as_table)
                .expect("storage table");
            assert!(storage.get("local_budget_bytes").is_none());
            assert!(storage.get("auto_default").is_none());
            assert_eq!(
                storage
                    .get("max_disk_usage_bytes")
                    .and_then(toml::Value::as_integer),
                Some(4_096)
            );

            Ok(())
        }

        #[test]
        fn reconcile_nexus_storage_budget_requires_config_path_for_first_run_auto_default()
        -> eyre::Result<()> {
            let (mut config, _dir, _config_path) =
                parse_config_with_overrides(|table, _genesis_key| {
                    iroha_config::base::toml::Writer::new(table).write(["nexus", "enabled"], true);
                })?;

            let err = reconcile_nexus_storage_budget(&mut config, None)
                .expect_err("auto-derived first-run budget should require a writable config path");
            assert!(matches!(
                err.current_context(),
                ConfigError::NexusStorageBudgetPersistenceRequired
            ));

            Ok(())
        }

        #[test]
        fn read_config_regenerates_auto_default_when_storage_layout_changes() -> eyre::Result<()> {
            let (config, _dir, config_path) = load_config_with_overrides(|table, _genesis_key| {
                let mut filesystem_group = toml::Table::new();
                filesystem_group.insert(
                    "filesystem_id".to_string(),
                    toml::Value::String("dev:fake".to_string()),
                );
                filesystem_group.insert("budget_bytes".to_string(), toml::Value::Integer(1_024));
                filesystem_group.insert(
                    "components".to_string(),
                    toml::Value::Array(vec![
                        toml::Value::String("kura".to_string()),
                        toml::Value::String("wsv_cold".to_string()),
                        toml::Value::String("sorafs".to_string()),
                        toml::Value::String("soranet_spool".to_string()),
                        toml::Value::String("soravpn_spool".to_string()),
                    ]),
                );
                let mut auto_default = toml::Table::new();
                auto_default.insert("version".to_string(), toml::Value::Integer(1));
                auto_default.insert(
                    "aggregate_budget_bytes".to_string(),
                    toml::Value::Integer(1_024),
                );
                auto_default.insert(
                    "filesystem_groups".to_string(),
                    toml::Value::Array(vec![toml::Value::Table(filesystem_group)]),
                );

                let mut storage = toml::Table::new();
                storage.insert(
                    "local_budget_bytes".to_string(),
                    toml::Value::Integer(1_024),
                );
                storage.insert("auto_default".to_string(), toml::Value::Table(auto_default));
                let nexus = table
                    .entry("nexus")
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                    .as_table_mut()
                    .expect("nexus table");
                nexus.insert("enabled".to_string(), toml::Value::Boolean(true));
                nexus.insert("storage".to_string(), toml::Value::Table(storage));
            })?;

            assert_eq!(
                config.nexus.storage.budget_source,
                NexusStorageBudgetSource::AutoDerived
            );
            assert_ne!(
                config
                    .nexus
                    .storage
                    .auto_default
                    .as_ref()
                    .and_then(|auto_default| auto_default.filesystem_groups.first())
                    .map(|filesystem| filesystem.filesystem_id.as_str()),
                Some("dev:fake")
            );

            let persisted: toml::Value =
                toml::from_str(&std::fs::read_to_string(&config_path)?).expect("persisted config");
            let persisted_auto_default = persisted
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|nexus| nexus.get("storage"))
                .and_then(toml::Value::as_table)
                .and_then(|storage| storage.get("auto_default"))
                .and_then(toml::Value::as_table)
                .expect("persisted auto_default");
            let persisted_first_filesystem_id = persisted_auto_default
                .get("filesystem_groups")
                .and_then(toml::Value::as_array)
                .and_then(|groups| groups.first())
                .and_then(toml::Value::as_table)
                .and_then(|group| group.get("filesystem_id"))
                .and_then(toml::Value::as_str)
                .expect("persisted filesystem id");
            assert_ne!(persisted_first_filesystem_id, "dev:fake");

            Ok(())
        }

        #[test]
        fn auto_default_budget_shortfall_warns_only_when_budget_exceeds_available() {
            let filesystem = StorageBudgetFilesystemProbe {
                filesystem_id: "dev:1".to_string(),
                path: PathBuf::from("/tmp/storage"),
                available_bytes: 1_000,
                components: vec![NexusStorageBudgetComponent::Kura],
            };
            let auto_default = NexusStorageAutoDefault {
                version: NexusStorageAutoDefault::VERSION,
                aggregate_budget_bytes: 2_000,
                filesystem_groups: vec![NexusStorageAutoDefaultFilesystemGroup {
                    filesystem_id: "dev:1".to_string(),
                    budget_bytes: 2_000,
                    components: vec![NexusStorageBudgetComponent::Kura],
                }],
            };

            assert_eq!(
                auto_default_budget_shortfall(&auto_default, &filesystem),
                Some(2_000)
            );

            let mut no_shortfall = filesystem.clone();
            no_shortfall.available_bytes = 2_000;
            assert_eq!(
                auto_default_budget_shortfall(&auto_default, &no_shortfall),
                None
            );
        }

        #[test]
        fn operator_explicit_budget_shortfall_warns_only_when_assigned_caps_exceed_available()
        -> eyre::Result<()> {
            let (mut config, _dir, _config_path) =
                parse_config_with_overrides(|table, _genesis_key| {
                    iroha_config::base::toml::Writer::new(table)
                        .write(["nexus", "enabled"], true)
                        .write(["nexus", "storage", "local_budget_bytes"], 2_000_i64);
                })?;
            config.apply_storage_budget();

            let filesystem = StorageBudgetFilesystemProbe {
                filesystem_id: "dev:1".to_string(),
                path: PathBuf::from("/tmp/storage"),
                available_bytes: 1_000,
                components: vec![
                    NexusStorageBudgetComponent::Kura,
                    NexusStorageBudgetComponent::WsvCold,
                    NexusStorageBudgetComponent::Sorafs,
                    NexusStorageBudgetComponent::SoranetSpool,
                    NexusStorageBudgetComponent::SoravpnSpool,
                ],
            };

            assert_eq!(
                operator_explicit_budget_shortfall(&config, &filesystem),
                Some(2_000)
            );

            let mut no_shortfall = filesystem.clone();
            no_shortfall.available_bytes = 2_000;
            assert_eq!(
                operator_explicit_budget_shortfall(&config, &no_shortfall),
                None
            );

            Ok(())
        }

        #[test]
        fn normalize_windows_volume_mount_point_adds_trailing_separator() {
            assert_eq!(
                normalize_windows_volume_mount_point(r"C:\nexus\storage"),
                r"C:\nexus\storage\"
            );
            assert_eq!(
                normalize_windows_volume_mount_point(
                    r"\\?\Volume{ABCDEF12-3456-7890-ABCD-EF1234567890}\"
                ),
                r"\\?\Volume{ABCDEF12-3456-7890-ABCD-EF1234567890}\"
            );
        }

        #[test]
        fn normalize_windows_volume_identity_uses_lowercased_guid_path() {
            assert_eq!(
                normalize_windows_volume_identity(
                    r"\\?\Volume{ABCDEF12-3456-7890-ABCD-EF1234567890}\"
                ),
                r"volume:\\?\volume{abcdef12-3456-7890-abcd-ef1234567890}\"
            );
        }

        #[test]
        fn windows_string_from_wide_buffer_stops_at_first_nul() {
            let buffer: Vec<u16> = "Volume\0ignored".encode_utf16().collect();
            assert_eq!(
                windows_string_from_wide_buffer(&buffer).as_deref(),
                Some("Volume")
            );
            assert_eq!(windows_string_from_wide_buffer(&[]), None);
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
            let (config, _dir, _config_path) =
                load_config_with_overrides(|table, _genesis_key| {
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
            let (config, _dir, _config_path) =
                load_config_with_overrides(|table, _genesis_key| {
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
            let (config, _dir, _config_path) =
                load_config_with_overrides(|table, _genesis_key| {
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
            let (config, _dir, _config_path) =
                load_config_with_overrides(|table, _genesis_key| {
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
            let (config, _dir, _config_path) =
                load_config_with_overrides(|table, _genesis_key| {
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

        let _registry_guard = instruction_registry_test_guard();

        // Start with an empty registry to simulate uninitialized state.
        set_instruction_registry(InstructionRegistry::new());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.genesis.signed.nrt");
        std::fs::write(&path, [0u8, 1u8, 2u8, 3u8]).unwrap();

        // `read_genesis` should initialize the registry internally and simply
        // return a decode error for the bogus file instead of panicking.
        let res = read_genesis_unlocked(&path);
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

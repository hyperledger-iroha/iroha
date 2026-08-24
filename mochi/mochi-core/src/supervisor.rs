//! Process supervision primitives for managing local `iroha3d` peers.
//!
//! The supervisor prepares filesystem layouts, generates a Kagami-aligned
//! default genesis manifest, and can launch or stop child `iroha3d` processes.
use crate::{
    compose::{SigningAuthority, development_signing_authorities},
    config::{
        GenesisProfile, NetworkPaths, NetworkProfile, PortAllocator, ProfilePreset,
        infer_workspace_root_from_sandbox_root,
    },
    generation::{
        GenerationInventoryContext, GenerationTransaction, PublicationFaultPoint,
        VerifiedGeneration, current_generation_id, try_lock_generation_selection,
        verify_selected_generation,
    },
    genesis,
    logs::{LifecycleEvent, LogStreamKind, PeerLogStream},
    torii::{
        ManagedBlockStream, ManagedEventStream, OperatorSigningContext, ReadinessSmokePlan,
        ToriiClient, ToriiError, ToriiResult,
    },
    vault::{SignerVault, SignerVaultError},
};
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair, PublicKey, bls_normal_pop_prove,
};
use iroha_data_model::{
    block::BlockHeader,
    parameter::system::SumeragiConsensusMode,
    peer::PeerId,
    prelude::{AccountId, ChainId, NetworkId},
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
#[cfg(any(test, feature = "test"))]
use izanami::genesis_support::sign_prepared_genesis_from_config;
use izanami::genesis_support::{
    ManagedNodeConfig, UNRESOLVED_GENESIS_EXPECTED_HASH, validate_prepared_genesis_for_startup,
};
use norito::json::{self, Map, Value};
use once_cell::sync::OnceCell;
use rand::{TryRngCore as _, rngs::OsRng};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str::FromStr,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::runtime::Handle;
use zeroize::{Zeroize as _, Zeroizing};
mod generation_lifecycle;
mod kagami_verify_parse;
mod ownership;
mod selected_storage;
mod snapshot_label;
use kagami_verify_parse::parse_kagami_verify_output;
use ownership::SupervisorOwnershipLock;
#[cfg(test)]
use selected_storage::resolve_selected_peer_storage_paths_with_hook;
use selected_storage::validate_selected_peer_storage_paths_under_lock;
pub use selected_storage::{SelectedPeerStoragePaths, resolve_selected_peer_storage_paths};
#[cfg(test)]
use snapshot_label::SNAPSHOT_LABEL_MAX_LEN;
use snapshot_label::{SNAPSHOT_STORAGE_LAYOUT, default_snapshot_slug, sanitize_snapshot_label};
const DEFAULT_CHAIN_ID: &str = "mochi-local";
const DEFAULT_TORII_BASE_PORT: u16 = 8080;
const DEFAULT_P2P_BASE_PORT: u16 = 1337;
const GENESIS_FILE_NAME: &str = "genesis.json";
const GENESIS_SIGNED_FILE_NAME: &str = "genesis.signed.nrt";
const GENESIS_EXPECTED_HASH_FILE_NAME: &str = "genesis.expected_hash";
const GENESIS_PUBLIC_KEY_FILE_NAME: &str = "genesis.public_key";
const GENESIS_EXPECTED_HASH_PLACEHOLDER: &str = UNRESOLVED_GENESIS_EXPECTED_HASH;
const GENERATED_GENESIS_RECORD_MAX_BYTES_V1: usize = 4 * 1024;
#[cfg(any(test, feature = "test"))]
const TEST_FINALIZE_KAGAMI_STUB_SIGNATURE: &str = "MOCHI_TEST_FINALIZE_KAGAMI_STUB_SIGNATURE";
const SNAPSHOT_GENERATIONS_DIR_NAME: &str = "generations";
const SNAPSHOT_METADATA_MAX_BYTES_V1: usize = 64 * 1024;
const SMOKE_MAX_ATTEMPTS: usize = 3;
const LOCAL_MCP_PROFILE: &str = "writer";
const LOCAL_MCP_TOOL_PREFIX: &str = "iroha.";
const LOCAL_NORITO_RPC_STAGE: &str = "ga";
const LOCAL_ONBOARDING_RUNTIME_DIRECTORY: &str = "runtime";
const LOCAL_ONBOARDING_SIGNER_KEY_FILE: &str = "onboarding-signer.key";
const LOCAL_ONBOARDING_TOKEN_FILE: &str = "onboarding.token";
const LOCAL_ONBOARDING_CREDENTIAL_ID: &str = "local-dev";
const LOCAL_ONBOARDING_DATASPACE: &str = "universal";
const LOCAL_ONBOARDING_SIGNER_MAX_BYTES: usize = 1_024;
const LOCAL_ONBOARDING_TOKEN_MIN_BYTES: usize = 32;
const LOCAL_ONBOARDING_TOKEN_MAX_BYTES: usize = 256;
const LOCAL_ONBOARDING_TOKEN_FILE_MAX_BYTES: usize = LOCAL_ONBOARDING_TOKEN_MAX_BYTES + 2;
const LOCAL_MULTI_PEER_POW_TICKET_TTL_SECS: i64 = 300;
// Mochi runs every validator on one developer machine. Keep the mandatory
// SoraNet memory-hard admission proof enabled, but use the protocol's minimum
// supported Argon2 work factors so a four-peer full mesh cannot monopolize the
// host before consensus has committed genesis. Production nodes which are not
// rendered by Mochi retain Iroha's canonical 64 MiB, two-pass defaults.
const LOCAL_MULTI_PEER_POW_PUZZLE_MEMORY_KIB: i64 = 4_096;
const LOCAL_MULTI_PEER_POW_PUZZLE_TIME_COST: i64 = 1;
const LOCAL_MULTI_PEER_POW_PUZZLE_LANES: i64 = 1;
const LOCAL_MULTI_PEER_POW_DIFFICULTY: i64 = 1;
// Keep `iroha_config` out of Mochi's production dependency graph. A dev-only contract test pins
// these generator literals and the checked formula below to the shared configuration defaults.
const GENERATED_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES: usize = 2;
const GENERATED_SUMERAGI_BODY_SOURCE_BYTES: usize = 33 * 1024 * 1024;
const GENERATED_SUMERAGI_BODY_BYTES_FLOOR: usize = 231 * 1024 * 1024;
const MANAGED_RANS_TABLE_RELATIVE_PATH: &str = "codec/rans/tables/rans_seed0.toml";
const MANAGED_RANS_SEED0_TABLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../codec/rans/tables/rans_seed0.toml"
));
fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
}
fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0F) as usize] as char);
    }
    out
}
fn read_generated_genesis_record(path: &Path, label: &str) -> io::Result<String> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` is not a regular file", path.display()),
        ));
    }
    let max_bytes = u64::try_from(GENERATED_GENESIS_RECORD_MAX_BYTES_V1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "generated genesis record byte limit does not fit u64",
        )
    })?;
    if named.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` exceeds its {}-byte limit",
                path.display(),
                GENERATED_GENESIS_RECORD_MAX_BYTES_V1
            ),
        ));
    }
    let mut file = OpenOptions::new().read(true).open(path)?;
    let opened = file.metadata()?;
    if !generated_genesis_record_metadata_unchanged(&named, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` changed while it was opened", path.display()),
        ));
    }
    let read_limit = GENERATED_GENESIS_RECORD_MAX_BYTES_V1
        .checked_add(1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "generated genesis record read limit overflow",
            )
        })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(read_limit).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "generated genesis record allocation failed",
        )
    })?;
    Read::by_ref(&mut file)
        .take(u64::try_from(read_limit).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "generated genesis record read limit does not fit u64",
            )
        })?)
        .read_to_end(&mut bytes)?;
    if bytes.len() > GENERATED_GENESIS_RECORD_MAX_BYTES_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` exceeds its {}-byte limit",
                path.display(),
                GENERATED_GENESIS_RECORD_MAX_BYTES_V1
            ),
        ));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || !generated_genesis_record_metadata_unchanged(&opened, &opened_after)
        || !generated_genesis_record_metadata_unchanged(&opened_after, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` changed while it was read", path.display()),
        ));
    }
    let record = String::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` is not UTF-8", path.display()),
        )
    })?;
    let payload = record.strip_suffix('\n').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` must contain exactly one LF-terminated record",
                path.display()
            ),
        )
    })?;
    if payload.is_empty()
        || payload.as_bytes().contains(&b'\r')
        || payload.as_bytes().contains(&b'\n')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` must contain exactly one LF-terminated record",
                path.display()
            ),
        ));
    }
    Ok(record)
}
fn generated_genesis_record_metadata_unchanged(
    expected: &fs::Metadata,
    observed: &fs::Metadata,
) -> bool {
    if !expected.is_file() || !observed.is_file() || expected.len() != observed.len() {
        return false;
    }
    #[cfg(unix)]
    {
        expected.dev() == observed.dev()
            && expected.ino() == observed.ino()
            && expected.mtime() == observed.mtime()
            && expected.mtime_nsec() == observed.mtime_nsec()
            && expected.ctime() == observed.ctime()
            && expected.ctime_nsec() == observed.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        expected.modified().ok() == observed.modified().ok()
    }
}
/// Result alias for supervisor operations.
pub type Result<T> = std::result::Result<T, SupervisorError>;
/// Errors emitted while preparing or managing the supervisor.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    /// Wrapper for I/O failures.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Wrapper for TOML serialization failures.
    #[error("failed to render peer config: {0}")]
    Toml(#[from] toml::ser::Error),
    /// JSON serialization failures for genesis manifests.
    #[error("failed to serialize genesis manifest: {0}")]
    Norito(#[from] norito::json::Error),
    /// Genesis construction failures.
    #[error("failed to build genesis manifest: {0}")]
    Genesis(#[from] color_eyre::Report),
    /// External Kagami invocation failed while preparing genesis.
    #[error("failed to generate genesis manifest via `kagami`: {0}")]
    KagamiInvocation(String),
    /// Validation of a Kagami verify report failed.
    #[error("kagami verify rejected: {0}")]
    KagamiVerify(String),
    /// Failed to probe a binary version string.
    #[error("failed to probe `{binary}` version: {message}")]
    VersionProbeFailed { binary: String, message: String },
    /// Attempted to start an already running peer.
    #[error("peer `{alias}` already running")]
    PeerAlreadyRunning { alias: String },
    /// Attempted to stop a peer that is not running.
    #[error("peer `{alias}` not running")]
    PeerNotRunning { alias: String },
    /// Referenced peer alias is not part of the supervised topology.
    #[error("peer `{alias}` not found")]
    PeerUnknown { alias: String },
    /// Attempted to mutate storage for a peer that is still running.
    #[error("cannot modify storage for running peer `{alias}`")]
    PeerStillRunning { alias: String },
    /// Requested snapshot already exists.
    #[error("snapshot `{name}` already exists under `{root}`")]
    SnapshotExists { name: String, root: PathBuf },
    /// Required executable could not be located or auto-built.
    #[error("failed to locate `{binary}`: {message}")]
    BinaryUnavailable {
        binary: &'static str,
        message: String,
    },
    /// Failed to spawn the peer process.
    #[error("failed to spawn `{alias}`: {source}")]
    Spawn {
        alias: String,
        #[source]
        source: std::io::Error,
    },
    /// Failed to terminate the peer process.
    #[error("failed to terminate `{alias}`: {source}")]
    Terminate {
        alias: String,
        #[source]
        source: std::io::Error,
    },
    /// Failed to wait on a terminated peer process.
    #[error("failed to collect exit status for `{alias}`: {source}")]
    Wait {
        alias: String,
        #[source]
        source: std::io::Error,
    },
    /// Invalid configuration detected while loading supervisor artifacts.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// Another operation holds the generation selection lock or a read lease.
    #[error("Mochi generation selection lock is already held at `{path}`")]
    GenerationLocked { path: PathBuf },
    /// Another live supervisor owns this network root.
    #[error("Mochi supervisor ownership lock is already held at `{path}`")]
    SupervisorLocked { path: PathBuf },
    /// A candidate or published generation failed exact validation.
    #[error("invalid Mochi generation: {0}")]
    GenerationValidation(String),
    /// The selected generation no longer matches the base used to prepare a candidate.
    #[error(
        "Mochi generation selection changed while preparing a candidate: expected {expected:?}, found {actual:?}"
    )]
    GenerationSelectionChanged {
        /// Selection the caller used as its immutable base.
        expected: Option<String>,
        /// Selection observed under the generation publication lock.
        actual: Option<String>,
    },
    /// The atomic commit occurred but synchronizing its parent directory failed.
    #[error(
        "publication of Mochi generation `{generation_id}` committed but durability is uncertain: {source}"
    )]
    PublicationUncertain {
        generation_id: String,
        #[source]
        source: std::io::Error,
    },
    /// Post-commit reconciliation failed while publication durability was also uncertain.
    #[error(
        "post-commit reconciliation failed: {reconciliation}; committed publication durability is also uncertain: {uncertainty}"
    )]
    ReconciliationAndPublicationUncertainty {
        /// Failure encountered while reconciling the committed generation in memory.
        reconciliation: Box<SupervisorError>,
        /// Durability uncertainty reported after the generation pointer commit.
        uncertainty: Box<SupervisorError>,
    },
    /// A generation operation failed and restoring the exact prior running set also failed.
    #[error(
        "generation operation failed: {primary}; restoring the prior running-peer set also failed: {restore}"
    )]
    OperationAndRunningSetRestore {
        primary: Box<SupervisorError>,
        restore: Box<SupervisorError>,
    },
    /// One or more peers from a captured running set could not be restored.
    #[error("failed to restore prior running-peer set: {details}")]
    RunningSetRestore { details: String },
    /// One or more peers in an explicitly requested start set failed.
    #[error("failed to start requested peer set: {details}")]
    PeerSetStart { details: String },
}
fn combine_post_commit_failures(
    reconciliation: Option<SupervisorError>,
    uncertainty: Option<SupervisorError>,
) -> Option<SupervisorError> {
    match (reconciliation, uncertainty) {
        (Some(reconciliation), Some(uncertainty)) => {
            Some(SupervisorError::ReconciliationAndPublicationUncertainty {
                reconciliation: Box::new(reconciliation),
                uncertainty: Box::new(uncertainty),
            })
        }
        (Some(reconciliation), None) => Some(reconciliation),
        (None, Some(uncertainty)) => Some(uncertainty),
        (None, None) => None,
    }
}
impl From<SignerVaultError> for SupervisorError {
    fn from(err: SignerVaultError) -> Self {
        match err {
            SignerVaultError::Io(err) => Self::Io(err),
            SignerVaultError::Json(err) => Self::Norito(err),
            SignerVaultError::InvalidEntry(message) => Self::Config(message),
        }
    }
}
#[derive(Clone)]
struct OnboardingRuntimeBundle {
    authority: AccountId,
    private_key_file: PathBuf,
    token_file: PathBuf,
    token_hash: [u8; 32],
}
impl std::fmt::Debug for OnboardingRuntimeBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OnboardingRuntimeBundle")
            .field("authority", &self.authority)
            .field("private_key_file", &self.private_key_file)
            .field("token_file", &self.token_file)
            .field("token_hash", &"[REDACTED]")
            .finish()
    }
}
impl OnboardingRuntimeBundle {
    fn create(paths: &NetworkPaths, authority: &SigningAuthority) -> Result<Self> {
        let root = fs::canonicalize(paths.root())?;
        let runtime_dir = root.join(LOCAL_ONBOARDING_RUNTIME_DIRECTORY);
        prepare_owner_only_runtime_directory(&runtime_dir, &root)?;
        let private_key_file = runtime_dir.join(LOCAL_ONBOARDING_SIGNER_KEY_FILE);
        let token_file = runtime_dir.join(LOCAL_ONBOARDING_TOKEN_FILE);
        let private_key = ExposedPrivateKey(authority.key_pair().private_key().clone());
        let signer_payload = Zeroizing::new(format!("{private_key}\n"));
        #[cfg(unix)]
        let owner_uid = fs::metadata(&runtime_dir)?.uid();
        #[cfg(not(unix))]
        let owner_uid = 0;
        let existing_signer = read_owner_only_runtime_file(
            &private_key_file,
            owner_uid,
            LOCAL_ONBOARDING_SIGNER_MAX_BYTES,
            "local onboarding signer",
        )?;
        let existing_token = read_owner_only_runtime_file(
            &token_file,
            owner_uid,
            LOCAL_ONBOARDING_TOKEN_FILE_MAX_BYTES,
            "local onboarding token",
        )?;
        let token = match (existing_signer, existing_token) {
            (Some(existing_signer), Some(existing_token)) => {
                if !secret_bytes_equal(&existing_signer, signer_payload.as_bytes()) {
                    return Err(SupervisorError::Config(
                        "persisted local onboarding signer conflicts with the bundled localnet administrator"
                            .to_owned(),
                    ));
                }
                validated_local_onboarding_token(existing_token)?
            }
            (None, None) => {
                let mut token_entropy = [0_u8; 32];
                OsRng.try_fill_bytes(&mut token_entropy).map_err(|error| {
                    SupervisorError::Config(format!(
                        "failed to obtain OS entropy for the local onboarding token: {error}"
                    ))
                })?;
                let token = Zeroizing::new(
                    format!("iroha-localnet-{}", encode_hex(token_entropy.as_slice())).into_bytes(),
                );
                token_entropy.zeroize();
                write_new_owner_only_runtime_file(&private_key_file, signer_payload.as_bytes())?;
                if let Err(error) = write_new_owner_only_runtime_file(&token_file, &token) {
                    let _ = fs::remove_file(&private_key_file);
                    return Err(error);
                }
                token
            }
            _ => {
                return Err(SupervisorError::Config(
                    "local onboarding signer and token must either both exist or both be absent"
                        .to_owned(),
                ));
            }
        };
        let token_hash = *blake3::hash(&token).as_bytes();
        Ok(Self {
            authority: authority.account_id().clone(),
            private_key_file,
            token_file,
            token_hash,
        })
    }
    fn config_table(&self) -> toml::Table {
        let mut scope = toml::Table::new();
        scope.insert(
            "dataspace".to_owned(),
            toml::Value::String(LOCAL_ONBOARDING_DATASPACE.to_owned()),
        );
        let mut credential = toml::Table::new();
        credential.insert(
            "id".to_owned(),
            toml::Value::String(LOCAL_ONBOARDING_CREDENTIAL_ID.to_owned()),
        );
        credential.insert("scope".to_owned(), toml::Value::Table(scope));
        credential.insert(
            "token_hash".to_owned(),
            toml::Value::String(format!("blake3:{}", encode_hex(&self.token_hash))),
        );
        let mut table = toml::Table::new();
        table.insert(
            "authority".to_owned(),
            toml::Value::String(self.authority.to_string()),
        );
        table.insert(
            "private_key_file".to_owned(),
            toml::Value::String(self.private_key_file.display().to_string()),
        );
        table.insert("lease_term_years".to_owned(), toml::Value::Integer(1));
        table.insert(
            "additional_permissions".to_owned(),
            toml::Value::Array(Vec::new()),
        );
        table.insert(
            "credentials".to_owned(),
            toml::Value::Array(vec![toml::Value::Table(credential)]),
        );
        table
    }
}
fn localnet_admin_signer() -> Result<&'static SigningAuthority> {
    development_signing_authorities().first().ok_or_else(|| {
        SupervisorError::Config(
            "Mochi local onboarding requires the bundled localnet administrator".to_owned(),
        )
    })
}
#[cfg(unix)]
fn prepare_owner_only_runtime_directory(path: &Path, trusted_root: &Path) -> Result<()> {
    let trusted_root_metadata = fs::metadata(trusted_root)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.uid() != trusted_root_metadata.uid()
                || metadata.permissions().mode() & 0o777 != 0o700
            {
                return Err(SupervisorError::Config(format!(
                    "local onboarding runtime `{}` must be an owner-controlled non-symlink directory",
                    path.display()
                )));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            let mut permissions = fs::metadata(path)?.permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(path, permissions)?;
        }
        Err(error) => return Err(error.into()),
    }
    let canonical = fs::canonicalize(path)?;
    if canonical.parent() != Some(trusted_root) {
        return Err(SupervisorError::Config(format!(
            "local onboarding runtime escaped the managed sandbox root `{}`",
            trusted_root.display()
        )));
    }
    Ok(())
}
#[cfg(not(unix))]
fn prepare_owner_only_runtime_directory(_path: &Path, _trusted_root: &Path) -> Result<()> {
    Err(SupervisorError::Config(
        "local onboarding requires owner-only runtime directory support".to_owned(),
    ))
}
#[cfg(unix)]
fn read_owner_only_runtime_file(
    path: &Path,
    owner_uid: u32,
    max_bytes: usize,
    label: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let initial = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate_owner_only_runtime_file_metadata(&initial, owner_uid, max_bytes, label)?;
    let mut file = OpenOptions::new().read(true).open(path)?;
    let opened = file.metadata()?;
    validate_owner_only_runtime_file_metadata(&opened, owner_uid, max_bytes, label)?;
    if initial.dev() != opened.dev() || initial.ino() != opened.ino() {
        return Err(SupervisorError::Config(format!(
            "{label} changed while it was opened"
        )));
    }
    let mut payload = Zeroizing::new(Vec::with_capacity(opened.len() as usize));
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut payload)?;
    if payload.len() > max_bytes {
        return Err(SupervisorError::Config(format!(
            "{label} exceeds the reviewed size limit"
        )));
    }
    let after = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    validate_owner_only_runtime_file_metadata(&after, owner_uid, max_bytes, label)?;
    validate_owner_only_runtime_file_metadata(&current, owner_uid, max_bytes, label)?;
    if !same_runtime_file_revision(&opened, &after) || !same_runtime_file_revision(&after, &current)
    {
        return Err(SupervisorError::Config(format!(
            "{label} changed while it was read"
        )));
    }
    Ok(Some(payload))
}
#[cfg(unix)]
fn validate_owner_only_runtime_file_metadata(
    metadata: &fs::Metadata,
    owner_uid: u32,
    max_bytes: usize,
    label: &str,
) -> Result<()> {
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != owner_uid
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > max_bytes as u64
    {
        return Err(SupervisorError::Config(format!(
            "{label} must be an owner-only 0600 regular single-link file"
        )));
    }
    Ok(())
}
#[cfg(unix)]
fn same_runtime_file_revision(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
fn secret_bytes_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}
fn validated_local_onboarding_token(mut payload: Zeroizing<Vec<u8>>) -> Result<Zeroizing<Vec<u8>>> {
    let token_len = if payload.ends_with(b"\r\n") {
        payload.len().saturating_sub(2)
    } else if payload.ends_with(b"\n") {
        payload.len().saturating_sub(1)
    } else {
        payload.len()
    };
    payload.truncate(token_len);
    if !(LOCAL_ONBOARDING_TOKEN_MIN_BYTES..=LOCAL_ONBOARDING_TOKEN_MAX_BYTES)
        .contains(&payload.len())
        || !payload.iter().all(|byte| (b'!'..=b'~').contains(byte))
    {
        return Err(SupervisorError::Config(
            "local onboarding token must contain 32 through 256 printable non-whitespace ASCII bytes"
                .to_owned(),
        ));
    }
    Ok(payload)
}
#[cfg(unix)]
fn write_new_owner_only_runtime_file(path: &Path, payload: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        SupervisorError::Config("local onboarding runtime file has no parent".to_owned())
    })?;
    let owner_uid = fs::metadata(parent)?.uid();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    let mut file = options.open(path)?;
    let result = (|| -> Result<()> {
        file.write_all(payload)?;
        file.sync_all()?;
        validate_owner_only_runtime_file_metadata(
            &file.metadata()?,
            owner_uid,
            payload.len(),
            "new local onboarding runtime file",
        )?;
        Ok(())
    })();
    if result.is_err() {
        drop(file);
        let _ = fs::remove_file(path);
    }
    result
}
#[cfg(not(unix))]
fn read_owner_only_runtime_file(
    _path: &Path,
    _owner_uid: u32,
    _max_bytes: usize,
    _label: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>> {
    Err(SupervisorError::Config(
        "local onboarding requires owner-only runtime file support".to_owned(),
    ))
}
#[cfg(not(unix))]
fn write_new_owner_only_runtime_file(_path: &Path, _payload: &[u8]) -> Result<()> {
    Err(SupervisorError::Config(
        "local onboarding requires owner-only runtime file support".to_owned(),
    ))
}
/// Policy governing automatic restarts for managed peers.
#[derive(Debug, Clone, Copy)]
pub enum RestartPolicy {
    /// Never restart automatically.
    Never,
    /// Restart after failures up to a maximum number of attempts using an exponential backoff.
    OnFailure {
        /// Maximum number of restart attempts (1-based).
        max_restarts: usize,
        /// Base backoff applied to the first restart attempt.
        backoff: Duration,
    },
}
impl RestartPolicy {
    /// Determine whether another restart attempt is permitted.
    fn should_retry(self, attempt: usize) -> bool {
        if attempt == 0 {
            return false;
        }
        match self {
            RestartPolicy::Never => false,
            RestartPolicy::OnFailure { max_restarts, .. } => attempt <= max_restarts,
        }
    }
    /// Compute the backoff for the provided attempt (1-based).
    fn backoff_for(self, attempt: usize) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        match self {
            RestartPolicy::Never => Duration::ZERO,
            RestartPolicy::OnFailure { backoff, .. } => {
                if attempt <= 1 {
                    backoff
                } else {
                    let exponent = ((attempt - 1) as u32).min(4);
                    backoff.saturating_mul(1 << exponent)
                }
            }
        }
    }
}
impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy::OnFailure {
            max_restarts: 3,
            backoff: Duration::from_secs(1),
        }
    }
}
/// Describes where a binary path originated so UI layers can surface actionable hints.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BinarySource {
    /// Explicit override supplied via config/CLI/env.
    Explicit,
    /// Discovered via a specific environment variable.
    Env(&'static str),
    /// Discovered via a Cargo-set binary env var.
    CargoEnv(&'static str),
    /// Located next to the currently running executable.
    CurrentExeNeighbor,
    /// Located under the workspace target directory (profile captured for context).
    WorkspaceTarget(String),
    /// Found on the system PATH.
    PathSearch,
    /// Default unresolved placeholder.
    AutoDefault,
    /// Produced by an on-demand cargo build.
    Built,
}
impl BinarySource {
    fn label(&self) -> String {
        match self {
            Self::Explicit => "explicit override".to_owned(),
            Self::Env(var) => format!("{var} environment variable"),
            Self::CargoEnv(var) => format!("{var} cargo env"),
            Self::CurrentExeNeighbor => "bundle directory".to_owned(),
            Self::WorkspaceTarget(profile) => format!("workspace target/{profile}"),
            Self::PathSearch => "PATH".to_owned(),
            Self::AutoDefault => "default lookup".to_owned(),
            Self::Built => "cargo build".to_owned(),
        }
    }
}
/// Version and build metadata discovered for a binary.
#[derive(Debug, Clone)]
pub struct BinaryVersionInfo {
    /// Logical name used by the supervisor.
    pub name: &'static str,
    /// Resolved path to the executable.
    pub path: PathBuf,
    /// Parsed version string when available.
    pub version: Option<String>,
    /// Raw stdout/stderr captured during probing.
    pub raw_output: Option<String>,
    /// Where the executable was discovered.
    source: BinarySource,
}
impl BinaryVersionInfo {
    /// Human-readable source label for display.
    pub fn source_label(&self) -> String {
        self.source.label()
    }
}
/// Structured summary emitted by `kagami verify`.
#[derive(Debug, Clone)]
pub struct KagamiVerifyReport {
    /// Profile the verification targeted.
    pub profile: GenesisProfile,
    /// Reported chain identifier (if present).
    pub chain_id: Option<String>,
    /// VRF seed (hex) forwarded to Kagami, if any.
    pub vrf_seed_hex: Option<String>,
    /// Number of peers with PoP keys, when surfaced by Kagami.
    pub peers_with_pop: Option<u16>,
    /// Optional Kagami fingerprint or hash.
    pub fingerprint: Option<String>,
    /// Full stdout/stderr output from the verification command.
    pub raw_output: String,
}
impl KagamiVerifyReport {
    fn summary_fragment(&self) -> String {
        let mut parts = Vec::new();
        if let Some(chain_id) = &self.chain_id {
            parts.push(format!("chain {chain_id}"));
        }
        if let Some(fingerprint) = &self.fingerprint {
            parts.push(format!("fingerprint {fingerprint}"));
        }
        parts.join(" • ")
    }
}
/// Compatibility and version summary for a prepared supervisor.
#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    /// Build-line and version data for binaries.
    pub versions: Vec<BinaryVersionInfo>,
    /// Optional verification output from `kagami verify`.
    pub verify: Option<KagamiVerifyReport>,
    /// Chain identifier bound to the supervisor.
    pub chain_id: String,
    /// Genesis profile used to construct the network, if any.
    pub profile: Option<GenesisProfile>,
}
impl CompatibilityReport {
    /// One-line human readable summary suitable for the UI header.
    pub fn summary_line(&self) -> String {
        let mut fragments = Vec::new();
        if let Some(profile) = self.profile {
            fragments.push(format!("profile {}", profile.as_kagami_arg()));
        }
        if let Some(verify) = &self.verify {
            let frag = verify.summary_fragment();
            if !frag.is_empty() {
                fragments.push(frag);
            }
        }
        if fragments.is_empty() {
            format!("chain {}", self.chain_id)
        } else {
            format!("chain {} • {}", self.chain_id, fragments.join(" • "))
        }
    }
}
/// Connection details for the active Mochi sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorSessionInfo {
    /// Human-readable profile slug for the sandbox.
    pub profile_slug: String,
    /// Chain identifier currently configured for the sandbox.
    pub chain_id: String,
    /// Immutable generation selected by the sandbox commit pointer.
    pub generation_id: String,
    /// Profile-specific sandbox root containing peer state and logs.
    pub sandbox_root: PathBuf,
    /// Workspace root when Mochi can infer it from the sandbox layout.
    pub workspace_root: Option<PathBuf>,
    /// Preferred peer alias for bootstrap snippets and health checks.
    pub peer_alias: String,
    /// Explorer/API base URL for the preferred peer.
    pub api_base: String,
    /// Torii submission URL for the preferred peer.
    pub torii_url: String,
    /// Native Torii MCP endpoint for the preferred peer.
    pub mcp_url: String,
    /// Preferred local dev signer account identifier.
    pub account_id: Option<String>,
    /// Preferred local dev signer private key.
    pub private_key: Option<String>,
    /// Stable identifier for the managed local account-onboarding credential.
    pub onboarding_credential_id: String,
    /// Owner-only file containing the local account-onboarding signer.
    pub onboarding_signer_file: PathBuf,
    /// Owner-only file containing the dedicated local account-onboarding token.
    pub onboarding_token_file: PathBuf,
}
/// Paths to external binaries used by the supervisor.
#[derive(Debug, Clone)]
pub struct BinaryPaths {
    irohad: PathBuf,
    irohad_verified: bool,
    irohad_build_attempted: bool,
    irohad_auto: bool,
    irohad_source: BinarySource,
    kagami: PathBuf,
    kagami_verified: bool,
    kagami_build_attempted: bool,
    kagami_auto: bool,
    kagami_source: BinarySource,
    iroha_cli: PathBuf,
    iroha_cli_verified: bool,
    iroha_cli_build_attempted: bool,
    iroha_cli_auto: bool,
    iroha_cli_source: BinarySource,
    allow_builds: bool,
}
fn default_binary_entry(
    env_override: &'static str,
    cargo_env: &'static str,
    binary: &str,
) -> (PathBuf, bool, BinarySource) {
    let exe_name = format!("{binary}{}", env::consts::EXE_SUFFIX);
    let env_path = |key: &str| {
        env::var_os(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if let Some(path) = env_path(env_override) {
        return (path, false, BinarySource::Env(env_override));
    }
    if let Some(path) = env_path(cargo_env) {
        return (path, false, BinarySource::CargoEnv(cargo_env));
    }
    if let Ok(current) = env::current_exe()
        && let Some(dir) = current.parent()
    {
        let candidate = dir.join(&exe_name);
        if candidate.exists() {
            return (candidate, true, BinarySource::CurrentExeNeighbor);
        }
    }
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace) = manifest_dir.parent().and_then(|dir| dir.parent()) {
        for profile in ["debug", "release"] {
            let candidate = workspace.join("target").join(profile).join(&exe_name);
            if candidate.exists() {
                return (
                    candidate,
                    true,
                    BinarySource::WorkspaceTarget(profile.to_owned()),
                );
            }
        }
    }
    (PathBuf::from(binary), true, BinarySource::AutoDefault)
}
fn default_irohad_entry() -> (PathBuf, bool, BinarySource) {
    default_binary_entry("MOCHI_IROHAD", "CARGO_BIN_EXE_iroha3d", "iroha3d")
}
fn default_kagami_entry() -> (PathBuf, bool, BinarySource) {
    default_binary_entry("MOCHI_KAGAMI", "CARGO_BIN_EXE_kagami", "kagami")
}
fn default_iroha_cli_entry() -> (PathBuf, bool, BinarySource) {
    default_binary_entry("MOCHI_IROHA_CLI", "CARGO_BIN_EXE_iroha_cli", "iroha_cli")
}
fn resolve_iroha_cli_alias() -> Option<(PathBuf, BinarySource)> {
    for bin in ["iroha3", "iroha"] {
        let exe_name = format!("{bin}{}", env::consts::EXE_SUFFIX);
        if let Ok(current) = env::current_exe()
            && let Some(dir) = current.parent()
        {
            let candidate = dir.join(&exe_name);
            if is_executable_file(&candidate) {
                return Some((candidate, BinarySource::CurrentExeNeighbor));
            }
        }
        let target_root = env::var_os("CARGO_TARGET_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| workspace_root().map(|workspace| workspace.join("target")));
        if let Some(target_root) = target_root {
            for profile in ["debug", "release"] {
                let candidate = target_root.join(profile).join(&exe_name);
                if is_executable_file(&candidate) {
                    return Some((candidate, BinarySource::WorkspaceTarget(profile.to_owned())));
                }
            }
        }
        if let Some(resolved) = resolve_name_on_path(OsStr::new(bin)) {
            return Some((resolved, BinarySource::PathSearch));
        }
    }
    None
}
impl BinaryPaths {
    /// Override the path to the `iroha3d` executable.
    pub fn irohad(mut self, path: impl Into<PathBuf>) -> Self {
        self.irohad = path.into();
        self.irohad_verified = false;
        self.irohad_build_attempted = false;
        self.irohad_auto = false;
        self.irohad_source = BinarySource::Explicit;
        self
    }
    /// Override the path to the `kagami` executable.
    pub fn kagami(mut self, path: impl Into<PathBuf>) -> Self {
        self.kagami = path.into();
        self.kagami_verified = false;
        self.kagami_build_attempted = false;
        self.kagami_auto = false;
        self.kagami_source = BinarySource::Explicit;
        self
    }
    /// Override the path to the `iroha_cli` executable.
    pub fn iroha_cli(mut self, path: impl Into<PathBuf>) -> Self {
        self.iroha_cli = path.into();
        self.iroha_cli_verified = false;
        self.iroha_cli_build_attempted = false;
        self.iroha_cli_auto = false;
        self.iroha_cli_source = BinarySource::Explicit;
        self
    }
    /// Enable or disable automatic cargo builds when binaries are missing.
    pub fn allow_auto_builds(mut self, allow: bool) -> Self {
        self.allow_builds = allow;
        self
    }
    fn probe_versions(&mut self) -> Result<Vec<BinaryVersionInfo>> {
        let irohad = self.ensure_irohad_ready()?.to_path_buf();
        let irohad_source = self.irohad_source.clone();
        let kagami = self.ensure_kagami_ready()?.to_path_buf();
        let kagami_source = self.kagami_source.clone();
        let iroha_cli = self.ensure_iroha_cli_ready()?.to_path_buf();
        let iroha_cli_source = self.iroha_cli_source.clone();
        let entries = [
            ("iroha3d", irohad, irohad_source),
            ("kagami", kagami, kagami_source),
            ("iroha_cli", iroha_cli, iroha_cli_source),
        ];
        let mut versions = Vec::with_capacity(entries.len());
        for (name, path, source) in entries {
            versions.push(Self::probe_single_version(name, &path, source)?);
        }
        Ok(versions)
    }
    fn probe_single_version(
        name: &'static str,
        path: &Path,
        source: BinarySource,
    ) -> Result<BinaryVersionInfo> {
        let (raw_output, version) = probe_version_output(path, name)?;
        Ok(BinaryVersionInfo {
            name,
            path: path.to_path_buf(),
            version,
            raw_output,
            source,
        })
    }
    fn irohad_executable(&self) -> &Path {
        &self.irohad
    }
    fn ensure_irohad_ready(&mut self) -> Result<&Path> {
        if self.irohad_verified && is_executable_file(&self.irohad) {
            return Ok(&self.irohad);
        }
        if is_explicit_path(&self.irohad) && is_executable_file(&self.irohad) {
            self.irohad_verified = true;
            self.irohad_source = BinarySource::Explicit;
            return Ok(&self.irohad);
        }
        if !is_explicit_path(&self.irohad) {
            if let Some(resolved) = resolve_name_on_path(self.irohad.as_os_str()) {
                self.irohad = resolved;
                self.irohad_verified = true;
                self.irohad_source = BinarySource::PathSearch;
                return Ok(&self.irohad);
            }
        } else if let Some(name) = self.irohad.file_name()
            && let Some(resolved) = resolve_name_on_path(name)
        {
            self.irohad = resolved;
            self.irohad_verified = true;
            self.irohad_source = BinarySource::PathSearch;
            return Ok(&self.irohad);
        }
        if self.allow_builds && self.irohad_auto && !self.irohad_build_attempted {
            self.irohad_build_attempted = true;
            if let Some(workspace) = workspace_root() {
                match try_build_irohad(&workspace) {
                    Ok(path) => {
                        self.irohad = path;
                        self.irohad_verified = true;
                        self.irohad_source = BinarySource::Built;
                        return Ok(&self.irohad);
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        if is_executable_file(&self.irohad) {
            self.irohad_verified = true;
            return Ok(&self.irohad);
        }
        let message = if self.irohad_auto {
            format!(
                "looked for `{}` and searched on PATH; run `cargo build -p irohad --bin iroha3d` \
                 or set `MOCHI_IROHAD`/`binaries.irohad` to the executable",
                self.irohad.display()
            )
        } else {
            format!(
                "configured path `{}` is not executable; adjust \
                 `MOCHI_IROHAD`/`binaries.irohad` to point at a valid `iroha3d` binary",
                self.irohad.display()
            )
        };
        Err(SupervisorError::BinaryUnavailable {
            binary: "iroha3d",
            message,
        })
    }
    fn ensure_kagami_ready(&mut self) -> Result<&Path> {
        if self.kagami_verified && is_executable_file(&self.kagami) {
            return Ok(&self.kagami);
        }
        if is_explicit_path(&self.kagami) && is_executable_file(&self.kagami) {
            self.kagami_verified = true;
            self.kagami_source = BinarySource::Explicit;
            return Ok(&self.kagami);
        }
        if !is_explicit_path(&self.kagami) {
            if let Some(resolved) = resolve_name_on_path(self.kagami.as_os_str()) {
                self.kagami = resolved;
                self.kagami_verified = true;
                self.kagami_source = BinarySource::PathSearch;
                return Ok(&self.kagami);
            }
        } else if let Some(name) = self.kagami.file_name()
            && let Some(resolved) = resolve_name_on_path(name)
        {
            self.kagami = resolved;
            self.kagami_verified = true;
            self.kagami_source = BinarySource::PathSearch;
            return Ok(&self.kagami);
        }
        if self.allow_builds && self.kagami_auto && !self.kagami_build_attempted {
            self.kagami_build_attempted = true;
            if let Some(workspace) = workspace_root() {
                match try_build_kagami(&workspace) {
                    Ok(path) => {
                        self.kagami = path;
                        self.kagami_verified = true;
                        self.kagami_source = BinarySource::Built;
                        return Ok(&self.kagami);
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        if is_executable_file(&self.kagami) {
            self.kagami_verified = true;
            return Ok(&self.kagami);
        }
        let message = if self.kagami_auto {
            format!(
                "looked for `{}` and searched on PATH; run `cargo build -p iroha_kagami` \
                 or set `MOCHI_KAGAMI`/`binaries.kagami` to the executable",
                self.kagami.display()
            )
        } else {
            format!(
                "configured path `{}` is not executable; adjust \
                 `MOCHI_KAGAMI`/`binaries.kagami` to point at a valid `kagami` binary",
                self.kagami.display()
            )
        };
        Err(SupervisorError::BinaryUnavailable {
            binary: "kagami",
            message,
        })
    }
    fn iroha_cli_executable(&self) -> &Path {
        &self.iroha_cli
    }
    fn ensure_iroha_cli_ready(&mut self) -> Result<&Path> {
        if self.iroha_cli_verified && is_executable_file(&self.iroha_cli) {
            return Ok(&self.iroha_cli);
        }
        if is_explicit_path(&self.iroha_cli) && is_executable_file(&self.iroha_cli) {
            self.iroha_cli_verified = true;
            self.iroha_cli_source = BinarySource::Explicit;
            return Ok(&self.iroha_cli);
        }
        if !is_explicit_path(&self.iroha_cli) {
            if let Some(resolved) = resolve_name_on_path(self.iroha_cli.as_os_str()) {
                self.iroha_cli = resolved;
                self.iroha_cli_verified = true;
                self.iroha_cli_source = BinarySource::PathSearch;
                return Ok(&self.iroha_cli);
            }
        } else if let Some(name) = self.iroha_cli.file_name()
            && let Some(resolved) = resolve_name_on_path(name)
        {
            self.iroha_cli = resolved;
            self.iroha_cli_verified = true;
            self.iroha_cli_source = BinarySource::PathSearch;
            return Ok(&self.iroha_cli);
        }
        if let Some((resolved, source)) = resolve_iroha_cli_alias() {
            self.iroha_cli = resolved;
            self.iroha_cli_verified = true;
            self.iroha_cli_source = source;
            return Ok(&self.iroha_cli);
        }
        if self.allow_builds && self.iroha_cli_auto && !self.iroha_cli_build_attempted {
            self.iroha_cli_build_attempted = true;
            if let Some(workspace) = workspace_root() {
                match try_build_iroha_cli(&workspace) {
                    Ok(path) => {
                        self.iroha_cli = path;
                        self.iroha_cli_verified = true;
                        self.iroha_cli_source = BinarySource::Built;
                        return Ok(&self.iroha_cli);
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        if is_executable_file(&self.iroha_cli) {
            self.iroha_cli_verified = true;
            return Ok(&self.iroha_cli);
        }
        let message = if self.iroha_cli_auto {
            format!(
                "looked for `{}` and searched on PATH; run `cargo build -p iroha_cli` \
                 or set `MOCHI_IROHA_CLI`/`binaries.iroha_cli` to the executable",
                self.iroha_cli.display()
            )
        } else {
            format!(
                "configured path `{}` is not executable; adjust \
                 `MOCHI_IROHA_CLI`/`binaries.iroha_cli` to point at a valid `iroha_cli` binary",
                self.iroha_cli.display()
            )
        };
        Err(SupervisorError::BinaryUnavailable {
            binary: "iroha_cli",
            message,
        })
    }
}
impl Default for BinaryPaths {
    fn default() -> Self {
        let (irohad, irohad_auto, irohad_source) = default_irohad_entry();
        let (kagami, kagami_auto, kagami_source) = default_kagami_entry();
        let (iroha_cli, iroha_cli_auto, iroha_cli_source) = default_iroha_cli_entry();
        Self {
            irohad,
            irohad_verified: false,
            irohad_build_attempted: false,
            irohad_auto,
            irohad_source,
            kagami,
            kagami_verified: false,
            kagami_build_attempted: false,
            kagami_auto,
            kagami_source,
            iroha_cli,
            iroha_cli_verified: false,
            iroha_cli_build_attempted: false,
            iroha_cli_auto,
            iroha_cli_source,
            allow_builds: false,
        }
    }
}
fn probe_version_output(path: &Path, binary: &str) -> Result<(Option<String>, Option<String>)> {
    let output = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| SupervisorError::VersionProbeFailed {
            binary: binary.to_owned(),
            message: err.to_string(),
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let mut raw_output = None;
    if !stdout.trim().is_empty() || !stderr.trim().is_empty() {
        let mut combined = stdout.trim_end().to_owned();
        let stderr_trimmed = stderr.trim_end();
        if !stderr_trimmed.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(stderr_trimmed);
        }
        raw_output = Some(combined);
    }
    if !output.status.success() {
        let message = if let Some(raw) = &raw_output {
            format!("`{binary} --version` exited with {}: {raw}", output.status)
        } else {
            format!("`{binary} --version` exited with {}", output.status)
        };
        return Err(SupervisorError::VersionProbeFailed {
            binary: binary.to_owned(),
            message,
        });
    }
    let version = raw_output
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
        .map(|line| line.trim().to_owned());
    Ok((raw_output, version))
}
fn is_explicit_path(path: &Path) -> bool {
    path.has_root() || path.components().count() > 1
}
fn is_executable_file(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) => meta.is_file(),
        Err(_) => false,
    }
}
fn socket_addr_literal(value: &str, parameter: &str) -> Result<String> {
    value
        .parse::<iroha_primitives::addr::SocketAddr>()
        .map(|addr| addr.to_literal())
        .map_err(|err| {
            SupervisorError::Config(format!(
                "failed to render `{parameter}` as a socket address literal from `{value}`: {err}"
            ))
        })
}
fn resolve_name_on_path(name: &OsStr) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let mut candidates = Vec::with_capacity(2);
    candidates.push(OsString::from(name));
    let suffix = env::consts::EXE_SUFFIX;
    if !suffix.is_empty() {
        let has_suffix = name.to_string_lossy().ends_with(suffix);
        if !has_suffix {
            let mut with_suffix = OsString::from(name);
            with_suffix.push(suffix);
            candidates.push(with_suffix);
        }
    }
    for dir in env::split_paths(&path_var) {
        for candidate in &candidates {
            let full = dir.join(candidate);
            if is_executable_file(&full) {
                return Some(full);
            }
        }
    }
    None
}
fn workspace_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|dir| dir.parent())
        .filter(|root| root.exists())
        .map(PathBuf::from)
}
fn try_build_irohad(workspace: &Path) -> Result<PathBuf> {
    let cargo = env::var_os("CARGO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let mut command = Command::new(&cargo);
    command
        .current_dir(workspace)
        .arg("build")
        .arg("-p")
        .arg("irohad")
        .arg("--bin")
        .arg("iroha3d")
        .stdout(Stdio::null());
    // Preserve stderr so build failures surface in the parent console.
    let status = command
        .status()
        .map_err(|err| SupervisorError::BinaryUnavailable {
            binary: "iroha3d",
            message: format!("failed to invoke `{}`: {err}", cargo.display()),
        })?;
    if !status.success() {
        return Err(SupervisorError::BinaryUnavailable {
            binary: "iroha3d",
            message: format!("`cargo build -p irohad --bin iroha3d` exited with status {status}"),
        });
    }
    let target_root = env::var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let exe_name = format!("iroha3d{}", env::consts::EXE_SUFFIX);
    let candidates = [
        target_root.join("debug").join(&exe_name),
        target_root.join("release").join(&exe_name),
    ];
    for candidate in candidates {
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    Err(SupervisorError::BinaryUnavailable {
        binary: "iroha3d",
        message: format!(
            "built `iroha3d` but could not find an executable under `{}`",
            target_root.display()
        ),
    })
}
fn try_build_kagami(workspace: &Path) -> Result<PathBuf> {
    let cargo = env::var_os("CARGO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let mut command = Command::new(&cargo);
    command
        .current_dir(workspace)
        .arg("build")
        .arg("-p")
        .arg("iroha_kagami")
        .stdout(Stdio::null());
    let status = command
        .status()
        .map_err(|err| SupervisorError::BinaryUnavailable {
            binary: "kagami",
            message: format!("failed to invoke `{}`: {err}", cargo.display()),
        })?;
    if !status.success() {
        return Err(SupervisorError::BinaryUnavailable {
            binary: "kagami",
            message: format!("`cargo build -p iroha_kagami` exited with status {status}"),
        });
    }
    let target_root = env::var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let exe_name = format!("kagami{}", env::consts::EXE_SUFFIX);
    let candidates = [
        target_root.join("debug").join(&exe_name),
        target_root.join("release").join(&exe_name),
    ];
    for candidate in candidates {
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    Err(SupervisorError::BinaryUnavailable {
        binary: "kagami",
        message: format!(
            "built `kagami` but could not find an executable under `{}`",
            target_root.display()
        ),
    })
}
fn try_build_iroha_cli(workspace: &Path) -> Result<PathBuf> {
    let cargo = env::var_os("CARGO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let mut command = Command::new(&cargo);
    command
        .current_dir(workspace)
        .arg("build")
        .arg("-p")
        .arg("iroha_cli")
        .stdout(Stdio::null());
    let status = command
        .status()
        .map_err(|err| SupervisorError::BinaryUnavailable {
            binary: "iroha_cli",
            message: format!("failed to invoke `{}`: {err}", cargo.display()),
        })?;
    if !status.success() {
        return Err(SupervisorError::BinaryUnavailable {
            binary: "iroha_cli",
            message: format!("`cargo build -p iroha_cli` exited with status {status}"),
        });
    }
    let target_root = env::var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    for bin in ["iroha3", "iroha"] {
        let exe_name = format!("{bin}{}", env::consts::EXE_SUFFIX);
        for candidate in [
            target_root.join("debug").join(&exe_name),
            target_root.join("release").join(&exe_name),
        ] {
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }
    Err(SupervisorError::BinaryUnavailable {
        binary: "iroha_cli",
        message: format!(
            "built `iroha_cli` but could not find an `iroha3`/`iroha` executable under `{}`",
            target_root.display()
        ),
    })
}
/// Builds a [`Supervisor`] with user-selected presets.
#[derive(Debug)]
pub struct SupervisorBuilder {
    profile: NetworkProfile,
    data_root: PathBuf,
    chain_id: String,
    torii_base_port: u16,
    p2p_base_port: u16,
    genesis_profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<String>,
    binaries: BinaryPaths,
    restart_policy: RestartPolicy,
    auto_build_binaries: bool,
    nexus_config: Option<toml::Table>,
    sumeragi_config: Option<toml::Table>,
    torii_config: Option<toml::Table>,
    #[cfg(test)]
    publication_fault: Option<PublicationFaultPoint>,
    #[cfg(test)]
    fail_early_post_commit_reconciliation: bool,
}
/// Failure produced by an ownership-preserving supervisor replacement.
///
/// A pre-commit failure returns the stopped previous supervisor only after
/// revalidating that its generation is still selected. Once publication may
/// have committed, the previous handle is permanently retired.
#[derive(Debug)]
pub struct SupervisorReplacementFailure {
    error: SupervisorError,
    previous: Option<Supervisor>,
}
impl SupervisorReplacementFailure {
    /// Split the build error from an optional still-current previous handle.
    pub fn into_parts(self) -> (SupervisorError, Option<Supervisor>) {
        (self.error, self.previous)
    }
}
impl SupervisorBuilder {
    /// Create a builder using one of the predefined presets.
    pub fn new(preset: ProfilePreset) -> Self {
        let profile = NetworkProfile::from_preset(preset);
        let data_root = default_data_root();
        Self {
            profile,
            data_root,
            chain_id: DEFAULT_CHAIN_ID.to_owned(),
            torii_base_port: DEFAULT_TORII_BASE_PORT,
            p2p_base_port: DEFAULT_P2P_BASE_PORT,
            genesis_profile: None,
            vrf_seed_hex: None,
            binaries: BinaryPaths::default(),
            restart_policy: RestartPolicy::default(),
            auto_build_binaries: false,
            nexus_config: None,
            sumeragi_config: None,
            torii_config: None,
            #[cfg(test)]
            publication_fault: None,
            #[cfg(test)]
            fail_early_post_commit_reconciliation: false,
        }
    }
    /// Override the default profile with a custom topology.
    pub fn with_profile(profile: NetworkProfile) -> Self {
        let data_root = default_data_root();
        Self {
            profile,
            data_root,
            chain_id: DEFAULT_CHAIN_ID.to_owned(),
            torii_base_port: DEFAULT_TORII_BASE_PORT,
            p2p_base_port: DEFAULT_P2P_BASE_PORT,
            genesis_profile: None,
            vrf_seed_hex: None,
            binaries: BinaryPaths::default(),
            restart_policy: RestartPolicy::default(),
            auto_build_binaries: false,
            nexus_config: None,
            sumeragi_config: None,
            torii_config: None,
            #[cfg(test)]
            publication_fault: None,
            #[cfg(test)]
            fail_early_post_commit_reconciliation: false,
        }
    }
    /// Override the network profile while preserving existing builder settings.
    pub fn set_profile(mut self, profile: NetworkProfile) -> Self {
        self.profile = profile;
        self
    }
    /// Override the profile using a preset value.
    pub fn profile_preset(self, preset: ProfilePreset) -> Self {
        let consensus_mode = self.profile.consensus_mode;
        let mut profile = NetworkProfile::from_preset(preset);
        profile.consensus_mode = consensus_mode;
        self.set_profile(profile)
    }
    /// Provide a custom data root directory that contains configs, logs, and snapshots.
    pub fn data_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.data_root = root.into();
        self
    }
    /// Set a custom chain identifier for the generated configurations.
    pub fn chain_id(mut self, chain_id: impl Into<String>) -> Self {
        self.chain_id = chain_id.into();
        self
    }
    /// Select a Kagami genesis profile; also aligns the chain id and consensus mode for NPoS.
    pub fn genesis_profile(mut self, profile: GenesisProfile) -> Self {
        self.genesis_profile = Some(profile);
        self.profile.consensus_mode = SumeragiConsensusMode::Npos;
        let defaults = profile.defaults();
        self.chain_id = defaults.chain_id.to_owned();
        self
    }
    /// Provide an explicit VRF seed for Kagami genesis (hex, 32 bytes).
    pub fn vrf_seed_hex(mut self, seed: impl Into<String>) -> Self {
        self.vrf_seed_hex = Some(seed.into());
        self
    }
    /// Adjust the starting port for Torii bindings.
    pub fn torii_base_port(mut self, port: u16) -> Self {
        self.torii_base_port = port;
        self
    }
    /// Adjust the starting port for the P2P listener.
    pub fn p2p_base_port(mut self, port: u16) -> Self {
        self.p2p_base_port = port;
        self
    }
    /// Override the paths to external binaries.
    pub fn binaries(mut self, binaries: BinaryPaths) -> Self {
        self.binaries = binaries;
        self
    }
    /// Allow the supervisor to build missing binaries automatically.
    pub fn auto_build_binaries(mut self, allow: bool) -> Self {
        self.auto_build_binaries = allow;
        self
    }
    /// Override just the `iroha3d` binary path.
    pub fn irohad_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binaries = self.binaries.clone().irohad(path);
        self
    }
    /// Override just the `kagami` binary path.
    pub fn kagami_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binaries = self.binaries.clone().kagami(path);
        self
    }
    /// Override just the `iroha_cli` binary path.
    pub fn iroha_cli_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binaries = self.binaries.clone().iroha_cli(path);
        self
    }
    /// Configure the restart policy for managed peers.
    pub fn restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.restart_policy = policy;
        self
    }
    /// Override the generated Nexus configuration table.
    pub fn nexus_config(mut self, config: toml::Table) -> Self {
        self.nexus_config = Some(config);
        self
    }
    /// Override the configured Nexus lane count.
    pub fn nexus_lane_count(mut self, lane_count: u32) -> Self {
        set_table_u32(&mut self.nexus_config, "lane_count", lane_count);
        self
    }
    /// Override the generated Sumeragi configuration table.
    pub fn sumeragi_config(mut self, config: toml::Table) -> Self {
        self.sumeragi_config = Some(config);
        self
    }
    /// Override the generated Torii configuration table.
    pub fn torii_config(mut self, config: toml::Table) -> Self {
        self.torii_config = Some(config);
        self
    }
    /// Retrieve the profile that will be used when building the supervisor.
    pub fn profile(&self) -> &NetworkProfile {
        &self.profile
    }
    #[cfg(test)]
    fn with_post_commit_faults_for_test(
        mut self,
        publication_fault: PublicationFaultPoint,
    ) -> Self {
        self.publication_fault = Some(publication_fault);
        self.fail_early_post_commit_reconciliation = true;
        self
    }
    fn reserve_unique_port(
        allocator: &mut PortAllocator,
        reserved: &mut HashSet<u16>,
        label: &str,
    ) -> Result<u16> {
        loop {
            let port = allocator.allocate().map_err(|err| {
                SupervisorError::Config(format!("failed to allocate {label} port: {err}"))
            })?;
            if reserved.insert(port) {
                return Ok(port);
            }
        }
    }
    /// Finalize the builder and construct a newly owned supervisor instance.
    pub fn build(self) -> Result<Supervisor> {
        self.build_inner(None)
    }
    /// Consume a stopped supervisor and atomically build its replacement.
    ///
    /// Consuming the prior handle prevents callers from using two active
    /// supervisors for one network root. See [`SupervisorReplacementFailure`]
    /// for the guarded pre-commit rollback behavior.
    #[expect(clippy::result_large_err, reason = "failure returns the prior owner")]
    pub fn build_replacing(
        self,
        previous: Supervisor,
    ) -> std::result::Result<Supervisor, SupervisorReplacementFailure> {
        if previous.is_any_running() {
            return Err(SupervisorReplacementFailure {
                error: SupervisorError::Config(
                    "existing supervisor peers must be stopped before ownership transfer"
                        .to_owned(),
                ),
                previous: Some(previous),
            });
        }
        let previous_generation = previous.generation_id().to_owned();
        let previous_root = previous.paths().root().to_path_buf();
        let ownership_lock = Arc::clone(&previous._ownership_lock);
        let target_root = match resolve_data_root(&self.data_root) {
            Ok(data_root) => NetworkPaths::from_root(data_root, &self.profile),
            Err(error) => {
                return Err(SupervisorReplacementFailure {
                    error: error.into(),
                    previous: Some(previous),
                });
            }
        };
        let same_root = match ownership_lock.matches_root(target_root.root()) {
            Ok(same_root) => same_root,
            Err(error) => {
                return Err(SupervisorReplacementFailure {
                    error,
                    previous: Some(previous),
                });
            }
        };
        let build = if same_root {
            self.build_inner(Some(ownership_lock))
        } else {
            self.build_inner(None)
        };
        match build {
            Ok(replacement) => Ok(replacement),
            Err(error) => {
                if !same_root {
                    return Err(SupervisorReplacementFailure {
                        error,
                        previous: Some(previous),
                    });
                }
                let publication_uncertain = matches!(
                    &error,
                    SupervisorError::PublicationUncertain { .. }
                        | SupervisorError::ReconciliationAndPublicationUncertainty { .. }
                );
                let previous_still_selected = !publication_uncertain
                    && current_generation_id(&previous_root).is_ok_and(|selected| {
                        selected.as_deref() == Some(previous_generation.as_str())
                    });
                let previous = previous_still_selected.then_some(previous);
                Err(SupervisorReplacementFailure { error, previous })
            }
        }
    }
    fn build_inner(
        self,
        transferred_ownership: Option<Arc<SupervisorOwnershipLock>>,
    ) -> Result<Supervisor> {
        self.profile.validate().map_err(SupervisorError::Config)?;
        if self.genesis_profile.is_some()
            && self.profile.consensus_mode != SumeragiConsensusMode::Npos
        {
            return Err(SupervisorError::Config(
                "genesis_profile requires consensus_mode npos".to_owned(),
            ));
        }
        // Peer processes run from their managed peer directory so upstream
        // relative defaults cannot collide. Resolve a caller-supplied relative
        // data root before deriving any paths; otherwise the rendered paths
        // would become relative to that peer directory at startup.
        let data_root = resolve_data_root(&self.data_root)?;
        let paths = NetworkPaths::from_root(data_root, &self.profile);
        let ownership_lock = if let Some(existing) = transferred_ownership.as_ref() {
            existing.ensure_root(paths.root())?;
            Arc::clone(existing)
        } else {
            SupervisorOwnershipLock::acquire(paths.root())?
        };
        paths.ensure()?;
        let onboarding = OnboardingRuntimeBundle::create(&paths, localnet_admin_signer()?)?;
        let mut binaries = self.binaries.allow_auto_builds(self.auto_build_binaries);
        let chain_id = if let Some(profile) = self.genesis_profile {
            let defaults = profile.defaults();
            if self.chain_id != defaults.chain_id {
                return Err(SupervisorError::Config(format!(
                    "genesis profile {profile:?} requires chain id `{}`; remove the chain override",
                    defaults.chain_id
                )));
            }
            defaults.chain_id.to_owned()
        } else {
            self.chain_id.clone()
        };
        let mut nexus_config = self.nexus_config.clone();
        let mut sumeragi_config = self.sumeragi_config.clone();
        let mut torii_config = self.torii_config.clone();
        normalize_peer_config_overrides(
            &mut nexus_config,
            &mut sumeragi_config,
            &mut torii_config,
        )?;
        install_managed_account_onboarding_config(&mut torii_config, &onboarding)?;
        let peer_config_overrides = PeerConfigOverrides {
            nexus: nexus_config,
            sumeragi: sumeragi_config,
            torii: torii_config,
        };
        let nexus_topology_custom = peer_config_overrides
            .nexus
            .as_ref()
            .is_some_and(nexus_table_uses_custom_topology);
        if nexus_topology_custom && self.profile.consensus_mode != SumeragiConsensusMode::Npos {
            return Err(SupervisorError::Config(
                "custom Nexus lane topology requires an NPoS signed-genesis consensus mode"
                    .to_owned(),
            ));
        }
        if self.genesis_profile.is_some() {
            binaries.ensure_irohad_ready()?;
            binaries.ensure_kagami_ready()?;
            binaries.ensure_iroha_cli_ready()?;
        }
        let mut torii_ports = PortAllocator::new(self.torii_base_port);
        let mut p2p_ports = PortAllocator::new(self.p2p_base_port);
        let mut reserved_ports = HashSet::new();
        let expected_base_generation = current_generation_id(paths.root())?;
        let mut generation_transaction =
            GenerationTransaction::begin_replacing(paths.root(), expected_base_generation)?;
        let generation_id = generation_transaction.id().to_owned();
        let generation_root = generation_transaction.root().to_path_buf();
        let mut specs = Vec::with_capacity(self.profile.topology.peer_count);
        for index in 0..self.profile.topology.peer_count {
            let alias = format!("peer{index}");
            let torii_port =
                Self::reserve_unique_port(&mut torii_ports, &mut reserved_ports, "Torii")?;
            let p2p_port = Self::reserve_unique_port(&mut p2p_ports, &mut reserved_ports, "P2P")?;
            let storage_dir = generation_transaction.create_runtime_storage(&alias)?;
            specs.push(PeerSpec::new_in_generation(
                &generation_root,
                storage_dir,
                alias,
                torii_port,
                p2p_port,
            )?);
        }
        let genesis = GenesisMaterial::create(
            &mut binaries,
            GenesisCreateContext {
                generation_id: &generation_id,
                generation_root: &generation_root,
                chain_id: &chain_id,
                peers: &specs,
                config_overrides: &peer_config_overrides,
                consensus_mode: self.profile.consensus_mode,
                block_cadence_ms: self.profile.signed_block_cadence_ms(),
                genesis_profile: self.genesis_profile,
                vrf_seed_hex: self.vrf_seed_hex.as_deref(),
                onboarding_authority: &onboarding.authority,
            },
        )?;
        for spec in &specs {
            spec.write_config(&chain_id, &genesis, &specs, &peer_config_overrides, &[])?;
        }
        genesis.validate_generation(&chain_id, &specs)?;
        let expected_hash = genesis.expected_hash.ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "validated generation omitted its exact genesis hash".to_owned(),
            )
        })?;
        let inventory = GenerationInventoryContext {
            chain_id: &chain_id,
            chain_discriminant: genesis.chain_discriminant,
            genesis_public_key: genesis.public_key(),
            expected_hash,
        };
        #[cfg(test)]
        let mut publication = match self.publication_fault {
            Some(fault) => generation_transaction.publish_with_fault(inventory, fault),
            None => generation_transaction.publish(inventory),
        }?;
        #[cfg(not(test))]
        let mut publication = generation_transaction.publish(inventory)?;
        let reconciliation = (|| -> Result<Supervisor> {
            #[cfg(test)]
            if self.fail_early_post_commit_reconciliation {
                return Err(SupervisorError::GenerationValidation(
                    "injected early builder post-commit reconciliation failure".to_owned(),
                ));
            }
            if publication.id() != generation_id {
                return Err(SupervisorError::GenerationValidation(
                    "published generation id differs from the prepared generation".to_owned(),
                ));
            }
            if current_generation_id(paths.root())?.as_deref() != Some(generation_id.as_str()) {
                return Err(SupervisorError::GenerationValidation(
                    "current-generation does not select the committed generation".to_owned(),
                ));
            }
            let selected_root = verify_selected_generation(paths.root(), &generation_id)?;
            if selected_root.chain_id != chain_id
                || selected_root.chain_discriminant != genesis.chain_discriminant
                || selected_root.genesis_public_key != *genesis.public_key()
                || selected_root.expected_hash != expected_hash
                || !genesis.manifest_path.starts_with(&selected_root.root)
                || !specs
                    .iter()
                    .all(|spec| spec.config_path.starts_with(&selected_root.root))
            {
                return Err(SupervisorError::GenerationValidation(
                    "published accessors do not resolve inside the selected generation".to_owned(),
                ));
            }
            let peers = specs
                .into_iter()
                .map(|spec| PeerHandle::prepared(spec, paths.logs_dir(), self.restart_policy))
                .collect::<Vec<_>>();
            let vault = SignerVault::new(&paths);
            let signers = vault.load_with_fallback();
            let mut supervisor = Supervisor {
                profile: self.profile,
                paths,
                chain_id,
                genesis,
                peers,
                signers,
                onboarding,
                binaries,
                peer_config_overrides: peer_config_overrides.clone(),
                compatibility: None,
                irohad_ready: false,
                iroha_cli_ready: false,
                _ownership_lock: ownership_lock,
            };
            supervisor.run_compatibility_checks()?;
            Ok(supervisor)
        })();
        let uncertainty = publication.take_uncertainty();
        drop(publication);
        match reconciliation {
            Ok(supervisor) => match uncertainty {
                Some(error) => Err(error),
                None => Ok(supervisor),
            },
            Err(reconciliation) => Err(combine_post_commit_failures(
                Some(reconciliation),
                uncertainty,
            )
            .expect("reconciliation failure is always retained")),
        }
    }
}
fn set_table_u32(target: &mut Option<toml::Table>, key: &str, value: u32) {
    let table = target.get_or_insert_with(toml::Table::new);
    table.insert(key.to_owned(), toml::Value::Integer(i64::from(value)));
}
fn merge_table(target: &mut toml::Table, overlay: &toml::Table) {
    for (key, value) in overlay {
        target.insert(key.clone(), value.clone());
    }
}
fn generated_sumeragi_body_ingress_required_byte_capacity(
    validator_count: usize,
    authenticated_non_validator_sources: usize,
    body_source_bytes: usize,
) -> Option<usize> {
    validator_count
        .checked_add(authenticated_non_validator_sources)
        .and_then(|source_count| source_count.checked_add(1))
        .and_then(|source_count| source_count.checked_mul(body_source_bytes))
}
fn generated_sumeragi_queue_capacity(
    queues: &toml::Table,
    field: &'static str,
    default: usize,
) -> Result<usize> {
    let Some(value) = queues.get(field) else {
        return Ok(default);
    };
    let value = value.as_integer().ok_or_else(|| {
        SupervisorError::Config(format!(
            "sumeragi.queues.{field} must be a positive integer"
        ))
    })?;
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "sumeragi.queues.{field} must be a positive integer"
            ))
        })
}
fn ensure_generated_sumeragi_body_bytes(
    root: &mut toml::Table,
    validator_count: usize,
) -> Result<()> {
    let sumeragi = root
        .entry("sumeragi")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| SupervisorError::Config("sumeragi must be a table".to_owned()))?;
    let queues = sumeragi
        .entry("queues")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| SupervisorError::Config("sumeragi.queues must be a table".to_owned()))?;
    let authenticated_non_validator_sources = generated_sumeragi_queue_capacity(
        queues,
        "authenticated_non_validator_sources",
        GENERATED_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES,
    )?;
    let body_source_bytes = generated_sumeragi_queue_capacity(
        queues,
        "body_source_bytes",
        GENERATED_SUMERAGI_BODY_SOURCE_BYTES,
    )?;
    let configured_body_bytes = generated_sumeragi_queue_capacity(
        queues,
        "body_bytes",
        GENERATED_SUMERAGI_BODY_BYTES_FLOOR,
    )?;
    let required_body_bytes = generated_sumeragi_body_ingress_required_byte_capacity(
        validator_count,
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .ok_or_else(|| {
        SupervisorError::Config(format!(
            "Mochi Sumeragi body-byte capacity overflowed for {validator_count} validators, {authenticated_non_validator_sources} authenticated non-validator sources, and {body_source_bytes} bytes per source"
        ))
    })?;
    let effective_body_bytes = configured_body_bytes
        .max(required_body_bytes)
        .max(GENERATED_SUMERAGI_BODY_BYTES_FLOOR);
    if effective_body_bytes != configured_body_bytes || !queues.contains_key("body_bytes") {
        queues.insert(
            "body_bytes".into(),
            toml::Value::Integer(i64::try_from(effective_body_bytes).map_err(|_| {
                SupervisorError::Config(format!(
                    "Mochi Sumeragi body-byte capacity {effective_body_bytes} exceeds the TOML integer range"
                ))
            })?),
        );
    }
    Ok(())
}
fn restore_streaming_soranet_defaults(table: &mut toml::Table) {
    // `StreamingSoranet` is read as one nested value. Once Mochi emits that
    // table to manage its spool, it must remain structurally complete instead
    // of relying on defaults that apply only when the whole table is absent.
    table
        .entry("enabled")
        .or_insert(toml::Value::Boolean(false));
    table
        .entry("exit_multiaddr")
        .or_insert_with(|| toml::Value::String("/dns/torii/udp/9443/quic".to_owned()));
    table
        .entry("padding_budget_ms")
        .or_insert(toml::Value::Integer(25));
    table
        .entry("access_kind")
        .or_insert_with(|| toml::Value::String("authenticated".to_owned()));
    table
        .entry("channel_salt")
        .or_insert_with(|| toml::Value::String("iroha.soranet.channel.seed.v1".to_owned()));
    table
        .entry("provision_spool_max_bytes")
        .or_insert(toml::Value::Integer(0));
    table
        .entry("provision_window_segments")
        .or_insert(toml::Value::Integer(4));
    table
        .entry("provision_queue_capacity")
        .or_insert(toml::Value::Integer(256));
}
fn restore_streaming_soravpn_defaults(table: &mut toml::Table) {
    table
        .entry("provision_spool_max_bytes")
        .or_insert(toml::Value::Integer(0));
}
fn lane_aliases(nexus: Option<&toml::Table>) -> BTreeMap<u32, String> {
    let Some(nexus) = nexus else {
        return BTreeMap::new();
    };
    let mut entries = BTreeMap::<u32, String>::new();
    let lane_count = nexus
        .get("lane_count")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1);
    if let Some(values) = nexus.get("lane_catalog").and_then(toml::Value::as_array) {
        for (idx, value) in values.iter().enumerate() {
            let Some(table) = value.as_table() else {
                continue;
            };
            let index = table
                .get("index")
                .and_then(toml::Value::as_integer)
                .and_then(|raw| u32::try_from(raw).ok())
                .unwrap_or_else(|| u32::try_from(idx).unwrap_or(0));
            let alias = table
                .get("alias")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| default_lane_alias(index));
            entries.insert(index, alias);
        }
    }
    if entries.is_empty() && lane_count > 1 {
        for index in 0..lane_count {
            entries.insert(index, default_lane_alias(index));
        }
    }
    entries
}
fn lane_path_comments(storage_root: &Path, nexus: Option<&toml::Table>) -> Vec<String> {
    let entries = lane_aliases(nexus);
    if entries.is_empty() {
        return Vec::new();
    }
    let mut comments = Vec::with_capacity(entries.len() * 3);
    for (lane_id, alias) in entries {
        let slug = lane_slug(&alias, lane_id);
        let kura_segment = format!("lane_{lane_id:03}_{slug}");
        let merge_segment = format!("lane_{lane_id:03}_{slug}_merge");
        let blocks_dir = storage_root.join("blocks").join(&kura_segment);
        let merge_log = storage_root
            .join("merge_ledger")
            .join(format!("{merge_segment}.log"));
        comments.push(format!("# mochi.lane[{lane_id}].alias = {alias}"));
        comments.push(format!(
            "# mochi.lane[{lane_id}].blocks_dir = {}",
            blocks_dir.display()
        ));
        comments.push(format!(
            "# mochi.lane[{lane_id}].merge_log = {}",
            merge_log.display()
        ));
    }
    comments
}
fn default_lane_alias(index: u32) -> String {
    if index == 0 {
        "default".to_owned()
    } else {
        format!("lane{index}")
    }
}
fn lane_slug(alias: &str, lane_id: u32) -> String {
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
        format!("lane{lane_id}")
    } else {
        slug
    }
}
fn normalize_peer_config_overrides(
    nexus: &mut Option<toml::Table>,
    _sumeragi: &mut Option<toml::Table>,
    torii: &mut Option<toml::Table>,
) -> Result<()> {
    if let Some(table) = nexus.as_mut() {
        let lane_count = parse_table_u32(table, "lane_count", "nexus.lane_count")?;
        let lane_catalog = parse_table_array(table, "lane_catalog", "nexus.lane_catalog")?;
        let dataspace_catalog =
            parse_table_array(table, "dataspace_catalog", "nexus.dataspace_catalog")?;
        let lane_summary = lane_catalog
            .map(|values| LaneCatalogSummary::from_values(values))
            .transpose()?
            .unwrap_or_default();
        if let Some(catalog) = dataspace_catalog {
            ensure_table_entries(catalog, "nexus.dataspace_catalog")?;
        }
        let lane_count = if lane_count.is_some() {
            lane_count
        } else if lane_summary.len > 0 {
            let computed = lane_summary.max_index.unwrap_or(0).saturating_add(1);
            if computed == 0 {
                return Err(SupervisorError::Config(
                    "nexus.lane_catalog must contain at least one entry".to_owned(),
                ));
            }
            set_table_u32(nexus, "lane_count", computed);
            Some(computed)
        } else {
            lane_count
        };
        if let Some(count) = lane_count {
            if count == 0 {
                return Err(SupervisorError::Config(
                    "nexus.lane_count must be greater than zero".to_owned(),
                ));
            }
            if lane_summary.len > count as usize {
                return Err(SupervisorError::Config(format!(
                    "nexus.lane_count {count} is smaller than lane_catalog size {}",
                    lane_summary.len
                )));
            }
            if let Some(max_index) = lane_summary.max_index
                && max_index >= count
            {
                return Err(SupervisorError::Config(format!(
                    "nexus.lane_catalog index {max_index} exceeds lane_count {count}"
                )));
            }
        }
    }
    if let Some(table) = torii.as_ref()
        && let Some(da_ingest) = table.get("da_ingest")
        && !matches!(da_ingest, toml::Value::Table(_))
    {
        return Err(SupervisorError::Config(
            "torii.da_ingest must be a table".to_owned(),
        ));
    }
    ensure_local_mcp_config(torii)?;
    ensure_local_norito_rpc_config(torii)?;
    Ok(())
}
fn nexus_table_uses_custom_topology(table: &toml::Table) -> bool {
    table
        .get("lane_count")
        .and_then(toml::Value::as_integer)
        .is_some_and(|count| count > 1)
        || table
            .get("lane_catalog")
            .and_then(toml::Value::as_array)
            .is_some_and(|catalog| catalog.len() > 1)
        || table
            .get("dataspace_catalog")
            .and_then(toml::Value::as_array)
            .is_some_and(|catalog| catalog.len() > 1)
}
fn install_managed_account_onboarding_config(
    torii: &mut Option<toml::Table>,
    onboarding: &OnboardingRuntimeBundle,
) -> Result<()> {
    let torii = torii.get_or_insert_with(toml::Table::new);
    if torii.contains_key("account_onboarding") {
        return Err(SupervisorError::Config(
            "torii.account_onboarding is managed by Mochi's owner-only local runtime bundle"
                .to_owned(),
        ));
    }
    torii.insert(
        "account_onboarding".to_owned(),
        toml::Value::Table(onboarding.config_table()),
    );
    Ok(())
}
fn ensure_local_mcp_config(torii: &mut Option<toml::Table>) -> Result<()> {
    let table = torii.get_or_insert_with(toml::Table::new);
    let entry = table
        .entry("mcp".to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(mcp) = entry.as_table_mut() else {
        return Err(SupervisorError::Config(
            "torii.mcp must be a table".to_owned(),
        ));
    };
    mcp.entry("enabled".to_owned())
        .or_insert(toml::Value::Boolean(true));
    mcp.entry("profile".to_owned())
        .or_insert(toml::Value::String(LOCAL_MCP_PROFILE.to_owned()));
    mcp.entry("expose_operator_routes".to_owned())
        .or_insert(toml::Value::Boolean(false));
    mcp.entry("allow_tool_prefixes".to_owned())
        .or_insert_with(|| {
            toml::Value::Array(vec![toml::Value::String(LOCAL_MCP_TOOL_PREFIX.to_owned())])
        });
    if !matches!(mcp.get("allow_tool_prefixes"), Some(toml::Value::Array(_))) {
        return Err(SupervisorError::Config(
            "torii.mcp.allow_tool_prefixes must be an array".to_owned(),
        ));
    }
    Ok(())
}
fn ensure_local_norito_rpc_config(torii: &mut Option<toml::Table>) -> Result<()> {
    let table = torii.get_or_insert_with(toml::Table::new);
    let transport_entry = table
        .entry("transport".to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(transport) = transport_entry.as_table_mut() else {
        return Err(SupervisorError::Config(
            "torii.transport must be a table".to_owned(),
        ));
    };
    let norito_entry = transport
        .entry("norito_rpc".to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let Some(norito_rpc) = norito_entry.as_table_mut() else {
        return Err(SupervisorError::Config(
            "torii.transport.norito_rpc must be a table".to_owned(),
        ));
    };
    norito_rpc
        .entry("enabled".to_owned())
        .or_insert(toml::Value::Boolean(true));
    norito_rpc
        .entry("require_mtls".to_owned())
        .or_insert(toml::Value::Boolean(false));
    norito_rpc
        .entry("stage".to_owned())
        .or_insert(toml::Value::String(LOCAL_NORITO_RPC_STAGE.to_owned()));
    Ok(())
}
fn parse_table_u32(table: &toml::Table, key: &str, label: &str) -> Result<Option<u32>> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::Integer(value)) => {
            if *value < 0 {
                return Err(SupervisorError::Config(format!(
                    "{label} must be a positive integer"
                )));
            }
            u32::try_from(*value)
                .map_err(|_| SupervisorError::Config(format!("{label} exceeds the u32 range")))
                .map(Some)
        }
        Some(_) => Err(SupervisorError::Config(format!(
            "{label} must be an integer"
        ))),
    }
}
fn parse_table_array<'a>(
    table: &'a toml::Table,
    key: &str,
    label: &str,
) -> Result<Option<&'a Vec<toml::Value>>> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::Array(values)) => Ok(Some(values)),
        Some(_) => Err(SupervisorError::Config(format!(
            "{label} must be an array of tables"
        ))),
    }
}
fn ensure_table_entries(values: &[toml::Value], label: &str) -> Result<()> {
    for (idx, value) in values.iter().enumerate() {
        if !matches!(value, toml::Value::Table(_)) {
            return Err(SupervisorError::Config(format!(
                "{label}[{idx}] must be a table"
            )));
        }
    }
    Ok(())
}
#[derive(Debug, Default)]
struct LaneCatalogSummary {
    len: usize,
    max_index: Option<u32>,
}
impl LaneCatalogSummary {
    fn from_values(values: &[toml::Value]) -> Result<Self> {
        let mut summary = LaneCatalogSummary {
            len: values.len(),
            max_index: None,
        };
        for (idx, value) in values.iter().enumerate() {
            let toml::Value::Table(table) = value else {
                return Err(SupervisorError::Config(format!(
                    "nexus.lane_catalog[{idx}] must be a table"
                )));
            };
            let index = match table.get("index") {
                Some(toml::Value::Integer(raw)) => {
                    if *raw < 0 {
                        return Err(SupervisorError::Config(format!(
                            "nexus.lane_catalog[{idx}].index must be a positive integer"
                        )));
                    }
                    u32::try_from(*raw).map_err(|_| {
                        SupervisorError::Config(format!(
                            "nexus.lane_catalog[{idx}].index exceeds the u32 range"
                        ))
                    })?
                }
                Some(_) => {
                    return Err(SupervisorError::Config(format!(
                        "nexus.lane_catalog[{idx}].index must be an integer"
                    )));
                }
                None => u32::try_from(idx).map_err(|_| {
                    SupervisorError::Config(format!(
                        "nexus.lane_catalog index {idx} exceeds the u32 range"
                    ))
                })?,
            };
            summary.max_index = Some(
                summary
                    .max_index
                    .map_or(index, |current| current.max(index)),
            );
        }
        Ok(summary)
    }
}
#[derive(Clone, Default)]
struct PeerConfigOverrides {
    nexus: Option<toml::Table>,
    sumeragi: Option<toml::Table>,
    torii: Option<toml::Table>,
}
impl std::fmt::Debug for PeerConfigOverrides {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut torii = self.torii.clone();
        if let Some(torii) = torii.as_mut()
            && torii.contains_key("account_onboarding")
        {
            torii.insert(
                "account_onboarding".to_owned(),
                toml::Value::String("[REDACTED managed account onboarding]".to_owned()),
            );
        }
        formatter
            .debug_struct("PeerConfigOverrides")
            .field("nexus", &self.nexus)
            .field("sumeragi", &self.sumeragi)
            .field("torii", &torii)
            .finish()
    }
}
/// Supervises a prepared set of peers for a local network.
#[derive(Debug)]
pub struct Supervisor {
    profile: NetworkProfile,
    paths: NetworkPaths,
    chain_id: String,
    genesis: GenesisMaterial,
    peers: Vec<PeerHandle>,
    signers: Vec<SigningAuthority>,
    onboarding: OnboardingRuntimeBundle,
    binaries: BinaryPaths,
    peer_config_overrides: PeerConfigOverrides,
    compatibility: Option<CompatibilityReport>,
    irohad_ready: bool,
    iroha_cli_ready: bool,
    _ownership_lock: Arc<SupervisorOwnershipLock>,
}
impl Supervisor {
    /// Access the Nexus config overrides applied when rendering peer configs.
    pub fn nexus_config_overrides(&self) -> Option<&toml::Table> {
        self.peer_config_overrides.nexus.as_ref()
    }
    /// Access the Sumeragi config overrides applied when rendering peer configs.
    pub fn sumeragi_config_overrides(&self) -> Option<&toml::Table> {
        self.peer_config_overrides.sumeragi.as_ref()
    }
    /// Access the Torii config overrides applied when rendering peer configs.
    pub fn torii_config_overrides(&self) -> Option<&toml::Table> {
        self.peer_config_overrides.torii.as_ref()
    }
    fn ensure_irohad(&mut self) -> Result<&Path> {
        if !self.irohad_ready {
            let path = self.binaries.ensure_irohad_ready()?;
            // Store the resolved path back so subsequent calls reuse it.
            self.irohad_ready = true;
            return Ok(path);
        }
        Ok(self.binaries.irohad_executable())
    }
    fn irohad_path(&mut self) -> Result<PathBuf> {
        self.ensure_irohad().map(|path| path.to_path_buf())
    }
    fn ensure_iroha_cli(&mut self) -> Result<&Path> {
        if !self.iroha_cli_ready {
            let path = self.binaries.ensure_iroha_cli_ready()?;
            self.iroha_cli_ready = true;
            return Ok(path);
        }
        Ok(self.binaries.iroha_cli_executable())
    }
    /// Resolve the path to the configured `iroha_cli` binary, verifying it if necessary.
    pub fn iroha_cli_path(&mut self) -> Result<PathBuf> {
        self.ensure_iroha_cli().map(|path| path.to_path_buf())
    }
    fn refresh_peer_states_with(&mut self, irohad: &Path) {
        let ownership_lock = Arc::clone(&self._ownership_lock);
        for peer in &mut self.peers {
            peer.refresh_state(irohad, &ownership_lock);
        }
    }
    /// Access metadata describing the topology and consensus profile the supervisor holds.
    pub fn profile(&self) -> &NetworkProfile {
        &self.profile
    }
    /// Returns the filesystem paths used by the supervisor.
    pub fn paths(&self) -> &NetworkPaths {
        &self.paths
    }
    /// Returns the configured chain identifier.
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }
    /// Return the exact transaction replay-protection domain derived from the
    /// validated genesis block hash.
    ///
    /// # Errors
    ///
    /// Returns a generation-validation error if the supervisor has no validated
    /// genesis hash from which to derive the network identity.
    pub fn network_id(&self) -> Result<NetworkId> {
        self.genesis
            .expected_hash
            .map(NetworkId::from_genesis_hash)
            .ok_or_else(|| {
                SupervisorError::GenerationValidation(
                    "validated generation omitted its exact genesis hash".to_owned(),
                )
            })
    }
    /// Return the immutable configuration/genesis generation identifier.
    pub fn generation_id(&self) -> &str {
        &self.genesis.generation_id
    }
    fn ensure_selected_generation_metadata(&self, selected: &VerifiedGeneration) -> Result<()> {
        if selected.generation_id != self.genesis.generation_id
            || selected.chain_id != self.chain_id
            || selected.chain_discriminant != self.genesis.chain_discriminant
            || selected.genesis_public_key != *self.genesis.public_key()
            || Some(selected.expected_hash) != self.genesis.expected_hash
            || !self.genesis.manifest_path.starts_with(&selected.root)
            || !self.genesis.block_path.starts_with(&selected.root)
            || !self.genesis.public_key_path.starts_with(&selected.root)
            || !self.genesis.expected_hash_path.starts_with(&selected.root)
            || !self
                .peers
                .iter()
                .all(|peer| peer.config_path().starts_with(&selected.root))
        {
            return Err(SupervisorError::GenerationValidation(
                "selected generation metadata differs from the validated supervisor state"
                    .to_owned(),
            ));
        }
        Ok(())
    }
    fn ensure_selected_peer_storage_paths_under_lock(
        &self,
        selected: &VerifiedGeneration,
    ) -> Result<()> {
        for peer in &self.peers {
            let validated = validate_selected_peer_storage_paths_under_lock(
                self.paths.root(),
                peer.alias(),
                selected,
            )?;
            let expected_config = fs::canonicalize(peer.config_path())?;
            let expected_storage = fs::canonicalize(peer.storage_dir())?;
            let expected_snapshot = fs::canonicalize(peer.snapshot_dir())?;
            let expected_storage_generation = peer
                .storage_dir()
                .file_name()
                .and_then(OsStr::to_str)
                .ok_or_else(|| {
                    SupervisorError::GenerationValidation(format!(
                        "cached storage path for `{}` has no UTF-8 generation id",
                        peer.alias()
                    ))
                })?;
            if validated.config_generation_id != selected.generation_id
                || validated.storage_generation_id != expected_storage_generation
                || validated.config_path != expected_config
                || validated.storage_dir != expected_storage
                || validated.snapshot_dir != expected_snapshot
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "selected storage metadata for `{}` differs from the cached supervisor paths",
                    peer.alias()
                )));
            }
        }
        Ok(())
    }
    fn selected_generation_with_lease(&self) -> Result<(VerifiedGeneration, Arc<fs::File>)> {
        let observed = current_generation_id(self.paths.root())?.ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "sandbox has no current-generation selection".to_owned(),
            )
        })?;
        let selection_lease = Arc::new(try_lock_generation_selection(self.paths.root())?);
        let selected_after_lock = current_generation_id(self.paths.root())?;
        if selected_after_lock.as_deref() != Some(observed.as_str()) {
            return Err(SupervisorError::GenerationSelectionChanged {
                expected: Some(observed),
                actual: selected_after_lock,
            });
        }
        let selected = verify_selected_generation(self.paths.root(), &observed)?;
        self.ensure_selected_generation_metadata(&selected)?;
        self.ensure_selected_peer_storage_paths_under_lock(&selected)?;
        Ok((selected, selection_lease))
    }
    fn retain_ownership_lock_indefinitely(&self) {
        // A failed stop can leave an unmanaged child alive. Keep the advisory
        // ownership lock held until this process exits so no replacement can
        // start against that possibly-live runtime.
        std::mem::forget(Arc::clone(&self._ownership_lock));
    }
    /// Path to the generated genesis manifest.
    pub fn genesis_manifest(&self) -> &Path {
        &self.genesis.manifest_path
    }
    /// Path to the generated signed genesis wire file consumed by `iroha3d`.
    pub fn genesis_block_file(&self) -> &Path {
        &self.genesis.block_path
    }
    /// Optional verification report emitted by `kagami verify` when a profile is selected.
    pub fn genesis_verify_report(&self) -> Option<&KagamiVerifyReport> {
        self.genesis.verify_report.as_ref()
    }
    /// Cached compatibility summary for the configured binaries and genesis.
    pub fn compatibility(&self) -> Option<&CompatibilityReport> {
        self.compatibility.as_ref()
    }
    /// Returns the prepared peers in their current states.
    pub fn peers(&self) -> &[PeerHandle] {
        &self.peers
    }
    /// Mutable access to the managed peers (primarily for UI refresh logic).
    pub fn peers_mut(&mut self) -> &mut [PeerHandle] {
        &mut self.peers
    }
    /// Available signing authorities for local transactions.
    pub fn signers(&self) -> &[SigningAuthority] {
        &self.signers
    }
    /// Access the signer vault handle for the current supervisor.
    #[must_use]
    pub fn signer_vault(&self) -> SignerVault {
        SignerVault::new(&self.paths)
    }
    /// Reload signing authorities from disk, applying fallback keys when necessary.
    pub fn reload_signers(&mut self) {
        let vault = SignerVault::new(&self.paths);
        self.signers = vault.load_with_fallback();
    }
    /// Persist new signing authorities and refresh the in-memory cache.
    pub fn save_signers(&mut self, signers: &[SigningAuthority]) -> Result<()> {
        let vault = SignerVault::new(&self.paths);
        vault.save(signers)?;
        self.signers = vault.load_with_fallback();
        Ok(())
    }
    /// Build a readiness smoke plan using the genesis signing authority.
    pub fn readiness_smoke_plan(&self, attempts: usize) -> Result<ReadinessSmokePlan> {
        self.readiness_smoke_plan_with_offset(attempts, 0)
    }
    /// Build a readiness smoke plan using the genesis signing authority and nonce offset.
    pub fn readiness_smoke_plan_with_offset(
        &self,
        attempts: usize,
        nonce_offset: usize,
    ) -> Result<ReadinessSmokePlan> {
        let signer = self.readiness_smoke_signer()?;
        let network_id = self.network_id()?;
        ReadinessSmokePlan::for_signer_with_attempts_and_offset(
            network_id,
            &signer,
            attempts.max(1),
            nonce_offset,
        )
        .map_err(|err| SupervisorError::Config(err.to_string()))
    }
    /// Build a readiness smoke plan with the default retry budget.
    pub fn default_readiness_smoke_plan(&self) -> Result<ReadinessSmokePlan> {
        self.readiness_smoke_plan(SMOKE_MAX_ATTEMPTS)
    }
    /// Construct a Torii client for the specified peer alias.
    pub fn torii_client(&self, alias: &str) -> Option<ToriiClient> {
        let peer = self.peers.iter().find(|peer| peer.alias() == alias)?;
        peer.torii_client().ok()
    }
    /// Produce an instantaneous, generation-validated snapshot of local
    /// sandbox connection details for bootstrap files and automation.
    pub fn session_info(&self) -> Result<SupervisorSessionInfo> {
        let (_selected, _selection_lease) = self.selected_generation_with_lease()?;
        let peer = self.peers.first().ok_or_else(|| {
            SupervisorError::Config("supervisor has no prepared peers".to_owned())
        })?;
        let torii_url = peer
            .torii_client()
            .map_err(|err| {
                SupervisorError::Config(format!(
                    "failed to build a Torii client for session metadata: {err}"
                ))
            })?
            .base_url()
            .trim_end_matches('/')
            .to_owned();
        let mcp_url = peer
            .torii_client()
            .map_err(|err| {
                SupervisorError::Config(format!(
                    "failed to build a Torii client for local MCP metadata: {err}"
                ))
            })?
            .mcp_endpoint()
            .map_err(|err| {
                SupervisorError::Config(format!(
                    "failed to compute the local MCP endpoint for session metadata: {err}"
                ))
            })?
            .to_string()
            .trim_end_matches('/')
            .to_owned();
        let signer = self.signers.first();
        Ok(SupervisorSessionInfo {
            profile_slug: self.profile.slug(),
            chain_id: self.chain_id.clone(),
            generation_id: self.genesis.generation_id.clone(),
            sandbox_root: self.paths.root().to_path_buf(),
            workspace_root: infer_workspace_root_from_sandbox_root(
                self.paths
                    .root()
                    .parent()
                    .unwrap_or_else(|| self.paths.root()),
            ),
            peer_alias: peer.alias().to_owned(),
            api_base: torii_url.clone(),
            torii_url,
            mcp_url,
            account_id: signer.map(|entry| entry.account_id().to_string()),
            private_key: signer
                .map(|entry| ExposedPrivateKey(entry.key_pair().private_key().clone()).to_string()),
            onboarding_credential_id: LOCAL_ONBOARDING_CREDENTIAL_ID.to_owned(),
            onboarding_signer_file: self.onboarding.private_key_file.clone(),
            onboarding_token_file: self.onboarding.token_file.clone(),
        })
    }
    /// Access the structured log stream for the given peer alias.
    pub fn log_stream(&self, alias: &str) -> Option<PeerLogStream> {
        self.peers
            .iter()
            .find(|peer| peer.alias() == alias)
            .map(|peer| peer.log_stream())
    }
    /// Create a managed block stream handle for the specified peer using the provided runtime.
    pub fn managed_block_stream(&self, alias: &str, handle: &Handle) -> Result<ManagedBlockStream> {
        let client = self
            .torii_client(alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        Ok(ManagedBlockStream::spawn(handle, alias.to_owned(), client))
    }
    /// Create a managed event stream handle for the specified peer using the provided runtime.
    pub fn managed_event_stream(&self, alias: &str, handle: &Handle) -> Result<ManagedEventStream> {
        let client = self
            .torii_client(alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        Ok(ManagedEventStream::spawn(handle, alias.to_owned(), client))
    }
    /// Refresh peer process state by polling for exited children.
    pub fn refresh_peer_states(&mut self) {
        let Ok((_selected, _selection_lease)) = self.selected_generation_with_lease() else {
            return;
        };
        if let Ok(path) = self.irohad_path() {
            self.refresh_peer_states_with(&path);
        }
    }
    fn readiness_smoke_signer(&self) -> Result<SigningAuthority> {
        development_signing_authorities()
            .first()
            .cloned()
            .or_else(|| self.signers.first().cloned())
            .ok_or_else(|| {
                SupervisorError::Config(
                    "no signing authorities available for readiness smoke".to_owned(),
                )
            })
    }
    /// Paths to the binaries used by the supervisor.
    pub fn binaries(&self) -> &BinaryPaths {
        &self.binaries
    }
    fn run_compatibility_checks(&mut self) -> Result<()> {
        let versions = self.binaries.probe_versions()?;
        if self.genesis.profile.is_some()
            && let Some(report) = &self.genesis.verify_report
        {
            if let Some(chain_id) = &report.chain_id
                && chain_id != &self.chain_id
            {
                return Err(SupervisorError::KagamiVerify(format!(
                    "kagami verify reported chain `{chain_id}` but supervisor is configured for `{}`",
                    self.chain_id
                )));
            }
            if let Some(expected_seed) = &self.genesis.vrf_seed_hex
                && let Some(observed_seed) = &report.vrf_seed_hex
                && observed_seed != expected_seed
            {
                return Err(SupervisorError::KagamiVerify(format!(
                    "kagami verify reported vrf seed `{observed_seed}` but `{expected}` was configured",
                    expected = expected_seed
                )));
            }
        }
        self.compatibility = Some(CompatibilityReport {
            versions,
            verify: self.genesis.verify_report.clone(),
            chain_id: self.chain_id.clone(),
            profile: self.genesis.profile,
        });
        Ok(())
    }
    /// Start all peers managed by the supervisor.
    pub fn start_all(&mut self) -> Result<()> {
        let (_selected, _selection_lease) = self.selected_generation_with_lease()?;
        self.run_compatibility_checks()?;
        let irohad_path = self.irohad_path()?;
        self.refresh_peer_states_with(&irohad_path);
        let mut started = Vec::new();
        let ownership_lock = Arc::clone(&self._ownership_lock);
        for (idx, peer) in self.peers.iter_mut().enumerate() {
            match peer.start(&irohad_path, StartReason::Manual, &ownership_lock) {
                Ok(()) => started.push(idx),
                Err(err) => {
                    for index in started.into_iter().rev() {
                        let peer = &mut self.peers[index];
                        let _ = peer.stop();
                    }
                    return Err(err);
                }
            }
        }
        Ok(())
    }
    /// Stop all peers managed by the supervisor.
    pub fn stop_all(&mut self) -> Result<()> {
        let mut last_err = None;
        for peer in &mut self.peers {
            if let Err(err) = peer.stop()
                && !matches!(err, SupervisorError::PeerNotRunning { .. })
            {
                last_err = Some(err);
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }
    /// Check whether any peer process is currently running.
    pub fn is_any_running(&self) -> bool {
        self.peers.iter().any(PeerHandle::is_running)
    }
    /// Start a single peer by alias.
    pub fn start_peer(&mut self, alias: &str) -> Result<()> {
        let (_selected, _selection_lease) = self.selected_generation_with_lease()?;
        self.run_compatibility_checks()?;
        let index = self
            .peers
            .iter()
            .position(|peer| peer.alias() == alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        let irohad_path = self.irohad_path()?;
        self.refresh_peer_states_with(&irohad_path);
        let ownership_lock = Arc::clone(&self._ownership_lock);
        let peer = self
            .peers
            .get_mut(index)
            .expect("peer index should remain valid");
        peer.start(&irohad_path, StartReason::Manual, &ownership_lock)
    }
    /// Stop a single peer by alias.
    pub fn stop_peer(&mut self, alias: &str) -> Result<()> {
        let index = self
            .peers
            .iter()
            .position(|peer| peer.alias() == alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        let peer = self
            .peers
            .get_mut(index)
            .expect("peer index should remain valid");
        peer.stop()
    }
    /// Export the current network state into a timestamped snapshot directory.
    ///
    /// The snapshot contains peer storage directories, rendered configs, and the latest genesis
    /// manifest so users can quickly restore the network to its present state.
    pub fn export_snapshot(&mut self, label: Option<&str>) -> Result<PathBuf> {
        self.export_snapshot_inner(label, || {})
    }
    #[cfg(test)]
    pub(super) fn export_snapshot_with_selection_hook<F>(
        &mut self,
        label: Option<&str>,
        selection_hook: F,
    ) -> Result<PathBuf>
    where
        F: FnOnce(),
    {
        self.export_snapshot_inner(label, selection_hook)
    }
    fn export_snapshot_inner<F>(
        &mut self,
        label: Option<&str>,
        selection_hook: F,
    ) -> Result<PathBuf>
    where
        F: FnOnce(),
    {
        let (_verified, _selection_lease) = self.selected_generation_with_lease()?;
        selection_hook();
        if let Ok(path) = self.irohad_path() {
            self.refresh_peer_states_with(&path);
        }
        let previously_running = self.running_peer_aliases();
        if let Err(primary) = self.stop_captured_running_peers(&previously_running) {
            return Err(self.restore_running_set_after_error(&previously_running, primary));
        }
        let export = (|| -> Result<PathBuf> {
            let root = self.paths.snapshots_dir();
            fs::create_dir_all(&root)?;
            let snapshot_name = label
                .and_then(sanitize_snapshot_label)
                .unwrap_or_else(default_snapshot_slug);
            let destination = root.join(&snapshot_name);
            if destination.exists() {
                return Err(SupervisorError::SnapshotExists {
                    name: snapshot_name,
                    root,
                });
            }
            fs::create_dir_all(&destination)?;
            let peers_root = destination.join("peers");
            fs::create_dir_all(&peers_root)?;
            for peer in &self.peers {
                let alias = peer.alias();
                let alias_dir = peers_root.join(alias);
                fs::create_dir_all(&alias_dir)?;
                let storage_dst = alias_dir.join("storage");
                copy_dir_recursive(peer.storage_dir(), &storage_dst)?;
                let config_dst = alias_dir.join("config.toml");
                if let Some(parent) = config_dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                if peer.config_path().exists() {
                    fs::copy(peer.config_path(), &config_dst)?;
                }
                let log_dst = alias_dir.join("latest.log");
                if peer.log_path().exists() {
                    fs::copy(peer.log_path(), &log_dst)?;
                }
            }
            let genesis_dir = destination.join("genesis");
            fs::create_dir_all(&genesis_dir)?;
            if !self.genesis_manifest().exists() {
                return Err(SupervisorError::Config(format!(
                    "missing genesis manifest `{}`; cannot export snapshot",
                    self.genesis_manifest().display()
                )));
            }
            if !self.genesis_block_file().exists() {
                return Err(SupervisorError::Config(format!(
                    "missing signed genesis file `{}`; cannot export snapshot",
                    self.genesis_block_file().display()
                )));
            }
            fs::copy(self.genesis_manifest(), genesis_dir.join(GENESIS_FILE_NAME))?;
            fs::copy(
                self.genesis_block_file(),
                genesis_dir.join(GENESIS_SIGNED_FILE_NAME),
            )?;
            let (genesis_hash, _) = hash_snapshot_file_bounded(
                self.genesis_manifest(),
                u64::try_from(iroha_genesis::GENESIS_MANIFEST_JSON_MAX_BYTES_V1)
                    .expect("genesis manifest byte bound fits u64"),
            )?;
            let mut kura_hashes = Map::new();
            for peer in &self.peers {
                let hash = hash_directory(peer.storage_dir())?;
                kura_hashes.insert(peer.alias().to_owned(), Value::String(hash.to_string()));
            }
            let mut metadata = Map::new();
            metadata.insert("chain_id".to_owned(), Value::String(self.chain_id.clone()));
            metadata.insert(
                "generation_id".to_owned(),
                Value::String(self.genesis.generation_id.clone()),
            );
            metadata.insert(
                "created_at_ms".to_owned(),
                Value::Number((timestamp_ms() as u64).into()),
            );
            metadata.insert(
                "peer_count".to_owned(),
                Value::Number((self.peers.len() as u64).into()),
            );
            metadata.insert("snapshot".to_owned(), Value::String(snapshot_name));
            metadata.insert(
                "storage_layout".to_owned(),
                Value::String(SNAPSHOT_STORAGE_LAYOUT.to_owned()),
            );
            metadata.insert(
                "genesis_hash".to_owned(),
                Value::String(genesis_hash.to_string()),
            );
            metadata.insert("kura_hashes".to_owned(), Value::Object(kura_hashes));
            let metadata =
                json::to_json_bounded(&Value::Object(metadata), SNAPSHOT_METADATA_MAX_BYTES_V1)
                    .map_err(|error| {
                        SupervisorError::Config(format!(
                            "snapshot metadata exceeds its first-release byte budget: {error}"
                        ))
                    })?;
            fs::write(destination.join("metadata.json"), metadata.as_bytes())?;
            Ok(destination)
        })();
        match export {
            Ok(destination) => {
                self.restore_captured_running_peers(&previously_running)?;
                Ok(destination)
            }
            Err(primary) => Err(self.restore_running_set_after_error(&previously_running, primary)),
        }
    }
    /// Restore a previously exported snapshot's mutable peer storage and logs.
    ///
    /// Snapshots are bound to one immutable generation. Configuration and
    /// genesis artifacts are verified but never overwritten during restore.
    pub fn restore_snapshot<P: AsRef<Path>>(&mut self, snapshot: P) -> Result<PathBuf> {
        self.restore_snapshot_inner(snapshot, || {})
    }
    #[cfg(test)]
    pub(super) fn restore_snapshot_with_selection_hook<P, F>(
        &mut self,
        snapshot: P,
        selection_hook: F,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
        F: FnOnce(),
    {
        self.restore_snapshot_inner(snapshot, selection_hook)
    }
    fn restore_snapshot_inner<P, F>(&mut self, snapshot: P, selection_hook: F) -> Result<PathBuf>
    where
        P: AsRef<Path>,
        F: FnOnce(),
    {
        let candidate = snapshot.as_ref();
        let snapshot_root = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            let under_root = self.paths.snapshots_dir().join(candidate);
            if under_root.exists() {
                under_root
            } else {
                candidate.to_path_buf()
            }
        };
        if !snapshot_root.exists() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` does not exist",
                snapshot_root.display()
            )));
        }
        if !snapshot_root.is_dir() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` is not a directory",
                snapshot_root.display()
            )));
        }
        let (verified_generation, _selection_lease) = self.selected_generation_with_lease()?;
        selection_hook();
        let metadata = load_snapshot_metadata(&snapshot_root)?;
        if metadata.chain_id != self.chain_id {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` targets chain `{}` but the supervisor is configured for `{}`",
                snapshot_root.display(),
                metadata.chain_id,
                self.chain_id
            )));
        }
        let selected_generation = verified_generation.generation_id.clone();
        if metadata.generation_id != self.genesis.generation_id
            || metadata.generation_id != selected_generation
        {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` targets generation `{}` but current generation is `{selected_generation}`; refusing to overwrite immutable configuration",
                snapshot_root.display(),
                metadata.generation_id
            )));
        }
        let expected_peers = self.peers.len() as u64;
        if metadata.peer_count != expected_peers {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` recorded {recorded} peer(s) but the supervisor manages {expected_peers}",
                snapshot_root.display(),
                recorded = metadata.peer_count
            )));
        }
        let peers_root = snapshot_root.join("peers");
        if !peers_root.is_dir() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` missing `peers/` directory",
                snapshot_root.display()
            )));
        }
        let genesis_src = snapshot_root.join("genesis").join(GENESIS_FILE_NAME);
        let genesis_block_src = snapshot_root.join("genesis").join(GENESIS_SIGNED_FILE_NAME);
        if !genesis_src.exists() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` missing `genesis/{GENESIS_FILE_NAME}`",
                snapshot_root.display()
            )));
        }
        if !genesis_block_src.exists() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` missing `genesis/{GENESIS_SIGNED_FILE_NAME}`",
                snapshot_root.display()
            )));
        }
        if !self.genesis_manifest().exists() {
            return Err(SupervisorError::Config(format!(
                "supervisor is missing genesis manifest `{}`",
                self.genesis_manifest().display()
            )));
        }
        if !self.genesis_block_file().exists() {
            return Err(SupervisorError::Config(format!(
                "supervisor is missing signed genesis file `{}`",
                self.genesis_block_file().display()
            )));
        }
        verify_snapshot_artifact_matches_selected(
            &snapshot_root,
            &genesis_block_src,
            &verified_generation
                .root
                .join("genesis")
                .join(GENESIS_SIGNED_FILE_NAME),
            "signed genesis",
        )?;
        let manifest_max_bytes = u64::try_from(iroha_genesis::GENESIS_MANIFEST_JSON_MAX_BYTES_V1)
            .expect("genesis manifest byte bound fits u64");
        let (snapshot_genesis_hash, _) =
            hash_snapshot_file_bounded(&genesis_src, manifest_max_bytes)?;
        if snapshot_genesis_hash != metadata.genesis_hash {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` genesis hash {} does not match recorded metadata {}; refusing restore",
                snapshot_root.display(),
                snapshot_genesis_hash,
                metadata.genesis_hash
            )));
        }
        let (current_genesis_hash, _) =
            hash_snapshot_file_bounded(self.genesis_manifest(), manifest_max_bytes)?;
        if snapshot_genesis_hash != current_genesis_hash {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` genesis hash {} does not match current genesis hash {}; refusing restore",
                snapshot_root.display(),
                snapshot_genesis_hash,
                current_genesis_hash
            )));
        }
        for peer in &self.peers {
            let alias_dir = peers_root.join(peer.alias());
            if !alias_dir.is_dir() {
                return Err(SupervisorError::Config(format!(
                    "snapshot `{}` missing directory for peer `{}`",
                    snapshot_root.display(),
                    peer.alias()
                )));
            }
            verify_snapshot_artifact_matches_selected(
                &snapshot_root,
                &alias_dir.join("config.toml"),
                &verified_generation
                    .root
                    .join("peers")
                    .join(peer.alias())
                    .join("config.toml"),
                &format!("config for peer `{}`", peer.alias()),
            )?;
            let expected_hash = metadata.kura_hashes.get(peer.alias()).ok_or_else(|| {
                SupervisorError::Config(format!(
                    "snapshot `{}` metadata missing kura hash for peer `{}`",
                    snapshot_root.display(),
                    peer.alias()
                ))
            })?;
            let storage_src = alias_dir.join("storage");
            let actual_hash = hash_directory(&storage_src)?;
            if &actual_hash != expected_hash {
                return Err(SupervisorError::Config(format!(
                    "snapshot `{}` storage for peer `{}` failed integrity check: expected {} but found {}",
                    snapshot_root.display(),
                    peer.alias(),
                    expected_hash,
                    actual_hash
                )));
            }
        }
        if let Ok(path) = self.irohad_path() {
            self.refresh_peer_states_with(&path);
        }
        let previously_running = self.running_peer_aliases();
        if let Err(primary) = self.stop_captured_running_peers(&previously_running) {
            return Err(self.restore_running_set_after_error(&previously_running, primary));
        }
        let restore = (|| -> Result<()> {
            for peer in &mut self.peers {
                let alias_dir = peers_root.join(peer.alias());
                peer.wipe_storage()?;
                copy_dir_recursive(&alias_dir.join("storage"), peer.storage_dir())?;
                copy_file_if_exists(&alias_dir.join("latest.log"), peer.log_path())?;
            }
            Ok(())
        })();
        match restore {
            Ok(()) => {
                self.restore_captured_running_peers(&previously_running)?;
                Ok(snapshot_root)
            }
            Err(primary) => Err(self.restore_running_set_after_error(&previously_running, primary)),
        }
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        if self.stop_all().is_err() {
            self.retain_ownership_lock_indefinitely();
        }
    }
}
/// Lightweight metadata and state for a child `iroha3d` process.
pub struct PeerHandle {
    spec: PeerSpec,
    log_path: PathBuf,
    process: Option<Child>,
    state: PeerState,
    log_stream: PeerLogStream,
    torii_client: OnceCell<ToriiClient>,
    log_threads: Vec<JoinHandle<()>>,
    log_file: Option<Arc<Mutex<fs::File>>>,
    restart_policy: RestartPolicy,
    restart_attempts: usize,
    next_restart_at: Option<Instant>,
    manual_stop: bool,
}
#[derive(Debug, Clone, Copy)]
enum StartReason {
    Manual,
    Restart { attempt: usize },
}
impl StartReason {
    fn attempt(self) -> usize {
        match self {
            StartReason::Manual => 0,
            StartReason::Restart { attempt } => attempt,
        }
    }
}
impl std::fmt::Debug for PeerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerHandle")
            .field("alias", &self.spec.alias)
            .field("config_path", &self.spec.config_path)
            .field("state", &self.state)
            .field("log_path", &self.log_path)
            .finish()
    }
}
impl PeerHandle {
    fn prepared(spec: PeerSpec, logs_dir: PathBuf, restart_policy: RestartPolicy) -> Self {
        let alias = spec.alias.clone();
        let log_path = logs_dir.join(format!("{alias}.log"));
        Self {
            spec,
            log_path,
            process: None,
            state: PeerState::Prepared,
            log_stream: PeerLogStream::new(alias),
            torii_client: OnceCell::new(),
            log_threads: Vec::new(),
            log_file: None,
            restart_policy,
            restart_attempts: 0,
            next_restart_at: None,
            manual_stop: false,
        }
    }
    fn is_running(&self) -> bool {
        matches!(self.state, PeerState::Running | PeerState::Restarting)
    }
    /// Mutable storage root selected for this peer's current generation.
    pub fn storage_dir(&self) -> &Path {
        &self.spec.storage_dir
    }
    /// Snapshot root nested under the peer's selected mutable storage.
    pub fn snapshot_dir(&self) -> &Path {
        &self.spec.snapshot_dir
    }
    pub(crate) fn kura_store_dir(&self) -> &Path {
        &self.spec.kura_dir
    }
    fn wipe_storage(&self) -> Result<()> {
        if self.is_running() {
            return Err(SupervisorError::PeerStillRunning {
                alias: self.spec.alias.clone(),
            });
        }
        if self.storage_dir().exists() {
            fs::remove_dir_all(self.storage_dir())?;
        }
        fs::create_dir_all(self.storage_dir())?;
        fs::create_dir_all(self.snapshot_dir().join(SNAPSHOT_GENERATIONS_DIR_NAME))?;
        Ok(())
    }
    fn replace_spec(&mut self, spec: PeerSpec) {
        self.spec = spec;
    }
    /// Stable identifier for the peer (e.g. `peer0`).
    pub fn alias(&self) -> &str {
        &self.spec.alias
    }
    /// Peer identifier derived from the generated key pair.
    pub fn peer_id(&self) -> PeerId {
        self.spec.peer_id()
    }
    /// Public Torii address advertised to clients.
    pub fn torii_address(&self) -> &str {
        &self.spec.torii_public
    }
    /// Public P2P address advertised to peers.
    pub fn p2p_address(&self) -> &str {
        &self.spec.p2p_public
    }
    /// Location of the rendered configuration file.
    pub fn config_path(&self) -> &Path {
        &self.spec.config_path
    }
    /// Path to the log file capturing stdout/stderr.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
    /// Structured log stream associated with the peer.
    pub fn log_stream(&self) -> PeerLogStream {
        self.log_stream.clone()
    }
    /// Current lifecycle state for the peer.
    pub fn state(&self) -> PeerState {
        self.state
    }
    /// Generate a Torii client targeting this peer.
    pub fn torii_client(&self) -> ToriiResult<ToriiClient> {
        if let Some(client) = self.torii_client.get() {
            return Ok(client.clone());
        }
        let config = ManagedNodeConfig::from_path(&self.spec.config_path).map_err(|error| {
            ToriiError::SignedQueryContext(format!(
                "failed to load exact network identity from managed peer config `{}`: {error}",
                self.spec.config_path.display()
            ))
        })?;
        let network_id = NetworkId::from_genesis_hash(config.genesis_expected_hash);
        let operator_signing_context = self.spec.operator_signing_context(network_id)?;
        let client = ToriiClient::builder(self.spec.torii_base_http())?
            .with_network_id(network_id)
            .with_operator_signing_context(operator_signing_context)
            .build()?;
        if self.torii_client.set(client.clone()).is_ok() {
            Ok(client)
        } else {
            // Cell already initialised by another thread; fall back to the stored value.
            Ok(self.torii_client.get().cloned().unwrap_or(client))
        }
    }
    fn start(
        &mut self,
        irohad: &Path,
        reason: StartReason,
        ownership_lock: &SupervisorOwnershipLock,
    ) -> Result<()> {
        match self.state {
            PeerState::Running | PeerState::Restarting => {
                return Err(SupervisorError::PeerAlreadyRunning {
                    alias: self.spec.alias.clone(),
                });
            }
            PeerState::Prepared | PeerState::Stopped => {}
        }
        self.manual_stop = false;
        if matches!(reason, StartReason::Manual) {
            self.restart_attempts = 0;
            self.next_restart_at = None;
        }
        self.teardown_log_threads();
        fs::create_dir_all(self.log_path.parent().unwrap_or_else(|| Path::new(".")))?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        let log_file = Arc::new(Mutex::new(file));
        self.log_file = Some(log_file.clone());
        let mut command = Command::new(irohad);
        let config_path = self.spec.config_path.canonicalize()?;
        let peer_dir = config_path.parent().ok_or_else(|| {
            SupervisorError::Config(format!(
                "managed peer config `{}` has no parent directory",
                config_path.display()
            ))
        })?;
        command
            // A managed peer must never inherit Mochi's launcher directory.
            // Keeping relative upstream defaults inside the peer directory is
            // the final containment boundary for state paths not yet rendered
            // explicitly by Mochi.
            .current_dir(peer_dir)
            .arg("--config")
            .arg(config_path)
            // Mochi reserves managed-peer stdin for an inherited duplicate of
            // the ownership descriptor. An orphaned `iroha3d` therefore keeps
            // the network root fenced until that peer exits.
            .stdin(ownership_lock.child_stdin()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|err| {
            let source = match err.kind() {
                io::ErrorKind::NotFound => {
                    let message = format!(
                        "{} (looked for `{}`); build `iroha3d` with \
                         `cargo build -p irohad --bin iroha3d` or set `MOCHI_IROHAD`/`binaries.irohad` \
                         to an absolute path",
                        err,
                        irohad.display()
                    );
                    io::Error::new(io::ErrorKind::NotFound, message)
                }
                _ => err,
            };
            SupervisorError::Spawn {
                alias: self.spec.alias.clone(),
                source,
            }
        })?;
        let stdout = child.stdout.take().ok_or_else(|| SupervisorError::Spawn {
            alias: self.spec.alias.clone(),
            source: io::Error::other("failed to capture stdout"),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| SupervisorError::Spawn {
            alias: self.spec.alias.clone(),
            source: io::Error::other("failed to capture stderr"),
        })?;
        let stdout_stream = self.log_stream.clone();
        let stderr_stream = self.log_stream.clone();
        let stdout_handle = spawn_log_forwarder(
            LogStreamKind::Stdout,
            stdout,
            log_file.clone(),
            stdout_stream,
        );
        let stderr_handle = spawn_log_forwarder(
            LogStreamKind::Stderr,
            stderr,
            log_file.clone(),
            stderr_stream,
        );
        self.log_threads = vec![stdout_handle, stderr_handle];
        self.process = Some(child);
        self.state = PeerState::Running;
        let attempt = reason.attempt();
        self.emit_lifecycle_event(LifecycleEvent::Started { attempt });
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        match (&mut self.process, self.state) {
            (Some(child), PeerState::Running) => {
                self.manual_stop = true;
                if let Err(err) = child.kill()
                    && err.kind() != std::io::ErrorKind::InvalidInput
                {
                    return Err(SupervisorError::Terminate {
                        alias: self.spec.alias.clone(),
                        source: err,
                    });
                }
                match child.wait() {
                    Ok(_) => {}
                    Err(err) => {
                        return Err(SupervisorError::Wait {
                            alias: self.spec.alias.clone(),
                            source: err,
                        });
                    }
                }
                self.process = None;
                self.teardown_log_threads();
                self.state = PeerState::Stopped;
                self.restart_attempts = 0;
                self.next_restart_at = None;
                self.emit_lifecycle_event(LifecycleEvent::StoppedByUser);
                self.manual_stop = false;
                Ok(())
            }
            (None, PeerState::Restarting) => {
                // Cancel a pending restart requested by the supervisor.
                self.restart_attempts = 0;
                self.next_restart_at = None;
                self.state = PeerState::Stopped;
                self.manual_stop = false;
                self.emit_lifecycle_event(LifecycleEvent::StoppedByUser);
                Ok(())
            }
            (None, PeerState::Running | PeerState::Prepared | PeerState::Stopped) => {
                Err(SupervisorError::PeerNotRunning {
                    alias: self.spec.alias.clone(),
                })
            }
            (Some(_), _) => {
                self.process = None;
                self.teardown_log_threads();
                self.state = PeerState::Stopped;
                Ok(())
            }
        }
    }
    fn refresh_state(&mut self, irohad: &Path, ownership_lock: &SupervisorOwnershipLock) {
        if let (PeerState::Running, Some(child)) = (self.state, self.process.as_mut())
            && let Ok(Some(status)) = child.try_wait()
        {
            self.process = None;
            self.teardown_log_threads();
            let code = status.code();
            let success = status.success();
            self.emit_lifecycle_event(LifecycleEvent::Exited { code, success });
            self.state = PeerState::Stopped;
            if self.manual_stop {
                self.manual_stop = false;
                self.restart_attempts = 0;
                self.next_restart_at = None;
                return;
            }
            if success {
                self.restart_attempts = 0;
                self.next_restart_at = None;
            } else {
                self.schedule_restart(irohad, ownership_lock);
            }
        }
        if matches!(self.state, PeerState::Restarting | PeerState::Stopped)
            && let Some(instant) = self.next_restart_at
            && Instant::now() >= instant
        {
            let attempt = self.restart_attempts;
            match self.start(irohad, StartReason::Restart { attempt }, ownership_lock) {
                Ok(()) => {
                    self.state = PeerState::Running;
                    self.next_restart_at = None;
                    self.emit_lifecycle_event(LifecycleEvent::RestartSucceeded { attempt });
                }
                Err(err) => {
                    self.emit_lifecycle_event(LifecycleEvent::RestartFailed {
                        attempt,
                        error: err.to_string(),
                    });
                    self.schedule_restart(irohad, ownership_lock);
                }
            }
        }
    }
    fn schedule_restart(&mut self, irohad: &Path, ownership_lock: &SupervisorOwnershipLock) {
        let next_attempt = self.restart_attempts + 1;
        if !self.restart_policy.should_retry(next_attempt) {
            self.emit_lifecycle_event(LifecycleEvent::RestartAborted {
                attempt: next_attempt,
            });
            self.restart_attempts = 0;
            self.next_restart_at = None;
            self.state = PeerState::Stopped;
            return;
        }
        let delay = self.restart_policy.backoff_for(next_attempt);
        self.set_restart_timer(next_attempt, delay);
        if delay.is_zero() {
            let attempt = self.restart_attempts;
            match self.start(irohad, StartReason::Restart { attempt }, ownership_lock) {
                Ok(()) => {
                    self.state = PeerState::Running;
                    self.next_restart_at = None;
                    self.emit_lifecycle_event(LifecycleEvent::RestartSucceeded { attempt });
                }
                Err(err) => {
                    self.emit_lifecycle_event(LifecycleEvent::RestartFailed {
                        attempt,
                        error: err.to_string(),
                    });
                    let upcoming_attempt = attempt + 1;
                    if self.restart_policy.should_retry(upcoming_attempt) {
                        let next_delay = self.restart_policy.backoff_for(upcoming_attempt);
                        self.set_restart_timer(upcoming_attempt, next_delay);
                    } else {
                        self.emit_lifecycle_event(LifecycleEvent::RestartAborted {
                            attempt: upcoming_attempt,
                        });
                        self.restart_attempts = 0;
                        self.next_restart_at = None;
                        self.state = PeerState::Stopped;
                    }
                }
            }
        }
    }
    fn teardown_log_threads(&mut self) {
        for handle in self.log_threads.drain(..) {
            let _ = handle.join();
        }
        self.log_file = None;
    }
    fn set_restart_timer(&mut self, attempt: usize, delay: Duration) {
        self.restart_attempts = attempt;
        self.state = PeerState::Restarting;
        self.next_restart_at = Some(Instant::now() + delay);
        self.emit_lifecycle_event(LifecycleEvent::RestartScheduled {
            attempt,
            delay_ms: delay.as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }
    fn emit_lifecycle_event(&self, event: LifecycleEvent) {
        if let Some(file) = &self.log_file {
            let message = format_lifecycle(&event);
            write_log_record(file, LogStreamKind::System, &message);
        }
        self.log_stream.emit_lifecycle(event);
    }
}
/// Lifecycle state for a peer managed by the supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Files and configuration exist, process has not been launched.
    Prepared,
    /// Process is running.
    Running,
    /// Process is waiting for an automatic restart attempt.
    Restarting,
    /// Process was stopped or exited.
    Stopped,
}
impl PeerState {
    /// Human-readable label for UI presentation.
    pub fn label(self) -> &'static str {
        match self {
            PeerState::Prepared => "Prepared",
            PeerState::Running => "Running",
            PeerState::Restarting => "Restarting",
            PeerState::Stopped => "Stopped",
        }
    }
}
fn write_log_record(file: &Arc<Mutex<fs::File>>, kind: LogStreamKind, message: &str) {
    if let Ok(mut guard) = file.lock() {
        let sanitized = sanitize_message(message);
        let _ = writeln!(guard, "{}|{}|{}", timestamp_ms(), kind, sanitized);
    }
}
fn sanitize_message(message: &str) -> String {
    message.replace(['\n', '\r'], " ")
}
fn format_lifecycle(event: &LifecycleEvent) -> String {
    match event {
        LifecycleEvent::Started { attempt } if *attempt == 0 => "started".to_owned(),
        LifecycleEvent::Started { attempt } => format!("started (restart attempt={attempt})"),
        LifecycleEvent::Exited { code, success } => match code {
            Some(code) => format!("exited with code {code} (success={success})"),
            None => format!("exited without code (success={success})"),
        },
        LifecycleEvent::RestartScheduled { attempt, delay_ms } => {
            format!("restart scheduled (attempt={attempt}, delay_ms={delay_ms})")
        }
        LifecycleEvent::RestartSucceeded { attempt } => {
            format!("restart succeeded (attempt={attempt})")
        }
        LifecycleEvent::RestartFailed { attempt, error } => {
            format!("restart failed (attempt={attempt}): {error}")
        }
        LifecycleEvent::RestartAborted { attempt } => {
            format!("restart aborted after attempt={attempt}")
        }
        LifecycleEvent::StoppedByUser => "stopped by user".to_owned(),
    }
}
fn spawn_log_forwarder<R>(
    kind: LogStreamKind,
    reader: R,
    file: Arc<Mutex<fs::File>>,
    stream: PeerLogStream,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();
        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let line = buffer.trim_end_matches(&['\r', '\n'][..]).to_owned();
                    write_log_record(&file, kind, &line);
                    stream.emit_line(kind, line);
                }
                Err(err) => {
                    let message = format!("log forwarder error: {err}");
                    write_log_record(&file, LogStreamKind::System, &message);
                    stream.emit_line(LogStreamKind::System, message);
                    break;
                }
            }
        }
    })
}
fn stage_managed_rans_tables(peer_dir: &Path) -> Result<PathBuf> {
    let peer_dir = peer_dir.canonicalize()?;
    let destination = peer_dir.join(MANAGED_RANS_TABLE_RELATIVE_PATH);
    let parent = destination.parent().ok_or_else(|| {
        SupervisorError::Config("managed rANS table path has no parent directory".to_owned())
    })?;
    fs::create_dir_all(parent)?;
    fs::write(&destination, MANAGED_RANS_SEED0_TABLE)?;
    let destination = destination.canonicalize()?;
    if !destination.starts_with(&peer_dir) {
        return Err(SupervisorError::Config(format!(
            "managed rANS table escaped peer directory `{}`",
            peer_dir.display()
        )));
    }
    Ok(destination)
}
#[derive(Debug, Clone)]
struct PeerSpec {
    alias: String,
    torii_bind: String,
    torii_public: String,
    p2p_bind: String,
    p2p_public: String,
    config_path: PathBuf,
    storage_dir: PathBuf,
    kura_dir: PathBuf,
    snapshot_dir: PathBuf,
    rans_tables_path: PathBuf,
    keys: PeerKeys,
}
impl PeerSpec {
    fn new_in_generation(
        generation_root: &Path,
        storage_dir: PathBuf,
        alias: String,
        torii_port: u16,
        p2p_port: u16,
    ) -> Result<Self> {
        let generation_peer_dir = generation_root.join("peers").join(&alias);
        fs::create_dir_all(&generation_peer_dir)?;
        Self::initialize_storage(&storage_dir)?;
        // Kura authenticates a pristine store root before establishing its
        // configured-catalog baseline. Keep its root separate from the other
        // per-peer runtime directories and let Kura create it on first start.
        let kura_dir = storage_dir.join("kura");
        let snapshot_dir = storage_dir.join("snapshot");
        let config_path = generation_peer_dir.join("config.toml");
        let rans_tables_path = stage_managed_rans_tables(&generation_peer_dir)?;
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (public_key, private_key) = key_pair.into_parts();
        let pop = bls_normal_pop_prove(&private_key)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        let identity_key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (identity_public_key, identity_private_key) = identity_key_pair.into_parts();
        let soranet_transport_key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (soranet_transport_public_key, soranet_transport_private_key) =
            soranet_transport_key_pair.into_parts();
        Ok(Self {
            alias,
            torii_bind: format!("0.0.0.0:{torii_port}"),
            torii_public: format!("127.0.0.1:{torii_port}"),
            p2p_bind: format!("0.0.0.0:{p2p_port}"),
            p2p_public: format!("127.0.0.1:{p2p_port}"),
            config_path,
            storage_dir,
            kura_dir,
            snapshot_dir,
            rans_tables_path,
            keys: PeerKeys {
                public_key,
                private_key: ExposedPrivateKey(private_key),
                soranet_transport_public_key,
                soranet_transport_private_key: ExposedPrivateKey(soranet_transport_private_key),
                identity_public_key,
                identity_private_key: ExposedPrivateKey(identity_private_key),
                pop,
            },
        })
    }
    fn in_generation(&self, generation_root: &Path) -> Result<Self> {
        let mut relocated = self.clone();
        let peer_dir = generation_root.join("peers").join(&self.alias);
        fs::create_dir_all(&peer_dir)?;
        relocated.config_path = peer_dir.join("config.toml");
        relocated.rans_tables_path = stage_managed_rans_tables(&peer_dir)?;
        Ok(relocated)
    }
    fn in_fresh_generation(&self, generation_root: &Path, storage_dir: PathBuf) -> Result<Self> {
        let mut relocated = self.in_generation(generation_root)?;
        Self::initialize_storage(&storage_dir)?;
        relocated.kura_dir = storage_dir.join("kura");
        relocated.snapshot_dir = storage_dir.join("snapshot");
        relocated.storage_dir = storage_dir;
        Ok(relocated)
    }
    fn initialize_storage(storage_dir: &Path) -> Result<()> {
        fs::create_dir_all(storage_dir)?;
        fs::create_dir_all(
            storage_dir
                .join("snapshot")
                .join(SNAPSHOT_GENERATIONS_DIR_NAME),
        )?;
        Ok(())
    }
    fn write_config(
        &self,
        chain_id: &str,
        genesis: &GenesisMaterial,
        all_peers: &[PeerSpec],
        config_overrides: &PeerConfigOverrides,
        extra_layers: &[toml::Table],
    ) -> Result<()> {
        let mut config_overrides = config_overrides.clone();
        normalize_peer_config_overrides(
            &mut config_overrides.nexus,
            &mut config_overrides.sumeragi,
            &mut config_overrides.torii,
        )?;
        let managed_account_onboarding = config_overrides
            .torii
            .as_ref()
            .and_then(|torii| torii.get("account_onboarding"))
            .cloned();
        let mut root = toml::Table::new();
        root.insert("chain".into(), toml::Value::String(chain_id.to_owned()));
        root.insert(
            "chain_discriminant".into(),
            toml::Value::Integer(i64::from(genesis.chain_discriminant)),
        );
        root.insert(
            "public_key".into(),
            toml::Value::String(self.keys.public_key.to_string()),
        );
        root.insert(
            "private_key".into(),
            toml::Value::String(self.keys.private_key.to_string()),
        );
        root.insert(
            "soranet_transport_public_key".into(),
            toml::Value::String(self.keys.soranet_transport_public_key.to_string()),
        );
        root.insert(
            "soranet_transport_private_key".into(),
            toml::Value::String(self.keys.soranet_transport_private_key.to_string()),
        );
        let trusted = all_peers
            .iter()
            .map(|peer| toml::Value::String(peer.trusted_peer_entry()))
            .collect();
        root.insert("trusted_peers".into(), toml::Value::Array(trusted));
        let trusted_pops = all_peers
            .iter()
            .map(|peer| {
                let mut entry = toml::Table::new();
                entry.insert(
                    "public_key".into(),
                    toml::Value::String(peer.keys.public_key.to_string()),
                );
                entry.insert(
                    "pop_hex".into(),
                    toml::Value::String(encode_hex(peer.pop_bytes())),
                );
                toml::Value::Table(entry)
            })
            .collect();
        root.insert("trusted_peers_pop".into(), toml::Value::Array(trusted_pops));
        let network_bind = socket_addr_literal(&self.p2p_bind, "network.address")?;
        let network_public = socket_addr_literal(&self.p2p_public, "network.public_address")?;
        let mut network = toml::Table::new();
        network.insert("address".into(), toml::Value::String(network_bind));
        network.insert("public_address".into(), toml::Value::String(network_public));
        if all_peers.len() > 1 {
            // Keep PoW and replay protection enabled for a local full mesh,
            // while bounding the work a single developer host must perform.
            let mut puzzle = toml::Table::new();
            puzzle.insert(
                "memory_kib".into(),
                toml::Value::Integer(LOCAL_MULTI_PEER_POW_PUZZLE_MEMORY_KIB),
            );
            puzzle.insert(
                "time_cost".into(),
                toml::Value::Integer(LOCAL_MULTI_PEER_POW_PUZZLE_TIME_COST),
            );
            puzzle.insert(
                "lanes".into(),
                toml::Value::Integer(LOCAL_MULTI_PEER_POW_PUZZLE_LANES),
            );
            let mut pow = toml::Table::new();
            pow.insert(
                "difficulty".into(),
                toml::Value::Integer(LOCAL_MULTI_PEER_POW_DIFFICULTY),
            );
            pow.insert(
                "ticket_ttl_secs".into(),
                toml::Value::Integer(LOCAL_MULTI_PEER_POW_TICKET_TTL_SECS),
            );
            pow.insert(
                "revocation_store_path".into(),
                toml::Value::String(
                    self.storage_dir
                        .canonicalize()?
                        .join("soranet/ticket_revocations.norito")
                        .display()
                        .to_string(),
                ),
            );
            pow.insert("puzzle".into(), toml::Value::Table(puzzle));
            let mut soranet_handshake = toml::Table::new();
            soranet_handshake.insert("pow".into(), toml::Value::Table(pow));
            network.insert(
                "soranet_handshake".into(),
                toml::Value::Table(soranet_handshake),
            );
        }
        root.insert("network".into(), toml::Value::Table(network));
        let torii_bind = socket_addr_literal(&self.torii_bind, "torii.address")?;
        let mut torii = toml::Table::new();
        if let Some(overrides) = config_overrides.torii.as_ref() {
            merge_table(&mut torii, overrides);
        }
        torii.insert("address".into(), toml::Value::String(torii_bind));
        let torii_dir = self.storage_dir.join("torii");
        torii.insert(
            "data_dir".into(),
            toml::Value::String(torii_dir.display().to_string()),
        );
        let operator_signatures = torii
            .entry("operator_signatures")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                SupervisorError::Config("torii.operator_signatures must be a table".to_owned())
            })?;
        let allowed_public_keys = operator_signatures
            .entry("allowed_public_keys")
            .or_insert_with(|| toml::Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| {
                SupervisorError::Config(
                    "torii.operator_signatures.allowed_public_keys must be an array".to_owned(),
                )
            })?;
        let operator_public_key = self.keys.identity_public_key.to_string();
        if !allowed_public_keys
            .iter()
            .any(|value| value.as_str() == Some(operator_public_key.as_str()))
        {
            allowed_public_keys.push(toml::Value::String(operator_public_key));
        }
        let entry = torii
            .entry("da_ingest")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let toml::Value::Table(da_ingest) = entry {
            if !da_ingest.contains_key("replay_cache_store_dir") {
                da_ingest.insert(
                    "replay_cache_store_dir".into(),
                    toml::Value::String(torii_dir.join("da_replay").display().to_string()),
                );
            }
            if !da_ingest.contains_key("manifest_store_dir") {
                da_ingest.insert(
                    "manifest_store_dir".into(),
                    toml::Value::String(torii_dir.join("da_manifests").display().to_string()),
                );
            }
        }
        root.insert("torii".into(), toml::Value::Table(torii));
        // Streaming components persist sessions and provisioned routes even
        // when a developer does not exercise streaming. Give every peer its
        // own absolute state tree instead of inheriting process-wide relative
        // defaults from Mochi's launcher directory.
        let streaming_dir = self.storage_dir.canonicalize()?.join("streaming");
        let soranet_spool_dir = streaming_dir.join("soranet_routes");
        let soravpn_spool_dir = streaming_dir.join("soravpn_routes");
        let mut streaming = toml::Table::new();
        streaming.insert(
            "identity_public_key".into(),
            toml::Value::String(self.keys.identity_public_key.to_string()),
        );
        streaming.insert(
            "identity_private_key".into(),
            toml::Value::String(self.keys.identity_private_key.to_string()),
        );
        streaming.insert(
            "session_store_dir".into(),
            toml::Value::String(streaming_dir.display().to_string()),
        );
        let mut codec = toml::Table::new();
        codec.insert(
            "rans_tables_path".into(),
            toml::Value::String(self.rans_tables_path.display().to_string()),
        );
        streaming.insert("codec".into(), toml::Value::Table(codec));
        let mut soranet = toml::Table::new();
        // Automatic route provisioning is optional and unused by Mochi's
        // local/NEVO profile. An explicit overlay can opt in without changing
        // ownership of the provision spool.
        restore_streaming_soranet_defaults(&mut soranet);
        soranet.insert(
            "provision_spool_dir".into(),
            toml::Value::String(soranet_spool_dir.display().to_string()),
        );
        streaming.insert("soranet".into(), toml::Value::Table(soranet));
        let mut soravpn = toml::Table::new();
        restore_streaming_soravpn_defaults(&mut soravpn);
        soravpn.insert(
            "provision_spool_dir".into(),
            toml::Value::String(soravpn_spool_dir.display().to_string()),
        );
        streaming.insert("soravpn".into(), toml::Value::Table(soravpn));
        root.insert("streaming".into(), toml::Value::Table(streaming));
        let mut genesis_table = toml::Table::new();
        genesis_table.insert(
            "public_key".into(),
            toml::Value::String(genesis.public_key().to_string()),
        );
        genesis_table.insert(
            "file".into(),
            toml::Value::String(genesis.block_path.display().to_string()),
        );
        genesis_table.insert(
            "manifest_json".into(),
            toml::Value::String(genesis.manifest_path.display().to_string()),
        );
        if genesis.expected_hash.is_some() {
            genesis_table.insert(
                "expected_hash_file".into(),
                toml::Value::String(genesis.expected_hash_path.display().to_string()),
            );
        } else {
            genesis_table.insert(
                "expected_hash".into(),
                toml::Value::String(GENESIS_EXPECTED_HASH_PLACEHOLDER.to_owned()),
            );
        }
        root.insert("genesis".into(), toml::Value::Table(genesis_table));
        let mut kura = toml::Table::new();
        kura.insert(
            "store_dir".into(),
            toml::Value::String(self.kura_dir.display().to_string()),
        );
        root.insert("kura".into(), toml::Value::Table(kura));
        let mut snapshot = toml::Table::new();
        snapshot.insert(
            "store_dir".into(),
            toml::Value::String(self.snapshot_dir.display().to_string()),
        );
        root.insert("snapshot".into(), toml::Value::Table(snapshot));
        let mut confidential = toml::Table::new();
        confidential.insert("enabled".into(), toml::Value::Boolean(true));
        root.insert("confidential".into(), toml::Value::Table(confidential));
        // The embedded SoraFS runtime persists transaction-forwarder
        // checkpoints even when provider storage and its worker loops are
        // disabled. Do not let multiple managed peers inherit the process-wide
        // `./storage/sorafs` default and contend for one checkpoint lock.
        let sorafs_dir = self.storage_dir.join("sorafs");
        let mut sorafs_storage = toml::Table::new();
        sorafs_storage.insert(
            "data_dir".into(),
            toml::Value::String(sorafs_dir.display().to_string()),
        );
        let mut sorafs = toml::Table::new();
        sorafs.insert("storage".into(), toml::Value::Table(sorafs_storage));
        root.insert("sorafs".into(), toml::Value::Table(sorafs));
        if let Some(table) = config_overrides.sumeragi.as_ref()
            && !table.is_empty()
        {
            root.insert("sumeragi".into(), toml::Value::Table(table.clone()));
        }
        if let Some(table) = config_overrides.nexus.as_ref()
            && !table.is_empty()
        {
            root.insert("nexus".into(), toml::Value::Table(table.clone()));
        }
        for overlay in extra_layers {
            merge_table(&mut root, overlay);
        }
        let configured_genesis = root
            .get("genesis")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| SupervisorError::Config("genesis must be a table".to_owned()))?;
        if genesis.expected_hash.is_some() {
            let expected_path = genesis.expected_hash_path.display().to_string();
            if configured_genesis
                .get("expected_hash_file")
                .and_then(toml::Value::as_str)
                != Some(expected_path.as_str())
                || configured_genesis.contains_key("expected_hash")
            {
                return Err(SupervisorError::Config(format!(
                    "runtime config must select only Mochi's generated genesis identity file `{expected_path}`"
                )));
            }
        } else if configured_genesis
            .get("expected_hash")
            .and_then(toml::Value::as_str)
            != Some(GENESIS_EXPECTED_HASH_PLACEHOLDER)
            || configured_genesis.contains_key("expected_hash_file")
        {
            return Err(SupervisorError::Config(
                "bootstrap config must select only Mochi's unresolved inline genesis identity"
                    .to_owned(),
            ));
        }
        // Apply the generator invariant after the shallow overlays have selected their effective
        // queue table. This preserves later authored values whenever they already cover the
        // generated PoP roster while raising only an under-budget aggregate capacity.
        ensure_generated_sumeragi_body_bytes(&mut root, all_peers.len())?;
        if let Some(expected) = managed_account_onboarding.as_ref() {
            let configured = root
                .get("torii")
                .and_then(toml::Value::as_table)
                .and_then(|torii| torii.get("account_onboarding"));
            if configured != Some(expected) {
                return Err(SupervisorError::Config(
                    "temporary config overlays must preserve Mochi's managed torii.account_onboarding bundle"
                        .to_owned(),
                ));
            }
        }
        let expected_kura_dir = self.kura_dir.display().to_string();
        let configured_kura_dir = root
            .get("kura")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("store_dir"))
            .and_then(toml::Value::as_str);
        if configured_kura_dir != Some(expected_kura_dir.as_str()) {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed Kura root `{expected_kura_dir}`"
            )));
        }
        let expected_sorafs_dir = sorafs_dir.display().to_string();
        let sorafs = root
            .entry("sorafs")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| SupervisorError::Config("sorafs must be a table".to_owned()))?;
        let sorafs_storage = sorafs
            .entry("storage")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| SupervisorError::Config("sorafs.storage must be a table".to_owned()))?;
        if let Some(configured) = sorafs_storage.get("data_dir")
            && configured.as_str() != Some(expected_sorafs_dir.as_str())
        {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed SoraFS root `{expected_sorafs_dir}`"
            )));
        }
        // A shallow top-level overlay such as `[sorafs.storage] enabled = true`
        // replaces the generated table. Restore the launcher-owned path after
        // validating that the overlay did not try to redirect it.
        sorafs_storage.insert("data_dir".into(), toml::Value::String(expected_sorafs_dir));
        let expected_streaming_dir = streaming_dir.display().to_string();
        let expected_soranet_spool_dir = soranet_spool_dir.display().to_string();
        let expected_soravpn_spool_dir = soravpn_spool_dir.display().to_string();
        let streaming = root
            .entry("streaming")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| SupervisorError::Config("streaming must be a table".to_owned()))?;
        if let Some(configured) = streaming.get("session_store_dir")
            && configured.as_str() != Some(expected_streaming_dir.as_str())
        {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed streaming session root `{expected_streaming_dir}`"
            )));
        }
        streaming.insert(
            "session_store_dir".into(),
            toml::Value::String(expected_streaming_dir),
        );
        // A shallow `[streaming.soranet]` overlay replaces the generated
        // top-level streaming table. Restore required peer identity fields
        // only when absent, while retaining any explicit overlay values.
        streaming
            .entry("identity_public_key")
            .or_insert_with(|| toml::Value::String(self.keys.identity_public_key.to_string()));
        streaming
            .entry("identity_private_key")
            .or_insert_with(|| toml::Value::String(self.keys.identity_private_key.to_string()));
        let expected_rans_tables_path = self.rans_tables_path.display().to_string();
        let codec = streaming
            .entry("codec")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| SupervisorError::Config("streaming.codec must be a table".to_owned()))?;
        // `StreamingCodec` is read as one nested value. Mochi creates
        // `[streaming.codec]` for its managed rANS table, so keep the generated
        // object structurally complete while retaining explicit overlay values.
        codec
            .entry("cabac_mode")
            .or_insert_with(|| toml::Value::String("disabled".to_owned()));
        codec
            .entry("trellis_blocks")
            .or_insert_with(|| toml::Value::Array(Vec::new()));
        codec
            .entry("entropy_mode")
            .or_insert_with(|| toml::Value::String("rans_bundled".to_owned()));
        codec
            .entry("bundle_width")
            .or_insert(toml::Value::Integer(2));
        codec
            .entry("bundle_accel")
            .or_insert_with(|| toml::Value::String("none".to_owned()));
        if let Some(configured) = codec.get("rans_tables_path")
            && configured.as_str() != Some(expected_rans_tables_path.as_str())
        {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed rANS table `{expected_rans_tables_path}`"
            )));
        }
        codec.insert(
            "rans_tables_path".into(),
            toml::Value::String(expected_rans_tables_path),
        );
        let soranet = streaming
            .entry("soranet")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                SupervisorError::Config("streaming.soranet must be a table".to_owned())
            })?;
        if let Some(configured) = soranet.get("provision_spool_dir")
            && configured.as_str() != Some(expected_soranet_spool_dir.as_str())
        {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed SoraNet provision spool `{expected_soranet_spool_dir}`"
            )));
        }
        soranet.insert(
            "provision_spool_dir".into(),
            toml::Value::String(expected_soranet_spool_dir),
        );
        soranet
            .entry("enabled")
            .or_insert(toml::Value::Boolean(false));
        restore_streaming_soranet_defaults(soranet);
        let soravpn = streaming
            .entry("soravpn")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                SupervisorError::Config("streaming.soravpn must be a table".to_owned())
            })?;
        if let Some(configured) = soravpn.get("provision_spool_dir")
            && configured.as_str() != Some(expected_soravpn_spool_dir.as_str())
        {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed SoraVPN provision spool `{expected_soravpn_spool_dir}`"
            )));
        }
        soravpn.insert(
            "provision_spool_dir".into(),
            toml::Value::String(expected_soravpn_spool_dir),
        );
        restore_streaming_soravpn_defaults(soravpn);
        let header = Self::config_header(
            chain_id,
            genesis,
            &self.kura_dir,
            config_overrides.nexus.as_ref(),
        );
        let config_str = toml::to_string_pretty(&toml::Value::Table(root))?;
        let rendered = format!("{header}\n\n{config_str}");
        self.write_owner_only_config(rendered.as_bytes())?;
        Ok(())
    }
    #[cfg(unix)]
    fn write_owner_only_config(&self, payload: &[u8]) -> Result<()> {
        let parent = self.config_path.parent().ok_or_else(|| {
            SupervisorError::Config("managed peer config has no parent directory".to_owned())
        })?;
        let owner_uid = fs::metadata(parent)?.uid();
        if let Ok(existing) = fs::symlink_metadata(&self.config_path)
            && (!existing.file_type().is_file()
                || existing.file_type().is_symlink()
                || existing.uid() != owner_uid
                || existing.nlink() != 1)
        {
            return Err(SupervisorError::Config(format!(
                "managed peer config `{}` must be an owner-owned regular single-link file",
                self.config_path.display()
            )));
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).mode(0o600);
        let mut file = options.open(&self.config_path)?;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(0o600);
        file.set_permissions(permissions)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != owner_uid
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(SupervisorError::Config(format!(
                "managed peer config `{}` could not be secured as an owner-only regular single-link file",
                self.config_path.display()
            )));
        }
        file.set_len(0)?;
        file.write_all(payload)?;
        file.sync_all()?;
        Ok(())
    }
    #[cfg(not(unix))]
    fn write_owner_only_config(&self, _payload: &[u8]) -> Result<()> {
        Err(SupervisorError::Config(
            "managed peer configs require owner-only file support".to_owned(),
        ))
    }
    fn config_header(
        chain_id: &str,
        genesis: &GenesisMaterial,
        storage_root: &Path,
        nexus: Option<&toml::Table>,
    ) -> String {
        let mut lines = Vec::new();
        lines.push(format!("# mochi.chain_id = {chain_id}"));
        if let Some(fingerprint) = genesis.consensus_fingerprint.as_deref() {
            lines.push(format!("# mochi.consensus_fingerprint = {fingerprint}"));
        }
        lines.extend(lane_path_comments(storage_root, nexus));
        lines.join("\n")
    }
    fn trusted_peer_entry(&self) -> String {
        format!("{}@{}", self.keys.public_key, self.p2p_public)
    }
    fn peer_id(&self) -> PeerId {
        self.keys.public_key.clone().into()
    }
    fn pop_bytes(&self) -> &[u8] {
        &self.keys.pop
    }
    fn torii_base_http(&self) -> String {
        format!("http://{}", self.torii_public)
    }
    fn operator_signing_context(
        &self,
        network_id: NetworkId,
    ) -> ToriiResult<OperatorSigningContext> {
        let key_pair = KeyPair::new(
            self.keys.identity_public_key.clone(),
            self.keys.identity_private_key.0.clone(),
        )
        .map_err(|error| {
            ToriiError::SignedQueryContext(format!(
                "managed peer `{}` has an invalid operator key pair: {error}",
                self.alias
            ))
        })?;
        Ok(OperatorSigningContext::new(network_id, key_pair))
    }
}
#[derive(Debug, Clone)]
struct PeerKeys {
    public_key: PublicKey,
    private_key: ExposedPrivateKey,
    soranet_transport_public_key: PublicKey,
    soranet_transport_private_key: ExposedPrivateKey,
    identity_public_key: PublicKey,
    identity_private_key: ExposedPrivateKey,
    pop: Vec<u8>,
}
#[derive(Debug)]
struct GenesisMaterial {
    generation_id: String,
    key_pair: KeyPair,
    manifest_path: PathBuf,
    block_path: PathBuf,
    expected_hash_path: PathBuf,
    public_key_path: PathBuf,
    expected_hash: Option<HashOf<BlockHeader>>,
    chain_discriminant: u16,
    profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<String>,
    verify_report: Option<KagamiVerifyReport>,
    consensus_fingerprint: Option<String>,
}
#[derive(Clone, Copy)]
struct GenesisCreateContext<'a> {
    generation_id: &'a str,
    generation_root: &'a Path,
    chain_id: &'a str,
    peers: &'a [PeerSpec],
    config_overrides: &'a PeerConfigOverrides,
    consensus_mode: SumeragiConsensusMode,
    block_cadence_ms: NonZeroU64,
    genesis_profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<&'a str>,
    onboarding_authority: &'a AccountId,
}
#[derive(Debug)]
struct TemporaryGenesisKeyFile {
    path: PathBuf,
}
impl TemporaryGenesisKeyFile {
    #[cfg(unix)]
    fn create(genesis_dir: &Path, key_pair: &KeyPair) -> Result<Self> {
        const MAX_CREATE_ATTEMPTS: u8 = 32;
        // Kagami rejects private-key paths containing any symbolic-link
        // component. macOS commonly exposes its temporary directory through
        // `/var`, so resolve the managed directory before deriving the file.
        let genesis_dir = fs::canonicalize(genesis_dir)?;
        for attempt in 0..MAX_CREATE_ATTEMPTS {
            let path = genesis_dir.join(format!(
                ".mochi-genesis-signing-key-{}-{}-{attempt}",
                std::process::id(),
                timestamp_ms()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true).mode(0o600);
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            let guard = Self { path };
            let canonical = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
            file.write_all(canonical.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            return Ok(guard);
        }
        Err(SupervisorError::Config(format!(
            "failed to allocate an owner-only genesis signing key beneath `{}`",
            genesis_dir.display()
        )))
    }
    #[cfg(not(unix))]
    fn create(_genesis_dir: &Path, _key_pair: &KeyPair) -> Result<Self> {
        Err(SupervisorError::Config(
            "config-bound genesis signing requires owner-only private-key file support".to_owned(),
        ))
    }
    fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for TemporaryGenesisKeyFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "warning: failed to remove temporary genesis signing key `{}`: {error}",
                self.path.display()
            );
        }
    }
}
/// Build the canonical signed genesis body used by Mochi's Kagami test stub.
///
/// This helper is intentionally available only to tests and consumers which
/// opt into `test`; production supervision always invokes Kagami.
///
/// # Errors
///
/// Returns an error when the prepared manifest or node configuration cannot
/// be parsed, their genesis bindings differ, or canonical signing fails.
#[cfg(any(test, feature = "test"))]
pub fn sign_kagami_stub_genesis_from_config(
    manifest_path: &Path,
    config_path: &Path,
    key_pair: &KeyPair,
    expected_consensus_mode: Option<SumeragiConsensusMode>,
) -> Result<iroha_data_model::block::SignedBlock> {
    sign_prepared_genesis_from_config(
        manifest_path,
        config_path,
        key_pair,
        expected_consensus_mode,
    )
    .map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "test Kagami stub failed signing canonical genesis: {error:#}"
        ))
    })
}
/// Derive the exact genesis policies selected by a finalized node config.
///
/// This test-only companion keeps config parsing behind Mochi's existing orchestration dependency
/// while allowing the Kagami mock to verify the canonical block it emitted.
///
/// # Errors
///
/// Returns an error when the node configuration cannot be parsed or omits a
/// required genesis binding.
#[cfg(any(test, feature = "test"))]
pub fn kagami_stub_genesis_policies_from_config(
    config_path: &Path,
) -> Result<(
    iroha_data_model::da::commitment::DaProofPolicyBundle,
    [u8; 32],
)> {
    let config = ManagedNodeConfig::from_path(config_path).map_err(|error| {
        SupervisorError::KagamiInvocation(format!(
            "test Kagami stub failed loading config `{}`: {error:#}",
            config_path.display()
        ))
    })?;
    Ok((
        config.da_proof_policies,
        config.genesis_confidential_policy_hash,
    ))
}
impl GenesisMaterial {
    fn create(binaries: &mut BinaryPaths, context: GenesisCreateContext<'_>) -> Result<Self> {
        let GenesisCreateContext {
            generation_id,
            generation_root,
            chain_id,
            peers,
            config_overrides,
            consensus_mode,
            block_cadence_ms,
            genesis_profile,
            vrf_seed_hex,
            onboarding_authority,
        } = context;
        let genesis_dir = generation_root.join("genesis");
        fs::create_dir_all(&genesis_dir)?;
        let manifest_path = genesis_dir.join(GENESIS_FILE_NAME);
        let block_path = genesis_dir.join(GENESIS_SIGNED_FILE_NAME);
        let expected_hash_path = genesis_dir.join(GENESIS_EXPECTED_HASH_FILE_NAME);
        let public_key_path = genesis_dir.join(GENESIS_PUBLIC_KEY_FILE_NAME);
        let key_pair = KeyPair::random();
        let manifest = Self::generate_manifest(
            binaries,
            &genesis_dir,
            chain_id,
            key_pair.public_key(),
            consensus_mode,
            genesis_profile,
            vrf_seed_hex,
        )?;
        // Public Kagami profiles own their signed cadence and are checked by
        // `kagami verify`. Unprofiled Mochi sandboxes bind the documented
        // one-second localnet cadence into their exact validator committee.
        let manifest = if genesis_profile.is_some() {
            manifest
        } else {
            manifest
                .into_builder()
                .with_block_cadence_ms(block_cadence_ms)
                .build_raw()
                .with_consensus_meta()
        };
        let manifest =
            genesis::with_local_account_onboarding_bootstrap(manifest, onboarding_authority)?;
        let topology: Vec<GenesisTopologyEntry> = peers
            .iter()
            .map(|spec| GenesisTopologyEntry::new(spec.peer_id(), spec.pop_bytes().to_vec()))
            .collect();
        let manifest = genesis::with_topology(manifest, topology);
        let json = norito::json::to_vec_pretty(&manifest)?;
        fs::write(&manifest_path, json)?;
        fs::write(&public_key_path, format!("{}\n", key_pair.public_key()))?;
        let mut material = Self {
            generation_id: generation_id.to_owned(),
            key_pair,
            manifest_path,
            block_path,
            expected_hash_path,
            public_key_path,
            expected_hash: None,
            chain_discriminant: manifest.chain_discriminant(),
            profile: genesis_profile,
            vrf_seed_hex: vrf_seed_hex.map(|value| value.to_owned()),
            verify_report: None,
            consensus_fingerprint: None,
        };
        let primary = peers.first().ok_or_else(|| {
            SupervisorError::Config(
                "Mochi genesis requires an exact 3f+1 validator committee".to_owned(),
            )
        })?;
        // Kagami must stage genesis against the exact peer configuration that
        // irohad will consume. The paths and public key are already stable, so
        // render the primary config once before signing, then let the caller
        // rewrite every peer config with the final bound fingerprint header.
        primary.write_config(chain_id, &material, peers, config_overrides, &[])?;
        let (manifest, expected_hash) = material.sign_manifest_with_kagami(
            binaries,
            primary.config_path.as_path(),
            consensus_mode,
        )?;
        material.expected_hash = Some(expected_hash);
        let verify_report = if let Some(profile) = genesis_profile {
            Some(Self::verify_manifest_with_kagami(
                binaries,
                &material.manifest_path,
                profile,
                vrf_seed_hex,
            )?)
        } else {
            None
        };
        let consensus_fingerprint = verify_report
            .as_ref()
            .and_then(|report| report.fingerprint.clone())
            .or_else(|| {
                manifest
                    .consensus_fingerprint()
                    .map(|value| value.to_string())
            })
            .or_else(|| {
                let normalized = manifest.clone().with_consensus_meta();
                normalized
                    .consensus_fingerprint()
                    .map(|value| value.to_string())
            });
        material.verify_report = verify_report;
        material.consensus_fingerprint = consensus_fingerprint;
        Ok(material)
    }
    fn copy_into_generation(&self, generation_id: &str, generation_root: &Path) -> Result<Self> {
        let genesis_dir = generation_root.join("genesis");
        fs::create_dir_all(&genesis_dir)?;
        let manifest_path = genesis_dir.join(GENESIS_FILE_NAME);
        let block_path = genesis_dir.join(GENESIS_SIGNED_FILE_NAME);
        let expected_hash_path = genesis_dir.join(GENESIS_EXPECTED_HASH_FILE_NAME);
        let public_key_path = genesis_dir.join(GENESIS_PUBLIC_KEY_FILE_NAME);
        fs::copy(&self.manifest_path, &manifest_path)?;
        fs::copy(&self.block_path, &block_path)?;
        fs::copy(&self.expected_hash_path, &expected_hash_path)?;
        fs::copy(&self.public_key_path, &public_key_path)?;
        Ok(Self {
            generation_id: generation_id.to_owned(),
            key_pair: self.key_pair.clone(),
            manifest_path,
            block_path,
            expected_hash_path,
            public_key_path,
            expected_hash: self.expected_hash,
            chain_discriminant: self.chain_discriminant,
            profile: self.profile,
            vrf_seed_hex: self.vrf_seed_hex.clone(),
            verify_report: self.verify_report.clone(),
            consensus_fingerprint: self.consensus_fingerprint.clone(),
        })
    }
    fn sign_manifest_with_kagami(
        &self,
        binaries: &mut BinaryPaths,
        config_path: &Path,
        consensus_mode: SumeragiConsensusMode,
    ) -> Result<(RawGenesisTransaction, HashOf<BlockHeader>)> {
        let kagami = binaries.ensure_kagami_ready()?;
        let genesis_dir = self.manifest_path.parent().ok_or_else(|| {
            SupervisorError::Config(format!(
                "genesis manifest path `{}` has no parent directory",
                self.manifest_path.display()
            ))
        })?;
        let private_key_file = TemporaryGenesisKeyFile::create(genesis_dir, &self.key_pair)?;
        let mut command = Command::new(kagami);
        command
            .current_dir(genesis_dir)
            .arg("genesis")
            .arg("sign")
            .arg(&self.manifest_path)
            .arg("--out-file")
            .arg(&self.block_path)
            .arg("--bound-manifest-out")
            .arg(&self.manifest_path)
            .arg("--expected-hash-out")
            .arg(&self.expected_hash_path)
            .arg("--private-key-file")
            .arg(private_key_file.path())
            .arg("--config")
            .arg(config_path)
            .arg("--consensus-mode")
            .arg(match consensus_mode {
                SumeragiConsensusMode::Permissioned => "permissioned",
                SumeragiConsensusMode::Npos => "npos",
            })
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "failed to invoke `kagami genesis sign`: {error}"
            ))
        })?;
        drop(private_key_file);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` exited with status {}: {stderr}",
                output.status
            )));
        }
        #[cfg(any(test, feature = "test"))]
        if std::env::var_os(TEST_FINALIZE_KAGAMI_STUB_SIGNATURE).is_some() {
            self.finalize_kagami_stub_signature(config_path, consensus_mode)?;
        }
        let signed_metadata = fs::metadata(&self.block_path).map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` did not create `{}`: {error}",
                self.block_path.display()
            ))
        })?;
        if !signed_metadata.is_file() || signed_metadata.len() == 0 {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` emitted an empty signed block at `{}`",
                self.block_path.display()
            )));
        }
        let expected_hash_record = read_generated_genesis_record(
            &self.expected_hash_path,
            "generated checked genesis network identity",
        )
        .map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` did not create an exact checked genesis network identity `{}`: {error}",
                self.expected_hash_path.display()
            ))
        })?;
        let expected_hash_literal = expected_hash_record
            .strip_suffix('\n')
            .expect("exact-record reader preserves one trailing LF");
        let expected_network_id = expected_hash_literal
            .parse::<NetworkId>()
            .map_err(|error| {
                SupervisorError::KagamiInvocation(format!(
                    "failed to parse checked genesis network identity `{}`: {error}",
                    self.expected_hash_path.display()
                ))
            })?;
        if expected_hash_record != format!("{expected_network_id}\n") {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami genesis sign` produced a non-canonical checked genesis network identity at `{}`",
                self.expected_hash_path.display()
            )));
        }
        let expected_hash = expected_network_id.into_genesis_hash();
        let manifest = RawGenesisTransaction::from_path(&self.manifest_path)?;
        Ok((manifest, expected_hash))
    }
    #[cfg(any(test, feature = "test"))]
    fn finalize_kagami_stub_signature(
        &self,
        config_path: &Path,
        consensus_mode: SumeragiConsensusMode,
    ) -> Result<()> {
        let block = sign_kagami_stub_genesis_from_config(
            &self.manifest_path,
            config_path,
            &self.key_pair,
            Some(consensus_mode),
        )?;
        let wire = block.encode_wire().map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "test Kagami stub failed encoding canonical genesis: {error}"
            ))
        })?;
        fs::write(&self.block_path, wire)?;
        fs::write(
            &self.expected_hash_path,
            format!("{}\n", NetworkId::from_genesis_hash(block.hash())),
        )?;
        Ok(())
    }
    fn validate_generation(&self, chain_id: &str, peers: &[PeerSpec]) -> Result<()> {
        let signed = iroha_genesis::read_signed_genesis_bytes(&self.block_path).map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "failed to read signed genesis `{}` under the {}-byte first-release limit: {error}",
                self.block_path.display(),
                iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1
            ))
        })?;
        let manifest = RawGenesisTransaction::from_path(&self.manifest_path)?;
        let expected_chain = chain_id.parse::<ChainId>().map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "configured chain id `{chain_id}` is invalid: {error}"
            ))
        })?;
        if manifest.chain_id() != &expected_chain
            || manifest.chain_discriminant() != self.chain_discriminant
        {
            return Err(SupervisorError::GenerationValidation(
                "persisted genesis manifest changed its chain or discriminant".to_owned(),
            ));
        }
        let expected_hash = self.expected_hash.ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "candidate generation has no exact genesis hash".to_owned(),
            )
        })?;
        let public_record = read_generated_genesis_record(
            &self.public_key_path,
            "generated genesis public-key record",
        )
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "failed to read candidate genesis public-key record `{}`: {error}",
                self.public_key_path.display()
            ))
        })?;
        let public_literal = public_record
            .strip_suffix('\n')
            .expect("exact-record reader preserves one trailing LF");
        let public_key = public_literal.parse::<PublicKey>().map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "candidate genesis public-key record `{}` is invalid: {error}",
                self.public_key_path.display()
            ))
        })?;
        if public_record != format!("{public_key}\n") || &public_key != self.public_key() {
            return Err(SupervisorError::GenerationValidation(
                "candidate genesis public-key record is not exact and canonical".to_owned(),
            ));
        }
        let validated = validate_prepared_genesis_for_startup(
            &signed,
            &manifest,
            self.public_key(),
            expected_hash,
            &expected_chain,
        )
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "prepared signed-genesis startup validation failed: {error:#}"
            ))
        })?;
        drop(signed);
        let expected_roster = peers
            .iter()
            .map(|peer| (peer.keys.public_key.clone(), peer.keys.pop.clone()))
            .collect::<BTreeMap<_, _>>();
        if validated.validator_pops() != &expected_roster {
            return Err(SupervisorError::GenerationValidation(
                "signed genesis validator roster differs from the candidate peers".to_owned(),
            ));
        }
        let canonical_block = fs::canonicalize(&self.block_path)?;
        let canonical_manifest = fs::canonicalize(&self.manifest_path)?;
        for peer in peers {
            let config = ManagedNodeConfig::from_path(&peer.config_path).map_err(|error| {
                SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` failed loading: {error:#}",
                    peer.config_path.display()
                ))
            })?;
            validate_managed_peer_paths(&config, peer, peers.len())?;
            if config.chain_id != expected_chain
                || config.chain_discriminant != self.chain_discriminant
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` has the wrong chain or discriminant",
                    peer.config_path.display()
                )));
            }
            if config.genesis_public_key != *self.public_key()
                || config.genesis_expected_hash != expected_hash
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` has a different genesis key or hash",
                    peer.config_path.display()
                )));
            }
            if validated.block().da_proof_policies() != Some(&config.da_proof_policies) {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` DA proof policy differs from the signed genesis header",
                    peer.config_path.display()
                )));
            }
            let signed_confidential_policy = validated
                .block()
                .header()
                .confidential_features()
                .and_then(|digest| digest.zk_policy_hash);
            if signed_confidential_policy != Some(config.genesis_confidential_policy_hash) {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` confidential policy differs from the signed genesis header",
                    peer.config_path.display()
                )));
            }
            if fs::canonicalize(&config.genesis_block_path)? != canonical_block
                || fs::canonicalize(&config.genesis_manifest_path)? != canonical_manifest
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` selects genesis outside its generation",
                    peer.config_path.display()
                )));
            }
            if config.trusted_peer_pops != expected_roster
                || config.local_public_key != peer.keys.public_key
            {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate peer config `{}` identity or PoP roster differs from signed genesis",
                    peer.config_path.display()
                )));
            }
        }
        Ok(())
    }
    fn generate_manifest(
        binaries: &mut BinaryPaths,
        genesis_dir: &Path,
        chain_id: &str,
        genesis_public_key: &PublicKey,
        consensus_mode: SumeragiConsensusMode,
        genesis_profile: Option<GenesisProfile>,
        vrf_seed_hex: Option<&str>,
    ) -> Result<RawGenesisTransaction> {
        if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {
            if let Some(profile) = genesis_profile {
                return Err(SupervisorError::KagamiInvocation(format!(
                    "MOCHI_TEST_USE_INTERNAL_GENESIS cannot satisfy genesis profile {profile:?}; unset the override to invoke kagami"
                )));
            }
            // Test-only escape hatch so integration harnesses can reuse the internal
            // genesis helper without shelling out to Kagami.
            let chain = chain_id.parse::<ChainId>().map_err(|error| {
                SupervisorError::KagamiInvocation(format!(
                    "configured chain id must be canonical: {error}"
                ))
            })?;
            return crate::genesis::default_manifest(
                chain,
                genesis_public_key,
                genesis_dir.to_path_buf(),
                consensus_mode,
                None,
            )
            .map_err(|err| {
                SupervisorError::KagamiInvocation(format!(
                    "failed to generate internal genesis manifest: {err}"
                ))
            });
        }
        if let Some(profile) = genesis_profile
            && profile.requires_seed()
            && vrf_seed_hex.is_none()
        {
            return Err(SupervisorError::KagamiInvocation(format!(
                "profile {profile:?} requires `--vrf-seed-hex <hex>` when generating genesis"
            )));
        }
        if genesis_profile.is_none() && vrf_seed_hex.is_some() {
            return Err(SupervisorError::KagamiInvocation(
                "`--vrf-seed-hex` requires a genesis profile (NPoS mode)".into(),
            ));
        }
        let kagami = binaries.ensure_kagami_ready()?;
        let mut command = Command::new(kagami);
        command
            .current_dir(genesis_dir)
            .arg("genesis")
            .arg("generate")
            .arg("--ivm-dir")
            .arg(".")
            .arg("--genesis-public-key")
            .arg(genesis_public_key.to_string())
            .arg("--chain-id")
            .arg(chain_id);
        if let Some(profile) = genesis_profile {
            command.arg("--profile").arg(profile.as_kagami_arg());
        }
        command.arg("--consensus-mode").arg(match consensus_mode {
            SumeragiConsensusMode::Permissioned => "permissioned",
            SumeragiConsensusMode::Npos => "npos",
        });
        if let Some(seed) = vrf_seed_hex {
            command.arg("--vrf-seed-hex").arg(seed);
        }
        command
            .arg("default")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().map_err(|err| {
            SupervisorError::KagamiInvocation(format!("failed to invoke `kagami`: {err}"))
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami` exited with status {}: {stderr}",
                output.status
            )));
        }
        if genesis_profile.is_some() && !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }
        if output.stdout.is_empty() {
            return Err(SupervisorError::KagamiInvocation(
                "`kagami` did not emit genesis JSON".into(),
            ));
        }
        let mut value: Value = norito::json::from_slice(&output.stdout).map_err(|err| {
            SupervisorError::KagamiInvocation(format!(
                "failed to parse `kagami` JSON output: {err}"
            ))
        })?;
        let object = value.as_object_mut().ok_or_else(|| {
            SupervisorError::KagamiInvocation("`kagami` JSON payload must be an object".into())
        })?;
        object.insert("chain".into(), Value::String(chain_id.to_owned()));
        let manifest: RawGenesisTransaction = norito::json::from_value(value).map_err(|err| {
            SupervisorError::KagamiInvocation(format!(
                "failed to decode genesis manifest from `kagami` output: {err}"
            ))
        })?;
        Ok(manifest)
    }
    fn verify_manifest_with_kagami(
        binaries: &mut BinaryPaths,
        manifest_path: &Path,
        profile: GenesisProfile,
        vrf_seed_hex: Option<&str>,
    ) -> Result<KagamiVerifyReport> {
        let kagami = binaries.ensure_kagami_ready()?;
        let mut command = Command::new(kagami);
        command
            .arg("verify")
            .arg("--profile")
            .arg(profile.as_kagami_arg())
            .arg("--genesis")
            .arg(manifest_path);
        if let Some(seed) = vrf_seed_hex {
            command.arg("--vrf-seed-hex").arg(seed);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().map_err(|err| {
            SupervisorError::KagamiInvocation(format!("failed to invoke `kagami verify`: {err}"))
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami verify` exited with status {}: {stderr}",
                output.status
            )));
        }
        let mut combined = stdout.trim_end().to_owned();
        let stderr_trimmed = stderr.trim_end();
        if !stderr_trimmed.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(stderr_trimmed);
        }
        Ok(parse_kagami_verify_output(profile, vrf_seed_hex, &combined))
    }
    fn public_key(&self) -> &PublicKey {
        self.key_pair.public_key()
    }
}
fn validate_managed_peer_paths(
    config: &ManagedNodeConfig,
    peer: &PeerSpec,
    peer_count: usize,
) -> Result<()> {
    validate_managed_peer_paths_against(
        config,
        &peer.config_path,
        &peer.storage_dir,
        &peer.rans_tables_path,
        peer_count,
    )
}
fn validate_managed_peer_paths_against(
    config: &ManagedNodeConfig,
    config_path: &Path,
    storage_dir: &Path,
    rans_tables_path: &Path,
    peer_count: usize,
) -> Result<()> {
    let torii_dir = storage_dir.join("torii");
    let streaming_dir = storage_dir.join("streaming");
    let managed_paths = [
        (
            "kura.store_dir",
            config.managed_paths.kura_store_dir.clone(),
            storage_dir.join("kura"),
        ),
        (
            "snapshot.store_dir",
            config.managed_paths.snapshot_store_dir.clone(),
            storage_dir.join("snapshot"),
        ),
        (
            "torii.data_dir",
            config.managed_paths.torii_data_dir.clone(),
            torii_dir.clone(),
        ),
        (
            "torii.da_ingest.replay_cache_store_dir",
            config.managed_paths.torii_da_replay_cache_store_dir.clone(),
            torii_dir.join("da_replay"),
        ),
        (
            "torii.da_ingest.manifest_store_dir",
            config.managed_paths.torii_da_manifest_store_dir.clone(),
            torii_dir.join("da_manifests"),
        ),
        (
            "torii.sorafs_storage.data_dir",
            config.managed_paths.torii_sorafs_storage_data_dir.clone(),
            storage_dir.join("sorafs"),
        ),
        (
            "streaming.session_store_dir",
            config.managed_paths.streaming_session_store_dir.clone(),
            streaming_dir.clone(),
        ),
        (
            "streaming.soranet.provision_spool_dir",
            config
                .managed_paths
                .streaming_soranet_provision_spool_dir
                .clone(),
            streaming_dir.join("soranet_routes"),
        ),
        (
            "streaming.soravpn.provision_spool_dir",
            config
                .managed_paths
                .streaming_soravpn_provision_spool_dir
                .clone(),
            streaming_dir.join("soravpn_routes"),
        ),
        (
            "streaming.codec.rans_tables_path",
            config.managed_paths.streaming_rans_tables_path.clone(),
            rans_tables_path.to_path_buf(),
        ),
    ];
    for (field, configured, expected) in managed_paths {
        if configured != expected {
            return Err(SupervisorError::GenerationValidation(format!(
                "candidate peer config `{}` redirects Mochi-managed `{field}` from `{}` to `{}`",
                config_path.display(),
                expected.display(),
                configured.display()
            )));
        }
    }
    if peer_count > 1 {
        let configured = &config.managed_paths.soranet_pow_revocation_store_path;
        let expected = storage_dir.join("soranet/ticket_revocations.norito");
        if configured != &expected {
            return Err(SupervisorError::GenerationValidation(format!(
                "candidate peer config `{}` redirects Mochi-managed `network.soranet_handshake.pow.revocation_store_path` from `{}` to `{}`",
                config_path.display(),
                expected.display(),
                configured.display()
            )));
        }
    }
    Ok(())
}
fn default_data_root() -> PathBuf {
    std::env::var_os("MOCHI_DATA_ROOT")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::env::temp_dir().join("mochi"))
}
fn resolve_data_root(data_root: &Path) -> io::Result<PathBuf> {
    if data_root.is_absolute() {
        Ok(data_root.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(data_root))
    }
}
struct SnapshotMetadata {
    chain_id: String,
    generation_id: String,
    peer_count: u64,
    genesis_hash: Hash,
    kura_hashes: HashMap<String, Hash>,
}
fn verify_snapshot_artifact_matches_selected(
    snapshot_root: &Path,
    snapshot_path: &Path,
    selected_path: &Path,
    label: &str,
) -> Result<()> {
    let metadata = fs::symlink_metadata(snapshot_path).map_err(|error| {
        SupervisorError::Config(format!(
            "snapshot `{}` cannot read {label} `{}`: {error}",
            snapshot_root.display(),
            snapshot_path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SupervisorError::Config(format!(
            "snapshot `{}` {label} `{}` must be a regular non-symlink file",
            snapshot_root.display(),
            snapshot_path.display()
        )));
    }
    let equal = regular_snapshot_files_equal(snapshot_path, selected_path).map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "snapshot `{}` cannot compare {label} `{}` with selected generation artifact `{}`: {error}",
            snapshot_root.display(),
            snapshot_path.display(),
            selected_path.display(),
        ))
    })?;
    if !equal {
        return Err(SupervisorError::Config(format!(
            "snapshot `{}` {label} differs byte-for-byte from selected generation artifact `{}`; refusing restore",
            snapshot_root.display(),
            selected_path.display()
        )));
    }
    Ok(())
}
fn load_snapshot_metadata(root: &Path) -> Result<SnapshotMetadata> {
    let metadata_path = root.join("metadata.json");
    let bytes = read_snapshot_file_bounded(&metadata_path, SNAPSHOT_METADATA_MAX_BYTES_V1)
        .map_err(|err| {
            SupervisorError::Config(format!(
                "failed to read snapshot metadata `{}`: {err}",
                metadata_path.display()
            ))
        })?;
    let value: Value = json::from_slice(&bytes).map_err(|err| {
        SupervisorError::Config(format!(
            "failed to parse snapshot metadata `{}`: {err}",
            metadata_path.display()
        ))
    })?;
    let object = value.as_object().ok_or_else(|| {
        SupervisorError::Config(format!(
            "snapshot metadata `{}` must be a JSON object",
            metadata_path.display()
        ))
    })?;
    let chain_id = object
        .get("chain_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `chain_id` string",
                metadata_path.display()
            ))
        })?;
    let generation_id = object
        .get("generation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `generation_id` string",
                metadata_path.display()
            ))
        })?;
    let peer_count = object
        .get("peer_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `peer_count` number",
                metadata_path.display()
            ))
        })?;
    let storage_layout = object
        .get("storage_layout")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `storage_layout` string; legacy aggregate Kura snapshots cannot be restored safely",
                metadata_path.display()
            ))
        })?;
    if storage_layout != SNAPSHOT_STORAGE_LAYOUT {
        return Err(SupervisorError::Config(format!(
            "snapshot metadata `{}` uses unsupported storage layout `{storage_layout}`; expected `{SNAPSHOT_STORAGE_LAYOUT}`",
            metadata_path.display()
        )));
    }
    let genesis_hash = object
        .get("genesis_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `genesis_hash` string",
                metadata_path.display()
            ))
        })
        .and_then(|value| {
            Hash::from_str(value).map_err(|err| {
                SupervisorError::Config(format!(
                    "snapshot metadata `{}` has invalid `genesis_hash`: {err}",
                    metadata_path.display()
                ))
            })
        })?;
    let kura_hashes_value = object
        .get("kura_hashes")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `kura_hashes` map",
                metadata_path.display()
            ))
        })?;
    let mut kura_hashes = HashMap::new();
    for (alias, value) in kura_hashes_value {
        let hash_str = value.as_str().ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` contains non-string hash for `{alias}`",
                metadata_path.display()
            ))
        })?;
        let hash = Hash::from_str(hash_str).map_err(|err| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` has invalid hash for `{alias}`: {err}",
                metadata_path.display()
            ))
        })?;
        kura_hashes.insert(alias.clone(), hash);
    }
    Ok(SnapshotMetadata {
        chain_id: chain_id.to_owned(),
        generation_id: generation_id.to_owned(),
        peer_count,
        genesis_hash,
        kura_hashes,
    })
}
include!("supervisor/copy_helpers.rs");
include!("supervisor/snapshot_hash_helpers.rs");
#[cfg(test)]
mod tests;

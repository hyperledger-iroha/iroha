//! Telemetry integrity helpers for durable hash-chained records.

use iroha_config::parameters::actual::TelemetryIntegrity as TelemetryIntegrityConfig;
use norito::json::{Map, Value};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

const STATE_VERSION: u64 = 1;
const STATE_FILE_PREFIX: &str = "telemetry_integrity_";
const RECORD_HASH_DOMAIN: &[u8] = b"iroha.telemetry.record.v1";
const SINK_FINGERPRINT_DOMAIN: &[u8] = b"iroha.telemetry.sink.v1";
const RECORD_MAX_BYTES: usize = 256 * 1024;
// JSON string escaping can nearly double an already serialized record.
const STATE_FILE_MAX_BYTES: u64 = (RECORD_MAX_BYTES as u64 * 2) + 4096;

struct IntegrityConfig {
    enabled: bool,
    signing_key: Option<[u8; 32]>,
    signing_key_id: Option<String>,
}

impl From<TelemetryIntegrityConfig> for IntegrityConfig {
    fn from(config: TelemetryIntegrityConfig) -> Self {
        Self {
            enabled: config.enabled,
            signing_key: config.signing_key,
            signing_key_id: config.signing_key_id,
        }
    }
}

struct PendingRecord {
    bytes: Vec<u8>,
    hash: Option<[u8; 32]>,
    durably_staged: bool,
    output_confirmed: bool,
}

struct StateLock {
    _file: File,
}

/// Integrity state shared by one telemetry exporter.
pub(crate) struct ChainState {
    config: IntegrityConfig,
    seq: u64,
    prev_hash: [u8; 32],
    pending: Option<PendingRecord>,
    sink_fingerprint: [u8; 32],
    state_path: Option<PathBuf>,
    _state_lock: Option<StateLock>,
}

impl ChainState {
    /// Restore a chain from an explicit state path.
    pub(crate) fn new_with_state_path(
        config: TelemetryIntegrityConfig,
        state_path: Option<PathBuf>,
        kind: &str,
        sink_identity: &[u8],
    ) -> Result<Self, IntegrityError> {
        let state_lock = if config.enabled {
            state_path.as_deref().map(acquire_state_lock).transpose()?
        } else {
            None
        };
        let sink_fingerprint = compute_sink_fingerprint(kind, sink_identity);
        let mut chain = Self::from_config(config.into(), state_path, state_lock, sink_fingerprint);
        chain.load_state()?;
        Ok(chain)
    }

    /// Restore the chain used by the named exporter kind.
    pub(crate) fn new_with_kind(
        config: TelemetryIntegrityConfig,
        kind: &str,
        sink_identity: &[u8],
    ) -> Result<Self, IntegrityError> {
        let state_path = state_path_for(kind, config.state_dir.as_ref());
        Self::new_with_state_path(config, state_path, kind, sink_identity)
    }

    fn from_config(
        config: IntegrityConfig,
        state_path: Option<PathBuf>,
        state_lock: Option<StateLock>,
        sink_fingerprint: [u8; 32],
    ) -> Self {
        Self {
            config,
            seq: 1,
            prev_hash: [0_u8; 32],
            pending: None,
            sink_fingerprint,
            state_path,
            _state_lock: state_lock,
        }
    }

    /// Return the exact serialized record awaiting output confirmation.
    pub(crate) fn pending_record(&self) -> Option<&[u8]> {
        self.pending
            .as_ref()
            .map(|pending| pending.bytes.as_slice())
    }

    /// Return whether the pending bytes have a durable journal entry.
    pub(crate) fn pending_is_durable(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.durably_staged)
    }

    /// Return whether this process observed a successful output operation.
    pub(crate) fn pending_output_is_confirmed(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.output_confirmed)
    }

    /// Mark the pending record as successfully emitted by this process.
    pub(crate) fn confirm_pending_output(&mut self) -> Result<(), IntegrityError> {
        let pending = self
            .pending
            .as_mut()
            .ok_or(IntegrityError::NoPendingRecord)?;
        if !pending.durably_staged {
            return Err(IntegrityError::PendingRecordNotDurable);
        }
        pending.output_confirmed = true;
        Ok(())
    }

    /// Retry journaling a record whose initial staging write failed.
    pub(crate) async fn persist_pending(&mut self) -> Result<(), IntegrityError> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(IntegrityError::NoPendingRecord)?;
        if pending.durably_staged {
            return Ok(());
        }
        if self.config.enabled {
            self.persist_snapshot(self.seq, self.prev_hash, Some(&pending.bytes))
                .await?;
        }
        self.pending
            .as_mut()
            .expect("pending record cannot disappear during persistence")
            .durably_staged = true;
        Ok(())
    }

    /// Serialize and durably stage one record before it is emitted.
    ///
    /// A staged record must be committed or retried byte-for-byte. Refusing a
    /// second record prevents sequence reuse when output success is ambiguous.
    pub(crate) async fn stage_record(
        &mut self,
        mut map: Map,
        trailing_newline: bool,
    ) -> Result<(), IntegrityError> {
        if self.pending.is_some() {
            return Err(IntegrityError::PendingRecordExists);
        }

        let hash = if self.config.enabled {
            self.seq
                .checked_add(1)
                .ok_or(IntegrityError::SequenceExhausted)?;
            let payload = norito::json::to_vec(&map)?;
            let hash = compute_hash(self.prev_hash, self.seq, &payload);
            let signature = self
                .config
                .signing_key
                .map(|key| blake3::keyed_hash(&key, &hash));

            let mut chain = Map::new();
            chain.insert("seq".into(), Value::from(self.seq));
            chain.insert("prev_hash".into(), Value::from(hex::encode(self.prev_hash)));
            chain.insert("hash".into(), Value::from(hex::encode(hash)));
            if let Some(signature) = signature {
                chain.insert(
                    "signature".into(),
                    Value::from(hex::encode(signature.as_bytes())),
                );
                if let Some(key_id) = self.config.signing_key_id.as_ref() {
                    chain.insert("key_id".into(), Value::from(key_id.clone()));
                }
            }
            map.insert("chain".into(), Value::Object(chain));
            Some(hash)
        } else {
            None
        };

        let mut bytes = norito::json::to_vec(&map)?;
        if trailing_newline {
            bytes.push(b'\n');
        }
        if bytes.len() > RECORD_MAX_BYTES {
            return Err(IntegrityError::RecordTooLarge {
                actual: bytes.len(),
                limit: RECORD_MAX_BYTES,
            });
        }

        self.pending = Some(PendingRecord {
            bytes,
            hash,
            durably_staged: !self.config.enabled,
            output_confirmed: false,
        });
        self.persist_pending().await
    }

    /// Commit the staged record after its output operation succeeds.
    ///
    /// Durable state is replaced before in-memory state advances. On failure,
    /// the exact pending record remains available for restart or retry.
    pub(crate) async fn commit_pending(&mut self) -> Result<(), IntegrityError> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(IntegrityError::NoPendingRecord)?;
        if !pending.durably_staged {
            return Err(IntegrityError::PendingRecordNotDurable);
        }
        if !pending.output_confirmed {
            return Err(IntegrityError::OutputNotConfirmed);
        }

        if let Some(hash) = pending.hash {
            let next_seq = self
                .seq
                .checked_add(1)
                .ok_or(IntegrityError::SequenceExhausted)?;
            self.persist_snapshot(next_seq, hash, None).await?;
            self.seq = next_seq;
            self.prev_hash = hash;
        }
        self.pending = None;
        Ok(())
    }

    fn load_state(&mut self) -> Result<(), IntegrityError> {
        if !self.config.enabled {
            return Ok(());
        }
        let Some(path) = self.state_path.as_ref() else {
            return Ok(());
        };
        let snapshot = load_state_snapshot(path).map_err(|message| {
            IntegrityError::LoadState(format!("{}: {message}", path.display()))
        })?;
        let Some(snapshot) = snapshot else {
            return Ok(());
        };
        if snapshot.sink_fingerprint != self.sink_fingerprint {
            return Err(IntegrityError::LoadState(format!(
                "{}: state belongs to a different telemetry sink",
                path.display()
            )));
        }
        let pending = snapshot
            .pending_record
            .map(|bytes| {
                validate_pending_record(&self.config, snapshot.seq, snapshot.prev_hash, bytes)
            })
            .transpose()
            .map_err(|message| {
                IntegrityError::LoadState(format!("{}: {message}", path.display()))
            })?;
        self.seq = snapshot.seq;
        self.prev_hash = snapshot.prev_hash;
        self.pending = pending;
        Ok(())
    }

    async fn persist_snapshot(
        &self,
        seq: u64,
        prev_hash: [u8; 32],
        pending_record: Option<&[u8]>,
    ) -> Result<(), IntegrityError> {
        let Some(path) = self.state_path.as_ref() else {
            return Ok(());
        };
        persist_state_snapshot(path, self.sink_fingerprint, seq, prev_hash, pending_record)
            .await
            .map_err(|message| {
                IntegrityError::PersistState(format!("{}: {message}", path.display()))
            })
    }
}

#[derive(Debug, Error)]
pub(crate) enum IntegrityError {
    #[error("failed to serialize telemetry payload for integrity hash: {0}")]
    Serialize(#[from] norito::json::Error),
    #[error("telemetry integrity sequence exhausted")]
    SequenceExhausted,
    #[error("a telemetry record is already awaiting output confirmation")]
    PendingRecordExists,
    #[error("no telemetry record is awaiting output confirmation")]
    NoPendingRecord,
    #[error("the pending telemetry record is not durably staged")]
    PendingRecordNotDurable,
    #[error("the pending telemetry record has not been emitted successfully")]
    OutputNotConfirmed,
    #[error("telemetry record is {actual} bytes, exceeding the {limit}-byte limit")]
    RecordTooLarge { actual: usize, limit: usize },
    #[error("failed to load telemetry integrity state: {0}")]
    LoadState(String),
    #[error("failed to persist telemetry integrity state: {0}")]
    PersistState(String),
    #[cfg(not(unix))]
    #[error("telemetry integrity state persistence is unsupported on this platform")]
    UnsupportedStatePersistence,
    #[cfg(unix)]
    #[error("telemetry integrity state is already in use: {0}")]
    StateAlreadyInUse(String),
    #[cfg(unix)]
    #[error("telemetry integrity state custody check failed: {0}")]
    StateCustody(String),
}

#[derive(Debug)]
struct ChainStateSnapshot {
    sink_fingerprint: [u8; 32],
    seq: u64,
    prev_hash: [u8; 32],
    pending_record: Option<Vec<u8>>,
}

fn compute_hash(prev_hash: [u8; 32], seq: u64, payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RECORD_HASH_DOMAIN);
    hasher.update(&prev_hash);
    hasher.update(&seq.to_be_bytes());
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

fn compute_sink_fingerprint(kind: &str, sink_identity: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SINK_FINGERPRINT_DOMAIN);
    let kind_len = u64::try_from(kind.len()).expect("telemetry sink kind length must fit u64");
    hasher.update(&kind_len.to_be_bytes());
    hasher.update(kind.as_bytes());
    let identity_len =
        u64::try_from(sink_identity.len()).expect("telemetry sink identity length must fit u64");
    hasher.update(&identity_len.to_be_bytes());
    hasher.update(sink_identity);
    *hasher.finalize().as_bytes()
}

fn state_path_for(kind: &str, state_dir: Option<&PathBuf>) -> Option<PathBuf> {
    state_dir.map(|dir| dir.join(format!("{STATE_FILE_PREFIX}{kind}.json")))
}

#[cfg(unix)]
fn state_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(unix)]
fn acquire_state_lock(path: &Path) -> Result<StateLock, IntegrityError> {
    let directory =
        ensure_private_state_directory(state_parent(path)).map_err(IntegrityError::StateCustody)?;
    let lock_path = path.with_extension("lock");
    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            open_private_file(&lock_path, directory.uid(), true)
                .map_err(IntegrityError::StateCustody)?
                .ok_or_else(|| {
                    IntegrityError::StateCustody(format!(
                        "lock file disappeared while opening {}",
                        lock_path.display()
                    ))
                })?
        }
        Err(error) => {
            return Err(IntegrityError::StateCustody(format!(
                "failed to create {}: {error}",
                lock_path.display()
            )));
        }
    };
    validate_open_private_file(&lock_path, &file, directory.uid())
        .map_err(IntegrityError::StateCustody)?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => {
            IntegrityError::StateAlreadyInUse(path.display().to_string())
        }
        std::fs::TryLockError::Error(error) => {
            IntegrityError::StateCustody(format!("failed to lock {}: {error}", lock_path.display()))
        }
    })?;
    Ok(StateLock { _file: file })
}

#[cfg(not(unix))]
fn acquire_state_lock(_path: &Path) -> Result<StateLock, IntegrityError> {
    Err(IntegrityError::UnsupportedStatePersistence)
}

#[cfg(unix)]
fn ensure_private_state_directory(path: &Path) -> Result<std::fs::Metadata, String> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder
                .create(path)
                .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
        }
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", path.display()));
        }
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{} is not a regular directory", path.display()));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(format!(
            "{} grants group or other access; expected owner-only permissions",
            path.display()
        ));
    }
    let effective_uid = rustix::process::geteuid().as_raw();
    if metadata.uid() != effective_uid {
        return Err(format!(
            "{} is not owned by the current process user",
            path.display()
        ));
    }
    Ok(metadata)
}

#[cfg(unix)]
fn validate_private_file_metadata(
    path: &Path,
    metadata: &std::fs::Metadata,
    directory_uid: u32,
) -> Result<(), String> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if metadata.mode() & 0o777 != 0o600 {
        return Err(format!("{} must have mode 0600", path.display()));
    }
    if metadata.nlink() != 1 {
        return Err(format!(
            "{} has {} hard links; expected exactly one",
            path.display(),
            metadata.nlink()
        ));
    }
    if metadata.uid() != directory_uid {
        return Err(format!(
            "{} is not owned by the state directory owner",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_open_private_file(path: &Path, file: &File, directory_uid: u32) -> Result<(), String> {
    let path_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    validate_private_file_metadata(path, &path_metadata, directory_uid)?;
    let file_metadata = file
        .metadata()
        .map_err(|error| format!("failed to inspect open {}: {error}", path.display()))?;
    validate_private_file_metadata(path, &file_metadata, directory_uid)?;
    if path_metadata.dev() != file_metadata.dev() || path_metadata.ino() != file_metadata.ino() {
        return Err(format!(
            "{} changed while it was being opened",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn open_private_file(path: &Path, directory_uid: u32, write: bool) -> Result<Option<File>, String> {
    let path_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
    };
    validate_private_file_metadata(path, &path_metadata, directory_uid)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(write)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let file = options
        .open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    validate_open_private_file(path, &file, directory_uid)?;
    Ok(Some(file))
}

fn decode_hash(value: Option<&Value>, field: &str) -> Result<[u8; 32], String> {
    let encoded = value
        .and_then(Value::as_str)
        .ok_or_else(|| format!("state file missing {field}"))?;
    let decoded =
        hex::decode(encoded).map_err(|err| format!("state file {field} is not hex: {err}"))?;
    decoded.try_into().map_err(|decoded: Vec<u8>| {
        format!(
            "state file {field} must be 32 bytes (got {})",
            decoded.len()
        )
    })
}

#[cfg(unix)]
fn load_state_snapshot(path: &Path) -> Result<Option<ChainStateSnapshot>, String> {
    let file = {
        let directory = ensure_private_state_directory(state_parent(path))?;
        let Some(file) = open_private_file(path, directory.uid(), false)? else {
            return Ok(None);
        };
        file
    };
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to inspect state file: {err}"))?;
    if !metadata.is_file() {
        return Err("state path is not a regular file".to_string());
    }
    if metadata.len() > STATE_FILE_MAX_BYTES {
        return Err(format!(
            "state file is {} bytes, exceeding the {STATE_FILE_MAX_BYTES}-byte limit",
            metadata.len()
        ));
    }

    let mut bytes = Vec::new();
    let mut limited = file.take(STATE_FILE_MAX_BYTES.saturating_add(1));
    limited
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read state file: {err}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > STATE_FILE_MAX_BYTES {
        return Err(format!(
            "state file grew beyond the {STATE_FILE_MAX_BYTES}-byte limit while reading"
        ));
    }

    let value: Value = norito::json::from_slice(&bytes)
        .map_err(|err| format!("failed to decode state file: {err}"))?;
    let map = value
        .as_object()
        .ok_or_else(|| "state file payload is not an object".to_string())?;
    const REQUIRED_FIELDS: [&str; 5] = [
        "pending_record",
        "prev_hash",
        "seq",
        "sink_fingerprint",
        "version",
    ];
    if map.len() != REQUIRED_FIELDS.len()
        || REQUIRED_FIELDS
            .iter()
            .any(|field| !map.contains_key(*field))
    {
        return Err(
            "state file must contain exactly version, sink_fingerprint, seq, prev_hash, and pending_record"
                .to_string(),
        );
    }
    let version = map
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "state file missing version".to_string())?;
    if version != STATE_VERSION {
        return Err(format!(
            "unsupported state file version {version}, expected {STATE_VERSION}"
        ));
    }
    let sink_fingerprint = decode_hash(map.get("sink_fingerprint"), "sink_fingerprint")?;
    let seq = map
        .get("seq")
        .and_then(Value::as_u64)
        .ok_or_else(|| "state file missing seq".to_string())?;
    if seq == 0 {
        return Err("state file seq must be >= 1".to_string());
    }
    let prev_hash = decode_hash(map.get("prev_hash"), "prev_hash")?;
    let pending_record = match map.get("pending_record") {
        Some(Value::Null) => None,
        Some(Value::String(record)) => {
            if record.len() > RECORD_MAX_BYTES {
                return Err(format!(
                    "pending record is {} bytes, exceeding the {RECORD_MAX_BYTES}-byte limit",
                    record.len()
                ));
            }
            Some(record.as_bytes().to_vec())
        }
        Some(_) => return Err("state file pending_record must be a string or null".to_string()),
        None => return Err("state file missing pending_record".to_string()),
    };
    Ok(Some(ChainStateSnapshot {
        sink_fingerprint,
        seq,
        prev_hash,
        pending_record,
    }))
}

#[cfg(not(unix))]
fn load_state_snapshot(_path: &Path) -> Result<Option<ChainStateSnapshot>, String> {
    Err("durable state files are unsupported on this platform".to_string())
}

fn validate_pending_record(
    config: &IntegrityConfig,
    seq: u64,
    prev_hash: [u8; 32],
    bytes: Vec<u8>,
) -> Result<PendingRecord, String> {
    let value: Value = norito::json::from_slice(&bytes)
        .map_err(|err| format!("pending record is not valid JSON: {err}"))?;
    let mut record = match value {
        Value::Object(map) => map,
        _ => return Err("pending record is not an object".to_string()),
    };
    let chain = record
        .remove("chain")
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .ok_or_else(|| "pending record is missing its chain object".to_string())?;
    if chain.get("seq").and_then(Value::as_u64) != Some(seq) {
        return Err("pending record sequence does not match state".to_string());
    }
    let record_prev_hash = decode_hash(chain.get("prev_hash"), "pending prev_hash")?;
    if record_prev_hash != prev_hash {
        return Err("pending record previous hash does not match state".to_string());
    }
    let record_hash = decode_hash(chain.get("hash"), "pending hash")?;
    let payload = norito::json::to_vec(&record)
        .map_err(|err| format!("failed to serialize pending record payload: {err}"))?;
    let expected_hash = compute_hash(prev_hash, seq, &payload);
    if record_hash != expected_hash {
        return Err("pending record hash does not match its payload".to_string());
    }
    seq.checked_add(1)
        .ok_or_else(|| "pending record sequence is exhausted".to_string())?;

    let allowed_fields = ["seq", "prev_hash", "hash", "signature", "key_id"];
    if chain
        .keys()
        .any(|field| !allowed_fields.contains(&field.as_str()))
    {
        return Err("pending record chain contains an unknown field".to_string());
    }
    match config.signing_key {
        Some(key) => {
            let signature = decode_hash(chain.get("signature"), "pending signature")?;
            let expected = *blake3::keyed_hash(&key, &record_hash).as_bytes();
            if signature != expected {
                return Err("pending record signature is invalid".to_string());
            }
            match config.signing_key_id.as_deref() {
                Some(expected) if chain.get("key_id").and_then(Value::as_str) == Some(expected) => {
                }
                Some(_) => {
                    return Err("pending record key_id does not match configuration".to_string());
                }
                None if chain.contains_key("key_id") => {
                    return Err("pending record has an unexpected key_id".to_string());
                }
                None => {}
            }
        }
        None if chain.contains_key("signature") || chain.contains_key("key_id") => {
            return Err("unsigned pending record contains signature fields".to_string());
        }
        None => {}
    }

    Ok(PendingRecord {
        bytes,
        hash: Some(record_hash),
        durably_staged: true,
        output_confirmed: false,
    })
}

#[cfg(unix)]
async fn persist_state_snapshot(
    path: &Path,
    sink_fingerprint: [u8; 32],
    seq: u64,
    prev_hash: [u8; 32],
    pending_record: Option<&[u8]>,
) -> Result<(), String> {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(STATE_VERSION));
    map.insert(
        "sink_fingerprint".into(),
        Value::from(hex::encode(sink_fingerprint)),
    );
    map.insert("seq".into(), Value::from(seq));
    map.insert("prev_hash".into(), Value::from(hex::encode(prev_hash)));
    let pending_value = match pending_record {
        Some(bytes) => Value::String(
            std::str::from_utf8(bytes)
                .map_err(|err| format!("pending record is not UTF-8: {err}"))?
                .to_owned(),
        ),
        None => Value::Null,
    };
    map.insert("pending_record".into(), pending_value);
    let payload = norito::json::to_vec(&map)
        .map_err(|err| format!("failed to serialize state file: {err}"))?;
    if u64::try_from(payload.len()).unwrap_or(u64::MAX) > STATE_FILE_MAX_BYTES {
        return Err(format!(
            "serialized state exceeds the {STATE_FILE_MAX_BYTES}-byte limit"
        ));
    }

    let tmp_path = path.with_extension("tmp");
    let result = async {
        match tokio::fs::remove_file(&tmp_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("failed to clear stale temp state file: {error}"));
            }
        }
        let mut options = tokio::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&tmp_path)
            .await
            .map_err(|err| format!("failed to open temp state file: {err}"))?;
        file.write_all(&payload)
            .await
            .map_err(|err| format!("failed to write temp state file: {err}"))?;
        file.sync_all()
            .await
            .map_err(|err| format!("failed to sync temp state file: {err}"))?;
        drop(file);
        tokio::fs::rename(&tmp_path, path)
            .await
            .map_err(|err| format!("failed to atomically replace state file: {err}"))?;
        sync_parent_directory(path).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&tmp_path).await;
    }
    result
}

#[cfg(not(unix))]
async fn persist_state_snapshot(
    _path: &Path,
    _sink_fingerprint: [u8; 32],
    _seq: u64,
    _prev_hash: [u8; 32],
    _pending_record: Option<&[u8]>,
) -> Result<(), String> {
    Err("durable state files are unsupported on this platform".to_string())
}

#[cfg(unix)]
async fn sync_parent_directory(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let directory = tokio::fs::File::open(parent)
        .await
        .map_err(|err| format!("failed to open state directory for sync: {err}"))?;
    directory
        .sync_all()
        .await
        .map_err(|err| format!("failed to sync state directory: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_KIND: &str = "test";
    const TEST_SINK_IDENTITY: &[u8] = b"test-output";

    fn config(enabled: bool) -> TelemetryIntegrityConfig {
        TelemetryIntegrityConfig {
            enabled,
            state_dir: None,
            signing_key: None,
            signing_key_id: None,
        }
    }

    fn test_chain(
        config: TelemetryIntegrityConfig,
        state_path: Option<PathBuf>,
    ) -> Result<ChainState, IntegrityError> {
        ChainState::new_with_state_path(config, state_path, TEST_KIND, TEST_SINK_IDENTITY)
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "iroha-telemetry-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[cfg(unix)]
    fn create_private_dir(path: &Path) {
        use std::os::unix::fs::DirBuilderExt as _;

        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).expect("create private directory");
    }

    #[cfg(unix)]
    fn write_private_file(path: &Path, bytes: &[u8]) {
        use std::{io::Write as _, os::unix::fs::OpenOptionsExt as _};

        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .expect("create private file");
        file.write_all(bytes).expect("write private file");
    }

    fn map_with_message(message: &str) -> Map {
        let mut map = Map::new();
        map.insert("msg".into(), Value::from(message));
        map
    }

    fn pending_map(chain: &ChainState) -> Map {
        let value: Value =
            norito::json::from_slice(chain.pending_record().expect("record should be pending"))
                .expect("pending record JSON");
        match value {
            Value::Object(map) => map,
            _ => panic!("pending record must be an object"),
        }
    }

    #[test]
    fn record_hash_has_pinned_v1_domain() {
        assert_eq!(
            hex::encode(compute_hash([0_u8; 32], 1, b"{}")),
            "515b5ddc5d0f355ddd99db1ee8d05d40dd0415c3be8ce7e295d0e98636e8404f"
        );
    }

    #[tokio::test]
    async fn chain_increments_with_previous_hash() {
        let mut chain = test_chain(config(true), None).expect("initialize chain");
        let first = map_with_message("hello");
        let payload = norito::json::to_vec(&first).expect("payload");
        let expected_hash = compute_hash([0_u8; 32], 1, &payload);
        chain.stage_record(first, false).await.expect("stage first");
        let first = pending_map(&chain);
        let first_chain = first
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        assert_eq!(first_chain.get("seq").and_then(Value::as_u64), Some(1));
        let expected_hash_hex = hex::encode(expected_hash);
        assert_eq!(
            first_chain.get("hash").and_then(Value::as_str),
            Some(expected_hash_hex.as_str())
        );
        chain.confirm_pending_output().expect("confirm first");
        chain.commit_pending().await.expect("commit first");

        chain
            .stage_record(map_with_message("world"), false)
            .await
            .expect("stage second");
        let second = pending_map(&chain);
        let second_chain = second
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        assert_eq!(second_chain.get("seq").and_then(Value::as_u64), Some(2));
        assert_eq!(
            second_chain.get("prev_hash").and_then(Value::as_str),
            Some(expected_hash_hex.as_str())
        );
    }

    #[tokio::test]
    async fn pending_record_is_stable_until_committed() {
        let mut chain = test_chain(config(true), None).expect("initialize chain");
        chain
            .stage_record(map_with_message("first"), false)
            .await
            .expect("stage first");
        let staged = chain.pending_record().expect("pending").to_vec();
        assert!(matches!(
            chain.stage_record(map_with_message("second"), false).await,
            Err(IntegrityError::PendingRecordExists)
        ));
        assert_eq!(chain.pending_record(), Some(staged.as_slice()));
        chain.confirm_pending_output().expect("confirm");
        chain.commit_pending().await.expect("commit");
        assert!(chain.pending_record().is_none());
    }

    #[tokio::test]
    async fn sequence_exhaustion_fails_closed() {
        let mut chain = ChainState::from_config(
            config(true).into(),
            None,
            None,
            compute_sink_fingerprint(TEST_KIND, TEST_SINK_IDENTITY),
        );
        chain.seq = u64::MAX;
        assert!(matches!(
            chain.stage_record(Map::new(), false).await,
            Err(IntegrityError::SequenceExhausted)
        ));
        assert!(chain.pending_record().is_none());
    }

    #[tokio::test]
    async fn chain_includes_signature_when_keyed() {
        let key = [7_u8; 32];
        let keyed = TelemetryIntegrityConfig {
            enabled: true,
            state_dir: None,
            signing_key: Some(key),
            signing_key_id: Some("primary".to_string()),
        };
        let mut chain = test_chain(keyed, None).expect("initialize chain");
        let map = map_with_message("signed");
        let hash = compute_hash([0_u8; 32], 1, &norito::json::to_vec(&map).expect("payload"));
        chain.stage_record(map, false).await.expect("stage signed");
        let record = pending_map(&chain);
        let chain_map = record
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        let signature_hex = hex::encode(blake3::keyed_hash(&key, &hash).as_bytes());
        assert_eq!(
            chain_map.get("signature").and_then(Value::as_str),
            Some(signature_hex.as_str())
        );
        assert_eq!(
            chain_map.get("key_id").and_then(Value::as_str),
            Some("primary")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pending_record_survives_restart_byte_for_byte() {
        let dir = temp_dir("integrity-restart");
        let state_path = dir.join("telemetry_integrity_ws.json");
        let mut first =
            test_chain(config(true), Some(state_path.clone())).expect("initialize first chain");
        first
            .stage_record(map_with_message("first"), false)
            .await
            .expect("stage first");
        let staged = first.pending_record().expect("pending").to_vec();
        drop(first);

        let mut resumed = test_chain(config(true), Some(state_path.clone())).expect("resume chain");
        assert_eq!(resumed.pending_record(), Some(staged.as_slice()));
        resumed.confirm_pending_output().expect("confirm resumed");
        resumed.commit_pending().await.expect("commit resumed");
        drop(resumed);

        let mut next = test_chain(config(true), Some(state_path)).expect("restore committed chain");
        next.stage_record(map_with_message("second"), false)
            .await
            .expect("stage second");
        let record = pending_map(&next);
        assert_eq!(
            record
                .get("chain")
                .and_then(Value::as_object)
                .and_then(|chain| chain.get("seq"))
                .and_then(Value::as_u64),
            Some(2)
        );
        drop(next);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn oversized_state_file_is_rejected_before_json_decode() {
        let dir = temp_dir("integrity-oversized");
        create_private_dir(&dir);
        let state_path = dir.join("telemetry_integrity_ws.json");
        write_private_file(
            &state_path,
            &vec![b' '; usize::try_from(STATE_FILE_MAX_BYTES).expect("limit fits") + 1],
        );
        let error = load_state_snapshot(&state_path).expect_err("oversized state must fail");
        assert!(error.contains("exceeding"), "unexpected error: {error}");
        assert!(matches!(
            test_chain(config(true), Some(state_path)),
            Err(IntegrityError::LoadState(_))
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn obsolete_state_schema_is_rejected() {
        let dir = temp_dir("integrity-schema");
        create_private_dir(&dir);
        let state_path = dir.join("telemetry_integrity_ws.json");
        write_private_file(
            &state_path,
            br#"{"version":1,"seq":1,"prev_hash":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
        );
        assert!(matches!(
            test_chain(config(true), Some(state_path)),
            Err(IntegrityError::LoadState(_))
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_temp_write_preserves_committed_snapshot() {
        let dir = temp_dir("integrity-atomic");
        create_private_dir(&dir);
        let state_path = dir.join("telemetry_integrity_ws.json");
        persist_state_snapshot(
            &state_path,
            compute_sink_fingerprint(TEST_KIND, TEST_SINK_IDENTITY),
            1,
            [0_u8; 32],
            None,
        )
        .await
        .expect("write initial state");
        let original = tokio::fs::read(&state_path)
            .await
            .expect("read initial state");
        let tmp_path = state_path.with_extension("tmp");
        tokio::fs::create_dir(&tmp_path)
            .await
            .expect("block temp path with directory");
        assert!(
            persist_state_snapshot(
                &state_path,
                compute_sink_fingerprint(TEST_KIND, TEST_SINK_IDENTITY),
                2,
                [1_u8; 32],
                None,
            )
            .await
            .is_err()
        );
        assert_eq!(
            tokio::fs::read(&state_path)
                .await
                .expect("read preserved state"),
            original
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_commit_keeps_pending_record() {
        let dir = temp_dir("integrity-commit-failure");
        create_private_dir(&dir);
        let mut chain = test_chain(config(true), None).expect("initialize chain");
        chain
            .stage_record(map_with_message("pending"), false)
            .await
            .expect("stage record");
        let pending = chain.pending_record().expect("pending").to_vec();
        chain.confirm_pending_output().expect("confirm output");
        chain.state_path = Some(dir.clone());
        assert!(matches!(
            chain.commit_pending().await,
            Err(IntegrityError::PersistState(_))
        ));
        assert_eq!(chain.pending_record(), Some(pending.as_slice()));
        assert_eq!(chain.seq, 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_stage_persistence_keeps_exact_record_for_retry() {
        let dir = temp_dir("integrity-stage-failure");
        create_private_dir(&dir);
        let mut chain = ChainState::from_config(
            config(true).into(),
            Some(dir.clone()),
            None,
            compute_sink_fingerprint(TEST_KIND, TEST_SINK_IDENTITY),
        );
        assert!(matches!(
            chain.stage_record(map_with_message("pending"), false).await,
            Err(IntegrityError::PersistState(_))
        ));
        let pending = chain.pending_record().expect("pending").to_vec();
        assert!(!chain.pending_is_durable());
        chain.state_path = Some(dir.join("telemetry_integrity_ws.json"));
        chain.persist_pending().await.expect("retry persistence");
        assert!(chain.pending_is_durable());
        assert_eq!(chain.pending_record(), Some(pending.as_slice()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn state_path_has_one_exclusive_writer() {
        let dir = temp_dir("integrity-lock");
        let state_path = dir.join("telemetry_integrity_ws.json");
        let first =
            test_chain(config(true), Some(state_path.clone())).expect("acquire first state lock");
        assert!(matches!(
            test_chain(config(true), Some(state_path.clone())),
            Err(IntegrityError::StateAlreadyInUse(_))
        ));
        drop(first);
        let resumed =
            test_chain(config(true), Some(state_path)).expect("lock must be released on drop");
        drop(resumed);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn persisted_state_is_owner_only() {
        use std::os::unix::fs::MetadataExt as _;

        let dir = temp_dir("integrity-permissions");
        let state_path = dir.join("telemetry_integrity_ws.json");
        let lock_path = state_path.with_extension("lock");
        let mut chain =
            test_chain(config(true), Some(state_path.clone())).expect("initialize state");
        chain
            .stage_record(map_with_message("private"), false)
            .await
            .expect("persist state");
        assert_eq!(
            std::fs::metadata(&dir).expect("directory metadata").mode() & 0o077,
            0
        );
        assert_eq!(
            std::fs::metadata(&state_path)
                .expect("state metadata")
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&lock_path).expect("lock metadata").mode() & 0o777,
            0o600
        );
        drop(chain);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn persisted_state_is_bound_to_its_sink() {
        for has_pending_record in [false, true] {
            let dir = temp_dir(if has_pending_record {
                "integrity-sink-pending"
            } else {
                "integrity-sink-committed"
            });
            let state_path = dir.join("telemetry_integrity_ws.json");
            let mut first = ChainState::new_with_state_path(
                config(true),
                Some(state_path.clone()),
                "ws",
                b"wss://first.example/events",
            )
            .expect("initialize first sink");
            first
                .stage_record(map_with_message("bound"), false)
                .await
                .expect("persist first sink state");
            if !has_pending_record {
                first.confirm_pending_output().expect("confirm output");
                first.commit_pending().await.expect("commit state");
            }
            drop(first);

            assert!(matches!(
                ChainState::new_with_state_path(
                    config(true),
                    Some(state_path),
                    "ws",
                    b"wss://second.example/events",
                ),
                Err(IntegrityError::LoadState(_))
            ));
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[cfg(unix)]
    #[test]
    fn state_directory_with_broad_permissions_is_rejected() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = temp_dir("integrity-public-directory");
        std::fs::create_dir_all(&dir).expect("create directory");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("set broad permissions");
        let state_path = dir.join("telemetry_integrity_ws.json");
        assert!(matches!(
            test_chain(config(true), Some(state_path)),
            Err(IntegrityError::StateCustody(_))
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn state_file_with_noncanonical_mode_is_rejected() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = temp_dir("integrity-state-mode");
        create_private_dir(&dir);
        let state_path = dir.join("telemetry_integrity_ws.json");
        write_private_file(&state_path, b"{}");
        std::fs::set_permissions(&state_path, std::fs::Permissions::from_mode(0o700))
            .expect("set noncanonical state permissions");
        assert!(matches!(
            test_chain(config(true), Some(state_path)),
            Err(IntegrityError::LoadState(_))
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(not(unix))]
    #[test]
    fn persisted_state_is_rejected_without_directory_sync() {
        let state_path = temp_dir("integrity-unsupported").join("telemetry_integrity_ws.json");
        assert!(matches!(
            test_chain(config(true), Some(state_path)),
            Err(IntegrityError::UnsupportedStatePersistence)
        ));
    }

    #[test]
    fn state_path_for_kind_uses_prefix() {
        let dir = PathBuf::from("telemetry-state");
        let path = state_path_for("ws", Some(&dir)).expect("state path");
        assert_eq!(path, dir.join("telemetry_integrity_ws.json"));
    }
}

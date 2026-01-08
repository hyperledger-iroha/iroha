//! Disk-backed persistence for full RBC session state (chunks, ready votes).
//! Used to recover in-flight data availability transfers across restarts.

use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{block::BlockHeader, peer::PeerId};
use iroha_logger::prelude::*;
use norito::codec::{Decode, Encode};
use norito::{decode_from_bytes, to_bytes};
use sha2::{Digest as _, Sha256};

use crate::panic_hook;

/// Persisted metadata describing the node software that produced the snapshot.
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct SoftwareManifest {
    version: String,
    profile: String,
    git_commit: Option<String>,
}

pub(super) fn load_session_from_dir(
    dir: &Path,
    key: &SessionKey,
    expected_chain_hash: &Hash,
    expected_manifest: &SoftwareManifest,
) -> io::Result<Option<PersistedSession>> {
    let _suppressor = panic_hook::ScopedSuppressor::new();
    ChunkStore::load_session_from_dir(dir, key, expected_chain_hash, expected_manifest)
}

impl SoftwareManifest {
    /// Capture the build manifest for the currently running binary.
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),
            git_commit: option_env!("GIT_COMMIT_HASH").map(str::to_owned),
        }
    }

    /// Returns true when manifests are equivalent; missing commit hashes only match when both are
    /// absent to avoid accidentally mixing builds.
    pub fn matches(&self, other: &Self) -> bool {
        if self.version != other.version || self.profile != other.profile {
            return false;
        }
        match (&self.git_commit, &other.git_commit) {
            (Some(this), Some(that)) => this == that,
            (None, None) => true,
            _ => false,
        }
    }
}

/// Key identifying an RBC session `(block_hash, height, view)`.
pub type SessionKey = (HashOf<BlockHeader>, u64, u64);

/// Current pressure state of the chunk store after enforcing limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorePressure {
    /// Usage is below soft quotas.
    Normal {
        /// Number of sessions retained on disk.
        sessions: usize,
        /// Total payload bytes retained on disk.
        bytes: usize,
    },
    /// Soft quota exceeded; back-pressure should engage.
    SoftLimit {
        /// Number of sessions retained on disk.
        sessions: usize,
        /// Total payload bytes retained on disk.
        bytes: usize,
    },
    /// Hard limit enforcement removed entries; indicates immediate action required.
    HardLimit {
        /// Number of sessions retained on disk.
        sessions: usize,
        /// Total payload bytes retained on disk.
        bytes: usize,
    },
}

impl StorePressure {
    /// Number of persisted sessions after enforcement.
    pub fn sessions(&self) -> usize {
        match self {
            Self::Normal { sessions, .. }
            | Self::SoftLimit { sessions, .. }
            | Self::HardLimit { sessions, .. } => *sessions,
        }
    }

    /// Total persisted payload bytes after enforcement.
    pub fn bytes(&self) -> usize {
        match self {
            Self::Normal { bytes, .. }
            | Self::SoftLimit { bytes, .. }
            | Self::HardLimit { bytes, .. } => *bytes,
        }
    }

    /// Returns true if soft quota was breached.
    pub fn is_soft(&self) -> bool {
        matches!(self, Self::SoftLimit { .. })
    }

    /// Returns true if hard limit eviction occurred.
    pub fn is_hard(&self) -> bool {
        matches!(self, Self::HardLimit { .. })
    }
}

/// Result of loading persisted sessions from disk.
pub(super) struct LoadResult {
    /// Sessions that survived TTL/capacity enforcement.
    pub(super) sessions: Vec<PersistedSession>,
    /// Session keys removed while enforcing TTL/capacity/size constraints.
    pub(super) removed: Vec<SessionKey>,
    /// Pressure state after applying limits to the on-disk snapshot.
    pub(super) pressure: StorePressure,
}

/// Result of persisting a session snapshot.
#[derive(Debug)]
pub(super) struct PersistOutcome {
    /// Session keys removed while enforcing limits after persist.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) removed: Vec<SessionKey>,
    /// Pressure state after writing the session and compacting if needed.
    pub(super) pressure: StorePressure,
}

pub(super) const PERSIST_VERSION: u8 = 4;

fn persist_version_supported(version: u8) -> bool {
    version == PERSIST_VERSION
}

/// Disk-backed store for RBC sessions.
pub struct ChunkStore {
    dir: PathBuf,
    ttl: Duration,
    soft_sessions: usize,
    soft_bytes: usize,
    max_sessions: usize,
    max_bytes: usize,
}

impl ChunkStore {
    /// Construct a new chunk store rooted at `dir`.
    ///
    /// # Errors
    /// Returns an error if the backing directory cannot be created.
    pub fn new(
        dir: PathBuf,
        ttl: Duration,
        soft_sessions: usize,
        soft_bytes: usize,
        max_sessions: usize,
        max_bytes: usize,
    ) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        let soft_sessions = if max_sessions == 0 {
            0
        } else {
            soft_sessions.min(max_sessions)
        };
        let soft_bytes = if max_bytes == 0 {
            0
        } else {
            soft_bytes.min(max_bytes)
        };
        Ok(Self {
            dir,
            ttl,
            soft_sessions,
            soft_bytes,
            max_sessions,
            max_bytes,
        })
    }

    /// Load persisted sessions, pruning any that violate TTL/capacity/size caps.
    pub(super) fn load(
        &self,
        expected_chain_hash: &Hash,
        expected_manifest: &SoftwareManifest,
    ) -> io::Result<LoadResult> {
        let _suppressor = panic_hook::ScopedSuppressor::new();
        let entries = self.scan_entries(Some(expected_chain_hash), Some(expected_manifest))?;
        let outcome = self.enforce_limits(entries)?;
        let sessions = outcome
            .entries
            .into_iter()
            .map(|entry| entry.persisted)
            .collect();
        Ok(LoadResult {
            sessions,
            removed: outcome.removed,
            pressure: outcome.pressure,
        })
    }

    /// Persist a single session snapshot and enforce store limits.
    pub(super) fn persist_session(
        &self,
        key: SessionKey,
        session: &super::main_loop::RbcSession,
        chain_hash: &Hash,
        manifest: &SoftwareManifest,
        session_roster: &[PeerId],
    ) -> io::Result<PersistOutcome> {
        let _suppressor = panic_hook::ScopedSuppressor::new();
        if self.max_sessions == 0 || self.max_bytes == 0 {
            // Storage disabled; ensure any existing file is removed.
            let _ = self.remove(&key);
            return Ok(PersistOutcome {
                removed: Vec::new(),
                pressure: StorePressure::Normal {
                    sessions: 0,
                    bytes: 0,
                },
            });
        }
        let persisted = session.to_persisted(key, *chain_hash, manifest, session_roster);
        self.write_session(&persisted)?;
        let entries = self.scan_entries(Some(chain_hash), Some(manifest))?;
        let outcome = self.enforce_limits(entries)?;
        Ok(PersistOutcome {
            removed: outcome.removed,
            pressure: outcome.pressure,
        })
    }

    pub(super) fn persist_snapshot(
        &self,
        persisted: &PersistedSession,
    ) -> io::Result<PersistOutcome> {
        let _suppressor = panic_hook::ScopedSuppressor::new();
        if self.max_sessions == 0 || self.max_bytes == 0 {
            let _ = self.remove(&persisted.key());
            return Ok(PersistOutcome {
                removed: Vec::new(),
                pressure: StorePressure::Normal {
                    sessions: 0,
                    bytes: 0,
                },
            });
        }
        self.write_session(persisted)?;
        let entries = self.scan_entries(
            Some(&persisted.chain_hash),
            Some(&persisted.software_manifest),
        )?;
        let outcome = self.enforce_limits(entries)?;
        Ok(PersistOutcome {
            removed: outcome.removed,
            pressure: outcome.pressure,
        })
    }

    /// Remove a persisted session explicitly.
    ///
    /// # Errors
    /// Returns an error if the underlying filesystem operation fails for a reason other than a
    /// missing file.
    pub fn remove(&self, key: &SessionKey) -> io::Result<()> {
        let path = Self::make_session_path(&self.dir, key);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn write_session(&self, persisted: &PersistedSession) -> io::Result<()> {
        let path = Self::make_session_path(&self.dir, &persisted.key());
        let tmp = temp_session_path(&path);
        let encoded =
            to_bytes(persisted).map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(&encoded)?;
            file.sync_all()?;
        }
        if let Err(err) = fs::rename(&tmp, &path) {
            if err.kind() == io::ErrorKind::AlreadyExists {
                fs::remove_file(&path)?;
                fs::rename(&tmp, &path)?;
            } else {
                return Err(err);
            }
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let file = fs::File::open(parent)?;
                file.sync_all()?;
            }
        }
        Ok(())
    }

    fn is_session_file(path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(|os| os.to_str()) else {
            return false;
        };
        let Some(stem) = name.strip_suffix(".norito") else {
            return false;
        };
        // Session files are `{hash}_{height}_{view}`.
        stem.split('_').count() == 3
    }

    fn is_temp_session_file(path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(|os| os.to_str()) else {
            return false;
        };
        let Some(stem) = name.strip_suffix(".norito.tmp") else {
            return false;
        };
        stem.split('_').count() == 3
    }

    fn session_file_name(key: &SessionKey) -> String {
        let (hash, height, view) = key;
        let hex = hex::encode(hash.as_ref().as_ref());
        format!("{hex}_{height}_{view}.norito")
    }

    fn make_session_path(dir: &Path, key: &SessionKey) -> PathBuf {
        dir.join(Self::session_file_name(key))
    }

    /// Load a persisted session directly from `dir` without instantiating a [`ChunkStore`].
    /// Returns `Ok(None)` when the session file is absent or invalid (mismatched key/chain/manifest).
    pub(super) fn load_session_from_dir(
        dir: &Path,
        key: &SessionKey,
        expected_chain_hash: &Hash,
        expected_manifest: &SoftwareManifest,
    ) -> io::Result<Option<PersistedSession>> {
        let path = Self::make_session_path(dir, key);
        let tmp_path = temp_session_path(&path);
        let tmp_bytes = read_session_bytes(&tmp_path)?;
        let main_bytes = read_session_bytes(&path)?;
        if tmp_bytes.is_none() && main_bytes.is_none() {
            return Ok(None);
        }
        for (candidate_path, bytes) in [(&tmp_path, tmp_bytes), (&path, main_bytes)] {
            let Some(bytes) = bytes.as_deref() else {
                continue;
            };
            let Some(persisted) = Self::decode_persisted_session_guarded(bytes, candidate_path)
            else {
                continue;
            };
            let Some(persisted) = Self::validate_persisted_session(
                persisted,
                candidate_path,
                Some(expected_chain_hash),
                Some(expected_manifest),
            ) else {
                continue;
            };
            if candidate_path == &tmp_path {
                let _ = promote_temp_session(&tmp_path, &path);
            }
            return Ok(Some(persisted));
        }
        Ok(None)
    }

    fn scan_entries(
        &self,
        expected_chain_hash: Option<&Hash>,
        expected_manifest: Option<&SoftwareManifest>,
    ) -> io::Result<Vec<Entry>> {
        let mut out = Vec::new();
        let mut temp_paths = Vec::new();
        let mut main_paths = Vec::new();
        let read_dir = match fs::read_dir(&self.dir) {
            Ok(iter) => iter,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err),
        };
        for entry in read_dir {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    warn!(?err, dir=?self.dir, "failed to read entry in RBC chunk store");
                    continue;
                }
            };
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if Self::is_temp_session_file(&path) {
                temp_paths.push(path);
                continue;
            }
            if Self::is_session_file(&path) {
                main_paths.push(path);
            }
        }
        let mut seen = BTreeSet::new();
        for path in temp_paths {
            match fs::read(&path) {
                Ok(data) => {
                    let Some(persisted) = Self::decode_persisted_session_guarded(&data, &path)
                    else {
                        continue;
                    };
                    let Some(persisted) = Self::validate_persisted_session(
                        persisted,
                        &path,
                        expected_chain_hash,
                        expected_manifest,
                    ) else {
                        continue;
                    };
                    let key = persisted.key();
                    let mut effective_path = path.clone();
                    let main_path = path.with_extension("");
                    if promote_temp_session(&path, &main_path) {
                        effective_path = main_path;
                    }
                    if seen.insert(key) {
                        out.push(Entry {
                            persisted,
                            path: effective_path,
                        });
                    }
                }
                Err(err) => {
                    warn!(?err, ?path, "failed to read persisted RBC temp session");
                }
            }
        }
        for path in main_paths {
            match fs::read(&path) {
                Ok(data) => {
                    let Some(persisted) = Self::decode_persisted_session_guarded(&data, &path)
                    else {
                        continue;
                    };
                    let Some(persisted) = Self::validate_persisted_session(
                        persisted,
                        &path,
                        expected_chain_hash,
                        expected_manifest,
                    ) else {
                        continue;
                    };
                    let key = persisted.key();
                    if seen.insert(key) {
                        out.push(Entry { persisted, path });
                    }
                }
                Err(err) => {
                    warn!(?err, ?path, "failed to read persisted RBC session");
                }
            }
        }
        Ok(out)
    }

    fn validate_persisted_session(
        persisted: PersistedSession,
        path: &Path,
        expected_chain_hash: Option<&Hash>,
        expected_manifest: Option<&SoftwareManifest>,
    ) -> Option<PersistedSession> {
        if persisted.invalid {
            warn!(?path, "Skipping persisted RBC session marked invalid");
            let _ = Self::delete_path(path);
            return None;
        }
        if persisted.key_mismatch_with_path(path) {
            warn!(?path, "RBC persisted session key mismatch; removing file");
            let _ = Self::delete_path(path);
            return None;
        }
        if !persist_version_supported(persisted.format_version()) {
            warn!(
                ?path,
                version = persisted.format_version(),
                supported = PERSIST_VERSION,
                "Dropping RBC persisted session with unsupported format version"
            );
            let _ = Self::delete_path(path);
            return None;
        }
        if let Some(expected) = expected_chain_hash {
            if &persisted.chain_hash != expected {
                warn!(
                    ?path,
                    "Dropping RBC persisted session with mismatched chain hash"
                );
                let _ = Self::delete_path(path);
                return None;
            }
        }
        if let Some(expected) = expected_manifest {
            if !persisted.software_manifest.matches(expected) {
                warn!(
                    ?path,
                    "Dropping RBC persisted session with mismatched software manifest"
                );
                let _ = Self::delete_path(path);
                return None;
            }
        }
        if let Err(reason) = validate_chunks(&persisted) {
            warn!(
                ?path,
                %reason,
                "Dropping RBC persisted session due to chunk integrity failure"
            );
            let _ = Self::delete_path(path);
            return None;
        }
        Some(persisted)
    }

    fn decode_persisted_session_guarded(data: &[u8], path: &Path) -> Option<PersistedSession> {
        let result = panic_hook::with_hook_suppressed(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                decode_from_bytes::<PersistedSession>(data)
            }))
        });
        match result {
            Ok(Ok(persisted)) => Some(persisted),
            Ok(Err(err)) => {
                warn!(
                    ?err,
                    ?path,
                    "failed to decode persisted RBC session; removing file"
                );
                let _ = Self::delete_path(path);
                None
            }
            Err(panic) => {
                warn!(
                    ?path,
                    "panic while decoding persisted RBC session; dropping file"
                );
                if let Some(msg) = panic.downcast_ref::<&str>() {
                    debug!(?path, panic = %msg, "RBC decode panic message");
                } else if let Some(msg) = panic.downcast_ref::<String>() {
                    debug!(?path, panic = %msg, "RBC decode panic message");
                }
                let _ = Self::delete_path(path);
                None
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn enforce_limits(&self, mut entries: Vec<Entry>) -> io::Result<EnforceOutcome> {
        let mut removed = Vec::new();
        let mut hard_eviction = false;

        if self.max_sessions == 0 || self.max_bytes == 0 {
            for entry in entries {
                removed.push(entry.persisted.key());
                Self::delete_path(&entry.path)?;
            }
            return Ok(EnforceOutcome {
                entries: Vec::new(),
                removed,
                pressure: StorePressure::Normal {
                    sessions: 0,
                    bytes: 0,
                },
            });
        }

        if self.ttl > Duration::ZERO {
            let now = SystemTime::now();
            let mut retained = Vec::with_capacity(entries.len());
            for entry in std::mem::take(&mut entries) {
                let updated = entry.persisted.updated_at();
                match now.duration_since(updated) {
                    Ok(age) => {
                        if age > self.ttl {
                            removed.push(entry.persisted.key());
                            Self::delete_path(&entry.path)?;
                        } else {
                            retained.push(entry);
                        }
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            ?entry.path,
                            last_updated_ms = entry.persisted.last_updated_ms,
                            "dropping RBC persisted session with future timestamp"
                        );
                        removed.push(entry.persisted.key());
                        Self::delete_path(&entry.path)?;
                    }
                }
            }
            entries = retained;
        }

        entries.sort_by_key(|entry| entry.persisted.last_updated_ms);

        if self.max_sessions > 0 && entries.len() > self.max_sessions {
            let excess = entries.len() - self.max_sessions;
            let evicted: Vec<Entry> = entries.drain(..excess).collect();
            for entry in evicted {
                removed.push(entry.persisted.key());
                Self::delete_path(&entry.path)?;
            }
            hard_eviction = true;
        }

        let mut total_bytes: usize = entries
            .iter()
            .map(|entry| entry.persisted.payload_bytes_len())
            .sum();

        if self.max_bytes > 0 && total_bytes > self.max_bytes {
            while total_bytes > self.max_bytes && !entries.is_empty() {
                let entry = entries.remove(0);
                let freed = entry.persisted.payload_bytes_len();
                total_bytes = total_bytes.saturating_sub(freed);
                removed.push(entry.persisted.key());
                Self::delete_path(&entry.path)?;
            }
            hard_eviction = true;
        }

        let pressure = if hard_eviction {
            StorePressure::HardLimit {
                sessions: entries.len(),
                bytes: total_bytes,
            }
        } else if self.soft_triggered(entries.len(), total_bytes) {
            StorePressure::SoftLimit {
                sessions: entries.len(),
                bytes: total_bytes,
            }
        } else {
            StorePressure::Normal {
                sessions: entries.len(),
                bytes: total_bytes,
            }
        };

        if hard_eviction {
            warn!(
                dir=?self.dir,
                sessions=pressure.sessions(),
                bytes=pressure.bytes(),
                removed=removed.len(),
                "RBC chunk store exceeded hard limit; evicted sessions"
            );
        }

        Ok(EnforceOutcome {
            entries,
            removed,
            pressure,
        })
    }

    fn delete_path(path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn soft_triggered(&self, sessions: usize, bytes: usize) -> bool {
        (self.soft_sessions > 0 && sessions > self.soft_sessions)
            || (self.soft_bytes > 0 && bytes > self.soft_bytes)
    }
}

fn temp_session_path(path: &Path) -> PathBuf {
    path.with_added_extension("tmp")
}

fn read_session_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn promote_temp_session(tmp_path: &Path, main_path: &Path) -> bool {
    let promoted = match fs::rename(tmp_path, main_path) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            if let Err(remove_err) = fs::remove_file(main_path) {
                warn!(
                    ?remove_err,
                    ?main_path,
                    "failed to remove RBC session before temp promotion"
                );
                false
            } else if let Err(rename_err) = fs::rename(tmp_path, main_path) {
                warn!(
                    ?rename_err,
                    ?tmp_path,
                    "failed to promote RBC temp session after removal"
                );
                false
            } else {
                true
            }
        }
        Err(err) => {
            warn!(?err, ?tmp_path, "failed to promote RBC temp session");
            false
        }
    };

    if promoted {
        if let Some(parent) = main_path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(err) = fs::File::open(parent).and_then(|file| file.sync_all()) {
                    warn!(?err, ?parent, "failed to sync RBC session directory");
                }
            }
        }
    }

    promoted
}

struct EnforceOutcome {
    entries: Vec<Entry>,
    removed: Vec<SessionKey>,
    pressure: StorePressure,
}

struct Entry {
    persisted: PersistedSession,
    path: PathBuf,
}

/// Persisted representation of an RBC session.
#[derive(Clone, Debug, Encode, Decode)]
pub(super) struct PersistedSession {
    pub(crate) format_version: u8,
    pub(crate) chain_hash: Hash,
    pub(crate) software_manifest: SoftwareManifest,
    pub(crate) block_hash: HashOf<BlockHeader>,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) epoch: u64,
    pub(crate) total_chunks: u32,
    /// SHA-256 digests for each chunk, indexed by chunk position.
    #[norito(default)]
    pub(crate) chunk_digests: Vec<[u8; 32]>,
    pub(crate) payload_hash: Option<Hash>,
    pub(crate) expected_chunk_root: Option<Hash>,
    pub(crate) computed_chunk_root: Option<Hash>,
    pub(crate) invalid: bool,
    pub(crate) sent_ready: bool,
    pub(crate) ready_signatures: Vec<PersistedReady>,
    pub(crate) delivered: bool,
    pub(crate) deliver_sender: Option<u32>,
    pub(crate) deliver_signature: Option<Vec<u8>>,
    pub(crate) chunks: Vec<PersistedChunk>,
    pub(crate) last_updated_ms: u64,
    /// Commit topology snapshot captured when this RBC session started.
    #[norito(default)]
    pub(crate) session_roster: Vec<PeerId>,
}

impl PersistedSession {
    /// Session key `(block_hash, height, view)`
    pub fn key(&self) -> SessionKey {
        (self.block_hash, self.height, self.view)
    }

    pub fn format_version(&self) -> u8 {
        self.format_version
    }

    fn key_mismatch_with_path(&self, path: &Path) -> bool {
        // Try to ensure filenames roughly align with key. Mismatch is non-fatal but aids debugging.
        path.file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|stem| {
                let expected_hex = hex::encode(self.block_hash.as_ref().as_ref());
                !stem.starts_with(&expected_hex)
            })
    }

    /// Wall-clock `SystemTime` when the session was last updated.
    pub fn updated_at(&self) -> SystemTime {
        ms_to_system_time(self.last_updated_ms)
    }

    /// Total payload bytes captured in this session.
    pub fn payload_bytes_len(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.bytes.len()).sum()
    }
}

/// Persisted RBC chunk representation (index + bytes).
#[derive(Clone, Debug, Encode, Decode)]
pub(super) struct PersistedChunk {
    pub(crate) idx: u32,
    pub(crate) bytes: Vec<u8>,
}

/// Persisted READY signature metadata.
#[derive(Clone, Debug, Encode, Decode)]
pub(super) struct PersistedReady {
    pub(crate) sender: u32,
    pub(crate) signature: Vec<u8>,
}

fn ms_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

fn validate_chunks(session: &PersistedSession) -> Result<(), &'static str> {
    let expected = session.total_chunks as usize;
    if expected == 0 {
        if !session.chunk_digests.is_empty() {
            return Err("chunk digest count mismatch");
        }
        return if session.chunks.is_empty() {
            Ok(())
        } else {
            Err("non-empty chunk list with zero expected chunks")
        };
    }
    if !session.chunk_digests.is_empty() && session.chunk_digests.len() != expected {
        return Err("chunk digest count mismatch");
    }
    if session.chunks.len() > expected {
        return Err("too many chunks");
    }

    let mut chunks: Vec<&PersistedChunk> = session.chunks.iter().collect();
    chunks.sort_by_key(|chunk| chunk.idx);

    for window in chunks.windows(2) {
        if window[0].idx == window[1].idx {
            return Err("duplicate chunk index");
        }
    }

    for chunk in &chunks {
        if (chunk.idx as usize) >= expected {
            return Err("chunk index exceeds expected count");
        }
    }

    let mut ready_seen = BTreeSet::new();
    for ready in &session.ready_signatures {
        if ready.signature.is_empty() {
            return Err("empty READY signature");
        }
        if !ready_seen.insert(ready.sender) {
            return Err("duplicate READY sender");
        }
    }

    if !session.session_roster.is_empty() {
        let roster_len = session.session_roster.len();
        for ready in &session.ready_signatures {
            if ready.sender as usize >= roster_len {
                return Err("READY sender exceeds roster length");
            }
        }
        if let Some(sender) = session.deliver_sender {
            if sender as usize >= roster_len {
                return Err("DELIVER sender exceeds roster length");
            }
        }
    }

    if session.delivered {
        match (&session.deliver_sender, &session.deliver_signature) {
            (Some(_), Some(sig)) if !sig.is_empty() => {}
            _ => return Err("delivered flag set without deliver sender/signature"),
        }
    }

    if let Some(expected_hash) = &session.payload_hash {
        if session.chunks.len() == expected {
            let total_len: usize = chunks.iter().map(|chunk| chunk.bytes.len()).sum();
            let mut bytes = Vec::with_capacity(total_len);
            for chunk in &chunks {
                bytes.extend_from_slice(&chunk.bytes);
            }
            let calculated = Hash::new(&bytes);
            if &calculated != expected_hash {
                return Err("payload hash mismatch");
            }
        }
    }

    if expected > 0 && session.chunks.len() == expected {
        let mut digests = Vec::with_capacity(expected);
        for chunk in &chunks {
            let digest = Sha256::digest(&chunk.bytes);
            let mut hashed = [0u8; 32];
            hashed.copy_from_slice(&digest);
            digests.push(hashed);
        }
        if !session.chunk_digests.is_empty() && session.chunk_digests != digests {
            return Err("chunk digest mismatch");
        }
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests);
        let Some(root) = tree.root().map(Hash::from) else {
            return Err("failed to compute chunk root");
        };
        if let Some(expected_root) = &session.expected_chunk_root {
            if expected_root != &root {
                return Err("chunk root mismatch");
            }
        }
        if let Some(computed_root) = &session.computed_chunk_root {
            if computed_root != &root {
                return Err("computed chunk root mismatch");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{block::BlockHeader, peer::PeerId};
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::main_loop::RbcSession;

    fn session_key(id: u8) -> SessionKey {
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([id; 32]));
        (hash, 1, 0)
    }

    fn test_chain_hash() -> Hash {
        Hash::prehashed([0xAB; 32])
    }

    fn test_manifest() -> SoftwareManifest {
        SoftwareManifest::current()
    }

    fn test_peer_id(seed: u8) -> PeerId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        PeerId::new(key_pair.public_key().clone())
    }

    fn sample_persisted_session(
        key: SessionKey,
        chain_hash: Hash,
        manifest: SoftwareManifest,
    ) -> PersistedSession {
        PersistedSession {
            format_version: PERSIST_VERSION,
            chain_hash,
            software_manifest: manifest,
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: 0,
            total_chunks: 0,
            chunk_digests: Vec::new(),
            payload_hash: None,
            expected_chunk_root: None,
            computed_chunk_root: None,
            invalid: false,
            sent_ready: false,
            ready_signatures: Vec::new(),
            delivered: false,
            deliver_sender: None,
            deliver_signature: None,
            chunks: Vec::new(),
            last_updated_ms: 0,
            session_roster: Vec::new(),
        }
    }

    /// Debug helper: set `RBC_SESSION_PATH` to a persisted session file to validate it.
    /// Ignored by default so it does not run in CI.
    #[test]
    #[ignore = "debug helper for inspecting persisted sessions"]
    fn debug_validate_external_session() {
        let path = std::env::var("RBC_SESSION_PATH").expect("set RBC_SESSION_PATH");
        let data = fs::read(&path).expect("read session file");
        let persisted =
            decode_from_bytes::<PersistedSession>(&data).expect("decode persisted session");
        match validate_chunks(&persisted) {
            Ok(()) => println!("validate_chunks: ok"),
            Err(reason) => println!("validate_chunks failed: {reason}"),
        }
    }

    #[test]
    fn temp_session_path_preserves_existing_extension() {
        let path = Path::new("/var/lib/iroha/rbc/session_a.norito");
        let tmp = temp_session_path(path);
        assert_eq!(tmp, Path::new("/var/lib/iroha/rbc/session_a.norito.tmp"));
    }

    #[test]
    fn software_manifest_matches_handles_missing_commit_hashes() {
        let with_commit = SoftwareManifest {
            version: "1.0.0".into(),
            profile: "release".into(),
            git_commit: Some("abcdef".into()),
        };
        let missing_commit = SoftwareManifest {
            version: "1.0.0".into(),
            profile: "release".into(),
            git_commit: None,
        };

        assert!(!with_commit.matches(&missing_commit));
        assert!(!missing_commit.matches(&with_commit));
        assert!(missing_commit.matches(&missing_commit));
    }

    #[test]
    fn software_manifest_matches_accepts_identical_builds() {
        let manifest = SoftwareManifest {
            version: "2.1.0".into(),
            profile: "debug".into(),
            git_commit: Some("123456".into()),
        };
        let clone = manifest.clone();
        assert!(manifest.matches(&clone));
    }

    #[test]
    fn truncated_session_is_removed_without_panic() {
        let dir = tempdir().unwrap();
        let key = session_key(9);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(60),
            4,
            1024,
            8,
            4096,
        )
        .expect("chunk store init");

        let chain_hash = test_chain_hash();
        let manifest = test_manifest();

        let persisted = PersistedSession {
            format_version: PERSIST_VERSION,
            chain_hash,
            software_manifest: manifest.clone(),
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: 0,
            total_chunks: 0,
            chunk_digests: Vec::new(),
            payload_hash: None,
            expected_chunk_root: None,
            computed_chunk_root: None,
            invalid: false,
            sent_ready: false,
            ready_signatures: Vec::new(),
            delivered: false,
            deliver_sender: None,
            deliver_signature: None,
            chunks: Vec::new(),
            last_updated_ms: 0,
            session_roster: Vec::new(),
        };
        let mut encoded = to_bytes(&persisted).expect("encode persisted session");
        assert!(encoded.len() > 8);
        encoded.truncate(encoded.len() - 8);

        let path = ChunkStore::make_session_path(dir.path(), &key);
        fs::write(&path, &encoded).expect("write truncated persisted session");

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert!(
            load.sessions.is_empty(),
            "invalid session should be dropped"
        );
        assert!(
            !path.exists(),
            "store should delete corrupt persisted files during load"
        );
    }

    #[test]
    fn temp_session_promotes_on_load() {
        let dir = tempdir().unwrap();
        let key = session_key(10);
        let store = ChunkStore::new(dir.path().to_path_buf(), Duration::ZERO, 4, 1024, 8, 4096)
            .expect("chunk store init");

        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let persisted = sample_persisted_session(key, chain_hash, manifest.clone());
        let encoded = to_bytes(&persisted).expect("encode persisted session");

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&tmp_path, &encoded).expect("write temp session");

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert_eq!(load.sessions.len(), 1, "temp session should load");
        assert!(path.exists(), "temp session should be promoted");
        assert!(!tmp_path.exists(), "temp session should be removed");
    }

    #[test]
    fn ttl_evicts_future_timestamp_sessions() {
        let dir = tempdir().unwrap();
        let key = session_key(11);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(60),
            4,
            1024,
            8,
            4096,
        )
        .expect("chunk store init");

        let chain_hash = test_chain_hash();
        let manifest = SoftwareManifest {
            version: "1.0.0".into(),
            profile: "test".into(),
            git_commit: Some("deadbeef".into()),
        };
        let mut persisted = sample_persisted_session(key, chain_hash, manifest.clone());
        let future_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .saturating_add(120_000);
        persisted.last_updated_ms = u64::try_from(future_ms).unwrap_or(u64::MAX);
        let encoded = to_bytes(&persisted).expect("encode persisted session");

        let path = ChunkStore::make_session_path(dir.path(), &key);
        fs::write(&path, &encoded).expect("write persisted session");

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert!(load.sessions.is_empty());
        assert!(
            !path.exists(),
            "future timestamp sessions should be evicted"
        );
    }

    #[test]
    fn load_session_from_dir_falls_back_to_main_when_temp_invalid() {
        let dir = tempdir().unwrap();
        let key = session_key(12);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let persisted = sample_persisted_session(key, chain_hash, manifest.clone());
        let encoded = to_bytes(&persisted).expect("encode persisted session");

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&path, &encoded).expect("write main session");
        fs::write(&tmp_path, b"corrupt").expect("write corrupt temp session");

        let loaded = ChunkStore::load_session_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("load session from dir");
        assert!(loaded.is_some(), "main session should load");
        assert!(path.exists(), "main session should remain");
        assert!(!tmp_path.exists(), "corrupt temp session should be removed");
    }

    #[test]
    fn non_session_files_are_ignored() {
        let dir = tempdir().unwrap();
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(60),
            4,
            1024,
            8,
            4096,
        )
        .expect("chunk store init");

        let status_file = dir.path().join("sessions.norito");
        fs::write(&status_file, b"status-placeholder").expect("write status snapshot");

        let chain_hash = test_chain_hash();
        let manifest = test_manifest();

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert!(
            load.sessions.is_empty(),
            "status snapshot should be ignored by session loader"
        );
        assert!(
            status_file.exists(),
            "status snapshot must not be deleted by chunk store"
        );
    }

    #[test]
    fn scan_entries_falls_back_to_main_when_temp_invalid() {
        let dir = tempdir().unwrap();
        let store = ChunkStore::new(dir.path().to_path_buf(), Duration::ZERO, 4, 1024, 8, 4096)
            .expect("chunk store init");

        let key = session_key(13);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let persisted = sample_persisted_session(key, chain_hash, manifest.clone());
        let encoded = to_bytes(&persisted).expect("encode persisted session");
        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&path, &encoded).expect("write main session");
        fs::write(&tmp_path, b"corrupt").expect("write corrupt temp session");

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert_eq!(load.sessions.len(), 1, "main session should load");
        assert!(path.exists(), "main session should remain");
        assert!(!tmp_path.exists(), "corrupt temp session should be removed");
    }

    #[test]
    fn load_session_from_dir_promotes_temp() {
        let dir = tempdir().unwrap();
        let key = session_key(11);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let persisted = sample_persisted_session(key, chain_hash, manifest.clone());
        let encoded = to_bytes(&persisted).expect("encode persisted session");

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&tmp_path, &encoded).expect("write temp session");

        let loaded = ChunkStore::load_session_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("load session from dir");
        assert!(loaded.is_some(), "temp session should load");
        assert!(path.exists(), "temp session should be promoted");
        assert!(!tmp_path.exists(), "temp session should be removed");
    }

    #[test]
    fn persisted_session_roundtrip_marks_recovered() {
        let dir = tempdir().unwrap();
        let key = session_key(1);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            1 << 19,
            4,
            1 << 20,
        )
        .expect("chunk store init");

        let mut session = RbcSession::test_new(3, None, None, 0);
        session.test_note_chunk(0, vec![1, 2, 3], 0);
        session.test_note_chunk(1, vec![4, 5, 6], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let roster = vec![test_peer_id(1), test_peer_id(2)];
        let outcome = store
            .persist_session(key, &session, &chain_hash, &manifest, &roster)
            .expect("persist session");
        assert!(outcome.removed.is_empty());
        assert!(matches!(
            outcome.pressure,
            StorePressure::Normal { sessions, .. } if sessions == 1
        ));

        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            1 << 19,
            4,
            1 << 20,
        )
        .expect("chunk store re-init");
        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert!(load.removed.is_empty());
        assert!(matches!(load.pressure, StorePressure::Normal { sessions, .. } if sessions == 1));
        assert_eq!(load.sessions.len(), 1);
        let persisted = load.sessions.into_iter().next().unwrap();
        assert_eq!(persisted.key(), key);
        assert_eq!(persisted.session_roster, roster);
        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert!(rebuilt.recovered_from_disk());
        assert_eq!(rebuilt.received_chunks(), 2);
    }

    #[test]
    fn persisted_incomplete_session_survives_reload() {
        let dir = tempdir().unwrap();
        let key = session_key(7);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            1 << 19,
            4,
            1 << 20,
        )
        .expect("chunk store init");

        let mut session = RbcSession::test_new(3, None, None, 0);
        session.test_note_chunk(0, vec![42, 24], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();

        let outcome = store
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist session");
        assert!(outcome.removed.is_empty());

        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            1 << 19,
            4,
            1 << 20,
        )
        .expect("chunk store re-init");
        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert!(load.removed.is_empty());
        assert_eq!(load.sessions.len(), 1);
        let persisted = load.sessions.into_iter().next().expect("session persisted");
        assert_eq!(
            persisted.chunks.len(),
            1,
            "partial chunk set should be retained"
        );
        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert!(rebuilt.recovered_from_disk());
        assert_eq!(rebuilt.total_chunks(), 3);
        assert_eq!(rebuilt.received_chunks(), 1);
    }

    #[test]
    fn persisted_session_adopts_computed_chunk_root_when_expected_missing() {
        let key = session_key(8);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let roster = vec![test_peer_id(1)];

        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, vec![1u8, 2, 3], 0);
        session.test_note_chunk(1, vec![4u8, 5, 6], 0);
        let computed_root = session.chunk_root().expect("chunk root");

        let persisted = session.to_persisted(key, chain_hash, &manifest, &roster);
        assert!(persisted.expected_chunk_root.is_none());
        assert_eq!(persisted.computed_chunk_root, Some(computed_root));

        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        let roundtrip = rebuilt.to_persisted(key, chain_hash, &manifest, &roster);
        assert_eq!(roundtrip.expected_chunk_root, Some(computed_root));
    }

    #[test]
    fn persisted_invalid_session_roundtrips_as_invalid() {
        let key = session_key(9);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let roster = vec![test_peer_id(1)];

        let mut session = RbcSession::test_new(1, None, None, 0);
        session.record_ready(1, vec![0xAA]);
        session.record_ready(1, vec![0xBB]);
        assert!(session.is_invalid());

        let persisted = session.to_persisted(key, chain_hash, &manifest, &roster);
        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert!(rebuilt.is_invalid());
        assert!(rebuilt.recovered_from_disk());
    }

    #[test]
    fn persisted_session_with_chunk_root_mismatch_is_dropped() {
        let dir = tempdir().unwrap();
        let key = session_key(11);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            1 << 19,
            4,
            1 << 20,
        )
        .expect("chunk store init");

        let chunk0 = vec![1u8, 2, 3];
        let chunk1 = vec![4u8, 5, 6];
        let mut payload = Vec::new();
        payload.extend_from_slice(&chunk0);
        payload.extend_from_slice(&chunk1);
        let payload_hash = Hash::new(&payload);

        let mut session = RbcSession::test_new(2, Some(payload_hash), None, 0);
        session.test_note_chunk(0, chunk0, 0);
        session.test_note_chunk(1, chunk1, 0);

        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let roster = vec![test_peer_id(1)];
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &roster);
        persisted.expected_chunk_root = Some(Hash::prehashed([0xEE; 32]));

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let encoded = to_bytes(&persisted).expect("encode persisted session");
        fs::write(&path, &encoded).expect("write persisted session");

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert!(load.sessions.is_empty(), "invalid root should be dropped");
        assert!(!path.exists(), "store should delete invalid session files");
    }

    #[test]
    fn store_retains_delivered_chunk_bytes_for_sampling() {
        let dir = tempdir().unwrap();
        let key = session_key(2);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            48,
            4,
            1 << 20,
        )
        .expect("chunk store init");

        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, vec![1u8; 32], 0);
        session.test_note_chunk(1, vec![2u8; 32], 0);
        session.test_set_sent_ready(true);
        // Mimic a real deliver event so persisted snapshots include the metadata required
        // by `validate_chunks`.
        session.record_deliver(0, vec![0xAA; 64]);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();

        let outcome = store
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist session");
        match outcome.pressure {
            StorePressure::SoftLimit { sessions, bytes } => {
                assert_eq!(sessions, 1);
                assert!(
                    bytes >= 64,
                    "stored payload bytes should remain available for sampling"
                );
            }
            other => panic!("unexpected pressure level: {other:?}"),
        }

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load after persistence");
        assert!(load.removed.is_empty());
        assert!(matches!(
            load.pressure,
            StorePressure::SoftLimit { sessions, bytes } if sessions == 1 && bytes >= 64
        ));
        let persisted = load.sessions.into_iter().next().expect("session persisted");
        assert!(
            persisted.chunks.len() == 2
                && persisted.chunks.iter().all(|chunk| !chunk.bytes.is_empty()),
            "persisted session should retain chunk bytes for sampling"
        );
    }

    #[test]
    fn soft_limit_without_delivered_sessions_signals_pressure() {
        let dir = tempdir().unwrap();
        let key = session_key(3);
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(120),
            2,
            24,
            4,
            1 << 20,
        )
        .expect("chunk store init");

        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![9u8; 32], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let outcome = store
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist session");
        assert!(matches!(
            outcome.pressure,
            StorePressure::SoftLimit { sessions, bytes } if sessions == 1 && bytes == 32
        ));
    }

    #[test]
    fn from_persisted_rejects_duplicate_chunk_indices() {
        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![1, 2, 3], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(4);
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        persisted.chunks.push(PersistedChunk {
            idx: 0,
            bytes: vec![9, 9, 9],
        });
        let err = RbcSession::from_persisted_unchecked(&persisted);
        assert!(matches!(
            err,
            Err(crate::sumeragi::main_loop::PersistedLoadError::DuplicateChunkIndex(0))
        ));
    }

    #[test]
    fn from_persisted_accepts_incomplete_chunk_set() {
        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, vec![7, 7, 7], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(5);
        let persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert_eq!(rebuilt.total_chunks(), 2);
        assert_eq!(rebuilt.received_chunks(), 1);
    }

    #[test]
    fn from_persisted_allows_delivered_without_chunk_bytes() {
        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, vec![5, 5, 5], 0);
        session.test_note_chunk(1, vec![6, 6, 6], 0);
        session.test_set_delivered(true);
        session.test_set_sent_ready(true);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(8);
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        persisted.chunks.clear();
        persisted.delivered = true;
        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert_eq!(rebuilt.total_chunks(), 2);
        assert_eq!(rebuilt.received_chunks(), 0);
    }

    #[test]
    fn from_persisted_rejects_payload_hash_mismatch() {
        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![1, 2, 3, 4], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(6);
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        persisted.payload_hash = Some(Hash::prehashed([0xAA; 32]));
        let err = RbcSession::from_persisted_unchecked(&persisted);
        assert!(matches!(
            err,
            Err(crate::sumeragi::main_loop::PersistedLoadError::PayloadHashMismatch)
        ));
    }
}

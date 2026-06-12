//! Disk-backed persistence for full RBC session state (chunks, ready votes).
//! Used to recover in-flight data availability transfers across restarts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    block::{BlockHeader, BlockSignature, consensus::RbcEncoding},
    peer::PeerId,
};
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

/// Metadata extracted from a persisted RBC session snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistedSessionMetadata {
    /// Block hash associated with the RBC session.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height associated with the RBC session.
    pub height: u64,
    /// Consensus view associated with the RBC session.
    pub view: u64,
    /// Total chunk count advertised by the persisted session.
    pub total_chunks: u32,
    /// Number of chunk payloads still retained in the persisted file.
    pub persisted_chunk_count: u32,
    /// Whether the persisted session had already observed DELIVER.
    pub delivered: bool,
    /// Whether the persisted session was marked invalid.
    pub invalid: bool,
}

/// Load validated metadata for a single persisted session directly from `dir`.
///
/// Returns `Ok(None)` when the session file is absent or fails the same chain/manifest guards
/// used by restart recovery.
pub fn load_session_metadata_from_dir(
    dir: &Path,
    key: &SessionKey,
    expected_chain_hash: &Hash,
    expected_manifest: &SoftwareManifest,
) -> io::Result<Option<PersistedSessionMetadata>> {
    let _suppressor = panic_hook::ScopedSuppressor::new();
    Ok(
        load_session_from_dir(dir, key, expected_chain_hash, expected_manifest)?
            .map(|persisted| persisted_session_metadata(&persisted)),
    )
}

/// Inspect persisted session metadata without validating the software manifest or deleting files.
///
/// Use this for tests and observability probes that run outside the peer process. Restart recovery
/// must continue using [`load_session_metadata_from_dir`] or `load_session_from_dir`, which apply
/// the full manifest guard before accepting a snapshot.
pub fn inspect_session_metadata_from_dir(
    dir: &Path,
    key: &SessionKey,
    expected_chain_hash: &Hash,
) -> io::Result<Option<PersistedSessionMetadata>> {
    let _suppressor = panic_hook::ScopedSuppressor::new();
    ChunkStore::inspect_session_metadata_from_dir(dir, key, expected_chain_hash)
}

fn persisted_session_metadata(persisted: &PersistedSession) -> PersistedSessionMetadata {
    PersistedSessionMetadata {
        block_hash: persisted.block_hash,
        height: persisted.height,
        view: persisted.view,
        total_chunks: persisted.total_chunks,
        persisted_chunk_count: u32::try_from(persisted.chunks.len()).unwrap_or(u32::MAX),
        delivered: persisted.delivered,
        invalid: persisted.invalid,
    }
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

pub(super) const PERSIST_VERSION: u8 = 5;

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
        let encoded = to_bytes(persisted).map_err(io::Error::other)?;
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
        let mut selected: Option<CandidateEntry> = None;
        for (candidate_path, main_path, is_temp, bytes) in [
            (&tmp_path, &path, true, tmp_bytes),
            (&path, &path, false, main_bytes),
        ] {
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
            let candidate = CandidateEntry {
                persisted,
                path: candidate_path.to_path_buf(),
                main_path: main_path.to_path_buf(),
                is_temp,
            };
            if Self::candidate_newer_than_selected(&candidate, selected.as_ref()) {
                selected = Some(candidate);
            } else if candidate.is_temp {
                let _ = Self::delete_path(&candidate.path);
            }
        }
        let Some(selected) = selected else {
            return Ok(None);
        };
        if selected.is_temp {
            let _ = promote_temp_session(&selected.path, &selected.main_path);
        } else {
            let _ = Self::delete_path(&tmp_path);
        }
        Ok(Some(selected.persisted))
    }

    fn inspect_session_metadata_from_dir(
        dir: &Path,
        key: &SessionKey,
        expected_chain_hash: &Hash,
    ) -> io::Result<Option<PersistedSessionMetadata>> {
        let path = Self::make_session_path(dir, key);
        let tmp_path = temp_session_path(&path);
        let main_bytes = read_session_bytes(&path)?;
        let tmp_bytes = read_session_bytes(&tmp_path)?;
        if main_bytes.is_none() && tmp_bytes.is_none() {
            return Ok(None);
        }
        let mut selected: Option<CandidateEntry> = None;
        for (candidate_path, main_path, is_temp, bytes) in [
            (&path, &path, false, main_bytes),
            (&tmp_path, &path, true, tmp_bytes),
        ] {
            let Some(bytes) = bytes.as_deref() else {
                continue;
            };
            let Some(persisted) =
                Self::decode_persisted_session_guarded_retaining_file(bytes, candidate_path)
            else {
                continue;
            };
            if persisted.key_mismatch_with_path(candidate_path) {
                continue;
            }
            if &persisted.chain_hash != expected_chain_hash {
                continue;
            }
            if !persist_version_supported(persisted.format_version()) {
                continue;
            }
            let Some(updated_at) = persisted.updated_at() else {
                continue;
            };
            if SystemTime::now().duration_since(updated_at).is_err() {
                continue;
            }
            if validate_chunks(&persisted).is_err() {
                continue;
            }
            let candidate = CandidateEntry {
                persisted,
                path: candidate_path.to_path_buf(),
                main_path: main_path.to_path_buf(),
                is_temp,
            };
            if Self::candidate_newer_than_selected(&candidate, selected.as_ref()) {
                selected = Some(candidate);
            }
        }
        Ok(selected.map(|candidate| persisted_session_metadata(&candidate.persisted)))
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
        let mut candidates = BTreeMap::new();
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
                    let candidate = CandidateEntry {
                        persisted,
                        path: path.clone(),
                        main_path: path.with_extension(""),
                        is_temp: true,
                    };
                    Self::insert_newest_candidate(&mut candidates, key, candidate);
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    debug!(
                        ?path,
                        "persisted RBC temp session disappeared before it could be read"
                    );
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
                    let candidate = CandidateEntry {
                        persisted,
                        path: path.clone(),
                        main_path: path.clone(),
                        is_temp: false,
                    };
                    Self::insert_newest_candidate(&mut candidates, key, candidate);
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    debug!(
                        ?path,
                        "persisted RBC session disappeared before it could be read"
                    );
                }
                Err(err) => {
                    warn!(?err, ?path, "failed to read persisted RBC session");
                }
            }
        }
        for (_, candidate) in candidates {
            if candidate.is_temp {
                let path = if promote_temp_session(&candidate.path, &candidate.main_path) {
                    candidate.main_path
                } else {
                    candidate.path
                };
                out.push(Entry {
                    persisted: candidate.persisted,
                    path,
                });
            } else {
                out.push(Entry {
                    persisted: candidate.persisted,
                    path: candidate.path,
                });
            }
        }
        Ok(out)
    }

    fn insert_newest_candidate(
        candidates: &mut BTreeMap<SessionKey, CandidateEntry>,
        key: SessionKey,
        candidate: CandidateEntry,
    ) {
        match candidates.entry(key) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(candidate);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if Self::candidate_newer_than_selected(&candidate, Some(slot.get())) {
                    let previous = slot.insert(candidate);
                    if previous.is_temp {
                        let _ = Self::delete_path(&previous.path);
                    }
                } else if candidate.is_temp {
                    let _ = Self::delete_path(&candidate.path);
                }
            }
        }
    }

    fn candidate_newer_than_selected(
        candidate: &CandidateEntry,
        selected: Option<&CandidateEntry>,
    ) -> bool {
        let Some(selected) = selected else {
            return true;
        };
        candidate.persisted.last_updated_ms > selected.persisted.last_updated_ms
            || (candidate.persisted.last_updated_ms == selected.persisted.last_updated_ms
                && !candidate.is_temp
                && selected.is_temp)
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
        let Some(updated_at) = persisted.updated_at() else {
            warn!(
                ?path,
                last_updated_ms = persisted.last_updated_ms,
                "Dropping RBC persisted session with unrepresentable timestamp"
            );
            let _ = Self::delete_path(path);
            return None;
        };
        if let Err(err) = SystemTime::now().duration_since(updated_at) {
            warn!(
                ?err,
                ?path,
                last_updated_ms = persisted.last_updated_ms,
                "Dropping RBC persisted session with future timestamp"
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

    fn decode_persisted_session_guarded_retaining_file(
        data: &[u8],
        path: &Path,
    ) -> Option<PersistedSession> {
        let result = panic_hook::with_hook_suppressed(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                decode_from_bytes::<PersistedSession>(data)
            }))
        });
        match result {
            Ok(Ok(persisted)) => Some(persisted),
            Ok(Err(err)) => {
                debug!(
                    ?err,
                    ?path,
                    "failed to decode persisted RBC session during non-destructive inspection"
                );
                None
            }
            Err(panic) => {
                debug!(
                    ?path,
                    "panic while decoding persisted RBC session during non-destructive inspection"
                );
                if let Some(msg) = panic.downcast_ref::<&str>() {
                    debug!(?path, panic = %msg, "RBC decode panic message");
                } else if let Some(msg) = panic.downcast_ref::<String>() {
                    debug!(?path, panic = %msg, "RBC decode panic message");
                }
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
                let Some(updated) = entry.persisted.updated_at() else {
                    warn!(
                        ?entry.path,
                        last_updated_ms = entry.persisted.last_updated_ms,
                        "dropping RBC persisted session with unrepresentable timestamp"
                    );
                    removed.push(entry.persisted.key());
                    Self::delete_path(&entry.path)?;
                    continue;
                };
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

struct CandidateEntry {
    persisted: PersistedSession,
    path: PathBuf,
    main_path: PathBuf,
    is_temp: bool,
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
    /// Optional block header snapshot for payload recovery.
    #[norito(default)]
    pub(crate) block_header: Option<BlockHeader>,
    /// Optional leader signature over the block header.
    #[norito(default)]
    pub(crate) leader_signature: Option<BlockSignature>,
    pub(crate) total_chunks: u32,
    /// Payload chunk encoding.
    #[norito(default)]
    pub(crate) encoding: RbcEncoding,
    /// Configured shard/chunk size in bytes.
    #[norito(default)]
    pub(crate) chunk_size_bytes: u32,
    /// Canonical payload size before any RS16 padding.
    #[norito(default)]
    pub(crate) payload_size_bytes: u64,
    /// RS16 data shards per stripe (`0` when plain chunking is active).
    #[norito(default)]
    pub(crate) data_shards: u16,
    /// RS16 parity shards per stripe (`0` when plain chunking is active).
    #[norito(default)]
    pub(crate) parity_shards: u16,
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
    #[norito(default)]
    pub(crate) reconstructed_stripes: u32,
    pub(crate) chunks: Vec<PersistedChunk>,
    pub(crate) last_updated_ms: u64,
    /// Commit topology snapshot captured when this RBC session started.
    #[norito(default)]
    pub(crate) session_roster: Vec<PeerId>,
    /// Per-lane ownership allocation captured when the RBC payload was produced.
    #[norito(default)]
    pub(crate) lane_allocations: Vec<PersistedLaneAllocation>,
    /// Per-dataspace ownership allocation captured when the RBC payload was produced.
    #[norito(default)]
    pub(crate) dataspace_allocations: Vec<PersistedDataspaceAllocation>,
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

    /// Wall-clock `SystemTime` when the session was last updated, if representable.
    pub fn updated_at(&self) -> Option<SystemTime> {
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

/// Persisted per-lane RBC payload ownership allocation.
#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq)]
pub(super) struct PersistedLaneAllocation {
    pub(crate) lane_id: u32,
    pub(crate) tx_count: u64,
    pub(crate) rbc_bytes_total: u64,
    pub(crate) teu_total: u64,
    pub(crate) total_chunks: u32,
}

/// Persisted per-dataspace RBC payload ownership allocation.
#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq)]
pub(super) struct PersistedDataspaceAllocation {
    pub(crate) lane_id: u32,
    pub(crate) dataspace_id: u64,
    pub(crate) tx_count: u64,
    pub(crate) rbc_bytes_total: u64,
    pub(crate) teu_total: u64,
    pub(crate) total_chunks: u32,
}

fn ms_to_system_time(ms: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_millis(ms))
}

fn persisted_payload_bytes(
    session: &PersistedSession,
    chunks: &[&PersistedChunk],
) -> Result<Vec<u8>, &'static str> {
    if session.chunk_size_bytes == 0 {
        let total_len: usize = chunks.iter().map(|chunk| chunk.bytes.len()).sum();
        let mut bytes = Vec::with_capacity(total_len);
        for chunk in chunks {
            bytes.extend_from_slice(&chunk.bytes);
        }
        return Ok(bytes);
    }

    let chunk_size =
        usize::try_from(session.chunk_size_bytes).map_err(|_| "chunk size exceeds platform")?;
    let payload_size =
        usize::try_from(session.payload_size_bytes).map_err(|_| "payload size exceeds platform")?;
    let payload_chunk_count = if payload_size == 0 {
        1
    } else {
        payload_size.div_ceil(chunk_size)
    };
    match session.encoding {
        RbcEncoding::Plain => {
            let total_len: usize = chunks.iter().map(|chunk| chunk.bytes.len()).sum();
            let mut bytes = Vec::with_capacity(total_len);
            for chunk in chunks {
                bytes.extend_from_slice(&chunk.bytes);
            }
            bytes.truncate(payload_size);
            Ok(bytes)
        }
        RbcEncoding::Rs16 => {
            let data_shards = usize::from(session.data_shards);
            let parity_shards = usize::from(session.parity_shards);
            if chunk_size % 2 != 0 || data_shards == 0 || parity_shards == 0 {
                return Err("invalid RBC erasure profile");
            }
            let stripe_width = data_shards.saturating_add(parity_shards);
            let mut bytes = Vec::with_capacity(payload_size);
            for payload_idx in 0..payload_chunk_count {
                let stripe = payload_idx / data_shards;
                let within = payload_idx % data_shards;
                let encoded_idx = stripe
                    .checked_mul(stripe_width)
                    .and_then(|base| base.checked_add(within))
                    .ok_or("encoded chunk index overflow")?;
                let chunk = chunks
                    .get(encoded_idx)
                    .ok_or("encoded chunk index missing")?;
                bytes.extend_from_slice(&chunk.bytes);
            }
            bytes.truncate(payload_size);
            Ok(bytes)
        }
    }
}

pub(super) fn validate_allocations(session: &PersistedSession) -> Result<(), &'static str> {
    if session.lane_allocations.is_empty() && session.dataspace_allocations.is_empty() {
        return Ok(());
    }
    if session.total_chunks == 0 {
        return Err("allocation metadata with zero chunks");
    }
    if session.lane_allocations.is_empty() || session.dataspace_allocations.is_empty() {
        return Err("incomplete allocation metadata");
    }

    let mut lane_totals: BTreeMap<u32, (u64, u64, u64, u64)> = BTreeMap::new();
    let mut lane_chunk_sum = 0u64;
    for alloc in &session.lane_allocations {
        if alloc.tx_count == 0 {
            return Err("zero lane allocation transaction count");
        }
        if lane_totals
            .insert(
                alloc.lane_id,
                (
                    alloc.tx_count,
                    u64::from(alloc.total_chunks),
                    alloc.rbc_bytes_total,
                    alloc.teu_total,
                ),
            )
            .is_some()
        {
            return Err("duplicate lane allocation");
        }
        lane_chunk_sum = lane_chunk_sum
            .checked_add(u64::from(alloc.total_chunks))
            .ok_or("lane allocation chunk sum overflow")?;
    }
    if lane_chunk_sum != u64::from(session.total_chunks) {
        return Err("lane allocation chunk sum mismatch");
    }

    let mut dataspace_seen = BTreeSet::new();
    let mut dataspace_sums: BTreeMap<u32, (u64, u64, u64, u64)> = BTreeMap::new();
    for alloc in &session.dataspace_allocations {
        if alloc.tx_count == 0 {
            return Err("zero dataspace allocation transaction count");
        }
        if !lane_totals.contains_key(&alloc.lane_id) {
            return Err("dataspace allocation references unknown lane");
        }
        if !dataspace_seen.insert((alloc.lane_id, alloc.dataspace_id)) {
            return Err("duplicate dataspace allocation");
        }
        let entry = dataspace_sums.entry(alloc.lane_id).or_insert((0, 0, 0, 0));
        entry.0 = entry
            .0
            .checked_add(alloc.tx_count)
            .ok_or("dataspace allocation transaction sum overflow")?;
        entry.1 = entry
            .1
            .checked_add(u64::from(alloc.total_chunks))
            .ok_or("dataspace allocation chunk sum overflow")?;
        entry.2 = entry
            .2
            .checked_add(alloc.rbc_bytes_total)
            .ok_or("dataspace allocation byte sum overflow")?;
        entry.3 = entry
            .3
            .checked_add(alloc.teu_total)
            .ok_or("dataspace allocation TEU sum overflow")?;
    }

    for (lane_id, expected) in lane_totals {
        if dataspace_sums.get(&lane_id).copied().unwrap_or_default() != expected {
            return Err("dataspace allocation sum mismatch");
        }
    }

    Ok(())
}

fn validate_chunks(session: &PersistedSession) -> Result<(), &'static str> {
    validate_allocations(session)?;

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
    if !session.chunk_digests.is_empty() {
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(session.chunk_digests.clone());
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
    } else if session.deliver_sender.is_some() || session.deliver_signature.is_some() {
        return Err("deliver metadata without delivered flag");
    }

    if let Some(expected_hash) = &session.payload_hash {
        if session.chunks.len() == expected {
            let bytes = persisted_payload_bytes(session, &chunks)?;
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
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{block::BlockHeader, peer::PeerId};
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::main_loop::{RbcProgressStage, RbcSession};

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
            block_header: None,
            leader_signature: None,
            total_chunks: 0,
            encoding: RbcEncoding::Plain,
            chunk_size_bytes: 0,
            payload_size_bytes: 0,
            data_shards: 0,
            parity_shards: 0,
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
            reconstructed_stripes: 0,
            chunks: Vec::new(),
            last_updated_ms: 0,
            session_roster: Vec::new(),
            lane_allocations: Vec::new(),
            dataspace_allocations: Vec::new(),
        }
    }

    fn store_for_tests(dir: &Path) -> ChunkStore {
        ChunkStore::new(
            dir.to_path_buf(),
            Duration::from_secs(120),
            4,
            1 << 19,
            8,
            1 << 20,
        )
        .expect("chunk store init")
    }

    fn write_persisted_session_at(
        dir: &Path,
        path_key: &SessionKey,
        persisted: &PersistedSession,
    ) -> PathBuf {
        let path = ChunkStore::make_session_path(dir, path_key);
        let encoded = to_bytes(persisted).expect("encode persisted session");
        fs::write(&path, &encoded).expect("write persisted session");
        path
    }

    fn assert_persisted_session_rejected_and_deleted(
        label: &str,
        path_key: SessionKey,
        persisted: PersistedSession,
        expected_chain_hash: Hash,
        expected_manifest: SoftwareManifest,
    ) {
        let dir = tempdir().unwrap();
        let store = store_for_tests(dir.path());
        let path = write_persisted_session_at(dir.path(), &path_key, &persisted);

        let load = store
            .load(&expected_chain_hash, &expected_manifest)
            .unwrap_or_else(|error| panic!("{label}: load failed: {error}"));

        assert!(
            load.sessions.is_empty(),
            "{label}: rejected persisted session must not load"
        );
        assert!(
            !path.exists(),
            "{label}: rejected persisted session file must be deleted"
        );
    }

    fn persisted_single_chunk_session(
        key: SessionKey,
        chain_hash: Hash,
        manifest: &SoftwareManifest,
        byte: u8,
        len: usize,
    ) -> PersistedSession {
        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![byte; len], 0);
        session.to_persisted(key, chain_hash, manifest, &[])
    }

    #[test]
    fn inconsistent_allocation_metadata_is_rejected_and_deleted() {
        let key = session_key(0x8A);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = persisted_single_chunk_session(key, chain_hash, &manifest, 0x44, 4);
        persisted.lane_allocations = vec![PersistedLaneAllocation {
            lane_id: 7,
            tx_count: 1,
            rbc_bytes_total: 4,
            teu_total: 1,
            total_chunks: 1,
        }];
        persisted.dataspace_allocations = vec![PersistedDataspaceAllocation {
            lane_id: 7,
            dataspace_id: 42,
            tx_count: 1,
            rbc_bytes_total: 4,
            teu_total: 1,
            total_chunks: 0,
        }];

        assert_persisted_session_rejected_and_deleted(
            "dataspace allocation must sum to its lane allocation",
            key,
            persisted,
            chain_hash,
            manifest,
        );
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
            block_header: None,
            leader_signature: None,
            total_chunks: 0,
            encoding: RbcEncoding::Plain,
            chunk_size_bytes: 0,
            payload_size_bytes: 0,
            data_shards: 0,
            parity_shards: 0,
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
            reconstructed_stripes: 0,
            chunks: Vec::new(),
            last_updated_ms: 0,
            session_roster: Vec::new(),
            lane_allocations: Vec::new(),
            dataspace_allocations: Vec::new(),
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
    fn load_session_from_dir_rejects_future_timestamp_session() {
        let dir = tempdir().unwrap();
        let key = session_key(45);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = sample_persisted_session(key, chain_hash, manifest.clone());
        let future_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .saturating_add(120_000);
        persisted.last_updated_ms = u64::try_from(future_ms).unwrap_or(u64::MAX);

        let path = ChunkStore::make_session_path(dir.path(), &key);
        fs::write(
            &path,
            to_bytes(&persisted).expect("encode persisted session"),
        )
        .expect("write future-dated persisted session");

        let loaded = ChunkStore::load_session_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("load session from dir");
        assert!(
            loaded.is_none(),
            "direct recovery must reject future-dated RBC snapshots"
        );
        assert!(
            !path.exists(),
            "future-dated RBC snapshots should be removed during direct recovery"
        );
    }

    #[test]
    fn load_session_from_dir_rejects_max_timestamp_session() {
        let dir = tempdir().unwrap();
        let key = session_key(48);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = persisted_single_chunk_session(key, chain_hash, &manifest, 0xD0, 8);
        persisted.last_updated_ms = u64::MAX;

        let path = ChunkStore::make_session_path(dir.path(), &key);
        fs::write(
            &path,
            to_bytes(&persisted).expect("encode persisted session"),
        )
        .expect("write max-timestamp persisted session");

        let loaded = ChunkStore::load_session_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("load session from dir");
        assert!(
            loaded.is_none(),
            "direct recovery must reject adversarial max-timestamp RBC snapshots"
        );
        assert!(
            !path.exists(),
            "max-timestamp RBC snapshots should be removed during direct recovery"
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
    fn load_session_from_dir_prefers_newer_main_over_stale_temp() {
        let dir = tempdir().unwrap();
        let key = session_key(42);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut main = sample_persisted_session(key, chain_hash, manifest.clone());
        main.last_updated_ms = 20;
        let mut temp = sample_persisted_session(key, chain_hash, manifest.clone());
        temp.last_updated_ms = 10;

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&path, to_bytes(&main).expect("encode main session"))
            .expect("write main session");
        fs::write(&tmp_path, to_bytes(&temp).expect("encode temp session"))
            .expect("write stale temp session");

        let loaded = ChunkStore::load_session_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("load session from dir")
            .expect("main session should load");
        assert_eq!(
            loaded.last_updated_ms, main.last_updated_ms,
            "newer main snapshot must not be shadowed by a stale temp file"
        );
        assert!(path.exists(), "main session should remain");
        assert!(!tmp_path.exists(), "stale temp session should be removed");
    }

    #[test]
    fn load_session_from_dir_promotes_newer_temp_over_main() {
        let dir = tempdir().unwrap();
        let key = session_key(43);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut main = sample_persisted_session(key, chain_hash, manifest.clone());
        main.last_updated_ms = 10;
        let mut temp = sample_persisted_session(key, chain_hash, manifest.clone());
        temp.last_updated_ms = 20;

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&path, to_bytes(&main).expect("encode main session"))
            .expect("write older main session");
        fs::write(&tmp_path, to_bytes(&temp).expect("encode temp session"))
            .expect("write newer temp session");

        let loaded = ChunkStore::load_session_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("load session from dir")
            .expect("temp session should load");
        assert_eq!(
            loaded.last_updated_ms, temp.last_updated_ms,
            "newer temp snapshot should still recover after a crash before rename"
        );
        assert!(path.exists(), "newer temp session should be promoted");
        assert!(
            !tmp_path.exists(),
            "promoted temp session should be removed"
        );
        let promoted = fs::read(&path).expect("read promoted main session");
        let promoted =
            decode_from_bytes::<PersistedSession>(&promoted).expect("decode promoted session");
        assert_eq!(promoted.last_updated_ms, temp.last_updated_ms);
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
    fn scan_entries_prefers_newer_main_over_stale_temp() {
        let dir = tempdir().unwrap();
        let store = ChunkStore::new(dir.path().to_path_buf(), Duration::ZERO, 4, 1024, 8, 4096)
            .expect("chunk store init");

        let key = session_key(44);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut main = sample_persisted_session(key, chain_hash, manifest.clone());
        main.last_updated_ms = 20;
        let mut temp = sample_persisted_session(key, chain_hash, manifest.clone());
        temp.last_updated_ms = 10;

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&path, to_bytes(&main).expect("encode main session"))
            .expect("write main session");
        fs::write(&tmp_path, to_bytes(&temp).expect("encode temp session"))
            .expect("write stale temp session");

        let load = store
            .load(&chain_hash, &manifest)
            .expect("load persisted sessions");
        assert_eq!(load.sessions.len(), 1, "main session should load");
        assert_eq!(
            load.sessions[0].last_updated_ms, main.last_updated_ms,
            "store scan must not let stale temp snapshots shadow newer main snapshots"
        );
        assert!(path.exists(), "main session should remain");
        assert!(!tmp_path.exists(), "stale temp session should be removed");
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
    fn incomplete_persisted_session_with_digest_root_mismatch_is_dropped() {
        let key = session_key(12);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let roster = vec![test_peer_id(1)];

        let chunk0 = vec![0x41; 4];
        let chunk1_digest = [0x42; 32];
        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, chunk0.clone(), 0);

        let mut chunk0_digest = [0u8; 32];
        chunk0_digest.copy_from_slice(&Sha256::digest(&chunk0));
        let chunk_digests = vec![chunk0_digest, chunk1_digest];
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone());
        let expected_root = tree.root().map(Hash::from).expect("chunk root");
        let mut mismatched_root = *expected_root.as_ref();
        mismatched_root[0] ^= 0xFF;

        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &roster);
        assert_eq!(persisted.chunks.len(), 1);
        persisted.chunk_digests = chunk_digests;
        persisted.expected_chunk_root = Some(Hash::prehashed(mismatched_root));

        assert_persisted_session_rejected_and_deleted(
            "incomplete digest/root mismatch",
            key,
            persisted,
            chain_hash,
            manifest,
        );
    }

    #[test]
    fn store_validation_deletes_adversarial_metadata_and_integrity_failures() {
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let base_key = session_key(20);
        let mut wrong_manifest = manifest.clone();
        wrong_manifest.version = "different-version".into();

        let mut invalid = sample_persisted_session(base_key, chain_hash, manifest.clone());
        invalid.invalid = true;
        assert_persisted_session_rejected_and_deleted(
            "invalid flag",
            base_key,
            invalid,
            chain_hash,
            manifest.clone(),
        );

        let mut unsupported_version =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        unsupported_version.format_version = PERSIST_VERSION.saturating_add(1);
        assert_persisted_session_rejected_and_deleted(
            "unsupported version",
            base_key,
            unsupported_version,
            chain_hash,
            manifest.clone(),
        );

        let wrong_path_key = session_key(21);
        let mismatched_key =
            sample_persisted_session(session_key(22), chain_hash, manifest.clone());
        assert_persisted_session_rejected_and_deleted(
            "path/key mismatch",
            wrong_path_key,
            mismatched_key,
            chain_hash,
            manifest.clone(),
        );

        let wrong_chain =
            sample_persisted_session(base_key, Hash::new(b"other-chain"), manifest.clone());
        assert_persisted_session_rejected_and_deleted(
            "wrong chain",
            base_key,
            wrong_chain,
            chain_hash,
            manifest.clone(),
        );

        let wrong_manifest_session = sample_persisted_session(base_key, chain_hash, wrong_manifest);
        assert_persisted_session_rejected_and_deleted(
            "wrong manifest",
            base_key,
            wrong_manifest_session,
            chain_hash,
            manifest.clone(),
        );

        let mut zero_total_with_digest =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        zero_total_with_digest.chunk_digests.push([0x11; 32]);
        assert_persisted_session_rejected_and_deleted(
            "zero total with digest",
            base_key,
            zero_total_with_digest,
            chain_hash,
            manifest.clone(),
        );

        let mut zero_total_with_chunk =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        zero_total_with_chunk.chunks.push(PersistedChunk {
            idx: 0,
            bytes: vec![0xAA],
        });
        assert_persisted_session_rejected_and_deleted(
            "zero total with chunk",
            base_key,
            zero_total_with_chunk,
            chain_hash,
            manifest.clone(),
        );

        let mut too_many_chunks =
            persisted_single_chunk_session(base_key, chain_hash, &manifest, 0xA0, 8);
        too_many_chunks.chunks.push(PersistedChunk {
            idx: 1,
            bytes: vec![0xA1; 8],
        });
        assert_persisted_session_rejected_and_deleted(
            "too many chunks",
            base_key,
            too_many_chunks,
            chain_hash,
            manifest.clone(),
        );

        let mut duplicate_chunk =
            persisted_single_chunk_session(base_key, chain_hash, &manifest, 0xA2, 8);
        duplicate_chunk.chunks.push(PersistedChunk {
            idx: 0,
            bytes: vec![0xA3; 8],
        });
        assert_persisted_session_rejected_and_deleted(
            "duplicate chunk",
            base_key,
            duplicate_chunk,
            chain_hash,
            manifest.clone(),
        );

        let mut chunk_out_of_range =
            persisted_single_chunk_session(base_key, chain_hash, &manifest, 0xA4, 8);
        chunk_out_of_range.chunks[0].idx = 1;
        assert_persisted_session_rejected_and_deleted(
            "chunk out of range",
            base_key,
            chunk_out_of_range,
            chain_hash,
            manifest.clone(),
        );

        let mut empty_ready_signature =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        empty_ready_signature.ready_signatures.push(PersistedReady {
            sender: 0,
            signature: Vec::new(),
        });
        assert_persisted_session_rejected_and_deleted(
            "empty ready signature",
            base_key,
            empty_ready_signature,
            chain_hash,
            manifest.clone(),
        );

        let mut duplicate_ready = sample_persisted_session(base_key, chain_hash, manifest.clone());
        duplicate_ready.ready_signatures.push(PersistedReady {
            sender: 0,
            signature: vec![0x01],
        });
        duplicate_ready.ready_signatures.push(PersistedReady {
            sender: 0,
            signature: vec![0x02],
        });
        assert_persisted_session_rejected_and_deleted(
            "duplicate ready sender",
            base_key,
            duplicate_ready,
            chain_hash,
            manifest.clone(),
        );

        let mut ready_sender_oob = sample_persisted_session(base_key, chain_hash, manifest.clone());
        ready_sender_oob.session_roster.push(test_peer_id(1));
        ready_sender_oob.ready_signatures.push(PersistedReady {
            sender: 1,
            signature: vec![0x01],
        });
        assert_persisted_session_rejected_and_deleted(
            "ready sender out of roster",
            base_key,
            ready_sender_oob,
            chain_hash,
            manifest.clone(),
        );

        let mut deliver_sender_oob =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        deliver_sender_oob.session_roster.push(test_peer_id(1));
        deliver_sender_oob.delivered = true;
        deliver_sender_oob.deliver_sender = Some(1);
        deliver_sender_oob.deliver_signature = Some(vec![0x01]);
        assert_persisted_session_rejected_and_deleted(
            "deliver sender out of roster",
            base_key,
            deliver_sender_oob,
            chain_hash,
            manifest.clone(),
        );

        let mut delivered_missing_signature =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        delivered_missing_signature.delivered = true;
        delivered_missing_signature.deliver_sender = Some(0);
        assert_persisted_session_rejected_and_deleted(
            "delivered without signature",
            base_key,
            delivered_missing_signature,
            chain_hash,
            manifest.clone(),
        );

        let mut delivered_empty_signature =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        delivered_empty_signature.delivered = true;
        delivered_empty_signature.deliver_sender = Some(0);
        delivered_empty_signature.deliver_signature = Some(Vec::new());
        assert_persisted_session_rejected_and_deleted(
            "delivered with empty signature",
            base_key,
            delivered_empty_signature,
            chain_hash,
            manifest.clone(),
        );

        let mut stale_deliver_metadata =
            sample_persisted_session(base_key, chain_hash, manifest.clone());
        stale_deliver_metadata.delivered = false;
        stale_deliver_metadata.deliver_sender = Some(0);
        stale_deliver_metadata.deliver_signature = Some(vec![0x01]);
        assert_persisted_session_rejected_and_deleted(
            "deliver metadata without delivered flag",
            base_key,
            stale_deliver_metadata,
            chain_hash,
            manifest.clone(),
        );

        let mut payload_hash_mismatch =
            persisted_single_chunk_session(base_key, chain_hash, &manifest, 0xA5, 8);
        payload_hash_mismatch.payload_hash = Some(Hash::prehashed([0xFF; 32]));
        assert_persisted_session_rejected_and_deleted(
            "payload hash mismatch",
            base_key,
            payload_hash_mismatch,
            chain_hash,
            manifest.clone(),
        );

        let mut digest_mismatch =
            persisted_single_chunk_session(base_key, chain_hash, &manifest, 0xA6, 8);
        digest_mismatch.chunk_digests[0] = [0xEE; 32];
        assert_persisted_session_rejected_and_deleted(
            "chunk digest mismatch",
            base_key,
            digest_mismatch,
            chain_hash,
            manifest.clone(),
        );

        let mut computed_root_mismatch =
            persisted_single_chunk_session(base_key, chain_hash, &manifest, 0xA7, 8);
        computed_root_mismatch.computed_chunk_root = Some(Hash::prehashed([0xDD; 32]));
        assert_persisted_session_rejected_and_deleted(
            "computed root mismatch",
            base_key,
            computed_root_mismatch,
            chain_hash,
            manifest,
        );
    }

    #[test]
    fn non_destructive_metadata_inspection_rejects_invalid_chunks_without_deleting() {
        let dir = tempdir().unwrap();
        let key = session_key(23);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = persisted_single_chunk_session(key, chain_hash, &manifest, 0xB0, 8);
        persisted.chunks.push(PersistedChunk {
            idx: 0,
            bytes: vec![0xB1; 8],
        });
        let path = write_persisted_session_at(dir.path(), &key, &persisted);

        let metadata = inspect_session_metadata_from_dir(dir.path(), &key, &chain_hash)
            .expect("inspect metadata");

        assert!(metadata.is_none());
        assert!(
            path.exists(),
            "non-destructive inspection must not delete invalid chunk snapshots"
        );
    }

    #[test]
    fn hard_session_limit_evicts_oldest_sessions_and_reports_hard_pressure() {
        let dir = tempdir().unwrap();
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let keys = [session_key(24), session_key(25), session_key(26)];
        for (idx, key) in keys.iter().enumerate() {
            let mut persisted = sample_persisted_session(*key, chain_hash, manifest.clone());
            persisted.last_updated_ms = u64::try_from(idx + 1).expect("timestamp fits");
            write_persisted_session_at(dir.path(), key, &persisted);
        }

        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::ZERO,
            2,
            1 << 20,
            2,
            1 << 20,
        )
        .expect("chunk store init");
        let load = store.load(&chain_hash, &manifest).expect("load sessions");

        assert_eq!(load.removed, vec![keys[0]]);
        assert!(matches!(
            load.pressure,
            StorePressure::HardLimit {
                sessions: 2,
                bytes: 0
            }
        ));
        let retained: Vec<SessionKey> = load.sessions.iter().map(PersistedSession::key).collect();
        assert_eq!(retained, vec![keys[1], keys[2]]);
        assert!(
            !ChunkStore::make_session_path(dir.path(), &keys[0]).exists(),
            "oldest session should be deleted"
        );
        assert!(ChunkStore::make_session_path(dir.path(), &keys[1]).exists());
        assert!(ChunkStore::make_session_path(dir.path(), &keys[2]).exists());
    }

    #[test]
    fn hard_byte_limit_evicts_oldest_payloads_until_under_cap() {
        let dir = tempdir().unwrap();
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let keys = [session_key(27), session_key(28), session_key(29)];
        for (idx, key) in keys.iter().enumerate() {
            let mut persisted = persisted_single_chunk_session(
                *key,
                chain_hash,
                &manifest,
                u8::try_from(0xC0 + idx).expect("byte fits"),
                20,
            );
            persisted.last_updated_ms = u64::try_from(idx + 1).expect("timestamp fits");
            write_persisted_session_at(dir.path(), key, &persisted);
        }

        let store = ChunkStore::new(dir.path().to_path_buf(), Duration::ZERO, 8, 40, 8, 40)
            .expect("chunk store init");
        let load = store.load(&chain_hash, &manifest).expect("load sessions");

        assert_eq!(load.removed, vec![keys[0]]);
        assert!(matches!(
            load.pressure,
            StorePressure::HardLimit {
                sessions: 2,
                bytes: 40
            }
        ));
        let retained: Vec<SessionKey> = load.sessions.iter().map(PersistedSession::key).collect();
        assert_eq!(retained, vec![keys[1], keys[2]]);
    }

    #[test]
    fn disabled_store_removes_existing_snapshot_on_persist() {
        let dir = tempdir().unwrap();
        let key = session_key(30);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![0xD0; 8], 0);
        let enabled = store_for_tests(dir.path());
        enabled
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist enabled session");
        assert!(ChunkStore::make_session_path(dir.path(), &key).exists());

        let disabled = ChunkStore::new(dir.path().to_path_buf(), Duration::ZERO, 0, 0, 0, 1 << 20)
            .expect("disabled chunk store init");
        let outcome = disabled
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist through disabled store");

        assert!(matches!(
            outcome.pressure,
            StorePressure::Normal {
                sessions: 0,
                bytes: 0
            }
        ));
        assert!(
            !ChunkStore::make_session_path(dir.path(), &key).exists(),
            "disabled store should remove the old snapshot for the same key"
        );
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
    fn from_persisted_rejects_zero_total_chunks() {
        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![1, 2, 3], 0);
        session.test_set_delivered(true);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(14);
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        persisted.total_chunks = 0;
        persisted.chunks.clear();
        persisted.chunk_digests.clear();
        persisted.expected_chunk_root = None;
        persisted.computed_chunk_root = None;

        let err = RbcSession::from_persisted_unchecked(&persisted);
        assert!(matches!(
            err,
            Err(crate::sumeragi::main_loop::PersistedLoadError::InvalidLayout("zero total chunks"))
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
    fn from_persisted_demotes_delivered_without_chunk_bytes_for_repair() {
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
        let mut rebuilt =
            RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert_eq!(rebuilt.total_chunks(), 2);
        assert_eq!(rebuilt.received_chunks(), 0);
        assert_eq!(
            rebuilt.progress_stage(),
            RbcProgressStage::LocalReadySent,
            "payload-less recovered delivery must not re-enter as terminal DELIVER"
        );
        assert!(
            rebuilt.allows_payload_recovery(),
            "payload-less recovered delivery must remain eligible for payload repair"
        );
        assert_eq!(
            rebuilt.take_delivered_payload_bytes_for_telemetry(),
            None,
            "payload-less recovered delivery must not consume delivered-byte telemetry"
        );
    }

    #[test]
    fn from_persisted_clears_deliver_metadata_when_not_delivered() {
        let mut session = RbcSession::test_new(1, None, None, 0);
        session.test_note_chunk(0, vec![1, 2, 3], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(17);
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        persisted.delivered = false;
        persisted.deliver_sender = Some(0);
        persisted.deliver_signature = Some(vec![0xAA]);

        let rebuilt = RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        let roundtrip = rebuilt.to_persisted(key, chain_hash, &manifest, &[]);
        assert!(!roundtrip.delivered);
        assert_eq!(roundtrip.deliver_sender, None);
        assert_eq!(roundtrip.deliver_signature, None);
    }

    #[test]
    fn from_persisted_demotes_delivered_without_payload_hash() {
        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, vec![5, 5, 5], 0);
        session.test_note_chunk(1, vec![6, 6, 6], 0);
        session.test_set_delivered(true);
        session.test_set_sent_ready(true);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(16);
        let persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        let mut rebuilt =
            RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");
        assert_eq!(rebuilt.total_chunks(), 2);
        assert_eq!(rebuilt.received_chunks(), 2);
        assert_eq!(
            rebuilt.progress_stage(),
            RbcProgressStage::LocalReadySent,
            "payload-hash-less recovered delivery must not re-enter as terminal DELIVER"
        );
        assert_eq!(
            rebuilt.delivered_payload_bytes(),
            None,
            "complete chunks without an advertised payload hash must not be treated as verified delivered bytes"
        );
        assert_eq!(
            rebuilt.take_delivered_payload_bytes_for_telemetry(),
            None,
            "payload-hash-less recovered delivery must not consume delivered-byte telemetry"
        );
    }

    #[test]
    fn from_persisted_demotes_delivered_with_complete_payload_for_revalidation() {
        let payload = b"complete-recovered-payload".to_vec();
        let payload_hash = Hash::new(&payload);
        let mut session = RbcSession::test_new(1, Some(payload_hash), None, 0);
        session.test_note_chunk(0, payload.clone(), 0);
        session.test_set_delivered(true);
        session.test_set_sent_ready(true);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let key = session_key(18);
        let persisted = session.to_persisted(key, chain_hash, &manifest, &[]);

        let mut rebuilt =
            RbcSession::from_persisted_unchecked(&persisted).expect("rebuild session");

        assert_eq!(rebuilt.total_chunks(), 1);
        assert_eq!(rebuilt.received_chunks(), 1);
        assert_eq!(
            rebuilt.progress_stage(),
            RbcProgressStage::LocalReadySent,
            "recovered delivery markers must be re-derived from fresh network evidence"
        );
        assert!(
            rebuilt.complete_payload_matches(&payload_hash),
            "complete recovered payload bytes should remain available after demoting delivery"
        );
        assert_eq!(rebuilt.delivered_payload_bytes(), None);
        assert_eq!(rebuilt.take_delivered_payload_bytes_for_telemetry(), None);
    }

    #[test]
    fn load_session_metadata_from_dir_reports_chunk_counts() {
        let dir = tempdir().unwrap();
        let key = session_key(13);
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
        session.test_note_chunk(0, vec![1u8; 8], 0);
        session.test_note_chunk(1, vec![2u8; 8], 0);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();

        store
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist session");

        let metadata = load_session_metadata_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("inspect persisted session")
            .expect("metadata should exist");
        assert_eq!(metadata.block_hash, key.0);
        assert_eq!(metadata.height, key.1);
        assert_eq!(metadata.view, key.2);
        assert_eq!(metadata.total_chunks, 2);
        assert_eq!(metadata.persisted_chunk_count, 2);
        assert!(!metadata.delivered);
        assert!(!metadata.invalid);
    }

    #[test]
    fn inspect_session_metadata_from_dir_ignores_manifest_mismatch_without_deleting() {
        let dir = tempdir().unwrap();
        let key = session_key(15);
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
        session.test_note_chunk(0, vec![1u8; 8], 0);
        let chain_hash = test_chain_hash();
        let persisted_manifest = SoftwareManifest {
            version: "1.0.0".into(),
            profile: "debug".into(),
            git_commit: Some("producer".into()),
        };
        let mismatched_manifest = SoftwareManifest {
            version: "1.0.0".into(),
            profile: "debug".into(),
            git_commit: Some("observer".into()),
        };

        store
            .persist_session(key, &session, &chain_hash, &persisted_manifest, &[])
            .expect("persist session");
        let path = ChunkStore::make_session_path(dir.path(), &key);

        assert!(
            load_session_metadata_from_dir(dir.path(), &key, &chain_hash, &mismatched_manifest)
                .expect("load metadata with mismatched manifest")
                .is_none(),
            "strict restart metadata loading should reject manifest mismatches"
        );
        assert!(
            !path.exists(),
            "strict loading should keep removing mismatched snapshots before restart recovery"
        );

        store
            .persist_session(key, &session, &chain_hash, &persisted_manifest, &[])
            .expect("persist session again");
        let metadata = inspect_session_metadata_from_dir(dir.path(), &key, &chain_hash)
            .expect("inspect metadata")
            .expect("metadata should exist despite manifest mismatch");
        assert_eq!(metadata.persisted_chunk_count, 1);
        assert!(
            path.exists(),
            "non-destructive inspection must not remove snapshots owned by the peer process"
        );
    }

    #[test]
    fn inspect_session_metadata_from_dir_ignores_future_timestamp_without_deleting() {
        let dir = tempdir().unwrap();
        let key = session_key(46);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = persisted_single_chunk_session(key, chain_hash, &manifest, 0xC0, 8);
        let future_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .saturating_add(120_000);
        persisted.last_updated_ms = u64::try_from(future_ms).unwrap_or(u64::MAX);
        let path = write_persisted_session_at(dir.path(), &key, &persisted);

        let metadata = inspect_session_metadata_from_dir(dir.path(), &key, &chain_hash)
            .expect("inspect metadata");
        assert!(
            metadata.is_none(),
            "non-destructive inspection must not report future-dated snapshots as evidence"
        );
        assert!(
            path.exists(),
            "non-destructive inspection must not delete peer-owned snapshots"
        );
    }

    #[test]
    fn inspect_session_metadata_from_dir_ignores_max_timestamp_without_deleting() {
        let dir = tempdir().unwrap();
        let key = session_key(49);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = persisted_single_chunk_session(key, chain_hash, &manifest, 0xD1, 8);
        persisted.last_updated_ms = u64::MAX;
        let path = write_persisted_session_at(dir.path(), &key, &persisted);

        let metadata = inspect_session_metadata_from_dir(dir.path(), &key, &chain_hash)
            .expect("inspect metadata");
        assert!(
            metadata.is_none(),
            "non-destructive inspection must not report max-timestamp snapshots as evidence"
        );
        assert!(
            path.exists(),
            "non-destructive inspection must not delete peer-owned snapshots"
        );
    }

    #[test]
    fn inspect_session_metadata_from_dir_prefers_newer_temp_without_promoting() {
        let dir = tempdir().unwrap();
        let key = session_key(47);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut main = persisted_single_chunk_session(key, chain_hash, &manifest, 0xC1, 8);
        main.last_updated_ms = 10;

        let mut session = RbcSession::test_new(2, None, None, 0);
        session.test_note_chunk(0, vec![0xC2; 8], 0);
        session.test_note_chunk(1, vec![0xC3; 8], 0);
        let mut temp = session.to_persisted(key, chain_hash, &manifest, &[]);
        temp.last_updated_ms = 20;

        let path = ChunkStore::make_session_path(dir.path(), &key);
        let tmp_path = temp_session_path(&path);
        fs::write(&path, to_bytes(&main).expect("encode main session"))
            .expect("write older main session");
        fs::write(&tmp_path, to_bytes(&temp).expect("encode temp session"))
            .expect("write newer temp session");

        let metadata = inspect_session_metadata_from_dir(dir.path(), &key, &chain_hash)
            .expect("inspect metadata")
            .expect("newer temp metadata should be visible");
        assert_eq!(
            metadata.persisted_chunk_count, 2,
            "non-destructive inspection should report the newest valid temp/main snapshot"
        );
        assert!(
            path.exists(),
            "inspection should not replace the main snapshot"
        );
        assert!(
            tmp_path.exists(),
            "inspection should not promote or delete temp snapshots"
        );
    }

    #[test]
    fn load_session_metadata_from_dir_keeps_delivered_metadata_without_chunk_bytes() {
        let dir = tempdir().unwrap();
        let key = session_key(14);
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
        session.test_note_chunk(0, vec![5u8; 8], 0);
        session.test_note_chunk(1, vec![6u8; 8], 0);
        session.test_set_sent_ready(true);
        session.record_deliver(0, vec![0xAA; 64]);
        let chain_hash = test_chain_hash();
        let manifest = test_manifest();
        let mut persisted = session.to_persisted(key, chain_hash, &manifest, &[]);
        persisted.chunks.clear();

        store
            .write_session(&persisted)
            .expect("write persisted session");

        let metadata = load_session_metadata_from_dir(dir.path(), &key, &chain_hash, &manifest)
            .expect("inspect persisted session")
            .expect("metadata should exist");
        assert_eq!(metadata.total_chunks, 2);
        assert_eq!(metadata.persisted_chunk_count, 0);
        assert!(metadata.delivered);
        assert!(!metadata.invalid);
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

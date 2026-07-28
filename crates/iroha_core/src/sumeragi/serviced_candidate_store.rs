//! Durable, bounded tombstones for terminally retired Sumeragi v2 candidates.
//!
//! The safety WAL cannot contain these records: reducer persistence identifiers
//! are deliberately one-to-one with WAL frame sequence numbers.  This adjacent
//! snapshot therefore uses its own context-bound, checksummed frame and the
//! same write/sync/atomic-replace/directory-sync publication order as the
//! consensus sidecars. Ordinary successful service markers remain
//! process-generation local because proposal, quorum-pool, and body-pipeline
//! state can be volatile; restart must permit retransmission to reconstruct
//! that state.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll, Encode};

// Version 3 records only restart-safe terminal retirements. Version 2
// snapshots also contained volatile successful-service markers and must fail
// closed instead of suppressing reconstruction after restart.
const FORMAT_VERSION: u16 = 3;
const FRAME_MAGIC: &[u8; 8] = b"SUMVCAND";
const HASH_BYTES: usize = 32;
const FRAME_HEADER_BYTES: usize = FRAME_MAGIC.len() + 2 + 8 + HASH_BYTES;
const FIXED_FRAME_HEADROOM_BYTES: u64 = 4 * 1024;
const RECORD_FRAME_HEADROOM_BYTES: u64 = 192;

/// Route-neutral identity of one reducer occurrence.
///
/// `context_id`, `height`, and `owner` deliberately repeat the snapshot header
/// binding, so a decoded record cannot be transplanted between otherwise
/// valid files or validators. The adapter may use this key transiently for any
/// successful service; this store persists it only after terminal retirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServicedCandidateKey {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    leader: wire::ValidatorIndex,
    source_view: wire::View,
    target: Option<[u8; 32]>,
    phase: u8,
    class: u8,
    kind: u8,
    evidence: [u8; 32],
}

impl ServicedCandidateKey {
    /// Construct a key from a fully validated, immutable semantic projection.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        leader: wire::ValidatorIndex,
        source_view: wire::View,
        target: Option<[u8; 32]>,
        phase: u8,
        class: u8,
        kind: u8,
        evidence: [u8; 32],
    ) -> Self {
        Self {
            context_id,
            height,
            owner,
            leader,
            source_view,
            target,
            phase,
            class,
            kind,
            evidence,
        }
    }

    /// Leader derived from the semantic occurrence's source view.
    #[cfg(test)]
    pub(crate) const fn leader(self) -> wire::ValidatorIndex {
        self.leader
    }

    /// View carried by the semantic occurrence itself.
    #[cfg(test)]
    pub(crate) const fn source_view(self) -> wire::View {
        self.source_view
    }

    /// Semantic adapter lane which owned the serviced occurrence.
    #[cfg(test)]
    pub(crate) const fn class(self) -> u8 {
        self.class
    }

    fn belongs_to(
        self,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
    ) -> bool {
        self.context_id == context_id && self.height == height && self.owner == owner
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedServicedCandidate {
    key: ServicedCandidateKey,
    /// Consumer episode metadata used only for strict-view reclamation.
    service_view: wire::View,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedServicedCandidates {
    format_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    capacity: u64,
    decision_reclaimed: bool,
    records: Vec<PersistedServicedCandidate>,
}

/// Restored tombstone set and its one-shot durable-Decision reclamation flag.
pub(crate) struct RestoredServicedCandidates {
    /// Canonically ordered, context-bound records.
    pub(crate) records: BTreeMap<ServicedCandidateKey, wire::View>,
    /// Whether the pre-Decision epoch has already been reclaimed.
    pub(crate) decision_reclaimed: bool,
}

/// Atomic per-height snapshot stored beside the safety WAL.
#[derive(Debug)]
pub(crate) struct ServicedCandidateStore {
    path: PathBuf,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    capacity: usize,
    max_frame_bytes: u64,
}

#[cfg(unix)]
type SnapshotIdentity = (u64, u64);
#[cfg(windows)]
type SnapshotIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type SnapshotIdentity = ();

#[cfg(unix)]
type SnapshotRevision = (u64, i64, i64, i64, i64, u64, u32, u32, u32);
#[cfg(windows)]
type SnapshotRevision = (u64, u64, u64, u32, Option<u32>);
#[cfg(not(any(unix, windows)))]
type SnapshotRevision = ();

#[cfg(unix)]
fn snapshot_identity(metadata: &fs::Metadata) -> SnapshotIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn snapshot_identity(metadata: &fs::Metadata) -> SnapshotIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn snapshot_identity(_metadata: &fs::Metadata) -> SnapshotIdentity {}

#[cfg(unix)]
fn snapshot_revision(metadata: &fs::Metadata) -> SnapshotRevision {
    use std::os::unix::fs::MetadataExt as _;

    (
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.nlink(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
    )
}

#[cfg(windows)]
fn snapshot_revision(metadata: &fs::Metadata) -> SnapshotRevision {
    use std::os::windows::fs::MetadataExt as _;

    (
        metadata.file_size(),
        metadata.creation_time(),
        metadata.last_write_time(),
        metadata.file_attributes(),
        metadata.number_of_links(),
    )
}

#[cfg(not(any(unix, windows)))]
fn snapshot_revision(_metadata: &fs::Metadata) -> SnapshotRevision {}

#[cfg(unix)]
const fn snapshot_identity_available(_identity: SnapshotIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn snapshot_identity_available(identity: SnapshotIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn snapshot_identity_available(_identity: SnapshotIdentity) -> bool {
    false
}

fn snapshot_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(windows)]
fn snapshot_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn snapshot_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn snapshot_metadata_is_safe(metadata: &fs::Metadata, max_frame_bytes: u64) -> bool {
    let identity = snapshot_identity(metadata);
    !metadata.file_type().is_symlink()
        && !snapshot_is_reparse_point(metadata)
        && metadata.is_file()
        && snapshot_identity_available(identity)
        && snapshot_is_single_link(metadata)
        && metadata.len() <= max_frame_bytes
}

fn snapshot_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let identity = snapshot_identity(left);
    snapshot_identity_available(identity)
        && identity == snapshot_identity(right)
        && snapshot_revision(left) == snapshot_revision(right)
}

#[cfg(any(unix, windows))]
fn open_snapshot_nofollow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_snapshot_nofollow(_path: &Path) -> std::io::Result<File> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "stable serviced-candidate file identities are unsupported on this platform",
    ))
}

fn open_bound_snapshot(
    path: &Path,
    max_frame_bytes: u64,
) -> Result<Option<(File, fs::Metadata)>, String> {
    let path_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to inspect serviced-candidate snapshot {}: {error}",
                path.display()
            ));
        }
    };
    if !snapshot_metadata_is_safe(&path_before, max_frame_bytes) {
        return Err(format!(
            "serviced-candidate snapshot {} is not a bounded direct single-link regular file",
            path.display()
        ));
    }
    let file = open_snapshot_nofollow(path).map_err(|error| {
        format!(
            "failed to open serviced-candidate snapshot {} without following links: {error}",
            path.display()
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        format!(
            "failed to inspect opened serviced-candidate snapshot {}: {error}",
            path.display()
        )
    })?;
    if !snapshot_metadata_is_safe(&opened, max_frame_bytes)
        || !snapshot_metadata_unchanged(&path_before, &opened)
    {
        return Err(format!(
            "serviced-candidate snapshot {} changed identity while opening",
            path.display()
        ));
    }
    Ok(Some((file, opened)))
}

impl ServicedCandidateStore {
    /// Open the height-bound snapshot adjacent to `safety_wal_path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the derived geometry overflows or an existing
    /// snapshot is missing its canonical framing, checksum, ordering, or exact
    /// height-context binding.
    pub(crate) fn open(
        safety_wal_path: &Path,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        capacity: usize,
    ) -> Result<(Self, RestoredServicedCandidates), String> {
        if capacity == 0 {
            return Err("serviced-candidate capacity must be non-zero".to_owned());
        }
        let mut file_name = safety_wal_path
            .file_name()
            .ok_or_else(|| "safety WAL path has no file name".to_owned())?
            .to_os_string();
        file_name.push(".serviced-candidates");
        let path = safety_wal_path.with_file_name(file_name);
        let max_frame_bytes = u64::try_from(capacity)
            .map_err(|_| "serviced-candidate capacity is not representable".to_owned())?
            .checked_mul(RECORD_FRAME_HEADROOM_BYTES)
            .and_then(|bytes| bytes.checked_add(FIXED_FRAME_HEADROOM_BYTES))
            .ok_or_else(|| "serviced-candidate frame bound overflowed".to_owned())?;
        let store = Self {
            path,
            context_id,
            height,
            owner,
            capacity,
            max_frame_bytes,
        };
        let restored = store.load()?;
        Ok((store, restored))
    }

    fn load(&self) -> Result<RestoredServicedCandidates, String> {
        let Some((mut file, opened_before)) =
            open_bound_snapshot(&self.path, self.max_frame_bytes)?
        else {
            return Ok(RestoredServicedCandidates {
                records: BTreeMap::new(),
                decision_reclaimed: false,
            });
        };
        let read_limit = self
            .max_frame_bytes
            .checked_add(1)
            .ok_or_else(|| "serviced-candidate read bound overflowed".to_owned())?;
        let mut bytes =
            Vec::with_capacity(usize::try_from(opened_before.len()).unwrap_or_default());
        Read::by_ref(&mut file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                format!(
                    "failed to read serviced-candidate snapshot {}: {error}",
                    self.path.display()
                )
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_frame_bytes {
            return Err("serviced-candidate snapshot grew beyond its read bound".to_owned());
        }
        let opened_after = file.metadata().map_err(|error| {
            format!(
                "failed to reinspect opened serviced-candidate snapshot {}: {error}",
                self.path.display()
            )
        })?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to reinspect serviced-candidate snapshot {}: {error}",
                self.path.display()
            )
        })?;
        if !snapshot_metadata_is_safe(&opened_after, self.max_frame_bytes)
            || !snapshot_metadata_is_safe(&path_after, self.max_frame_bytes)
            || !snapshot_metadata_unchanged(&opened_before, &opened_after)
            || !snapshot_metadata_unchanged(&opened_before, &path_after)
            || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "serviced-candidate snapshot {} changed while reading",
                self.path.display()
            ));
        }
        let state = decode_frame(&bytes, self.max_frame_bytes)?;
        let expected_capacity = u64::try_from(self.capacity)
            .map_err(|_| "serviced-candidate capacity is not representable".to_owned())?;
        if state.context_id != self.context_id
            || state.height != self.height
            || state.owner != self.owner
            || state.capacity != expected_capacity
            || state.records.len() > self.capacity
            || state.records.iter().any(|record| {
                !record
                    .key
                    .belongs_to(self.context_id, self.height, self.owner)
            })
            || state
                .records
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
            || state.decision_reclaimed && !state.records.is_empty()
        {
            return Err(
                "serviced-candidate snapshot crossed its immutable context geometry".to_owned(),
            );
        }
        Ok(RestoredServicedCandidates {
            records: state
                .records
                .into_iter()
                .map(|record| (record.key, record.service_view))
                .collect(),
            decision_reclaimed: state.decision_reclaimed,
        })
    }

    /// Publish one complete canonical snapshot before its candidate owner retires.
    ///
    /// # Errors
    ///
    /// Returns an error when a record crosses the immutable store geometry or
    /// the checksummed atomic-replace publication cannot be synchronized.
    pub(crate) fn persist(
        &self,
        records: &BTreeMap<ServicedCandidateKey, wire::View>,
        decision_reclaimed: bool,
    ) -> Result<(), String> {
        if decision_reclaimed && !records.is_empty() {
            return Err(
                "a decision-reclaimed serviced-candidate snapshot must be empty".to_owned(),
            );
        }
        if records.len() > self.capacity
            || records
                .keys()
                .any(|record| !record.belongs_to(self.context_id, self.height, self.owner))
        {
            return Err(
                "serviced-candidate snapshot crossed its immutable context geometry".to_owned(),
            );
        }
        let state = PersistedServicedCandidates {
            format_version: FORMAT_VERSION,
            context_id: self.context_id,
            height: self.height,
            owner: self.owner,
            capacity: u64::try_from(self.capacity)
                .map_err(|_| "serviced-candidate capacity is not representable".to_owned())?,
            decision_reclaimed,
            records: records
                .iter()
                .map(|(key, service_view)| PersistedServicedCandidate {
                    key: *key,
                    service_view: *service_view,
                })
                .collect(),
        };
        let frame = encode_frame(&state, self.max_frame_bytes)?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "serviced-candidate snapshot path has no parent".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create serviced-candidate snapshot directory {}: {error}",
                parent.display()
            )
        })?;
        let temporary = temporary_path(&self.path)?;
        if let Ok(metadata) = fs::symlink_metadata(&temporary) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "serviced-candidate temporary path {} is not a regular file",
                    temporary.display()
                ));
            }
            fs::remove_file(&temporary).map_err(|error| {
                format!(
                    "failed to remove stale serviced-candidate temporary file {}: {error}",
                    temporary.display()
                )
            })?;
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "failed to create serviced-candidate temporary snapshot {}: {error}",
                    temporary.display()
                )
            })?;
        let publication = file
            .write_all(&frame)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&temporary, &self.path))
            .and_then(|()| File::open(parent))
            .and_then(|directory| directory.sync_all());
        if let Err(error) = publication {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "failed to publish serviced-candidate snapshot {}: {error}",
                self.path.display()
            ));
        }
        Ok(())
    }

    /// Remove and directory-sync the finalized height's obsolete snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or its containing directory cannot
    /// be synchronized and retired.
    pub(crate) fn retire(self) -> Result<(), String> {
        let Some((file, opened_before)) = open_bound_snapshot(&self.path, self.max_frame_bytes)?
        else {
            return Ok(());
        };
        file.sync_all().map_err(|error| {
            format!(
                "failed to sync serviced-candidate snapshot {} for retirement: {error}",
                self.path.display()
            )
        })?;
        let opened_after = file.metadata().map_err(|error| {
            format!(
                "failed to reinspect serviced-candidate snapshot {} for retirement: {error}",
                self.path.display()
            )
        })?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to bind serviced-candidate snapshot {} for retirement: {error}",
                self.path.display()
            )
        })?;
        if !snapshot_metadata_is_safe(&opened_after, self.max_frame_bytes)
            || !snapshot_metadata_is_safe(&path_after, self.max_frame_bytes)
            || !snapshot_metadata_unchanged(&opened_before, &opened_after)
            || !snapshot_metadata_unchanged(&opened_before, &path_after)
        {
            return Err(format!(
                "serviced-candidate snapshot {} changed before retirement",
                self.path.display()
            ));
        }
        drop(file);
        fs::remove_file(&self.path).map_err(|error| {
            format!(
                "failed to retire serviced-candidate snapshot {}: {error}",
                self.path.display()
            )
        })?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "serviced-candidate snapshot path has no parent".to_owned())?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!(
                    "failed to sync retired serviced-candidate directory {}: {error}",
                    parent.display()
                )
            })
    }

    /// Return the exact snapshot path for failure-injection tests.
    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> &Path {
        &self.path
    }
}

fn temporary_path(path: &Path) -> Result<PathBuf, String> {
    let mut name = path
        .file_name()
        .ok_or_else(|| "serviced-candidate snapshot path has no file name".to_owned())?
        .to_os_string();
    name.push(".tmp");
    Ok(path.with_file_name(name))
}

fn encode_frame(
    state: &PersistedServicedCandidates,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, String> {
    let payload = state.encode();
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| "serviced-candidate payload length overflowed".to_owned())?;
    let frame_len = u64::try_from(FRAME_HEADER_BYTES)
        .expect("serviced-candidate frame header fits u64")
        .checked_add(payload_len)
        .ok_or_else(|| "serviced-candidate frame length overflowed".to_owned())?;
    if frame_len > max_frame_bytes {
        return Err("serviced-candidate frame exceeds its derived byte bound".to_owned());
    }
    let mut frame = Vec::with_capacity(
        usize::try_from(frame_len)
            .map_err(|_| "serviced-candidate frame is not addressable".to_owned())?,
    );
    frame.extend_from_slice(FRAME_MAGIC);
    frame.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(Hash::new(&payload).as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_frame(bytes: &[u8], max_frame_bytes: u64) -> Result<PersistedServicedCandidates, String> {
    if bytes.len() < FRAME_HEADER_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_frame_bytes
        || bytes.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice())
    {
        return Err("serviced-candidate snapshot has an invalid frame header".to_owned());
    }
    let version_offset = FRAME_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| "serviced-candidate frame version is truncated".to_owned())?,
    );
    if version != FORMAT_VERSION {
        return Err(format!(
            "serviced-candidate snapshot uses unsupported version {version}"
        ));
    }
    let length_offset = version_offset + 2;
    let payload_len = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| "serviced-candidate frame length is truncated".to_owned())?,
    );
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| "serviced-candidate payload is not addressable".to_owned())?;
    let digest_offset = length_offset + 8;
    let payload_offset = digest_offset + HASH_BYTES;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err("serviced-candidate frame length is inconsistent".to_owned());
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
        return Err("serviced-candidate snapshot checksum mismatch".to_owned());
    }
    let mut cursor = payload;
    let state = PersistedServicedCandidates::decode_all(&mut cursor)
        .map_err(|error| format!("failed to decode serviced-candidate snapshot: {error}"))?;
    if state.format_version != FORMAT_VERSION || state.encode() != payload {
        return Err("serviced-candidate snapshot is not canonically encoded".to_owned());
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const OWNER_A: [u8; 32] = [0xA1; 32];
    const OWNER_B: [u8; 32] = [0xB2; 32];

    fn context() -> wire::HeightContext {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::peer::PeerId;

        let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::BlsNormal)
            .expect("deterministic validator");
        let roster = vec![wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        }];
        let context = wire::HeightContext {
            chain_id: "serviced-candidate-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 7,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(wire::SnapshotBootstrapAnchor {
                snapshot_height: 6,
                snapshot_block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"snapshot block",
                )),
                snapshot_block_creation_time_ms: 6_000,
                snapshot_state_hash: Hash::new(b"snapshot state"),
            }),
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [9; 32],
        };
        context.validate().expect("valid snapshot-bound context");
        context
    }

    fn successor_context(predecessor: &wire::HeightContext) -> wire::HeightContext {
        let round = wire::ConsensusRound {
            context_id: predecessor.id(),
            height: predecessor.height,
            view: 0,
        };
        let parent = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: predecessor
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash),
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"predecessor block",
                )),
                payload_hash: Hash::new(b"predecessor payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"predecessor parent state"),
                Hash::new(b"predecessor post state"),
                Hash::new(b"predecessor ordinary writes"),
                Hash::new(b"predecessor wire"),
            ),
            signers: vec![0],
            aggregate_signature: vec![0xA7; 96],
        };
        let mut successor = predecessor.clone();
        successor.height = predecessor
            .height
            .checked_add(1)
            .expect("fixture height has a successor");
        successor.parent_commit_qc = Some(parent);
        successor.snapshot_bootstrap = None;
        successor.validate().expect("valid successor context");
        assert_ne!(successor.id(), predecessor.id());
        successor
    }

    fn key(context: &wire::HeightContext, source_view: u64, evidence: u8) -> ServicedCandidateKey {
        ServicedCandidateKey::new(
            context.id(),
            context.height,
            OWNER_A,
            context.leader(source_view),
            source_view,
            Some([evidence; 32]),
            1,
            3,
            2,
            [evidence; 32],
        )
    }

    fn state(
        store: &ServicedCandidateStore,
        records: Vec<PersistedServicedCandidate>,
        decision_reclaimed: bool,
    ) -> PersistedServicedCandidates {
        PersistedServicedCandidates {
            format_version: FORMAT_VERSION,
            context_id: store.context_id,
            height: store.height,
            owner: store.owner,
            capacity: u64::try_from(store.capacity).expect("test capacity fits u64"),
            decision_reclaimed,
            records,
        }
    }

    fn write_frame(store: &ServicedCandidateStore, state: &PersistedServicedCandidates) {
        let frame = encode_frame(state, store.max_frame_bytes).expect("encode fixture frame");
        fs::write(store.path_for_test(), frame).expect("write fixture frame");
    }

    #[test]
    fn snapshot_roundtrips_and_rejects_a_b_a_resurrection() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("00000000000000000007.wal");
        let (store, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("open snapshot");
        assert!(restored.records.is_empty());
        let a = key(&context, 2, 1);
        let b = key(&context, 2, 2);
        assert_eq!(a.class(), 3);
        let service_view = 5;
        let mut records = BTreeMap::from([(a, service_view), (b, service_view)]);
        store.persist(&records, false).expect("persist A and B");
        assert_eq!(
            records.insert(a, service_view),
            Some(service_view),
            "A remains serviced after equal-rank B replacement"
        );
        let (_reopened, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("same-height reopen");
        assert_eq!(restored.records, records);
    }

    #[test]
    fn snapshot_rejects_corruption_stale_context_and_capacity_exhaustion() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("height.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open snapshot");
        let records = BTreeMap::from([(key(&context, 0, 1), 0)]);
        store.persist(&records, false).expect("persist record");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height + 1, OWNER_A, 1,)
                .is_err(),
            "stale height is rejected"
        );
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_B, 1).is_err(),
            "a snapshot cannot be transplanted between local validator owners"
        );
        assert!(
            store
                .persist(
                    &BTreeMap::from([(key(&context, 0, 1), 0), (key(&context, 0, 2), 0)]),
                    false,
                )
                .is_err(),
            "capacity exhaustion fails closed instead of evicting A"
        );
        let mut bytes = fs::read(store.path_for_test()).expect("read snapshot");
        let last = bytes.last_mut().expect("nonempty snapshot");
        *last ^= 1;
        fs::write(store.path_for_test(), bytes).expect("corrupt snapshot");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "checksum corruption is rejected"
        );
    }

    #[test]
    fn decision_reclamation_is_canonical_only_for_an_empty_snapshot() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("decision.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open snapshot");
        let record = key(&context, 0, 1);
        assert!(
            store.persist(&BTreeMap::from([(record, 0)]), true).is_err(),
            "Decision reclamation cannot coexist with an unreclaimed owner"
        );
        store
            .persist(&BTreeMap::new(), true)
            .expect("publish canonical reclaimed state");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("restore canonical reclaimed state");
        assert!(restored.records.is_empty());
        assert!(restored.decision_reclaimed);

        let forged = state(
            &store,
            vec![PersistedServicedCandidate {
                key: record,
                service_view: 0,
            }],
            true,
        );
        write_frame(&store, &forged);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a checksummed nonempty Decision-reclaimed mutation fails closed"
        );
    }

    #[test]
    fn snapshot_rejects_truncation_version_ordering_duplicates_and_oversize() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();

        let wal = directory.path().join("truncated.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open truncated fixture");
        let valid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: key(&context, 0, 1),
                service_view: 0,
            }],
            false,
        );
        let mut frame = encode_frame(&valid, store.max_frame_bytes).expect("encode valid frame");
        frame.pop();
        fs::write(store.path_for_test(), frame).expect("write truncated frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err()
        );

        let wal = directory.path().join("version.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open version fixture");
        let valid = state(&store, Vec::new(), false);
        let mut frame = encode_frame(&valid, store.max_frame_bytes).expect("encode valid frame");
        frame[FRAME_MAGIC.len()..FRAME_MAGIC.len() + 2]
            .copy_from_slice(&(FORMAT_VERSION - 1).to_le_bytes());
        fs::write(store.path_for_test(), frame).expect("write old-version frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "the prior schema version fails closed instead of being guessed"
        );

        for (name, records) in [
            (
                "unordered",
                vec![
                    PersistedServicedCandidate {
                        key: key(&context, 0, 2),
                        service_view: 0,
                    },
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 0,
                    },
                ],
            ),
            (
                "duplicate",
                vec![
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 0,
                    },
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 1,
                    },
                ],
            ),
        ] {
            let wal = directory.path().join(format!("{name}.wal"));
            let (store, _) =
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .expect("open ordering fixture");
            write_frame(&store, &state(&store, records, false));
            assert!(
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .is_err(),
                "{name} records must be rejected"
            );
        }

        let wal = directory.path().join("oversize.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open oversize fixture");
        let oversized_len =
            usize::try_from(store.max_frame_bytes + 1).expect("small fixture bound fits usize");
        fs::write(store.path_for_test(), vec![0; oversized_len]).expect("write oversized frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err()
        );
    }

    #[test]
    fn snapshot_rejects_nonregular_artifacts() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("directory.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("derive snapshot path");
        fs::create_dir(store.path_for_test()).expect("place directory at snapshot path");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_load_and_retire_never_follow_substituted_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("symlink.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open snapshot");
        store
            .persist(&BTreeMap::from([(key(&context, 0, 1), 0)]), false)
            .expect("persist target frame");
        let snapshot = store.path_for_test().to_path_buf();
        let hard_link = directory.path().join("hard-linked.snapshot");
        fs::hard_link(&snapshot, &hard_link).expect("create second link to snapshot");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "load must reject a multiply linked snapshot"
        );
        fs::remove_file(hard_link).expect("restore single-link fixture");

        let target = directory.path().join("target.snapshot");
        fs::rename(&snapshot, &target).expect("move direct frame to symlink target");
        let target_before = fs::read(&target).expect("read target before substitution");
        symlink(&target, &snapshot).expect("substitute snapshot symlink");

        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "load must reject a direct-path symlink"
        );
        assert!(
            store.retire().is_err(),
            "retirement must reject rather than follow a substituted symlink"
        );
        assert_eq!(
            fs::read(&target).expect("read target after rejected retirement"),
            target_before,
            "the symlink target remains untouched"
        );
        assert!(snapshot.is_symlink());
    }

    #[test]
    fn finalized_snapshot_retirement_leaves_successor_rollover_empty() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let successor = successor_context(&context);
        let wal = directory.path().join("00000000000000000007.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open finalized-height snapshot");
        store
            .persist(&BTreeMap::from([(key(&context, 0, 1), 0)]), false)
            .expect("persist finalized-height owner");
        store
            .persist(&BTreeMap::new(), true)
            .expect("atomically reclaim finalized-height owners");
        assert!(
            ServicedCandidateStore::open(&wal, successor.id(), successor.height, OWNER_A, 2,)
                .is_err(),
            "a predecessor snapshot cannot be transplanted into the successor context"
        );
        let snapshot_path = store.path_for_test().to_path_buf();
        store.retire().expect("retire finalized-height snapshot");
        assert!(!snapshot_path.exists());

        let successor_wal = directory.path().join("00000000000000000008.wal");
        let (_successor, restored) = ServicedCandidateStore::open(
            &successor_wal,
            successor.id(),
            successor.height,
            OWNER_A,
            2,
        )
        .expect("open independent successor path");
        assert!(restored.records.is_empty());
        assert!(!restored.decision_reclaimed);
    }
}

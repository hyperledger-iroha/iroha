//! Crash-atomic Kura lane-geometry transitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    num::NonZeroUsize,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use iroha_config::{
    kura::FsyncMode,
    parameters::actual::{LaneConfig, LaneConfigEntry},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{
        BlockHeader, SignedBlock,
        consensus::LaneBlockProposalV1,
        execution_context::{ExternalExecutionContext, ExternalExecutionRouteRole},
    },
    merge::MergeLedgerEntry,
    name::Name,
    nexus::{DataSpaceId, LaneId},
    transaction::signed::TransactionEntrypoint,
};
use norito::codec::{Decode, Encode};
#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
use rustix::fs::{CWD, RenameFlags, renameat_with};

use super::{
    AUTONOMOUS_LANE_BLOCKS_DATA_FILE, AUTONOMOUS_LANE_BLOCKS_INDEX_FILE,
    AutonomousLaneBlockArtifact, BlockStore, BlockStoreCommitMarker,
    CERTIFIED_LANE_BLOCKS_DATA_FILE, CERTIFIED_LANE_BLOCKS_INDEX_FILE, COUNT_FILE_NAME,
    DATA_FILE_NAME, Error, HASHES_FILE_NAME, INDEX_FILE_NAME, Kura, LANE_ARTIFACTS_DATA_FILE,
    LANE_ARTIFACTS_DIR_NAME, LANE_ARTIFACTS_INDEX_FILE, LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
    LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE, LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE, LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE, LaneBlockApplicationReceiptArtifact,
    LaneBlockApplicationReceiptArtifactFormat, MergeLedgerCarrierRecord, PIPELINE_INDEX_ENTRY_SIZE,
    RecoveredLaneBlockPayload, Result, SidecarIndexEntry, SidecarIndexLayout,
    create_dir_all_with_context, sync_dir,
};

const JOURNAL_VERSION: u8 = 6;
const MARKER_VERSION: u8 = 3;
const CHECKPOINT_VERSION: u8 = 4;
const JOURNAL_FILE_NAME: &str = "lane_geometry_journal.norito";
const JOURNAL_TEMP_FILE_NAME: &str = "lane_geometry_journal.norito.tmp";
const JOURNAL_RESTORE_TEMP_FILE_NAME: &str = "lane_geometry_journal.norito.restore.tmp";
#[cfg(test)]
const JOURNAL_IDENTITY_SWAP_FILE_NAME: &str = "lane_geometry_journal.norito.identity-swap";
#[cfg(test)]
const JOURNAL_IDENTITY_DISPLACED_FILE_NAME: &str =
    "lane_geometry_journal.norito.identity-displaced";
const MARKER_FILE_NAME: &str = ".lane-incarnation.norito";
const MARKER_TEMP_FILE_NAME: &str = ".lane-incarnation.norito.tmp";
const TRANSITION_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-transition:v3\0";
const CATALOG_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-catalog:v1\0";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-checkpoint:v3\0";
#[cfg(test)]
const UNSCOPED_LINEAGE_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-unscoped-lineage:v1\0";
const PENDING_GC_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-pending-gc:v3\0";
const MERGE_RELEASE_MARKERS_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-merge-markers:v1\0";
const MERGE_RELEASE_RECEIPT_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-merge-receipt:v1\0";
const GEOMETRY_MERGE_DIGEST_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-merge-digest:v1\0";
const GEOMETRY_BLOCK_STORE_DIGEST_DOMAIN: &[u8] =
    b"iroha:kura:lane-geometry-block-store-digest:v1\0";
const GC_QUARANTINE_PREFIX: &str = ".gc-";
const MAX_GEOMETRY_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GEOMETRY_TRANSITIONS: usize = 16_384;
const MAX_GEOMETRY_BINDINGS: usize = 65_536;
const MAX_GEOMETRY_MERGE_RELEASES: usize = 1_000_000;
const MAX_GEOMETRY_ARCHIVE_DEPTH: usize = 128;
const MAX_GEOMETRY_ARCHIVE_ENTRIES: usize = 4_000_000;
const MAX_LANE_MARKER_BYTES: u64 = 4 * 1024;
const MAX_BLOCK_STORE_COMMIT_MARKER_BYTES: u64 = 4 * 1024;
const MAX_LANE_RETIREMENT_ARTIFACT_FILES: usize = 65_536;
const MAX_LANE_RETIREMENT_WORK_ITEMS: usize = 65_536;

#[cfg(test)]
static CONFIGURED_CATALOG_PREFLIGHT_IDENTITY_SWAP: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
static CONFIGURED_CATALOG_PREFLIGHT_FAIL_AFTER_ESTABLISH: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
static GEOMETRY_MOVE_TARGET_COLLISION: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);

#[cfg(not(unix))]
static UNSUPPORTED_GEOMETRY_IDENTITY_NONCE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

const GC_FAIL_AFTER_COMPACTION_INTENT: usize = 1;
const GC_FAIL_AFTER_ARCHIVE_QUARANTINE: usize = 2;
const GC_FAIL_AFTER_ARCHIVE_DELETION: usize = 3;
const GC_FAIL_AFTER_COMPLETION: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneGeometryPhase {
    Intent,
    FilesApplied,
    CatalogPublished,
    RolledBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaneGeometryRecoveryCursor {
    #[cfg(test)]
    Catalog,
    AtHeight(u64),
    BeforeTransition(u64),
    BeforeFirstTransitionAtHeight(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneGeometryOperationKind {
    Create,
    Retire,
    Replace,
    Relabel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryMoveLocation {
    Source,
    Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryPairTargetKind {
    MutableLive,
    ImmutableRetained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryEvidencePolicy {
    AllowJournalIntentProvisioning,
    RequireDurableEvidence,
}

impl GeometryEvidencePolicy {
    const fn allows_journal_intent_provisioning(self) -> bool {
        matches!(self, Self::AllowJournalIntentProvisioning)
    }
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
fn rename_geometry_path_noreplace(source: &Path, target: &Path) -> std::io::Result<()> {
    renameat_with(CWD, source, CWD, target, RenameFlags::NOREPLACE).map_err(std::io::Error::from)
}

#[cfg(windows)]
fn rename_geometry_path_noreplace(_source: &Path, _target: &Path) -> std::io::Result<()> {
    // `std::fs::rename` uses `MOVEFILE_REPLACE_EXISTING` on Windows, so it cannot uphold the
    // authenticated no-clobber invariant for merge files. Fail closed until the Windows backend
    // provides a true atomic no-replace primitive.
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic no-clobber lane geometry rename is unsupported on Windows",
    ))
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox",
    windows
)))]
fn rename_geometry_path_noreplace(_source: &Path, _target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic no-clobber lane geometry rename is unsupported on this platform",
    ))
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryBinding {
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
    blocks_path: String,
    merge_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryOperation {
    kind: LaneGeometryOperationKind,
    lane_id: LaneId,
    previous: Option<LaneGeometryBinding>,
    updated: Option<LaneGeometryBinding>,
    archived_blocks_path: String,
    archived_merge_path: String,
    unpublished_blocks_path: String,
    unpublished_merge_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryIntent {
    transition_id: Hash,
    transition_sequence: u64,
    transition_height: u64,
    previous_catalog: Hash,
    previous_lineage_root: Hash,
    updated_catalog: Hash,
    updated_lineage_root: Hash,
    previous_bindings: Vec<LaneGeometryBinding>,
    updated_bindings: Vec<LaneGeometryBinding>,
    phase: LaneGeometryPhase,
    operations: Vec<LaneGeometryOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometrySnapshotCheckpoint {
    version: u8,
    snapshot_height: u64,
    snapshot_block_hash: Option<HashOf<BlockHeader>>,
    snapshot_state_hash: Hash,
    catalog: Hash,
    lineage_root: Hash,
    transition_sequence: Option<u64>,
    transition_height: Option<u64>,
    transition_previous_catalog: Option<Hash>,
    transition_previous_lineage_root: Option<Hash>,
    transition_id: Option<Hash>,
    bindings: Vec<LaneGeometryBinding>,
    merge_releases: Vec<LaneGeometryMergeRelease>,
    pending_archive_gc_root: Option<Hash>,
    commitment: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryPendingArchiveGc {
    intent: LaneGeometryIntent,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
struct LaneGeometryMergeRelease {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    application_block_height: u64,
    application_block_hash: HashOf<BlockHeader>,
    merge_entry_hash: HashOf<MergeLedgerEntry>,
    merge_epoch_id: u64,
    source_bundle_hash: Hash,
    batch_identity_hash: Hash,
    batch_hash: Hash,
    lane_execution_hash: Hash,
    marker_set_root: Hash,
    receipt_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LaneGeometryMergeCarrier {
    block_height: u64,
    block_hash: HashOf<BlockHeader>,
    entry_hash: HashOf<MergeLedgerEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LaneRetirementIdentity {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LanePayloadHintIdentity {
    proposal_hash: Hash,
    proposal_height: u64,
    proposal_view: u64,
    proposal_block_hash: HashOf<BlockHeader>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeometryFileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: Option<u32>,
    #[cfg(windows)]
    file_index: Option<u64>,
    #[cfg(not(unix))]
    unsupported_nonce: u64,
}

#[derive(Clone, Debug)]
/// Authenticated filesystem identities carried across configured-primary constructor opens.
pub(super) struct ConfiguredPrimaryGeometryPreflight {
    store_root: PathBuf,
    root_identity: GeometryFileIdentity,
    blocks_path: PathBuf,
    blocks_identity: Option<GeometryFileIdentity>,
    merge_path: PathBuf,
    merge_identity: Option<GeometryFileIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryJournal {
    version: u8,
    configured_catalog_hash: Option<Hash>,
    configured_primary_binding: Option<LaneGeometryBinding>,
    checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
    pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
    records: Vec<LaneGeometryIntent>,
}

impl Default for LaneGeometryJournal {
    fn default() -> Self {
        Self {
            version: JOURNAL_VERSION,
            configured_catalog_hash: None,
            configured_primary_binding: None,
            checkpoint: None,
            pending_archive_gc: Vec::new(),
            records: Vec::new(),
        }
    }
}

/// Result of one durable snapshot checkpoint and archive-GC pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaneGeometryGcSummary {
    /// Geometry transition records removed from the recoverable journal prefix.
    pub(crate) compacted_transitions: usize,
    /// Transition archive roots durably removed in this pass.
    pub(crate) removed_archive_roots: usize,
    /// Regular-file bytes removed from authenticated transition archive roots.
    pub(crate) reclaimed_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneIncarnationMarker {
    version: u8,
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
    move_target_blocks: Option<String>,
    move_target_merge: Option<String>,
    block_store_digest: Hash,
    merge_log_digest: Hash,
}

struct ConfiguredCatalogPreflightJournal {
    bytes: Vec<u8>,
    journal: LaneGeometryJournal,
    identity: GeometryFileIdentity,
}

fn configured_catalog_preflight_error(
    store_root: &Path,
    kind: ErrorKind,
    message: impl Into<String>,
) -> Error {
    Error::IO(
        std::io::Error::new(kind, message.into()),
        store_root.join(JOURNAL_FILE_NAME),
    )
}

fn configured_catalog_store_root_identity(store_root: &Path) -> Result<GeometryFileIdentity> {
    let metadata = fs::symlink_metadata(store_root)
        .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "Kura configured-catalog store root must be a non-symlink directory",
        ));
    }
    Ok(geometry_file_identity(&metadata))
}

fn configured_catalog_require_store_root_identity(
    store_root: &Path,
    expected: GeometryFileIdentity,
) -> Result<()> {
    let actual = configured_catalog_store_root_identity(store_root)?;
    if actual != expected {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "Kura configured-catalog store root changed during startup preflight",
        ));
    }
    Ok(())
}

fn read_configured_catalog_journal_for_preflight(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    path: &Path,
    inject_identity_swap: bool,
) -> Result<Option<ConfiguredCatalogPreflightJournal>> {
    if path.parent() != Some(store_root) {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidInput,
            "configured-catalog preflight file must be a direct child of the Kura store root",
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::IO(error, path.to_path_buf())),
    };
    if path_metadata.file_type().is_symlink() || !path_metadata.file_type().is_file() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog journal path is a symlink or has the wrong file type",
            ),
            path.to_path_buf(),
        ));
    }
    if path_metadata.len() > MAX_GEOMETRY_JOURNAL_BYTES {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "lane geometry journal exceeds the encoded byte limit",
            ),
            path.to_path_buf(),
        ));
    }

    let expected_identity = geometry_file_identity(&path_metadata);
    let mut file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !opened_metadata.is_file()
        || geometry_file_identity(&opened_metadata) != expected_identity
        || opened_metadata.len() > MAX_GEOMETRY_JOURNAL_BYTES
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "opened configured-catalog journal does not match its directory entry",
            ),
            path.to_path_buf(),
        ));
    }

    #[cfg(test)]
    if inject_identity_swap {
        let should_swap = {
            let mut hook = CONFIGURED_CATALOG_PREFLIGHT_IDENTITY_SWAP
                .lock()
                .expect("configured-catalog identity-swap hook lock");
            if hook.as_deref() == Some(path) {
                hook.take();
                true
            } else {
                false
            }
        };
        if should_swap {
            let replacement = store_root.join(JOURNAL_IDENTITY_SWAP_FILE_NAME);
            let displaced = store_root.join(JOURNAL_IDENTITY_DISPLACED_FILE_NAME);
            fs::rename(path, &displaced).map_err(|error| Error::IO(error, path.to_path_buf()))?;
            fs::rename(&replacement, path)
                .map_err(|error| Error::IO(error, replacement.to_path_buf()))?;
        }
    }
    #[cfg(not(test))]
    let _ = inject_identity_swap;

    let capacity = usize::try_from(opened_metadata.len())?;
    let mut bytes = Vec::with_capacity(capacity);
    (&mut file)
        .take(MAX_GEOMETRY_JOURNAL_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_GEOMETRY_JOURNAL_BYTES {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "lane geometry journal exceeded the encoded byte limit while being read",
            ),
            path.to_path_buf(),
        ));
    }
    let final_opened_metadata = file
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let final_path_metadata =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !final_opened_metadata.is_file()
        || geometry_file_identity(&final_opened_metadata) != expected_identity
        || final_opened_metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || final_path_metadata.file_type().is_symlink()
        || !final_path_metadata.file_type().is_file()
        || geometry_file_identity(&final_path_metadata) != expected_identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog journal changed during startup preflight",
            ),
            path.to_path_buf(),
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;

    let journal = decode_exact::<LaneGeometryJournal>(&bytes).map_err(Error::NoritoFrame)?;
    validate_lane_geometry_journal_structure(store_root, &journal)?;
    Ok(Some(ConfiguredCatalogPreflightJournal {
        bytes,
        journal,
        identity: expected_identity,
    }))
}

fn validate_configured_catalog_journal(
    store_root: &Path,
    journal: &LaneGeometryJournal,
    attempted: Hash,
) -> Result<()> {
    validate_lane_geometry_journal_structure(store_root, journal)?;
    match journal.configured_catalog_hash {
        Some(expected) if expected == attempted => Ok(()),
        Some(expected) => Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            format!(
                "configured lane catalog baseline mismatch: expected {expected}, attempted {attempted}"
            ),
        )),
        None => Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "existing lane geometry journal has no configured lane catalog baseline",
        )),
    }
}

fn preflight_configured_geometry_path(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    path: &Path,
    directory: bool,
) -> Result<bool> {
    let relative = path.strip_prefix(store_root).map_err(|_| {
        configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidInput,
            "configured geometry path escapes the Kura store root",
        )
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidInput,
            "configured geometry path is not a canonical store-root descendant",
        ));
    }

    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let mut current = store_root.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            unreachable!("validated normal path component")
        };
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                configured_catalog_require_store_root_identity(store_root, root_identity)?;
                return Ok(false);
            }
            Err(error) => return Err(Error::IO(error, current)),
        };
        if metadata.file_type().is_symlink() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "configured geometry path contains a symbolic link",
                ),
                current,
            ));
        }
        let is_target = index + 1 == components.len();
        let valid_type = if is_target {
            if directory {
                metadata.file_type().is_dir()
            } else {
                metadata.file_type().is_file()
            }
        } else {
            metadata.file_type().is_dir()
        };
        if !valid_type {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "configured geometry path has an unsafe file type",
                ),
                current,
            ));
        }
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    Ok(true)
}

fn configured_geometry_path_identity(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    path: &Path,
    directory: bool,
) -> Result<Option<GeometryFileIdentity>> {
    if !preflight_configured_geometry_path(store_root, root_identity, path, directory)? {
        return Ok(None);
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink()
        || if directory {
            !file_type.is_dir()
        } else {
            !file_type.is_file()
        }
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured geometry path changed after preflight",
            ),
            path.to_path_buf(),
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    Ok(Some(geometry_file_identity(&metadata)))
}

fn preflight_configured_store_tree(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
) -> Result<()> {
    let mut pending = vec![(store_root.to_path_buf(), 0_usize, root_identity)];
    let mut entries_seen = 0_usize;

    while let Some((directory, depth, expected_directory_identity)) = pending.pop() {
        configured_catalog_require_store_root_identity(store_root, root_identity)?;
        let before = fs::symlink_metadata(&directory)
            .map_err(|error| Error::IO(error, directory.clone()))?;
        if before.file_type().is_symlink()
            || !before.file_type().is_dir()
            || geometry_file_identity(&before) != expected_directory_identity
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "configured Kura directory changed during bounded tree preflight",
                ),
                directory,
            ));
        }

        let entries =
            fs::read_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, directory.clone()))?;
            entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured Kura tree entry count overflow",
                )
            })?;
            if entries_seen > MAX_GEOMETRY_ARCHIVE_ENTRIES {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured Kura tree exceeds its bounded entry count",
                ));
            }

            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| Error::IO(error, path.clone()))?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "configured Kura tree contains a symbolic link",
                    ),
                    path,
                ));
            }
            let identity = geometry_file_identity(&metadata);
            if file_type.is_dir() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    configured_catalog_preflight_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "configured Kura tree depth overflow",
                    )
                })?;
                if child_depth > MAX_GEOMETRY_ARCHIVE_DEPTH {
                    return Err(configured_catalog_preflight_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "configured Kura tree exceeds its bounded depth",
                    ));
                }
                pending.push((path, child_depth, identity));
            } else if file_type.is_file() {
                let file = File::open(&path).map_err(|error| Error::IO(error, path.clone()))?;
                let opened = file
                    .metadata()
                    .map_err(|error| Error::IO(error, path.clone()))?;
                let final_path =
                    fs::symlink_metadata(&path).map_err(|error| Error::IO(error, path.clone()))?;
                if !opened.is_file()
                    || final_path.file_type().is_symlink()
                    || !final_path.file_type().is_file()
                    || geometry_file_identity(&opened) != identity
                    || geometry_file_identity(&final_path) != identity
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "configured Kura file changed while being opened for preflight",
                        ),
                        path,
                    ));
                }
            } else {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "configured Kura tree contains an unsafe file type",
                    ),
                    path,
                ));
            }
        }

        let after = fs::symlink_metadata(&directory)
            .map_err(|error| Error::IO(error, directory.clone()))?;
        if after.file_type().is_symlink()
            || !after.file_type().is_dir()
            || geometry_file_identity(&after) != expected_directory_identity
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "configured Kura directory changed during bounded tree traversal",
                ),
                directory,
            ));
        }
    }

    configured_catalog_require_store_root_identity(store_root, root_identity)
}

fn read_preflight_file_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    read_preflight_file_bounded_with_identity(path, max_bytes).map(|(bytes, _)| bytes)
}

fn read_preflight_file_bounded_with_identity(
    path: &Path,
    max_bytes: u64,
) -> Result<(Vec<u8>, GeometryFileIdentity)> {
    let path_metadata =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.file_type().is_file() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured geometry evidence is not a regular file",
            ),
            path.to_path_buf(),
        ));
    }
    if path_metadata.len() > max_bytes {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured geometry evidence exceeds its encoded byte limit",
            ),
            path.to_path_buf(),
        ));
    }
    let expected_identity = geometry_file_identity(&path_metadata);
    let mut file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !opened_metadata.is_file() || geometry_file_identity(&opened_metadata) != expected_identity {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured geometry evidence changed while being opened",
            ),
            path.to_path_buf(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(path_metadata.len())?);
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured geometry evidence exceeds its encoded byte limit",
            ),
            path.to_path_buf(),
        ));
    }
    let final_path_metadata =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let final_open_metadata = file
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if final_path_metadata.file_type().is_symlink()
        || !final_path_metadata.file_type().is_file()
        || geometry_file_identity(&final_path_metadata) != expected_identity
        || geometry_file_identity(&final_open_metadata) != expected_identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured geometry evidence changed while being read",
            ),
            path.to_path_buf(),
        ));
    }
    Ok((bytes, expected_identity))
}

fn preflight_configured_journal_paths(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    let mut binding_sets = Vec::new();
    if let Some(binding) = journal.configured_primary_binding.as_ref() {
        binding_sets.push(std::slice::from_ref(binding));
    }
    if let Some(checkpoint) = journal.checkpoint.as_ref() {
        binding_sets.push(checkpoint.bindings.as_slice());
    }
    for pending in &journal.pending_archive_gc {
        binding_sets.push(pending.intent.previous_bindings.as_slice());
        binding_sets.push(pending.intent.updated_bindings.as_slice());
    }
    for record in &journal.records {
        binding_sets.push(record.previous_bindings.as_slice());
        binding_sets.push(record.updated_bindings.as_slice());
    }
    for bindings in binding_sets {
        for binding in bindings {
            preflight_configured_geometry_path(
                store_root,
                root_identity,
                &store_root.join(&binding.blocks_path),
                true,
            )?;
            preflight_configured_geometry_path(
                store_root,
                root_identity,
                &store_root.join(&binding.merge_path),
                false,
            )?;
        }
    }
    for intent in journal.records.iter().chain(
        journal
            .pending_archive_gc
            .iter()
            .map(|pending| &pending.intent),
    ) {
        for operation in &intent.operations {
            for (relative, directory) in [
                (&operation.archived_blocks_path, true),
                (&operation.archived_merge_path, false),
                (&operation.unpublished_blocks_path, true),
                (&operation.unpublished_merge_path, false),
            ] {
                preflight_configured_geometry_path(
                    store_root,
                    root_identity,
                    &store_root.join(relative),
                    directory,
                )?;
            }
        }
    }
    Ok(())
}

fn empty_geometry_merge_digest() -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GEOMETRY_MERGE_DIGEST_DOMAIN);
    hasher.update(&0_u64.to_le_bytes());
    Hash::prehashed(*hasher.finalize().as_bytes())
}

fn preflight_empty_block_store_without_marker(
    blocks_path: &Path,
    expected_marker: Option<&LaneGeometryBinding>,
    allow_durable_marker: bool,
) -> Result<()> {
    let count_temp_name = format!("{COUNT_FILE_NAME}.tmp");
    let lane_marker_temp_name = MARKER_TEMP_FILE_NAME;
    for entry in
        fs::read_dir(blocks_path).map_err(|error| Error::IO(error, blocks_path.to_path_buf()))?
    {
        let entry = entry.map_err(|error| Error::IO(error, blocks_path.to_path_buf()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| Error::IO(error, path.clone()))?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "unbound configured primary block store contains an unsafe entry",
                ),
                path,
            ));
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "unbound configured primary block store contains a non-UTF-8 entry",
                ),
                path.clone(),
            )
        })?;
        match name {
            INDEX_FILE_NAME | DATA_FILE_NAME | HASHES_FILE_NAME => {
                if entry
                    .metadata()
                    .map_err(|error| Error::IO(error, path.clone()))?
                    .len()
                    != 0
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "unbound configured primary block store is not empty",
                        ),
                        path,
                    ));
                }
            }
            COUNT_FILE_NAME => {
                let bytes =
                    read_preflight_file_bounded(&path, MAX_BLOCK_STORE_COMMIT_MARKER_BYTES)?;
                let marker = norito::decode_from_bytes::<BlockStoreCommitMarker>(&bytes)
                    .map_err(Error::NoritoFrame)?;
                if marker.version != BlockStoreCommitMarker::VERSION || marker.count != 0 {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "unbound configured primary block-store marker is not empty",
                        ),
                        path,
                    ));
                }
            }
            MARKER_FILE_NAME if allow_durable_marker => {
                let expected = expected_marker.ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "journal-owned empty block store has a marker without an expected binding",
                        ),
                        path.clone(),
                    )
                })?;
                let bytes = read_preflight_file_bounded(&path, MAX_LANE_MARKER_BYTES)?;
                let marker =
                    decode_exact::<LaneIncarnationMarker>(&bytes).map_err(Error::NoritoFrame)?;
                if marker.version != MARKER_VERSION
                    || marker.lane_id != expected.lane_id
                    || marker.incarnation != expected.incarnation
                    || marker.activation_height != expected.activation_height
                    || marker.move_target_blocks.is_some()
                    || marker.move_target_merge.is_some()
                    || marker.merge_log_digest != empty_geometry_merge_digest()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "journal-owned empty block-store marker is not an exact unsealed binding",
                        ),
                        path,
                    ));
                }
            }
            name if name == count_temp_name => {
                let bytes =
                    read_preflight_file_bounded(&path, MAX_BLOCK_STORE_COMMIT_MARKER_BYTES)?;
                let marker = norito::decode_from_bytes::<BlockStoreCommitMarker>(&bytes)
                    .map_err(Error::NoritoFrame)?;
                if marker.version != BlockStoreCommitMarker::VERSION || marker.count != 0 {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "unbound configured primary block-store temp marker is not empty",
                        ),
                        path,
                    ));
                }
            }
            name if name == lane_marker_temp_name => {
                let expected = expected_marker.ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "unbound configured primary block store contains a lane-marker temp",
                        ),
                        path.clone(),
                    )
                })?;
                let bytes = read_preflight_file_bounded(&path, MAX_LANE_MARKER_BYTES)?;
                let marker =
                    decode_exact::<LaneIncarnationMarker>(&bytes).map_err(Error::NoritoFrame)?;
                if marker.version != MARKER_VERSION
                    || marker.lane_id != expected.lane_id
                    || marker.incarnation != expected.incarnation
                    || marker.activation_height != expected.activation_height
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane-marker temp differs from the durable geometry binding",
                        ),
                        path,
                    ));
                }
            }
            _ => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "unbound configured primary block store contains an unexpected entry",
                    ),
                    path,
                ));
            }
        }
    }
    Ok(())
}

fn require_pristine_configured_catalog_root(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    allowed_publication_temp: Option<&ConfiguredCatalogPreflightJournal>,
) -> Result<()> {
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let allowed_path = allowed_publication_temp.map(|_| store_root.join(JOURNAL_TEMP_FILE_NAME));
    let mut saw_allowed_temp = false;
    for entry in
        fs::read_dir(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?
    {
        let entry = entry.map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
        let path = entry.path();
        if allowed_path.as_deref() == Some(path.as_path()) {
            let expected = allowed_publication_temp.expect("allowed path has a preflight value");
            let metadata = entry
                .metadata()
                .map_err(|error| Error::IO(error, path.clone()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if saw_allowed_temp
                || file_type.is_symlink()
                || !file_type.is_file()
                || geometry_file_identity(&metadata) != expected.identity
            {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured-catalog startup temp changed during pristine-root validation",
                ));
            }
            saw_allowed_temp = true;
            continue;
        }
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "cannot establish a configured-catalog baseline on a non-pristine Kura root",
            ),
            path,
        ));
    }
    if allowed_publication_temp.is_some() != saw_allowed_temp {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "configured-catalog startup temp disappeared during pristine-root validation",
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)
}

fn write_initial_configured_catalog_temp(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    temp_path: &Path,
    bytes: &[u8],
) -> Result<GeometryFileIdentity> {
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(temp_path)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    let identity = geometry_file_identity(
        &file
            .metadata()
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?,
    );
    file.write_all(bytes)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    file.sync_all()
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    let path_metadata = fs::symlink_metadata(temp_path)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.file_type().is_file()
        || geometry_file_identity(&path_metadata) != identity
        || path_metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog startup temp changed while being persisted",
            ),
            temp_path.to_path_buf(),
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    Ok(identity)
}

fn configured_catalog_reserved_temp_identity(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    temp_path: &Path,
) -> Result<Option<GeometryFileIdentity>> {
    if temp_path.parent() != Some(store_root)
        || !matches!(
            temp_path.file_name().and_then(std::ffi::OsStr::to_str),
            Some(JOURNAL_TEMP_FILE_NAME | JOURNAL_RESTORE_TEMP_FILE_NAME)
        )
    {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidInput,
            "configured-catalog cleanup path is not a reserved direct-child temp",
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let metadata = match fs::symlink_metadata(temp_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::IO(error, temp_path.to_path_buf())),
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog reserved temp is a symlink or has an unsafe file type",
            ),
            temp_path.to_path_buf(),
        ));
    }
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    Ok(Some(geometry_file_identity(&metadata)))
}

fn remove_uncommitted_configured_catalog_temp(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    temp_path: &Path,
    expected_identity: GeometryFileIdentity,
) -> Result<()> {
    let current_identity =
        configured_catalog_reserved_temp_identity(store_root, root_identity, temp_path)?
            .ok_or_else(|| {
                configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::NotFound,
                    "configured-catalog reserved temp disappeared before cleanup",
                )
            })?;
    if current_identity != expected_identity {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "configured-catalog reserved temp identity changed before cleanup",
        ));
    }
    fs::remove_file(temp_path).map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    sync_dir(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    configured_catalog_require_store_root_identity(store_root, root_identity)
}

fn promote_initial_configured_catalog_temp(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    temp_path: &Path,
    temp_identity: GeometryFileIdentity,
    journal_path: &Path,
    expected_bytes: &[u8],
) -> Result<()> {
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let temp_metadata = fs::symlink_metadata(temp_path)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    if temp_metadata.file_type().is_symlink()
        || !temp_metadata.file_type().is_file()
        || geometry_file_identity(&temp_metadata) != temp_identity
        || temp_metadata.len() != u64::try_from(expected_bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog startup temp identity changed before promotion",
            ),
            temp_path.to_path_buf(),
        ));
    }

    match fs::hard_link(temp_path, journal_path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let existing = read_configured_catalog_journal_for_preflight(
                store_root,
                root_identity,
                journal_path,
                false,
            )?
            .ok_or_else(|| Error::IO(error, journal_path.to_path_buf()))?;
            if existing.bytes != expected_bytes {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::AlreadyExists,
                    "a different configured-catalog journal won the startup establishment race",
                ));
            }
        }
        Err(error) => return Err(Error::IO(error, journal_path.to_path_buf())),
    }

    let journal_metadata = fs::symlink_metadata(journal_path)
        .map_err(|error| Error::IO(error, journal_path.to_path_buf()))?;
    if journal_metadata.file_type().is_symlink()
        || !journal_metadata.file_type().is_file()
        || geometry_file_identity(&journal_metadata) != temp_identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog journal does not retain the promoted temp identity",
            ),
            journal_path.to_path_buf(),
        ));
    }
    sync_dir(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?;

    let final_temp_metadata = fs::symlink_metadata(temp_path)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    if final_temp_metadata.file_type().is_symlink()
        || !final_temp_metadata.file_type().is_file()
        || geometry_file_identity(&final_temp_metadata) != temp_identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "configured-catalog startup temp changed before exact cleanup",
            ),
            temp_path.to_path_buf(),
        ));
    }
    fs::remove_file(temp_path).map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    sync_dir(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    configured_catalog_require_store_root_identity(store_root, root_identity)
}

impl Kura {
    /// Establish or authenticate the stable configured-catalog journal before
    /// Kura opens any lane-derived storage path.
    pub(super) fn establish_or_verify_configured_lane_catalog_baseline(
        store_root: &Path,
        attempted: Hash,
    ) -> Result<()> {
        create_dir_all_with_context(store_root)?;
        let root_identity = configured_catalog_store_root_identity(store_root)?;
        let journal_path = store_root.join(JOURNAL_FILE_NAME);
        let publication_temp_path = store_root.join(JOURNAL_TEMP_FILE_NAME);
        let restore_temp_path = store_root.join(JOURNAL_RESTORE_TEMP_FILE_NAME);

        let journal = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &journal_path,
            true,
        )?;
        if let Some(journal) = journal {
            validate_configured_catalog_journal(store_root, &journal.journal, attempted)?;
            for temp_path in [&publication_temp_path, &restore_temp_path] {
                if let Some(identity) =
                    configured_catalog_reserved_temp_identity(store_root, root_identity, temp_path)?
                {
                    let (temp_bytes, read_identity) = read_preflight_file_bounded_with_identity(
                        temp_path,
                        MAX_GEOMETRY_JOURNAL_BYTES,
                    )?;
                    if read_identity != identity {
                        return Err(configured_catalog_preflight_error(
                            store_root,
                            ErrorKind::InvalidData,
                            "configured-catalog reserved temp identity changed while being read",
                        ));
                    }
                    if temp_bytes == journal.bytes && read_identity != journal.identity {
                        return Err(configured_catalog_preflight_error(
                            store_root,
                            ErrorKind::InvalidData,
                            "configured-catalog byte-identical reserved temp lacks authoritative hard-link ownership",
                        ));
                    }
                    remove_uncommitted_configured_catalog_temp(
                        store_root,
                        root_identity,
                        temp_path,
                        read_identity,
                    )?;
                }
            }
            let authoritative = read_configured_catalog_journal_for_preflight(
                store_root,
                root_identity,
                &journal_path,
                false,
            )?
            .ok_or_else(|| {
                configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::NotFound,
                    "authoritative configured-catalog journal disappeared during temp cleanup",
                )
            })?;
            if authoritative.identity != journal.identity || authoritative.bytes != journal.bytes {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "authoritative configured-catalog journal changed during temp cleanup",
                ));
            }
            configured_catalog_require_store_root_identity(store_root, root_identity)?;
            return Ok(());
        }

        if configured_catalog_reserved_temp_identity(store_root, root_identity, &restore_temp_path)?
            .is_some()
        {
            return Err(configured_catalog_preflight_error(
                store_root,
                ErrorKind::InvalidData,
                "configured-catalog restore temp exists without its authoritative journal",
            ));
        }

        let publication_temp = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &publication_temp_path,
            false,
        )?;

        require_pristine_configured_catalog_root(
            store_root,
            root_identity,
            publication_temp.as_ref(),
        )?;

        let expected_journal = LaneGeometryJournal {
            configured_catalog_hash: Some(attempted),
            ..LaneGeometryJournal::default()
        };
        let expected_bytes = expected_journal.encode();
        let publication_temp_identity = if let Some(temp) = publication_temp {
            if temp.bytes != expected_bytes || temp.journal != expected_journal {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured-catalog startup temp is not the exact initial baseline journal",
                ));
            }
            temp.identity
        } else {
            write_initial_configured_catalog_temp(
                store_root,
                root_identity,
                &publication_temp_path,
                &expected_bytes,
            )?
        };

        let publication_temp = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &publication_temp_path,
            false,
        )?
        .ok_or_else(|| {
            configured_catalog_preflight_error(
                store_root,
                ErrorKind::NotFound,
                "configured-catalog startup temp disappeared before publication",
            )
        })?;
        require_pristine_configured_catalog_root(
            store_root,
            root_identity,
            Some(&publication_temp),
        )?;

        promote_initial_configured_catalog_temp(
            store_root,
            root_identity,
            &publication_temp_path,
            publication_temp_identity,
            &journal_path,
            &expected_bytes,
        )?;
        let established = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &journal_path,
            false,
        )?
        .ok_or_else(|| {
            configured_catalog_preflight_error(
                store_root,
                ErrorKind::NotFound,
                "configured-catalog baseline journal disappeared after establishment",
            )
        })?;
        if established.bytes != expected_bytes || established.journal != expected_journal {
            return Err(configured_catalog_preflight_error(
                store_root,
                ErrorKind::InvalidData,
                "established configured-catalog baseline journal differs from the exact startup value",
            ));
        }
        configured_catalog_require_store_root_identity(store_root, root_identity)
    }

    /// Authenticate every configured-primary and journal-derived path before Kura opens files.
    pub(super) fn preflight_configured_primary_geometry(
        store_root: &Path,
        primary: &LaneConfigEntry,
    ) -> Result<ConfiguredPrimaryGeometryPreflight> {
        let root_identity = configured_catalog_store_root_identity(store_root)?;
        let journal_path = store_root.join(JOURNAL_FILE_NAME);
        let journal = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &journal_path,
            false,
        )?
        .ok_or_else(|| {
            configured_catalog_preflight_error(
                store_root,
                ErrorKind::NotFound,
                "configured-primary preflight has no authenticated geometry journal",
            )
        })?;
        preflight_configured_journal_paths(store_root, root_identity, &journal.journal)?;

        let blocks = primary.blocks_dir(store_root);
        let merge = primary.merge_log_path(store_root);
        let blocks_exist =
            preflight_configured_geometry_path(store_root, root_identity, &blocks, true)?;
        let merge_exists =
            preflight_configured_geometry_path(store_root, root_identity, &merge, false)?;

        let expected = journal.journal.configured_primary_binding.as_ref();
        if let Some(expected) = expected {
            let expected_blocks = store_root.join(&expected.blocks_path);
            let expected_merge = store_root.join(&expected.merge_path);
            if expected.lane_id != primary.lane_id
                || expected.activation_height != 0
                || expected_blocks != blocks
                || expected_merge != merge
            {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured primary lane differs from its durable geometry binding",
                ));
            }
            if !blocks_exist || !merge_exists {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::NotFound,
                    "bound configured primary storage is missing",
                ));
            }
            let marker_path = blocks.join(MARKER_FILE_NAME);
            let marker_exists =
                preflight_configured_geometry_path(store_root, root_identity, &marker_path, false)?;
            if marker_exists {
                let blocks_identity = geometry_file_identity(
                    &fs::symlink_metadata(&blocks)
                        .map_err(|error| Error::IO(error, blocks.clone()))?,
                );
                let bytes = read_preflight_file_bounded(&marker_path, MAX_LANE_MARKER_BYTES)?;
                let marker =
                    decode_exact::<LaneIncarnationMarker>(&bytes).map_err(Error::NoritoFrame)?;
                if marker.version != MARKER_VERSION
                    || marker.lane_id != expected.lane_id
                    || marker.incarnation != expected.incarnation
                    || marker.activation_height != expected.activation_height
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "configured primary marker differs from its durable binding",
                        ),
                        marker_path,
                    ));
                }
                let final_blocks_metadata = fs::symlink_metadata(&blocks)
                    .map_err(|error| Error::IO(error, blocks.clone()))?;
                if final_blocks_metadata.file_type().is_symlink()
                    || !final_blocks_metadata.file_type().is_dir()
                    || geometry_file_identity(&final_blocks_metadata) != blocks_identity
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "configured primary block directory changed during marker authentication",
                        ),
                        blocks,
                    ));
                }
            } else {
                preflight_empty_block_store_without_marker(&blocks, Some(expected), false)?;
                if fs::symlink_metadata(&merge)
                    .map_err(|error| Error::IO(error, merge.clone()))?
                    .len()
                    != 0
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "markerless configured primary merge log is not empty",
                        ),
                        merge,
                    ));
                }
            }
        } else {
            if merge_exists && !blocks_exist {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "unbound configured primary storage is only partially present",
                ));
            }
            if blocks_exist {
                let marker_path = blocks.join(MARKER_FILE_NAME);
                if preflight_configured_geometry_path(
                    store_root,
                    root_identity,
                    &marker_path,
                    false,
                )? {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "configured primary marker has no durable geometry binding",
                        ),
                        marker_path,
                    ));
                }
                preflight_empty_block_store_without_marker(&blocks, None, false)?;
            }
            if merge_exists
                && fs::symlink_metadata(&merge)
                    .map_err(|error| Error::IO(error, merge.clone()))?
                    .len()
                    != 0
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "unbound configured primary merge log is not empty",
                    ),
                    merge,
                ));
            }
        }
        preflight_configured_store_tree(store_root, root_identity)?;
        let blocks_identity =
            configured_geometry_path_identity(store_root, root_identity, &blocks, true)?;
        let merge_identity =
            configured_geometry_path_identity(store_root, root_identity, &merge, false)?;
        configured_catalog_require_store_root_identity(store_root, root_identity)?;
        Ok(ConfiguredPrimaryGeometryPreflight {
            store_root: store_root.to_path_buf(),
            root_identity,
            blocks_path: blocks,
            blocks_identity,
            merge_path: merge,
            merge_identity,
        })
    }

    fn reverify_configured_primary_open_path(
        preflight: &mut ConfiguredPrimaryGeometryPreflight,
        path: &Path,
        directory: bool,
        establish_created: bool,
    ) -> Result<()> {
        let store_root = preflight.store_root.clone();
        preflight_configured_store_tree(&store_root, preflight.root_identity)?;
        let expected_path = if directory {
            &preflight.blocks_path
        } else {
            &preflight.merge_path
        };
        if path != expected_path {
            return Err(configured_catalog_preflight_error(
                &store_root,
                ErrorKind::InvalidInput,
                "configured primary constructor path differs from its authenticated path",
            ));
        }
        let actual = configured_geometry_path_identity(
            &store_root,
            preflight.root_identity,
            path,
            directory,
        )?;
        let expected = if directory {
            &mut preflight.blocks_identity
        } else {
            &mut preflight.merge_identity
        };
        match (*expected, actual, establish_created) {
            (Some(expected), Some(actual), _) if expected == actual => Ok(()),
            (None, None, false) => Ok(()),
            (None, Some(actual), true) => {
                *expected = Some(actual);
                Ok(())
            }
            (None, None, true) => Err(configured_catalog_preflight_error(
                &store_root,
                ErrorKind::NotFound,
                "configured primary constructor did not create its authenticated path",
            )),
            _ => Err(configured_catalog_preflight_error(
                &store_root,
                ErrorKind::InvalidData,
                "configured primary path identity changed across its constructor open",
            )),
        }
    }

    /// Reverify or establish the configured-primary block directory identity around an open.
    pub(super) fn reverify_configured_primary_blocks_open(
        preflight: &mut ConfiguredPrimaryGeometryPreflight,
        path: &Path,
        establish_created: bool,
    ) -> Result<()> {
        Self::reverify_configured_primary_open_path(preflight, path, true, establish_created)
    }

    /// Reverify or establish the configured-primary merge-log identity around an open.
    pub(super) fn reverify_configured_primary_merge_open(
        preflight: &mut ConfiguredPrimaryGeometryPreflight,
        path: &Path,
        establish_created: bool,
    ) -> Result<()> {
        Self::reverify_configured_primary_open_path(preflight, path, false, establish_created)
    }

    #[cfg(test)]
    pub(super) fn replace_configured_catalog_journal_after_open_for_test(store_root: &Path) {
        *CONFIGURED_CATALOG_PREFLIGHT_IDENTITY_SWAP
            .lock()
            .expect("configured-catalog identity-swap hook lock") =
            Some(store_root.join(JOURNAL_FILE_NAME));
    }

    #[cfg(test)]
    pub(super) fn fail_after_configured_catalog_preflight_for_test(store_root: &Path) {
        *CONFIGURED_CATALOG_PREFLIGHT_FAIL_AFTER_ESTABLISH
            .lock()
            .expect("configured-catalog crash hook lock") = Some(store_root.to_path_buf());
    }

    #[cfg(test)]
    pub(super) fn configured_catalog_preflight_crash_boundary(store_root: &Path) -> Result<()> {
        let should_fail = {
            let mut hook = CONFIGURED_CATALOG_PREFLIGHT_FAIL_AFTER_ESTABLISH
                .lock()
                .expect("configured-catalog crash hook lock");
            if hook.as_deref() == Some(store_root) {
                hook.take();
                true
            } else {
                false
            }
        };
        if should_fail {
            return Err(configured_catalog_preflight_error(
                store_root,
                ErrorKind::Interrupted,
                "configured-catalog startup crash boundary injected after baseline establishment",
            ));
        }
        Ok(())
    }

    /// Verify the exact process-configured lane-catalog baseline.
    ///
    /// This commitment is independent of physical geometry because display-only
    /// catalog fields may not change any path or incarnation commitment.
    pub(crate) fn verify_configured_lane_catalog_baseline(&self, attempted: Hash) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let journal = self.read_lane_geometry_journal()?;
        match journal.configured_catalog_hash {
            Some(expected) if expected == attempted => Ok(()),
            Some(expected) => Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!(
                    "configured lane catalog baseline mismatch: expected {expected}, attempted {attempted}"
                ),
            )),
            None => Err(self.geometry_error(
                ErrorKind::InvalidData,
                "durable chain has no configured lane catalog baseline",
            )),
        }
    }

    /// Read the exact configured-catalog baseline, if it has been initialized.
    pub(crate) fn configured_lane_catalog_baseline(&self) -> Result<Option<Hash>> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(None);
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        Ok(self.read_lane_geometry_journal()?.configured_catalog_hash)
    }

    /// Durably bind the configured primary segment before State publishes its marker.
    pub(crate) fn establish_or_verify_configured_primary_geometry_anchor(
        &self,
        primary: &LaneConfigEntry,
        incarnation: Hash,
        configured_catalog_hash: Hash,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let binding = LaneGeometryBinding {
            lane_id: primary.lane_id,
            incarnation,
            activation_height: 0,
            blocks_path: self.relative_geometry_path(&primary.blocks_dir(&self.store_root))?,
            merge_path: self.relative_geometry_path(&primary.merge_log_path(&self.store_root))?,
        };
        if binding.lane_id != LaneId::SINGLE
            || self.binding_blocks_path(&binding) != *self.active_blocks_dir.lock()
            || self.binding_merge_path(&binding) != *self.active_merge_path.lock()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "configured primary binding differs from Kura's active storage paths",
            ));
        }

        let mut journal = self.read_lane_geometry_journal()?;
        if journal.configured_catalog_hash != Some(configured_catalog_hash) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "configured primary anchor differs from the authenticated catalog baseline",
            ));
        }
        if journal.configured_catalog_hash.is_none() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "configured primary binding has no authenticated catalog baseline",
            ));
        }
        match journal.configured_primary_binding.as_ref() {
            Some(expected) if expected == &binding => {}
            Some(_) => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured primary binding differs from its durable anchor",
                ));
            }
            None => {
                journal.configured_primary_binding = Some(binding.clone());
                self.write_lane_geometry_journal(&journal)?;
            }
        }

        let marker_path = self.binding_blocks_path(&binding).join(MARKER_FILE_NAME);
        if self.validate_path_kind(&marker_path, false)? {
            self.require_lane_marker(&binding)
        } else {
            self.write_lane_marker(&binding)
        }
    }

    /// Apply one lane geometry transition under a durable, replayable intent.
    ///
    /// The intent remains in the journal after publication so a snapshot that
    /// predates several committed transitions can roll their filesystem effects
    /// back before block replay and deterministically reapply them afterwards.
    #[cfg(test)]
    pub(crate) fn apply_lane_geometry_transition(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        replaced_lane_ids: &BTreeSet<LaneId>,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            replaced_lane_ids,
            None,
        )
    }

    /// Apply a test geometry transition at its exact committed height.
    #[cfg(test)]
    pub(crate) fn apply_lane_geometry_transition_at_height(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        replaced_lane_ids: &BTreeSet<LaneId>,
        transition_height: u64,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            replaced_lane_ids,
            Some(transition_height),
        )
    }

    #[cfg(test)]
    fn apply_lane_geometry_transition_inner(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        replaced_lane_ids: &BTreeSet<LaneId>,
        transition_height: Option<u64>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }
        let previous_bindings =
            self.geometry_bindings(previous, previous_incarnations, previous_activation_heights)?;
        let updated_bindings =
            self.geometry_bindings(updated, updated_incarnations, updated_activation_heights)?;
        self.apply_lane_geometry_transition_with_lineage_roots_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            unscoped_lineage_root(&previous_bindings),
            unscoped_lineage_root(&updated_bindings),
            replaced_lane_ids,
            transition_height,
        )
    }

    /// Apply an authenticated geometry transition at its exact committed height.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_lane_geometry_transition_at_height_with_lineage_roots(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        previous_lineage_root: Hash,
        updated_lineage_root: Hash,
        replaced_lane_ids: &BTreeSet<LaneId>,
        transition_height: u64,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_with_lineage_roots_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            previous_lineage_root,
            updated_lineage_root,
            replaced_lane_ids,
            Some(transition_height),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_lane_geometry_transition_with_lineage_roots_inner(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        previous_lineage_root: Hash,
        updated_lineage_root: Hash,
        replaced_lane_ids: &BTreeSet<LaneId>,
        transition_height: Option<u64>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }
        self.ensure_nonzero_lineage_root(previous_lineage_root)?;
        self.ensure_nonzero_lineage_root(updated_lineage_root)?;
        let _geometry_guard = self.lane_geometry_lock.lock();

        let previous_bindings =
            self.geometry_bindings(previous, previous_incarnations, previous_activation_heights)?;
        let updated_bindings =
            self.geometry_bindings(updated, updated_incarnations, updated_activation_heights)?;
        let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
        let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
        let journal_was_present =
            self.validate_path_kind(&self.lane_geometry_journal_path(), false)?;
        let mut journal = self.read_lane_geometry_journal()?;
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        let current_applied_count = journal
            .records
            .iter()
            .position(|record| record.phase == LaneGeometryPhase::RolledBack)
            .unwrap_or(journal.records.len());
        let uncertain_index = journal.records.iter().position(|record| {
            matches!(
                record.phase,
                LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied
            )
        });
        let requested_transition_height = transition_height;
        let record_matches = |index: usize, height: Option<u64>| {
            journal.records.get(index).is_some_and(|record| {
                height.is_none_or(|height| record.transition_height == height)
                    && record.previous_catalog == previous_catalog
                    && record.previous_lineage_root == previous_lineage_root
                    && record.updated_catalog == updated_catalog
                    && record.updated_lineage_root == updated_lineage_root
            })
        };
        let frontier_retry = uncertain_index
            .filter(|index| record_matches(*index, requested_transition_height))
            .or_else(|| {
                (current_applied_count < journal.records.len()
                    && record_matches(current_applied_count, requested_transition_height))
                .then_some(current_applied_count)
            });
        let published_retry = current_applied_count.checked_sub(1).filter(|index| {
            let record = &journal.records[*index];
            record.phase == LaneGeometryPhase::CatalogPublished
                && record_matches(*index, requested_transition_height)
        });
        let retained_retry = frontier_retry.or(published_retry).or_else(|| {
            let mut matches = journal
                .records
                .iter()
                .enumerate()
                .filter_map(|(index, record)| {
                    (requested_transition_height.is_some_and(|height| {
                        record.transition_height == height
                            && record.previous_catalog == previous_catalog
                            && record.previous_lineage_root == previous_lineage_root
                            && record.updated_catalog == updated_catalog
                            && record.updated_lineage_root == updated_lineage_root
                    }))
                    .then_some(index)
                });
            let candidate = matches.next()?;
            matches.next().is_none().then_some(candidate)
        });
        let transition_height = match requested_transition_height {
            Some(height) => height,
            None => {
                if let Some(index) = retained_retry {
                    journal.records[index].transition_height
                } else if let Some(last) = journal.records.last() {
                    last.transition_height.checked_add(1).ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane geometry transition height overflow",
                        )
                    })?
                } else if let Some(checkpoint) = journal.checkpoint.as_ref() {
                    checkpoint.snapshot_height.checked_add(1).ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane geometry transition height overflow after checkpoint",
                        )
                    })?
                } else {
                    0
                }
            }
        };
        let existing_index = retained_retry
            .filter(|index| journal.records[*index].transition_height == transition_height);

        if previous_catalog == updated_catalog
            && previous_lineage_root == updated_lineage_root
            && existing_index.is_none()
        {
            let _sidecar_guard = self.sidecar_lock.lock();
            if requested_transition_height.is_none() {
                self.reconcile_lane_geometry_history(
                    &mut journal,
                    previous_catalog,
                    previous_lineage_root,
                )?;
            } else {
                self.reconcile_lane_geometry_history_to_count(
                    &mut journal,
                    previous_catalog,
                    previous_lineage_root,
                    current_applied_count,
                )?;
            }
            self.ensure_authoritative_lane_markers(
                previous,
                previous_incarnations,
                previous_activation_heights,
            )?;
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return if journal_was_present || journal != LaneGeometryJournal::default() {
                self.write_lane_geometry_journal(&journal)
            } else {
                Ok(())
            };
        }

        if let Some(published_index) = published_retry
            && existing_index == Some(published_index)
            && published_index + 1 == current_applied_count
        {
            let _sidecar_guard = self.sidecar_lock.lock();
            self.apply_geometry_operations_forward(
                &journal.records[published_index].operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
            self.ensure_authoritative_lane_markers(
                updated,
                updated_incarnations,
                updated_activation_heights,
            )?;
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }
        let desired_previous_count = existing_index.unwrap_or(current_applied_count);
        if existing_index.is_none() && current_applied_count != journal.records.len() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry cannot branch across a retained rolled-back transition",
            ));
        }
        let _sidecar_guard = self.sidecar_lock.lock();
        self.reconcile_lane_geometry_history_to_count(
            &mut journal,
            previous_catalog,
            previous_lineage_root,
            desired_previous_count,
        )?;
        self.ensure_authoritative_lane_markers(
            previous,
            previous_incarnations,
            previous_activation_heights,
        )?;
        if let Some(existing_index) = existing_index {
            let existing = &journal.records[existing_index];
            if existing.previous_catalog != previous_catalog
                || existing.previous_lineage_root != previous_lineage_root
                || existing.updated_catalog != updated_catalog
                || existing.updated_lineage_root != updated_lineage_root
                || existing.previous_bindings != previous_bindings
                || existing.updated_bindings != updated_bindings
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry transition id collides with a different exact identity",
                ));
            }
            let operations = journal.records[existing_index].operations.clone();
            let retiring = self.geometry_retirement_identities(previous, &operations)?;
            self.ensure_lane_retirement_admissible_locked(&retiring)?;
            // Keep the retained terminal phase until the replay finishes. Downgrading a
            // `RolledBack` record to `Intent` would let a crash erase the fact that subsequent
            // recovery must authenticate existing storage rather than provision an empty pair.
            self.apply_geometry_operations_forward(
                &operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
            journal.records[existing_index].phase = LaneGeometryPhase::FilesApplied;
            self.write_lane_geometry_journal(&journal)?;
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }

        let last_sequence = journal
            .records
            .iter()
            .map(|record| record.transition_sequence)
            .chain(
                journal
                    .pending_archive_gc
                    .iter()
                    .map(|pending| pending.intent.transition_sequence),
            )
            .chain(
                journal
                    .checkpoint
                    .iter()
                    .filter_map(|checkpoint| checkpoint.transition_sequence),
            )
            .max();
        let transition_sequence = match last_sequence {
            Some(sequence) => sequence.checked_add(1).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry transition sequence overflow",
                )
            })?,
            None => 0,
        };
        let transition_id = geometry_transition_id(
            transition_sequence,
            transition_height,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        );

        let operations = self.build_geometry_operations(
            transition_id,
            &previous_bindings,
            &updated_bindings,
            replaced_lane_ids,
        )?;
        let retiring = self.geometry_retirement_identities(previous, &operations)?;
        self.ensure_lane_retirement_admissible_locked(&retiring)?;
        let intent = LaneGeometryIntent {
            transition_id,
            transition_sequence,
            transition_height,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
            previous_bindings,
            updated_bindings,
            phase: LaneGeometryPhase::Intent,
            operations,
        };
        journal.records.push(intent);
        self.write_lane_geometry_journal(&journal)?;

        let record_index = journal.records.len() - 1;
        if let Err(error) = self.apply_geometry_operations_forward(
            &journal.records[record_index].operations,
            GeometryEvidencePolicy::AllowJournalIntentProvisioning,
        ) {
            if let Err(rollback_error) = self.apply_geometry_operations_rollback(
                &journal.records[record_index].operations,
                GeometryEvidencePolicy::AllowJournalIntentProvisioning,
            ) {
                return Err(Error::IO(
                    std::io::Error::other(format!(
                        "lane geometry apply failed ({error}); rollback failed ({rollback_error})"
                    )),
                    self.lane_geometry_journal_path(),
                ));
            }
            journal.records[record_index].phase = LaneGeometryPhase::RolledBack;
            self.write_lane_geometry_journal(&journal)?;
            return Err(error);
        }

        journal.records[record_index].phase = LaneGeometryPhase::FilesApplied;
        self.write_lane_geometry_journal(&journal)?;
        *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
        Ok(())
    }

    /// Mark the transition targeting the authoritative catalog as published.
    #[cfg(test)]
    pub(crate) fn mark_lane_geometry_catalog_published(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        configured_baseline: Option<Hash>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        self.mark_lane_geometry_catalog_published_with_lineage_root(
            authoritative,
            incarnations,
            activation_heights,
            unscoped_lineage_root(&bindings),
            configured_baseline,
        )
    }

    /// Mark the transition targeting the exact catalog and retained-lineage identity as published.
    pub(crate) fn mark_lane_geometry_catalog_published_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        configured_baseline: Option<Hash>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        self.ensure_nonzero_lineage_root(lineage_root)?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        #[cfg(test)]
        if self
            .fail_next_lane_geometry_publication
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(self.geometry_error(
                ErrorKind::Other,
                "lane geometry publication failed for test injection",
            ));
        }
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let fingerprint = geometry_catalog_fingerprint(&bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        if let Some(attempted) = configured_baseline {
            if journal.configured_catalog_hash != Some(attempted) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured catalog publication differs from its authenticated startup baseline",
                ));
            }
            let primary_binding = bindings.first().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured catalog publication has no primary geometry binding",
                )
            })?;
            if journal.configured_primary_binding.as_ref() != Some(primary_binding) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured catalog publication has no matching authenticated primary geometry anchor",
                ));
            }
            self.require_lane_marker(primary_binding)?;
        }
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        let journal_path = self.lane_geometry_journal_path();
        let publication_temp = self.store_root.join(JOURNAL_TEMP_FILE_NAME);
        let prior_journal_bytes = self.read_geometry_file_bytes(&journal_path)?;
        let publication_temp_preexisted = self.validate_path_kind(&publication_temp, false)?;
        let uncertain = journal.records.iter().position(|record| {
            matches!(
                record.phase,
                LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied
            )
        });
        if let Some(index) = uncertain {
            let record = &journal.records[index];
            if record.updated_catalog != fingerprint
                || record.updated_lineage_root != lineage_root
                || record.updated_bindings != bindings
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "catalog publication does not match the uncertain geometry identity",
                ));
            }
            journal.records[index].phase = LaneGeometryPhase::CatalogPublished;
        } else if !journal.records.is_empty() {
            let applied_count = journal
                .records
                .iter()
                .position(|record| record.phase == LaneGeometryPhase::RolledBack)
                .unwrap_or(journal.records.len());
            let current_matches = if applied_count == 0 {
                let record = &journal.records[0];
                record.previous_catalog == fingerprint
                    && record.previous_lineage_root == lineage_root
                    && record.previous_bindings == bindings
            } else {
                let record = &journal.records[applied_count - 1];
                record.updated_catalog == fingerprint
                    && record.updated_lineage_root == lineage_root
                    && record.updated_bindings == bindings
            };
            if !current_matches {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "catalog publication does not match the durable geometry frontier identity",
                ));
            }
        } else if journal.checkpoint.as_ref().is_some_and(|checkpoint| {
            checkpoint.catalog != fingerprint
                || checkpoint.lineage_root != lineage_root
                || checkpoint.bindings != bindings
        }) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "catalog publication does not match the compacted geometry identity",
            ));
        }
        if let Some(attempted) = configured_baseline {
            match journal.configured_catalog_hash {
                Some(expected) if expected == attempted => {}
                None => {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "configured catalog publication has no authenticated startup baseline",
                    ));
                }
                Some(expected) => {
                    return Err(self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!(
                            "configured lane catalog baseline mismatch: expected {expected}, attempted {attempted}"
                        ),
                    ));
                }
            }
            let primary_binding = bindings.first().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured catalog publication has no primary geometry binding",
                )
            })?;
            if primary_binding.lane_id != LaneId::SINGLE || primary_binding.activation_height != 0 {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured primary geometry binding is not lane zero at activation zero",
                ));
            }
            match journal.configured_primary_binding.as_ref() {
                Some(expected) if expected == primary_binding => {}
                None => {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "configured catalog publication has no authenticated primary geometry anchor",
                    ));
                }
                Some(_) => {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "configured primary geometry binding differs from its durable anchor",
                    ));
                }
            }
            self.require_lane_marker(primary_binding)?;
        }
        self.validate_lane_geometry_journal(&journal)?;
        let published_journal_bytes = journal.encode();
        // Use the same encoded bytes for the target replacement and rollback comparison. This
        // makes the exact value whose publication was attempted explicit even if the encoder is
        // changed in the future.
        let publication_result = self.atomic_write_geometry_file(
            &journal_path,
            &publication_temp,
            &published_journal_bytes,
        );
        #[cfg(test)]
        let publication_result = publication_result.and_then(|()| {
            if self
                .fail_next_lane_geometry_publication_after_write
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Err(self.geometry_error(
                    ErrorKind::Other,
                    "lane geometry publication failed after journal replacement for test injection",
                ));
            }
            Ok(())
        });
        if let Err(publication_error) = publication_result {
            if let Err(restore_error) = self.restore_lane_geometry_journal_file(
                prior_journal_bytes.as_deref(),
                &published_journal_bytes,
                publication_temp_preexisted,
            ) {
                return Err(Error::LaneGeometryPublicationRestoreFailed {
                    publication: publication_error.to_string(),
                    restoration: restore_error.to_string(),
                });
            }
            return Err(publication_error);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn fail_next_lane_geometry_publication_for_test(&self) {
        self.fail_next_lane_geometry_publication
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn fail_next_lane_geometry_publication_after_write_for_test(&self) {
        self.fail_next_lane_geometry_publication_after_write
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn lane_geometry_journal_state_for_test(
        &self,
    ) -> Result<(Option<Hash>, Vec<&'static str>, bool)> {
        let _geometry_guard = self.lane_geometry_lock.lock();
        let journal = self.read_lane_geometry_journal()?;
        let phases = journal
            .records
            .iter()
            .map(|record| match record.phase {
                LaneGeometryPhase::Intent => "intent",
                LaneGeometryPhase::FilesApplied => "files_applied",
                LaneGeometryPhase::CatalogPublished => "catalog_published",
                LaneGeometryPhase::RolledBack => "rolled_back",
            })
            .collect();
        let has_temp = self
            .validate_path_kind(&self.store_root.join(JOURNAL_TEMP_FILE_NAME), false)?
            || self
                .validate_path_kind(&self.store_root.join(JOURNAL_RESTORE_TEMP_FILE_NAME), false)?;
        Ok((journal.configured_catalog_hash, phases, has_temp))
    }

    /// Recover every retained geometry intent against a restored authoritative catalog.
    #[cfg(test)]
    pub(crate) fn recover_lane_geometry_journal(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<()> {
        self.recover_lane_geometry_journal_inner(
            authoritative,
            incarnations,
            activation_heights,
            LaneGeometryRecoveryCursor::Catalog,
        )
    }

    #[cfg(test)]
    pub(crate) fn recover_lane_geometry_journal_at_height(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        authoritative_height: u64,
    ) -> Result<()> {
        self.recover_lane_geometry_journal_inner(
            authoritative,
            incarnations,
            activation_heights,
            LaneGeometryRecoveryCursor::AtHeight(authoritative_height),
        )
    }

    /// Recover to the exact cursor before every retained transition at `transition_height`.
    #[cfg(test)]
    pub(crate) fn recover_lane_geometry_journal_before_first_transition_at_height(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        transition_height: u64,
    ) -> Result<()> {
        self.recover_lane_geometry_journal_inner(
            authoritative,
            incarnations,
            activation_heights,
            LaneGeometryRecoveryCursor::BeforeFirstTransitionAtHeight(transition_height),
        )
    }

    #[cfg(test)]
    fn recover_lane_geometry_journal_inner(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        cursor: LaneGeometryRecoveryCursor,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() =
                Self::lane_storage_entries_from_config(authoritative);
            return Ok(());
        }
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        self.recover_lane_geometry_journal_with_lineage_root_inner(
            authoritative,
            incarnations,
            activation_heights,
            unscoped_lineage_root(&bindings),
            cursor,
        )
    }

    /// Recover the authenticated geometry identity at an exact committed height.
    pub(crate) fn recover_lane_geometry_journal_at_height_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        authoritative_height: u64,
        lineage_root: Hash,
    ) -> Result<()> {
        self.recover_lane_geometry_journal_with_lineage_root_inner(
            authoritative,
            incarnations,
            activation_heights,
            lineage_root,
            LaneGeometryRecoveryCursor::AtHeight(authoritative_height),
        )
    }

    /// Recover the authenticated cursor immediately before its transition.
    pub(crate) fn recover_lane_geometry_journal_before_transition_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        transition_height: u64,
    ) -> Result<()> {
        self.recover_lane_geometry_journal_with_lineage_root_inner(
            authoritative,
            incarnations,
            activation_heights,
            lineage_root,
            LaneGeometryRecoveryCursor::BeforeTransition(transition_height),
        )
    }

    /// Recover the authenticated cursor before every transition at one height.
    pub(crate) fn recover_lane_geometry_journal_before_first_transition_at_height_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        transition_height: u64,
    ) -> Result<()> {
        self.recover_lane_geometry_journal_with_lineage_root_inner(
            authoritative,
            incarnations,
            activation_heights,
            lineage_root,
            LaneGeometryRecoveryCursor::BeforeFirstTransitionAtHeight(transition_height),
        )
    }

    fn recover_lane_geometry_journal_with_lineage_root_inner(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        cursor: LaneGeometryRecoveryCursor,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() =
                Self::lane_storage_entries_from_config(authoritative);
            return Ok(());
        }
        self.ensure_nonzero_lineage_root(lineage_root)?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let fingerprint = geometry_catalog_fingerprint(&bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        match cursor {
            #[cfg(test)]
            LaneGeometryRecoveryCursor::Catalog => {
                self.reconcile_lane_geometry_history(&mut journal, fingerprint, lineage_root)?;
            }
            LaneGeometryRecoveryCursor::AtHeight(authoritative_height) => {
                let desired_applied_count = journal
                    .records
                    .iter()
                    .take_while(|record| record.transition_height <= authoritative_height)
                    .count();
                self.reconcile_lane_geometry_history_to_count(
                    &mut journal,
                    fingerprint,
                    lineage_root,
                    desired_applied_count,
                )?;
            }
            LaneGeometryRecoveryCursor::BeforeFirstTransitionAtHeight(transition_height) => {
                let desired_applied_count = journal
                    .records
                    .iter()
                    .take_while(|record| record.transition_height < transition_height)
                    .count();
                self.reconcile_lane_geometry_history_to_count(
                    &mut journal,
                    fingerprint,
                    lineage_root,
                    desired_applied_count,
                )?;
            }
            LaneGeometryRecoveryCursor::BeforeTransition(transition_height) => {
                let mut matching =
                    journal
                        .records
                        .iter()
                        .enumerate()
                        .filter_map(|(index, record)| {
                            (record.transition_height == transition_height
                                && record.previous_catalog == fingerprint
                                && record.previous_lineage_root == lineage_root)
                                .then_some(index)
                        });
                let candidate = matching.next();
                if candidate.is_some() && matching.next().is_some() {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "authoritative geometry is ambiguous before transitions at the requested height",
                    ));
                }
                let desired_applied_count = candidate.unwrap_or_else(|| {
                    journal
                        .records
                        .iter()
                        .take_while(|record| record.transition_height < transition_height)
                        .count()
                });
                self.reconcile_lane_geometry_history_to_count(
                    &mut journal,
                    fingerprint,
                    lineage_root,
                    desired_applied_count,
                )?;
            }
        }
        self.ensure_authoritative_lane_markers(authoritative, incarnations, activation_heights)?;
        *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(authoritative);
        self.write_lane_geometry_journal(&journal)
    }

    /// Checkpoint recoverable lane geometry after a complete snapshot bundle is durable.
    ///
    /// The caller must invoke this only after the snapshot data, digest, signature, Merkle
    /// metadata, and snapshot directory entry have all been synchronized. Kura independently
    /// joins the supplied snapshot identity to its canonical block hash and WSV checkpoint before
    /// compacting any transition history. Archive deletion is then replayable from the compacted
    /// journal and can never run ahead of that checkpoint.
    ///
    /// # Errors
    /// Returns an error without deleting recovery evidence when the snapshot identity is stale,
    /// does not match the durable block/WSV checkpoint, names an unreachable geometry catalog, or
    /// archive contents fail authenticated path/type validation.
    #[cfg(test)]
    pub(crate) fn checkpoint_lane_geometry_after_durable_snapshot(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
        snapshot_smart_contract_state: &BTreeMap<Name, Vec<u8>>,
    ) -> Result<LaneGeometryGcSummary> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        self.checkpoint_lane_geometry_after_durable_snapshot_with_lineage_root(
            authoritative,
            incarnations,
            activation_heights,
            unscoped_lineage_root(&bindings),
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
            snapshot_smart_contract_state,
        )
    }

    /// Checkpoint recoverable geometry against an exact retained-lineage identity.
    pub(crate) fn checkpoint_lane_geometry_after_durable_snapshot_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
        snapshot_smart_contract_state: &BTreeMap<Name, Vec<u8>>,
    ) -> Result<LaneGeometryGcSummary> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        self.ensure_nonzero_lineage_root(lineage_root)?;
        if snapshot_height == 0
            || snapshot_block_hash.is_none()
            || snapshot_state_hash.as_ref().iter().all(|byte| *byte == 0)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry GC requires a non-genesis snapshot with a block and state hash",
            ));
        }
        self.validate_durable_geometry_snapshot_identity(
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
        )?;

        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let merge_releases = self.geometry_merge_releases_proven_by_snapshot(
            snapshot_height,
            snapshot_smart_contract_state,
        )?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        self.checkpoint_lane_geometry_with_proven_snapshot(
            bindings,
            lineage_root,
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
            merge_releases,
        )
    }

    /// Resume archive deletions that were proven safe by an already durable checkpoint.
    ///
    /// This never creates a new checkpoint or broadens the deletable set, so storage-budget
    /// maintenance may call it safely. A missing/corrupt journal or tampered archive fails closed.
    pub(super) fn resume_proven_lane_geometry_archive_gc(&self) -> Result<LaneGeometryGcSummary> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let mut journal = self.read_lane_geometry_journal()?;
        self.finish_pending_lane_geometry_gc_locked(&mut journal)
    }

    fn checkpoint_lane_geometry_with_proven_snapshot(
        &self,
        bindings: Vec<LaneGeometryBinding>,
        lineage_root: Hash,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
        mut merge_releases: Vec<LaneGeometryMergeRelease>,
    ) -> Result<LaneGeometryGcSummary> {
        self.ensure_nonzero_lineage_root(lineage_root)?;
        self.validate_durable_geometry_snapshot_identity(
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
        )?;
        let catalog = geometry_catalog_fingerprint(&bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        let mut summary = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;

        if let Some(existing) = journal.checkpoint.as_ref() {
            if snapshot_height < existing.snapshot_height {
                return Err(self.geometry_error(
                    ErrorKind::InvalidInput,
                    "refusing a stale lane geometry snapshot checkpoint",
                ));
            }
            if snapshot_height == existing.snapshot_height
                && (snapshot_block_hash != existing.snapshot_block_hash
                    || snapshot_state_hash != existing.snapshot_state_hash
                    || catalog != existing.catalog
                    || lineage_root != existing.lineage_root
                    || bindings != existing.bindings)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry snapshot checkpoint identity changed at the same height",
                ));
            }
        }

        if journal.records.is_empty() && journal.checkpoint.is_none() {
            // There is no transition history or recovery archive to compact. Avoid creating a
            // sidecar solely for the static genesis geometry.
            return Ok(summary);
        }

        let prune_count = journal
            .records
            .iter()
            .take_while(|record| record.transition_height <= snapshot_height)
            .count();
        let (catalog_at_snapshot, lineage_root_at_snapshot) = if prune_count > 0 {
            let latest = &journal.records[prune_count - 1];
            (latest.updated_catalog, latest.updated_lineage_root)
        } else if let Some(first) = journal.records.first() {
            (first.previous_catalog, first.previous_lineage_root)
        } else if let Some(checkpoint) = journal.checkpoint.as_ref() {
            (checkpoint.catalog, checkpoint.lineage_root)
        } else {
            (catalog, lineage_root)
        };
        if catalog_at_snapshot != catalog || lineage_root_at_snapshot != lineage_root {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "snapshot geometry catalog or lineage does not match transition history at its exact height",
            ));
        }

        let exact_bindings_match = if prune_count > 0 {
            journal.records[prune_count - 1].updated_bindings == bindings
        } else if let Some(first) = journal.records.first() {
            first.previous_bindings == bindings
        } else {
            journal
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.bindings == bindings)
        };
        if !exact_bindings_match
            || journal.records[..prune_count]
                .iter()
                .any(|record| record.phase != LaneGeometryPhase::CatalogPublished)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "snapshot geometry does not exactly match published transition history",
            ));
        }
        if bindings
            .iter()
            .any(|binding| binding.activation_height > snapshot_height)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "snapshot predates an active lane incarnation",
            ));
        }

        let (
            transition_sequence,
            transition_height,
            transition_previous_catalog,
            transition_previous_lineage_root,
            transition_id,
        ) = if prune_count > 0 {
            let latest = &journal.records[prune_count - 1];
            (
                Some(latest.transition_sequence),
                Some(latest.transition_height),
                Some(latest.previous_catalog),
                Some(latest.previous_lineage_root),
                Some(latest.transition_id),
            )
        } else {
            journal
                .checkpoint
                .as_ref()
                .map_or((None, None, None, None, None), |checkpoint| {
                    (
                        checkpoint.transition_sequence,
                        checkpoint.transition_height,
                        checkpoint.transition_previous_catalog,
                        checkpoint.transition_previous_lineage_root,
                        checkpoint.transition_id,
                    )
                })
        };
        let pending_archive_gc = journal.records[..prune_count]
            .iter()
            .map(|record| LaneGeometryPendingArchiveGc {
                intent: record.clone(),
            })
            .collect::<Vec<_>>();
        let archived_incarnations = pending_archive_gc
            .iter()
            .flat_map(|pending| &pending.intent.operations)
            .flat_map(|operation| operation.previous.iter().chain(operation.updated.iter()))
            .map(|binding| (binding.lane_id, binding.incarnation))
            .collect::<BTreeSet<_>>();
        merge_releases.retain(|release| {
            archived_incarnations.contains(&(release.lane_id, release.lane_incarnation))
        });
        let pending_archive_gc_root = (!pending_archive_gc.is_empty())
            .then(|| geometry_pending_archive_gc_root(&pending_archive_gc));
        let checkpoint = lane_geometry_snapshot_checkpoint(
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
            bindings,
            lineage_root,
            transition_sequence,
            transition_height,
            transition_previous_catalog,
            transition_previous_lineage_root,
            transition_id,
            merge_releases,
            pending_archive_gc_root,
        );
        self.validate_lane_geometry_checkpoint(&checkpoint)?;

        journal.records.drain(..prune_count);
        journal.checkpoint = Some(checkpoint);
        journal.pending_archive_gc = pending_archive_gc;
        self.validate_lane_geometry_journal(&journal)?;

        if matches!(self.sidecar_fsync_mode(), FsyncMode::Off) {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "lane geometry archive GC requires durable sidecar fsync",
            ));
        }
        self.write_lane_geometry_journal(&journal)?;
        // Publishing the compaction intent can grow the journal. Refresh before the first
        // deletion so any later partial failure leaves conservative (never low) accounting.
        let _ = self.refresh_disk_usage_bytes()?;
        summary.compacted_transitions = summary.compacted_transitions.saturating_add(prune_count);
        self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT)?;

        let finished = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        summary.removed_archive_roots = summary
            .removed_archive_roots
            .saturating_add(finished.removed_archive_roots);
        summary.reclaimed_bytes = summary
            .reclaimed_bytes
            .saturating_add(finished.reclaimed_bytes);
        Ok(summary)
    }

    fn finish_pending_lane_geometry_gc_locked(
        &self,
        journal: &mut LaneGeometryJournal,
    ) -> Result<LaneGeometryGcSummary> {
        if journal.pending_archive_gc.is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        if matches!(self.sidecar_fsync_mode(), FsyncMode::Off) {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "lane geometry archive GC requires durable sidecar fsync",
            ));
        }
        self.validate_lane_geometry_journal(journal)?;
        let checkpoint = journal
            .checkpoint
            .as_ref()
            .expect("validated pending geometry GC has a checkpoint");
        self.validate_durable_geometry_snapshot_identity(
            checkpoint.snapshot_height,
            checkpoint.snapshot_block_hash,
            checkpoint.snapshot_state_hash,
        )?;
        let pending = journal.pending_archive_gc.clone();
        let merge_releases = journal
            .checkpoint
            .as_ref()
            .expect("validated pending geometry GC has a checkpoint")
            .merge_releases
            .clone();
        let mut summary = LaneGeometryGcSummary::default();
        for archive in &pending {
            let (bytes, existed) = match self
                .remove_authenticated_geometry_archive(archive, &merge_releases)
            {
                Ok(removed) => removed,
                Err(error) => {
                    self.refresh_disk_usage_bytes().map_err(|refresh_error| {
                            self.geometry_error_owned(
                                ErrorKind::Other,
                                format!(
                                    "lane geometry archive GC failed ({error}); exact disk-usage repair also failed ({refresh_error})"
                                ),
                            )
                        })?;
                    return Err(error);
                }
            };
            summary.reclaimed_bytes = summary.reclaimed_bytes.saturating_add(bytes);
            summary.removed_archive_roots = summary
                .removed_archive_roots
                .saturating_add(usize::from(existed));
        }
        // Refuse to acknowledge deletion if accounting sees any unsafe or unreadable geometry
        // entry. Keeping the pending intent makes the already-deleted subset replayable.
        let _ = self.kura_disk_usage_bytes()?;
        let _ = self.kura_total_disk_usage_bytes()?;
        let _ = self.refresh_disk_usage_bytes()?;
        self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_ARCHIVE_DELETION)?;
        journal.pending_archive_gc.clear();
        if let Some(checkpoint) = journal.checkpoint.as_mut() {
            checkpoint.merge_releases.clear();
            checkpoint.pending_archive_gc_root = None;
            checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
        }
        self.validate_lane_geometry_journal(journal)?;
        self.write_lane_geometry_journal(journal)?;
        let _ = self.refresh_disk_usage_bytes()?;
        self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_COMPLETION)?;
        Ok(summary)
    }

    fn validate_durable_geometry_snapshot_identity(
        &self,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
    ) -> Result<()> {
        if snapshot_height == 0
            || snapshot_block_hash.is_none()
            || snapshot_state_hash.as_ref().iter().all(|byte| *byte == 0)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry GC checkpoint has no non-genesis block/WSV identity",
            ));
        }
        let height = NonZeroUsize::new(usize::try_from(snapshot_height)?).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry GC checkpoint height is not representable",
            )
        })?;
        let expected_block_hash = snapshot_block_hash.expect("validated non-zero snapshot height");
        let durable_block_hash = self.get_durable_block_hash(height).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::NotFound,
                "lane geometry GC checkpoint is ahead of the durable canonical block log",
            )
        })?;
        if durable_block_hash != expected_block_hash {
            return Err(Error::BlockHeightConflict {
                height: snapshot_height,
                expected: durable_block_hash,
                actual: expected_block_hash,
            });
        }
        let wsv = self.wsv_checkpoint(snapshot_height)?.ok_or_else(|| {
            self.geometry_error(
                ErrorKind::NotFound,
                "lane geometry GC checkpoint has no durable WSV checkpoint",
            )
        })?;
        if wsv.state_hash() != snapshot_state_hash {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry GC state hash differs from the durable WSV checkpoint",
            ));
        }
        Ok(())
    }

    fn geometry_merge_releases_proven_by_snapshot(
        &self,
        snapshot_height: u64,
        snapshot_smart_contract_state: &BTreeMap<Name, Vec<u8>>,
    ) -> Result<Vec<LaneGeometryMergeRelease>> {
        let entries = self.merge_ledger_all_entries()?;
        let carriers = self.geometry_merge_carrier_map()?;
        let mut releases = Vec::new();
        for entry in &entries {
            let Some(batch) = entry.execution_batch.as_ref() else {
                continue;
            };
            let entry_hash = entry.canonical_hash();
            let carrier_record = carriers.get(&entry_hash).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::WouldBlock,
                    "merge execution entry has no durable canonical carrier mapping",
                )
            })?;
            let carrier =
                self.validate_geometry_merge_batch_block_binding(entry, batch, *carrier_record)?;
            if carrier.block_height > snapshot_height {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "snapshot merge release points to a carrier block beyond the snapshot",
                ));
            }
            let expected_markers =
                crate::state::State::expected_merge_execution_marker_payloads(entry, batch)
                    .map_err(|error| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!("cannot derive canonical merge execution markers: {error}"),
                        )
                    })?;
            let present = expected_markers
                .iter()
                .filter(|(key, _)| snapshot_smart_contract_state.contains_key(key))
                .count();
            if present == 0 {
                continue;
            }
            if present != expected_markers.len()
                || expected_markers
                    .iter()
                    .any(|(key, expected)| snapshot_smart_contract_state.get(key) != Some(expected))
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "snapshot has partial or conflicting merge execution marker state",
                ));
            }
            let marker_set_root = geometry_merge_marker_set_root(&expected_markers);
            for execution in &batch.lanes {
                releases.push(self.geometry_merge_release(
                    entry,
                    batch,
                    execution,
                    carrier,
                    marker_set_root,
                )?);
                if releases.len() > MAX_GEOMETRY_MERGE_RELEASES {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "snapshot proves too many merge execution archive releases",
                    ));
                }
            }
        }
        releases.sort();
        if releases.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "snapshot proves duplicate merge execution archive releases",
            ));
        }
        Ok(releases)
    }

    fn validate_geometry_merge_batch_block_binding(
        &self,
        entry: &MergeLedgerEntry,
        batch: &iroha_data_model::merge::MergeExecutionBatch,
        record: MergeLedgerCarrierRecord,
    ) -> Result<LaneGeometryMergeCarrier> {
        let entry_hash = entry.canonical_hash();
        if record.entry_hash != entry_hash || record.epoch_id != entry.epoch_id {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "merge carrier mapping does not identify its durable merge-log entry",
            ));
        }
        if batch.application_block_header.height().get() != record.block_height {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "merge execution context height differs from its canonical carrier mapping",
            ));
        }
        let height = NonZeroUsize::new(usize::try_from(record.block_height)?).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "merge execution carrier block height is zero",
            )
        })?;
        let durable_hash = self.get_durable_block_hash(height).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::NotFound,
                "merge execution carrier block is not durable",
            )
        })?;
        let carrier = self.get_block(height).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::NotFound,
                "merge execution carrier block payload is unavailable",
            )
        })?;
        if durable_hash != record.block_hash || carrier.hash() != record.block_hash {
            return Err(Error::BlockHeightConflict {
                height: record.block_height,
                expected: record.block_hash,
                actual: carrier.hash(),
            });
        }
        let reference = carrier
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref())
            .ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "merge execution carrier block has no certified merge reference",
                )
            })?;
        if !reference.matches_entry(entry)
            || reference.entry_hash != entry_hash
            || reference.execution_batch_hash != Some(batch.batch_hash)
            || reference.epoch_id != entry.epoch_id
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "merge execution carrier reference does not identify its durable merge entry",
            ));
        }
        Ok(LaneGeometryMergeCarrier {
            block_height: record.block_height,
            block_hash: record.block_hash,
            entry_hash,
        })
    }

    fn geometry_merge_carrier_map(
        &self,
    ) -> Result<BTreeMap<HashOf<MergeLedgerEntry>, MergeLedgerCarrierRecord>> {
        let records = self.merge_carrier_records()?;
        let mut carriers = BTreeMap::new();
        for record in records {
            if carriers.insert(record.entry_hash, record).is_some() {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "merge carrier mapping duplicates a committed entry hash",
                ));
            }
        }
        Ok(carriers)
    }

    fn geometry_merge_release(
        &self,
        entry: &MergeLedgerEntry,
        batch: &iroha_data_model::merge::MergeExecutionBatch,
        execution: &iroha_data_model::merge::MergeLaneExecution,
        carrier: LaneGeometryMergeCarrier,
        marker_set_root: Hash,
    ) -> Result<LaneGeometryMergeRelease> {
        let descriptor = &execution.proposal.descriptor;
        let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
            entry,
            batch,
            execution,
            Self::merge_lane_block_artifact(execution),
            carrier.block_height,
            carrier.block_hash,
        );
        let receipt_bytes = receipt.encode_framed()?;
        Ok(LaneGeometryMergeRelease {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
            application_block_height: carrier.block_height,
            application_block_hash: carrier.block_hash,
            merge_entry_hash: carrier.entry_hash,
            merge_epoch_id: entry.epoch_id,
            source_bundle_hash: execution.source_bundle_hash,
            batch_identity_hash: crate::merge::merge_execution_batch_identity_hash(batch),
            batch_hash: batch.batch_hash,
            lane_execution_hash: crate::merge::merge_lane_execution_hash(execution),
            marker_set_root,
            receipt_hash: Hash::new_from_chunks(&[
                MERGE_RELEASE_RECEIPT_DOMAIN,
                receipt_bytes.as_slice(),
            ]),
        })
    }

    fn validate_geometry_merge_releases(
        &self,
        releases: &[LaneGeometryMergeRelease],
        snapshot_height: u64,
    ) -> Result<()> {
        if releases.len() > MAX_GEOMETRY_MERGE_RELEASES
            || releases.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "geometry checkpoint merge releases are duplicated, unsorted, or oversized",
            ));
        }
        if releases.is_empty() {
            return Ok(());
        }
        let entries = self.merge_ledger_all_entries()?;
        let carriers = self.geometry_merge_carrier_map()?;
        let mut entries_by_hash = BTreeMap::new();
        for entry in &entries {
            if entries_by_hash
                .insert(entry.canonical_hash(), entry)
                .is_some()
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "committed merge log duplicates a canonical entry hash",
                ));
            }
        }
        for release in releases {
            if release.application_block_height > snapshot_height {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "geometry merge release carrier is beyond its snapshot checkpoint",
                ));
            }
            let entry = entries_by_hash
                .get(&release.merge_entry_hash)
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::NotFound,
                        "geometry checkpoint merge release has no durable block-carried entry",
                    )
                })?;
            let batch = entry.execution_batch.as_ref().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "geometry checkpoint merge release points to a non-execution entry",
                )
            })?;
            let carrier_record = carriers.get(&release.merge_entry_hash).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::WouldBlock,
                    "geometry checkpoint merge release has no durable carrier mapping",
                )
            })?;
            let carrier =
                self.validate_geometry_merge_batch_block_binding(entry, batch, *carrier_record)?;
            if carrier.block_hash != release.application_block_hash
                || carrier.block_height != release.application_block_height
                || carrier.entry_hash != release.merge_entry_hash
                || entry.epoch_id != release.merge_epoch_id
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "geometry checkpoint merge release block or epoch binding changed",
                ));
            }
            let expected_markers =
                crate::state::State::expected_merge_execution_marker_payloads(entry, batch)
                    .map_err(|error| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!("cannot rederive merge execution markers: {error}"),
                        )
                    })?;
            if geometry_merge_marker_set_root(&expected_markers) != release.marker_set_root {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "geometry checkpoint merge marker proof changed",
                ));
            }
            let execution = batch
                .lanes
                .iter()
                .find(|execution| {
                    let descriptor = &execution.proposal.descriptor;
                    descriptor.lane_id == release.lane_id
                        && descriptor.dataspace_id == release.dataspace_id
                        && descriptor.lane_incarnation == release.lane_incarnation
                        && descriptor.lane_block_height == release.lane_block_height
                        && execution.source_bundle_hash == release.source_bundle_hash
                })
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "geometry checkpoint merge release no longer matches its lane execution",
                    )
                })?;
            let expected = self.geometry_merge_release(
                entry,
                batch,
                execution,
                carrier,
                release.marker_set_root,
            )?;
            if expected != *release {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "geometry checkpoint merge release evidence changed",
                ));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn fail_next_lane_geometry_gc_at_stage_for_test(&self, stage: usize) {
        self.fail_lane_geometry_gc_stage
            .store(stage, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    fn fail_lane_geometry_gc_stage_for_test(&self, stage: usize) -> Result<()> {
        if self
            .fail_lane_geometry_gc_stage
            .compare_exchange(
                stage,
                0,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            return Err(self.geometry_error(
                ErrorKind::Interrupted,
                "lane geometry archive GC crash boundary injected for testing",
            ));
        }
        Ok(())
    }

    #[cfg(not(test))]
    fn fail_lane_geometry_gc_stage_for_test(&self, _stage: usize) -> Result<()> {
        Ok(())
    }

    fn reconcile_lane_geometry_history(
        &self,
        journal: &mut LaneGeometryJournal,
        authoritative_catalog: Hash,
        authoritative_lineage_root: Hash,
    ) -> Result<()> {
        if journal.records.is_empty() {
            if journal.checkpoint.as_ref().is_some_and(|checkpoint| {
                checkpoint.catalog != authoritative_catalog
                    || checkpoint.lineage_root != authoritative_lineage_root
            }) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "authoritative geometry does not match the compacted snapshot checkpoint",
                ));
            }
            return Ok(());
        }
        let mut candidates = Vec::new();
        if journal.records[0].previous_catalog == authoritative_catalog
            && journal.records[0].previous_lineage_root == authoritative_lineage_root
        {
            candidates.push(0);
        }
        candidates.extend(
            journal
                .records
                .iter()
                .enumerate()
                .filter_map(|(index, record)| {
                    (record.updated_catalog == authoritative_catalog
                        && record.updated_lineage_root == authoritative_lineage_root)
                        .then_some(index + 1)
                }),
        );
        if candidates.len() != 1 {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "authoritative geometry identity is absent or ambiguous without an exact transition height",
            ));
        }
        self.reconcile_lane_geometry_history_to_count(
            journal,
            authoritative_catalog,
            authoritative_lineage_root,
            candidates[0],
        )
    }

    fn reconcile_lane_geometry_history_to_count(
        &self,
        journal: &mut LaneGeometryJournal,
        authoritative_catalog: Hash,
        authoritative_lineage_root: Hash,
        desired_applied_count: usize,
    ) -> Result<()> {
        if desired_applied_count > journal.records.len() {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry recovery cursor exceeds retained transition history",
            ));
        }
        for pair in journal.records.windows(2) {
            if pair[0].updated_catalog != pair[1].previous_catalog
                || pair[0].updated_lineage_root != pair[1].previous_lineage_root
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal transition chain is not contiguous",
                ));
            }
        }
        let identity_at_cursor = if desired_applied_count == 0 {
            journal
                .records
                .first()
                .map(|record| (record.previous_catalog, record.previous_lineage_root))
                .or_else(|| {
                    journal
                        .checkpoint
                        .as_ref()
                        .map(|checkpoint| (checkpoint.catalog, checkpoint.lineage_root))
                })
        } else {
            journal
                .records
                .get(desired_applied_count - 1)
                .map(|record| (record.updated_catalog, record.updated_lineage_root))
        };
        if identity_at_cursor
            .is_some_and(|identity| identity != (authoritative_catalog, authoritative_lineage_root))
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "authoritative geometry identity does not match its exact transition cursor",
            ));
        }

        if let Some(boundary) = journal.records.iter().position(|record| {
            matches!(
                record.phase,
                LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied
            )
        }) {
            let evidence_policy = if journal.records[boundary].phase == LaneGeometryPhase::Intent {
                GeometryEvidencePolicy::AllowJournalIntentProvisioning
            } else {
                GeometryEvidencePolicy::RequireDurableEvidence
            };
            if boundary < desired_applied_count {
                self.apply_geometry_operations_forward(
                    &journal.records[boundary].operations,
                    evidence_policy,
                )?;
                journal.records[boundary].phase = LaneGeometryPhase::CatalogPublished;
            } else {
                self.apply_geometry_operations_rollback(
                    &journal.records[boundary].operations,
                    evidence_policy,
                )?;
                journal.records[boundary].phase = LaneGeometryPhase::RolledBack;
            }
            self.write_lane_geometry_journal(journal)?;
        }

        let mut current_applied_count = journal
            .records
            .iter()
            .position(|record| record.phase == LaneGeometryPhase::RolledBack)
            .unwrap_or(journal.records.len());
        for index in (desired_applied_count..current_applied_count).rev() {
            // Preserve `CatalogPublished` as durable evidence provenance until the inverse is
            // complete. The authoritative cursor makes this idempotently resumable after a crash.
            self.apply_geometry_operations_rollback(
                &journal.records[index].operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
            journal.records[index].phase = LaneGeometryPhase::RolledBack;
            self.write_lane_geometry_journal(journal)?;
        }

        current_applied_count = journal
            .records
            .iter()
            .position(|record| record.phase == LaneGeometryPhase::RolledBack)
            .unwrap_or(journal.records.len());
        for index in current_applied_count..desired_applied_count {
            // Preserve `RolledBack` until the exact retained image is live again. Only a newly
            // appended transition may carry `Intent` and authorize empty staging provisioning.
            self.apply_geometry_operations_forward(
                &journal.records[index].operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
            journal.records[index].phase = LaneGeometryPhase::CatalogPublished;
            self.write_lane_geometry_journal(journal)?;
        }

        // A terminal phase is a durable direction decision, not proof that a process completed
        // both filesystem renames before it died. Reassert the exact frontier operation on every
        // recovery. The pair movers authenticate an already-complete target and resume only the
        // block-before-merge crash frontier, so this is idempotent without reopening history.
        if let Some(record) = journal.records.get(desired_applied_count) {
            self.apply_geometry_operations_rollback(
                &record.operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
        } else if let Some(record) = desired_applied_count
            .checked_sub(1)
            .and_then(|index| journal.records.get(index))
        {
            self.apply_geometry_operations_forward(
                &record.operations,
                GeometryEvidencePolicy::RequireDurableEvidence,
            )?;
        }
        Ok(())
    }

    fn geometry_retirement_identities(
        &self,
        previous: &LaneConfig,
        operations: &[LaneGeometryOperation],
    ) -> Result<Vec<LaneRetirementIdentity>> {
        let mut identities = Vec::new();
        for operation in operations {
            if !matches!(
                operation.kind,
                LaneGeometryOperationKind::Retire | LaneGeometryOperationKind::Replace
            ) {
                continue;
            }
            let binding = operation.previous.as_ref().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement operation has no previous incarnation",
                )
            })?;
            let entry = previous.entry(operation.lane_id).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement operation is absent from the previous catalog",
                )
            })?;
            identities.push(LaneRetirementIdentity {
                lane_id: operation.lane_id,
                dataspace_id: entry.dataspace_id,
                lane_incarnation: binding.incarnation,
            });
        }
        identities.sort();
        identities.dedup();
        Ok(identities)
    }

    /// Reject scale-in while any durable autonomous payload still depends on an
    /// exact retiring coordinator or participant lane incarnation.
    ///
    /// The caller holds `sidecar_lock` from this scan until the geometry files
    /// move, preventing a producer or recovery worker from publishing new work
    /// into the retiring paths after admission.
    fn ensure_lane_retirement_admissible_locked(
        &self,
        retiring: &[LaneRetirementIdentity],
    ) -> Result<()> {
        if retiring.is_empty() {
            return Ok(());
        }
        let retiring = retiring.iter().copied().collect::<BTreeSet<_>>();
        let entries = self
            .lane_storage_entries
            .lock()
            .iter()
            .map(|(lane_id, entry)| (*lane_id, entry.clone()))
            .collect::<Vec<_>>();
        let mut autonomous = BTreeMap::new();
        let mut inputs = BTreeMap::new();
        let mut preflights = BTreeMap::new();
        let mut certified = BTreeMap::new();
        let mut receipts = BTreeMap::new();
        let mut autonomous_view_states = BTreeSet::new();
        let mut artifact_files_seen = 0_usize;
        let mut work_items_seen = 0_usize;
        let count_work_items = |current: &mut usize, additional: usize| -> Result<()> {
            *current = current.checked_add(additional).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement work-item count overflows",
                )
            })?;
            if *current > MAX_LANE_RETIREMENT_WORK_ITEMS {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan exceeds the maximum durable work-item count",
                ));
            }
            Ok(())
        };

        for (storage_lane_id, entry) in entries {
            let lane_artifacts = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
            if !self.validate_path_kind(&lane_artifacts, true)? {
                continue;
            }
            for directory_entry in fs::read_dir(&lane_artifacts)
                .map_err(|error| Error::IO(error, lane_artifacts.clone()))?
            {
                let directory_entry =
                    directory_entry.map_err(|error| Error::IO(error, lane_artifacts.clone()))?;
                artifact_files_seen = artifact_files_seen.checked_add(1).ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement artifact-file count overflows",
                    )
                })?;
                if artifact_files_seen > MAX_LANE_RETIREMENT_ARTIFACT_FILES {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan exceeds the maximum artifact-file count",
                    ));
                }
                let path = directory_entry.path();
                let file_type = directory_entry
                    .file_type()
                    .map_err(|error| Error::IO(error, path.clone()))?;
                if file_type.is_symlink() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a symlink artifact",
                        ),
                        path,
                    ));
                }
                let name = directory_entry.file_name().into_string().map_err(|_| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a non-UTF-8 artifact",
                        ),
                        path.clone(),
                    )
                })?;
                if name.ends_with(".tmp") {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::WouldBlock,
                            "lane retirement scan found an in-flight autonomous sidecar",
                        ),
                        path,
                    ));
                }
                if !file_type.is_file() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a non-regular artifact",
                        ),
                        path,
                    ));
                }
                if matches!(
                    name.as_str(),
                    LANE_ARTIFACTS_DATA_FILE
                        | LANE_ARTIFACTS_INDEX_FILE
                        | CERTIFIED_LANE_BLOCKS_DATA_FILE
                        | CERTIFIED_LANE_BLOCKS_INDEX_FILE
                        | AUTONOMOUS_LANE_BLOCKS_DATA_FILE
                        | AUTONOMOUS_LANE_BLOCKS_INDEX_FILE
                        | LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE
                        | LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE
                        | LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE
                        | LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE
                        | LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE
                        | LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE
                ) {
                    continue;
                }
                let Some(raw_height) = name
                    .strip_prefix("autonomous_view_")
                    .and_then(|suffix| suffix.strip_suffix(".norito"))
                else {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered an unknown artifact filename",
                        ),
                        path,
                    ));
                };
                let lane_block_height = raw_height.parse::<u64>().map_err(|_| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a malformed view-state filename",
                        ),
                        path.clone(),
                    )
                })?;
                if lane_block_height == 0
                    || name != format!("autonomous_view_{lane_block_height:020}.norito")
                    || !autonomous_view_states.insert((storage_lane_id, lane_block_height))
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a non-canonical view-state filename",
                        ),
                        path,
                    ));
                }
            }

            let (autonomous_data, autonomous_index) =
                Self::autonomous_lane_block_paths_for_entry(&entry, &self.store_root);
            let autonomous_heights = self
                .geometry_retirement_sidecar_payload_heights(&autonomous_data, &autonomous_index)?;
            count_work_items(&mut work_items_seen, autonomous_heights.len())?;
            for lane_block_height in autonomous_heights {
                let raw = Self::read_indexed_sidecar_from_paths_with_recovery(
                    lane_block_height,
                    &autonomous_data,
                    &autonomous_index,
                    norito::decode_from_bytes::<AutonomousLaneBlockArtifact>,
                    "autonomous lane block",
                    false,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan cannot decode an autonomous payload",
                    )
                })?;
                let expected_chain_id_hash = raw.executable_payload.chain_id_hash;
                let expected_epoch = raw.executable_payload.epoch;
                let artifact = self
                    .read_autonomous_lane_block_artifact_from_paths_locked(
                        storage_lane_id,
                        lane_block_height,
                        &autonomous_data,
                        &autonomous_index,
                        expected_chain_id_hash,
                        expected_epoch,
                        false,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement scan found malformed producer-authenticated work",
                        )
                    })?;
                let current = Self::validate_autonomous_lane_block_artifact(
                    &artifact,
                    expected_chain_id_hash,
                    expected_epoch,
                )
                .map_err(|message| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!("lane retirement autonomous payload is invalid: {message}"),
                    )
                })?;
                if autonomous
                    .insert((storage_lane_id, lane_block_height), (artifact, current))
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate autonomous payload identity",
                    ));
                }
            }

            let (input_data, input_index) =
                Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
            let input_heights =
                self.geometry_retirement_sidecar_payload_heights(&input_data, &input_index)?;
            count_work_items(&mut work_items_seen, input_heights.len())?;
            for lane_block_height in input_heights {
                let input = self
                    .read_lane_block_execution_input_from_paths_locked(
                        storage_lane_id,
                        lane_block_height,
                        &input_data,
                        &input_index,
                        false,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement scan found a malformed execution input",
                        )
                    })?;
                if inputs
                    .insert((storage_lane_id, lane_block_height), input)
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate execution input identity",
                    ));
                }
            }

            let (preflight_data, preflight_index) =
                Self::lane_block_execution_preflight_paths_for_entry(&entry, &self.store_root);
            let preflight_heights = self
                .geometry_retirement_sidecar_payload_heights(&preflight_data, &preflight_index)?;
            count_work_items(&mut work_items_seen, preflight_heights.len())?;
            for lane_block_height in preflight_heights {
                let preflight = self
                    .read_lane_block_execution_preflight_from_paths_locked(
                        storage_lane_id,
                        lane_block_height,
                        &preflight_data,
                        &preflight_index,
                        false,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement scan found a malformed execution preflight",
                        )
                    })?;
                if preflights
                    .insert((storage_lane_id, lane_block_height), preflight)
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate execution preflight identity",
                    ));
                }
            }

            let (certified_data, certified_index) =
                Self::certified_lane_block_paths_for_entry(&entry, &self.store_root);
            let certified_heights = self
                .geometry_retirement_sidecar_payload_heights(&certified_data, &certified_index)?;
            count_work_items(&mut work_items_seen, certified_heights.len())?;
            for lane_block_height in certified_heights {
                let artifact = self
                    .read_certified_lane_block_artifact_from_paths_locked(
                        storage_lane_id,
                        lane_block_height,
                        &certified_data,
                        &certified_index,
                        false,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement scan found a malformed certified lane block",
                        )
                    })?;
                if certified
                    .insert((storage_lane_id, lane_block_height), artifact)
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate certified work identity",
                    ));
                }
            }

            let (receipt_data, receipt_index) =
                Self::lane_block_application_receipt_paths_for_entry(&entry, &self.store_root);
            let receipt_heights =
                self.geometry_retirement_sidecar_payload_heights(&receipt_data, &receipt_index)?;
            count_work_items(&mut work_items_seen, receipt_heights.len())?;
            for lane_block_height in receipt_heights {
                let receipt = self
                    .read_lane_block_application_receipt_from_paths_locked(
                        storage_lane_id,
                        lane_block_height,
                        &receipt_data,
                        &receipt_index,
                        false,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement scan found a malformed application receipt",
                        )
                    })?;
                if receipts
                    .insert((storage_lane_id, lane_block_height), receipt)
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate application receipt identity",
                    ));
                }
            }
        }

        if autonomous_view_states
            .iter()
            .any(|identity| !autonomous.contains_key(identity))
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement scan found an orphan autonomous view state",
            ));
        }

        for (identity, input) in &inputs {
            let autonomous_binding = (
                input.autonomous_chain_id_hash,
                input.autonomous_epoch,
                input.autonomous_payload_hash,
            );
            let (Some(chain_id_hash), Some(epoch), Some(payload_hash)) = autonomous_binding else {
                if autonomous_binding != (None, None, None) {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement execution input has a partial autonomous binding",
                    ));
                }
                continue;
            };
            let (artifact, current) = autonomous.get(identity).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement execution input has no producer-authenticated payload",
                )
            })?;
            let payload = &artifact.executable_payload;
            if input.proposal != *current
                || payload.chain_id_hash != chain_id_hash
                || payload.epoch != epoch
                || payload.payload_hash != payload_hash
                || input.entrypoint_hashes != payload.entrypoint_hashes
                || input.entrypoints != payload.entrypoints
                || input.reservation_keys != payload.reservation_keys
                || input.routing_plans != payload.routing_plans
                || input.native_amx_receipts != payload.native_amx_receipts
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement execution input differs from its authenticated payload",
                ));
            }
        }

        for (identity, receipt) in &receipts {
            let valid = match receipt.format {
                LaneBlockApplicationReceiptArtifactFormat::Current => {
                    self.lane_retirement_current_receipt_matches_canonical_block(receipt)
                }
                LaneBlockApplicationReceiptArtifactFormat::DirectExecution => {
                    inputs
                        .get(identity)
                        .zip(preflights.get(identity))
                        .and_then(|(input, preflight)| {
                            LaneBlockApplicationReceiptArtifact::new_direct_execution(
                                input, preflight,
                            )
                        })
                        .as_ref()
                        == Some(receipt)
                }
                LaneBlockApplicationReceiptArtifactFormat::MergeExecution => {
                    self.lane_block_application_receipt_matches_merge_log(receipt)
                }
            };
            if !valid {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan found an application receipt without canonical evidence",
                ));
            }
        }

        let receipt_applies = |identity, proposal: &LaneBlockProposalV1| {
            receipts
                .get(identity)
                .is_some_and(|receipt| &receipt.proposal == proposal)
        };
        let receipt_applies_autonomous =
            |identity, artifact: &AutonomousLaneBlockArtifact, proposal: &LaneBlockProposalV1| {
                receipts.get(identity).is_some_and(|receipt| {
                    if &receipt.proposal != proposal {
                        return false;
                    }
                    match receipt.format {
                        LaneBlockApplicationReceiptArtifactFormat::Current => false,
                        LaneBlockApplicationReceiptArtifactFormat::DirectExecution => {
                            inputs.get(identity).is_some_and(|input| {
                                input.proposal == *proposal
                                    && input.autonomous_payload_hash
                                        == Some(artifact.executable_payload.payload_hash)
                            })
                        }
                        LaneBlockApplicationReceiptArtifactFormat::MergeExecution => self
                            .lane_retirement_merge_receipt_applies_autonomous_payload(
                                receipt,
                                &artifact.executable_payload,
                            ),
                    }
                })
            };
        let mut hinted_target_cache = BTreeMap::new();
        let mut hinted_payload_targets = |proposal: &LaneBlockProposalV1| -> Result<bool> {
            let hint = proposal.payload_block_hint.ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement pending global payload has no canonical block hint",
                )
            })?;
            let key = LanePayloadHintIdentity {
                proposal_hash: proposal.proposal_hash,
                proposal_height: hint.proposal_height,
                proposal_view: hint.proposal_view,
                proposal_block_hash: hint.proposal_block_hash,
            };
            if let Some(targets) = hinted_target_cache.get(&key) {
                return Ok(*targets);
            }
            let targets = self.hinted_lane_payload_targets_retirement(proposal, &retiring)?;
            hinted_target_cache.insert(key, targets);
            Ok(targets)
        };

        for (identity, certified) in &certified {
            if receipt_applies(identity, &certified.proposal) {
                continue;
            }
            if lane_proposal_coordinator_targets_retirement(&certified.proposal, &retiring) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending certified work belongs to a retiring lane incarnation",
                ));
            }
            let mut authenticated = autonomous
                .get(identity)
                .is_some_and(|(_, current)| current == &certified.proposal);
            authenticated |= inputs.get(identity).is_some_and(|input| {
                input.autonomous_payload_hash.is_some() && input.proposal == certified.proposal
            });
            if certified.proposal.payload_block_hint.is_some() {
                if hinted_payload_targets(&certified.proposal)? {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "pending certified global payload targets a retiring lane incarnation",
                    ));
                }
                authenticated = true;
            }
            if !authenticated {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "certified autonomous work has no authenticated executable payload",
                ));
            }
        }

        for (identity, (artifact, current)) in &autonomous {
            if receipt_applies_autonomous(identity, artifact, current) {
                continue;
            }
            if lane_payload_targets_retirement(&artifact.executable_payload, &retiring) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending autonomous payload targets a retiring lane incarnation",
                ));
            }
            if artifact
                .executable_payload
                .origin_proposal
                .payload_block_hint
                .is_some()
                && hinted_payload_targets(&artifact.executable_payload.origin_proposal)?
            {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending hinted autonomous payload targets a retiring lane incarnation",
                ));
            }
        }

        for (identity, input) in &inputs {
            if input.autonomous_payload_hash.is_none() && receipt_applies(identity, &input.proposal)
            {
                continue;
            }
            if lane_proposal_coordinator_targets_retirement(&input.proposal, &retiring) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending execution input belongs to a retiring lane incarnation",
                ));
            }
            if input.autonomous_payload_hash.is_some() {
                continue;
            }
            if hinted_payload_targets(&input.proposal)? {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending global execution input targets a retiring lane incarnation",
                ));
            }
        }
        Ok(())
    }

    fn lane_retirement_current_receipt_matches_canonical_block(
        &self,
        receipt: &LaneBlockApplicationReceiptArtifact,
    ) -> bool {
        let Ok(height) = usize::try_from(receipt.application_block_height) else {
            return false;
        };
        let Some(height) = NonZeroUsize::new(height) else {
            return false;
        };
        if self.get_durable_block_hash(height) != Some(receipt.application_block_hash) {
            return false;
        }
        let Some(block) = self.get_block(height) else {
            return false;
        };
        if block.hash() != receipt.application_block_hash
            || block.header().height().get() != receipt.application_block_height
            || block.header().view_change_index() != receipt.artifact.ownership.proposal_view
        {
            return false;
        }
        let Some(bundle) = block.execution_context() else {
            return false;
        };
        if block.header().execution_context_hash() != Some(HashOf::new(bundle))
            || !bundle
                .lane_payload_ownerships
                .iter()
                .any(|ownership| ownership == &receipt.artifact.ownership)
        {
            return false;
        }
        let descriptor = &receipt.proposal.descriptor;
        let mut entrypoints = Vec::with_capacity(descriptor.accepted_candidate_indices.len());
        let mut results = Vec::with_capacity(descriptor.accepted_candidate_indices.len());
        for (raw_index, expected_hash) in descriptor
            .accepted_candidate_indices
            .iter()
            .copied()
            .zip(descriptor.accepted_transaction_hashes.iter().copied())
        {
            let Ok(index) = usize::try_from(raw_index) else {
                return false;
            };
            let Some(entrypoint) = Self::block_entrypoint_at(&block, index) else {
                return false;
            };
            if Hash::from(entrypoint.hash()) != expected_hash {
                return false;
            }
            let Some(result) = Self::block_transaction_result_at(&block, index) else {
                return false;
            };
            entrypoints.push(entrypoint);
            results.push(result);
        }
        let expected = LaneBlockApplicationReceiptArtifact::new(
            RecoveredLaneBlockPayload {
                proposal: receipt.proposal.clone(),
                artifact: receipt.artifact.clone(),
                autonomous_chain_id_hash: None,
                autonomous_epoch: None,
                autonomous_payload_hash: None,
                entrypoints,
                reservation_keys: Vec::new(),
                routing_plans: Vec::new(),
                native_amx_receipts: Vec::new(),
            },
            receipt.application_block_height,
            receipt.application_block_hash,
            results,
        );
        expected == *receipt
    }

    fn lane_retirement_merge_receipt_applies_autonomous_payload(
        &self,
        receipt: &LaneBlockApplicationReceiptArtifact,
        payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    ) -> bool {
        let Some(entry_hash) = receipt.merge_entry_hash else {
            return false;
        };
        let Ok(entries) = self.merge_ledger_all_entries() else {
            return false;
        };
        let Some(execution) = entries
            .iter()
            .find(|entry| entry.canonical_hash() == entry_hash)
            .and_then(|entry| entry.execution_batch.as_ref())
            .and_then(|batch| {
                batch
                    .lanes
                    .iter()
                    .find(|execution| execution.proposal == receipt.proposal)
            })
        else {
            return false;
        };
        execution.origin_proposal == payload.origin_proposal
            && execution.autonomous_chain_id_hash == payload.chain_id_hash
            && execution.autonomous_epoch == payload.epoch
            && execution.autonomous_payload_hash == payload.payload_hash
            && execution.entrypoint_hashes == payload.entrypoint_hashes
            && execution.entrypoints == payload.entrypoints
            && execution.reservation_keys
                == payload
                    .reservation_keys
                    .iter()
                    .map(Encode::encode)
                    .collect::<Vec<_>>()
            && execution.routing_plans
                == payload
                    .routing_plans
                    .iter()
                    .map(Encode::encode)
                    .collect::<Vec<_>>()
            && execution.native_amx_receipts == payload.native_amx_receipts
    }

    fn hinted_lane_payload_targets_retirement(
        &self,
        proposal: &LaneBlockProposalV1,
        retiring: &BTreeSet<LaneRetirementIdentity>,
    ) -> Result<bool> {
        if lane_proposal_coordinator_targets_retirement(proposal, retiring) {
            return Ok(true);
        }
        let block = self.canonical_hinted_lane_payload_block(proposal)?;
        let bundle = block.execution_context().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement hinted block has no execution context",
            )
        })?;
        let descriptor = &proposal.descriptor;
        for (raw_index, expected_hash) in descriptor
            .accepted_candidate_indices
            .iter()
            .copied()
            .zip(descriptor.accepted_transaction_hashes.iter().copied())
        {
            let index = usize::try_from(raw_index).map_err(|_| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement hinted payload index does not fit memory",
                )
            })?;
            let entrypoint = Self::block_entrypoint_at(&block, index).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement hinted block is missing an accepted entrypoint",
                )
            })?;
            if Hash::from(entrypoint.hash()) != expected_hash {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement hinted block entrypoint hash does not match the proposal",
                ));
            }
            let context = bundle.external.get(index).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement hinted block is missing accepted routing context",
                )
            })?;
            if Hash::from(context.entrypoint_hash) != expected_hash
                || context.lane_id != descriptor.lane_id
                || context.dataspace_id != descriptor.dataspace_id
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement hinted routing context differs from lane ownership",
                ));
            }
            let plan = routing_plan_from_execution_context(context).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement hinted routing plan is malformed",
                )
            })?;
            let signed_transaction = match &entrypoint {
                TransactionEntrypoint::External(transaction) => Some(transaction),
                TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => None,
            };
            if matches!(&plan, crate::queue::RoutingPlan::NativeAmx(_)) {
                let signed_transaction = signed_transaction.ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement native AMX context has no signed source transaction",
                    )
                })?;
                let source_id = signed_transaction.hash();
                let chain_id_hash =
                    Hash::new(signed_transaction.chain().clone().into_inner().as_bytes());
                if !crate::native_amx::receipt_shape_matches_coordinator_payload(
                    context.native_amx_receipt.as_ref(),
                    &plan,
                    source_id.as_ref(),
                    expected_hash,
                    chain_id_hash,
                    proposal,
                ) {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement hinted native AMX receipt is malformed",
                    ));
                }
            } else if context.native_amx_receipt.is_some() {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement single-route context carries a native AMX receipt",
                ));
            }
            if context.native_amx_receipt.as_ref().is_some_and(|receipt| {
                receipt.legs.iter().any(|leg| {
                    retiring.iter().any(|identity| {
                        identity.lane_id == leg.lane_id && identity.dataspace_id == leg.dataspace_id
                    })
                })
            }) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn canonical_hinted_lane_payload_block(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Result<Arc<SignedBlock>> {
        crate::lane_consensus::validate_lane_block_proposal(proposal).map_err(|_| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement hinted proposal is malformed",
            )
        })?;
        let hint = proposal.payload_block_hint.ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement pending global payload has no block hint",
            )
        })?;
        if hint.proposal_height == 0 || hint.proposal_height != proposal.descriptor.proposal_height
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement payload hint height differs from the certified descriptor",
            ));
        }
        let height = usize::try_from(hint.proposal_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement payload hint height does not fit memory",
                )
            })?;
        if self.get_durable_block_hash(height) != Some(hint.proposal_block_hash) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement payload hint does not identify the canonical durable block",
            ));
        }
        let block = self.get_block(height).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement canonical hinted block body is unavailable",
            )
        })?;
        if block.hash() != hint.proposal_block_hash
            || block.header().height().get() != hint.proposal_height
            || block.header().view_change_index() != hint.proposal_view
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement payload hint differs from its canonical block header",
            ));
        }
        let bundle = block.execution_context().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement canonical hinted block has no execution context",
            )
        })?;
        if block.header().execution_context_hash() != Some(HashOf::new(bundle)) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement hinted block execution context is not header-authenticated",
            ));
        }
        let mut matching = bundle.lane_payload_ownerships.iter().filter(|ownership| {
            Self::lane_block_artifact_matches_descriptor(ownership, &proposal.descriptor)
                && ownership.proposal_view == hint.proposal_view
        });
        let ownership = matching.next().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement hinted block has no exact lane payload ownership",
            )
        })?;
        if matching.next().is_some() || ownership.validate_replay_material().is_err() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane retirement hinted block has ambiguous or malformed lane ownership",
            ));
        }
        Ok(block)
    }

    fn build_geometry_operations(
        &self,
        transition_id: Hash,
        previous: &[LaneGeometryBinding],
        updated: &[LaneGeometryBinding],
        replaced_lane_ids: &BTreeSet<LaneId>,
    ) -> Result<Vec<LaneGeometryOperation>> {
        let previous_by_lane = previous
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        let updated_by_lane = updated
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        let lane_ids = previous_by_lane
            .keys()
            .chain(updated_by_lane.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let transition_hex = hex::encode(transition_id.as_ref());
        let mut operations = Vec::new();
        for lane_id in &lane_ids {
            let before = previous_by_lane
                .get(lane_id)
                .map(|binding| (*binding).clone());
            let after = updated_by_lane
                .get(lane_id)
                .map(|binding| (*binding).clone());
            if replaced_lane_ids.contains(lane_id)
                && before
                    .as_ref()
                    .zip(after.as_ref())
                    .is_some_and(|(previous, updated)| {
                        previous.incarnation == updated.incarnation
                            && previous.activation_height == updated.activation_height
                    })
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidInput,
                    "lane replacement must use a fresh incarnation and activation",
                ));
            }
            let kind = match (&before, &after) {
                (None, Some(_)) => LaneGeometryOperationKind::Create,
                (Some(_), None) => LaneGeometryOperationKind::Retire,
                (Some(before), Some(after))
                    if replaced_lane_ids.contains(lane_id)
                        || before.incarnation != after.incarnation
                        || before.activation_height != after.activation_height =>
                {
                    LaneGeometryOperationKind::Replace
                }
                (Some(before), Some(after))
                    if before.blocks_path != after.blocks_path
                        || before.merge_path != after.merge_path =>
                {
                    LaneGeometryOperationKind::Relabel
                }
                _ => continue,
            };
            let archive_root = format!(
                "retired/lane_geometry/{transition_hex}/lane_{:010}",
                lane_id.as_u32()
            );
            let operation = LaneGeometryOperation {
                kind,
                lane_id: *lane_id,
                previous: before,
                updated: after,
                archived_blocks_path: format!("{archive_root}/previous_blocks"),
                archived_merge_path: format!("{archive_root}/previous_merge.log"),
                unpublished_blocks_path: format!("{archive_root}/unpublished_blocks"),
                unpublished_merge_path: format!("{archive_root}/unpublished_merge.log"),
            };
            self.preflight_geometry_operation(&operation)?;
            operations.push(operation);
        }
        Ok(operations)
    }

    fn preflight_geometry_operation(&self, operation: &LaneGeometryOperation) -> Result<()> {
        for (relative, directory) in [
            (&operation.archived_blocks_path, true),
            (&operation.archived_merge_path, false),
            (&operation.unpublished_blocks_path, true),
            (&operation.unpublished_merge_path, false),
        ] {
            let retained = self.resolve_relative_path(relative)?;
            if self.validate_path_kind(&retained, directory)? {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "lane geometry archive target already exists",
                    ),
                    retained,
                ));
            }
        }
        if let Some(previous) = operation.previous.as_ref() {
            let previous_blocks = self.binding_blocks_path(previous);
            let previous_merge = self.binding_merge_path(previous);
            let previous_blocks_exists = self.validate_path_kind(&previous_blocks, true)?;
            let previous_merge_exists = self.validate_path_kind(&previous_merge, false)?;
            if !previous_blocks_exists || !previous_merge_exists {
                return Err(self.geometry_error(
                    ErrorKind::NotFound,
                    "active previous lane geometry is incomplete or missing",
                ));
            }
            self.require_lane_marker(previous)?;
        }
        if let Some(updated) = operation.updated.as_ref() {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    let blocks = self.binding_blocks_path(updated);
                    let merge = self.binding_merge_path(updated);
                    if self.validate_path_kind(&blocks, true)?
                        || self.validate_path_kind(&merge, false)?
                    {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane storage already exists at a create target",
                        ));
                    }
                }
                LaneGeometryOperationKind::Replace | LaneGeometryOperationKind::Relabel => {
                    let previous = operation
                        .previous
                        .as_ref()
                        .expect("replace and relabel have previous bindings");
                    if previous.blocks_path != updated.blocks_path
                        && self.validate_path_kind(&self.binding_blocks_path(updated), true)?
                    {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane geometry target block path already exists",
                        ));
                    }
                    if previous.merge_path != updated.merge_path
                        && self.validate_path_kind(&self.binding_merge_path(updated), false)?
                    {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane geometry target merge path already exists",
                        ));
                    }
                }
                LaneGeometryOperationKind::Retire => {}
            }
        }
        Ok(())
    }

    fn apply_geometry_operations_forward(
        &self,
        operations: &[LaneGeometryOperation],
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        for operation in operations {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    self.restore_unpublished_or_provision(operation, evidence_policy)?;
                }
                LaneGeometryOperationKind::Retire => {
                    let previous = operation
                        .previous
                        .as_ref()
                        .expect("retire has previous binding");
                    self.archive_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                }
                LaneGeometryOperationKind::Replace => {
                    self.apply_replaced_geometry_binding_forward(operation, evidence_policy)?;
                }
                LaneGeometryOperationKind::Relabel => {
                    self.move_geometry_binding(
                        operation.previous.as_ref().expect("relabel previous"),
                        operation.updated.as_ref().expect("relabel updated"),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn apply_geometry_operations_rollback(
        &self,
        operations: &[LaneGeometryOperation],
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        for operation in operations.iter().rev() {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    self.rollback_created_geometry_binding(operation, evidence_policy)?;
                }
                LaneGeometryOperationKind::Retire => {
                    let previous = operation.previous.as_ref().expect("retire previous");
                    self.restore_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                }
                LaneGeometryOperationKind::Replace => {
                    self.rollback_replaced_geometry_binding(operation, evidence_policy)?;
                }
                LaneGeometryOperationKind::Relabel => {
                    self.move_geometry_binding(
                        operation.updated.as_ref().expect("relabel updated"),
                        operation.previous.as_ref().expect("relabel previous"),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn apply_replaced_geometry_binding_forward(
        &self,
        operation: &LaneGeometryOperation,
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        let previous = operation.previous.as_ref().expect("replace previous");
        let updated = operation.updated.as_ref().expect("replace updated");
        let updated_blocks = self.binding_blocks_path(updated);
        let updated_merge = self.binding_merge_path(updated);
        let archived_blocks = self.resolve_relative_path(&operation.archived_blocks_path)?;
        let archived_merge = self.resolve_relative_path(&operation.archived_merge_path)?;

        let updated_blocks_exist = self.validate_path_kind(&updated_blocks, true)?;
        let updated_merge_exists = self.validate_path_kind(&updated_merge, false)?;
        if updated_blocks_exist {
            let marker_matches_updated = self
                .lane_marker_matches_at_if_present(&updated_blocks, updated)?
                .expect("validated replacement block path exists");
            if marker_matches_updated {
                if !self.require_absent_or_sealed_geometry_binding_at(
                    previous,
                    &archived_blocks,
                    &archived_merge,
                )? {
                    return Err(self.geometry_error(
                        ErrorKind::NotFound,
                        "live replacement has no complete authenticated previous archive",
                    ));
                }
                self.restore_unpublished_or_provision(operation, evidence_policy)?;
                return self.require_complete_geometry_binding_at(
                    updated,
                    &updated_blocks,
                    &updated_merge,
                );
            }
            let marker_matches_previous = self
                .lane_marker_matches_at_if_present(&updated_blocks, previous)?
                .expect("validated replacement block path exists");
            if !marker_matches_previous {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "replacement live block path has a foreign incarnation marker",
                ));
            }
        } else if updated_merge_exists && self.binding_merge_path(previous) != updated_merge {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "replacement live merge path exists without its authenticated block directory",
            ));
        }

        self.archive_geometry_binding(
            previous,
            &operation.archived_blocks_path,
            &operation.archived_merge_path,
        )?;
        self.restore_unpublished_or_provision(operation, evidence_policy)?;
        self.require_complete_geometry_binding_at(updated, &updated_blocks, &updated_merge)
    }

    fn provision_empty_retained_updated_geometry_binding(
        &self,
        operation: &LaneGeometryOperation,
    ) -> Result<()> {
        let updated = operation
            .updated
            .as_ref()
            .expect("create and replace operations have an updated binding");
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;
        let unpublished_merge = self.resolve_relative_path(&operation.unpublished_merge_path)?;
        let blocks_exist = self.validate_path_kind(&unpublished_blocks, true)?;
        let merge_exists = self.validate_path_kind(&unpublished_merge, false)?;
        if merge_exists {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "journal-owned empty staging unexpectedly has a merge-log path",
            ));
        }
        if blocks_exist {
            let marker_exists =
                self.validate_path_kind(&unpublished_blocks.join(MARKER_FILE_NAME), false)?;
            preflight_empty_block_store_without_marker(
                &unpublished_blocks,
                Some(updated),
                marker_exists,
            )?;
            if marker_exists && !self.lane_marker_is_unsealed_at(&unpublished_blocks, updated)? {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "journal-owned incomplete staging already carries a move seal",
                ));
            }
        }
        let staged = LaneGeometryBinding {
            blocks_path: operation.unpublished_blocks_path.clone(),
            merge_path: operation.unpublished_merge_path.clone(),
            ..updated.clone()
        };
        self.provision_geometry_binding(&staged)?;
        self.require_exact_empty_journal_owned_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
        )?;
        self.seal_geometry_pair_move(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )?;
        self.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
    }

    fn normalize_complete_retained_updated_geometry_binding(
        &self,
        operation: &LaneGeometryOperation,
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<bool> {
        let updated = operation
            .updated
            .as_ref()
            .expect("create and replace operations have an updated binding");
        let live_blocks = self.binding_blocks_path(updated);
        let live_merge = self.binding_merge_path(updated);
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;
        let unpublished_merge = self.resolve_relative_path(&operation.unpublished_merge_path)?;
        match (
            self.validate_path_kind(&unpublished_blocks, true)?,
            self.validate_path_kind(&unpublished_merge, false)?,
        ) {
            (false, false) => return Ok(false),
            (true, true) => {}
            _ => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retained updated geometry pair is only partially present",
                ));
            }
        }
        if self.lane_marker_is_unsealed_at(&unpublished_blocks, updated)? {
            if !evidence_policy.allows_journal_intent_provisioning() {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "terminal rollback has an unsealed retained updated geometry pair",
                ));
            }
            self.require_exact_empty_journal_owned_pair_at(
                updated,
                &unpublished_blocks,
                &unpublished_merge,
            )?;
            self.seal_geometry_pair_move(
                updated,
                &unpublished_blocks,
                &unpublished_merge,
                &unpublished_blocks,
                &unpublished_merge,
            )?;
        } else {
            self.normalize_completed_geometry_pair(
                updated,
                &unpublished_blocks,
                &unpublished_merge,
                &live_blocks,
                &live_merge,
                &unpublished_blocks,
                &unpublished_merge,
                GeometryPairTargetKind::ImmutableRetained,
            )?;
        }
        self.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )?;
        Ok(true)
    }

    fn retain_updated_geometry_binding_for_rollback(
        &self,
        operation: &LaneGeometryOperation,
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        let updated = operation
            .updated
            .as_ref()
            .expect("create and replace operations have an updated binding");
        let live_blocks = self.binding_blocks_path(updated);
        let live_merge = self.binding_merge_path(updated);
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;
        let unpublished_merge = self.resolve_relative_path(&operation.unpublished_merge_path)?;
        let live_blocks_exist = self.validate_path_kind(&live_blocks, true)?;
        let live_merge_exists = self.validate_path_kind(&live_merge, false)?;
        let unpublished_blocks_exist = self.validate_path_kind(&unpublished_blocks, true)?;
        let unpublished_merge_exists = self.validate_path_kind(&unpublished_merge, false)?;

        if unpublished_blocks_exist
            && unpublished_merge_exists
            && !live_blocks_exist
            && !live_merge_exists
            && self
                .normalize_complete_retained_updated_geometry_binding(operation, evidence_policy)?
        {
            return Ok(());
        }
        if !live_blocks_exist
            && !live_merge_exists
            && !unpublished_blocks_exist
            && !unpublished_merge_exists
        {
            if !evidence_policy.allows_journal_intent_provisioning() {
                return Err(self.geometry_error(
                    ErrorKind::NotFound,
                    "durable lane geometry evidence is missing; refusing to provision an empty replacement",
                ));
            }
            return self.provision_empty_retained_updated_geometry_binding(operation);
        }
        if unpublished_blocks_exist
            && !unpublished_merge_exists
            && !live_blocks_exist
            && !live_merge_exists
            && evidence_policy.allows_journal_intent_provisioning()
        {
            return self.provision_empty_retained_updated_geometry_binding(operation);
        }
        if evidence_policy.allows_journal_intent_provisioning()
            && live_blocks_exist
            && live_merge_exists
            && !unpublished_blocks_exist
            && !unpublished_merge_exists
            && self.lane_marker_is_unsealed_at(&live_blocks, updated)?
        {
            self.require_exact_empty_journal_owned_pair_at(updated, &live_blocks, &live_merge)?;
        }
        self.move_geometry_binding_pair(
            updated,
            &live_blocks,
            &live_merge,
            &unpublished_blocks,
            &unpublished_merge,
            GeometryPairTargetKind::ImmutableRetained,
        )?;
        self.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
    }

    fn rollback_replaced_geometry_binding(
        &self,
        operation: &LaneGeometryOperation,
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        let previous = operation.previous.as_ref().expect("replace previous");
        let updated = operation.updated.as_ref().expect("replace updated");
        let previous_blocks = self.binding_blocks_path(previous);
        let updated_blocks = self.binding_blocks_path(updated);
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;

        // A replacement Intent can crash while archiving the previous incarnation, before any
        // updated block directory exists. Restore that exact previous pair first so shared-path
        // replacements are not mistaken for an updated inverse half.
        if !self.validate_path_kind(&updated_blocks, true)?
            && !self.validate_path_kind(&unpublished_blocks, true)?
        {
            self.restore_geometry_binding(
                previous,
                &operation.archived_blocks_path,
                &operation.archived_merge_path,
            )?;
        }

        if self.validate_path_kind(&previous_blocks, true)?
            && self.lane_marker_matches_at_if_present(&previous_blocks, previous)? == Some(true)
        {
            self.restore_geometry_binding(
                previous,
                &operation.archived_blocks_path,
                &operation.archived_merge_path,
            )?;
            if self
                .normalize_complete_retained_updated_geometry_binding(operation, evidence_policy)?
            {
                return self.require_rolled_back_replacement_postconditions(previous, updated);
            }
            if !evidence_policy.allows_journal_intent_provisioning() {
                return Err(self.geometry_error(
                    ErrorKind::NotFound,
                    "rolled-back replacement has no authenticated updated-incarnation image",
                ));
            }
            let updated_merge = self.binding_merge_path(updated);
            if (updated_blocks != previous_blocks
                && self.validate_path_kind(&updated_blocks, true)?)
                || (updated_merge != self.binding_merge_path(previous)
                    && self.validate_path_kind(&updated_merge, false)?)
            {
                return Err(self.geometry_error(
                    ErrorKind::AlreadyExists,
                    "rolled-back replacement has duplicate updated live storage",
                ));
            }
            self.provision_empty_retained_updated_geometry_binding(operation)?;
            return self.require_rolled_back_replacement_postconditions(previous, updated);
        }

        // Normalize every authenticated full/half move directly toward the retained updated
        // image. An Intent may create only an exact empty image; terminal phases never provision.
        self.retain_updated_geometry_binding_for_rollback(operation, evidence_policy)?;
        self.restore_geometry_binding(
            previous,
            &operation.archived_blocks_path,
            &operation.archived_merge_path,
        )?;
        self.require_rolled_back_replacement_postconditions(previous, updated)
    }

    fn require_rolled_back_replacement_postconditions(
        &self,
        previous: &LaneGeometryBinding,
        updated: &LaneGeometryBinding,
    ) -> Result<()> {
        let previous_blocks = self.binding_blocks_path(previous);
        let previous_merge = self.binding_merge_path(previous);
        let updated_blocks = self.binding_blocks_path(updated);
        let updated_merge = self.binding_merge_path(updated);
        self.require_complete_geometry_binding_at(previous, &previous_blocks, &previous_merge)?;
        if updated_blocks != previous_blocks && self.validate_path_kind(&updated_blocks, true)? {
            return Err(self.geometry_error(
                ErrorKind::AlreadyExists,
                "replacement rollback left the updated incarnation live",
            ));
        }
        if updated_merge != previous_merge && self.validate_path_kind(&updated_merge, false)? {
            return Err(self.geometry_error(
                ErrorKind::AlreadyExists,
                "replacement rollback left the updated merge log live",
            ));
        }
        Ok(())
    }

    fn rollback_created_geometry_binding(
        &self,
        operation: &LaneGeometryOperation,
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        self.retain_updated_geometry_binding_for_rollback(operation, evidence_policy)
    }

    fn archive_geometry_binding(
        &self,
        binding: &LaneGeometryBinding,
        archived_blocks: &str,
        archived_merge: &str,
    ) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        if blocks == *self.active_blocks_dir.lock() {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "refusing to archive the active primary block store",
            ));
        }
        let merge = self.binding_merge_path(binding);
        if merge == *self.active_merge_path.lock() {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "refusing to archive the active primary merge log",
            ));
        }
        self.move_geometry_binding_pair(
            binding,
            &blocks,
            &merge,
            &self.resolve_relative_path(archived_blocks)?,
            &self.resolve_relative_path(archived_merge)?,
            GeometryPairTargetKind::ImmutableRetained,
        )
    }

    fn restore_geometry_binding(
        &self,
        binding: &LaneGeometryBinding,
        archived_blocks: &str,
        archived_merge: &str,
    ) -> Result<()> {
        self.move_geometry_binding_pair(
            binding,
            &self.resolve_relative_path(archived_blocks)?,
            &self.resolve_relative_path(archived_merge)?,
            &self.binding_blocks_path(binding),
            &self.binding_merge_path(binding),
            GeometryPairTargetKind::MutableLive,
        )
    }

    fn restore_unpublished_or_provision(
        &self,
        operation: &LaneGeometryOperation,
        evidence_policy: GeometryEvidencePolicy,
    ) -> Result<()> {
        let updated = operation
            .updated
            .as_ref()
            .expect("create and replace operations have an updated binding");
        let live_blocks = self.binding_blocks_path(updated);
        let live_merge = self.binding_merge_path(updated);
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;
        let unpublished_merge = self.resolve_relative_path(&operation.unpublished_merge_path)?;
        let live_blocks_exist = self.validate_path_kind(&live_blocks, true)?;
        let live_merge_exists = self.validate_path_kind(&live_merge, false)?;
        let unpublished_blocks_exist = self.validate_path_kind(&unpublished_blocks, true)?;
        let unpublished_merge_exists = self.validate_path_kind(&unpublished_merge, false)?;

        if live_blocks_exist
            && self.lane_marker_matches_at_if_present(&live_blocks, updated)? != Some(true)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "replacement live block path has a foreign incarnation marker",
            ));
        }
        if unpublished_blocks_exist {
            let marker_path = unpublished_blocks.join(MARKER_FILE_NAME);
            if self.validate_path_kind(&marker_path, false)? {
                if self.lane_marker_matches_at_if_present(&unpublished_blocks, updated)?
                    != Some(true)
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "replacement staging block path has a foreign incarnation marker",
                    ));
                }
                if !unpublished_merge_exists && !live_merge_exists {
                    if !evidence_policy.allows_journal_intent_provisioning()
                        || live_blocks_exist
                        || live_merge_exists
                    {
                        return Err(self.geometry_error(
                            ErrorKind::InvalidData,
                            "durable replacement staging is missing its merge-log evidence",
                        ));
                    }
                    preflight_empty_block_store_without_marker(
                        &unpublished_blocks,
                        Some(updated),
                        true,
                    )?;
                    let staged = LaneGeometryBinding {
                        blocks_path: operation.unpublished_blocks_path.clone(),
                        merge_path: operation.unpublished_merge_path.clone(),
                        ..updated.clone()
                    };
                    self.provision_geometry_binding(&staged)?;
                }
            } else if evidence_policy.allows_journal_intent_provisioning()
                && !live_blocks_exist
                && !live_merge_exists
                && !unpublished_merge_exists
            {
                let staged = LaneGeometryBinding {
                    blocks_path: operation.unpublished_blocks_path.clone(),
                    merge_path: operation.unpublished_merge_path.clone(),
                    ..updated.clone()
                };
                self.provision_geometry_binding(&staged)?;
            } else {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "unmarked replacement staging is not a repairable journal Intent frontier",
                ));
            }
        }
        // The journal-owned repair paths above may have created a marker or merge log. Re-read
        // every location before deciding whether the pair is orphaned or durably complete.
        let live_blocks_exist = self.validate_path_kind(&live_blocks, true)?;
        let live_merge_exists = self.validate_path_kind(&live_merge, false)?;
        let unpublished_blocks_exist = self.validate_path_kind(&unpublished_blocks, true)?;
        let unpublished_merge_exists = self.validate_path_kind(&unpublished_merge, false)?;
        if evidence_policy.allows_journal_intent_provisioning() {
            if live_blocks_exist
                && live_merge_exists
                && self.lane_marker_is_unsealed_at(&live_blocks, updated)?
            {
                self.require_exact_empty_journal_owned_pair_at(updated, &live_blocks, &live_merge)?;
            }
            if unpublished_blocks_exist
                && unpublished_merge_exists
                && self.lane_marker_is_unsealed_at(&unpublished_blocks, updated)?
            {
                self.require_exact_empty_journal_owned_pair_at(
                    updated,
                    &unpublished_blocks,
                    &unpublished_merge,
                )?;
            }
        }
        let any_blocks_exist = live_blocks_exist || unpublished_blocks_exist;
        let any_merge_exists = live_merge_exists || unpublished_merge_exists;
        if any_blocks_exist != any_merge_exists {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "replacement provisioning has an orphan block or merge-log path",
            ));
        }
        if !any_blocks_exist {
            if !evidence_policy.allows_journal_intent_provisioning() {
                return Err(self.geometry_error(
                    ErrorKind::NotFound,
                    "durable lane geometry evidence is missing; refusing to provision an empty replacement",
                ));
            }
            let staged = LaneGeometryBinding {
                blocks_path: operation.unpublished_blocks_path.clone(),
                merge_path: operation.unpublished_merge_path.clone(),
                ..updated.clone()
            };
            self.provision_geometry_binding(&staged)?;
        }
        if evidence_policy == GeometryEvidencePolicy::RequireDurableEvidence
            && !live_blocks_exist
            && !live_merge_exists
            && unpublished_blocks_exist
            && unpublished_merge_exists
        {
            // A terminal replay can have crashed after retargeting the retained pair's seal to
            // the live paths but before performing either rename. Normalize that exact frontier
            // back to an immutable retained image first. This also rejects an unsealed or
            // foreign pair, so terminal phases never gain authority to adopt mutable staging.
            self.move_geometry_binding_pair(
                updated,
                &live_blocks,
                &live_merge,
                &unpublished_blocks,
                &unpublished_merge,
                GeometryPairTargetKind::ImmutableRetained,
            )?;
        }
        self.move_geometry_binding_pair(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &live_blocks,
            &live_merge,
            GeometryPairTargetKind::MutableLive,
        )
    }

    fn move_geometry_binding(
        &self,
        previous: &LaneGeometryBinding,
        updated: &LaneGeometryBinding,
    ) -> Result<()> {
        let old_blocks = self.binding_blocks_path(previous);
        let new_blocks = self.binding_blocks_path(updated);
        let old_merge = self.binding_merge_path(previous);
        let new_merge = self.binding_merge_path(updated);
        self.move_geometry_binding_pair(
            updated,
            &old_blocks,
            &old_merge,
            &new_blocks,
            &new_merge,
            GeometryPairTargetKind::MutableLive,
        )?;
        self.retarget_active_geometry_paths(&old_blocks, &new_blocks, &old_merge, &new_merge)?;
        self.require_lane_marker(updated)
    }

    fn require_absent_or_sealed_geometry_binding_at(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
    ) -> Result<bool> {
        let blocks_exist = self.validate_path_kind(blocks, true)?;
        let merge_exists = self.validate_path_kind(merge, false)?;
        match (blocks_exist, merge_exists) {
            (false, false) => Ok(false),
            (true, true) => {
                self.require_sealed_geometry_pair_at(binding, blocks, merge, blocks, merge)?;
                Ok(true)
            }
            _ => Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry block and merge paths are only partially present",
            )),
        }
    }

    fn require_complete_geometry_binding_at(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
    ) -> Result<()> {
        match (
            self.validate_path_kind(blocks, true)?,
            self.validate_path_kind(merge, false)?,
        ) {
            (true, true) => self.require_lane_marker_at(blocks, binding),
            (false, false) => Err(self.geometry_error(
                ErrorKind::NotFound,
                "complete authenticated lane geometry pair is missing",
            )),
            _ => Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry block and merge paths are only partially present",
            )),
        }
    }

    fn lane_marker_is_unsealed_at(
        &self,
        blocks: &Path,
        binding: &LaneGeometryBinding,
    ) -> Result<bool> {
        let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
        self.require_lane_marker_value(&marker, blocks, binding)?;
        match (
            marker.move_target_blocks.as_deref(),
            marker.move_target_merge.as_deref(),
        ) {
            (None, None) => Ok(true),
            (Some(_), Some(_)) => Ok(false),
            _ => Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry marker has incomplete pair-move evidence",
                ),
                blocks.join(MARKER_FILE_NAME),
            )),
        }
    }

    fn require_exact_empty_journal_owned_pair_at(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
    ) -> Result<()> {
        self.require_complete_geometry_binding_at(binding, blocks, merge)?;
        preflight_empty_block_store_without_marker(blocks, Some(binding), true)?;
        if self.geometry_merge_log_digest(merge)? != empty_geometry_merge_digest() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "journal-owned unsealed geometry pair is not exactly empty",
                ),
                merge.to_path_buf(),
            ));
        }
        if !self.lane_marker_is_unsealed_at(blocks, binding)? {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "journal-owned empty geometry pair unexpectedly carries a move seal",
                ),
                blocks.join(MARKER_FILE_NAME),
            ));
        }
        Ok(())
    }

    fn geometry_block_store_digest(&self, blocks: &Path) -> Result<Hash> {
        fn hash_path(hasher: &mut blake3::Hasher, tag: u8, relative: &str) {
            hasher.update(&[tag]);
            hasher.update(
                &u64::try_from(relative.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            hasher.update(relative.as_bytes());
        }

        fn hash_directory(
            kura: &Kura,
            root: &Path,
            directory: &Path,
            depth: usize,
            entries_seen: &mut usize,
            hasher: &mut blake3::Hasher,
        ) -> Result<()> {
            if depth > MAX_GEOMETRY_ARCHIVE_DEPTH {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry block-store digest exceeds the maximum directory depth",
                    ),
                    directory.to_path_buf(),
                ));
            }
            let directory_identity = kura.geometry_path_identity(directory, true)?;
            let mut entries = fs::read_dir(directory)
                .map_err(|error| Error::IO(error, directory.to_path_buf()))?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| Error::IO(error, directory.to_path_buf()))?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let name = entry.file_name();
                if directory == root
                    && matches!(
                        name.to_str(),
                        Some(MARKER_FILE_NAME) | Some(MARKER_TEMP_FILE_NAME)
                    )
                {
                    let file_type = entry
                        .file_type()
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    if file_type.is_symlink() || !file_type.is_file() {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane geometry marker path has an unsafe file type",
                            ),
                            path,
                        ));
                    }
                    continue;
                }
                *entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                    kura.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry block-store digest entry count overflows",
                    )
                })?;
                if *entries_seen > MAX_GEOMETRY_ARCHIVE_ENTRIES {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry block-store digest exceeds the maximum entry count",
                        ),
                        path,
                    ));
                }
                let relative = path.strip_prefix(root).map_err(|_| {
                    kura.geometry_error(
                        ErrorKind::InvalidInput,
                        "lane geometry block-store digest path escapes its root",
                    )
                })?;
                let relative = relative.to_str().ok_or_else(|| {
                    kura.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry block-store digest path is not valid UTF-8",
                    )
                })?;
                let file_type = entry
                    .file_type()
                    .map_err(|error| Error::IO(error, path.clone()))?;
                if file_type.is_symlink() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry block store contains a symbolic link",
                        ),
                        path,
                    ));
                }
                if file_type.is_dir() {
                    hash_path(hasher, b'd', relative);
                    hash_directory(
                        kura,
                        root,
                        &path,
                        depth.saturating_add(1),
                        entries_seen,
                        hasher,
                    )?;
                } else if file_type.is_file() {
                    hash_path(hasher, b'f', relative);
                    let identity = kura.geometry_path_identity(&path, false)?;
                    let mut file =
                        File::open(&path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
                    kura.verify_open_geometry_file(&path, &file)?;
                    let initial_len = file
                        .metadata()
                        .map_err(|error| Error::IO(error, path.to_path_buf()))?
                        .len();
                    hasher.update(&initial_len.to_le_bytes());
                    let mut bytes_read = 0_u64;
                    let mut buffer = [0_u8; 64 * 1024];
                    loop {
                        let read = file
                            .read(&mut buffer)
                            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                        if read == 0 {
                            break;
                        }
                        bytes_read =
                            bytes_read
                                .checked_add(u64::try_from(read)?)
                                .ok_or_else(|| {
                                    kura.geometry_error(
                                        ErrorKind::InvalidData,
                                        "lane geometry block-store digest byte count overflows",
                                    )
                                })?;
                        hasher.update(&buffer[..read]);
                    }
                    let final_len = file
                        .metadata()
                        .map_err(|error| Error::IO(error, path.to_path_buf()))?
                        .len();
                    kura.verify_open_geometry_file(&path, &file)?;
                    kura.require_geometry_path_identity(&path, false, identity)?;
                    if bytes_read != initial_len || final_len != initial_len {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane geometry block store changed while its move evidence was hashed",
                            ),
                            path,
                        ));
                    }
                } else {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry block store contains a non-regular entry",
                        ),
                        path,
                    ));
                }
            }
            kura.require_geometry_path_identity(directory, true, directory_identity)
        }

        let root_identity = self.geometry_path_identity(blocks, true)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(GEOMETRY_BLOCK_STORE_DIGEST_DOMAIN);
        let mut entries_seen = 0_usize;
        hash_directory(self, blocks, blocks, 0, &mut entries_seen, &mut hasher)?;
        self.require_geometry_path_identity(blocks, true, root_identity)?;
        Ok(Hash::prehashed(*hasher.finalize().as_bytes()))
    }

    fn geometry_merge_log_digest(&self, merge: &Path) -> Result<Hash> {
        let identity = self.geometry_path_identity(merge, false)?;
        let mut file = File::open(merge).map_err(|error| Error::IO(error, merge.to_path_buf()))?;
        self.verify_open_geometry_file(merge, &file)?;
        let initial_len = file
            .metadata()
            .map_err(|error| Error::IO(error, merge.to_path_buf()))?
            .len();
        let mut hasher = blake3::Hasher::new();
        hasher.update(GEOMETRY_MERGE_DIGEST_DOMAIN);
        hasher.update(&initial_len.to_le_bytes());
        let mut buffer = [0_u8; 64 * 1024];
        let mut bytes_read = 0_u64;
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| Error::IO(error, merge.to_path_buf()))?;
            if read == 0 {
                break;
            }
            bytes_read = bytes_read.saturating_add(u64::try_from(read)?);
            hasher.update(&buffer[..read]);
        }
        let final_len = file
            .metadata()
            .map_err(|error| Error::IO(error, merge.to_path_buf()))?
            .len();
        self.verify_open_geometry_file(merge, &file)?;
        self.require_geometry_path_identity(merge, false, identity)?;
        if bytes_read != initial_len || final_len != initial_len {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry merge log changed while its move evidence was hashed",
                ),
                merge.to_path_buf(),
            ));
        }
        Ok(Hash::prehashed(*hasher.finalize().as_bytes()))
    }

    fn require_sealed_geometry_pair_at(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
        target_blocks: &Path,
        target_merge: &Path,
    ) -> Result<()> {
        let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
        self.require_lane_marker_value(&marker, blocks, binding)?;
        let expected_blocks = self.relative_geometry_path(target_blocks)?;
        let expected_merge = self.relative_geometry_path(target_merge)?;
        if marker.move_target_blocks.as_deref() != Some(expected_blocks.as_str())
            || marker.move_target_merge.as_deref() != Some(expected_merge.as_str())
            || marker.block_store_digest != self.geometry_block_store_digest(blocks)?
            || marker.merge_log_digest != self.geometry_merge_log_digest(merge)?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry pair does not match its durable block/merge evidence",
                ),
                blocks.to_path_buf(),
            ));
        }
        Ok(())
    }

    fn seal_geometry_pair_move(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
        target_blocks: &Path,
        target_merge: &Path,
    ) -> Result<()> {
        let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
        self.require_lane_marker_value(&marker, blocks, binding)?;
        let target_blocks = self.relative_geometry_path(target_blocks)?;
        let target_merge = self.relative_geometry_path(target_merge)?;
        match (
            marker.move_target_blocks.as_deref(),
            marker.move_target_merge.as_deref(),
        ) {
            (None, None) => {}
            (Some(sealed_blocks), Some(sealed_merge))
                if sealed_blocks == target_blocks && sealed_merge == target_merge =>
            {
                return self.require_sealed_geometry_pair_at(
                    binding,
                    blocks,
                    merge,
                    &self.resolve_relative_path(&target_blocks)?,
                    &self.resolve_relative_path(&target_merge)?,
                );
            }
            (Some(sealed_blocks), Some(sealed_merge)) => {
                let source_blocks = self.relative_geometry_path(blocks)?;
                let source_merge = self.relative_geometry_path(merge)?;
                if sealed_blocks != source_blocks || sealed_merge != source_merge {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry pair carries stale move-target evidence",
                        ),
                        blocks.join(MARKER_FILE_NAME),
                    ));
                }
                self.require_sealed_geometry_pair_at(binding, blocks, merge, blocks, merge)?;
            }
            _ => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry marker has incomplete pair-move evidence",
                    ),
                    blocks.join(MARKER_FILE_NAME),
                ));
            }
        }
        let merge_log_digest = self.geometry_merge_log_digest(merge)?;
        self.write_lane_marker_at(
            blocks,
            binding,
            Some(target_blocks),
            Some(target_merge),
            merge_log_digest,
        )
    }

    fn retarget_inverse_geometry_pair_move_seal(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
        prior_target_blocks: &Path,
        prior_target_merge: &Path,
        target_blocks: &Path,
        target_merge: &Path,
    ) -> Result<()> {
        let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
        self.require_lane_marker_value(&marker, blocks, binding)?;
        let prior_blocks = self.relative_geometry_path(prior_target_blocks)?;
        let prior_merge = self.relative_geometry_path(prior_target_merge)?;
        let target_blocks_relative = self.relative_geometry_path(target_blocks)?;
        let target_merge_relative = self.relative_geometry_path(target_merge)?;
        let sealed_target = (
            marker.move_target_blocks.as_deref(),
            marker.move_target_merge.as_deref(),
        );
        if sealed_target
            == (
                Some(target_blocks_relative.as_str()),
                Some(target_merge_relative.as_str()),
            )
        {
            return self.require_sealed_geometry_pair_at(
                binding,
                blocks,
                merge,
                target_blocks,
                target_merge,
            );
        }
        if sealed_target != (Some(prior_blocks.as_str()), Some(prior_merge.as_str())) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "inverse lane geometry move has no matching prior move seal",
                ),
                blocks.join(MARKER_FILE_NAME),
            ));
        }
        self.require_sealed_geometry_pair_at(
            binding,
            blocks,
            merge,
            prior_target_blocks,
            prior_target_merge,
        )?;
        let merge_log_digest = self.geometry_merge_log_digest(merge)?;
        self.write_lane_marker_at(
            blocks,
            binding,
            Some(target_blocks_relative),
            Some(target_merge_relative),
            merge_log_digest,
        )
    }

    fn clear_geometry_pair_move_seal(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
    ) -> Result<()> {
        let merge_log_digest = self.geometry_merge_log_digest(merge)?;
        self.write_lane_marker_at(blocks, binding, None, None, merge_log_digest)
    }

    fn normalize_completed_geometry_pair(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
        merge: &Path,
        source_blocks: &Path,
        source_merge: &Path,
        target_blocks: &Path,
        target_merge: &Path,
        target_kind: GeometryPairTargetKind,
    ) -> Result<bool> {
        let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
        self.require_lane_marker_value(&marker, blocks, binding)?;
        let source_blocks_relative = self.relative_geometry_path(source_blocks)?;
        let source_merge_relative = self.relative_geometry_path(source_merge)?;
        let target_blocks_relative = self.relative_geometry_path(target_blocks)?;
        let target_merge_relative = self.relative_geometry_path(target_merge)?;
        match (
            marker.move_target_blocks.as_deref(),
            marker.move_target_merge.as_deref(),
        ) {
            (None, None) => {
                if target_kind == GeometryPairTargetKind::ImmutableRetained {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "immutable retained lane geometry pair has no durable move seal",
                        ),
                        blocks.join(MARKER_FILE_NAME),
                    ));
                }
                Ok(false)
            }
            (Some(sealed_blocks), Some(sealed_merge))
                if sealed_blocks == target_blocks_relative
                    && sealed_merge == target_merge_relative =>
            {
                self.require_sealed_geometry_pair_at(
                    binding,
                    blocks,
                    merge,
                    target_blocks,
                    target_merge,
                )?;
                Ok(true)
            }
            (Some(sealed_blocks), Some(sealed_merge))
                if sealed_blocks == source_blocks_relative
                    && sealed_merge == source_merge_relative =>
            {
                // The prior direction durably sealed but crashed before its first rename. The
                // inverse therefore finds a physically complete target carrying an exact seal to
                // the opposite pair. Authenticate those bytes before retargeting the seal.
                self.require_sealed_geometry_pair_at(
                    binding,
                    blocks,
                    merge,
                    source_blocks,
                    source_merge,
                )?;
                let merge_log_digest = self.geometry_merge_log_digest(merge)?;
                self.write_lane_marker_at(
                    blocks,
                    binding,
                    Some(target_blocks_relative),
                    Some(target_merge_relative),
                    merge_log_digest,
                )?;
                Ok(true)
            }
            (Some(_), Some(_)) => Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "completed lane geometry pair carries stale move-target evidence",
                ),
                blocks.join(MARKER_FILE_NAME),
            )),
            _ => Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry marker has incomplete pair-move evidence",
                ),
                blocks.join(MARKER_FILE_NAME),
            )),
        }
    }

    fn geometry_move_location(
        &self,
        source: &Path,
        target: &Path,
        directory: bool,
    ) -> Result<GeometryMoveLocation> {
        if source == target {
            return self
                .validate_path_kind(source, directory)?
                .then_some(GeometryMoveLocation::Target)
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::NotFound,
                        "shared lane geometry move path is missing",
                    )
                });
        }
        match (
            self.validate_path_kind(source, directory)?,
            self.validate_path_kind(target, directory)?,
        ) {
            (true, false) => Ok(GeometryMoveLocation::Source),
            (false, true) => Ok(GeometryMoveLocation::Target),
            (false, false) => Err(self.geometry_error(
                ErrorKind::NotFound,
                "lane geometry move has neither its source nor target",
            )),
            (true, true) => Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "lane geometry move has both its source and target",
                ),
                target.to_path_buf(),
            )),
        }
    }

    fn move_geometry_binding_pair(
        &self,
        binding: &LaneGeometryBinding,
        source_blocks: &Path,
        source_merge: &Path,
        target_blocks: &Path,
        target_merge: &Path,
        target_kind: GeometryPairTargetKind,
    ) -> Result<()> {
        let blocks_location = self.geometry_move_location(source_blocks, target_blocks, true)?;
        let merge_location = self.geometry_move_location(source_merge, target_merge, false)?;

        let marker_root = match blocks_location {
            GeometryMoveLocation::Source => source_blocks,
            GeometryMoveLocation::Target => target_blocks,
        };
        let merge_root = match merge_location {
            GeometryMoveLocation::Source => source_merge,
            GeometryMoveLocation::Target => target_merge,
        };
        let pair_is_complete = blocks_location == GeometryMoveLocation::Target
            && merge_location == GeometryMoveLocation::Target;
        if pair_is_complete {
            if self.normalize_completed_geometry_pair(
                binding,
                marker_root,
                merge_root,
                source_blocks,
                source_merge,
                target_blocks,
                target_merge,
                target_kind,
            )? && target_kind == GeometryPairTargetKind::MutableLive
            {
                self.clear_geometry_pair_move_seal(binding, marker_root, merge_root)?;
            }
        } else {
            match (blocks_location, merge_location) {
                (GeometryMoveLocation::Source, GeometryMoveLocation::Source) => {
                    self.seal_geometry_pair_move(
                        binding,
                        marker_root,
                        merge_root,
                        target_blocks,
                        target_merge,
                    )?;
                }
                (GeometryMoveLocation::Target, GeometryMoveLocation::Source)
                    if source_blocks == target_blocks =>
                {
                    self.seal_geometry_pair_move(
                        binding,
                        marker_root,
                        merge_root,
                        target_blocks,
                        target_merge,
                    )?;
                }
                (GeometryMoveLocation::Target, GeometryMoveLocation::Source) => {
                    self.require_sealed_geometry_pair_at(
                        binding,
                        marker_root,
                        merge_root,
                        target_blocks,
                        target_merge,
                    )?;
                }
                (GeometryMoveLocation::Source, GeometryMoveLocation::Target)
                    if source_merge == target_merge =>
                {
                    self.seal_geometry_pair_move(
                        binding,
                        marker_root,
                        merge_root,
                        target_blocks,
                        target_merge,
                    )?;
                }
                (GeometryMoveLocation::Source, GeometryMoveLocation::Target) => {
                    self.retarget_inverse_geometry_pair_move_seal(
                        binding,
                        marker_root,
                        merge_root,
                        source_blocks,
                        source_merge,
                        target_blocks,
                        target_merge,
                    )?;
                }
                (GeometryMoveLocation::Target, GeometryMoveLocation::Target) => {
                    unreachable!("complete geometry pairs are handled above")
                }
            }
        }
        let blocks_identity = self.geometry_path_identity(marker_root, true)?;
        let merge_identity = self.geometry_path_identity(merge_root, false)?;

        if blocks_location == GeometryMoveLocation::Source {
            self.move_geometry_path(source_blocks, target_blocks, true)?;
        }
        if merge_location == GeometryMoveLocation::Source {
            self.move_geometry_path(source_merge, target_merge, false)?;
        }

        self.require_geometry_path_identity(target_blocks, true, blocks_identity)?;
        self.require_geometry_path_identity(target_merge, false, merge_identity)?;
        if !pair_is_complete {
            self.require_sealed_geometry_pair_at(
                binding,
                target_blocks,
                target_merge,
                target_blocks,
                target_merge,
            )?;
        }
        self.sync_geometry_path_contents(target_blocks, true)?;
        self.sync_geometry_path_contents(target_merge, false)?;
        if source_blocks != target_blocks && self.validate_path_kind(source_blocks, true)? {
            return Err(self.geometry_error(
                ErrorKind::AlreadyExists,
                "lane geometry block source remained after its authenticated move",
            ));
        }
        if source_merge != target_merge && self.validate_path_kind(source_merge, false)? {
            return Err(self.geometry_error(
                ErrorKind::AlreadyExists,
                "lane geometry merge source remained after its authenticated move",
            ));
        }
        if !pair_is_complete && target_kind == GeometryPairTargetKind::MutableLive {
            self.clear_geometry_pair_move_seal(binding, target_blocks, target_merge)?;
        }
        Ok(())
    }

    fn remove_authenticated_geometry_archive(
        &self,
        pending: &LaneGeometryPendingArchiveGc,
        merge_releases: &[LaneGeometryMergeRelease],
    ) -> Result<(u64, bool)> {
        let transition_hex = hex::encode(pending.intent.transition_id.as_ref());
        let archive_parent = self.resolve_relative_path("retired/lane_geometry")?;
        let root = archive_parent.join(&transition_hex);
        let quarantine = archive_parent.join(format!("{GC_QUARANTINE_PREFIX}{transition_hex}"));
        let root_exists = self.validate_path_kind(&root, true)?;
        let quarantine_exists = self.validate_path_kind(&quarantine, true)?;
        if root_exists && quarantine_exists {
            return Err(self.geometry_error(
                ErrorKind::AlreadyExists,
                "lane geometry archive and its GC quarantine both exist",
            ));
        }
        if !root_exists && !quarantine_exists {
            return Ok((0, false));
        }

        let deletion_root = if root_exists {
            let (_, identity) =
                self.authenticate_geometry_archive(&root, pending, merge_releases)?;
            self.require_geometry_path_identity(&root, true, identity)?;
            self.inject_geometry_move_target_collision_for_test(&quarantine, true)?;
            rename_geometry_path_noreplace(&root, &quarantine)
                .map_err(|error| Error::IO(error, root.clone()))?;
            self.sync_geometry_parent(root.parent())?;
            self.require_geometry_path_identity(&quarantine, true, identity)?;
            self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_ARCHIVE_QUARANTINE)?;
            quarantine
        } else {
            quarantine
        };

        // Revalidate after quarantine promotion. The root identity check prevents a path swap
        // between authentication and rename from turning this into an arbitrary tree deletion.
        let (bytes, identity) =
            self.authenticate_geometry_archive(&deletion_root, pending, merge_releases)?;
        self.require_geometry_path_identity(&deletion_root, true, identity)?;
        fs::remove_dir_all(&deletion_root)
            .map_err(|error| Error::IO(error, deletion_root.clone()))?;
        self.sync_geometry_parent(deletion_root.parent())?;
        Ok((bytes, true))
    }

    fn authenticate_geometry_archive(
        &self,
        root: &Path,
        pending: &LaneGeometryPendingArchiveGc,
        merge_releases: &[LaneGeometryMergeRelease],
    ) -> Result<(u64, GeometryFileIdentity)> {
        let identity = self.geometry_path_identity(root, true)?;
        let expected_lane_dirs = pending
            .intent
            .operations
            .iter()
            .map(|operation| {
                (
                    format!("lane_{:010}", operation.lane_id.as_u32()),
                    operation,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut bytes = 0_u64;
        let entries = fs::read_dir(root).map_err(|error| Error::IO(error, root.to_path_buf()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, root.to_path_buf()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive contains a non-UTF-8 entry",
                    ),
                    path.clone(),
                )
            })?;
            let operation = expected_lane_dirs.get(&name).ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive contains an unauthenticated lane directory",
                    ),
                    path.clone(),
                )
            })?;
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if file_type.is_symlink() || !file_type.is_dir() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive lane entry is not a regular directory",
                    ),
                    path,
                ));
            }
            bytes = bytes.saturating_add(self.authenticated_geometry_lane_archive_bytes(
                &path,
                operation,
                merge_releases,
            )?);
        }
        self.require_geometry_path_identity(root, true, identity)?;
        Ok((bytes, identity))
    }

    fn authenticated_geometry_lane_archive_bytes(
        &self,
        lane_root: &Path,
        operation: &LaneGeometryOperation,
        merge_releases: &[LaneGeometryMergeRelease],
    ) -> Result<u64> {
        self.validate_path_kind(lane_root, true)?;
        let mut bytes = 0_u64;
        let entries =
            fs::read_dir(lane_root).map_err(|error| Error::IO(error, lane_root.to_path_buf()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, lane_root.to_path_buf()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry lane archive contains a non-UTF-8 entry",
                    ),
                    path.clone(),
                )
            })?;
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            match name.as_str() {
                "previous_blocks" if file_type.is_dir() && !file_type.is_symlink() => {
                    let binding = operation.previous.as_ref().ok_or_else(|| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "previous block archive has no authenticated previous binding",
                            ),
                            path.clone(),
                        )
                    })?;
                    self.require_lane_marker_at(&path, binding)?;
                    self.ensure_archived_lane_work_released(&path, binding, merge_releases)?;
                    bytes = bytes.saturating_add(Self::regular_geometry_archive_tree_bytes(&path)?);
                }
                "unpublished_blocks" if file_type.is_dir() && !file_type.is_symlink() => {
                    let binding = operation.updated.as_ref().ok_or_else(|| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "unpublished block archive has no authenticated updated binding",
                            ),
                            path.clone(),
                        )
                    })?;
                    self.require_lane_marker_at(&path, binding)?;
                    self.ensure_archived_lane_work_released(&path, binding, merge_releases)?;
                    bytes = bytes.saturating_add(Self::regular_geometry_archive_tree_bytes(&path)?);
                }
                "previous_merge.log"
                    if operation.previous.is_some()
                        && file_type.is_file()
                        && !file_type.is_symlink() =>
                {
                    bytes = bytes.saturating_add(
                        entry
                            .metadata()
                            .map_err(|error| Error::IO(error, path.clone()))?
                            .len(),
                    );
                }
                "unpublished_merge.log"
                    if operation.updated.is_some()
                        && file_type.is_file()
                        && !file_type.is_symlink() =>
                {
                    bytes = bytes.saturating_add(
                        entry
                            .metadata()
                            .map_err(|error| Error::IO(error, path.clone()))?
                            .len(),
                    );
                }
                _ => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry lane archive contains an unauthenticated or unsafe entry",
                        ),
                        path,
                    ));
                }
            }
        }
        for (binding, blocks_name, merge_name, target_blocks, target_merge) in [
            (
                operation.previous.as_ref(),
                "previous_blocks",
                "previous_merge.log",
                operation.archived_blocks_path.as_str(),
                operation.archived_merge_path.as_str(),
            ),
            (
                operation.updated.as_ref(),
                "unpublished_blocks",
                "unpublished_merge.log",
                operation.unpublished_blocks_path.as_str(),
                operation.unpublished_merge_path.as_str(),
            ),
        ] {
            let blocks = lane_root.join(blocks_name);
            let merge = lane_root.join(merge_name);
            let blocks_exist = self.validate_path_kind(&blocks, true)?;
            let merge_exists = self.validate_path_kind(&merge, false)?;
            if blocks_exist != merge_exists {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry archive contains only one half of a block/merge pair",
                ));
            }
            if blocks_exist {
                let binding = binding.ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry archive pair has no authenticated catalog binding",
                    )
                })?;
                self.require_sealed_geometry_pair_at(
                    binding,
                    &blocks,
                    &merge,
                    &self.resolve_relative_path(target_blocks)?,
                    &self.resolve_relative_path(target_merge)?,
                )?;
            }
        }
        Ok(bytes)
    }

    fn ensure_archived_lane_work_released(
        &self,
        blocks_path: &Path,
        binding: &LaneGeometryBinding,
        merge_releases: &[LaneGeometryMergeRelease],
    ) -> Result<()> {
        let lane_artifacts = blocks_path.join(LANE_ARTIFACTS_DIR_NAME);
        if !self.validate_path_kind(&lane_artifacts, true)? {
            return Ok(());
        }
        let paths =
            |data: &str, index: &str| (lane_artifacts.join(data), lane_artifacts.join(index));
        let (certified_data, certified_index) = paths(
            CERTIFIED_LANE_BLOCKS_DATA_FILE,
            CERTIFIED_LANE_BLOCKS_INDEX_FILE,
        );
        let (autonomous_data, autonomous_index) = paths(
            AUTONOMOUS_LANE_BLOCKS_DATA_FILE,
            AUTONOMOUS_LANE_BLOCKS_INDEX_FILE,
        );
        let (input_data, input_index) = paths(
            LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
            LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE,
        );
        let (preflight_data, preflight_index) = paths(
            LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
            LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE,
        );
        let (receipt_data, receipt_index) = paths(
            LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
            LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE,
        );

        let certified_heights =
            self.geometry_indexed_sidecar_payload_heights(&certified_data, &certified_index)?;
        let mut work_heights = certified_heights.clone();
        for (data, index) in [
            (&autonomous_data, &autonomous_index),
            (&input_data, &input_index),
            (&preflight_data, &preflight_index),
        ] {
            work_heights.extend(self.geometry_indexed_sidecar_payload_heights(data, index)?);
        }
        for entry in fs::read_dir(&lane_artifacts)
            .map_err(|error| Error::IO(error, lane_artifacts.clone()))?
        {
            let entry = entry.map_err(|error| Error::IO(error, lane_artifacts.clone()))?;
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane artifact archive contains a non-UTF-8 filename",
                    ),
                    entry.path(),
                )
            })?;
            if name.ends_with(".tmp")
                && (name.starts_with("certified_blocks")
                    || name.starts_with("autonomous_blocks")
                    || name.starts_with("execution_inputs")
                    || name.starts_with("execution_preflights")
                    || name.starts_with("application_receipts")
                    || name.starts_with("autonomous_view_"))
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane has an ambiguous autonomous-work temp sidecar",
                ));
            }
            if let Some(raw_height) = name
                .strip_prefix("autonomous_view_")
                .and_then(|suffix| suffix.strip_suffix(".norito"))
            {
                let height = raw_height.parse::<u64>().map_err(|_| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired lane has a malformed autonomous view-state filename",
                    )
                })?;
                if height == 0 {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired lane has a zero-height autonomous view state",
                    ));
                }
                work_heights.insert(height);
            } else if name.starts_with("autonomous_view_") {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane has an unrecognized autonomous view-state sidecar",
                ));
            }
        }

        if work_heights.is_empty() {
            return Ok(());
        }
        let _sidecar_guard = self.sidecar_lock.lock();
        for lane_block_height in work_heights {
            if !certified_heights.contains(&lane_block_height) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "retired lane has autonomous work without a certified merge artifact",
                ));
            }
            let certified = self
                .read_certified_lane_block_artifact_from_paths_locked(
                    binding.lane_id,
                    lane_block_height,
                    &certified_data,
                    &certified_index,
                    false,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired lane certified artifact is malformed or incomplete",
                    )
                })?;
            let descriptor = &certified.proposal.descriptor;
            if descriptor.lane_incarnation != binding.incarnation {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane certified artifact has the wrong incarnation",
                ));
            }
            let release = merge_releases
                .iter()
                .find(|release| {
                    release.lane_id == descriptor.lane_id
                        && release.dataspace_id == descriptor.dataspace_id
                        && release.lane_incarnation == descriptor.lane_incarnation
                        && release.lane_block_height == descriptor.lane_block_height
                })
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::WouldBlock,
                        "retired lane certified artifact is not snapshot-proven as merge-applied",
                    )
                })?;
            let receipt = self
                .read_lane_block_application_receipt_from_paths_locked(
                    binding.lane_id,
                    lane_block_height,
                    &receipt_data,
                    &receipt_index,
                    false,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::WouldBlock,
                        "retired lane merge application receipt is missing or malformed",
                    )
                })?;
            if receipt.format != LaneBlockApplicationReceiptArtifactFormat::MergeExecution
                || receipt.proposal != certified.proposal
                || !self.lane_block_application_receipt_matches_merge_log(&receipt)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane merge receipt does not match its certified artifact and merge log",
                ));
            }
            let receipt_bytes = receipt.encode_framed()?;
            let receipt_hash =
                Hash::new_from_chunks(&[MERGE_RELEASE_RECEIPT_DOMAIN, receipt_bytes.as_slice()]);
            if receipt_hash != release.receipt_hash
                || receipt.merge_epoch_id != Some(release.merge_epoch_id)
                || receipt.merge_entry_hash != Some(release.merge_entry_hash)
                || receipt.merge_carrier_block_height != Some(release.application_block_height)
                || receipt.merge_carrier_block_hash != Some(release.application_block_hash)
                || receipt.application_block_height != release.application_block_height
                || receipt.application_block_hash != release.application_block_hash
                || receipt.merge_source_bundle_hash != Some(release.source_bundle_hash)
                || receipt.merge_batch_identity_hash != Some(release.batch_identity_hash)
                || receipt.merge_batch_hash != Some(release.batch_hash)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane merge receipt differs from the snapshot release proof",
                ));
            }
        }
        Ok(())
    }

    fn geometry_indexed_sidecar_payload_heights(
        &self,
        data_path: &Path,
        index_path: &Path,
    ) -> Result<BTreeSet<u64>> {
        self.geometry_indexed_sidecar_payload_heights_bounded(
            data_path,
            index_path,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
        )
    }

    fn geometry_retirement_sidecar_payload_heights(
        &self,
        data_path: &Path,
        index_path: &Path,
    ) -> Result<BTreeSet<u64>> {
        self.geometry_indexed_sidecar_payload_heights_bounded(
            data_path,
            index_path,
            MAX_LANE_RETIREMENT_WORK_ITEMS,
        )
    }

    fn geometry_indexed_sidecar_payload_heights_bounded(
        &self,
        data_path: &Path,
        index_path: &Path,
        max_entries: usize,
    ) -> Result<BTreeSet<u64>> {
        let data_exists = self.validate_path_kind(data_path, false)?;
        let index_exists = self.validate_path_kind(index_path, false)?;
        if !data_exists && !index_exists {
            return Ok(BTreeSet::new());
        }
        if data_exists != index_exists {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retired lane indexed sidecar is only partially present",
            ));
        }
        let data =
            File::open(data_path).map_err(|error| Error::IO(error, data_path.to_path_buf()))?;
        self.verify_open_geometry_file(data_path, &data)?;
        let data_len = data
            .metadata()
            .map_err(|error| Error::IO(error, data_path.to_path_buf()))?
            .len();
        let mut index =
            File::open(index_path).map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
        self.verify_open_geometry_file(index_path, &index)?;
        let index_len = index
            .metadata()
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?
            .len();
        let layout = SidecarIndexLayout::read_from(&mut index, index_len).map_err(|message| {
            self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("retired lane sidecar index is malformed: {message}"),
            )
        })?;
        if layout.aligned_len != index_len
            || usize::try_from(layout.entry_count).unwrap_or(usize::MAX) > max_entries
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retired lane sidecar index is misaligned or oversized",
            ));
        }
        let mut heights = BTreeSet::new();
        index
            .seek(SeekFrom::Start(layout.entries_offset))
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
        let mut encoded = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        for offset in 0..layout.entry_count {
            index
                .read_exact(&mut encoded)
                .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
            let entry = SidecarIndexEntry::from_bytes(encoded);
            if entry.len == 0 {
                continue;
            }
            let end = entry.offset.checked_add(entry.len).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane sidecar payload range overflows",
                )
            })?;
            if end > data_len {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane sidecar payload points past its data file",
                ));
            }
            let height = layout.base_height.checked_add(offset).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane sidecar height overflows",
                )
            })?;
            if height == 0 || !heights.insert(height) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane sidecar has a zero or duplicate height",
                ));
            }
        }
        self.verify_open_geometry_file(index_path, &index)?;
        self.verify_open_geometry_file(data_path, &data)?;
        Ok(heights)
    }

    fn regular_geometry_archive_tree_bytes(root: &Path) -> Result<u64> {
        let mut bytes = 0_u64;
        let mut entries_seen = 0_usize;
        let mut pending = vec![(root.to_path_buf(), 0_usize)];
        while let Some((directory, depth)) = pending.pop() {
            if depth > MAX_GEOMETRY_ARCHIVE_DEPTH {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive exceeds the maximum directory depth",
                    ),
                    directory,
                ));
            }
            let metadata = fs::symlink_metadata(&directory)
                .map_err(|error| Error::IO(error, directory.clone()))?;
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry block archive root is not a regular directory",
                    ),
                    directory,
                ));
            }
            let entries =
                fs::read_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
            for entry in entries {
                let entry = entry.map_err(|error| Error::IO(error, directory.clone()))?;
                entries_seen = entries_seen.saturating_add(1);
                if entries_seen > MAX_GEOMETRY_ARCHIVE_ENTRIES {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry archive exceeds the maximum entry count",
                        ),
                        directory,
                    ));
                }
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .map_err(|error| Error::IO(error, path.clone()))?;
                if file_type.is_symlink() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry block archive contains a symbolic link",
                        ),
                        path,
                    ));
                }
                if file_type.is_file() {
                    bytes = bytes.saturating_add(
                        entry
                            .metadata()
                            .map_err(|error| Error::IO(error, path.clone()))?
                            .len(),
                    );
                } else if file_type.is_dir() {
                    pending.push((path, depth.saturating_add(1)));
                } else {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry block archive contains a non-regular filesystem entry",
                        ),
                        path,
                    ));
                }
            }
        }
        Ok(bytes)
    }

    fn move_geometry_path(&self, source: &Path, target: &Path, directory: bool) -> Result<()> {
        if source == target {
            return self
                .validate_path_kind(source, directory)?
                .then_some(())
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::NotFound,
                        "shared lane geometry move path is missing",
                    )
                });
        }
        let source_exists = self.validate_path_kind(source, directory)?;
        let target_exists = self.validate_path_kind(target, directory)?;
        match (source_exists, target_exists) {
            (false, false) | (false, true) => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "lane geometry move source is missing",
                    ),
                    source.to_path_buf(),
                ));
            }
            (true, true) => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "both source and target exist during idempotent geometry move",
                    ),
                    target.to_path_buf(),
                ));
            }
            (true, false) => {}
        }
        let source_identity = self.geometry_path_identity(source, directory)?;
        self.sync_geometry_path_contents(source, directory)?;
        self.require_geometry_path_identity(source, directory, source_identity)?;
        if let Some(parent) = target.parent() {
            create_dir_all_with_context(parent)?;
            self.validate_path_kind(parent, true)?;
            self.sync_geometry_parent(Some(parent))?;
        }
        self.inject_geometry_move_target_collision_for_test(target, directory)?;
        rename_geometry_path_noreplace(source, target)
            .map_err(|error| Error::IO(error, source.to_path_buf()))?;
        self.require_geometry_path_identity(target, directory, source_identity)?;
        self.sync_geometry_parent(source.parent())?;
        if source.parent() != target.parent() {
            self.sync_geometry_parent(target.parent())?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn inject_geometry_move_target_collision_for_test(
        &self,
        target: &Path,
        directory: bool,
    ) -> Result<()> {
        let inject_collision = {
            let mut hook = GEOMETRY_MOVE_TARGET_COLLISION
                .lock()
                .expect("geometry move collision hook lock");
            if hook.as_deref() == Some(target) {
                hook.take();
                true
            } else {
                false
            }
        };
        if inject_collision {
            if directory {
                fs::create_dir(target).map_err(|error| Error::IO(error, target.to_path_buf()))?;
            } else {
                fs::write(target, b"injected-no-clobber-target")
                    .map_err(|error| Error::IO(error, target.to_path_buf()))?;
            }
        }
        Ok(())
    }

    #[cfg(not(test))]
    fn inject_geometry_move_target_collision_for_test(
        &self,
        _target: &Path,
        _directory: bool,
    ) -> Result<()> {
        Ok(())
    }

    fn sync_geometry_path_contents(&self, path: &Path, directory: bool) -> Result<()> {
        if !directory {
            let file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
            self.verify_open_geometry_file(path, &file)?;
            file.sync_all()
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
            return Ok(());
        }

        let root_identity = self.geometry_path_identity(path, true)?;
        let mut seen = 0_usize;
        let mut pending = vec![(path.to_path_buf(), 0_usize)];
        let mut directories = Vec::new();
        while let Some((current, depth)) = pending.pop() {
            if depth > MAX_GEOMETRY_ARCHIVE_DEPTH {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry storage exceeds the maximum directory depth",
                    ),
                    current,
                ));
            }
            self.geometry_path_identity(&current, true)?;
            directories.push(current.clone());
            for entry in
                fs::read_dir(&current).map_err(|error| Error::IO(error, current.clone()))?
            {
                let entry = entry.map_err(|error| Error::IO(error, current.clone()))?;
                seen = seen.saturating_add(1);
                if seen > MAX_GEOMETRY_ARCHIVE_ENTRIES {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry storage exceeds the maximum entry count",
                        ),
                        current,
                    ));
                }
                let child = entry.path();
                let file_type = entry
                    .file_type()
                    .map_err(|error| Error::IO(error, child.clone()))?;
                if file_type.is_symlink() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry storage contains a symbolic link",
                        ),
                        child,
                    ));
                }
                if file_type.is_dir() {
                    pending.push((child, depth.saturating_add(1)));
                } else if file_type.is_file() {
                    let file =
                        File::open(&child).map_err(|error| Error::IO(error, child.clone()))?;
                    self.verify_open_geometry_file(&child, &file)?;
                    file.sync_all().map_err(|error| Error::IO(error, child))?;
                } else {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry storage contains a non-regular entry",
                        ),
                        child,
                    ));
                }
            }
        }
        for directory in directories.into_iter().rev() {
            sync_dir(&directory).map_err(|error| Error::IO(error, directory))?;
        }
        self.require_geometry_path_identity(path, true, root_identity)
    }

    fn provision_geometry_binding(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        if self.validate_path_kind(&blocks, true)? {
            let marker_path = blocks.join(MARKER_FILE_NAME);
            if self.validate_path_kind(&marker_path, false)? {
                self.require_lane_marker(binding)?;
            } else {
                preflight_empty_block_store_without_marker(&blocks, Some(binding), false)?;
                self.write_lane_marker(binding)?;
            }
            self.sync_geometry_path_contents(&blocks, true)?;
        } else {
            if let Some(parent) = blocks.parent() {
                create_dir_all_with_context(parent)?;
                self.validate_path_kind(parent, true)?;
            }
            fs::create_dir(&blocks).map_err(|error| Error::IO(error, blocks.clone()))?;
            self.sync_geometry_parent(blocks.parent())?;
            self.write_lane_marker(binding)?;
        }
        let mut store = BlockStore::new(&blocks);
        store.create_files_if_they_do_not_exist()?;
        self.sync_geometry_path_contents(&blocks, true)?;
        if let Some(parent) = merge.parent() {
            create_dir_all_with_context(parent)?;
        }
        if !self.validate_path_kind(&merge, false)? {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&merge)
                .map_err(|error| Error::IO(error, merge.clone()))?;
            self.verify_open_geometry_file(&merge, &file)?;
            file.sync_all()
                .map_err(|error| Error::IO(error, merge.clone()))?;
            self.sync_geometry_parent(merge.parent())?;
        }
        Ok(())
    }

    fn ensure_authoritative_lane_markers(
        &self,
        lane_config: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<()> {
        for entry in lane_config.entries() {
            let binding = self.geometry_binding(entry, incarnations, activation_heights)?;
            let blocks = self.binding_blocks_path(&binding);
            let merge = self.binding_merge_path(&binding);
            let blocks_exists = self.validate_path_kind(&blocks, true)?;
            let merge_exists = self.validate_path_kind(&merge, false)?;
            if blocks_exists != merge_exists {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "active lane block and merge storage are only partially present",
                ));
            }
            if !blocks_exists {
                return Err(self.geometry_error(
                    ErrorKind::NotFound,
                    "authoritative lane storage is missing; refusing to provision an empty replacement",
                ));
            }
            let marker_path = blocks.join(MARKER_FILE_NAME);
            if !self.validate_path_kind(&marker_path, false)? {
                if binding.activation_height != 0 {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "active dynamic lane storage has no incarnation marker",
                        ),
                        marker_path,
                    ));
                }
                self.write_lane_marker(&binding)?;
            } else {
                self.require_lane_marker(&binding)?;
            }
        }
        Ok(())
    }

    fn require_lane_marker(&self, binding: &LaneGeometryBinding) -> Result<()> {
        self.require_lane_marker_at(&self.binding_blocks_path(binding), binding)
    }

    fn lane_marker_matches_at_if_present(
        &self,
        blocks: &Path,
        binding: &LaneGeometryBinding,
    ) -> Result<Option<bool>> {
        if !self.validate_path_kind(blocks, true)? {
            return Ok(None);
        }
        let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
        Ok(Some(
            marker.version == MARKER_VERSION
                && marker.lane_id == binding.lane_id
                && marker.incarnation == binding.incarnation
                && marker.activation_height == binding.activation_height,
        ))
    }

    fn require_lane_marker_at(
        &self,
        blocks_path: &Path,
        binding: &LaneGeometryBinding,
    ) -> Result<()> {
        let path = blocks_path.join(MARKER_FILE_NAME);
        let marker = self.read_lane_marker(&path)?;
        self.require_lane_marker_value(&marker, blocks_path, binding)
    }

    fn require_lane_marker_value(
        &self,
        marker: &LaneIncarnationMarker,
        blocks_path: &Path,
        binding: &LaneGeometryBinding,
    ) -> Result<()> {
        if marker.version != MARKER_VERSION
            || marker.lane_id != binding.lane_id
            || marker.incarnation != binding.incarnation
            || marker.activation_height != binding.activation_height
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane storage incarnation marker does not match authoritative binding",
                ),
                blocks_path.join(MARKER_FILE_NAME),
            ));
        }
        Ok(())
    }

    fn read_lane_marker(&self, path: &Path) -> Result<LaneIncarnationMarker> {
        let identity = self.geometry_path_identity(path, false)?;
        let mut file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.verify_open_geometry_file(path, &file)?;
        let length = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?
            .len();
        if length > MAX_LANE_MARKER_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane incarnation marker exceeds its encoded byte limit",
                ),
                path.to_path_buf(),
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(length)?);
        (&mut file)
            .take(MAX_LANE_MARKER_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_LANE_MARKER_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane incarnation marker exceeds its encoded byte limit",
                ),
                path.to_path_buf(),
            ));
        }
        self.verify_open_geometry_file(path, &file)?;
        self.require_geometry_path_identity(path, false, identity)?;
        decode_exact(&bytes).map_err(Error::NoritoFrame)
    }

    fn write_lane_marker(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        let merge_log_digest = if self.validate_path_kind(&merge, false)? {
            self.geometry_merge_log_digest(&merge)?
        } else {
            empty_geometry_merge_digest()
        };
        self.write_lane_marker_at(&blocks, binding, None, None, merge_log_digest)
    }

    fn write_lane_marker_at(
        &self,
        blocks: &Path,
        binding: &LaneGeometryBinding,
        move_target_blocks: Option<String>,
        move_target_merge: Option<String>,
        merge_log_digest: Hash,
    ) -> Result<()> {
        create_dir_all_with_context(blocks)?;
        let path = blocks.join(MARKER_FILE_NAME);
        let temp = blocks.join(MARKER_TEMP_FILE_NAME);
        self.validate_path_kind(&path, false)?;
        let marker = LaneIncarnationMarker {
            version: MARKER_VERSION,
            lane_id: binding.lane_id,
            incarnation: binding.incarnation,
            activation_height: binding.activation_height,
            move_target_blocks,
            move_target_merge,
            block_store_digest: self.geometry_block_store_digest(blocks)?,
            merge_log_digest,
        };
        self.prepare_lane_marker_temp_for_write(&temp, binding, &marker)?;
        self.atomic_write_geometry_file(&path, &temp, &marker.encode())
    }

    fn prepare_lane_marker_temp_for_write(
        &self,
        temp: &Path,
        binding: &LaneGeometryBinding,
        intended: &LaneIncarnationMarker,
    ) -> Result<()> {
        if !self.validate_path_kind(temp, false)? {
            return Ok(());
        }
        let identity = self.geometry_path_identity(temp, false)?;
        let stale = self.read_lane_marker(temp)?;
        if &stale == intended {
            return Ok(());
        }
        self.require_lane_marker_value(&stale, temp.parent().unwrap_or(temp), binding)?;
        match (
            stale.move_target_blocks.as_deref(),
            stale.move_target_merge.as_deref(),
        ) {
            (None, None) => {}
            (Some(blocks), Some(merge)) => {
                self.resolve_relative_path(blocks)?;
                self.resolve_relative_path(merge)?;
            }
            _ => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane marker temp has incomplete pair-move evidence",
                    ),
                    temp.to_path_buf(),
                ));
            }
        }
        if stale.block_store_digest != intended.block_store_digest
            || stale.merge_log_digest != intended.merge_log_digest
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "lane marker temp does not belong to the current physical geometry pair",
                ),
                temp.to_path_buf(),
            ));
        }
        self.require_geometry_path_identity(temp, false, identity)?;
        fs::remove_file(temp).map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        self.sync_geometry_parent(temp.parent())?;
        if self.validate_path_kind(temp, false)? {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "lane marker temp remained after authenticated crash cleanup",
                ),
                temp.to_path_buf(),
            ));
        }
        Ok(())
    }

    fn geometry_bindings(
        &self,
        lane_config: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<Vec<LaneGeometryBinding>> {
        let bindings = lane_config
            .entries()
            .iter()
            .map(|entry| self.geometry_binding(entry, incarnations, activation_heights))
            .collect::<Result<Vec<_>>>()?;
        self.validate_geometry_binding_set(&bindings)?;
        Ok(bindings)
    }

    fn geometry_binding(
        &self,
        entry: &LaneConfigEntry,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<LaneGeometryBinding> {
        let incarnation = incarnations.get(&entry.lane_id).copied().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry is missing an incarnation commitment",
            )
        })?;
        let activation_height =
            activation_heights
                .get(&entry.lane_id)
                .copied()
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidInput,
                        "lane geometry is missing an incarnation activation height",
                    )
                })?;
        Ok(LaneGeometryBinding {
            lane_id: entry.lane_id,
            incarnation,
            activation_height,
            blocks_path: self.relative_geometry_path(&entry.blocks_dir(&self.store_root))?,
            merge_path: self.relative_geometry_path(&entry.merge_log_path(&self.store_root))?,
        })
    }

    fn relative_geometry_path(&self, path: &Path) -> Result<String> {
        let relative = path.strip_prefix(&self.store_root).map_err(|_| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry path escapes the Kura store root",
            )
        })?;
        validate_relative_path(relative)?;
        relative.to_str().map(str::to_owned).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry path is not valid UTF-8",
            )
        })
    }

    fn resolve_relative_path(&self, relative: &str) -> Result<PathBuf> {
        let relative = Path::new(relative);
        validate_relative_path(relative)?;
        Ok(self.store_root.join(relative))
    }

    fn binding_blocks_path(&self, binding: &LaneGeometryBinding) -> PathBuf {
        self.store_root.join(&binding.blocks_path)
    }

    fn binding_merge_path(&self, binding: &LaneGeometryBinding) -> PathBuf {
        self.store_root.join(&binding.merge_path)
    }

    fn validate_path_kind(&self, path: &Path, directory: bool) -> Result<bool> {
        self.validate_geometry_ancestors(path)?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(Error::IO(error, path.to_path_buf())),
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink()
            || (directory && !file_type.is_dir())
            || (!directory && !file_type.is_file())
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry path is a symlink or has the wrong file type",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(true)
    }

    fn geometry_path_identity(&self, path: &Path, directory: bool) -> Result<GeometryFileIdentity> {
        if !self.validate_path_kind(path, directory)? {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::NotFound, "lane geometry path is missing"),
                path.to_path_buf(),
            ));
        }
        let metadata =
            fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let file_type = metadata.file_type();
        if file_type.is_symlink()
            || (directory && !file_type.is_dir())
            || (!directory && !file_type.is_file())
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry path changed type during identity validation",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(geometry_file_identity(&metadata))
    }

    fn require_geometry_path_identity(
        &self,
        path: &Path,
        directory: bool,
        expected: GeometryFileIdentity,
    ) -> Result<()> {
        let actual = self.geometry_path_identity(path, directory)?;
        if actual != expected {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry path inode changed during a protected operation",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(())
    }

    fn verify_open_geometry_file(&self, path: &Path, file: &File) -> Result<()> {
        let path_identity = self.geometry_path_identity(path, false)?;
        let metadata = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !metadata.is_file() || geometry_file_identity(&metadata) != path_identity {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "opened lane geometry file does not match its directory entry",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(())
    }

    fn validate_geometry_ancestors(&self, path: &Path) -> Result<()> {
        let root_metadata = fs::symlink_metadata(&self.store_root)
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura geometry store root must be a non-symlink directory",
                ),
                self.store_root.clone(),
            ));
        }
        let relative = path.strip_prefix(&self.store_root).map_err(|_| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry path escapes the Kura store root",
            )
        })?;
        if relative.as_os_str().is_empty() {
            return Ok(());
        }
        validate_relative_path(relative)?;
        let mut cursor = self.store_root.clone();
        let components = relative.components().collect::<Vec<_>>();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            cursor.push(component.as_os_str());
            match fs::symlink_metadata(&cursor) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry path traverses a symlink",
                        ),
                        cursor,
                    ));
                }
                Ok(metadata) if !metadata.file_type().is_dir() => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry path traverses a non-directory",
                        ),
                        cursor,
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(error) => return Err(Error::IO(error, cursor)),
            }
        }
        Ok(())
    }

    fn retarget_active_geometry_paths(
        &self,
        old_blocks: &Path,
        new_blocks: &Path,
        old_merge: &Path,
        new_merge: &Path,
    ) -> Result<()> {
        {
            let mut active = self.active_blocks_dir.lock();
            if active.as_path() == old_blocks {
                active.clone_from(&new_blocks.to_path_buf());
                let _write_guard = self.block_store_write_lock.lock();
                self.block_store
                    .lock()
                    .retarget_path(new_blocks.to_path_buf())?;
                self.invalidate_durable_budget_snapshot();
            }
        }
        {
            let mut active = self.active_merge_path.lock();
            if active.as_path() == old_merge {
                active.clone_from(&new_merge.to_path_buf());
            }
        }
        {
            let mut plain_text = self.block_plain_text_path.lock();
            if let Some(path) = plain_text.as_mut()
                && let Ok(suffix) = path.strip_prefix(old_blocks)
            {
                *path = new_blocks.join(suffix);
            }
        }
        Ok(())
    }

    fn read_geometry_file_bytes(&self, path: &Path) -> Result<Option<Vec<u8>>> {
        if !self.validate_path_kind(path, false)? {
            return Ok(None);
        }
        let mut file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.verify_open_geometry_file(path, &file)?;
        let initial_metadata = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let file_len = initial_metadata.len();
        let identity = geometry_file_identity(&initial_metadata);
        if file_len > MAX_GEOMETRY_JOURNAL_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry journal exceeds the encoded byte limit",
                ),
                path.to_path_buf(),
            ));
        }
        let capacity = usize::try_from(file_len)?;
        let mut bytes = Vec::with_capacity(capacity);
        (&mut file)
            .take(MAX_GEOMETRY_JOURNAL_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let final_len = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?
            .len();
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_GEOMETRY_JOURNAL_BYTES
            || final_len != file_len
            || bytes.len() != capacity
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry journal changed while it was read or exceeded its encoded byte limit",
                ),
                path.to_path_buf(),
            ));
        }
        self.verify_open_geometry_file(path, &file)?;
        self.require_geometry_path_identity(path, false, identity)?;
        Ok(Some(bytes))
    }

    fn read_lane_geometry_journal(&self) -> Result<LaneGeometryJournal> {
        let path = self.lane_geometry_journal_path();
        let Some(bytes) = self.read_geometry_file_bytes(&path)? else {
            return Ok(LaneGeometryJournal::default());
        };
        let journal = decode_exact::<LaneGeometryJournal>(&bytes).map_err(Error::NoritoFrame)?;
        if journal.version != JOURNAL_VERSION {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "unsupported lane geometry journal version {}; expected {JOURNAL_VERSION}",
                        journal.version
                    ),
                ),
                path,
            ));
        }
        self.validate_lane_geometry_journal(&journal)?;
        Ok(journal)
    }

    fn restore_lane_geometry_journal_file(
        &self,
        prior_bytes: Option<&[u8]>,
        published_bytes: &[u8],
        publication_temp_preexisted: bool,
    ) -> Result<()> {
        let path = self.lane_geometry_journal_path();
        let current_bytes = self.read_geometry_file_bytes(&path)?;
        match (prior_bytes, current_bytes.as_deref()) {
            (None, None) => {}
            (None, Some(current)) if current == published_bytes => {
                fs::remove_file(&path).map_err(|error| Error::IO(error, path.clone()))?;
                self.sync_geometry_parent(path.parent())?;
            }
            (None, Some(_)) => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "refusing to remove an unexpected lane geometry journal while restoring prior absence",
                ));
            }
            (Some(prior), Some(current)) if current == prior => {}
            (Some(prior), Some(current)) if current == published_bytes => {
                let restore_temp = self.store_root.join(JOURNAL_RESTORE_TEMP_FILE_NAME);
                self.atomic_write_geometry_file(&path, &restore_temp, prior)?;
            }
            (Some(prior), None) => {
                let restore_temp = self.store_root.join(JOURNAL_RESTORE_TEMP_FILE_NAME);
                self.atomic_write_geometry_file(&path, &restore_temp, prior)?;
            }
            (Some(_), Some(_)) => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "refusing to overwrite an unexpected lane geometry journal while restoring the exact prior value",
                ));
            }
        }

        // A preexisting temp is never ours to remove. `atomic_write_geometry_file` consumes it
        // only when its bytes exactly equal this publication; otherwise it is left untouched and
        // the publication fails. A temp absent at entry can be cleaned only when its full value
        // proves that it belongs to this attempt.
        if !publication_temp_preexisted {
            let publication_temp = self.store_root.join(JOURNAL_TEMP_FILE_NAME);
            if let Some(temp_bytes) = self.read_geometry_file_bytes(&publication_temp)? {
                if temp_bytes != published_bytes {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "refusing to remove an unexpected lane geometry publication temp file",
                        ),
                        publication_temp,
                    ));
                }
                fs::remove_file(&publication_temp)
                    .map_err(|error| Error::IO(error, publication_temp.clone()))?;
                self.sync_geometry_parent(publication_temp.parent())?;
            }
            if self.validate_path_kind(&publication_temp, false)? {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry publication temp remained after rollback cleanup",
                    ),
                    publication_temp,
                ));
            }
        }

        let restored_bytes = self.read_geometry_file_bytes(&path)?;
        if restored_bytes.as_deref() != prior_bytes {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry publication rollback did not restore the exact prior journal value",
            ));
        }
        Ok(())
    }

    fn validate_lane_geometry_journal(&self, journal: &LaneGeometryJournal) -> Result<()> {
        validate_lane_geometry_journal_structure(&self.store_root, journal)?;
        self.validate_lane_geometry_journal_with_durable_evidence(journal)
    }

    fn validate_lane_geometry_journal_with_durable_evidence(
        &self,
        journal: &LaneGeometryJournal,
    ) -> Result<()> {
        if journal.version != JOURNAL_VERSION
            || journal.records.len() > MAX_GEOMETRY_TRANSITIONS
            || journal.pending_archive_gc.len() > MAX_GEOMETRY_TRANSITIONS
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal has an unsupported version or too many transitions",
            ));
        }
        if journal.configured_primary_binding.is_some() && journal.configured_catalog_hash.is_none()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "configured primary geometry binding has no configured-catalog baseline",
            ));
        }
        if let Some(checkpoint) = journal.checkpoint.as_ref() {
            self.validate_lane_geometry_checkpoint(checkpoint)?;
            if journal.records.first().is_some_and(|record| {
                record.previous_catalog != checkpoint.catalog
                    || record.previous_lineage_root != checkpoint.lineage_root
            }) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal retained history does not start at its checkpoint catalog",
                ));
            }
            if let Some(first) = journal.records.first()
                && (checkpoint
                    .transition_sequence
                    .is_some_and(|sequence| first.transition_sequence <= sequence)
                    || first.transition_height <= checkpoint.snapshot_height)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retained lane geometry history does not advance beyond its checkpoint cursor",
                ));
            }
        } else if !journal.pending_archive_gc.is_empty() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal has pending archive GC without a durable checkpoint",
            ));
        }
        if let Some(primary) = journal.configured_primary_binding.as_ref() {
            if primary.lane_id != LaneId::SINGLE || primary.activation_height != 0 {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured primary geometry binding is not lane zero at activation zero",
                ));
            }
            self.validate_geometry_binding_from_journal(primary)?;
        }
        self.validate_pending_lane_geometry_gc(journal)?;
        let mut transition_ids = BTreeSet::new();
        let mut retained_paths = BTreeSet::new();
        if journal.records.windows(2).any(|pair| {
            pair[0].transition_sequence >= pair[1].transition_sequence
                || pair[0].transition_height > pair[1].transition_height
        }) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal transition cursor is not monotonic",
            ));
        }
        for (record_index, record) in journal.records.iter().enumerate() {
            if record.transition_id
                != geometry_transition_id(
                    record.transition_sequence,
                    record.transition_height,
                    record.previous_catalog,
                    record.previous_lineage_root,
                    record.updated_catalog,
                    record.updated_lineage_root,
                )
                || record.previous_catalog == record.updated_catalog
                    && record.previous_lineage_root == record.updated_lineage_root
                || lineage_root_is_zero(record.previous_lineage_root)
                || lineage_root_is_zero(record.updated_lineage_root)
                || !transition_ids.insert(record.transition_id)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal contains an invalid or duplicate transition",
                ));
            }
            for bindings in [&record.previous_bindings, &record.updated_bindings] {
                self.validate_geometry_binding_set(bindings)?;
            }
            if geometry_catalog_fingerprint(&record.previous_bindings) != record.previous_catalog
                || geometry_catalog_fingerprint(&record.updated_bindings) != record.updated_catalog
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal catalog fingerprint does not match its bindings",
                ));
            }
            if record_index > 0
                && (journal.records[record_index - 1].updated_catalog != record.previous_catalog
                    || journal.records[record_index - 1].updated_lineage_root
                        != record.previous_lineage_root)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal transition chain is not contiguous",
                ));
            }
            let transition_hex = hex::encode(record.transition_id.as_ref());
            let previous_by_lane = record
                .previous_bindings
                .iter()
                .map(|binding| (binding.lane_id, binding))
                .collect::<BTreeMap<_, _>>();
            let updated_by_lane = record
                .updated_bindings
                .iter()
                .map(|binding| (binding.lane_id, binding))
                .collect::<BTreeMap<_, _>>();
            if record
                .operations
                .windows(2)
                .any(|pair| pair[0].lane_id >= pair[1].lane_id)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal operations are duplicated or unsorted",
                ));
            }
            let mut lane_ids = BTreeSet::new();
            for operation in &record.operations {
                if !lane_ids.insert(operation.lane_id)
                    || operation
                        .previous
                        .as_ref()
                        .is_some_and(|binding| binding.lane_id != operation.lane_id)
                    || operation
                        .updated
                        .as_ref()
                        .is_some_and(|binding| binding.lane_id != operation.lane_id)
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal contains duplicate or mismatched lane operations",
                    ));
                }
                let expected_root = format!(
                    "retired/lane_geometry/{transition_hex}/lane_{:010}",
                    operation.lane_id.as_u32()
                );
                let expected_paths = [
                    format!("{expected_root}/previous_blocks"),
                    format!("{expected_root}/previous_merge.log"),
                    format!("{expected_root}/unpublished_blocks"),
                    format!("{expected_root}/unpublished_merge.log"),
                ];
                let actual_paths = [
                    &operation.archived_blocks_path,
                    &operation.archived_merge_path,
                    &operation.unpublished_blocks_path,
                    &operation.unpublished_merge_path,
                ];
                if actual_paths
                    .iter()
                    .zip(expected_paths.iter())
                    .any(|(actual, expected)| *actual != expected)
                    || actual_paths
                        .iter()
                        .any(|path| !retained_paths.insert((*path).clone()))
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal contains forged or colliding archive paths",
                    ));
                }
                for binding in operation.previous.iter().chain(operation.updated.iter()) {
                    self.validate_geometry_binding_from_journal(binding)?;
                }
                if operation.previous.as_ref() != previous_by_lane.get(&operation.lane_id).copied()
                    || operation.updated.as_ref()
                        != updated_by_lane.get(&operation.lane_id).copied()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry operation does not match its authenticated catalog bindings",
                    ));
                }
                let shape_is_valid = match operation.kind {
                    LaneGeometryOperationKind::Create => {
                        operation.previous.is_none() && operation.updated.is_some()
                    }
                    LaneGeometryOperationKind::Retire => {
                        operation.previous.is_some() && operation.updated.is_none()
                    }
                    LaneGeometryOperationKind::Replace => operation
                        .previous
                        .as_ref()
                        .zip(operation.updated.as_ref())
                        .is_some_and(|(previous, updated)| {
                            previous.incarnation != updated.incarnation
                                || previous.activation_height != updated.activation_height
                        }),
                    LaneGeometryOperationKind::Relabel => operation
                        .previous
                        .as_ref()
                        .zip(operation.updated.as_ref())
                        .is_some_and(|(previous, updated)| {
                            previous.incarnation == updated.incarnation
                                && previous.activation_height == updated.activation_height
                                && (previous.blocks_path != updated.blocks_path
                                    || previous.merge_path != updated.merge_path)
                        }),
                };
                if !shape_is_valid {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal contains an invalid operation shape",
                    ));
                }
            }
            let expected_changed_lanes = previous_by_lane
                .keys()
                .chain(updated_by_lane.keys())
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|lane_id| {
                    previous_by_lane.get(lane_id).copied() != updated_by_lane.get(lane_id).copied()
                })
                .count();
            if record.operations.len() != expected_changed_lanes {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal omits or invents a catalog binding operation",
                ));
            }
        }
        Ok(())
    }

    fn validate_lane_geometry_checkpoint(
        &self,
        checkpoint: &LaneGeometrySnapshotCheckpoint,
    ) -> Result<()> {
        if checkpoint.version != CHECKPOINT_VERSION
            || checkpoint
                .snapshot_state_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || lineage_root_is_zero(checkpoint.lineage_root)
            || checkpoint.catalog != geometry_catalog_fingerprint(&checkpoint.bindings)
            || checkpoint.commitment != geometry_checkpoint_commitment(checkpoint)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry checkpoint commitment or catalog is invalid",
            ));
        }
        self.validate_geometry_binding_set(&checkpoint.bindings)?;
        self.validate_geometry_merge_releases(
            &checkpoint.merge_releases,
            checkpoint.snapshot_height,
        )?;
        if checkpoint.snapshot_height == 0
            || checkpoint.snapshot_block_hash.is_none()
            || checkpoint
                .snapshot_block_hash
                .is_some_and(|hash| hash.as_ref().iter().all(|byte| *byte == 0))
            || checkpoint
                .bindings
                .iter()
                .any(|binding| binding.activation_height > checkpoint.snapshot_height)
            || checkpoint
                .transition_height
                .is_some_and(|height| height > checkpoint.snapshot_height)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry checkpoint height, block hash, or activation is invalid",
            ));
        }
        match (
            checkpoint.transition_sequence,
            checkpoint.transition_height,
            checkpoint.transition_previous_catalog,
            checkpoint.transition_previous_lineage_root,
            checkpoint.transition_id,
        ) {
            (None, None, None, None, None) => {}
            (
                Some(sequence),
                Some(height),
                Some(previous_catalog),
                Some(previous_lineage_root),
                Some(transition_id),
            ) if !lineage_root_is_zero(previous_lineage_root)
                && transition_id
                    == geometry_transition_id(
                        sequence,
                        height,
                        previous_catalog,
                        previous_lineage_root,
                        checkpoint.catalog,
                        checkpoint.lineage_root,
                    ) => {}
            _ => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry checkpoint transition binding is invalid",
                ));
            }
        }
        Ok(())
    }

    fn validate_pending_lane_geometry_gc(&self, journal: &LaneGeometryJournal) -> Result<()> {
        if journal.pending_archive_gc.is_empty() {
            if journal
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.pending_archive_gc_root.is_some())
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry checkpoint commits a missing pending archive GC set",
                ));
            }
            return Ok(());
        }
        let checkpoint = journal.checkpoint.as_ref().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "pending lane geometry GC has no checkpoint",
            )
        })?;
        let retained_ids = journal
            .records
            .iter()
            .map(|record| record.transition_id)
            .collect::<BTreeSet<_>>();
        let mut pending_ids = BTreeSet::new();
        for (index, pending) in journal.pending_archive_gc.iter().enumerate() {
            let intent = &pending.intent;
            let standalone = LaneGeometryJournal {
                version: JOURNAL_VERSION,
                configured_catalog_hash: None,
                configured_primary_binding: None,
                checkpoint: None,
                pending_archive_gc: Vec::new(),
                records: vec![intent.clone()],
            };
            self.validate_lane_geometry_journal(&standalone)?;
            if intent.phase != LaneGeometryPhase::CatalogPublished
                || !pending_ids.insert(intent.transition_id)
                || retained_ids.contains(&intent.transition_id)
                || index > 0
                    && (journal.pending_archive_gc[index - 1].intent.updated_catalog
                        != intent.previous_catalog
                        || journal.pending_archive_gc[index - 1]
                            .intent
                            .updated_lineage_root
                            != intent.previous_lineage_root
                        || journal.pending_archive_gc[index - 1]
                            .intent
                            .transition_sequence
                            >= intent.transition_sequence
                        || journal.pending_archive_gc[index - 1]
                            .intent
                            .transition_height
                            > intent.transition_height)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal has forged or non-contiguous pending archive GC",
                ));
            }
        }
        if checkpoint.pending_archive_gc_root
            != Some(geometry_pending_archive_gc_root(
                &journal.pending_archive_gc,
            ))
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry checkpoint does not bind its exact pending archive GC set",
            ));
        }
        let last = journal
            .pending_archive_gc
            .last()
            .expect("non-empty pending archive GC");
        if last.intent.updated_catalog != checkpoint.catalog
            || last.intent.updated_lineage_root != checkpoint.lineage_root
            || checkpoint.transition_sequence != Some(last.intent.transition_sequence)
            || checkpoint.transition_height != Some(last.intent.transition_height)
            || checkpoint.transition_previous_catalog != Some(last.intent.previous_catalog)
            || checkpoint.transition_previous_lineage_root
                != Some(last.intent.previous_lineage_root)
            || checkpoint.transition_id != Some(last.intent.transition_id)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry pending archive GC does not terminate at its checkpoint",
            ));
        }
        Ok(())
    }

    fn validate_geometry_binding_from_journal(&self, binding: &LaneGeometryBinding) -> Result<()> {
        if binding.incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal contains a zero incarnation",
            ));
        }
        self.resolve_relative_path(&binding.blocks_path)?;
        self.resolve_relative_path(&binding.merge_path)?;
        Ok(())
    }

    fn validate_geometry_binding_set(&self, bindings: &[LaneGeometryBinding]) -> Result<()> {
        if bindings.is_empty()
            || bindings.len() > MAX_GEOMETRY_BINDINGS
            || bindings
                .windows(2)
                .any(|pair| pair[0].lane_id >= pair[1].lane_id)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry catalog bindings are empty, duplicated, or unsorted",
            ));
        }
        let mut incarnations = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for binding in bindings {
            self.validate_geometry_binding_from_journal(binding)?;
            if !incarnations.insert(binding.incarnation)
                || !paths.insert(binding.blocks_path.clone())
                || !paths.insert(binding.merge_path.clone())
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry catalog contains duplicate incarnations or storage paths",
                ));
            }
        }
        Ok(())
    }

    fn write_lane_geometry_journal(&self, journal: &LaneGeometryJournal) -> Result<()> {
        self.validate_lane_geometry_journal(journal)?;
        let path = self.lane_geometry_journal_path();
        let temp = self.store_root.join(JOURNAL_TEMP_FILE_NAME);
        self.atomic_write_geometry_file(&path, &temp, &journal.encode())
    }

    fn atomic_write_geometry_file(&self, path: &Path, temp: &Path, bytes: &[u8]) -> Result<()> {
        if path.parent() != temp.parent() || path == temp {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "geometry sidecar temp must be a distinct sibling of its target",
                ),
                temp.to_path_buf(),
            ));
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_GEOMETRY_JOURNAL_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "geometry sidecar exceeds the encoded byte limit",
                ),
                path.to_path_buf(),
            ));
        }
        self.validate_geometry_ancestors(path)?;
        self.validate_geometry_ancestors(temp)?;
        match fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() =>
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "geometry sidecar target has an unsafe file type",
                    ),
                    path.to_path_buf(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(Error::IO(error, path.to_path_buf())),
        }
        if let Some(parent) = path.parent() {
            create_dir_all_with_context(parent)?;
            self.validate_path_kind(parent, true)?;
            self.sync_geometry_parent(Some(parent))?;
        }
        let file = match fs::symlink_metadata(temp) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::AlreadyExists,
                            "geometry sidecar temp collision has an unsafe file type",
                        ),
                        temp.to_path_buf(),
                    ));
                }
                let mut stale = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(temp)
                    .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
                self.verify_open_geometry_file(temp, &stale)?;
                let intended_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                if metadata.len() != intended_len {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::AlreadyExists,
                            "geometry sidecar temp collision differs from the intended write",
                        ),
                        temp.to_path_buf(),
                    ));
                }
                let mut stale_bytes = Vec::with_capacity(bytes.len());
                (&mut stale)
                    .take(intended_len.saturating_add(1))
                    .read_to_end(&mut stale_bytes)
                    .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
                if stale_bytes != bytes {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::AlreadyExists,
                            "geometry sidecar temp collision differs from the intended write",
                        ),
                        temp.to_path_buf(),
                    ));
                }
                stale
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let mut created = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(temp)
                    .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
                self.verify_open_geometry_file(temp, &created)?;
                created
                    .write_all(bytes)
                    .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
                created
            }
            Err(error) => return Err(Error::IO(error, temp.to_path_buf())),
        };
        // Geometry intents and incarnation markers are correctness barriers even when ordinary
        // block fsync is disabled. Always synchronize the file before its directory entry.
        file.sync_all()
            .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        self.verify_open_geometry_file(temp, &file)?;
        fs::rename(temp, path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.verify_open_geometry_file(path, &file)?;
        file.sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.sync_geometry_parent(path.parent())
    }

    fn sync_geometry_parent(&self, parent: Option<&Path>) -> Result<()> {
        let Some(mut directory) = parent else {
            return Ok(());
        };
        loop {
            if !directory.starts_with(&self.store_root) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidInput,
                    "geometry durability path escapes the Kura store root",
                ));
            }
            self.geometry_path_identity(directory, true)?;
            sync_dir(directory).map_err(|error| Error::IO(error, directory.to_path_buf()))?;
            if directory == self.store_root {
                break;
            }
            directory = directory.parent().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidInput,
                    "geometry durability path has no store-root ancestor",
                )
            })?;
        }
        Ok(())
    }

    pub(crate) fn lane_geometry_journal_path(&self) -> PathBuf {
        self.store_root.join(JOURNAL_FILE_NAME)
    }

    fn geometry_error(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error::IO(
            std::io::Error::new(kind, message),
            self.lane_geometry_journal_path(),
        )
    }

    fn geometry_error_owned(&self, kind: ErrorKind, message: String) -> Error {
        Error::IO(
            std::io::Error::new(kind, message),
            self.lane_geometry_journal_path(),
        )
    }

    fn ensure_nonzero_lineage_root(&self, lineage_root: Hash) -> Result<()> {
        if lineage_root_is_zero(lineage_root) {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry lineage root must not be all zero",
            ));
        }
        Ok(())
    }
}

fn lane_geometry_journal_structure_error(
    store_root: &Path,
    kind: ErrorKind,
    message: &'static str,
) -> Error {
    Error::IO(
        std::io::Error::new(kind, message),
        store_root.join(JOURNAL_FILE_NAME),
    )
}

fn validate_lane_geometry_phase_frontier(
    store_root: &Path,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    let mut saw_uncertain_boundary = false;
    let mut saw_rolled_back = false;
    for record in &journal.records {
        match record.phase {
            LaneGeometryPhase::CatalogPublished => {
                if saw_uncertain_boundary || saw_rolled_back {
                    return Err(lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "lane geometry journal phases do not form a durable applied frontier",
                    ));
                }
            }
            LaneGeometryPhase::Intent | LaneGeometryPhase::FilesApplied => {
                if saw_uncertain_boundary || saw_rolled_back {
                    return Err(lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "lane geometry journal has more than one uncertain transition boundary",
                    ));
                }
                saw_uncertain_boundary = true;
            }
            LaneGeometryPhase::RolledBack => {
                saw_rolled_back = true;
            }
        }
    }
    Ok(())
}

fn validate_lane_geometry_journal_structure(
    store_root: &Path,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    if journal.version != JOURNAL_VERSION
        || journal.records.len() > MAX_GEOMETRY_TRANSITIONS
        || journal.pending_archive_gc.len() > MAX_GEOMETRY_TRANSITIONS
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal has an unsupported version or too many transitions",
        ));
    }
    if journal.configured_primary_binding.is_some() && journal.configured_catalog_hash.is_none() {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "configured primary geometry binding has no configured-catalog baseline",
        ));
    }
    if let Some(primary) = journal.configured_primary_binding.as_ref() {
        if primary.lane_id != LaneId::SINGLE || primary.activation_height != 0 {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "configured primary geometry binding is not lane zero at activation zero",
            ));
        }
        validate_geometry_binding_structure(store_root, primary)?;
    }
    if let Some(checkpoint) = journal.checkpoint.as_ref() {
        validate_lane_geometry_checkpoint_structure(store_root, checkpoint)?;
        if journal.records.first().is_some_and(|record| {
            record.previous_catalog != checkpoint.catalog
                || record.previous_lineage_root != checkpoint.lineage_root
        }) {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal retained history does not start at its checkpoint catalog",
            ));
        }
        if let (Some(checkpoint), Some(first)) =
            (journal.checkpoint.as_ref(), journal.records.first())
            && (checkpoint
                .transition_sequence
                .is_some_and(|sequence| first.transition_sequence <= sequence)
                || first.transition_height <= checkpoint.snapshot_height)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "retained lane geometry history does not advance beyond its checkpoint cursor",
            ));
        }
    } else if !journal.pending_archive_gc.is_empty() {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal has pending archive GC without a durable checkpoint",
        ));
    }
    validate_pending_lane_geometry_gc_structure(store_root, journal)?;
    validate_lane_geometry_phase_frontier(store_root, journal)?;

    let mut transition_ids = BTreeSet::new();
    let mut retained_paths = BTreeSet::new();
    if journal.records.windows(2).any(|pair| {
        pair[0].transition_sequence >= pair[1].transition_sequence
            || pair[0].transition_height > pair[1].transition_height
    }) {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal transition cursor is not monotonic",
        ));
    }
    for (record_index, record) in journal.records.iter().enumerate() {
        if record.transition_id
            != geometry_transition_id(
                record.transition_sequence,
                record.transition_height,
                record.previous_catalog,
                record.previous_lineage_root,
                record.updated_catalog,
                record.updated_lineage_root,
            )
            || record.previous_catalog == record.updated_catalog
                && record.previous_lineage_root == record.updated_lineage_root
            || lineage_root_is_zero(record.previous_lineage_root)
            || lineage_root_is_zero(record.updated_lineage_root)
            || !transition_ids.insert(record.transition_id)
            || record.operations.len() > MAX_GEOMETRY_BINDINGS.saturating_mul(2)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal contains an invalid or duplicate transition",
            ));
        }
        for bindings in [&record.previous_bindings, &record.updated_bindings] {
            validate_geometry_binding_set_structure(store_root, bindings)?;
        }
        if geometry_catalog_fingerprint(&record.previous_bindings) != record.previous_catalog
            || geometry_catalog_fingerprint(&record.updated_bindings) != record.updated_catalog
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal catalog fingerprint does not match its bindings",
            ));
        }
        if record_index > 0
            && (journal.records[record_index - 1].updated_catalog != record.previous_catalog
                || journal.records[record_index - 1].updated_lineage_root
                    != record.previous_lineage_root)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal transition chain is not contiguous",
            ));
        }

        let transition_hex = hex::encode(record.transition_id.as_ref());
        let previous_by_lane = record
            .previous_bindings
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        let updated_by_lane = record
            .updated_bindings
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        if record
            .operations
            .windows(2)
            .any(|pair| pair[0].lane_id >= pair[1].lane_id)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal operations are duplicated or unsorted",
            ));
        }

        let mut lane_ids = BTreeSet::new();
        for operation in &record.operations {
            if !lane_ids.insert(operation.lane_id)
                || operation
                    .previous
                    .as_ref()
                    .is_some_and(|binding| binding.lane_id != operation.lane_id)
                || operation
                    .updated
                    .as_ref()
                    .is_some_and(|binding| binding.lane_id != operation.lane_id)
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry journal contains duplicate or mismatched lane operations",
                ));
            }
            let expected_root = format!(
                "retired/lane_geometry/{transition_hex}/lane_{:010}",
                operation.lane_id.as_u32()
            );
            let expected_paths = [
                format!("{expected_root}/previous_blocks"),
                format!("{expected_root}/previous_merge.log"),
                format!("{expected_root}/unpublished_blocks"),
                format!("{expected_root}/unpublished_merge.log"),
            ];
            let actual_paths = [
                &operation.archived_blocks_path,
                &operation.archived_merge_path,
                &operation.unpublished_blocks_path,
                &operation.unpublished_merge_path,
            ];
            if actual_paths
                .iter()
                .zip(expected_paths.iter())
                .any(|(actual, expected)| *actual != expected)
                || actual_paths
                    .iter()
                    .any(|path| !retained_paths.insert((*path).clone()))
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry journal contains forged or colliding archive paths",
                ));
            }
            for (path, directory) in actual_paths.iter().zip([true, false, true, false]) {
                validate_geometry_journal_relative_path(store_root, path, directory)?;
            }
            for binding in operation.previous.iter().chain(operation.updated.iter()) {
                validate_geometry_binding_structure(store_root, binding)?;
            }
            if operation.previous.as_ref() != previous_by_lane.get(&operation.lane_id).copied()
                || operation.updated.as_ref() != updated_by_lane.get(&operation.lane_id).copied()
            {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry operation does not match its authenticated catalog bindings",
                ));
            }
            let shape_is_valid = match operation.kind {
                LaneGeometryOperationKind::Create => {
                    operation.previous.is_none() && operation.updated.is_some()
                }
                LaneGeometryOperationKind::Retire => {
                    operation.previous.is_some() && operation.updated.is_none()
                }
                LaneGeometryOperationKind::Replace => operation
                    .previous
                    .as_ref()
                    .zip(operation.updated.as_ref())
                    .is_some_and(|(previous, updated)| {
                        previous.incarnation != updated.incarnation
                            || previous.activation_height != updated.activation_height
                    }),
                LaneGeometryOperationKind::Relabel => operation
                    .previous
                    .as_ref()
                    .zip(operation.updated.as_ref())
                    .is_some_and(|(previous, updated)| {
                        previous.incarnation == updated.incarnation
                            && previous.activation_height == updated.activation_height
                            && (previous.blocks_path != updated.blocks_path
                                || previous.merge_path != updated.merge_path)
                    }),
            };
            if !shape_is_valid {
                return Err(lane_geometry_journal_structure_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "lane geometry journal contains an invalid operation shape",
                ));
            }
        }

        let expected_changed_lanes = previous_by_lane
            .keys()
            .chain(updated_by_lane.keys())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|lane_id| {
                previous_by_lane.get(lane_id).copied() != updated_by_lane.get(lane_id).copied()
            })
            .count();
        if record.operations.len() != expected_changed_lanes {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal omits or invents a catalog binding operation",
            ));
        }
    }
    Ok(())
}

fn validate_lane_geometry_checkpoint_structure(
    store_root: &Path,
    checkpoint: &LaneGeometrySnapshotCheckpoint,
) -> Result<()> {
    validate_geometry_binding_set_structure(store_root, &checkpoint.bindings)?;
    validate_geometry_merge_release_structure(
        store_root,
        &checkpoint.merge_releases,
        checkpoint.snapshot_height,
    )?;
    if checkpoint.version != CHECKPOINT_VERSION
        || checkpoint
            .snapshot_state_hash
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
        || checkpoint.catalog != geometry_catalog_fingerprint(&checkpoint.bindings)
        || lineage_root_is_zero(checkpoint.lineage_root)
        || checkpoint.commitment != geometry_checkpoint_commitment(checkpoint)
        || checkpoint.snapshot_height == 0
        || checkpoint.snapshot_block_hash.is_none()
        || checkpoint
            .snapshot_block_hash
            .is_some_and(|hash| hash.as_ref().iter().all(|byte| *byte == 0))
        || checkpoint
            .bindings
            .iter()
            .any(|binding| binding.activation_height > checkpoint.snapshot_height)
        || checkpoint
            .transition_height
            .is_some_and(|height| height > checkpoint.snapshot_height)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint commitment, catalog, height, block hash, or activation is invalid",
        ));
    }
    match (
        checkpoint.transition_sequence,
        checkpoint.transition_height,
        checkpoint.transition_previous_catalog,
        checkpoint.transition_previous_lineage_root,
        checkpoint.transition_id,
    ) {
        (None, None, None, None, None) => Ok(()),
        (
            Some(sequence),
            Some(height),
            Some(previous_catalog),
            Some(previous_lineage_root),
            Some(transition_id),
        ) if !lineage_root_is_zero(previous_lineage_root)
            && transition_id
                == geometry_transition_id(
                    sequence,
                    height,
                    previous_catalog,
                    previous_lineage_root,
                    checkpoint.catalog,
                    checkpoint.lineage_root,
                ) =>
        {
            Ok(())
        }
        _ => Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint transition binding is invalid",
        )),
    }
}

fn validate_geometry_merge_release_structure(
    store_root: &Path,
    releases: &[LaneGeometryMergeRelease],
    snapshot_height: u64,
) -> Result<()> {
    if releases.len() > MAX_GEOMETRY_MERGE_RELEASES
        || releases.windows(2).any(|pair| pair[0] >= pair[1])
        || releases.iter().any(|release| {
            release.lane_block_height == 0
                || release.application_block_height == 0
                || release.application_block_height > snapshot_height
                || release
                    .lane_incarnation
                    .as_ref()
                    .iter()
                    .all(|byte| *byte == 0)
        })
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "geometry checkpoint merge releases are invalid, duplicated, unsorted, or oversized",
        ));
    }
    Ok(())
}

fn validate_pending_lane_geometry_gc_structure(
    store_root: &Path,
    journal: &LaneGeometryJournal,
) -> Result<()> {
    if journal.pending_archive_gc.is_empty() {
        if journal
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.pending_archive_gc_root.is_some())
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry checkpoint commits a missing pending archive GC set",
            ));
        }
        return Ok(());
    }
    let checkpoint = journal.checkpoint.as_ref().ok_or_else(|| {
        lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "pending lane geometry GC has no checkpoint",
        )
    })?;
    let retained_ids = journal
        .records
        .iter()
        .map(|record| record.transition_id)
        .collect::<BTreeSet<_>>();
    let mut pending_ids = BTreeSet::new();
    for (index, pending) in journal.pending_archive_gc.iter().enumerate() {
        let intent = &pending.intent;
        let standalone = LaneGeometryJournal {
            version: JOURNAL_VERSION,
            configured_catalog_hash: None,
            configured_primary_binding: None,
            checkpoint: None,
            pending_archive_gc: Vec::new(),
            records: vec![intent.clone()],
        };
        validate_lane_geometry_journal_structure(store_root, &standalone)?;
        if intent.phase != LaneGeometryPhase::CatalogPublished
            || !pending_ids.insert(intent.transition_id)
            || retained_ids.contains(&intent.transition_id)
            || index > 0
                && (journal.pending_archive_gc[index - 1].intent.updated_catalog
                    != intent.previous_catalog
                    || journal.pending_archive_gc[index - 1]
                        .intent
                        .updated_lineage_root
                        != intent.previous_lineage_root
                    || journal.pending_archive_gc[index - 1]
                        .intent
                        .transition_sequence
                        >= intent.transition_sequence
                    || journal.pending_archive_gc[index - 1]
                        .intent
                        .transition_height
                        > intent.transition_height)
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal has forged or non-contiguous pending archive GC",
            ));
        }
    }
    if checkpoint.pending_archive_gc_root
        != Some(geometry_pending_archive_gc_root(
            &journal.pending_archive_gc,
        ))
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint does not bind its exact pending archive GC set",
        ));
    }
    let last = journal
        .pending_archive_gc
        .last()
        .expect("non-empty pending archive GC");
    if last.intent.updated_catalog != checkpoint.catalog
        || last.intent.updated_lineage_root != checkpoint.lineage_root
        || checkpoint.transition_sequence != Some(last.intent.transition_sequence)
        || checkpoint.transition_height != Some(last.intent.transition_height)
        || checkpoint.transition_previous_catalog != Some(last.intent.previous_catalog)
        || checkpoint.transition_previous_lineage_root != Some(last.intent.previous_lineage_root)
        || checkpoint.transition_id != Some(last.intent.transition_id)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry pending archive GC does not terminate at its checkpoint",
        ));
    }
    Ok(())
}

fn validate_geometry_binding_structure(
    store_root: &Path,
    binding: &LaneGeometryBinding,
) -> Result<()> {
    if binding.incarnation.as_ref().iter().all(|byte| *byte == 0) {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal contains a zero incarnation",
        ));
    }
    validate_geometry_journal_relative_path(store_root, &binding.blocks_path, true)?;
    validate_geometry_journal_relative_path(store_root, &binding.merge_path, false)
}

fn validate_geometry_binding_set_structure(
    store_root: &Path,
    bindings: &[LaneGeometryBinding],
) -> Result<()> {
    if bindings.is_empty()
        || bindings.len() > MAX_GEOMETRY_BINDINGS
        || bindings
            .windows(2)
            .any(|pair| pair[0].lane_id >= pair[1].lane_id)
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry catalog bindings are empty, duplicated, or unsorted",
        ));
    }
    let mut incarnations = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for binding in bindings {
        validate_geometry_binding_structure(store_root, binding)?;
        if !incarnations.insert(binding.incarnation)
            || !paths.insert(binding.blocks_path.clone())
            || !paths.insert(binding.merge_path.clone())
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry catalog contains duplicate incarnations or storage paths",
            ));
        }
    }
    Ok(())
}

fn validate_geometry_journal_relative_path(
    store_root: &Path,
    relative: &str,
    directory: bool,
) -> Result<()> {
    let relative = Path::new(relative);
    validate_relative_path(relative)?;
    let root_metadata = fs::symlink_metadata(store_root)
        .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "Kura geometry store root must remain a non-symlink directory",
        ));
    }

    let components = relative.components().collect::<Vec<_>>();
    let mut cursor = store_root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        cursor.push(component.as_os_str());
        let is_target = index + 1 == components.len();
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry journal path traverses or targets a symlink",
                    ),
                    cursor,
                ));
            }
            Ok(metadata) if !is_target && !metadata.file_type().is_dir() => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry journal path traverses a non-directory",
                    ),
                    cursor,
                ));
            }
            Ok(metadata)
                if is_target
                    && ((directory && !metadata.file_type().is_dir())
                        || (!directory && !metadata.file_type().is_file())) =>
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry journal path target has the wrong file type",
                    ),
                    cursor,
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(Error::IO(error, cursor)),
        }
    }
    Ok(())
}

fn lane_payload_targets_retirement(
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    retiring: &BTreeSet<LaneRetirementIdentity>,
) -> bool {
    let descriptor = &payload.origin_proposal.descriptor;
    if retiring.contains(&LaneRetirementIdentity {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
    }) {
        return true;
    }
    payload
        .routing_plans
        .iter()
        .zip(&payload.native_amx_receipts)
        .any(|(plan, receipt)| {
            let (crate::queue::RoutingPlan::NativeAmx(plan), Some(receipt)) = (plan, receipt)
            else {
                return false;
            };
            plan.participants
                .iter()
                .zip(&receipt.legs)
                .any(|(planned, leg)| {
                    planned.route.lane_id == leg.lane_id
                        && planned.route.dataspace_id == leg.dataspace_id
                        && retiring.iter().any(|identity| {
                            identity.lane_id == leg.lane_id
                                && identity.dataspace_id == leg.dataspace_id
                        })
                })
        })
}

fn lane_proposal_coordinator_targets_retirement(
    proposal: &LaneBlockProposalV1,
    retiring: &BTreeSet<LaneRetirementIdentity>,
) -> bool {
    let descriptor = &proposal.descriptor;
    retiring.contains(&LaneRetirementIdentity {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
    })
}

fn routing_plan_from_execution_context(
    context: &ExternalExecutionContext,
) -> Option<crate::queue::RoutingPlan> {
    let coordinator = context.routing_plan_legs.first()?;
    if coordinator.role != ExternalExecutionRouteRole::Coordinator
        || coordinator.lane_id != context.lane_id
        || coordinator.dataspace_id != context.dataspace_id
    {
        return None;
    }
    let coordinator =
        crate::queue::RoutingDecision::new(coordinator.lane_id, coordinator.dataspace_id);
    let plan = if context.routing_plan_legs.len() == 1 {
        crate::queue::RoutingPlan::single(coordinator)
    } else {
        let participants = context
            .routing_plan_legs
            .iter()
            .skip(1)
            .map(|leg| {
                (leg.role == ExternalExecutionRouteRole::Participant).then_some(
                    crate::queue::RouteLeg::new(
                        crate::queue::RoutingDecision::new(leg.lane_id, leg.dataspace_id),
                        crate::queue::RouteLegRole::Participant,
                    ),
                )
            })
            .collect::<Option<Vec<_>>>()?;
        crate::queue::RoutingPlan::native_amx(coordinator, participants)
    };
    (plan.digest() == context.routing_plan_digest
        && crate::queue::execution_context_legs_for_routing_plan(&plan)
            == context.routing_plan_legs)
        .then_some(plan)
}

fn geometry_catalog_fingerprint(bindings: &[LaneGeometryBinding]) -> Hash {
    let encoded = bindings.to_vec().encode();
    Hash::new_from_chunks(&[CATALOG_DOMAIN, encoded.as_slice()])
}

#[cfg(test)]
fn unscoped_lineage_root(bindings: &[LaneGeometryBinding]) -> Hash {
    let catalog = geometry_catalog_fingerprint(bindings);
    Hash::new_from_chunks(&[UNSCOPED_LINEAGE_DOMAIN, catalog.as_ref()])
}

fn lineage_root_is_zero(root: Hash) -> bool {
    root.as_ref().iter().all(|byte| *byte == 0)
}

fn geometry_transition_id(
    transition_sequence: u64,
    transition_height: u64,
    previous_catalog: Hash,
    previous_lineage_root: Hash,
    updated_catalog: Hash,
    updated_lineage_root: Hash,
) -> Hash {
    Hash::new_from_chunks(&[
        TRANSITION_DOMAIN,
        &transition_sequence.to_le_bytes(),
        &transition_height.to_le_bytes(),
        previous_catalog.as_ref(),
        previous_lineage_root.as_ref(),
        updated_catalog.as_ref(),
        updated_lineage_root.as_ref(),
    ])
}

fn geometry_checkpoint_commitment(checkpoint: &LaneGeometrySnapshotCheckpoint) -> Hash {
    let mut payload = Vec::new();
    payload.push(checkpoint.version);
    payload.extend_from_slice(&checkpoint.snapshot_height.to_le_bytes());
    match checkpoint.snapshot_block_hash {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(checkpoint.snapshot_state_hash.as_ref());
    payload.extend_from_slice(checkpoint.catalog.as_ref());
    payload.extend_from_slice(checkpoint.lineage_root.as_ref());
    match checkpoint.transition_sequence {
        Some(sequence) => {
            payload.push(1);
            payload.extend_from_slice(&sequence.to_le_bytes());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_height {
        Some(height) => {
            payload.push(1);
            payload.extend_from_slice(&height.to_le_bytes());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_previous_catalog {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_previous_lineage_root {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_id {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(&checkpoint.bindings.clone().encode());
    payload.extend_from_slice(&checkpoint.merge_releases.clone().encode());
    match checkpoint.pending_archive_gc_root {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    Hash::new_from_chunks(&[CHECKPOINT_DOMAIN, payload.as_slice()])
}

fn geometry_pending_archive_gc_root(pending: &[LaneGeometryPendingArchiveGc]) -> Hash {
    Hash::new_from_chunks(&[PENDING_GC_DOMAIN, pending.to_vec().encode().as_slice()])
}

fn geometry_merge_marker_set_root(markers: &[(Name, Vec<u8>)]) -> Hash {
    Hash::new_from_chunks(&[
        MERGE_RELEASE_MARKERS_DOMAIN,
        markers.to_vec().encode().as_slice(),
    ])
}

fn lane_geometry_snapshot_checkpoint(
    snapshot_height: u64,
    snapshot_block_hash: Option<HashOf<BlockHeader>>,
    snapshot_state_hash: Hash,
    bindings: Vec<LaneGeometryBinding>,
    lineage_root: Hash,
    transition_sequence: Option<u64>,
    transition_height: Option<u64>,
    transition_previous_catalog: Option<Hash>,
    transition_previous_lineage_root: Option<Hash>,
    transition_id: Option<Hash>,
    merge_releases: Vec<LaneGeometryMergeRelease>,
    pending_archive_gc_root: Option<Hash>,
) -> LaneGeometrySnapshotCheckpoint {
    let mut checkpoint = LaneGeometrySnapshotCheckpoint {
        version: CHECKPOINT_VERSION,
        snapshot_height,
        snapshot_block_hash,
        snapshot_state_hash,
        catalog: geometry_catalog_fingerprint(&bindings),
        lineage_root,
        transition_sequence,
        transition_height,
        transition_previous_catalog,
        transition_previous_lineage_root,
        transition_id,
        bindings,
        merge_releases,
        pending_archive_gc_root,
        commitment: Hash::prehashed([0; Hash::LENGTH]),
    };
    checkpoint.commitment = geometry_checkpoint_commitment(&checkpoint);
    checkpoint
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "lane geometry journal contains an unsafe relative path",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(())
}

fn geometry_file_identity(metadata: &fs::Metadata) -> GeometryFileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        GeometryFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(windows)]
    {
        use std::{os::windows::fs::MetadataExt, sync::atomic::Ordering};

        let volume_serial_number = metadata.volume_serial_number();
        let file_index = metadata.file_index();
        let unsupported_nonce = if volume_serial_number.is_some() && file_index.is_some() {
            0
        } else {
            // Some Windows filesystems do not expose stable volume/file IDs. A fresh nonce makes
            // every subsequent comparison fail closed instead of treating all paths as equal.
            UNSUPPORTED_GEOMETRY_IDENTITY_NONCE.fetch_add(1, Ordering::Relaxed)
        };
        GeometryFileIdentity {
            volume_serial_number,
            file_index,
            unsupported_nonce,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        use std::sync::atomic::Ordering;

        let _ = metadata;
        GeometryFileIdentity {
            unsupported_nonce: UNSUPPORTED_GEOMETRY_IDENTITY_NONCE.fetch_add(1, Ordering::Relaxed),
        }
    }
}

fn decode_exact<T: Decode>(bytes: &[u8]) -> std::result::Result<T, norito::core::Error> {
    let mut input = bytes;
    let value = T::decode(&mut input)?;
    if !input.is_empty() {
        return Err(norito::core::Error::Message(
            "trailing bytes in lane geometry sidecar".to_owned(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap, fs, num::NonZeroU32, sync::Arc};

    use iroha_config::{
        base::WithOrigin,
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
            defaults::kura::{
                BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, EVICTION_REQUIRED_REPLICAS,
                FSYNC_INTERVAL, MAX_DISK_USAGE_BYTES, MERGE_LEDGER_CACHE_CAPACITY,
                ROSTER_SIDECAR_RETENTION,
            },
        },
    };
    use iroha_crypto::{Algorithm, KeyPair, Signature, bls_normal_pop_prove};
    use iroha_data_model::{
        ChainId, Level,
        block::{
            BlockExecutionContextBundle, SignedBlock,
            consensus::{
                CertPhase, LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1,
                LaneBlockProposalV1, NativeAmxAttestationBodyV2, NativeAmxAttestationQcV2,
                NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
                SumeragiLanePayloadOwnership,
            },
            consensus_v2::{ConsensusRound, HeightContext, HeightContextId},
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        isi::Log,
        nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneLifecycleParameterV1},
        peer::PeerId,
        transaction::{TransactionBuilder, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    use nonzero_ext::nonzero;
    use tempfile::TempDir;

    use super::*;
    use crate::kura::CertifiedLaneBlockArtifact;
    use crate::{
        block::BlockBuilder,
        lane_consensus::{
            CommittedLaneBlockSession, LaneBlockVoteV1, aggregate_lane_block_votes_to_qc,
        },
        tx::AcceptedTransaction,
    };

    // Keep the authenticated archive payload comfortably larger than the checkpoint sidecar.
    // This makes the net disk-reclamation assertion independent of small encoding-size changes.
    const GC_PAYLOAD_LEN: usize = 16 * 1024;

    fn open_kura(root: &Path, lane_config: &RuntimeLaneConfig) -> Arc<Kura> {
        let config = kura_config(root);
        Kura::new(&config, lane_config).expect("open test Kura").0
    }

    fn kura_config(root: &Path) -> KuraConfig {
        KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(root.to_path_buf()),
            max_disk_usage_bytes: MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::On,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            eviction_required_replicas: EVICTION_REQUIRED_REPLICAS,
        }
    }

    fn configured_primary_catalog(alias: &str) -> LaneCatalog {
        LaneCatalog::new(
            nonzero!(1_u32),
            vec![ModelLaneConfig {
                alias: alias.to_owned(),
                ..ModelLaneConfig::default()
            }],
        )
        .expect("configured primary-lane catalog")
    }

    fn assert_lane_paths_absent(root: &Path, lane_config: &RuntimeLaneConfig) {
        let primary = lane_config.primary();
        assert!(
            !primary.blocks_dir(root).exists(),
            "rejected startup must not create its block-store path"
        );
        assert!(
            !primary.merge_log_path(root).exists(),
            "rejected startup must not create its merge-ledger path"
        );
    }

    fn assert_kura_io_error(error: &Error, kind: std::io::ErrorKind, message: &str) {
        let Error::IO(source, _) = error else {
            panic!("expected Kura IO error containing {message:?}, got {error:?}");
        };
        assert_eq!(source.kind(), kind, "unexpected Kura IO error: {error:?}");
        assert!(
            source.to_string().contains(message),
            "Kura IO source did not contain {message:?}: {error:?}"
        );
    }

    fn initial_and_extended_configs() -> (RuntimeLaneConfig, RuntimeLaneConfig) {
        let lane0 = ModelLaneConfig::default();
        let lane1 = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "elastic-one".to_owned(),
            ..ModelLaneConfig::default()
        };
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let initial = LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("initial catalog");
        let extended = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("extended catalog");
        (
            RuntimeLaneConfig::from_catalog(&initial),
            RuntimeLaneConfig::from_catalog(&extended),
        )
    }

    fn initial_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
        (
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x11; Hash::LENGTH]))]),
            BTreeMap::from([(LaneId::SINGLE, 0)]),
        )
    }

    fn extended_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
        (
            BTreeMap::from([
                (LaneId::SINGLE, Hash::prehashed([0x11; Hash::LENGTH])),
                (LaneId::new(1), Hash::prehashed([0x22; Hash::LENGTH])),
            ]),
            BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 9)]),
        )
    }

    fn persist_create_intent(
        kura: &Kura,
        previous: &RuntimeLaneConfig,
        updated: &RuntimeLaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activations: &BTreeMap<LaneId, u64>,
        updated_activations: &BTreeMap<LaneId, u64>,
    ) -> LaneGeometryOperation {
        let previous_bindings = kura
            .geometry_bindings(previous, previous_incarnations, previous_activations)
            .expect("previous geometry bindings");
        let updated_bindings = kura
            .geometry_bindings(updated, updated_incarnations, updated_activations)
            .expect("updated geometry bindings");
        let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
        let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
        let previous_lineage_root = unscoped_lineage_root(&previous_bindings);
        let updated_lineage_root = unscoped_lineage_root(&updated_bindings);
        let transition_id = geometry_transition_id(
            0,
            0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        );
        let operations = kura
            .build_geometry_operations(
                transition_id,
                &previous_bindings,
                &updated_bindings,
                &BTreeSet::new(),
            )
            .expect("create operation");
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].kind, LaneGeometryOperationKind::Create);
        let operation = operations[0].clone();
        let mut journal = LaneGeometryJournal::default();
        journal.records.push(LaneGeometryIntent {
            transition_id,
            transition_sequence: 0,
            transition_height: 0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
            previous_bindings,
            updated_bindings,
            phase: LaneGeometryPhase::Intent,
            operations,
        });
        kura.write_lane_geometry_journal(&journal)
            .expect("persist create intent");
        operation
    }

    #[test]
    fn before_first_height_cursor_replays_same_height_transitions_in_sequence() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = nonzero!(2_u32);
        let primary = ModelLaneConfig::default();
        let second = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "same-height-a".to_owned(),
            ..ModelLaneConfig::default()
        };
        let relabelled = ModelLaneConfig {
            alias: "same-height-b".to_owned(),
            ..second.clone()
        };
        let initial_catalog =
            LaneCatalog::new(lane_count, vec![primary.clone()]).expect("initial catalog");
        let added_catalog =
            LaneCatalog::new(lane_count, vec![primary.clone(), second]).expect("added catalog");
        let relabelled_catalog =
            LaneCatalog::new(lane_count, vec![primary, relabelled]).expect("relabelled catalog");
        let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
        let added = RuntimeLaneConfig::from_catalog(&added_catalog);
        let relabelled = RuntimeLaneConfig::from_catalog(&relabelled_catalog);
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x51; Hash::LENGTH]))]);
        let added_incarnations = BTreeMap::from([
            (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x52; Hash::LENGTH])),
        ]);
        let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let added_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 7)]);
        let kura = open_kura(&root, &initial);

        kura.apply_lane_geometry_transition_at_height(
            &initial,
            &added,
            &initial_incarnations,
            &added_incarnations,
            &initial_activations,
            &added_activations,
            &BTreeSet::new(),
            7,
        )
        .expect("apply first height-seven transition");
        kura.mark_lane_geometry_catalog_published(
            &added,
            &added_incarnations,
            &added_activations,
            None,
        )
        .expect("publish first height-seven transition");
        kura.apply_lane_geometry_transition_at_height(
            &added,
            &relabelled,
            &added_incarnations,
            &added_incarnations,
            &added_activations,
            &added_activations,
            &BTreeSet::new(),
            7,
        )
        .expect("apply second height-seven transition");
        kura.mark_lane_geometry_catalog_published(
            &relabelled,
            &added_incarnations,
            &added_activations,
            None,
        )
        .expect("publish second height-seven transition");
        let original = kura
            .read_lane_geometry_journal()
            .expect("published journal");
        let cursors = original
            .records
            .iter()
            .map(|record| (record.transition_id, record.transition_sequence))
            .collect::<Vec<_>>();
        assert_eq!(original.records.len(), 2);

        kura.recover_lane_geometry_journal_before_first_transition_at_height(
            &initial,
            &initial_incarnations,
            &initial_activations,
            7,
        )
        .expect("restore cursor before every transition at height seven");
        assert!(
            kura.read_lane_geometry_journal()
                .expect("rolled-back journal")
                .records
                .iter()
                .all(|record| record.phase == LaneGeometryPhase::RolledBack)
        );

        kura.apply_lane_geometry_transition_at_height(
            &initial,
            &added,
            &initial_incarnations,
            &added_incarnations,
            &initial_activations,
            &added_activations,
            &BTreeSet::new(),
            7,
        )
        .expect("retry first transition in sequence");
        kura.mark_lane_geometry_catalog_published(
            &added,
            &added_incarnations,
            &added_activations,
            None,
        )
        .expect("republish first transition");
        kura.apply_lane_geometry_transition_at_height(
            &added,
            &relabelled,
            &added_incarnations,
            &added_incarnations,
            &added_activations,
            &added_activations,
            &BTreeSet::new(),
            7,
        )
        .expect("retry second transition in sequence");
        kura.mark_lane_geometry_catalog_published(
            &relabelled,
            &added_incarnations,
            &added_activations,
            None,
        )
        .expect("republish second transition");

        let replayed = kura.read_lane_geometry_journal().expect("replayed journal");
        assert_eq!(
            replayed
                .records
                .iter()
                .map(|record| (record.transition_id, record.transition_sequence))
                .collect::<Vec<_>>(),
            cursors
        );
        assert!(
            replayed
                .records
                .iter()
                .all(|record| record.phase == LaneGeometryPhase::CatalogPublished)
        );
    }

    fn open_configured_anchor_for_publication_test(
        root: &Path,
        lane_config: &RuntimeLaneConfig,
        baseline: Hash,
        primary_incarnation: Hash,
    ) -> Arc<Kura> {
        Kura::establish_or_verify_configured_lane_catalog_baseline(root, baseline)
            .expect("establish configured baseline before opening lane storage");
        let kura = open_kura(root, lane_config);
        kura.establish_or_verify_configured_primary_geometry_anchor(
            lane_config.primary(),
            primary_incarnation,
            baseline,
        )
        .expect("anchor configured primary before catalog publication");
        kura
    }

    #[test]
    fn post_write_publication_failure_restores_anchored_description_only_journal() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let mut lanes = LaneCatalog::default().lanes().to_vec();
        lanes[0].description = Some("operator-only catalog description".to_owned());
        let catalog =
            LaneCatalog::new(nonzero!(1_u32), lanes).expect("description-only lane catalog");
        let config = RuntimeLaneConfig::from_catalog(&catalog);
        let baseline = iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(&catalog);
        let (incarnations, activation_heights) = initial_geometry();
        let kura = open_configured_anchor_for_publication_test(
            &root,
            &config,
            baseline,
            incarnations[&LaneId::SINGLE],
        );
        let journal_path = kura.lane_geometry_journal_path();
        let prior_bytes = fs::read(&journal_path).expect("anchored journal");
        kura.apply_lane_geometry_transition(
            &config,
            &config,
            &incarnations,
            &incarnations,
            &activation_heights,
            &activation_heights,
            &BTreeSet::new(),
        )
        .expect("description-only catalog has no physical geometry transition");
        assert_eq!(
            fs::read(&journal_path).expect("unchanged journal"),
            prior_bytes
        );
        kura.fail_next_lane_geometry_publication_after_write_for_test();

        let error = kura
            .mark_lane_geometry_catalog_published(
                &config,
                &incarnations,
                &activation_heights,
                Some(baseline),
            )
            .expect_err("failure after target replacement must restore prior absence");
        assert!(
            !matches!(&error, Error::LaneGeometryPublicationRestoreFailed { .. }),
            "exact restoration should preserve the original injected publication error: {error}"
        );
        assert_eq!(
            fs::read(&journal_path).expect("restored anchored journal"),
            prior_bytes
        );
        let (restored_baseline, phases, has_temp) = kura
            .lane_geometry_journal_state_for_test()
            .expect("read restored absent journal state");
        assert_eq!(restored_baseline, Some(baseline));
        assert!(phases.is_empty());
        assert!(!has_temp, "rollback must not leave owned temp files");

        kura.mark_lane_geometry_catalog_published(
            &config,
            &incarnations,
            &activation_heights,
            Some(baseline),
        )
        .expect("one-shot failure permits an exact corrected retry");
        let (retried_baseline, phases, has_temp) = kura
            .lane_geometry_journal_state_for_test()
            .expect("read corrected publication");
        assert_eq!(retried_baseline, Some(baseline));
        assert!(phases.is_empty());
        assert!(!has_temp);
    }

    #[test]
    fn publication_temp_recovery_consumes_only_an_exact_preexisting_value() {
        let catalog = LaneCatalog::default();
        let config = RuntimeLaneConfig::from_catalog(&catalog);
        let baseline = iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(&catalog);
        let (incarnations, activation_heights) = initial_geometry();

        let unrelated_temp = TempDir::new().expect("temporary directory");
        let unrelated_root = unrelated_temp.path().join("kura");
        let unrelated_kura = open_configured_anchor_for_publication_test(
            &unrelated_root,
            &config,
            baseline,
            incarnations[&LaneId::SINGLE],
        );
        let publication_temp = unrelated_root.join(JOURNAL_TEMP_FILE_NAME);
        fs::write(&publication_temp, b"operator-owned-temp").expect("seed unrelated temp");
        let error = unrelated_kura
            .mark_lane_geometry_catalog_published(
                &config,
                &incarnations,
                &activation_heights,
                Some(baseline),
            )
            .expect_err("an unrelated preexisting temp must fail closed");
        assert!(
            !matches!(&error, Error::LaneGeometryPublicationRestoreFailed { .. }),
            "an untouched preexisting temp does not make prior-target restoration ambiguous: {error}"
        );
        assert_eq!(
            fs::read(&publication_temp).expect("unrelated temp retained"),
            b"operator-owned-temp"
        );
        assert!(
            unrelated_kura.lane_geometry_journal_path().is_file(),
            "a temp collision must retain the authenticated target"
        );

        let resumable_temp = TempDir::new().expect("temporary directory");
        let resumable_root = resumable_temp.path().join("kura");
        let resumable_kura = open_configured_anchor_for_publication_test(
            &resumable_root,
            &config,
            baseline,
            incarnations[&LaneId::SINGLE],
        );
        let expected_journal = resumable_kura
            .read_lane_geometry_journal()
            .expect("anchored resumable journal");
        let publication_temp = resumable_root.join(JOURNAL_TEMP_FILE_NAME);
        fs::write(&publication_temp, expected_journal.encode()).expect("seed exact resume temp");
        resumable_kura.fail_next_lane_geometry_publication_after_write_for_test();

        let error = resumable_kura
            .mark_lane_geometry_catalog_published(
                &config,
                &incarnations,
                &activation_heights,
                Some(baseline),
            )
            .expect_err("inject failure after consuming exact resume temp");
        assert!(!matches!(
            &error,
            Error::LaneGeometryPublicationRestoreFailed { .. }
        ));
        assert!(
            !publication_temp.exists(),
            "an exact resumable temp is consumed by target replacement"
        );
        assert!(
            fs::read(resumable_kura.lane_geometry_journal_path())
                .expect("post-write rollback restores the authenticated target")
                == expected_journal.encode(),
            "post-write rollback must restore the exact authenticated target"
        );
    }

    #[test]
    fn post_write_publication_failure_restores_exact_files_applied_journal() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let baseline = Hash::new(b"configured-catalog-baseline");
        let kura = open_configured_anchor_for_publication_test(
            &root,
            &initial,
            baseline,
            initial_incarnations[&LaneId::SINGLE],
        );
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare files-applied geometry intent");
        let journal_path = kura.lane_geometry_journal_path();
        let prior_bytes = fs::read(&journal_path).expect("capture exact files-applied journal");
        let prior_journal = decode_exact::<LaneGeometryJournal>(&prior_bytes)
            .expect("decode files-applied journal");
        assert_eq!(prior_journal.configured_catalog_hash, Some(baseline));
        assert_eq!(
            prior_journal.records.last().map(|record| record.phase),
            Some(LaneGeometryPhase::FilesApplied)
        );
        kura.fail_next_lane_geometry_publication_after_write_for_test();

        let error = kura
            .mark_lane_geometry_catalog_published(
                &extended,
                &extended_incarnations,
                &extended_activations,
                Some(baseline),
            )
            .expect_err("inject failure after replacing an existing journal");
        assert!(!matches!(
            &error,
            Error::LaneGeometryPublicationRestoreFailed { .. }
        ));
        assert_eq!(
            fs::read(&journal_path).expect("read restored journal"),
            prior_bytes,
            "rollback must restore the exact prior encoding, including FilesApplied phase"
        );
        let (restored_baseline, phases, has_temp) = kura
            .lane_geometry_journal_state_for_test()
            .expect("read exact restored journal state");
        assert_eq!(restored_baseline, Some(baseline));
        assert_eq!(phases, vec!["files_applied"]);
        assert!(!has_temp);

        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("restored FilesApplied intent remains available for State geometry rollback");
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("journal after State-equivalent rollback")
                .records
                .last()
                .map(|record| record.phase),
            Some(LaneGeometryPhase::RolledBack)
        );
    }

    #[test]
    fn publication_restore_failure_is_distinct_and_leaves_published_journal_fail_closed() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let baseline = Hash::new(b"configured-catalog-baseline");
        let kura = open_configured_anchor_for_publication_test(
            &root,
            &initial,
            baseline,
            initial_incarnations[&LaneId::SINGLE],
        );
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare files-applied geometry intent");
        let prior_bytes = fs::read(kura.lane_geometry_journal_path())
            .expect("capture exact files-applied journal");
        let restore_temp = root.join(JOURNAL_RESTORE_TEMP_FILE_NAME);
        fs::write(&restore_temp, b"operator-owned-restore-temp")
            .expect("seed restore-temp collision");
        kura.fail_next_lane_geometry_publication_after_write_for_test();

        let error = kura
            .mark_lane_geometry_catalog_published(
                &extended,
                &extended_incarnations,
                &extended_activations,
                Some(baseline),
            )
            .expect_err("restore-temp collision must prevent claiming exact restoration");
        assert!(matches!(
            &error,
            Error::LaneGeometryPublicationRestoreFailed { .. }
        ));
        assert_eq!(
            fs::read(&restore_temp).expect("restore collision retained"),
            b"operator-owned-restore-temp"
        );
        assert_ne!(
            fs::read(kura.lane_geometry_journal_path()).expect("published journal remains"),
            prior_bytes,
            "restore failure must not be reported as if the prior journal were restored"
        );
        let journal = kura
            .read_lane_geometry_journal()
            .expect("published journal remains internally valid");
        assert_eq!(journal.configured_catalog_hash, Some(baseline));
        assert_eq!(
            journal.records.last().map(|record| record.phase),
            Some(LaneGeometryPhase::CatalogPublished),
            "State must stop instead of rolling geometry back under a published journal"
        );
    }

    fn retirement_test_configs() -> (RuntimeLaneConfig, RuntimeLaneConfig) {
        let lane0 = ModelLaneConfig {
            dataspace_id: DataSpaceId::new(7),
            ..ModelLaneConfig::default()
        };
        let lane1 = ModelLaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(8),
            alias: "retirement-participant".to_owned(),
            ..ModelLaneConfig::default()
        };
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let initial =
            LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("retirement initial catalog");
        let extended =
            LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("retirement extended catalog");
        (
            RuntimeLaneConfig::from_catalog(&initial),
            RuntimeLaneConfig::from_catalog(&extended),
        )
    }

    fn retirement_test_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
        (
            BTreeMap::from([
                (LaneId::SINGLE, Hash::prehashed([0x61; Hash::LENGTH])),
                (LaneId::new(1), Hash::prehashed([0x62; Hash::LENGTH])),
            ]),
            BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 1)]),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn open_published_retirement_kura(
        root: &Path,
        initial: &RuntimeLaneConfig,
        extended: &RuntimeLaneConfig,
        initial_incarnations: &BTreeMap<LaneId, Hash>,
        extended_incarnations: &BTreeMap<LaneId, Hash>,
        initial_activations: &BTreeMap<LaneId, u64>,
        extended_activations: &BTreeMap<LaneId, u64>,
    ) -> (Arc<Kura>, Vec<u8>, usize) {
        let kura = open_kura(root, initial);
        kura.apply_lane_geometry_transition(
            initial,
            extended,
            initial_incarnations,
            extended_incarnations,
            initial_activations,
            extended_activations,
            &BTreeSet::new(),
        )
        .expect("journal dynamic retirement-test lane creation");
        kura.mark_lane_geometry_catalog_published(
            extended,
            extended_incarnations,
            extended_activations,
            None,
        )
        .expect("publish dynamic retirement-test lane catalog");
        let journal = kura
            .read_lane_geometry_journal()
            .expect("read published retirement-test journal");
        let journal_bytes = fs::read(kura.lane_geometry_journal_path())
            .expect("read exact published retirement-test journal bytes");
        (kura, journal_bytes, journal.records.len())
    }

    fn assert_geometry_io_error(error: &Error, expected_kind: ErrorKind, expected_message: &str) {
        let Error::IO(source, _) = error else {
            panic!("unexpected lane geometry error: {error:?}");
        };
        assert_eq!(source.kind(), expected_kind);
        assert_eq!(source.to_string(), expected_message);
    }

    struct RetiredGeometryFixture {
        initial: RuntimeLaneConfig,
        extended: RuntimeLaneConfig,
        initial_incarnations: BTreeMap<LaneId, Hash>,
        initial_activations: BTreeMap<LaneId, u64>,
        extended_incarnations: BTreeMap<LaneId, Hash>,
        extended_activations: BTreeMap<LaneId, u64>,
        archive_root: PathBuf,
    }

    fn prepare_retired_geometry_archive(kura: &Kura, root: &Path) -> RetiredGeometryFixture {
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("create elastic lane");
        kura.mark_lane_geometry_catalog_published(
            &extended,
            &extended_incarnations,
            &extended_activations,
            None,
        )
        .expect("publish elastic lane catalog");
        let lane_one_blocks = extended
            .entry(LaneId::new(1))
            .expect("elastic lane")
            .blocks_dir(root);
        fs::write(
            lane_one_blocks.join("gc-payload.norito"),
            [0xA5; GC_PAYLOAD_LEN],
        )
        .expect("seed archived payload bytes");

        kura.apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect("retire elastic lane");
        kura.mark_lane_geometry_catalog_published(
            &initial,
            &initial_incarnations,
            &initial_activations,
            None,
        )
        .expect("publish retired catalog");
        let journal = kura.read_lane_geometry_journal().expect("geometry journal");
        let retired = journal.records.last().expect("retire transition");
        let archive_root = root
            .join("retired/lane_geometry")
            .join(hex::encode(retired.transition_id.as_ref()));
        assert!(archive_root.exists(), "retired lane archive exists");
        RetiredGeometryFixture {
            initial,
            extended,
            initial_incarnations,
            initial_activations,
            extended_incarnations,
            extended_activations,
            archive_root,
        }
    }

    fn checkpoint_retired_geometry(
        kura: &Kura,
        fixture: &RetiredGeometryFixture,
        height: u64,
    ) -> Result<LaneGeometryGcSummary> {
        let (block_hash, state_hash) = durable_geometry_snapshot_identity(kura, height);
        let bindings = kura.geometry_bindings(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )?;
        let lineage_root = unscoped_lineage_root(&bindings);
        kura.checkpoint_lane_geometry_with_proven_snapshot(
            bindings,
            lineage_root,
            height,
            Some(block_hash),
            state_hash,
            Vec::new(),
        )
    }

    fn durable_geometry_snapshot_identity(kura: &Kura, height: u64) -> (HashOf<BlockHeader>, Hash) {
        assert!(height > 0, "geometry GC test proof must be non-genesis");
        let mut previous = NonZeroUsize::new(kura.durable_blocks_count())
            .and_then(|height| kura.get_block(height));
        while u64::try_from(kura.durable_blocks_count()).expect("block count fits u64") < height {
            let block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
                .chain(0, previous.as_deref())
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                .unpack(|_| {})
                .into();
            let block = Arc::new(block);
            kura.store_block(Arc::clone(&block))
                .expect("store durable geometry proof block");
            previous = Some(block);
        }
        let height_usize = NonZeroUsize::new(usize::try_from(height).expect("height fits usize"))
            .expect("non-zero height");
        let block_hash = kura
            .get_durable_block_hash(height_usize)
            .expect("durable geometry proof block hash");
        let state_hash = Hash::new([0xC0, u8::try_from(height).unwrap_or(u8::MAX)]);
        kura.store_wsv_checkpoint(height, block_hash, state_hash)
            .expect("store durable geometry proof WSV checkpoint");
        (block_hash, state_hash)
    }

    fn certified_geometry_lane_block(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        incarnation: Hash,
        lane_block_height: u64,
    ) -> CertifiedLaneBlockArtifact {
        let keypair = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let validator_set = vec![PeerId::new(keypair.public_key().clone())];
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id,
            dataspace_id,
            lane_incarnation: incarnation,
            proposal_height: lane_block_height.max(1),
            previous_lane_block_height: lane_block_height.saturating_sub(1),
            previous_lane_block_descriptor_hash: lane_block_height
                .checked_sub(1)
                .filter(|height| *height > 0)
                .map(|height| Hash::new(height.to_le_bytes())),
            lane_block_height,
            lane_block_view: 1,
            subject_hash: Hash::new(b"geometry-gc-certified-subject"),
            payload_ownership_hash: Hash::new(b"geometry-gc-certified-ownership"),
            rbc_instance_hash: Hash::new(b"geometry-gc-certified-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"geometry-gc-certified-entrypoint")],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned:geometry-gc".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        certified_geometry_lane_block_for_proposal(proposal, &keypair)
    }

    fn certified_geometry_lane_block_for_proposal(
        proposal: LaneBlockProposalV1,
        keypair: &iroha_crypto::KeyPair,
    ) -> CertifiedLaneBlockArtifact {
        let signer_pop =
            bls_normal_pop_prove(keypair.private_key()).expect("geometry GC signer PoP");
        let validator_set = proposal.descriptor.validator_set.clone();
        assert_eq!(
            validator_set,
            vec![PeerId::new(keypair.public_key().clone())],
            "geometry certified fixture uses its signing peer as the only validator"
        );
        let vote = |phase| {
            let body = proposal.vote_body(phase);
            LaneBlockVoteV1 {
                bls_signature: Signature::try_new(
                    keypair.private_key(),
                    &body.signature_preimage(),
                )
                .expect("geometry GC lane vote signature")
                .payload()
                .to_vec(),
                body,
                signer: PeerId::new(keypair.public_key().clone()),
                payload_availability_vote: None,
            }
        };
        let prepare_vote = vote(CertPhase::Prepare);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_vote.body.clone(),
            validator_set.clone(),
            std::slice::from_ref(&prepare_vote),
        )
        .expect("geometry GC prepare QC");
        let commit_vote = vote(CertPhase::Commit);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_vote.body.clone(),
            validator_set,
            std::slice::from_ref(&commit_vote),
        )
        .expect("geometry GC commit QC");
        CertifiedLaneBlockArtifact::new(
            CommittedLaneBlockSession {
                proposal,
                prepare_qc,
                commit_qc,
            },
            BTreeMap::from([(keypair.public_key().clone(), signer_pop)]),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn geometry_lane_proposal_and_ownership(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
        proposal_view: u64,
        lane_block_height: u64,
        lane_block_view: u64,
        entrypoint_hash: Hash,
        keypair: &KeyPair,
    ) -> (LaneBlockProposalV1, SumeragiLanePayloadOwnership) {
        let validator_set = vec![PeerId::new(keypair.public_key().clone())];
        let mut ownership = SumeragiLanePayloadOwnership {
            proposal_height,
            proposal_view,
            lane_id,
            dataspace_id,
            lane_incarnation,
            lane_block_height,
            lane_block_view,
            subject_hash: Hash::new(b"geometry-retirement-subject-placeholder"),
            qc_mode_tag: "permissioned:geometry-retirement".to_owned(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![entrypoint_hash],
            previous_lane_block_height: lane_block_height.saturating_sub(1),
            previous_lane_block_descriptor_hash: lane_block_height
                .checked_sub(1)
                .filter(|height| *height > 0)
                .map(|height| Hash::new(height.to_le_bytes())),
            lane_block_descriptor_hash: Some(Hash::new(
                b"geometry-retirement-descriptor-placeholder",
            )),
            lane_block_descriptor_validator_set: validator_set.clone(),
            lane_block_descriptor_validator_count: 1,
            lane_block_descriptor_min_quorum: 1,
            payload_ownership_hash: Hash::new(b"geometry-retirement-payload-placeholder"),
            rbc_instance_hash: Hash::new(b"geometry-retirement-rbc-placeholder"),
        };
        let replay = ownership
            .compute_replay_hashes()
            .expect("geometry retirement replay hashes");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        let descriptor = LaneBlockDescriptorV1 {
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height,
            previous_lane_block_height: ownership.previous_lane_block_height,
            previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
            lane_block_height,
            lane_block_view,
            subject_hash: ownership.subject_hash,
            payload_ownership_hash: ownership.payload_ownership_hash,
            rbc_instance_hash: ownership.rbc_instance_hash,
            accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: ownership.qc_mode_tag.clone(),
            descriptor_hash: replay.lane_block_descriptor_hash,
        };
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        (proposal, ownership)
    }

    fn geometry_native_amx_receipt(
        chain_id_hash: Hash,
        source_id: [u8; Hash::LENGTH],
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        plan: &crate::queue::RoutingPlan,
        coordinator_proposal: &LaneBlockProposalV1,
        epoch: u64,
        participant_keypair: &KeyPair,
    ) -> NativeAmxReceipt {
        let crate::queue::RoutingPlan::NativeAmx(native_plan) = plan else {
            panic!("geometry retirement fixture requires a native AMX plan");
        };
        let participant = native_plan
            .participants
            .first()
            .expect("geometry retirement fixture participant");
        let participant_validator_set = vec![PeerId::new(participant_keypair.public_key().clone())];
        let descriptor = &coordinator_proposal.descriptor;
        let prepare_body = NativeAmxAttestationBodyV2 {
            round: ConsensusRound {
                context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                    Hash::new(b"geometry-native-amx-v2-context"),
                )),
                height: descriptor.proposal_height,
                view: descriptor.lane_block_view,
            },
            epoch,
            source_id,
            tx_entrypoint_hash: entrypoint_hash,
            plan_digest: plan.digest(),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: descriptor.lane_id,
            coordinator_dataspace_id: descriptor.dataspace_id,
            participant_lane_id: participant.route.lane_id,
            participant_dataspace_id: participant.route.dataspace_id,
            planned_coordinator_block_height: descriptor.lane_block_height,
        };
        let qc = |body| NativeAmxAttestationQcV2 {
            body,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&participant_validator_set),
            validator_set: participant_validator_set.clone(),
            signers_bitmap: vec![1],
            bls_aggregate_signature: vec![0_u8; crate::native_amx::NATIVE_AMX_BLS_PROOF_BYTES],
        };
        let prepare_qc = qc(prepare_body);
        let mut commit_body = prepare_body;
        commit_body.phase = NativeAmxPhase::Commit;
        let commit_qc = qc(commit_body);
        NativeAmxReceipt {
            version: 2,
            source_id,
            chain_id_hash,
            plan_digest: plan.digest(),
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            authority_context_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            coordinator_proposal_hash: coordinator_proposal.proposal_hash,
            legs: vec![NativeAmxLegRecordV2 {
                lane_id: participant.route.lane_id,
                dataspace_id: participant.route.dataspace_id,
                prepare_qc,
                commit_qc,
            }],
        }
    }

    fn autonomous_retirement_payload(
        coordinator_incarnation: Hash,
        participant_lane_id: LaneId,
        participant_dataspace_id: DataSpaceId,
        producer: &KeyPair,
    ) -> (Hash, u64, crate::lane_consensus::LaneExecutablePayloadV1) {
        let chain: ChainId = "geometry-retirement-autonomous"
            .parse()
            .expect("geometry retirement chain id");
        let transaction =
            TransactionBuilder::new(chain.clone(), (*SAMPLE_GENESIS_ACCOUNT_ID).clone())
                .with_instructions([Log::new(
                    Level::INFO,
                    "geometry retirement payload".to_owned(),
                )])
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let source_hash = transaction.hash();
        let mut source_id = [0_u8; Hash::LENGTH];
        source_id.copy_from_slice(source_hash.as_ref());
        let entrypoint = TransactionEntrypoint::External(transaction);
        let entrypoint_hash = entrypoint.hash();
        let coordinator = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::new(7));
        let participant = crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(participant_lane_id, participant_dataspace_id),
            crate::queue::RouteLegRole::Participant,
        );
        let plan = crate::queue::RoutingPlan::native_amx(coordinator, vec![participant]);
        let (proposal, _) = geometry_lane_proposal_and_ownership(
            LaneId::SINGLE,
            DataSpaceId::new(7),
            coordinator_incarnation,
            42,
            0,
            1,
            0,
            Hash::from(entrypoint_hash),
            producer,
        );
        let chain_id_hash = Hash::new(chain.into_inner().as_bytes());
        let epoch = 9;
        let receipt = geometry_native_amx_receipt(
            chain_id_hash,
            source_id,
            entrypoint_hash,
            &plan,
            &proposal,
            epoch,
            producer,
        );
        let reservation = crate::queue::LaneQueueReservationKeyV1 {
            signed_transaction_hash: source_hash,
            entrypoint_hash,
            routing_plan_digest: plan.digest(),
            coordinator_leg: plan.coordinator_leg(),
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            proposal_height: proposal.descriptor.proposal_height,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"geometry-retirement-reservation-owner"),
            proposal_identity_hash: Hash::new(b"geometry-retirement-proposal-identity"),
        };
        let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
            chain_id_hash,
            epoch,
            proposal,
            vec![entrypoint],
            vec![reservation],
            vec![plan],
            vec![Some(receipt)],
            PeerId::new(producer.public_key().clone()),
            producer.private_key(),
        )
        .expect("geometry autonomous retirement payload");
        (chain_id_hash, epoch, payload)
    }

    #[test]
    fn unjournaled_nonzero_activation_without_marker_fails_closed_before_intent() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let kura = open_kura(&root, &extended);

        let error = kura
            .apply_lane_geometry_transition(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
            )
            .expect_err("unjournaled dynamic storage must not be adopted without its marker");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "active dynamic lane storage has no incarnation marker",
        );
        assert!(
            kura.read_lane_geometry_journal()
                .expect("default geometry journal")
                .records
                .is_empty(),
            "missing-marker rejection must precede retirement intent publication"
        );
        let participant_blocks = extended
            .entry(LaneId::new(1))
            .expect("dynamic participant lane")
            .blocks_dir(&root);
        assert!(participant_blocks.is_dir());
        assert!(
            !participant_blocks.join(MARKER_FILE_NAME).exists(),
            "rejection must not synthesize authority for the unjournaled dynamic lane"
        );
    }

    #[test]
    fn scale_in_conservatively_rejects_pending_native_amx_participant_route() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, journal_before, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = autonomous_retirement_payload(
            extended_incarnations[&LaneId::SINGLE],
            LaneId::new(1),
            DataSpaceId::new(8),
            &producer,
        );
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist coordinator-owned participant work");

        let error = kura
            .apply_lane_geometry_transition(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
            )
            .expect_err("pending participant route must conservatively pin retirement");
        assert_geometry_io_error(
            &error,
            ErrorKind::WouldBlock,
            "pending autonomous payload targets a retiring lane incarnation",
        );
        assert!(
            extended
                .entry(LaneId::new(1))
                .expect("participant lane")
                .blocks_dir(&root)
                .exists(),
            "retirement admission fails before moving lane files"
        );
        assert_eq!(
            fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
            journal_before,
            "rejected retirement must not alter the published geometry journal"
        );
    }

    #[test]
    fn scale_in_allows_unrelated_participant_work() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("unrelated-lane");
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, _, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = autonomous_retirement_payload(
            extended_incarnations[&LaneId::SINGLE],
            LaneId::new(9),
            DataSpaceId::new(19),
            &producer,
        );
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist non-target participant work");

        kura.apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .unwrap_or_else(|error| panic!("unrelated lane should not pin old retirement: {error}"));
    }

    #[test]
    fn scale_in_rejects_unknown_and_malformed_artifact_files_before_intent() {
        for (label, file_name, bytes, expected_kind, expected_message) in [
            (
                "unknown",
                "operator-junk.bin",
                b"junk".as_slice(),
                ErrorKind::InvalidData,
                "lane retirement scan encountered an unknown artifact filename",
            ),
            (
                "stale-temp",
                "autonomous_blocks.norito.tmp",
                b"partial".as_slice(),
                ErrorKind::WouldBlock,
                "lane retirement scan found an in-flight autonomous sidecar",
            ),
            (
                "malformed-view",
                "autonomous_view_1.norito",
                b"not-a-view-state".as_slice(),
                ErrorKind::InvalidData,
                "lane retirement scan encountered a non-canonical view-state filename",
            ),
            (
                "orphan-view",
                "autonomous_view_00000000000000000001.norito",
                b"not-a-view-state".as_slice(),
                ErrorKind::InvalidData,
                "lane retirement scan found an orphan autonomous view state",
            ),
        ] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(label);
            let (initial, extended) = retirement_test_configs();
            let (extended_incarnations, extended_activations) = retirement_test_geometry();
            let initial_incarnations =
                BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
            let initial_activations =
                BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
            let (kura, journal_before, _) = open_published_retirement_kura(
                &root,
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
            );
            let artifact_dir = Kura::lane_artifact_dir(
                &extended
                    .entry(LaneId::SINGLE)
                    .expect("coordinator lane")
                    .blocks_dir(&root),
            );
            fs::create_dir_all(&artifact_dir).expect("artifact directory");
            fs::write(artifact_dir.join(file_name), bytes).expect("hostile artifact");

            let error = kura
                .apply_lane_geometry_transition(
                    &extended,
                    &initial,
                    &extended_incarnations,
                    &initial_incarnations,
                    &extended_activations,
                    &initial_activations,
                    &BTreeSet::new(),
                )
                .expect_err("hostile retirement artifact must fail before intent publication");
            assert_geometry_io_error(&error, expected_kind, expected_message);
            assert_eq!(
                fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
                journal_before,
                "{label} artifact must fail before an intent is published"
            );
        }
    }

    #[test]
    fn fake_stale_and_forked_payload_hints_cannot_bypass_scale_in_admission() {
        for label in ["fork-hash", "stale-height", "stale-view"] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(label);
            let (initial, extended) = retirement_test_configs();
            let (extended_incarnations, extended_activations) = retirement_test_geometry();
            let initial_incarnations =
                BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
            let initial_activations =
                BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
            let (kura, journal_before, _) = open_published_retirement_kura(
                &root,
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
            );
            let (canonical_hash, _) = durable_geometry_snapshot_identity(&kura, 1);
            let canonical_view = kura
                .get_block(NonZeroUsize::new(1).expect("non-zero height"))
                .expect("canonical block")
                .header()
                .view_change_index();
            let hint = match label {
                "fork-hash" => LaneBlockProposalPayloadHintV1 {
                    proposal_height: 1,
                    proposal_view: canonical_view,
                    proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"geometry-retirement-fork",
                    )),
                },
                "stale-height" => LaneBlockProposalPayloadHintV1 {
                    proposal_height: 2,
                    proposal_view: canonical_view,
                    proposal_block_hash: canonical_hash,
                },
                "stale-view" => LaneBlockProposalPayloadHintV1 {
                    proposal_height: 1,
                    proposal_view: canonical_view.saturating_add(1),
                    proposal_block_hash: canonical_hash,
                },
                _ => unreachable!(),
            };
            let mut certified = certified_geometry_lane_block(
                LaneId::SINGLE,
                DataSpaceId::new(7),
                extended_incarnations[&LaneId::SINGLE],
                1,
            );
            certified.proposal.payload_block_hint = Some(hint);
            kura.write_certified_lane_block_artifact(&certified)
                .expect("persist adversarial hinted certificate");

            let error = kura
                .apply_lane_geometry_transition(
                    &extended,
                    &initial,
                    &extended_incarnations,
                    &initial_incarnations,
                    &extended_activations,
                    &initial_activations,
                    &BTreeSet::new(),
                )
                .expect_err("an unproven hint cannot mark certified work as applied");
            let expected_message = match label {
                "fork-hash" => {
                    "lane retirement payload hint does not identify the canonical durable block"
                }
                "stale-height" => {
                    "lane retirement payload hint height differs from the certified descriptor"
                }
                "stale-view" => {
                    "lane retirement payload hint differs from its canonical block header"
                }
                _ => unreachable!(),
            };
            assert_geometry_io_error(&error, ErrorKind::InvalidData, expected_message);
            assert_eq!(
                fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
                journal_before,
                "{label} hint must fail before retirement intent"
            );
        }
    }

    #[test]
    fn canonical_block_and_current_receipt_release_applied_participant_work() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, _, baseline_records) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let chain: ChainId = "geometry-retirement-committed"
            .parse()
            .expect("geometry retirement committed chain");
        let transaction =
            TransactionBuilder::new(chain.clone(), (*SAMPLE_GENESIS_ACCOUNT_ID).clone())
                .with_instructions([Log::new(
                    Level::INFO,
                    "geometry retirement committed participant work".to_owned(),
                )])
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let source_hash = transaction.hash();
        let mut source_id = [0_u8; Hash::LENGTH];
        source_id.copy_from_slice(source_hash.as_ref());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
        let mut block: SignedBlock = BlockBuilder::new(vec![accepted])
            .chain(0, None)
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
        let entrypoint = block
            .external_entrypoints_cloned()
            .next()
            .expect("committed external entrypoint");
        let entrypoint_hash = entrypoint.hash();
        let (mut proposal, ownership) = geometry_lane_proposal_and_ownership(
            LaneId::SINGLE,
            DataSpaceId::new(7),
            extended_incarnations[&LaneId::SINGLE],
            block.header().height().get(),
            block.header().view_change_index(),
            1,
            0,
            Hash::from(entrypoint_hash),
            &producer,
        );
        let plan = crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::new(7)),
            vec![crate::queue::RouteLeg::new(
                crate::queue::RoutingDecision::new(LaneId::new(1), DataSpaceId::new(8)),
                crate::queue::RouteLegRole::Participant,
            )],
        );
        let receipt = geometry_native_amx_receipt(
            Hash::new(chain.into_inner().as_bytes()),
            source_id,
            entrypoint_hash,
            &plan,
            &proposal,
            0,
            &producer,
        );
        let context = crate::queue::execution_context_for_routing_plan(entrypoint_hash, &plan)
            .with_native_amx_receipt(receipt);
        block.set_execution_context(Some(
            BlockExecutionContextBundle::new(vec![context])
                .with_lane_payload_ownerships(vec![ownership]),
        ));
        let entrypoint_hashes = block
            .external_entrypoints_cloned()
            .map(|entrypoint| entrypoint.hash())
            .collect::<Vec<_>>();
        block
            .set_transaction_results(
                Vec::new(),
                &entrypoint_hashes,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach committed result");
        let block = Arc::new(block);
        proposal.payload_block_hint = Some(LaneBlockProposalPayloadHintV1 {
            proposal_height: block.header().height().get(),
            proposal_view: block.header().view_change_index(),
            proposal_block_hash: block.hash(),
        });
        kura.store_block(Arc::clone(&block))
            .expect("store canonical global block");
        let certified = certified_geometry_lane_block_for_proposal(proposal.clone(), &producer);
        kura.write_certified_lane_block_artifact(&certified)
            .expect("persist globally backed lane certificate");
        kura.persist_lane_block_application_receipt(&proposal)
            .expect("persist canonical current application receipt");

        kura.apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect("canonically applied participant work no longer pins scale-in");
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("geometry journal")
                .records
                .len(),
            baseline_records + 1
        );
    }

    #[test]
    fn zero_file_create_intent_rolls_back_to_a_sealed_image_and_replays() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let operation = persist_create_intent(
            &kura,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let updated = operation.updated.as_ref().expect("created binding");
        let live_blocks = kura.binding_blocks_path(updated);
        let live_merge = kura.binding_merge_path(updated);
        let unpublished_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("unpublished blocks");
        let unpublished_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("unpublished merge");
        assert!(!live_blocks.exists());
        assert!(!live_merge.exists());
        assert!(!unpublished_blocks.exists());
        assert!(!unpublished_merge.exists());

        for _ in 0..2 {
            kura.recover_lane_geometry_journal(
                &initial,
                &initial_incarnations,
                &initial_activations,
            )
            .expect("zero-file Intent rollback is idempotent");
            assert!(!live_blocks.exists());
            assert!(!live_merge.exists());
            kura.require_sealed_geometry_pair_at(
                updated,
                &unpublished_blocks,
                &unpublished_merge,
                &unpublished_blocks,
                &unpublished_merge,
            )
            .expect("rollback persists an authenticated empty image");
            assert_eq!(
                kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                LaneGeometryPhase::RolledBack
            );
        }

        // A same-authority retry must resume when replay durably retargeted the retained pair to
        // live but crashed before the first rename. The terminal phase is deliberately left at
        // `RolledBack` across that filesystem window.
        kura.seal_geometry_pair_move(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &live_blocks,
            &live_merge,
        )
        .expect("inject same-authority replay crash before first rename");
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("same-authority replay resumes its pre-rename seal");
        kura.require_complete_geometry_binding_at(updated, &live_blocks, &live_merge)
            .expect("created lane is live after same-authority replay");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("return replayed create to its retained rollback image");
        kura.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("same-authority lifecycle restores an immutable rollback image");

        // Replay persisted its live-target seal but died before either rename. Remaining on the
        // old catalog must recognize that exact opposite-path seal and normalize it back to the
        // retained rollback image.
        kura.seal_geometry_pair_move(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &live_blocks,
            &live_merge,
        )
        .expect("inject RolledBack replay crash before first rename");
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("old-catalog recovery reverses the pre-rename replay seal");
        kura.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("rollback image is sealed back to itself");

        for _ in 0..2 {
            kura.recover_lane_geometry_journal(
                &extended,
                &extended_incarnations,
                &extended_activations,
            )
            .expect("sealed rollback image replays exactly");
            kura.require_complete_geometry_binding_at(updated, &live_blocks, &live_merge)
                .expect("created lane is live after replay");
            assert!(!unpublished_blocks.exists());
            assert!(!unpublished_merge.exists());
            assert_eq!(
                kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                LaneGeometryPhase::CatalogPublished
            );
        }
    }

    #[test]
    fn create_intent_repairs_authenticated_blocks_before_merge_for_rollback_and_replay() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let operation = persist_create_intent(
            &kura,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let updated = operation.updated.as_ref().expect("created binding");
        let staged = LaneGeometryBinding {
            blocks_path: operation.unpublished_blocks_path.clone(),
            merge_path: operation.unpublished_merge_path.clone(),
            ..updated.clone()
        };
        let staged_blocks = kura.binding_blocks_path(&staged);
        let staged_merge = kura.binding_merge_path(&staged);
        kura.provision_geometry_binding(&staged)
            .expect("provision journal-owned staging");
        fs::remove_file(&staged_merge).expect("inject crash before merge creation");
        assert!(staged_blocks.join(MARKER_FILE_NAME).is_file());
        assert!(!staged_merge.exists());

        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("rollback repairs authenticated partial provisioning");
        kura.require_sealed_geometry_pair_at(
            updated,
            &staged_blocks,
            &staged_merge,
            &staged_blocks,
            &staged_merge,
        )
        .expect("repaired rollback image is sealed");

        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("replay consumes the repaired image");
        kura.require_complete_geometry_binding_at(
            updated,
            &kura.binding_blocks_path(updated),
            &kura.binding_merge_path(updated),
        )
        .expect("created binding is complete after replay");
    }

    #[test]
    fn create_intent_rejects_merge_only_staging_without_adopting_it() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let operation = persist_create_intent(
            &kura,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let staged_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("staged blocks");
        let staged_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("staged merge");
        create_dir_all_with_context(staged_merge.parent().expect("merge parent"))
            .expect("create merge parent");
        fs::write(&staged_merge, b"").expect("inject merge-only staging");

        let error = kura
            .recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
            .expect_err("merge-only staging must fail closed");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "replacement provisioning has an orphan block or merge-log path",
        );
        assert!(!staged_blocks.exists());
        assert!(staged_merge.is_file());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::Intent
        );
    }

    #[test]
    fn create_intent_rejects_complete_unsealed_foreign_pairs() {
        for location in ["staging", "live"] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(format!("kura-{location}"));
            let (initial, extended) = initial_and_extended_configs();
            let (initial_incarnations, initial_activations) = initial_geometry();
            let (extended_incarnations, extended_activations) = extended_geometry();
            let kura = open_kura(&root, &initial);
            let operation = persist_create_intent(
                &kura,
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
            );
            let updated = operation.updated.as_ref().expect("created binding");
            let injected = if location == "staging" {
                LaneGeometryBinding {
                    blocks_path: operation.unpublished_blocks_path.clone(),
                    merge_path: operation.unpublished_merge_path.clone(),
                    ..updated.clone()
                }
            } else {
                updated.clone()
            };
            kura.provision_geometry_binding(&injected)
                .expect("provision valid-looking unsealed pair");
            let injected_blocks = kura.binding_blocks_path(&injected);
            let injected_merge = kura.binding_merge_path(&injected);
            let sentinel = injected_blocks.join("foreign-intent-payload");
            fs::write(&sentinel, b"must-not-be-adopted").expect("inject foreign block payload");

            let error = kura
                .recover_lane_geometry_journal(
                    &extended,
                    &extended_incarnations,
                    &extended_activations,
                )
                .expect_err("an unsealed nonempty pair must not gain authority from Intent");
            assert_geometry_io_error(
                &error,
                ErrorKind::InvalidData,
                "unbound configured primary block store contains an unexpected entry",
            );
            assert_eq!(
                fs::read(&sentinel).expect("foreign payload retained for diagnosis"),
                b"must-not-be-adopted"
            );
            assert!(injected_merge.is_file());
            assert_eq!(
                kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                LaneGeometryPhase::Intent
            );
        }
    }

    #[test]
    fn terminal_geometry_replay_never_reauthorizes_empty_provisioning() {
        // A failed rollback of a published transition must retain `CatalogPublished`; otherwise a
        // restart could reinterpret it as a first-application Intent and manufacture empty state.
        {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join("kura");
            let (initial, extended) = initial_and_extended_configs();
            let (initial_incarnations, initial_activations) = initial_geometry();
            let (extended_incarnations, extended_activations) = extended_geometry();
            let kura = open_kura(&root, &initial);
            kura.apply_lane_geometry_transition(
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
                &BTreeSet::new(),
            )
            .expect("apply create transition");
            kura.mark_lane_geometry_catalog_published(
                &extended,
                &extended_incarnations,
                &extended_activations,
                None,
            )
            .expect("publish create transition");
            let operation = kura
                .read_lane_geometry_journal()
                .expect("published journal")
                .records[0]
                .operations[0]
                .clone();
            let updated = operation.updated.as_ref().expect("created binding");
            fs::remove_dir_all(kura.binding_blocks_path(updated))
                .expect("simulate loss of published blocks");
            fs::remove_file(kura.binding_merge_path(updated))
                .expect("simulate loss of published merge log");

            for _ in 0..2 {
                let error = kura
                    .recover_lane_geometry_journal(
                        &initial,
                        &initial_incarnations,
                        &initial_activations,
                    )
                    .expect_err("missing published evidence must fail on every retry");
                assert_geometry_io_error(
                    &error,
                    ErrorKind::NotFound,
                    "durable lane geometry evidence is missing; refusing to provision an empty replacement",
                );
                assert_eq!(
                    kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                    LaneGeometryPhase::CatalogPublished
                );
                assert!(
                    !kura
                        .resolve_relative_path(&operation.unpublished_blocks_path)
                        .expect("unpublished blocks")
                        .exists()
                );
                assert!(
                    !kura
                        .resolve_relative_path(&operation.unpublished_merge_path)
                        .expect("unpublished merge")
                        .exists()
                );
            }
        }

        // The inverse direction must likewise retain `RolledBack` when its authenticated retained
        // image disappears; replay is not authority to create a replacement from nothing.
        {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join("kura");
            let (initial, extended) = initial_and_extended_configs();
            let (initial_incarnations, initial_activations) = initial_geometry();
            let (extended_incarnations, extended_activations) = extended_geometry();
            let kura = open_kura(&root, &initial);
            kura.apply_lane_geometry_transition(
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
                &BTreeSet::new(),
            )
            .expect("apply create transition");
            kura.recover_lane_geometry_journal(
                &initial,
                &initial_incarnations,
                &initial_activations,
            )
            .expect("roll transition back to its retained image");
            let operation = kura
                .read_lane_geometry_journal()
                .expect("rolled-back journal")
                .records[0]
                .operations[0]
                .clone();
            let unpublished_blocks = kura
                .resolve_relative_path(&operation.unpublished_blocks_path)
                .expect("unpublished blocks");
            let unpublished_merge = kura
                .resolve_relative_path(&operation.unpublished_merge_path)
                .expect("unpublished merge");
            fs::remove_dir_all(&unpublished_blocks).expect("simulate loss of retained block image");
            fs::remove_file(&unpublished_merge).expect("simulate loss of retained merge image");

            for _ in 0..2 {
                let error = kura
                    .recover_lane_geometry_journal(
                        &extended,
                        &extended_incarnations,
                        &extended_activations,
                    )
                    .expect_err("missing retained evidence must fail on every retry");
                assert_geometry_io_error(
                    &error,
                    ErrorKind::NotFound,
                    "durable lane geometry evidence is missing; refusing to provision an empty replacement",
                );
                assert_eq!(
                    kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                    LaneGeometryPhase::RolledBack
                );
                assert!(!unpublished_blocks.exists());
                assert!(!unpublished_merge.exists());
            }
        }
    }

    #[test]
    fn recovery_rolls_back_partial_unpublished_create_and_replays_it_idempotently() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare journaled create");
        let lane1 = extended.entry(LaneId::new(1)).expect("lane one");
        assert!(lane1.blocks_dir(&root).exists());

        kura.apply_lane_geometry_transition(
            &initial,
            &initial,
            &initial_incarnations,
            &initial_incarnations,
            &initial_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect("same-catalog startup rolls back unpublished create");
        assert!(!lane1.blocks_dir(&root).exists());

        // Model a process dying after restoring only the block directory from
        // the unpublished archive. Recovery of the old catalog must complete
        // the inverse operation without duplicating or dropping either path.
        let mut journal = kura.read_lane_geometry_journal().expect("read journal");
        journal.records[0].phase = LaneGeometryPhase::Intent;
        kura.write_lane_geometry_journal(&journal)
            .expect("persist in-progress roll-forward phase");
        let operation = journal.records[0].operations[0].clone();
        let updated = operation.updated.as_ref().expect("created binding");
        let unpublished_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("unpublished blocks path");
        let unpublished_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("unpublished merge path");
        let live_blocks = lane1.blocks_dir(&root);
        let live_merge = lane1.merge_log_path(&root);
        kura.seal_geometry_pair_move(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &live_blocks,
            &live_merge,
        )
        .expect("seal partial roll-forward exactly as production does");
        kura.move_geometry_path(&unpublished_blocks, &live_blocks, true)
            .expect("inject partial roll-forward");
        assert!(lane1.blocks_dir(&root).exists());

        for _ in 0..2 {
            kura.recover_lane_geometry_journal(
                &initial,
                &initial_incarnations,
                &initial_activations,
            )
            .expect("idempotent rollback recovery");
            assert!(!lane1.blocks_dir(&root).exists());
            assert!(!lane1.merge_log_path(&root).exists());
        }

        // The catalog is now authoritative (as after snapshot/block replay),
        // so the same retained intent must roll forward and recover the exact
        // unpublished segment instead of provisioning an empty replacement.
        for _ in 0..2 {
            kura.recover_lane_geometry_journal(
                &extended,
                &extended_incarnations,
                &extended_activations,
            )
            .expect("idempotent roll-forward recovery");
            assert!(lane1.blocks_dir(&root).exists());
            assert!(lane1.merge_log_path(&root).exists());
        }
        let recovered = kura.read_lane_geometry_journal().expect("read journal");
        assert_eq!(
            recovered.records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
    }

    #[test]
    fn geometry_moves_never_clobber_targets_materialized_after_preflight() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);

        let source_blocks = root.join("move-collision/source-blocks");
        let target_blocks = root.join("move-collision/target-blocks");
        fs::create_dir_all(&source_blocks).expect("seed source block directory");
        fs::write(source_blocks.join("sentinel"), b"source-blocks")
            .expect("seed source block sentinel");
        *GEOMETRY_MOVE_TARGET_COLLISION
            .lock()
            .expect("geometry collision hook lock") = Some(target_blocks.clone());
        kura.move_geometry_path(&source_blocks, &target_blocks, true)
            .expect_err("a target created after preflight must stop the block-directory move");
        assert_eq!(
            fs::read(source_blocks.join("sentinel")).expect("source block sentinel retained"),
            b"source-blocks"
        );
        assert!(
            target_blocks.is_dir(),
            "the injected target must not be replaced by the source directory"
        );
        assert!(
            fs::read_dir(&target_blocks)
                .expect("read injected block target")
                .next()
                .is_none(),
            "the injected directory must remain untouched"
        );

        let source_merge = root.join("move-collision/source-merge.log");
        let target_merge = root.join("move-collision/target-merge.log");
        fs::write(&source_merge, b"source-merge").expect("seed source merge file");
        *GEOMETRY_MOVE_TARGET_COLLISION
            .lock()
            .expect("geometry collision hook lock") = Some(target_merge.clone());
        kura.move_geometry_path(&source_merge, &target_merge, false)
            .expect_err("a target created after preflight must stop the merge-file move");
        assert_eq!(
            fs::read(&source_merge).expect("source merge file retained"),
            b"source-merge"
        );
        assert_eq!(
            fs::read(&target_merge).expect("injected merge target retained"),
            b"injected-no-clobber-target"
        );
    }

    #[test]
    fn mutable_pair_move_supports_a_stationary_block_path_and_later_merge_appends() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        let blocks = root.join("pair-move/live-blocks");
        let old_merge = root.join("pair-move/old-merge.log");
        let new_merge = root.join("pair-move/new-merge.log");
        let binding = LaneGeometryBinding {
            lane_id: LaneId::new(7),
            incarnation: Hash::new(b"stationary-block-pair"),
            activation_height: 1,
            blocks_path: kura
                .relative_geometry_path(&blocks)
                .expect("relative block path"),
            merge_path: kura
                .relative_geometry_path(&old_merge)
                .expect("relative old merge path"),
        };
        kura.provision_geometry_binding(&binding)
            .expect("provision movable geometry pair");
        fs::write(&old_merge, b"before-move").expect("seed merge bytes");

        kura.move_geometry_binding_pair(
            &binding,
            &blocks,
            &old_merge,
            &blocks,
            &new_merge,
            GeometryPairTargetKind::MutableLive,
        )
        .expect("move only the merge half under a stationary block path");
        assert!(!old_merge.exists());
        assert_eq!(fs::read(&new_merge).expect("moved merge"), b"before-move");
        let marker = kura
            .read_lane_marker(&blocks.join(MARKER_FILE_NAME))
            .expect("read completed live marker");
        assert!(marker.move_target_blocks.is_none());
        assert!(marker.move_target_merge.is_none());

        fs::write(&new_merge, b"before-move-and-legitimate-append")
            .expect("append live merge history");
        kura.move_geometry_binding_pair(
            &binding,
            &blocks,
            &old_merge,
            &blocks,
            &new_merge,
            GeometryPairTargetKind::MutableLive,
        )
        .expect("a completed live move remains idempotent after legitimate merge growth");
        assert_eq!(
            fs::read(&new_merge).expect("appended merge retained"),
            b"before-move-and-legitimate-append"
        );
    }

    #[test]
    fn inverse_pair_move_recovers_a_seal_persisted_before_the_first_rename() {
        fn exercise(
            kura: &Kura,
            root: &Path,
            label: &str,
            shared_blocks: bool,
            shared_merge: bool,
            inverse_target_kind: GeometryPairTargetKind,
        ) {
            let case_root = root.join(label);
            let original_blocks = case_root.join("original-blocks");
            let original_merge = case_root.join("original-merge.log");
            let forward_blocks = if shared_blocks {
                original_blocks.clone()
            } else {
                case_root.join("forward-blocks")
            };
            let forward_merge = if shared_merge {
                original_merge.clone()
            } else {
                case_root.join("forward-merge.log")
            };
            let binding = LaneGeometryBinding {
                lane_id: LaneId::new(20),
                incarnation: Hash::new(label.as_bytes()),
                activation_height: 1,
                blocks_path: kura
                    .relative_geometry_path(&original_blocks)
                    .expect("relative original blocks"),
                merge_path: kura
                    .relative_geometry_path(&original_merge)
                    .expect("relative original merge"),
            };
            kura.provision_geometry_binding(&binding)
                .expect("provision original pair");
            fs::write(&original_merge, format!("{label}-merge-evidence"))
                .expect("seed merge evidence");
            kura.seal_geometry_pair_move(
                &binding,
                &original_blocks,
                &original_merge,
                &forward_blocks,
                &forward_merge,
            )
            .expect("persist forward seal before first rename");

            kura.move_geometry_binding_pair(
                &binding,
                &forward_blocks,
                &forward_merge,
                &original_blocks,
                &original_merge,
                inverse_target_kind,
            )
            .expect("inverse move recognizes the exact opposite-path seal");

            assert!(original_blocks.is_dir());
            assert!(original_merge.is_file());
            if forward_blocks != original_blocks {
                assert!(!forward_blocks.exists());
            }
            if forward_merge != original_merge {
                assert!(!forward_merge.exists());
            }
            match inverse_target_kind {
                GeometryPairTargetKind::MutableLive => {
                    let marker = kura
                        .read_lane_marker(&original_blocks.join(MARKER_FILE_NAME))
                        .expect("read normalized mutable marker");
                    assert!(marker.move_target_blocks.is_none());
                    assert!(marker.move_target_merge.is_none());
                }
                GeometryPairTargetKind::ImmutableRetained => kura
                    .require_sealed_geometry_pair_at(
                        &binding,
                        &original_blocks,
                        &original_merge,
                        &original_blocks,
                        &original_merge,
                    )
                    .expect("immutable inverse target retains its normalized seal"),
            }
        }

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        exercise(
            &kura,
            &root,
            "full-mutable",
            false,
            false,
            GeometryPairTargetKind::MutableLive,
        );
        exercise(
            &kura,
            &root,
            "full-immutable",
            false,
            false,
            GeometryPairTargetKind::ImmutableRetained,
        );
        exercise(
            &kura,
            &root,
            "stationary-blocks",
            true,
            false,
            GeometryPairTargetKind::MutableLive,
        );
        exercise(
            &kura,
            &root,
            "stationary-merge",
            false,
            true,
            GeometryPairTargetKind::MutableLive,
        );
    }

    #[test]
    fn inverse_pair_move_recovers_clear_temp_after_both_renames() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        let original_blocks = root.join("clear-temp/original-blocks");
        let original_merge = root.join("clear-temp/original-merge.log");
        let moved_blocks = root.join("clear-temp/moved-blocks");
        let moved_merge = root.join("clear-temp/moved-merge.log");
        let binding = LaneGeometryBinding {
            lane_id: LaneId::new(21),
            incarnation: Hash::new(b"clear-temp-direction-reversal"),
            activation_height: 1,
            blocks_path: kura
                .relative_geometry_path(&original_blocks)
                .expect("relative original blocks"),
            merge_path: kura
                .relative_geometry_path(&original_merge)
                .expect("relative original merge"),
        };
        kura.provision_geometry_binding(&binding)
            .expect("provision movable pair");
        fs::write(&original_merge, b"clear-temp-merge-evidence").expect("seed merge evidence");
        fs::write(original_blocks.join("payload"), b"block-image-evidence")
            .expect("seed block evidence");

        kura.seal_geometry_pair_move(
            &binding,
            &original_blocks,
            &original_merge,
            &moved_blocks,
            &moved_merge,
        )
        .expect("seal forward pair move");
        kura.move_geometry_path(&original_blocks, &moved_blocks, true)
            .expect("move block half");
        kura.move_geometry_path(&original_merge, &moved_merge, false)
            .expect("move merge half");
        let stale_clear = LaneIncarnationMarker {
            version: MARKER_VERSION,
            lane_id: binding.lane_id,
            incarnation: binding.incarnation,
            activation_height: binding.activation_height,
            move_target_blocks: None,
            move_target_merge: None,
            block_store_digest: kura
                .geometry_block_store_digest(&moved_blocks)
                .expect("moved block digest"),
            merge_log_digest: kura
                .geometry_merge_log_digest(&moved_merge)
                .expect("moved merge digest"),
        };
        let stale_temp = moved_blocks.join(MARKER_TEMP_FILE_NAME);
        fs::write(&stale_temp, stale_clear.encode())
            .expect("simulate crash before seal-clear marker rename");

        kura.move_geometry_binding_pair(
            &binding,
            &moved_blocks,
            &moved_merge,
            &original_blocks,
            &original_merge,
            GeometryPairTargetKind::MutableLive,
        )
        .expect("inverse direction discards the authenticated uncommitted clear temp");
        assert!(!stale_temp.exists());
        assert_eq!(
            fs::read(original_blocks.join("payload")).expect("block bytes restored"),
            b"block-image-evidence"
        );
        assert_eq!(
            fs::read(&original_merge).expect("merge bytes restored"),
            b"clear-temp-merge-evidence"
        );
        let marker = kura
            .read_lane_marker(&original_blocks.join(MARKER_FILE_NAME))
            .expect("read restored live marker");
        assert!(marker.move_target_blocks.is_none());
        assert!(marker.move_target_merge.is_none());

        kura.move_geometry_binding_pair(
            &binding,
            &moved_blocks,
            &moved_merge,
            &original_blocks,
            &original_merge,
            GeometryPairTargetKind::MutableLive,
        )
        .expect("completed inverse remains idempotent");

        let foreign_temp = original_blocks.join(MARKER_TEMP_FILE_NAME);
        fs::write(
            &foreign_temp,
            LaneIncarnationMarker {
                version: MARKER_VERSION,
                lane_id: binding.lane_id,
                incarnation: Hash::new(b"foreign-marker-temp"),
                activation_height: binding.activation_height,
                move_target_blocks: None,
                move_target_merge: None,
                block_store_digest: kura
                    .geometry_block_store_digest(&original_blocks)
                    .expect("current block digest"),
                merge_log_digest: kura
                    .geometry_merge_log_digest(&original_merge)
                    .expect("current merge digest"),
            }
            .encode(),
        )
        .expect("inject foreign marker temp");
        let error = kura
            .move_geometry_binding_pair(
                &binding,
                &original_blocks,
                &original_merge,
                &moved_blocks,
                &moved_merge,
                GeometryPairTargetKind::MutableLive,
            )
            .expect_err("foreign marker temp must not be removed or adopted");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "lane storage incarnation marker does not match authoritative binding",
        );
        assert!(foreign_temp.is_file());
        assert!(original_blocks.is_dir());
        assert!(original_merge.is_file());
    }

    #[test]
    fn immutable_pair_move_rejects_a_post_crash_foreign_merge_swap() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        let source_blocks = root.join("sealed-pair/live-blocks");
        let source_merge = root.join("sealed-pair/live-merge.log");
        let target_blocks = root.join("sealed-pair/archive-blocks");
        let target_merge = root.join("sealed-pair/archive-merge.log");
        let binding = LaneGeometryBinding {
            lane_id: LaneId::new(8),
            incarnation: Hash::new(b"immutable-retained-pair"),
            activation_height: 1,
            blocks_path: kura
                .relative_geometry_path(&source_blocks)
                .expect("relative source block path"),
            merge_path: kura
                .relative_geometry_path(&source_merge)
                .expect("relative source merge path"),
        };
        kura.provision_geometry_binding(&binding)
            .expect("provision retained geometry pair");
        fs::write(&source_merge, b"authoritative-merge-history")
            .expect("seed authoritative merge bytes");
        kura.move_geometry_binding_pair(
            &binding,
            &source_blocks,
            &source_merge,
            &target_blocks,
            &target_merge,
            GeometryPairTargetKind::ImmutableRetained,
        )
        .expect("archive authenticated pair");

        fs::write(&target_merge, b"foreign-valid-looking-merge-history")
            .expect("swap retained merge bytes");
        let error = kura
            .move_geometry_binding_pair(
                &binding,
                &source_blocks,
                &source_merge,
                &target_blocks,
                &target_merge,
                GeometryPairTargetKind::ImmutableRetained,
            )
            .expect_err("retained pair digest must reject a foreign merge swap");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "lane geometry pair does not match its durable block/merge evidence",
        );
        assert!(target_blocks.is_dir());
        assert_eq!(
            fs::read(&target_merge).expect("foreign bytes retained for operator inspection"),
            b"foreign-valid-looking-merge-history"
        );
    }

    #[test]
    fn immutable_pair_move_rejects_a_post_crash_block_image_swap() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        let source_blocks = root.join("sealed-block-pair/live-blocks");
        let source_merge = root.join("sealed-block-pair/live-merge.log");
        let target_blocks = root.join("sealed-block-pair/archive-blocks");
        let target_merge = root.join("sealed-block-pair/archive-merge.log");
        let binding = LaneGeometryBinding {
            lane_id: LaneId::new(9),
            incarnation: Hash::new(b"immutable-retained-block-image"),
            activation_height: 1,
            blocks_path: kura
                .relative_geometry_path(&source_blocks)
                .expect("relative source block path"),
            merge_path: kura
                .relative_geometry_path(&source_merge)
                .expect("relative source merge path"),
        };
        kura.provision_geometry_binding(&binding)
            .expect("provision retained geometry pair");
        let payload = source_blocks.join("retained-payload");
        fs::write(&payload, b"authoritative-block-image").expect("seed block image bytes");
        kura.move_geometry_binding_pair(
            &binding,
            &source_blocks,
            &source_merge,
            &target_blocks,
            &target_merge,
            GeometryPairTargetKind::ImmutableRetained,
        )
        .expect("archive authenticated pair");

        let retained_payload = target_blocks.join("retained-payload");
        fs::write(&retained_payload, b"foreign-valid-block-image")
            .expect("swap retained block bytes");
        let error = kura
            .move_geometry_binding_pair(
                &binding,
                &source_blocks,
                &source_merge,
                &target_blocks,
                &target_merge,
                GeometryPairTargetKind::ImmutableRetained,
            )
            .expect_err("retained pair digest must reject a foreign block image");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "lane geometry pair does not match its durable block/merge evidence",
        );
        assert_eq!(
            fs::read(&retained_payload).expect("foreign bytes retained for inspection"),
            b"foreign-valid-block-image"
        );
    }

    #[test]
    fn recovery_completes_journal_owned_staging_created_before_marker() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);

        let previous_bindings = kura
            .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
            .expect("initial bindings");
        let updated_bindings = kura
            .geometry_bindings(&extended, &extended_incarnations, &extended_activations)
            .expect("extended bindings");
        let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
        let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
        let previous_lineage_root = unscoped_lineage_root(&previous_bindings);
        let updated_lineage_root = unscoped_lineage_root(&updated_bindings);
        let transition_id = geometry_transition_id(
            0,
            0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        );
        let operations = kura
            .build_geometry_operations(
                transition_id,
                &previous_bindings,
                &updated_bindings,
                &BTreeSet::new(),
            )
            .expect("create operation");
        let intent = LaneGeometryIntent {
            transition_id,
            transition_sequence: 0,
            transition_height: 0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
            previous_bindings,
            updated_bindings,
            phase: LaneGeometryPhase::Intent,
            operations,
        };
        let mut journal = LaneGeometryJournal::default();
        journal.records.push(intent);
        kura.write_lane_geometry_journal(&journal)
            .expect("persist create intent before provisioning");
        let operation = &journal.records[0].operations[0];
        let staged_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("staged blocks path");
        fs::create_dir_all(&staged_blocks)
            .expect("simulate crash after creating the journal-owned staging directory");
        assert!(!staged_blocks.join(MARKER_FILE_NAME).exists());

        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("recovery must finish marker-first staging and publish it atomically");
        let lane = extended.entry(LaneId::new(1)).expect("created lane");
        assert!(lane.blocks_dir(&root).join(MARKER_FILE_NAME).is_file());
        assert!(lane.merge_log_path(&root).is_file());
        assert!(!staged_blocks.exists());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
    }

    #[test]
    fn replacement_rollback_finishes_merge_half_after_block_archive_crash() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = nonzero!(2_u32);
        let primary = ModelLaneConfig::default();
        let active_lane = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "replace-before".to_owned(),
            ..ModelLaneConfig::default()
        };
        let replacement_lane = ModelLaneConfig {
            alias: "replace-after".to_owned(),
            visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
            ..active_lane.clone()
        };
        let base_catalog =
            LaneCatalog::new(lane_count, vec![primary.clone()]).expect("base catalog");
        let active_catalog = LaneCatalog::new(lane_count, vec![primary.clone(), active_lane])
            .expect("active catalog");
        let replacement_catalog = LaneCatalog::new(lane_count, vec![primary, replacement_lane])
            .expect("replacement catalog");
        let base = RuntimeLaneConfig::from_catalog(&base_catalog);
        let active = RuntimeLaneConfig::from_catalog(&active_catalog);
        let replacement = RuntimeLaneConfig::from_catalog(&replacement_catalog);
        let base_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x31; Hash::LENGTH]))]);
        let active_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x32; Hash::LENGTH])),
        ]);
        let replacement_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x33; Hash::LENGTH])),
        ]);
        let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let active_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 4)]);
        let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 5)]);
        let kura = open_kura(&root, &base);
        kura.apply_lane_geometry_transition(
            &base,
            &active,
            &base_incarnations,
            &active_incarnations,
            &base_activations,
            &active_activations,
            &BTreeSet::new(),
        )
        .expect("create replaceable lane");
        kura.mark_lane_geometry_catalog_published(
            &active,
            &active_incarnations,
            &active_activations,
            None,
        )
        .expect("publish replaceable lane");
        kura.apply_lane_geometry_transition(
            &active,
            &replacement,
            &active_incarnations,
            &replacement_incarnations,
            &active_activations,
            &replacement_activations,
            &BTreeSet::from([LaneId::new(1)]),
        )
        .expect("apply replacement before simulated rollback crash");

        let journal = kura
            .read_lane_geometry_journal()
            .expect("replacement journal");
        let operation = journal.records[1].operations[0].clone();
        assert_eq!(operation.kind, LaneGeometryOperationKind::Replace);
        let updated = operation.updated.as_ref().expect("updated binding");
        let updated_blocks = kura.binding_blocks_path(updated);
        let updated_merge = kura.binding_merge_path(updated);
        let unpublished_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("unpublished blocks");
        let unpublished_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("unpublished merge");
        kura.seal_geometry_pair_move(
            updated,
            &updated_blocks,
            &updated_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("seal replacement rollback move before its block half");
        kura.move_geometry_path(&updated_blocks, &unpublished_blocks, true)
            .expect("simulate crash after archiving replacement blocks only");
        assert!(!updated_blocks.exists());
        assert!(updated_merge.is_file());
        assert!(!unpublished_merge.exists());

        kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
            .expect(
                "rollback must finish the replacement merge half before restoring the prior lane",
            );
        assert!(!updated_merge.exists());
        assert!(unpublished_blocks.is_dir());
        assert!(unpublished_merge.is_file());
        let active_lane = active.entry(LaneId::new(1)).expect("active lane");
        assert!(active_lane.blocks_dir(&root).is_dir());
        assert!(active_lane.merge_log_path(&root).is_file());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[1].phase,
            LaneGeometryPhase::RolledBack
        );

        // Replacement replay has the same pre-first-rename frontier as Create: the retained
        // updated incarnation can already carry its exact live-target seal while the journal is
        // still terminally `RolledBack`. Retrying the replacement authority must consume it.
        kura.seal_geometry_pair_move(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &updated_blocks,
            &updated_merge,
        )
        .expect("inject replacement replay crash before first rename");
        kura.recover_lane_geometry_journal(
            &replacement,
            &replacement_incarnations,
            &replacement_activations,
        )
        .expect("same-authority replacement replay resumes its pre-rename seal");
        kura.require_complete_geometry_binding_at(updated, &updated_blocks, &updated_merge)
            .expect("replacement incarnation is live after replay");
        assert!(!unpublished_blocks.exists());
        assert!(!unpublished_merge.exists());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[1].phase,
            LaneGeometryPhase::CatalogPublished
        );
        kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
            .expect("return replayed replacement to its retained rollback image");
        kura.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("replacement lifecycle restores an immutable rollback image");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[1].phase,
            LaneGeometryPhase::RolledBack
        );

        let previous = operation.previous.as_ref().expect("previous binding");
        assert_ne!(updated_blocks, kura.binding_blocks_path(previous));
        assert_ne!(updated_merge, kura.binding_merge_path(previous));
        fs::create_dir_all(&updated_blocks).expect("create duplicate updated block path");
        fs::copy(
            unpublished_blocks.join(MARKER_FILE_NAME),
            updated_blocks.join(MARKER_FILE_NAME),
        )
        .expect("copy duplicate updated marker");
        create_dir_all_with_context(updated_merge.parent().expect("updated merge parent"))
            .expect("create duplicate updated merge parent");
        fs::copy(&unpublished_merge, &updated_merge).expect("copy duplicate updated merge log");

        let error = kura
            .recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
            .expect_err("rolled-back replacement must reject duplicate updated live storage");
        assert_geometry_io_error(
            &error,
            ErrorKind::AlreadyExists,
            "replacement rollback left the updated incarnation live",
        );
        assert!(updated_blocks.is_dir());
        assert!(updated_merge.is_file());
        assert!(unpublished_blocks.is_dir());
        assert!(unpublished_merge.is_file());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[1].phase,
            LaneGeometryPhase::RolledBack
        );
    }

    #[test]
    fn replacement_intent_rollback_resumes_block_only_inverse_half() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = nonzero!(2_u32);
        let primary = ModelLaneConfig::default();
        let active_lane = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "intent-replace-before".to_owned(),
            ..ModelLaneConfig::default()
        };
        let replacement_lane = ModelLaneConfig {
            alias: "intent-replace-after".to_owned(),
            visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
            ..active_lane.clone()
        };
        let base_catalog =
            LaneCatalog::new(lane_count, vec![primary.clone()]).expect("base catalog");
        let active_catalog = LaneCatalog::new(lane_count, vec![primary.clone(), active_lane])
            .expect("active catalog");
        let replacement_catalog = LaneCatalog::new(lane_count, vec![primary, replacement_lane])
            .expect("replacement catalog");
        let base = RuntimeLaneConfig::from_catalog(&base_catalog);
        let active = RuntimeLaneConfig::from_catalog(&active_catalog);
        let replacement = RuntimeLaneConfig::from_catalog(&replacement_catalog);
        let base_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x51; Hash::LENGTH]))]);
        let active_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x52; Hash::LENGTH])),
        ]);
        let replacement_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x53; Hash::LENGTH])),
        ]);
        let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let active_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 4)]);
        let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 5)]);
        let kura = open_kura(&root, &base);
        kura.apply_lane_geometry_transition(
            &base,
            &active,
            &base_incarnations,
            &active_incarnations,
            &base_activations,
            &active_activations,
            &BTreeSet::new(),
        )
        .expect("create replaceable lane");
        kura.mark_lane_geometry_catalog_published(
            &active,
            &active_incarnations,
            &active_activations,
            None,
        )
        .expect("publish replaceable lane");
        kura.apply_lane_geometry_transition(
            &active,
            &replacement,
            &active_incarnations,
            &replacement_incarnations,
            &active_activations,
            &replacement_activations,
            &BTreeSet::from([LaneId::new(1)]),
        )
        .expect("apply replacement before simulated Intent crash");

        let mut journal = kura
            .read_lane_geometry_journal()
            .expect("replacement journal");
        journal.records[1].phase = LaneGeometryPhase::Intent;
        let operation = journal.records[1].operations[0].clone();
        kura.write_lane_geometry_journal(&journal)
            .expect("restore the pre-files-applied Intent frontier");
        let updated = operation.updated.as_ref().expect("updated binding");
        let updated_blocks = kura.binding_blocks_path(updated);
        let updated_merge = kura.binding_merge_path(updated);
        let unpublished_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("unpublished blocks");
        let unpublished_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("unpublished merge");
        kura.seal_geometry_pair_move(
            updated,
            &updated_blocks,
            &updated_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("seal Intent rollback before its block half");
        kura.move_geometry_path(&updated_blocks, &unpublished_blocks, true)
            .expect("simulate Intent rollback crash after moving only blocks");

        kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
            .expect("Intent retry must resume its own inverse merge half");
        assert!(!updated_blocks.exists());
        assert!(!updated_merge.exists());
        kura.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("Intent rollback retains the exact updated image");
        let previous = operation.previous.as_ref().expect("previous binding");
        kura.require_complete_geometry_binding_at(
            previous,
            &kura.binding_blocks_path(previous),
            &kura.binding_merge_path(previous),
        )
        .expect("previous replacement image restored");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[1].phase,
            LaneGeometryPhase::RolledBack
        );
    }

    #[test]
    fn same_path_replacement_rollback_preserves_old_merge_after_forward_half_archive() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = nonzero!(2_u32);
        let primary = ModelLaneConfig::default();
        let active_lane = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "same-path-replacement".to_owned(),
            ..ModelLaneConfig::default()
        };
        let replacement_lane = ModelLaneConfig {
            visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
            ..active_lane.clone()
        };
        let base_catalog =
            LaneCatalog::new(lane_count, vec![primary.clone()]).expect("base catalog");
        let active_catalog = LaneCatalog::new(lane_count, vec![primary.clone(), active_lane])
            .expect("active catalog");
        let replacement_catalog = LaneCatalog::new(lane_count, vec![primary, replacement_lane])
            .expect("replacement catalog");
        let base = RuntimeLaneConfig::from_catalog(&base_catalog);
        let active = RuntimeLaneConfig::from_catalog(&active_catalog);
        let replacement = RuntimeLaneConfig::from_catalog(&replacement_catalog);
        let base_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x41; Hash::LENGTH]))]);
        let active_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x42; Hash::LENGTH])),
        ]);
        let replacement_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x43; Hash::LENGTH])),
        ]);
        let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let active_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 4)]);
        let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 5)]);
        let kura = open_kura(&root, &base);
        kura.apply_lane_geometry_transition_at_height(
            &base,
            &active,
            &base_incarnations,
            &active_incarnations,
            &base_activations,
            &active_activations,
            &BTreeSet::new(),
            4,
        )
        .expect("create replaceable lane");
        kura.mark_lane_geometry_catalog_published(
            &active,
            &active_incarnations,
            &active_activations,
            None,
        )
        .expect("publish replaceable lane");

        let previous_bindings = kura
            .geometry_bindings(&active, &active_incarnations, &active_activations)
            .expect("active bindings");
        let updated_bindings = kura
            .geometry_bindings(
                &replacement,
                &replacement_incarnations,
                &replacement_activations,
            )
            .expect("replacement bindings");
        let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
        let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
        let previous_lineage_root = unscoped_lineage_root(&previous_bindings);
        let updated_lineage_root = unscoped_lineage_root(&updated_bindings);
        let mut journal = kura.read_lane_geometry_journal().expect("active journal");
        let transition_sequence = journal.records[0]
            .transition_sequence
            .checked_add(1)
            .expect("transition sequence");
        let transition_height = 5;
        let transition_id = geometry_transition_id(
            transition_sequence,
            transition_height,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        );
        let operations = kura
            .build_geometry_operations(
                transition_id,
                &previous_bindings,
                &updated_bindings,
                &BTreeSet::from([LaneId::new(1)]),
            )
            .expect("same-path replacement operation");
        let operation = operations[0].clone();
        let previous = operation.previous.as_ref().expect("previous binding");
        let updated = operation.updated.as_ref().expect("updated binding");
        assert_eq!(previous.blocks_path, updated.blocks_path);
        assert_eq!(previous.merge_path, updated.merge_path);
        journal.records.push(LaneGeometryIntent {
            transition_id,
            transition_sequence,
            transition_height,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
            previous_bindings,
            updated_bindings,
            phase: LaneGeometryPhase::Intent,
            operations,
        });
        kura.write_lane_geometry_journal(&journal)
            .expect("persist replacement intent");

        let previous_blocks = kura.binding_blocks_path(previous);
        let previous_merge = kura.binding_merge_path(previous);
        let archived_blocks = kura
            .resolve_relative_path(&operation.archived_blocks_path)
            .expect("archived blocks");
        let archived_merge = kura
            .resolve_relative_path(&operation.archived_merge_path)
            .expect("archived merge");
        let unpublished_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("unpublished merge");
        let unpublished_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("unpublished blocks");
        let sentinel = b"old-merge-half-must-remain-live";
        fs::write(&previous_merge, sentinel).expect("write old merge sentinel");
        kura.seal_geometry_pair_move(
            previous,
            &previous_blocks,
            &previous_merge,
            &archived_blocks,
            &archived_merge,
        )
        .expect("seal previous archive move before its block half");
        kura.move_geometry_path(&previous_blocks, &archived_blocks, true)
            .expect("simulate crash after archiving only old blocks");
        assert!(!previous_blocks.exists());
        assert!(previous_merge.is_file());
        assert!(!archived_merge.exists());

        kura.recover_lane_geometry_journal_at_height(
            &active,
            &active_incarnations,
            &active_activations,
            4,
        )
        .expect("rollback must recognize the shared live merge as the old half");
        assert!(previous_blocks.is_dir());
        assert_eq!(
            fs::read(&previous_merge).expect("old merge restored"),
            sentinel
        );
        kura.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("rollback retains an authenticated empty replacement image");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[1].phase,
            LaneGeometryPhase::RolledBack
        );
    }

    #[test]
    fn recovery_distinguishes_repeated_catalogs_by_retained_lineage_root() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (first_incarnations, first_activations) = extended_geometry();
        let mut second_incarnations = first_incarnations.clone();
        second_incarnations.insert(LaneId::new(1), Hash::prehashed([0x44; Hash::LENGTH]));
        let mut second_activations = first_activations.clone();
        second_activations.insert(LaneId::new(1), 10);
        let lineage_initial = Hash::new(b"lineage:initial:never-seen");
        let lineage_first_active = Hash::new(b"lineage:first:active");
        let lineage_first_retired = Hash::new(b"lineage:first:retired");
        let lineage_second_active = Hash::new(b"lineage:second:active");
        let lineage_second_retired = Hash::new(b"lineage:second:retired");
        let kura = open_kura(&root, &initial);

        kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
            &initial,
            &extended,
            &initial_incarnations,
            &first_incarnations,
            &initial_activations,
            &first_activations,
            lineage_initial,
            lineage_first_active,
            &BTreeSet::new(),
            9,
        )
        .expect("create first lane incarnation");
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &extended,
            &first_incarnations,
            &first_activations,
            Hash::new(b"lineage:first:wrong"),
            None,
        )
        .expect_err("publication must reject a mismatched retained-lineage root");
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("unpublished rooted journal")
                .records[0]
                .phase,
            LaneGeometryPhase::FilesApplied
        );
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &extended,
            &first_incarnations,
            &first_activations,
            lineage_first_active,
            None,
        )
        .expect("publish first active lineage");
        kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
            &extended,
            &initial,
            &first_incarnations,
            &initial_incarnations,
            &first_activations,
            &initial_activations,
            lineage_first_active,
            lineage_first_retired,
            &BTreeSet::new(),
            10,
        )
        .expect("retire first lane incarnation");
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            lineage_first_retired,
            None,
        )
        .expect("publish first retired lineage");
        kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
            &initial,
            &extended,
            &initial_incarnations,
            &second_incarnations,
            &initial_activations,
            &second_activations,
            lineage_first_retired,
            lineage_second_active,
            &BTreeSet::new(),
            10,
        )
        .expect("create second lane incarnation");
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &extended,
            &second_incarnations,
            &second_activations,
            lineage_second_active,
            None,
        )
        .expect("publish second active lineage");

        let lane1 = extended.entry(LaneId::new(1)).expect("lane one");
        kura.recover_lane_geometry_journal_before_transition_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            lineage_first_retired,
            10,
        )
        .expect("recover first retired lineage while second incarnation is live");
        assert!(!lane1.blocks_dir(&root).exists());
        kura.recover_lane_geometry_journal_at_height_with_lineage_root(
            &extended,
            &second_incarnations,
            &second_activations,
            10,
            lineage_second_active,
        )
        .expect("restore second active lineage after exact rooted rollback");
        assert!(lane1.blocks_dir(&root).exists());

        kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
            &extended,
            &initial,
            &second_incarnations,
            &initial_incarnations,
            &second_activations,
            &initial_activations,
            lineage_second_active,
            lineage_second_retired,
            &BTreeSet::new(),
            11,
        )
        .expect("retire second lane incarnation");
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            lineage_second_retired,
            None,
        )
        .expect("publish second retired lineage");

        let phases = kura
            .read_lane_geometry_journal()
            .expect("four-transition journal")
            .records
            .into_iter()
            .map(|record| record.phase)
            .collect::<Vec<_>>();
        assert_eq!(phases, vec![LaneGeometryPhase::CatalogPublished; 4]);

        kura.recover_lane_geometry_journal_before_transition_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            lineage_first_retired,
            10,
        )
        .expect("recover the first repeated retired catalog exactly");
        let phases = kura
            .read_lane_geometry_journal()
            .expect("rolled-back future lineage journal")
            .records
            .into_iter()
            .map(|record| record.phase)
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            vec![
                LaneGeometryPhase::CatalogPublished,
                LaneGeometryPhase::CatalogPublished,
                LaneGeometryPhase::RolledBack,
                LaneGeometryPhase::RolledBack,
            ]
        );

        let before_unknown = kura
            .read_lane_geometry_journal()
            .expect("journal before unknown root");
        kura.recover_lane_geometry_journal_before_transition_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            Hash::new(b"lineage:unknown"),
            10,
        )
        .expect_err("an unretained lineage root must fail closed");
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("journal after unknown root"),
            before_unknown,
            "failed recovery must not rewrite transition phases"
        );

        drop(kura);
        let restarted = open_kura(&root, &initial);
        restarted
            .recover_lane_geometry_journal_at_height_with_lineage_root(
                &initial,
                &initial_incarnations,
                &initial_activations,
                11,
                lineage_second_retired,
            )
            .expect("restart recovers the latest repeated retired catalog");
        assert!(
            restarted
                .read_lane_geometry_journal()
                .expect("restarted journal")
                .records
                .iter()
                .all(|record| record.phase == LaneGeometryPhase::CatalogPublished)
        );
    }

    #[test]
    fn files_applied_phase_rolls_forward_when_catalog_is_already_authoritative() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare transition");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::FilesApplied
        );

        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("recover post-catalog crash");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
    }

    #[test]
    fn recovery_publishes_uncertain_boundary_before_rolling_tail_forward() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = nonzero!(3_u32);
        let lane0 = ModelLaneConfig::default();
        let lane1 = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "frontier-one".to_owned(),
            ..ModelLaneConfig::default()
        };
        let lane2 = ModelLaneConfig {
            id: LaneId::new(2),
            alias: "frontier-two".to_owned(),
            ..ModelLaneConfig::default()
        };
        let base_catalog = LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("base catalog");
        let one_catalog = LaneCatalog::new(lane_count, vec![lane0.clone(), lane1.clone()])
            .expect("one-lane extension");
        let two_catalog =
            LaneCatalog::new(lane_count, vec![lane0, lane1, lane2]).expect("two-lane extension");
        let base = RuntimeLaneConfig::from_catalog(&base_catalog);
        let one = RuntimeLaneConfig::from_catalog(&one_catalog);
        let two = RuntimeLaneConfig::from_catalog(&two_catalog);
        let base_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x41; Hash::LENGTH]))]);
        let one_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x42; Hash::LENGTH])),
        ]);
        let two_incarnations = BTreeMap::from([
            (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), one_incarnations[&LaneId::new(1)]),
            (LaneId::new(2), Hash::prehashed([0x43; Hash::LENGTH])),
        ]);
        let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let one_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 6)]);
        let two_activations = BTreeMap::from([
            (LaneId::SINGLE, 0),
            (LaneId::new(1), 6),
            (LaneId::new(2), 7),
        ]);
        let kura = open_kura(&root, &base);
        kura.apply_lane_geometry_transition(
            &base,
            &one,
            &base_incarnations,
            &one_incarnations,
            &base_activations,
            &one_activations,
            &BTreeSet::new(),
        )
        .expect("apply first transition");
        kura.mark_lane_geometry_catalog_published(&one, &one_incarnations, &one_activations, None)
            .expect("publish first transition");
        kura.apply_lane_geometry_transition(
            &one,
            &two,
            &one_incarnations,
            &two_incarnations,
            &one_activations,
            &two_activations,
            &BTreeSet::new(),
        )
        .expect("apply second transition");
        kura.mark_lane_geometry_catalog_published(&two, &two_incarnations, &two_activations, None)
            .expect("publish second transition");

        let mut journal = kura
            .read_lane_geometry_journal()
            .expect("published journal");
        kura.apply_geometry_operations_rollback(
            &journal.records[1].operations,
            GeometryEvidencePolicy::RequireDurableEvidence,
        )
        .expect("place second transition behind the physical frontier");
        journal.records[0].phase = LaneGeometryPhase::FilesApplied;
        journal.records[1].phase = LaneGeometryPhase::RolledBack;
        kura.write_lane_geometry_journal(&journal)
            .expect("persist valid uncertain-plus-rolled-back frontier");

        kura.recover_lane_geometry_journal(&two, &two_incarnations, &two_activations)
            .expect("recovery must publish the uncertain boundary before the tail");
        let recovered = kura
            .read_lane_geometry_journal()
            .expect("recovered journal");
        assert_eq!(
            recovered
                .records
                .iter()
                .map(|record| record.phase)
                .collect::<Vec<_>>(),
            vec![
                LaneGeometryPhase::CatalogPublished,
                LaneGeometryPhase::CatalogPublished,
            ]
        );
        assert!(
            two.entry(LaneId::new(2))
                .expect("lane two")
                .blocks_dir(&root)
                .is_dir()
        );
    }

    #[test]
    fn recovery_rejects_stale_incarnation_marker() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare transition");

        let mut stale_incarnations = extended_incarnations.clone();
        stale_incarnations.insert(LaneId::new(1), Hash::prehashed([0x77; Hash::LENGTH]));
        let stale = kura
            .geometry_binding(
                extended.entry(LaneId::new(1)).expect("lane one"),
                &stale_incarnations,
                &extended_activations,
            )
            .expect("stale binding");
        kura.write_lane_marker(&stale).expect("write stale marker");

        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("stale incarnation marker must fail closed");
    }

    #[test]
    fn transition_rejects_reserved_archive_collision_before_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let previous_bindings = kura
            .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
            .expect("initial bindings");
        let updated_bindings = kura
            .geometry_bindings(&extended, &extended_incarnations, &extended_activations)
            .expect("updated bindings");
        let transition = geometry_transition_id(
            0,
            0,
            geometry_catalog_fingerprint(&previous_bindings),
            unscoped_lineage_root(&previous_bindings),
            geometry_catalog_fingerprint(&updated_bindings),
            unscoped_lineage_root(&updated_bindings),
        );
        let collision = root
            .join("retired/lane_geometry")
            .join(hex::encode(transition.as_ref()))
            .join("lane_0000000001/previous_blocks");
        fs::create_dir_all(&collision).expect("seed archive collision");

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect_err("archive collision must fail before applying files");
        assert!(
            !extended
                .entry(LaneId::new(1))
                .expect("lane one")
                .blocks_dir(&root)
                .exists()
        );
        assert!(
            kura.read_lane_geometry_journal()
                .expect("journal remains readable")
                .records
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn transition_rejects_symlink_lane_target() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside directory");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let target = extended
            .entry(LaneId::new(1))
            .expect("lane one")
            .blocks_dir(&root);
        fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
        symlink(&outside, &target).expect("seed symlink target");

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect_err("symlink target must fail closed");
        assert!(
            outside
                .read_dir()
                .expect("outside remains readable")
                .next()
                .is_none()
        );
    }

    #[test]
    fn snapshot_checkpoint_compacts_only_proven_history_and_preserves_latest_recovery() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);

        // Before checkpoint publication, both the old and current authoritative catalogs remain
        // recoverable from the retained transition chain.
        kura.recover_lane_geometry_journal_at_height(
            &fixture.extended,
            &fixture.extended_incarnations,
            &fixture.extended_activations,
            0,
        )
        .expect("old snapshot geometry remains recoverable before GC");
        kura.recover_lane_geometry_journal_at_height(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
            1,
        )
        .expect("restore current snapshot geometry");

        kura.refresh_disk_usage_bytes().expect("usage before GC");
        let summary = checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect("checkpoint current snapshot geometry");
        assert_eq!(summary.compacted_transitions, 2);
        assert_eq!(summary.removed_archive_roots, 2);
        assert!(
            summary.reclaimed_bytes
                >= u64::try_from(GC_PAYLOAD_LEN).expect("GC payload length fits u64")
        );
        assert!(!fixture.archive_root.exists());
        let journal = kura
            .read_lane_geometry_journal()
            .expect("compacted journal");
        assert!(journal.records.is_empty());
        assert!(journal.pending_archive_gc.is_empty());
        assert_eq!(
            journal
                .checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.catalog),
            Some(geometry_catalog_fingerprint(
                &kura
                    .geometry_bindings(
                        &fixture.initial,
                        &fixture.initial_incarnations,
                        &fixture.initial_activations,
                    )
                    .expect("initial bindings")
            ))
        );
        let cached_after = kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            cached_after,
            kura.kura_disk_usage_bytes().expect("exact usage scan")
        );

        assert_eq!(
            checkpoint_retired_geometry(&kura, &fixture, 20)
                .expect("checkpoint replay is idempotent"),
            LaneGeometryGcSummary::default()
        );
        kura.recover_lane_geometry_journal(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .expect("new snapshot remains recoverable");
        kura.recover_lane_geometry_journal(
            &fixture.extended,
            &fixture.extended_incarnations,
            &fixture.extended_activations,
        )
        .expect_err("checkpointed-away old snapshot must not synthesize empty lane storage");

        drop(kura);
        let restarted = open_kura(&root, &fixture.initial);
        restarted
            .recover_lane_geometry_journal(
                &fixture.initial,
                &fixture.initial_incarnations,
                &fixture.initial_activations,
            )
            .expect("restart recovers checkpoint-authoritative geometry");
    }

    #[test]
    fn public_checkpoint_requires_exact_durable_block_and_wsv_identity() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let (block_hash, state_hash) = durable_geometry_snapshot_identity(&kura, 20);

        kura.checkpoint_lane_geometry_after_durable_snapshot(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
            20,
            Some(HashOf::from_untyped_unchecked(Hash::new(b"wrong-block"))),
            state_hash,
            &BTreeMap::new(),
        )
        .expect_err("mismatched canonical block hash must retain rollback evidence");
        assert!(fixture.archive_root.exists());
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("retained journal")
                .records
                .len(),
            2
        );

        kura.checkpoint_lane_geometry_after_durable_snapshot(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
            20,
            Some(block_hash),
            Hash::new(b"wrong-state"),
            &BTreeMap::new(),
        )
        .expect_err("mismatched canonical state hash must retain rollback evidence");
        assert!(fixture.archive_root.exists());

        let summary = kura
            .checkpoint_lane_geometry_after_durable_snapshot(
                &fixture.initial,
                &fixture.initial_incarnations,
                &fixture.initial_activations,
                20,
                Some(block_hash),
                state_hash,
                &BTreeMap::new(),
            )
            .expect("exact durable snapshot identity permits GC");
        assert_eq!(summary.compacted_transitions, 2);
        assert_eq!(summary.removed_archive_roots, 2);
    }

    #[test]
    fn pending_gc_rejoins_checkpoint_to_current_canonical_wsv_before_deletion() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("leave a durable pending deletion intent");
        let height = NonZeroUsize::new(20).expect("non-zero height");
        let block_hash = kura
            .get_durable_block_hash(height)
            .expect("durable block hash");
        let original_state_hash = kura
            .wsv_checkpoint(20)
            .expect("read WSV checkpoint")
            .expect("WSV checkpoint exists")
            .state_hash();
        kura.store_wsv_checkpoint(20, block_hash, Hash::new(b"forked-state"))
            .expect("replace WSV checkpoint for adversarial test");

        kura.resume_proven_lane_geometry_archive_gc()
            .expect_err("changed WSV identity must block replayed deletion");
        assert!(fixture.archive_root.exists());
        assert!(
            !kura
                .read_lane_geometry_journal()
                .expect("pending journal")
                .pending_archive_gc
                .is_empty()
        );

        kura.store_wsv_checkpoint(20, block_hash, original_state_hash)
            .expect("restore authoritative WSV checkpoint");
        let resumed = kura
            .resume_proven_lane_geometry_archive_gc()
            .expect("matching canonical WSV resumes deletion");
        assert_eq!(resumed.removed_archive_roots, 2);
        assert!(!fixture.archive_root.exists());
    }

    #[test]
    fn pending_gc_rejects_ahead_missing_and_unbound_checkpoint_metadata() {
        for case in ["ahead", "missing", "unbound"] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(format!("kura-{case}"));
            let kura = open_kura(&root, &initial_and_extended_configs().0);
            let fixture = prepare_retired_geometry_archive(&kura, &root);
            kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
            checkpoint_retired_geometry(&kura, &fixture, 20)
                .expect_err("leave a durable pending deletion intent");
            let mut journal = kura.read_lane_geometry_journal().expect("pending journal");
            match case {
                "ahead" => {
                    let checkpoint = journal.checkpoint.as_mut().expect("checkpoint");
                    checkpoint.snapshot_height = 21;
                    checkpoint.snapshot_block_hash =
                        Some(HashOf::from_untyped_unchecked(Hash::new(b"ahead-block")));
                    checkpoint.snapshot_state_hash = Hash::new(b"ahead-state");
                    checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
                }
                "missing" => journal.checkpoint = None,
                "unbound" => {
                    let checkpoint = journal.checkpoint.as_mut().expect("checkpoint");
                    checkpoint.pending_archive_gc_root = Some(Hash::new(b"wrong-gc-root"));
                    checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
                }
                _ => unreachable!(),
            }
            fs::write(kura.lane_geometry_journal_path(), journal.encode())
                .expect("persist adversarial journal");

            kura.resume_proven_lane_geometry_archive_gc()
                .expect_err("invalid pending checkpoint metadata must fail closed");
            assert!(fixture.archive_root.exists());
        }
    }

    #[test]
    fn checkpoint_rejects_stale_height_and_lane_incarnation_aba() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        checkpoint_retired_geometry(&kura, &fixture, 20).expect("initial checkpoint");
        checkpoint_retired_geometry(&kura, &fixture, 19)
            .expect_err("older snapshot checkpoint must fail closed");

        let mut recreated_incarnations = fixture.extended_incarnations.clone();
        recreated_incarnations.insert(LaneId::new(1), Hash::prehashed([0x33; Hash::LENGTH]));
        let mut recreated_activations = fixture.extended_activations.clone();
        recreated_activations.insert(LaneId::new(1), 21);
        kura.apply_lane_geometry_transition_at_height(
            &fixture.initial,
            &fixture.extended,
            &fixture.initial_incarnations,
            &recreated_incarnations,
            &fixture.initial_activations,
            &recreated_activations,
            &BTreeSet::new(),
            21,
        )
        .expect("recreate lane id with fresh incarnation");
        kura.mark_lane_geometry_catalog_published(
            &fixture.extended,
            &recreated_incarnations,
            &recreated_activations,
            None,
        )
        .expect("publish recreated lane");

        let stale_bindings = kura
            .geometry_bindings(
                &fixture.extended,
                &fixture.extended_incarnations,
                &fixture.extended_activations,
            )
            .expect("stale bindings");
        let (block_hash, state_hash) = durable_geometry_snapshot_identity(&kura, 30);
        let stale_lineage_root = unscoped_lineage_root(&stale_bindings);
        kura.checkpoint_lane_geometry_with_proven_snapshot(
            stale_bindings,
            stale_lineage_root,
            30,
            Some(block_hash),
            state_hash,
            Vec::new(),
        )
        .expect_err("same lane id with an old incarnation is not a reachable checkpoint");

        let recreated_bindings = kura
            .geometry_bindings(
                &fixture.extended,
                &recreated_incarnations,
                &recreated_activations,
            )
            .expect("recreated bindings");
        let recreated_lineage_root = unscoped_lineage_root(&recreated_bindings);
        let summary = kura
            .checkpoint_lane_geometry_with_proven_snapshot(
                recreated_bindings,
                recreated_lineage_root,
                30,
                Some(block_hash),
                state_hash,
                Vec::new(),
            )
            .expect("fresh incarnation checkpoint");
        assert_eq!(summary.compacted_transitions, 1);
    }

    #[test]
    fn geometry_gc_crash_boundaries_replay_safely_after_restart() {
        for stage in [
            GC_FAIL_AFTER_COMPACTION_INTENT,
            GC_FAIL_AFTER_ARCHIVE_QUARANTINE,
            GC_FAIL_AFTER_ARCHIVE_DELETION,
            GC_FAIL_AFTER_COMPLETION,
        ] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(format!("kura-stage-{stage}"));
            let kura = open_kura(&root, &initial_and_extended_configs().0);
            let fixture = prepare_retired_geometry_archive(&kura, &root);
            let transition_roots = kura
                .read_lane_geometry_journal()
                .expect("journal before GC")
                .records
                .iter()
                .map(|record| {
                    root.join("retired/lane_geometry")
                        .join(hex::encode(record.transition_id.as_ref()))
                })
                .collect::<Vec<_>>();
            assert_eq!(transition_roots.len(), 2);
            assert!(transition_roots.iter().all(|archive| archive.exists()));
            let first_archive = &transition_roots[0];
            let quarantine = first_archive
                .parent()
                .expect("archive parent")
                .join(format!(
                    "{GC_QUARANTINE_PREFIX}{}",
                    first_archive
                        .file_name()
                        .expect("transition id")
                        .to_string_lossy()
                ));
            kura.fail_next_lane_geometry_gc_at_stage_for_test(stage);
            checkpoint_retired_geometry(&kura, &fixture, 20)
                .expect_err("injected GC boundary must interrupt acknowledgement");
            let after_failure = kura
                .read_lane_geometry_journal()
                .expect("journal after crash");
            assert!(after_failure.records.is_empty());
            if stage == GC_FAIL_AFTER_COMPACTION_INTENT {
                assert!(transition_roots.iter().all(|archive| archive.exists()));
                assert!(!quarantine.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else if stage == GC_FAIL_AFTER_ARCHIVE_QUARANTINE {
                assert!(!first_archive.exists());
                assert!(fixture.archive_root.exists());
                assert!(quarantine.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else if stage == GC_FAIL_AFTER_ARCHIVE_DELETION {
                assert!(transition_roots.iter().all(|archive| !archive.exists()));
                assert!(!quarantine.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else {
                assert!(transition_roots.iter().all(|archive| !archive.exists()));
                assert!(!quarantine.exists());
                assert!(after_failure.pending_archive_gc.is_empty());
            }

            drop(kura);
            let restarted = open_kura(&root, &fixture.initial);
            restarted
                .recover_lane_geometry_journal(
                    &fixture.initial,
                    &fixture.initial_incarnations,
                    &fixture.initial_activations,
                )
                .expect("restart completes or observes completed GC");
            let recovered = restarted
                .read_lane_geometry_journal()
                .expect("recovered journal");
            assert!(recovered.pending_archive_gc.is_empty());
            assert!(transition_roots.iter().all(|archive| !archive.exists()));
            assert!(!quarantine.exists());
        }
    }

    #[test]
    fn storage_budget_purge_only_resumes_snapshot_proven_geometry_gc() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("leave durable pending-GC intent");
        assert!(fixture.archive_root.exists());

        assert!(
            kura.purge_retired_segments(),
            "budget purge resumes only the already-proven archive deletion"
        );
        assert!(!fixture.archive_root.exists());
        assert!(
            kura.read_lane_geometry_journal()
                .expect("journal after budget purge")
                .pending_archive_gc
                .is_empty()
        );
        assert_eq!(
            kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
            kura.kura_disk_usage_bytes()
                .expect("exact usage after purge")
        );
    }

    #[test]
    fn storage_budget_purge_never_deletes_uncheckpointed_geometry_by_age_or_pressure() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let sentinel = fixture
            .archive_root
            .join("lane_0000000001/previous_blocks/gc-payload.norito");
        assert!(sentinel.exists());

        let _ = kura.purge_retired_segments();
        assert!(fixture.archive_root.exists());
        assert_eq!(
            fs::read(sentinel).expect("uncheckpointed archive retained"),
            [0xA5; GC_PAYLOAD_LEN]
        );
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("retained recovery journal")
                .records
                .len(),
            2
        );
    }

    #[cfg(unix)]
    #[test]
    fn geometry_sidecar_temp_symlink_and_regular_collision_fail_without_clobbering() {
        use std::os::unix::fs::symlink;

        for collision_kind in ["symlink", "regular"] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(format!("kura-{collision_kind}"));
            let (initial, extended) = initial_and_extended_configs();
            let (initial_incarnations, initial_activations) = initial_geometry();
            let (extended_incarnations, extended_activations) = extended_geometry();
            let kura = open_kura(&root, &initial);
            let collision = root.join(JOURNAL_TEMP_FILE_NAME);
            let outside = temp.path().join("operator-data");
            fs::write(&outside, b"operator-owned").expect("outside sentinel");
            if collision_kind == "symlink" {
                symlink(&outside, &collision).expect("journal temp symlink");
            } else {
                fs::write(&collision, b"operator-owned").expect("journal temp collision");
            }

            kura.apply_lane_geometry_transition(
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
                &BTreeSet::new(),
            )
            .expect_err("unsafe or unrelated temp collision must fail closed");
            assert_eq!(
                fs::read(&outside).expect("outside retained"),
                b"operator-owned"
            );
            if collision_kind == "regular" {
                assert_eq!(
                    fs::read(&collision).expect("regular collision retained"),
                    b"operator-owned"
                );
            }
        }
    }

    #[test]
    fn geometry_inode_identity_detects_path_replacement() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let path = root.join("inode-guard.norito");
        fs::write(&path, b"first").expect("first inode");
        let identity = kura
            .geometry_path_identity(&path, false)
            .expect("capture first inode");
        fs::rename(&path, root.join("inode-guard.old")).expect("move first inode");
        fs::write(&path, b"second").expect("replacement inode");
        kura.require_geometry_path_identity(&path, false, identity)
            .expect_err("replacement inode must not pass identity revalidation");
    }

    #[test]
    fn geometry_gc_rejects_preexisting_quarantine_collision() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
        checkpoint_retired_geometry(&kura, &fixture, 20).expect_err("leave pending deletion");
        let quarantine = fixture
            .archive_root
            .parent()
            .expect("archive parent")
            .join(format!(
                "{GC_QUARANTINE_PREFIX}{}",
                fixture
                    .archive_root
                    .file_name()
                    .expect("transition id")
                    .to_string_lossy()
            ));
        fs::create_dir(&quarantine).expect("quarantine collision");
        fs::write(quarantine.join("operator-data"), b"retain").expect("collision sentinel");

        kura.resume_proven_lane_geometry_archive_gc()
            .expect_err("root plus quarantine collision must fail closed");
        assert!(fixture.archive_root.exists());
        assert_eq!(
            fs::read(quarantine.join("operator-data")).expect("collision retained"),
            b"retain"
        );
    }

    #[test]
    fn geometry_gc_rejects_unauthenticated_archive_collision_without_deleting_it() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let collision = fixture.archive_root.join("operator-data.txt");
        fs::write(&collision, b"must not delete").expect("seed unauthenticated collision");

        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("unexpected archive content must fail closed");
        assert_eq!(
            fs::read(&collision).expect("collision retained"),
            b"must not delete"
        );
        let journal = kura.read_lane_geometry_journal().expect("pending journal");
        assert!(journal.records.is_empty());
        assert!(!journal.pending_archive_gc.is_empty());

        fs::remove_file(&collision).expect("operator resolves collision");
        let resumed = kura
            .resume_proven_lane_geometry_archive_gc()
            .expect("resume proven GC after repair");
        assert_eq!(resumed.removed_archive_roots, 1);
    }

    #[test]
    fn geometry_gc_pins_unmerged_autonomous_work_and_preserves_global_claim_evidence() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let lane_artifacts = fixture
            .archive_root
            .join("lane_0000000001/previous_blocks")
            .join(LANE_ARTIFACTS_DIR_NAME);
        fs::create_dir_all(&lane_artifacts).expect("archived lane artifact directory");
        let autonomous_data = lane_artifacts.join(AUTONOMOUS_LANE_BLOCKS_DATA_FILE);
        let autonomous_index = lane_artifacts.join(AUTONOMOUS_LANE_BLOCKS_INDEX_FILE);
        fs::write(&autonomous_data, b"unmerged autonomous payload")
            .expect("autonomous payload sidecar");
        fs::write(
            &autonomous_index,
            SidecarIndexEntry {
                offset: 0,
                len: u64::try_from(b"unmerged autonomous payload".len()).expect("payload length"),
            }
            .to_bytes(),
        )
        .expect("autonomous payload index");
        let claim = root
            .join("blocks/autonomous_entrypoint_claims_ff")
            .join("claim.norito");
        fs::create_dir_all(claim.parent().expect("claim parent")).expect("claim directory");
        fs::write(
            &claim,
            b"reservation/entrypoint claim outside retired geometry",
        )
        .expect("global claim sentinel");

        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("unmerged autonomous sidecar must pin retired geometry");
        assert!(fixture.archive_root.exists());
        assert!(
            !kura
                .read_lane_geometry_journal()
                .expect("pinned pending journal")
                .pending_archive_gc
                .is_empty()
        );

        // Model an operator/recovery worker discarding an uncertified local proposal. Remove the
        // fixture-created directory as well as its sidecars so the archived block image exactly
        // matches the durable move seal again. Once no certified or autonomous work remains, the
        // already-proven snapshot may release storage.
        fs::remove_file(autonomous_data).expect("remove uncertified payload");
        fs::remove_file(autonomous_index).expect("remove uncertified index");
        fs::remove_dir(lane_artifacts).expect("restore sealed archived block image");
        let resumed = kura
            .resume_proven_lane_geometry_archive_gc()
            .expect("empty retired work set releases after repair");
        assert_eq!(resumed.removed_archive_roots, 1);
        assert_eq!(
            fs::read(&claim).expect("global claim evidence retained"),
            b"reservation/entrypoint claim outside retired geometry"
        );
    }

    #[test]
    fn geometry_gc_pins_certified_work_without_a_durable_merge_receipt() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let lane_id = LaneId::new(1);
        let incarnation = fixture.extended_incarnations[&lane_id];
        let dataspace_id = fixture
            .extended
            .entry(lane_id)
            .expect("retired lane")
            .dataspace_id;
        let certified = certified_geometry_lane_block(lane_id, dataspace_id, incarnation, 1);
        let descriptor = &certified.proposal.descriptor;
        let archived_blocks = fixture.archive_root.join("lane_0000000001/previous_blocks");
        let lane_artifacts = archived_blocks.join(LANE_ARTIFACTS_DIR_NAME);
        fs::create_dir_all(&lane_artifacts).expect("archived lane artifacts");
        let payload = certified
            .encode_framed()
            .expect("encode certified lane block");
        fs::write(
            lane_artifacts.join(CERTIFIED_LANE_BLOCKS_DATA_FILE),
            &payload,
        )
        .expect("certified data sidecar");
        fs::write(
            lane_artifacts.join(CERTIFIED_LANE_BLOCKS_INDEX_FILE),
            SidecarIndexEntry {
                offset: 0,
                len: u64::try_from(payload.len()).expect("payload length"),
            }
            .to_bytes(),
        )
        .expect("certified index sidecar");
        let journal = kura.read_lane_geometry_journal().expect("geometry journal");
        let binding = journal
            .records
            .last()
            .expect("retirement transition")
            .operations
            .iter()
            .find_map(|operation| {
                (operation.lane_id == lane_id)
                    .then_some(operation.previous.as_ref())
                    .flatten()
            })
            .expect("retired lane binding")
            .clone();
        let carrier_hash = HashOf::from_untyped_unchecked(Hash::new(b"carrier"));
        let release = LaneGeometryMergeRelease {
            lane_id,
            dataspace_id,
            lane_incarnation: incarnation,
            lane_block_height: descriptor.lane_block_height,
            application_block_height: 20,
            application_block_hash: carrier_hash,
            merge_entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"merge-entry")),
            merge_epoch_id: 7,
            source_bundle_hash: Hash::new(b"source-bundle"),
            batch_identity_hash: Hash::new(b"batch-identity"),
            batch_hash: Hash::new(b"batch"),
            lane_execution_hash: Hash::new(b"lane-execution"),
            marker_set_root: Hash::new(b"markers"),
            receipt_hash: Hash::new(b"receipt"),
        };

        let error = kura
            .ensure_archived_lane_work_released(&archived_blocks, &binding, &[release])
            .expect_err("a merge release without its durable receipt must pin the archive");
        assert_geometry_io_error(
            &error,
            ErrorKind::WouldBlock,
            "retired lane merge application receipt is missing or malformed",
        );
        let Error::IO(_, path) = &error else {
            unreachable!("assert_geometry_io_error established the error variant")
        };
        assert_eq!(path, &kura.lane_geometry_journal_path());
        assert!(fixture.archive_root.exists());
    }

    #[test]
    fn partial_multi_archive_gc_retains_intent_and_repairs_disk_accounting_on_resume() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);

        let mut recreated_incarnations = fixture.extended_incarnations.clone();
        recreated_incarnations.insert(LaneId::new(1), Hash::prehashed([0x44; Hash::LENGTH]));
        let mut recreated_activations = fixture.extended_activations.clone();
        recreated_activations.insert(LaneId::new(1), 10);
        kura.apply_lane_geometry_transition(
            &fixture.initial,
            &fixture.extended,
            &fixture.initial_incarnations,
            &recreated_incarnations,
            &fixture.initial_activations,
            &recreated_activations,
            &BTreeSet::new(),
        )
        .expect("recreate retired lane");
        kura.mark_lane_geometry_catalog_published(
            &fixture.extended,
            &recreated_incarnations,
            &recreated_activations,
            None,
        )
        .expect("publish recreated lane");
        let recreated_blocks = fixture
            .extended
            .entry(LaneId::new(1))
            .expect("recreated lane")
            .blocks_dir(&root);
        fs::write(
            recreated_blocks.join("second-gc-payload.norito"),
            [0x5A; 53],
        )
        .expect("seed second archive payload");
        kura.apply_lane_geometry_transition(
            &fixture.extended,
            &fixture.initial,
            &recreated_incarnations,
            &fixture.initial_incarnations,
            &recreated_activations,
            &fixture.initial_activations,
            &BTreeSet::new(),
        )
        .expect("retire recreated lane");
        kura.mark_lane_geometry_catalog_published(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
            None,
        )
        .expect("publish second retirement");

        let journal = kura.read_lane_geometry_journal().expect("four transitions");
        assert_eq!(journal.records.len(), 4);
        let second_archive = root
            .join("retired/lane_geometry")
            .join(hex::encode(journal.records[3].transition_id.as_ref()));
        let collision = second_archive.join("operator-data.txt");
        fs::write(&collision, b"retain until operator repair").expect("collision");
        kura.refresh_disk_usage_bytes()
            .expect("usage before partial GC");

        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("second archive collision interrupts a multi-root GC pass");
        assert!(
            !fixture.archive_root.exists(),
            "first proven root was deleted"
        );
        assert!(second_archive.exists(), "failing root remains intact");
        assert_eq!(
            fs::read(&collision).expect("collision retained"),
            b"retain until operator repair"
        );
        assert!(
            !kura
                .read_lane_geometry_journal()
                .expect("pending partial GC")
                .pending_archive_gc
                .is_empty()
        );
        let exact_after_partial = kura.kura_disk_usage_bytes().expect("exact partial usage");
        assert_eq!(
            kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
            exact_after_partial,
            "a failed partial pass must repair the live disk-usage cache to the exact retained tree"
        );

        fs::remove_file(&collision).expect("repair archive collision");
        let resumed = kura
            .resume_proven_lane_geometry_archive_gc()
            .expect("resume all exact pending roots");
        assert_eq!(resumed.removed_archive_roots, 1);
        assert!(!second_archive.exists());
        assert_eq!(
            kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
            kura.kura_disk_usage_bytes()
                .expect("exact usage after completed resume")
        );
    }

    #[cfg(unix)]
    #[test]
    fn geometry_gc_rejects_symlink_inside_archive_tree() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside.txt");
        fs::write(&outside, b"outside").expect("outside sentinel");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let archived_blocks = fixture.archive_root.join("lane_0000000001/previous_blocks");
        let link = archived_blocks.join("escape");
        symlink(&outside, &link).expect("seed archive symlink");

        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("archive symlink must fail closed");
        assert_eq!(fs::read(&outside).expect("outside retained"), b"outside");
        assert!(link.exists());
    }

    #[test]
    fn recovery_rejects_pre_release_journal_layout() {
        #[derive(Encode)]
        struct PreReleaseLaneGeometryJournal {
            version: u8,
            records: Vec<LaneGeometryIntent>,
        }

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        let pre_release = PreReleaseLaneGeometryJournal {
            version: 1,
            records: Vec::new(),
        };
        fs::write(kura.lane_geometry_journal_path(), pre_release.encode())
            .expect("write pre-release journal");

        kura.read_lane_geometry_journal()
            .expect_err("pre-release journal layout must fail closed");
    }

    #[test]
    fn recovery_rejects_corrupt_and_forged_journals() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        fs::write(kura.lane_geometry_journal_path(), b"not norito").expect("write corrupt journal");
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect_err("corrupt journal must fail closed");

        fs::remove_file(kura.lane_geometry_journal_path()).expect("remove corrupt journal");
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare valid journal");
        let valid = kura.read_lane_geometry_journal().expect("valid journal");
        let mut forged_root = valid.clone();
        forged_root.records[0].updated_lineage_root = Hash::new(b"forged-lineage-root");
        fs::write(kura.lane_geometry_journal_path(), forged_root.encode())
            .expect("write forged lineage root");
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("lineage-root tampering must invalidate the transition id");

        let mut forged_sequence = valid.clone();
        forged_sequence.records[0].transition_sequence = forged_sequence.records[0]
            .transition_sequence
            .checked_add(1)
            .expect("test transition sequence");
        fs::write(kura.lane_geometry_journal_path(), forged_sequence.encode())
            .expect("write forged transition sequence");
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("transition-sequence tampering must invalidate the transition id");

        let mut forged_height = valid.clone();
        forged_height.records[0].transition_height = forged_height.records[0]
            .transition_height
            .checked_add(1)
            .expect("test transition height");
        fs::write(kura.lane_geometry_journal_path(), forged_height.encode())
            .expect("write forged transition height");
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("transition-height tampering must invalidate the transition id");

        fs::write(kura.lane_geometry_journal_path(), valid.encode())
            .expect("restore valid journal");
        let mut forged = valid;
        forged.records[0].operations[0].archived_blocks_path = "../escape".to_owned();
        fs::write(kura.lane_geometry_journal_path(), forged.encode())
            .expect("write forged journal bytes");
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("forged archive path must fail closed");
    }

    #[test]
    fn recovery_rejects_noncontiguous_phase_frontiers() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let _fixture = prepare_retired_geometry_archive(&kura, &root);
        let valid = kura
            .read_lane_geometry_journal()
            .expect("two-transition published journal");
        assert_eq!(valid.records.len(), 2);

        for (_label, phases, expected_message) in [
            (
                "published-after-rollback",
                [
                    LaneGeometryPhase::RolledBack,
                    LaneGeometryPhase::CatalogPublished,
                ],
                "lane geometry journal phases do not form a durable applied frontier",
            ),
            (
                "multiple-uncertain-boundaries",
                [LaneGeometryPhase::Intent, LaneGeometryPhase::FilesApplied],
                "lane geometry journal has more than one uncertain transition boundary",
            ),
        ] {
            let mut forged = valid.clone();
            for (record, phase) in forged.records.iter_mut().zip(phases) {
                record.phase = phase;
            }
            fs::write(kura.lane_geometry_journal_path(), forged.encode())
                .expect("write phase-frontier forgery");
            let error = kura
                .read_lane_geometry_journal()
                .expect_err("impossible phase topology must fail closed");
            assert_geometry_io_error(&error, ErrorKind::InvalidData, expected_message);
        }
    }

    #[test]
    fn recovery_rejects_both_branch_v5_journal_layouts_without_migration() {
        #[derive(Encode)]
        struct HeightCursorJournalV5 {
            version: u8,
            configured_catalog_hash: Option<Hash>,
            configured_primary_binding: Option<LaneGeometryBinding>,
            checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
            pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
            records: Vec<LaneGeometryIntent>,
        }

        #[derive(Encode)]
        struct LineageJournalV5 {
            version: u8,
            configured_catalog_hash: Option<Hash>,
            // These containers are empty below, so their bytes exactly match the lineage
            // branch's checkpoint and transition container encodings.
            checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
            pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
            records: Vec<LaneGeometryIntent>,
        }

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let kura = open_kura(&root, &initial);
        let obsolete_layouts = [
            (
                "height-cursor v5",
                HeightCursorJournalV5 {
                    version: 5,
                    configured_catalog_hash: None,
                    configured_primary_binding: None,
                    checkpoint: None,
                    pending_archive_gc: Vec::new(),
                    records: Vec::new(),
                }
                .encode(),
            ),
            (
                "lineage v5",
                LineageJournalV5 {
                    version: 5,
                    configured_catalog_hash: Some(Hash::new(b"lineage-v5")),
                    checkpoint: None,
                    pending_archive_gc: Vec::new(),
                    records: Vec::new(),
                }
                .encode(),
            ),
        ];

        for (name, bytes) in obsolete_layouts {
            let journal_path = kura.lane_geometry_journal_path();
            fs::write(&journal_path, &bytes).expect("write obsolete v5 journal");

            let error = match kura.recover_lane_geometry_journal(
                &initial,
                &initial_incarnations,
                &initial_activations,
            ) {
                Ok(()) => panic!("{name} must not be migrated to journal v6"),
                Err(error) => error,
            };
            assert_eq!(
                fs::read(&journal_path).expect("read rejected v5 journal"),
                bytes,
                "recovery must leave the rejected {name} bytes untouched"
            );
            if name == "height-cursor v5" {
                assert_kura_io_error(
                    &error,
                    std::io::ErrorKind::InvalidData,
                    "unsupported lane geometry journal version 5; expected 6",
                );
            }
        }
    }

    #[test]
    fn recovery_rejects_prior_lane_geometry_checkpoint_version() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        checkpoint_retired_geometry(&kura, &fixture, 20).expect("create rooted checkpoint v4");
        let mut prior = kura
            .read_lane_geometry_journal()
            .expect("read rooted checkpoint journal");
        let checkpoint = prior.checkpoint.as_mut().expect("checkpoint exists");
        checkpoint.version = CHECKPOINT_VERSION - 1;
        checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
        fs::write(kura.lane_geometry_journal_path(), prior.encode())
            .expect("write prior-version checkpoint");

        let error = kura
            .recover_lane_geometry_journal(
                &fixture.initial,
                &fixture.initial_incarnations,
                &fixture.initial_activations,
            )
            .expect_err("checkpoint v3 must not be interpreted as rooted checkpoint v4");
        assert_kura_io_error(
            &error,
            std::io::ErrorKind::InvalidData,
            "lane geometry checkpoint commitment, catalog, height, block hash, or activation is invalid",
        );
    }

    #[test]
    fn configured_catalog_preflight_persists_baseline_before_any_lane_path() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured_a = configured_primary_catalog("crash-a");
        let configured_b = configured_primary_catalog("crash-b");
        let lane_config_a = RuntimeLaneConfig::from_catalog(&configured_a);
        let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
        let config = kura_config(&root);

        Kura::fail_after_configured_catalog_preflight_for_test(&root);
        let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
            .expect_err("injected crash must stop immediately after baseline establishment");
        assert!(matches!(
            error,
            Error::IO(ref source, _) if source.kind() == ErrorKind::Interrupted
        ));
        assert_lane_paths_absent(&root, &lane_config_a);
        let journal = decode_exact::<LaneGeometryJournal>(
            &fs::read(root.join(JOURNAL_FILE_NAME)).expect("durable baseline journal"),
        )
        .expect("decode durable baseline journal");
        assert_eq!(
            journal.configured_catalog_hash,
            Some(LaneLifecycleParameterV1::catalog_hash(&configured_a))
        );

        Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
            .expect_err("a reconstructed process must reject a different configured catalog");
        assert_lane_paths_absent(&root, &lane_config_b);

        Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
            .expect("the exact configured catalog must resume after the crash boundary");
    }

    #[cfg(unix)]
    #[test]
    fn configured_primary_preflight_rejects_block_path_symlink_before_external_write() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside-blocks");
        fs::create_dir_all(&outside).expect("outside directory");
        let configured = configured_primary_catalog("primary-block-symlink");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(
            &root,
            LaneLifecycleParameterV1::catalog_hash(&configured),
        )
        .expect("establish configured-catalog baseline");
        let blocks = lane_config.primary().blocks_dir(&root);
        fs::create_dir_all(blocks.parent().expect("block parent")).expect("block parent");
        symlink(&outside, &blocks).expect("configured primary block symlink");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("configured primary block symlink must fail before BlockStore opens it");
        assert!(blocks.is_symlink());
        assert_eq!(
            fs::read_dir(&outside).expect("outside directory").count(),
            0,
            "preflight rejection must not create block-store files outside the Kura root"
        );
        assert!(!lane_config.primary().merge_log_path(&root).exists());
    }

    #[cfg(unix)]
    #[test]
    fn configured_primary_preflight_rejects_merge_path_symlink_before_external_write() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside-merge.log");
        fs::write(&outside, b"operator-owned").expect("outside merge sentinel");
        let configured = configured_primary_catalog("primary-merge-symlink");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(
            &root,
            LaneLifecycleParameterV1::catalog_hash(&configured),
        )
        .expect("establish configured-catalog baseline");
        let merge = lane_config.primary().merge_log_path(&root);
        fs::create_dir_all(merge.parent().expect("merge parent")).expect("merge parent");
        symlink(&outside, &merge).expect("configured primary merge symlink");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err(
                "configured primary merge symlink must fail before MergeLedgerLog opens it",
            );
        assert!(merge.is_symlink());
        assert_eq!(
            fs::read(&outside).expect("outside sentinel"),
            b"operator-owned"
        );
        assert!(!lane_config.primary().blocks_dir(&root).exists());
    }

    #[cfg(unix)]
    #[test]
    fn configured_primary_preflight_rejects_core_block_file_symlinks_before_external_write() {
        use std::os::unix::fs::symlink;

        for file_name in [
            INDEX_FILE_NAME,
            DATA_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join("kura");
            let outside = temp.path().join(format!("outside-{file_name}"));
            fs::write(&outside, b"operator-owned-block-file").expect("outside sentinel");
            let configured = configured_primary_catalog("child-link");
            let lane_config = RuntimeLaneConfig::from_catalog(&configured);
            let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
            let incarnation = Hash::prehashed([0xA7; Hash::LENGTH]);
            let (kura, _) = Kura::new_with_configured_lane_catalog(
                &kura_config(&root),
                &lane_config,
                &configured,
            )
            .expect("open authenticated configured Kura");
            kura.establish_or_verify_configured_primary_geometry_anchor(
                lane_config.primary(),
                incarnation,
                baseline,
            )
            .expect("bind configured primary");
            drop(kura);

            let child = lane_config.primary().blocks_dir(&root).join(file_name);
            fs::remove_file(&child).expect("remove core block file before symlink injection");
            symlink(&outside, &child).expect("inject core block-file symlink");

            Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
                .expect_err(
                    "configured primary descendants must be rejected before BlockStore opens",
                );
            assert!(child.is_symlink());
            assert_eq!(
                fs::read(&outside).expect("outside sentinel retained"),
                b"operator-owned-block-file",
                "outside target changed for {file_name}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn configured_primary_preflight_rejects_root_sidecar_temp_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside-roster-temp");
        fs::write(&outside, b"operator-owned-roster-temp").expect("outside sentinel");
        let configured = configured_primary_catalog("root-sidecar-link");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
                .expect("open authenticated configured Kura");
        kura.establish_or_verify_configured_primary_geometry_anchor(
            lane_config.primary(),
            Hash::prehashed([0xA8; Hash::LENGTH]),
            baseline,
        )
        .expect("bind configured primary");
        drop(kura);
        let sidecar_temp = root.join("commit-rosters.norito.tmp");
        symlink(&outside, &sidecar_temp).expect("inject roster temp symlink");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("root sidecar temp symlink must fail before CommitRosterJournal opens");
        assert!(sidecar_temp.is_symlink());
        assert_eq!(
            fs::read(&outside).expect("outside sentinel retained"),
            b"operator-owned-roster-temp"
        );
    }

    #[test]
    fn configured_primary_preflight_rejects_foreign_marker_before_kura_reconciliation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("primary-marker");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
                .expect("open configured Kura");
        let incarnation = Hash::prehashed([0xA1; Hash::LENGTH]);
        kura.establish_or_verify_configured_primary_geometry_anchor(
            lane_config.primary(),
            incarnation,
            LaneLifecycleParameterV1::catalog_hash(&configured),
        )
        .expect("bind configured primary");
        let marker_path = lane_config
            .primary()
            .blocks_dir(&root)
            .join(MARKER_FILE_NAME);
        fs::write(
            &marker_path,
            LaneIncarnationMarker {
                version: MARKER_VERSION,
                lane_id: LaneId::SINGLE,
                incarnation: Hash::prehashed([0xA2; Hash::LENGTH]),
                activation_height: 0,
                move_target_blocks: None,
                move_target_merge: None,
                block_store_digest: Hash::prehashed([0xA4; Hash::LENGTH]),
                merge_log_digest: Hash::prehashed([0xA3; Hash::LENGTH]),
            }
            .encode(),
        )
        .expect("write foreign marker");
        drop(kura);

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("foreign configured-primary marker must fail before Kura reconciliation");
        let marker = decode_exact::<LaneIncarnationMarker>(
            &fs::read(&marker_path).expect("foreign marker retained"),
        )
        .expect("decode retained marker");
        assert_eq!(marker.incarnation, Hash::prehashed([0xA2; Hash::LENGTH]));
    }

    #[test]
    fn configured_catalog_preflight_rejects_nonzero_physical_primary_without_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let nonzero_root = temp.path().join("nonzero-primary");
        let nonzero_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![ModelLaneConfig {
                id: LaneId::new(1),
                alias: "not-physical-primary".to_owned(),
                ..ModelLaneConfig::default()
            }],
        )
        .expect("sparse nonzero-only catalog");
        let nonzero_config = RuntimeLaneConfig::from_catalog(&nonzero_catalog);
        let error = Kura::new_with_configured_lane_catalog(
            &kura_config(&nonzero_root),
            &nonzero_config,
            &nonzero_catalog,
        )
        .expect_err("authenticated Kura must require physical lane zero");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidInput,
            "authenticated configured catalog must contain physical primary lane zero",
        );
        assert!(!nonzero_root.exists());
    }

    #[test]
    fn configured_catalog_preflight_refuses_to_bind_a_nonpristine_root() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        fs::create_dir_all(&root).expect("seed Kura root");
        let sentinel = root.join("operator-ledger-data");
        fs::write(&sentinel, b"must-not-adopt-or-delete").expect("seed foreign ledger data");
        let configured = configured_primary_catalog("pristine-root-required");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);

        let error =
            Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
                .expect_err("a missing baseline must never bind an existing ledger root");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "cannot establish a configured-catalog baseline on a non-pristine Kura root",
        );
        assert_eq!(
            fs::read(&sentinel).expect("foreign data retained"),
            b"must-not-adopt-or-delete"
        );
        assert!(!root.join(JOURNAL_FILE_NAME).exists());
        assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
        assert_lane_paths_absent(&root, &lane_config);
    }

    #[test]
    fn configured_multilane_startup_defers_secondary_provisioning_to_geometry_journal() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let primary = ModelLaneConfig::default();
        let secondary = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "configured-secondary".to_owned(),
            ..ModelLaneConfig::default()
        };
        let initial_catalog = LaneCatalog::new(lane_count, vec![primary.clone()])
            .expect("configured startup base catalog");
        let configured_catalog = LaneCatalog::new(lane_count, vec![primary, secondary])
            .expect("configured startup two-lane catalog");
        let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
        let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x81; Hash::LENGTH]))]);
        let configured_incarnations = BTreeMap::from([
            (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x82; Hash::LENGTH])),
        ]);
        let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
        let secondary_entry = configured.entry(LaneId::new(1)).expect("secondary lane");
        let secondary_blocks = secondary_entry.blocks_dir(&root);
        let secondary_merge = secondary_entry.merge_log_path(&root);

        let (kura, _) = Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &configured,
            &configured_catalog,
        )
        .expect("open authenticated configured Kura");
        kura.establish_or_verify_configured_primary_geometry_anchor(
            initial.primary(),
            initial_incarnations[&LaneId::SINGLE],
            LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
        )
        .expect("bind configured primary before publishing the full catalog");
        assert!(
            !secondary_blocks.exists() && !secondary_merge.exists(),
            "authenticated Kura open must not precreate secondary storage without incarnation evidence"
        );
        assert!(
            kura.lane_storage_entry(LaneId::new(1)).is_err(),
            "authenticated Kura must not advertise an unowned secondary segment"
        );

        kura.apply_lane_geometry_transition(
            &initial,
            &configured,
            &initial_incarnations,
            &configured_incarnations,
            &initial_activations,
            &configured_activations,
            &BTreeSet::new(),
        )
        .expect("journal configured secondary-lane creation");
        kura.mark_lane_geometry_catalog_published(
            &configured,
            &configured_incarnations,
            &configured_activations,
            Some(LaneLifecycleParameterV1::catalog_hash(&configured_catalog)),
        )
        .expect("publish configured secondary-lane geometry");
        let secondary_binding = kura
            .geometry_bindings(
                &configured,
                &configured_incarnations,
                &configured_activations,
            )
            .expect("configured geometry bindings")
            .into_iter()
            .find(|binding| binding.lane_id == LaneId::new(1))
            .expect("secondary geometry binding");
        kura.require_lane_marker(&secondary_binding)
            .expect("secondary storage has the exact authoritative marker");
        assert!(secondary_merge.is_file());
        assert!(kura.lane_storage_entry(LaneId::new(1)).is_ok());

        drop(kura);
        let (reopened, _) = Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &configured,
            &configured_catalog,
        )
        .expect("reopen exact configured Kura");
        reopened
            .recover_lane_geometry_journal(
                &configured,
                &configured_incarnations,
                &configured_activations,
            )
            .expect("reopen authenticates published configured geometry");
        reopened
            .require_lane_marker(&secondary_binding)
            .expect("reopened secondary marker remains exact");

        fs::remove_dir_all(&secondary_blocks).expect("simulate loss of published secondary blocks");
        fs::remove_file(&secondary_merge).expect("simulate loss of published secondary merge log");
        let error = reopened
            .recover_lane_geometry_journal(
                &configured,
                &configured_incarnations,
                &configured_activations,
            )
            .expect_err("published configured secondary must never be silently recreated empty");
        assert_geometry_io_error(
            &error,
            ErrorKind::NotFound,
            "durable lane geometry evidence is missing; refusing to provision an empty replacement",
        );
        assert!(!secondary_blocks.exists());
        assert!(!secondary_merge.exists());
    }

    #[test]
    fn configured_multilane_startup_rejects_unjournaled_secondary_storage() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let primary = ModelLaneConfig::default();
        let secondary = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "unjournaled-secondary".to_owned(),
            ..ModelLaneConfig::default()
        };
        let initial_catalog = LaneCatalog::new(lane_count, vec![primary.clone()])
            .expect("configured startup base catalog");
        let configured_catalog = LaneCatalog::new(lane_count, vec![primary, secondary])
            .expect("configured startup two-lane catalog");
        let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
        let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x91; Hash::LENGTH]))]);
        let configured_incarnations = BTreeMap::from([
            (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
            (LaneId::new(1), Hash::prehashed([0x92; Hash::LENGTH])),
        ]);
        let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
        let secondary_blocks = configured
            .entry(LaneId::new(1))
            .expect("secondary lane")
            .blocks_dir(&root);
        Kura::establish_or_verify_configured_lane_catalog_baseline(
            &root,
            LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
        )
        .expect("establish the authenticated baseline before injecting foreign storage");
        fs::create_dir_all(&secondary_blocks).expect("seed unjournaled secondary directory");
        let sentinel = secondary_blocks.join("operator-sentinel");
        fs::write(&sentinel, b"must-not-adopt-or-delete").expect("seed unjournaled sentinel");

        let (kura, _) = Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &configured,
            &configured_catalog,
        )
        .expect("authenticated Kura open preserves unproven secondary path for diagnosis");
        let error = kura
            .apply_lane_geometry_transition(
                &initial,
                &configured,
                &initial_incarnations,
                &configured_incarnations,
                &initial_activations,
                &configured_activations,
                &BTreeSet::new(),
            )
            .expect_err("unjournaled secondary storage must not be adopted");
        assert_geometry_io_error(
            &error,
            ErrorKind::AlreadyExists,
            "lane storage already exists at a create target",
        );
        assert_eq!(
            fs::read(&sentinel).expect("unjournaled sentinel retained"),
            b"must-not-adopt-or-delete"
        );
        assert!(
            kura.read_lane_geometry_journal()
                .expect("configured baseline journal")
                .records
                .is_empty(),
            "rejection must precede geometry intent publication"
        );
    }

    #[test]
    fn configured_catalog_preflight_recovers_exact_first_start_temp() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        fs::create_dir_all(&root).expect("Kura root");
        let configured = configured_primary_catalog("temp-recovery");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let expected = LaneGeometryJournal {
            configured_catalog_hash: Some(LaneLifecycleParameterV1::catalog_hash(&configured)),
            ..LaneGeometryJournal::default()
        };
        fs::write(root.join(JOURNAL_TEMP_FILE_NAME), expected.encode())
            .expect("simulate synced first-start temp before hard-link promotion");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("reconstructed process must promote the exact baseline temp");
        assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
        let recovered = decode_exact::<LaneGeometryJournal>(
            &fs::read(root.join(JOURNAL_FILE_NAME)).expect("promoted baseline journal"),
        )
        .expect("decode promoted baseline journal");
        assert_eq!(recovered, expected);
    }

    #[test]
    fn configured_catalog_preflight_cleans_exact_startup_owned_hard_link_temp() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("link-recovery");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish baseline before simulated crash");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        fs::hard_link(&journal_path, root.join(JOURNAL_TEMP_FILE_NAME))
            .expect("simulate crash after durable hard-link promotion");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("exact startup-owned hard-link temp must be cleaned before lane storage opens");
        assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
        assert!(journal_path.is_file());
    }

    #[test]
    fn configured_catalog_preflight_rejects_unproven_restore_temp() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("restore-temp");
        let attempted = configured_primary_catalog("restore-must-not-open");
        let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        fs::copy(&journal_path, root.join(JOURNAL_RESTORE_TEMP_FILE_NAME))
            .expect("seed byte-identical but unowned restore temp");

        Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &attempted_lane_config,
            &configured,
        )
        .expect_err("byte equality does not prove restore-temp ownership");
        assert_lane_paths_absent(&root, &attempted_lane_config);
        assert!(root.join(JOURNAL_RESTORE_TEMP_FILE_NAME).is_file());
    }

    #[test]
    fn configured_catalog_preflight_discards_uncommitted_restore_temp() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("restore-temp");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        let authoritative = fs::read(&journal_path).expect("authoritative journal bytes");
        let root_identity = configured_catalog_store_root_identity(&root).expect("root identity");
        write_initial_configured_catalog_temp(
            &root,
            root_identity,
            &root.join(JOURNAL_RESTORE_TEMP_FILE_NAME),
            b"synced-but-uncommitted-restore-bytes",
        )
        .expect("simulate crash before restore-temp rename");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("the final journal is the sole restore commit point");
        assert!(!root.join(JOURNAL_RESTORE_TEMP_FILE_NAME).exists());
        assert_eq!(
            fs::read(&journal_path).expect("journal retained"),
            authoritative
        );
    }

    #[test]
    fn configured_catalog_preflight_discards_different_uncommitted_publication_temp() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("publication-temp");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        let authoritative = fs::read(&journal_path).expect("authoritative journal bytes");
        let different = LaneGeometryJournal {
            configured_catalog_hash: Some(Hash::new(b"different-uncommitted-catalog")),
            ..LaneGeometryJournal::default()
        }
        .encode();
        let root_identity = configured_catalog_store_root_identity(&root).expect("root identity");
        write_initial_configured_catalog_temp(
            &root,
            root_identity,
            &root.join(JOURNAL_TEMP_FILE_NAME),
            &different,
        )
        .expect("simulate crash before publication-temp rename");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("the final journal is the sole publication commit point");
        assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
        assert_eq!(
            fs::read(&journal_path).expect("journal retained"),
            authoritative
        );
    }

    #[cfg(unix)]
    #[test]
    fn configured_catalog_preflight_rejects_reserved_temp_symlink_without_touching_target() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside-temp-target");
        fs::write(&outside, b"operator-owned").expect("outside sentinel");
        let configured = configured_primary_catalog("reserved-temp-symlink");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish authoritative baseline");
        let reserved = root.join(JOURNAL_TEMP_FILE_NAME);
        symlink(&outside, &reserved).expect("reserved temp symlink");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("reserved temp symlinks must never be deleted or followed");
        assert!(reserved.is_symlink());
        assert_eq!(
            fs::read(&outside).expect("outside sentinel"),
            b"operator-owned"
        );
    }

    #[test]
    fn configured_catalog_preflight_rejects_tampered_v6_structure_before_lane_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("structural-baseline");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish valid v6 baseline");

        let journal_path = root.join(JOURNAL_FILE_NAME);
        let mut journal = decode_exact::<LaneGeometryJournal>(
            &fs::read(&journal_path).expect("read valid baseline journal"),
        )
        .expect("decode valid baseline journal");
        let previous_catalog = Hash::new(b"forged previous catalog");
        let previous_lineage_root = Hash::new(b"forged previous lineage");
        let updated_catalog = Hash::new(b"forged updated catalog");
        let updated_lineage_root = Hash::new(b"forged updated lineage");
        journal.records.push(LaneGeometryIntent {
            transition_id: geometry_transition_id(
                0,
                0,
                previous_catalog,
                previous_lineage_root,
                updated_catalog,
                updated_lineage_root,
            ),
            transition_sequence: 0,
            transition_height: 0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
            previous_bindings: Vec::new(),
            updated_bindings: Vec::new(),
            phase: LaneGeometryPhase::Intent,
            operations: Vec::new(),
        });
        fs::write(&journal_path, journal.encode()).expect("write decodable structural forgery");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("correct baseline must not mask a malformed v6 journal");
        assert_lane_paths_absent(&root, &lane_config);
    }

    #[test]
    fn configured_catalog_preflight_rejects_version_mismatch_before_lane_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("version-baseline");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish valid v6 baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        let mut journal = decode_exact::<LaneGeometryJournal>(
            &fs::read(&journal_path).expect("read valid baseline journal"),
        )
        .expect("decode valid baseline journal");
        journal.version = JOURNAL_VERSION.saturating_add(1);
        fs::write(&journal_path, journal.encode()).expect("write unsupported journal version");

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("unsupported journal version must fail at the startup boundary");
        assert_lane_paths_absent(&root, &lane_config);
    }

    #[cfg(unix)]
    #[test]
    fn configured_catalog_preflight_rejects_journal_derived_symlink_before_lane_mutation() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = LaneCatalog::default();
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&kura_config(&root), &initial, &configured)
                .expect("establish valid configured startup");
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("persist a valid journal-derived archive path");
        let journal = kura
            .read_lane_geometry_journal()
            .expect("transition journal");
        let relative_link = &journal.records[0].operations[0].archived_blocks_path;
        let link = root.join(relative_link);
        fs::create_dir_all(link.parent().expect("archive path parent"))
            .expect("archive path parent");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("outside directory");
        symlink(&outside, &link).expect("inject journal-derived symlink");
        drop(kura);

        Kura::new_with_configured_lane_catalog(&kura_config(&root), &initial, &configured)
            .expect_err("journal-derived symlink must fail before opening attempted lane storage");
        assert!(link.is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn configured_catalog_preflight_rejects_journal_symlink_before_lane_mutation() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("journal-symlink-baseline");
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish valid configured baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        let outside_journal = temp.path().join("outside-journal.norito");
        fs::rename(&journal_path, &outside_journal).expect("move journal outside Kura root");
        symlink(&outside_journal, &journal_path).expect("replace journal with a symlink");

        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("configured-catalog journal symlink must fail closed");
        assert_lane_paths_absent(&root, &lane_config);
        assert!(journal_path.is_symlink());
        assert!(outside_journal.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn configured_catalog_preflight_rejects_journal_identity_swap_before_lane_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("identity-baseline");
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish valid configured baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        fs::copy(&journal_path, root.join(JOURNAL_IDENTITY_SWAP_FILE_NAME))
            .expect("prepare same-content replacement inode");
        Kura::replace_configured_catalog_journal_after_open_for_test(&root);

        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("journal identity replacement during read must fail closed");
        assert_lane_paths_absent(&root, &lane_config);
        assert!(root.join(JOURNAL_IDENTITY_DISPLACED_FILE_NAME).is_file());
    }
}

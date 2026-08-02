//! Crash-atomic Kura lane-geometry transitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    num::NonZeroUsize,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use iroha_config::parameters::actual::{LaneConfig, LaneConfigEntry};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{
        BlockHeader, SignedBlock,
        consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1, SumeragiLanePayloadOwnership},
        execution_context::{ExternalExecutionContext, ExternalExecutionRouteRole},
    },
    merge::{LaneDrainFrontierV1, MergeLedgerEntry},
    nexus::{DataSpaceId, LaneId},
    state_path::StatePath,
    transaction::signed::TransactionEntrypoint,
};
use norito::codec::{Decode, DecodeAll, Encode};
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
use rustix::fs::{
    AtFlags, Dir, FileType as RustixFileType, Mode, OFlags, openat, statat, unlinkat,
};
#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
use rustix::fs::{RenameFlags, renameat_with};

use super::{
    AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES, AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
    AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX, AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE,
    AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE, AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE,
    AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX, AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
    AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES, AutonomousLaneBlockArtifact,
    AutonomousLaneBlockLatestAttemptV1, AutonomousLaneMergeBundleV1,
    AutonomousLifecycleBootstrapRecoveryStage, AutonomousLifecycleBootstrapV1,
    AutonomousLifecycleCursorPhaseV2, AutonomousLifecycleCursorV2, BlockStore,
    BlockStoreCommitMarker, BoundProgressDirectory, BoundProgressNamespace, BoundProgressPair,
    BoundProgressRecoveryFailure, CERTIFIED_LANE_BLOCKS_DATA_FILE,
    CERTIFIED_LANE_BLOCKS_INDEX_FILE, COUNT_FILE_NAME, DATA_FILE_NAME, Error, HASHES_FILE_NAME,
    HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1, HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
    HistoricalAutonomousLaneRecoveryRecordV1, INDEX_FILE_NAME, Kura, LANE_ARTIFACTS_DATA_FILE,
    LANE_ARTIFACTS_DIR_NAME, LANE_ARTIFACTS_INDEX_FILE, LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
    LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE, LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE, LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE, LANE_MERGE_APPLICATION_FRONTIER_FILE,
    LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_BUILD_FILE, LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_FILE,
    LaneBlockApplicationReceiptArtifact, LaneBlockApplicationReceiptArtifactFormat,
    LaneBlockExecutionInputArtifact, LaneBlockExecutionPreflightArtifact,
    LaneMergeApplicationFrontierV1, MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
    MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES, MergeLedgerCarrierRecord,
    NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE, NativeAmxEvidenceKind,
    NativeAmxParticipantApplicationManifestArtifactV1,
    NativeAmxParticipantApplicationReceiptArtifact, RecoveredLaneBlockPayload, Result,
    STRICT_INIT_MAX_BLOCK_BYTES, bounded_historical_autonomous_recovery_entries,
    create_dir_all_with_context, sync_dir,
};
#[cfg(test)]
use super::{
    AUTONOMOUS_LANE_BLOCK_ATTEMPT_PREFIX, DEFAULT_NATIVE_AMX_PARTICIPANT_EVIDENCE_FILE_BYTES,
    NATIVE_AMX_APPLICATION_MANIFEST_FILE_PREFIX, NATIVE_AMX_EVIDENCE_FILE_SUFFIX,
    NATIVE_AMX_EVIDENCE_HEIGHT_DIGITS, OBSOLETE_AUTONOMOUS_LANE_BLOCKS_DATA_FILE,
    OBSOLETE_AUTONOMOUS_LANE_BLOCKS_INDEX_FILE, SidecarIndexEntry,
    V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY,
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
const MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR: usize = 65_536;
const LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE: usize = 6;
const LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE: usize = 2;
// In addition to the six regular data/index pairs, every route may retain
// the certified frontier, Native latest index, and merge-application frontier.
const LANE_RETIREMENT_FIXED_FRONTIERS_PER_ROUTE: usize = 3;
const LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE: usize =
    LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE * 2 + LANE_RETIREMENT_FIXED_FRONTIERS_PER_ROUTE;
const LANE_RETIREMENT_HISTORICAL_RECOVERY_NAMESPACES_PER_ROUTE: usize = 1;

/// Bound the aggregate retirement scan without treating legitimate route
/// multiplicity as corruption.
///
/// Each route can retain six ordinary histories (autonomous payload, input,
/// preflight, certificate, canonical merge bundle, and application receipt), plus two Native evidence
/// artifact families sharing one configured byte bound. Ordinary histories may also contain
/// the globally bounded pending-merge depth beyond their terminal frontier.
/// Historical autonomous recovery contributes one additional globally bounded
/// record inventory rather than a per-route multiplier.
/// Startup recovery may admit one entry beyond the compact Native window, but
/// retirement runs only after startup repair and therefore accepts exactly the
/// configured retained record count.
fn lane_retirement_aggregate_work_item_limit(
    route_count: usize,
    regular_retention: usize,
    native_retention: usize,
    pending_work_allowance: usize,
) -> Option<usize> {
    let regular_per_route = regular_retention
        .checked_add(pending_work_allowance)?
        .checked_mul(LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE)?;
    let native_per_route =
        native_retention.checked_mul(LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE)?;
    route_count
        .checked_mul(regular_per_route.checked_add(native_per_route)?)?
        .checked_add(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)
}

/// Bound one route's complete retirement artifact namespace.
fn lane_retirement_per_route_artifact_file_limit(native_retention: usize) -> Option<usize> {
    native_retention
        .checked_mul(LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE)?
        .checked_add(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES)?
        .checked_add(LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE)?
        .checked_add(LANE_RETIREMENT_HISTORICAL_RECOVERY_NAMESPACES_PER_ROUTE)
}

fn accumulate_lane_retirement_historical_recovery_records(
    current: usize,
    additional: usize,
) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)
}

fn remaining_lane_retirement_historical_recovery_budget(
    records_seen: usize,
    bytes_seen: u64,
    aggregate_byte_limit: u64,
) -> Option<(usize, u64)> {
    Some((
        HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS.checked_sub(records_seen)?,
        aggregate_byte_limit.checked_sub(bytes_seen)?,
    ))
}

/// Return whether Native manifest and receipt windows are the same complete,
/// contiguous retained suffix.
///
/// Publication may transiently leave one highest half-pair, but retirement
/// and archive validation run only after repair and pair pruning. Neither path
/// may accept family-skewed or punctured evidence.
fn native_amx_retained_windows_are_complete(
    manifest_heights: &BTreeSet<u64>,
    receipt_heights: &BTreeSet<u64>,
) -> bool {
    manifest_heights == receipt_heights
        && manifest_heights
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0].checked_add(1) == Some(pair[1]))
}

#[cfg(test)]
static CONFIGURED_CATALOG_PREFLIGHT_IDENTITY_SWAP: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
static CONFIGURED_CATALOG_PREFLIGHT_FAIL_AFTER_ESTABLISH: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
static GEOMETRY_MOVE_TARGET_COLLISION: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);
#[cfg(all(test, unix))]
static GEOMETRY_MOVE_PARENT_SUBSTITUTION: std::sync::Mutex<Option<(PathBuf, PathBuf, PathBuf)>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
enum ProgressSidecarDurabilityFault {
    Data,
    Index,
    ImmediateDirectory,
    Ancestor(usize),
}

#[cfg(test)]
impl ProgressSidecarDurabilityFault {
    fn inject(self) {
        match self {
            Self::Data => super::fail_next_indexed_sidecar_data_sync_for_tests(),
            Self::Index => super::fail_next_indexed_sidecar_index_sync_for_tests(),
            Self::ImmediateDirectory => super::fail_next_indexed_sidecar_dir_sync_for_tests(),
            Self::Ancestor(index) => {
                super::fail_progress_sidecar_ancestor_sync_at_for_tests(index);
            }
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static FAIL_ARCHIVED_RECEIPT_DURABILITY_ATTESTATION: std::cell::Cell<Option<ProgressSidecarDurabilityFault>> = const { std::cell::Cell::new(None) };
    static SUBSTITUTE_PROGRESS_DIRECTORY_AFTER_RECOVERY: std::cell::RefCell<Option<(String, PathBuf, PathBuf)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn fail_next_archived_receipt_durability_attestation_for_test(
    fault: ProgressSidecarDurabilityFault,
) {
    FAIL_ARCHIVED_RECEIPT_DURABILITY_ATTESTATION.with(|slot| slot.set(Some(fault)));
}

#[cfg(test)]
fn inject_archived_receipt_durability_fault_for_test() {
    if let Some(fault) =
        FAIL_ARCHIVED_RECEIPT_DURABILITY_ATTESTATION.with(|slot| slot.replace(None))
    {
        fault.inject();
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
fn substitute_progress_directory_after_recovery_for_test(
    kind: &str,
    target_lane_artifacts: &Path,
    displaced: PathBuf,
) {
    SUBSTITUTE_PROGRESS_DIRECTORY_AFTER_RECOVERY.with(|slot| {
        let previous = slot.replace(Some((
            kind.to_owned(),
            target_lane_artifacts.to_path_buf(),
            displaced,
        )));
        assert!(
            previous.is_none(),
            "progress-directory substitution injection must be single-owner"
        );
    });
}

#[cfg(test)]
fn maybe_substitute_progress_directory_after_recovery_for_test(kind: &str, lane_artifacts: &Path) {
    let displaced = SUBSTITUTE_PROGRESS_DIRECTORY_AFTER_RECOVERY.with(|slot| {
        let mut injection = slot.borrow_mut();
        if injection
            .as_ref()
            .is_some_and(|(target_kind, target_lane_artifacts, _)| {
                target_kind == kind && target_lane_artifacts.as_path() == lane_artifacts
            })
        {
            injection.take().map(|(_, _, displaced)| displaced)
        } else {
            None
        }
    });
    let Some(displaced) = displaced else {
        return;
    };
    fs::rename(lane_artifacts, &displaced)
        .expect("displace progress directory at the injected refresh boundary");
    fs::create_dir(lane_artifacts)
        .expect("install substituted progress directory at the injected refresh boundary");
}

#[cfg(test)]
fn canonical_test_store_root(store_root: &Path) -> PathBuf {
    fs::canonicalize(store_root).unwrap_or_else(|_| {
        store_root
            .parent()
            .and_then(|parent| fs::canonicalize(parent).ok())
            .and_then(|parent| store_root.file_name().map(|name| parent.join(name)))
            .unwrap_or_else(|| store_root.to_path_buf())
    })
}

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
fn rename_geometry_path_noreplace_at(
    source_parent: &File,
    source_name: &std::ffi::OsStr,
    target_parent: &File,
    target_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    renameat_with(
        source_parent,
        source_name,
        target_parent,
        target_name,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
fn geometry_stat_identity(stat: &rustix::fs::Stat) -> GeometryFileIdentity {
    GeometryFileIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
    }
}

#[cfg(windows)]
fn rename_geometry_path_noreplace_at(
    _source_parent: &File,
    _source_name: &std::ffi::OsStr,
    _target_parent: &File,
    _target_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic descriptor-relative lane geometry rename is unsupported on Windows",
    ))
}

#[cfg(test)]
fn inject_geometry_move_target_collision_for_test(target: &Path, directory: bool) -> Result<()> {
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
fn inject_geometry_move_target_collision_for_test(_target: &Path, _directory: bool) -> Result<()> {
    Ok(())
}

#[cfg(all(test, unix))]
fn inject_geometry_move_parent_substitution_for_test(target_parent: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;

    let substitution = {
        let mut hook = GEOMETRY_MOVE_PARENT_SUBSTITUTION
            .lock()
            .expect("geometry move parent-substitution hook lock");
        if hook
            .as_ref()
            .is_some_and(|(expected, _, _)| expected == target_parent)
        {
            hook.take()
        } else {
            None
        }
    };
    if let Some((_, displaced_parent, replacement_parent)) = substitution {
        fs::rename(target_parent, &displaced_parent)
            .map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
        symlink(&replacement_parent, target_parent)
            .map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
    }
    Ok(())
}

#[cfg(not(all(test, unix)))]
fn inject_geometry_move_parent_substitution_for_test(_target_parent: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox",
    windows
)))]
fn rename_geometry_path_noreplace_at(
    _source_parent: &File,
    _source_name: &std::ffi::OsStr,
    _target_parent: &File,
    _target_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic descriptor-relative lane geometry rename is unsupported on this platform",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundProgressDirectoryEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoundProgressDirectoryEntrySnapshot {
    kind: BoundProgressDirectoryEntryKind,
    identity: GeometryFileIdentity,
}

type BoundProgressDirectorySnapshot =
    BTreeMap<std::ffi::OsString, BoundProgressDirectoryEntrySnapshot>;

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
    checked_geometry_file_identity(&metadata, store_root)
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

fn configured_catalog_store_root_lock_identity(
    store_root: &Path,
    lock_file: &File,
) -> Result<GeometryFileIdentity> {
    let lock_path = store_root.join(super::STORE_ROOT_LOCK_FILE_NAME);
    let opened_metadata = lock_file
        .metadata()
        .map_err(|error| Error::IO(error, lock_path.clone()))?;
    let path_metadata =
        fs::symlink_metadata(&lock_path).map_err(|error| Error::IO(error, lock_path.clone()))?;
    let opened_identity = geometry_file_identity(&opened_metadata);
    if opened_metadata.file_type().is_symlink()
        || !opened_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || !path_metadata.file_type().is_file()
        || !Kura::sidecar_is_single_link(&opened_metadata)
        || !Kura::sidecar_is_single_link(&path_metadata)
        || geometry_file_identity(&path_metadata) != opened_identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "authenticated Kura store-root lock changed before configured-catalog preflight",
            ),
            lock_path,
        ));
    }
    Ok(opened_identity)
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

    let expected_identity = checked_geometry_file_identity(&path_metadata, path)?;
    let mut file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !opened_metadata.is_file()
        || checked_geometry_file_identity(&opened_metadata, path)? != expected_identity
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
        || checked_geometry_file_identity(&final_opened_metadata, path)? != expected_identity
        || final_opened_metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || final_path_metadata.file_type().is_symlink()
        || !final_path_metadata.file_type().is_file()
        || checked_geometry_file_identity(&final_path_metadata, path)? != expected_identity
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

fn preflight_empty_lane_artifact_directory(path: &Path) -> Result<()> {
    let before =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if before.file_type().is_symlink() || !before.is_dir() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "journal-owned empty block store has an invalid lane-artifact directory",
            ),
            path.to_path_buf(),
        ));
    }
    let identity = checked_geometry_file_identity(&before, path)?;
    let directory = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let opened = directory
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !opened.is_dir() || checked_geometry_file_identity(&opened, path)? != identity {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "journal-owned empty block store lane-artifact directory changed while opening",
            ),
            path.to_path_buf(),
        ));
    }

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    {
        let mut entries = Dir::read_from(&directory)
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        for entry in &mut entries {
            let entry = entry
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
            if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "journal-owned empty block store has a non-empty lane-artifact directory",
                    ),
                    path.to_path_buf(),
                ));
            }
        }
    }
    #[cfg(not(all(unix, not(any(target_os = "espidf", target_os = "redox")))))]
    if fs::read_dir(path)
        .map_err(|error| Error::IO(error, path.to_path_buf()))?
        .next()
        .transpose()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?
        .is_some()
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "journal-owned empty block store has a non-empty lane-artifact directory",
            ),
            path.to_path_buf(),
        ));
    }

    let opened_after = directory
        .metadata()
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    let after = fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !opened_after.is_dir()
        || checked_geometry_file_identity(&opened_after, path)? != identity
        || after.file_type().is_symlink()
        || !after.is_dir()
        || checked_geometry_file_identity(&after, path)? != identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "journal-owned empty block store lane-artifact directory changed during preflight",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(())
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
        if name == LANE_ARTIFACTS_DIR_NAME {
            preflight_empty_lane_artifact_directory(&path)?;
            continue;
        }
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "unbound configured primary block store contains an unsafe entry",
                ),
                path,
            ));
        }
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
    authenticated_lock_identity: Option<GeometryFileIdentity>,
) -> Result<()> {
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let allowed_path = allowed_publication_temp.map(|_| store_root.join(JOURNAL_TEMP_FILE_NAME));
    let lock_path = store_root.join(super::STORE_ROOT_LOCK_FILE_NAME);
    let mut saw_allowed_temp = false;
    let mut saw_authenticated_lock = false;
    for entry in
        fs::read_dir(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?
    {
        let entry = entry.map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
        let path = entry.path();
        if path == lock_path && authenticated_lock_identity.is_some() {
            let expected = authenticated_lock_identity.expect("authenticated lock identity exists");
            let metadata = entry
                .metadata()
                .map_err(|error| Error::IO(error, path.clone()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if saw_authenticated_lock
                || file_type.is_symlink()
                || !file_type.is_file()
                || !Kura::sidecar_is_single_link(&metadata)
                || geometry_file_identity(&metadata) != expected
            {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "authenticated Kura store-root lock changed during pristine-root validation",
                ));
            }
            saw_authenticated_lock = true;
            continue;
        }
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
    if authenticated_lock_identity.is_some() != saw_authenticated_lock {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "authenticated Kura store-root lock disappeared during pristine-root validation",
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
    let identity = checked_geometry_file_identity(
        &file
            .metadata()
            .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?,
        temp_path,
    )?;
    file.write_all(bytes)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    file.sync_all()
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    let path_metadata = fs::symlink_metadata(temp_path)
        .map_err(|error| Error::IO(error, temp_path.to_path_buf()))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.file_type().is_file()
        || checked_geometry_file_identity(&path_metadata, temp_path)? != identity
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
        || checked_geometry_file_identity(&temp_metadata, temp_path)? != temp_identity
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
        || checked_geometry_file_identity(&journal_metadata, journal_path)? != temp_identity
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
        || checked_geometry_file_identity(&final_temp_metadata, temp_path)? != temp_identity
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
    #[cfg(test)]
    pub(super) fn establish_or_verify_configured_lane_catalog_baseline(
        store_root: &Path,
        attempted: Hash,
    ) -> Result<()> {
        Self::establish_or_verify_configured_lane_catalog_baseline_inner(
            store_root, attempted, None,
        )
    }

    pub(super) fn establish_or_verify_configured_lane_catalog_baseline_with_lock(
        store_root: &Path,
        attempted: Hash,
        lock_file: &File,
    ) -> Result<()> {
        let root_identity = configured_catalog_store_root_identity(store_root)?;
        let lock_identity = configured_catalog_store_root_lock_identity(store_root, lock_file)?;
        configured_catalog_require_store_root_identity(store_root, root_identity)?;
        Self::establish_or_verify_configured_lane_catalog_baseline_inner(
            store_root,
            attempted,
            Some(lock_identity),
        )
    }

    /// Authenticate an already-established configured-catalog journal without
    /// promoting, deleting, or creating any filesystem entry.
    pub(super) fn verify_configured_lane_catalog_baseline_read_only(
        store_root: &Path,
        attempted: Hash,
        lock_file: &File,
    ) -> Result<()> {
        let root_identity = configured_catalog_store_root_identity(store_root)?;
        let _lock_identity = configured_catalog_store_root_lock_identity(store_root, lock_file)?;
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
                "configured-catalog baseline is missing during provisional snapshot startup",
            )
        })?;
        validate_configured_catalog_journal(store_root, &journal.journal, attempted)?;
        for temp_path in [
            store_root.join(JOURNAL_TEMP_FILE_NAME),
            store_root.join(JOURNAL_RESTORE_TEMP_FILE_NAME),
        ] {
            if configured_catalog_reserved_temp_identity(store_root, root_identity, &temp_path)?
                .is_some()
            {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured-catalog temporary journal requires recovery before provisional snapshot startup",
                ));
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
                "configured-catalog baseline disappeared during read-only verification",
            )
        })?;
        if authoritative.identity != journal.identity || authoritative.bytes != journal.bytes {
            return Err(configured_catalog_preflight_error(
                store_root,
                ErrorKind::InvalidData,
                "configured-catalog baseline changed during read-only verification",
            ));
        }
        configured_catalog_require_store_root_identity(store_root, root_identity)
    }

    fn establish_or_verify_configured_lane_catalog_baseline_inner(
        store_root: &Path,
        attempted: Hash,
        authenticated_lock_identity: Option<GeometryFileIdentity>,
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
            authenticated_lock_identity,
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
            authenticated_lock_identity,
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
            Some(canonical_test_store_root(store_root).join(JOURNAL_FILE_NAME));
    }

    #[cfg(test)]
    pub(super) fn fail_after_configured_catalog_preflight_for_test(store_root: &Path) {
        *CONFIGURED_CATALOG_PREFLIGHT_FAIL_AFTER_ESTABLISH
            .lock()
            .expect("configured-catalog crash hook lock") =
            Some(canonical_test_store_root(store_root));
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
            &BTreeSet::new(),
            None,
        )
    }

    /// Apply one geometry transition while allowing node-local pending work
    /// owned by an exactly certified retiring lane incarnation to be archived.
    ///
    /// The exception is intentionally narrower than general retirement
    /// admission: payloads coordinated by another lane, malformed artifacts,
    /// and in-flight files remain blocking. Callers must first validate a
    /// globally committed drain certificate for every supplied identity.
    #[cfg(test)]
    pub(crate) fn apply_lane_geometry_transition_with_certified_retirements(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        replaced_lane_ids: &BTreeSet<LaneId>,
        certified_retirements: &BTreeSet<(LaneId, DataSpaceId, Hash)>,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            replaced_lane_ids,
            certified_retirements,
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
            &BTreeSet::new(),
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
        certified_retirements: &BTreeSet<(LaneId, DataSpaceId, Hash)>,
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
            certified_retirements,
            transition_height,
        )
    }

    /// Apply an authenticated geometry transition at its exact committed height.
    #[cfg(test)]
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
            &BTreeSet::new(),
            Some(transition_height),
        )
    }

    #[cfg(test)]
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
        certified_retirements: &BTreeSet<(LaneId, DataSpaceId, Hash)>,
        transition_height: Option<u64>,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            previous_lineage_root,
            updated_lineage_root,
            replaced_lane_ids,
            certified_retirements,
            transition_height,
        )
    }

    fn validate_certified_lane_drain_frontier(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        frontier: &LaneDrainFrontierV1,
    ) -> Result<()> {
        if !frontier.matches_route(lane_id, dataspace_id, lane_incarnation)
            || crate::lane_consensus::validate_lane_drain_frontier(frontier).is_err()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "certified lane drain frontier is malformed or targets another incarnation",
            ));
        }
        if let Some(expected_native) = frontier.native_application {
            let receipt = self
                .read_native_amx_participant_application_receipt(
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    frontier.lane_block_height,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "certified Native-derived drain frontier lacks its exact durable receipt",
                    )
                })?;
            if self.native_amx_participant_application_drain_evidence(&receipt)
                != Some(expected_native)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "certified Native-derived drain frontier differs from its durable manifest, finality, application, receipt, or latest index",
                ));
            }
        }
        Ok(())
    }

    /// Apply an exact-height retained-lineage transition with signed drain frontiers.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_lane_geometry_transition_at_height_with_lineage_roots_and_certified_drain_frontiers(
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
        certified_frontiers: &BTreeMap<(LaneId, DataSpaceId, Hash), LaneDrainFrontierV1>,
        transition_height: u64,
    ) -> Result<()> {
        let mut certified_retirements = BTreeSet::new();
        for (&(lane_id, dataspace_id, lane_incarnation), frontier) in certified_frontiers {
            self.validate_certified_lane_drain_frontier(
                lane_id,
                dataspace_id,
                lane_incarnation,
                frontier,
            )?;
            certified_retirements.insert((lane_id, dataspace_id, lane_incarnation));
        }
        self.apply_lane_geometry_transition_at_height_with_lineage_roots_and_certified_retirements(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            previous_lineage_root,
            updated_lineage_root,
            replaced_lane_ids,
            &certified_retirements,
            transition_height,
        )
    }

    /// Apply an exact-height retained-lineage transition with certified retirements.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_lane_geometry_transition_at_height_with_lineage_roots_and_certified_retirements(
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
        certified_retirements: &BTreeSet<(LaneId, DataSpaceId, Hash)>,
        transition_height: u64,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            previous_lineage_root,
            updated_lineage_root,
            replaced_lane_ids,
            certified_retirements,
            Some(transition_height),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner(
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
        certified_retirements: &BTreeSet<(LaneId, DataSpaceId, Hash)>,
        transition_height: Option<u64>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }
        self.ensure_nonzero_lineage_root(previous_lineage_root)?;
        self.ensure_nonzero_lineage_root(updated_lineage_root)?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.resolve_canonical_storage_before_mutation()?;
        let _geometry_guard = self.lane_geometry_lock.lock();

        let previous_bindings =
            self.geometry_bindings(previous, previous_incarnations, previous_activation_heights)?;
        let updated_bindings =
            self.geometry_bindings(updated, updated_incarnations, updated_activation_heights)?;
        let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
        let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
        let certified_retirements = certified_retirements
            .iter()
            .map(
                |(lane_id, dataspace_id, lane_incarnation)| LaneRetirementIdentity {
                    lane_id: *lane_id,
                    dataspace_id: *dataspace_id,
                    lane_incarnation: *lane_incarnation,
                },
            )
            .collect::<BTreeSet<_>>();
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
            self.ensure_lane_retirement_admissible_locked(&retiring, &certified_retirements)?;
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
        self.ensure_lane_retirement_admissible_locked(&retiring, &certified_retirements)?;
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
                let ambiguous = Error::IO(
                    std::io::Error::other(format!(
                        "lane geometry apply failed ({error}); rollback failed ({rollback_error})"
                    )),
                    self.lane_geometry_journal_path(),
                );
                self.poison_canonical_storage("lane geometry apply rollback", &ambiguous);
                return Err(Error::CanonicalStoragePoisoned);
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
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.resolve_canonical_storage_before_mutation()?;
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

    /// Verify that compacted geometry can still reach the configured-primary replay floor.
    ///
    /// This is a read-only startup preflight. It deliberately does not finish pending GC, move
    /// lane paths, repair markers, or rewrite the journal; callers use it before deciding that an
    /// unusable snapshot may safely fall back to genesis-height Kura replay.
    pub(crate) fn preflight_lane_geometry_recovery_floor_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        self.ensure_nonzero_lineage_root(lineage_root)?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let fingerprint = geometry_catalog_fingerprint(&bindings);
        let journal = self.read_lane_geometry_journal()?;
        let primary_binding = bindings.first().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "configured-primary replay geometry has no primary binding",
            )
        })?;
        if journal
            .configured_primary_binding
            .as_ref()
            .is_some_and(|expected| expected != primary_binding)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "configured-primary geometry binding differs from its durable anchor",
            ));
        }
        let recovery_floor = Self::lane_geometry_identity_at_applied_count(&journal, 0);
        if recovery_floor.is_some_and(|identity| identity != (fingerprint, lineage_root)) {
            if let Some(checkpoint) = journal.checkpoint.as_ref() {
                return Err(self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!(
                        "state snapshot at height {} is required because the configured-primary lane-geometry recovery floor was compacted",
                        checkpoint.snapshot_height
                    ),
                ));
            }
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "configured-primary geometry identity does not match the retained recovery floor",
            ));
        }
        Ok(())
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
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.resolve_canonical_storage_before_mutation()?;
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
    /// The caller must invoke this only after the payload has passed the semantic restart reader
    /// and the snapshot data, digest, signature, Merkle metadata, and snapshot directory entry
    /// have all been synchronized. Kura independently joins the supplied snapshot identity to its
    /// canonical block hash and WSV checkpoint before compacting any transition history. Archive
    /// deletion is then replayable from the compacted journal and can never run ahead of that
    /// checkpoint.
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
        snapshot_smart_contract_state: &BTreeMap<StatePath, Vec<u8>>,
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

    /// Checkpoint restart-validated geometry against an exact retained-lineage identity.
    ///
    /// Callers must prove the snapshot can pass semantic restart initialization before invoking
    /// this compaction boundary; durable bytes and canonical block/WSV identity alone are not
    /// sufficient recovery evidence.
    pub(crate) fn checkpoint_lane_geometry_after_durable_snapshot_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
        snapshot_smart_contract_state: &BTreeMap<StatePath, Vec<u8>>,
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
        self.checkpoint_lane_geometry_with_proven_snapshot(
            bindings,
            lineage_root,
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
            merge_releases,
        )
    }

    /// Exercise startup/storage-budget archive-GC resumption with production
    /// lock acquisition from unit tests.
    #[cfg(test)]
    pub(super) fn resume_proven_lane_geometry_archive_gc(&self) -> Result<LaneGeometryGcSummary> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.resume_proven_lane_geometry_archive_gc_under_prune_and_canonical_guards()
    }

    /// Resume proven archive GC while the caller holds `prune_lock` and
    /// `canonical_chain_lock`, in that order.
    pub(super) fn resume_proven_lane_geometry_archive_gc_under_prune_and_canonical_guards(
        &self,
    ) -> Result<LaneGeometryGcSummary> {
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
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
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
        if !exact_bindings_match {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "snapshot geometry bindings do not exactly match transition history",
            ));
        }
        if journal.records[..prune_count]
            .iter()
            .any(|record| record.phase != LaneGeometryPhase::CatalogPublished)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "snapshot geometry includes a transition that is not catalog-published",
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
        // Callers hold prune -> canonical-chain -> geometry locks. Archive
        // validation acquires sidecar last and uses only no-relock Native AMX
        // evidence readers while that complete guard set is live.
        if journal.pending_archive_gc.is_empty() {
            return Ok(LaneGeometryGcSummary::default());
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
        snapshot_smart_contract_state: &BTreeMap<StatePath, Vec<u8>>,
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
        let (carrier_header, finality, _) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(
                record.block_height,
            )?
            .ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::NotFound,
                    "merge execution carrier has no retained finality evidence",
                )
            })?;
        let expected_commitment =
            iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(entry_hash);
        if carrier_header.hash() != record.block_hash
            || finality.block_hash != record.block_hash
            || finality.commit_qc.execution_commitment.merge_carrier != Some(expected_commitment)
            || crate::merge::merge_application_header_from_carrier(&carrier_header)
                != batch.application_block_header
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "merge execution carrier finality does not identify its durable merge entry",
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
        let records = self.merge_carrier_records_under_prune_and_canonical_guards()?;
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
        let identity_at_cursor =
            Self::lane_geometry_identity_at_applied_count(journal, desired_applied_count);
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

    fn lane_geometry_identity_at_applied_count(
        journal: &LaneGeometryJournal,
        desired_applied_count: usize,
    ) -> Option<(Hash, Hash)> {
        if desired_applied_count == 0 {
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
        }
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

    fn validate_certified_retirements_against_geometry(
        &self,
        retiring: &[LaneRetirementIdentity],
        certified_retirements: &BTreeSet<LaneRetirementIdentity>,
    ) -> Result<()> {
        let retiring = retiring.iter().copied().collect::<BTreeSet<_>>();
        if !certified_retirements.is_subset(&retiring) {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "certified retirement identity does not exactly match the retiring geometry",
            ));
        }
        Ok(())
    }

    /// Read and durability-attest the first-release Native AMX per-height
    /// evidence namespace without accepting the obsolete dense data/index
    /// layout.
    ///
    /// The caller holds the sidecar lock and compares the complete bound
    /// directory snapshot after this returns. Each file is also opened through
    /// the strict regular-sidecar reader, which rejects symlinks, hardlinks,
    /// non-canonical ancestors, replacement races, and oversized payloads.
    fn read_geometry_native_amx_per_height_evidence(
        &self,
        lane_artifacts: &Path,
        artifact_snapshot: &BoundProgressDirectorySnapshot,
        retained_record_limit: usize,
        context: &str,
    ) -> Result<(
        BTreeMap<u64, NativeAmxParticipantApplicationManifestArtifactV1>,
        BTreeMap<u64, NativeAmxParticipantApplicationReceiptArtifact>,
    )> {
        let payload_limit = usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES)?;
        let mut manifests = BTreeMap::new();
        let mut receipts = BTreeMap::new();
        let mut evidence_bytes = 0_u64;

        for (raw_name, entry_snapshot) in artifact_snapshot {
            let path = lane_artifacts.join(raw_name);
            let Some((kind, lane_block_height, temporary)) =
                Self::parse_native_amx_evidence_path(&path)?
            else {
                continue;
            };
            if temporary {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence is still temporary"),
                    ),
                    path,
                ));
            }
            let retained_count = match kind {
                NativeAmxEvidenceKind::Manifest => manifests.len(),
                NativeAmxEvidenceKind::Receipt => receipts.len(),
            };
            if retained_count >= retained_record_limit {
                let evidence_kind = match kind {
                    NativeAmxEvidenceKind::Manifest => "manifest",
                    NativeAmxEvidenceKind::Receipt => "receipt",
                };
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "{context} Native AMX {evidence_kind} count exceeds configured retention"
                        ),
                    ),
                    path,
                ));
            }
            if entry_snapshot.kind != BoundProgressDirectoryEntryKind::File {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence is not a regular file"),
                    ),
                    path,
                ));
            }
            let metadata =
                Self::regular_sidecar_metadata_for(&self.store_root, &path, lane_artifacts)?
                    .ok_or_else(|| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "{context} Native AMX evidence disappeared during validation"
                                ),
                            ),
                            path.clone(),
                        )
                    })?;
            let encoded_len = metadata.file.len();
            evidence_bytes = evidence_bytes.checked_add(encoded_len).ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence byte count overflows"),
                    ),
                    path.clone(),
                )
            })?;
            if evidence_bytes > self.native_amx_participant_evidence_file_bytes() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "{context} Native AMX manifests and receipts exceed their shared aggregate byte bound"
                        ),
                    ),
                    path,
                ));
            }
            let before = self
                .read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("{context} Native AMX evidence disappeared while reading"),
                        ),
                        path.clone(),
                    )
                })?;
            if !Self::stable_sidecar_metadata_unchanged(&metadata, &before.metadata) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence changed before decoding"),
                    ),
                    path,
                ));
            }

            match kind {
                NativeAmxEvidenceKind::Manifest => {
                    let artifact = norito::decode_from_bytes::<
                        NativeAmxParticipantApplicationManifestArtifactV1,
                    >(&before.bytes)
                    .map_err(Error::NoritoFrame)?;
                    if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes
                        || artifact.leaf.participant_height != lane_block_height
                        || Self::validate_native_amx_participant_application_manifest_artifact(
                            &artifact,
                        )
                        .is_err()
                        || manifests.insert(lane_block_height, artifact).is_some()
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "{context} Native AMX manifest is non-canonical, malformed, or duplicated"
                                ),
                            ),
                            path,
                        ));
                    }
                }
                NativeAmxEvidenceKind::Receipt => {
                    let artifact = norito::decode_from_bytes::<
                        NativeAmxParticipantApplicationReceiptArtifact,
                    >(&before.bytes)
                    .map_err(Error::NoritoFrame)?;
                    if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes
                        || artifact.participant_proposal.descriptor.lane_block_height
                            != lane_block_height
                        || Self::validate_native_amx_participant_application_receipt_artifact(
                            &artifact,
                        )
                        .is_err()
                        || receipts.insert(lane_block_height, artifact).is_some()
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "{context} Native AMX receipt is non-canonical, malformed, or duplicated"
                                ),
                            ),
                            path,
                        ));
                    }
                }
            }

            let file = OpenOptions::new()
                .read(true)
                .open(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            let opened_metadata = file
                .metadata()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if !Self::sidecar_file_metadata_unchanged(&before.metadata.file, &opened_metadata) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence changed before durability sync"),
                    ),
                    path,
                ));
            }
            file.sync_all()
                .map_err(|error| Error::IO(error, path.clone()))?;
            let after = self
                .read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!(
                                "{context} Native AMX evidence disappeared after durability sync"
                            ),
                        ),
                        path.clone(),
                    )
                })?;
            if after.bytes_hash != before.bytes_hash
                || !Self::stable_sidecar_metadata_unchanged(&before.metadata, &after.metadata)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence changed during durability sync"),
                    ),
                    path,
                ));
            }
        }
        sync_dir(lane_artifacts).map_err(|error| Error::IO(error, lane_artifacts.to_path_buf()))?;
        Ok((manifests, receipts))
    }

    /// Read and durability-attest the one bounded historical-autonomous
    /// recovery subdirectory carried by a lane incarnation. The outer
    /// lane-artifact snapshot accounts for the directory entry; this reader
    /// separately accounts every immutable record and encoded byte.
    #[allow(clippy::too_many_arguments)]
    fn read_geometry_historical_autonomous_recovery_records(
        &self,
        lane_artifacts: &Path,
        artifact_snapshot: &BoundProgressDirectorySnapshot,
        lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        entry_limit: usize,
        aggregate_byte_limit: u64,
        context: &str,
    ) -> Result<(Vec<HistoricalAutonomousLaneRecoveryRecordV1>, u64)> {
        let raw_name = std::ffi::OsStr::new(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1);
        let Some(snapshot) = artifact_snapshot.get(raw_name) else {
            return Ok((Vec::new(), 0));
        };
        let directory = lane_artifacts.join(raw_name);
        if snapshot.kind != BoundProgressDirectoryEntryKind::Directory {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{context} historical recovery namespace is not a directory"),
                ),
                directory,
            ));
        }
        let before = fs::symlink_metadata(&directory)
            .map_err(|error| Error::IO(error, directory.clone()))?;
        if before.file_type().is_symlink()
            || !before.file_type().is_dir()
            || geometry_file_identity(&before) != snapshot.identity
            || self.canonical_sidecar_directory(&directory)?.is_none()
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{context} historical recovery directory changed or escaped Kura"),
                ),
                directory,
            ));
        }
        let entry_limit = entry_limit.min(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS);
        let (entries, encoded_bytes) = bounded_historical_autonomous_recovery_entries(
            &directory,
            entry_limit,
            aggregate_byte_limit,
            |path| {
                let metadata = fs::symlink_metadata(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                Ok((metadata.clone(), metadata))
            },
        )?;
        let mut records = Vec::with_capacity(entries.len());
        for (path, accounted) in entries {
            let record = self
                .read_historical_autonomous_recovery_record_from_inventory(
                    &path,
                    &directory,
                    Some(&accounted),
                )?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("{context} historical recovery record disappeared"),
                        ),
                        path.clone(),
                    )
                })?;
            let descriptor = &record.payload.origin_proposal.descriptor;
            if descriptor.lane_id != lane_id
                || descriptor.lane_incarnation != expected_incarnation
                || descriptor.proposal_height <= activation_height
                || expected_dataspace_id
                    .is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} historical recovery record has a stale lane binding"),
                    ),
                    path,
                ));
            }
            let (retained_header, finality, _) = self
                .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(
                    record.canonical_body.height,
                )?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("{context} historical recovery finality is unavailable"),
                        ),
                        path.clone(),
                    )
                })?;
            if retained_header.hash() != record.canonical_body.block_hash
                || retained_header.height().get() != record.canonical_body.height
                || retained_header.view_change_index() != record.carrier_view
                || finality.height != record.canonical_body.height
                || finality.block_hash != record.canonical_body.block_hash
                || HashOf::new(&finality) != record.canonical_body.finality_artifact_hash
                || finality.commit_qc.execution_commitment
                    != record.canonical_body.execution_commitment
                || finality.height_context != record.historical_context
                || finality.verify().is_err()
                || finality.validate_for_header(&retained_header).is_err()
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} historical recovery has conflicting retained finality"),
                    ),
                    path,
                ));
            }
            File::open(&path)
                .and_then(|file| file.sync_all())
                .map_err(|error| Error::IO(error, path))?;
            records.push(record);
        }
        self.validate_historical_autonomous_recovery_inventory_collisions(&records)?;
        sync_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
        let after = fs::symlink_metadata(&directory)
            .map_err(|error| Error::IO(error, directory.clone()))?;
        if after.file_type().is_symlink()
            || !after.file_type().is_dir()
            || geometry_file_identity(&after) != snapshot.identity
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{context} historical recovery directory changed while reading"),
                ),
                directory,
            ));
        }
        Ok((records, encoded_bytes))
    }

    /// Admit first-release lane retirement only when durable work is terminal
    /// or is owned by the exact globally certified retiring incarnation.
    fn ensure_lane_retirement_admissible_locked(
        &self,
        retiring: &[LaneRetirementIdentity],
        certified_retirements: &BTreeSet<LaneRetirementIdentity>,
    ) -> Result<()> {
        self.validate_certified_retirements_against_geometry(retiring, certified_retirements)?;
        self.ensure_first_release_lane_retirement_admissible_with_certified_locked(
            retiring,
            certified_retirements,
        )
    }

    /// Exercise the production retirement scanner without a certified drain.
    #[cfg(test)]
    fn ensure_first_release_lane_retirement_admissible_locked(
        &self,
        retiring: &[LaneRetirementIdentity],
    ) -> Result<()> {
        self.ensure_first_release_lane_retirement_admissible_with_certified_locked(
            retiring,
            &BTreeSet::new(),
        )
    }

    /// Apply the Native-aware, fail-closed first-release retirement policy.
    fn ensure_first_release_lane_retirement_admissible_with_certified_locked(
        &self,
        retiring: &[LaneRetirementIdentity],
        certified_retirements: &BTreeSet<LaneRetirementIdentity>,
    ) -> Result<()> {
        // The geometry transition owns prune -> canonical-chain -> geometry ->
        // sidecar before entering this scanner. Native AMX validation below
        // must therefore use only the corresponding no-relock readers.
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
        let lifecycle_process_generation = self
            .read_autonomous_lifecycle_process_generation_record()?
            .map(|(record, _)| record);
        let aggregate_work_item_limit = lane_retirement_aggregate_work_item_limit(
            entries.len(),
            self.roster_sidecar_retention().get(),
            self.native_amx_participant_evidence_retention().get(),
            self.pending_control_sidecar_limits.certified_merge_entries,
        )
        .ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "configured lane retirement work-item bound overflows",
            )
        })?;
        let per_route_artifact_file_limit = lane_retirement_per_route_artifact_file_limit(
            self.native_amx_participant_evidence_retention().get(),
        )
        .ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "configured per-route lane retirement artifact-file bound overflows",
            )
        })?;
        let aggregate_artifact_file_limit = entries
            .len()
            .checked_mul(per_route_artifact_file_limit)
            .and_then(|outer_limit| {
                outer_limit.checked_add(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS)
            })
            .ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "configured lane retirement artifact-file bound overflows",
                )
            })?;
        let mut autonomous = BTreeMap::new();
        let mut inputs = BTreeMap::new();
        let mut preflights = BTreeMap::new();
        let mut certified = BTreeMap::new();
        let mut merge_bundles = BTreeMap::new();
        let mut receipts = BTreeMap::new();
        let mut native_manifests = BTreeMap::new();
        let mut native_receipts = BTreeMap::new();
        let mut historical_recoveries = BTreeMap::new();
        let mut artifact_files_seen = 0_usize;
        let mut work_items_seen = 0_usize;
        let mut historical_recovery_records_seen = 0_usize;
        let mut historical_recovery_bytes_seen = 0_u64;
        let historical_recovery_byte_limit =
            self.historical_autonomous_recovery_aggregate_byte_limit();
        let count_work_items = |current: &mut usize, additional: usize| -> Result<()> {
            *current = current.checked_add(additional).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement work-item count overflows",
                )
            })?;
            if *current > aggregate_work_item_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan exceeds its route/configuration-derived work-item bound",
                ));
            }
            Ok(())
        };

        for (storage_lane_id, entry) in entries {
            let blocks_path = entry.blocks_dir(&self.store_root);
            let lane_artifacts = Self::lane_artifact_dir(&blocks_path);
            let storage_route_is_retiring = retiring.iter().any(|identity| {
                identity.lane_id == storage_lane_id && identity.dataspace_id == entry.dataspace_id
            });
            if !self.validate_path_kind(&lane_artifacts, true)? {
                let blocks_guard =
                    Self::open_bound_progress_directory(&self.store_root, &blocks_path)?;
                if self.validate_path_kind(&lane_artifacts, true)?
                    || !self.geometry_bound_progress_directory_unchanged(&blocks_guard)
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement artifact namespace changed while proving it empty",
                    ));
                }
                continue;
            }
            let (lane_data, lane_index) =
                Self::lane_artifact_paths_for_entry(&entry, &self.store_root);
            let (input_data, input_index) =
                Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
            let (preflight_data, preflight_index) =
                Self::lane_block_execution_preflight_paths_for_entry(&entry, &self.store_root);
            let (certified_data, certified_index) =
                Self::certified_lane_block_paths_for_entry(&entry, &self.store_root);
            let (merge_bundle_data, merge_bundle_index) =
                Self::autonomous_lane_merge_bundle_paths_for_entry(&entry, &self.store_root);
            let (receipt_data, receipt_index) =
                Self::lane_block_application_receipt_paths_for_entry(&entry, &self.store_root);
            let native_receipt_latest =
                Self::native_amx_participant_receipt_latest_index_path_for_entry(
                    &entry,
                    &self.store_root,
                );
            let merge_application_frontier =
                Self::lane_merge_application_frontier_path_for_entry(&entry, &self.store_root);
            if let Some(frontier_read) =
                self.read_latest_certified_lane_block_frontier_locked(&entry, true)?
            {
                self.recover_certified_lane_block_pair_from_frontier_locked(
                    &entry,
                    &frontier_read.frontier.artifact,
                    None,
                )
                .map_err(|error| match error {
                    Error::IO(source, _) if source.kind() == ErrorKind::WouldBlock => self
                        .geometry_error(
                            ErrorKind::WouldBlock,
                            "lane retirement certified lane block durability attestation failed",
                        ),
                    error => error,
                })?;
                self.confirm_latest_certified_lane_block_frontier_read_locked(
                    &entry,
                    &frontier_read.snapshot,
                )?;
                self.note_certified_frontier_artifact_validation(
                    storage_lane_id,
                    &frontier_read.frontier,
                    &frontier_read.snapshot,
                );
            }
            let fixed_progress_pairs: [(&Path, &Path, &str); 6] = [
                (
                    &lane_data,
                    &lane_index,
                    "lane retirement lane-block artifact",
                ),
                (&input_data, &input_index, "lane retirement execution input"),
                (
                    &preflight_data,
                    &preflight_index,
                    "lane retirement execution preflight",
                ),
                (
                    &certified_data,
                    &certified_index,
                    "lane retirement certified lane block",
                ),
                (
                    &merge_bundle_data,
                    &merge_bundle_index,
                    "lane retirement autonomous merge bundle",
                ),
                (
                    &receipt_data,
                    &receipt_index,
                    "lane retirement application receipt",
                ),
            ];
            if let Some(frontier) =
                self.decode_lane_merge_application_frontier(&entry, &merge_application_frontier)?
            {
                if self
                    .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
                        &frontier,
                    )
                    .is_none()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement merge application frontier has no authenticated carrier",
                    ));
                }
                let frontier_identity = LaneRetirementIdentity {
                    lane_id: frontier.lane_id,
                    dataspace_id: frontier.dataspace_id,
                    lane_incarnation: frontier.lane_incarnation,
                };
                if retiring.contains(&frontier_identity)
                    && !self
                        .compact_lane_histories_through_merge_frontier_locked(&entry, &frontier)?
                {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "lane retirement resumed terminal auxiliary cleanup; retry to continue",
                    ));
                }
            }
            let lane_artifacts_guard = self.recover_geometry_progress_pairs_before_snapshot(
                &lane_artifacts,
                &fixed_progress_pairs,
                "first-release lane retirement",
            )?;
            let artifact_snapshot = self.geometry_bound_progress_directory_snapshot(
                &lane_artifacts_guard,
                per_route_artifact_file_limit,
                "first-release lane retirement artifact scan",
            )?;
            artifact_files_seen = artifact_files_seen
                .checked_add(artifact_snapshot.len())
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement artifact-file count overflows",
                    )
                })?;
            if artifact_files_seen > aggregate_artifact_file_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan exceeds its route-derived artifact-file count",
                ));
            }
            for (raw_name, snapshot) in &artifact_snapshot {
                let path = lane_artifacts.join(raw_name);
                if snapshot.kind == BoundProgressDirectoryEntryKind::Symlink {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a symlink artifact",
                        ),
                        path,
                    ));
                }
                let name = raw_name.to_str().ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a non-UTF-8 artifact",
                        ),
                        path.clone(),
                    )
                })?;
                if name == HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1 {
                    if snapshot.kind != BoundProgressDirectoryEntryKind::Directory {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane retirement historical recovery namespace is not a directory",
                            ),
                            path,
                        ));
                    }
                    continue;
                }
                if snapshot.kind != BoundProgressDirectoryEntryKind::File {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan encountered a non-regular artifact",
                        ),
                        path,
                    ));
                }
                if name.ends_with(".tmp") {
                    let message = if name.starts_with("autonomous_") {
                        "lane retirement scan found an in-flight autonomous sidecar"
                    } else {
                        "lane retirement scan found an in-flight sidecar"
                    };
                    return Err(Error::IO(
                        std::io::Error::new(ErrorKind::WouldBlock, message),
                        path,
                    ));
                }
                if name == LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_BUILD_FILE {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::WouldBlock,
                            "lane retirement scan found an in-flight sidecar",
                        ),
                        path,
                    ));
                }
                if name.starts_with(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX) {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan found an unresolved lifecycle-bootstrap temporary",
                        ),
                        path,
                    ));
                }
                if Self::autonomous_lifecycle_bootstrap_coordinates(name).is_some() {
                    let bytes = self
                        .read_regular_sidecar_bytes(
                            &path,
                            &lane_artifacts,
                            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                        )?
                        .ok_or_else(|| {
                            Error::IO(
                                std::io::Error::new(
                                    ErrorKind::InvalidData,
                                    "lane retirement lifecycle bootstrap disappeared during validation",
                                ),
                                path.clone(),
                            )
                        })?;
                    let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&path, &bytes)?;
                    let process_generation =
                        lifecycle_process_generation.as_ref().ok_or_else(|| {
                            Error::IO(
                                std::io::Error::new(
                                    ErrorKind::InvalidData,
                                    "lane retirement lifecycle bootstrap lacks its Kura-root process generation",
                                ),
                                path.clone(),
                            )
                        })?;
                    Self::validate_autonomous_lifecycle_bootstrap_process_generation(
                        process_generation,
                        &bootstrap,
                    )
                    .map_err(|message| {
                        Error::IO(
                            std::io::Error::new(ErrorKind::InvalidData, message),
                            path.clone(),
                        )
                    })?;
                    let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
                    let (active_incarnation, activation_height) =
                        self.active_lane_incarnation_marker(&entry)?;
                    if descriptor.lane_id != storage_lane_id
                        || descriptor.dataspace_id != entry.dataspace_id
                        || descriptor.lane_incarnation != active_incarnation
                        || descriptor.proposal_height <= activation_height
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane retirement lifecycle bootstrap targets a stale route or incarnation",
                            ),
                            path,
                        ));
                    }
                    if retiring.contains(&LaneRetirementIdentity {
                        lane_id: descriptor.lane_id,
                        dataspace_id: descriptor.dataspace_id,
                        lane_incarnation: descriptor.lane_incarnation,
                    }) {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::WouldBlock,
                                "lane retirement is blocked by an unfinished lifecycle bootstrap",
                            ),
                            path,
                        ));
                    }
                    continue;
                }
                if name.starts_with("autonomous_lifecycle_bootstrap") {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan found a malformed or legacy lifecycle bootstrap",
                        ),
                        path,
                    ));
                }
                if matches!(
                    name,
                    LANE_ARTIFACTS_DATA_FILE
                        | LANE_ARTIFACTS_INDEX_FILE
                        | CERTIFIED_LANE_BLOCKS_DATA_FILE
                        | CERTIFIED_LANE_BLOCKS_INDEX_FILE
                        | LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_FILE
                        | LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE
                        | LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE
                        | LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE
                        | LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE
                        | AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE
                        | AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE
                        | LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE
                        | LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE
                        | NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE
                        | LANE_MERGE_APPLICATION_FRONTIER_FILE
                ) {
                    continue;
                }
                if Self::parse_native_amx_evidence_path(&path)?.is_some() {
                    continue;
                }
                if !storage_route_is_retiring
                    && let Some(raw_height) = name
                        .strip_prefix("autonomous_view_")
                        .and_then(|suffix| suffix.strip_suffix(".norito"))
                {
                    let lane_block_height = raw_height.parse::<u64>().map_err(|_| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane retirement scan encountered a non-canonical view-state filename",
                            ),
                            path.clone(),
                        )
                    })?;
                    if lane_block_height == 0
                        || name != format!("autonomous_view_{lane_block_height:020}.norito")
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane retirement scan encountered a non-canonical view-state filename",
                            ),
                            path,
                        ));
                    }
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane retirement scan found an orphan autonomous view state",
                        ),
                        path,
                    ));
                }
                if Self::autonomous_lane_block_attempt_coordinates(name).is_some()
                    || Self::autonomous_lifecycle_cursor_coordinates(name).is_some()
                    || Self::autonomous_two_height_coordinates(
                        name,
                        AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
                    )
                    .is_some()
                    || Self::autonomous_one_height_coordinate(
                        name,
                        AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
                    )
                    .is_some()
                    || name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE
                {
                    continue;
                }
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane retirement scan encountered an unknown artifact filename",
                    ),
                    path,
                ));
            }
            let lane_bound = self.open_geometry_bound_progress_sidecar(&lane_data, &lane_index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &lane_bound,
                &lane_artifacts_guard,
                &lane_data,
                &lane_index,
                "lane retirement lane-block artifact",
            )?;
            let (active_incarnation, activation_height) =
                self.active_lane_incarnation_marker(&entry)?;
            let (remaining_historical_records, remaining_historical_bytes) =
                remaining_lane_retirement_historical_recovery_budget(
                    historical_recovery_records_seen,
                    historical_recovery_bytes_seen,
                    historical_recovery_byte_limit,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement historical recovery budget is already exhausted",
                    )
                })?;
            let (route_historical_recoveries, historical_recovery_bytes) = self
                .read_geometry_historical_autonomous_recovery_records(
                    &lane_artifacts,
                    &artifact_snapshot,
                    storage_lane_id,
                    Some(entry.dataspace_id),
                    active_incarnation,
                    activation_height,
                    remaining_historical_records.min(MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR),
                    remaining_historical_bytes,
                    "lane retirement",
                )?;
            let expected_route_historical_recoveries = route_historical_recoveries.clone();
            historical_recovery_bytes_seen = historical_recovery_bytes_seen
                .checked_add(historical_recovery_bytes)
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement historical recovery byte count overflows",
                    )
                })?;
            if historical_recovery_bytes_seen > historical_recovery_byte_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement historical recovery bytes exceed their aggregate bound",
                ));
            }
            historical_recovery_records_seen =
                accumulate_lane_retirement_historical_recovery_records(
                    historical_recovery_records_seen,
                    route_historical_recoveries.len(),
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement historical recovery records exceed their aggregate bound",
                    )
                })?;
            artifact_files_seen = artifact_files_seen
                .checked_add(route_historical_recoveries.len())
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement historical recovery file count overflows",
                    )
                })?;
            if artifact_files_seen > aggregate_artifact_file_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement historical recovery files exceed the route-derived bound",
                ));
            }
            count_work_items(&mut work_items_seen, route_historical_recoveries.len())?;
            for record in route_historical_recoveries {
                let lane_block_height = record.payload.origin_proposal.descriptor.lane_block_height;
                if historical_recoveries
                    .insert((storage_lane_id, lane_block_height), record)
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate historical recovery work",
                    ));
                }
            }
            let autonomous_attempts = self.read_geometry_autonomous_attempt_namespace(
                &lane_artifacts,
                storage_lane_id,
                Some(entry.dataspace_id),
                active_incarnation,
                activation_height,
                Some(&entry),
                MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR,
                storage_route_is_retiring,
            )?;
            count_work_items(&mut work_items_seen, autonomous_attempts.len())?;
            for (lane_block_height, (artifact, current, retired)) in autonomous_attempts {
                if autonomous
                    .insert(
                        (storage_lane_id, lane_block_height),
                        (artifact, current, retired),
                    )
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate autonomous work identity",
                    ));
                }
            }
            let mut input_bound =
                self.open_geometry_bound_progress_sidecar(&input_data, &input_index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &input_bound,
                &lane_artifacts_guard,
                &input_data,
                &input_index,
                "lane retirement execution input",
            )?;
            let input_heights = input_bound.sidecar_mut().map_or_else(
                || Ok(BTreeSet::new()),
                |bound| {
                    self.bound_indexed_sidecar_payload_heights(
                        bound,
                        "lane retirement execution input",
                        MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR,
                    )
                },
            )?;
            count_work_items(&mut work_items_seen, input_heights.len())?;
            for lane_block_height in input_heights {
                let input = self
                    .read_geometry_execution_input_from_bound(
                        storage_lane_id,
                        lane_block_height,
                        input_bound
                            .sidecar_mut()
                            .expect("non-empty height set has a bound execution input sidecar"),
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
            let mut preflight_bound =
                self.open_geometry_bound_progress_sidecar(&preflight_data, &preflight_index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &preflight_bound,
                &lane_artifacts_guard,
                &preflight_data,
                &preflight_index,
                "lane retirement execution preflight",
            )?;
            let preflight_heights = preflight_bound.sidecar_mut().map_or_else(
                || Ok(BTreeSet::new()),
                |bound| {
                    self.bound_indexed_sidecar_payload_heights(
                        bound,
                        "lane retirement execution preflight",
                        MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR,
                    )
                },
            )?;
            count_work_items(&mut work_items_seen, preflight_heights.len())?;
            for lane_block_height in preflight_heights {
                let preflight = self
                    .read_geometry_execution_preflight_from_bound(
                        storage_lane_id,
                        lane_block_height,
                        preflight_bound
                            .sidecar_mut()
                            .expect("non-empty height set has a bound execution preflight sidecar"),
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
            let mut certified_bound =
                self.open_geometry_bound_progress_sidecar(&certified_data, &certified_index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &certified_bound,
                &lane_artifacts_guard,
                &certified_data,
                &certified_index,
                "lane retirement certified lane block",
            )?;
            let certified_heights = certified_bound.sidecar_mut().map_or_else(
                || Ok(BTreeSet::new()),
                |bound| {
                    self.bound_indexed_sidecar_payload_heights(
                        bound,
                        "lane retirement certified lane block",
                        MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR,
                    )
                },
            )?;
            count_work_items(&mut work_items_seen, certified_heights.len())?;
            for lane_block_height in certified_heights {
                let artifact = self
                    .read_certified_lane_block_artifact_from_bound_locked(
                        storage_lane_id,
                        lane_block_height,
                        certified_bound
                            .sidecar_mut()
                            .expect("non-empty height set has a bound certified sidecar"),
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
            if certified_bound.sidecar().is_some_and(|bound| {
                !self.sync_bound_progress_sidecar(bound, "lane retirement certified lane block")
            }) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "lane retirement certified lane block durability attestation failed",
                ));
            }
            let mut merge_bundle_bound =
                self.open_geometry_bound_progress_sidecar(&merge_bundle_data, &merge_bundle_index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &merge_bundle_bound,
                &lane_artifacts_guard,
                &merge_bundle_data,
                &merge_bundle_index,
                "lane retirement autonomous merge bundle",
            )?;
            let merge_bundle_heights = merge_bundle_bound.sidecar_mut().map_or_else(
                || Ok(BTreeSet::new()),
                |bound| {
                    self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                        .map(|(_, heights)| heights)
                        .map_err(|message| {
                            self.geometry_error_owned(
                                ErrorKind::InvalidData,
                                format!(
                                    "lane retirement autonomous merge bundle pair is invalid: {message}"
                                ),
                            )
                        })
                },
            )?;
            count_work_items(&mut work_items_seen, merge_bundle_heights.len())?;
            for lane_block_height in merge_bundle_heights {
                let (bundle, _) = self
                    .read_autonomous_lane_merge_bundle_from_bound_locked(
                        storage_lane_id,
                        lane_block_height,
                        merge_bundle_bound.sidecar_mut().expect(
                            "non-empty height set has a bound autonomous merge bundle sidecar",
                        ),
                    )
                    .map_err(|message| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!(
                                "lane retirement autonomous merge bundle is invalid: {message}"
                            ),
                        )
                    })?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement autonomous merge bundle disappeared during validation",
                        )
                    })?;
                self.require_active_lane_artifact(&entry, &bundle.certified.proposal.descriptor)?;
                if merge_bundles
                    .insert((storage_lane_id, lane_block_height), bundle)
                    .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found duplicate autonomous merge bundle identity",
                    ));
                }
            }
            if merge_bundle_bound.sidecar().is_some_and(|bound| {
                !self.sync_bound_progress_sidecar(bound, AutonomousLaneMergeBundleV1::FORMAT_LABEL)
            }) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "lane retirement autonomous merge bundle durability attestation failed",
                ));
            }
            let mut receipt_bound =
                self.open_geometry_bound_progress_sidecar(&receipt_data, &receipt_index)?;
            self.ensure_geometry_progress_pair_uses_directory(
                &receipt_bound,
                &lane_artifacts_guard,
                &receipt_data,
                &receipt_index,
                "lane retirement application receipt",
            )?;
            let receipt_heights = receipt_bound.sidecar_mut().map_or_else(
                || Ok(BTreeSet::new()),
                |bound| {
                    self.bound_indexed_sidecar_payload_heights(
                        bound,
                        "lane retirement application receipt",
                        MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR,
                    )
                },
            )?;
            count_work_items(&mut work_items_seen, receipt_heights.len())?;
            for lane_block_height in receipt_heights {
                let receipt = self
                    .read_lane_block_application_receipt_from_bound_locked(
                        storage_lane_id,
                        lane_block_height,
                        receipt_bound
                            .sidecar_mut()
                            .expect("non-empty height set has a bound receipt sidecar"),
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
            if receipt_bound.sidecar().is_some_and(|bound| {
                !self.sync_bound_progress_sidecar(bound, "lane retirement application receipt")
            }) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "lane retirement application receipt durability attestation failed",
                ));
            }
            let (retained_native_manifests, retained_native_receipts) = self
                .read_geometry_native_amx_per_height_evidence(
                    &lane_artifacts,
                    &artifact_snapshot,
                    self.native_amx_participant_evidence_retention().get(),
                    "lane retirement",
                )?;
            let native_manifest_heights = retained_native_manifests
                .keys()
                .copied()
                .collect::<BTreeSet<_>>();
            let native_receipt_heights = retained_native_receipts
                .keys()
                .copied()
                .collect::<BTreeSet<_>>();
            count_work_items(&mut work_items_seen, native_manifest_heights.len())?;
            count_work_items(&mut work_items_seen, native_receipt_heights.len())?;
            if !native_amx_retained_windows_are_complete(
                &native_manifest_heights,
                &native_receipt_heights,
            ) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement Native AMX manifest/receipt evidence is not an exact contiguous retained suffix",
                ));
            }
            Self::validate_native_amx_retained_history_continuity(
                &retained_native_manifests,
                &retained_native_receipts,
                false,
            )
            .map_err(|message| {
                self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!("lane retirement Native AMX retained history is invalid: {message}"),
                )
            })?;
            for (lane_block_height, manifest) in retained_native_manifests {
                if manifest.leaf.lane_id != entry.lane_id
                    || manifest.leaf.dataspace_id != entry.dataspace_id
                    || self
                        .require_active_lane_incarnation(
                            &entry,
                            manifest.leaf.lane_incarnation,
                            manifest.leaf.application_block_height,
                        )
                        .is_err()
                    || native_manifests
                        .insert((storage_lane_id, lane_block_height), manifest)
                        .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found a stale or duplicate Native AMX participant manifest identity",
                    ));
                }
            }
            for (lane_block_height, receipt) in retained_native_receipts {
                let descriptor = &receipt.participant_proposal.descriptor;
                let Some(manifest) = native_manifests.get(&(storage_lane_id, lane_block_height))
                else {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement Native AMX participant receipt has no manifest proof",
                    ));
                };
                if self
                    .require_active_lane_artifact(&entry, descriptor)
                    .is_err()
                    || receipt.manifest_artifact_hash != HashOf::new(manifest)
                    || !Self::native_amx_participant_receipt_matches_manifest_leaf(
                        &receipt,
                        &manifest.leaf,
                    )
                    || native_receipts
                        .insert((storage_lane_id, lane_block_height), receipt)
                        .is_some()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement scan found a stale or duplicate Native AMX participant receipt identity",
                    ));
                }
            }
            let latest_native_receipt = self
                .decode_native_amx_participant_receipt_latest_index(&entry, &native_receipt_latest)
                .map_err(|error| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!("lane retirement Native AMX latest index is malformed: {error}"),
                    )
                })?;
            match native_receipt_heights.last().copied() {
                Some(latest_height) => {
                    let receipt = native_receipts
                        .get(&(storage_lane_id, latest_height))
                        .expect("latest scanned Native AMX receipt is retained");
                    if !latest_native_receipt.is_some_and(|latest| {
                        self.require_active_lane_incarnation(
                            &entry,
                            latest.lane_incarnation,
                            latest.application_block_height,
                        )
                        .is_ok()
                            && latest.matches_receipt(receipt)
                    }) {
                        return Err(self.geometry_error(
                            ErrorKind::InvalidData,
                            "lane retirement Native AMX latest index does not match the highest receipt",
                        ));
                    }
                }
                None if latest_native_receipt.is_some() => {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement Native AMX latest index has no receipt pair",
                    ));
                }
                None => {}
            }
            for (pair, data, index, kind, failure) in [
                (
                    &lane_bound,
                    &lane_data,
                    &lane_index,
                    "lane retirement lane-block artifact",
                    "lane retirement lane-block artifact durability attestation failed",
                ),
                (
                    &input_bound,
                    &input_data,
                    &input_index,
                    "lane retirement execution input",
                    "lane retirement execution input durability attestation failed",
                ),
                (
                    &preflight_bound,
                    &preflight_data,
                    &preflight_index,
                    "lane retirement execution preflight",
                    "lane retirement execution preflight durability attestation failed",
                ),
            ] {
                if pair
                    .sidecar()
                    .is_some_and(|bound| !self.sync_bound_progress_sidecar(bound, kind))
                {
                    return Err(self.geometry_error(ErrorKind::WouldBlock, failure));
                }
                self.ensure_absent_geometry_progress_sidecar_remains_absent(pair, data, index)?;
            }
            self.ensure_absent_geometry_progress_sidecar_remains_absent(
                &certified_bound,
                &certified_data,
                &certified_index,
            )?;
            self.ensure_absent_geometry_progress_sidecar_remains_absent(
                &merge_bundle_bound,
                &merge_bundle_data,
                &merge_bundle_index,
            )?;
            self.ensure_absent_geometry_progress_sidecar_remains_absent(
                &receipt_bound,
                &receipt_data,
                &receipt_index,
            )?;
            let (confirmed_route_historical_recoveries, confirmed_historical_recovery_bytes) = self
                .read_geometry_historical_autonomous_recovery_records(
                    &lane_artifacts,
                    &artifact_snapshot,
                    storage_lane_id,
                    Some(entry.dataspace_id),
                    active_incarnation,
                    activation_height,
                    expected_route_historical_recoveries.len(),
                    historical_recovery_bytes,
                    "lane retirement rescan",
                )?;
            if confirmed_route_historical_recoveries != expected_route_historical_recoveries
                || confirmed_historical_recovery_bytes != historical_recovery_bytes
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement historical recovery namespace changed during validation",
                ));
            }
            let confirmed_snapshot = self.geometry_bound_progress_directory_snapshot(
                &lane_artifacts_guard,
                per_route_artifact_file_limit,
                "lane retirement artifact rescan",
            )?;
            if confirmed_snapshot != artifact_snapshot
                || !self.geometry_bound_progress_directory_unchanged(&lane_artifacts_guard)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement artifact namespace changed during progress scan",
                ));
            }
        }

        for (identity, preflight) in &preflights {
            if !inputs.get(identity).is_some_and(|input| {
                input.proposal == preflight.proposal && input.artifact == preflight.artifact
            }) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan found an orphan execution preflight",
                ));
            }
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
            let (payload, current) = if let Some((artifact, current, _)) = autonomous.get(identity)
            {
                (&artifact.executable_payload, current)
            } else {
                let bundle = merge_bundles.get(identity).ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement execution input has no producer-authenticated payload or durable merge bundle",
                    )
                })?;
                let applied = receipts.get(identity).is_some_and(|receipt| {
                    receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution
                        && receipt.merge_source_bundle_hash == bundle.bundle_hash().ok()
                        && self
                            .lane_block_application_receipt_matches_merge_log_under_prune_and_canonical_guards(
                                receipt,
                            )
                });
                if !applied {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement execution input lost its payload before canonical merge application",
                    ));
                }
                (bundle.executable_payload(), &bundle.certified.proposal)
            };
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

        for (identity, bundle) in &merge_bundles {
            let certified_artifact = certified.get(identity).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement autonomous merge bundle has no exact certified slot",
                )
            })?;
            let input = inputs.get(identity).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement autonomous merge bundle has no durable execution input",
                )
            })?;
            let payload = bundle.executable_payload();
            let expected_input = Self::autonomous_lane_block_execution_input_candidate(
                payload,
                payload.chain_id_hash,
                payload.epoch,
            )
            .map_err(|availability| {
                self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!(
                        "lane retirement autonomous merge bundle input is invalid: {availability:?}"
                    ),
                )
            })?;
            let autonomous_matches =
                autonomous
                    .get(identity)
                    .is_some_and(|(autonomous_artifact, _, retired)| {
                        !*retired && &bundle.autonomous == autonomous_artifact
                    });
            let applied_bundle_is_self_contained = receipts.get(identity).is_some_and(|receipt| {
                receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution
                    && receipt.merge_source_bundle_hash == bundle.bundle_hash().ok()
                    && self
                        .lane_block_application_receipt_matches_merge_log_under_prune_and_canonical_guards(
                            receipt,
                        )
            });
            if (!autonomous_matches && !applied_bundle_is_self_contained)
                || &bundle.certified != certified_artifact
                || input != &expected_input
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement autonomous merge bundle differs from its exact durable components",
                ));
            }
        }
        for (identity, certified_artifact) in &certified {
            if certified_artifact
                .prepare_qc
                .payload_availability_qc
                .is_some()
                && !merge_bundles.contains_key(identity)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement certified autonomous work lacks its durable merge bundle",
                ));
            }
        }

        for (identity, record) in &historical_recoveries {
            let (autonomous_artifact, _, retired) = autonomous.get(identity).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement historical recovery has no durable autonomous payload",
                )
            })?;
            let input = inputs.get(identity).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement historical recovery has no durable execution input",
                )
            })?;
            let expected_input = Self::autonomous_lane_block_execution_input_candidate(
                &record.payload,
                record.payload.chain_id_hash,
                record.payload.epoch,
            )
            .map_err(|availability| {
                self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!(
                        "lane retirement historical recovery input is invalid: {availability:?}"
                    ),
                )
            })?;
            if *retired
                || autonomous_artifact.executable_payload != record.payload
                || input != &expected_input
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement historical recovery differs from its payload or execution input",
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
                    self.lane_block_application_receipt_matches_merge_log_under_prune_and_canonical_guards(
                        receipt,
                    )
                }
            };
            if !valid {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan found an application receipt without canonical evidence",
                ));
            }
        }

        for manifest in native_manifests.values() {
            if !self
                .native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(manifest)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan found a Native AMX participant manifest without canonical finality",
                ));
            }
        }
        for (identity, receipt) in &native_receipts {
            let Some(manifest) = native_manifests.get(identity) else {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan found an orphan Native AMX participant receipt",
                ));
            };
            if !self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(
                receipt,
                manifest,
            ) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane retirement scan found a Native AMX participant receipt without canonical evidence",
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
                    "first-release certified payload has no canonical block hint",
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

        for (identity, artifact) in &certified {
            if receipt_applies(identity, &artifact.proposal) {
                continue;
            }
            if lane_proposal_coordinator_targets_retirement(&artifact.proposal, &retiring) {
                if lane_proposal_coordinator_targets_retirement(
                    &artifact.proposal,
                    certified_retirements,
                ) {
                    continue;
                }
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending certified work belongs to a retiring lane incarnation",
                ));
            }
            if let Some((autonomous_artifact, current, retired)) = autonomous.get(identity) {
                if current != &artifact.proposal || *retired {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "certified autonomous work conflicts with its durable executable payload",
                    ));
                }
                if lane_payload_targets_retirement(
                    &autonomous_artifact.executable_payload,
                    &retiring,
                ) {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "pending certified autonomous work targets a retiring lane incarnation",
                    ));
                }
                continue;
            }
            if hinted_payload_targets(&artifact.proposal)? {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending certified global work targets a retiring lane incarnation",
                ));
            }
        }

        for (identity, (artifact, current, retired)) in &autonomous {
            if *retired || receipt_applies_autonomous(identity, artifact, current) {
                continue;
            }
            let coordinator_has_certified_drain = lane_proposal_coordinator_targets_retirement(
                &artifact.executable_payload.origin_proposal,
                certified_retirements,
            );
            if lane_payload_targets_retirement(&artifact.executable_payload, &retiring) {
                if coordinator_has_certified_drain {
                    continue;
                }
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
                if coordinator_has_certified_drain {
                    continue;
                }
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending hinted autonomous payload targets a retiring lane incarnation",
                ));
            }
        }

        for (identity, input) in &inputs {
            if receipt_applies(identity, &input.proposal) {
                continue;
            }
            let coordinator_has_certified_drain = lane_proposal_coordinator_targets_retirement(
                &input.proposal,
                certified_retirements,
            );
            if input.autonomous_payload_hash.is_some() {
                let (artifact, _, _) = autonomous.get(identity).ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "pending autonomous execution input has no durable executable payload",
                    )
                })?;
                if lane_payload_targets_retirement(&artifact.executable_payload, &retiring) {
                    if coordinator_has_certified_drain {
                        continue;
                    }
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "pending autonomous execution input targets a retiring lane incarnation",
                    ));
                }
                continue;
            }
            if lane_proposal_coordinator_targets_retirement(&input.proposal, &retiring)
                || hinted_payload_targets(&input.proposal)?
            {
                if coordinator_has_certified_drain {
                    continue;
                }
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending execution input targets a retiring lane incarnation",
                ));
            }
        }
        Ok(())
    }

    /// Exercise the production first-release retirement policy from parent-module tests.
    #[cfg(test)]
    pub(super) fn first_release_lane_retirement_admissible_for_test(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        self.ensure_lane_retirement_admissible_locked(
            &[LaneRetirementIdentity {
                lane_id,
                dataspace_id,
                lane_incarnation,
            }],
            &BTreeSet::new(),
        )
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
            if let Some(receipt) = context.native_amx_receipt.as_ref() {
                if native_amx_receipt_targets_retirement(receipt, retiring).map_err(|_| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane retirement hinted native AMX application role is invalid",
                    )
                })? {
                    return Ok(true);
                }
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
        let active_blocks = *self.active_blocks_dir.lock() == old_blocks;
        let active_merge = *self.active_merge_path.lock() == old_merge;
        // Keep the active block-store writer excluded from the first handle flush
        // through the final path retarget. Releasing this guard before the rename
        // would let a concurrent append reopen the old pathname after its cached
        // handles were dropped.
        let _write_guard = active_blocks.then(|| self.block_store_write_lock.lock());
        let mut block_store = active_blocks.then(|| self.block_store.lock());
        if let Some(store) = block_store.as_mut() {
            if !self.validate_path_kind(&old_blocks, true)? {
                return Err(self.geometry_error(
                    ErrorKind::NotFound,
                    "active primary block store disappeared before relabel",
                ));
            }
            store.flush_pending_fsync(true)?;
            store.drop_cached_handles();
        }
        let mut merge_log = active_merge.then(|| self.merge_log.lock());
        if active_merge && !self.validate_path_kind(&old_merge, false)? {
            return Err(self.geometry_error(
                ErrorKind::NotFound,
                "active primary merge log disappeared before relabel",
            ));
        }
        if let Some(log) = merge_log.as_mut()
            && let Some(file) = log.file.as_mut()
        {
            file.try_io(|inner| {
                inner.flush()?;
                inner.sync_all()
            })?;
        }
        self.move_geometry_binding_pair(
            updated,
            &old_blocks,
            &old_merge,
            &new_blocks,
            &new_merge,
            GeometryPairTargetKind::MutableLive,
        )?;
        #[cfg(test)]
        if active_blocks
            && self
                .pause_primary_relabel_before_retarget
                .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.primary_relabel_paused
                .store(true, std::sync::atomic::Ordering::Release);
            while self
                .primary_relabel_paused
                .load(std::sync::atomic::Ordering::Acquire)
            {
                std::thread::yield_now();
            }
        }
        if let Some(store) = block_store.as_mut() {
            store.retarget_existing_path(new_blocks.clone());
            *self.active_blocks_dir.lock() = new_blocks.clone();
            self.invalidate_durable_budget_snapshot();
        }
        if let Some(log) = merge_log.as_mut() {
            if let Some(file) = log.file.as_mut() {
                file.path.clone_from(&new_merge);
            }
            *self.active_merge_path.lock() = new_merge.clone();
        }
        if old_blocks != new_blocks {
            let mut plain_text = self.block_plain_text_path.lock();
            if let Some(path) = plain_text.as_mut()
                && let Ok(suffix) = path.strip_prefix(&old_blocks)
            {
                *path = new_blocks.join(suffix);
            }
        }
        Ok(())
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    fn remove_geometry_directory_contents_at(
        directory: &File,
        display_path: &Path,
        depth: usize,
        entries_seen: &mut usize,
    ) -> Result<()> {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        if depth > MAX_GEOMETRY_ARCHIVE_DEPTH {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry GC tree exceeds the maximum directory depth",
                ),
                display_path.to_path_buf(),
            ));
        }
        loop {
            // Re-open the directory stream after each unlink. POSIX does not promise that a
            // stream continues without skipping entries when its directory is mutated in place.
            let mut entries = Dir::read_from(directory)
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, display_path.to_path_buf()))?;
            let mut next_name = None;
            for entry in &mut entries {
                let entry = entry
                    .map_err(std::io::Error::from)
                    .map_err(|error| Error::IO(error, display_path.to_path_buf()))?;
                if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
                    next_name = Some(entry.file_name().to_owned());
                    break;
                }
            }
            let Some(name) = next_name else { break };
            let name = name.as_c_str();
            *entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry GC tree entry count overflow",
                    ),
                    display_path.to_path_buf(),
                )
            })?;
            if *entries_seen > MAX_GEOMETRY_ARCHIVE_ENTRIES {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry GC tree exceeds the maximum entry count",
                    ),
                    display_path.to_path_buf(),
                ));
            }

            let child_path = display_path.join(OsStr::from_bytes(name.to_bytes()));
            let before = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, child_path.clone()))?;
            let before_identity = geometry_stat_identity(&before);
            match RustixFileType::from_raw_mode(before.st_mode) {
                RustixFileType::Directory => {
                    let child_depth = depth.checked_add(1).ok_or_else(|| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane geometry GC tree depth overflow",
                            ),
                            child_path.clone(),
                        )
                    })?;
                    let child = File::from(
                        openat(
                            directory,
                            name,
                            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(std::io::Error::from)
                        .map_err(|error| Error::IO(error, child_path.clone()))?,
                    );
                    let opened = child
                        .metadata()
                        .map_err(|error| Error::IO(error, child_path.clone()))?;
                    if !opened.is_dir()
                        || checked_geometry_file_identity(&opened, &child_path)? != before_identity
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane geometry GC directory changed while being opened",
                            ),
                            child_path,
                        ));
                    }
                    Self::remove_geometry_directory_contents_at(
                        &child,
                        &child_path,
                        child_depth,
                        entries_seen,
                    )?;
                    let after = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .map_err(|error| Error::IO(error, child_path.clone()))?;
                    if RustixFileType::from_raw_mode(after.st_mode) != RustixFileType::Directory
                        || geometry_stat_identity(&after) != before_identity
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane geometry GC directory entry changed before removal",
                            ),
                            child_path,
                        ));
                    }
                    drop(child);
                    unlinkat(directory, name, AtFlags::REMOVEDIR)
                        .map_err(std::io::Error::from)
                        .map_err(|error| Error::IO(error, child_path))?;
                }
                RustixFileType::RegularFile => {
                    let after = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .map_err(|error| Error::IO(error, child_path.clone()))?;
                    if RustixFileType::from_raw_mode(after.st_mode) != RustixFileType::RegularFile
                        || geometry_stat_identity(&after) != before_identity
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                "lane geometry GC file entry changed before removal",
                            ),
                            child_path,
                        ));
                    }
                    unlinkat(directory, name, AtFlags::empty())
                        .map_err(std::io::Error::from)
                        .map_err(|error| Error::IO(error, child_path))?;
                }
                _ => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry GC tree contains a non-regular entry",
                        ),
                        child_path,
                    ));
                }
            }
        }
        directory
            .sync_all()
            .map_err(|error| Error::IO(error, display_path.to_path_buf()))
    }

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    fn remove_authenticated_geometry_tree_at(
        parent: &File,
        name: &std::ffi::OsStr,
        expected_identity: GeometryFileIdentity,
        display_path: &Path,
    ) -> Result<()> {
        let root = File::from(
            openat(
                parent,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, display_path.to_path_buf()))?,
        );
        let opened = root
            .metadata()
            .map_err(|error| Error::IO(error, display_path.to_path_buf()))?;
        if !opened.is_dir()
            || checked_geometry_file_identity(&opened, display_path)? != expected_identity
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry GC root changed while being opened",
                ),
                display_path.to_path_buf(),
            ));
        }
        let mut entries_seen = 0_usize;
        Self::remove_geometry_directory_contents_at(&root, display_path, 0, &mut entries_seen)?;
        let final_entry = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, display_path.to_path_buf()))?;
        if RustixFileType::from_raw_mode(final_entry.st_mode) != RustixFileType::Directory
            || geometry_stat_identity(&final_entry) != expected_identity
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry GC root entry changed before removal",
                ),
                display_path.to_path_buf(),
            ));
        }
        drop(root);
        unlinkat(parent, name, AtFlags::REMOVEDIR)
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, display_path.to_path_buf()))?;
        parent
            .sync_all()
            .map_err(|error| Error::IO(error, display_path.to_path_buf()))
    }

    #[cfg(not(all(unix, not(any(target_os = "espidf", target_os = "redox")))))]
    fn remove_authenticated_geometry_tree_at(
        _parent: &File,
        _name: &std::ffi::OsStr,
        _expected_identity: GeometryFileIdentity,
        display_path: &Path,
    ) -> Result<()> {
        Err(Error::IO(
            std::io::Error::new(
                ErrorKind::Unsupported,
                "descriptor-relative lane geometry GC is unsupported on this platform",
            ),
            display_path.to_path_buf(),
        ))
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
        let root_name = root.file_name().map(ToOwned::to_owned).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry archive root has no name",
            )
        })?;
        let quarantine_name = quarantine
            .file_name()
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidInput,
                    "lane geometry GC quarantine has no name",
                )
            })?;
        let (archive_parent_handle, archive_parent_identity) =
            self.open_geometry_parent(&archive_parent)?;
        self.require_geometry_path_identity(&archive_parent, true, archive_parent_identity)?;

        // The archive root and its quarantine name are both below the counted retired-geometry
        // tree. Register the entire rename/deletion window so a concurrent usage scan cannot
        // publish a pre-quarantine snapshot after the archive has been removed.
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let deletion_root = if root_exists {
            let (_, identity) =
                self.authenticate_geometry_archive(&root, pending, merge_releases)?;
            self.require_geometry_path_identity(&root, true, identity)?;
            self.inject_geometry_move_target_collision_for_test(&quarantine, true)?;
            self.require_geometry_path_identity(&root, true, identity)?;
            self.require_geometry_path_identity(&archive_parent, true, archive_parent_identity)?;
            self.inject_geometry_move_parent_substitution_for_test(&archive_parent)?;
            rename_geometry_path_noreplace_at(
                &archive_parent_handle,
                &root_name,
                &archive_parent_handle,
                &quarantine_name,
            )
            .map_err(|error| Error::IO(error, root.clone()))?;
            archive_parent_handle
                .sync_all()
                .map_err(|error| Error::IO(error, archive_parent.clone()))?;
            self.require_geometry_path_identity(&archive_parent, true, archive_parent_identity)?;
            self.require_geometry_path_identity(&quarantine, true, identity)?;
            self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_ARCHIVE_QUARANTINE)?;
            quarantine
        } else {
            quarantine
        };

        // Revalidate after quarantine promotion. The root identity check prevents a path swap
        // between authentication and rename from turning this into an arbitrary tree deletion.
        // Descent and unlinking remain relative to the already-authenticated parent/root handles,
        // so an ancestor substitution after this check cannot redirect removal through a symlink.
        self.require_geometry_path_identity(&archive_parent, true, archive_parent_identity)?;
        let (bytes, identity) =
            self.authenticate_geometry_archive(&deletion_root, pending, merge_releases)?;
        self.require_geometry_path_identity(&deletion_root, true, identity)?;
        self.require_geometry_path_identity(&archive_parent, true, archive_parent_identity)?;
        Self::remove_authenticated_geometry_tree_at(
            &archive_parent_handle,
            &quarantine_name,
            identity,
            &deletion_root,
        )?;
        self.update_disk_usage_delta(bytes, 0);
        accounting_mutation.finish();
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
        // Snapshot-proven GC owns prune -> canonical-chain -> geometry. Take
        // sidecar last and keep it live through every bound-file and Native
        // AMX evidence check.
        let lane_artifacts = blocks_path.join(LANE_ARTIFACTS_DIR_NAME);
        let _sidecar_guard = self.sidecar_lock.lock();
        if !self.validate_path_kind(&lane_artifacts, true)? {
            let blocks_guard = Self::open_bound_progress_directory(&self.store_root, blocks_path)?;
            if self.validate_path_kind(&lane_artifacts, true)?
                || !self.geometry_bound_progress_directory_unchanged(&blocks_guard)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane artifact namespace changed while proving it empty",
                ));
            }
            return Ok(());
        }
        let lane_artifacts_guard =
            Self::open_bound_progress_directory(&self.store_root, &lane_artifacts)?;
        let artifact_snapshot = self.geometry_bound_progress_directory_snapshot(
            &lane_artifacts_guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "retired lane artifact scan",
        )?;
        let paths =
            |data: &str, index: &str| (lane_artifacts.join(data), lane_artifacts.join(index));
        let (lane_data, lane_index) = paths(LANE_ARTIFACTS_DATA_FILE, LANE_ARTIFACTS_INDEX_FILE);
        let (certified_data, certified_index) = paths(
            CERTIFIED_LANE_BLOCKS_DATA_FILE,
            CERTIFIED_LANE_BLOCKS_INDEX_FILE,
        );
        let (input_data, input_index) = paths(
            LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
            LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE,
        );
        let (preflight_data, preflight_index) = paths(
            LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
            LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE,
        );
        let (merge_bundle_data, merge_bundle_index) = paths(
            AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE,
            AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE,
        );
        let (receipt_data, receipt_index) = paths(
            LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
            LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE,
        );
        let native_receipt_latest =
            lane_artifacts.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE);
        let merge_application_frontier = lane_artifacts.join(LANE_MERGE_APPLICATION_FRONTIER_FILE);
        if let Some(bytes) = self.read_regular_sidecar_bytes(
            &merge_application_frontier,
            &lane_artifacts,
            super::LANE_MERGE_APPLICATION_FRONTIER_MAX_BYTES,
        )? {
            let frontier = norito::decode_from_bytes::<LaneMergeApplicationFrontierV1>(&bytes)
                .map_err(Error::NoritoFrame)?;
            if norito::to_bytes(&frontier).map_err(Error::NoritoFrame)? != bytes
                || frontier.version != LaneMergeApplicationFrontierV1::VERSION
                || frontier.lane_id != binding.lane_id
                || frontier.lane_incarnation != binding.incarnation
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane merge application frontier is non-canonical or stale",
                ));
            }
            let expected = self
                .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
                    &frontier,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired lane merge application frontier has no authenticated carrier",
                    )
                })?;
            let release_receipt_hash = Hash::new_from_chunks(&[
                MERGE_RELEASE_RECEIPT_DOMAIN,
                expected.encode_framed()?.as_slice(),
            ]);
            if !merge_releases.iter().any(|release| {
                release.lane_id == frontier.lane_id
                    && release.dataspace_id == frontier.dataspace_id
                    && release.lane_incarnation == frontier.lane_incarnation
                    && release.lane_block_height == frontier.lane_block_height
                    && release.application_block_height == frontier.application_block_height
                    && release.application_block_hash == frontier.application_block_hash
                    && release.merge_entry_hash == frontier.merge_entry_hash
                    && release.merge_epoch_id == frontier.merge_epoch_id
                    && release.receipt_hash == release_receipt_hash
            }) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "retired lane merge application frontier is not snapshot-proven",
                ));
            }
        }

        for (raw_name, snapshot) in &artifact_snapshot {
            let path = lane_artifacts.join(raw_name);
            if snapshot.kind == BoundProgressDirectoryEntryKind::Symlink {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane artifact archive contains a symlink",
                    ),
                    path,
                ));
            }
            let name = raw_name.to_str().ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane artifact archive contains a non-UTF-8 filename",
                    ),
                    path.clone(),
                )
            })?;
            if name == HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1 {
                if snapshot.kind != BoundProgressDirectoryEntryKind::Directory {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "retired historical recovery namespace is not a directory",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if snapshot.kind != BoundProgressDirectoryEntryKind::File {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane artifact archive contains a non-regular entry",
                    ),
                    path,
                ));
            }
            if name.ends_with(".tmp") {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "retired lane has an ambiguous temporary artifact",
                    ),
                    path,
                ));
            }
            if name == LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_BUILD_FILE {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "retired lane has an unresolved latest-certified frontier build",
                ));
            }
            if matches!(
                name,
                LANE_ARTIFACTS_DATA_FILE
                    | LANE_ARTIFACTS_INDEX_FILE
                    | CERTIFIED_LANE_BLOCKS_DATA_FILE
                    | CERTIFIED_LANE_BLOCKS_INDEX_FILE
                    | LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_FILE
                    | LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE
                    | LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE
                    | LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE
                    | LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE
                    | AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE
                    | AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE
                    | LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE
                    | LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE
                    | NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE
                    | LANE_MERGE_APPLICATION_FRONTIER_FILE
            ) {
                continue;
            }
            if Self::parse_native_amx_evidence_path(&path)?.is_some() {
                continue;
            }
            if Self::autonomous_lane_block_attempt_coordinates(name).is_some()
                || Self::autonomous_lifecycle_cursor_coordinates(name).is_some()
                || Self::autonomous_two_height_coordinates(
                    name,
                    AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
                )
                .is_some()
                || Self::autonomous_one_height_coordinate(
                    name,
                    AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
                )
                .is_some()
                || name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE
            {
                continue;
            }
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane artifact archive contains an unexpected artifact",
                ),
                path,
            ));
        }
        let (historical_recoveries, historical_recovery_bytes) = self
            .read_geometry_historical_autonomous_recovery_records(
                &lane_artifacts,
                &artifact_snapshot,
                binding.lane_id,
                None,
                binding.incarnation,
                binding.activation_height,
                MAX_GEOMETRY_ARCHIVE_ENTRIES,
                self.historical_autonomous_recovery_aggregate_byte_limit(),
                "retired lane",
            )?;
        if artifact_snapshot
            .len()
            .checked_add(historical_recoveries.len())
            .is_none_or(|count| count > MAX_GEOMETRY_ARCHIVE_ENTRIES)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retired lane historical recovery files exceed the archive entry bound",
            ));
        }
        let historical_recoveries = historical_recoveries
            .into_iter()
            .map(|record| {
                (
                    record.payload.origin_proposal.descriptor.lane_block_height,
                    record,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let lane_bound = self.open_geometry_bound_progress_sidecar(&lane_data, &lane_index)?;
        self.ensure_geometry_progress_pair_uses_directory(
            &lane_bound,
            &lane_artifacts_guard,
            &lane_data,
            &lane_index,
            "retired lane-block artifact",
        )?;
        let autonomous = self.read_geometry_autonomous_attempt_namespace(
            &lane_artifacts,
            binding.lane_id,
            None,
            binding.incarnation,
            binding.activation_height,
            None,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            true,
        )?;

        let mut certified_bound =
            self.open_geometry_bound_progress_sidecar(&certified_data, &certified_index)?;
        self.ensure_geometry_progress_pair_uses_directory(
            &certified_bound,
            &lane_artifacts_guard,
            &certified_data,
            &certified_index,
            "retired certified lane block",
        )?;
        let certified_heights = certified_bound.sidecar_mut().map_or_else(
            || Ok(BTreeSet::new()),
            |bound| {
                self.bound_indexed_sidecar_payload_heights(
                    bound,
                    "retired certified lane block",
                    MAX_GEOMETRY_ARCHIVE_ENTRIES,
                )
            },
        )?;
        let mut merge_bundle_bound =
            self.open_geometry_bound_progress_sidecar(&merge_bundle_data, &merge_bundle_index)?;
        self.ensure_geometry_progress_pair_uses_directory(
            &merge_bundle_bound,
            &lane_artifacts_guard,
            &merge_bundle_data,
            &merge_bundle_index,
            "retired autonomous merge bundle",
        )?;
        let merge_bundle_heights = merge_bundle_bound.sidecar_mut().map_or_else(
            || Ok(BTreeSet::new()),
            |bound| {
                self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                    .map(|(_, heights)| heights)
                    .map_err(|message| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!("retired autonomous merge bundle pair is invalid: {message}"),
                        )
                    })
            },
        )?;
        let mut receipt_bound =
            self.open_geometry_bound_progress_sidecar(&receipt_data, &receipt_index)?;
        self.ensure_geometry_progress_pair_uses_directory(
            &receipt_bound,
            &lane_artifacts_guard,
            &receipt_data,
            &receipt_index,
            "retired lane application receipt",
        )?;
        let _receipt_heights = receipt_bound.sidecar_mut().map_or_else(
            || Ok(BTreeSet::new()),
            |bound| {
                self.bound_indexed_sidecar_payload_heights(
                    bound,
                    "retired lane application receipt",
                    MAX_GEOMETRY_ARCHIVE_ENTRIES,
                )
            },
        )?;
        let (retained_native_manifests, retained_native_receipts) = self
            .read_geometry_native_amx_per_height_evidence(
                &lane_artifacts,
                &artifact_snapshot,
                self.native_amx_participant_evidence_retention().get(),
                "retired lane",
            )?;
        let native_manifest_heights = retained_native_manifests
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let native_receipt_heights = retained_native_receipts
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        if !native_amx_retained_windows_are_complete(
            &native_manifest_heights,
            &native_receipt_heights,
        ) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retired Native AMX manifest/receipt evidence is not an exact contiguous retained suffix",
            ));
        }
        Self::validate_native_amx_retained_history_continuity(
            &retained_native_manifests,
            &retained_native_receipts,
            false,
        )
        .map_err(|message| {
            self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("retired Native AMX retained history is invalid: {message}"),
            )
        })?;
        let mut native_manifests = BTreeMap::new();
        for (lane_block_height, manifest) in retained_native_manifests {
            if manifest.leaf.lane_incarnation != binding.incarnation
                || !self
                    .native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(
                        &manifest,
                    )
                || native_manifests
                    .insert(lane_block_height, manifest)
                    .is_some()
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired Native AMX participant manifest has stale, duplicate, or unverifiable evidence",
                ));
            }
        }
        let mut latest_native_receipt = None;
        for (lane_block_height, receipt) in retained_native_receipts {
            let Some(manifest) = native_manifests.get(&lane_block_height) else {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired Native AMX participant receipt has no manifest proof",
                ));
            };
            if receipt.participant_proposal.descriptor.lane_incarnation != binding.incarnation
                || receipt.manifest_artifact_hash != HashOf::new(manifest)
                || !Self::native_amx_participant_receipt_matches_manifest_leaf(
                    &receipt,
                    &manifest.leaf,
                )
                || !self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(
                    &receipt,
                    manifest,
                )
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired Native AMX participant receipt has stale or unverifiable evidence",
                ));
            }
            if native_receipt_heights.last().copied() == Some(lane_block_height) {
                latest_native_receipt = Some(receipt);
            }
        }
        let latest_native_index = match latest_native_receipt.as_ref() {
            Some(receipt) => self
                .decode_native_amx_participant_receipt_latest_index_for_route(
                    binding.lane_id,
                    receipt.participant_proposal.descriptor.dataspace_id,
                    &native_receipt_latest,
                )
                .map_err(|error| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!("retired Native AMX latest index is malformed: {error}"),
                    )
                })?,
            None if native_receipt_latest.exists() => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired Native AMX latest index has no receipt pair",
                ));
            }
            None => None,
        };
        match latest_native_receipt.as_ref() {
            Some(receipt)
                if latest_native_index.is_some_and(|latest| {
                    latest.lane_incarnation == binding.incarnation
                        && latest.matches_receipt(receipt)
                }) => {}
            Some(_) => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired Native AMX latest index does not match the highest receipt",
                ));
            }
            None if latest_native_index.is_some() => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired Native AMX latest index has no receipt pair",
                ));
            }
            None => {}
        }
        let mut work_heights = certified_heights.clone();
        work_heights.extend(merge_bundle_heights.iter().copied());
        for (lane_block_height, (_, _, retired)) in &autonomous {
            if *retired {
                if certified_heights.contains(lane_block_height) {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired autonomous slot conflicts with a certified lane block",
                    ));
                }
            } else {
                work_heights.insert(*lane_block_height);
            }
        }
        work_heights.extend(historical_recoveries.keys().copied());
        let mut input_bound =
            self.open_geometry_bound_progress_sidecar(&input_data, &input_index)?;
        self.ensure_geometry_progress_pair_uses_directory(
            &input_bound,
            &lane_artifacts_guard,
            &input_data,
            &input_index,
            "retired lane execution input",
        )?;
        work_heights.extend(input_bound.sidecar_mut().map_or_else(
            || Ok(BTreeSet::new()),
            |bound| {
                self.bound_indexed_sidecar_payload_heights(
                    bound,
                    "retired lane execution input",
                    MAX_GEOMETRY_ARCHIVE_ENTRIES,
                )
            },
        )?);
        let mut preflight_bound =
            self.open_geometry_bound_progress_sidecar(&preflight_data, &preflight_index)?;
        self.ensure_geometry_progress_pair_uses_directory(
            &preflight_bound,
            &lane_artifacts_guard,
            &preflight_data,
            &preflight_index,
            "retired lane execution preflight",
        )?;
        work_heights.extend(preflight_bound.sidecar_mut().map_or_else(
            || Ok(BTreeSet::new()),
            |bound| {
                self.bound_indexed_sidecar_payload_heights(
                    bound,
                    "retired lane execution preflight",
                    MAX_GEOMETRY_ARCHIVE_ENTRIES,
                )
            },
        )?);

        for lane_block_height in work_heights {
            if !certified_heights.contains(&lane_block_height) {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "retired lane has work without a certified settlement artifact",
                ));
            }
            let certified = self
                .read_certified_lane_block_artifact_from_bound_locked(
                    binding.lane_id,
                    lane_block_height,
                    certified_bound
                        .sidecar_mut()
                        .expect("non-empty archived work has a bound certified sidecar"),
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired lane certified artifact is malformed or incomplete",
                    )
                })?;
            let merge_bundle = if merge_bundle_heights.contains(&lane_block_height) {
                Some(
                    self.read_autonomous_lane_merge_bundle_from_bound_locked(
                        binding.lane_id,
                        lane_block_height,
                        merge_bundle_bound.sidecar_mut().ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::InvalidData,
                                "retired autonomous merge bundle sidecar disappeared",
                            )
                        })?,
                    )
                    .map_err(|message| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!("retired autonomous merge bundle is invalid: {message}"),
                        )
                    })?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "retired autonomous merge bundle slot disappeared",
                        )
                    })?
                    .0,
                )
            } else {
                None
            };
            let certified_is_autonomous = certified.prepare_qc.payload_availability_qc.is_some();
            if certified_is_autonomous != merge_bundle.is_some() {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired certified autonomous work and its durable merge bundle are incomplete",
                ));
            }
            if let Some(bundle) = merge_bundle.as_ref() {
                let payload = bundle.executable_payload();
                let expected_input = Self::autonomous_lane_block_execution_input_candidate(
                    payload,
                    payload.chain_id_hash,
                    payload.epoch,
                )
                .map_err(|availability| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!(
                            "retired autonomous merge bundle input is invalid: {availability:?}"
                        ),
                    )
                })?;
                let actual_input = self
                    .read_geometry_execution_input_from_bound(
                        binding.lane_id,
                        lane_block_height,
                        input_bound.sidecar_mut().ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::InvalidData,
                                "retired autonomous merge bundle has no execution-input sidecar",
                            )
                        })?,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "retired autonomous merge bundle execution input is unreadable",
                        )
                    })?;
                if bundle.certified != certified
                    || actual_input != expected_input
                    || autonomous.get(&lane_block_height).is_some_and(
                        |(autonomous_artifact, _, retired)| {
                            *retired || &bundle.autonomous != autonomous_artifact
                        },
                    )
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired autonomous merge bundle differs from its exact durable evidence",
                    ));
                }
            }
            if let Some(record) = historical_recoveries.get(&lane_block_height) {
                let (autonomous_artifact, _, retired) =
                    autonomous.get(&lane_block_height).ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "retired historical recovery has no autonomous payload",
                        )
                    })?;
                let expected_input = Self::autonomous_lane_block_execution_input_candidate(
                    &record.payload,
                    record.payload.chain_id_hash,
                    record.payload.epoch,
                )
                .map_err(|availability| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!("retired historical recovery input is invalid: {availability:?}"),
                    )
                })?;
                let actual_input = self
                    .read_geometry_execution_input_from_bound(
                        binding.lane_id,
                        lane_block_height,
                        input_bound.sidecar_mut().ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::InvalidData,
                                "retired historical recovery has no execution-input sidecar",
                            )
                        })?,
                    )
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "retired historical recovery execution input is unreadable",
                        )
                    })?;
                if *retired
                    || autonomous_artifact.executable_payload != record.payload
                    || actual_input != expected_input
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired historical recovery differs from its payload or execution input",
                    ));
                }
            }
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
                .read_lane_block_application_receipt_from_bound_locked(
                    binding.lane_id,
                    lane_block_height,
                    receipt_bound.sidecar_mut().ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::WouldBlock,
                            "retired lane merge application receipt is missing or malformed",
                        )
                    })?,
                )
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::WouldBlock,
                        "retired lane merge application receipt is missing or malformed",
                    )
                })?;
            if receipt.format != LaneBlockApplicationReceiptArtifactFormat::MergeExecution
                || receipt.proposal != certified.proposal
                || !self
                    .lane_block_application_receipt_matches_merge_log_under_prune_and_canonical_guards(
                        &receipt,
                    )
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired lane merge receipt does not match its certified artifact and merge log",
                ));
            }
            if let Some((autonomous_artifact, current, retired)) =
                autonomous.get(&lane_block_height)
            {
                if *retired
                    || current != &certified.proposal
                    || !self.lane_retirement_merge_receipt_applies_autonomous_payload(
                        &receipt,
                        &autonomous_artifact.executable_payload,
                    )
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "retired autonomous lane evidence differs from its certified merge execution",
                    ));
                }
            }
            if merge_bundle
                .as_ref()
                .is_some_and(|bundle| bundle.bundle_hash().ok() != Some(release.source_bundle_hash))
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "retired autonomous merge bundle hash differs from the snapshot release proof",
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
        if certified_bound.sidecar().is_some_and(|bound| {
            !self.sync_bound_progress_sidecar(bound, "retired certified lane block")
        }) {
            return Err(self.geometry_error(
                ErrorKind::WouldBlock,
                "retired certified lane block durability attestation failed",
            ));
        }
        if merge_bundle_bound.sidecar().is_some_and(|bound| {
            !self.sync_bound_progress_sidecar(bound, AutonomousLaneMergeBundleV1::FORMAT_LABEL)
        }) {
            return Err(self.geometry_error(
                ErrorKind::WouldBlock,
                "retired autonomous merge bundle durability attestation failed",
            ));
        }
        if let Some(bound) = receipt_bound.sidecar() {
            #[cfg(test)]
            inject_archived_receipt_durability_fault_for_test();
            if !self.sync_bound_progress_sidecar(bound, "retired lane application receipt") {
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "retired lane merge application receipt durability attestation failed",
                ));
            }
        }
        for (pair, data, index, kind, failure) in [
            (
                &lane_bound,
                &lane_data,
                &lane_index,
                "retired lane-block artifact",
                "retired lane-block artifact durability attestation failed",
            ),
            (
                &input_bound,
                &input_data,
                &input_index,
                "retired lane execution input",
                "retired lane execution input durability attestation failed",
            ),
            (
                &preflight_bound,
                &preflight_data,
                &preflight_index,
                "retired lane execution preflight",
                "retired lane execution preflight durability attestation failed",
            ),
        ] {
            if pair
                .sidecar()
                .is_some_and(|bound| !self.sync_bound_progress_sidecar(bound, kind))
            {
                return Err(self.geometry_error(ErrorKind::WouldBlock, failure));
            }
            self.ensure_absent_geometry_progress_sidecar_remains_absent(pair, data, index)?;
        }
        self.ensure_absent_geometry_progress_sidecar_remains_absent(
            &certified_bound,
            &certified_data,
            &certified_index,
        )?;
        self.ensure_absent_geometry_progress_sidecar_remains_absent(
            &merge_bundle_bound,
            &merge_bundle_data,
            &merge_bundle_index,
        )?;
        self.ensure_absent_geometry_progress_sidecar_remains_absent(
            &receipt_bound,
            &receipt_data,
            &receipt_index,
        )?;
        let (confirmed_historical_recoveries, confirmed_historical_recovery_bytes) = self
            .read_geometry_historical_autonomous_recovery_records(
                &lane_artifacts,
                &artifact_snapshot,
                binding.lane_id,
                None,
                binding.incarnation,
                binding.activation_height,
                historical_recoveries.len(),
                historical_recovery_bytes,
                "retired lane rescan",
            )?;
        let confirmed_historical_recoveries = confirmed_historical_recoveries
            .into_iter()
            .map(|record| {
                (
                    record.payload.origin_proposal.descriptor.lane_block_height,
                    record,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if confirmed_historical_recoveries != historical_recoveries
            || confirmed_historical_recovery_bytes != historical_recovery_bytes
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retired historical recovery namespace changed during release validation",
            ));
        }
        let confirmed_snapshot = self.geometry_bound_progress_directory_snapshot(
            &lane_artifacts_guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "retired lane artifact rescan",
        )?;
        if confirmed_snapshot != artifact_snapshot
            || !self.geometry_bound_progress_directory_unchanged(&lane_artifacts_guard)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retired lane artifact namespace changed during release validation",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn ensure_archived_lane_work_released_for_test(
        &self,
        blocks_path: &Path,
        binding: &LaneGeometryBinding,
        merge_releases: &[LaneGeometryMergeRelease],
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        self.ensure_archived_lane_work_released(blocks_path, binding, merge_releases)
    }

    fn open_geometry_bound_progress_sidecar(
        &self,
        data_path: &Path,
        index_path: &Path,
    ) -> Result<BoundProgressPair> {
        self.open_bound_progress_pair(data_path, index_path)
    }

    /// Recover the fixed progress pairs before freezing the retirement namespace.
    ///
    /// Every recovery operation preserves one authenticated directory-object identity. Recovery
    /// may legitimately change directory timestamps while promoting or removing a temp, so its
    /// handle is rebound and same-object checked after each pair; the immutable scan receives a
    /// final fresh handle only after the same identity proof.
    fn recover_geometry_progress_pairs_before_snapshot(
        &self,
        lane_artifacts: &Path,
        pairs: &[(&Path, &Path, &str)],
        context: &str,
    ) -> Result<BoundProgressDirectory> {
        if !super::sumeragi_v2_validator_storage_supported() {
            return Err(self.geometry_error_owned(
                ErrorKind::Unsupported,
                format!(
                    "{context} requires the first-release Linux/macOS validator-storage contract"
                ),
            ));
        }
        if pairs.is_empty() {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidInput,
                format!("{context} has no fixed progress-sidecar pairs"),
            ));
        }
        let mut recovery_directory =
            Self::open_bound_progress_directory(&self.store_root, lane_artifacts)?;
        for &(data_path, index_path, kind) in pairs {
            if data_path.parent() != Some(lane_artifacts)
                || index_path.parent() != Some(lane_artifacts)
            {
                return Err(self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!("{kind} is outside the authenticated lane-artifact directory"),
                ));
            }
            let pair_namespace = self.open_bound_progress_namespace(data_path, index_path)?;
            self.ensure_geometry_progress_namespace_uses_directory(
                &pair_namespace,
                &recovery_directory,
                data_path,
                index_path,
                kind,
            )?;
            if let Err(failure) = self
                .recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &pair_namespace,
                    data_path,
                    index_path,
                    kind,
                )
            {
                let error_kind = match failure {
                    BoundProgressRecoveryFailure::RetryableIo => ErrorKind::WouldBlock,
                    BoundProgressRecoveryFailure::InvalidData => ErrorKind::InvalidData,
                };
                return Err(self.geometry_error_owned(
                    error_kind,
                    format!("{kind} recovery did not reach a durable fixed point"),
                ));
            }
            if !Self::progress_mutation_namespace_unchanged(&pair_namespace) {
                return Err(self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!("{kind} namespace changed during progress recovery"),
                ));
            }
            #[cfg(test)]
            maybe_substitute_progress_directory_after_recovery_for_test(kind, lane_artifacts);
            // Promotion or cleanup legitimately changes directory timestamps.
            // Rebind the guard after each pair, while proving that the path
            // still names the same directory object, so the next pair is not
            // compared with stale mutation metadata.
            let refreshed_directory =
                Self::open_bound_progress_directory(&self.store_root, lane_artifacts)?;
            if recovery_directory.expected_path != refreshed_directory.expected_path
                || recovery_directory.canonical_path != refreshed_directory.canonical_path
                || !Self::sidecar_metadata_same_object(
                    &recovery_directory.metadata,
                    &refreshed_directory.metadata,
                )
                || !self.geometry_bound_progress_directory_unchanged(&refreshed_directory)
            {
                return Err(self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!("{kind} artifact directory changed during progress recovery"),
                ));
            }
            recovery_directory = refreshed_directory;
        }
        let immutable_directory =
            Self::open_bound_progress_directory(&self.store_root, lane_artifacts)?;
        if recovery_directory.expected_path != immutable_directory.expected_path
            || recovery_directory.canonical_path != immutable_directory.canonical_path
            || !Self::sidecar_metadata_same_object(
                &recovery_directory.metadata,
                &immutable_directory.metadata,
            )
            || !self.geometry_bound_progress_directory_unchanged(&immutable_directory)
        {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} artifact namespace changed before its immutable scan"),
            ));
        }
        Ok(immutable_directory)
    }

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    fn geometry_bound_progress_directory_snapshot(
        &self,
        directory: &BoundProgressDirectory,
        max_entries: usize,
        context: &str,
    ) -> Result<BoundProgressDirectorySnapshot> {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

        if !self.geometry_bound_progress_directory_unchanged(directory) {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} directory changed before descriptor-bound enumeration"),
            ));
        }
        let mut snapshot = BTreeMap::new();
        let mut entries = Dir::read_from(&directory.file)
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, directory.expected_path.clone()))?;
        for entry in &mut entries {
            let entry = entry
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, directory.expected_path.clone()))?;
            if matches!(entry.file_name().to_bytes(), b"." | b"..") {
                continue;
            }
            if snapshot.len() >= max_entries {
                return Err(self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!("{context} exceeds its bounded directory-entry count"),
                ));
            }
            let name = entry.file_name();
            let path = directory
                .expected_path
                .join(OsStr::from_bytes(name.to_bytes()));
            let metadata = statat(&directory.file, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .map_err(|error| Error::IO(error, path.clone()))?;
            let file_type = RustixFileType::from_raw_mode(metadata.st_mode);
            let kind = if file_type == RustixFileType::RegularFile {
                BoundProgressDirectoryEntryKind::File
            } else if file_type == RustixFileType::Directory {
                BoundProgressDirectoryEntryKind::Directory
            } else if file_type == RustixFileType::Symlink {
                BoundProgressDirectoryEntryKind::Symlink
            } else {
                BoundProgressDirectoryEntryKind::Other
            };
            let previous = snapshot.insert(
                OsStr::from_bytes(name.to_bytes()).to_os_string(),
                BoundProgressDirectoryEntrySnapshot {
                    kind,
                    identity: geometry_stat_identity(&metadata),
                },
            );
            if previous.is_some() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} contains a duplicate directory entry"),
                    ),
                    path,
                ));
            }
        }
        if !self.geometry_bound_progress_directory_unchanged(directory) {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} directory changed during descriptor-bound enumeration"),
            ));
        }
        Ok(snapshot)
    }

    #[cfg(not(all(unix, not(any(target_os = "espidf", target_os = "redox")))))]
    fn geometry_bound_progress_directory_snapshot(
        &self,
        directory: &BoundProgressDirectory,
        max_entries: usize,
        context: &str,
    ) -> Result<BoundProgressDirectorySnapshot> {
        if !self.geometry_bound_progress_directory_unchanged(directory) {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} directory changed before enumeration"),
            ));
        }
        let mut snapshot = BTreeMap::new();
        for entry in fs::read_dir(&directory.expected_path)
            .map_err(|error| Error::IO(error, directory.expected_path.clone()))?
        {
            let entry = entry.map_err(|error| Error::IO(error, directory.expected_path.clone()))?;
            if snapshot.len() >= max_entries {
                return Err(self.geometry_error_owned(
                    ErrorKind::InvalidData,
                    format!("{context} exceeds its bounded directory-entry count"),
                ));
            }
            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| Error::IO(error, path.clone()))?;
            let file_type = metadata.file_type();
            let kind = if file_type.is_file() {
                BoundProgressDirectoryEntryKind::File
            } else if file_type.is_dir() {
                BoundProgressDirectoryEntryKind::Directory
            } else if file_type.is_symlink() {
                BoundProgressDirectoryEntryKind::Symlink
            } else {
                BoundProgressDirectoryEntryKind::Other
            };
            let previous = snapshot.insert(
                entry.file_name(),
                BoundProgressDirectoryEntrySnapshot {
                    kind,
                    identity: checked_geometry_file_identity(&metadata, &path)?,
                },
            );
            if previous.is_some() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} contains a duplicate directory entry"),
                    ),
                    path,
                ));
            }
        }
        if !self.geometry_bound_progress_directory_unchanged(directory) {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} directory changed during enumeration"),
            ));
        }
        Ok(snapshot)
    }

    fn ensure_geometry_progress_pair_uses_directory(
        &self,
        pair: &BoundProgressPair,
        directory: &BoundProgressDirectory,
        data_path: &Path,
        index_path: &Path,
        context: &str,
    ) -> Result<()> {
        let namespace = match pair {
            BoundProgressPair::Absent(namespace) => namespace,
            BoundProgressPair::Present(sidecar) => &sidecar.namespace,
        };
        self.ensure_geometry_progress_namespace_uses_directory(
            namespace, directory, data_path, index_path, context,
        )
    }

    fn ensure_geometry_progress_namespace_uses_directory(
        &self,
        namespace: &BoundProgressNamespace,
        directory: &BoundProgressDirectory,
        data_path: &Path,
        index_path: &Path,
        context: &str,
    ) -> Result<()> {
        let immediate = namespace.directories.first().ok_or_else(|| {
            self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} has no bound immediate directory"),
            )
        })?;
        if namespace.data_path != data_path
            || namespace.index_path != index_path
            || data_path.parent() != Some(directory.expected_path.as_path())
            || index_path.parent() != Some(directory.expected_path.as_path())
            || immediate.expected_path != directory.expected_path
            || immediate.canonical_path != directory.canonical_path
            || !Self::sidecar_directory_metadata_unchanged(&directory.metadata, &immediate.metadata)
            || !self.geometry_bound_progress_directory_unchanged(directory)
            || !self.bound_progress_namespace_unchanged(namespace)
        {
            return Err(self.geometry_error_owned(
                ErrorKind::InvalidData,
                format!("{context} is not bound to the authenticated lane-artifact directory"),
            ));
        }
        Ok(())
    }

    fn read_geometry_execution_input_from_bound(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        bound: &mut super::BoundProgressSidecar,
    ) -> Option<LaneBlockExecutionInputArtifact> {
        let artifact = Self::read_indexed_sidecar_from_open_files(
            lane_block_height,
            &mut bound.data,
            &mut bound.index,
            &bound.namespace.data_path,
            &bound.namespace.index_path,
            norito::decode_from_bytes::<LaneBlockExecutionInputArtifact>,
            "lane block execution input",
        )?;
        let descriptor = &artifact.proposal.descriptor;
        if descriptor.lane_id != lane_id || descriptor.lane_block_height != lane_block_height {
            return None;
        }
        Self::validate_lane_block_execution_input_artifact(&artifact)
            .is_ok()
            .then_some(artifact)
    }

    #[allow(clippy::too_many_arguments)]
    fn read_geometry_autonomous_attempt_namespace(
        &self,
        lane_artifacts: &Path,
        lane_id: LaneId,
        expected_dataspace_id: Option<DataSpaceId>,
        expected_incarnation: Hash,
        activation_height: u64,
        active_entry: Option<&LaneConfigEntry>,
        entry_limit: usize,
        require_terminal_lifecycle: bool,
    ) -> Result<BTreeMap<u64, (AutonomousLaneBlockArtifact, LaneBlockProposalV1, bool)>> {
        let entry_limit = entry_limit.min(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES);
        let lifecycle_process_generation = self
            .read_autonomous_lifecycle_process_generation_record()?
            .map(|(record, _)| record);
        let mut attempts = BTreeMap::<
            u64,
            Vec<(
                AutonomousLaneBlockLatestAttemptV1,
                AutonomousLaneBlockArtifact,
                LaneBlockProposalV1,
                bool,
            )>,
        >::new();
        let mut attempt_identities = BTreeSet::new();
        let mut view_identities = BTreeSet::new();
        let mut height_pointers = BTreeMap::new();
        let mut route_pointer = None;
        let mut lifecycle_cursors = BTreeMap::<(u64, u64), AutonomousLifecycleCursorV2>::new();
        let mut lifecycle_bootstraps =
            BTreeMap::<(u64, u64), (PathBuf, AutonomousLifecycleBootstrapV1)>::new();
        let entries = match fs::read_dir(lane_artifacts) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(error) => return Err(Error::IO(error, lane_artifacts.to_path_buf())),
        };
        let mut related_entries = 0_usize;
        let mut related_bytes = 0_u64;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, lane_artifacts.to_path_buf()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous attempt namespace contains a non-UTF-8 artifact",
                    ),
                    path.clone(),
                )
            })?;
            if name.starts_with(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous attempt namespace contains a bootstrap atomic temporary",
                    ),
                    path,
                ));
            }
            if !name.starts_with("autonomous_") {
                continue;
            }
            related_entries = related_entries.checked_add(1).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace entry count overflows",
                )
            })?;
            if related_entries > entry_limit {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace exceeds its bounded entry limit",
                ));
            }
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous attempt namespace contains a non-regular, linked, or symlinked artifact",
                    ),
                    path,
                ));
            }
            related_bytes = related_bytes.checked_add(metadata.len()).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace byte count overflows",
                )
            })?;
            if related_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace exceeds the shared sidecar aggregate byte budget",
                ));
            }
            File::open(&path)
                .and_then(|file| file.sync_all())
                .map_err(|error| Error::IO(error, path.clone()))?;
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lane_block_attempt_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous attempt disappeared during geometry validation",
                        )
                    })?;
                let mut artifact = norito::decode_from_bytes::<AutonomousLaneBlockArtifact>(&bytes)
                    .map_err(Error::NoritoFrame)?;
                if artifact.encode_framed().map_err(Error::NoritoFrame)? != bytes {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous attempt is not canonical framed Norito",
                        ),
                        path,
                    ));
                }
                let pointer =
                    AutonomousLaneBlockLatestAttemptV1::from_payload(&artifact.executable_payload);
                let descriptor = &artifact.executable_payload.origin_proposal.descriptor;
                if pointer.lane_id != lane_id
                    || pointer.lane_block_height != lane_block_height
                    || pointer.proposal_height != proposal_height
                    || descriptor.lane_incarnation != expected_incarnation
                    || descriptor.proposal_height <= activation_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id)
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous attempt has a stale or namespace-conflicting route identity",
                        ),
                        path,
                    ));
                }
                if let Some(active_entry) = active_entry {
                    self.require_active_lane_artifact(active_entry, descriptor)
                        .map_err(|error| {
                            self.geometry_error_owned(
                                ErrorKind::InvalidData,
                                format!("autonomous attempt has a stale active binding: {error}"),
                            )
                        })?;
                }
                let view_path = lane_artifacts.join(format!(
                    "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{proposal_height:020}.norito"
                ));
                let view_state = self.read_autonomous_lane_block_view_state_locked(
                    &artifact.executable_payload,
                    &view_path,
                    false,
                )?;
                let retired = view_state
                    .as_ref()
                    .is_some_and(|state| state.retirement.is_some());
                if let Some(state) = view_state {
                    artifact.availability_certificate = state.availability_certificate;
                    artifact.view_checkpoint = state.checkpoint;
                    artifact.new_view_certificates = state.certificates;
                }
                let current = Self::validate_autonomous_lane_block_artifact(
                    &artifact,
                    artifact.executable_payload.chain_id_hash,
                    artifact.executable_payload.epoch,
                )
                .map_err(|message| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!("autonomous attempt is invalid: {message}"),
                    )
                })?;
                attempt_identities.insert((lane_block_height, proposal_height));
                let attempts_at_height = attempts.entry(lane_block_height).or_default();
                attempts_at_height.push((pointer, artifact, current, retired));
                if attempts_at_height.len() > self.roster_sidecar_retention().get() {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous proposal-height attempts exceed the configured sidecar retention bound",
                    ));
                }
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lifecycle_bootstrap_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap disappeared during geometry validation",
                        )
                    })?;
                let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&path, &bytes)?;
                let process_generation = lifecycle_process_generation.as_ref().ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap exists without a Kura-root process generation",
                        ),
                        path.clone(),
                    )
                })?;
                Self::validate_autonomous_lifecycle_bootstrap_process_generation(
                    process_generation,
                    &bootstrap,
                )
                .map_err(|message| {
                    Error::IO(
                        std::io::Error::new(ErrorKind::InvalidData, message),
                        path.clone(),
                    )
                })?;
                let descriptor = &bootstrap.body.executable_payload.origin_proposal.descriptor;
                if descriptor.lane_id != lane_id
                    || descriptor.lane_incarnation != expected_incarnation
                    || descriptor.proposal_height <= activation_height
                    || descriptor.lane_block_height != lane_block_height
                    || descriptor.proposal_height != proposal_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| descriptor.dataspace_id != dataspace_id)
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                if active_entry.is_none() {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle bootstrap must never be present in an archive",
                    ));
                }
                if require_terminal_lifecycle {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "lane retirement is blocked by an unfinished lifecycle bootstrap",
                    ));
                }
                if lifecycle_bootstraps
                    .insert(
                        (lane_block_height, proposal_height),
                        (path.clone(), bootstrap),
                    )
                    .is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle bootstrap has a duplicate identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lifecycle_cursor_coordinates(&name)
            {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle cursor disappeared during geometry validation",
                        )
                    })?;
                let cursor = Self::decode_autonomous_lifecycle_cursor(&path, &bytes)?;
                let binding = cursor.binding();
                if binding.lane_id != lane_id
                    || binding.lane_incarnation != expected_incarnation
                    || binding.proposal_height <= activation_height
                    || binding.lane_block_height != lane_block_height
                    || binding.proposal_height != proposal_height
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| binding.dataspace_id != dataspace_id)
                    || lifecycle_cursors
                        .insert((lane_block_height, proposal_height), cursor)
                        .is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous lifecycle cursor has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if let Some(identity) = Self::autonomous_two_height_coordinates(
                &name,
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
            ) {
                view_identities.insert(identity);
                continue;
            }
            if let Some(lane_block_height) = Self::autonomous_one_height_coordinate(
                &name,
                AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
            ) {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        super::AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous latest pointer disappeared during geometry validation",
                        )
                    })?;
                let pointer = Self::decode_autonomous_lane_block_latest_attempt(&path, &bytes)?;
                if pointer.lane_id != lane_id
                    || pointer.lane_block_height != lane_block_height
                    || pointer.lane_incarnation != expected_incarnation
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| pointer.dataspace_id != dataspace_id)
                    || height_pointers.insert(lane_block_height, pointer).is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous latest pointer has a stale, duplicate, or namespace-conflicting identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            if name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE {
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        lane_artifacts,
                        super::AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        self.geometry_error(
                            ErrorKind::InvalidData,
                            "autonomous route pointer disappeared during geometry validation",
                        )
                    })?;
                let pointer = Self::decode_autonomous_lane_block_latest_attempt(&path, &bytes)?;
                if pointer.lane_id != lane_id
                    || pointer.lane_incarnation != expected_incarnation
                    || expected_dataspace_id
                        .is_some_and(|dataspace_id| pointer.dataspace_id != dataspace_id)
                    || route_pointer.replace(pointer).is_some()
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "autonomous route pointer has a stale or duplicate identity",
                        ),
                        path,
                    ));
                }
                continue;
            }
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "unexpected or obsolete autonomous persistence artifact",
                ),
                path,
            ));
        }
        if !view_identities.is_subset(&attempt_identities) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace contains an orphan view state",
            ));
        }
        let lifecycle_identities = lifecycle_cursors.keys().copied().collect::<BTreeSet<_>>();
        if !lifecycle_identities.is_subset(&attempt_identities) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace contains an orphan lifecycle cursor",
            ));
        }
        let mut payload_only_bootstrap_identities = BTreeSet::new();
        for (identity, (path, bootstrap)) in &lifecycle_bootstraps {
            let process_generation = lifecycle_process_generation.as_ref().ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle bootstrap exists without a Kura-root process generation",
                    ),
                    path.clone(),
                )
            })?;
            Self::validate_autonomous_lifecycle_bootstrap_process_generation(
                process_generation,
                bootstrap,
            )
            .map_err(|message| {
                Error::IO(
                    std::io::Error::new(ErrorKind::InvalidData, message),
                    path.clone(),
                )
            })?;
            let active_entry = active_entry.expect(
                "an autonomous lifecycle bootstrap was rejected above without an active entry",
            );
            let stage =
                self.classify_autonomous_lifecycle_bootstrap_locked(active_entry, bootstrap)?;
            let payload_present = attempts.get(&identity.0).is_some_and(|attempts_at_height| {
                attempts_at_height.iter().any(|(pointer, artifact, _, _)| {
                    pointer.proposal_height == identity.1
                        && artifact.executable_payload == bootstrap.body.executable_payload
                })
            });
            let cursor = lifecycle_cursors.get(identity);
            let stage_matches = match stage {
                AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly => {
                    !payload_present && cursor.is_none()
                }
                AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable => {
                    payload_only_bootstrap_identities.insert(*identity);
                    payload_present && cursor.is_none()
                }
                AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable => {
                    payload_present && cursor == Some(&bootstrap.body.prepared_activate)
                }
                AutonomousLifecycleBootstrapRecoveryStage::LiveDurable => {
                    payload_present && cursor == Some(&bootstrap.body.live_activate)
                }
            };
            if !stage_matches {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle bootstrap crash boundary conflicts with geometry inventory",
                    ),
                    path.clone(),
                ));
            }
        }
        if attempt_identities.iter().any(|identity| {
            !lifecycle_cursors.contains_key(identity)
                && !payload_only_bootstrap_identities.contains(identity)
        }) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous payload attempt lacks its exact lifecycle cursor or signed payload-durable bootstrap",
            ));
        }
        // Every initial Prepared cursor requires its exact signed bootstrap authority.
        for (lane_block_height, attempts_at_height) in &attempts {
            for (pointer, artifact, _, _) in attempts_at_height {
                let identity = (*lane_block_height, pointer.proposal_height);
                let Some(cursor) = lifecycle_cursors.get(&identity) else {
                    continue;
                };
                cursor
                    .validate_for_payload(&artifact.executable_payload)
                    .map_err(|message| {
                        self.geometry_error_owned(
                            ErrorKind::InvalidData,
                            format!("autonomous lifecycle cursor is invalid: {message}"),
                        )
                    })?;
                let process_generation =
                    lifecycle_process_generation.as_ref().ok_or_else(|| {
                        self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous lifecycle cursor exists without a Kura-root process generation",
                    )
                    })?;
                Self::validate_autonomous_lifecycle_cursor_process_generation(
                    process_generation,
                    cursor,
                )
                .map_err(|message| {
                    self.geometry_error_owned(
                        ErrorKind::InvalidData,
                        format!(
                            "autonomous lifecycle cursor process generation is invalid: {message}"
                        ),
                    )
                })?;
                if cursor.sequence() == 1
                    && cursor.phase_kind() == AutonomousLifecycleCursorPhaseKindV2::Prepared
                    && !lifecycle_bootstraps.contains_key(&identity)
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "initial Prepared lifecycle cursor is orphaned from its signed bootstrap",
                    ));
                }
                if require_terminal_lifecycle
                    && !matches!(
                        cursor.phase(),
                        AutonomousLifecycleCursorPhaseV2::Terminal { .. }
                    )
                {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "lane retirement requires every autonomous lifecycle cursor to be terminal",
                    ));
                }
            }
        }
        if attempts.is_empty() {
            if !height_pointers.is_empty()
                || route_pointer.is_some()
                || !lifecycle_cursors.is_empty()
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous pointer exists without an immutable payload attempt",
                ));
            }
            return Ok(BTreeMap::new());
        }
        let mut latest_by_height = BTreeMap::new();
        let mut route_identity: Option<AutonomousLaneBlockLatestAttemptV1> = None;
        for (lane_block_height, attempts_at_height) in &mut attempts {
            attempts_at_height.sort_by_key(|(pointer, _, _, _)| pointer.proposal_height);
            for adjacent in attempts_at_height.windows(2) {
                let (previous_pointer, previous_artifact, _, previous_retired) = &adjacent[0];
                let (successor_pointer, successor_artifact, _, _) = &adjacent[1];
                let previous = &previous_artifact
                    .executable_payload
                    .origin_proposal
                    .descriptor;
                let successor = &successor_artifact
                    .executable_payload
                    .origin_proposal
                    .descriptor;
                if !previous_retired
                    || successor_pointer.proposal_height <= previous_pointer.proposal_height
                    || successor.lane_id != previous.lane_id
                    || successor.dataspace_id != previous.dataspace_id
                    || successor.lane_incarnation != previous.lane_incarnation
                    || successor.lane_block_height != previous.lane_block_height
                    || successor.previous_lane_block_height != previous.previous_lane_block_height
                    || successor.previous_lane_block_descriptor_hash
                        != previous.previous_lane_block_descriptor_hash
                    || successor_pointer.chain_id_hash != previous_pointer.chain_id_hash
                    || successor_pointer.epoch < previous_pointer.epoch
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "autonomous attempts are not a retired monotonic proposal-height chain",
                    ));
                }
            }
            let (pointer, artifact, current, retired) = attempts_at_height
                .last()
                .expect("non-empty autonomous attempt group");
            if height_pointers.get(lane_block_height) != Some(pointer) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous lane-height pointer does not select the exact latest attempt",
                ));
            }
            if let Some(route) = route_identity.as_ref()
                && (pointer.chain_id_hash != route.chain_id_hash
                    || pointer.lane_id != route.lane_id
                    || pointer.dataspace_id != route.dataspace_id
                    || pointer.lane_incarnation != route.lane_incarnation
                    || pointer.proposal_height < route.proposal_height
                    || pointer.epoch < route.epoch)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "autonomous attempt namespace regresses its route or global context",
                ));
            }
            route_identity = Some(pointer.clone());
            latest_by_height.insert(
                *lane_block_height,
                (artifact.clone(), current.clone(), *retired),
            );
        }
        if height_pointers.len() != latest_by_height.len()
            || route_pointer.as_ref() != route_identity.as_ref()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "autonomous attempt namespace has an orphan or stale latest pointer",
            ));
        }
        sync_dir(lane_artifacts).map_err(|error| Error::IO(error, lane_artifacts.to_path_buf()))?;
        Ok(latest_by_height)
    }

    fn read_geometry_execution_preflight_from_bound(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        bound: &mut super::BoundProgressSidecar,
    ) -> Option<LaneBlockExecutionPreflightArtifact> {
        let artifact = Self::read_indexed_sidecar_from_open_files(
            lane_block_height,
            &mut bound.data,
            &mut bound.index,
            &bound.namespace.data_path,
            &bound.namespace.index_path,
            norito::decode_from_bytes::<LaneBlockExecutionPreflightArtifact>,
            "lane block execution preflight",
        )?;
        let descriptor = &artifact.proposal.descriptor;
        if descriptor.lane_id != lane_id || descriptor.lane_block_height != lane_block_height {
            return None;
        }
        Self::validate_lane_block_execution_preflight_artifact(&artifact)
            .is_ok()
            .then_some(artifact)
    }

    fn ensure_absent_geometry_progress_sidecar_remains_absent(
        &self,
        pair: &BoundProgressPair,
        data_path: &Path,
        index_path: &Path,
    ) -> Result<()> {
        match pair {
            BoundProgressPair::Present(_) => Ok(()),
            BoundProgressPair::Absent(namespace) => {
                if namespace.data_path != data_path || namespace.index_path != index_path {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "bound progress-sidecar absence has the wrong namespace identity",
                    ));
                }
                if !self.sync_bound_progress_absence(namespace, "absent retired lane progress") {
                    return Err(self.geometry_error(
                        ErrorKind::WouldBlock,
                        "absent progress sidecar durability attestation failed",
                    ));
                }
                Ok(())
            }
        }
    }

    fn geometry_bound_progress_directory_unchanged(
        &self,
        directory: &BoundProgressDirectory,
    ) -> bool {
        let Ok(opened) = directory.file.metadata() else {
            return false;
        };
        opened.is_dir()
            && Self::sidecar_directory_metadata_unchanged(&directory.metadata, &opened)
            && Self::canonical_sidecar_directory_for(&self.store_root, &directory.expected_path)
                .ok()
                .flatten()
                .is_some_and(|(canonical_path, metadata)| {
                    canonical_path == directory.canonical_path
                        && Self::sidecar_directory_metadata_unchanged(
                            &directory.metadata,
                            &metadata,
                        )
                })
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
        let source_parent = source.parent().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry move source has no parent",
            )
        })?;
        let target_parent = target.parent().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry move target has no parent",
            )
        })?;
        let source_name = source.file_name().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry move source has no name",
            )
        })?;
        let target_name = target.file_name().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry move target has no name",
            )
        })?;
        // Active lane storage and retired geometry are both included in Kura's enforced and total
        // usage scans. The rename preserves their exact byte totals, but it must still advance the
        // accounting generation so a scan spanning the move retries instead of publishing a
        // mixed directory snapshot.
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        bootstrap_ensure_geometry_directory(&self.store_root, target_parent)?;
        self.validate_path_kind(target_parent, true)?;
        self.sync_geometry_parent(Some(target_parent))?;
        let (source_parent_handle, source_parent_identity) =
            self.open_geometry_parent(source_parent)?;
        let (target_parent_handle, target_parent_identity) =
            self.open_geometry_parent(target_parent)?;
        self.inject_geometry_move_target_collision_for_test(target, directory)?;
        self.require_geometry_path_identity(source, directory, source_identity)?;
        self.require_geometry_path_identity(source_parent, true, source_parent_identity)?;
        self.require_geometry_path_identity(target_parent, true, target_parent_identity)?;
        self.inject_geometry_move_parent_substitution_for_test(target_parent)?;
        rename_geometry_path_noreplace_at(
            &source_parent_handle,
            source_name,
            &target_parent_handle,
            target_name,
        )
        .map_err(|error| Error::IO(error, source.to_path_buf()))?;
        source_parent_handle
            .sync_all()
            .map_err(|error| Error::IO(error, source_parent.to_path_buf()))?;
        if source_parent != target_parent {
            target_parent_handle
                .sync_all()
                .map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
        }
        self.require_geometry_path_identity(source_parent, true, source_parent_identity)?;
        self.require_geometry_path_identity(target_parent, true, target_parent_identity)?;
        self.require_geometry_path_identity(target, directory, source_identity)?;
        self.sync_geometry_parent(Some(source_parent))?;
        if source_parent != target_parent {
            self.sync_geometry_parent(Some(target_parent))?;
        }
        accounting_mutation.finish();
        Ok(())
    }

    #[cfg(test)]
    fn inject_geometry_move_target_collision_for_test(
        &self,
        target: &Path,
        directory: bool,
    ) -> Result<()> {
        inject_geometry_move_target_collision_for_test(target, directory)
    }

    fn inject_geometry_move_parent_substitution_for_test(
        &self,
        target_parent: &Path,
    ) -> Result<()> {
        inject_geometry_move_parent_substitution_for_test(target_parent)
    }

    #[cfg(not(test))]
    fn inject_geometry_move_target_collision_for_test(
        &self,
        target: &Path,
        directory: bool,
    ) -> Result<()> {
        inject_geometry_move_target_collision_for_test(target, directory)
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
        let blocks_exist = self.validate_path_kind(&blocks, true)?;
        let marker_exists = if blocks_exist {
            let marker_path = blocks.join(MARKER_FILE_NAME);
            let marker_exists = self.validate_path_kind(&marker_path, false)?;
            if !marker_exists {
                preflight_empty_block_store_without_marker(&blocks, Some(binding), false)?;
            }
            marker_exists
        } else {
            if let Some(parent) = blocks.parent() {
                create_dir_all_with_context(parent)?;
                self.validate_path_kind(parent, true)?;
            }
            false
        };
        let historical_byte_limit = self.historical_autonomous_recovery_aggregate_byte_limit();
        let before = Self::block_store_bytes_with_historical_limit(&blocks, historical_byte_limit)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let mut store = BlockStore::new(&blocks);
        store.create_files_if_they_do_not_exist()?;
        create_dir_all_with_context(&Self::lane_artifact_dir(&blocks))?;
        self.sync_geometry_path_contents(&blocks, true)?;
        let after = Self::block_store_bytes_with_historical_limit(&blocks, historical_byte_limit)?;
        self.update_disk_usage_delta(before, after);
        accounting_mutation.finish();
        if marker_exists {
            self.require_lane_marker(binding)?;
        } else {
            self.write_lane_marker(binding)?;
        }
        if !self.validate_path_kind(&merge, false)? {
            let before = Self::file_len_or_zero(&merge)?;
            let accounting_mutation = self.begin_total_disk_usage_mutation();
            if let Some(parent) = merge.parent() {
                create_dir_all_with_context(parent)?;
            }
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
            let after = Self::file_len_or_zero(&merge)?;
            self.update_disk_usage_delta(before, after);
            accounting_mutation.finish();
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
            self.ensure_authoritative_lane_artifact_namespace(&binding, &blocks)?;
        }
        Ok(())
    }

    /// Provision the structural lane-artifact namespace only after its lane binding is trusted.
    ///
    /// Authenticated startup deliberately defers lane provisioning until snapshot/journal
    /// validation. A restored archive may legitimately omit this empty directory, but leaving it
    /// absent turns every certified-lane lookup into a synchronous recovery warning. Bind and
    /// synchronize both the new directory and its ancestor chain before publishing the
    /// authoritative lane map, then revalidate the lane marker so path replacement fails closed.
    fn ensure_authoritative_lane_artifact_namespace(
        &self,
        binding: &LaneGeometryBinding,
        blocks: &Path,
    ) -> Result<()> {
        let blocks_identity = self.geometry_path_identity(blocks, true)?;
        let lane_artifacts = Self::lane_artifact_dir(blocks);
        match fs::create_dir(&lane_artifacts) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(Error::MkDir(error, lane_artifacts)),
        }

        let namespace = Self::open_bound_progress_directory(&self.store_root, &lane_artifacts)?;
        namespace
            .file
            .sync_all()
            .map_err(|error| Error::IO(error, lane_artifacts.clone()))?;
        self.sync_geometry_parent(Some(blocks))?;
        self.require_geometry_path_identity(blocks, true, blocks_identity)?;
        if !self.geometry_bound_progress_directory_unchanged(&namespace) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "authoritative lane artifact namespace changed while becoming durable",
                ),
                lane_artifacts,
            ));
        }
        self.require_lane_marker_at(blocks, binding)
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

    /// Require a lane artifact to target the exact active storage binding and
    /// a height strictly after that incarnation's activation.
    ///
    /// Callers hold `lane_geometry_lock`, so the marker and active segment
    /// cannot be replaced between this check and the sidecar read or write.
    pub(super) fn require_active_lane_artifact(
        &self,
        entry: &LaneConfigEntry,
        descriptor: &LaneBlockDescriptorV1,
    ) -> Result<()> {
        if descriptor.lane_id != entry.lane_id || descriptor.dataspace_id != entry.dataspace_id {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane artifact does not match the active lane or dataspace",
                ),
                entry.blocks_dir(&self.store_root).join(MARKER_FILE_NAME),
            ));
        }
        self.require_active_lane_incarnation(
            entry,
            descriptor.lane_incarnation,
            descriptor.proposal_height,
        )
    }

    /// Require a global-block ownership artifact to target the exact active
    /// storage binding and a height strictly after incarnation activation.
    ///
    /// Callers hold `lane_geometry_lock`, so the marker and active segment
    /// cannot be replaced between this check and the sidecar read or write.
    pub(super) fn require_active_lane_ownership_artifact(
        &self,
        entry: &LaneConfigEntry,
        ownership: &SumeragiLanePayloadOwnership,
    ) -> Result<()> {
        if ownership.lane_id != entry.lane_id || ownership.dataspace_id != entry.dataspace_id {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane ownership artifact does not match the active lane or dataspace",
                ),
                entry.blocks_dir(&self.store_root).join(MARKER_FILE_NAME),
            ));
        }
        self.require_active_lane_incarnation(
            entry,
            ownership.lane_incarnation,
            ownership.proposal_height,
        )
    }

    /// Require an incarnation and proposal height to match an active marker.
    ///
    /// This lower-level form is reserved for replay claims that do not carry a
    /// full lane descriptor. Artifact paths should use
    /// [`Self::require_active_lane_artifact`] so lane and dataspace are checked
    /// as well.
    pub(super) fn require_active_lane_incarnation(
        &self,
        entry: &LaneConfigEntry,
        expected_incarnation: Hash,
        proposal_height: u64,
    ) -> Result<()> {
        let path = entry.blocks_dir(&self.store_root).join(MARKER_FILE_NAME);
        let marker = self.read_lane_marker(&path)?;
        if marker.version != MARKER_VERSION
            || marker.lane_id != entry.lane_id
            || marker.incarnation != expected_incarnation
            || proposal_height <= marker.activation_height
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane artifact does not match the active geometry marker",
                ),
                path,
            ));
        }
        Ok(())
    }

    /// Return the exact active incarnation and activation height under the
    /// caller-held geometry lock.
    pub(super) fn active_lane_incarnation_marker(
        &self,
        entry: &LaneConfigEntry,
    ) -> Result<(Hash, u64)> {
        let path = entry.blocks_dir(&self.store_root).join(MARKER_FILE_NAME);
        let marker = self.read_lane_marker(&path)?;
        if marker.version != MARKER_VERSION || marker.lane_id != entry.lane_id {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "active lane marker has the wrong route identity",
                ),
                path,
            ));
        }
        Ok((marker.incarnation, marker.activation_height))
    }

    /// Install the exact active lane marker required by an isolated test fixture.
    pub(crate) fn install_lane_incarnation_marker_for_test(
        &self,
        entry: &LaneConfigEntry,
        incarnation: Hash,
        activation_height: u64,
    ) -> Result<()> {
        let binding = LaneGeometryBinding {
            lane_id: entry.lane_id,
            incarnation,
            activation_height,
            blocks_path: self.relative_geometry_path(&entry.blocks_dir(&self.store_root))?,
            merge_path: self.relative_geometry_path(&entry.merge_log_path(&self.store_root))?,
        };
        self.write_lane_marker(&binding)
    }

    /// Install a marker for a blank test store without rewriting existing geometry.
    pub(crate) fn install_lane_incarnation_marker_if_missing_for_test(
        &self,
        entry: &LaneConfigEntry,
        incarnation: Hash,
        activation_height: u64,
    ) -> Result<()> {
        let path = entry.blocks_dir(&self.store_root).join(MARKER_FILE_NAME);
        if self.validate_path_kind(&path, false)? {
            return Ok(());
        }
        self.install_lane_incarnation_marker_for_test(entry, incarnation, activation_height)
    }

    /// Replace the in-memory active lane entries after a test mutates its Nexus fixture.
    pub(crate) fn replace_lane_storage_entries_for_test(&self, lane_config: &LaneConfig) {
        let _geometry_guard = self.lane_geometry_lock.lock();
        *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(lane_config);
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
        checked_geometry_file_identity(&metadata, path)
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

    fn open_geometry_parent(&self, parent: &Path) -> Result<(File, GeometryFileIdentity)> {
        let before = self.geometry_path_identity(parent, true)?;
        let directory =
            File::open(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        let opened = directory
            .metadata()
            .map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        if !opened.is_dir()
            || checked_geometry_file_identity(&opened, parent)? != before
            || self.geometry_path_identity(parent, true)? != before
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry parent changed while being opened",
                ),
                parent.to_path_buf(),
            ));
        }
        Ok((directory, before))
    }

    fn verify_open_geometry_file(&self, path: &Path, file: &File) -> Result<()> {
        let path_identity = self.geometry_path_identity(path, false)?;
        let metadata = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !metadata.is_file() || checked_geometry_file_identity(&metadata, path)? != path_identity
        {
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
                self.remove_accounted_geometry_file(&path)?;
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
                self.remove_accounted_geometry_file(&publication_temp)?;
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

    fn remove_accounted_geometry_file(&self, path: &Path) -> Result<()> {
        let before = Self::file_len_or_zero(path)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        fs::remove_file(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.sync_geometry_parent(path.parent())?;
        self.update_disk_usage_delta(before, 0);
        accounting_mutation.finish();
        Ok(())
    }

    fn accounted_atomic_geometry_file_len(&self, path: &Path) -> Result<u64> {
        // The restore temp is deliberately excluded from Kura's usage scans: it is an
        // attempt-local rollback file, whereas the authoritative journal and its publication
        // temp are durable recovery state. All other atomic geometry files live in counted block
        // stores or are one of those two journal names.
        if path == self.store_root.join(JOURNAL_RESTORE_TEMP_FILE_NAME) {
            return Ok(0);
        }
        Self::file_len_or_zero(path)
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
        self.validate_path_kind(temp, false)?;
        let before = self
            .accounted_atomic_geometry_file_len(path)?
            .saturating_add(self.accounted_atomic_geometry_file_len(temp)?);
        // The temp creation/write and target replacement are one accounting mutation. Counting
        // both sibling names makes exact recovery of a preexisting, authenticated temp possible
        // without transiently under-reporting either enforced or total usage.
        let accounting_mutation = self.begin_total_disk_usage_mutation();
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
        // block fsync is deferred by batching. Always synchronize the file before its directory
        // entry.
        file.sync_all()
            .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        self.verify_open_geometry_file(temp, &file)?;
        fs::rename(temp, path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.verify_open_geometry_file(path, &file)?;
        file.sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.sync_geometry_parent(path.parent())?;
        let after = self
            .accounted_atomic_geometry_file_len(path)?
            .saturating_add(self.accounted_atomic_geometry_file_len(temp)?);
        self.update_disk_usage_delta(before, after);
        accounting_mutation.finish();
        Ok(())
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

    /// Resolve any interrupted primary-lane relabel before opening canonical files.
    ///
    /// State-driven geometry recovery runs only after Kura has loaded the canonical chain. A
    /// primary relabel therefore needs this smaller root-level bootstrap pass so startup never
    /// creates an empty configured path while the exact chain is already present under a durable
    /// journal binding.
    pub(super) fn resolve_primary_storage_paths_before_open(
        store_root: &Path,
        configured: &LaneConfigEntry,
    ) -> Result<(PathBuf, PathBuf, bool)> {
        let journal_path = store_root.join(JOURNAL_FILE_NAME);
        let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
            store_root,
            &journal_path,
            store_root,
            usize::try_from(MAX_GEOMETRY_JOURNAL_BYTES)?,
        )?
        else {
            return Ok((
                configured.blocks_dir(store_root),
                configured.merge_log_path(store_root),
                false,
            ));
        };
        let mut cursor = bytes.as_slice();
        let journal = LaneGeometryJournal::decode_all(&mut cursor).map_err(Error::NoritoFrame)?;
        if journal.encode() != bytes {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal is not canonically encoded",
            ));
        }
        validate_lane_geometry_journal_structure(store_root, &journal)?;

        let primary_lane = configured.lane_id;
        let applied_prefix = journal
            .records
            .iter()
            .take_while(|record| {
                matches!(
                    record.phase,
                    LaneGeometryPhase::FilesApplied | LaneGeometryPhase::CatalogPublished
                )
            })
            .count();

        for record in journal.records[applied_prefix..].iter().rev() {
            for operation in record.operations.iter().rev().filter(|operation| {
                operation.kind == LaneGeometryOperationKind::Relabel
                    && operation.lane_id == primary_lane
            }) {
                let updated = operation.updated.as_ref().ok_or_else(|| {
                    lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "primary relabel has no updated binding",
                    )
                })?;
                let previous = operation.previous.as_ref().ok_or_else(|| {
                    lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "primary relabel has no previous binding",
                    )
                })?;
                bootstrap_move_geometry_binding(store_root, updated, previous)?;
            }
        }
        for record in &journal.records[..applied_prefix] {
            for operation in record.operations.iter().filter(|operation| {
                operation.kind == LaneGeometryOperationKind::Relabel
                    && operation.lane_id == primary_lane
            }) {
                let previous = operation.previous.as_ref().ok_or_else(|| {
                    lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "primary relabel has no previous binding",
                    )
                })?;
                let updated = operation.updated.as_ref().ok_or_else(|| {
                    lane_geometry_journal_structure_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "primary relabel has no updated binding",
                    )
                })?;
                bootstrap_move_geometry_binding(store_root, previous, updated)?;
            }
        }

        let checkpoint_binding = journal.checkpoint.as_ref().and_then(|checkpoint| {
            checkpoint
                .bindings
                .iter()
                .find(|binding| binding.lane_id == primary_lane)
        });
        let first_previous = journal.records.iter().find_map(|record| {
            record
                .previous_bindings
                .iter()
                .find(|binding| binding.lane_id == primary_lane)
        });
        let mut active_binding = checkpoint_binding.or(first_previous);
        for record in &journal.records[..applied_prefix] {
            if let Some(binding) = record
                .updated_bindings
                .iter()
                .find(|binding| binding.lane_id == primary_lane)
            {
                active_binding = Some(binding);
            }
        }
        let Some(binding) = active_binding else {
            return Ok((
                configured.blocks_dir(store_root),
                configured.merge_log_path(store_root),
                false,
            ));
        };
        let blocks = store_root.join(&binding.blocks_path);
        let merge = store_root.join(&binding.merge_path);
        if !bootstrap_validate_path_kind(store_root, &blocks, true)?
            || !bootstrap_validate_path_kind(store_root, &merge, false)?
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::NotFound,
                "durable primary binding is not fully present before Kura startup",
            ));
        }
        bootstrap_require_lane_marker(store_root, &blocks, binding)?;
        Ok((blocks, merge, true))
    }

    /// Resolve the committed primary binding without completing any pending geometry move.
    pub(super) fn resolve_primary_storage_paths_read_only(
        store_root: &Path,
        configured: &LaneConfigEntry,
    ) -> Result<(PathBuf, PathBuf, bool)> {
        let journal_path = store_root.join(JOURNAL_FILE_NAME);
        let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
            store_root,
            &journal_path,
            store_root,
            usize::try_from(MAX_GEOMETRY_JOURNAL_BYTES)?,
        )?
        else {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::NotFound,
                "configured lane journal is missing during provisional snapshot startup",
            ));
        };
        let mut cursor = bytes.as_slice();
        let journal = LaneGeometryJournal::decode_all(&mut cursor).map_err(Error::NoritoFrame)?;
        if journal.encode() != bytes {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "lane geometry journal is not canonically encoded",
            ));
        }
        validate_lane_geometry_journal_structure(store_root, &journal)?;
        if journal.records.iter().any(|record| {
            !matches!(
                record.phase,
                LaneGeometryPhase::FilesApplied | LaneGeometryPhase::CatalogPublished
            )
        }) {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::InvalidData,
                "pending lane geometry transition requires recovery before provisional snapshot startup",
            ));
        }

        let primary_lane = configured.lane_id;
        let checkpoint_binding = journal.checkpoint.as_ref().and_then(|checkpoint| {
            checkpoint
                .bindings
                .iter()
                .find(|binding| binding.lane_id == primary_lane)
        });
        let first_previous = journal.records.iter().find_map(|record| {
            record
                .previous_bindings
                .iter()
                .find(|binding| binding.lane_id == primary_lane)
        });
        let mut active_binding = checkpoint_binding.or(first_previous);
        for record in &journal.records {
            if let Some(binding) = record
                .updated_bindings
                .iter()
                .find(|binding| binding.lane_id == primary_lane)
            {
                active_binding = Some(binding);
            }
        }
        let Some(binding) = active_binding else {
            return Ok((
                configured.blocks_dir(store_root),
                configured.merge_log_path(store_root),
                false,
            ));
        };
        let blocks = store_root.join(&binding.blocks_path);
        let merge = store_root.join(&binding.merge_path);
        if !bootstrap_validate_path_kind(store_root, &blocks, true)?
            || !bootstrap_validate_path_kind(store_root, &merge, false)?
        {
            return Err(lane_geometry_journal_structure_error(
                store_root,
                ErrorKind::NotFound,
                "durable primary binding is not fully present before provisional startup",
            ));
        }
        bootstrap_require_lane_marker(store_root, &blocks, binding)?;
        Ok((blocks, merge, true))
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

fn bootstrap_validate_path_kind(store_root: &Path, path: &Path, directory: bool) -> Result<bool> {
    let relative = path.strip_prefix(store_root).map_err(|_| {
        lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidInput,
            "bootstrap geometry path escapes the Kura store root",
        )
    })?;
    validate_relative_path(relative)?;
    bootstrap_validate_existing_ancestors(store_root, path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(Error::IO(error, path.to_path_buf())),
    };
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && (!metadata.is_file() || !Kura::sidecar_is_single_link(&metadata)))
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap geometry path is not an authenticated regular path",
            ),
            path.to_path_buf(),
        ));
    }
    let canonical_root =
        fs::canonicalize(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    let canonical_path =
        fs::canonicalize(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if canonical_path != canonical_root.join(relative) {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap geometry path traverses a symlink or escapes the store root",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(true)
}

fn bootstrap_validate_existing_ancestors(store_root: &Path, path: &Path) -> Result<()> {
    let relative = path.strip_prefix(store_root).map_err(|_| {
        lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidInput,
            "bootstrap geometry path escapes the Kura store root",
        )
    })?;
    validate_relative_path(relative)?;
    let root_metadata = fs::symlink_metadata(store_root)
        .map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap Kura root is not a non-symlink directory",
            ),
            store_root.to_path_buf(),
        ));
    }
    let canonical_root =
        fs::canonicalize(store_root).map_err(|error| Error::IO(error, store_root.to_path_buf()))?;
    let components = relative.components().collect::<Vec<_>>();
    let mut cursor = store_root.to_path_buf();
    let mut expected = PathBuf::new();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        cursor.push(component.as_os_str());
        expected.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&cursor) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(Error::IO(error, cursor)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry ancestor is not a non-symlink directory",
                ),
                cursor,
            ));
        }
        let canonical =
            fs::canonicalize(&cursor).map_err(|error| Error::IO(error, cursor.clone()))?;
        if canonical != canonical_root.join(&expected) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry ancestor escapes the Kura store root",
                ),
                cursor,
            ));
        }
    }
    Ok(())
}

fn bootstrap_ensure_geometry_directory(store_root: &Path, directory: &Path) -> Result<()> {
    let relative = directory.strip_prefix(store_root).map_err(|_| {
        lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidInput,
            "bootstrap geometry directory escapes the Kura store root",
        )
    })?;
    validate_relative_path(relative)?;
    let mut cursor = store_root.to_path_buf();
    for component in relative.components() {
        let parent = cursor.clone();
        let parent_before =
            fs::symlink_metadata(&parent).map_err(|error| Error::IO(error, parent.clone()))?;
        if parent_before.file_type().is_symlink() || !parent_before.is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry parent is not a non-symlink directory",
                ),
                parent,
            ));
        }
        cursor.push(component.as_os_str());
        match fs::create_dir(&cursor) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(Error::IO(error, cursor)),
        }
        if !bootstrap_validate_path_kind(store_root, &cursor, true)? {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "bootstrap geometry directory disappeared after creation",
                ),
                cursor,
            ));
        }
        let parent_after =
            fs::symlink_metadata(&parent).map_err(|error| Error::IO(error, parent.clone()))?;
        if checked_geometry_file_identity(&parent_before, &parent)?
            != checked_geometry_file_identity(&parent_after, &parent)?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry parent changed during child creation",
                ),
                parent,
            ));
        }
        sync_dir(&parent).map_err(|error| Error::IO(error, parent))?;
    }
    Ok(())
}

fn bootstrap_sync_geometry_path(store_root: &Path, path: &Path, directory: bool) -> Result<()> {
    let before =
        fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if !bootstrap_validate_path_kind(store_root, path, directory)? {
        return Err(Error::IO(
            std::io::Error::new(ErrorKind::NotFound, "bootstrap geometry source is missing"),
            path.to_path_buf(),
        ));
    }
    if directory {
        sync_dir(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    } else {
        let file = File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if checked_geometry_file_identity(&before, path)?
            != checked_geometry_file_identity(&opened, path)?
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "bootstrap geometry file changed while opening",
                ),
                path.to_path_buf(),
            ));
        }
        file.sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    }
    let after = fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if checked_geometry_file_identity(&before, path)?
        != checked_geometry_file_identity(&after, path)?
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap geometry path changed while synchronizing",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(())
}

fn bootstrap_open_geometry_parent(store_root: &Path, parent: &Path) -> Result<File> {
    if !bootstrap_validate_path_kind(store_root, parent, true)? {
        return Err(Error::IO(
            std::io::Error::new(ErrorKind::NotFound, "bootstrap geometry parent is missing"),
            parent.to_path_buf(),
        ));
    }
    let before =
        fs::symlink_metadata(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    let directory = File::open(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    let opened = directory
        .metadata()
        .map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    let after =
        fs::symlink_metadata(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    if before.file_type().is_symlink()
        || !before.is_dir()
        || checked_geometry_file_identity(&before, parent)?
            != checked_geometry_file_identity(&opened, parent)?
        || checked_geometry_file_identity(&before, parent)?
            != checked_geometry_file_identity(&after, parent)?
        || !bootstrap_validate_path_kind(store_root, parent, true)?
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "bootstrap geometry parent changed while being opened",
            ),
            parent.to_path_buf(),
        ));
    }
    Ok(directory)
}

fn bootstrap_move_geometry_path(
    store_root: &Path,
    source: &Path,
    target: &Path,
    directory: bool,
) -> Result<bool> {
    if source == target {
        if bootstrap_validate_path_kind(store_root, source, directory)? {
            return Ok(false);
        }
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::NotFound,
                "unchanged primary relabel path is missing during bootstrap recovery",
            ),
            source.to_path_buf(),
        ));
    }
    let source_exists = bootstrap_validate_path_kind(store_root, source, directory)?;
    let target_exists = bootstrap_validate_path_kind(store_root, target, directory)?;
    match (source_exists, target_exists) {
        (false, false) | (false, true) => return Ok(false),
        (true, true) => {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "both primary relabel paths exist during bootstrap recovery",
                ),
                target.to_path_buf(),
            ));
        }
        (true, false) => {}
    }
    bootstrap_sync_geometry_path(store_root, source, directory)?;
    let identity = checked_geometry_file_identity(
        &fs::symlink_metadata(source).map_err(|error| Error::IO(error, source.to_path_buf()))?,
        source,
    )?;
    let source_parent = source.parent().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel source has no parent",
            ),
            source.to_path_buf(),
        )
    })?;
    let target_parent = target.parent().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel target has no parent",
            ),
            target.to_path_buf(),
        )
    })?;
    let source_name = source.file_name().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel source has no name",
            ),
            source.to_path_buf(),
        )
    })?;
    let target_name = target.file_name().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "primary relabel target has no name",
            ),
            target.to_path_buf(),
        )
    })?;
    bootstrap_ensure_geometry_directory(store_root, target_parent)?;
    sync_dir(target_parent).map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
    let source_parent_handle = bootstrap_open_geometry_parent(store_root, source_parent)?;
    let target_parent_handle = bootstrap_open_geometry_parent(store_root, target_parent)?;
    inject_geometry_move_target_collision_for_test(target, directory)?;
    if !bootstrap_validate_path_kind(store_root, source, directory)?
        || checked_geometry_file_identity(
            &fs::symlink_metadata(source)
                .map_err(|error| Error::IO(error, source.to_path_buf()))?,
            source,
        )? != identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "primary relabel source identity changed before bootstrap rename",
            ),
            source.to_path_buf(),
        ));
    }
    rename_geometry_path_noreplace_at(
        &source_parent_handle,
        source_name,
        &target_parent_handle,
        target_name,
    )
    .map_err(|error| Error::IO(error, source.to_path_buf()))?;
    if !bootstrap_validate_path_kind(store_root, target, directory)?
        || checked_geometry_file_identity(
            &fs::symlink_metadata(target)
                .map_err(|error| Error::IO(error, target.to_path_buf()))?,
            target,
        )? != identity
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "primary relabel target identity changed during bootstrap recovery",
            ),
            target.to_path_buf(),
        ));
    }
    source_parent_handle
        .sync_all()
        .map_err(|error| Error::IO(error, source_parent.to_path_buf()))?;
    if source.parent() != target.parent() {
        target_parent_handle
            .sync_all()
            .map_err(|error| Error::IO(error, target_parent.to_path_buf()))?;
    }
    Ok(true)
}

fn bootstrap_preflight_geometry_path(
    store_root: &Path,
    source: &Path,
    target: &Path,
    directory: bool,
) -> Result<()> {
    if source == target {
        return bootstrap_validate_path_kind(store_root, source, directory)?
            .then_some(())
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "unchanged primary relabel path is missing during bootstrap recovery",
                    ),
                    source.to_path_buf(),
                )
            });
    }
    let source_exists = bootstrap_validate_path_kind(store_root, source, directory)?;
    let target_exists = bootstrap_validate_path_kind(store_root, target, directory)?;
    match (source_exists, target_exists) {
        (false, false) => {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "neither primary relabel path exists during bootstrap recovery",
                ),
                source.to_path_buf(),
            ));
        }
        (true, true) => {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "both primary relabel paths exist during bootstrap recovery",
                ),
                target.to_path_buf(),
            ));
        }
        (true, false) | (false, true) => {}
    }
    bootstrap_validate_existing_ancestors(store_root, source)?;
    bootstrap_validate_existing_ancestors(store_root, target)
}

fn bootstrap_move_geometry_binding(
    store_root: &Path,
    source: &LaneGeometryBinding,
    target: &LaneGeometryBinding,
) -> Result<()> {
    let source_blocks = store_root.join(&source.blocks_path);
    let target_blocks = store_root.join(&target.blocks_path);
    let source_merge = store_root.join(&source.merge_path);
    let target_merge = store_root.join(&target.merge_path);
    bootstrap_preflight_geometry_path(store_root, &source_blocks, &target_blocks, true)?;
    bootstrap_preflight_geometry_path(store_root, &source_merge, &target_merge, false)?;

    let rollback = || -> Result<()> {
        let merge_result =
            bootstrap_move_geometry_path(store_root, &target_merge, &source_merge, false);
        let blocks_result =
            bootstrap_move_geometry_path(store_root, &target_blocks, &source_blocks, true);
        match (merge_result, blocks_result) {
            (Ok(_), Ok(_)) => Ok(()),
            (merge, blocks) => Err(Error::IO(
                std::io::Error::other(format!(
                    "primary relabel rollback failed (merge: {merge:?}; blocks: {blocks:?})"
                )),
                source_blocks.clone(),
            )),
        }
    };

    if let Err(error) =
        bootstrap_move_geometry_path(store_root, &source_blocks, &target_blocks, true)
    {
        return match rollback() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(Error::IO(
                std::io::Error::other(format!(
                    "primary relabel block move failed ({error}); rollback failed ({rollback_error})"
                )),
                source_blocks,
            )),
        };
    }
    match bootstrap_move_geometry_path(store_root, &source_merge, &target_merge, false) {
        Ok(_) => Ok(()),
        Err(error) => match rollback() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(Error::IO(
                std::io::Error::other(format!(
                    "primary relabel merge move failed ({error}); block-directory rollback failed ({rollback_error})"
                )),
                source_blocks,
            )),
        },
    }
}

fn bootstrap_require_lane_marker(
    store_root: &Path,
    blocks: &Path,
    binding: &LaneGeometryBinding,
) -> Result<()> {
    let path = blocks.join(MARKER_FILE_NAME);
    let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
        store_root,
        &path,
        blocks,
        usize::try_from(MAX_LANE_MARKER_BYTES)?,
    )?
    else {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::NotFound,
                "durable primary binding has no incarnation marker",
            ),
            path,
        ));
    };
    let mut cursor = bytes.as_slice();
    let marker = LaneIncarnationMarker::decode_all(&mut cursor).map_err(Error::NoritoFrame)?;
    if marker.encode() != bytes
        || marker.version != MARKER_VERSION
        || marker.lane_id != binding.lane_id
        || marker.incarnation != binding.incarnation
        || marker.activation_height != binding.activation_height
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidData,
                "durable primary binding marker does not match its journal identity",
            ),
            path,
        ));
    }
    Ok(())
}

include!("lane_geometry/catalog_validation.rs");

#[cfg(test)]
mod tests {
    include!("lane_geometry_tests/00_support.rs");
    include!("lane_geometry/native_amx_retained_window_tests.rs");
    include!("lane_geometry_tests/00_retirement.rs");
    include!("lane_geometry_tests/01_retirement_and_recovery.rs");
    include!("lane_geometry_tests/02_geometry_moves_and_journal.rs");
    include!("lane_geometry_tests/03_gc_and_startup.rs");
}

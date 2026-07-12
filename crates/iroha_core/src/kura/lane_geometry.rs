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

use super::{
    AUTONOMOUS_LANE_BLOCKS_DATA_FILE, AUTONOMOUS_LANE_BLOCKS_INDEX_FILE,
    AutonomousLaneBlockArtifact, BlockStore, CERTIFIED_LANE_BLOCKS_DATA_FILE,
    CERTIFIED_LANE_BLOCKS_INDEX_FILE, Error, Kura, LANE_ARTIFACTS_DATA_FILE,
    LANE_ARTIFACTS_DIR_NAME, LANE_ARTIFACTS_INDEX_FILE, LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
    LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE, LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE, LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE, LaneBlockApplicationReceiptArtifact,
    LaneBlockApplicationReceiptArtifactFormat, MergeLedgerCarrierRecord, PIPELINE_INDEX_ENTRY_SIZE,
    RecoveredLaneBlockPayload, Result, SidecarIndexEntry, SidecarIndexLayout,
    create_dir_all_with_context, sync_dir,
};

const JOURNAL_VERSION: u8 = 5;
const MARKER_VERSION: u8 = 1;
const CHECKPOINT_VERSION: u8 = 3;
const JOURNAL_FILE_NAME: &str = "lane_geometry_journal.norito";
const JOURNAL_TEMP_FILE_NAME: &str = "lane_geometry_journal.norito.tmp";
const JOURNAL_RESTORE_TEMP_FILE_NAME: &str = "lane_geometry_journal.norito.restore.tmp";
#[cfg(test)]
const JOURNAL_IDENTITY_SWAP_FILE_NAME: &str = "lane_geometry_journal.norito.identity-swap";
#[cfg(test)]
const JOURNAL_IDENTITY_DISPLACED_FILE_NAME: &str =
    "lane_geometry_journal.norito.identity-displaced";
const MARKER_FILE_NAME: &str = ".lane-incarnation.norito";
const TRANSITION_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-transition:v2\0";
const CATALOG_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-catalog:v1\0";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-checkpoint:v2\0";
const UNSCOPED_LINEAGE_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-unscoped-lineage:v1\0";
const PENDING_GC_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-pending-gc:v2\0";
const MERGE_RELEASE_MARKERS_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-merge-markers:v1\0";
const MERGE_RELEASE_RECEIPT_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-merge-receipt:v1\0";
const GC_QUARANTINE_PREFIX: &str = ".gc-";
const MAX_GEOMETRY_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GEOMETRY_TRANSITIONS: usize = 16_384;
const MAX_GEOMETRY_BINDINGS: usize = 65_536;
const MAX_GEOMETRY_MERGE_RELEASES: usize = 1_000_000;
const MAX_GEOMETRY_ARCHIVE_DEPTH: usize = 128;
const MAX_GEOMETRY_ARCHIVE_ENTRIES: usize = 4_000_000;
const MAX_LANE_RETIREMENT_ARTIFACT_FILES: usize = 65_536;
const MAX_LANE_RETIREMENT_WORK_ITEMS: usize = 65_536;

#[cfg(test)]
static CONFIGURED_CATALOG_PREFLIGHT_IDENTITY_SWAP: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);
#[cfg(test)]
static CONFIGURED_CATALOG_PREFLIGHT_FAIL_AFTER_ESTABLISH: std::sync::Mutex<Option<PathBuf>> =
    std::sync::Mutex::new(None);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneGeometryOperationKind {
    Create,
    Retire,
    Replace,
    Relabel,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryJournal {
    version: u8,
    configured_catalog_hash: Option<Hash>,
    checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
    pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
    records: Vec<LaneGeometryIntent>,
}

impl Default for LaneGeometryJournal {
    fn default() -> Self {
        Self {
            version: JOURNAL_VERSION,
            configured_catalog_hash: None,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneIncarnationMarker {
    version: u8,
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
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

fn remove_exact_configured_catalog_temp(
    store_root: &Path,
    root_identity: GeometryFileIdentity,
    journal_path: &Path,
    expected_journal: &ConfiguredCatalogPreflightJournal,
    temp_path: &Path,
    expected_temp: &ConfiguredCatalogPreflightJournal,
) -> Result<()> {
    configured_catalog_require_store_root_identity(store_root, root_identity)?;
    let current_journal = read_configured_catalog_journal_for_preflight(
        store_root,
        root_identity,
        journal_path,
        false,
    )?
    .ok_or_else(|| {
        configured_catalog_preflight_error(
            store_root,
            ErrorKind::NotFound,
            "authoritative configured-catalog journal disappeared during temp recovery",
        )
    })?;
    let current_temp =
        read_configured_catalog_journal_for_preflight(store_root, root_identity, temp_path, false)?
            .ok_or_else(|| {
                configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::NotFound,
                    "configured-catalog temp disappeared during exact recovery",
                )
            })?;
    if current_journal.identity != expected_journal.identity
        || current_journal.bytes != expected_journal.bytes
        || current_temp.identity != expected_temp.identity
        || current_temp.bytes != expected_temp.bytes
        || current_temp.bytes != current_journal.bytes
        || current_temp.identity != current_journal.identity
    {
        return Err(configured_catalog_preflight_error(
            store_root,
            ErrorKind::InvalidData,
            "configured-catalog journal or temp identity changed during exact recovery",
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
        let publication_temp = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &publication_temp_path,
            false,
        )?;
        let restore_temp = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &restore_temp_path,
            false,
        )?;

        if let Some(journal) = journal {
            validate_configured_catalog_journal(store_root, &journal.journal, attempted)?;
            if let Some(temp) = publication_temp.as_ref() {
                validate_configured_catalog_journal(store_root, &temp.journal, attempted)?;
                if temp.bytes != journal.bytes {
                    return Err(configured_catalog_preflight_error(
                        store_root,
                        ErrorKind::InvalidData,
                        "unresolved configured-catalog publication temp differs from the authoritative journal",
                    ));
                }
                remove_exact_configured_catalog_temp(
                    store_root,
                    root_identity,
                    &journal_path,
                    &journal,
                    &publication_temp_path,
                    temp,
                )?;
            }
            if let Some(temp) = restore_temp.as_ref() {
                validate_configured_catalog_journal(store_root, &temp.journal, attempted)?;
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "configured-catalog restore temp has no startup-owned provenance",
                ));
            }
            configured_catalog_require_store_root_identity(store_root, root_identity)?;
            return Ok(());
        }

        if restore_temp.is_some() {
            return Err(configured_catalog_preflight_error(
                store_root,
                ErrorKind::InvalidData,
                "configured-catalog restore temp exists without its authoritative journal",
            ));
        }

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
        self.apply_lane_geometry_transition_with_certified_retirements(
            previous,
            updated,
            previous_incarnations,
            updated_incarnations,
            previous_activation_heights,
            updated_activation_heights,
            replaced_lane_ids,
            &BTreeSet::new(),
        )
    }

    /// Apply one geometry transition while allowing node-local pending work
    /// owned by an exactly certified retiring lane incarnation to be archived.
    ///
    /// The exception is intentionally narrower than general retirement
    /// admission: payloads coordinated by another lane, malformed artifacts,
    /// and in-flight files remain blocking. Callers must first validate a
    /// globally committed drain certificate for every supplied identity.
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
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }
        let previous_bindings =
            self.geometry_bindings(previous, previous_incarnations, previous_activation_heights)?;
        let updated_bindings =
            self.geometry_bindings(updated, updated_incarnations, updated_activation_heights)?;
        self.apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements(
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
        )
    }

    /// Apply one lane geometry transition bound to the complete retained lineage state.
    pub(crate) fn apply_lane_geometry_transition_with_lineage_roots(
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
    ) -> Result<()> {
        self.apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements(
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
        )
    }

    /// Apply a retained-lineage-bound transition with certified exact-incarnation retirements.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements(
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
        self.reconcile_lane_geometry_history(
            &mut journal,
            previous_catalog,
            previous_lineage_root,
        )?;
        self.ensure_authoritative_lane_markers(
            previous,
            previous_incarnations,
            previous_activation_heights,
        )?;
        if previous_catalog == updated_catalog && previous_lineage_root == updated_lineage_root {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            // Reconciliation can update retained transition phases, so an existing or non-empty
            // journal must still be rewritten. A genuinely absent, default journal has no
            // recovery state to invent; exact configured-baseline publication remains atomic in
            // the later catalog-publication step.
            return if journal_was_present || journal != LaneGeometryJournal::default() {
                self.write_lane_geometry_journal(&journal)
            } else {
                Ok(())
            };
        }

        let transition_id = geometry_transition_id(
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        );
        if let Some(existing_index) = journal
            .records
            .iter()
            .position(|record| record.transition_id == transition_id)
        {
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
            let _sidecar_guard = self.sidecar_lock.lock();
            self.ensure_lane_retirement_admissible_locked(&retiring, &certified_retirements)?;
            journal.records[existing_index].phase = LaneGeometryPhase::Intent;
            self.write_lane_geometry_journal(&journal)?;
            if let Err(error) = self.apply_geometry_operations_forward(&operations) {
                let rollback = self.apply_geometry_operations_rollback(&operations);
                if let Err(rollback_error) = rollback {
                    return Err(Error::IO(
                        std::io::Error::other(format!(
                            "lane geometry reapply failed ({error}); rollback failed ({rollback_error})"
                        )),
                        self.lane_geometry_journal_path(),
                    ));
                }
                journal.records[existing_index].phase = LaneGeometryPhase::RolledBack;
                self.write_lane_geometry_journal(&journal)?;
                return Err(error);
            }
            journal.records[existing_index].phase = LaneGeometryPhase::FilesApplied;
            self.write_lane_geometry_journal(&journal)?;
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }

        let operations = self.build_geometry_operations(
            transition_id,
            &previous_bindings,
            &updated_bindings,
            replaced_lane_ids,
        )?;
        let retiring = self.geometry_retirement_identities(previous, &operations)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        self.ensure_lane_retirement_admissible_locked(&retiring, &certified_retirements)?;
        let intent = LaneGeometryIntent {
            transition_id,
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
        if let Err(error) =
            self.apply_geometry_operations_forward(&journal.records[record_index].operations)
        {
            let rollback =
                self.apply_geometry_operations_rollback(&journal.records[record_index].operations);
            if let Err(rollback_error) = rollback {
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
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        let journal_path = self.lane_geometry_journal_path();
        let publication_temp = self.store_root.join(JOURNAL_TEMP_FILE_NAME);
        let prior_journal_bytes = self.read_geometry_file_bytes(&journal_path)?;
        let publication_temp_preexisted = self.validate_path_kind(&publication_temp, false)?;
        if let Some(record) = journal.records.iter_mut().rev().find(|record| {
            record.updated_catalog == fingerprint
                && record.updated_lineage_root == lineage_root
                && record.updated_bindings == bindings
        }) {
            record.phase = LaneGeometryPhase::CatalogPublished;
        } else if !(journal.records.is_empty()
            && journal.checkpoint.as_ref().is_none_or(|checkpoint| {
                checkpoint.catalog == fingerprint
                    && checkpoint.lineage_root == lineage_root
                    && checkpoint.bindings == bindings
            }))
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry publication identity is not retained in the journal",
            ));
        }
        if let Some(attempted) = configured_baseline {
            match journal.configured_catalog_hash {
                Some(expected) if expected == attempted => {}
                None => {
                    journal.configured_catalog_hash = Some(attempted);
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
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() =
                Self::lane_storage_entries_from_config(authoritative);
            return Ok(());
        }
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        self.recover_lane_geometry_journal_with_lineage_root(
            authoritative,
            incarnations,
            activation_heights,
            unscoped_lineage_root(&bindings),
        )
    }

    /// Recover retained geometry intents against an exact catalog and lineage identity.
    pub(crate) fn recover_lane_geometry_journal_with_lineage_root(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
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
        self.reconcile_lane_geometry_history(&mut journal, fingerprint, lineage_root)?;
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
            .rposition(|record| {
                record.updated_catalog == catalog && record.updated_lineage_root == lineage_root
            })
            .map_or_else(
                || {
                    if journal
                        .records
                        .first()
                        .is_some_and(|record| {
                            record.previous_catalog == catalog
                                && record.previous_lineage_root == lineage_root
                        })
                        || journal
                            .checkpoint
                            .as_ref()
                            .is_some_and(|checkpoint| {
                                checkpoint.catalog == catalog
                                    && checkpoint.lineage_root == lineage_root
                            })
                    {
                        Ok(0)
                    } else {
                        Err(self.geometry_error(
                            ErrorKind::InvalidData,
                            "snapshot geometry catalog is not reachable from retained transition history",
                        ))
                    }
                },
                |index| Ok(index + 1),
            )?;

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

        let (transition_previous_catalog, transition_previous_lineage_root, transition_id) =
            if prune_count > 0 {
                let latest = &journal.records[prune_count - 1];
                (
                    Some(latest.previous_catalog),
                    Some(latest.previous_lineage_root),
                    Some(latest.transition_id),
                )
            } else {
                journal
                    .checkpoint
                    .as_ref()
                    .map_or((None, None, None), |checkpoint| {
                        (
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
            let (bytes, existed) =
                self.remove_authenticated_geometry_archive(archive, &merge_releases)?;
            summary.reclaimed_bytes = summary.reclaimed_bytes.saturating_add(bytes);
            summary.removed_archive_roots = summary
                .removed_archive_roots
                .saturating_add(usize::from(existed));
        }
        // Refuse to acknowledge deletion if accounting sees any unsafe or unreadable geometry
        // entry. Keeping the pending intent makes the already-deleted subset replayable.
        let _ = self.kura_disk_usage_bytes()?;
        let _ = self.kura_total_disk_usage_bytes()?;
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
        // Prefer the newest transition that reaches the authoritative identity. A catalog can
        // recur after a create/retire cycle, and selecting the journal's initial boundary merely
        // because it has the same active bindings would roll back later, already-published
        // history. The initial boundary is authoritative only when no retained transition reaches
        // the requested catalog and lineage identity.
        let applied_count = journal
            .records
            .iter()
            .rposition(|record| {
                record.updated_catalog == authoritative_catalog
                    && record.updated_lineage_root == authoritative_lineage_root
            })
            .map(|index| index + 1)
            .or_else(|| {
                (journal.records[0].previous_catalog == authoritative_catalog
                    && journal.records[0].previous_lineage_root == authoritative_lineage_root)
                    .then_some(0)
            })
            .ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal does not contain the authoritative catalog",
                )
            })?;

        let mut saw_unapplied = false;
        for record in &journal.records {
            match record.phase {
                LaneGeometryPhase::CatalogPublished | LaneGeometryPhase::FilesApplied
                    if !saw_unapplied => {}
                LaneGeometryPhase::Intent | LaneGeometryPhase::RolledBack => {
                    saw_unapplied = true;
                }
                LaneGeometryPhase::CatalogPublished | LaneGeometryPhase::FilesApplied => {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal phases do not form a contiguous applied prefix",
                    ));
                }
            }
        }

        // Remove transitions beyond the authoritative point before restoring
        // its prefix. A later incarnation can reuse the same live paths, so
        // rolling the suffix back first prevents it from being mistaken for an
        // older incarnation while the prefix is repaired.
        for record_index in (applied_count..journal.records.len()).rev() {
            let phase = journal.records[record_index].phase;
            if phase != LaneGeometryPhase::RolledBack {
                // Persist an in-progress phase before mutating paths. Recovery
                // can then use the authoritative identity to complete either
                // direction without guessing whether a stable published tip
                // was interrupted. In particular, a CatalogPublished record
                // must never be replayed merely because it is the last record.
                if phase != LaneGeometryPhase::Intent {
                    journal.records[record_index].phase = LaneGeometryPhase::Intent;
                    self.write_lane_geometry_journal(journal)?;
                }
                let operations = journal.records[record_index].operations.clone();
                self.apply_geometry_operations_rollback(&operations)?;
                journal.records[record_index].phase = LaneGeometryPhase::RolledBack;
                // Persist each completed transition. A crash can then leave at
                // most the operation adjacent to the durable phase boundary in
                // doubt instead of losing the progress of an entire chain.
                self.write_lane_geometry_journal(journal)?;
            }
        }
        for record_index in 0..applied_count {
            let phase = journal.records[record_index].phase;
            // Published/files-applied records before the boundary are already
            // reflected in the filesystem. Replaying all of them is not
            // idempotent across a chain: an earlier create may have subsequently
            // been retired into a later record's archive, and replaying that
            // create would provision a duplicate live store. Reconcile only
            // records that are known to be rolled back/incomplete, plus the
            // transition whose explicit in-progress phase records a crash.
            if matches!(
                phase,
                LaneGeometryPhase::Intent | LaneGeometryPhase::RolledBack
            ) {
                if phase != LaneGeometryPhase::Intent {
                    journal.records[record_index].phase = LaneGeometryPhase::Intent;
                    self.write_lane_geometry_journal(journal)?;
                }
                let operations = journal.records[record_index].operations.clone();
                self.apply_geometry_operations_forward(&operations)?;
            }
            if phase != LaneGeometryPhase::CatalogPublished {
                journal.records[record_index].phase = LaneGeometryPhase::CatalogPublished;
                self.write_lane_geometry_journal(journal)?;
            }
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
        certified_retirements: &BTreeSet<LaneRetirementIdentity>,
    ) -> Result<()> {
        let retiring = retiring.iter().copied().collect::<BTreeSet<_>>();
        if !certified_retirements.is_subset(&retiring) {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "certified retirement identity does not exactly match the retiring geometry",
            ));
        }
        if retiring.is_empty() {
            return Ok(());
        }
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
                if lane_proposal_coordinator_targets_retirement(
                    &certified.proposal,
                    certified_retirements,
                ) {
                    continue;
                }
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
            if input.autonomous_payload_hash.is_none() && receipt_applies(identity, &input.proposal)
            {
                continue;
            }
            let coordinator_has_certified_drain = lane_proposal_coordinator_targets_retirement(
                &input.proposal,
                certified_retirements,
            );
            if lane_proposal_coordinator_targets_retirement(&input.proposal, &retiring) {
                if coordinator_has_certified_drain {
                    continue;
                }
                return Err(self.geometry_error(
                    ErrorKind::WouldBlock,
                    "pending execution input belongs to a retiring lane incarnation",
                ));
            }
            if input.autonomous_payload_hash.is_some() {
                continue;
            }
            if hinted_payload_targets(&input.proposal)? {
                if coordinator_has_certified_drain {
                    continue;
                }
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
            self.validate_binding_paths(previous)?;
            if self.binding_blocks_path(previous).exists() {
                self.require_lane_marker(previous)?;
            }
        }
        if let Some(updated) = operation.updated.as_ref() {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    self.validate_binding_paths(updated)?;
                    let blocks = self.binding_blocks_path(updated);
                    let merge = self.binding_merge_path(updated);
                    if blocks.exists() || merge.exists() {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane storage already exists at a create target",
                        ));
                    }
                }
                LaneGeometryOperationKind::Replace | LaneGeometryOperationKind::Relabel => {
                    if operation
                        .previous
                        .as_ref()
                        .is_some_and(|previous| previous.blocks_path != updated.blocks_path)
                        && self.binding_blocks_path(updated).exists()
                    {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane geometry target block path already exists",
                        ));
                    }
                    if operation
                        .previous
                        .as_ref()
                        .is_some_and(|previous| previous.merge_path != updated.merge_path)
                        && self.binding_merge_path(updated).exists()
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
    ) -> Result<()> {
        for operation in operations {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    self.restore_unpublished_or_provision(operation)?;
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
                    let previous = operation
                        .previous
                        .as_ref()
                        .expect("replace has previous binding");
                    self.archive_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                    self.restore_unpublished_or_provision(operation)?;
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
    ) -> Result<()> {
        for operation in operations.iter().rev() {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    let updated = operation.updated.as_ref().expect("create updated");
                    self.archive_geometry_binding(
                        updated,
                        &operation.unpublished_blocks_path,
                        &operation.unpublished_merge_path,
                    )?;
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
                    let updated = operation.updated.as_ref().expect("replace updated");
                    self.archive_geometry_binding(
                        updated,
                        &operation.unpublished_blocks_path,
                        &operation.unpublished_merge_path,
                    )?;
                    let previous = operation.previous.as_ref().expect("replace previous");
                    self.restore_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
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
        self.move_geometry_path(&blocks, &self.resolve_relative_path(archived_blocks)?, true)?;
        self.move_geometry_path(&merge, &self.resolve_relative_path(archived_merge)?, false)
    }

    fn restore_geometry_binding(
        &self,
        binding: &LaneGeometryBinding,
        archived_blocks: &str,
        archived_merge: &str,
    ) -> Result<()> {
        self.move_geometry_path(
            &self.resolve_relative_path(archived_blocks)?,
            &self.binding_blocks_path(binding),
            true,
        )?;
        self.move_geometry_path(
            &self.resolve_relative_path(archived_merge)?,
            &self.binding_merge_path(binding),
            false,
        )?;
        self.require_lane_marker(binding)
    }

    fn restore_unpublished_or_provision(&self, operation: &LaneGeometryOperation) -> Result<()> {
        let updated = operation
            .updated
            .as_ref()
            .expect("create and replace operations have an updated binding");
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;
        let unpublished_merge = self.resolve_relative_path(&operation.unpublished_merge_path)?;
        if self.validate_path_kind(&unpublished_blocks, true)?
            || self.validate_path_kind(&unpublished_merge, false)?
        {
            self.restore_geometry_binding(
                updated,
                &operation.unpublished_blocks_path,
                &operation.unpublished_merge_path,
            )
        } else {
            self.provision_geometry_binding(updated)
        }
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
        self.move_geometry_path(&old_blocks, &new_blocks, true)?;
        self.move_geometry_path(&old_merge, &new_merge, false)?;
        self.retarget_active_geometry_paths(&old_blocks, &new_blocks, &old_merge, &new_merge)?;
        self.require_lane_marker(updated)
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
            fs::rename(&root, &quarantine).map_err(|error| Error::IO(error, root.clone()))?;
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
        let source_exists = self.validate_path_kind(source, directory)?;
        let target_exists = self.validate_path_kind(target, directory)?;
        match (source_exists, target_exists) {
            (false, false) | (false, true) => return Ok(()),
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
        fs::rename(source, target).map_err(|error| Error::IO(error, source.to_path_buf()))?;
        self.require_geometry_path_identity(target, directory, source_identity)?;
        self.sync_geometry_parent(source.parent())?;
        if source.parent() != target.parent() {
            self.sync_geometry_parent(target.parent())?;
        }
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
        if blocks.exists() {
            self.require_lane_marker(binding)?;
        } else {
            if let Some(parent) = blocks.parent() {
                create_dir_all_with_context(parent)?;
            }
            let mut store = BlockStore::new(&blocks);
            store.create_files_if_they_do_not_exist()?;
            self.write_lane_marker(binding)?;
            self.sync_geometry_path_contents(&blocks, true)?;
        }
        if let Some(parent) = merge.parent() {
            create_dir_all_with_context(parent)?;
        }
        if !merge.exists() {
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
        } else {
            self.validate_path_kind(&merge, false)?;
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
                if binding.activation_height != 0 {
                    return Err(self.geometry_error(
                        ErrorKind::NotFound,
                        "active dynamic lane storage is missing; refusing to provision an empty replacement",
                    ));
                }
                self.provision_geometry_binding(&binding)?;
                continue;
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

    fn require_lane_marker_at(
        &self,
        blocks_path: &Path,
        binding: &LaneGeometryBinding,
    ) -> Result<()> {
        let path = blocks_path.join(MARKER_FILE_NAME);
        let marker = self.read_lane_marker(&path)?;
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
                path,
            ));
        }
        Ok(())
    }

    /// Require an autonomous lane artifact to target the exact active storage
    /// incarnation and a height strictly after that incarnation's activation.
    ///
    /// Callers hold `lane_geometry_lock`, so the marker and active segment
    /// cannot be replaced between this check and the sidecar read or write.
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
                    "autonomous lane artifact does not match the active incarnation marker",
                ),
                path,
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn install_lane_incarnation_marker_for_test(
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

    fn read_lane_marker(&self, path: &Path) -> Result<LaneIncarnationMarker> {
        self.validate_path_kind(path, false)?;
        let mut bytes = Vec::new();
        fs::File::open(path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        decode_exact(&bytes).map_err(Error::NoritoFrame)
    }

    fn write_lane_marker(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        create_dir_all_with_context(&blocks)?;
        let path = blocks.join(MARKER_FILE_NAME);
        let temp = blocks.join(format!("{MARKER_FILE_NAME}.tmp"));
        self.validate_path_kind(&path, false)?;
        let marker = LaneIncarnationMarker {
            version: MARKER_VERSION,
            lane_id: binding.lane_id,
            incarnation: binding.incarnation,
            activation_height: binding.activation_height,
        };
        self.atomic_write_geometry_file(&path, &temp, &marker.encode())
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

    fn validate_binding_paths(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        let blocks_exists = self.validate_path_kind(&blocks, true)?;
        let merge_exists = self.validate_path_kind(&merge, false)?;
        if blocks_exists != merge_exists {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane storage block and merge paths are only partially present",
            ));
        }
        Ok(())
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
        let file_len = file
            .metadata()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?
            .len();
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
        file.read_to_end(&mut bytes)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.verify_open_geometry_file(path, &file)?;
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
        } else if !journal.pending_archive_gc.is_empty() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal has pending archive GC without a durable checkpoint",
            ));
        }
        self.validate_pending_lane_geometry_gc(journal)?;
        let mut transition_ids = BTreeSet::new();
        let mut retained_paths = BTreeSet::new();
        for (record_index, record) in journal.records.iter().enumerate() {
            if record.transition_id
                != geometry_transition_id(
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
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry checkpoint height, block hash, or activation is invalid",
            ));
        }
        match (
            checkpoint.transition_previous_catalog,
            checkpoint.transition_previous_lineage_root,
            checkpoint.transition_id,
        ) {
            (None, None, None) => {}
            (Some(previous_catalog), Some(previous_lineage_root), Some(transition_id))
                if !lineage_root_is_zero(previous_lineage_root)
                    && transition_id
                        == geometry_transition_id(
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
                            != intent.previous_lineage_root)
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
                let mut stale_bytes = Vec::new();
                stale
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
    } else if !journal.pending_archive_gc.is_empty() {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry journal has pending archive GC without a durable checkpoint",
        ));
    }
    validate_pending_lane_geometry_gc_structure(store_root, journal)?;

    let mut transition_ids = BTreeSet::new();
    let mut retained_paths = BTreeSet::new();
    for (record_index, record) in journal.records.iter().enumerate() {
        if record.transition_id
            != geometry_transition_id(
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
    {
        return Err(lane_geometry_journal_structure_error(
            store_root,
            ErrorKind::InvalidData,
            "lane geometry checkpoint commitment, catalog, height, block hash, or activation is invalid",
        ));
    }
    match (
        checkpoint.transition_previous_catalog,
        checkpoint.transition_previous_lineage_root,
        checkpoint.transition_id,
    ) {
        (None, None, None) => Ok(()),
        (Some(previous_catalog), Some(previous_lineage_root), Some(transition_id))
            if !lineage_root_is_zero(previous_lineage_root)
                && transition_id
                    == geometry_transition_id(
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
            checkpoint: None,
            pending_archive_gc: Vec::new(),
            records: vec![intent.clone()],
        };
        validate_lane_geometry_journal_structure(store_root, &standalone)?;
        if intent.phase != LaneGeometryPhase::CatalogPublished
            || !pending_ids.insert(intent.transition_id)
            || retained_ids.contains(&intent.transition_id)
            || index > 0
                && journal.pending_archive_gc[index - 1].intent.updated_catalog
                    != intent.previous_catalog
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
        || checkpoint.transition_previous_catalog != Some(last.intent.previous_catalog)
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
                                && identity.lane_incarnation
                                    == leg.prepare_qc.body.participant_lane_incarnation
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

fn unscoped_lineage_root(bindings: &[LaneGeometryBinding]) -> Hash {
    let catalog = geometry_catalog_fingerprint(bindings);
    Hash::new_from_chunks(&[UNSCOPED_LINEAGE_DOMAIN, catalog.as_ref()])
}

fn lineage_root_is_zero(root: Hash) -> bool {
    root.as_ref().iter().all(|byte| *byte == 0)
}

fn geometry_transition_id(
    previous_catalog: Hash,
    previous_lineage_root: Hash,
    updated_catalog: Hash,
    updated_lineage_root: Hash,
) -> Hash {
    Hash::new_from_chunks(&[
        TRANSITION_DOMAIN,
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
    #[cfg(not(unix))]
    {
        let _ = metadata;
        GeometryFileIdentity {}
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
    use crate::{
        block::BlockBuilder,
        kura::CertifiedLaneBlockArtifact,
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

    #[test]
    fn post_write_publication_failure_restores_absent_description_only_journal() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let mut lanes = LaneCatalog::default().lanes().to_vec();
        lanes[0].description = Some("operator-only catalog description".to_owned());
        let catalog =
            LaneCatalog::new(nonzero!(1_u32), lanes).expect("description-only lane catalog");
        let config = RuntimeLaneConfig::from_catalog(&catalog);
        let previous = RuntimeLaneConfig::default();
        let baseline = iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(&catalog);
        let (incarnations, activation_heights) = initial_geometry();
        let kura = open_kura(&root, &previous);
        kura.apply_lane_geometry_transition(
            &previous,
            &config,
            &incarnations,
            &incarnations,
            &activation_heights,
            &activation_heights,
            &BTreeSet::new(),
        )
        .expect("description-only catalog has no physical geometry transition");
        let journal_path = kura.lane_geometry_journal_path();
        assert!(
            !journal_path.exists(),
            "description-only geometry application must not invent a recovery journal"
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
        assert!(
            !journal_path.exists(),
            "rollback must remove the newly published target when no prior journal existed"
        );
        let (restored_baseline, phases, has_temp) = kura
            .lane_geometry_journal_state_for_test()
            .expect("read restored absent journal state");
        assert_eq!(restored_baseline, None);
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
        let unrelated_kura = open_kura(&unrelated_root, &config);
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
            !unrelated_kura.lane_geometry_journal_path().exists(),
            "a temp collision must not publish a target"
        );

        let resumable_temp = TempDir::new().expect("temporary directory");
        let resumable_root = resumable_temp.path().join("kura");
        let resumable_kura = open_kura(&resumable_root, &config);
        let mut expected_journal = LaneGeometryJournal::default();
        expected_journal.configured_catalog_hash = Some(baseline);
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
            !resumable_kura.lane_geometry_journal_path().exists(),
            "post-write rollback restores the prior absent target"
        );
    }

    #[test]
    fn post_write_publication_failure_restores_exact_files_applied_journal() {
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
        .expect("prepare files-applied geometry intent");
        let journal_path = kura.lane_geometry_journal_path();
        let prior_bytes = fs::read(&journal_path).expect("capture exact files-applied journal");
        let prior_journal = decode_exact::<LaneGeometryJournal>(&prior_bytes)
            .expect("decode files-applied journal");
        assert_eq!(prior_journal.configured_catalog_hash, None);
        assert_eq!(
            prior_journal.records.last().map(|record| record.phase),
            Some(LaneGeometryPhase::FilesApplied)
        );
        let baseline = Hash::new(b"configured-catalog-baseline");
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
        assert_eq!(restored_baseline, None);
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
        .expect("prepare files-applied geometry intent");
        let prior_bytes = fs::read(kura.lane_geometry_journal_path())
            .expect("capture exact files-applied journal");
        let restore_temp = root.join(JOURNAL_RESTORE_TEMP_FILE_NAME);
        fs::write(&restore_temp, b"operator-owned-restore-temp")
            .expect("seed restore-temp collision");
        let baseline = Hash::new(b"configured-catalog-baseline");
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
            BTreeMap::from([(LaneId::SINGLE, 1), (LaneId::new(1), 1)]),
        )
    }

    fn install_retirement_test_lane_markers(
        kura: &Kura,
        config: &RuntimeLaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) {
        for binding in kura
            .geometry_bindings(config, incarnations, activation_heights)
            .expect("retirement test geometry bindings")
        {
            kura.write_lane_marker(&binding)
                .expect("install authoritative retirement-test lane marker");
        }
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
        participant_lane_incarnation: Hash,
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
            chain_id_hash,
            source_id,
            tx_entrypoint_hash: entrypoint_hash,
            plan_digest: plan.digest(),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: descriptor.lane_id,
            coordinator_dataspace_id: descriptor.dataspace_id,
            coordinator_lane_incarnation: descriptor.lane_incarnation,
            participant_lane_id: participant.route.lane_id,
            participant_dataspace_id: participant.route.dataspace_id,
            participant_lane_incarnation,
            participant_validator_set_hash: HashOf::new(&participant_validator_set),
            participant_validator_count: 1,
            participant_min_quorum: 1,
            authority_context_height: descriptor.proposal_height,
            planned_coordinator_block_height: descriptor.lane_block_height,
            coordinator_lane_block_view: descriptor.lane_block_view,
            coordinator_proposal_hash: coordinator_proposal.proposal_hash,
        };
        let participant_pop = bls_normal_pop_prove(participant_keypair.private_key())
            .expect("geometry retirement participant PoP");
        let qc = |body| NativeAmxAttestationQcV2 {
            body,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&participant_validator_set),
            validator_set: participant_validator_set.clone(),
            validator_set_pops: vec![participant_pop.clone()],
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
        participant_incarnation: Hash,
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
            participant_incarnation,
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
    fn scale_in_conservatively_rejects_pending_native_amx_participant_route() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let kura = open_kura(&root, &extended);
        install_retirement_test_lane_markers(
            &kura,
            &extended,
            &extended_incarnations,
            &extended_activations,
        );
        let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = autonomous_retirement_payload(
            extended_incarnations[&LaneId::SINGLE],
            LaneId::new(1),
            DataSpaceId::new(8),
            extended_incarnations[&LaneId::new(1)],
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
        assert_kura_io_error(
            &error,
            std::io::ErrorKind::WouldBlock,
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
        assert!(
            kura.read_lane_geometry_journal()
                .expect("geometry journal")
                .records
                .is_empty(),
            "rejected retirement does not publish an intent"
        );

        let certified_retirement = BTreeSet::from([(
            LaneId::new(1),
            DataSpaceId::new(8),
            extended_incarnations[&LaneId::new(1)],
        )]);
        let error = kura
            .apply_lane_geometry_transition_with_certified_retirements(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
                &certified_retirement,
            )
            .expect_err(
                "a drain certificate must not discard participant work coordinated elsewhere",
            );
        assert!(
            format!("{error:?}")
                .contains("pending autonomous payload targets a retiring lane incarnation"),
            "unexpected certified-retirement participant error: {error:?}"
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
        let kura = open_kura(&root, &extended);
        install_retirement_test_lane_markers(
            &kura,
            &extended,
            &extended_incarnations,
            &extended_activations,
        );
        let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = autonomous_retirement_payload(
            extended_incarnations[&LaneId::SINGLE],
            LaneId::new(9),
            DataSpaceId::new(19),
            Hash::new(b"unrelated participant incarnation"),
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
    fn certified_scale_in_archives_pending_work_owned_by_exact_retiring_incarnation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let kura = open_kura(&root, &extended);
        install_retirement_test_lane_markers(
            &kura,
            &extended,
            &extended_incarnations,
            &extended_activations,
        );
        let retiring_lane = LaneId::new(1);
        let retiring_dataspace = DataSpaceId::new(8);
        let retiring_incarnation = extended_incarnations[&retiring_lane];
        let artifact = certified_geometry_lane_block(
            retiring_lane,
            retiring_dataspace,
            retiring_incarnation,
            1,
        );
        kura.write_certified_lane_block_artifact(&artifact)
            .expect("persist late local certified work");

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
            .expect_err("ordinary retirement must remain blocked by pending certified work");
        assert!(
            format!("{error:?}")
                .contains("pending certified work belongs to a retiring lane incarnation"),
            "unexpected ordinary-retirement error: {error:?}"
        );

        kura.apply_lane_geometry_transition_with_certified_retirements(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
            &BTreeSet::from([(retiring_lane, retiring_dataspace, retiring_incarnation)]),
        )
        .expect("globally certified exact-incarnation retirement archives local stragglers");
        assert!(
            !extended
                .entry(retiring_lane)
                .expect("retiring lane entry")
                .blocks_dir(&root)
                .exists(),
            "certified retirement must move the old lane directory into the journal archive"
        );
    }

    #[test]
    fn certified_scale_in_rejects_wrong_retirement_identity() {
        for (label, dataspace, incarnation) in [
            (
                "wrong-dataspace",
                DataSpaceId::new(9),
                Hash::prehashed([0x62; Hash::LENGTH]),
            ),
            (
                "wrong-incarnation",
                DataSpaceId::new(8),
                Hash::prehashed([0x63; Hash::LENGTH]),
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
            let kura = open_kura(&root, &extended);
            install_retirement_test_lane_markers(
                &kura,
                &extended,
                &extended_incarnations,
                &extended_activations,
            );

            let error = kura
                .apply_lane_geometry_transition_with_certified_retirements(
                    &extended,
                    &initial,
                    &extended_incarnations,
                    &initial_incarnations,
                    &extended_activations,
                    &initial_activations,
                    &BTreeSet::new(),
                    &BTreeSet::from([(LaneId::new(1), dataspace, incarnation)]),
                )
                .expect_err("mismatched certified identity must not bypass retirement admission");
            assert!(
                format!("{error:?}").contains(
                    "certified retirement identity does not exactly match the retiring geometry"
                ),
                "unexpected {label} identity error: {error:?}"
            );
            assert!(
                kura.read_lane_geometry_journal()
                    .expect("geometry journal")
                    .records
                    .is_empty(),
                "{label} must fail before publishing a retirement intent"
            );
        }
    }

    #[test]
    fn scale_in_allows_recreated_incarnation_and_unrelated_participant_work() {
        for (label, participant_lane, participant_dataspace, participant_incarnation) in [
            (
                "recreated-incarnation",
                LaneId::new(1),
                DataSpaceId::new(8),
                Hash::prehashed([0x63; Hash::LENGTH]),
            ),
            (
                "unrelated-lane",
                LaneId::new(9),
                DataSpaceId::new(19),
                Hash::prehashed([0x69; Hash::LENGTH]),
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
            let kura = open_kura(&root, &extended);
            install_retirement_test_lane_markers(
                &kura,
                &extended,
                &extended_incarnations,
                &extended_activations,
            );
            let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let (chain_id_hash, epoch, payload) = autonomous_retirement_payload(
                extended_incarnations[&LaneId::SINGLE],
                participant_lane,
                participant_dataspace,
                participant_incarnation,
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
            .unwrap_or_else(|error| panic!("{label} should not pin old retirement: {error}"));
        }
    }

    #[test]
    fn scale_in_rejects_unknown_and_malformed_artifact_files_before_intent() {
        for (label, file_name, bytes) in [
            ("unknown", "operator-junk.bin", b"junk".as_slice()),
            (
                "stale-temp",
                "autonomous_blocks.norito.tmp",
                b"partial".as_slice(),
            ),
            (
                "malformed-view",
                "autonomous_view_1.norito",
                b"not-a-view-state".as_slice(),
            ),
            (
                "orphan-view",
                "autonomous_view_00000000000000000001.norito",
                b"not-a-view-state".as_slice(),
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
            let kura = open_kura(&root, &extended);
            install_retirement_test_lane_markers(
                &kura,
                &extended,
                &extended_incarnations,
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

            kura.apply_lane_geometry_transition(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
            )
            .unwrap_err();
            assert!(
                kura.read_lane_geometry_journal()
                    .expect("geometry journal")
                    .records
                    .is_empty(),
                "{label} artifact fails before an intent is published"
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
            let kura = open_kura(&root, &extended);
            install_retirement_test_lane_markers(
                &kura,
                &extended,
                &extended_incarnations,
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
            assert_kura_io_error(
                &error,
                std::io::ErrorKind::InvalidData,
                "lane retirement payload hint",
            );
            assert!(
                kura.read_lane_geometry_journal()
                    .expect("geometry journal")
                    .records
                    .is_empty(),
                "{label} fails before retirement intent"
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
        let kura = open_kura(&root, &extended);
        install_retirement_test_lane_markers(
            &kura,
            &extended,
            &extended_incarnations,
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
            extended_incarnations[&LaneId::new(1)],
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
            1
        );
    }

    #[test]
    fn consecutive_published_retirements_do_not_resurrect_intermediate_lanes() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let catalog = |active: u32| {
            let lanes = (0..active)
                .map(|raw| ModelLaneConfig {
                    id: LaneId::new(raw),
                    alias: if raw == 0 {
                        "default".to_owned()
                    } else {
                        format!("elastic-{raw}")
                    },
                    ..ModelLaneConfig::default()
                })
                .collect::<Vec<_>>();
            let catalog = LaneCatalog::new(
                NonZeroU32::new(4).expect("fixed geometry bound is non-zero"),
                lanes,
            )
            .expect("consecutive retirement catalog");
            RuntimeLaneConfig::from_catalog(&catalog)
        };
        let configs = [catalog(4), catalog(3), catalog(2), catalog(1)];
        let all_incarnations = (0_u32..4)
            .map(|raw| {
                (
                    LaneId::new(raw),
                    Hash::new(format!("consecutive-retirement-incarnation-{raw}")),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let all_activations = (0_u32..4)
            .map(|raw| (LaneId::new(raw), u64::from(raw)))
            .collect::<BTreeMap<_, _>>();
        let geometry_for = |active: u32| {
            let incarnations = all_incarnations
                .iter()
                .filter(|(lane, _)| lane.as_u32() < active)
                .map(|(lane, incarnation)| (*lane, *incarnation))
                .collect::<BTreeMap<_, _>>();
            let activations = all_activations
                .iter()
                .filter(|(lane, _)| lane.as_u32() < active)
                .map(|(lane, activation)| (*lane, *activation))
                .collect::<BTreeMap<_, _>>();
            (incarnations, activations)
        };
        let geometries = [
            geometry_for(4),
            geometry_for(3),
            geometry_for(2),
            geometry_for(1),
        ];
        let kura = open_kura(&root, &configs[0]);
        install_retirement_test_lane_markers(
            &kura,
            &configs[0],
            &geometries[0].0,
            &geometries[0].1,
        );

        for index in 0..3 {
            kura.apply_lane_geometry_transition(
                &configs[index],
                &configs[index + 1],
                &geometries[index].0,
                &geometries[index + 1].0,
                &geometries[index].1,
                &geometries[index + 1].1,
                &BTreeSet::new(),
            )
            .unwrap_or_else(|error| {
                panic!("consecutive retirement {} failed: {error:?}", index + 1)
            });
            kura.mark_lane_geometry_catalog_published(
                &configs[index + 1],
                &geometries[index + 1].0,
                &geometries[index + 1].1,
                None,
            )
            .expect("publish consecutive retirement catalog");

            for retired in (4 - u32::try_from(index).expect("index fits u32") - 1)..4 {
                assert!(
                    !configs[0]
                        .entry(LaneId::new(retired))
                        .expect("retired lane entry")
                        .blocks_dir(&root)
                        .exists(),
                    "published transition resurrected retired lane {retired}"
                );
            }
        }
        assert_eq!(
            kura.read_lane_geometry_journal()
                .expect("geometry journal")
                .records
                .len(),
            3,
            "every consecutive retirement remains recoverable until checkpoint GC"
        );
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
        kura.move_geometry_path(
            &kura
                .resolve_relative_path(&operation.unpublished_blocks_path)
                .expect("unpublished blocks path"),
            &lane1.blocks_dir(&root),
            true,
        )
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

        kura.apply_lane_geometry_transition_with_lineage_roots(
            &initial,
            &extended,
            &initial_incarnations,
            &first_incarnations,
            &initial_activations,
            &first_activations,
            lineage_initial,
            lineage_first_active,
            &BTreeSet::new(),
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
        kura.apply_lane_geometry_transition_with_lineage_roots(
            &extended,
            &initial,
            &first_incarnations,
            &initial_incarnations,
            &first_activations,
            &initial_activations,
            lineage_first_active,
            lineage_first_retired,
            &BTreeSet::new(),
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
        kura.apply_lane_geometry_transition_with_lineage_roots(
            &initial,
            &extended,
            &initial_incarnations,
            &second_incarnations,
            &initial_activations,
            &second_activations,
            lineage_first_retired,
            lineage_second_active,
            &BTreeSet::new(),
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

        // The later incarnation currently occupies the same live paths. The
        // rooted rollback must remove that suffix before repairing the earlier
        // retired prefix; otherwise the older retirement archive collides with
        // the live second incarnation.
        let lane1 = extended.entry(LaneId::new(1)).expect("lane one");
        kura.recover_lane_geometry_journal_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            lineage_first_retired,
        )
        .expect("recover first retired lineage while second incarnation is live");
        assert!(!lane1.blocks_dir(&root).exists());
        kura.recover_lane_geometry_journal_with_lineage_root(
            &extended,
            &second_incarnations,
            &second_activations,
            lineage_second_active,
        )
        .expect("restore second active lineage after exact rooted rollback");
        assert!(lane1.blocks_dir(&root).exists());

        kura.apply_lane_geometry_transition_with_lineage_roots(
            &extended,
            &initial,
            &second_incarnations,
            &initial_incarnations,
            &second_activations,
            &initial_activations,
            lineage_second_active,
            lineage_second_retired,
            &BTreeSet::new(),
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

        kura.recover_lane_geometry_journal_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            lineage_first_retired,
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
        kura.recover_lane_geometry_journal_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            Hash::new(b"lineage:unknown"),
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
            .recover_lane_geometry_journal_with_lineage_root(
                &initial,
                &initial_incarnations,
                &initial_activations,
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
        kura.recover_lane_geometry_journal(
            &fixture.extended,
            &fixture.extended_incarnations,
            &fixture.extended_activations,
        )
        .expect("old snapshot geometry remains recoverable before GC");
        kura.recover_lane_geometry_journal(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .expect("restore current snapshot geometry");

        durable_geometry_snapshot_identity(&kura, 20);
        let cached_before = kura.refresh_disk_usage_bytes().expect("usage before GC");
        let summary = checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect("checkpoint current snapshot geometry");
        assert_eq!(summary.compacted_transitions, 2);
        assert_eq!(summary.removed_archive_roots, 1);
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
        assert!(cached_after < cached_before);

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
        assert_eq!(summary.removed_archive_roots, 1);
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
        assert_eq!(resumed.removed_archive_roots, 1);
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
        kura.apply_lane_geometry_transition(
            &fixture.initial,
            &fixture.extended,
            &fixture.initial_incarnations,
            &recreated_incarnations,
            &fixture.initial_activations,
            &recreated_activations,
            &BTreeSet::new(),
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
            kura.fail_next_lane_geometry_gc_at_stage_for_test(stage);
            checkpoint_retired_geometry(&kura, &fixture, 20)
                .expect_err("injected GC boundary must interrupt acknowledgement");
            let after_failure = kura
                .read_lane_geometry_journal()
                .expect("journal after crash");
            assert!(after_failure.records.is_empty());
            if stage == GC_FAIL_AFTER_COMPACTION_INTENT {
                assert!(fixture.archive_root.exists());
                assert!(!quarantine.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else if stage == GC_FAIL_AFTER_ARCHIVE_QUARANTINE {
                assert!(!fixture.archive_root.exists());
                assert!(quarantine.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else if stage == GC_FAIL_AFTER_ARCHIVE_DELETION {
                assert!(!fixture.archive_root.exists());
                assert!(!quarantine.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else {
                assert!(!fixture.archive_root.exists());
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
            assert!(!fixture.archive_root.exists());
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

        // Model an operator/recovery worker discarding an uncertified local proposal. Once no
        // certified or autonomous work remains, the already-proven snapshot may release storage.
        fs::remove_file(autonomous_data).expect("remove uncertified payload");
        fs::remove_file(autonomous_index).expect("remove uncertified index");
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
        assert_kura_io_error(
            &error,
            std::io::ErrorKind::WouldBlock,
            "merge application receipt is missing or malformed",
        );
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
        durable_geometry_snapshot_identity(&kura, 20);
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
        let cached_after_partial = kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            cached_after_partial >= exact_after_partial,
            "a failed partial pass may over-account, but must never under-account retained bytes"
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
    fn recovery_rejects_both_branch_v4_journal_layouts_without_migration() {
        #[derive(Encode)]
        struct RootedCheckpointJournalV4 {
            version: u8,
            checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
            pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
            records: Vec<LaneGeometryIntent>,
        }

        #[derive(Encode)]
        struct ConfiguredCatalogJournalV4 {
            version: u8,
            configured_catalog_hash: Option<Hash>,
            // These containers are empty below, so their bytes exactly match the configured-
            // catalog branch's pre-lineage checkpoint and transition element types.
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
                "rooted-checkpoint v4",
                RootedCheckpointJournalV4 {
                    version: 4,
                    checkpoint: None,
                    pending_archive_gc: Vec::new(),
                    records: Vec::new(),
                }
                .encode(),
            ),
            (
                "configured-catalog v4",
                ConfiguredCatalogJournalV4 {
                    version: 4,
                    configured_catalog_hash: Some(Hash::new(b"configured-catalog-v4")),
                    checkpoint: None,
                    pending_archive_gc: Vec::new(),
                    records: Vec::new(),
                }
                .encode(),
            ),
        ];

        for (name, bytes) in obsolete_layouts {
            let journal_path = kura.lane_geometry_journal_path();
            fs::write(&journal_path, &bytes).expect("write obsolete v4 journal");

            let error = match kura.recover_lane_geometry_journal(
                &initial,
                &initial_incarnations,
                &initial_activations,
            ) {
                Ok(()) => panic!("{name} must not be migrated to journal v5"),
                Err(error) => error,
            };
            assert_eq!(
                fs::read(&journal_path).expect("read rejected v4 journal"),
                bytes,
                "recovery must leave the rejected {name} bytes untouched"
            );
            if name == "configured-catalog v4" {
                assert_kura_io_error(
                    &error,
                    std::io::ErrorKind::InvalidData,
                    "unsupported lane geometry journal version 4; expected 5",
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
        checkpoint_retired_geometry(&kura, &fixture, 20).expect("create rooted checkpoint v3");
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
            .expect_err("checkpoint v2 must not be interpreted as rooted checkpoint v3");
        assert_kura_io_error(
            &error,
            std::io::ErrorKind::InvalidData,
            "lane geometry checkpoint commitment or catalog is invalid",
        );
    }

    #[test]
    fn legacy_geometry_restore_fails_closed_without_retained_lineage() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let kura = open_kura(&root, &initial);
        let journal_path = kura.lane_geometry_journal_path();
        let journal_before = fs::read(&journal_path).ok();

        let error = kura
            .restore_lane_segments_with_geometry(
                &initial,
                &initial_incarnations,
                &initial_activations,
            )
            .expect_err("legacy restore cannot authenticate retained lineage");
        assert_kura_io_error(
            &error,
            std::io::ErrorKind::InvalidInput,
            "exact retained lane-incarnation lineage is required",
        );
        assert_eq!(
            fs::read(&journal_path).ok(),
            journal_before,
            "rejected legacy restore must not mutate recovery state"
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
    fn configured_catalog_preflight_rejects_tampered_v5_structure_before_lane_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("structural-baseline");
        let attempted_lane_catalog = configured_primary_catalog("must-not-open");
        let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted_lane_catalog);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish valid v5 baseline");

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
                previous_catalog,
                previous_lineage_root,
                updated_catalog,
                updated_lineage_root,
            ),
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

        Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &attempted_lane_config,
            &configured,
        )
        .expect_err("correct baseline must not mask a malformed v5 journal");
        assert_lane_paths_absent(&root, &attempted_lane_config);
    }

    #[test]
    fn configured_catalog_preflight_rejects_version_mismatch_before_lane_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let configured = configured_primary_catalog("version-baseline");
        let attempted_lane_catalog = configured_primary_catalog("version-must-not-open");
        let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted_lane_catalog);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
            .expect("establish valid v5 baseline");
        let journal_path = root.join(JOURNAL_FILE_NAME);
        let mut journal = decode_exact::<LaneGeometryJournal>(
            &fs::read(&journal_path).expect("read valid baseline journal"),
        )
        .expect("decode valid baseline journal");
        journal.version = JOURNAL_VERSION.saturating_add(1);
        fs::write(&journal_path, journal.encode()).expect("write unsupported journal version");

        Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &attempted_lane_config,
            &configured,
        )
        .expect_err("unsupported journal version must fail at the startup boundary");
        assert_lane_paths_absent(&root, &attempted_lane_config);
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

        let attempted_catalog = configured_primary_catalog("symlink-must-not-open");
        let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted_catalog);
        Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &attempted_lane_config,
            &configured,
        )
        .expect_err("journal-derived symlink must fail before opening attempted lane storage");
        assert_lane_paths_absent(&root, &attempted_lane_config);
        assert!(link.is_symlink());
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

        let attempted_catalog = configured_primary_catalog("identity-must-not-open");
        let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted_catalog);
        Kura::new_with_configured_lane_catalog(
            &kura_config(&root),
            &attempted_lane_config,
            &configured,
        )
        .expect_err("journal identity replacement during read must fail closed");
        assert_lane_paths_absent(&root, &attempted_lane_config);
        assert!(root.join(JOURNAL_IDENTITY_DISPLACED_FILE_NAME).is_file());
    }
}

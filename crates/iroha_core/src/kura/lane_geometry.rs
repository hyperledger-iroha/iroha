//! Crash-atomic Kura lane-geometry transitions.
use super::{
    AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES, AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
    AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX, AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE,
    AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE, AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE,
    AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX, AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
    AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES, AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
    AutonomousLaneBlockArtifact, AutonomousLaneBlockLatestAttemptV1,
    AutonomousLaneEntrypointClaimStateV1, AutonomousLaneEntrypointClaimV1,
    AutonomousLaneMergeBundleV1, AutonomousLifecycleBootstrapRecoveryStage,
    AutonomousLifecycleBootstrapV1, AutonomousLifecycleCursorPhaseKindV1,
    AutonomousLifecycleCursorPhaseV1, AutonomousLifecycleCursorV1,
    AutonomousLifecycleTerminalOutcomeBasisV1, AutonomousLifecycleTerminalOutcomeSourceV1,
    AutonomousLifecycleTerminalOutcomeV1, BlockStore, BlockStoreCommitMarker,
    BoundProgressDirectory, BoundProgressNamespace, BoundProgressPair,
    BoundProgressRecoveryFailure, CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
    CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE, CANONICAL_AUTONOMOUS_LANE_REPLICAS_INDEX_FILE,
    CERTIFIED_LANE_BLOCKS_DATA_FILE, CERTIFIED_LANE_BLOCKS_INDEX_FILE, COUNT_FILE_NAME,
    DATA_FILE_NAME, Error, HASHES_FILE_NAME, HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1,
    HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS, HistoricalAutonomousLaneRecoveryRecordV1,
    INDEX_FILE_NAME, Kura, LANE_ARTIFACTS_DATA_FILE, LANE_ARTIFACTS_DIR_NAME,
    LANE_ARTIFACTS_INDEX_FILE, LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
    LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE, LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE, LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
    LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE, LANE_MERGE_APPLICATION_FRONTIER_FILE,
    LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_BUILD_FILE, LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_FILE,
    LaneArtifactPhysicalTarget, LaneArtifactStorageView, LaneBlockApplicationReceiptArtifact,
    LaneBlockApplicationReceiptArtifactFormat, LaneBlockArtifact, LaneBlockExecutionInputArtifact,
    LaneBlockExecutionPreflightArtifact, LaneBlockExecutionSourceV1, LaneHistoryCompactionOutcome,
    LaneMergeApplicationFrontierV1, LaneStorageEntry, LaneStorageIdentity,
    MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES, MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
    MergeLedgerCarrierRecord, NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE,
    NativeAmxEvidenceKind, NativeAmxParticipantApplicationManifestArtifactV1,
    NativeAmxParticipantApplicationObservation, NativeAmxParticipantApplicationReceiptArtifact,
    RecoveredLaneBlockPayload, Result, STRICT_INIT_MAX_BLOCK_BYTES,
    bounded_historical_autonomous_recovery_entries, create_dir_all_with_context, sync_dir,
};
#[cfg(test)]
use super::{
    AUTONOMOUS_LANE_BLOCK_ATTEMPT_PREFIX, AutonomousLaneReleaseProjectionContext,
    AutonomousLaneReleasedClaimDisposition, AutonomousLifecycleAttemptBindingV1,
    AutonomousLifecycleCursorUnsignedV1, AutonomousLifecycleProcessGenerationClaim,
    AutonomousLifecycleStableStateV1, DEFAULT_NATIVE_AMX_PARTICIPANT_EVIDENCE_FILE_BYTES,
    NATIVE_AMX_APPLICATION_MANIFEST_FILE_PREFIX, NATIVE_AMX_EVIDENCE_FILE_SUFFIX,
    NATIVE_AMX_EVIDENCE_HEIGHT_DIGITS, OBSOLETE_AUTONOMOUS_LANE_BLOCKS_DATA_FILE,
    OBSOLETE_AUTONOMOUS_LANE_BLOCKS_INDEX_FILE, Ordering,
    ProductionInFlightFirstReleaseTransitionProjection, SidecarIndexEntry, SidecarIndexLayout,
    V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY,
    lane_queue_reservation_group_binding_from_ordered_keys,
};
#[cfg(unix)]
use crate::json_macros::{JsonDeserialize, JsonSerialize};
use crate::secure_file_metadata::{self, SecureMetadata};
#[cfg(test)]
use crate::{
    queue::canonical_lane_queue_reservation_group_identity_projection,
    sumeragi::v2_core::{
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN,
        ProductionInFlightFirstReleaseCarrierProjection,
        ProductionInFlightFirstReleaseDecisionProjection,
        ProductionInFlightFirstReleaseHistoryProjection,
        ProductionInFlightFirstReleaseQueueProjection,
        ProductionInFlightFirstReleaseReleaseProjection,
        ProductionInFlightFirstReleaseSessionProjection,
        ProductionInFlightFirstReleaseStateProjection,
        production_in_flight_first_release_state_kernel,
    },
};
use iroha_config::parameters::actual::{LaneConfig, LaneConfigEntry};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader, SignedBlock,
        consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1, SumeragiLanePayloadOwnership},
        execution_context::ExternalExecutionContext,
    },
    merge::{LaneDrainFrontierV1, MergeLedgerEntry},
    transaction::signed::TransactionEntrypoint,
};
use iroha_model_base::state_path::StatePath;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use norito::codec::{Decode, Encode};
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
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    num::NonZeroUsize,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use prepared_journal::PreparedGeometryJournalTransition;
pub(super) use raw_attempt::RawGeometryClaimGate;
use raw_attempt::RawGeometryMutation;
pub use raw_attempt::RawGeometryWait;
pub(crate) use raw_attempt::{RawGeometryAttempt, RawGeometryPhase};
use retirement_observation::{LaneRetirementCensus, RetirementScanEffects};

#[cfg(test)]
std::thread_local! {
    // A native mkdir and descriptor capture have succeeded; inventory capture fails.
    static FAIL_NEXT_GEOMETRY_NAMESPACE_INVENTORY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_GEOMETRY_INSTANCE_AFTER_FILES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}


#[cfg(test)]
std::thread_local! {
    static FAIL_ARCHIVED_RECEIPT_DURABILITY_ATTESTATION: std::cell::Cell<Option<ProgressSidecarDurabilityFault>> = const { std::cell::Cell::new(None) };
    static SUBSTITUTE_PROGRESS_DIRECTORY_AFTER_RECOVERY: std::cell::RefCell<Option<(String, PathBuf, PathBuf)>> = const { std::cell::RefCell::new(None) };
}
#[cfg(not(unix))]
static UNSUPPORTED_GEOMETRY_IDENTITY_NONCE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);
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
pub(super) struct StorageOpenPreflight {
    store_root: PathBuf,
    root_identity: GeometryFileIdentity,
    blocks_path: PathBuf,
    blocks_identity: Option<GeometryFileIdentity>,
    merge_path: PathBuf,
    merge_identity: Option<GeometryFileIdentity>,
}
/// Fixed-cardinality canonical namespace identities; never scans unrelated lanes.
#[derive(Debug)]
pub(super) struct CanonicalStoragePreflight {
    paths: StorageOpenPreflight,
    parents: [Option<GeometryFileIdentity>; 2],
    pub(super) requires_existing_files: bool,
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
    let metadata = secure_file_metadata::from_path(store_root)
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
    let metadata = secure_file_metadata::from_path(path)
        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
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



impl Kura {
    /// Canonical storage is chain-scoped and never belongs to a lane binding.
    pub fn canonical_storage_paths(store_root: &Path) -> (PathBuf, PathBuf) {
        (
            store_root.join("blocks/canonical"),
            store_root.join("merge_ledger/canonical.log"),
        )
    }

    /// Authenticate the fixed chain namespace before any canonical file is opened.
    /// An anchored catalog cannot silently recreate a missing canonical chain.
    pub(super) fn preflight_canonical_storage(
        store_root: &Path,
    ) -> Result<CanonicalStoragePreflight> {
        let root_identity = configured_catalog_store_root_identity(store_root)?;
        let journal = read_configured_catalog_journal_for_preflight(
            store_root,
            root_identity,
            &store_root.join(JOURNAL_FILE_NAME),
            false,
        )?;
        let bound = journal.as_ref().is_some_and(|journal| {
            journal.journal.configured_primary_binding.is_some()
                || journal.journal.checkpoint.is_some()
                || !journal.journal.records.is_empty()
        });
        let (blocks, merge) = Self::canonical_storage_paths(store_root);
        let blocks_identity =
            configured_geometry_path_identity(store_root, root_identity, &blocks, true)?;
        let merge_identity =
            configured_geometry_path_identity(store_root, root_identity, &merge, false)?;
        // Canonical journals are mutable single-owner files. A foreign hard link
        // must be rejected before startup can repair or truncate any journal.
        for path in [
            blocks.join(DATA_FILE_NAME),
            blocks.join(INDEX_FILE_NAME),
            blocks.join(HASHES_FILE_NAME),
            blocks.join(COUNT_FILE_NAME),
            merge.clone(),
        ] {
            match secure_file_metadata::from_path(&path) {
                Ok(metadata)
                    if metadata.file_type().is_symlink()
                        || !metadata.is_file()
                        || !Self::sidecar_is_single_link(&metadata) =>
                {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "canonical journal must be a regular single-link file",
                        ),
                        path,
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::IO(error, path)),
            }
        }
        if bound {
            if blocks_identity.is_none() || merge_identity.is_none() {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::NotFound,
                    "bound canonical storage is missing",
                ));
            }
        } else {
            if merge_identity.is_some() && blocks_identity.is_none() {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "unbound canonical storage is only partially present",
                ));
            }
            if blocks_identity.is_some() {
                preflight_empty_block_store_without_marker(&blocks, None, false)?;
            }
            if merge_identity.is_some()
                && fs::symlink_metadata(&merge)
                    .map_err(|error| Error::IO(error, merge.clone()))?
                    .len()
                    != 0
            {
                return Err(configured_catalog_preflight_error(
                    store_root,
                    ErrorKind::InvalidData,
                    "unbound canonical merge ledger is not empty",
                ));
            }
        }
        configured_catalog_require_store_root_identity(store_root, root_identity)?;
        let parents = [
            configured_geometry_path_identity(
                store_root,
                root_identity,
                &store_root.join("blocks"),
                true,
            )?,
            configured_geometry_path_identity(
                store_root,
                root_identity,
                &store_root.join("merge_ledger"),
                true,
            )?,
        ];
        Ok(CanonicalStoragePreflight {
            paths: StorageOpenPreflight {
                store_root: store_root.to_owned(),
                root_identity,
                blocks_path: blocks,
                blocks_identity,
                merge_path: merge,
                merge_identity,
            },
            parents,
            requires_existing_files: bound,
        })
    }

    fn reverify_storage_open_path(
        preflight: &mut StorageOpenPreflight,
        path: &Path,
        directory: bool,
        establish_created: bool,
    ) -> Result<()> {
        let store_root = preflight.store_root.clone();
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
    /// Bind parents created by this constructor's admitted initial lane provisioning.
    pub(super) fn reverify_canonical_storage_parents(
        preflight: &mut CanonicalStoragePreflight,
        establish_created: bool,
    ) -> Result<()> {
        let paths = &preflight.paths;
        for (index, name) in ["blocks", "merge_ledger"].iter().enumerate() {
            let path = paths.store_root.join(name);
            let actual = configured_geometry_path_identity(
                &paths.store_root,
                paths.root_identity,
                &path,
                true,
            )?;
            match (preflight.parents[index], actual, establish_created) {
                (Some(expected), Some(actual), _) if expected == actual => {}
                (None, None, _) => {}
                (None, Some(actual), true) => preflight.parents[index] = Some(actual),
                _ => {
                    return Err(configured_catalog_preflight_error(
                        &paths.store_root,
                        ErrorKind::InvalidData,
                        "canonical storage parent identity changed during open",
                    ));
                }
            }
        }
        Ok(())
    }
    /// Reverify only the retained chain namespace, independent of retired lane evidence.
    pub(super) fn reverify_canonical_blocks_open(
        preflight: &mut CanonicalStoragePreflight,
        path: &Path,
        establish_created: bool,
    ) -> Result<()> {
        Self::reverify_canonical_storage_parents(preflight, establish_created)?;
        Self::reverify_storage_open_path(&mut preflight.paths, path, true, establish_created)
    }






}
include!("lane_geometry/catalog_validation.rs");


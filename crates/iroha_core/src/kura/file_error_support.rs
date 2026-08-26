/// Helper to reduce boilerplate of file operations while preserving path context.
struct FileWrap {
    path: PathBuf,
    file: std::fs::File,
}
impl std::fmt::Debug for FileWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWrap")
            .field("path", &self.path)
            .field("file", &self.file)
            .finish()
    }
}
impl FileWrap {
    fn open_with(path: PathBuf, configure: impl FnOnce(&mut std::fs::OpenOptions)) -> Result<Self> {
        let mut options = std::fs::OpenOptions::new();
        configure(&mut options);
        let file = options.open(path.clone()).add_err_context(&path)?;
        Ok(Self { path, file })
    }
    fn open_read_write(path: PathBuf) -> Result<Self> {
        Self::open_with(path, |opts| {
            opts.write(true).read(true).create(true).truncate(false);
        })
    }
    fn try_io<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut std::fs::File) -> std::io::Result<T>,
    {
        let value = f(&mut self.file).add_err_context(&self.path)?;
        Ok(value)
    }
}
fn create_dir_all_with_context(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|err| Error::MkDir(err, path.to_path_buf()))
}
fn sync_dir(path: &Path) -> std::io::Result<()> {
    let file = std::fs::File::open(path)?;
    file.sync_all()
}
fn remove_commit_marker_temp_and_sync(path: &Path) -> Result<()> {
    std::fs::remove_file(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if let Some(parent) = path.parent() {
        sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    }
    Ok(())
}
fn promote_commit_marker_temp_and_sync(temporary_path: &Path, stable_path: &Path) -> Result<()> {
    std::fs::rename(temporary_path, stable_path)
        .map_err(|error| Error::IO(error, stable_path.to_path_buf()))?;
    let persisted = std::fs::OpenOptions::new()
        .read(true)
        .open(stable_path)
        .map_err(|error| Error::IO(error, stable_path.to_path_buf()))?;
    persisted
        .sync_all()
        .map_err(|error| Error::IO(error, stable_path.to_path_buf()))?;
    if let Some(parent) = stable_path.parent() {
        sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
    }
    Ok(())
}
fn sync_bound_progress_intent_file(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_BOUND_PROGRESS_INTENT_FILE_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected bound progress append-intent sync failure",
        ));
    }
    file.sync_data()
}
fn sync_bound_progress_append_data(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_BOUND_PROGRESS_APPEND_DATA_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected journaled progress payload sync failure",
        ));
    }
    file.sync_data()
}
fn sync_bound_progress_append_index(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_BOUND_PROGRESS_APPEND_INDEX_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected journaled progress index sync failure",
        ));
    }
    file.sync_data()
}
fn sync_native_amx_latest_index_recovery_temp(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_NATIVE_AMX_LATEST_INDEX_RECOVERY_TEMP_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected Native AMX latest-index recovery temporary sync failure",
        ));
    }
    file.sync_all()
}
fn sync_indexed_sidecar_data(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected indexed sidecar data sync failure",
        ));
    }
    file.sync_data()
}
fn sync_indexed_sidecar_initial_data(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_INDEXED_SIDECAR_INITIAL_DATA_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected initial indexed sidecar data sync failure",
        ));
    }
    file.sync_data()
}
fn rollback_unindexed_sidecar_payload(
    file: &std::fs::File,
    offset: u64,
    data_path: &Path,
    kind: &str,
) -> bool {
    if let Err(err) = file.set_len(offset) {
        iroha_logger::warn!(
            ?err,
            ?data_path,
            offset,
            kind,
            "failed to truncate unpublished sidecar payload"
        );
        return false;
    }
    if let Err(err) = file.sync_data() {
        iroha_logger::warn!(
            ?err,
            ?data_path,
            offset,
            kind,
            "failed to synchronize unpublished sidecar payload rollback"
        );
        return false;
    }
    true
}
fn sync_indexed_sidecar_index(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected indexed sidecar index sync failure",
        ));
    }
    file.sync_data()
}
fn sync_indexed_sidecar_dir(path: &Path) -> std::io::Result<()> {
    let file = std::fs::File::open(path)?;
    sync_indexed_sidecar_dir_handle(&file)
}
fn sync_indexed_sidecar_dir_handle(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected indexed sidecar directory sync failure",
        ));
    }
    file.sync_all()
}
fn sync_progress_sidecar_ancestor_dir_handle(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_PROGRESS_SIDECAR_ANCESTOR_SYNC_AT.with(|slot| {
        let Some(mut fault) = slot.get() else {
            return false;
        };
        if fault.remaining_to_target > 0 {
            fault.remaining_to_target -= 1;
            slot.set(Some(fault));
            return false;
        }
        fault.failures_remaining -= 1;
        if fault.failures_remaining == 0 {
            slot.set(None);
        } else {
            fault.remaining_to_target = fault.target_index;
            slot.set(Some(fault));
        }
        true
    }) {
        return Err(std::io::Error::other(
            "injected progress sidecar ancestor directory sync failure",
        ));
    }
    file.sync_all()
}
fn sync_sidecar_promotion_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_SIDECAR_PROMOTION_DIR_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected sidecar promotion directory sync failure",
        ));
    }
    sync_dir(path)
}
fn sync_sidecar_temp_marker_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected sidecar temp marker directory sync failure",
        ));
    }
    sync_dir(path)
}
fn numbered_norito_sidecar_height(path: &Path) -> Option<u64> {
    let file_name = path.file_name()?.to_str()?;
    let height = file_name
        .strip_suffix(".norito")
        .or_else(|| file_name.strip_suffix(".norito.tmp"))?;
    height.parse().ok()
}
#[cfg(test)]
const CONFIGURED_PRIMARY_OPEN_IDENTITY_SWAP_SUFFIX: &str = ".configured-primary-open-identity-swap";
#[cfg(test)]
const CONFIGURED_PRIMARY_OPEN_IDENTITY_DISPLACED_SUFFIX: &str =
    ".configured-primary-open-identity-displaced";
#[cfg(test)]
fn configured_primary_open_identity_test_path(path: &Path, suffix: &str) -> Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "configured-primary identity test path has no file name",
            ),
            path.to_path_buf(),
        )
    })?;
    let mut sibling_name = file_name.to_os_string();
    sibling_name.push(suffix);
    Ok(path.with_file_name(sibling_name))
}
/// Deterministically model an inode replacement after authenticated preflight.
///
/// Test fixtures opt in by placing a replacement at the reserved sibling path.
/// The constructor must reject that replacement at its next identity boundary,
/// before opening it for mutation.
#[cfg(test)]
fn configured_primary_open_identity_swap_boundary(path: &Path) -> Result<()> {
    let replacement = configured_primary_open_identity_test_path(
        path,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_SWAP_SUFFIX,
    )?;
    match std::fs::symlink_metadata(&replacement) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::IO(error, replacement)),
    }
    let displaced = configured_primary_open_identity_test_path(
        path,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_DISPLACED_SUFFIX,
    )?;
    if std::fs::symlink_metadata(&displaced).is_ok() {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::AlreadyExists,
                "configured-primary identity test displaced path already exists",
            ),
            displaced,
        ));
    }
    std::fs::rename(path, &displaced).map_err(|error| Error::IO(error, path.to_path_buf()))?;
    if let Err(error) = std::fs::rename(&replacement, path) {
        let _ = std::fs::rename(&displaced, path);
        return Err(Error::IO(error, replacement));
    }
    Ok(())
}
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
/// Error variants for persistent storage logic
#[derive(thiserror::Error, Debug, displaydoc::Display)]
pub enum Error {
    /// Production Kura store root resolved to an empty path
    EmptyStoreRoot,
    /// Failed reading/writing {1:?} from disk: {0}
    IO(#[source] std::io::Error, PathBuf),
    /// Lane-geometry publication failed and exact prior-journal restoration was not proven: publication={publication}; restoration={restoration}
    LaneGeometryPublicationRestoreFailed {
        /// Original catalog-publication error.
        publication: String,
        /// Exact prior-journal restoration error.
        restoration: String,
    },
    /// Failed to create the directory {1:?}
    MkDir(#[source] std::io::Error, PathBuf),
    /// Failed to serialize/deserialize versioned payloads
    VersionedCodec(#[from] iroha_version::error::Error),
    /// Failed to frame or deframe Norito payload
    NoritoFrame(#[from] norito::core::Error),
    /// Invalid Sumeragi v2 finality artifact: {0}
    V2FinalityArtifact(#[from] V2FinalityValidationError),
    /// Invalid Sumeragi v2 finality cryptography: {0}
    V2FinalityCryptography(#[from] V2QuorumCertificateVerificationError),
    /// Encoded Sumeragi v2 finality artifact is {actual} bytes; hard maximum is {max}
    V2FinalityArtifactTooLarge {
        /// Encoded artifact size.
        actual: usize,
        /// Hard persistence/read limit.
        max: usize,
    },
    /// Encoded Kura Sumeragi v2 finality record is {actual} bytes; hard maximum is {max}
    V2FinalityRecordTooLarge {
        /// Encoded private record size.
        actual: usize,
        /// Hard persistence/read limit.
        max: usize,
    },
    /// Encoded Kura retained block record is {actual} bytes; hard maximum is {max}
    RetainedBlockRecordTooLarge {
        /// Encoded retained block-record size.
        actual: usize,
        /// Hard persistence/read limit.
        max: usize,
    },
    /// Immutable retained block record at height `{height}` conflicts with canonical data
    ConflictingRetainedBlockRecord {
        /// Height whose retained-block path contains different canonical data.
        height: u64,
    },
    /// Sumeragi-v2 finality at height `{height}` authenticates a different canonical proposal wire image
    V2FinalityPayloadHashMismatch {
        /// Height whose signed subject differs from the retained resultless proposal hash.
        height: u64,
    },
    /// Sumeragi-v2 finality at height `{height}` authenticates a different executed block wire length
    V2FinalityExecutedBlockWireLengthMismatch {
        /// Height whose execution commitment differs from the retained result-bearing block length.
        height: u64,
    },
    /// Sumeragi-v2 finality at height `{height}` authenticates a different executed block wire image
    V2FinalityExecutedBlockWireHashMismatch {
        /// Height whose execution commitment differs from the retained result-bearing block hash.
        height: u64,
    },
    /// Submitted block wire at existing canonical height `{height}` differs from durable canonical bytes
    CanonicalBlockWireMismatch {
        /// Existing height whose header matched but complete block bytes differed.
        height: u64,
    },
    /// DA block rewrite commit state is unknown after an I/O failure: {detail}
    DaBlockRewriteCommitStateUnknown {
        /// Failure details retained for fail-stop diagnostics.
        detail: String,
    },
    /// Canonical block publication is committed but requires restart recovery: {detail}
    CanonicalBlockCommittedRecoveryRequired {
        /// Failure details retained for fail-stop diagnostics.
        detail: String,
    },
    /// Canonical Kura storage is fail-stop poisoned after an ambiguous rewrite publication
    CanonicalStoragePoisoned,
    /// Invalid provisional snapshot bootstrap marker at `{path:?}`: {reason}
    InvalidSnapshotBootstrapMarker {
        /// Marker path whose bytes or bounds are invalid.
        path: PathBuf,
        /// Stable validation diagnostic.
        reason: String,
    },
    /// Kura hash-only history is provisional until a signed snapshot authenticates its lineage
    SnapshotBootstrapAuthenticationPending,
    /// Authenticated snapshot bootstrap finalization failed: {reason}
    SnapshotBootstrapFinalization {
        /// Exact deferred recovery or immutable context-publication failure.
        reason: String,
    },
    /// Kura is already bound to a different authoritative consensus output guard
    ConsensusOutputGuardAlreadyBound,
    /// Kura is already bound to a different local peer identity
    KuraReplicaLocalPeerConflict,
    /// Kura cannot start its writer before the immutable local peer identity is bound
    KuraReplicaLocalPeerUnbound,
    /// Invalid authenticated Kura replica advert: {0}
    InvalidKuraReplicaAdvert(String),
    /// Invalid Kura replica-advert runtime configuration: {0}
    InvalidKuraReplicaAdvertConfiguration(String),
    /// Canonical block at height `{height}` is missing its required durable retained record
    MissingRetainedBlockRecord {
        /// Height whose eviction/finality evidence lacks the required record.
        height: u64,
    },
    /// Evicted canonical block at height `{height}` is missing signed complete-wire finality
    MissingV2FinalityArtifact {
        /// Height whose durable eviction marker has no finality artifact.
        height: u64,
    },
    /// Highest retained-block height `{retained_height}` exceeds the canonical durable block height `{durable_height}`
    RetainedBlockBeyondDurableChain {
        /// Highest canonical retained-block file discovered in the immutable inventory.
        retained_height: u64,
        /// Height published by the durable block-store marker.
        durable_height: u64,
    },
    /// Invalid retained SCCP archive at height `{height}`: {reason}
    InvalidRetainedSccpArchive {
        /// Canonical block height whose bounded archive is invalid.
        height: u64,
        /// Bounded structural or commitment diagnostic.
        reason: String,
    },
    /// Canonical block header for Sumeragi v2 finality height `{height}` is unavailable
    V2FinalityCanonicalHeaderUnavailable {
        /// Height whose complete canonical header could not be loaded.
        height: u64,
    },
    /// Invalid or conflicting Kagemusha top-up finality sidecar: {0}
    KagemushaTopUpFinalitySidecar(String),
    /// Encoded Kagemusha top-up finality sidecar is {actual} bytes; hard maximum is {max}
    KagemushaTopUpFinalitySidecarTooLarge {
        /// Encoded sidecar size.
        actual: usize,
        /// Hard persistence/read limit.
        max: usize,
    },
    /// Invalid or conflicting Kagemusha active-receiver finality sidecar: {0}
    KagemushaActiveReceiverFinalitySidecar(String),
    /// Encoded Kagemusha active-receiver sidecar is {actual} bytes; hard maximum is {max}
    KagemushaActiveReceiverFinalitySidecarTooLarge {
        /// Encoded sidecar size.
        actual: usize,
        /// Hard persistence/read limit.
        max: usize,
    },
    /// Conflicting immutable Sumeragi v2 finality artifact at height `{height}`
    ConflictingV2FinalityArtifact {
        /// Height whose finality path already contains a different artifact.
        height: u64,
    },
    /// Retired first-release-incompatible Kura artifact remains at `{path:?}`
    RetiredKuraArtifact {
        /// Exact retired artifact that must be removed by the operator.
        path: PathBuf,
    },
    /// Canonical-chain mutation from height `{rewrite_from_height}` would rewrite durable Sumeragi-v2 finality at height `{finalized_height}`
    FinalizedV2BlockMutation {
        /// First canonical height the requested mutation could replace or remove.
        rewrite_from_height: u64,
        /// Highest durable finality artifact that makes the mutation invalid.
        finalized_height: u64,
    },
    /// Canonical block at height `{height}` has committed WSV replay metadata and cannot be replaced
    CommittedBlockReplacementForbidden {
        /// State-committed canonical height protected by its checkpoint or commit manifest.
        height: u64,
    },
    /// Highest durable Sumeragi-v2 finality height `{finalized_height}` exceeds the canonical durable block height `{durable_height}`
    V2FinalityBeyondDurableChain {
        /// Highest canonical finality sidecar file discovered at startup.
        finalized_height: u64,
        /// Height published by the durable block-store marker.
        durable_height: u64,
    },
    /// Failed to allocate buffer
    Alloc(#[from] std::collections::TryReserveError),
    /// Tried reading block data out of bounds: start `{start_block_height}`, count `{block_count}`
    OutOfBoundsBlockRead {
        /// The block height from which the read was supposed to start
        start_block_height: u64,
        /// The actual block count
        block_count: usize,
    },
    /// Another live Kura instance owns the store-root lock at {0}
    Locked(PathBuf),
    /// Block writer thread unavailable; persistence notifications cannot be delivered
    BlockWriterUnavailable,
    /// Block writer thread faulted and stopped processing new blocks: {0}
    BlockWriterFaulted(String),
    /// Conversion of wide integer into narrow integer failed. This error cannot be caught at compile time at present
    IntConversion(#[from] std::num::TryFromIntError),
    /// Blocks count differs hashes file and index file
    HashesFileHeightMismatch,
    /// Invalid canonical suffix above provisional snapshot prefix at height `{height}`: {reason}
    InvalidProvisionalSnapshotSuffix {
        /// One-based suffix height which failed validation.
        height: u64,
        /// Stable validation diagnostic.
        reason: String,
    },
    /// Hard-fork snapshot bootstrap requires Kura hashes height `{hashes_count}` to match index height `{index_count}`
    HardForkSnapshotBootstrapHashHeightMismatch {
        /// Number of durable block index entries.
        index_count: usize,
        /// Number of block hashes recorded in the hashes journal.
        hashes_count: usize,
    },
    /// Block index length {length} exceeds strict-init guard {limit} bytes
    CorruptedBlockLength {
        /// Length of the corrupted block index entry in bytes.
        length: u64,
        /// Configured upper bound for permissible block index entries.
        limit: u64,
    },
    /// Block range start {start} + length {length} exceeds data file length `{data_len}` bytes
    CorruptedBlockRange {
        /// Offset in the data file where the range begins.
        start: u64,
        /// Number of bytes that were requested to be read starting at `start`.
        length: u64,
        /// Total number of bytes available in the data file.
        data_len: u64,
    },
    /// Kura storage budget exceeded: limit {limit} bytes, used {used} bytes, required {required} bytes
    StorageBudgetExceeded {
        /// Configured storage cap in bytes.
        limit: u64,
        /// Bytes currently occupied by the block store.
        used: u64,
        /// Bytes required after accepting the next block.
        required: u64,
    },
    /// Block height gap: expected next canonical height `{expected_next_height}`, got `{actual_height}`
    BlockHeightGap {
        /// Next height Kura can append without leaving a gap.
        expected_next_height: u64,
        /// Height declared by the block being stored.
        actual_height: u64,
    },
    /// Block height conflict at `{height}`: stored hash `{expected:?}`, incoming hash `{actual:?}`
    BlockHeightConflict {
        /// Conflicting block height.
        height: u64,
        /// Hash already stored at that height.
        expected: HashOf<BlockHeader>,
        /// Hash of the incoming block.
        actual: HashOf<BlockHeader>,
    },
    /// Certified merge sidecar `{entry_hash:?}` is unavailable
    MissingCertifiedMergeSidecar {
        /// Hash requested by the compact block reference.
        entry_hash: HashOf<MergeLedgerEntry>,
    },
    /// Certified merge compact reference is inconsistent: {0}
    MergeReferenceMismatch(String),
    /// Durable sparse merge carrier is inconsistent: {0}
    MergeCarrierConflict(String),
    /// Kura requires restart to complete an interrupted durable prune transaction
    PruneRecoveryRequired,
    /// Durable Kura prune intent is inconsistent: {0}
    PruneIntentConflict(String),
}
impl Error {
    /// Return whether this error proves that the canonical publication boundary cannot be
    /// retried safely by the live consensus process.
    #[must_use]
    pub(crate) const fn requires_restart_recovery(&self) -> bool {
        matches!(
            self,
            Self::DaBlockRewriteCommitStateUnknown { .. }
                | Self::CanonicalBlockCommittedRecoveryRequired { .. }
                | Self::CanonicalStoragePoisoned
                | Self::PruneRecoveryRequired
        )
    }
}
trait AddErrContextExt<T> {
    type Context;
    fn add_err_context(self, context: &Self::Context) -> Result<T, Error>;
}
impl<T> AddErrContextExt<T> for Result<T, std::io::Error> {
    type Context = PathBuf;
    fn add_err_context(self, path: &Self::Context) -> Result<T, Error> {
        self.map_err(|e| Error::IO(e, path.clone()))
    }
}

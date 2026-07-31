#[derive(Debug, Clone)]
struct NativeAmxEvidenceFile {
    kind: NativeAmxEvidenceKind,
    participant_height: u64,
    path: PathBuf,
    metadata: StableSidecarMetadata,
}

#[derive(Debug, Default)]
struct NativeAmxEvidenceInventory {
    manifests: BTreeMap<u64, NativeAmxEvidenceFile>,
    receipts: BTreeMap<u64, NativeAmxEvidenceFile>,
    temporaries: BTreeMap<NativeAmxEvidenceKind, NativeAmxEvidenceFile>,
    manifest_stable_bytes: u64,
    receipt_stable_bytes: u64,
}

impl NativeAmxEvidenceInventory {
    fn stable(&self, kind: NativeAmxEvidenceKind) -> &BTreeMap<u64, NativeAmxEvidenceFile> {
        match kind {
            NativeAmxEvidenceKind::Manifest => &self.manifests,
            NativeAmxEvidenceKind::Receipt => &self.receipts,
        }
    }

    fn stable_bytes(&self, kind: NativeAmxEvidenceKind) -> u64 {
        match kind {
            NativeAmxEvidenceKind::Manifest => self.manifest_stable_bytes,
            NativeAmxEvidenceKind::Receipt => self.receipt_stable_bytes,
        }
    }

    fn temporary(&self, kind: NativeAmxEvidenceKind) -> Option<&NativeAmxEvidenceFile> {
        self.temporaries.get(&kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeAmxEvidenceRecoveryPhase {
    ManifestPublication,
    ReceiptPublication,
    Startup,
}

impl BoundProgressNamespace {
    /// Return the descriptor-bound parent path as canonical UTF-8 components
    /// relative to the Kura root. The identity survives relocation of the
    /// entire Kura root while distinguishing same-named pairs in sibling lane
    /// directories.
    fn stable_relative_components(
        &self,
        data_path: &Path,
        index_path: &Path,
    ) -> std::result::Result<Vec<String>, &'static str> {
        if self.data_path != data_path || self.index_path != index_path {
            return Err("bound progress namespace names a different main pair");
        }
        let parent = data_path
            .parent()
            .ok_or("bound progress data path has no parent")?;
        if index_path.parent() != Some(parent) {
            return Err("bound progress main files do not share one parent");
        }
        let mut directories = self.directories.iter().rev();
        let root = directories
            .next()
            .ok_or("bound progress directory chain is empty")?;
        if root.entry_name.is_some() {
            return Err("bound progress root unexpectedly has a relative name");
        }
        let mut reconstructed = root.expected_path.clone();
        let mut components = Vec::with_capacity(self.directories.len().saturating_sub(1));
        for directory in directories {
            let name = directory
                .entry_name
                .as_deref()
                .ok_or("bound progress child directory has no relative name")?;
            let mut path_components = Path::new(name).components();
            if !matches!(
                path_components.next(),
                Some(std::path::Component::Normal(component)) if component == name
            ) || path_components.next().is_some()
            {
                return Err("bound progress relative directory name is not canonical");
            }
            let name = name
                .to_str()
                .ok_or("bound progress relative directory name is not UTF-8")?;
            reconstructed.push(name);
            if reconstructed != directory.expected_path {
                return Err("bound progress directory chain is not contiguous");
            }
            components.push(name.to_owned());
        }
        if reconstructed != parent {
            return Err("bound progress directory chain ends at the wrong parent");
        }
        Ok(components)
    }
}

#[derive(Debug)]
struct BoundProgressSidecar {
    namespace: BoundProgressNamespace,
    data: std::fs::File,
    index: std::fs::File,
    data_metadata: StableSidecarMetadata,
    index_metadata: StableSidecarMetadata,
}

#[derive(Debug)]
enum BoundProgressPair {
    Absent(BoundProgressNamespace),
    Present(BoundProgressSidecar),
}

#[derive(Debug)]
struct BoundProgressPromotionError {
    published: bool,
    source: std::io::Error,
}

/// Stable classification for a failed bound progress-sidecar recovery pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundProgressRecoveryFailure {
    /// The on-disk protocol state remains structurally recoverable, but an I/O
    /// or durability operation did not complete.
    RetryableIo,
    /// The namespace or protocol state is hostile, malformed, or ambiguous.
    InvalidData,
}

impl BoundProgressRecoveryFailure {
    fn from_io(error: &std::io::Error) -> Self {
        match error.kind() {
            ErrorKind::InvalidData
            | ErrorKind::InvalidInput
            | ErrorKind::NotFound
            | ErrorKind::AlreadyExists
            | ErrorKind::PermissionDenied
            | ErrorKind::UnexpectedEof => Self::InvalidData,
            _ => Self::RetryableIo,
        }
    }

    fn from_kura(error: &Error) -> Self {
        match error {
            Error::IO(source, _) | Error::MkDir(source, _) => Self::from_io(source),
            _ => Self::InvalidData,
        }
    }
}

#[derive(Debug)]
struct BoundSidecarIndexSnapshot {
    layout: SidecarIndexLayout,
    entries: Vec<SidecarIndexEntry>,
    indexed_end: u64,
}

/// Durable undo/redo record for one ordinary progress-sidecar append.
///
/// The record is published before either main file is mutated. Its index byte
/// windows are bounded by the maximum permitted sparse append, so recovery is
/// independent of the total historical index size. Its structured parent
/// identity is relative to the authenticated Kura root: root relocation stays
/// valid, but same-basename sibling namespaces cannot exchange intents.
/// This is the first-release V1 layout; pre-release development markers that
/// omitted the relative identity intentionally fail closed instead of using a
/// legacy decoding fallback.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct BoundProgressAppendIntentV1 {
    version: u16,
    namespace_components: Vec<String>,
    data_file: String,
    index_file: String,
    height: u64,
    pair_was_present: bool,
    old_data_len: u64,
    new_data_len: u64,
    payload_hash: Hash,
    old_index_len: u64,
    new_index_len: u64,
    index_write_offset: u64,
    old_index_bytes: Vec<u8>,
    new_index_bytes: Vec<u8>,
    integrity_hash: Hash,
}

impl BoundProgressAppendIntentV1 {
    fn payload_digest(payload: &[u8]) -> Hash {
        Hash::new_from_chunks(&[BOUND_PROGRESS_APPEND_DIGEST_DOMAIN, payload])
    }

    fn payload_len(&self) -> Option<u64> {
        self.new_data_len.checked_sub(self.old_data_len)
    }

    fn computed_integrity_hash(&self) -> Option<Hash> {
        let mut canonical = self.clone();
        canonical.integrity_hash = Hash::prehashed([0; Hash::LENGTH]);
        norito::encode_canonical(&canonical).ok().map(|bytes| {
            Hash::new_from_chunks(&[BOUND_PROGRESS_APPEND_INTENT_DIGEST_DOMAIN, &bytes])
        })
    }

    fn seal(mut self) -> Self {
        self.integrity_hash = self
            .computed_integrity_hash()
            .expect("fixed progress append intent must encode");
        self
    }

    fn validate_for(
        &self,
        namespace: &BoundProgressNamespace,
        data_path: &Path,
        index_path: &Path,
    ) -> std::result::Result<(), &'static str> {
        if self.computed_integrity_hash() != Some(self.integrity_hash) {
            return Err("bound progress append intent integrity hash is invalid");
        }
        if self.version != BOUND_PROGRESS_APPEND_INTENT_VERSION {
            return Err("unsupported bound progress append intent version");
        }
        let expected_namespace = namespace.stable_relative_components(data_path, index_path)?;
        if self.namespace_components != expected_namespace {
            return Err("bound progress append intent names the wrong relative namespace");
        }
        if data_path.file_name().and_then(std::ffi::OsStr::to_str) != Some(self.data_file.as_str())
            || index_path.file_name().and_then(std::ffi::OsStr::to_str)
                != Some(self.index_file.as_str())
        {
            return Err("bound progress append intent names the wrong main pair");
        }
        if self.height == 0 || self.height == u64::MAX {
            return Err("bound progress append intent height is invalid");
        }
        let payload_len = self
            .payload_len()
            .ok_or("bound progress append intent data length regresses")?;
        if payload_len == 0 || payload_len > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err("bound progress append intent payload length is invalid");
        }
        if !self.pair_was_present
            && (self.old_data_len != 0
                || self.old_index_len != 0
                || !self.old_index_bytes.is_empty())
        {
            return Err("absent bound progress pair has a non-empty preimage");
        }
        if self.old_index_len % PIPELINE_INDEX_ENTRY_SIZE_U64 != 0
            || self.new_index_len % PIPELINE_INDEX_ENTRY_SIZE_U64 != 0
            || self.index_write_offset % PIPELINE_INDEX_ENTRY_SIZE_U64 != 0
        {
            return Err("bound progress append intent index lengths are misaligned");
        }
        let old_bytes_len = u64::try_from(self.old_index_bytes.len())
            .map_err(|_| "bound progress append old index window is too large")?;
        let new_bytes_len = u64::try_from(self.new_index_bytes.len())
            .map_err(|_| "bound progress append new index window is too large")?;
        let max_index_window = INDEXED_SIDECAR_BASE_HEADER_SIZE_U64
            + (MAX_INDEXED_SIDECAR_GAP_ENTRIES + 1) * PIPELINE_INDEX_ENTRY_SIZE_U64;
        if new_bytes_len == 0 || new_bytes_len > max_index_window {
            return Err("bound progress append new index window exceeds its hard limit");
        }

        if self.index_write_offset == self.old_index_len {
            if old_bytes_len != 0
                || self
                    .old_index_len
                    .checked_add(new_bytes_len)
                    .is_none_or(|end| end != self.new_index_len)
            {
                return Err("bound progress append suffix has inconsistent index lengths");
            }
        } else if old_bytes_len != PIPELINE_INDEX_ENTRY_SIZE_U64
            || new_bytes_len != PIPELINE_INDEX_ENTRY_SIZE_U64
            || self.new_index_len != self.old_index_len
            || self
                .index_write_offset
                .checked_add(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .is_none_or(|end| end > self.old_index_len)
        {
            return Err("bound progress append replacement has an invalid index window");
        }
        Ok(())
    }

    fn validate_against_old_layout(
        &self,
        old_layout: SidecarIndexLayout,
    ) -> std::result::Result<(), &'static str> {
        if old_layout.aligned_len != self.old_index_len {
            return Err("bound progress append intent names the wrong old index layout");
        }
        let payload_len = self
            .payload_len()
            .ok_or("bound progress append intent data length regresses")?;
        let expected_entry = SidecarIndexEntry {
            offset: self.old_data_len,
            len: payload_len,
        };
        let Some(encoded_entry) = self.new_index_bytes.get(
            self.new_index_bytes
                .len()
                .saturating_sub(PIPELINE_INDEX_ENTRY_SIZE)..,
        ) else {
            return Err("bound progress append intent has no target index entry");
        };
        let encoded_entry: [u8; PIPELINE_INDEX_ENTRY_SIZE] = encoded_entry
            .try_into()
            .map_err(|_| "bound progress append intent target entry has the wrong size")?;
        if SidecarIndexEntry::from_bytes(encoded_entry) != expected_entry {
            return Err("bound progress append intent target entry is inconsistent");
        }

        if self.index_write_offset != self.old_index_len {
            if old_layout.entry_position(self.height) != Some(self.index_write_offset) {
                return Err("bound progress append replacement names the wrong height");
            }
            return Ok(());
        }

        let prefix_len = self
            .new_index_bytes
            .len()
            .checked_sub(PIPELINE_INDEX_ENTRY_SIZE)
            .ok_or("bound progress append suffix is truncated")?;
        let prefix = &self.new_index_bytes[..prefix_len];
        if self.old_index_len != 0 {
            let expected_height = old_layout
                .next_height()
                .ok_or("bound progress append old index height overflows")?;
            let missing = self
                .height
                .checked_sub(expected_height)
                .ok_or("bound progress append target precedes the old index")?;
            if missing > MAX_INDEXED_SIDECAR_GAP_ENTRIES
                || missing
                    .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    != Some(prefix_len)
                || prefix.iter().any(|byte| *byte != 0)
            {
                return Err("bound progress append gap is not canonical");
            }
            return Ok(());
        }

        if self.new_index_bytes.len() % PIPELINE_INDEX_ENTRY_SIZE != 0 {
            return Err("bound progress initial index window is misaligned");
        }
        let first = self
            .new_index_bytes
            .get(..PIPELINE_INDEX_ENTRY_SIZE)
            .ok_or("bound progress initial index window is truncated")?;
        let first = SidecarIndexEntry::from_bytes(
            first
                .try_into()
                .map_err(|_| "bound progress initial index marker has the wrong size")?,
        );
        let marker_field_present = first.offset == u64::MAX || first.len == u64::MAX;
        let (base_height, entries_prefix) = if marker_field_present {
            if first.offset != u64::MAX
                || first.len != u64::MAX
                || self.new_index_bytes.len()
                    < INDEXED_SIDECAR_BASE_HEADER_SIZE + PIPELINE_INDEX_ENTRY_SIZE
            {
                return Err("bound progress initial based index header is malformed");
            }
            let metadata = SidecarIndexEntry::from_bytes(
                self.new_index_bytes[PIPELINE_INDEX_ENTRY_SIZE..INDEXED_SIDECAR_BASE_HEADER_SIZE]
                    .try_into()
                    .map_err(|_| "bound progress initial based metadata has the wrong size")?,
            );
            if metadata.offset <= SidecarIndexLayout::LEGACY_BASE_HEIGHT
                || metadata.len != metadata.offset ^ INDEXED_SIDECAR_BASE_CHECK_MASK
            {
                return Err("bound progress initial based index metadata is invalid");
            }
            (
                metadata.offset,
                &self.new_index_bytes[INDEXED_SIDECAR_BASE_HEADER_SIZE..prefix_len],
            )
        } else {
            (SidecarIndexLayout::LEGACY_BASE_HEIGHT, prefix)
        };
        if entries_prefix.iter().any(|byte| *byte != 0) {
            return Err("bound progress initial index gap is not canonical");
        }
        let entries_offset = if marker_field_present {
            INDEXED_SIDECAR_BASE_HEADER_SIZE
        } else {
            0
        };
        let entry_count = self
            .new_index_bytes
            .len()
            .checked_sub(entries_offset)
            .and_then(|bytes| bytes.checked_div(PIPELINE_INDEX_ENTRY_SIZE))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or("bound progress initial index entry count overflows")?;
        if marker_field_present {
            if base_height != self.height || entry_count != 1 {
                return Err("bound progress initial based index is not canonical");
            }
        } else if entry_count.saturating_sub(1) > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
            return Err("bound progress initial legacy index gap exceeds its hard limit");
        }
        if base_height.checked_add(entry_count.saturating_sub(1)) != Some(self.height) {
            return Err("bound progress initial index target height is inconsistent");
        }
        Ok(())
    }
}

impl BoundProgressPair {
    fn sidecar(&self) -> Option<&BoundProgressSidecar> {
        match self {
            Self::Absent(_) => None,
            Self::Present(sidecar) => Some(sidecar),
        }
    }

    fn sidecar_mut(&mut self) -> Option<&mut BoundProgressSidecar> {
        match self {
            Self::Absent(_) => None,
            Self::Present(sidecar) => Some(sidecar),
        }
    }
}

/// One canonical outbound SCCP payload retained in commitment-index order.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct KuraRetainedSccpMessage {
    /// Zero-based leaf position in the block header's SCCP commitment tree.
    commitment_index: u32,
    /// Exact governed lane and destination/route binding context.
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Exact canonical SCCP V1 payload bytes.
    payload_bytes: Vec<u8>,
}

/// Immutable Kura-local block evidence retained before body eviction or finality publication.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct KuraRetainedBlockRecord {
    /// Kura-local envelope version.
    format_version: u16,
    /// Exact canonical height also encoded in the file name and header.
    height: u64,
    /// Canonical hash stored in Kura's durable hash journal.
    block_hash: HashOf<BlockHeader>,
    /// Exact canonical header needed by later finality association.
    block_header: BlockHeader,
    /// Hash of the canonical resultless proposal wire authenticated by the subject.
    proposal_wire_hash: Hash,
    /// Hash of the complete result-bearing canonical `SignedBlock::encode_wire()` bytes.
    executed_block_wire_hash: Hash,
    /// Successful outbound SCCP messages in exact commitment-index order.
    sccp_archive: Vec<KuraRetainedSccpMessage>,
}

impl KuraRetainedBlockRecord {
    fn new(
        block_header: BlockHeader,
        proposal_wire_hash: Hash,
        executed_block_wire_hash: Hash,
        sccp_archive: Vec<KuraRetainedSccpMessage>,
    ) -> Self {
        Self {
            format_version: RETAINED_BLOCK_RECORD_VERSION,
            height: block_header.height().get(),
            block_hash: block_header.hash(),
            block_header,
            proposal_wire_hash,
            executed_block_wire_hash,
            sccp_archive,
        }
    }
}

/// Fixed-size inventory entry for one nonempty retained SCCP archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RetainedSccpArchiveSummary {
    /// Canonical block height containing the outbound messages.
    pub(crate) height: u64,
    /// Exact canonical block hash that authenticates the retained archive root.
    pub(crate) block_hash: HashOf<BlockHeader>,
    /// Number of dense commitment positions in the retained archive.
    pub(crate) message_count: u32,
}

/// Raw and independently scanned Kura disk-usage state exposed only to crate tests.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DiskUsageAccountingSnapshotForTesting {
    /// Whether the enforced-usage cache is currently valid.
    pub(crate) enforced_initialized: bool,
    /// Whether the total-usage cache is currently valid.
    pub(crate) total_initialized: bool,
    /// Raw cached enforced bytes without triggering a refresh.
    pub(crate) cached_enforced_bytes: u64,
    /// Raw cached total bytes without triggering a refresh.
    pub(crate) cached_total_bytes: u64,
    /// Exact enforced bytes from a read-only filesystem scan.
    pub(crate) exact_enforced_bytes: u64,
    /// Exact total bytes from a read-only filesystem scan.
    pub(crate) exact_total_bytes: u64,
}

#[derive(Debug)]
struct StagedRetainedBlockRewriteEntry {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    bytes_hash: Hash,
    bytes_len: u64,
}

#[derive(Debug)]
struct StagedRetainedBlockRewrite {
    blocks_dir: PathBuf,
    entries: Vec<StagedRetainedBlockRewriteEntry>,
    removed_total_bytes: u64,
}

enum RetainedBlockRewritePublication<T> {
    Complete(T),
    CommittedWithDeferredCleanup { cleanup_error: Error },
}

impl<T> RetainedBlockRewritePublication<T> {
    fn into_result(self, kura: &Kura) -> Result<T> {
        match self {
            Self::Complete(output) => Ok(output),
            Self::CommittedWithDeferredCleanup { cleanup_error } => {
                error!(
                    ?cleanup_error,
                    "canonical rewrite committed with retained-record cleanup deferred"
                );
                let error = Error::CanonicalBlockCommittedRecoveryRequired {
                    detail: format!(
                        "retained-block rewrite cleanup is not recoverable in-process: {cleanup_error}"
                    ),
                };
                // The durable rewrite won, but the in-memory canonical image has not yet been
                // published by the caller. Allowing another mutation in this process could apply
                // it against the stale image, so this is a recovery gate rather than a warning.
                kura.poison_canonical_storage("retained-block rewrite cleanup", &error);
                Err(error)
            }
        }
    }
}

#[derive(Debug, Default)]
struct TotalDiskUsageAccountingState {
    generation: u64,
    mutations_in_flight: usize,
}

/// In-flight filesystem mutation registered with Kura's total-usage seqlock.
#[must_use]
pub(crate) struct TotalDiskUsageMutation<'a> {
    kura: &'a Kura,
    published: bool,
}

impl TotalDiskUsageMutation<'_> {
    /// Mark the mutation's cache delta as completely published.
    pub(crate) fn finish(mut self) {
        self.published = true;
    }
}

impl Drop for TotalDiskUsageMutation<'_> {
    fn drop(&mut self) {
        self.kura.finish_total_disk_usage_mutation(self.published);
    }
}

/// Private durable finality envelope paired by height with a retained block record.
///
/// The companion retained record stores independent hashes of the canonical
/// resultless proposal and the exact result-bearing executed block. Readers
/// require the subject and execution commitment to match those respective
/// hashes in addition to this envelope's canonical-header association.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct KuraV2FinalityRecord {
    /// Kura-local envelope version.
    format_version: u16,
    /// Exact canonical header whose hash is certified by `artifact`.
    block_header: BlockHeader,
    /// Self-contained consensus finality evidence.
    artifact: V2FinalityArtifact,
}

impl KuraV2FinalityRecord {
    fn new(block_header: BlockHeader, artifact: V2FinalityArtifact) -> Self {
        Self {
            format_version: KURA_V2_FINALITY_RECORD_VERSION,
            block_header,
            artifact,
        }
    }
}

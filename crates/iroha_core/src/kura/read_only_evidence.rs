// Included at Kura module scope. This owner never constructs or repairs runtime Kura.

/// Failure of bounded immutable Kura evidence admission or consumption.
#[derive(Debug)]
pub enum CanonicalKuraEvidenceError {
    /// A structural, ordering, identity or resource contract failed.
    Invalid(&'static str),
    /// A read-only operating-system operation failed.
    Io(std::io::Error),
    /// The platform lacks this owner's descriptor-relative no-follow implementation.
    UnsupportedPlatform,
}
impl std::fmt::Display for CanonicalKuraEvidenceError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(code) => write!(output, "canonical Kura evidence: {code}"),
            Self::Io(_) => output.write_str("canonical Kura evidence: read-only I/O failure"),
            Self::UnsupportedPlatform => {
                output.write_str("canonical Kura evidence: unsupported secure reader platform")
            }
        }
    }
}
impl std::error::Error for CanonicalKuraEvidenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
impl From<std::io::Error> for CanonicalKuraEvidenceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
/// Result of a read-only tooling operation; it conveys no consensus finality authority.
pub type CanonicalKuraEvidenceResult<T> = std::result::Result<T, CanonicalKuraEvidenceError>;

/// Independently supplied finite bounds for one immutable evidence session.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalKuraEvidenceLimits {
    /// First one-based requested carrier height, inclusive.
    pub first_height: u64,
    /// Last requested height, inclusive; every height in the interval must be read.
    pub last_height: u64,
    /// Maximum complete journal height admitted during the initial index scan.
    pub max_committed_blocks: u64,
    /// Maximum underlying blocks.data length, including unrequested committed heights.
    pub max_store_data_bytes: u64,
    /// Maximum canonical SignedBlockWire bytes for one requested carrier (at most 32 MiB).
    pub max_carrier_bytes: usize,
    /// Maximum complete merge-log bytes, including its length prefixes (at most 256 MiB).
    pub max_merge_log_bytes: u64,
    /// Maximum frames in the entire merge log, including unrequested epochs.
    pub max_merge_frames: u64,
    /// Cumulative returned carrier and requested canonical merge-entry bytes (at most 256 MiB).
    pub max_output_bytes: u64,
    /// Maximum cumulative owned allocation per decoder invocation (at most 512 MiB).
    pub max_decode_allocation_bytes: usize,
    /// Independently expected Unix uid of both supplied immediate directories and all five files.
    pub owner_uid: u32,
}
impl CanonicalKuraEvidenceLimits {
    fn validate(self) -> CanonicalKuraEvidenceResult<()> {
        evidence_require(
            self.first_height > 0
                && self.first_height <= self.last_height
                && self.last_height <= self.max_committed_blocks
                && self.max_committed_blocks <= 1_000_000,
            "height bounds",
        )?;
        evidence_require(
            self.max_store_data_bytes > 0
                && self.max_store_data_bytes <= 2 * 1024 * 1024 * 1024
                && self.max_carrier_bytes > 0
                && self.max_carrier_bytes <= 32 * 1024 * 1024
                && self.max_merge_log_bytes <= 256 * 1024 * 1024
                && self.max_merge_frames <= self.max_committed_blocks
                && self.max_output_bytes > 0
                && self.max_output_bytes <= 256 * 1024 * 1024
                && self.max_decode_allocation_bytes > 0
                && self.max_decode_allocation_bytes <= 512 * 1024 * 1024,
            "byte or frame bounds",
        )
    }
    fn decode_limits(self, size: usize) -> norito::DecodeLimits {
        let canonical = norito::canonical_decode_limits(size);
        norito::DecodeLimits::new(
            canonical.max_sequence_elements(),
            canonical.max_field_bytes(),
            canonical.max_total_elements(),
            canonical
                .max_total_allocated_bytes()
                .min(self.max_decode_allocation_bytes),
            64,
        )
    }
}
fn evidence_require(condition: bool, reason: &'static str) -> CanonicalKuraEvidenceResult<()> {
    if condition {
        Ok(())
    } else {
        Err(CanonicalKuraEvidenceError::Invalid(reason))
    }
}

/// One exact full-entry reference from an independently authenticated carrier.
#[derive(Debug)]
pub struct CanonicalKuraMergeRequest {
    /// One-based height; requests must be unique and strictly increasing.
    pub carrier_height: u64,
    /// The full compact reference, including its canonical hash/length and merge QC.
    pub reference: iroha_data_model::block::CertifiedMergeLedgerReference,
}

/// Consuming completion of disk admission, complete reads, full scan and final identity checks.
///
/// This non-serializable token is not an anchored finality or useful-effect proof.
/// The downstream authenticator must independently complete before publishing evidence.
pub struct CanonicalKuraEvidenceComplete {
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    sources: canonical_evidence_read_only_fs::Sources,
    committed_height: u64,
    carrier_count: u64,
    merge_frames: u64,
    output_bytes: u64,
}
impl std::fmt::Debug for CanonicalKuraEvidenceComplete {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalKuraEvidenceComplete")
            .field("committed_height", &self.committed_height)
            .field("carrier_count", &self.carrier_count)
            .field("merge_frames", &self.merge_frames)
            .field("output_bytes", &self.output_bytes)
            .finish()
    }
}
impl CanonicalKuraEvidenceComplete {
    /// Recheck the exact read-only source descriptors retained by successful finish.
    ///
    /// This grants no write handle or finality authority. A publication owner must
    /// retain this capability through its final durability check.
    pub fn recheck_sources(&self) -> CanonicalKuraEvidenceResult<()> {
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            self.sources.check()
        }
        #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
        {
            Err(CanonicalKuraEvidenceError::UnsupportedPlatform)
        }
    }
    /// Reject publication within any directory namespace actually read by Core.
    ///
    /// `ancestry` must come from the publication owner's held directory chain,
    /// ordered from root to immediate parent. Only bounded dev/inode identities
    /// cross this boundary; no raw descriptor or arbitrary callback escapes.
    pub fn ensure_publication_ancestry(
        &self,
        ancestry: &[(u64, u64)],
    ) -> CanonicalKuraEvidenceResult<()> {
        evidence_require(
            !ancestry.is_empty() && ancestry.len() <= 64,
            "publication ancestry bound",
        )?;
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            self.sources.ensure_publication_ancestry(ancestry)
        }
        #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
        {
            Err(CanonicalKuraEvidenceError::UnsupportedPlatform)
        }
    }
    /// Height of the exact published commit marker admitted at open.
    #[must_use]
    pub fn committed_height(&self) -> u64 {
        self.committed_height
    }
    /// Number of canonical carriers returned from the complete requested interval.
    #[must_use]
    pub fn carrier_count(&self) -> u64 {
        self.carrier_count
    }
    /// Number of complete, canonical epochs scanned from the entire merge log.
    #[must_use]
    pub fn merge_frames(&self) -> u64 {
        self.merge_frames
    }
    /// Sum of returned carrier and requested canonical merge-entry byte lengths.
    #[must_use]
    pub fn output_bytes(&self) -> u64 {
        self.output_bytes
    }
}

/// Immutable, bounded primary-store evidence reader with no mutation or recovery path.
///
/// Unix targets with the required descriptor APIs use retained no-follow handles from
/// the filesystem root. Redox, espidf and non-Unix targets fail closed. Only clean, nonempty, unpruned committed prefixes are
/// admitted: exact index/hash counts and contiguous data, with no uncommitted suffix.
/// The admitted journal images remain resident (at most 48 MB combined), bounded by
/// `max_committed_blocks`; body and merge decoding remain frame bounded.
/// Scan and carrier reads may occur in either order. Every failure, including a caught
/// consumer panic, poisons the session. Nothing is finally qualified before `finish`.
#[derive(Debug)]
pub struct CanonicalKuraEvidenceReader {
    limits: CanonicalKuraEvidenceLimits,
    marker: BlockStoreCommitMarker,
    marker_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    hash_bytes: Vec<u8>,
    next_height: u64,
    output_bytes: u64,
    merge_frames: u64,
    scanned: bool,
    poisoned: bool,
    carrier_references: BTreeMap<u64, Hash>,
    requested_references: BTreeMap<u64, Hash>,
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    sources: canonical_evidence_read_only_fs::Sources,
}
impl CanonicalKuraEvidenceReader {
    /// Admit explicit primary block and merge-log paths without discovery, creation or repair.
    ///
    /// Paths must be absolute, already normalized, bounded and free of symlink components.
    /// Every requested carrier must lie within the exact published commit marker.
    /// # Errors
    /// Returns invalid-input, immutable-prefix, secure-I/O or unsupported-platform errors.
    pub fn open(
        block_store: &Path,
        merge_log: &Path,
        limits: CanonicalKuraEvidenceLimits,
    ) -> CanonicalKuraEvidenceResult<Self> {
        Self::open_after_admission(block_store, merge_log, limits, |_| {})
    }
    fn open_after_admission(
        block_store: &Path,
        merge_log: &Path,
        limits: CanonicalKuraEvidenceLimits,
        after_admission: impl FnMut(&Path),
    ) -> CanonicalKuraEvidenceResult<Self> {
        limits.validate()?;
        #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
        {
            let _ = (block_store, merge_log, after_admission);
            // TODO: add retained descriptor APIs on Redox/espidf and Windows relative non-reparse handles; never reopen paths as a fallback.
            return Err(CanonicalKuraEvidenceError::UnsupportedPlatform);
        }
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            let sources = canonical_evidence_read_only_fs::Sources::open(
                block_store,
                merge_log,
                limits,
                after_admission,
            )?;
            let marker_bytes = sources.marker.read(0, sources.marker.len())?;
            let marker: BlockStoreCommitMarker = norito::decode_canonical_with_limits(
                &marker_bytes,
                limits.decode_limits(marker_bytes.len()),
            )
            .map_err(|_| CanonicalKuraEvidenceError::Invalid("canonical published marker"))?;
            evidence_require(
                marker.version == BlockStoreCommitMarker::VERSION
                    && marker.count > 0
                    && marker.tip_hash.is_some()
                    && marker.count <= limits.max_committed_blocks
                    && limits.last_height <= marker.count,
                "published marker boundary",
            )?;
            evidence_require(
                sources.index.len()
                    == marker
                        .count
                        .checked_mul(BlockIndex::SIZE)
                        .ok_or(CanonicalKuraEvidenceError::Invalid("index count overflow"))?
                    && sources.hashes.len()
                        == marker
                            .count
                            .checked_mul(SIZE_OF_BLOCK_HASH)
                            .ok_or(CanonicalKuraEvidenceError::Invalid("hash count overflow"))?,
                "exact committed journal lengths",
            )?;
            let index_bytes = sources.index.read(0, sources.index.len())?;
            let hash_bytes = sources.hashes.read(0, sources.hashes.len())?;
            let mut reader = Self {
                limits,
                marker,
                marker_bytes,
                index_bytes,
                hash_bytes,
                next_height: limits.first_height,
                output_bytes: 0,
                merge_frames: 0,
                scanned: false,
                poisoned: true,
                carrier_references: BTreeMap::new(),
                requested_references: BTreeMap::new(),
                sources,
            };
            let mut cursor = 0_u64;
            for height in 1..=reader.marker.count {
                let index = reader.index(height)?;
                evidence_require(
                    index.start == cursor && index.length > 0 && !index.is_evicted(),
                    "clean contiguous data prefix",
                )?;
                cursor = cursor
                    .checked_add(index.length)
                    .ok_or(CanonicalKuraEvidenceError::Invalid("data offset overflow"))?;
                evidence_require(cursor <= reader.sources.data.len(), "data index range")?;
                let hash = reader.hash(height)?;
                if height == reader.marker.count {
                    evidence_require(Some(hash) == reader.marker.tip_hash, "published marker tip")?;
                }
            }
            evidence_require(
                cursor == reader.sources.data.len(),
                "uncommitted data suffix",
            )?;
            reader.check_sources()?;
            reader.poisoned = false;
            Ok(reader)
        }
    }
    fn begin(&mut self) -> CanonicalKuraEvidenceResult<()> {
        evidence_require(!self.poisoned, "poisoned reader")?;
        // Set before fallible work or external callbacks; unwinding leaves this true.
        self.poisoned = true;
        self.check_sources()
    }
    fn add_output(&mut self, size: usize) -> CanonicalKuraEvidenceResult<()> {
        let next = self
            .output_bytes
            .checked_add(
                u64::try_from(size)
                    .map_err(|_| CanonicalKuraEvidenceError::Invalid("output size overflow"))?,
            )
            .ok_or(CanonicalKuraEvidenceError::Invalid("output sum overflow"))?;
        evidence_require(
            next <= self.limits.max_output_bytes,
            "cumulative output bound",
        )?;
        self.output_bytes = next;
        Ok(())
    }
    fn reference_digest(
        reference: &iroha_data_model::block::CertifiedMergeLedgerReference,
    ) -> CanonicalKuraEvidenceResult<Hash> {
        let size = norito::canonical_frame_len(reference)
            .map_err(|_| CanonicalKuraEvidenceError::Invalid("reference encoding"))?;
        evidence_require(
            size <= MAX_MERGE_LEDGER_ENTRY_BYTES,
            "reference frame bound",
        )?;
        let bytes = norito::encode_canonical(reference)
            .map_err(|_| CanonicalKuraEvidenceError::Invalid("reference encoding"))?;
        evidence_require(bytes.len() == size, "reference encoding changed")?;
        Ok(Hash::new(bytes))
    }
    /// Read exactly the next requested canonical carrier and bind its stored header association.
    ///
    /// Returned wire bytes still require the independently anchored finality verifier.
    /// # Errors
    /// Fails and poisons on skipped/repeated heights, bounds, codec or file identity errors.
    pub fn read_carrier(&mut self, height: u64) -> CanonicalKuraEvidenceResult<Vec<u8>> {
        self.begin()?;
        #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
        {
            let _ = height;
            Err(CanonicalKuraEvidenceError::UnsupportedPlatform)
        }
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            evidence_require(
                height == self.next_height && height <= self.limits.last_height,
                "exact increasing carrier interval",
            )?;
            let index = self.index(height)?;
            evidence_require(
                index.length <= self.limits.max_carrier_bytes as u64,
                "carrier frame bound",
            )?;
            let wire = self.sources.data.read(index.start, index.length)?;
            let block =
                norito::with_decode_limits_scope(self.limits.decode_limits(wire.len()), || {
                    iroha_data_model::block::decode_versioned_signed_block(&wire)
                })
                .map_err(|_| CanonicalKuraEvidenceError::Invalid("carrier decode"))?;
            evidence_require(
                norito::canonical_frame_len(&block)
                    .map_err(|_| CanonicalKuraEvidenceError::Invalid("carrier encoding"))?
                    <= self.limits.max_carrier_bytes,
                "carrier canonical bound",
            )?;
            let canonical = block
                .encode_wire()
                .map_err(|_| CanonicalKuraEvidenceError::Invalid("carrier wire encoding"))?;
            evidence_require(
                canonical == wire
                    && block.header().height().get() == height
                    && block.hash() == self.hash(height)?,
                "canonical carrier association",
            )?;
            let previous = if height == 1 {
                None
            } else {
                Some(self.hash(height - 1)?)
            };
            evidence_require(
                block.header().prev_block_hash() == previous,
                "carrier parent journal association",
            )?;
            if let Some(reference) = block
                .execution_context()
                .and_then(|context| context.merge_entry.as_ref())
            {
                self.carrier_references
                    .insert(height, Self::reference_digest(reference)?);
            }
            self.add_output(wire.len())?;
            self.check_sources()?;
            self.next_height = height
                .checked_add(1)
                .ok_or(CanonicalKuraEvidenceError::Invalid(
                    "height successor overflow",
                ))?;
            self.poisoned = false;
            Ok(wire)
        }
    }
    /// Scan the entire admitted merge log once, yielding only exact requested full entries.
    ///
    /// Requests must cover every log carrier in the requested interval. The callback receives
    /// canonical framed entry bytes, not the distinct headerless stored codec frame. Callback
    /// effects remain provisional until this reader and the anchored consumer both finish.
    /// Even zero requests require this complete scan; a trailing fragment is never repaired.
    /// # Errors
    /// Fails and poisons on order, missing/extra references, frame/codec/budget errors or callback failure.
    pub fn scan_merge_entries(
        &mut self,
        requests: &[CanonicalKuraMergeRequest],
        mut consume: impl FnMut(u64, &MergeLedgerEntry, &[u8]) -> CanonicalKuraEvidenceResult<()>,
    ) -> CanonicalKuraEvidenceResult<()> {
        self.begin()?;
        #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
        {
            let _ = (requests, consume);
            Err(CanonicalKuraEvidenceError::UnsupportedPlatform)
        }
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            evidence_require(
                !self.scanned
                    && requests.len() as u64 <= self.limits.max_merge_frames
                    && requests.len() as u64
                        <= self.limits.last_height - self.limits.first_height + 1,
                "request count or repeated scan",
            )?;
            let mut previous_height = 0;
            let mut previous_epoch = 0;
            let mut reference_bytes = 0_u64;
            for request in requests {
                let height = request.carrier_height;
                evidence_require(
                    height >= self.limits.first_height
                        && height <= self.limits.last_height
                        && height > previous_height
                        && request.reference.epoch_id > previous_epoch
                        && request.reference.merge_qc.carrier_height == height,
                    "ordered exact merge requests",
                )?;
                let size = norito::canonical_frame_len(&request.reference)
                    .map_err(|_| CanonicalKuraEvidenceError::Invalid("reference encoding"))?;
                reference_bytes = reference_bytes.checked_add(size as u64).ok_or(
                    CanonicalKuraEvidenceError::Invalid("reference size overflow"),
                )?;
                evidence_require(
                    reference_bytes <= self.limits.max_output_bytes,
                    "request allocation bound",
                )?;
                self.requested_references
                    .insert(height, Self::reference_digest(&request.reference)?);
                previous_height = height;
                previous_epoch = request.reference.epoch_id;
            }
            let mut offset = 0_u64;
            let mut found = 0_usize;
            let mut last_carrier = 0_u64;
            while offset < self.sources.merge.len() {
                evidence_require(
                    self.merge_frames < self.limits.max_merge_frames,
                    "merge frame count",
                )?;
                let length_bytes = self.sources.merge.read(offset, 4)?;
                let length = u32::from_le_bytes(
                    length_bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| CanonicalKuraEvidenceError::Invalid("merge length prefix"))?,
                ) as usize;
                evidence_require(
                    length > 0 && length <= MAX_MERGE_LEDGER_ENTRY_BYTES,
                    "merge frame byte bound",
                )?;
                let payload = offset
                    .checked_add(4)
                    .ok_or(CanonicalKuraEvidenceError::Invalid("merge offset overflow"))?;
                let bytes = self.sources.merge.read(payload, length as u64)?;
                let entry = norito::with_decode_limits(self.limits.decode_limits(length), || {
                    MergeLedgerEntry::decode_all(&mut bytes.as_slice())
                })
                .map_err(|_| CanonicalKuraEvidenceError::Invalid("merge exact decode"))?;
                let canonical_size = norito::canonical_frame_len(&entry)
                    .map_err(|_| CanonicalKuraEvidenceError::Invalid("merge canonical length"))?;
                evidence_require(
                    entry.has_current_version() && canonical_size <= MAX_MERGE_LEDGER_ENTRY_BYTES,
                    "merge version or canonical frame bound",
                )?;
                evidence_require(entry.encode() == bytes, "merge codec canonicality")?;
                evidence_require(
                    entry.epoch_id == self.merge_frames + 1
                        && entry.merge_qc.carrier_height > last_carrier
                        && entry.merge_qc.carrier_height <= self.marker.count,
                    "contiguous merge epochs and carriers",
                )?;
                let carrier = entry.merge_qc.carrier_height;
                if carrier >= self.limits.first_height && carrier <= self.limits.last_height {
                    let request = requests
                        .get(found)
                        .ok_or(CanonicalKuraEvidenceError::Invalid("missing merge request"))?;
                    evidence_require(
                        request.carrier_height == carrier
                            && request.reference.matches_entry(&entry),
                        "merge request identity",
                    )?;
                    let canonical = norito::encode_canonical(&entry).map_err(|_| {
                        CanonicalKuraEvidenceError::Invalid("merge canonical encoding")
                    })?;
                    evidence_require(
                        canonical.len() == canonical_size,
                        "merge canonical size changed",
                    )?;
                    self.add_output(canonical.len())?;
                    consume(carrier, &entry, &canonical)?;
                    self.check_sources()?;
                    found += 1;
                }
                offset = payload
                    .checked_add(length as u64)
                    .ok_or(CanonicalKuraEvidenceError::Invalid("merge end overflow"))?;
                self.merge_frames += 1;
                last_carrier = carrier;
            }
            evidence_require(found == requests.len(), "unfulfilled merge requests")?;
            self.check_sources()?;
            self.scanned = true;
            self.poisoned = false;
            Ok(())
        }
    }
    /// Consume this owner only after the complete carrier interval and complete merge scan.
    /// # Errors
    /// Fails on a missing phase, caught earlier error/panic, reference mismatch, or final source drift.
    pub fn finish(mut self) -> CanonicalKuraEvidenceResult<CanonicalKuraEvidenceComplete> {
        self.begin()?;
        evidence_require(
            self.next_height == self.limits.last_height + 1 && self.scanned,
            "incomplete evidence phases",
        )?;
        evidence_require(
            self.carrier_references == self.requested_references,
            "carrier and requested merge references differ",
        )?;
        self.check_sources()?;
        Ok(CanonicalKuraEvidenceComplete {
            #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
            sources: self.sources,
            committed_height: self.marker.count,
            carrier_count: self.limits.last_height - self.limits.first_height + 1,
            merge_frames: self.merge_frames,
            output_bytes: self.output_bytes,
        })
    }
    fn check_sources(&self) -> CanonicalKuraEvidenceResult<()> {
        #[cfg(not(all(unix, not(any(target_os = "redox", target_os = "espidf")))))]
        {
            Err(CanonicalKuraEvidenceError::UnsupportedPlatform)
        }
        #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
        {
            self.sources.check()?;
            evidence_require(
                self.sources.marker.read(0, self.sources.marker.len())? == self.marker_bytes,
                "published marker changed",
            )
        }
    }
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    fn index(&self, height: u64) -> CanonicalKuraEvidenceResult<BlockIndex> {
        evidence_require(height > 0, "one-based index height")?;
        let offset = (height - 1)
            .checked_mul(BlockIndex::SIZE)
            .ok_or(CanonicalKuraEvidenceError::Invalid("index offset overflow"))?;
        let offset = usize::try_from(offset)
            .map_err(|_| CanonicalKuraEvidenceError::Invalid("index offset conversion"))?;
        let bytes = self
            .index_bytes
            .get(offset..offset + BlockIndex::SIZE as usize)
            .ok_or(CanonicalKuraEvidenceError::Invalid("index slice boundary"))?;
        evidence_require(bytes.len() == 16, "block index layout")?;
        Ok(BlockIndex {
            start: u64::from_le_bytes(
                bytes[..8]
                    .try_into()
                    .map_err(|_| CanonicalKuraEvidenceError::Invalid("index start"))?,
            ),
            length: u64::from_le_bytes(
                bytes[8..]
                    .try_into()
                    .map_err(|_| CanonicalKuraEvidenceError::Invalid("index length"))?,
            ),
        })
    }
    #[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
    fn hash(&self, height: u64) -> CanonicalKuraEvidenceResult<HashOf<BlockHeader>> {
        evidence_require(height > 0, "one-based hash height")?;
        let offset = (height - 1)
            .checked_mul(SIZE_OF_BLOCK_HASH)
            .ok_or(CanonicalKuraEvidenceError::Invalid("hash offset overflow"))?;
        let offset = usize::try_from(offset)
            .map_err(|_| CanonicalKuraEvidenceError::Invalid("hash offset conversion"))?;
        let bytes = self
            .hash_bytes
            .get(offset..offset + SIZE_OF_BLOCK_HASH as usize)
            .ok_or(CanonicalKuraEvidenceError::Invalid("hash slice boundary"))?;
        let bytes: [u8; Hash::LENGTH] = bytes
            .try_into()
            .map_err(|_| CanonicalKuraEvidenceError::Invalid("hash record size"))?;
        evidence_require(bytes[Hash::LENGTH - 1] & 1 == 1, "canonical hash marker")?;
        Ok(HashOf::from_untyped_unchecked(Hash::prehashed(bytes)))
    }
}

#[cfg(all(unix, not(any(target_os = "redox", target_os = "espidf"))))]
mod canonical_evidence_read_only_fs {
    use super::{
        CanonicalKuraEvidenceError as Error, CanonicalKuraEvidenceLimits as Limits,
        CanonicalKuraEvidenceResult as Result, evidence_require as require,
    };
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};
    use std::{
        ffi::OsString,
        fs::File,
        os::unix::{ffi::OsStrExt as _, fs::FileExt as _},
        path::{Component, Path, PathBuf},
    };
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Identity {
        dev: u64,
        ino: u64,
        mode: u32,
        uid: u32,
        gid: u32,
        links: u64,
        size: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    }
    impl Identity {
        fn new(value: Stat) -> Result<Self> {
            Ok(Self {
                dev: value.st_dev as u64,
                ino: value.st_ino as u64,
                mode: value.st_mode as u32,
                uid: value.st_uid,
                gid: value.st_gid,
                links: value.st_nlink as u64,
                size: u64::try_from(value.st_size)
                    .map_err(|_| Error::Invalid("negative file size"))?,
                modified: (value.st_mtime as i64, value.st_mtime_nsec as i64),
                changed: (value.st_ctime as i64, value.st_ctime_nsec as i64),
            })
        }
        fn same_owner(self, other: Self) -> bool {
            (self.dev, self.ino, self.mode, self.uid, self.gid)
                == (other.dev, other.ino, other.mode, other.uid, other.gid)
        }
        fn directory(self) -> bool {
            FileType::from_raw_mode(self.mode as rustix::fs::RawMode) == FileType::Directory
        }
        fn regular(self) -> bool {
            FileType::from_raw_mode(self.mode as rustix::fs::RawMode) == FileType::RegularFile
                && self.links == 1
        }
    }
    fn held(file: &File) -> Result<Identity> {
        Identity::new(rustix::fs::fstat(file).map_err(std::io::Error::from)?)
    }
    fn named(parent: &File, name: &std::ffi::OsStr) -> Result<Identity> {
        Identity::new(
            rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)?,
        )
    }
    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);
    const REGULAR: OFlags = OFlags::RDONLY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);
    #[derive(Debug)]
    struct Directory {
        file: File,
        name: Option<OsString>,
        identity: Identity,
    }
    #[derive(Debug)]
    pub(super) struct Source {
        chain: Vec<Directory>,
        file: File,
        name: OsString,
        identity: Identity,
    }
    impl Source {
        fn open(
            path: &Path,
            max_bytes: u64,
            uid: u32,
            hook: &mut impl FnMut(&Path),
        ) -> Result<Self> {
            require(
                path.is_absolute() && path.as_os_str().as_bytes().len() <= 4096,
                "absolute bounded source path",
            )?;
            let mut normalized = PathBuf::new();
            let mut parts = Vec::new();
            for component in path.components() {
                match component {
                    Component::RootDir => normalized.push(component.as_os_str()),
                    Component::Normal(name) => {
                        normalized.push(name);
                        parts.push(name.to_os_string());
                    }
                    _ => return Err(Error::Invalid("normalized source path")),
                }
            }
            require(
                normalized.as_os_str() == path.as_os_str()
                    && !parts.is_empty()
                    && parts.len() <= 64,
                "bounded source components",
            )?;
            let name = parts.pop().ok_or(Error::Invalid("source leaf"))?;
            let root = File::from(
                rustix::fs::open("/", DIRECTORY, Mode::empty()).map_err(std::io::Error::from)?,
            );
            let root_identity = held(&root)?;
            require(root_identity.directory(), "filesystem root directory")?;
            let mut chain = vec![Directory {
                file: root,
                name: None,
                identity: root_identity,
            }];
            for part in parts {
                let parent = &chain.last().ok_or(Error::Invalid("directory chain"))?.file;
                let before = named(parent, &part)?;
                require(before.directory(), "non-symlink directory required")?;
                let child = File::from(
                    rustix::fs::openat(parent, &part, DIRECTORY, Mode::empty())
                        .map_err(std::io::Error::from)?,
                );
                require(
                    held(&child)?.same_owner(before) && named(parent, &part)?.same_owner(before),
                    "directory changed at open",
                )?;
                chain.push(Directory {
                    file: child,
                    name: Some(part),
                    identity: before,
                });
            }
            let parent = &chain.last().ok_or(Error::Invalid("source parent"))?.file;
            require(held(parent)?.uid == uid, "source parent owner")?;
            let before = named(parent, &name)?;
            require(
                before.regular() && before.uid == uid && before.size <= max_bytes,
                "bounded owned single-link regular source",
            )?;
            hook(path);
            let file = File::from(
                rustix::fs::openat(parent, &name, REGULAR, Mode::empty())
                    .map_err(std::io::Error::from)?,
            );
            require(
                held(&file)? == before && named(parent, &name)? == before,
                "file changed at open",
            )?;
            let source = Self {
                chain,
                file,
                name,
                identity: before,
            };
            source.check()?;
            Ok(source)
        }
        pub(super) fn len(&self) -> u64 {
            self.identity.size
        }
        pub(super) fn check(&self) -> Result<()> {
            for (index, directory) in self.chain.iter().enumerate() {
                require(
                    held(&directory.file)?.same_owner(directory.identity),
                    "held directory changed",
                )?;
                if let Some(name) = &directory.name {
                    require(
                        index > 0
                            && named(&self.chain[index - 1].file, name)?
                                .same_owner(directory.identity),
                        "named directory changed",
                    )?;
                }
            }
            let parent = &self
                .chain
                .last()
                .ok_or(Error::Invalid("source parent"))?
                .file;
            require(
                held(&self.file)? == self.identity && named(parent, &self.name)? == self.identity,
                "held or named source changed",
            )
        }
        #[cfg(test)]
        pub(super) fn flags_for_test(&self) -> Result<(OFlags, rustix::io::FdFlags)> {
            Ok((
                rustix::fs::fcntl_getfl(&self.file).map_err(std::io::Error::from)?,
                rustix::io::fcntl_getfd(&self.file).map_err(std::io::Error::from)?,
            ))
        }
        pub(super) fn read(&self, offset: u64, length: u64) -> Result<Vec<u8>> {
            self.read_after_check(offset, length, || {})
        }
        pub(super) fn read_after_check(
            &self,
            offset: u64,
            length: u64,
            after_check: impl FnOnce(),
        ) -> Result<Vec<u8>> {
            self.check()?;
            require(
                offset
                    .checked_add(length)
                    .is_some_and(|end| end <= self.len())
                    && length <= 32 * 1024 * 1024,
                "bounded source read range",
            )?;
            let length =
                usize::try_from(length).map_err(|_| Error::Invalid("read size conversion"))?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| Error::Invalid("read allocation"))?;
            bytes.resize(length, 0);
            after_check();
            self.file.read_exact_at(&mut bytes, offset)?;
            self.check()?;
            Ok(bytes)
        }
    }
    #[derive(Debug)]
    pub(super) struct Sources {
        pub(super) data: Source,
        pub(super) index: Source,
        pub(super) hashes: Source,
        pub(super) marker: Source,
        pub(super) merge: Source,
    }
    impl Sources {
        pub(super) fn ensure_publication_ancestry(&self, ancestry: &[(u64, u64)]) -> Result<()> {
            self.check()?;
            for source in [
                &self.data,
                &self.index,
                &self.hashes,
                &self.marker,
                &self.merge,
            ] {
                let root = source
                    .chain
                    .last()
                    .ok_or(Error::Invalid("source parent chain"))?
                    .identity;
                require(
                    !ancestry.contains(&(root.dev, root.ino)),
                    "publication inside protected Core source namespace",
                )?;
            }
            self.check()
        }
        pub(super) fn open(
            store: &Path,
            merge: &Path,
            limits: Limits,
            mut hook: impl FnMut(&Path),
        ) -> Result<Self> {
            let data = Source::open(
                &store.join(super::DATA_FILE_NAME),
                limits.max_store_data_bytes,
                limits.owner_uid,
                &mut hook,
            )?;
            let index = Source::open(
                &store.join(super::INDEX_FILE_NAME),
                limits.max_committed_blocks * super::BlockIndex::SIZE,
                limits.owner_uid,
                &mut hook,
            )?;
            let hashes = Source::open(
                &store.join(super::HASHES_FILE_NAME),
                limits.max_committed_blocks * super::SIZE_OF_BLOCK_HASH,
                limits.owner_uid,
                &mut hook,
            )?;
            let marker = Source::open(
                &store.join(super::COUNT_FILE_NAME),
                super::MAX_BLOCK_COMMIT_MARKER_BYTES as u64,
                limits.owner_uid,
                &mut hook,
            )?;
            let merge = Source::open(
                merge,
                limits.max_merge_log_bytes,
                limits.owner_uid,
                &mut hook,
            )?;
            let sources = Self {
                data,
                index,
                hashes,
                marker,
                merge,
            };
            let mut objects = std::collections::BTreeSet::new();
            for source in [
                &sources.data,
                &sources.index,
                &sources.hashes,
                &sources.marker,
                &sources.merge,
            ] {
                require(
                    objects.insert((source.identity.dev, source.identity.ino)),
                    "aliased evidence sources",
                )?;
            }
            sources.check()?;
            Ok(sources)
        }
        pub(super) fn check(&self) -> Result<()> {
            for source in [
                &self.data,
                &self.index,
                &self.hashes,
                &self.marker,
                &self.merge,
            ] {
                source.check()?;
            }
            Ok(())
        }
    }
}

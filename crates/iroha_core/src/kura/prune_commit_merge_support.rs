const MERGE_CARRIERS_DIR: &str = "merge_carriers";
const MERGE_CARRIER_MAX_BYTES: usize = 4 * 1024;
const PRUNE_INTENT_FILE_NAME: &str = "prune_intent.norito";
const PRUNE_INTENT_TEMP_FILE_NAME: &str = "prune_intent.norito.tmp";
const PRUNE_INTENT_MAX_BYTES: usize = 4 * 1024;
const PRUNE_STAGE_INTENT: usize = 1;
const PRUNE_STAGE_BLOCK_MARKER: usize = 2;
const PRUNE_STAGE_BLOCK_INDEX: usize = 3;
const PRUNE_STAGE_BLOCK_HASHES: usize = 4;
const PRUNE_STAGE_BLOCK_DATA: usize = 5;
const PRUNE_STAGE_DA_SIDECARS: usize = 6;
const PRUNE_STAGE_MERGE_CARRIERS: usize = 7;
const PRUNE_STAGE_MERGE_LOG: usize = 8;
const PRUNE_STAGE_WSV_CHECKPOINTS: usize = 9;
const PRUNE_STAGE_COMMIT_MANIFESTS: usize = 10;
const PRUNE_STAGE_PIPELINE_SIDECARS: usize = 11;
const PRUNE_STAGE_MEMORY: usize = 12;
const PRUNE_SIDECAR_PROMOTION_DATA: usize = 1;
const PRUNE_SIDECAR_PROMOTION_INDEX: usize = 2;
#[derive(Clone, Copy)]
enum NativeAmxMergeAssociation<'a> {
    Live(Option<&'a MergeLedgerEntry>),
    Startup(Option<&'a MergeLedgerEntry>),
    CommittedOnly,
}
/// Clears the in-process prune gate on every return and unwind path.
#[derive(Debug)]
struct PruneInProgressGuard<'a> {
    flag: &'a AtomicBool,
}
impl<'a> PruneInProgressGuard<'a> {
    fn begin(flag: &'a AtomicBool) -> Self {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .expect("canonical prune gate must be clear while prune_lock is held");
        Self { flag }
    }
}
impl Drop for PruneInProgressGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}
type BlockData = Vec<(HashOf<BlockHeader>, Option<Arc<SignedBlock>>)>;
type BlockHeightIndex = BTreeMap<HashOf<BlockHeader>, NonZeroUsize>;
type TransactionEntrypointHeights = BTreeMap<HashOf<TransactionEntrypoint>, BTreeSet<NonZeroUsize>>;
type OfflineOperationHeights = BTreeMap<(AccountId, [u8; 32]), BTreeSet<NonZeroUsize>>;
type TransactionAuthorityHeights = BTreeMap<AccountId, BTreeSet<NonZeroUsize>>;
type TransactionTimestampHeights = BTreeMap<u64, BTreeSet<NonZeroUsize>>;
type TransactionResultStatusHeights = BTreeMap<bool, BTreeSet<NonZeroUsize>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BlockReplicaKey {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    executed_block_wire_len: u64,
    executed_block_wire_hash: Hash,
}
type BlockReplicaRegistry = BTreeMap<BlockReplicaKey, BTreeMap<PeerId, BlockReplicaAdvert>>;
#[derive(Debug, Default)]
struct MergeCarrierIndex {
    initialized: bool,
    generation: u64,
    by_height: BTreeMap<u64, MergeLedgerCarrierRecord>,
    by_entry: BTreeMap<HashOf<MergeLedgerEntry>, MergeLedgerCarrierRecord>,
    #[cfg(test)]
    directory_scans: usize,
    #[cfg(test)]
    full_inventory_clones: usize,
}
/// Exact retained output for one indexed-sidecar rewrite in a canonical prune.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode)]
struct KuraPruneSidecarPairProjectionV3 {
    /// Whether this pair must be rewritten or removed.
    required: bool,
    /// Exact retained payload bytes written to the temporary data file.
    retained_data_bytes: u64,
    /// Exact retained index bytes written to the temporary index file.
    retained_index_bytes: u64,
}
impl KuraPruneSidecarPairProjectionV3 {
    fn temp_pair_bytes(self) -> Option<u64> {
        self.retained_data_bytes
            .checked_add(self.retained_index_bytes)
    }
}
/// Authenticated rewrite projection for the canonical pipeline sidecar pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode)]
struct KuraPruneSidecarRewriteProjectionV3 {
    /// Retained pipeline recovery data/index output.
    pipeline: KuraPruneSidecarPairProjectionV3,
    /// Temporary bytes allocated by the retained-pair rewrite.
    sequential_peak_bytes: u64,
}
impl KuraPruneSidecarRewriteProjectionV3 {
    #[cfg(test)]
    const fn none() -> Self {
        Self {
            pipeline: KuraPruneSidecarPairProjectionV3 {
                required: false,
                retained_data_bytes: 0,
                retained_index_bytes: 0,
            },
            sequential_peak_bytes: 0,
        }
    }
    fn has_work(self) -> bool {
        self.pipeline.required
    }
    fn is_canonical(self) -> bool {
        let Some(pipeline) = self.pipeline.temp_pair_bytes() else {
            return false;
        };
        (self.pipeline.required
            || (self.pipeline.retained_data_bytes == 0 && self.pipeline.retained_index_bytes == 0))
            && self.sequential_peak_bytes == pipeline
    }
    fn authorizes(self, remaining: Self) -> bool {
        self.is_canonical()
            && remaining.is_canonical()
            && (!remaining.pipeline.required || self.pipeline.required)
            && remaining.pipeline.retained_data_bytes <= self.pipeline.retained_data_bytes
            && remaining.pipeline.retained_index_bytes <= self.pipeline.retained_index_bytes
            && remaining.sequential_peak_bytes <= self.sequential_peak_bytes
    }
}
/// Exact live capacity admission retained as forward-recovery authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct KuraPruneCapacityAdmissionV3 {
    /// Physical Kura bytes before the intent is published.
    source_physical_bytes: u64,
    /// Pending canonical bytes excluded from the physical scan.
    pending_canonical_bytes: u64,
    /// Outstanding post-WSV carrier reservation bytes.
    post_wsv_reserved_bytes: u64,
    /// Outstanding certified frontier/bundle reservation bytes.
    certified_bundle_reserved_bytes: u64,
    /// Outstanding autonomous terminal-outcome reservation bytes.
    autonomous_terminal_reserved_bytes: u64,
    /// Exact encoded durable intent length.
    intent_bytes: u64,
    /// Exact block-marker temporary written before replacement.
    marker_temporary_bytes: u64,
    /// Positive stable marker growth retained by later stages.
    marker_stable_growth_bytes: u64,
    /// Absolute no-deletion-credit peak admitted before the first write.
    admitted_peak_bytes: u64,
}
impl KuraPruneCapacityAdmissionV3 {
    fn reserved_bytes(self) -> Option<u64> {
        self.pending_canonical_bytes
            .checked_add(self.post_wsv_reserved_bytes)
            .and_then(|bytes| bytes.checked_add(self.certified_bundle_reserved_bytes))
            .and_then(|bytes| bytes.checked_add(self.autonomous_terminal_reserved_bytes))
    }
    fn transaction_peak_bytes(self, sidecar: KuraPruneSidecarRewriteProjectionV3) -> Option<u64> {
        self.marker_stable_growth_bytes
            .checked_add(sidecar.sequential_peak_bytes)
            .map(|post_marker| self.marker_temporary_bytes.max(post_marker))
    }
    fn required_peak_bytes(self, sidecar: KuraPruneSidecarRewriteProjectionV3) -> Option<u64> {
        self.source_physical_bytes
            .checked_add(self.reserved_bytes()?)
            .and_then(|bytes| bytes.checked_add(self.intent_bytes))
            .and_then(|bytes| bytes.checked_add(self.transaction_peak_bytes(sidecar)?))
    }
    fn is_canonical(self, sidecar: KuraPruneSidecarRewriteProjectionV3) -> bool {
        self.intent_bytes > 0
            && self.intent_bytes <= PRUNE_INTENT_MAX_BYTES as u64
            && self.marker_temporary_bytes > 0
            && self.marker_temporary_bytes <= MAX_VERIFIED_SNAPSHOT_TAIL_MARKER_BYTES as u64
            && self.marker_stable_growth_bytes <= self.marker_temporary_bytes
            && self.required_peak_bytes(sidecar) == Some(self.admitted_peak_bytes)
    }
    fn remaining_required_bytes(
        self,
        physical_bytes: u64,
        remaining_sidecar: KuraPruneSidecarRewriteProjectionV3,
    ) -> Option<u64> {
        physical_bytes
            .checked_add(remaining_sidecar.sequential_peak_bytes)
            .and_then(|bytes| bytes.checked_add(self.reserved_bytes()?))
    }
}
/// Durable forward-recovery record for a canonical Kura prune transaction.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct KuraPruneIntentV3 {
    /// Intent schema version. Only version three is accepted.
    version: u8,
    /// Canonical height before the prune began.
    source_height: u64,
    /// Canonical source tip hash, absent only for an empty source.
    source_tip_hash: Option<HashOf<BlockHeader>>,
    /// Canonical height retained by the prune.
    target_height: u64,
    /// Canonical target tip hash, absent only when pruning to height zero.
    target_tip_hash: Option<HashOf<BlockHeader>>,
    /// Exact merge-log prefix length retained by the prune.
    retained_merge_entries: u64,
    /// Hash of the terminal retained merge entry, absent for an empty prefix.
    retained_merge_tip_hash: Option<HashOf<MergeLedgerEntry>>,
    /// Exact authenticated retained sidecar rewrite and allocation projection.
    sidecar_rewrite: KuraPruneSidecarRewriteProjectionV3,
    /// Exact capacity proof admitted before publication and reused after crash.
    capacity: KuraPruneCapacityAdmissionV3,
}
impl Kura {
    /// Detect prune recovery state without cleaning or applying it while the
    /// signed snapshot lineage is still provisional.
    fn read_prune_intent_for_startup(
        store_root: &Path,
        provisional: bool,
    ) -> Result<Option<KuraPruneIntentV3>> {
        if !provisional {
            return Self::read_prune_intent(store_root);
        }
        let inventory = Self::canonical_prune_intent_artifact_inventory(store_root)?;
        if inventory.stable.is_some() || inventory.temporary.is_some() {
            return Err(Error::InvalidSnapshotBootstrapMarker {
                path: Self::prune_intent_path_for(store_root),
                reason: "pending prune requires recovery before provisional snapshot startup"
                    .to_owned(),
            });
        }
        Ok(None)
    }
}
#[derive(Debug, Clone)]
struct QueuedFastpqProofSnapshot {
    snapshot: FastpqProofSnapshot,
    retries: usize,
}
/// Result of enqueueing pipeline recovery metadata for sidecar persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum PipelineSidecarEnqueueResult {
    /// The sidecar was accepted.
    Enqueued {
        /// Queue depth after the sidecar was accepted.
        queue_depth: usize,
    },
    /// The queue is already at the configured capacity.
    RejectedQueueFull {
        /// Configured queue capacity.
        cap: usize,
    },
    /// A canonical prune is active or prune recovery requires a process restart.
    RejectedPruneRecovery,
}
/// Result of enqueueing a FASTPQ proof snapshot for sidecar persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastpqProofEnqueueResult {
    /// The snapshot was accepted.
    Enqueued {
        /// Queue depth after the snapshot was accepted.
        queue_depth: usize,
    },
    /// The encoded snapshot exceeded the configured byte limit.
    RejectedTooLarge {
        /// Encoded snapshot size in bytes.
        actual: usize,
        /// Configured maximum size in bytes.
        max: usize,
    },
    /// The queue is already at the configured capacity.
    RejectedQueueFull {
        /// Configured queue capacity.
        cap: usize,
    },
    /// The snapshot could not be encoded for size accounting.
    RejectedEncode {
        /// Human-readable encode failure.
        reason: String,
    },
    /// A canonical prune is active or prune recovery requires a process restart.
    RejectedPruneRecovery,
}
/// Proof that Kura durably associated a canonical block with a v2 finality artifact.
///
/// Fields are intentionally private and the type has no public constructor.
/// Kura creates a receipt only after the artifact file and its directory entry
/// have been synchronously persisted.
#[derive(Clone, Debug)]
#[must_use]
pub struct KuraV2CommitReceipt {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    context_id: HeightContextId,
    subject: BlockSubject,
    certificate: QuorumCertificateRef,
    artifact_hash: HashOf<V2FinalityArtifact>,
}
impl KuraV2CommitReceipt {
    /// Return the durably associated block height.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Return the durably associated canonical block hash.
    #[must_use]
    pub fn block_hash(&self) -> HashOf<BlockHeader> {
        self.block_hash
    }
    /// Return the frozen height-context identifier.
    #[must_use]
    pub fn context_id(&self) -> HeightContextId {
        self.context_id
    }
    /// Return the exact subject durably certified by Kura.
    #[must_use]
    pub fn subject(&self) -> BlockSubject {
        self.subject
    }
    /// Return the exact CommitQC reference durably associated with the block.
    #[must_use]
    pub fn certificate(&self) -> QuorumCertificateRef {
        self.certificate
    }
    /// Return the hash of the exact artifact bytes represented by this receipt.
    #[must_use]
    pub fn artifact_hash(&self) -> HashOf<V2FinalityArtifact> {
        self.artifact_hash
    }
    #[cfg(test)]
    pub(crate) fn for_test(artifact: &V2FinalityArtifact) -> Self {
        v2_commit_receipt(artifact)
    }
}
fn v2_commit_receipt(artifact: &V2FinalityArtifact) -> KuraV2CommitReceipt {
    KuraV2CommitReceipt {
        height: artifact.height,
        block_hash: artifact.block_hash,
        context_id: artifact.context_id(),
        subject: artifact.subject,
        certificate: artifact.commit_qc.as_ref(),
        artifact_hash: HashOf::new(artifact),
    }
}
#[derive(Clone, Default, Debug)]
struct FastpqProofSidecarTelemetry;
impl FastpqProofSidecarTelemetry {
    fn set_queue_depth(&self, depth: usize) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.set_fastpq_proof_sidecar_queue_depth(u64::try_from(depth).unwrap_or(u64::MAX));
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = depth;
    }
    fn record_event(&self, event: &'static str) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.inc_fastpq_proof_sidecar_event(event);
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = event;
    }
}
#[derive(Debug)]
struct TransactionEntrypointIndex {
    complete: bool,
    indexed_heights: BTreeSet<NonZeroUsize>,
    incomplete_merge_heights: BTreeSet<NonZeroUsize>,
    heights_by_entrypoint: TransactionEntrypointHeights,
    heights_by_offline_operation_id: OfflineOperationHeights,
    heights_by_authority: TransactionAuthorityHeights,
    heights_by_timestamp_ms: TransactionTimestampHeights,
    heights_by_result_status: TransactionResultStatusHeights,
}
impl TransactionEntrypointIndex {
    fn complete_empty() -> Self {
        Self {
            complete: true,
            indexed_heights: BTreeSet::new(),
            incomplete_merge_heights: BTreeSet::new(),
            heights_by_entrypoint: BTreeMap::new(),
            heights_by_offline_operation_id: BTreeMap::new(),
            heights_by_authority: BTreeMap::new(),
            heights_by_timestamp_ms: BTreeMap::new(),
            heights_by_result_status: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub(crate) struct WsvCheckpoint {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    state_hash: Hash,
    /// Digest of the complete commit manifest written after this checkpoint, when available.
    #[norito(default)]
    commit_manifest_hash: Option<Hash>,
}
impl WsvCheckpoint {
    fn new(height: u64, block_hash: HashOf<BlockHeader>, state_hash: Hash) -> Self {
        Self {
            height,
            block_hash,
            state_hash,
            commit_manifest_hash: None,
        }
    }
    pub(crate) fn state_hash(&self) -> Hash {
        self.state_hash
    }
}
/// Durable record tying a canonical block to the committed in-memory WSV root.
///
/// WSV remains memory-only at runtime. In first-release v2, every replay-complete full-body commit
/// has an exact checkpoint-bound manifest written after the block body and WSV are durable. Only
/// the sole interrupted pending tip may temporarily lack this join record; authenticated hash-only
/// snapshot prefixes are exempt because their bodies cannot be replayed locally.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub(crate) struct CommitManifest {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    parent_state_root: Option<Hash>,
    post_state_root: Option<Hash>,
    wsv_checkpoint_hash: Hash,
    commit_qc_hash: Option<Hash>,
    /// Digest of the exact authenticated QC, checkpoint, and parent-state stake authority.
    ///
    /// A sole interrupted pending-tip window may temporarily omit this field until startup binds
    /// the exact authenticated v2 finality authority.
    #[norito(default)]
    commit_authority_hash: Option<Hash>,
}
/// Relationship between a durable manifest and the digest slot in its WSV checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommitManifestBindingState {
    /// The checkpoint exists but the post-manifest digest was not published yet.
    Unbound,
    /// The checkpoint digest matches every encoded manifest byte.
    Bound,
    /// The checkpoint names a different manifest digest and must fail closed.
    Mismatched,
}
#[derive(Encode)]
struct V2CommitAuthoritySeal {
    domain: String,
    artifact: V2FinalityArtifact,
}
fn v2_commit_authority_hash(artifact: &V2FinalityArtifact) -> Hash {
    Hash::new(
        V2CommitAuthoritySeal {
            domain: "iroha.v2.commit-authority-seal.v1".to_owned(),
            artifact: artifact.clone(),
        }
        .encode(),
    )
}
/// Known immutable Kagemusha top-up finality sidecar formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum KagemushaTopUpFinalitySidecarFormat {
    /// Canonical bounded block-local top-up tree and Commit-QC binding.
    #[codec(index = 1)]
    Current,
}
/// One canonical Kagemusha top-up anchor and its block-local Merkle path.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct KagemushaTopUpFinalityLeaf {
    /// Top-up operation identifier (the bytes following V4 witness key tag `0xD2`).
    pub operation_id: [u8; 32],
    /// Digest of the complete on-chain top-up anchor.
    pub anchor_digest: [u8; 32],
    /// Zero-based position in canonical operation-id order.
    pub leaf_index: u32,
    /// Number of real leaves in the block-local tree.
    pub leaf_count: u32,
    /// Merkle siblings from leaf level to root.
    pub siblings: Vec<Hash>,
}
/// Immutable Kura record used to serve a finalized Kagemusha top-up proof.
///
/// The sidecar intentionally retains only the bounded top-up subtree, the
/// ordinary-write root needed to reconstruct the consensus post-state root,
/// and the hash of the exact durable Sumeragi-v2 finality artifact. Unrelated
/// execution-witness values and duplicate certificates are not copied.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct KagemushaTopUpFinalitySidecar {
    /// Schema/evolution discriminator.
    pub format: KagemushaTopUpFinalitySidecarFormat,
    /// Numeric format version, bound independently from the enum codec index.
    pub version: u16,
    /// Canonical block height.
    pub height: u64,
    /// Canonical block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Root of all non-Kagemusha writes in the execution witness.
    pub ordinary_writes_root: Hash,
    /// Root of the canonical bounded top-up tree.
    pub topup_anchor_root: Hash,
    /// Consensus post-state root certified by the bound finality artifact.
    pub post_state_root: Hash,
    /// Hash of the exact durably persisted Sumeragi-v2 finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Canonically sorted top-up leaves and their exact paths.
    pub leaves: Vec<KagemushaTopUpFinalityLeaf>,
}
impl KagemushaTopUpFinalitySidecar {
    /// Current numeric sidecar version.
    pub const VERSION: u16 = 1;
    /// Return the proof leaf for an exact operation id.
    #[must_use]
    pub fn leaf_for_operation(
        &self,
        operation_id: &[u8; 32],
    ) -> Option<&KagemushaTopUpFinalityLeaf> {
        self.leaves
            .binary_search_by_key(operation_id, |leaf| leaf.operation_id)
            .ok()
            .and_then(|index| self.leaves.get(index))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct StagedKagemushaTopUpFinalitySidecar {
    format: KagemushaTopUpFinalitySidecarFormat,
    version: u16,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    ordinary_writes_root: Hash,
    topup_anchor_root: Hash,
    post_state_root: Hash,
    leaves: Vec<KagemushaTopUpFinalityLeaf>,
}
/// Immutable Kura proof that one block's receiver snapshot synthetic write is
/// included in the ordinary-write root authenticated by its exact finality artifact.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct KagemushaActiveReceiverFinalitySidecarV1 {
    /// Sidecar version.
    pub version: u16,
    /// Canonical block height.
    pub height: u64,
    /// Canonical block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Root of all non-top-up execution writes.
    pub ordinary_writes_root: Hash,
    /// Final post-state root, including top-up composition when present.
    pub post_state_root: Hash,
    /// Hash of the exact durable finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Fixed-key sparse-SMT proof and exact encoded receiver commitment.
    pub witness_proof: KagemushaActiveReceiverWitnessProofV1,
    /// Fixed-key sparse-SMT proof and exact encoded validation-fee commitment.
    pub validation_fee_policy_witness: ValidationFeePolicyWitnessProofV1,
}
impl KagemushaActiveReceiverFinalitySidecarV1 {
    /// Current sidecar version.
    pub const VERSION: u16 = 1;
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct StagedKagemushaActiveReceiverFinalitySidecarV1 {
    version: u16,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    ordinary_writes_root: Hash,
    post_state_root: Hash,
    witness_proof: KagemushaActiveReceiverWitnessProofV1,
    validation_fee_policy_witness: ValidationFeePolicyWitnessProofV1,
}
impl CommitManifest {
    /// Construct a manifest for a committed height.
    pub(crate) fn new(
        height: u64,
        block_hash: HashOf<BlockHeader>,
        parent_state_root: Option<Hash>,
        post_state_root: Option<Hash>,
        wsv_checkpoint_hash: Hash,
        commit_qc_hash: Option<Hash>,
    ) -> Self {
        Self {
            height,
            block_hash,
            parent_state_root,
            post_state_root,
            wsv_checkpoint_hash,
            commit_qc_hash,
            commit_authority_hash: None,
        }
    }
    /// Bind the exact authenticated v2 finality artifact and its execution roots.
    ///
    /// The caller must first perform the artifact's structural and cryptographic verification.
    /// Startup recovery rechecks the resulting manifest with
    /// [`Self::binds_authenticated_v2_commit_authority`] before trusting either root.
    #[must_use]
    pub(crate) fn with_authenticated_v2_commit_authority(
        mut self,
        artifact: &V2FinalityArtifact,
    ) -> Self {
        let commitment = artifact.commit_qc.execution_commitment;
        self.parent_state_root = Some(commitment.parent_state_root);
        self.post_state_root = Some(commitment.post_state_root);
        self.commit_qc_hash = Some(Hash::new(artifact.commit_qc.encode()));
        self.commit_authority_hash = Some(v2_commit_authority_hash(artifact));
        self
    }
    fn encoded_hash(&self) -> Hash {
        Hash::new(self.encode())
    }
    /// Return whether every retained root and authority byte matches this verified v2 artifact.
    pub(crate) fn binds_authenticated_v2_commit_authority(
        &self,
        artifact: &V2FinalityArtifact,
    ) -> bool {
        let commitment = artifact.commit_qc.execution_commitment;
        self.height == artifact.height
            && self.block_hash == artifact.block_hash
            && artifact.subject.block_hash == artifact.block_hash
            && self.parent_state_root == Some(commitment.parent_state_root)
            && self.post_state_root == Some(commitment.post_state_root)
            && self.commit_qc_hash == Some(Hash::new(artifact.commit_qc.encode()))
            && self.commit_authority_hash == Some(v2_commit_authority_hash(artifact))
    }
}
#[derive(Clone, Copy, Debug)]
struct BlockReplicaAdvert {
    keeper_index: u32,
    observed_at: Instant,
}
#[derive(Clone, Debug)]
struct VerifiedKuraReplicaAuthority {
    key: BlockReplicaKey,
    network_id: NetworkId,
    selected_keepers: Vec<(u32, PeerId)>,
}
#[derive(Encode)]
struct KuraReplicaKeeperScoreV1 {
    domain: Vec<u8>,
    network_id: NetworkId,
    context_id: HeightContextId,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    signer_index: u32,
    signer: PeerId,
}
/// Local body availability for a canonical block known to Kura.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockBodyStatus {
    /// Body is cached in memory.
    Cached,
    /// Body is present in `blocks.data`.
    Inline,
    /// Body is present only in the local sidecar cache.
    LocalSidecar,
    /// Body is not local, but enough peers have advertised replicas.
    RemoteOnly {
        /// Number of distinct matching peer adverts.
        replicas: usize,
    },
    /// Body is neither local nor sufficiently replicated.
    Missing,
}
#[derive(Clone, Copy, Debug)]
enum FsyncTarget {
    Data,
    Index,
    Hashes,
}
impl FsyncTarget {
    #[cfg(feature = "telemetry")]
    fn label(self) -> &'static str {
        match self {
            Self::Data => "blocks.data",
            Self::Index => "blocks.index",
            Self::Hashes => "blocks.hashes",
        }
    }
}
#[derive(Debug, Clone)]
struct FsyncState {
    mode: FsyncMode,
    interval: Duration,
    pending_since: Option<Instant>,
}
impl FsyncState {
    fn new(mode: FsyncMode, interval: Duration) -> Self {
        Self {
            mode,
            interval,
            pending_since: None,
        }
    }
    fn record_write(&mut self, now: Instant) {
        self.pending_since.get_or_insert(now);
    }
    fn clear(&mut self) {
        self.pending_since = None;
    }
    fn deadline(&self) -> Option<Instant> {
        match (self.mode, self.pending_since) {
            (_, None) => None,
            (FsyncMode::Always, Some(ts)) => Some(ts),
            (FsyncMode::Batched, Some(ts)) => Some(ts + self.interval),
        }
    }
    fn is_due(&self, now: Instant, force: bool) -> bool {
        match self.mode {
            FsyncMode::Always => self.pending_since.is_some(),
            FsyncMode::Batched => self.pending_since.is_some_and(|pending| {
                force
                    || self.interval == Duration::ZERO
                    || now.saturating_duration_since(pending) >= self.interval
            }),
        }
    }
}
#[derive(Clone, Default, Debug)]
struct FsyncTelemetry;
impl FsyncTelemetry {
    fn new(mode: FsyncMode) -> Self {
        let telemetry = Self;
        telemetry.update_mode(mode);
        telemetry
    }
    fn update_mode(&self, mode: FsyncMode) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.set_kura_fsync_mode(mode);
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = mode;
    }
    fn record_success(&self, target: FsyncTarget, duration: Duration) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.record_kura_fsync_latency(target.label(), duration);
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = (target, duration);
    }
    fn record_failure(&self, target: FsyncTarget, duration: Option<Duration>) {
        let _ = self;
        #[cfg(feature = "telemetry")]
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics.inc_kura_fsync_failure(target.label());
            if let Some(duration) = duration {
                metrics.record_kura_fsync_latency(target.label(), duration);
            }
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = (target, duration);
    }
}
#[derive(Debug)]
struct ChainValidation {
    hashes: Vec<HashOf<BlockHeader>>,
    truncated: bool,
    hash_mismatch: bool,
    hard_fork_hash_only_block_count: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvisionalSnapshotBootstrap {
    hash_only_prefix_height: usize,
    bootstrap_lineage_hash: Option<Hash>,
    hash_journal_digest: Option<Hash>,
}
#[derive(Clone, Debug)]
enum SnapshotBootstrapRuntimeState {
    Authenticated,
    Pending(ProvisionalSnapshotBootstrap),
    Finalizing,
}
impl SnapshotBootstrapRuntimeState {
    fn pending_metadata(&self) -> Option<&ProvisionalSnapshotBootstrap> {
        let Self::Pending(metadata) = self else {
            return None;
        };
        Some(metadata)
    }
    fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated)
    }
    fn begin_finalization(&mut self, expected: &ProvisionalSnapshotBootstrap) -> bool {
        if !matches!(self, Self::Pending(current) if current == expected) {
            return false;
        }
        *self = Self::Finalizing;
        true
    }
    #[cfg(test)]
    fn finish_finalization(&mut self) -> bool {
        if !matches!(self, Self::Finalizing) {
            return false;
        }
        *self = Self::Authenticated;
        true
    }
}
/// Non-forgeable, instance-bound authority for the narrow set of deferred
/// recovery writes performed while snapshot bootstrap is `Finalizing`.
struct SnapshotFinalizationMutationAuthority<'a> {
    kura: &'a Kura,
}
impl<'a> SnapshotFinalizationMutationAuthority<'a> {
    fn new(kura: &'a Kura) -> Result<Self> {
        if !matches!(
            *kura.provisional_snapshot_bootstrap.lock(),
            SnapshotBootstrapRuntimeState::Finalizing
        ) {
            return Err(Error::SnapshotBootstrapAuthenticationPending);
        }
        Ok(Self { kura })
    }
    fn validate_for(&self, kura: &Kura) -> Result<()> {
        if !std::ptr::eq(self.kura, kura)
            || !matches!(
                *kura.provisional_snapshot_bootstrap.lock(),
                SnapshotBootstrapRuntimeState::Finalizing
            )
        {
            return Err(Error::SnapshotBootstrapAuthenticationPending);
        }
        kura.ensure_canonical_storage_not_poisoned()
    }
}
enum StartupRecoveryMutationAuthority<'a> {
    Authenticated,
    SnapshotFinalization(&'a SnapshotFinalizationMutationAuthority<'a>),
}
impl StartupRecoveryMutationAuthority<'_> {
    fn validate_for(&self, kura: &Kura) -> Result<()> {
        match self {
            Self::Authenticated => kura.durable_mutation_authorized(),
            Self::SnapshotFinalization(authority) => authority.validate_for(kura),
        }
    }
}
#[derive(Debug)]
struct CommitManifestReconciliation {
    manifests_present: bool,
    pruned_manifests: bool,
    pruned_checkpoints: bool,
    retained_height: usize,
}
#[derive(Debug)]
struct MergeLedgerLog {
    file: Option<FileWrap>,
    entries: Vec<MergeLedgerEntry>,
    cache_capacity: usize,
    total_entries: usize,
    frames_by_hash: BTreeMap<HashOf<MergeLedgerEntry>, MergeLedgerFrameIndex>,
    frames_by_epoch: BTreeMap<u64, MergeLedgerFrameIndex>,
    in_memory_entries: BTreeMap<HashOf<MergeLedgerEntry>, MergeLedgerEntry>,
    /// Latest execution coordinate and exact entry hash by route/incarnation.
    ///
    /// This index is rebuilt while the validated log is streamed at startup;
    /// post-WSV recovery must never reverse-scan historical merge entries.
    latest_execution_entries:
        BTreeMap<(LaneId, DataSpaceId, Hash), (u64, HashOf<MergeLedgerEntry>)>,
    append_recovery_offset: Option<u64>,
    #[cfg(test)]
    full_history_scans: usize,
    #[cfg(test)]
    indexed_lookups: usize,
    #[cfg(test)]
    indexed_membership_checks: usize,
    #[cfg(test)]
    complete_execution_scans: usize,
    #[cfg(test)]
    fail_next_append: bool,
    #[cfg(test)]
    fail_next_append_after: Option<MergeLedgerAppendFailurePoint>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MergeLedgerFrameIndex {
    frame_offset: u64,
    payload_len: u32,
    epoch_id: u64,
    entry_hash: HashOf<MergeLedgerEntry>,
}
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeLedgerAppendFailurePoint {
    AfterLength,
    AfterPayload,
    AfterSync,
}
/// Durable sparse association between one committed merge entry and the exact
/// global block whose compact reference ordered its application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub(crate) struct MergeLedgerCarrierRecord {
    /// Carrier-record schema version. Only version one is accepted.
    pub version: u8,
    /// Canonical full-entry sidecar hash.
    pub entry_hash: HashOf<MergeLedgerEntry>,
    /// Contiguous merge-ledger epoch authenticated by the entry QC.
    pub epoch_id: u64,
    /// Sparse canonical global block height carrying the compact reference.
    pub block_height: u64,
    /// Exact canonical global block hash at `block_height`.
    pub block_hash: HashOf<BlockHeader>,
}
impl MergeLedgerCarrierRecord {
    fn new(entry: &MergeLedgerEntry, block: &SignedBlock) -> Self {
        Self {
            version: 1,
            entry_hash: entry.canonical_hash(),
            epoch_id: entry.epoch_id,
            block_height: block.header().height().get(),
            block_hash: block.hash(),
        }
    }
}

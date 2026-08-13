/// Per-route Native AMX application leaf and its QC-authenticated Merkle proof.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct NativeAmxParticipantApplicationManifestArtifactV1 {
    /// Exact durable artifact schema version.
    pub version: u16,
    /// Canonical manifest leaf committed by the global CommitQC.
    pub leaf: NativeAmxApplicationManifestLeafV1,
    /// Zero-based position of `leaf` in canonical route order.
    pub leaf_index: u32,
    /// Merkle proof from `leaf` to `manifest_root`.
    pub proof: MerkleProof<NativeAmxApplicationManifestLeafV1>,
    /// Exact root authenticated by the global CommitQC.
    pub manifest_root: Hash,
    /// Exact route-leaf count authenticated by the global CommitQC.
    pub manifest_leaf_count: u32,
    /// Hash of the independently persisted and verified v2 finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
}
impl NativeAmxParticipantApplicationManifestArtifactV1 {
    const VERSION: u16 = 1;
    const FORMAT_LABEL: &'static str = "lane.native_amx_participant_application_manifest.v1";
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct NativeAmxEvidencePruneEntryV2 {
    kind: u8,
    participant_height: u64,
    artifact_hash: Hash,
}
/// Exact durable evidence that a Native AMX prune operation may never replace.
///
/// `identity` is the same independently versioned projection used by the
/// derived latest pointer, but the prune intent owns its own immutable copy.
/// The receipt hash additionally binds the complete result-bearing artifact;
/// `identity` already binds its manifest, finality, carrier, and route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct NativeAmxEvidencePruneProtectedLatestV2 {
    identity: NativeAmxParticipantReceiptLatestIndexV2,
    receipt_artifact_hash: HashOf<NativeAmxParticipantApplicationReceiptArtifact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct NativeAmxEvidencePruneIntentV2 {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    protected_latest: NativeAmxEvidencePruneProtectedLatestV2,
    entries: Vec<NativeAmxEvidencePruneEntryV2>,
}
impl NativeAmxEvidencePruneIntentV2 {
    const VERSION: u8 = 2;
    const MANIFEST_KIND: u8 = 1;
    const RECEIPT_KIND: u8 = 2;
}
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct NativeAmxParticipantApplicationReceiptArtifact {
    /// Exact durable sidecar schema version.
    pub version: u16,
    /// Participant control proposal certified by its lane committee.
    pub participant_proposal: LaneBlockProposalV1,
    /// Exact zero-effect control settlement certified alongside the proposal.
    pub participant_settlement: LaneBlockCommitment,
    /// Canonical hash of `participant_settlement` carried by both participant QCs.
    pub participant_settlement_hash: HashOf<LaneBlockCommitment>,
    /// Canonical global block which executed the control members.
    pub application_block_height: u64,
    /// Canonical global block identity. It binds the execution context, not the
    /// result root (which consensus hashing deliberately excludes).
    pub application_block_hash: HashOf<BlockHeader>,
    /// Hash of the canonical result-bearing global block wire.
    pub executed_block_wire_hash: Hash,
    /// Hash of the exact independently verified durable v2 finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Hash of the distinct per-route manifest leaf/proof artifact.
    pub manifest_artifact_hash: HashOf<NativeAmxParticipantApplicationManifestArtifactV1>,
    /// Source transaction identities in canonical block order. These are
    /// intentionally distinct from `entrypoint_hashes`.
    pub source_ids: Vec<[u8; Hash::LENGTH]>,
    /// Canonical global entrypoint indices whose results apply this control.
    pub entrypoint_indices: Vec<u64>,
    /// Canonical entrypoint hashes at `entrypoint_indices`.
    pub entrypoint_hashes: Vec<HashOf<TransactionEntrypoint>>,
    /// Hashes of the exact committed transaction results.
    pub result_hashes: Vec<HashOf<TransactionResult>>,
    /// Exact canonical transaction results.
    pub results: Vec<TransactionResult>,
}
type NativeAmxParticipantApplicationArtifactPair = (
    NativeAmxParticipantApplicationManifestArtifactV1,
    NativeAmxParticipantApplicationReceiptArtifact,
);
/// Side-effect-free failure while projecting the exact durable Native AMX
/// artifact bytes which a candidate would require.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NativeAmxParticipantApplicationEvidenceByteBudgetError {
    /// The canonical manifest could not supply one proof per route.
    #[error("Native AMX participant evidence artifact construction failed")]
    ArtifactConstruction,
    /// Exact Norito framing failed before any persistence boundary.
    #[error("Native AMX participant evidence artifact framing failed: {0}")]
    ArtifactFraming(#[source] norito::Error),
    /// The exact framed pair violates a configured or hard byte bound.
    #[error("{0}")]
    Budget(String),
}
/// Bounded route/incarnation pointer to the latest Native AMX application receipt.
///
/// The immutable per-height manifest and receipt files remain authoritative.
/// This independently versioned derived pointer is rebuilt from that
/// standalone evidence set during startup and lets consensus/drain readers
/// avoid reverse history scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct NativeAmxParticipantReceiptLatestIndexV2 {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    participant_proposal_hash: Hash,
    participant_settlement_hash: HashOf<LaneBlockCommitment>,
    application_block_height: u64,
    application_block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    manifest_artifact_hash: HashOf<NativeAmxParticipantApplicationManifestArtifactV1>,
}
/// Exact in-memory attestation that every separate-participant frontier in one
/// canonical carrier has crossed the pre-WSV durable publication boundary.
///
/// This token is deliberately not a wire or persistence layout. It can only be
/// constructed after Kura has read back the exact manifest, receipt, and
/// latest-index bytes under the publication guards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeAmxParticipantApplicationPrepublicationToken {
    application_block_height: u64,
    application_block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    manifest_root: Hash,
    manifest_leaf_count: u32,
    identities: Vec<NativeAmxParticipantApplicationPrepublicationIdentity>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeAmxParticipantApplicationPrepublicationIdentity {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    participant_height: u64,
    manifest_leaf_hash: HashOf<NativeAmxApplicationManifestLeafV1>,
    manifest_artifact_hash: HashOf<NativeAmxParticipantApplicationManifestArtifactV1>,
    receipt_artifact_hash: HashOf<NativeAmxParticipantApplicationReceiptArtifact>,
    latest_index_artifact_hash: HashOf<NativeAmxParticipantReceiptLatestIndexV2>,
}
struct NativeAmxParticipantApplicationEvidencePlan {
    application_block_height: u64,
    application_block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    manifest_root: Hash,
    manifest_leaf_count: u32,
    artifacts: Vec<NativeAmxParticipantApplicationArtifactPair>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeAmxParticipantApplicationRoutePreflight {
    incoming: NativeAmxParticipantReceiptLatestIndexV2,
    current: Option<NativeAmxParticipantReceiptLatestIndexV2>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeAmxParticipantApplicationManifestReadback {
    manifest_root: Hash,
    manifest_leaf_count: u32,
    artifact_hashes: Vec<HashOf<NativeAmxParticipantApplicationManifestArtifactV1>>,
}
impl NativeAmxParticipantApplicationManifestReadback {
    fn authenticates(
        &self,
        plan: &NativeAmxParticipantApplicationEvidencePlan,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
    ) -> bool {
        self.manifest_root == plan.manifest_root
            && self.manifest_leaf_count == plan.manifest_leaf_count
            && usize::try_from(self.manifest_leaf_count).ok() == Some(self.artifact_hashes.len())
            && self
                .artifact_hashes
                .get(usize::try_from(manifest.leaf_index).unwrap_or(usize::MAX))
                == Some(&HashOf::new(manifest))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeAmxParticipantApplicationPublicationMode {
    PreWsv,
    PostWsvRepair,
}
impl NativeAmxParticipantApplicationPublicationMode {
    const fn requires_post_apply_metadata(self) -> bool {
        matches!(self, Self::PostWsvRepair)
    }
    const fn permits_retention_cleanup(self) -> bool {
        matches!(self, Self::PostWsvRepair)
    }
}
/// Startup-only classification for a retained Native AMX participant receipt.
///
/// Runtime readers never accept either pending state: they continue to require
/// the complete manifest/finality/checkpoint/commit-manifest join. Startup may
/// admit `PendingTipMetadata` only for the highest receipt at the interrupted
/// canonical tip. A missing manifest is weaker and is admitted only for that
/// same highest receipt: its structurally valid bytes are retained for exact
/// comparison during repair, but never promoted to the derived latest pointer
/// until the QC-authenticated manifest is restored. Every older retained
/// receipt must already have the complete durable evidence join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeAmxParticipantReceiptStartupEvidence {
    DurablyApplied,
    PendingTipMetadata,
    PendingManifestRepair,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeAmxLatestIndexTempReconciliation {
    Absent,
    RemovedIdentical,
    Promoted,
}
fn native_amx_startup_retention_cleanup_authorized(
    newest_evidence: Option<NativeAmxParticipantReceiptStartupEvidence>,
    has_partial_pair: bool,
) -> bool {
    !has_partial_pair
        && newest_evidence.is_none_or(|evidence| {
            evidence == NativeAmxParticipantReceiptStartupEvidence::DurablyApplied
        })
}
impl NativeAmxParticipantReceiptLatestIndexV2 {
    const VERSION: u8 = 2;
    fn from_receipt(receipt: &NativeAmxParticipantApplicationReceiptArtifact) -> Self {
        let descriptor = &receipt.participant_proposal.descriptor;
        Self {
            version: Self::VERSION,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
            participant_proposal_hash: receipt.participant_proposal.proposal_hash,
            participant_settlement_hash: receipt.participant_settlement_hash,
            application_block_height: receipt.application_block_height,
            application_block_hash: receipt.application_block_hash,
            executed_block_wire_hash: receipt.executed_block_wire_hash,
            finality_artifact_hash: receipt.finality_artifact_hash,
            manifest_artifact_hash: receipt.manifest_artifact_hash,
        }
    }
    fn matches_receipt(&self, receipt: &NativeAmxParticipantApplicationReceiptArtifact) -> bool {
        *self == Self::from_receipt(receipt)
    }
    fn matches_manifest(
        &self,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
    ) -> bool {
        let leaf = &manifest.leaf;
        self.lane_id == leaf.lane_id
            && self.dataspace_id == leaf.dataspace_id
            && self.lane_incarnation == leaf.lane_incarnation
            && self.lane_block_height == leaf.participant_height
            && self.participant_proposal_hash == leaf.proposal_hash
            && self.participant_settlement_hash == leaf.settlement_hash
            && self.application_block_height == leaf.application_block_height
            && self.application_block_hash == leaf.application_block_hash
            && self.executed_block_wire_hash == leaf.executed_block_wire_hash
            && self.finality_artifact_hash == manifest.finality_artifact_hash
            && self.manifest_artifact_hash == HashOf::new(manifest)
    }
}
impl NativeAmxEvidencePruneProtectedLatestV2 {
    fn from_artifacts(
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Option<Self> {
        let identity = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt);
        (receipt.manifest_artifact_hash == HashOf::new(manifest)
            && identity.matches_manifest(manifest))
        .then(|| Self {
            identity,
            receipt_artifact_hash: HashOf::new(receipt),
        })
    }
}
impl NativeAmxParticipantApplicationReceiptArtifact {
    const VERSION: u16 = 2;
    const FORMAT_LABEL: &'static str = "lane.native_amx_participant_application_receipt.v2";
    fn new(
        entry: &crate::sumeragi::exec::NativeAmxApplicationManifestEntryV1,
        manifest_artifact_hash: HashOf<NativeAmxParticipantApplicationManifestArtifactV1>,
        finality_artifact_hash: HashOf<V2FinalityArtifact>,
    ) -> Self {
        let leaf = &entry.leaf;
        Self {
            version: Self::VERSION,
            participant_proposal: entry.participant_proposal.clone(),
            participant_settlement: entry.participant_settlement.clone(),
            participant_settlement_hash: leaf.settlement_hash,
            application_block_height: leaf.application_block_height,
            application_block_hash: leaf.application_block_hash,
            executed_block_wire_hash: leaf.executed_block_wire_hash,
            finality_artifact_hash,
            manifest_artifact_hash,
            source_ids: leaf.members.iter().map(|member| member.source_id).collect(),
            entrypoint_indices: leaf
                .members
                .iter()
                .map(|member| member.entrypoint_index)
                .collect(),
            entrypoint_hashes: leaf
                .members
                .iter()
                .map(|member| member.entrypoint_hash)
                .collect(),
            result_hashes: leaf
                .members
                .iter()
                .map(|member| member.result_hash)
                .collect(),
            results: entry.results.clone(),
        }
    }
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }
}
fn native_amx_participant_application_finality_placeholder_hash() -> HashOf<V2FinalityArtifact> {
    HashOf::from_untyped_unchecked(Hash::new(
        b"Native AMX participant evidence finality placeholder",
    ))
}
/// Build the exact ordered per-route artifact pairs without consulting Kura
/// state or performing I/O. The finality identity is fixed-width, so callers
/// may use the typed placeholder during candidate validation and the actual
/// artifact hash during decided-block application without changing lengths.
fn native_amx_participant_application_artifacts(
    manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
) -> Option<Vec<NativeAmxParticipantApplicationArtifactPair>> {
    let mut artifacts = Vec::with_capacity(manifest.entries().len());
    for (index, entry) in manifest.entries().iter().enumerate() {
        let leaf_index = u32::try_from(index).ok()?;
        let manifest_artifact = NativeAmxParticipantApplicationManifestArtifactV1 {
            version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
            leaf: entry.leaf.clone(),
            leaf_index,
            proof: manifest.proof(leaf_index)?,
            manifest_root: manifest.root(),
            manifest_leaf_count: manifest.count(),
            finality_artifact_hash,
        };
        let receipt = NativeAmxParticipantApplicationReceiptArtifact::new(
            entry,
            HashOf::new(&manifest_artifact),
            finality_artifact_hash,
        );
        artifacts.push((manifest_artifact, receipt));
    }
    Some(artifacts)
}
fn native_amx_participant_application_pair_framed_bytes(
    manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
    receipt: &NativeAmxParticipantApplicationReceiptArtifact,
) -> Result<(Vec<u8>, Vec<u8>), norito::Error> {
    Ok((manifest.encode_framed()?, receipt.encode_framed()?))
}
fn checked_native_amx_participant_application_pair_bytes(
    manifest_bytes: u64,
    receipt_bytes: u64,
) -> std::result::Result<u64, NativeAmxParticipantApplicationEvidenceByteBudgetError> {
    manifest_bytes.checked_add(receipt_bytes).ok_or_else(|| {
        NativeAmxParticipantApplicationEvidenceByteBudgetError::Budget(
            "Native AMX participant manifest/receipt pair byte length overflowed".to_owned(),
        )
    })
}
impl NativeAmxParticipantApplicationPrepublicationIdentity {
    fn from_artifacts(
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Option<Self> {
        let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt);
        if receipt.manifest_artifact_hash != HashOf::new(manifest)
            || receipt.finality_artifact_hash != manifest.finality_artifact_hash
            || !latest.matches_manifest(manifest)
        {
            return None;
        }
        let leaf = &manifest.leaf;
        Some(Self {
            lane_id: leaf.lane_id,
            dataspace_id: leaf.dataspace_id,
            lane_incarnation: leaf.lane_incarnation,
            participant_height: leaf.participant_height,
            manifest_leaf_hash: HashOf::new(leaf),
            manifest_artifact_hash: HashOf::new(manifest),
            receipt_artifact_hash: HashOf::new(receipt),
            latest_index_artifact_hash: HashOf::new(&latest),
        })
    }
}
impl NativeAmxParticipantApplicationPrepublicationToken {
    fn from_plan(
        plan: &NativeAmxParticipantApplicationEvidencePlan,
        identities: Vec<NativeAmxParticipantApplicationPrepublicationIdentity>,
    ) -> Option<Self> {
        if usize::try_from(plan.manifest_leaf_count).ok() != Some(identities.len()) {
            return None;
        }
        Some(Self {
            application_block_height: plan.application_block_height,
            application_block_hash: plan.application_block_hash,
            executed_block_wire_hash: plan.executed_block_wire_hash,
            finality_artifact_hash: plan.finality_artifact_hash,
            manifest_root: plan.manifest_root,
            manifest_leaf_count: plan.manifest_leaf_count,
            identities,
        })
    }
    /// Verify that this read-back token covers exactly the canonical manifest
    /// which will stage Native participant frontiers in State.
    #[must_use]
    pub(crate) fn authenticates(
        &self,
        block: &SignedBlock,
        manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
        finality: &V2FinalityArtifact,
    ) -> bool {
        let Ok(executed_block_wire) = block.encode_wire() else {
            return false;
        };
        let Ok(executed_block_wire_len) = u64::try_from(executed_block_wire.len()) else {
            return false;
        };
        let executed_block_wire_hash = Hash::new(&executed_block_wire);
        let execution = &finality.commit_qc.execution_commitment;
        if self.application_block_height != block.header().height().get()
            || self.application_block_hash != block.hash()
            || self.executed_block_wire_hash != executed_block_wire_hash
            || self.finality_artifact_hash != HashOf::new(finality)
            || self.manifest_root != manifest.root()
            || self.manifest_leaf_count != manifest.count()
            || finality.block_hash != block.hash()
            || execution.native_amx_application_manifest_version
                != iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION
            || execution.native_amx_application_manifest_root != manifest.root()
            || execution.native_amx_application_manifest_count != manifest.count()
            || execution.executed_block_wire_len != executed_block_wire_len
            || execution.executed_block_wire_len != manifest.executed_block_wire_len()
            || execution.executed_block_wire_hash != executed_block_wire_hash
        {
            return false;
        }
        let Some(artifacts) =
            native_amx_participant_application_artifacts(manifest, self.finality_artifact_hash)
        else {
            return false;
        };
        let mut expected = Vec::with_capacity(artifacts.len());
        for (manifest_artifact, receipt) in artifacts {
            let Some(identity) =
                NativeAmxParticipantApplicationPrepublicationIdentity::from_artifacts(
                    &manifest_artifact,
                    &receipt,
                )
            else {
                return false;
            };
            expected.push(identity);
        }
        self.identities == expected
    }
    /// Verify the exact ordered State frontier projection authenticated by
    /// this durable prepublication token.
    #[must_use]
    pub(crate) fn authenticates_state_frontiers(
        &self,
        block: &SignedBlock,
        manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
        finality: &V2FinalityArtifact,
        frontiers: &[crate::state::AppliedNativeAmxParticipantFrontierMarker],
    ) -> bool {
        if !self.authenticates(block, manifest, finality)
            || frontiers.len() != manifest.entries().len()
        {
            return false;
        }
        manifest
            .entries()
            .iter()
            .zip(frontiers)
            .all(|(entry, frontier)| {
                let leaf = &entry.leaf;
                frontier.version == 2
                    && frontier.lane_id == leaf.lane_id
                    && frontier.dataspace_id == leaf.dataspace_id
                    && frontier.lane_incarnation == leaf.lane_incarnation
                    && frontier.lane_block_height == leaf.participant_height
                    && frontier.participant_view == leaf.participant_view
                    && frontier.previous_lane_block_height == leaf.predecessor_height
                    && frontier.previous_lane_block_descriptor_hash
                        == leaf.predecessor_descriptor_hash
                    && frontier.lane_block_descriptor_hash == leaf.descriptor_hash
                    && frontier.participant_proposal_hash == leaf.proposal_hash
                    && frontier.participant_settlement_hash == leaf.settlement_hash
                    && frontier.application_block_height == leaf.application_block_height
                    && frontier.application_block_hash == leaf.application_block_hash
                    && u64::try_from(leaf.members.len())
                        .is_ok_and(|source_count| frontier.source_count == source_count)
            })
    }
}

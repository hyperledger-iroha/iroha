#[derive(Debug, Clone)]
struct VerifiedV2FinalityCacheEntry {
    height: u64,
    artifact_hash: HashOf<V2FinalityArtifact>,
    bytes_hash: Hash,
    metadata: StableSidecarMetadata,
}

#[derive(Debug, Clone)]
struct VerifiedRetainedBlockCacheEntry {
    bytes_hash: Hash,
    metadata: StableSidecarMetadata,
}

const V2_STARTUP_INHERITED_AUTHORITY_DOMAIN: &[u8] =
    b"iroha:kura:v2-startup-inherited-authority:v1\0";

/// Exact predecessor-controlled inputs of one Sumeragi-v2 height context.
///
/// `next_epoch_snapshot` is projected into the successor's current election
/// fields rather than copied as an optional child field. This makes an NPoS
/// transition consume exactly the powered roster, quorum, PoPs, epoch bounds,
/// and leader seed authenticated by the preceding roster.
#[derive(Encode)]
struct V2StartupInheritedAuthoritySeal {
    version: u16,
    chain_id: ChainId,
    protocol_version: u16,
    height: u64,
    epoch: u64,
    epoch_end_height: u64,
    mode: ConsensusMode,
    parent_commit_qc: Option<QuorumCertificate>,
    snapshot_bootstrap: Option<SnapshotBootstrapAnchor>,
    roster: Vec<ValidatorPower>,
    validator_set_pops: Vec<Vec<u8>>,
    quorum: DualQuorum,
    da_layout: DataAvailabilityLayout,
    leader_seed: [u8; 32],
}

impl V2StartupInheritedAuthoritySeal {
    const VERSION: u16 = 1;

    fn from_context(context: &HeightContext, validator_set_pops: &[Vec<u8>]) -> Self {
        Self {
            version: Self::VERSION,
            chain_id: context.chain_id.clone(),
            protocol_version: context.protocol_version,
            height: context.height,
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            mode: context.mode,
            parent_commit_qc: context.parent_commit_qc.clone(),
            snapshot_bootstrap: context.snapshot_bootstrap,
            roster: context.roster.clone(),
            validator_set_pops: validator_set_pops.to_vec(),
            quorum: context.quorum,
            da_layout: context.da_layout,
            leader_seed: context.leader_seed,
        }
    }

    fn expected_successor(artifact: &V2FinalityArtifact) -> Option<Self> {
        let height = artifact.height.checked_add(1)?;
        let (epoch, epoch_end_height, mode, roster, validator_set_pops, quorum, leader_seed) =
            artifact
                .height_context
                .next_epoch_snapshot
                .as_ref()
                .map_or_else(
                    || {
                        (
                            artifact.height_context.epoch,
                            artifact.height_context.epoch_end_height,
                            artifact.height_context.mode,
                            artifact.height_context.roster.clone(),
                            artifact.validator_set_pops.clone(),
                            artifact.height_context.quorum,
                            artifact.height_context.leader_seed,
                        )
                    },
                    |snapshot| {
                        (
                            snapshot.epoch,
                            snapshot.epoch_end_height,
                            snapshot.mode,
                            snapshot.roster.clone(),
                            snapshot.validator_set_pops.clone(),
                            snapshot.quorum,
                            snapshot.leader_seed,
                        )
                    },
                );
        Some(Self {
            version: Self::VERSION,
            chain_id: artifact.height_context.chain_id.clone(),
            protocol_version: artifact.height_context.protocol_version,
            height,
            epoch,
            epoch_end_height,
            mode,
            parent_commit_qc: Some(artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            roster,
            validator_set_pops,
            quorum,
            da_layout: artifact.height_context.da_layout,
            leader_seed,
        })
    }

    fn hash(&self) -> Hash {
        let encoded = self.encode();
        Hash::new_from_chunks(&[V2_STARTUP_INHERITED_AUTHORITY_DOMAIN, &encoded])
    }
}

/// Small immutable projection of one fully verified finality artifact.
///
/// Retaining complete historical artifacts would allow a maximum-size roster
/// to consume several MiB per height. Startup replay retains fixed commitments
/// to both the artifact's consumed authority and its exact predecessor-derived
/// successor authority; the sole durable-tip artifact is retained separately
/// for lane completion validation.
#[derive(Debug, Clone)]
pub(crate) struct V2StartupFinalityProjection {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    subject_block_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    commit_qc_hash: Hash,
    commit_authority_hash: Hash,
    parent_commit_qc_hash: Option<Hash>,
    inherited_authority_hash: Hash,
    successor_authority_hash: Option<Hash>,
    snapshot_bootstrap: Option<(u64, HashOf<BlockHeader>)>,
}

impl V2StartupFinalityProjection {
    fn from_artifact(artifact: &V2FinalityArtifact) -> Self {
        let execution = artifact.commit_qc.execution_commitment;
        Self {
            height: artifact.height,
            block_hash: artifact.block_hash,
            subject_block_hash: artifact.subject.block_hash,
            parent_state_root: execution.parent_state_root,
            post_state_root: execution.post_state_root,
            commit_qc_hash: Hash::new(artifact.commit_qc.encode()),
            commit_authority_hash: v2_commit_authority_hash(artifact),
            parent_commit_qc_hash: artifact
                .height_context
                .parent_commit_qc
                .as_ref()
                .map(|qc| Hash::new(qc.encode())),
            inherited_authority_hash: V2StartupInheritedAuthoritySeal::from_context(
                &artifact.height_context,
                &artifact.validator_set_pops,
            )
            .hash(),
            successor_authority_hash: V2StartupInheritedAuthoritySeal::expected_successor(artifact)
                .map(|authority| authority.hash()),
            snapshot_bootstrap: artifact
                .height_context
                .snapshot_bootstrap
                .map(|anchor| (anchor.snapshot_height, anchor.snapshot_block_hash)),
        }
    }

    pub(crate) fn binds_manifest(&self, manifest: &CommitManifest) -> bool {
        manifest.height == self.height
            && manifest.block_hash == self.block_hash
            && self.subject_block_hash == self.block_hash
            && manifest.parent_state_root == Some(self.parent_state_root)
            && manifest.post_state_root == Some(self.post_state_root)
            && manifest.commit_qc_hash == Some(self.commit_qc_hash)
            && manifest.commit_authority_hash == Some(self.commit_authority_hash)
    }

    pub(crate) const fn commit_qc_hash(&self) -> Hash {
        self.commit_qc_hash
    }

    pub(crate) const fn parent_commit_qc_hash(&self) -> Option<Hash> {
        self.parent_commit_qc_hash
    }

    pub(crate) const fn inherited_authority_hash(&self) -> Hash {
        self.inherited_authority_hash
    }

    pub(crate) const fn successor_authority_hash(&self) -> Option<Hash> {
        self.successor_authority_hash
    }

    pub(crate) const fn snapshot_bootstrap(&self) -> Option<(u64, HashOf<BlockHeader>)> {
        self.snapshot_bootstrap
    }
}

#[derive(Debug, Clone)]
struct VerifiedV2StartupFinalityEntry {
    finality: VerifiedV2FinalityCacheEntry,
    retained_block: VerifiedRetainedBlockCacheEntry,
    projection: V2StartupFinalityProjection,
}

#[derive(Debug, Clone)]
struct StableCanonicalBlockStoreMetadata {
    data: StableSidecarMetadata,
    index: StableSidecarMetadata,
    hashes: StableSidecarMetadata,
    commit_marker: StableSidecarMetadata,
}

#[derive(Debug, Clone)]
struct StableSidecarDirectoryMetadata {
    expected_path: PathBuf,
    canonical_path: Option<PathBuf>,
    metadata: Option<std::fs::Metadata>,
}

#[derive(Debug, Clone)]
struct StableSidecarDirectoryInventory {
    directory: StableSidecarDirectoryMetadata,
    files: BTreeMap<PathBuf, StableSidecarMetadata>,
}

#[derive(Debug, Clone)]
struct V2StartupReplaySidecar<T> {
    value: T,
    metadata: StableSidecarMetadata,
}

#[derive(Debug, Clone, Default)]
struct V2StartupReplaySidecarsAtHeight {
    checkpoint: Option<V2StartupReplaySidecar<WsvCheckpoint>>,
    manifest: Option<V2StartupReplaySidecar<CommitManifest>>,
}

#[derive(Debug)]
struct V2StartupFinalityVerificationInventory {
    boundary: ExactReplayBoundary,
    canonical_storage: StableCanonicalBlockStoreMetadata,
    finality_directory: StableSidecarDirectoryMetadata,
    retained_directory: StableSidecarDirectoryMetadata,
    auxiliary_sidecars: BTreeMap<PathBuf, StableSidecarDirectoryInventory>,
    /// Exact subset of `auxiliary_sidecars` derived from the active lane catalog.
    lane_auxiliary_directories: BTreeSet<PathBuf>,
    hash_only_heights: BTreeSet<u64>,
    entries: BTreeMap<u64, VerifiedV2StartupFinalityEntry>,
    replay_sidecars: Vec<V2StartupReplaySidecarsAtHeight>,
    durable_tip_artifact: Option<V2FinalityArtifact>,
}

/// Kura-minted identity binding carried from replay planning into active-height
/// recovery.
///
/// The binding never retains historical block bodies or full historical
/// finality artifacts. It carries fixed-size projections for history and the
/// sole durable-tip artifact needed for exact lane-completion validation. Its
/// fields are private so callers cannot construct a replay authorization
/// without Kura's complete startup audit.
#[derive(Debug, Clone)]
pub(crate) struct V2StartupReplayStorageBinding {
    inventory: Arc<V2StartupFinalityVerificationInventory>,
}

impl V2StartupReplayStorageBinding {
    pub(crate) fn replay_boundary(&self) -> &ExactReplayBoundary {
        &self.inventory.boundary
    }
}

/// Mutation-closed view of the startup finality inventory used by one replay
/// planning pass.
///
/// Construction is restricted to [`Kura`]. The held prune and canonical-chain
/// guards keep internal writers out while bodyless historical reads reuse the
/// exact live-body validation performed by the startup audit.
pub(crate) struct V2StartupFinalityVerificationSession<'a> {
    _prune_guard: parking_lot::MutexGuard<'a, ()>,
    _canonical_chain_guard: parking_lot::MutexGuard<'a, ()>,
    inventory: Arc<V2StartupFinalityVerificationInventory>,
}

#[derive(Debug, Clone)]
struct StableSidecarMetadata {
    canonical_path: PathBuf,
    file: std::fs::Metadata,
    directory: std::fs::Metadata,
}

#[derive(Debug)]
struct StableSidecarRead {
    bytes: Vec<u8>,
    bytes_hash: Hash,
    metadata: StableSidecarMetadata,
}

#[derive(Debug, Clone)]
struct CertifiedFrontierPairDurabilityAttestation {
    artifact_hash: HashOf<CertifiedLaneBlockArtifact>,
    data_metadata: StableSidecarMetadata,
    index_metadata: StableSidecarMetadata,
}

#[derive(Debug, Clone)]
struct CertifiedFrontierArtifactValidationAttestation {
    artifact_hash: HashOf<CertifiedLaneBlockArtifact>,
    bytes_hash: Hash,
    frontier_metadata: StableSidecarMetadata,
}

#[derive(Debug)]
struct BoundProgressDirectory {
    expected_path: PathBuf,
    canonical_path: PathBuf,
    /// Entry name relative to the next bound ancestor; `None` only for Kura root.
    entry_name: Option<std::ffi::OsString>,
    file: std::fs::File,
    metadata: std::fs::Metadata,
}

#[derive(Debug)]
struct BoundProgressNamespace {
    data_path: PathBuf,
    index_path: PathBuf,
    /// Bound directories in durability order: immediate parent through Kura root.
    directories: Vec<BoundProgressDirectory>,
}

/// Norito-encoded pipeline recovery metadata sidecar stored alongside block data.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PipelineRecoverySidecar {
    /// Schema / evolution tag for the pipeline metadata format.
    pub format: PipelineRecoveryFormat,
    /// Block height the metadata belongs to.
    pub height: u64,
    /// Block hash the metadata belongs to.
    pub block_hash: HashOf<BlockHeader>,
    /// Deterministic DAG fingerprint and key count summary.
    pub dag: PipelineDagSnapshot,
    /// Per-transaction access summaries for recovery heuristics.
    pub txs: Vec<PipelineTxSnapshot>,
    /// Optional zero-knowledge proof attachments captured for this block.
    #[norito(default)]
    pub proofs: Vec<PipelineProofSnapshot>,
    /// FASTPQ proof artifacts generated asynchronously for committed execution witnesses.
    #[norito(default)]
    pub fastpq_proofs: Vec<FastpqProofSnapshot>,
}
impl PipelineRecoverySidecar {
    const FORMAT_LABEL: &'static str = "pipeline.recovery";
    /// Create a new recovery sidecar payload.
    pub fn new(
        height: u64,
        block_hash: HashOf<BlockHeader>,
        dag: PipelineDagSnapshot,
        txs: Vec<PipelineTxSnapshot>,
    ) -> Self {
        Self {
            format: PipelineRecoveryFormat::Current,
            height,
            block_hash,
            dag,
            txs,
            proofs: Vec::new(),
            fastpq_proofs: Vec::new(),
        }
    }
    /// Return the human-readable format tag describing the recovery payload.
    pub fn format_label(&self) -> &'static str {
        match self.format {
            PipelineRecoveryFormat::Current => Self::FORMAT_LABEL,
        }
    }
    /// Convert the sidecar into a JSON value for operator tooling.
    pub fn to_json_value(&self) -> JsonValue {
        let dag = {
            let mut dag = norito::json::Map::new();
            dag.insert(
                "fingerprint".to_string(),
                norito::json::to_value(&hex::encode(self.dag.fingerprint))
                    .expect("serialize fingerprint"),
            );
            dag.insert(
                "key_count".to_string(),
                norito::json::to_value(&self.dag.key_count).expect("serialize key_count"),
            );
            norito::json::Value::Object(dag)
        };
        let txs = self
            .txs
            .iter()
            .map(|tx| {
                let mut entry = norito::json::Map::new();
                entry.insert(
                    "hash".to_string(),
                    norito::json::to_value(&tx.hash.to_string()).expect("serialize tx hash"),
                );
                entry.insert(
                    "read_count".to_string(),
                    norito::json::to_value(&tx.read_count()).expect("serialize read count"),
                );
                entry.insert(
                    "write_count".to_string(),
                    norito::json::to_value(&tx.write_count()).expect("serialize write count"),
                );
                entry.insert(
                    "reads".to_string(),
                    norito::json::to_value(&tx.reads).expect("serialize sampled reads"),
                );
                entry.insert(
                    "writes".to_string(),
                    norito::json::to_value(&tx.writes).expect("serialize sampled writes"),
                );
                norito::json::Value::Object(entry)
            })
            .collect::<Vec<_>>();
        let proofs = self
            .proofs
            .iter()
            .map(|proof| {
                let mut entry = norito::json::Map::new();
                entry.insert(
                    "backend".to_string(),
                    norito::json::to_value(&proof.backend).expect("serialize backend"),
                );
                entry.insert(
                    "proof".to_string(),
                    norito::json::to_value(&BASE64_STANDARD.encode(&proof.proof))
                        .expect("serialize proof"),
                );
                entry.insert(
                    "code_hash".to_string(),
                    norito::json::to_value(&hex::encode(proof.code_hash))
                        .expect("serialize code hash"),
                );
                if let Some(tx_hash) = proof.tx_hash {
                    entry.insert(
                        "tx_hash".to_string(),
                        norito::json::to_value(&hex::encode(tx_hash)).expect("serialize tx hash"),
                    );
                }
                norito::json::Value::Object(entry)
            })
            .collect::<Vec<_>>();
        let fastpq_proofs = self
            .fastpq_proofs
            .iter()
            .map(FastpqProofSnapshot::to_json_value)
            .collect::<Vec<_>>();
        let mut root = norito::json::Map::new();
        root.insert(
            "format".to_string(),
            norito::json::to_value(&self.format_label()).expect("serialize format label"),
        );
        root.insert(
            "height".to_string(),
            norito::json::to_value(&self.height).expect("serialize pipeline height"),
        );
        root.insert(
            "block_hash".to_string(),
            norito::json::to_value(&self.block_hash.to_string())
                .expect("serialize pipeline block hash"),
        );
        root.insert("dag".to_string(), dag);
        root.insert("txs".to_string(), norito::json::Value::Array(txs));
        root.insert("proofs".to_string(), norito::json::Value::Array(proofs));
        root.insert(
            "fastpq_proofs".to_string(),
            norito::json::Value::Array(fastpq_proofs),
        );
        norito::json::Value::Object(root)
    }
    /// Encode the sidecar into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails (e.g., compression/header mismatch).
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.len() > MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES {
            return Err(norito::Error::Message(
                "certified lane block exceeds the merge source envelope byte limit".to_owned(),
            ));
        }
        Ok(bytes)
    }
}
/// Known metadata format variants for pipeline recovery sidecars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum PipelineRecoveryFormat {
    #[codec(index = 0)]
    /// Sidecars anchored to a specific block hash to avoid reuse across forks.
    Current,
}
/// Deterministic DAG summary embedded in pipeline recovery metadata.
#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct PipelineDagSnapshot {
    /// Blake2 hash summarising the DAG structure for the block.
    pub fingerprint: [u8; 32],
    /// Number of unique DAG keys observed during block construction.
    pub key_count: u32,
}
/// Transaction access summary persisted for pipeline recovery/replay.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PipelineTxSnapshot {
    /// Transaction hash to correlate with block entries.
    pub hash: HashOf<TransactionEntrypoint>,
    /// Optional sampled state keys read during execution.
    pub reads: Vec<String>,
    /// Optional sampled state keys written during execution.
    pub writes: Vec<String>,
    /// Total number of state keys read during execution.
    pub read_count: u32,
    /// Total number of state keys written during execution.
    pub write_count: u32,
}
impl PipelineTxSnapshot {
    /// Create a compact tx access summary without embedding the full key lists.
    #[must_use]
    pub fn compact(
        hash: HashOf<TransactionEntrypoint>,
        read_count: usize,
        write_count: usize,
    ) -> Self {
        Self {
            hash,
            reads: Vec::new(),
            writes: Vec::new(),
            read_count: u32::try_from(read_count).unwrap_or(u32::MAX),
            write_count: u32::try_from(write_count).unwrap_or(u32::MAX),
        }
    }
    /// Total number of read keys represented by this snapshot.
    #[must_use]
    pub fn read_count(&self) -> u32 {
        self.read_count
    }
    /// Total number of write keys represented by this snapshot.
    #[must_use]
    pub fn write_count(&self) -> u32 {
        self.write_count
    }
}
/// ZK proof artifacts captured alongside pipeline metadata.
#[derive(Debug, Clone, Encode, Decode)]
pub struct PipelineProofSnapshot {
    /// Backend identifier for the proof format.
    pub backend: String,
    /// Raw proof bytes recorded for the trace.
    pub proof: Vec<u8>,
    /// Code hash of the executed program producing the trace.
    pub code_hash: [u8; 32],
    /// Optional transaction hash associated with the trace.
    #[norito(default)]
    pub tx_hash: Option<[u8; 32]>,
}
/// FASTPQ proof artifact captured after block commit for local AXT packaging and audits.
///
/// Sidecar persistence stores compact metadata-only snapshots to keep per-block
/// recovery metadata bounded under sustained throughput. Full proof payloads
/// should be exported through dedicated proof artifact paths rather than folded
/// into the pipeline sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct FastpqProofSnapshot {
    /// Block height the proof belongs to.
    pub height: u64,
    /// Block hash the proof belongs to.
    pub block_hash: HashOf<BlockHeader>,
    /// Transaction entrypoint or execution-witness entry hash proven by this batch.
    pub entry_hash: Hash,
    /// Zero-based batch position in the committed execution witness.
    pub batch_index: u32,
    /// FASTPQ parameter set used to produce the proof.
    pub parameter: String,
    /// Number of transitions carried by `batch`.
    pub transition_count: u32,
    /// Batch trace commitment proven by `proof`.
    pub trace_commitment: Hash,
    /// Stable digest of the Norito-encoded FASTPQ proof bytes.
    pub proof_digest: Hash,
    /// Canonical transition batch proven by the FASTPQ proof, or compact public inputs for sidecars.
    pub batch: fastpq_prover::TransitionBatch,
    /// Norito-encoded FASTPQ proof bytes; empty when persisted as sidecar metadata.
    pub proof: Vec<u8>,
}
impl FastpqProofSnapshot {
    /// Create a compact sidecar snapshot from a proven batch without embedding
    /// transition rows or proof bytes.
    #[must_use]
    pub fn compact_from_batch(
        height: u64,
        block_hash: HashOf<BlockHeader>,
        entry_hash: Hash,
        batch_index: u32,
        batch: &fastpq_prover::TransitionBatch,
        trace_commitment: Hash,
        proof_digest: Hash,
    ) -> Self {
        let transition_count = u32::try_from(batch.transitions.len()).unwrap_or(u32::MAX);
        let compact_batch =
            fastpq_prover::TransitionBatch::new(batch.parameter.clone(), batch.public_inputs);
        Self {
            height,
            block_hash,
            entry_hash,
            batch_index,
            parameter: batch.parameter.clone(),
            transition_count,
            trace_commitment,
            proof_digest,
            batch: compact_batch,
            proof: Vec::new(),
        }
    }
    /// Return a bounded sidecar representation while retaining proof identity.
    #[must_use]
    pub fn compact_for_sidecar(&self) -> Self {
        let compact_batch = fastpq_prover::TransitionBatch::new(
            self.batch.parameter.clone(),
            self.batch.public_inputs,
        );
        Self {
            height: self.height,
            block_hash: self.block_hash,
            entry_hash: self.entry_hash,
            batch_index: self.batch_index,
            parameter: self.parameter.clone(),
            transition_count: self.transition_count,
            trace_commitment: self.trace_commitment,
            proof_digest: self.proof_digest,
            batch: compact_batch,
            proof: Vec::new(),
        }
    }
    /// Return `true` when both snapshots describe the same proof attachment.
    #[must_use]
    pub fn same_attachment(&self, other: &Self) -> bool {
        self.entry_hash == other.entry_hash
            && self.batch_index == other.batch_index
            && self.proof_digest == other.proof_digest
    }
    /// Decode the embedded FASTPQ proof.
    ///
    /// # Errors
    ///
    /// Returns a Norito decode error when the proof bytes are malformed.
    pub fn decode_proof(&self) -> Result<fastpq_prover::Proof, norito::Error> {
        norito::decode_from_bytes(&self.proof)
    }
    /// Convert this FASTPQ proof snapshot to the JSON object used by recovery endpoints.
    #[must_use]
    pub fn to_json_value(&self) -> JsonValue {
        let mut entry = norito::json::Map::new();
        entry.insert(
            "entry_hash".to_string(),
            norito::json::to_value(&self.entry_hash.to_string()).expect("serialize entry hash"),
        );
        entry.insert(
            "batch_index".to_string(),
            norito::json::to_value(&self.batch_index).expect("serialize batch index"),
        );
        entry.insert(
            "parameter".to_string(),
            norito::json::to_value(&self.parameter).expect("serialize parameter"),
        );
        entry.insert(
            "transition_count".to_string(),
            norito::json::to_value(&self.transition_count).expect("serialize transition count"),
        );
        entry.insert(
            "trace_commitment".to_string(),
            norito::json::to_value(&self.trace_commitment.to_string())
                .expect("serialize trace commitment"),
        );
        entry.insert(
            "proof_digest".to_string(),
            norito::json::to_value(&self.proof_digest.to_string()).expect("serialize proof digest"),
        );
        entry.insert(
            "batch".to_string(),
            norito::json::to_value(
                &BASE64_STANDARD
                    .encode(norito::to_bytes(&self.batch).expect("encode FASTPQ batch")),
            )
            .expect("serialize FASTPQ batch"),
        );
        entry.insert(
            "proof".to_string(),
            norito::json::to_value(&BASE64_STANDARD.encode(&self.proof))
                .expect("serialize FASTPQ proof"),
        );
        norito::json::Value::Object(entry)
    }
    /// Package this snapshot as an AXT proof blob.
    ///
    /// The snapshot batch must have been bound before proving with the exact
    /// manifest root, DA commitment, committed amount, and expiry supplied to
    /// this export path. This method only compares and packages those values;
    /// it never repairs legacy proof metadata. Pre-binding snapshots therefore
    /// require reproving before they can be exported as AXT proof blobs.
    ///
    /// # Errors
    ///
    /// Returns a FASTPQ prover error when the embedded proof is malformed, the
    /// batch was not already AXT-bound before proof generation, or supplied
    /// outer metadata differs from its proof-bound value.
    pub fn to_axt_proof_blob(
        &self,
        manifest_root: [u8; 32],
        da_commitment: Option<[u8; 32]>,
        expiry_slot: Option<u64>,
    ) -> fastpq_prover::Result<iroha_data_model::nexus::ProofBlob> {
        let proof = norito::decode_from_bytes(&self.proof)
            .map_err(|source| fastpq_prover::Error::AxtProofPayloadDecode { source })?;
        fastpq_prover::axt_proof_blob_from_bound_batch(
            &self.batch,
            proof,
            manifest_root,
            da_commitment,
            expiry_slot,
        )
    }
}
/// Known metadata format variants for certified standalone lane blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum CertifiedLaneBlockArtifactFormat {
    #[codec(index = 0)]
    /// Standalone lane block certified by prepare and commit lane-local QCs.
    Current,
}
/// Persisted standalone lane block certification artifact.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedLaneBlockArtifact {
    /// Schema / evolution tag for the certified lane block format.
    pub format: CertifiedLaneBlockArtifactFormat,
    /// Lane block proposal that defines the certified descriptor and payload subject.
    pub proposal: LaneBlockProposalV1,
    /// Prepare QC for the lane block proposal.
    pub prepare_qc: LaneBlockQcV1,
    /// Commit QC for the lane block proposal.
    pub commit_qc: LaneBlockQcV1,
    /// Proof-of-possession material for every signer selected by either QC.
    pub signer_pops: BTreeMap<PublicKey, Vec<u8>>,
}
impl CertifiedLaneBlockArtifact {
    const FORMAT_LABEL: &'static str = "lane.certified_block";
    /// Construct a certified lane block artifact using the current schema.
    #[must_use]
    pub(crate) fn new(
        session: crate::lane_consensus::CommittedLaneBlockSession,
        signer_pops: BTreeMap<PublicKey, Vec<u8>>,
    ) -> Self {
        Self {
            format: CertifiedLaneBlockArtifactFormat::Current,
            proposal: session.proposal,
            prepare_qc: session.prepare_qc,
            commit_qc: session.commit_qc,
            signer_pops,
        }
    }
    /// Return the human-readable format tag describing the artifact payload.
    #[must_use]
    pub fn format_label(&self) -> &'static str {
        match self.format {
            CertifiedLaneBlockArtifactFormat::Current => Self::FORMAT_LABEL,
        }
    }
    /// Encode the artifact into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails or the complete certified source
    /// exceeds its protocol-reserved merge envelope.
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.len() > MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES {
            return Err(norito::Error::Message(
                "certified lane block exceeds the merge source envelope byte limit".to_owned(),
            ));
        }
        Ok(bytes)
    }
}
/// Bounded durable head for one active lane's latest certified session.
///
/// The complete artifact is retained so a frontier publication that survives a
/// crash can repair the exact ordinary progress-pair entry without scanning
/// lane-local history.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct LatestCertifiedLaneBlockFrontierV1 {
    version: u16,
    artifact: CertifiedLaneBlockArtifact,
    integrity_hash: Hash,
}
#[derive(Debug)]
struct LatestCertifiedLaneBlockFrontierRead {
    frontier: LatestCertifiedLaneBlockFrontierV1,
    snapshot: StableSidecarRead,
}
impl LatestCertifiedLaneBlockFrontierV1 {
    fn new(artifact: CertifiedLaneBlockArtifact) -> Option<Self> {
        let mut frontier = Self {
            version: LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_VERSION,
            artifact,
            integrity_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        frontier.integrity_hash = frontier.computed_integrity_hash()?;
        Some(frontier)
    }
    fn computed_integrity_hash(&self) -> Option<Hash> {
        let mut canonical = self.clone();
        canonical.integrity_hash = Hash::prehashed([0; Hash::LENGTH]);
        norito::encode_canonical(&canonical).ok().map(|bytes| {
            Hash::new_from_chunks(&[LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_DIGEST_DOMAIN, &bytes])
        })
    }
    fn ordinary_height(&self) -> u64 {
        self.artifact.proposal.descriptor.lane_block_height
    }
}
/// Known metadata formats for lane-owned executable payloads and view proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub(crate) enum AutonomousLaneBlockArtifactFormat {
    #[codec(index = 0)]
    /// Canonical executable payload followed by a contiguous NewView proof chain.
    Current,
}
/// Durable lane-owned payload and authenticated view-transition chain.
///
/// Unlike [`LaneBlockArtifact`], this artifact does not depend on a global block
/// body. Its payload is producer-signed, its availability certificate remains
/// bound to the immutable origin proposal, and every later synthetic view
/// cursor is authorized by a lane-committee aggregate certificate carrying
/// restart-verifiable PoPs.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub(crate) struct AutonomousLaneBlockArtifact {
    /// Schema/evolution tag.
    pub(crate) format: AutonomousLaneBlockArtifactFormat,
    /// View-neutral executable payload authenticated at its origin view.
    pub(crate) executable_payload: LaneExecutablePayloadV1,
    /// Origin-Prepare quorum proof that READY signers retained the exact payload.
    pub(crate) availability_certificate: Option<DurableLanePayloadAvailabilityCertificateV1>,
    /// Latest quorum-signed restart checkpoint after older transitions were
    /// compacted away.
    pub(crate) view_checkpoint: Option<DurableLaneBlockViewCheckpointV1>,
    /// Contiguous certificates from the origin proposal, or from the retained
    /// checkpoint target, to the current view.
    pub(crate) new_view_certificates: Vec<DurableLaneBlockNewViewCertificateV1>,
}
impl AutonomousLaneBlockArtifact {
    fn new(executable_payload: LaneExecutablePayloadV1) -> Self {
        Self {
            format: AutonomousLaneBlockArtifactFormat::Current,
            executable_payload,
            availability_certificate: None,
            view_checkpoint: None,
            new_view_certificates: Vec::new(),
        }
    }
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.len() > MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES {
            return Err(norito::Error::Message(
                "autonomous lane block exceeds the merge source byte limit".to_owned(),
            ));
        }
        Ok(bytes)
    }
}
/// Authenticated pointer to the latest attempt at one lane-local height.
///
/// Every attempt, including the first, is immutable in its versioned
/// proposal-height namespace. A later global proposal height may reuse that
/// lane-local height only after the prior attempt is durably retired and its
/// Queue release is complete. This pointer is published last and reconstructed
/// from the bounded attempt inventory at startup.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLaneBlockLatestAttemptV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    lane_block_height: u64,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
}
impl AutonomousLaneBlockLatestAttemptV1 {
    const VERSION: u16 = 1;
    fn from_payload(payload: &LaneExecutablePayloadV1) -> Self {
        let descriptor = &payload.origin_proposal.descriptor;
        Self {
            version: Self::VERSION,
            network_id: payload.network_id,
            epoch: payload.epoch,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            previous_lane_block_height: descriptor.previous_lane_block_height,
            previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
            lane_block_height: descriptor.lane_block_height,
            origin_proposal_hash: payload.origin_proposal.proposal_hash,
            executable_payload_hash: payload.payload_hash,
        }
    }
    fn matches_payload(&self, payload: &LaneExecutablePayloadV1) -> bool {
        self.version == Self::VERSION && self == &Self::from_payload(payload)
    }
}
/// Canonical identity and monotonic value stored for one Kura-root process generation.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleProcessGenerationBodyV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    local_peer_id: PeerId,
    generation: u64,
}
impl AutonomousLifecycleProcessGenerationBodyV1 {
    const VERSION: u16 = 1;
    fn new(
        network_id: iroha_data_model::NetworkId,
        local_peer_id: PeerId,
        generation: u64,
    ) -> Result<Self, &'static str> {
        let body = Self {
            version: Self::VERSION,
            network_id,
            local_peer_id,
            generation,
        };
        body.validate_structure()?;
        Ok(body)
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        if self.version != Self::VERSION || self.generation == 0 {
            return Err(
                "autonomous lifecycle process generation has an unsupported version or zero generation",
            );
        }
        if self.network_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err("autonomous lifecycle process generation has a zero network identity");
        }
        if self.local_peer_id.public_key().try_algorithm().is_err() {
            return Err(
                "autonomous lifecycle process generation has an invalid local key identity",
            );
        }
        Ok(())
    }
    fn canonical_hash(&self) -> Result<Hash, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        Ok(Hash::new_from_chunks(&[
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_HASH_DOMAIN,
            &encoded,
        ]))
    }
}
/// Self-hashed first-release process-generation record for one exclusive Kura root.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleProcessGenerationRecordV1 {
    body: AutonomousLifecycleProcessGenerationBodyV1,
    record_hash: Hash,
}
impl AutonomousLifecycleProcessGenerationRecordV1 {
    fn new(
        network_id: iroha_data_model::NetworkId,
        local_peer_id: PeerId,
        generation: u64,
    ) -> Result<Self, &'static str> {
        let body =
            AutonomousLifecycleProcessGenerationBodyV1::new(network_id, local_peer_id, generation)?;
        let record_hash = body.canonical_hash().map_err(
            |_| "autonomous lifecycle process-generation body is not canonically encodable",
        )?;
        Ok(Self { body, record_hash })
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        self.body.validate_structure()?;
        let expected_hash = self.body.canonical_hash().map_err(
            |_| "autonomous lifecycle process-generation body is not canonically encodable",
        )?;
        if self.record_hash != expected_hash {
            return Err("autonomous lifecycle process-generation record hash is invalid");
        }
        Ok(())
    }
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.is_empty() || bytes.len() > AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES {
            return Err(norito::Error::Message(
                "autonomous lifecycle process-generation record exceeds its hard byte limit"
                    .to_owned(),
            ));
        }
        Ok(bytes)
    }
}
/// Opaque proof that this Kura instance durably claimed one exact process generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutonomousLifecycleProcessGenerationClaim {
    store_root: PathBuf,
    network_id: iroha_data_model::NetworkId,
    local_peer_id: PeerId,
    generation: u64,
    record_hash: Hash,
}
impl AutonomousLifecycleProcessGenerationClaim {
    /// Return the non-zero durable generation owned by this process.
    #[must_use]
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }
    /// Return the exact network identity bound into the durable claim.
    #[must_use]
    pub(crate) const fn network_id(&self) -> iroha_data_model::NetworkId {
        self.network_id
    }
    /// Return the exact local public-key identity bound into the durable claim.
    #[must_use]
    pub(crate) fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }
}
/// Stable, versioned encoding of a fixed-width canonical identity projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleCanonicalIdentityV1 {
    domain: u8,
    kind: u8,
    word0: u64,
    word1: u64,
    word2: u64,
    word3: u64,
}
impl AutonomousLifecycleCanonicalIdentityV1 {
    fn from_production(value: CanonicalIdentityProjection) -> Self {
        Self {
            domain: value.domain,
            kind: value.kind,
            word0: value.word0,
            word1: value.word1,
            word2: value.word2,
            word3: value.word3,
        }
    }
    fn to_production(self) -> CanonicalIdentityProjection {
        CanonicalIdentityProjection {
            domain: self.domain,
            kind: self.kind,
            word0: self.word0,
            word1: self.word1,
            word2: self.word2,
            word3: self.word3,
        }
    }
}
/// Stable Queue/QueuePlan portion of one autonomous lifecycle projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleQueueProjectionV1 {
    plan_state: u8,
    selected_count: u64,
    reservation_state: u8,
}
/// Stable Kura and carrier-evidence portion of one lifecycle projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleCarrierProjectionV1 {
    kura_active: u128,
    execution_input_durable: u128,
    ready_qc_durable: bool,
}
/// Stable volatile-custody portion of one autonomous lifecycle projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleSessionProjectionV1 {
    bodies: u128,
    ready_authorized: u128,
    crashed: u128,
    producer_alive: bool,
}
/// Stable monotonic-history portion of one autonomous lifecycle projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleHistoryProjectionV1 {
    ever_queue_plan_v1: bool,
    ever_reservation_v1: bool,
    ever_execution_input_durable: u128,
    ever_ready_authorized: u128,
    ready_signed: u128,
    ever_ready_qc_durable: bool,
    reservation_committed_prefix: u64,
    queue_plan_tombstoned_prefix: u64,
    reservation_commit_forgotten_prefix: u64,
    pending_high_water: u64,
    released_high_water: u64,
}
/// Stable decision/application portion of one autonomous lifecycle projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleDecisionProjectionV1 {
    lane_commit_scope: AutonomousLifecycleCanonicalIdentityV1,
    release_scope: AutonomousLifecycleCanonicalIdentityV1,
    lane_commit_owner: u128,
    release_owner: u128,
    wsv_committed: bool,
    application_count: u8,
    applied_by: u128,
}
/// Stable release-progress portion of one autonomous lifecycle projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleReleaseProjectionV1 {
    kura_retired: bool,
    pending_prefix: u64,
    released_prefix: u64,
    fifo_restored: bool,
}
/// Versioned durable mirror of the complete first-release safety projection.
///
/// The persistence layout deliberately does not encode the internal
/// refinement struct. Conversion is field-for-field and every decoded value
/// must pass the current production state or transition kernel before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLifecycleStableStateV1 {
    version: u16,
    validator_count: u8,
    producer: u128,
    producer_selected_owner: u128,
    replicated_carrier_owners: u128,
    payload_binding_a: u128,
    binding_a: AutonomousLifecycleCanonicalIdentityV1,
    queue: AutonomousLifecycleQueueProjectionV1,
    carrier: AutonomousLifecycleCarrierProjectionV1,
    session: AutonomousLifecycleSessionProjectionV1,
    history: AutonomousLifecycleHistoryProjectionV1,
    decision: AutonomousLifecycleDecisionProjectionV1,
    release: AutonomousLifecycleReleaseProjectionV1,
}
impl AutonomousLifecycleStableStateV1 {
    const VERSION: u16 = 1;
    /// Reserve the exact fixed-width terminal-state payload for Pending outcomes.
    /// This deliberately unsupported production projection gives Pending and
    /// Complete identical framed lengths in the first-release persistence layout.
    const fn terminal_outcome_pending_reservation() -> Self {
        const ZERO_IDENTITY: AutonomousLifecycleCanonicalIdentityV1 =
            AutonomousLifecycleCanonicalIdentityV1 {
                domain: 0,
                kind: 0,
                word0: 0,
                word1: 0,
                word2: 0,
                word3: 0,
            };
        Self {
            version: 0,
            validator_count: 0,
            producer: 0,
            producer_selected_owner: 0,
            replicated_carrier_owners: 0,
            payload_binding_a: 0,
            binding_a: ZERO_IDENTITY,
            queue: AutonomousLifecycleQueueProjectionV1 {
                plan_state: 0,
                selected_count: 0,
                reservation_state: 0,
            },
            carrier: AutonomousLifecycleCarrierProjectionV1 {
                kura_active: 0,
                execution_input_durable: 0,
                ready_qc_durable: false,
            },
            session: AutonomousLifecycleSessionProjectionV1 {
                bodies: 0,
                ready_authorized: 0,
                crashed: 0,
                producer_alive: false,
            },
            history: AutonomousLifecycleHistoryProjectionV1 {
                ever_queue_plan_v1: false,
                ever_reservation_v1: false,
                ever_execution_input_durable: 0,
                ever_ready_authorized: 0,
                ready_signed: 0,
                ever_ready_qc_durable: false,
                reservation_committed_prefix: 0,
                queue_plan_tombstoned_prefix: 0,
                reservation_commit_forgotten_prefix: 0,
                pending_high_water: 0,
                released_high_water: 0,
            },
            decision: AutonomousLifecycleDecisionProjectionV1 {
                lane_commit_scope: ZERO_IDENTITY,
                release_scope: ZERO_IDENTITY,
                lane_commit_owner: 0,
                release_owner: 0,
                wsv_committed: false,
                application_count: 0,
                applied_by: 0,
            },
            release: AutonomousLifecycleReleaseProjectionV1 {
                kura_retired: false,
                pending_prefix: 0,
                released_prefix: 0,
                fifo_restored: false,
            },
        }
    }
    fn is_terminal_outcome_pending_reservation(self) -> bool {
        self == Self::terminal_outcome_pending_reservation()
    }
    /// Convert one complete checked production projection into its durable
    /// first-release layout.
    #[must_use]
    pub(crate) fn from_production(value: ProductionInFlightFirstReleaseStateProjection) -> Self {
        Self {
            version: Self::VERSION,
            validator_count: value.validator_count,
            producer: value.producer,
            producer_selected_owner: value.producer_selected_owner,
            replicated_carrier_owners: value.replicated_carrier_owners,
            payload_binding_a: value.payload_binding_a,
            binding_a: AutonomousLifecycleCanonicalIdentityV1::from_production(value.binding_a),
            queue: AutonomousLifecycleQueueProjectionV1 {
                plan_state: value.queue.plan_state,
                selected_count: value.queue.selected_count,
                reservation_state: value.queue.reservation_state,
            },
            carrier: AutonomousLifecycleCarrierProjectionV1 {
                kura_active: value.carrier.kura_active,
                execution_input_durable: value.carrier.execution_input_durable,
                ready_qc_durable: value.carrier.ready_qc_durable,
            },
            session: AutonomousLifecycleSessionProjectionV1 {
                bodies: value.session.bodies,
                ready_authorized: value.session.ready_authorized,
                crashed: value.session.crashed,
                producer_alive: value.session.producer_alive,
            },
            history: AutonomousLifecycleHistoryProjectionV1 {
                ever_queue_plan_v1: value.history.ever_queue_plan_v1,
                ever_reservation_v1: value.history.ever_reservation_v1,
                ever_execution_input_durable: value.history.ever_execution_input_durable,
                ever_ready_authorized: value.history.ever_ready_authorized,
                ready_signed: value.history.ready_signed,
                ever_ready_qc_durable: value.history.ever_ready_qc_durable,
                reservation_committed_prefix: value.history.reservation_committed_prefix,
                queue_plan_tombstoned_prefix: value.history.queue_plan_tombstoned_prefix,
                reservation_commit_forgotten_prefix: value
                    .history
                    .reservation_commit_forgotten_prefix,
                pending_high_water: value.history.pending_high_water,
                released_high_water: value.history.released_high_water,
            },
            decision: AutonomousLifecycleDecisionProjectionV1 {
                lane_commit_scope: AutonomousLifecycleCanonicalIdentityV1::from_production(
                    value.decision.lane_commit_scope,
                ),
                release_scope: AutonomousLifecycleCanonicalIdentityV1::from_production(
                    value.decision.release_scope,
                ),
                lane_commit_owner: value.decision.lane_commit_owner,
                release_owner: value.decision.release_owner,
                wsv_committed: value.decision.wsv_committed,
                application_count: value.decision.application_count,
                applied_by: value.decision.applied_by,
            },
            release: AutonomousLifecycleReleaseProjectionV1 {
                kura_retired: value.release.kura_retired,
                pending_prefix: value.release.pending_prefix,
                released_prefix: value.release.released_prefix,
                fifo_restored: value.release.fifo_restored,
            },
        }
    }
    fn to_production(self) -> Option<ProductionInFlightFirstReleaseStateProjection> {
        (self.version == Self::VERSION).then_some(ProductionInFlightFirstReleaseStateProjection {
            validator_count: self.validator_count,
            producer: self.producer,
            producer_selected_owner: self.producer_selected_owner,
            replicated_carrier_owners: self.replicated_carrier_owners,
            payload_binding_a: self.payload_binding_a,
            binding_a: self.binding_a.to_production(),
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: self.queue.plan_state,
                selected_count: self.queue.selected_count,
                reservation_state: self.queue.reservation_state,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: self.carrier.kura_active,
                execution_input_durable: self.carrier.execution_input_durable,
                ready_qc_durable: self.carrier.ready_qc_durable,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: self.session.bodies,
                ready_authorized: self.session.ready_authorized,
                crashed: self.session.crashed,
                producer_alive: self.session.producer_alive,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: self.history.ever_queue_plan_v1,
                ever_reservation_v1: self.history.ever_reservation_v1,
                ever_execution_input_durable: self.history.ever_execution_input_durable,
                ever_ready_authorized: self.history.ever_ready_authorized,
                ready_signed: self.history.ready_signed,
                ever_ready_qc_durable: self.history.ever_ready_qc_durable,
                reservation_committed_prefix: self.history.reservation_committed_prefix,
                queue_plan_tombstoned_prefix: self.history.queue_plan_tombstoned_prefix,
                reservation_commit_forgotten_prefix: self
                    .history
                    .reservation_commit_forgotten_prefix,
                pending_high_water: self.history.pending_high_water,
                released_high_water: self.history.released_high_water,
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection {
                lane_commit_scope: self.decision.lane_commit_scope.to_production(),
                release_scope: self.decision.release_scope.to_production(),
                lane_commit_owner: self.decision.lane_commit_owner,
                release_owner: self.decision.release_owner,
                wsv_committed: self.decision.wsv_committed,
                application_count: self.decision.application_count,
                applied_by: self.decision.applied_by,
            },
            release: ProductionInFlightFirstReleaseReleaseProjection {
                kura_retired: self.release.kura_retired,
                pending_prefix: self.release.pending_prefix,
                released_prefix: self.release.released_prefix,
                fifo_restored: self.release.fifo_restored,
            },
        })
    }
}
/// Complete stable discriminator for one prepared production transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLifecycleActionV1 {
    action: u8,
    actor: u128,
    target: u128,
}
/// Durable lifecycle phase for one exact autonomous proposal attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutonomousLifecycleCursorPhaseKindV1 {
    /// A checked transition is durable before its production mutation.
    Prepared,
    /// One process generation owns a stable live projection.
    Live,
    /// A newer process generation owns a durable crash observation.
    Crashed,
    /// Terminal economic ownership is durable for this attempt.
    Terminal,
}
/// Durable lifecycle phase for one exact autonomous proposal attempt.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub(crate) enum AutonomousLifecycleCursorPhaseV1 {
    /// A checked transition is durable before its production mutation.
    #[codec(index = 0)]
    Prepared {
        /// Non-zero process generation that prepared this mutation.
        owner_generation: u64,
        /// Complete stable state before the mutation.
        before: AutonomousLifecycleStableStateV1,
        /// Exact action/actor/target discriminator.
        action: AutonomousLifecycleActionV1,
        /// Complete stable state after the mutation.
        after: AutonomousLifecycleStableStateV1,
    },
    /// One process generation owns a stable live projection.
    #[codec(index = 1)]
    Live {
        /// Non-zero process-owner generation.
        owner_generation: u64,
        /// Complete stable live state.
        projection: AutonomousLifecycleStableStateV1,
    },
    /// A later generation durably observed and projected a prior crash.
    #[codec(index = 2)]
    Crashed {
        /// Prior live owner generation.
        source_generation: u64,
        /// Strictly newer observing process generation.
        observing_generation: u64,
        /// Complete state before the exact Crash action.
        before: AutonomousLifecycleStableStateV1,
        /// Complete state after the exact Crash action.
        after: AutonomousLifecycleStableStateV1,
    },
    /// Terminal economic ownership is durable for this attempt.
    #[codec(index = 3)]
    Terminal {
        /// Process generation that published the terminal cursor.
        owner_generation: u64,
        /// Complete terminal state.
        projection: AutonomousLifecycleStableStateV1,
    },
}
impl AutonomousLifecycleCursorPhaseV1 {
    /// Build a prepared phase only from a production transition accepted by
    /// the current composed first-release transition gate.
    pub(crate) fn prepared(
        owner_generation: u64,
        transition: ProductionInFlightFirstReleaseTransitionProjection,
    ) -> Result<Self, &'static str> {
        if owner_generation == 0 {
            return Err("autonomous lifecycle prepared generation must be non-zero");
        }
        let checked = check_production_in_flight_first_release_transition(transition)
            .ok_or("autonomous lifecycle prepared transition is invalid")?;
        if checked.into_projection() != transition {
            return Err("autonomous lifecycle prepared transition changed during validation");
        }
        Ok(Self::Prepared {
            owner_generation,
            before: AutonomousLifecycleStableStateV1::from_production(transition.before),
            action: AutonomousLifecycleActionV1 {
                action: transition.action,
                actor: transition.actor,
                target: transition.target,
            },
            after: AutonomousLifecycleStableStateV1::from_production(transition.after),
        })
    }
    /// Build a live phase from a valid complete state and non-zero owner generation.
    pub(crate) fn live(
        owner_generation: u64,
        projection: ProductionInFlightFirstReleaseStateProjection,
    ) -> Result<Self, &'static str> {
        if owner_generation == 0 {
            return Err("autonomous lifecycle live generation must be non-zero");
        }
        if !production_in_flight_first_release_state_kernel(projection) {
            return Err("autonomous lifecycle live projection is invalid");
        }
        Ok(Self::Live {
            owner_generation,
            projection: AutonomousLifecycleStableStateV1::from_production(projection),
        })
    }
    /// Build a crash observation; the cursor binding later supplies the exact
    /// local actor used to recheck the production Crash transition.
    pub(crate) fn crashed(
        source_generation: u64,
        observing_generation: u64,
        before: ProductionInFlightFirstReleaseStateProjection,
        after: ProductionInFlightFirstReleaseStateProjection,
    ) -> Result<Self, &'static str> {
        if source_generation == 0 || observing_generation <= source_generation {
            return Err("autonomous lifecycle crash generations are not monotonic");
        }
        if !production_in_flight_first_release_state_kernel(before)
            || !production_in_flight_first_release_state_kernel(after)
        {
            return Err("autonomous lifecycle crash projection is invalid");
        }
        Ok(Self::Crashed {
            source_generation,
            observing_generation,
            before: AutonomousLifecycleStableStateV1::from_production(before),
            after: AutonomousLifecycleStableStateV1::from_production(after),
        })
    }
    /// Transfer ownership of an already durable crash observation to a newer
    /// process generation without fabricating a second Crash transition.
    pub(crate) fn observed_crashed(
        source_generation: u64,
        observing_generation: u64,
        projection: ProductionInFlightFirstReleaseStateProjection,
    ) -> Result<Self, &'static str> {
        if source_generation == 0 || observing_generation <= source_generation {
            return Err("autonomous lifecycle crash-observation generations are not monotonic");
        }
        if !production_in_flight_first_release_state_kernel(projection) {
            return Err("autonomous lifecycle observed-crash projection is invalid");
        }
        let stable = AutonomousLifecycleStableStateV1::from_production(projection);
        Ok(Self::Crashed {
            source_generation,
            observing_generation,
            before: stable,
            after: stable,
        })
    }
    /// Return the process generation that owns this phase.
    #[must_use]
    pub(crate) const fn owner_generation(&self) -> u64 {
        match self {
            Self::Prepared {
                owner_generation, ..
            }
            | Self::Live {
                owner_generation, ..
            }
            | Self::Terminal {
                owner_generation, ..
            } => *owner_generation,
            Self::Crashed {
                observing_generation,
                ..
            } => *observing_generation,
        }
    }
    /// Return the earlier owner generation displaced by a crash observation.
    #[must_use]
    pub(crate) const fn source_generation(&self) -> Option<u64> {
        match self {
            Self::Crashed {
                source_generation, ..
            } => Some(*source_generation),
            Self::Prepared { .. } | Self::Live { .. } | Self::Terminal { .. } => None,
        }
    }
    /// Return the stable phase discriminator without exposing the durable DTO.
    #[must_use]
    pub(crate) const fn kind(&self) -> AutonomousLifecycleCursorPhaseKindV1 {
        match self {
            Self::Prepared { .. } => AutonomousLifecycleCursorPhaseKindV1::Prepared,
            Self::Live { .. } => AutonomousLifecycleCursorPhaseKindV1::Live,
            Self::Crashed { .. } => AutonomousLifecycleCursorPhaseKindV1::Crashed,
            Self::Terminal { .. } => AutonomousLifecycleCursorPhaseKindV1::Terminal,
        }
    }
    /// Return the checked first stable projection carried by this phase.
    pub(crate) fn before_projection(
        &self,
    ) -> Result<ProductionInFlightFirstReleaseStateProjection, &'static str> {
        let state = match self {
            Self::Prepared { before, .. } | Self::Crashed { before, .. } => *before,
            Self::Live { projection, .. } | Self::Terminal { projection, .. } => *projection,
        };
        let projection = state
            .to_production()
            .ok_or("unsupported autonomous lifecycle phase state version")?;
        production_in_flight_first_release_state_kernel(projection)
            .then_some(projection)
            .ok_or("autonomous lifecycle phase state is invalid")
    }
    /// Return the checked second stable projection carried by a prepared or
    /// crashed phase.
    pub(crate) fn after_projection(
        &self,
    ) -> Result<Option<ProductionInFlightFirstReleaseStateProjection>, &'static str> {
        let state = match self {
            Self::Prepared { after, .. } | Self::Crashed { after, .. } => Some(*after),
            Self::Live { .. } | Self::Terminal { .. } => None,
        };
        state
            .map(|state| {
                let projection = state
                    .to_production()
                    .ok_or("unsupported autonomous lifecycle phase state version")?;
                production_in_flight_first_release_state_kernel(projection)
                    .then_some(projection)
                    .ok_or("autonomous lifecycle phase state is invalid")
            })
            .transpose()
    }
    /// Return the checked production transition carried by a prepared phase.
    pub(crate) fn prepared_transition_projection(
        &self,
    ) -> Result<Option<ProductionInFlightFirstReleaseTransitionProjection>, &'static str> {
        let Self::Prepared {
            before,
            action,
            after,
            ..
        } = self
        else {
            return Ok(None);
        };
        let transition = ProductionInFlightFirstReleaseTransitionProjection {
            action: action.action,
            actor: action.actor,
            target: action.target,
            before: before
                .to_production()
                .ok_or("unsupported prepared before-state version")?,
            after: after
                .to_production()
                .ok_or("unsupported prepared after-state version")?,
        };
        let checked = check_production_in_flight_first_release_transition(transition)
            .ok_or("autonomous lifecycle prepared transition is invalid")?;
        (checked.into_projection() == transition)
            .then_some(Some(transition))
            .ok_or("autonomous lifecycle prepared transition is unstable")
    }
    fn states(&self) -> [Option<AutonomousLifecycleStableStateV1>; 2] {
        match self {
            Self::Prepared { before, after, .. } | Self::Crashed { before, after, .. } => {
                [Some(*before), Some(*after)]
            }
            Self::Live { projection, .. } | Self::Terminal { projection, .. } => {
                [Some(*projection), None]
            }
        }
    }
}
/// Versioned, payload-free identity of one ordered Queue reservation group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleReservationGroupV1 {
    version: u16,
    reservation_owner_hash: Hash,
    proposal_identity_hash: Hash,
    reservation_group_hash: Hash,
    reservation_count: u64,
}
impl AutonomousLifecycleReservationGroupV1 {
    const VERSION: u16 = 1;
    fn from_binding(binding: LaneQueueReservationGroupBindingV1) -> Self {
        Self {
            version: Self::VERSION,
            reservation_owner_hash: binding.identity.reservation_owner_hash,
            proposal_identity_hash: binding.identity.proposal_identity_hash,
            reservation_group_hash: binding.reservation_group_hash,
            reservation_count: binding.reservation_count,
        }
    }
    fn matches_binding(&self, binding: LaneQueueReservationGroupBindingV1) -> bool {
        self.version == Self::VERSION && self == &Self::from_binding(binding)
    }
}
/// Complete immutable identity bound into every autonomous lifecycle cursor.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLifecycleAttemptBindingV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    height_context_id: HeightContextId,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    lane_block_height: u64,
    planned_lane_block_height: u64,
    lane_block_view: u64,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
    reservation_group: AutonomousLifecycleReservationGroupV1,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_count: u16,
    producer_index: u16,
    local_validator_index: u16,
}
impl AutonomousLifecycleAttemptBindingV1 {
    const VERSION: u16 = 1;
    /// Derive the exact stable cursor binding from a validated payload, its
    /// Queue-authenticated ordered reservation group, and local committee member.
    pub(crate) fn from_payload(
        height_context_id: HeightContextId,
        planned_lane_block_height: u64,
        payload: &LaneExecutablePayloadV1,
        reservation_group: LaneQueueReservationGroupBindingV1,
        local_validator: &PeerId,
    ) -> Result<Self, &'static str> {
        payload
            .validate(payload.network_id, payload.epoch)
            .map_err(|_| "autonomous lifecycle payload is invalid")?;
        let descriptor = &payload.origin_proposal.descriptor;
        if planned_lane_block_height == 0
            || planned_lane_block_height != descriptor.lane_block_height
        {
            return Err("autonomous lifecycle planned lane height is not exact");
        }
        let expected_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .map_err(|_| "autonomous lifecycle reservation group is invalid")?;
        if reservation_group != expected_group
            || reservation_group.identity.lane_id != descriptor.lane_id
            || reservation_group.identity.dataspace_id != descriptor.dataspace_id
            || reservation_group.identity.lane_incarnation != descriptor.lane_incarnation
            || reservation_group.identity.proposal_height != descriptor.proposal_height
            || reservation_group.identity.lane_block_height != descriptor.lane_block_height
            || reservation_group.identity.lane_block_view != descriptor.lane_block_view
        {
            return Err("autonomous lifecycle reservation group conflicts with the payload");
        }
        let (expected_reservation_owner_hash, expected_proposal_identity_hash) =
            autonomous_lane_reservation_identity_hashes_for_proposal(
                payload.network_id,
                height_context_id,
                payload.epoch,
                &payload.origin_proposal,
                &payload.producer,
            )
            .map_err(|_| "autonomous lifecycle proposal identity cannot be derived")?;
        if reservation_group.identity.reservation_owner_hash != expected_reservation_owner_hash
            || reservation_group.identity.proposal_identity_hash != expected_proposal_identity_hash
        {
            return Err(
                "autonomous lifecycle reservation group has the wrong height-context identity",
            );
        }
        let validator_count = u16::try_from(descriptor.validator_set.len())
            .map_err(|_| "autonomous lifecycle validator count overflows")?;
        if validator_count == 0 || validator_count > 128 {
            return Err("autonomous lifecycle validator count is outside 1..=128");
        }
        if descriptor.validator_count != u32::from(validator_count)
            || descriptor.validator_set_hash != HashOf::new(&descriptor.validator_set)
        {
            return Err("autonomous lifecycle validator-set identity is invalid");
        }
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &payload.producer)
            .and_then(|index| u16::try_from(index).ok())
            .ok_or("autonomous lifecycle producer is outside the validator set")?;
        let local_validator_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == local_validator)
            .and_then(|index| u16::try_from(index).ok())
            .ok_or("autonomous lifecycle local validator is outside the validator set")?;
        if local_validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal) {
            return Err("autonomous lifecycle local validator is not BLS-normal");
        }
        let binding = Self {
            version: Self::VERSION,
            network_id: payload.network_id,
            epoch: payload.epoch,
            height_context_id,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            previous_lane_block_height: descriptor.previous_lane_block_height,
            previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
            lane_block_height: descriptor.lane_block_height,
            planned_lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            origin_proposal_hash: payload.origin_proposal.proposal_hash,
            executable_payload_hash: payload.payload_hash,
            reservation_group: AutonomousLifecycleReservationGroupV1::from_binding(
                reservation_group,
            ),
            validator_set_hash_version: descriptor.validator_set_hash_version,
            validator_set_hash: descriptor.validator_set_hash,
            validator_count,
            producer_index,
            local_validator_index,
        };
        binding.validate_for_payload(payload)?;
        Ok(binding)
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        if self.version != Self::VERSION {
            return Err("unsupported autonomous lifecycle binding version");
        }
        if self.proposal_height == 0
            || self.lane_block_height == 0
            || (self.previous_lane_block_height == 0)
                != self.previous_lane_block_descriptor_hash.is_none()
            || self.previous_lane_block_height >= self.lane_block_height
            || self.planned_lane_block_height != self.lane_block_height
            || self.validator_count == 0
            || self.validator_count > 128
            || self.producer_index >= self.validator_count
            || self.local_validator_index >= self.validator_count
            || self.reservation_group.version != AutonomousLifecycleReservationGroupV1::VERSION
            || self.reservation_group.reservation_count == 0
        {
            return Err("autonomous lifecycle binding has noncanonical geometry");
        }
        let hashes = [
            self.lane_incarnation,
            self.origin_proposal_hash,
            self.executable_payload_hash,
            self.reservation_group.reservation_owner_hash,
            self.reservation_group.proposal_identity_hash,
            self.reservation_group.reservation_group_hash,
        ];
        if self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || hashes
                .iter()
                .any(|hash| hash.as_ref().iter().all(|byte| *byte == 0))
            || self
                .height_context_id
                .0
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .validator_set_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err("autonomous lifecycle binding contains a zero identity");
        }
        Ok(())
    }
    fn validate_for_payload(&self, payload: &LaneExecutablePayloadV1) -> Result<(), &'static str> {
        self.validate_structure()?;
        payload
            .validate(self.network_id, self.epoch)
            .map_err(|_| "autonomous lifecycle payload validation failed")?;
        let descriptor = &payload.origin_proposal.descriptor;
        let group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .map_err(|_| "autonomous lifecycle payload reservation group is invalid")?;
        let (expected_reservation_owner_hash, expected_proposal_identity_hash) =
            autonomous_lane_reservation_identity_hashes_for_proposal(
                payload.network_id,
                self.height_context_id,
                payload.epoch,
                &payload.origin_proposal,
                &payload.producer,
            )
            .map_err(|_| "autonomous lifecycle proposal identity cannot be rederived")?;
        if self.lane_id != descriptor.lane_id
            || self.dataspace_id != descriptor.dataspace_id
            || self.lane_incarnation != descriptor.lane_incarnation
            || self.proposal_height != descriptor.proposal_height
            || self.previous_lane_block_height != descriptor.previous_lane_block_height
            || self.previous_lane_block_descriptor_hash
                != descriptor.previous_lane_block_descriptor_hash
            || self.lane_block_height != descriptor.lane_block_height
            || self.planned_lane_block_height != descriptor.lane_block_height
            || self.lane_block_view != descriptor.lane_block_view
            || self.origin_proposal_hash != payload.origin_proposal.proposal_hash
            || self.executable_payload_hash != payload.payload_hash
            || self.validator_set_hash_version != descriptor.validator_set_hash_version
            || self.validator_set_hash != descriptor.validator_set_hash
            || usize::from(self.validator_count) != descriptor.validator_set.len()
            || descriptor.validator_count != u32::from(self.validator_count)
            || descriptor.validator_set_hash != HashOf::new(&descriptor.validator_set)
            || descriptor
                .validator_set
                .get(usize::from(self.producer_index))
                != Some(&payload.producer)
            || !self.reservation_group.matches_binding(group)
            || self.reservation_group.reservation_owner_hash != expected_reservation_owner_hash
            || self.reservation_group.proposal_identity_hash != expected_proposal_identity_hash
        {
            return Err("autonomous lifecycle binding conflicts with its executable payload");
        }
        Ok(())
    }
    fn local_actor(&self) -> u128 {
        1_u128 << u32::from(self.local_validator_index)
    }
    fn producer_actor(&self) -> u128 {
        1_u128 << u32::from(self.producer_index)
    }
    /// Return the exact active route and incarnation bound into this attempt.
    #[must_use]
    pub(crate) const fn route_identity(&self) -> (LaneId, DataSpaceId, Hash) {
        (self.lane_id, self.dataspace_id, self.lane_incarnation)
    }
    /// Return proposal height, lane-block height, and lane-block view.
    #[must_use]
    pub(crate) const fn attempt_coordinates(&self) -> (u64, u64, u64) {
        (
            self.proposal_height,
            self.lane_block_height,
            self.lane_block_view,
        )
    }
    /// Return the authenticated global height-context identity.
    #[must_use]
    pub(crate) const fn height_context_id(&self) -> HeightContextId {
        self.height_context_id
    }
    /// Return the immutable origin-proposal hash bound into the signed cursor.
    #[must_use]
    pub(crate) const fn origin_proposal_hash(&self) -> Hash {
        self.origin_proposal_hash
    }
    /// Return the canonical executable-payload hash bound into the signed cursor.
    #[must_use]
    pub(crate) const fn executable_payload_hash(&self) -> Hash {
        self.executable_payload_hash
    }
    /// Return the ordered reservation-group hash.
    #[must_use]
    pub(crate) const fn reservation_group_hash(&self) -> Hash {
        self.reservation_group.reservation_group_hash
    }
    /// Return the frozen validator-set hash version, hash, and member count.
    #[must_use]
    pub(crate) const fn validator_set_identity(&self) -> (u16, HashOf<Vec<PeerId>>, u16) {
        (
            self.validator_set_hash_version,
            self.validator_set_hash,
            self.validator_count,
        )
    }
    /// Return the local validator index and its one-hot projection actor.
    #[must_use]
    pub(crate) fn local_validator_identity(&self) -> (u16, u128) {
        (self.local_validator_index, self.local_actor())
    }
    /// Return the frozen producer's one-hot projection actor.
    #[must_use]
    pub(crate) fn producer_actor_projection(&self) -> u128 {
        self.producer_actor()
    }
    /// Reconstruct the exact Queue reservation-group binding.
    #[must_use]
    pub(crate) const fn reservation_group_binding(&self) -> LaneQueueReservationGroupBindingV1 {
        LaneQueueReservationGroupBindingV1 {
            identity: LaneQueueReservationGroupIdentityV1 {
                lane_id: self.lane_id,
                dataspace_id: self.dataspace_id,
                lane_incarnation: self.lane_incarnation,
                proposal_height: self.proposal_height,
                lane_block_height: self.lane_block_height,
                lane_block_view: self.lane_block_view,
                reservation_owner_hash: self.reservation_group.reservation_owner_hash,
                proposal_identity_hash: self.reservation_group.proposal_identity_hash,
            },
            reservation_group_hash: self.reservation_group.reservation_group_hash,
            reservation_count: self.reservation_group.reservation_count,
        }
    }
    fn validate_state(&self, state: AutonomousLifecycleStableStateV1) -> Result<(), &'static str> {
        let projection = state
            .to_production()
            .ok_or("unsupported autonomous lifecycle stable-state version")?;
        let reservation_identity = LaneQueueReservationGroupIdentityV1 {
            lane_id: self.lane_id,
            dataspace_id: self.dataspace_id,
            lane_incarnation: self.lane_incarnation,
            proposal_height: self.proposal_height,
            lane_block_height: self.lane_block_height,
            lane_block_view: self.lane_block_view,
            reservation_owner_hash: self.reservation_group.reservation_owner_hash,
            proposal_identity_hash: self.reservation_group.proposal_identity_hash,
        };
        let group = LaneQueueReservationGroupBindingV1 {
            identity: reservation_identity,
            reservation_group_hash: self.reservation_group.reservation_group_hash,
            reservation_count: self.reservation_group.reservation_count,
        };
        if !production_in_flight_first_release_state_kernel(projection)
            || projection.validator_count != u8::try_from(self.validator_count).unwrap_or(0)
            || projection.producer != self.producer_actor()
            || projection.binding_a
                != canonical_lane_queue_reservation_group_identity_projection(group)
            || projection.queue.selected_count != self.reservation_group.reservation_count
        {
            return Err("autonomous lifecycle stable state conflicts with its attempt binding");
        }
        Ok(())
    }
    fn validate_phase(&self, phase: &AutonomousLifecycleCursorPhaseV1) -> Result<(), &'static str> {
        self.validate_structure()?;
        for state in phase.states().into_iter().flatten() {
            self.validate_state(state)?;
        }
        match phase {
            AutonomousLifecycleCursorPhaseV1::Prepared {
                owner_generation,
                before,
                action,
                after,
            } => {
                if *owner_generation == 0 {
                    return Err("autonomous lifecycle prepared generation is zero");
                }
                let transition = ProductionInFlightFirstReleaseTransitionProjection {
                    action: action.action,
                    actor: action.actor,
                    target: action.target,
                    before: before
                        .to_production()
                        .ok_or("unsupported prepared before-state version")?,
                    after: after
                        .to_production()
                        .ok_or("unsupported prepared after-state version")?,
                };
                let checked = check_production_in_flight_first_release_transition(transition)
                    .ok_or("autonomous lifecycle prepared transition is invalid")?;
                if checked.into_projection() != transition {
                    return Err("autonomous lifecycle prepared transition is unstable");
                }
            }
            AutonomousLifecycleCursorPhaseV1::Live {
                owner_generation,
                projection,
            } => {
                if *owner_generation == 0 {
                    return Err("autonomous lifecycle live generation is zero");
                }
                let projection = projection
                    .to_production()
                    .ok_or("unsupported autonomous lifecycle live projection version")?;
                if projection.session.crashed & self.local_actor() != 0 {
                    return Err("autonomous lifecycle live owner is marked crashed");
                }
            }
            AutonomousLifecycleCursorPhaseV1::Crashed {
                source_generation,
                observing_generation,
                before,
                after,
            } => {
                if *source_generation == 0 || observing_generation <= source_generation {
                    return Err("autonomous lifecycle crash generations are not monotonic");
                }
                let before_projection = before
                    .to_production()
                    .ok_or("unsupported crash before-state version")?;
                let after_projection = after
                    .to_production()
                    .ok_or("unsupported crash after-state version")?;
                if before_projection == after_projection {
                    if after_projection.session.crashed & self.local_actor() == 0 {
                        return Err(
                            "autonomous lifecycle repeated crash observation is not marked crashed",
                        );
                    }
                    return Ok(());
                }
                let transition = ProductionInFlightFirstReleaseTransitionProjection {
                    action: crate::sumeragi::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH,
                    actor: self.local_actor(),
                    target: 0,
                    before: before_projection,
                    after: after_projection,
                };
                let checked = check_production_in_flight_first_release_transition(transition)
                    .ok_or("autonomous lifecycle Crash transition is invalid")?;
                if checked.into_projection() != transition {
                    return Err("autonomous lifecycle Crash transition is unstable");
                }
            }
            AutonomousLifecycleCursorPhaseV1::Terminal {
                owner_generation,
                projection,
            } => {
                if *owner_generation == 0
                    || production_in_flight_first_release_terminal_owner(
                        projection
                            .to_production()
                            .ok_or("unsupported terminal projection version")?,
                    )
                    .is_none()
                {
                    return Err("autonomous lifecycle terminal phase is not terminal");
                }
            }
        }
        Ok(())
    }
}
/// Canonical unsigned portion of one autonomous lifecycle cursor.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLifecycleCursorUnsignedV1 {
    version: u16,
    sequence: u64,
    previous_cursor_hash: Option<Hash>,
    binding: AutonomousLifecycleAttemptBindingV1,
    phase: AutonomousLifecycleCursorPhaseV1,
    signer: PeerId,
}
impl AutonomousLifecycleCursorUnsignedV1 {
    const VERSION: u16 = 1;
    /// Construct the only signable canonical lifecycle body.
    pub(crate) fn new(
        sequence: u64,
        previous_cursor_hash: Option<Hash>,
        binding: AutonomousLifecycleAttemptBindingV1,
        phase: AutonomousLifecycleCursorPhaseV1,
        signer: PeerId,
    ) -> Result<Self, &'static str> {
        let body = Self {
            version: Self::VERSION,
            sequence,
            previous_cursor_hash,
            binding,
            phase,
            signer,
        };
        body.validate_structure()?;
        Ok(body)
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        if self.version != Self::VERSION || self.sequence == 0 {
            return Err("autonomous lifecycle cursor has an unsupported version or zero sequence");
        }
        match (self.sequence, self.previous_cursor_hash) {
            (1, None) => {}
            (1, Some(_)) | (_, None) => {
                return Err("autonomous lifecycle cursor hash chain is not contiguous");
            }
            (_, Some(hash)) if hash.as_ref().iter().all(|byte| *byte == 0) => {
                return Err("autonomous lifecycle previous cursor hash is zero");
            }
            (_, Some(_)) => {}
        }
        self.binding.validate_phase(&self.phase)?;
        if self.signer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal) {
            return Err("autonomous lifecycle cursor signer is not BLS-normal");
        }
        Ok(())
    }
    /// Return the domain-separated hash of this exact canonical unsigned body.
    pub(crate) fn cursor_hash(&self) -> Result<Hash, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        Ok(Hash::new_from_chunks(&[
            AUTONOMOUS_LIFECYCLE_CURSOR_HASH_DOMAIN,
            &encoded,
        ]))
    }
    /// Return the exact domain-separated bytes the adapter must sign.
    pub(crate) fn signing_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        let cursor_hash = self.cursor_hash()?;
        let mut preimage =
            Vec::with_capacity(AUTONOMOUS_LIFECYCLE_CURSOR_SIGNATURE_DOMAIN.len() + Hash::LENGTH);
        preimage.extend_from_slice(AUTONOMOUS_LIFECYCLE_CURSOR_SIGNATURE_DOMAIN);
        preimage.extend_from_slice(cursor_hash.as_ref());
        Ok(preimage)
    }
    /// Finalize a cursor with one exact 96-byte BLS-normal signature and
    /// immediately reverify it against the supplied canonical validator set.
    pub(crate) fn finalize(
        self,
        signature: [u8; 96],
        validator_set: &[PeerId],
    ) -> Result<AutonomousLifecycleCursorV1, &'static str> {
        let cursor_hash = self
            .cursor_hash()
            .map_err(|_| "autonomous lifecycle unsigned body failed canonical encoding")?;
        let cursor = AutonomousLifecycleCursorV1 {
            body: self,
            cursor_hash,
            signature,
        };
        cursor.validate_against_validator_set(validator_set)?;
        Ok(cursor)
    }
}
/// Signed, hash-chained durable lifecycle cursor for one proposal attempt.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLifecycleCursorV1 {
    body: AutonomousLifecycleCursorUnsignedV1,
    cursor_hash: Hash,
    signature: [u8; 96],
}
impl AutonomousLifecycleCursorV1 {
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.is_empty() || bytes.len() > AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES {
            return Err(norito::Error::Message(
                "autonomous lifecycle cursor exceeds its hard byte limit".to_owned(),
            ));
        }
        Ok(bytes)
    }
    fn validate_against_validator_set(&self, validator_set: &[PeerId]) -> Result<(), &'static str> {
        self.validate_signature()?;
        if validator_set.is_empty()
            || validator_set.len() != usize::from(self.body.binding.validator_count)
            || HashOf::new(&validator_set.to_vec()) != self.body.binding.validator_set_hash
            || validator_set.get(usize::from(self.body.binding.local_validator_index))
                != Some(&self.body.signer)
        {
            return Err("autonomous lifecycle cursor signer is outside the exact validator set");
        }
        Ok(())
    }
    fn validate_signature(&self) -> Result<(), &'static str> {
        self.body.validate_structure()?;
        let expected_hash = self
            .body
            .cursor_hash()
            .map_err(|_| "autonomous lifecycle cursor body failed canonical encoding")?;
        if self.cursor_hash != expected_hash {
            return Err("autonomous lifecycle cursor hash does not match its unsigned body");
        }
        let signature = Signature::try_from_bytes(&self.signature)
            .map_err(|_| "autonomous lifecycle cursor signature is malformed")?;
        let preimage = self
            .body
            .signing_preimage()
            .map_err(|_| "autonomous lifecycle cursor signature preimage failed encoding")?;
        signature
            .verify(self.body.signer.public_key(), &preimage)
            .map_err(|_| "autonomous lifecycle cursor signature verification failed")?;
        Ok(())
    }
    fn validate_for_payload(&self, payload: &LaneExecutablePayloadV1) -> Result<(), &'static str> {
        self.body.binding.validate_for_payload(payload)?;
        self.validate_against_validator_set(&payload.origin_proposal.descriptor.validator_set)
    }
    /// Monotonic sequence number of this cursor.
    #[must_use]
    pub(crate) fn sequence(&self) -> u64 {
        self.body.sequence
    }
    /// Hash of this exact cursor's unsigned canonical body.
    #[must_use]
    pub(crate) fn cursor_hash(&self) -> Hash {
        self.cursor_hash
    }
    /// Immutable attempt binding carried by this cursor.
    #[must_use]
    pub(crate) fn binding(&self) -> &AutonomousLifecycleAttemptBindingV1 {
        &self.body.binding
    }
    /// Durable lifecycle phase carried by this cursor.
    #[must_use]
    pub(crate) fn phase(&self) -> &AutonomousLifecycleCursorPhaseV1 {
        &self.body.phase
    }
    /// Return the stable phase kind without exposing persistence DTO fields.
    #[must_use]
    pub(crate) const fn phase_kind(&self) -> AutonomousLifecycleCursorPhaseKindV1 {
        self.body.phase.kind()
    }
    /// Return the process generation that owns the current cursor phase.
    #[must_use]
    pub(crate) const fn owner_generation(&self) -> u64 {
        self.body.phase.owner_generation()
    }
    /// Return the displaced owner for a crash observation, when applicable.
    #[must_use]
    pub(crate) const fn source_generation(&self) -> Option<u64> {
        self.body.phase.source_generation()
    }
    /// Return the checked first production projection carried by the phase.
    pub(crate) fn before_projection(
        &self,
    ) -> Result<ProductionInFlightFirstReleaseStateProjection, &'static str> {
        self.body.phase.before_projection()
    }
    /// Return the checked second production projection carried by the phase.
    pub(crate) fn after_projection(
        &self,
    ) -> Result<Option<ProductionInFlightFirstReleaseStateProjection>, &'static str> {
        self.body.phase.after_projection()
    }
    /// Return the checked production transition carried by a prepared phase.
    pub(crate) fn prepared_transition_projection(
        &self,
    ) -> Result<Option<ProductionInFlightFirstReleaseTransitionProjection>, &'static str> {
        self.body.phase.prepared_transition_projection()
    }
    fn signer_actor(&self) -> u128 {
        self.body.binding.local_actor()
    }
}
/// Queue disposition durably joined to one nonproducer replica retirement.
///
/// This records what Queue's move-only per-hash fence proved at the Kura
/// terminal sink. `ExactOrdinaryFifo` means the observer preserved an already
/// admitted FIFO copy; it does not mean the release protocol restored FIFO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) enum AutonomousLifecycleReplicaQueueDispositionV1 {
    /// Every exact entrypoint was absent from all local Queue owner indexes.
    #[codec(index = 0)]
    StrictQueueAbsent,
    /// Every exact entrypoint retained its byte-identical ordinary FIFO owner.
    #[codec(index = 1)]
    ExactOrdinaryFifo,
}
/// Source-authenticated economic outcome for one exact autonomous attempt.
///
/// The compact source coordinates are never sufficient on their own. Kura
/// reopens the referenced merge entry/carrier or the exact durable retirement
/// and released claims before either publishing or consuming this record.
#[allow(variant_size_differences)] // Fixed V1 Norito fields preserve canonical source hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) enum AutonomousLifecycleTerminalOutcomeSourceV1 {
    /// Canonical WSV application through a globally committed merge carrier.
    #[codec(index = 0)]
    CanonicalCarrier {
        merge_epoch_id: u64,
        merge_entry_hash: HashOf<MergeLedgerEntry>,
        carrier_block_height: u64,
        carrier_block_hash: HashOf<BlockHeader>,
        application_receipt_hash: HashOf<LaneBlockApplicationReceiptArtifact>,
    },
    /// Ordered FIFO restoration after an exact durable losing-slot retirement.
    #[codec(index = 1)]
    RetiredRelease { retirement_hash: Hash },
    /// Local replicated custody ended without using the producer's Queue
    /// reservation/release ownership corridor.
    #[codec(index = 2)]
    RetiredReplicaQueueDisposition {
        retirement_hash: Hash,
        queue_disposition: AutonomousLifecycleReplicaQueueDispositionV1,
    },
}
impl AutonomousLifecycleTerminalOutcomeSourceV1 {
    fn validate_structure(&self) -> Result<(), &'static str> {
        match self {
            Self::CanonicalCarrier {
                merge_epoch_id: _,
                merge_entry_hash,
                carrier_block_height,
                carrier_block_hash,
                application_receipt_hash,
            } => {
                if *carrier_block_height == 0
                    || merge_entry_hash.as_ref().iter().all(|byte| *byte == 0)
                    || carrier_block_hash.as_ref().iter().all(|byte| *byte == 0)
                    || application_receipt_hash
                        .as_ref()
                        .iter()
                        .all(|byte| *byte == 0)
                {
                    return Err(
                        "autonomous lifecycle canonical terminal source has a zero identity",
                    );
                }
            }
            Self::RetiredRelease { retirement_hash }
                if retirement_hash.as_ref().iter().all(|byte| *byte == 0) =>
            {
                return Err("autonomous lifecycle release terminal source has a zero identity");
            }
            Self::RetiredRelease { .. } => {}
            Self::RetiredReplicaQueueDisposition {
                retirement_hash, ..
            } if retirement_hash.as_ref().iter().all(|byte| *byte == 0) => {
                return Err(
                    "autonomous lifecycle replica terminal source has a zero retirement identity",
                );
            }
            Self::RetiredReplicaQueueDisposition { .. } => {}
        }
        Ok(())
    }
    const fn is_canonical_carrier(&self) -> bool {
        matches!(self, Self::CanonicalCarrier { .. })
    }
    const fn is_retired_release(&self) -> bool {
        matches!(self, Self::RetiredRelease { .. })
    }
    const fn replica_queue_disposition(
        &self,
    ) -> Option<AutonomousLifecycleReplicaQueueDispositionV1> {
        match self {
            Self::RetiredReplicaQueueDisposition {
                queue_disposition, ..
            } => Some(*queue_disposition),
            Self::CanonicalCarrier { .. } | Self::RetiredRelease { .. } => None,
        }
    }
}
/// Crash-safe publication stage for one exact terminal outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) enum AutonomousLifecycleTerminalOutcomeStageV1 {
    /// Source evidence is durable and revalidated before Queue ownership moves.
    /// The exact reserved value keeps the on-disk stage width equal to Complete.
    #[codec(index = 0)]
    Pending {
        reserved_terminal: AutonomousLifecycleStableStateV1,
    },
    /// Queue supplied a move-only, exact terminal-owner authorization.
    #[codec(index = 1)]
    Complete {
        terminal: AutonomousLifecycleStableStateV1,
    },
}
/// Hash-protected body of one autonomous lifecycle terminal outcome.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleTerminalOutcomeBodyV1 {
    version: u16,
    binding: AutonomousLifecycleAttemptBindingV1,
    source: AutonomousLifecycleTerminalOutcomeSourceV1,
    stage: AutonomousLifecycleTerminalOutcomeStageV1,
}
/// Durable source/Queue join for terminal ownership of one exact attempt.
///
/// A `Pending` file is intentionally a drain blocker. `Complete` is accepted
/// only after Kura consumes a move-only Queue proof and independently
/// revalidates this file's source. Canonical archive validation may use a
/// revalidated complete carrier outcome when a removed validator cannot sign a
/// new local lifecycle cursor. A local release outcome never substitutes for
/// globally authenticated retirement/drain evidence.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLifecycleTerminalOutcomeV1 {
    body: AutonomousLifecycleTerminalOutcomeBodyV1,
    outcome_hash: Hash,
}
impl AutonomousLifecycleTerminalOutcomeV1 {
    const VERSION: u16 = 1;
    fn pending(
        binding: AutonomousLifecycleAttemptBindingV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
    ) -> Result<Self, &'static str> {
        let body = AutonomousLifecycleTerminalOutcomeBodyV1 {
            version: Self::VERSION,
            binding,
            source,
            stage: AutonomousLifecycleTerminalOutcomeStageV1::Pending {
                reserved_terminal:
                    AutonomousLifecycleStableStateV1::terminal_outcome_pending_reservation(),
            },
        };
        Self::from_body(body)
    }
    fn complete(
        &self,
        terminal: ProductionInFlightFirstReleaseStateProjection,
    ) -> Result<Self, &'static str> {
        if !matches!(
            self.body.stage,
            AutonomousLifecycleTerminalOutcomeStageV1::Pending { .. }
        ) {
            return Err("autonomous lifecycle terminal outcome is not pending");
        }
        let body = AutonomousLifecycleTerminalOutcomeBodyV1 {
            version: Self::VERSION,
            binding: self.body.binding.clone(),
            source: self.body.source.clone(),
            stage: AutonomousLifecycleTerminalOutcomeStageV1::Complete {
                terminal: AutonomousLifecycleStableStateV1::from_production(terminal),
            },
        };
        Self::from_body(body)
    }
    fn from_body(body: AutonomousLifecycleTerminalOutcomeBodyV1) -> Result<Self, &'static str> {
        Self::validate_body(&body)?;
        let encoded = norito::encode_canonical(&body)
            .map_err(|_| "autonomous lifecycle terminal outcome body failed encoding")?;
        let outcome_hash =
            Hash::new_from_chunks(&[AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_HASH_DOMAIN, &encoded]);
        Ok(Self { body, outcome_hash })
    }
    fn validate_body(body: &AutonomousLifecycleTerminalOutcomeBodyV1) -> Result<(), &'static str> {
        if body.version != Self::VERSION {
            return Err("unsupported autonomous lifecycle terminal outcome version");
        }
        body.binding.validate_structure()?;
        body.source.validate_structure()?;
        match body.stage {
            AutonomousLifecycleTerminalOutcomeStageV1::Pending { reserved_terminal } => {
                if !reserved_terminal.is_terminal_outcome_pending_reservation() {
                    return Err(
                        "autonomous lifecycle pending terminal outcome changed its reserved terminal payload",
                    );
                }
            }
            AutonomousLifecycleTerminalOutcomeStageV1::Complete { terminal } => {
                body.binding.validate_state(terminal)?;
                let projection = terminal
                    .to_production()
                    .ok_or("unsupported autonomous lifecycle terminal outcome state version")?;
                let owner = production_in_flight_first_release_terminal_owner(projection)
                    .ok_or("autonomous lifecycle terminal outcome has no terminal owner")?;
                if body.source.is_canonical_carrier()
                    && (!owner.canonical_wsv_owner
                        || owner.ordinary_fifo_owner
                        || !owner.commit_terminal
                        || owner.release_terminal)
                {
                    return Err(
                        "canonical autonomous lifecycle outcome has the wrong terminal owner",
                    );
                }
                if body.source.is_retired_release()
                    && (!owner.ordinary_fifo_owner
                        || owner.canonical_wsv_owner
                        || owner.commit_terminal
                        || !owner.release_terminal)
                {
                    return Err(
                        "release autonomous lifecycle outcome has the wrong terminal owner",
                    );
                }
                if let Some(disposition) = body.source.replica_queue_disposition() {
                    let exact_fifo = disposition
                        == AutonomousLifecycleReplicaQueueDispositionV1::ExactOrdinaryFifo;
                    let expected_reservation_state = if exact_fifo {
                        IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_FIFO_PRESERVED
                    } else {
                        IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_ABSENT
                    };
                    let (_, local_actor) = body.binding.local_validator_identity();
                    if local_actor == body.binding.producer_actor_projection()
                        || projection.decision.release_owner != local_actor
                        || owner.ordinary_fifo_owner != exact_fifo
                        || owner.canonical_wsv_owner
                        || owner.commit_terminal
                        || !owner.release_terminal
                        || projection.queue.reservation_state != expected_reservation_state
                        || projection.release.fifo_restored
                    {
                        return Err(
                            "replica autonomous lifecycle outcome has the wrong terminal Queue disposition",
                        );
                    }
                }
            }
        }
        Ok(())
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        Self::validate_body(&self.body)?;
        let encoded = norito::encode_canonical(&self.body)
            .map_err(|_| "autonomous lifecycle terminal outcome body failed encoding")?;
        let expected =
            Hash::new_from_chunks(&[AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_HASH_DOMAIN, &encoded]);
        if self.outcome_hash != expected {
            return Err("autonomous lifecycle terminal outcome hash does not match its body");
        }
        Ok(())
    }
    fn validate_for_payload(&self, payload: &LaneExecutablePayloadV1) -> Result<(), &'static str> {
        self.validate_structure()?;
        self.body.binding.validate_for_payload(payload)
    }
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.is_empty() || bytes.len() > AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES {
            return Err(norito::Error::Message(
                "autonomous lifecycle terminal outcome exceeds its hard byte limit".to_owned(),
            ));
        }
        Ok(bytes)
    }
    const fn binding(&self) -> &AutonomousLifecycleAttemptBindingV1 {
        &self.body.binding
    }
    fn source(&self) -> AutonomousLifecycleTerminalOutcomeSourceV1 {
        self.body.source.clone()
    }
    #[cfg(test)]
    const fn stage(&self) -> AutonomousLifecycleTerminalOutcomeStageV1 {
        self.body.stage
    }
    fn is_complete(&self) -> bool {
        matches!(
            self.body.stage,
            AutonomousLifecycleTerminalOutcomeStageV1::Complete { .. }
        )
    }
    fn terminal_projection(
        &self,
    ) -> Result<Option<ProductionInFlightFirstReleaseStateProjection>, &'static str> {
        match self.body.stage {
            AutonomousLifecycleTerminalOutcomeStageV1::Pending { .. } => Ok(None),
            AutonomousLifecycleTerminalOutcomeStageV1::Complete { terminal } => terminal
                .to_production()
                .ok_or("unsupported autonomous lifecycle terminal outcome state version")
                .map(Some),
        }
    }
}
/// Move-only Kura proof that one exact canonical source-outcome record is
/// durable for an ordered reservation group.
///
/// The bound record is Pending before first cleanup and may be the identical
/// source-equivalent Complete record on an idempotent live retry. Queue must
/// pair it with independently authenticated ApplyCarrier authority; this proof
/// alone never authorizes canonical ownership or reservation deletion.
#[must_use = "a canonical lifecycle source-outcome authorization must be consumed by Queue"]
pub(crate) struct AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization {
    reservation_group: LaneQueueReservationGroupBindingV1,
    ordered_keys: Vec<LaneQueueReservationKeyV1>,
    source_outcome_hash: Hash,
}
impl AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization {
    /// Consume only when the exact FIFO-ordered key bytes still derive the
    /// bound group. This deliberately exposes no source selector or terminal
    /// projection to callers.
    pub(crate) fn consume_for_queue(
        self,
    ) -> Option<(
        LaneQueueReservationGroupBindingV1,
        Vec<LaneQueueReservationKeyV1>,
        Hash,
    )> {
        let derived =
            lane_queue_reservation_group_binding_from_ordered_keys(self.ordered_keys.iter())
                .ok()?;
        (derived == self.reservation_group
            && self
                .source_outcome_hash
                .as_ref()
                .iter()
                .any(|byte| *byte != 0))
        .then_some((
            self.reservation_group,
            self.ordered_keys,
            self.source_outcome_hash,
        ))
    }
}
/// Complete canonical carrier source-outcome set published before live Queue
/// cleanup. Members remain in canonical merge-lane order and cover the whole
/// execution set. Before first cleanup every member is Pending; a retry after
/// Queue completion may contain source-equivalent Complete members.
#[must_use = "the full canonical carrier source-outcome set must reach v2_apply"]
pub(crate) struct AutonomousLifecycleCanonicalCarrierSourceOutcomePublication {
    entry_hash: HashOf<MergeLedgerEntry>,
    queue_authorizations: Vec<(
        LaneQueueReservationGroupBindingV1,
        AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
    )>,
}
impl AutonomousLifecycleCanonicalCarrierSourceOutcomePublication {
    /// Return the non-authorizing committed-entry identity used to deduplicate
    /// whole-carrier startup reconstruction requests.
    #[must_use]
    pub(crate) const fn entry_hash(&self) -> HashOf<MergeLedgerEntry> {
        self.entry_hash
    }
    /// Consume only for the exact committed entry whose complete execution
    /// set produced these source-outcome records.
    pub(crate) fn consume_for_v2_apply(
        self,
        entry: &MergeLedgerEntry,
    ) -> Option<
        Vec<(
            LaneQueueReservationGroupBindingV1,
            AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
        )>,
    > {
        let expected_count = entry.execution_batch.as_ref()?.lanes.len();
        let mut seen = BTreeSet::new();
        (crate::merge::merge_ledger_entry_hash(entry) == self.entry_hash
            && expected_count != 0
            && self.queue_authorizations.len() == expected_count
            && self
                .queue_authorizations
                .iter()
                .all(|(group, authorization)| {
                    authorization.reservation_group == *group
                        && seen.insert(group.reservation_group_hash)
                }))
        .then_some(self.queue_authorizations)
    }
}
/// Move-only proof that one exact retired-release source-outcome record is durable.
/// Queue must pair this token with the byte-identical release barrier before
/// it can bind terminal evidence to the record hash.
#[must_use = "the release source-outcome authorization must be consumed by Queue"]
pub(crate) struct AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization {
    barrier: LaneQueueReservationReleaseBarrierV1,
    source_outcome_hash: Hash,
}
impl AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization {
    /// Consume only for the exact durable barrier bound into this token.
    pub(crate) fn consume_for_queue(
        self,
        barrier: &LaneQueueReservationReleaseBarrierV1,
    ) -> Option<Hash> {
        (self.barrier == *barrier
            && self
                .source_outcome_hash
                .as_ref()
                .iter()
                .any(|byte| *byte != 0))
        .then_some(self.source_outcome_hash)
    }
}
/// Opaque source-bound input for reconstructing the independent ApplyCarrier
/// authority associated with a canonical Pending outcome.
///
/// `v2_apply` consumes this value, reruns its merge-QC/source-bundle/network and
/// State-membership checks, selects exactly `reservation_group`, and only then
/// may pass a cleanup authority to Queue alongside the separate Pending token.
#[must_use = "canonical carrier recovery input must be independently authenticated"]
pub(crate) struct AutonomousLifecyclePendingCanonicalCarrierRecovery {
    pending_queue_authorizations: Vec<(
        LaneQueueReservationGroupBindingV1,
        AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
    )>,
    complete_reservation_groups: Vec<LaneQueueReservationGroupBindingV1>,
    reference: CertifiedMergeLedgerReference,
    entry: MergeLedgerEntry,
    carrier_block_height: u64,
    carrier_block_hash: HashOf<BlockHeader>,
    expected_network_id: iroha_data_model::NetworkId,
}
impl AutonomousLifecyclePendingCanonicalCarrierRecovery {
    /// Consume the source coordinates only after their compact reference still
    /// exactly identifies the complete entry. This method never returns a
    /// first-release projection or Queue mutation authority.
    pub(crate) fn consume_for_v2_apply(
        self,
    ) -> Option<(
        Vec<(
            LaneQueueReservationGroupBindingV1,
            AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
        )>,
        Vec<LaneQueueReservationGroupBindingV1>,
        CertifiedMergeLedgerReference,
        MergeLedgerEntry,
        u64,
        HashOf<BlockHeader>,
        iroha_data_model::NetworkId,
    )> {
        let mut seen = BTreeSet::new();
        let pending_is_exact = !self.pending_queue_authorizations.is_empty()
            && self
                .pending_queue_authorizations
                .iter()
                .all(|(group, authorization)| {
                    authorization.reservation_group == *group
                        && seen.insert(group.reservation_group_hash)
                });
        let complete_is_disjoint = self
            .complete_reservation_groups
            .iter()
            .all(|group| seen.insert(group.reservation_group_hash));
        let expected_group_count = self
            .entry
            .execution_batch
            .as_ref()
            .map(|batch| batch.lanes.len());
        (pending_is_exact
            && complete_is_disjoint
            && expected_group_count
                == Some(
                    self.pending_queue_authorizations.len()
                        + self.complete_reservation_groups.len(),
                )
            && self.reference.matches_entry(&self.entry)
            && self.carrier_block_height != 0
            && self
                .expected_network_id
                .as_bytes()
                .iter()
                .any(|byte| *byte != 0))
        .then_some((
            self.pending_queue_authorizations,
            self.complete_reservation_groups,
            self.reference,
            self.entry,
            self.carrier_block_height,
            self.carrier_block_hash,
            self.expected_network_id,
        ))
    }
}
/// One bounded, fully validated active-route lifecycle recovery candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutonomousLifecycleAttemptInventoryEntry {
    executable_payload: LaneExecutablePayloadV1,
    cursor: Option<AutonomousLifecycleCursorV1>,
}
impl AutonomousLifecycleAttemptInventoryEntry {
    /// Borrow the exact immutable executable payload retained by Kura.
    #[must_use]
    pub(crate) fn executable_payload(&self) -> &LaneExecutablePayloadV1 {
        &self.executable_payload
    }
    /// Borrow the validated local lifecycle cursor, if one is durable.
    #[must_use]
    pub(crate) fn cursor(&self) -> Option<&AutonomousLifecycleCursorV1> {
        self.cursor.as_ref()
    }
}
/// Move-only exact-file compare-and-swap authority for one lifecycle cursor.
#[must_use = "the exact lifecycle cursor lease must be consumed or deliberately dropped"]
pub(crate) struct AutonomousLifecycleCursorLease {
    path: PathBuf,
    expected_bytes: Option<Vec<u8>>,
    expected_bytes_hash: Option<Hash>,
    sequence: u64,
    cursor_hash: Option<Hash>,
    owner_generation: u64,
    process_generation_record_hash: Hash,
    actor: u128,
    binding: AutonomousLifecycleAttemptBindingV1,
    validator_set: Vec<PeerId>,
}
/// Exact cursor observation plus its single-use compare-and-swap authority.
#[must_use = "the cursor observation contains a move-only mutation lease"]
pub(crate) struct AutonomousLifecycleCursorRead {
    cursor: Option<AutonomousLifecycleCursorV1>,
    lease: AutonomousLifecycleCursorLease,
}
impl AutonomousLifecycleCursorRead {
    /// Borrow the exact cursor observed while minting the lease.
    #[must_use]
    pub(crate) fn cursor(&self) -> Option<&AutonomousLifecycleCursorV1> {
        self.cursor.as_ref()
    }
    /// Consume the observation into its cursor and move-only lease.
    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<AutonomousLifecycleCursorV1>,
        AutonomousLifecycleCursorLease,
    ) {
        (self.cursor, self.lease)
    }
}
/// Authenticated origin of local executable-payload custody before Kura activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[allow(dead_code)] // Reserved for source-specific validators at the audited persistence sinks.
pub(crate) enum AutonomousLifecyclePayloadCustodySourceV1 {
    /// Producer-side custody remains fenced by the exact live Queue reservation group.
    #[codec(index = 0)]
    ProducerQueue,
    /// A losing proposal is retained only to publish its exact durable retirement.
    #[codec(index = 1)]
    LosingRetirement,
    /// Exact payload bytes were reconstructed from a committed canonical carrier.
    #[codec(index = 2)]
    CanonicalCarrierRepair,
    /// The protected live carrier delivered the producer-signed payload locally.
    #[codec(index = 3)]
    ProtectedCarrierReceive,
    /// A fully authenticated historical Prepare/Commit-QC response supplied the payload.
    #[codec(index = 4)]
    HistoricalQcResponse,
    /// A canonical durable historical-recovery record supplied the payload.
    #[codec(index = 5)]
    CanonicalHistoricalRecoveryRecord,
}
/// Canonical identity of the exact losing-slot retirement which supplied
/// payload custody. This is deliberately an internal, encode-only DTO: callers
/// must pass the complete typed retirement to Kura's source-specific validator
/// and cannot choose the resulting evidence digest.
#[derive(Debug, Encode)]
struct AutonomousLifecycleLosingRetirementCustodyEvidenceV1 {
    version: u16,
    height_context_id: HeightContextId,
    retirement_hash: HashOf<AutonomousLaneSlotRetirementV1>,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
}
/// Canonical identity of one QC-authenticated canonical carrier repair.
#[derive(Debug, Encode)]
struct AutonomousLifecycleCanonicalCarrierRepairCustodyEvidenceV1 {
    version: u16,
    height_context_id: HeightContextId,
    height_context_hash: HashOf<HeightContext>,
    block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    execution_commitment: ExecutionCommitment,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
}
/// Canonical identity of one locally protected global carrier delivery.
#[derive(Debug, Encode)]
struct AutonomousLifecycleProtectedCarrierReceiveCustodyEvidenceV1 {
    version: u16,
    height_context_id: HeightContextId,
    height_context_hash: HashOf<HeightContext>,
    locked_round: iroha_data_model::block::consensus_v2::ConsensusRound,
    locked_subject: BlockSubject,
    local_peer: PeerId,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
}
/// Canonical identity of an outstanding-request-bound historical QC response.
#[derive(Debug, Encode)]
struct AutonomousLifecycleHistoricalQcResponseCustodyEvidenceV1 {
    version: u16,
    height_context_id: HeightContextId,
    height_context_hash: HashOf<HeightContext>,
    request_hash: HashOf<LaneHistoricalRecoveryRequestV1>,
    response_hash: HashOf<LaneHistoricalRecoveryResponseV1>,
    responder: PeerId,
    prepare_qc_hash: HashOf<LaneBlockQcV1>,
    commit_qc_hash: HashOf<LaneBlockQcV1>,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
}
/// Canonical identity of the complete State-preflighted historical recovery
/// record which supplied payload custody before its dependent sidecars exist.
#[derive(Debug, Encode)]
struct AutonomousLifecycleCanonicalHistoricalRecoveryCustodyEvidenceV1 {
    version: u16,
    height_context_id: HeightContextId,
    height_context_hash: HashOf<HeightContext>,
    recovery_id: Hash,
    record_hash: HashOf<HistoricalAutonomousLaneRecoveryRecordV1>,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
}
/// Exact typed custody evidence bound into the signed lifecycle bootstrap.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecyclePayloadCustodyBindingV1 {
    version: u16,
    source: AutonomousLifecyclePayloadCustodySourceV1,
    evidence_hash: Hash,
}
impl AutonomousLifecyclePayloadCustodyBindingV1 {
    const VERSION: u16 = 1;
    fn producer_queue(binding: &AutonomousLifecycleAttemptBindingV1) -> Result<Self, &'static str> {
        let encoded = norito::encode_canonical(binding)
            .map_err(|_| "producer Queue custody binding is not canonically encodable")?;
        Ok(Self {
            version: Self::VERSION,
            source: AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue,
            evidence_hash: Hash::new_from_chunks(&[
                AUTONOMOUS_LIFECYCLE_PRODUCER_QUEUE_CUSTODY_HASH_DOMAIN,
                &encoded,
            ]),
        })
    }
    fn authenticated(
        source: AutonomousLifecyclePayloadCustodySourceV1,
        evidence_hash: Hash,
    ) -> Result<Self, &'static str> {
        if source == AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue {
            return Err("non-Queue custody cannot claim producer Queue ownership");
        }
        let binding = Self {
            version: Self::VERSION,
            source,
            evidence_hash,
        };
        binding.validate(None)?;
        Ok(binding)
    }
    fn validate(
        &self,
        attempt: Option<&AutonomousLifecycleAttemptBindingV1>,
    ) -> Result<(), &'static str> {
        if self.version != Self::VERSION
            || self.evidence_hash.as_ref().iter().all(|byte| *byte == 0)
        {
            return Err("autonomous lifecycle custody evidence is malformed");
        }
        if self.source == AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue {
            let attempt = attempt
                .ok_or("producer Queue custody requires its exact autonomous attempt binding")?;
            if attempt.local_actor() != attempt.producer_actor()
                || *self != Self::producer_queue(attempt)?
            {
                return Err("producer Queue custody hash differs from its exact attempt binding");
            }
        }
        Ok(())
    }
}
/// Move-only checked custody authority consumed at initial bootstrap persistence.
#[must_use = "authenticated payload custody must be consumed by its exact lifecycle bootstrap"]
pub(crate) struct AutonomousLifecyclePayloadCustodyAuthorization {
    binding: AutonomousLifecycleAttemptBindingV1,
    custody: AutonomousLifecyclePayloadCustodyBindingV1,
    activate_kura: ProductionInFlightFirstReleaseTransitionProjection,
}
impl AutonomousLifecyclePayloadCustodyAuthorization {
    /// Borrow the exact checked ActivateKura projection callers must sign.
    #[must_use]
    #[allow(dead_code)] // Consumed by source-specific bootstrap adapters.
    pub(crate) const fn activate_kura_projection(
        &self,
    ) -> ProductionInFlightFirstReleaseTransitionProjection {
        self.activate_kura
    }
}
#[allow(variant_size_differences)] // Ephemeral checked Queue facts stay inline and allocation-free.
enum AutonomousLifecycleBootstrapPersistenceAuthentication<'authorization> {
    ProducerQueue {
        height_context_id: HeightContextId,
        validator_count: u8,
        producer: u128,
        reservation_group: LaneQueueReservationGroupBindingV1,
    },
    PayloadCustody(&'authorization AutonomousLifecyclePayloadCustodyAuthorization),
}
/// Canonical signed intent persisted before the first autonomous payload mutation.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleBootstrapBodyV1 {
    version: u16,
    process_generation: u64,
    process_generation_record_hash: Hash,
    executable_payload: LaneExecutablePayloadV1,
    binding: AutonomousLifecycleAttemptBindingV1,
    custody: AutonomousLifecyclePayloadCustodyBindingV1,
    prepared_activate: AutonomousLifecycleCursorV1,
    live_activate: AutonomousLifecycleCursorV1,
}
impl AutonomousLifecycleBootstrapBodyV1 {
    const VERSION: u16 = 1;
    fn new(
        process_generation: &AutonomousLifecycleProcessGenerationClaim,
        executable_payload: LaneExecutablePayloadV1,
        binding: AutonomousLifecycleAttemptBindingV1,
        custody: AutonomousLifecyclePayloadCustodyBindingV1,
        prepared_activate: AutonomousLifecycleCursorV1,
        live_activate: AutonomousLifecycleCursorV1,
    ) -> Result<Self, &'static str> {
        let body = Self {
            version: Self::VERSION,
            process_generation: process_generation.generation,
            process_generation_record_hash: process_generation.record_hash,
            executable_payload,
            binding,
            custody,
            prepared_activate,
            live_activate,
        };
        body.validate_structure()?;
        Ok(body)
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        if self.version != Self::VERSION
            || self.process_generation == 0
            || self
                .process_generation_record_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err("autonomous lifecycle bootstrap has an unsupported version or generation");
        }
        if self
            .executable_payload
            .origin_proposal
            .descriptor
            .lane_block_view
            != 0
        {
            return Err("autonomous lifecycle bootstrap payload must originate at view zero");
        }
        self.binding
            .validate_for_payload(&self.executable_payload)?;
        self.custody.validate(Some(&self.binding))?;
        self.prepared_activate
            .validate_for_payload(&self.executable_payload)?;
        self.live_activate
            .validate_for_payload(&self.executable_payload)?;
        if self.prepared_activate.binding() != &self.binding
            || self.live_activate.binding() != &self.binding
            || self.prepared_activate.owner_generation() != self.process_generation
            || self.live_activate.owner_generation() != self.process_generation
            || self.prepared_activate.sequence() != 1
            || self.prepared_activate.body.previous_cursor_hash.is_some()
            || self.live_activate.sequence() != 2
            || self.live_activate.body.previous_cursor_hash
                != Some(self.prepared_activate.cursor_hash())
            || self.prepared_activate.body.signer != self.live_activate.body.signer
        {
            return Err("autonomous lifecycle bootstrap cursor identity is not contiguous");
        }
        let transition = self
            .prepared_activate
            .prepared_transition_projection()?
            .ok_or("autonomous lifecycle bootstrap first cursor is not Prepared")?;
        if transition.action != IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA
            || transition.actor != self.binding.local_actor()
            || transition.target != 0
            || transition.before.carrier.kura_active & transition.actor != 0
            || transition.after.carrier.kura_active & transition.actor == 0
        {
            return Err(
                "autonomous lifecycle bootstrap does not carry the exact ActivateKura transition",
            );
        }
        if self.live_activate.phase_kind() != AutonomousLifecycleCursorPhaseKindV1::Live
            || self.live_activate.before_projection()? != transition.after
            || self.live_activate.after_projection()?.is_some()
        {
            return Err(
                "autonomous lifecycle bootstrap Live cursor does not complete Prepared ActivateKura",
            );
        }
        Ok(())
    }
    fn canonical_hash(&self) -> Result<Hash, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        Ok(Hash::new_from_chunks(&[
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_HASH_DOMAIN,
            &encoded,
        ]))
    }
    fn signing_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        let body_hash = self.canonical_hash()?;
        let mut preimage = Vec::with_capacity(
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_SIGNATURE_DOMAIN.len() + Hash::LENGTH,
        );
        preimage.extend_from_slice(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_SIGNATURE_DOMAIN);
        preimage.extend_from_slice(body_hash.as_ref());
        Ok(preimage)
    }
}
/// Self-hashed first-release bootstrap artifact retained through every crash boundary.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLifecycleBootstrapV1 {
    body: AutonomousLifecycleBootstrapBodyV1,
    bootstrap_hash: Hash,
    signature: [u8; 96],
}
impl AutonomousLifecycleBootstrapV1 {
    fn from_body(
        body: AutonomousLifecycleBootstrapBodyV1,
        signature: [u8; 96],
    ) -> Result<Self, &'static str> {
        let bootstrap_hash = body
            .canonical_hash()
            .map_err(|_| "autonomous lifecycle bootstrap body is not canonically encodable")?;
        let bootstrap = Self {
            body,
            bootstrap_hash,
            signature,
        };
        bootstrap.validate_structure()?;
        Ok(bootstrap)
    }
    fn validate_structure(&self) -> Result<(), &'static str> {
        self.body.validate_structure()?;
        let expected_hash = self
            .body
            .canonical_hash()
            .map_err(|_| "autonomous lifecycle bootstrap body is not canonically encodable")?;
        if self.bootstrap_hash != expected_hash {
            return Err("autonomous lifecycle bootstrap self-hash is invalid");
        }
        let signature = Signature::try_from_bytes(&self.signature)
            .map_err(|_| "autonomous lifecycle bootstrap signature is malformed")?;
        let preimage = self
            .body
            .signing_preimage()
            .map_err(|_| "autonomous lifecycle bootstrap signature preimage failed encoding")?;
        signature
            .verify(
                self.body.prepared_activate.body.signer.public_key(),
                &preimage,
            )
            .map_err(|_| "autonomous lifecycle bootstrap signature verification failed")?;
        Ok(())
    }
    fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.is_empty() || bytes.len() > AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES {
            return Err(norito::Error::Message(
                "autonomous lifecycle bootstrap exceeds its hard byte limit".to_owned(),
            ));
        }
        Ok(bytes)
    }
}
/// Exact durable crash boundary observed for one signed lifecycle bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutonomousLifecycleBootstrapRecoveryStage {
    /// Only the bootstrap intent is durable; Kura has not accepted the payload.
    BootstrapOnly,
    /// The exact payload and its immutable sidecars are durable.
    PayloadDurable,
    /// The exact signed Prepared ActivateKura cursor is durable.
    PreparedDurable,
    /// The exact signed Live successor is durable; bootstrap deletion remains.
    LiveDurable,
}
/// Move-only authority bound to one exact bootstrap file observation and Kura root.
#[must_use = "bootstrap recovery authority must be authenticated or deliberately dropped"]
pub(crate) struct AutonomousLifecycleBootstrapRecoveryAuthority {
    store_root: PathBuf,
    path: PathBuf,
    expected_bytes: Vec<u8>,
    expected_bytes_hash: Hash,
    process_generation: AutonomousLifecycleProcessGenerationClaim,
    bootstrap: AutonomousLifecycleBootstrapV1,
    stage: AutonomousLifecycleBootstrapRecoveryStage,
}
impl AutonomousLifecycleBootstrapRecoveryAuthority {
    /// Return the structurally observed durable crash boundary.
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn stage(&self) -> AutonomousLifecycleBootstrapRecoveryStage {
        self.stage
    }
    /// Borrow the exact signed payload retained by the bootstrap.
    #[must_use]
    pub(crate) fn executable_payload(&self) -> &LaneExecutablePayloadV1 {
        &self.bootstrap.body.executable_payload
    }
    /// Borrow the exact height-context and attempt binding requiring caller authentication.
    #[must_use]
    pub(crate) fn binding(&self) -> &AutonomousLifecycleAttemptBindingV1 {
        &self.bootstrap.body.binding
    }
    /// Borrow the signature-validated Live successor retained by this bootstrap observation.
    ///
    /// The bootstrap body validation proves that this cursor is the contiguous successor of the
    /// signed Prepared cursor in the same body and names the same immutable attempt.
    #[must_use]
    pub(crate) fn live_cursor(&self) -> &AutonomousLifecycleCursorV1 {
        &self.bootstrap.body.live_activate
    }
    /// Identify which source-specific recovery path must authenticate this bootstrap.
    #[must_use]
    pub(crate) const fn custody_source(&self) -> AutonomousLifecyclePayloadCustodySourceV1 {
        self.bootstrap.body.custody.source
    }
}
include!("autonomous_merge_bundle_support.rs");
/// One source-revalidated Pending terminal outcome ready for startup Queue
/// reconciliation. Every variant is move-only; inventory never exposes raw
/// lifecycle projections or a constructor from caller-selected identities.
#[must_use = "Pending lifecycle recovery authority must be consumed during startup"]
pub(crate) enum AutonomousLifecyclePendingTerminalOutcomeRecovery {
    /// Canonical source plus exact reservation bytes; Queue still requires its
    /// independently authenticated ApplyCarrier cleanup authority.
    Canonical(AutonomousLifecyclePendingCanonicalCarrierRecovery),
    /// Exact retired release plus Kura's existing action-20..23 authority.
    RetiredRelease {
        barrier: LaneQueueReservationReleaseBarrierV1,
        finalization: AutonomousLaneQueueReleaseFinalizationAuthorization,
        source_outcome_authorization: AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization,
    },
}
/// Read-only exact Queue-group coordinates for partitioning Pending startup work.
///
/// This observation carries no terminal source hash, merge entry, release
/// finalization, or Queue mutation authority. The ordered keys let startup
/// distinguish an exact surviving owner prefix from a same-slot,
/// byte-different Queue group before it chooses whether recovery may run ahead
/// of the immutable replay receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutonomousLifecyclePendingReservationGroupObservation {
    binding: LaneQueueReservationGroupBindingV1,
    ordered_keys: Vec<LaneQueueReservationKeyV1>,
}
impl AutonomousLifecyclePendingReservationGroupObservation {
    /// Return the complete order-sensitive reservation-group binding.
    #[must_use]
    pub(crate) const fn binding(&self) -> LaneQueueReservationGroupBindingV1 {
        self.binding
    }
    /// Borrow the exact source-authenticated FIFO-ordered reservation keys.
    #[must_use]
    pub(crate) fn ordered_keys(&self) -> &[LaneQueueReservationKeyV1] {
        &self.ordered_keys
    }
}
/// Source role independently revalidated while proving one expected terminal
/// outcome file still exists at its exact durable coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutonomousLifecycleTerminalOutcomeSourceKind {
    /// Economic effects reached WSV through one committed global carrier.
    CanonicalCarrier,
    /// A losing lane slot durably returned its exact reservations to FIFO.
    RetiredRelease,
    /// A nonproducer replica durably ended local custody without consuming the
    /// producer's reservation or release owner.
    RetiredReplicaQueueDisposition,
}
/// Durable stage independently revalidated for one expected terminal outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutonomousLifecycleTerminalOutcomeDurableStage {
    /// The source is durable but its exact Queue terminal owner is not joined.
    Pending,
    /// The source and exact Queue terminal owner are durably joined.
    Complete,
}
/// Non-authorizing proof that one caller-expected terminal outcome still
/// exists, source-revalidates, and has one exact durable stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AutonomousLifecycleTerminalOutcomeStageObservation {
    binding: LaneQueueReservationGroupBindingV1,
    source_kind: AutonomousLifecycleTerminalOutcomeSourceKind,
    stage: AutonomousLifecycleTerminalOutcomeDurableStage,
}
impl AutonomousLifecycleTerminalOutcomeStageObservation {
    /// Return the exact order-sensitive reservation-group identity.
    #[must_use]
    pub(crate) const fn binding(&self) -> LaneQueueReservationGroupBindingV1 {
        self.binding
    }
    /// Return the independently revalidated terminal source role.
    #[must_use]
    pub(crate) const fn source_kind(&self) -> AutonomousLifecycleTerminalOutcomeSourceKind {
        self.source_kind
    }
    /// Return the exact durable publication stage.
    #[must_use]
    pub(crate) const fn stage(&self) -> AutonomousLifecycleTerminalOutcomeDurableStage {
        self.stage
    }
}
impl AutonomousLifecyclePendingTerminalOutcomeRecovery {
    /// Return the exact non-authorizing reservation groups whose outcome stage
    /// is still Pending and may therefore require Queue mutation.
    ///
    /// Complete canonical members are deliberately excluded. The result is
    /// fallible so callers cannot partition startup work from malformed or
    /// duplicate recovery coordinates.
    #[must_use]
    pub(crate) fn pending_reservation_groups(
        &self,
    ) -> Option<Vec<AutonomousLifecyclePendingReservationGroupObservation>> {
        match self {
            Self::Canonical(recovery) => {
                if recovery.pending_queue_authorizations.is_empty() {
                    return None;
                }
                let mut groups = Vec::new();
                groups
                    .try_reserve_exact(recovery.pending_queue_authorizations.len())
                    .ok()?;
                let mut seen = BTreeSet::new();
                for (group, authorization) in &recovery.pending_queue_authorizations {
                    if authorization.reservation_group != *group
                        || lane_queue_reservation_group_binding_from_ordered_keys(
                            authorization.ordered_keys.iter(),
                        )
                        .ok()
                            != Some(*group)
                        || !seen.insert(group.reservation_group_hash)
                    {
                        return None;
                    }
                    groups.push(AutonomousLifecyclePendingReservationGroupObservation {
                        binding: *group,
                        ordered_keys: authorization.ordered_keys.clone(),
                    });
                }
                Some(groups)
            }
            Self::RetiredRelease { barrier, .. } => {
                let group = lane_queue_reservation_group_binding_from_ordered_keys(
                    barrier.ordered_keys.iter(),
                )
                .ok()?;
                Some(vec![
                    AutonomousLifecyclePendingReservationGroupObservation {
                        binding: group,
                        ordered_keys: barrier.ordered_keys.clone(),
                    },
                ])
            }
        }
    }
    /// Return every already source-validated route/incarnation for independent
    /// comparison with authoritative startup State. Canonical carrier recovery
    /// is all-group and may therefore span multiple routes. This accessor is
    /// deliberately non-authorizing.
    #[must_use]
    pub(crate) fn route_identities(&self) -> Vec<(LaneId, DataSpaceId, Hash)> {
        match self {
            Self::Canonical(recovery) => recovery
                .pending_queue_authorizations
                .iter()
                .map(|(group, _)| group)
                .chain(recovery.complete_reservation_groups.iter())
                .map(|group| {
                    (
                        group.identity.lane_id,
                        group.identity.dataspace_id,
                        group.identity.lane_incarnation,
                    )
                })
                .collect(),
            Self::RetiredRelease { barrier, .. } => vec![(
                barrier.lane_id,
                barrier.dataspace_id,
                barrier.lane_incarnation,
            )],
        }
    }
    /// Count exact Pending outcome files represented by this recovery unit.
    #[must_use]
    pub(crate) fn pending_outcome_count(&self) -> usize {
        match self {
            Self::Canonical(recovery) => recovery.pending_queue_authorizations.len(),
            Self::RetiredRelease { .. } => 1,
        }
    }
    /// Return the source-validated network binding for comparison with the
    /// active height context before Queue mutation.
    #[must_use]
    pub(crate) const fn network_id(&self) -> iroha_data_model::NetworkId {
        match self {
            Self::Canonical(recovery) => recovery.expected_network_id,
            Self::RetiredRelease { barrier, .. } => barrier.network_id,
        }
    }
}
/// Durable state of one autonomous executable-entrypoint owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
enum AutonomousLaneEntrypointClaimStateV1 {
    /// The lane payload exclusively owns the entrypoint.
    #[codec(index = 0)]
    Active,
    /// The exact slot retirement is durable, but Queue still owns the
    /// reservations behind an ordered release barrier.
    #[codec(index = 1)]
    ReleasePending(Hash),
    /// Queue proved its ordered barrier durable, so canonical ownership may
    /// return to ordinary FIFO.
    #[codec(index = 2)]
    Released(Hash),
    /// A nonproducer replica proved one exact local Queue disposition before
    /// advancing this claim. The disposition remains durable across a crash
    /// until the matching terminal outcome reaches `Complete`.
    #[codec(index = 3)]
    ReplicaReleased(Hash, AutonomousLifecycleReplicaQueueDispositionV1),
    /// The replica Queue disposition is joined to its synced terminal outcome;
    /// this self-contained claim may survive lane archive and be replaced by a
    /// successor without reopening retired lane storage.
    #[codec(index = 4)]
    ReplicaReleasedComplete(Hash, AutonomousLifecycleReplicaQueueDispositionV1, Hash),
}
/// Durable exact-key owner for one autonomous executable entrypoint.
///
/// Claims live in hash-addressed files outside individual lane segments, so a
/// lookup touches at most one bounded record and never scans historical lane
/// blocks. An active claim binds the complete immutable payload identity. A
/// terminal slot retirement first changes every claim to `ReleasePending`.
/// Only a durable Queue release barrier permits the second transition to
/// the immediately replaceable `Released` state. Replica observation instead
/// reaches `ReplicaReleased`; after the matching terminal outcome is synced,
/// Kura seals it as `ReplicaReleasedComplete`. Only that archive-independent
/// sealed state may be replaced by a later payload.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct AutonomousLaneEntrypointClaimV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    entrypoint_hash: Hash,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
    state: AutonomousLaneEntrypointClaimStateV1,
}
impl AutonomousLaneEntrypointClaimV1 {
    const VERSION: u16 = 1;
    fn new(payload: &LaneExecutablePayloadV1, entrypoint_hash: Hash) -> Self {
        let descriptor = &payload.origin_proposal.descriptor;
        Self {
            version: Self::VERSION,
            network_id: payload.network_id,
            epoch: payload.epoch,
            entrypoint_hash,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            origin_proposal_hash: payload.origin_proposal.proposal_hash,
            executable_payload_hash: payload.payload_hash,
            state: AutonomousLaneEntrypointClaimStateV1::Active,
        }
    }
    fn owns_payload(&self, payload: &LaneExecutablePayloadV1) -> bool {
        let mut expected = Self::new(payload, self.entrypoint_hash);
        expected.state = self.state;
        self.version == Self::VERSION
            && payload.entrypoint_hashes.contains(&self.entrypoint_hash)
            && self == &expected
    }
    fn active_for_payload(&self, payload: &LaneExecutablePayloadV1) -> bool {
        matches!(self.state, AutonomousLaneEntrypointClaimStateV1::Active)
            && self.owns_payload(payload)
    }
    fn release_pending_for_payload(
        payload: &LaneExecutablePayloadV1,
        entrypoint_hash: Hash,
        retirement_hash: Hash,
    ) -> Self {
        let mut claim = Self::new(payload, entrypoint_hash);
        claim.state = AutonomousLaneEntrypointClaimStateV1::ReleasePending(retirement_hash);
        claim
    }
    fn released_for_payload(
        payload: &LaneExecutablePayloadV1,
        entrypoint_hash: Hash,
        retirement_hash: Hash,
    ) -> Self {
        let mut claim = Self::new(payload, entrypoint_hash);
        claim.state = AutonomousLaneEntrypointClaimStateV1::Released(retirement_hash);
        claim
    }
    fn replica_released_for_payload(
        payload: &LaneExecutablePayloadV1,
        entrypoint_hash: Hash,
        retirement_hash: Hash,
        queue_disposition: AutonomousLifecycleReplicaQueueDispositionV1,
    ) -> Self {
        let mut claim = Self::new(payload, entrypoint_hash);
        claim.state = AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(
            retirement_hash,
            queue_disposition,
        );
        claim
    }
    fn replica_released_complete_for_payload(
        payload: &LaneExecutablePayloadV1,
        entrypoint_hash: Hash,
        retirement_hash: Hash,
        queue_disposition: AutonomousLifecycleReplicaQueueDispositionV1,
        terminal_outcome_hash: Hash,
    ) -> Self {
        let mut claim = Self::new(payload, entrypoint_hash);
        claim.state = AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(
            retirement_hash,
            queue_disposition,
            terminal_outcome_hash,
        );
        claim
    }
    fn retirement_hash(&self) -> Option<Hash> {
        match self.state {
            AutonomousLaneEntrypointClaimStateV1::Active => None,
            AutonomousLaneEntrypointClaimStateV1::ReleasePending(hash)
            | AutonomousLaneEntrypointClaimStateV1::Released(hash)
            | AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(hash, _)
            | AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(hash, _, _) => {
                Some(hash)
            }
        }
    }
    fn replica_queue_disposition(&self) -> Option<AutonomousLifecycleReplicaQueueDispositionV1> {
        match self.state {
            AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(_, disposition) => {
                Some(disposition)
            }
            AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(_, disposition, _) => {
                Some(disposition)
            }
            AutonomousLaneEntrypointClaimStateV1::Active
            | AutonomousLaneEntrypointClaimStateV1::ReleasePending(_)
            | AutonomousLaneEntrypointClaimStateV1::Released(_) => None,
        }
    }
    fn replica_terminal_outcome_hash(&self) -> Option<Hash> {
        match self.state {
            AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(_, _, outcome_hash) => {
                Some(outcome_hash)
            }
            AutonomousLaneEntrypointClaimStateV1::Active
            | AutonomousLaneEntrypointClaimStateV1::ReleasePending(_)
            | AutonomousLaneEntrypointClaimStateV1::Released(_)
            | AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(_, _) => None,
        }
    }
}
/// Terminal durable identity for one abandoned autonomous lane-height slot.
///
/// The record is written before any queue reservation returns to ordinary FIFO
/// ownership. Its ordered reservation vector is therefore both the exact
/// release recipe and the restart proof that a delayed proposal, READY vote,
/// QC, or merge bundle belongs to a closed slot.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLaneSlotRetirementV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    lane_block_view: u64,
    origin_descriptor_hash: Hash,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
    reservation_keys: Vec<crate::queue::LaneQueueReservationKeyV1>,
}
impl AutonomousLaneSlotRetirementV1 {
    const VERSION: u16 = 1;
    /// Build the only valid retirement identity for an authenticated payload.
    #[must_use]
    pub(crate) fn from_payload(payload: &LaneExecutablePayloadV1) -> Self {
        let proposal = &payload.origin_proposal;
        let descriptor = &proposal.descriptor;
        Self {
            version: Self::VERSION,
            network_id: payload.network_id,
            epoch: payload.epoch,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            origin_descriptor_hash: descriptor.descriptor_hash,
            origin_proposal_hash: proposal.proposal_hash,
            executable_payload_hash: payload.payload_hash,
            reservation_keys: payload.reservation_keys.clone(),
        }
    }
    fn matches_payload(&self, payload: &LaneExecutablePayloadV1) -> bool {
        self.version == Self::VERSION && self == &Self::from_payload(payload)
    }
    pub(crate) fn digest(&self) -> Result<Hash> {
        let bytes = norito::encode_canonical(self).map_err(Error::NoritoFrame)?;
        Ok(Hash::new_from_chunks(&[
            b"iroha:nexus:autonomous-lane-slot-retirement:v1\0",
            &bytes,
        ]))
    }
    /// Build the exact Queue-side ordered barrier for this durable retirement.
    pub(crate) fn queue_release_barrier(
        &self,
    ) -> Result<crate::queue::LaneQueueReservationReleaseBarrierV1> {
        Ok(crate::queue::LaneQueueReservationReleaseBarrierV1 {
            version: crate::queue::LaneQueueReservationReleaseBarrierV1::VERSION,
            network_id: self.network_id,
            epoch: self.epoch,
            lane_id: self.lane_id,
            dataspace_id: self.dataspace_id,
            lane_incarnation: self.lane_incarnation,
            proposal_height: self.proposal_height,
            lane_block_height: self.lane_block_height,
            lane_block_view: self.lane_block_view,
            origin_descriptor_hash: self.origin_descriptor_hash,
            origin_proposal_hash: self.origin_proposal_hash,
            executable_payload_hash: self.executable_payload_hash,
            retirement_hash: self.digest()?,
            ordered_keys: self.reservation_keys.clone(),
        })
    }
}
/// Known formats for the bounded mutable view state of an autonomous payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
enum AutonomousLaneBlockViewStateFormat {
    #[codec(index = 0)]
    /// Quorum checkpoint plus a bounded contiguous certificate suffix.
    Current,
}
/// Mutable view state stored separately from the immutable executable payload.
///
/// Separating this small record prevents every timeout from appending another
/// copy of an executable payload that may be as large as the consensus frame
/// limit. All identity fields are repeated and validated so a stale view file
/// cannot be attached to a recreated lane or another payload at the same
/// lane-local height.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct AutonomousLaneBlockViewState {
    format: AutonomousLaneBlockViewStateFormat,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
    availability_certificate: Option<DurableLanePayloadAvailabilityCertificateV1>,
    checkpoint: Option<DurableLaneBlockViewCheckpointV1>,
    certificates: Vec<DurableLaneBlockNewViewCertificateV1>,
    retirement: Option<AutonomousLaneSlotRetirementV1>,
}
impl AutonomousLaneBlockViewState {
    fn from_artifact(artifact: &AutonomousLaneBlockArtifact) -> Self {
        let payload = &artifact.executable_payload;
        let descriptor = &payload.origin_proposal.descriptor;
        Self {
            format: AutonomousLaneBlockViewStateFormat::Current,
            network_id: payload.network_id,
            epoch: payload.epoch,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            origin_proposal_hash: payload.origin_proposal.proposal_hash,
            executable_payload_hash: payload.payload_hash,
            availability_certificate: artifact.availability_certificate.clone(),
            checkpoint: artifact.view_checkpoint.clone(),
            certificates: artifact.new_view_certificates.clone(),
            retirement: None,
        }
    }
    fn matches_payload(&self, payload: &LaneExecutablePayloadV1) -> bool {
        let descriptor = &payload.origin_proposal.descriptor;
        matches!(self.format, AutonomousLaneBlockViewStateFormat::Current)
            && self.network_id == payload.network_id
            && self.epoch == payload.epoch
            && self.lane_id == descriptor.lane_id
            && self.dataspace_id == descriptor.dataspace_id
            && self.lane_incarnation == descriptor.lane_incarnation
            && self.proposal_height == descriptor.proposal_height
            && self.lane_block_height == descriptor.lane_block_height
            && self.origin_proposal_hash == payload.origin_proposal.proposal_hash
            && self.executable_payload_hash == payload.payload_hash
    }
}
struct AutonomousLaneBlockDurableRecord {
    artifact: AutonomousLaneBlockArtifact,
    retirement: Option<AutonomousLaneSlotRetirementV1>,
    view_state_path: PathBuf,
}
include!("autonomous_reservation_types.rs");
include!("autonomous_reservation_inventory.rs");
include!("autonomous_reservation_classifier.rs");
include!("historical_autonomous_recovery.rs");
/// Known metadata format variants for lane-local block artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum LaneBlockArtifactFormat {
    #[codec(index = 0)]
    /// Lane payload ownership artifact anchored to a committed global block hash.
    Current,
}
/// Persisted lane-local payload ownership artifact anchored to a global block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct LaneBlockArtifact {
    /// Schema / evolution tag for the lane artifact format.
    pub format: LaneBlockArtifactFormat,
    /// Global block hash that committed the lane payload ownership.
    pub proposal_block_hash: HashOf<BlockHeader>,
    /// Lane-local payload ownership and RBC instance identity.
    pub ownership: SumeragiLanePayloadOwnership,
}
impl LaneBlockArtifact {
    const FORMAT_LABEL: &'static str = "lane.block_artifact";
    /// Construct a lane block artifact using the current schema.
    #[must_use]
    pub fn new(
        proposal_block_hash: HashOf<BlockHeader>,
        ownership: SumeragiLanePayloadOwnership,
    ) -> Self {
        Self {
            format: LaneBlockArtifactFormat::Current,
            proposal_block_hash,
            ownership,
        }
    }
    /// Return the human-readable format tag describing the artifact payload.
    #[must_use]
    pub fn format_label(&self) -> &'static str {
        match self.format {
            LaneBlockArtifactFormat::Current => Self::FORMAT_LABEL,
        }
    }
    /// Encode the artifact into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails.
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SidecarIndexEntry {
    offset: u64,
    len: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SidecarIndexLayout {
    base_height: u64,
    entries_offset: u64,
    entry_count: u64,
    aligned_len: u64,
}
#[derive(Debug, Clone, Copy)]
enum IndexedSidecarRewrite {
    RetainNewest {
        retention: NonZeroUsize,
        pinned_height: Option<u64>,
    },
    /// Advance the based-index window together with the retained payloads.
    ///
    /// This is reserved for evidence whose configured retention is also its
    /// hard startup scan bound. Generic pipeline sidecars retain zero slots so
    /// every height in the canonical V1 window keeps a stable position.
    #[cfg(test)]
    RetainNewestWindow { retention: NonZeroUsize },
    /// Discard only the authenticated terminal prefix while retaining a
    /// configured diagnostic window and every later (possibly pending) slot.
    RetainAfterTerminalFrontier {
        terminal_height: u64,
        retention: NonZeroUsize,
    },
}
#[derive(Debug, Clone, Copy)]
enum LaneBlockArtifactConflictPolicy {
    PreserveCanonical,
    AllowCanonicalReplacementAtProposalHeight(u64),
}
#[derive(Debug)]
struct LaneBlockArtifactWriteCheckpoint {
    data_path: PathBuf,
    index_path: PathBuf,
    data_existed: bool,
    index_existed: bool,
    data_len: u64,
    index_len: u64,
    index_layout: Option<SidecarIndexLayout>,
    index_entry_pos: Option<u64>,
    index_entry_bytes: Option<[u8; PIPELINE_INDEX_ENTRY_SIZE]>,
    tracked_bytes_before: Option<u64>,
}
struct LaneBlockArtifactWriteBatch<'a> {
    kura: &'a Kura,
    // Fields drop in declaration order, so the inner sidecar gate is released
    // before the outer geometry gate.
    _sidecar_guard: parking_lot::MutexGuard<'a, ()>,
    _geometry_guard: parking_lot::MutexGuard<'a, ()>,
    checkpoints: Vec<LaneBlockArtifactWriteCheckpoint>,
    finished: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FastpqProofWriteResult {
    Written,
    Retry,
    Drop,
}
impl SidecarIndexEntry {
    fn to_bytes(self) -> [u8; PIPELINE_INDEX_ENTRY_SIZE] {
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        buf[..8].copy_from_slice(&self.offset.to_le_bytes());
        buf[8..].copy_from_slice(&self.len.to_le_bytes());
        buf
    }
    fn from_bytes(bytes: [u8; PIPELINE_INDEX_ENTRY_SIZE]) -> Self {
        let offset = u64::from_le_bytes(bytes[..8].try_into().expect("slice length matches"));
        let len = u64::from_le_bytes(bytes[8..].try_into().expect("slice length matches"));
        Self { offset, len }
    }
}
impl SidecarIndexLayout {
    fn based(base_height: u64, index_len: u64) -> Result<Self, &'static str> {
        if base_height == 0 || base_height == u64::MAX {
            return Err("sidecar base height is invalid");
        }
        let entries_len = index_len
            .checked_sub(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64)
            .ok_or("sidecar base-height header is truncated")?;
        let aligned_entries_len = entries_len - entries_len % PIPELINE_INDEX_ENTRY_SIZE_U64;
        let entry_count = aligned_entries_len / PIPELINE_INDEX_ENTRY_SIZE_U64;
        base_height
            .checked_add(entry_count)
            .ok_or("sidecar base height and entry count overflow")?;
        Ok(Self {
            base_height,
            entries_offset: INDEXED_SIDECAR_BASE_HEADER_SIZE_U64,
            entry_count,
            aligned_len: INDEXED_SIDECAR_BASE_HEADER_SIZE_U64 + aligned_entries_len,
        })
    }
    fn next_height(self) -> Option<u64> {
        self.base_height.checked_add(self.entry_count)
    }
    fn entry_position(self, height: u64) -> Option<u64> {
        let relative = height.checked_sub(self.base_height)?;
        if relative >= self.entry_count {
            return None;
        }
        relative
            .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
            .and_then(|offset| self.entries_offset.checked_add(offset))
    }
    fn height_range(self) -> Option<core::ops::RangeInclusive<u64>> {
        if self.entry_count == 0 {
            return None;
        }
        let end = self.next_height()?.checked_sub(1)?;
        Some(self.base_height..=end)
    }
    fn base_header(base_height: u64) -> [u8; INDEXED_SIDECAR_BASE_HEADER_SIZE] {
        let mut header = [0u8; INDEXED_SIDECAR_BASE_HEADER_SIZE];
        header[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        header[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        header[16..24].copy_from_slice(&base_height.to_le_bytes());
        header[24..]
            .copy_from_slice(&(base_height ^ INDEXED_SIDECAR_BASE_CHECK_MASK).to_le_bytes());
        header
    }
    fn read_from(index: &mut std::fs::File, index_len: u64) -> Result<Self, &'static str> {
        if index_len < INDEXED_SIDECAR_BASE_HEADER_SIZE_U64 {
            return Err("sidecar V1 base-height header is truncated");
        }
        let mut first_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        index
            .seek(SeekFrom::Start(0))
            .and_then(|_| index.read_exact(&mut first_buf))
            .map_err(|_| "failed to read sidecar index prefix")?;
        let first = SidecarIndexEntry::from_bytes(first_buf);
        if first.offset != u64::MAX || first.len != u64::MAX {
            return Err("sidecar V1 base-height marker is missing");
        }
        let mut metadata_buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        index
            .read_exact(&mut metadata_buf)
            .map_err(|_| "failed to read sidecar base-height metadata")?;
        let metadata = SidecarIndexEntry::from_bytes(metadata_buf);
        if metadata.len != metadata.offset ^ INDEXED_SIDECAR_BASE_CHECK_MASK {
            return Err("sidecar base-height checksum mismatch");
        }
        Self::based(metadata.offset, index_len)
    }
}
/// Local availability state for the executable payload behind a certified lane block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneBlockPayloadAvailability {
    /// The canonical global block body still provides every accepted entrypoint.
    Available,
    /// No lane payload ownership artifact is stored for the certified lane height.
    MissingLaneArtifact,
    /// The ownership artifact no longer matches the certified lane descriptor.
    DescriptorMismatch,
    /// The global block that anchored the ownership artifact is not locally readable.
    MissingProposalBlock,
    /// An accepted entrypoint index is not present in the canonical global block body.
    MissingEntrypoint,
    /// The canonical global block has no committed result at an accepted entrypoint index.
    MissingTransactionResult,
    /// An accepted entrypoint hash differs from the certified descriptor.
    EntrypointHashMismatch,
}
/// Read-only startup classification for one ordinary application receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LaneBlockApplicationReceiptRepairPreflight {
    /// Every canonical input and result is locally available and exact.
    Ready(LaneBlockApplicationReceiptArtifact),
    /// The finality-authenticated result-bearing global body must be rehydrated.
    MissingCanonicalBody,
}
/// Verified payload material recovered for a certified standalone lane block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredLaneBlockPayload {
    /// Certified lane-block proposal whose descriptor selected this payload.
    pub proposal: LaneBlockProposalV1,
    /// Authenticated source from which the executable payload was recovered.
    pub source: LaneBlockExecutionSourceV1,
    /// Accepted entrypoints in lane descriptor order.
    pub entrypoints: Vec<TransactionEntrypoint>,
    /// Exact durable queue reservation identities in entrypoint order.
    pub reservation_keys: Vec<LaneQueueReservationKeyV1>,
    /// Complete routing plans in entrypoint order.
    pub routing_plans: Vec<RoutingPlan>,
    /// Producer-authenticated native-AMX receipts in entrypoint order.
    pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
}
/// Authenticated source of a recovered standalone lane-block execution.
///
/// The variants are intentionally disjoint. A globally committed payload is
/// anchored by its exact ownership sidecar, while a lane-owned autonomous
/// payload is bound directly to its chain, epoch, and executable-payload hash.
/// Autonomous inputs never manufacture a global block hash or proposal view.
#[allow(variant_size_differences)] // The exact global artifact must remain inline and canonical.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub enum LaneBlockExecutionSourceV1 {
    /// Payload reconstructed from a canonical globally committed block.
    #[codec(index = 0)]
    GlobalBlock {
        /// Exact lane ownership sidecar committed by the global block.
        artifact: LaneBlockArtifact,
    },
    /// Producer-authenticated payload owned and executed by its lane.
    #[codec(index = 1)]
    AutonomousLane {
        /// Chain whose authenticated lane payload is authoritative.
        network_id: iroha_data_model::NetworkId,
        /// Consensus epoch of the authenticated lane payload.
        epoch: u64,
        /// Canonical digest of the complete executable payload.
        payload_hash: Hash,
    },
}
impl LaneBlockExecutionSourceV1 {
    /// Construct a source anchored to a globally committed block.
    #[must_use]
    pub fn global_block(artifact: LaneBlockArtifact) -> Self {
        Self::GlobalBlock { artifact }
    }

    /// Construct a source bound directly to an autonomous lane payload.
    #[must_use]
    pub fn autonomous_lane(
        network_id: iroha_data_model::NetworkId,
        epoch: u64,
        payload_hash: Hash,
    ) -> Self {
        Self::AutonomousLane {
            network_id,
            epoch,
            payload_hash,
        }
    }

    /// Return the global ownership artifact, when global-block recovery was used.
    #[must_use]
    pub fn global_artifact(&self) -> Option<&LaneBlockArtifact> {
        match self {
            Self::GlobalBlock { artifact } => Some(artifact),
            Self::AutonomousLane { .. } => None,
        }
    }

    /// Return the autonomous chain, epoch, and payload binding, when applicable.
    #[must_use]
    pub fn autonomous_binding(&self) -> Option<(iroha_data_model::NetworkId, u64, Hash)> {
        match self {
            Self::GlobalBlock { .. } => None,
            Self::AutonomousLane {
                network_id,
                epoch,
                payload_hash,
            } => Some((*network_id, *epoch, *payload_hash)),
        }
    }
}
/// Known metadata format variants for durable lane-block execution input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum LaneBlockExecutionInputArtifactFormat {
    #[codec(index = 0)]
    /// Recovered standalone lane-block payload awaiting state application.
    Current,
}
/// Durable recovered input for a certified standalone lane block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct LaneBlockExecutionInputArtifact {
    /// Schema / evolution tag for the execution input format.
    pub format: LaneBlockExecutionInputArtifactFormat,
    /// Certified lane-block proposal whose descriptor selected this payload.
    pub proposal: LaneBlockProposalV1,
    /// Exact authenticated source of the recovered executable payload.
    pub source: LaneBlockExecutionSourceV1,
    /// Accepted entrypoint hashes in lane descriptor order.
    pub entrypoint_hashes: Vec<Hash>,
    /// Accepted entrypoints in lane descriptor order.
    pub entrypoints: Vec<TransactionEntrypoint>,
    /// Exact durable queue reservation identities in entrypoint order.
    pub reservation_keys: Vec<LaneQueueReservationKeyV1>,
    /// Complete routing plans in entrypoint order.
    pub routing_plans: Vec<RoutingPlan>,
    /// Producer-authenticated native-AMX receipts in entrypoint order.
    pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
}
impl LaneBlockExecutionInputArtifact {
    const FORMAT_LABEL: &'static str = "lane.execution_input";
    /// Construct a durable execution input artifact from a verified recovery result.
    #[must_use]
    pub fn new(recovered: RecoveredLaneBlockPayload) -> Self {
        let entrypoint_hashes = recovered
            .entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect();
        Self {
            format: LaneBlockExecutionInputArtifactFormat::Current,
            proposal: recovered.proposal,
            source: recovered.source,
            entrypoint_hashes,
            entrypoints: recovered.entrypoints,
            reservation_keys: recovered.reservation_keys,
            routing_plans: recovered.routing_plans,
            native_amx_receipts: recovered.native_amx_receipts,
        }
    }
    /// Return the human-readable format tag describing the artifact payload.
    #[must_use]
    pub fn format_label(&self) -> &'static str {
        match self.format {
            LaneBlockExecutionInputArtifactFormat::Current => Self::FORMAT_LABEL,
        }
    }
    /// Encode the artifact into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails.
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }
}
/// Known metadata format variants for durable lane-block direct-execution preflights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum LaneBlockExecutionPreflightArtifactFormat {
    #[codec(index = 0)]
    /// Non-committing direct-execution preflight result for recovered lane input.
    Current,
}
/// Durable result of non-committing direct-execution preflight for a lane block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct LaneBlockExecutionPreflightArtifact {
    /// Schema / evolution tag for the preflight format.
    pub format: LaneBlockExecutionPreflightArtifactFormat,
    /// Certified lane-block proposal whose descriptor selected this payload.
    pub proposal: LaneBlockProposalV1,
    /// Exact authenticated source of the preflighted executable payload.
    pub source: LaneBlockExecutionSourceV1,
    /// Local committed state height used as the preflight base.
    pub preflight_state_height: u64,
    /// Local committed WSV snapshot hash used as the preflight base.
    pub preflight_state_hash: Option<HashOf<BlockHeader>>,
    /// Accepted entrypoint indices in lane descriptor order.
    pub entrypoint_indices: Vec<u64>,
    /// Accepted entrypoint hashes in lane descriptor order.
    pub entrypoint_hashes: Vec<Hash>,
    /// Hashes of preflight transaction results in lane descriptor order.
    pub result_hashes: Vec<Hash>,
    /// Preflight transaction results in lane descriptor order.
    pub results: Vec<TransactionResult>,
}
impl LaneBlockExecutionPreflightArtifact {
    const FORMAT_LABEL: &'static str = "lane.execution_preflight";
    /// Construct a durable direct-execution preflight result from recovered input.
    #[must_use]
    pub fn new(
        input: &LaneBlockExecutionInputArtifact,
        preflight_state_height: u64,
        preflight_state_hash: Option<HashOf<BlockHeader>>,
        results: Vec<TransactionResult>,
    ) -> Self {
        let result_hashes = results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect();
        Self {
            format: LaneBlockExecutionPreflightArtifactFormat::Current,
            proposal: input.proposal.clone(),
            source: input.source.clone(),
            preflight_state_height,
            preflight_state_hash,
            entrypoint_indices: input.proposal.descriptor.accepted_candidate_indices.clone(),
            entrypoint_hashes: input.entrypoint_hashes.clone(),
            result_hashes,
            results,
        }
    }
    /// Return the human-readable format tag describing the artifact payload.
    #[must_use]
    pub fn format_label(&self) -> &'static str {
        match self.format {
            LaneBlockExecutionPreflightArtifactFormat::Current => Self::FORMAT_LABEL,
        }
    }
    /// Whether any transaction failed during preflight.
    #[must_use]
    pub fn has_rejections(&self) -> bool {
        self.results.iter().any(|result| result.0.is_err())
    }
    /// Encode the artifact into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails.
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }
}
/// Known metadata format variants for durable lane-block application receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum LaneBlockApplicationReceiptArtifactFormat {
    #[codec(index = 0)]
    /// Canonical global block results proving lane payload state application.
    Current,
    #[codec(index = 1)]
    /// Direct standalone execution results proving lane payload state application.
    DirectExecution,
    #[codec(index = 2)]
    /// Canonical merge-batch execution results proving lane payload state application.
    MergeExecution,
}
/// Durable receipt proving a certified standalone lane block has committed results.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct LaneBlockApplicationReceiptArtifact {
    /// Schema / evolution tag for the application receipt format.
    pub format: LaneBlockApplicationReceiptArtifactFormat,
    /// Certified lane-block proposal whose descriptor selected this payload.
    pub proposal: LaneBlockProposalV1,
    /// Exact authenticated source of the applied executable payload.
    pub source: LaneBlockExecutionSourceV1,
    /// Canonical block height, or committed preflight base height for direct execution.
    pub application_block_height: u64,
    /// Canonical block hash, or committed preflight base WSV hash for direct execution.
    pub application_block_hash: HashOf<BlockHeader>,
    /// Accepted entrypoint indices in the canonical block body.
    pub entrypoint_indices: Vec<u64>,
    /// Accepted entrypoint hashes in lane descriptor order.
    pub entrypoint_hashes: Vec<Hash>,
    /// Hashes of committed transaction results in lane descriptor order.
    pub result_hashes: Vec<Hash>,
    /// Committed transaction results in lane descriptor order.
    pub results: Vec<TransactionResult>,
    /// Merge epoch whose durable entry authorized this application.
    pub merge_epoch_id: Option<u64>,
    /// Hash of the exact full merge-ledger entry referenced by the carrier block.
    pub merge_entry_hash: Option<HashOf<MergeLedgerEntry>>,
    /// Actual globally committed block height carrying the compact merge reference.
    pub merge_carrier_block_height: Option<u64>,
    /// Actual globally committed block hash carrying the compact merge reference.
    pub merge_carrier_block_hash: Option<HashOf<BlockHeader>>,
    /// Hash-addressed authenticated source bundle committed by the merge batch.
    pub merge_source_bundle_hash: Option<Hash>,
    /// Stable pre-marker batch identity included in the complete write set.
    pub merge_batch_identity_hash: Option<Hash>,
    /// Final marker-inclusive merge batch hash sealed by the merge QC.
    pub merge_batch_hash: Option<Hash>,
    /// Canonical base WSV commitment sealed by the merge batch.
    pub merge_base_state_hash: Option<HashOf<BlockHeader>>,
    /// Canonical complete marker-inclusive write-set root.
    pub merge_write_set_root: Option<Hash>,
    /// Expected post-state transition commitment after the batch.
    pub merge_expected_post_state_hash: Option<HashOf<BlockHeader>>,
    /// Exact lane settlement commitment hash staged atomically with execution.
    pub merge_settlement_hash:
        Option<HashOf<iroha_data_model::block::consensus::LaneBlockCommitment>>,
}
/// Versioned durable cursor proving a contiguous lane-local prefix reached the
/// canonical global merge carrier.
///
/// The cursor is published only after the exact application receipt is
/// durability-attested. It lets Kura compact append histories without
/// retaining a lifetime-sized startup or retirement scan. Every field is
/// reconstructed from the QC-authenticated merge entry and carrier before the
/// cursor can authorize compaction or archive validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct LaneMergeApplicationFrontierV1 {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    lane_block_descriptor_hash: Hash,
    proposal_hash: Hash,
    merge_epoch_id: u64,
    merge_entry_hash: HashOf<MergeLedgerEntry>,
    application_block_height: u64,
    application_block_hash: HashOf<BlockHeader>,
    receipt_hash: HashOf<LaneBlockApplicationReceiptArtifact>,
}
impl LaneMergeApplicationFrontierV1 {
    const VERSION: u8 = 1;
    fn from_receipt(receipt: &LaneBlockApplicationReceiptArtifact) -> Option<Self> {
        if receipt.format != LaneBlockApplicationReceiptArtifactFormat::MergeExecution {
            return None;
        }
        let descriptor = &receipt.proposal.descriptor;
        Some(Self {
            version: Self::VERSION,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_descriptor_hash: descriptor.descriptor_hash,
            proposal_hash: receipt.proposal.proposal_hash,
            merge_epoch_id: receipt.merge_epoch_id?,
            merge_entry_hash: receipt.merge_entry_hash?,
            application_block_height: receipt.merge_carrier_block_height?,
            application_block_hash: receipt.merge_carrier_block_hash?,
            receipt_hash: HashOf::new(receipt),
        })
    }
    fn matches_receipt(&self, receipt: &LaneBlockApplicationReceiptArtifact) -> bool {
        Self::from_receipt(receipt).as_ref() == Some(self)
    }
}
include!("native_amx_participant_application_artifacts.rs");
impl LaneBlockApplicationReceiptArtifact {
    const FORMAT_LABEL: &'static str = "lane.application_receipt";
    /// Construct a durable application receipt from canonical block results.
    #[must_use]
    pub fn new(
        recovered: RecoveredLaneBlockPayload,
        application_block_height: u64,
        application_block_hash: HashOf<BlockHeader>,
        results: Vec<TransactionResult>,
    ) -> Self {
        let entrypoint_indices = recovered
            .proposal
            .descriptor
            .accepted_candidate_indices
            .clone();
        let entrypoint_hashes = recovered
            .entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect();
        let result_hashes = results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect();
        Self {
            format: LaneBlockApplicationReceiptArtifactFormat::Current,
            proposal: recovered.proposal,
            source: recovered.source,
            application_block_height,
            application_block_hash,
            entrypoint_indices,
            entrypoint_hashes,
            result_hashes,
            results,
            merge_epoch_id: None,
            merge_entry_hash: None,
            merge_carrier_block_height: None,
            merge_carrier_block_hash: None,
            merge_source_bundle_hash: None,
            merge_batch_identity_hash: None,
            merge_batch_hash: None,
            merge_base_state_hash: None,
            merge_write_set_root: None,
            merge_expected_post_state_hash: None,
            merge_settlement_hash: None,
        }
    }
    /// Construct a durable application receipt from clean direct-execution preflight results.
    #[must_use]
    pub fn new_direct_execution(
        input: &LaneBlockExecutionInputArtifact,
        preflight: &LaneBlockExecutionPreflightArtifact,
    ) -> Option<Self> {
        let application_block_hash = preflight.preflight_state_hash?;
        if preflight.has_rejections()
            || input.proposal != preflight.proposal
            || input.source != preflight.source
            || input.proposal.descriptor.accepted_candidate_indices != preflight.entrypoint_indices
            || input.entrypoint_hashes != preflight.entrypoint_hashes
        {
            return None;
        }
        Some(Self {
            format: LaneBlockApplicationReceiptArtifactFormat::DirectExecution,
            proposal: input.proposal.clone(),
            source: input.source.clone(),
            application_block_height: preflight.preflight_state_height,
            application_block_hash,
            entrypoint_indices: preflight.entrypoint_indices.clone(),
            entrypoint_hashes: preflight.entrypoint_hashes.clone(),
            result_hashes: preflight.result_hashes.clone(),
            results: preflight.results.clone(),
            merge_epoch_id: None,
            merge_entry_hash: None,
            merge_carrier_block_height: None,
            merge_carrier_block_hash: None,
            merge_source_bundle_hash: None,
            merge_batch_identity_hash: None,
            merge_batch_hash: None,
            merge_base_state_hash: None,
            merge_write_set_root: None,
            merge_expected_post_state_hash: None,
            merge_settlement_hash: None,
        })
    }
    fn new_merge_execution(
        entry: &MergeLedgerEntry,
        batch: &MergeExecutionBatch,
        execution: &MergeLaneExecution,
        source: LaneBlockExecutionSourceV1,
        carrier_block_height: u64,
        carrier_block_hash: HashOf<BlockHeader>,
    ) -> Self {
        Self {
            format: LaneBlockApplicationReceiptArtifactFormat::MergeExecution,
            proposal: execution.proposal.clone(),
            source,
            application_block_height: carrier_block_height,
            application_block_hash: carrier_block_hash,
            entrypoint_indices: execution
                .proposal
                .descriptor
                .accepted_candidate_indices
                .clone(),
            entrypoint_hashes: execution.entrypoint_hashes.clone(),
            result_hashes: execution.result_hashes.clone(),
            results: execution.results.clone(),
            merge_epoch_id: Some(entry.epoch_id),
            merge_entry_hash: Some(crate::merge::merge_ledger_entry_hash(entry)),
            merge_carrier_block_height: Some(carrier_block_height),
            merge_carrier_block_hash: Some(carrier_block_hash),
            merge_source_bundle_hash: Some(execution.source_bundle_hash),
            merge_batch_identity_hash: Some(crate::merge::merge_execution_batch_identity_hash(
                batch,
            )),
            merge_batch_hash: Some(batch.batch_hash),
            merge_base_state_hash: Some(batch.base_state_hash),
            merge_write_set_root: Some(batch.write_set_root),
            merge_expected_post_state_hash: Some(batch.expected_post_state_hash),
            merge_settlement_hash: Some(execution.settlement_hash),
        }
    }
    /// Return the human-readable format tag describing the artifact payload.
    #[must_use]
    pub fn format_label(&self) -> &'static str {
        match self.format {
            LaneBlockApplicationReceiptArtifactFormat::Current
            | LaneBlockApplicationReceiptArtifactFormat::DirectExecution
            | LaneBlockApplicationReceiptArtifactFormat::MergeExecution => Self::FORMAT_LABEL,
        }
    }
    /// Encode the artifact into a framed Norito buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if framing fails.
    pub fn encode_framed(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }
}
impl LaneBlockArtifactConflictPolicy {
    fn allows_canonical_replacement(
        self,
        existing: &LaneBlockArtifact,
        replacement: &LaneBlockArtifact,
    ) -> bool {
        match self {
            Self::PreserveCanonical => false,
            Self::AllowCanonicalReplacementAtProposalHeight(height) => {
                existing.ownership.proposal_height == height
                    && replacement.ownership.proposal_height == height
                    && existing.proposal_block_hash != replacement.proposal_block_hash
            }
        }
    }
}
impl<'a> LaneBlockArtifactWriteBatch<'a> {
    fn new(kura: &'a Kura) -> Self {
        let geometry_guard = kura.lane_geometry_lock.lock();
        let sidecar_guard = kura.sidecar_lock.lock();
        Self {
            kura,
            _sidecar_guard: sidecar_guard,
            _geometry_guard: geometry_guard,
            checkpoints: Vec::new(),
            finished: false,
        }
    }
    fn push(&mut self, checkpoint: LaneBlockArtifactWriteCheckpoint) {
        self.checkpoints.push(checkpoint);
    }
    fn commit(mut self) {
        self.finished = true;
    }
    fn rollback(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        let mut first_error = None;
        while let Some(checkpoint) = self.checkpoints.pop() {
            if let Err(err) = self
                .kura
                .restore_lane_block_artifact_checkpoint_locked(&checkpoint)
            {
                error!(?err, "failed to roll back lane block artifact write");
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        self.finished = true;
        if let Some(err) = first_error {
            Err(err)
        } else {
            Ok(())
        }
    }
}
impl Drop for LaneBlockArtifactWriteBatch<'_> {
    fn drop(&mut self) {
        if !self.finished
            && let Err(err) = self.rollback()
        {
            error!(?err, "failed to roll back uncommitted lane block artifacts");
        }
    }
}

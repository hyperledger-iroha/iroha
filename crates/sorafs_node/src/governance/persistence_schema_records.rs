// Durable records declared in the parent module.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::FencedPrivacyStateV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct FencedPrivacyStateV1 {
    version: u8,
    pending: Option<FencedPrivacyPendingRequestV1>,
    publication_cache: Option<FencedPrivacyPublicationCacheV1>,
    authoritative_head_sync: Option<FencedPrivacyAuthoritativeHeadSyncV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProviderBindingV1 {
    signer_handle: String,
    signer_revision: u64,
    signer_policy_digest: [u8; 32],
    checkpoint_store_handle: String,
    checkpoint_store_revision: u64,
    checkpoint_store_policy_digest: [u8; 32],
    publisher_peer_id: Vec<u8>,
    publisher_public_key: [u8; 32],
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagQualificationTransitionBodyV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationTransitionBodyV1 {
    version: u8,
    root_digest: [u8; 32],
    generation: u64,
    predecessor_transition_digest: Option<[u8; 32]>,
    predecessor_checkpoint_revision: [u8; 32],
    previous: RuntimeDagProviderBindingV1,
    next: RuntimeDagProviderBindingV1,
    block_count: u64,
    head_block_cid: [u8; 32],
    head_bytes_digest: [u8; 32],
    predecessor_index_digest: [u8; 32],
    successor_index_digest: [u8; 32],
    archive_generation: u64,
    archive_digest: [u8; 32],
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagKeyTransitionSigningPayloadV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagKeyTransitionSigningPayloadV1 {
    version: u8,
    outgoing_segment_revision: u64,
    incoming_segment_revision: u64,
    transition_body_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagKeyTransitionEnvelopeV1 {
    version: u8,
    outgoing_segment_revision: u64,
    incoming_segment_revision: u64,
    transition_body_digest: [u8; 32],
    outgoing_signature: [u8; 64],
    incoming_signature: [u8; 64],
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagQualificationTransitionV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationTransitionV1 {
    body: RuntimeDagQualificationTransitionBodyV1,
    key_transition: RuntimeDagKeyTransitionEnvelopeV1,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagQualificationHistoryV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationHistoryV1 {
    version: u8,
    root_digest: [u8; 32],
    archive_generation: u64,
    archive_digest: [u8; 32],
    archived_through_generation: u64,
    archive_tail_transition_digest: [u8; 32],
    transitions: Vec<RuntimeDagQualificationTransitionV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagQualificationStateV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationStateV1 {
    version: u8,
    history: Option<RuntimeDagQualificationHistoryV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagQualificationArchiveBodyV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationArchiveBodyV1 {
    version: u8,
    root_digest: [u8; 32],
    archive_generation: u64,
    predecessor_archive_digest: [u8; 32],
    predecessor_transition_digest: [u8; 32],
    first_transition_generation: u64,
    last_transition_generation: u64,
    tail_transition_digest: [u8; 32],
    signer: RuntimeDagProviderBindingV1,
    transitions: Vec<RuntimeDagQualificationTransitionV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagQualificationArchiveV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagQualificationArchiveV1 {
    body: RuntimeDagQualificationArchiveBodyV1,
    signature: [u8; 64],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeDagQualificationSummary {
    transition_generation: u64,
    transition_digest: [u8; 32],
    archive_generation: u64,
    archive_digest: [u8; 32],
}
impl RuntimeDagQualificationSummary {
    const EMPTY: Self = Self {
        transition_generation: 0,
        transition_digest: [0; 32],
        archive_generation: 0,
        archive_digest: [0; 32],
    };
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagProducerCheckpointV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RuntimeDagProducerCheckpointV1 {
    pub(crate) version: u8,
    pub(crate) root_digest: [u8; 32],
    pub(crate) signer_handle: String,
    pub(crate) signer_revision: u64,
    pub(crate) signer_policy_digest: [u8; 32],
    pub(crate) checkpoint_store_handle: String,
    pub(crate) checkpoint_store_revision: u64,
    pub(crate) checkpoint_store_policy_digest: [u8; 32],
    pub(crate) publisher_peer_id: Vec<u8>,
    pub(crate) publisher_public_key: [u8; 32],
    pub(crate) block_count: u64,
    pub(crate) head_block_cid: [u8; 32],
    pub(crate) head_bytes_digest: [u8; 32],
    pub(crate) index_bytes_digest: [u8; 32],
    pub(crate) qualification_transition_generation: u64,
    pub(crate) qualification_transition_digest: [u8; 32],
    pub(crate) qualification_archive_generation: u64,
    pub(crate) qualification_archive_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagedArtifactV1 {
    byte_len: u64,
    blake3: [u8; 32],
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagProducerPublishIntentV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerPublishIntentV1 {
    version: u8,
    checkpoint: RuntimeDagProducerCheckpointV1,
    previous_checkpoint_revision: Option<[u8; 32]>,
    staging_revision: [u8; 32],
    block: RuntimeDagProducerStagedArtifactV1,
    head: RuntimeDagProducerStagedArtifactV1,
    index: RuntimeDagProducerStagedArtifactV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagedTransactionV1 {
    block_bytes: Vec<u8>,
    head_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagedEnvelopeV1 {
    intent: RuntimeDagProducerPublishIntentV1,
    transaction: RuntimeDagProducerStagedTransactionV1,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagProducerStagingStateV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagProducerStagingStateV1 {
    version: u8,
    staged: Option<RuntimeDagProducerStagedEnvelopeV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance::RuntimeDagCommittedStateV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RuntimeDagCommittedStateV1 {
    version: u8,
    head_bytes: Option<Vec<u8>>,
    index_bytes: Option<Vec<u8>>,
}

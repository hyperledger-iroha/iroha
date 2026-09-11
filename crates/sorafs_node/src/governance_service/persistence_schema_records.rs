// Durable records declared in the parent module.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance_service::SignedBlockPrefixArchiveV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct SignedBlockPrefixArchiveV1 {
    version: u8,
    archive_generation: u64,
    predecessor: BlockPrefixArchiveHeadV1,
    predecessor_checkpoint_revision: [u8; 32],
    predecessor_checkpoint_digest: [u8; 32],
    predecessor_block_count: u64,
    predecessor_head_block_cid: Vec<u8>,
    target_checkpoint_generation: u64,
    target_head_block_cid: Vec<u8>,
    target_block_count: u64,
    target_source_chain_blake3: [u8; 32],
    ipfs_authenticator_handle: String,
    ipfs_authenticator_revision: u64,
    ipfs_authenticator_policy_digest: [u8; 32],
    ipfs_authenticator_public_key: [u8; 32],
    checkpoint_store_handle: String,
    checkpoint_store_revision: u64,
    checkpoint_store_policy_digest: [u8; 32],
    archived_block_count: u64,
    blocks: Vec<SignedBlockPrefixArchiveEntryV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance_service::MirrorIndexStorePayloadV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct MirrorIndexStorePayloadV1 {
    version: u8,
    checkpoint_generation: u64,
    publish_intent_blake3: [u8; 32],
    mirror_blake3: [u8; 32],
    canonical_json: Vec<u8>,
}
impl MirrorIndexStorePayloadV1 {
    fn empty() -> Self {
        Self {
            version: MIRROR_INDEX_STORE_PAYLOAD_VERSION_V1,
            checkpoint_generation: 0,
            publish_intent_blake3: [0; 32],
            mirror_blake3: [0; 32],
            canonical_json: Vec::new(),
        }
    }
    fn committed(
        checkpoint_generation: u64,
        publish_intent_blake3: [u8; 32],
        canonical_json: Vec<u8>,
    ) -> Result<Self, GovernanceDagServiceError> {
        let payload = Self {
            version: MIRROR_INDEX_STORE_PAYLOAD_VERSION_V1,
            checkpoint_generation,
            publish_intent_blake3,
            mirror_blake3: blake3_array(&canonical_json),
            canonical_json,
        };
        validate_mirror_index_store_payload(&payload)?;
        Ok(payload)
    }
    fn is_empty(&self) -> bool {
        self.checkpoint_generation == 0
    }
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance_service::CheckpointBodyV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct CheckpointBodyV1 {
    version: u8,
    generation: u64,
    head_block_cid: Vec<u8>,
    block_count: u64,
    head_bytes: Vec<u8>,
    head_bytes_blake3: [u8; 32],
    head_ipfs_cid: String,
    source_chain_blake3: [u8; 32],
    mirror_blake3: [u8; 32],
    published_at_unix: u64,
    archive_head: BlockPrefixArchiveHeadV1,
    mirror_blocks: Vec<PublishedBlockV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance_service::IntentBlockV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct IntentBlockV1 {
    sequence: u64,
    governance_block_cid: Vec<u8>,
    governance_node_cid: Vec<u8>,
    payload_kind: String,
    timestamp: u64,
    encoded_blake3: [u8; 32],
    encoded_len: u64,
    ipfs_cid: Option<String>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance_service::PublishIntentBodyV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct PublishIntentBodyV1 {
    version: u8,
    generation: u64,
    target_head_block_cid: Vec<u8>,
    target_block_count: u64,
    target_head_bytes: Vec<u8>,
    target_head_blake3: [u8; 32],
    target_source_chain_blake3: [u8; 32],
    previous_public_head_blake3: Option<[u8; 32]>,
    created_at_unix: u64,
    archive_head: BlockPrefixArchiveHeadV1,
    blocks: Vec<IntentBlockV1>,
    head_ipfs_cid: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckpointCommitmentV1 {
    revision: [u8; 32],
    digest: [u8; 32],
    block_count: u64,
    head_block_cid: Vec<u8>,
}
impl CheckpointCommitmentV1 {
    fn empty() -> Self {
        Self {
            revision: [0; 32],
            digest: [0; 32],
            block_count: 0,
            head_block_cid: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct RequestAuthReplayEntryV1 {
    nonce: [u8; 32],
    expires_at_unix_secs: u64,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::governance_service::RequestAuthReplayStateV1")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct RequestAuthReplayStateV1 {
    version: u8,
    entries: Vec<RequestAuthReplayEntryV1>,
}

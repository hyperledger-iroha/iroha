/// Payload-free failure while validating a canonical Governance DAG signing payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceDagSigningPayloadValidationErrorV1 {
    /// The bytes are not one exact canonical payload of the expected schema.
    Malformed,
    /// The decoded payload violates a frozen Governance DAG invariant.
    Invalid,
}

fn decode_governance_signing_payload_v1<T>(
    bytes: &[u8],
) -> Result<T, GovernanceDagSigningPayloadValidationErrorV1>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    if bytes.is_empty() || bytes.len() > GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1 {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Malformed);
    }
    norito::decode_canonical(bytes)
        .map_err(|_| GovernanceDagSigningPayloadValidationErrorV1::Malformed)
}

fn validate_governance_node_signing_fields_v1(
    node: &GovernanceLogSignaturePayloadV1,
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    if node.version != GOVERNANCE_LOG_VERSION_V1
        || node.node_cid.len() != GOVERNANCE_DAG_CID_BYTES_V1
        || node
            .prev_cid
            .as_ref()
            .is_some_and(|cid| cid.len() != GOVERNANCE_DAG_CID_BYTES_V1)
        || node.publisher_peer_id.is_empty()
        || node.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        || node.payload.validate(node.timestamp).is_err()
        || node
            .payload
            .validate_submission_provenance(node.submission_provenance.as_ref())
            .is_err()
    {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    let expected = governance_log_node_cid_v1(
        node.prev_cid.as_deref(),
        node.timestamp,
        &node.publisher_peer_id,
        node.submission_provenance.as_ref(),
        &node.payload,
    )
    .map_err(|_| GovernanceDagSigningPayloadValidationErrorV1::Invalid)?;
    if expected != node.node_cid {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    Ok(())
}

/// Validate one exact canonical governance log-node signing payload.
pub fn validate_governance_log_node_signing_payload_v1(
    bytes: &[u8],
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    let node: GovernanceLogSignaturePayloadV1 = decode_governance_signing_payload_v1(bytes)?;
    validate_governance_node_signing_fields_v1(&node)
}

/// Validate a log-node payload for one exact configured publisher identity.
pub fn validate_governance_log_node_signing_payload_for_publisher_v1(
    bytes: &[u8],
    expected_publisher_peer_id: &[u8],
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    let node: GovernanceLogSignaturePayloadV1 = decode_governance_signing_payload_v1(bytes)?;
    validate_governance_node_signing_fields_v1(&node)?;
    if node.publisher_peer_id != expected_publisher_peer_id {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    Ok(())
}

/// Validate one exact canonical Governance DAG block signing payload.
pub fn validate_governance_dag_block_signing_payload_v1(
    bytes: &[u8],
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    let block: GovernanceDagBlockSignaturePayloadV1 = decode_governance_signing_payload_v1(bytes)?;
    validate_governance_dag_block_signing_fields_v1(&block)
}

fn validate_governance_dag_block_signing_fields_v1(
    block: &GovernanceDagBlockSignaturePayloadV1,
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    if block.version != GOVERNANCE_DAG_BLOCK_VERSION_V1
        || block.block_cid.len() != GOVERNANCE_DAG_CID_BYTES_V1
        || block
            .prev_block_cid
            .as_ref()
            .is_some_and(|cid| cid.len() != GOVERNANCE_DAG_CID_BYTES_V1)
        || (block.sequence == 0) != block.prev_block_cid.is_none()
        || (block.sequence == 0) != block.node.prev_cid.is_none()
        || block.publisher_peer_id.is_empty()
        || block.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        || block.node.publisher_peer_id != block.publisher_peer_id
        || block.node.timestamp > block.timestamp
        || block.node.publisher_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || block.node.validate().is_err()
        || block.node.verify_publisher_signature().is_err()
    {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    let expected = governance_dag_block_cid_v1(
        block.prev_block_cid.as_deref(),
        block.sequence,
        block.timestamp,
        &block.publisher_peer_id,
        &block.node,
    )
    .map_err(|_| GovernanceDagSigningPayloadValidationErrorV1::Invalid)?;
    if expected != block.block_cid {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    Ok(())
}

/// Validate a block payload for one exact configured publisher identity/key.
pub fn validate_governance_dag_block_signing_payload_for_publisher_v1(
    bytes: &[u8],
    expected_publisher_peer_id: &[u8],
    expected_public_key: [u8; 32],
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    let block: GovernanceDagBlockSignaturePayloadV1 = decode_governance_signing_payload_v1(bytes)?;
    validate_governance_dag_block_signing_fields_v1(&block)?;
    if block.publisher_peer_id != expected_publisher_peer_id
        || block.node.publisher_signature.public_key.as_slice() != expected_public_key
    {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    Ok(())
}

/// Validate one exact canonical Governance DAG head signing payload.
pub fn validate_governance_dag_head_signing_payload_v1(
    bytes: &[u8],
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    let head: GovernanceDagHeadSignaturePayloadV1 = decode_governance_signing_payload_v1(bytes)?;
    validate_governance_dag_head_signing_fields_v1(&head)
}

fn validate_governance_dag_head_signing_fields_v1(
    head: &GovernanceDagHeadSignaturePayloadV1,
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    if head.version != GOVERNANCE_DAG_HEAD_VERSION_V1
        || head.head_block_cid.len() != GOVERNANCE_DAG_CID_BYTES_V1
        || head.block_count == 0
        || head.generated_at == 0
        || head.publisher_peer_id.is_empty()
        || head.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        || head
            .checkpoint_cid
            .as_ref()
            .is_some_and(|cid| cid.len() != GOVERNANCE_DAG_CID_BYTES_V1)
    {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    Ok(())
}

/// Validate a head payload for one exact configured publisher identity.
pub fn validate_governance_dag_head_signing_payload_for_publisher_v1(
    bytes: &[u8],
    expected_publisher_peer_id: &[u8],
) -> Result<(), GovernanceDagSigningPayloadValidationErrorV1> {
    let head: GovernanceDagHeadSignaturePayloadV1 = decode_governance_signing_payload_v1(bytes)?;
    validate_governance_dag_head_signing_fields_v1(&head)?;
    if head.publisher_peer_id != expected_publisher_peer_id {
        return Err(GovernanceDagSigningPayloadValidationErrorV1::Invalid);
    }
    Ok(())
}

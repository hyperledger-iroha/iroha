//! Privacy operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn privacy_cycle_prf_derive(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PrivacyCyclePrfRequestWireV1>(
        &request.payload,
        MAX_TRANSPARENCY_PRF_FRAME_BYTES_V1,
    )?;
    let request_value = wire.to_request()?;
    let output = broker_backend!(state, privacy_cycle_prf_provider)
        .derive_cycle_output(&request_value)
        .map_err(|error| match error {
            sorafs_node::PrivacyCyclePrfProviderErrorV1::Unavailable
            | sorafs_node::PrivacyCyclePrfProviderErrorV1::RateLimited => BrokerError::Unavailable,
            sorafs_node::PrivacyCyclePrfProviderErrorV1::AuthenticationFailed
            | sorafs_node::PrivacyCyclePrfProviderErrorV1::Internal => BrokerError::Rejected,
        })?;
    let wire = PrivacyCyclePrfOutputWireV1 {
        output: output.runtime_transport_bytes(),
    };
    requalify()?;
    encode_canonical(&wire, MAX_TRANSPARENCY_PRF_FRAME_BYTES_V1)
}

pub(super) fn privacy_release_anchor_finalized_head(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let query = validate_privacy_release_anchor_query(decode_canonical::<
        PrivacyReleaseAnchorFinalizedHeadRequestWireV1,
    >(
        &request.payload,
        MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
    )?)?;
    let anchor = broker_backend!(state, privacy_release_anchor);
    let head = anchor.finalized_head(query).map_err(|error| match error {
        sorafs_node::PrivacyReleaseAnchorErrorV1::Unavailable
        | sorafs_node::PrivacyReleaseAnchorErrorV1::Internal => BrokerError::Unavailable,
        sorafs_node::PrivacyReleaseAnchorErrorV1::AuthenticationFailed
        | sorafs_node::PrivacyReleaseAnchorErrorV1::Conflict
        | sorafs_node::PrivacyReleaseAnchorErrorV1::InvalidState => BrokerError::Rejected,
    })?;
    let head = PrivacyReleaseAnchorHeadWireV1::from_head(head);
    if head.query_id != query || head.to_head().is_err() {
        return Err(BrokerError::Rejected);
    }
    requalify()?;
    encode_canonical(&head, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
}

pub(super) fn privacy_release_anchor_compare_and_set(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<PrivacyReleaseAnchorCompareAndSetRequestWireV1>(
        &request.payload,
        MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
    )?;
    let (expected, next, lease) = validate_privacy_release_anchor_compare_and_set(&compare)?;
    let anchor = broker_backend!(state, privacy_release_anchor);
    anchor
        .compare_and_set_finalized_head(expected, next, &lease)
        .map_err(|error| match error {
            sorafs_node::PrivacyReleaseAnchorErrorV1::Conflict => BrokerError::Conflict,
            sorafs_node::PrivacyReleaseAnchorErrorV1::AuthenticationFailed
            | sorafs_node::PrivacyReleaseAnchorErrorV1::InvalidState => BrokerError::Rejected,
            sorafs_node::PrivacyReleaseAnchorErrorV1::Unavailable
            | sorafs_node::PrivacyReleaseAnchorErrorV1::Internal => BrokerError::Ambiguous,
        })?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    let readback = anchor
        .finalized_head(next.query_id())
        .map_err(|_| BrokerError::Ambiguous)?;
    if readback != next {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn transparency_leader_lease_acquire(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let configured = transparency_runtime_binding_from_wire(&request.binding)?;
    let wire = decode_canonical::<TransparencyLeaderLeaseAcquireRequestWireV1>(
        &request.payload,
        MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
    )?;
    let lease_request = validate_transparency_leader_lease_acquire(&wire, &configured)?;
    let provider = broker_backend!(state, transparency_leader_lease_provider);
    let grant = provider
        .acquire(&lease_request)
        .map_err(transparency_leader_lease_provider_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    validate_transparency_leader_lease_acquire_grant(&lease_request, &grant, &configured)
        .map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &TransparencyLeaderLeaseGrantWireV1::from_grant(&grant),
        MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
    )
    .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn transparency_leader_lease_renew(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let configured = transparency_runtime_binding_from_wire(&request.binding)?;
    let wire = decode_canonical::<TransparencyLeaderLeaseRenewRequestWireV1>(
        &request.payload,
        MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
    )?;
    let lease_request = validate_transparency_leader_lease_renew(&wire, &configured)?;
    let provider = broker_backend!(state, transparency_leader_lease_provider);
    let grant = provider
        .renew(&lease_request)
        .map_err(transparency_leader_lease_provider_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    validate_transparency_leader_lease_renew_grant(&lease_request, &grant, &configured)
        .map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &TransparencyLeaderLeaseGrantWireV1::from_grant(&grant),
        MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
    )
    .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn transparency_leader_lease_release(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let configured = transparency_runtime_binding_from_wire(&request.binding)?;
    let wire = decode_canonical::<TransparencyLeaderLeaseReleaseRequestWireV1>(
        &request.payload,
        MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
    )?;
    let lease_request = validate_transparency_leader_lease_release(&wire, &configured)?;
    let provider = broker_backend!(state, transparency_leader_lease_provider);
    let receipt = provider
        .release(&lease_request)
        .map_err(transparency_leader_lease_provider_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    validate_transparency_leader_lease_release_receipt(&lease_request, &receipt, &configured)
        .map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &TransparencyLeaderLeaseReleaseReceiptWireV1::from_receipt(&receipt),
        MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
    )
    .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn fenced_privacy_compare_and_append(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let publish = decode_canonical::<FencedPrivacyPublicationRequestWireV1>(
        &request.payload,
        MAX_FENCED_PRIVACY_PUBLICATION_FRAME_BYTES_V1,
    )?
    .to_request()?;
    let publisher = broker_backend!(state, fenced_privacy_publisher);
    let receipt = publisher
        .compare_and_append_privacy(&publish)
        .map_err(fenced_privacy_publish_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    let qualification = qualification_from_binding(&request.binding)?;
    receipt
        .validate_for_request(&publish, &request.binding.handle, qualification)
        .map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &FencedPrivacyPublicationReceiptWireV1::from_receipt(&receipt),
        MAX_FENCED_PRIVACY_PUBLICATION_FRAME_BYTES_V1,
    )
    .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn fenced_privacy_read_head_with_ancestry(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let (required_ancestors, required_publications) =
        decode_canonical::<FencedPrivacyHeadReadRequestWireV1>(
            &request.payload,
            MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1,
        )?
        .to_required_evidence()?;
    let reader = broker_backend!(state, fenced_privacy_head_reader);
    let proof = reader
        .read_authoritative_head_with_ancestry(&required_ancestors, &required_publications)
        .map_err(|_| BrokerError::Unavailable)?;
    requalify()?;
    let proof_wire = FencedTransparencyHeadAncestryProofWireV1::from_proof(&proof);
    proof_wire
        .to_proof(&required_ancestors, &required_publications)
        .map_err(|_| BrokerError::Rejected)?;
    encode_canonical(&proof_wire, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
}

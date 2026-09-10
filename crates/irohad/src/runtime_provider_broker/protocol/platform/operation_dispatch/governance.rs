//! Governance operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn qualify_governance_authenticator(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let governance_ipfs_auth_slot =
        IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator.wire_id();
    let authenticator = if slot == governance_ipfs_auth_slot {
        state.backends.governance_dag_ipfs_authenticator.as_ref()
    } else {
        state.backends.governance_dag_head_authenticator.as_ref()
    }
    .ok_or(BrokerError::BindingMismatch)?;
    let qualification = authenticator
        .ingress_qualification()
        .map_err(|_| BrokerError::StaleOrRevoked)?;
    let expected_binding =
        governance_request_ingress_binding_from_provider_binding(&request.binding)?;
    if authenticator.handle() != request.binding.handle
        || !qualification_matches(
            &request.binding,
            qualification.provider().revision,
            qualification.provider().policy_digest,
        )
        || qualification.binding() != expected_binding
    {
        return Err(BrokerError::BindingMismatch);
    }
    encode_canonical(
        &governance_request_ingress_qualification_to_wire(qualification),
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let sign = decode_canonical::<PurposeSignRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    let purpose = validate_governance_purpose_signing_request(&sign, &request.binding)?;
    let signer = broker_backend!(state, governance_dag_signer);
    let signature = signer
        .sign(purpose, &sign.payload)
        .map_err(|_| BrokerError::Rejected)?;
    if signature == [0; 64] {
        return Err(BrokerError::Rejected);
    }
    requalify()?;
    encode_canonical(
        &SignResultWireV1 { signature },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn governance_request_authenticate(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let governance_ipfs_auth_slot =
        IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator.wire_id();
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<GovernanceRequestAuthRequestWireV1>(
        &request.payload,
        MAX_GOVERNANCE_REQUEST_AUTH_FRAME_BYTES_V1,
    )?;
    let ingress = governance_request_ingress_binding_from_provider_binding(&request.binding)?;
    let descriptor = governance_request_auth_from_wire(&wire, ingress.max_body_bytes())?;
    let (authenticator, expected_scope) = if slot == governance_ipfs_auth_slot {
        (
            state.backends.governance_dag_ipfs_authenticator.as_ref(),
            sorafs_node::GovernanceDagAuthenticationScope::Ipfs,
        )
    } else {
        (
            state.backends.governance_dag_head_authenticator.as_ref(),
            sorafs_node::GovernanceDagAuthenticationScope::SignedHead,
        )
    };
    if descriptor.scope() != expected_scope || descriptor.scope() != ingress.scope() {
        return Err(BrokerError::BindingMismatch);
    }
    let authenticator = authenticator.ok_or(BrokerError::BindingMismatch)?;
    let envelope = authenticator
        .authenticate(&descriptor)
        .map_err(|_| BrokerError::Unavailable)?;
    let envelope = validate_governance_request_auth_envelope(
        &descriptor,
        governance_request_auth_result_to_wire(&envelope),
        ingress.public_key(),
    )?;
    requalify()?;
    encode_canonical(
        &governance_request_auth_result_to_wire(&envelope),
        MAX_GOVERNANCE_REQUEST_AUTH_FRAME_BYTES_V1,
    )
}

pub(super) fn sealed_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let load = decode_canonical::<SealedLoadRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    let sealed_slot = sealed_slot_from_wire(load.slot)?;
    let store = broker_backend!(state, governance_dag_checkpoint_store);
    let record = store
        .load(sealed_slot)
        .map_err(|_| BrokerError::Unavailable)?
        .map(|record| SealedRecordWireV1 {
            generation: record.generation,
            revision: record.revision,
            payload: record.payload,
        });
    if let Some(record) = record.as_ref() {
        validate_sealed_record_fields(
            sealed_slot,
            record.generation,
            record.revision,
            &record.payload,
        )
        .map_err(|_| BrokerError::Protocol)?;
    }
    requalify()?;
    encode_canonical(&record, MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn sealed_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<SealedCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    let sealed_slot = sealed_slot_from_wire(compare.slot)?;
    let next = sorafs_node::GovernanceDagSealedStateRecord {
        generation: compare.next.generation,
        revision: compare.next.revision,
        payload: compare.next.payload,
    };
    let store = broker_backend!(state, governance_dag_checkpoint_store);
    let current = store
        .load(sealed_slot)
        .map_err(|_| BrokerError::Unavailable)?;
    validate_sealed_successor(
        sealed_slot,
        current.as_ref(),
        compare.expected_revision,
        &next,
    )?;
    store
        .compare_and_swap(sealed_slot, compare.expected_revision, next.clone())
        .map_err(|_| BrokerError::Ambiguous)?;
    let readback = store
        .load(sealed_slot)
        .map_err(|_| BrokerError::Ambiguous)?;
    if readback.as_ref() != Some(&next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn sealed_delete(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let delete = decode_canonical::<SealedDeleteRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    let sealed_slot = sealed_slot_from_wire(delete.slot)?;
    validate_sealed_delete(sealed_slot, delete.expected_revision)?;
    let store = broker_backend!(state, governance_dag_checkpoint_store);
    let current = store
        .load(sealed_slot)
        .map_err(|_| BrokerError::Unavailable)?;
    if let Some(current) = current.as_ref() {
        validate_sealed_record_fields(
            sealed_slot,
            current.generation,
            current.revision,
            &current.payload,
        )
        .map_err(|_| BrokerError::Protocol)?;
    }
    if current.as_ref().map(|record| record.revision) != Some(delete.expected_revision) {
        return Err(BrokerError::Conflict);
    }
    store
        .delete(sealed_slot, delete.expected_revision)
        .map_err(|_| BrokerError::Ambiguous)?;
    if store
        .load(sealed_slot)
        .map_err(|_| BrokerError::Ambiguous)?
        .is_some()
    {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
}

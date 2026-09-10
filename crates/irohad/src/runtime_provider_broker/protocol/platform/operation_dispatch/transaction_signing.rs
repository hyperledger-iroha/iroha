//! Transaction signing operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn qualify_soracloud_signer(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let signer = qualified_soracloud_runtime_signer(state, &request.binding)?;
    let qualification = signer
        .qualification()
        .map_err(|_| BrokerError::Unavailable)?;
    encode_canonical(
        &SoracloudSignerQualificationWireV1 {
            revision: qualification.revision(),
            policy_digest: qualification.policy_digest(),
            active: qualification.active(),
            test_only: qualification.test_only(),
        },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn moderation_transaction_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let payload = decode_native_transaction_payload(&request.payload)?;
    ensure_transaction_session_network(&payload, &state.network_id)?;
    let signed = sign_moderation_transaction(state, &payload)?;
    requalify()?;
    encode_canonical(&signed, MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1)
}

pub(super) fn native_transaction_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let payload = decode_native_transaction_payload(&request.payload)?;
    ensure_transaction_session_network(&payload, &state.network_id)?;
    let signed = sign_native_transaction(state, &request.binding, payload)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&signed, MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn soracloud_transaction_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let payload = decode_native_transaction_payload(&request.payload)?;
    ensure_transaction_session_network(&payload, &state.network_id)?;
    let backend = qualified_soracloud_runtime_signer(state, &request.binding)?;
    let transaction = backend
        .sign_transaction(payload)
        .map_err(map_soracloud_runtime_signing_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&transaction, MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn soracloud_provenance_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let request_payload = decode_canonical::<SoracloudProvenanceSignRequestWireV1>(
        &request.payload,
        MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1,
    )?;
    let purpose =
        iroha_data_model::soracloud::SoracloudRuntimeProvenancePurposeV1::try_from_wire_id(
            request_payload.purpose,
        )
        .map_err(|_| BrokerError::Rejected)?;
    iroha_data_model::soracloud::validate_soracloud_runtime_provenance_preimage_v1(
        purpose,
        &request_payload.preimage,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let signer = qualified_soracloud_runtime_signer(state, &request.binding)?;
    let signature = signer
        .sign_provenance(purpose, &request_payload.preimage)
        .map_err(map_soracloud_runtime_signing_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&signature, MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

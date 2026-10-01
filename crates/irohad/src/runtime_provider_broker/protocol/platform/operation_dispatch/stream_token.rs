//! Stream token operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn stream_token_check(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let instruction = decode_stream_token_check_request(&request.binding, &request.payload)?;
    let observer = broker_backend!(state, stream_token_state_observer);
    let signed = observer
        .finalize_check(&instruction)
        .map_err(|error| stream_token_backend_error(error, true))?;
    let bytes = encode_canonical(&signed, 32 * 1024).map_err(|_| BrokerError::Ambiguous)?;
    decode_stream_token_check_result(&request.binding, &request.payload, &bytes)
        .map_err(|_| BrokerError::Ambiguous)?;
    qualify_server_binding(state, &request.binding, request.provider_metadata_digest)
        .map_err(|_| BrokerError::Ambiguous)?;
    Ok(bytes)
}

pub(super) fn stream_token_sign_or_recover(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let (body, expected) = prepare_stream_token_broker_request(&request.binding, &request.payload)?;
    let client = broker_backend!(state, stream_token_signer_client);
    let mutating = request.operation == OPERATION_STREAM_TOKEN_SIGN_V1;
    let receipt = if mutating {
        client.sign(&expected, &body)
    } else {
        client.recover(&expected, &body)
    }
    .map_err(|error| stream_token_backend_error(error, mutating))?;
    validate_stream_token_receipt_result(request, receipt.bytes()).map_err(|_| {
        if mutating {
            BrokerError::Ambiguous
        } else {
            BrokerError::Protocol
        }
    })?;
    requalify().map_err(|error| {
        if mutating {
            BrokerError::Ambiguous
        } else {
            error
        }
    })?;
    Ok(receipt.bytes().to_vec())
}

pub(super) fn stream_token_observe(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let query = decode_stream_token_observer_request(&request.binding, &request.payload)?;
    let observer = broker_backend!(state, stream_token_state_observer);
    let reply = observer
        .observe(&query)
        .map_err(|error| stream_token_backend_error(error, false))?;
    let encoded = encode_stream_token_observer_reply(&query, &reply)?;
    requalify()?;
    Ok(encoded)
}

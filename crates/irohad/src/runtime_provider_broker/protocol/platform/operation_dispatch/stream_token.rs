//! Stream token operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn stream_token_sign_or_recover(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let (body, expected) = prepare_stream_token_broker_request(&request.binding, &request.payload)?;
    let client = broker_backend!(state, stream_token_hardware_client);
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

pub(super) fn stream_token_gateway_admit(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let admission = decode_canonical::<iroha_torii::sorafs::StreamTokenGatewayAdmissionRequestV1>(
        &request.payload,
        MAX_BROKER_UNARY_FRAME_BYTES_V1,
    )?;
    admission.validate().map_err(|_| BrokerError::Rejected)?;
    let provider = broker_backend!(state, stream_token_gateway_admission);
    let admission_result = provider
        .admit(&admission)
        .map_err(stream_token_gateway_provider_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    let qualification = required_binding_value!(
        &request.binding,
        stream_token_gateway_admission_qualification
    );
    admission_result
        .validate_for_request(&admission, qualification)
        .map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&admission_result, MAX_BROKER_UNARY_FRAME_BYTES_V1)
}

pub(super) fn stream_token_gateway_pending(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let max_items = decode_canonical::<u32>(&request.payload, MAX_BROKER_UNARY_FRAME_BYTES_V1)?;
    let configured = required_binding_value!(
        &request.binding,
        stream_token_gateway_admission_reconcile_max_items
    );
    if max_items == 0 || max_items > configured {
        return Err(BrokerError::Rejected);
    }
    let provider = broker_backend!(state, stream_token_gateway_admission);
    let pending = provider
        .pending(max_items)
        .map_err(stream_token_gateway_provider_error)?;
    let qualification = required_binding_value!(
        &request.binding,
        stream_token_gateway_admission_qualification
    );
    pending
        .validate(max_items, qualification)
        .map_err(|_| BrokerError::Rejected)?;
    requalify()?;
    encode_canonical(&pending, MAX_BROKER_UNARY_FRAME_BYTES_V1)
}

pub(super) fn stream_token_gateway_complete(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let record = decode_canonical::<iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1>(
        &request.payload,
        MAX_BROKER_UNARY_FRAME_BYTES_V1,
    )?;
    let qualification = required_binding_value!(
        &request.binding,
        stream_token_gateway_admission_qualification
    );
    record
        .validate_shape(qualification)
        .map_err(|_| BrokerError::Rejected)?;
    let provider = broker_backend!(state, stream_token_gateway_admission);
    let outcome = if request.operation == OPERATION_STREAM_TOKEN_GATEWAY_ACKNOWLEDGE_V1 {
        provider.acknowledge(record)
    } else {
        provider.release_lease(record)
    }
    .map_err(stream_token_gateway_provider_error)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&outcome, MAX_BROKER_UNARY_FRAME_BYTES_V1)
}

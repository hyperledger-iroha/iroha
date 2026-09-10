//! Por archive operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn por_replay_archive_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    decode_canonical::<()>(
        &request.payload,
        MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
    )?;
    broker_backend!(state, por_finalized_replay_archive)
        .check_readiness()
        .map_err(|error| match error {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                BrokerError::Unavailable
            }
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
        })?;
    requalify()?;
    encode_canonical(&(), MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn por_replay_archive_current_head(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    decode_canonical::<()>(
        &request.payload,
        MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
    )?;
    let exact = por_replay_archive_exact_binding(&request.binding)?;
    let head = broker_backend!(state, por_finalized_replay_archive)
        .current_head()
        .map_err(|error| match error {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                BrokerError::Unavailable
            }
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
        })?;
    if let Some(head) = head {
        validate_por_replay_archive_receipt(&head, exact)?;
    }
    requalify()?;
    encode_canonical(&head, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn por_replay_archive_append(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let append = decode_canonical::<PorReplayArchiveAppendRequestWireV1>(
        &request.payload,
        MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1,
    )?;
    let record = validate_por_replay_archive_append_request(&append)?;
    let exact = por_replay_archive_exact_binding(&request.binding)?;
    let (_, configured_bounds) = por_replay_archive_configured_proof_bounds(&request.binding)?;
    let archive = broker_backend!(state, por_finalized_replay_archive);
    let receipt = archive
        .append(&record, append.expected_previous_head)
        .map_err(|error| match error {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                BrokerError::Ambiguous
            }
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
        })?;
    receipt
        .validate_record(exact, &record, Some(append.expected_previous_head))
        .map_err(|_| BrokerError::Ambiguous)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    let head = archive
        .current_head()
        .map_err(|_| BrokerError::Ambiguous)?
        .ok_or(BrokerError::Ambiguous)?;
    validate_por_replay_archive_receipt(&head, exact).map_err(|_| BrokerError::Ambiguous)?;
    if head != receipt {
        if head.reputation_sequence() <= receipt.reputation_sequence() {
            return Err(BrokerError::Ambiguous);
        }
        let readback = archive
            .lookup(record.challenge_id(), head, configured_bounds)
            .map_err(|_| BrokerError::Ambiguous)?;
        match readback {
            sorafs_node::PorFinalizedReplayArchiveLookupV1::Found(readback) => {
                if readback.record != record || readback.receipt != receipt {
                    return Err(BrokerError::Ambiguous);
                }
                readback
                    .validate_at_checkpoint(exact, head, configured_bounds)
                    .map_err(|_| BrokerError::Ambiguous)?;
            }
            sorafs_node::PorFinalizedReplayArchiveLookupV1::Absent(_) => {
                return Err(BrokerError::Ambiguous);
            }
        }
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&receipt, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn por_replay_archive_lookup(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let lookup = decode_canonical::<PorReplayArchiveLookupRequestWireV1>(
        &request.payload,
        MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
    )?;
    let bounds = validate_por_replay_archive_lookup_request(&lookup, &request.binding)?;
    let exact = por_replay_archive_exact_binding(&request.binding)?;
    let outcome = broker_backend!(state, por_finalized_replay_archive)
        .lookup(lookup.challenge_id, lookup.expected_checkpoint_head, bounds)
        .map_err(|error| match error {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                BrokerError::Unavailable
            }
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                BrokerError::Rejected
            }
        })?;
    let outcome = por_replay_archive_lookup_to_wire(outcome, &lookup, exact, bounds)?;
    requalify()?;
    encode_canonical(&outcome, MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1)
}

//! Appeal finance operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn appeal_finance_transaction_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let payload = decode_transaction_payload_bounded(
        &request.payload,
        MAX_APPEAL_FINANCE_TRANSACTION_BYTES_V1,
    )?;
    ensure_transaction_session_network(&payload, &state.network_id)?;
    let expected = payload.clone();
    let exact = required_binding_ref!(&request.binding, appeal_finance_signer_binding);
    if payload.authority() != &exact.authority {
        return Err(BrokerError::Rejected);
    }
    let backend = appeal_finance_signer_backend(&state.backends, &request.binding.handle)
        .map_err(|_| BrokerError::BindingMismatch)?;
    let transaction = backend.sign(payload).map_err(|error| match error {
        iroha_torii::SoraFsAppealFinanceSigningError::Unavailable => BrokerError::Unavailable,
        iroha_torii::SoraFsAppealFinanceSigningError::Refused => BrokerError::Rejected,
        iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged => {
            BrokerError::StaleOrRevoked
        }
    })?;
    if transaction.payload() != &expected
        || transaction.authority() != &exact.authority
        || transaction.verify_signature().is_err()
    {
        return Err(BrokerError::Rejected);
    }
    requalify().map_err(|_| BrokerError::StaleOrRevoked)?;
    encode_canonical(&transaction, MAX_APPEAL_FINANCE_TRANSACTION_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Protocol)
}

pub(super) fn appeal_finance_checkpoint_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let digest = decode_canonical::<[u8; 32]>(&request.payload, MAX_STREAM_TOKEN_FRAME_BYTES_V1)?;
    if digest == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    let checkpoint = broker_backend!(state, appeal_finance_checkpoint);
    let signature = checkpoint.sign_digest(digest).map_err(|error| {
        match error {
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Unavailable
        | sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Ambiguous => {
                BrokerError::Unavailable
            }
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Rejected => {
                BrokerError::Rejected
            }
        }
    })?;
    let public_key = exact_ed25519_public_key_bytes(
        &required_binding_ref!(&request.binding, appeal_finance_checkpoint_binding).public_key,
    )?;
    verify_evidence_viewer_ed25519_signature(public_key, signature, &digest)
        .map_err(|_| BrokerError::Rejected)?;
    requalify()?;
    encode_canonical(
        &SignResultWireV1 { signature },
        MAX_STREAM_TOKEN_FRAME_BYTES_V1,
    )
}

pub(super) fn appeal_finance_checkpoint_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    decode_canonical::<()>(&request.payload, MAX_STREAM_TOKEN_FRAME_BYTES_V1)?;
    let checkpoint_max =
        required_binding_value!(&request.binding, appeal_finance_checkpoint_max_bytes);
    let record = broker_backend!(state, appeal_finance_checkpoint)
        .load_latest()
        .map_err(|error| {
            match error {
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Unavailable
            | sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Ambiguous => {
                    BrokerError::Unavailable
                }
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Rejected => {
                    BrokerError::Rejected
                }
            }
        })?;
    if let Some(record) = record.as_ref() {
        record
            .validate(checkpoint_max)
            .map_err(|_| BrokerError::Rejected)?;
    }
    requalify()?;
    encode_canonical(&record, MAX_APPEAL_FINANCE_CHECKPOINT_FRAME_BYTES_V1)
}

pub(super) fn appeal_finance_checkpoint_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<AppealFinanceCheckpointCompareAndSwapWireV1>(
        &request.payload,
        MAX_APPEAL_FINANCE_CHECKPOINT_FRAME_BYTES_V1,
    )?;
    let checkpoint_max =
        required_binding_value!(&request.binding, appeal_finance_checkpoint_max_bytes);
    compare
        .next
        .validate(checkpoint_max)
        .map_err(|_| BrokerError::Rejected)?;
    let checkpoint = broker_backend!(state, appeal_finance_checkpoint);
    let current = checkpoint
        .load_latest()
        .map_err(|_| BrokerError::Unavailable)?;
    if current.as_ref().map(|record| record.revision) != compare.expected_revision {
        return Err(BrokerError::Conflict);
    }
    let monotonic = match current.as_ref() {
        None => compare.expected_revision.is_none() && compare.next.checkpoint_sequence == 1,
        Some(record) => record
            .checkpoint_sequence
            .checked_add(1)
            .is_some_and(|sequence| sequence == compare.next.checkpoint_sequence),
    };
    if !monotonic {
        return Err(BrokerError::Rejected);
    }
    checkpoint
        .compare_and_swap_latest(compare.expected_revision, &compare.next)
        .map_err(|error| {
            match error {
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Unavailable => {
                BrokerError::Unavailable
            }
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Rejected => {
                BrokerError::Rejected
            }
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Ambiguous => {
                BrokerError::Ambiguous
            }
    }
        })?;
    if checkpoint
        .load_latest()
        .map_err(|_| BrokerError::Ambiguous)?
        .as_ref()
        != Some(&compare.next)
    {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_STREAM_TOKEN_FRAME_BYTES_V1)
}

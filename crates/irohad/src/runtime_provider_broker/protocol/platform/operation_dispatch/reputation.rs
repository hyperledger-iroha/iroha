//! Reputation operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn reputation_journal_supports_authority(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let supports = decode_canonical::<ReputationJournalSupportsAuthorityRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )?;
    let submitter = broker_backend!(state, reputation_journal_transaction_submitter);
    let supported = submitter.supports_authority(&supports.authority);
    requalify()?;
    encode_canonical(&supported, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn reputation_journal_submit(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<ReputationJournalTransactionRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )?;
    let submit = reputation_journal_request_from_wire(wire)?;
    ensure_reputation_session_network(&submit.network_id, &state.network_id)?;
    let submitter = broker_backend!(state, reputation_journal_transaction_submitter);
    if !submitter.supports_authority(&submit.authority) {
        return Err(BrokerError::Rejected);
    }
    let outcome = reputation_journal_submit_result_to_wire(submitter.submit(&submit))?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&outcome, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn reputation_threshold_reconcile(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<ReputationThresholdSigningRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )?;
    let signing = reputation_threshold_request_from_wire(wire)?;
    ensure_reputation_session_network(&signing.material.network_id, &state.network_id)?;
    let threshold_signer = broker_backend!(state, reputation_threshold_signer);
    let reconciled = threshold_signer.reconcile_signature(&signing);
    let result = match reconciled {
        Ok(None) => ReputationReconcileResultWireV1 {
            outcome: 0,
            canonical_result: Vec::new(),
            failure_receipt: [0; 32],
        },
        Ok(Some(signature)) => ReputationReconcileResultWireV1 {
            outcome: 1,
            canonical_result: validate_reputation_signature(&signing, &signature)?,
            failure_receipt: [0; 32],
        },
        Err(error) => ReputationReconcileResultWireV1 {
            outcome: 2,
            canonical_result: Vec::new(),
            failure_receipt: error.receipt(),
        },
    };
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&result, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn reputation_governance_reconcile(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<ReputationGovernanceDagPublicationRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )?;
    let publication = reputation_governance_request_from_wire(wire)?;
    let governance_dag = broker_backend!(state, reputation_governance_dag);
    let reconciled = governance_dag.reconcile_publication(&publication);
    let result = match reconciled {
        Ok(None) => ReputationReconcileResultWireV1 {
            outcome: 0,
            canonical_result: Vec::new(),
            failure_receipt: [0; 32],
        },
        Ok(Some(readback)) => {
            validate_reputation_governance_readback(&readback, &publication.signed_result)?;
            ReputationReconcileResultWireV1 {
                outcome: 1,
                canonical_result: encode_canonical(
                    &readback,
                    MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
                )?,
                failure_receipt: [0; 32],
            }
        }
        Err(error) => ReputationReconcileResultWireV1 {
            outcome: 2,
            canonical_result: Vec::new(),
            failure_receipt: error.receipt(),
        },
    };
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&result, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn reputation_journal_checkpoint_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let checkpoint = broker_backend!(state, reputation_journal_checkpoint);
    let record = checkpoint.load_latest().map_err(|error| {
        match error {
    sorafs_node::reputation::runtime::
        ReputationJournalCheckpointExternalErrorV1::Unavailable => {
        BrokerError::Unavailable
    }
    sorafs_node::reputation::runtime::
        ReputationJournalCheckpointExternalErrorV1::Rejected => {
        BrokerError::Rejected
    }
    sorafs_node::reputation::runtime::
        ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
        BrokerError::Protocol
    }
}
    })?;
    let record = record
        .map(|record| {
            record
            .to_canonical_bytes(
                sorafs_node::reputation::runtime::
                    REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1,
            )
            .map_err(|_| BrokerError::Protocol)
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&record, MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1)
}

pub(super) fn reputation_journal_checkpoint_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<ReputationJournalCheckpointCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
    )?;
    let next = sorafs_node::reputation::runtime::
    ReputationJournalSealedCheckpointRecordV1::from_canonical_bytes(
        &compare.next_record,
        sorafs_node::reputation::runtime::
            REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let checkpoint = broker_backend!(state, reputation_journal_checkpoint);
    let current = checkpoint.load_latest().map_err(|error| {
        match error {
    sorafs_node::reputation::runtime::
        ReputationJournalCheckpointExternalErrorV1::Unavailable => {
        BrokerError::Unavailable
    }
    sorafs_node::reputation::runtime::
        ReputationJournalCheckpointExternalErrorV1::Rejected => {
        BrokerError::Rejected
    }
    sorafs_node::reputation::runtime::
        ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
        BrokerError::Protocol
    }
}
    })?;
    let monotonic = match &current {
        None => {
            compare.expected_revision.is_none()
                && next.checkpoint_sequence() == 1
                && next.predecessor_checkpoint_digest().is_none()
        }
        Some(previous) => {
            compare.expected_revision == Some(previous.revision())
                && previous
                    .checkpoint_sequence()
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == next.checkpoint_sequence())
                && next.predecessor_checkpoint_digest() == Some(previous.checkpoint_digest())
        }
    };
    if !monotonic {
        return Err(BrokerError::Rejected);
    }
    checkpoint
        .compare_and_swap_latest(compare.expected_revision, &next)
        .map_err(|error| {
            match error {
        sorafs_node::reputation::runtime::
            ReputationJournalCheckpointExternalErrorV1::Unavailable
        | sorafs_node::reputation::runtime::
            ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
            BrokerError::Ambiguous
        }
        sorafs_node::reputation::runtime::
            ReputationJournalCheckpointExternalErrorV1::Rejected => {
            BrokerError::Rejected
        }
    }
        })?;
    let readback = checkpoint
        .load_latest()
        .map_err(|_| BrokerError::Ambiguous)?;
    if readback.as_ref() != Some(&next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1)
}

pub(super) fn reputation_retention_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let load = decode_canonical::<ReputationRetentionLoadRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_RETENTION_FRAME_BYTES_V1,
    )?;
    if state.network_id != load.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    let authority = broker_backend!(state, reputation_finalized_archive_retention_authority);
    let record = authority.load_latest(&load.network_id).map_err(|error| {
        match error {
    iroha_core::query::reputation_finalized::
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Unavailable => BrokerError::Unavailable,
    iroha_core::query::reputation_finalized::
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Rejected => BrokerError::Rejected,
    iroha_core::query::reputation_finalized::
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Ambiguous => BrokerError::Protocol,
}
    })?;
    let record = record
        .map(|record| {
            let bytes = record
                .to_canonical_bytes()
                .map_err(|_| BrokerError::Protocol)?;
            if bytes.is_empty() || bytes.len() > MAX_REPUTATION_RETENTION_APPROVAL_BYTES_V1 {
                return Err(BrokerError::Protocol);
            }
            Ok(bytes)
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&record, MAX_REPUTATION_RETENTION_FRAME_BYTES_V1)
}

pub(super) fn reputation_retention_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<ReputationRetentionCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_REPUTATION_RETENTION_FRAME_BYTES_V1,
    )?;
    if state.network_id != compare.network_id
        || compare.expected_revision == Some([0; 32])
        || compare.next_record.is_empty()
        || compare.next_record.len() > MAX_REPUTATION_RETENTION_APPROVAL_BYTES_V1
    {
        return Err(BrokerError::BindingMismatch);
    }
    reserve_external_canonical_decode(
        compare.next_record.len(),
        MAX_REPUTATION_RETENTION_APPROVAL_BYTES_V1,
    )?;
    let next = iroha_core::query::reputation_finalized::
    ReputationFinalizedArchiveRetentionApprovalRecordV1::
        from_canonical_bytes(&compare.next_record)
        .map_err(|_| BrokerError::Rejected)?;
    let expected_qualification = qualification_from_binding(&request.binding)?;
    let next_qualification = next.authority_qualification();
    if next_qualification.revision() != expected_qualification.revision
        || next_qualification.policy_digest() != expected_qualification.policy_digest
        || next.predecessor_revision() != compare.expected_revision
    {
        return Err(BrokerError::BindingMismatch);
    }
    let authority = broker_backend!(state, reputation_finalized_archive_retention_authority);
    let current = authority
        .load_latest(&compare.network_id)
        .map_err(|error| {
            match error {
    iroha_core::query::reputation_finalized::
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Unavailable => BrokerError::Unavailable,
    iroha_core::query::reputation_finalized::
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Rejected => BrokerError::Rejected,
    iroha_core::query::reputation_finalized::
        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Ambiguous => BrokerError::Protocol,
}
        })?;
    let monotonic = match &current {
        None => {
            compare.expected_revision.is_none()
                && next.sequence() == 1
                && next.predecessor_revision().is_none()
                && next.predecessor_checkpoint_digest().is_none()
        }
        Some(previous) => {
            previous.authority_qualification() == next_qualification
                && compare.expected_revision == Some(previous.revision())
                && previous
                    .sequence()
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == next.sequence())
                && next.predecessor_revision() == Some(previous.revision())
                && next.predecessor_checkpoint_digest()
                    == Some(previous.proposal().checkpoint_digest())
        }
    };
    if !monotonic {
        return Err(BrokerError::Rejected);
    }
    authority
        .compare_and_swap_latest(&compare.network_id, compare.expected_revision, &next)
        .map_err(|error| {
            match error {
        iroha_core::query::reputation_finalized::
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
            Unavailable
        | iroha_core::query::reputation_finalized::
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
            Ambiguous => BrokerError::Ambiguous,
        iroha_core::query::reputation_finalized::
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
            Rejected => BrokerError::Rejected,
    }
        })?;
    let readback = authority
        .load_latest(&compare.network_id)
        .map_err(|_| BrokerError::Ambiguous)?;
    if readback.as_ref() != Some(&next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_REPUTATION_RETENTION_FRAME_BYTES_V1)
}

//! Provider ingest operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn qualify_source(state: &BrokerServerStateV1) -> Result<Vec<u8>, BrokerError> {
    let source = broker_backend!(state, provider_ingest_authenticated_source);
    let qualification = source
        .qualification()
        .map_err(|_| BrokerError::StaleOrRevoked)?;
    encode_canonical(
        &ProviderIngestRuntimeQualificationWireV1 {
            revision: qualification.revision,
            policy_digest: qualification.policy_digest,
        },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn provider_ingest_source_readiness(
    state: &BrokerServerStateV1,
) -> Result<Vec<u8>, BrokerError> {
    broker_backend!(state, provider_ingest_authenticated_source)
        .check_readiness()
        .map_err(|error| match error {
            sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable => BrokerError::Unavailable,
            sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected => BrokerError::Rejected,
            sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected => BrokerError::StaleOrRevoked,
        })?;
    encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn qualify_resolver(state: &BrokerServerStateV1) -> Result<Vec<u8>, BrokerError> {
    let resolver = broker_backend!(state, provider_ingest_signer_resolver);
    let qualification = resolver
        .qualification()
        .map_err(|_| BrokerError::StaleOrRevoked)?;
    let signer_binding = resolver
        .signer_binding()
        .map_err(|_| BrokerError::StaleOrRevoked)?;
    let signer_binding = ProviderIngestSignerBindingWireV1::try_from_binding(&signer_binding)
        .map_err(|_| BrokerError::StaleOrRevoked)?;
    encode_canonical(
        &ProviderIngestResolverQualificationWireV1 {
            revision: qualification.revision,
            policy_digest: qualification.policy_digest,
            signer_binding,
        },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn provider_ingest_resolver_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let resolver = broker_backend!(state, provider_ingest_signer_resolver);
    resolver.check_readiness().map_err(|error| match error {
        sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Unavailable => {
            BrokerError::Unavailable
        }
        sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Rejected => {
            BrokerError::Rejected
        }
    })?;
    requalify()?;
    encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn provider_ingest_resolve_signer(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let resolve = decode_canonical::<ProviderIngestResolveSignerRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    let context = provider_ingest_signer_context_from_wire(&resolve.context)?;
    let expected = provider_ingest_expected_signer_binding(&request.binding)?;
    if !expected
        .qualification
        .matches_authority(&context.provider_owner)
        || expected.qualification.signer_policy != context.signer_policy
    {
        return Err(BrokerError::BindingMismatch);
    }
    let signer = resolved_provider_signer(state, context.clone())?;
    if let Some(signer) = &signer {
        validate_resolved_provider_signer(signer.as_ref(), &expected, &context.provider_owner)?;
    }
    requalify()?;
    encode_canonical(
        &ProviderIngestResolveSignerResultWireV1 {
            eligible: signer.is_some(),
        },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn provider_ingest_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let (context, expected, payload) = decode_provider_ingest_sign_operation(request)?;
    let max_signed = usize::try_from(required_binding_value!(
        &request.binding,
        provider_ingest_max_signed_transaction_bytes
    ))
    .map_err(|_| BrokerError::Rejected)?;
    ensure_provider_ingest_completion_payload(&payload, &context, &state.network_id)?;
    let backend = resolved_provider_signer(state, context.clone())?.ok_or(BrokerError::Rejected)?;
    validate_resolved_provider_signer(backend.as_ref(), &expected, &context.provider_owner)?;
    let transaction =
        block_on_provider_future(backend.sign(payload.clone()))?.map_err(|error| match error {
            sorafs_node::ProviderIngestCompletionSignerErrorV1::Unavailable => {
                BrokerError::Unavailable
            }
            sorafs_node::ProviderIngestCompletionSignerErrorV1::Rejected => BrokerError::Rejected,
        })?;
    validate_resolved_provider_signer(backend.as_ref(), &expected, &context.provider_owner)?;
    if transaction.payload() != &payload {
        return Err(BrokerError::Rejected);
    }
    ensure_provider_ingest_completion_transaction(&transaction, &context, &state.network_id)?;
    requalify()?;
    let signed_transaction = encode_canonical(&transaction, max_signed)?;
    encode_canonical(
        &ProviderIngestSignResultWireV1 { signed_transaction },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn provider_ingest_checkpoint_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let store = broker_backend!(state, provider_ingest_checkpoint_store);
    let max_bytes = required_binding_value!(&request.binding, provider_ingest_checkpoint_max_bytes);
    let record = store.load_latest().map_err(|error| match error {
        sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable => {
            BrokerError::Unavailable
        }
        sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected => BrokerError::Rejected,
        sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => BrokerError::Protocol,
    })?;
    let record = record
        .map(|record| {
            record
                .to_canonical_bytes(max_bytes)
                .map_err(|_| BrokerError::Protocol)
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&record, MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn provider_ingest_checkpoint_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<ProviderIngestCheckpointCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    let max_bytes = required_binding_value!(&request.binding, provider_ingest_checkpoint_max_bytes);
    let max_bytes_limit = usize::try_from(max_bytes).map_err(|_| BrokerError::Rejected)?;
    reserve_external_canonical_decode(compare.next_record.len(), max_bytes_limit)?;
    let next = sorafs_node::ProviderIngestSealedCheckpointRecordV1::from_canonical_bytes(
        &compare.next_record,
        max_bytes,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let store = broker_backend!(state, provider_ingest_checkpoint_store);
    let current = store.load_latest().map_err(|error| match error {
        sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable => {
            BrokerError::Unavailable
        }
        sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected => BrokerError::Rejected,
        sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => BrokerError::Protocol,
    })?;
    let monotonic = match &current {
        None => {
            compare.expected_revision.is_none()
                && next.checkpoint_sequence == 1
                && next.predecessor_revision.is_none()
                && next.predecessor_checkpoint_digest.is_none()
        }
        Some(previous) => {
            compare.expected_revision == Some(previous.revision)
                && previous
                    .checkpoint_sequence
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == next.checkpoint_sequence)
                && next.predecessor_revision == Some(previous.revision)
                && next.predecessor_checkpoint_digest == Some(previous.checkpoint_digest)
        }
    };
    if !monotonic {
        return Err(BrokerError::Rejected);
    }
    store
        .compare_and_swap_latest(compare.expected_revision, &next)
        .map_err(|error| match error {
            sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable
            | sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => {
                BrokerError::Ambiguous
            }
            sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected => BrokerError::Rejected,
        })?;
    let readback = store.load_latest().map_err(|_| BrokerError::Ambiguous)?;
    if readback.as_ref() != Some(&next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn provider_ingest_retention_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let load = decode_canonical::<ProviderIngestRetentionLoadRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    if state.network_id != load.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    let authority = broker_backend!(state, provider_ingest_retention_authority);
    let record = authority.load_latest(&load.network_id).map_err(|error| {
        match error {
    iroha_core::query::provider_ingest_finalized::
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Unavailable => BrokerError::Unavailable,
    iroha_core::query::provider_ingest_finalized::
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Rejected => BrokerError::Rejected,
    iroha_core::query::provider_ingest_finalized::
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Ambiguous => BrokerError::Protocol,
}
    })?;
    let record = record
        .map(|record| {
            record
                .to_canonical_bytes()
                .map_err(|_| BrokerError::Protocol)
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&record, MAX_OPERATION_FRAME_BYTES_V1)
}

pub(super) fn provider_ingest_retention_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<ProviderIngestRetentionCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )?;
    if state.network_id != compare.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    reserve_external_canonical_decode(
        compare.next_record.len(),
        MAX_PROVIDER_INGEST_RETENTION_APPROVAL_BYTES_V1,
    )?;
    let next = iroha_core::query::provider_ingest_finalized::
    ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::
        from_canonical_bytes(&compare.next_record)
        .map_err(|_| BrokerError::Rejected)?;
    let authority = broker_backend!(state, provider_ingest_retention_authority);
    let current = authority
        .load_latest(&compare.network_id)
        .map_err(|error| {
            match error {
    iroha_core::query::provider_ingest_finalized::
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Unavailable => BrokerError::Unavailable,
    iroha_core::query::provider_ingest_finalized::
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
        Rejected => BrokerError::Rejected,
    iroha_core::query::provider_ingest_finalized::
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
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
            compare.expected_revision == Some(previous.revision())
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
        iroha_core::query::provider_ingest_finalized::
            ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
            Unavailable
        | iroha_core::query::provider_ingest_finalized::
            ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
            Ambiguous => BrokerError::Ambiguous,
        iroha_core::query::provider_ingest_finalized::
            ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
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
    encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
}

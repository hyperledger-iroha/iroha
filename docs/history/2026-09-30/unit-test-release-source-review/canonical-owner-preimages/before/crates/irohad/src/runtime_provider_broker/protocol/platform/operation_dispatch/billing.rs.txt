//! Billing operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn billing_query_identity(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let identity = broker_backend!(state, billing_finalized_query)
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    if identity.handle != request.binding.handle {
        return Err(BrokerError::BindingMismatch);
    }
    requalify()?;
    encode_canonical(
        &BillingAdapterIdentityWireV1 {
            handle: identity.handle,
        },
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )
}

pub(super) fn billing_verifier_identity(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let identity = broker_backend!(state, billing_journal_verifier)
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    if identity.handle != request.binding.handle {
        return Err(BrokerError::BindingMismatch);
    }
    requalify()?;
    encode_canonical(
        &BillingAdapterIdentityWireV1 {
            handle: identity.handle,
        },
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )
}

pub(super) fn billing_signer_identity(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let identity = broker_backend!(state, billing_statement_signer)
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    let wire = BillingStatementSignerIdentityWireV1 {
        provider_handle: identity.provider_handle,
        signer_id: identity.signer_id,
        public_key: identity.public_key,
    };
    if wire.provider_handle != request.binding.handle
        || !validate_billing_public_identity_text(
            &wire.signer_id,
            sorafs_node::hedging_billing_service::BILLING_SIGNER_ID_MAX_BYTES_V1,
        )
        || iroha_crypto::ed25519_parse_public_key(&wire.public_key).is_err()
    {
        return Err(BrokerError::BindingMismatch);
    }
    requalify()?;
    encode_canonical(&wire, MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_publisher_identity(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let identity = broker_backend!(state, billing_statement_publisher)
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    let wire = BillingStatementPublisherIdentityWireV1 {
        provider_handle: identity.provider_handle,
        publisher_id: identity.publisher_id,
        route_id: identity.route_id,
        public_key: identity.public_key,
    };
    if wire.provider_handle != request.binding.handle
        || !validate_billing_public_identity_text(
            &wire.publisher_id,
            sorafs_node::hedging_billing_service::BILLING_SIGNER_ID_MAX_BYTES_V1,
        )
        || !validate_billing_public_identity_text(
            &wire.route_id,
            sorafs_node::hedging_billing_service::BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1,
        )
        || iroha_crypto::ed25519_parse_public_key(&wire.public_key).is_err()
    {
        return Err(BrokerError::BindingMismatch);
    }
    requalify()?;
    encode_canonical(&wire, MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_acknowledgement_identity(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let identity = broker_backend!(state, billing_acknowledgement_authority)
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    if identity.provider_handle != request.binding.handle {
        return Err(BrokerError::BindingMismatch);
    }
    requalify()?;
    encode_canonical(
        &BillingAdapterIdentityWireV1 {
            handle: identity.provider_handle,
        },
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )
}

pub(super) fn billing_query_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    broker_backend!(state, billing_finalized_query)
        .check_readiness()
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_verifier_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    broker_backend!(state, billing_journal_verifier)
        .check_readiness()
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_signer_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    broker_backend!(state, billing_statement_signer)
        .check_readiness()
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_publisher_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    broker_backend!(state, billing_statement_publisher)
        .check_readiness()
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_acknowledgement_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    broker_backend!(state, billing_acknowledgement_authority)
        .check_readiness()
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_epoch_store_readiness(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    broker_backend!(state, billing_epoch_witness_store)
        .check_readiness()
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_query_capabilities(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let supplies_period_closes =
        broker_backend!(state, billing_finalized_query).supplies_period_closes();
    if !supplies_period_closes {
        return Err(BrokerError::StaleOrRevoked);
    }
    requalify()?;
    encode_canonical(
        &BillingFinalizedQueryCapabilitiesWireV1 {
            supplies_period_closes,
        },
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )
}

pub(super) fn billing_finalized_head(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let head = broker_backend!(state, billing_finalized_query)
        .finalized_head()
        .map_err(|error| billing_external_error(error, false))?;
    validate_billing_cursor(head)?;
    requalify()?;
    encode_canonical(&head, MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_query_page(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let query = decode_canonical::<BillingQueryPageRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    validate_billing_query_position(query.position, state.network_id)?;
    let page = broker_backend!(state, billing_finalized_query)
        .query_finalized_page(
            billing_query_position_from_wire(query.position),
            query.max_events,
        )
        .map_err(|error| billing_external_error(error, false))?;
    if let Some(page) = page.as_ref() {
        validate_billing_page_shape(page, Some((query.position, query.max_events)))?;
        if page.network_id != state.network_id {
            return Err(BrokerError::BindingMismatch);
        }
    }
    requalify()?;
    encode_canonical(&page, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_query_period_close(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let query = decode_canonical::<BillingQueryPeriodCloseRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    validate_billing_query_position(query.position, state.network_id)?;
    let close = broker_backend!(state, billing_finalized_query)
        .query_finalized_period_close(
            query.period_end_unix,
            billing_query_position_from_wire(query.position),
        )
        .map_err(|error| billing_external_error(error, false))?;
    if let Some(close) = close.as_ref() {
        validate_billing_period_close_shape(close, Some(query.period_end_unix))?;
        if close.network_id != state.network_id {
            return Err(BrokerError::BindingMismatch);
        }
    }
    requalify()?;
    encode_canonical(&close, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_verify_page(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let verify = decode_canonical::<BillingVerifyPageRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    if verify.network_id != state.network_id || verify.page.network_id != verify.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    validate_billing_page_shape(&verify.page, None)?;
    if let Some(previous) = verify.previous {
        validate_billing_journal_commitment(previous, verify.network_id)?;
    }
    broker_backend!(state, billing_journal_verifier)
        .verify_page(&verify.network_id, verify.previous, &verify.page)
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_verify_period_close(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let verify = decode_canonical::<BillingVerifyPeriodCloseRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    if verify.network_id != state.network_id || verify.close.network_id != verify.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    validate_billing_period_close_shape(&verify.close, None)?;
    broker_backend!(state, billing_journal_verifier)
        .verify_period_close(&verify.network_id, &verify.close)
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_verify_epoch_transition(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let verify = decode_canonical::<BillingVerifyEpochTransitionRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    if verify.network_id != state.network_id
        || verify.transition.previous_service_policy.network_id != verify.network_id
        || verify.transition.next_service_policy.network_id != verify.network_id
    {
        return Err(BrokerError::BindingMismatch);
    }
    broker_backend!(state, billing_journal_verifier)
        .verify_epoch_transition(&verify.network_id, &verify.transition)
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_sign_statement_digest(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let sign = decode_canonical::<BillingSignDigestRequestWireV1>(
        &request.payload,
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )?;
    let signer = broker_backend!(state, billing_statement_signer);
    let identity = signer
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    let signature = signer
        .sign_digest(sign.digest)
        .map_err(|error| billing_external_error(error, false))?;
    let identity_after = signer
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    if identity_after != identity || identity.provider_handle != request.binding.handle {
        return Err(BrokerError::StaleOrRevoked);
    }
    verify_evidence_viewer_ed25519_signature(identity.public_key, signature, &sign.digest)?;
    requalify()?;
    encode_canonical(
        &BillingSignDigestResultWireV1 { signature },
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )
}

pub(super) fn billing_publish_statement(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let publish = decode_canonical::<BillingPublishStatementRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    validate_billing_publish_request(&publish, state.network_id)?;
    let publisher = broker_backend!(state, billing_statement_publisher);
    let identity = publisher
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    let identity_wire = BillingStatementPublisherIdentityWireV1 {
        provider_handle: identity.provider_handle,
        publisher_id: identity.publisher_id,
        route_id: identity.route_id,
        public_key: identity.public_key,
    };
    let receipt = publisher
        .publish(
            publish.idempotency_key,
            publish.signed_statement_digest,
            &publish.statement,
        )
        .map_err(|error| billing_external_error(error, true))?;
    let readback = publisher
        .lookup(publish.idempotency_key)
        .map_err(|error| billing_external_error(error, true))?
        .ok_or(BrokerError::Ambiguous)?;
    let readback_wire = BillingAuthoritativePublicationWireV1 {
        signed_statement: readback.signed_statement,
        receipt: readback.receipt,
    };
    if readback_wire.signed_statement != publish.statement || readback_wire.receipt != receipt {
        return Err(BrokerError::Ambiguous);
    }
    validate_billing_publication_shape(
        &readback_wire,
        publish.idempotency_key,
        &identity_wire,
        state.network_id,
    )
    .map_err(|_| BrokerError::Ambiguous)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&receipt, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_lookup_publication(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let lookup = decode_canonical::<BillingLookupRequestWireV1>(
        &request.payload,
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )?;
    let publisher = broker_backend!(state, billing_statement_publisher);
    let identity = publisher
        .identity()
        .map_err(|error| billing_external_error(error, false))?;
    let identity_wire = BillingStatementPublisherIdentityWireV1 {
        provider_handle: identity.provider_handle,
        publisher_id: identity.publisher_id,
        route_id: identity.route_id,
        public_key: identity.public_key,
    };
    let publication = publisher
        .lookup(lookup.record_id)
        .map_err(|error| billing_external_error(error, false))?
        .map(|publication| BillingAuthoritativePublicationWireV1 {
            signed_statement: publication.signed_statement,
            receipt: publication.receipt,
        });
    if let Some(publication) = publication.as_ref() {
        validate_billing_publication_shape(
            publication,
            lookup.record_id,
            &identity_wire,
            state.network_id,
        )?;
    }
    requalify()?;
    encode_canonical(&publication, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_verify_acknowledgement(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let acknowledgement = decode_canonical::<BillingAcknowledgementRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    validate_billing_acknowledgement_request(&acknowledgement, state.network_id)?;
    broker_backend!(state, billing_acknowledgement_authority)
        .verify(&acknowledgement.statement, &acknowledgement.acknowledgement)
        .map_err(|error| billing_external_error(error, false))?;
    requalify()?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

pub(super) fn billing_record_acknowledgement(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let acknowledgement = decode_canonical::<BillingAcknowledgementRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    validate_billing_acknowledgement_request(&acknowledgement, state.network_id)?;
    let authority = broker_backend!(state, billing_acknowledgement_authority);
    let recorded = authority
        .record(&acknowledgement.statement, &acknowledgement.acknowledgement)
        .map_err(|error| billing_external_error(error, true))?;
    if recorded != acknowledgement.acknowledgement {
        return Err(BrokerError::Ambiguous);
    }
    let readback = authority
        .lookup(recorded.statement_id)
        .map_err(|error| billing_external_error(error, true))?;
    if readback.as_ref() != Some(&recorded) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&recorded, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_lookup_acknowledgement(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let lookup = decode_canonical::<BillingLookupRequestWireV1>(
        &request.payload,
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )?;
    let acknowledgement = broker_backend!(state, billing_acknowledgement_authority)
        .lookup(lookup.record_id)
        .map_err(|error| billing_external_error(error, false))?;
    if let Some(acknowledgement) = acknowledgement.as_ref()
        && (acknowledgement.statement_id.ne(&lookup.record_id)
            || acknowledgement.network_id != state.network_id)
    {
        return Err(BrokerError::Rejected);
    }
    requalify()?;
    encode_canonical(&acknowledgement, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_load_latest_epoch(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let record = broker_backend!(state, billing_epoch_witness_store)
        .load_latest()
        .map_err(|error| billing_external_error(error, false))?;
    if let Some(record) = record.as_ref() {
        if record.network_id != state.network_id {
            return Err(BrokerError::BindingMismatch);
        }
        record
            .validate(sorafs_node::hedging_billing_service::HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1)
            .map_err(|_| BrokerError::Rejected)?;
    }
    requalify()?;
    encode_canonical(&record, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_load_epoch(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let load = decode_canonical::<BillingLoadEpochRequestWireV1>(
        &request.payload,
        MAX_BILLING_CONTROL_FRAME_BYTES_V1,
    )?;
    let record = broker_backend!(state, billing_epoch_witness_store)
        .load_epoch(load.epoch_sequence)
        .map_err(|error| billing_external_error(error, false))?;
    if let Some(record) = record.as_ref() {
        if record.network_id != state.network_id {
            return Err(BrokerError::BindingMismatch);
        }
        record
            .validate(sorafs_node::hedging_billing_service::HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1)
            .map_err(|_| BrokerError::Rejected)?;
        if record.epoch_sequence != load.epoch_sequence {
            return Err(BrokerError::Rejected);
        }
    }
    requalify()?;
    encode_canonical(&record, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn billing_compare_and_swap_epoch(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<BillingCompareAndSwapEpochRequestWireV1>(
        &request.payload,
        MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
    )?;
    compare
        .next
        .validate(sorafs_node::hedging_billing_service::HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1)
        .map_err(|_| BrokerError::Rejected)?;
    if compare.next.network_id != state.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    let store = broker_backend!(state, billing_epoch_witness_store);
    let current = store
        .load_latest()
        .map_err(|error| billing_external_error(error, false))?;
    if current
        .as_ref()
        .is_some_and(|record| record.network_id != state.network_id)
    {
        return Err(BrokerError::BindingMismatch);
    }
    let monotonic = match current.as_ref() {
        None => compare.expected_revision.is_none() && compare.next.epoch_sequence == 1,
        Some(current) => {
            current.revision == compare.expected_revision.unwrap_or([0; 32])
                && current
                    .epoch_sequence
                    .checked_add(1)
                    .is_some_and(|next| next == compare.next.epoch_sequence)
        }
    };
    if current.as_ref().map(|record| record.revision) != compare.expected_revision {
        return Err(BrokerError::Conflict);
    }
    if !monotonic {
        return Err(BrokerError::Rejected);
    }
    store
        .compare_and_swap_latest(compare.expected_revision, &compare.next)
        .map_err(|error| billing_external_error(error, true))?;
    let latest = store
        .load_latest()
        .map_err(|error| billing_external_error(error, true))?;
    let historical = store
        .load_epoch(compare.next.epoch_sequence)
        .map_err(|error| billing_external_error(error, true))?;
    if latest.as_ref() != Some(&compare.next) || historical.as_ref() != Some(&compare.next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
}

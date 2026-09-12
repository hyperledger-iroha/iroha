//! Pop operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn pop_runtime_open(
    state: &BrokerServerStateV1,
    pop_session: &mut PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    if pop_session.providers.is_some() {
        return Err(BrokerError::Rejected);
    }
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let requested = decode_canonical::<PopCredentialRuntimeBindingWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    if &requested != exact {
        return Err(BrokerError::BindingMismatch);
    }
    let bindings = pop_runtime_bindings_from_wire(&request.binding)?;
    let registry = broker_backend!(state, pop_credential_provider_registry);
    let providers_result = registry.resolve(&bindings);
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    let providers = providers_result.map_err(|error| {
        match error {
    iroha_torii::sorafs::pop_api::
        PopCredentialRuntimeProviderRegistryErrorV1::Unavailable => {
        BrokerError::Unavailable
    }
    iroha_torii::sorafs::pop_api::
        PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked => {
        BrokerError::StaleOrRevoked
    }
    iroha_torii::sorafs::pop_api::
        PopCredentialRuntimeProviderRegistryErrorV1::RejectedBindings => {
        BrokerError::Rejected
    }
}
    })?;
    if providers.issuer_signer.key_id() != exact.issuer_signer_handle
        || providers.issuer_signer.public_key() != exact.issuer_public_key
        || providers.enrollment_recipient.key_id() != exact.enrollment_recipient_key_id
        || providers.enrollment_recipient.public_key_digest()
            != exact.enrollment_recipient_public_key_digest
        || providers.wallet_recipient.key_id() != exact.wallet_recipient_key_id
        || providers.wallet_recipient.public_key_digest()
            != exact.wallet_recipient_public_key_digest
        || providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id
        || !iroha_config::parameters::is_production_runtime_handle(providers.issuer_signer.key_id())
        || !iroha_config::parameters::is_production_runtime_handle(
            providers.enrollment_recipient.key_id(),
        )
        || !iroha_config::parameters::is_production_runtime_handle(
            providers.wallet_recipient.key_id(),
        )
        || !iroha_config::parameters::is_production_runtime_handle(
            providers.wallet_key_wrapper.active_key_id(),
        )
    {
        return Err(BrokerError::Ambiguous);
    }
    let outcome = PopRuntimeOpenResultWireV1 {
        issuer_signer_handle: providers.issuer_signer.key_id().to_owned(),
        issuer_public_key: providers.issuer_signer.public_key(),
        enrollment_recipient_key_id: providers.enrollment_recipient.key_id().to_owned(),
        enrollment_recipient_public_key_digest: providers.enrollment_recipient.public_key_digest(),
        wallet_recipient_key_id: providers.wallet_recipient.key_id().to_owned(),
        wallet_recipient_public_key_digest: providers.wallet_recipient.public_key_digest(),
        wallet_wrapping_key_id: providers.wallet_key_wrapper.active_key_id().to_owned(),
    };
    validate_pop_open_result(&outcome, exact).map_err(|_| BrokerError::Ambiguous)?;
    let encoded = encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)?;
    pop_session.providers = Some(providers);
    Ok(encoded)
}

pub(super) fn pop_recipient_open(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
    operation: u16,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopRecipientOpenRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    validate_pop_recipient_open_request(&wire, operation)?;
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let opened = if operation == OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1 {
        if providers.enrollment_recipient.key_id() != exact.enrollment_recipient_key_id
            || providers.enrollment_recipient.public_key_digest()
                != exact.enrollment_recipient_public_key_digest
        {
            return Err(BrokerError::StaleOrRevoked);
        }
        providers
            .enrollment_recipient
            .open_enrollment(&wire.encrypted_payload, &wire.aad)
    } else {
        if providers.wallet_recipient.key_id() != exact.wallet_recipient_key_id
            || providers.wallet_recipient.public_key_digest()
                != exact.wallet_recipient_public_key_digest
        {
            return Err(BrokerError::StaleOrRevoked);
        }
        providers
            .wallet_recipient
            .open_wallet_delivery(&wire.encrypted_payload, &wire.aad)
    };
    requalify()?;
    if operation == OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1 {
        if providers.enrollment_recipient.key_id() != exact.enrollment_recipient_key_id
            || providers.enrollment_recipient.public_key_digest()
                != exact.enrollment_recipient_public_key_digest
        {
            return Err(BrokerError::StaleOrRevoked);
        }
    } else if providers.wallet_recipient.key_id() != exact.wallet_recipient_key_id
        || providers.wallet_recipient.public_key_digest()
            != exact.wallet_recipient_public_key_digest
    {
        return Err(BrokerError::StaleOrRevoked);
    }
    let plaintext = opened.map_err(|error| match error {
        sorafs_node::pop_credentials::PopRecipientOpenErrorV1::Unavailable => {
            BrokerError::Unavailable
        }
        sorafs_node::pop_credentials::PopRecipientOpenErrorV1::Rejected => BrokerError::Rejected,
    })?;
    let outcome = PopRecipientOpenResultWireV1 { plaintext };
    validate_pop_recipient_open_result(&outcome, operation)?;
    encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn pop_issuer_sign(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopIssuerSignRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    if providers.issuer_signer.key_id() != exact.issuer_signer_handle
        || providers.issuer_signer.public_key() != exact.issuer_public_key
    {
        return Err(BrokerError::StaleOrRevoked);
    }
    let purpose =
        sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::try_from_wire_id(wire.purpose)
            .ok_or(BrokerError::Rejected)?;
    if wire.digest == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    let signature_result = providers.issuer_signer.sign_digest(purpose, wire.digest);
    requalify()?;
    if providers.issuer_signer.key_id() != exact.issuer_signer_handle
        || providers.issuer_signer.public_key() != exact.issuer_public_key
    {
        return Err(BrokerError::StaleOrRevoked);
    }
    let signature = signature_result.map_err(|_| BrokerError::Rejected)?;
    verify_evidence_viewer_ed25519_signature(exact.issuer_public_key, signature, &wire.digest)?;
    encode_canonical(
        &PopIssuerSignResultWireV1 { signature },
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )
}

pub(super) fn pop_authenticate(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopAuthenticateRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    let action = pop_action_from_wire(wire.action)?;
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let principal_result = providers.authenticator.authenticate(
        &wire.opaque_credential,
        action,
        wire.request_binding,
        wire.now_epoch,
    );
    requalify()?;
    let principal = principal_result.map_err(|_| BrokerError::Rejected)?;
    let outcome = PopAuthenticatedPrincipalWireV1 {
        principal_digest: principal.principal_digest,
        expires_at_epoch: principal.expires_at_epoch,
        caller_signed_transaction: matches!(
            principal.request_authority,
            sorafs_node::pop_credentials::PopRequestAuthorityV1::CallerSignedTransaction
        ),
    };
    validate_pop_principal(outcome, &wire)?;
    encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn pop_registry_submit(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopRegistrySubmitRequestWireV1>(
        &request.payload,
        MAX_POP_REGISTRY_OPERATION_BYTES_V1,
    )?;
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let submit_result = providers
        .registry_submitter
        .submit(wire.idempotency_key, &wire.operation);
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    submit_result.map_err(|_| BrokerError::Rejected)?;
    encode_canonical(&(), MAX_POP_RUNTIME_FRAME_BYTES_V1).map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn pop_registry_next(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopRegistryNextRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let projection_result = providers.registry_reader.next_after(wire.cursor);
    requalify()?;
    let projection = projection_result.map_err(|_| BrokerError::Unavailable)?;
    if let Some(projection) = projection.as_ref() {
        validate_pop_projection(projection, exact)?;
    }
    encode_canonical(
        &PopRegistryNextResultWireV1 { projection },
        MAX_POP_PROJECTION_BYTES_V1,
    )
}

pub(super) fn pop_issuance_draft(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopIssuanceDraftRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let draft_result = providers
        .issuance_draft_provider
        .resolve(wire.request_id, wire.now_epoch);
    requalify()?;
    let draft = draft_result.map_err(|_| BrokerError::Unavailable)?;
    let outcome = PopIssuanceDraftResultWireV1 {
        request_id: draft.request_id,
        credential: draft.credential.clone(),
        commitment_root: draft.commitment_root.clone(),
        revocation_list: draft.revocation_list.clone(),
        witness: PopMembershipWitnessWireV1::from_witness(&draft.witness),
    };
    validate_pop_draft(&outcome, wire, exact)?;
    encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn pop_wallet_wrap_dek(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopWalletWrapDekRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
        return Err(BrokerError::StaleOrRevoked);
    }
    let wrapped_result = providers
        .wallet_key_wrapper
        .wrap_dek(wire.context, &wire.dek);
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
        return Err(BrokerError::Ambiguous);
    }
    let wrapped_dek = wrapped_result.map_err(|_| BrokerError::Rejected)?;
    if wrapped_dek.is_empty() || wrapped_dek.len() > MAX_POP_WRAPPED_DEK_BYTES_V1 {
        return Err(BrokerError::Ambiguous);
    }
    encode_canonical(
        &PopWalletWrapDekResultWireV1 { wrapped_dek },
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )
    .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn pop_wallet_unwrap_dek(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopWalletUnwrapDekRequestWireV1>(
        &request.payload,
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )?;
    let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
        return Err(BrokerError::StaleOrRevoked);
    }
    let dek_result =
        providers
            .wallet_key_wrapper
            .unwrap_dek(&wire.key_id, wire.context, &wire.wrapped_dek);
    requalify()?;
    if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
        return Err(BrokerError::StaleOrRevoked);
    }
    let dek = dek_result.map_err(|_| BrokerError::Rejected)?;
    if dek == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    encode_canonical(
        &PopWalletUnwrapDekResultWireV1 { dek },
        MAX_POP_RUNTIME_FRAME_BYTES_V1,
    )
}

pub(super) fn pop_wallet_witness(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<PopWalletWitnessRequestWireV1>(
        &request.payload,
        MAX_POP_PROJECTION_BYTES_V1,
    )?;
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let witness_result = providers
        .wallet_witness_provider
        .resolve(wire.credential_commitment, &wire.projection);
    requalify()?;
    let witness = witness_result.map_err(|_| BrokerError::Unavailable)?;
    let outcome = PopMembershipWitnessWireV1::from_witness(&witness);
    validate_pop_witness_wire(&outcome)?;
    encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
}

pub(super) fn pop_finalized_time(
    state: &BrokerServerStateV1,
    pop_session: &PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    decode_canonical::<()>(&request.payload, MAX_POP_RUNTIME_FRAME_BYTES_V1)?;
    let providers = pop_session
        .providers
        .as_ref()
        .ok_or(BrokerError::Rejected)?;
    let sample_result = providers.finalized_time_provider.sample();
    requalify()?;
    let sample = sample_result.map_err(|_| BrokerError::Unavailable)?;
    let outcome = PopFinalizedTimeResultWireV1 {
        finalized_block_height: sample.finalized_block_height,
        finalized_block_hash: sample.finalized_block_hash,
        finalized_epoch: sample.finalized_epoch,
        observed_epoch: sample.observed_epoch,
    };
    validate_pop_finalized_time(outcome)?;
    encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
}

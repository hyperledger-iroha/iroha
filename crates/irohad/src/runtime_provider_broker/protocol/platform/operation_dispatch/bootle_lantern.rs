//! Bootle lantern operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn qualify_bootle_lantern(state: &BrokerServerStateV1) -> Result<Vec<u8>, BrokerError> {
    let qualification = broker_backend!(state, bootle_lantern_issuance)
        .qualification()
        .map_err(|error| {
            match error {
        iroha_torii::privacy_issuance_api::
            BootleLanternIssuanceRuntimeProviderRegistryErrorV1::Unavailable =>
        {
            BrokerError::Unavailable
        }
        iroha_torii::privacy_issuance_api::
            BootleLanternIssuanceRuntimeProviderRegistryErrorV1::StaleOrRevoked =>
        {
            BrokerError::StaleOrRevoked
        }
        iroha_torii::privacy_issuance_api::
            BootleLanternIssuanceRuntimeProviderRegistryErrorV1::RejectedBindings =>
        {
            BrokerError::BindingMismatch
        }
    }
        })?;
    encode_canonical(
        &QualificationResultWireV1 {
            revision: qualification.revision,
            policy_digest: qualification.policy_digest,
        },
        MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
    )
}

pub(super) fn bootle_lantern_issuance_authenticate(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let authenticate = decode_canonical::<BootleLanternAuthenticateRequestWireV1>(
        &request.payload,
        MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
    )?;
    let action = bootle_lantern_action_from_wire(authenticate.action)?;
    let outcome = broker_backend!(state, bootle_lantern_issuance).authenticate(
        &authenticate.opaque_credential,
        action,
        authenticate.request_binding,
        authenticate.committed_height,
    );
    requalify()?;
    let principal = outcome.map_err(|error| {
        match error {
    iroha_torii::privacy_issuance_api::
        BootleLanternIssuanceAuthenticationErrorV1::Denied =>
    {
        BrokerError::Rejected
    }
    iroha_torii::privacy_issuance_api::
        BootleLanternIssuanceAuthenticationErrorV1::Unavailable =>
    {
        BrokerError::Unavailable
    }
}
    })?;
    encode_canonical(
        &BootleLanternAuthenticatedPrincipalWireV1 {
            principal_digest: principal.principal_digest,
            issued_at_height: principal.issued_at_height,
            expires_at_height: principal.expires_at_height,
        },
        MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
    )
}

pub(super) fn bootle_lantern_issuance_prepare_authorization(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let prepare = decode_canonical::<BootleLanternPrepareAuthorizationRequestWireV1>(
        &request.payload,
        MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
    )?;
    let outcome = broker_backend!(state, bootle_lantern_issuance).prepare_authorization(
        &prepare.context,
        prepare.canonical_genesis_hash,
        &prepare.policy,
        prepare.requester_authorization_digest,
        prepare.issued_at_height,
        prepare.expires_at_height,
    );
    requalify()?;
    let authorization = outcome.map_err(|error| {
        match error {
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest =>
    {
        BrokerError::Rejected
    }
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch =>
    {
        BrokerError::StaleOrRevoked
    }
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::Unavailable =>
    {
        BrokerError::Unavailable
    }
}
    })?;
    iroha_core::privacy_engines::bootle_lantern::issuer::
    issuer_validate_prepared_blind_issuance_authorization_v1(
        &prepare.context,
        prepare.canonical_genesis_hash,
        &prepare.policy,
        &authorization,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let authorization = authorization.encode().map_err(|_| BrokerError::Rejected)?;
    if authorization.len() != BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 {
        return Err(BrokerError::Rejected);
    }
    encode_canonical(
        &BootleLanternAuthorizationWireV1 { authorization },
        MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
    )
}

pub(super) fn bootle_lantern_issuance_validate_request(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let (issue, authorization) =
        decode_bootle_lantern_issue_request(&request.payload, &request.binding, &state.network_id)?;
    let expected = iroha_core::privacy_engines::bootle_lantern::issuer::
    issuer_validate_blind_issuance_request_encoded_v1(
        &issue.context,
        issue.canonical_genesis_hash,
        &issue.policy,
        &authorization,
        &issue.request,
        issue.current_height,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let outcome = broker_backend!(state, bootle_lantern_issuance).validate_request(
        &issue.context,
        issue.canonical_genesis_hash,
        &issue.policy,
        &authorization,
        &issue.request,
        issue.current_height,
    );
    requalify()?;
    let digest = outcome.map_err(|error| {
        match error {
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest =>
    {
        BrokerError::Rejected
    }
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch =>
    {
        BrokerError::StaleOrRevoked
    }
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::Unavailable =>
    {
        BrokerError::Unavailable
    }
}
    })?;
    if digest == [0; 32] || digest != expected {
        return Err(BrokerError::Rejected);
    }
    encode_canonical(&digest, MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1)
}

pub(super) fn bootle_lantern_issuance_issue_validated(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let (issue, authorization) =
        decode_bootle_lantern_issue_request(&request.payload, &request.binding, &state.network_id)?;
    let outcome = broker_backend!(state, bootle_lantern_issuance).issue_validated(
        &issue.context,
        issue.canonical_genesis_hash,
        &issue.policy,
        &authorization,
        &issue.request,
        issue.current_height,
    );
    requalify()?;
    let response = outcome.map_err(|error| {
        match error {
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest =>
    {
        BrokerError::Rejected
    }
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch =>
    {
        BrokerError::StaleOrRevoked
    }
    crate::runtime_provider_broker::
        BootleLanternIssuanceBrokerBackendErrorV1::Unavailable =>
    {
        BrokerError::Unavailable
    }
}
    })?;
    let response = response.encode().map_err(|_| BrokerError::Rejected)?;
    if response.len() != BOOTLE_LANTERN_RESPONSE_BYTES_V1 {
        return Err(BrokerError::Rejected);
    }
    iroha_core::privacy_engines::bootle_lantern::issuer::
    issuer_validate_cached_blind_issuance_response_encoded_v1(
        &issue.context,
        issue.canonical_genesis_hash,
        &issue.policy,
        &authorization,
        &issue.request,
        &response,
    )
    .map_err(|_| BrokerError::Rejected)?;
    encode_canonical(
        &BootleLanternIssuanceResponseWireV1 { response },
        MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
    )
}

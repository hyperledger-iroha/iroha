const fn governance_request_auth_scope_to_wire(
    scope: sorafs_node::GovernanceDagAuthenticationScope,
) -> u8 {
    match scope {
        sorafs_node::GovernanceDagAuthenticationScope::Ipfs => 1,
        sorafs_node::GovernanceDagAuthenticationScope::SignedHead => 2,
    }
}
fn governance_request_auth_scope_from_wire(
    scope: u8,
) -> Result<sorafs_node::GovernanceDagAuthenticationScope, BrokerError> {
    match scope {
        1 => Ok(sorafs_node::GovernanceDagAuthenticationScope::Ipfs),
        2 => Ok(sorafs_node::GovernanceDagAuthenticationScope::SignedHead),
        _ => Err(BrokerError::Rejected),
    }
}
fn governance_request_ingress_binding_to_wire(
    binding: sorafs_node::GovernanceDagRequestIngressBindingV1,
) -> GovernanceRequestIngressBindingWireV1 {
    GovernanceRequestIngressBindingWireV1 {
        scope: governance_request_auth_scope_to_wire(binding.scope()),
        endpoint_binding: binding.endpoint_binding(),
        public_key: binding.public_key(),
        max_body_bytes: binding.max_body_bytes(),
        max_envelope_lifetime_secs: binding.max_envelope_lifetime_secs(),
        max_future_skew_secs: binding.max_future_skew_secs(),
    }
}
fn governance_request_ingress_binding_from_wire(
    binding: GovernanceRequestIngressBindingWireV1,
) -> Result<sorafs_node::GovernanceDagRequestIngressBindingV1, BrokerError> {
    sorafs_node::GovernanceDagRequestIngressBindingV1::try_new(
        governance_request_auth_scope_from_wire(binding.scope)
            .map_err(|_| BrokerError::BindingMismatch)?,
        binding.endpoint_binding,
        binding.public_key,
        binding.max_body_bytes,
        binding.max_envelope_lifetime_secs,
        binding.max_future_skew_secs,
    )
    .map_err(|_| BrokerError::BindingMismatch)
}
fn governance_request_ingress_binding_from_provider_binding(
    binding: &ProviderBindingWireV1,
) -> Result<sorafs_node::GovernanceDagRequestIngressBindingV1, BrokerError> {
    let ingress = governance_request_ingress_binding_from_wire(
        binding
            .governance_request_ingress_binding
            .ok_or(BrokerError::BindingMismatch)?,
    )?;
    let expected_scope = if binding.slot
        == IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator.wire_id()
    {
        sorafs_node::GovernanceDagAuthenticationScope::Ipfs
    } else if binding.slot == IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator.wire_id() {
        sorafs_node::GovernanceDagAuthenticationScope::SignedHead
    } else {
        return Err(BrokerError::BindingMismatch);
    };
    if ingress.scope() != expected_scope {
        return Err(BrokerError::BindingMismatch);
    }
    Ok(ingress)
}
fn governance_request_ingress_qualification_to_wire(
    qualification: sorafs_node::GovernanceDagRequestIngressQualificationV1,
) -> GovernanceRequestIngressQualificationWireV1 {
    let provider = qualification.provider();
    GovernanceRequestIngressQualificationWireV1 {
        provider: QualificationResultWireV1 {
            revision: provider.revision,
            policy_digest: provider.policy_digest,
        },
        binding: governance_request_ingress_binding_to_wire(qualification.binding()),
        receiver_policy_digest: qualification.receiver_policy_digest(),
        replay_namespace_digest: qualification.replay_namespace_digest(),
        replica_set_digest: qualification.replica_set_digest(),
    }
}
fn governance_request_ingress_qualification_from_wire(
    qualification: GovernanceRequestIngressQualificationWireV1,
) -> Result<sorafs_node::GovernanceDagRequestIngressQualificationV1, BrokerError> {
    let binding = governance_request_ingress_binding_from_wire(qualification.binding)
        .map_err(|_| BrokerError::Protocol)?;
    sorafs_node::GovernanceDagRequestIngressQualificationV1::try_new(
        sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
            qualification.provider.revision,
            qualification.provider.policy_digest,
        ),
        binding,
        qualification.receiver_policy_digest,
        qualification.replay_namespace_digest,
        qualification.replica_set_digest,
    )
    .map_err(|_| BrokerError::Protocol)
}
fn governance_request_auth_to_wire(
    request: &sorafs_node::GovernanceDagCanonicalRequestV1,
) -> GovernanceRequestAuthRequestWireV1 {
    GovernanceRequestAuthRequestWireV1 {
        scope: governance_request_auth_scope_to_wire(request.scope()),
        method: request.method().to_owned(),
        canonical_url: request.canonical_url().to_owned(),
        selected_headers: request
            .selected_headers()
            .iter()
            .map(|header| GovernanceRequestAuthHeaderWireV1 {
                name: header.name().to_owned(),
                value: header.value().to_owned(),
            })
            .collect(),
        body_length: request.body_length(),
        body_blake3: request.body_blake3(),
        request_digest: request.request_digest(),
    }
}
fn governance_request_auth_from_wire(
    wire: &GovernanceRequestAuthRequestWireV1,
    max_body_bytes: u64,
) -> Result<sorafs_node::GovernanceDagCanonicalRequestV1, BrokerError> {
    if wire.selected_headers.len() > sorafs_node::GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADERS_V1
        || wire.canonical_url.len() > sorafs_node::GOVERNANCE_DAG_REQUEST_AUTH_MAX_URL_BYTES_V1
        || wire.request_digest == [0; 32]
    {
        return Err(BrokerError::Rejected);
    }
    let selected_headers = wire
        .selected_headers
        .iter()
        .map(|header| {
            sorafs_node::GovernanceDagCanonicalRequestHeaderV1::try_new(&header.name, &header.value)
                .map_err(|_| BrokerError::Rejected)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let request = sorafs_node::GovernanceDagCanonicalRequestV1::try_new(
        governance_request_auth_scope_from_wire(wire.scope)?,
        &wire.method,
        &wire.canonical_url,
        selected_headers,
        wire.body_length,
        wire.body_blake3,
        max_body_bytes,
    )
    .map_err(|_| BrokerError::Rejected)?;
    if request.request_digest() != wire.request_digest {
        return Err(BrokerError::Rejected);
    }
    Ok(request)
}
fn validate_governance_request_auth_envelope(
    request: &sorafs_node::GovernanceDagCanonicalRequestV1,
    result: GovernanceRequestAuthResultWireV1,
    expected_public_key: [u8; 32],
) -> Result<sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1, BrokerError> {
    if result.scope != governance_request_auth_scope_to_wire(request.scope())
        || result.request_digest != request.request_digest()
        || result.public_key != expected_public_key
    {
        return Err(BrokerError::BindingMismatch);
    }
    let envelope = sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
        request,
        result.issued_at_unix_secs,
        result.expires_at_unix_secs,
        result.nonce,
        result.public_key,
        result.signature,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let payload = sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
        request,
        envelope.issued_at_unix_secs(),
        envelope.expires_at_unix_secs(),
        envelope.nonce(),
        envelope.public_key(),
    );
    let public_key = iroha_crypto::PublicKey::from_bytes(
        iroha_crypto::Algorithm::Ed25519,
        &envelope.public_key(),
    )
    .map_err(|_| BrokerError::BindingMismatch)?;
    iroha_crypto::ed25519_parse_signature(&envelope.signature())
        .map_err(|_| BrokerError::Rejected)?
        .verify(&public_key, &payload)
        .map_err(|_| BrokerError::Rejected)?;
    Ok(envelope)
}
fn governance_request_auth_result_to_wire(
    envelope: &sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1,
) -> GovernanceRequestAuthResultWireV1 {
    GovernanceRequestAuthResultWireV1 {
        scope: governance_request_auth_scope_to_wire(envelope.scope()),
        issued_at_unix_secs: envelope.issued_at_unix_secs(),
        expires_at_unix_secs: envelope.expires_at_unix_secs(),
        nonce: envelope.nonce(),
        request_digest: envelope.request_digest(),
        public_key: envelope.public_key(),
        signature: envelope.signature(),
    }
}

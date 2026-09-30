//! Gateway operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn gateway_acme_order_certificate(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<GatewayAcmeOrderRequestWireV1>(
        &request.payload,
        MAX_GATEWAY_ACME_FRAME_BYTES_V1,
    )?;
    validate_gateway_acme_order(&wire)?;
    let order = iroha_torii::sorafs::gateway::CertificateOrder {
        hostnames: wire.hostnames,
        account_email: wire.account_email,
        directory_url: wire.directory_url,
        dns_provider_id: wire.dns_provider_id,
        challenge: iroha_torii::sorafs::gateway::ChallengeProfile {
            dns01: wire.dns01,
            tls_alpn_01: wire.tls_alpn_01,
        },
    };
    let outcome = match broker_backend!(state, gateway_acme_client).order_certificate(&order) {
        Ok(bundle) => GatewayAcmeOrderOutcomeWireV1 {
            outcome: 0,
            certificate_pem: bundle.certificate_pem.clone(),
            private_key_pem: bundle.private_key_pem.clone(),
            ech_config: bundle.ech_config.clone(),
            not_after: Some(
                SystemTimeWireV1::from_system_time(bundle.not_after)
                    .map_err(|_| BrokerError::Ambiguous)?,
            ),
            retry_after: None,
        },
        Err(iroha_torii::sorafs::gateway::AcmeClientError::Rejected) => {
            GatewayAcmeOrderOutcomeWireV1 {
                outcome: 1,
                certificate_pem: String::new(),
                private_key_pem: String::new(),
                ech_config: None,
                not_after: None,
                retry_after: None,
            }
        }
        Err(iroha_torii::sorafs::gateway::AcmeClientError::Temporary { retry_after }) => {
            GatewayAcmeOrderOutcomeWireV1 {
                outcome: 2,
                certificate_pem: String::new(),
                private_key_pem: String::new(),
                ech_config: None,
                not_after: None,
                retry_after: retry_after.map(DurationWireV1::from_duration),
            }
        }
        Err(iroha_torii::sorafs::gateway::AcmeClientError::Transport) => {
            GatewayAcmeOrderOutcomeWireV1 {
                outcome: 3,
                certificate_pem: String::new(),
                private_key_pem: String::new(),
                ech_config: None,
                not_after: None,
                retry_after: None,
            }
        }
    };
    validate_gateway_acme_outcome(&outcome).map_err(|_| BrokerError::Ambiguous)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&outcome, MAX_GATEWAY_ACME_FRAME_BYTES_V1).map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn gateway_compliance_resolve(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire =
        decode_canonical::<GatewayComplianceResolveRequestWireV1>(&request.payload, 128 * 1024)?;
    let timeout = validate_gateway_compliance_resolve_request(&wire)?;
    let outcome = match broker_backend!(state, gateway_compliance_feed_transport)
        .resolve(&wire.hostname, timeout)
    {
        Ok(addresses) => {
            let addresses = addresses
                .into_iter()
                .map(IpAddressWireV1::from)
                .collect::<Vec<_>>();
            let outcome = GatewayComplianceResolveOutcomeWireV1 {
                outcome: 0,
                addresses,
                found: 0,
                maximum: 0,
            };
            validate_gateway_compliance_resolve_outcome(&outcome)?;
            outcome
        }
        Err(error) => {
            let (outcome, found, maximum) = gateway_compliance_error_wire(&error);
            GatewayComplianceResolveOutcomeWireV1 {
                outcome,
                addresses: Vec::new(),
                found,
                maximum,
            }
        }
    };
    requalify()?;
    encode_canonical(&outcome, 128 * 1024)
}

pub(super) fn gateway_compliance_fetch(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<GatewayComplianceFetchRequestWireV1>(
        &request.payload,
        MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1,
    )?;
    let (url, pinned_addresses, connect_timeout, total_timeout, max_encoded_bytes) =
        validate_gateway_compliance_fetch_request(&wire)?;
    let fetch = iroha_torii::sorafs::gateway::GatewayComplianceFetchRequest {
        url,
        pinned_addresses,
        connect_timeout,
        total_timeout,
        max_encoded_bytes,
    };
    let outcome = match broker_backend!(state, gateway_compliance_feed_transport).fetch(&fetch) {
        Ok(response) => GatewayComplianceFetchOutcomeWireV1 {
            outcome: 0,
            status: response.status,
            redirect_location: response.redirect_location,
            connected_address: Some(IpAddressWireV1::from(response.connected_address)),
            peer_spki_sha256: response.peer_spki_sha256,
            content_encoding: match response.content_encoding {
                iroha_torii::sorafs::gateway::GatewayComplianceContentEncoding::Identity => 0,
                iroha_torii::sorafs::gateway::GatewayComplianceContentEncoding::Gzip => 1,
                iroha_torii::sorafs::gateway::GatewayComplianceContentEncoding::Zstd => 2,
            },
            body: response.body,
            elapsed: Some(DurationWireV1::from_duration(response.elapsed)),
            found: 0,
            maximum: 0,
        },
        Err(error) => {
            let (outcome, found, maximum) = gateway_compliance_error_wire(&error);
            GatewayComplianceFetchOutcomeWireV1 {
                outcome,
                status: 0,
                redirect_location: None,
                connected_address: None,
                peer_spki_sha256: [0; 32],
                content_encoding: 0,
                body: Vec::new(),
                elapsed: None,
                found,
                maximum,
            }
        }
    };
    validate_gateway_compliance_fetch_outcome(&outcome, &wire)?;
    requalify()?;
    encode_canonical(&outcome, MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1)
}

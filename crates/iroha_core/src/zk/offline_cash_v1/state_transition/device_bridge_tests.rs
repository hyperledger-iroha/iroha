use super::*;

const TEST_ATTESTATION_EVIDENCE: &[u8] = b"authenticated test-only platform evidence";
const TEST_AUTHENTICATOR_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:test-device-lifecycle-authenticator";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponseMutation {
    None,
    WrongOperation,
    WrongRequestId,
    RelabelledPayload,
    ResponseContentAfterAuthentication,
    RejectedAuthenticator,
    RemoteConflict,
}

struct TestTransport {
    capability_frame: Vec<u8>,
    expected_evidence: Vec<u8>,
    authenticate_capability: bool,
    mutation: ResponseMutation,
}

impl transport_sealed::Sealed for TestTransport {}

impl AuthenticatedDeviceLifecycleTransportV1 for TestTransport {
    fn capabilities_frame(&self) -> Result<Zeroizing<Vec<u8>>, DeviceLifecycleBridgeErrorV1> {
        Ok(Zeroizing::new(self.capability_frame.clone()))
    }

    fn authenticate_capabilities(
        &self,
        frame: &[u8; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1],
        attestation_evidence: &[u8],
    ) -> Result<(), DeviceLifecycleBridgeErrorV1> {
        if self.authenticate_capability
            && frame.as_slice() == self.capability_frame
            && attestation_evidence == self.expected_evidence
        {
            Ok(())
        } else {
            Err(DeviceLifecycleBridgeErrorV1::CapabilityAuthentication)
        }
    }

    fn execute(&self, command: &[u8]) -> Result<Zeroizing<Vec<u8>>, DeviceLifecycleBridgeErrorV1> {
        let (operation, request_id) = inspect_command(command)?;
        let capabilities: &[u8; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1] = self
            .capability_frame
            .as_slice()
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::CapabilityMismatch)?;
        let capabilities = decode_capabilities(capabilities)?;
        let response_operation = if self.mutation == ResponseMutation::WrongOperation {
            DeviceLifecycleOperationV1::ALL
                .into_iter()
                .find(|candidate| *candidate != operation)
                .expect("the operation inventory is not a singleton")
        } else {
            operation
        };
        let response_request_id = if self.mutation == ResponseMutation::WrongRequestId {
            [0x99; 32]
        } else {
            request_id
        };
        let status = if self.mutation == ResponseMutation::RemoteConflict {
            DeviceLifecycleStatusV1::Conflict
        } else {
            DeviceLifecycleStatusV1::Success
        };
        let payload_operation = if self.mutation == ResponseMutation::RelabelledPayload {
            DeviceLifecycleOperationV1::ALL
                .into_iter()
                .find(|candidate| *candidate != response_operation)
                .expect("the operation inventory is not a singleton")
        } else {
            response_operation
        };
        let payload = if status == DeviceLifecycleStatusV1::Success {
            Writer::response(payload_operation).finish()?.to_vec()
        } else {
            Vec::new()
        };
        Ok(Zeroizing::new(encode_test_response(
            &capabilities,
            command,
            response_operation,
            status,
            response_request_id,
            payload,
            self.mutation,
        )))
    }

    fn authenticate_response(
        &self,
        capabilities: &AuthenticatedDeviceLifecycleCapabilitiesV1,
        response_authentication_digest: Digest,
        authenticator: &[u8],
    ) -> Result<(), DeviceLifecycleBridgeErrorV1> {
        let expected = test_authenticator(capabilities, response_authentication_digest);
        if authenticator == expected {
            Ok(())
        } else {
            Err(DeviceLifecycleBridgeErrorV1::ResponseAuthentication)
        }
    }
}

fn test_authenticator(
    capabilities: &AuthenticatedDeviceLifecycleCapabilitiesV1,
    response_authentication_digest: Digest,
) -> Digest {
    digest_framed(
        TEST_AUTHENTICATOR_DOMAIN,
        &[
            &capabilities.hardware_policy_id,
            &capabilities.attestation_digest,
            &response_authentication_digest,
        ],
    )
}

fn capability_frame(
    platform: DeviceLifecyclePlatformV1,
    hardware_policy_id: Digest,
    attestation_digest: Digest,
) -> [u8; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1] {
    let mut frame = [0; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1];
    frame[..8].copy_from_slice(CAPABILITY_MAGIC);
    frame[8..10].copy_from_slice(&DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1.to_le_bytes());
    frame[10] = platform as u8;
    frame[12..16].copy_from_slice(&DEVICE_LIFECYCLE_REQUIRED_CAPABILITY_MASK_V1.to_le_bytes());
    frame[16..20].copy_from_slice(
        &u32::try_from(DEVICE_LIFECYCLE_MAX_COMMAND_PAYLOAD_BYTES_V1)
            .expect("test constant fits u32")
            .to_le_bytes(),
    );
    frame[20..24].copy_from_slice(
        &u32::try_from(DEVICE_LIFECYCLE_MAX_RESPONSE_PAYLOAD_BYTES_V1)
            .expect("test constant fits u32")
            .to_le_bytes(),
    );
    frame[24..56].copy_from_slice(&hardware_policy_id);
    frame[56..88].copy_from_slice(&attestation_digest);
    frame
}

fn test_transport(mutation: ResponseMutation) -> TestTransport {
    let attestation_digest = sha256(TEST_ATTESTATION_EVIDENCE);
    TestTransport {
        capability_frame: capability_frame(
            DeviceLifecyclePlatformV1::Android,
            [0x21; 32],
            attestation_digest,
        )
        .to_vec(),
        expected_evidence: TEST_ATTESTATION_EVIDENCE.to_vec(),
        authenticate_capability: true,
        mutation,
    }
}

fn adapter(mutation: ResponseMutation) -> DeviceLifecycleBackendAdapterV1<TestTransport> {
    DeviceLifecycleBackendAdapterV1::authenticate(
        test_transport(mutation),
        DeviceLifecycleExpectedIdentityV1::new(
            DeviceLifecyclePlatformV1::Android,
            [0x21; 32],
            sha256(TEST_ATTESTATION_EVIDENCE),
        )
        .expect("valid expected identity"),
        TEST_ATTESTATION_EVIDENCE,
    )
    .expect("test transport authenticates")
}

fn inspect_command(
    command: &[u8],
) -> Result<(DeviceLifecycleOperationV1, Digest), DeviceLifecycleBridgeErrorV1> {
    if command.len() < DEVICE_LIFECYCLE_COMMAND_HEADER_BYTES_V1 {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let mut reader = Reader::new(command);
    reader.expect(COMMAND_MAGIC)?;
    if reader.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1 {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let operation = DeviceLifecycleOperationV1::from_u8(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let request_id = reader.digest()?;
    let payload_len =
        usize::try_from(reader.u32()?).map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
    let payload_digest = reader.digest()?;
    let payload = reader.bytes(payload_len)?;
    if !reader.is_empty()
        || payload.is_empty()
        || payload.len() > DEVICE_LIFECYCLE_MAX_COMMAND_PAYLOAD_BYTES_V1
        || sha256(payload) != payload_digest
        || !valid_digest(request_id)
    {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let mut typed = Reader::new(payload);
    typed.expect(COMMAND_PAYLOAD_MAGIC)?;
    if typed.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1
        || typed.u8()? != operation as u8
        || typed.u8()? != 0
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    Ok((operation, request_id))
}

fn encode_test_response(
    capabilities: &AuthenticatedDeviceLifecycleCapabilitiesV1,
    command: &[u8],
    operation: DeviceLifecycleOperationV1,
    status: DeviceLifecycleStatusV1,
    request_id: Digest,
    mut payload: Vec<u8>,
    mutation: ResponseMutation,
) -> Vec<u8> {
    let success = status == DeviceLifecycleStatusV1::Success;
    let authenticator_len = if success { 32_u32 } else { 0 };
    let mut response =
        Vec::with_capacity(DEVICE_LIFECYCLE_RESPONSE_HEADER_BYTES_V1 + payload.len() + 32);
    response.extend_from_slice(RESPONSE_MAGIC);
    response.extend_from_slice(&DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1.to_le_bytes());
    response.push(operation as u8);
    response.push(status as u8);
    response.extend_from_slice(&request_id);
    response.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test response payload fits u32")
            .to_le_bytes(),
    );
    response.extend_from_slice(&authenticator_len.to_le_bytes());
    response.extend_from_slice(&sha256(&payload));
    debug_assert_eq!(response.len(), RESPONSE_AUTHENTICATOR_DIGEST_OFFSET);

    let mut authenticator = if success {
        let digest = response_authentication_digest(capabilities, command, &response, &payload);
        test_authenticator(capabilities, digest).to_vec()
    } else {
        Vec::new()
    };
    if mutation == ResponseMutation::ResponseContentAfterAuthentication {
        payload.push(0x41);
        response[44..48].copy_from_slice(
            &u32::try_from(payload.len())
                .expect("test response payload fits u32")
                .to_le_bytes(),
        );
        response[52..84].copy_from_slice(&sha256(&payload));
    } else if mutation == ResponseMutation::RejectedAuthenticator {
        authenticator[0] ^= 0x80;
    }
    response.extend_from_slice(&sha256(&authenticator));
    response.extend_from_slice(&payload);
    response.extend_from_slice(&authenticator);
    response
}

#[test]
fn operation_and_status_inventories_are_exact() {
    assert_eq!(DEVICE_LIFECYCLE_OPERATION_COUNT_V1, 14);
    assert_eq!(
        DeviceLifecycleOperationV1::ALL.map(|operation| operation as u8),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    assert_eq!(
        DeviceLifecycleStatusV1::ALL.map(|status| status as u8),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1, 96);
    assert_eq!(DEVICE_LIFECYCLE_REQUIRED_CAPABILITY_MASK_V1, 0x01ff);
}

#[test]
fn exact_capability_and_separate_evidence_are_required() {
    let attestation_digest = sha256(TEST_ATTESTATION_EVIDENCE);
    let expected = DeviceLifecycleExpectedIdentityV1::new(
        DeviceLifecyclePlatformV1::Android,
        [0x21; 32],
        attestation_digest,
    )
    .expect("valid expected identity");
    let accepted = DeviceLifecycleBackendAdapterV1::authenticate(
        test_transport(ResponseMutation::None),
        expected,
        TEST_ATTESTATION_EVIDENCE,
    )
    .expect("exact capability and evidence authenticate");
    assert_eq!(
        accepted.capabilities().platform,
        DeviceLifecyclePlatformV1::Android
    );

    let wrong_evidence = DeviceLifecycleBackendAdapterV1::authenticate(
        test_transport(ResponseMutation::None),
        expected,
        b"different evidence",
    );
    assert!(matches!(
        wrong_evidence,
        Err(DeviceLifecycleBridgeErrorV1::CapabilityAuthentication)
    ));
    let mut transport = test_transport(ResponseMutation::None);
    transport.authenticate_capability = false;
    let self_asserted = DeviceLifecycleBackendAdapterV1::authenticate(
        transport,
        expected,
        TEST_ATTESTATION_EVIDENCE,
    );
    assert!(matches!(
        self_asserted,
        Err(DeviceLifecycleBridgeErrorV1::CapabilityAuthentication)
    ));
}

#[test]
fn every_capability_field_fails_closed() {
    let original = capability_frame(
        DeviceLifecyclePlatformV1::Android,
        [0x21; 32],
        sha256(TEST_ATTESTATION_EVIDENCE),
    );
    assert!(decode_capabilities(&original).is_ok());

    for offset in [0, 8, 10, 11, 12, 15, 16, 20, 88, 95] {
        let mut mutated = original;
        mutated[offset] ^= 0x80;
        assert_eq!(
            decode_capabilities(&mutated),
            Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch),
            "offset {offset} must be authenticated exactly"
        );
    }
    let mut partial_mask = original;
    partial_mask[12..16].copy_from_slice(&(0x00ff_u32).to_le_bytes());
    assert_eq!(
        decode_capabilities(&partial_mask),
        Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch)
    );
    let mut unknown_bit = original;
    unknown_bit[12..16].copy_from_slice(&(0x03ff_u32).to_le_bytes());
    assert_eq!(
        decode_capabilities(&unknown_bit),
        Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch)
    );
    let mut equal_digests = original;
    equal_digests[56..88].copy_from_slice(&original[24..56]);
    assert_eq!(
        decode_capabilities(&equal_digests),
        Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch)
    );
    for offset in [24, 56] {
        let mut transport = test_transport(ResponseMutation::None);
        transport.capability_frame[offset] ^= 0x80;
        let result = DeviceLifecycleBackendAdapterV1::authenticate(
            transport,
            DeviceLifecycleExpectedIdentityV1::new(
                DeviceLifecyclePlatformV1::Android,
                [0x21; 32],
                sha256(TEST_ATTESTATION_EVIDENCE),
            )
            .expect("valid expected identity"),
            TEST_ATTESTATION_EVIDENCE,
        );
        assert!(matches!(
            result,
            Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch)
        ));
    }
    assert!(
        DeviceLifecycleExpectedIdentityV1::new(DeviceLifecyclePlatformV1::Ios, [0; 32], [1; 32])
            .is_err()
    );
    assert!(
        DeviceLifecycleExpectedIdentityV1::new(DeviceLifecyclePlatformV1::Ios, [1; 32], [1; 32])
            .is_err()
    );
}

#[test]
fn all_fourteen_operations_round_trip_only_after_response_authentication() {
    let adapter = adapter(ResponseMutation::None);
    for operation in DeviceLifecycleOperationV1::ALL {
        let payload = Writer::command(operation)
            .finish()
            .expect("typed command header is bounded");
        adapter
            .call(operation, payload, |payload| {
                let reader = begin_response_payload(payload, operation)?;
                require_end(&reader)
            })
            .unwrap_or_else(|error| panic!("operation {operation:?} failed: {error:?}"));
    }
}

#[test]
fn relabel_request_and_response_are_rejected() {
    let capabilities = *adapter(ResponseMutation::None).capabilities();
    let operation = DeviceLifecycleOperationV1::ReserveReceiveIntentAndSign;
    let other = DeviceLifecycleOperationV1::RecoverReceiveIntentAndSignature;
    let payload = Writer::command(operation).finish().expect("typed payload");
    assert_eq!(
        encode_command(&capabilities, other, &payload),
        Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)
    );
    let error = adapter(ResponseMutation::RelabelledPayload)
        .call(operation, payload, |_| Ok(()))
        .expect_err("response payload operation relabelling must fail");
    assert_eq!(error, DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
}

#[test]
fn response_echo_content_and_authenticator_fail_closed() {
    let operation = DeviceLifecycleOperationV1::RecoverActiveIntent;
    for (mutation, expected) in [
        (
            ResponseMutation::WrongOperation,
            DeviceLifecycleBridgeErrorV1::MalformedFrame,
        ),
        (
            ResponseMutation::WrongRequestId,
            DeviceLifecycleBridgeErrorV1::MalformedFrame,
        ),
        (
            ResponseMutation::ResponseContentAfterAuthentication,
            DeviceLifecycleBridgeErrorV1::ResponseAuthentication,
        ),
        (
            ResponseMutation::RejectedAuthenticator,
            DeviceLifecycleBridgeErrorV1::ResponseAuthentication,
        ),
        (
            ResponseMutation::RemoteConflict,
            DeviceLifecycleBridgeErrorV1::RemoteStatus(DeviceLifecycleStatusV1::Conflict),
        ),
    ] {
        let payload = Writer::command(operation).finish().expect("typed payload");
        let error = adapter(mutation)
            .call(operation, payload, |_| Ok(()))
            .expect_err("adversarial response must fail");
        assert_eq!(error, expected, "mutation {mutation:?}");
    }
}

#[test]
fn malformed_response_lengths_digests_and_failure_payloads_are_rejected() {
    let capabilities = *adapter(ResponseMutation::None).capabilities();
    let operation = DeviceLifecycleOperationV1::StagePayment;
    let command_payload = Writer::command(operation).finish().expect("typed payload");
    let command = encode_command(&capabilities, operation, &command_payload).expect("command");
    let (_, request_id) = inspect_command(&command).expect("well-formed command");
    let payload = Writer::response(operation)
        .finish()
        .expect("typed response")
        .to_vec();
    let valid = encode_test_response(
        &capabilities,
        &command,
        operation,
        DeviceLifecycleStatusV1::Success,
        request_id,
        payload.clone(),
        ResponseMutation::None,
    );
    assert!(decode_response(&valid, operation, request_id).is_ok());

    for offset in [0, 8, 44, 48, 52, 84, 115] {
        let mut mutated = valid.clone();
        mutated[offset] ^= 0x40;
        assert!(
            decode_response(&mutated, operation, request_id).is_err(),
            "offset {offset} must be checked"
        );
    }
    let failure_with_payload = encode_test_response(
        &capabilities,
        &command,
        operation,
        DeviceLifecycleStatusV1::Conflict,
        request_id,
        payload,
        ResponseMutation::None,
    );
    assert!(matches!(
        decode_response(&failure_with_payload, operation, request_id),
        Err(DeviceLifecycleBridgeErrorV1::MalformedFrame)
    ));
}

#[test]
fn status_taxonomy_maps_without_authority_widening() {
    assert_eq!(
        guard_error(DeviceLifecycleBridgeErrorV1::RemoteStatus(
            DeviceLifecycleStatusV1::TrustedTimeRejected,
        )),
        HardwareGuardErrorV1::TrustedTimeRejected
    );
    assert_eq!(
        guard_error(DeviceLifecycleBridgeErrorV1::RemoteStatus(
            DeviceLifecycleStatusV1::Conflict,
        )),
        HardwareGuardErrorV1::StaleOrConcurrent
    );
    assert_eq!(
        guard_error(DeviceLifecycleBridgeErrorV1::ResponseAuthentication),
        HardwareGuardErrorV1::PolicyRejected
    );
    assert_eq!(
        outbox_error(DeviceLifecycleBridgeErrorV1::RemoteStatus(
            DeviceLifecycleStatusV1::Missing,
        )),
        AuthenticatedPaymentOutboxErrorV1::Missing
    );
    assert_eq!(
        outbox_error(DeviceLifecycleBridgeErrorV1::CapabilityAuthentication),
        AuthenticatedPaymentOutboxErrorV1::Corrupt
    );
}

#[test]
fn sender_order_remains_stage_then_hardware_cas_then_authenticated_publish() {
    let source = include_str!("send.rs");
    let start = source
        .find("pub(crate) fn publish_send_split_v1")
        .expect("publish flow exists");
    let end = source[start..]
        .find("pub(crate) fn recover_published_send_v1")
        .map(|offset| start + offset)
        .expect("publish flow has a bounded source section");
    let flow = &source[start..end];
    let staged = flow
        .find("recover_staged_payment_digest")
        .expect("authenticated staging is recovered first");
    let hardware_cas = flow
        .find("publish_send_payment")
        .expect("hardware CAS follows staging");
    let authenticated_publish = flow
        .find("publish_payment_or_recover")
        .expect("outbox is marked publishable after CAS");
    let exposure = flow
        .find("payment_from_outbox_record")
        .expect("canonical payment is exposed last");
    assert!(staged < hardware_cas);
    assert!(hardware_cas < authenticated_publish);
    assert!(authenticated_publish < exposure);
}

#[test]
fn adapter_source_has_no_secret_output_or_software_fallback() {
    let source = include_str!("device_bridge.rs");
    for forbidden in [
        "println!",
        "eprintln!",
        "dbg!",
        "tracing::",
        "log::",
        "software fallback",
        "private_key",
    ] {
        assert!(
            !source.contains(forbidden),
            "device adapter must not contain `{forbidden}`"
        );
    }
    assert!(source.contains("Zeroizing<Vec<u8>>"));
    assert!(source.contains("transport_sealed::Sealed"));
}

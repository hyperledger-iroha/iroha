// Exercise the actual stream-token client over isolated socket pairs. The peer uses a
// deterministic software key only as a protocol fixture; this is not hardware qualification.
mod stream_token_mutation_tests {
    use super::*;
    use std::io::{Read as _, Write as _};

    struct Fixture {
        signer: StreamTokenHardwareBrokerClient,
        peer: UnixStream,
        reconnect_listener: UnixListener,
        _directory: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("isolated endpoint directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private endpoint directory");
        let path = directory.path().join("stream-token.sock");
        let reconnect_listener = UnixListener::bind(&path).expect("isolated reconnect listener");
        set_socket_mode(&path).expect("exact test endpoint mode");
        reconnect_listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let (stream, peer) = UnixStream::pair().expect("isolated accepted broker session");
        for socket in [&stream, &peer] {
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("bounded read");
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("bounded write");
        }
        let mut binding = token_signer_binding();
        binding.handle = "hsm://sorafs/stream-token/primary-a".to_owned();
        validate_wire_binding(&binding).expect("canonical simulated peer binding");
        validate_observation(&binding, &observation(&binding))
            .expect("exact simulated peer observation");
        let metadata_digest = observation(&binding).metadata_digest;
        let signer = StreamTokenHardwareBrokerClient {
            session: Arc::new(BrokerSession {
                connection: Mutex::new(BrokerConnection {
                    stream,
                    session_id: TEST_SESSION_ID,
                    next_request_id: 1,
                    poison_reason: None,
                }),
                chain_id: "stream-token-mutation-test-chain".to_owned(),
                network_id: network_id(),
                endpoint: EndpointPolicy::for_test(path),
                requested_catalog: vec![binding.clone()],
            }),
            binding,
            metadata_digest,
            latch: Arc::new(Mutex::new(StreamTokenSignLatchV1::Idle)),
        };
        Fixture {
            signer,
            peer,
            reconnect_listener,
            _directory: directory,
        }
    }

    fn token_body() -> sorafs_manifest::StreamTokenBodyV1 {
        stream_token_hardware_test_support::body()
    }

    fn expected() -> SignerStreamTokenExpectedV1 {
        stream_token_hardware_test_support::expected(&token_body())
    }

    fn signing_payload() -> Vec<u8> {
        token_body()
            .signing_payload_bytes()
            .expect("canonical domain-separated token payload")
    }

    fn read_request(peer: &mut UnixStream, id: u64, operation: u16) -> OperationRequestV1 {
        let (slot, actual_operation, bytes) =
            read_operation_request_frame(peer).expect("one complete admitted broker request");
        assert_eq!(
            slot,
            IrohaRuntimeProviderSlotV1::StreamTokenSigner.wire_id()
        );
        assert_eq!(actual_operation, operation);
        let request = decode_operation_frame::<OperationRequestV1>(
            &bytes,
            FRAME_KIND_OPERATION_REQUEST_V1,
            operation,
        )
        .expect("canonical request frame");
        validate_operation_request(&request).expect("valid typed operation payload");
        assert_eq!(request.request_id, id);
        assert_eq!(request.operation, operation);
        request
    }

    fn write_response(peer: &mut UnixStream, request: &OperationRequestV1, result: Vec<u8>) {
        let response = make_operation_response(request, STATUS_OK_V1, result, &network_id())
            .expect("valid response for the exact request");
        write_response_frame(peer, &response);
    }

    fn write_response_frame(peer: &mut UnixStream, response: &OperationResponseV1) {
        let limit = operation_frame_limit(response.operation);
        let frame = encode_frame(FRAME_KIND_OPERATION_RESPONSE_V1, response, limit)
            .expect("canonical response frame");
        write_length_prefixed(peer, &frame, limit).expect("deliver response");
    }

    fn qualify(peer: &mut UnixStream, id: u64) {
        let request = read_request(peer, id, OPERATION_QUALIFY_V1);
        let result = encode_canonical(
            request
                .binding
                .stream_token_hardware_binding
                .as_ref()
                .expect("complete hardware metadata"),
            MAX_STREAM_TOKEN_METADATA_BYTES_V1,
        )
        .expect("encode exact non-authorizing metadata");
        write_response(peer, &request, result);
    }

    fn admit_sign(peer: &mut UnixStream) -> (OperationRequestV1, Vec<u8>) {
        let request = read_request(peer, 2, OPERATION_STREAM_TOKEN_SIGN_V1);
        assert_eq!(
            request.payload,
            signing_payload(),
            "the originally admitted body is exact"
        );
        let result = stream_token_hardware_test_support::receipt(&request.payload)
            .encode_canonical()
            .expect("canonical untrusted receipt claims");
        (request, result)
    }

    fn assert_no_request_bytes(peer: &mut UnixStream) {
        peer.set_nonblocking(true)
            .expect("inspect outbound queue without waiting");
        let mut byte = [0_u8; 1];
        let error = peer
            .read(&mut byte)
            .expect_err("no second request may reach the peer");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    }

    fn assert_fenced(
        signer: &StreamTokenHardwareBrokerClient,
        listener: &UnixListener,
        next_id: u64,
    ) {
        {
            let connection = signer.session.connection.lock().expect("session state");
            assert_eq!(
                connection.next_request_id, next_id,
                "every admitted qualification and Sign ID is retired"
            );
            assert_eq!(connection.poison_reason, Some(BrokerError::Ambiguous));
            assert_eq!(connection.session_id, TEST_SESSION_ID);
        }
        assert!(matches!(
            signer.sign(&expected(), &token_body()),
            Err(StreamTokenHardwareCallErrorV1::AmbiguousCompletion)
        ));
        assert!(matches!(signer.metadata(), Err(BrokerError::Ambiguous)));
        assert_eq!(signer.session.reconnect(), Err(BrokerError::Ambiguous));
        let error = listener
            .accept()
            .expect_err("poison blocks even a fresh endpoint connection");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        let connection = signer
            .session
            .connection
            .lock()
            .expect("unchanged fenced state");
        assert_eq!(
            connection.next_request_id, next_id,
            "failed retries allocate no new request ID"
        );
        assert_eq!(connection.poison_reason, Some(BrokerError::Ambiguous));
    }

    #[derive(Clone, Copy, Debug)]
    enum Failure {
        Lost,
        Truncated,
        MalformedFrame,
        SubstitutedRequest,
        SubstitutedBody,
        MalformedResult,
        WrongSignature,
    }

    fn fail_response(
        peer: &mut UnixStream,
        request: &OperationRequestV1,
        result: Vec<u8>,
        failure: Failure,
    ) {
        match failure {
            Failure::Lost => {}
            Failure::Truncated => {
                peer.write_all(&32_u32.to_be_bytes())
                    .expect("declared response length");
                peer.write_all(&[0x51; 7])
                    .expect("incomplete response bytes");
            }
            Failure::MalformedFrame => {
                write_length_prefixed(peer, &[0x51; 32], MAX_STREAM_TOKEN_HARDWARE_FRAME_BYTES_V1)
                    .expect("complete invalid frame");
            }
            Failure::SubstitutedRequest | Failure::SubstitutedBody => {
                let mut body = signing_payload();
                let id = if matches!(failure, Failure::SubstitutedRequest) {
                    99
                } else {
                    2
                };
                if matches!(failure, Failure::SubstitutedBody) {
                    // The peer supplies a fully self-consistent envelope for another body.
                    let mut other_body = token_body();
                    other_body.token_id = "1123456789abcdef0123456789abcdef".to_owned();
                    body = other_body
                        .signing_payload_bytes()
                        .expect("another canonical token body");
                }
                let other = make_operation_request(
                    request.session_id,
                    id,
                    request.binding.clone(),
                    request.provider_metadata_digest,
                    request.operation,
                    body.clone(),
                )
                .expect("substituted request envelope");
                validate_operation_request(&other)
                    .expect("substitution itself is a valid signing request");
                let result = stream_token_hardware_test_support::receipt(&body)
                    .encode_canonical()
                    .expect("substituted canonical receipt claims");
                let response = make_operation_response(&other, STATUS_OK_V1, result, &network_id())
                    .expect("self-consistent substituted response");
                assert_eq!(
                    validate_operation_response_for_client(request, &response, &network_id()),
                    Err(BrokerError::Protocol)
                );
                write_response_frame(peer, &response);
            }
            Failure::MalformedResult | Failure::WrongSignature => {
                let mut response =
                    make_operation_response(request, STATUS_OK_V1, result, &network_id())
                        .expect("valid original signed response");
                response.result = if matches!(failure, Failure::MalformedResult) {
                    vec![0x41; 17]
                } else {
                    let mut receipt = stream_token_hardware_test_support::receipt(&request.payload);
                    receipt.signatures[0].signature.fill(0);
                    receipt
                        .encode_canonical()
                        .expect("canonical invalid role signature claim")
                };
                response.result_digest = operation_result_digest(&response.result);
                response.response_digest = operation_response_digest(&OperationResponseFieldsV1 {
                    session_id: response.session_id,
                    request_id: response.request_id,
                    request_digest: response.request_digest,
                    observed_binding: response.observed_binding.clone(),
                    provider_metadata_digest: response.provider_metadata_digest,
                    operation: response.operation,
                    payload_digest: response.payload_digest,
                    status: response.status,
                    result_digest: response.result_digest,
                    result_len: response
                        .result
                        .len()
                        .try_into()
                        .expect("bounded result length"),
                })
                .expect("rebound response digest");
                validate_operation_response_envelope(request, &response)
                    .expect("typed-result negative control has a valid envelope");
                assert!(
                    validate_operation_response_for_client(request, &response, &network_id())
                        .is_err()
                );
                write_response_frame(peer, &response);
            }
        }
        peer.shutdown(std::net::Shutdown::Write)
            .expect("end only the response direction");
    }

    #[test]
    fn uncertain_sign_response_retires_id_and_blocks_resign_and_reconnect() {
        for failure in [
            Failure::Lost,
            Failure::Truncated,
            Failure::MalformedFrame,
            Failure::SubstitutedRequest,
            Failure::SubstitutedBody,
            Failure::MalformedResult,
            Failure::WrongSignature,
        ] {
            let Fixture {
                signer,
                mut peer,
                reconnect_listener,
                _directory,
            } = fixture();
            let client = signer.clone();
            let signing = thread::spawn(move || client.sign(&expected(), &token_body()));
            qualify(&mut peer, 1);
            let (request, result) = admit_sign(&mut peer);
            fail_response(&mut peer, &request, result, failure);
            assert!(
                matches!(
                    signing.join().expect("signing client terminates"),
                    Err(StreamTokenHardwareCallErrorV1::AmbiguousCompletion)
                ),
                "{failure:?}"
            );
            assert_fenced(&signer, &reconnect_listener, 3);
            assert_no_request_bytes(&mut peer);
        }
    }

    #[test]
    fn exact_sign_response_requires_pre_and_post_qualification_and_preserves_signature() {
        let Fixture {
            signer,
            mut peer,
            reconnect_listener,
            _directory,
        } = fixture();
        let client = signer.clone();
        let signing = thread::spawn(move || client.sign(&expected(), &token_body()));
        qualify(&mut peer, 1);
        let (request, result) = admit_sign(&mut peer);
        let original_receipt = result.clone();
        write_response(&mut peer, &request, result);
        qualify(&mut peer, 3);
        let actual = signing
            .join()
            .expect("signing client terminates")
            .expect("bounded receipt");
        assert_eq!(actual.bytes(), original_receipt);
        let receipt = SignerStreamTokenReceiptV1::decode_canonical(actual.bytes())
            .expect("canonical receipt");
        verify_evidence_viewer_ed25519_signature(
            test_auth_public_key(),
            receipt
                .role_signature_claim()
                .expect("exact role signature shape"),
            &signing_payload(),
        )
        .expect("role signature verifies for original exact token body");
        let connection = signer
            .session
            .connection
            .lock()
            .expect("successful session");
        assert_eq!(connection.next_request_id, 4);
        assert_eq!(connection.poison_reason, None);
        drop(connection);
        assert_no_request_bytes(&mut peer);
        assert_eq!(
            reconnect_listener
                .accept()
                .expect_err("no hidden reconnect")
                .kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn accepted_sign_then_uncertain_postqualification_retires_id_and_fences_output() {
        for failure in 0..3 {
            let Fixture {
                signer,
                mut peer,
                reconnect_listener,
                _directory,
            } = fixture();
            let client = signer.clone();
            let signing = thread::spawn(move || client.sign(&expected(), &token_body()));
            qualify(&mut peer, 1);
            let (request, result) = admit_sign(&mut peer);
            write_response(&mut peer, &request, result);
            let postqualification = read_request(&mut peer, 3, OPERATION_QUALIFY_V1);
            match failure {
                0 => {}
                1 => {
                    peer.write_all(&32_u32.to_be_bytes())
                        .expect("declared postqualification length");
                    peer.write_all(&[0x51; 7])
                        .expect("incomplete postqualification bytes");
                }
                2 => {
                    let unit = encode_canonical(&(), MAX_QUALIFICATION_FRAME_BYTES_V1)
                        .expect("payload-free outage");
                    let response = make_operation_response(
                        &postqualification,
                        STATUS_UNAVAILABLE_V1,
                        unit,
                        &network_id(),
                    )
                    .expect("exact unavailable read response");
                    write_response_frame(&mut peer, &response);
                }
                _ => unreachable!("fixed failure cases"),
            }
            peer.shutdown(std::net::Shutdown::Write)
                .expect("end postqualification response");
            assert!(matches!(
                signing.join().expect("signing client terminates"),
                Err(StreamTokenHardwareCallErrorV1::AmbiguousCompletion)
            ));
            assert_fenced(&signer, &reconnect_listener, 4);
            assert_no_request_bytes(&mut peer);
        }
    }

    #[test]
    fn accepted_sign_keeps_terminal_postqualification_denials_fenced() {
        for reason in [BrokerError::StaleOrRevoked, BrokerError::Protocol] {
            let Fixture {
                signer,
                mut peer,
                reconnect_listener,
                _directory,
            } = fixture();
            let client = signer.clone();
            let signing = thread::spawn(move || client.sign(&expected(), &token_body()));
            qualify(&mut peer, 1);
            let (request, result) = admit_sign(&mut peer);
            write_response(&mut peer, &request, result);
            let postqualification = read_request(&mut peer, 3, OPERATION_QUALIFY_V1);
            if reason == BrokerError::StaleOrRevoked {
                let unit = encode_canonical(&(), MAX_QUALIFICATION_FRAME_BYTES_V1)
                    .expect("payload-free revocation");
                let response = make_operation_response(
                    &postqualification,
                    STATUS_STALE_OR_REVOKED_V1,
                    unit,
                    &network_id(),
                )
                .expect("exact terminal revocation response");
                write_response_frame(&mut peer, &response);
            } else {
                write_length_prefixed(&mut peer, &[0x51; 32], MAX_QUALIFICATION_FRAME_BYTES_V1)
                    .expect("complete malformed final-read response");
            }
            assert_eq!(
                signing.join().expect("signing client terminates").err(),
                Some(stream_token_transport_error(reason))
            );
            {
                let connection = signer
                    .session
                    .connection
                    .lock()
                    .expect("terminal denial state");
                assert_eq!(connection.next_request_id, 4);
                assert_eq!(connection.poison_reason, Some(reason));
            }
            assert_eq!(
                signer.sign(&expected(), &token_body()).err(),
                Some(stream_token_transport_error(reason))
            );
            assert_eq!(signer.session.reconnect(), Err(reason));
            assert_eq!(
                reconnect_listener
                    .accept()
                    .expect_err("terminal denial never reconnects")
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            assert_no_request_bytes(&mut peer);
            let connection = signer
                .session
                .connection
                .lock()
                .expect("unchanged terminal denial");
            assert_eq!(connection.next_request_id, 4);
            assert_eq!(connection.poison_reason, Some(reason));
        }
    }

    #[test]
    fn prequalification_outage_sends_no_sign_and_permits_fresh_authenticated_connection() {
        let Fixture {
            signer,
            mut peer,
            reconnect_listener,
            _directory,
        } = fixture();
        let client = signer.clone();
        let signing = thread::spawn(move || client.sign(&expected(), &token_body()));
        read_request(&mut peer, 1, OPERATION_QUALIFY_V1);
        peer.shutdown(std::net::Shutdown::Write)
            .expect("lose only the prequalification response");
        assert!(matches!(
            signing.join().expect("signing client terminates"),
            Err(StreamTokenHardwareCallErrorV1::Unavailable)
        ));
        {
            let connection = signer
                .session
                .connection
                .lock()
                .expect("prequalification failure state");
            assert_eq!(connection.next_request_id, 2);
            assert_eq!(connection.poison_reason, Some(BrokerError::Unavailable));
        }
        assert_no_request_bytes(&mut peer);

        let client = Arc::clone(&signer.session);
        let reconnect = thread::spawn(move || client.reconnect());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut replacement = loop {
            match reconnect_listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "explicit reconnect must reach endpoint"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("reconnect accept failed: {error}"),
            }
        };
        replacement
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("bounded replacement read");
        replacement
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("bounded replacement write");
        let frame = read_length_prefixed(&mut replacement, MAX_HANDSHAKE_FRAME_BYTES_V1)
            .expect("fresh handshake");
        let request = decode_frame::<HandshakeRequestV1>(
            &frame,
            FRAME_KIND_HANDSHAKE_REQUEST_V1,
            MAX_HANDSHAKE_FRAME_BYTES_V1,
        )
        .expect("canonical handshake request");
        validate_handshake_request(&request).expect("valid fresh client transcript");
        assert_eq!(request.chain_id, signer.session.chain_id);
        assert_eq!(request.network_id, network_id());
        assert_eq!(request.requested_catalog, vec![signer.binding.clone()]);
        let response =
            make_handshake_response(&request, [0xB7; 32], vec![observation(&signer.binding)])
                .expect("exact binding on fresh session");
        let frame = encode_frame(
            FRAME_KIND_HANDSHAKE_RESPONSE_V1,
            &response,
            MAX_HANDSHAKE_FRAME_BYTES_V1,
        )
        .expect("canonical handshake response");
        write_length_prefixed(&mut replacement, &frame, MAX_HANDSHAKE_FRAME_BYTES_V1)
            .expect("complete handshake");
        assert_eq!(
            reconnect.join().expect("explicit reconnect terminates"),
            Ok(())
        );
        let connection = signer
            .session
            .connection
            .lock()
            .expect("fresh read-only session");
        assert_eq!(connection.session_id, [0xB7; 32]);
        assert_eq!(connection.next_request_id, 1);
        assert_eq!(connection.poison_reason, None);
        drop(connection);
        assert_no_request_bytes(&mut replacement);
    }
    include!("stream_token_recovery_tests.rs");
}

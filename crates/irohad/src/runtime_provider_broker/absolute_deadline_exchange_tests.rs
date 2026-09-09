// Real local Unix exchanges exercise deadline admission and post-dispatch ambiguity.

fn accept_deadline_test_stream(listener: &UnixListener) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = BrokerDeadlineV1::new(Duration::from_secs(5)).unwrap();
    loop {
        deadline
            .remaining()
            .expect("test peer must connect before its guard expires");
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::park_timeout(Duration::from_millis(1));
            }
            Err(error) => panic!("test peer accept failed: {error}"),
        }
    }
}

#[test]
fn expired_connect_deadline_does_not_open_the_hardened_socket() {
    let (_directory, _path, policy, listener) = bind_fake_broker();
    let deadline = BrokerDeadlineV1::new(Duration::from_nanos(1)).unwrap();
    while deadline.remaining().is_ok() {
        thread::yield_now();
    }
    assert!(matches!(
        connect_verified_before(&policy, deadline),
        Err(BrokerError::Unavailable)
    ));
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn partial_handshake_bytes_cannot_renew_the_original_exchange_deadline() {
    use std::io::Write as _;
    let (_directory, _path, policy, listener) = bind_fake_broker();
    let server = thread::spawn(move || {
        let mut stream = accept_deadline_test_stream(&listener);
        let request = read_handshake(&mut stream);
        let response = handshake_response(&request);
        let frame = encode_frame(
            FRAME_KIND_HANDSHAKE_RESPONSE_V1,
            &response,
            MAX_HANDSHAKE_FRAME_BYTES_V1,
        )
        .unwrap();
        let length = u32::try_from(frame.len()).unwrap().to_be_bytes();
        // The complete valid reply arrives after the caller's deadline, while every individual
        // gap remains well below the old per-syscall timeout. No forged wire codec is used.
        for byte in length {
            if stream.write_all(&[byte]).is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(200));
        }
        let _ = stream.write_all(&frame);
    });
    let result = BrokerSession::connect_before(
        &policy,
        "test-chain",
        server_test_network_id(),
        vec![signer_binding()],
        BrokerDeadlineV1::new(Duration::from_millis(500)).unwrap(),
    );
    assert!(matches!(result, Err(BrokerError::Unavailable)));
    server.join().unwrap();
}

#[test]
fn occupied_session_deadline_does_not_dispatch_retire_or_poison_a_request() {
    use std::io::Read as _;
    let (stream, mut peer) = UnixStream::pair().unwrap();
    let binding = signer_binding();
    let session = BrokerSession {
        connection: Mutex::new(BrokerConnection {
            stream,
            session_id: TEST_SESSION_ID,
            next_request_id: 1,
            poison_reason: None,
        }),
        chain_id: "test-chain".to_owned(),
        network_id: server_test_network_id(),
        endpoint: EndpointPolicy::production(),
        requested_catalog: vec![binding.clone()],
    };
    let held = session.connection.lock().unwrap();
    let result = session.call_before(
        &binding,
        observation(&binding).metadata_digest,
        OPERATION_QUALIFY_V1,
        ScrubbedBytes::new(encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).unwrap()),
        false,
        BrokerDeadlineV1::new(Duration::from_millis(25)).unwrap(),
    );
    assert!(matches!(result, Err(BrokerError::Unavailable)));
    assert_eq!(held.next_request_id, 1);
    assert!(held.poison_reason.is_none());
    peer.set_nonblocking(true).unwrap();
    assert_eq!(
        peer.read(&mut [0_u8; 1]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn dispatched_deadlines_keep_mutating_ambiguity_and_read_only_unavailability() {
    for mutating in [false, true] {
        let (_directory, _path, policy, listener) = bind_fake_broker();
        let (observed_tx, observed_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut stream = accept_deadline_test_stream(&listener);
            let handshake = read_handshake(&mut stream);
            send_handshake(&mut stream, &handshake_response(&handshake));
            let request = read_operation(&mut stream);
            observed_tx
                .send((request.request_id, request.operation))
                .unwrap();
            // Keep the connection open without a reply until the caller proves its deadline.
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
        });
        let binding = signer_binding();
        let (session, observations) = BrokerSession::connect(
            &policy,
            "test-chain",
            server_test_network_id(),
            vec![binding.clone()],
        )
        .unwrap();
        let (operation, payload) = if mutating {
            let payload =
                sorafs_node::governance_dag_key_transition_signing_payload_v1(1, 2, [0x47; 32])
                    .unwrap();
            (
                OPERATION_SIGN_V1,
                encode_canonical(
                    &PurposeSignRequestWireV1 {
                        purpose: sorafs_node::GovernanceDagSigningPurposeV1::KeyTransition
                            .wire_id(),
                        payload,
                    },
                    MAX_OPERATION_FRAME_BYTES_V1,
                )
                .unwrap(),
            )
        } else {
            (
                OPERATION_QUALIFY_V1,
                encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).unwrap(),
            )
        };
        let error = if mutating {
            BrokerError::Ambiguous
        } else {
            BrokerError::Unavailable
        };
        let result = session.call_before(
            &binding,
            observations[0].metadata_digest,
            operation,
            ScrubbedBytes::new(payload),
            mutating,
            BrokerDeadlineV1::new(Duration::from_millis(500)).unwrap(),
        );
        assert!(matches!(result, Err(actual) if actual == error));
        assert_eq!(
            observed_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            (1, operation)
        );
        let connection = session.connection.lock().unwrap();
        assert_eq!(connection.next_request_id, 2);
        assert_eq!(connection.poison_reason, Some(error));
        drop(connection);
        release_tx.send(()).unwrap();
        server.join().unwrap();
    }
}

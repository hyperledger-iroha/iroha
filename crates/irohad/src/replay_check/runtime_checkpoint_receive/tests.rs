//! Real loopback HTTP verifies published paths, exact signatures, finite bodies and deadlines.

use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{collections::BTreeMap, io::Write as _, net::TcpListener};

fn signer() -> KeyPair {
    KeyPair::from_seed(vec![0xCA; 32], iroha_crypto::Algorithm::Ed25519)
}
fn network() -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"HTTP checkpoint root"),
    ))
}
fn descriptor() -> PackageDescriptorV1 {
    PackageDescriptorV1 {
        version: 1,
        chain_id: "HTTP-checkpoint".into(),
        network_id: network(),
        height: 2,
        committed_ms: 3,
        max_chunk_bytes: 1024,
        total_bytes: 4,
        inventory_sha256: [31; 32],
    }
}

struct Reply {
    path: &'static str,
    content_type: &'static str,
    status: &'static str,
    extent: Option<u64>,
    body: Vec<u8>,
}

fn serve(replies: Vec<Reply>, token: bool) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let join = std::thread::spawn(move || {
        for reply in replies {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
                assert!(head.len() <= 16 * 1024);
            }
            let head = String::from_utf8(head).unwrap();
            let mut lines = head.split("\r\n");
            let first: Vec<_> = lines.next().unwrap().split(' ').collect();
            assert_eq!(
                first[1], reply.path,
                "published route spelling is independent of client route builder"
            );
            let method: iroha_torii::Method = first[0].parse().unwrap();
            let uri: iroha_torii::Uri = first[1].parse().unwrap();
            let headers: BTreeMap<_, _> = lines
                .filter_map(|line| line.split_once(':'))
                .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_owned()))
                .collect();
            let len = headers
                .get("content-length")
                .map_or(0, |value| value.parse::<usize>().unwrap());
            assert!(len <= REQUEST_BYTES);
            let mut body = vec![0; len];
            socket.read_exact(&mut body).unwrap();
            assert_eq!(headers["accept-encoding"], "identity");
            assert_eq!(headers.contains_key("x-api-token"), token);
            if token {
                assert_eq!(headers["x-api-token"], "test-checkpoint-token");
            }
            let key: iroha_crypto::PublicKey =
                headers["x-iroha-operator-public-key"].parse().unwrap();
            assert_eq!(&key, signer().public_key());
            let signature = iroha_crypto::ed25519_parse_signature(
                &STANDARD
                    .decode(&headers["x-iroha-operator-signature"])
                    .unwrap(),
            )
            .unwrap();
            let canonical = iroha_torii::canonical_request_message(&method, &uri, &body).unwrap();
            let mut message = b"iroha.operator.http-request.network.v1\0".to_vec();
            message.extend_from_slice(network().as_bytes());
            message.extend_from_slice(&canonical);
            message.extend_from_slice(b"\n");
            message.extend_from_slice(headers["x-iroha-operator-timestamp-ms"].as_bytes());
            message.extend_from_slice(b"\n");
            message.extend_from_slice(headers["x-iroha-operator-nonce"].as_bytes());
            signature.verify(&key, &message).unwrap();
            if method == iroha_torii::Method::POST {
                let value: norito::json::Value = norito::json::from_slice(&body).unwrap();
                assert!(value.get("inventory_sha256").is_some());
            }
            write!(
                socket,
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nConnection: close\r\n",
                reply.status, reply.content_type
            )
            .unwrap();
            if let Some(extent) = reply.extent {
                write!(socket, "Content-Length: {extent}\r\n").unwrap();
            }
            socket.write_all(b"\r\n").unwrap();
            let _ = socket.write_all(&reply.body);
        }
    });
    (endpoint, join)
}

fn json_reply(path: &'static str, body: Vec<u8>) -> Reply {
    Reply {
        path,
        content_type: "application/json",
        status: "200 OK",
        extent: Some(body.len() as u64),
        body,
    }
}

#[test]
fn checkpoint_http_receiver_uses_exact_published_paths_and_signed_bodies() {
    let descriptor = descriptor();
    let artifact = ArtifactV1 {
        index: 4,
        length: 4,
        sha256: [32; 32],
    };
    let (endpoint, server) = serve(
        vec![
            json_reply(
                "/v1/runtime/checkpoint/package",
                norito::json::to_vec(&descriptor).unwrap(),
            ),
            json_reply(
                "/v1/runtime/checkpoint/artifact",
                norito::json::to_vec(&artifact).unwrap(),
            ),
            Reply {
                path: "/v1/runtime/checkpoint/range",
                content_type: "application/octet-stream",
                status: "200 OK",
                extent: Some(4),
                body: b"data".to_vec(),
            },
            Reply {
                path: "/v1/runtime/checkpoint/finish",
                content_type: "application/octet-stream",
                status: "204 No Content",
                extent: Some(0),
                body: Vec::new(),
            },
        ],
        true,
    );
    let budget = AllocationBudget::new(128 * 1024);
    let mut token = reqwest::header::HeaderValue::from_static("test-checkpoint-token");
    token.set_sensitive(true);
    let mut source = HttpCheckpointTransport::new(
        &endpoint,
        &signer(),
        network(),
        Some(token),
        &ToriiRuntimeCheckpoint::default(),
        &budget,
    )
    .unwrap();
    assert_eq!(source.descriptor().unwrap(), descriptor);
    assert_eq!(source.artifact(4).unwrap(), artifact);
    let mut output = [0; 4];
    source.read_range(4, 0, &mut output).unwrap();
    assert_eq!(&output, b"data");
    source.finish(&descriptor).unwrap();
    server.join().unwrap();
}

#[test]
fn checkpoint_http_receiver_bounds_metadata_and_exact_ranges() {
    let budget = AllocationBudget::new(128 * 1024);
    for reply in [
        Reply {
            path: "/v1/runtime/checkpoint/package",
            content_type: "application/json",
            status: "200 OK",
            extent: Some((METADATA_BYTES + 1) as u64),
            body: Vec::new(),
        },
        Reply {
            path: "/v1/runtime/checkpoint/package",
            content_type: "application/json",
            status: "200 OK",
            extent: None,
            body: vec![b' '; METADATA_BYTES + 1],
        },
        Reply {
            path: "/v1/runtime/checkpoint/package",
            content_type: "application/octet-stream",
            status: "200 OK",
            extent: Some(0),
            body: Vec::new(),
        },
    ] {
        let (endpoint, server) = serve(vec![reply], false);
        let mut source = HttpCheckpointTransport::new(
            &endpoint,
            &signer(),
            network(),
            None,
            &ToriiRuntimeCheckpoint::default(),
            &budget,
        )
        .unwrap();
        assert!(matches!(
            source.descriptor(),
            Err(ExecutionAttemptError::Rejected(transport::Error::Invalid(
                _
            )))
        ));
        server.join().unwrap();
    }
    for (extent, body, io_error) in [
        (Some(4), b"xx".to_vec(), true),
        (Some(5), b"extra".to_vec(), false),
        (None, b"extra".to_vec(), false),
    ] {
        let (endpoint, server) = serve(
            vec![Reply {
                path: "/v1/runtime/checkpoint/range",
                content_type: "application/octet-stream",
                status: "200 OK",
                extent,
                body,
            }],
            false,
        );
        let mut source = HttpCheckpointTransport::new(
            &endpoint,
            &signer(),
            network(),
            None,
            &ToriiRuntimeCheckpoint::default(),
            &budget,
        )
        .unwrap();
        source.pinned = Some(descriptor());
        let result = source.read_range(4, 0, &mut [0; 4]);
        if io_error {
            assert!(matches!(
                result,
                Err(ExecutionAttemptError::Rejected(transport::Error::Io(_)))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ExecutionAttemptError::Rejected(transport::Error::Invalid(
                    _
                )))
            ));
        }
        server.join().unwrap();
    }
}

#[test]
fn checkpoint_http_receiver_preserves_deadline_and_original_memory_refusal() {
    let policy = ToriiRuntimeCheckpoint::default();
    let budget = AllocationBudget::new(1);
    assert!(matches!(
        HttpCheckpointTransport::new(
            "http://127.0.0.1:1",
            &signer(),
            network(),
            None,
            &policy,
            &budget
        ),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    let budget = AllocationBudget::new(128 * 1024);
    for endpoint in [
        "file:///tmp/checkpoint",
        "https://user:secret@example.test",
        "https://example.test/other",
        "https://example.test/?query",
        "https://example.test/#fragment",
    ] {
        assert!(
            HttpCheckpointTransport::new(endpoint, &signer(), network(), None, &policy, &budget)
                .is_err()
        );
    }
    let mut source = HttpCheckpointTransport::new(
        "http://127.0.0.1:1",
        &signer(),
        network(),
        None,
        &policy,
        &budget,
    )
    .unwrap();
    source.deadline = Instant::now() - Duration::from_secs(1);
    assert!(
        matches!(source.descriptor(), Err(ExecutionAttemptError::Rejected(transport::Error::Io(ref error))) if error.kind() == std::io::ErrorKind::TimedOut)
    );
    assert!(source.inventory().is_err());
}

#[test]
fn checkpoint_receive_api_token_requires_private_bounded_custody() {
    let directory = tempfile::Builder::new()
        .prefix(".checkpoint-token-test-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let private =
        iroha_fs::PrivateDirectory::open_or_create(directory.path().join("private")).unwrap();
    private
        .write_atomic(
            "token",
            b"test-checkpoint-token\n",
            iroha_fs::PublishMode::CreateNew,
        )
        .unwrap();
    let token = read_api_token(&private.path().join("token")).unwrap();
    assert!(token.is_sensitive());
    assert_eq!(token.as_bytes(), b"test-checkpoint-token");
    for (name, bytes) in [
        ("newline", b"bad\nsecond".as_slice()),
        ("space", b"bad token".as_slice()),
        ("empty", b"\n".as_slice()),
    ] {
        private
            .write_atomic(name, bytes, iroha_fs::PublishMode::CreateNew)
            .unwrap();
        assert!(read_api_token(&private.path().join(name)).is_err());
    }
    private
        .write_atomic(
            "oversized",
            &vec![b'x'; TOKEN_BYTES + 2],
            iroha_fs::PublishMode::CreateNew,
        )
        .unwrap();
    assert!(read_api_token(&private.path().join("oversized")).is_err());
    assert!(read_api_token(Path::new("relative-token")).is_err());
}

#[test]
fn checkpoint_receive_keeps_independent_root_pins_before_opening_any_store() {
    let directory = tempfile::tempdir().unwrap();
    let config = super::super::tests::store_config(directory.path());
    let manifest = CheckpointTransferV1 {
        version: 1,
        chain_id: config.common.chain.clone(),
        network_id: NetworkId::from_genesis_hash(config.genesis.expected_hash),
        height: 0,
        block_hash: config.genesis.expected_hash,
        published_state_root: iroha_data_model::sumeragi_finality::KeyedStateRoot([0; 32]),
        snapshot_verification_key: signer().public_key().clone(),
    };
    let output = directory.path().join("unopened");
    assert!(authenticate_received(&config, &output, &manifest).is_err());
    assert!(!output.exists());
    let mut foreign = manifest;
    foreign.height = 2;
    foreign.network_id = network();
    assert!(authenticate_received(&config, &output, &foreign).is_err());
    assert!(!output.exists());
}

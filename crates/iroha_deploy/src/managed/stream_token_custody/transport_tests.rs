//! Real HTTP refusal before initial intent publication; this is not recovery/finality qualification.

use super::*;
use iroha_data_model::transaction::FeePaymentIntent;
use sorafs_manifest::signer::{
    custody::SignerCustodyAuthorityV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use std::{
    collections::BTreeMap,
    io::{self, Read as _, Write as _},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

const FINALITY_PATH: &str = "/v1/bridge/finality/1";

struct Request {
    peer: usize,
    method: String,
    path: String,
}

struct UnavailablePeers {
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<Request>>>,
    workers: Vec<JoinHandle<io::Result<()>>>,
}

impl UnavailablePeers {
    fn start(prepared: &PreparedLocalnet) -> Self {
        let listeners: Vec<_> = prepared
            .peers
            .iter()
            .map(|peer| {
                let url: url::Url = peer.torii_url.parse().unwrap();
                assert_eq!(url.host_str(), Some("127.0.0.1"));
                assert_eq!(url.scheme(), "http");
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, url.port().unwrap()))
                    .expect("bind exactly the generated peer endpoint");
                listener.set_nonblocking(true).unwrap();
                listener
            })
            .collect();
        let capabilities = norito::json::to_vec(&norito::json!({
            "data_model_version": (iroha_data_model::DATA_MODEL_VERSION),
            "signed_transaction_schema_hash_hex":
                (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>()))
        }))
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let workers = listeners
            .into_iter()
            .enumerate()
            .map(|(peer, listener)| {
                let stop = Arc::clone(&stop);
                let requests = Arc::clone(&requests);
                let capabilities = capabilities.clone();
                thread::spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((mut socket, _)) => {
                                let (method, path) = request_line(&mut socket)?;
                                let success = method == "GET" && path == "/v1/node/capabilities";
                                let mut seen = requests.lock().unwrap();
                                if seen.len() >= 32 {
                                    return Err(io::Error::other("HTTP fixture request cap exceeded"));
                                }
                                seen.push(Request { peer, method, path });
                                drop(seen);
                                let (status, body): (&str, &[u8]) = if success {
                                    ("200 OK", &capabilities)
                                } else {
                                    ("503 Service Unavailable", b"unavailable")
                                };
                                write!(
                                    socket,
                                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                )?;
                                socket.write_all(body)?;
                            }
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(5));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Ok(())
                })
            })
            .collect();
        Self {
            stop,
            requests,
            workers,
        }
    }

    fn finish(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for worker in self.workers.drain(..) {
            worker
                .join()
                .expect("HTTP fixture worker panicked")
                .unwrap();
        }
    }
}

impl Drop for UnavailablePeers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn request_line(socket: &mut TcpStream) -> io::Result<(String, String)> {
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = [0; 8192];
    let mut length = 0;
    loop {
        if length == bytes.len() {
            return Err(io::Error::other("HTTP fixture header exceeds bound"));
        }
        let read = socket.read(&mut bytes[length..])?;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        length += read;
        if bytes[..length].windows(4).any(|part| part == b"\r\n\r\n") {
            break;
        }
    }
    let line_end = bytes[..length]
        .windows(2)
        .position(|part| part == b"\r\n")
        .ok_or_else(|| io::Error::other("missing HTTP request line"))?;
    let line = std::str::from_utf8(&bytes[..line_end])
        .map_err(|_| io::Error::other("invalid HTTP request line"))?;
    let fields: Vec<_> = line.split(' ').collect();
    if fields.len() != 3 || fields[2] != "HTTP/1.1" {
        return Err(io::Error::other("invalid HTTP request line"));
    }
    Ok((fields[0].into(), fields[1].into()))
}

fn policy(coordinator: &ManagedStreamTokenCustody) -> SignerCustodyPolicyV1 {
    let now = now_ms().unwrap();
    SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: coordinator.config.chain.to_string(),
            network_id: *coordinator.config.network_id.as_bytes(),
            runtime_handle: "software://stream/runtime".into(),
            key_handle: "software://stream/key".into(),
            service_id: "stream-service".into(),
            administrator_id: "stream-admin".into(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: *coordinator.manifest.provider_id.as_bytes(),
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: coordinator
                .role(StreamTokenAuthorityRole::TokenSigner)
                .unwrap()
                .try_signatory()
                .unwrap()
                .clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [4; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-service".into(),
            administrator_id: "custody-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [5; 32],
        },
        attester_public_key: coordinator
            .role(StreamTokenAuthorityRole::CustodyAttester)
            .unwrap()
            .try_signatory()
            .unwrap()
            .clone(),
        active_from_unix_ms: now - 1_000,
        active_until_unix_ms: now + 300_000,
        max_validity_ms: 60_000,
        max_anchor_age_ms: 30_000,
    }
}

#[test]
fn unavailable_finality_over_http_cannot_publish_intent_quote_or_dispatch() {
    let _resources = crate::managed::native_test_guard();
    // Respect the validation runner's private temporary directory on every platform.
    let temporary = tempfile::Builder::new()
        .prefix(".custody-http-")
        .tempdir()
        .unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "custody-http",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let mut coordinator = ManagedStreamTokenCustody::open(&prepared).unwrap();
    let policy = policy(&coordinator);
    coordinator.validate_policy(&policy).unwrap();
    drop(ports);
    let mut peers = UnavailablePeers::start(&prepared);
    let utc_deadline = now_ms().unwrap() + 60_000;
    for _ in 0..2 {
        let options = BoundedTransactionOptions {
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            max_total_fees: BTreeMap::new(),
            deadline: Instant::now() + Duration::from_secs(15),
        };
        let error = coordinator
            .configure(&policy, utc_deadline, &options)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot read original genesis result")
        );
        let directory = coordinator.directory.open_child("configure").unwrap();
        assert!(journal::read_original(&directory).unwrap().is_none());
        journal::require_empty(&directory).unwrap();
        assert_eq!(
            directory.open_child("transaction").err().unwrap().kind(),
            io::ErrorKind::NotFound
        );
        assert!(
            journal::read_optional(
                &coordinator.directory,
                "current-checkpoint.nrt",
                MAX_CHECKPOINT_BYTES
            )
            .unwrap()
            .is_none()
        );
        drop(coordinator);
        coordinator = ManagedStreamTokenCustody::open(&prepared).unwrap();
    }
    peers.finish();
    let requests = peers.requests.lock().unwrap();
    for request in requests.iter() {
        assert_eq!(
            request.method, "GET",
            "no transaction or quote POST is allowed"
        );
        assert!(
            matches!(
                request.path.as_str(),
                "/v1/node/capabilities" | FINALITY_PATH
            ),
            "no wallet quote, status, dispatch or custody read before finality: {}",
            request.path
        );
    }
    for peer in 0..4 {
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.peer == peer && request.path == FINALITY_PATH)
                .count(),
            2,
            "each refusal must try every independently selected original peer"
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.peer == peer && request.path == "/v1/node/capabilities")
                .count(),
            2,
            "each reopened client must admit the real capabilities response before requesting finality"
        );
    }
}

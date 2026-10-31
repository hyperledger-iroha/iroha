//! Cleartext pools stay direct while ordinary TLS authenticates server certificates.

use super::*;
use std::{
    io::{ErrorKind, Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

#[test]
fn cleartext_pools_remain_direct_and_ordinary_tls_rejects_untrusted_servers() {
    const CHILD: &str = "IROHA_LOOPBACK_TLS_ROOTS_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // Select rustls explicitly so dependency feature unification cannot make the
        // native-TLS backend change the verifier under test.
        let blocking = direct_loopback_http_client_builder()
            .tls_backend_rustls()
            .build()
            .expect("cleartext blocking pool must not load native roots");
        let asynchronous = direct_loopback_async_http_client_builder()
            .tls_backend_rustls()
            .build()
            .expect("cleartext async pool must not load native roots");
        // Platform verification no longer reads SSL_CERT_FILE during construction.
        // Exercise certificate authentication with a real, untrusted local server.
        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .expect("self-signed TLS fixture");
        let configuration = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der())
                .into(),
        )
        .unwrap();
        let tls_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let tls_address = tls_listener.local_addr().unwrap();
        let tls_url = format!("https://localhost:{}/", tls_address.port());
        tls_listener.set_nonblocking(true).unwrap();
        let tls_server = thread::spawn(move || {
            for _ in 0..2 {
                let deadline = Instant::now() + Duration::from_secs(5);
                let socket = loop {
                    match tls_listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "TLS connection acceptance deadline"
                            );
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("TLS fixture listener: {error}"),
                    }
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let session =
                    rustls::ServerConnection::new(Arc::new(configuration.clone())).unwrap();
                let mut stream = rustls::StreamOwned::new(session, socket);
                let mut request = [0; 1];
                assert!(
                    stream.read(&mut request).is_err(),
                    "untrusted TLS handshake must fail"
                );
            }
        });
        let blocking_error = blocking_http_client_builder()
            .tls_backend_rustls()
            .no_proxy()
            .resolve("localhost", tls_address)
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(&tls_url)
            .send()
            .expect_err("ordinary blocking TLS must reject an untrusted server");
        assert_certificate_refusal(&blocking_error);
        let asynchronous_error = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                async_http_client_builder()
                    .tls_backend_rustls()
                    .no_proxy()
                    .resolve("localhost", tls_address)
                    .connect_timeout(Duration::from_secs(2))
                    .timeout(Duration::from_secs(5))
                    .build()
                    .unwrap()
                    .get(&tls_url)
                    .send()
                    .await
            })
            .expect_err("ordinary asynchronous TLS must reject an untrusted server");
        assert_certificate_refusal(&asynchronous_error);
        tls_server.join().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").expect("cleartext listener");
        let url = format!(
            "http://localhost:{}/status",
            listener.local_addr().expect("cleartext address").port()
        );
        let (account, key_pair) = iroha_test_samples::gen_account_in("loopback-policy");
        let pools = Arc::new(ReqwestHttpTransport::default());
        let base = Url::parse(&url).unwrap().join("/").unwrap();
        let config = crate::config::Config {
            chain: iroha_model_base::chain::ChainId::from("loopback-policy"),
            network_id: crate::client::test_network_id(),
            account,
            account_chain_discriminant: 777,
            key_pair,
            basic_auth: None,
            api_token: None,
            torii_api_url: base,
            http_transport_policy: crate::config::HttpTransportPolicy::DirectLoopback,
            torii_request_timeout: Duration::from_secs(2),
            transaction_ttl: Duration::from_secs(100),
            transaction_status_timeout: Duration::from_secs(15),
            transaction_add_nonce: false,
            sorafs_alias_cache: crate::client::default_alias_policy(),
            sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::GuardPq,
            sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::Canary,
        };
        let client = crate::client::Client::builder(config)
            .http_transport(pools.clone())
            .build()
            .unwrap();
        listener.set_nonblocking(true).expect("bounded listener");
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut received = 0;
            while received < 4 && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).expect("blocking stream");
                        stream
                            .set_write_timeout(Some(Duration::from_secs(1)))
                            .expect("bounded response write");
                        let mut request = [0_u8; 2048];
                        let mut length = 0;
                        while !request[..length].ends_with(b"\r\n\r\n") {
                            assert!(length < request.len(), "bounded request headers");
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            assert!(!remaining.is_zero(), "request header deadline");
                            stream
                                .set_read_timeout(Some(remaining.min(Duration::from_secs(1))))
                                .expect("bounded request read");
                            let read = stream
                                .read(&mut request[length..])
                                .expect("cleartext request");
                            assert!(read > 0, "complete request headers");
                            assert!(Instant::now() < deadline, "request header deadline");
                            length += read;
                        }
                        assert!(request[..length].starts_with(b"GET /status HTTP/1.1\r\n"));
                        stream
                            .write_all(
                                b"HTTP/1.1 302 Found\r\nLocation: https://example.invalid/must-not-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .expect("cleartext response");
                        received += 1;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("cleartext listener: {error}"),
                }
            }
            received
        });
        let blocking_result = blocking.get(&url).timeout(Duration::from_secs(2)).send();
        let async_result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("cleartext runtime")
            .block_on(async {
                asynchronous
                    .get(&url)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await
            });
        // No per-request direct_loopback call: the immutable SDK policy selects each pool.
        let sdk_blocking = client
            .default_request(Method::GET, url.parse().unwrap())
            .build()
            .unwrap()
            .send_blocking();
        let sdk_async = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async { client.status().get().await });
        assert_eq!(server.join().expect("cleartext server"), 4);
        assert_eq!(
            blocking_result.unwrap().status(),
            reqwest::StatusCode::FOUND
        );
        assert_eq!(async_result.unwrap().status(), reqwest::StatusCode::FOUND);
        assert_eq!(sdk_blocking.unwrap().status(), http::StatusCode::FOUND);
        assert!(matches!(
            sdk_async,
            Err(crate::Error::Http { status: 302, .. })
        ));
        assert!(pools.blocking.get().is_none());
        assert!(pools.asynchronous.get().is_none());
        assert!(pools.blocking_direct_loopback.get().is_some());
        assert!(pools.asynchronous_direct_loopback.get().is_some());
        return;
    }

    let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "http_default::loopback_tls_tests::cleartext_pools_remain_direct_and_ordinary_tls_rejects_untrusted_servers",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("ALL_PROXY", "http://127.0.0.1:1")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .output()
        .expect("isolated proxy-policy child");
    assert!(
        child.status.success(),
        "TLS and loopback child failed:\n{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}

// Require a certificate-validation cause; DNS, TCP, timeout and EOF failures do
// not establish that ordinary HTTPS still authenticates its peer.
fn assert_certificate_refusal(error: &reqwest::Error) {
    assert!(error.is_connect(), "TLS connection failure: {error:?}");
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = cause {
        if matches!(
            current.downcast_ref::<rustls::Error>(),
            Some(rustls::Error::InvalidCertificate(_))
        ) {
            return;
        }
        // io::Error can retain the Rustls error as its custom payload rather than
        // exposing that payload itself through Error::source().
        cause = current
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .map(|inner| -> &(dyn std::error::Error + 'static) { inner })
            .or_else(|| current.source());
    }
    panic!("expected certificate-validation refusal, got {error:?}");
}

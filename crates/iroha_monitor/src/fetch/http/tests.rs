//! Raw HTTP regressions for polling limits, framing, deadlines and connection custody.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};

fn client() -> PollingClient {
    PollingClient {
        client: PollingClient::builder().no_proxy().build().unwrap(),
    }
}

async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let byte = stream.read_u8().await.expect("complete request headers");
        request.push(byte);
        assert!(request.len() <= 8192);
    }
    request
}

async fn serve_response(response: Vec<u8>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/status", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        // Early rejection of an oversized declared body may close the socket.
        let _ = stream.write_all(&response).await;
    });
    (url, task)
}

#[tokio::test]
async fn accepts_bounded_length_chunked_close_and_empty_bodies() {
    let client = client();
    for (response, expected) in [
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest".as_slice(),
            b"test".as_slice(),
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nte\r\n2\r\nst\r\n0\r\n\r\n",
            b"test",
        ),
        (b"HTTP/1.0 200 OK\r\n\r\ntest", b"test"),
        (b"HTTP/1.1 204 No Content\r\n\r\n", b""),
    ] {
        let (url, task) = serve_response(response.to_vec()).await;
        let result = client.get(&url, 4, Duration::from_secs(1)).await.unwrap();
        assert_eq!(result.body, expected);
        assert!(!result.truncated);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn rejects_oversized_announced_chunked_and_close_bodies() {
    for response in [
        // Reject the announced length without waiting for any body bytes.
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nlarge\r\n0\r\n\r\n",
        b"HTTP/1.0 200 OK\r\n\r\nlarge",
    ] {
        let (url, task) = serve_response(response.to_vec()).await;
        let result = client().get(&url, 4, Duration::from_secs(1)).await.unwrap();
        assert!(result.truncated);
        assert!(result.body.len() <= 4);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn rejects_truncated_and_conflicting_http_framing() {
    for response in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n{}".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n{}",
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\n{}",
        b"HTTP/1.1 200 OK\r\nContent-Length: 18446744073709551615\r\n\r\n",
    ] {
        let (url, task) = serve_response(response.to_vec()).await;
        assert!(matches!(
            client().get(&url, 32, Duration::from_secs(1)).await,
            Err(PollError {
                kind: PollErrorKind::Transport,
                ..
            })
        ));
        task.await.unwrap();
    }
}

#[tokio::test]
async fn reports_unsuccessful_status_without_consuming_body() {
    let (url, task) =
        serve_response(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 100\r\n\r\n".to_vec())
            .await;
    assert!(matches!(
        client().get(&url, 4, Duration::from_secs(1)).await,
        Err(PollError {
            kind: PollErrorKind::Status(StatusCode::SERVICE_UNAVAILABLE),
            ..
        })
    ));
    task.await.unwrap();
}

#[tokio::test]
async fn limits_response_headers() {
    let mut response = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n".to_vec();
    for index in 0..101 {
        response.extend_from_slice(format!("x-{index}: v\r\n").as_bytes());
    }
    response.extend_from_slice(b"\r\n");
    let (url, task) = serve_response(response).await;
    assert!(matches!(
        client().get(&url, 32, Duration::from_secs(1)).await,
        Err(PollError {
            kind: PollErrorKind::Transport,
            ..
        })
    ));
    task.await.unwrap();
}

#[tokio::test]
async fn deadline_covers_stalled_headers_and_dripping_body() {
    for body_started in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/status", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            if body_started {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nx")
                    .await
                    .unwrap();
                // Each byte arrives inside a hypothetical per-read timeout; only
                // the total deadline ends this otherwise continuously active body.
                for _ in 0..20 {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    if stream.write_all(b"x").await.is_err() {
                        break;
                    }
                }
            }
            let mut byte = [0];
            let read = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte))
                .await
                .unwrap();
            assert!(
                matches!(read, Ok(0) | Err(_)),
                "deadline must release socket"
            );
        });
        assert!(matches!(
            client().get(&url, 128, Duration::from_millis(100)).await,
            Err(PollError {
                kind: PollErrorKind::Deadline,
                ..
            })
        ));
        task.await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_request_releases_stalled_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/status", listener.local_addr().unwrap());
    let (started_tx, started_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        started_tx.send(()).unwrap();
        let mut byte = [0];
        let read = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(read, Ok(0) | Err(_)), "cancel must release socket");
    });
    let client = client();
    let request_client = client.clone();
    let request =
        tokio::spawn(async move { request_client.get(&url, 32, Duration::from_secs(30)).await });
    started_rx.await.unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    // Keeping another client clone alive must not retain the cancelled request.
    server.await.unwrap();
    drop(client);
}

#[tokio::test]
async fn follows_at_most_five_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/status", listener.local_addr().unwrap());
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let server = tokio::spawn(async move {
        for _ in 0..6 {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            count.fetch_add(1, Ordering::Relaxed);
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /status\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
        }
    });
    let error = client()
        .get(&url, 32, Duration::from_secs(1))
        .await
        .err()
        .unwrap();
    assert!(
        error
            .source
            .as_ref()
            .is_some_and(reqwest::Error::is_redirect)
    );
    server.await.unwrap();
    assert_eq!(requests.load(Ordering::Relaxed), 6);
}

#[tokio::test]
async fn reuses_connections_after_consuming_bounded_bodies() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/status", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for _ in 0..2 {
            tokio::time::timeout(Duration::from_secs(1), read_request(&mut stream))
                .await
                .unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                .await
                .unwrap();
        }
    });
    let client = client();
    for _ in 0..2 {
        assert_eq!(
            client
                .get(&url, 32, Duration::from_secs(1))
                .await
                .unwrap()
                .body,
            b"{}"
        );
    }
    server.await.unwrap();
}

#[tokio::test]
async fn forwards_http_requests_to_configured_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        assert!(request.starts_with(b"GET http://monitor.invalid/status HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .await
            .unwrap();
    });
    let client = PollingClient {
        client: PollingClient::builder()
            .no_proxy()
            .proxy(reqwest::Proxy::http(proxy_url).unwrap())
            .build()
            .unwrap(),
    };
    assert_eq!(
        client
            .get("http://monitor.invalid/status", 32, Duration::from_secs(1))
            .await
            .unwrap()
            .body,
        b"{}"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn redirects_do_not_send_original_query_in_referer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/status?token=private-value",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: /destination\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        let request = read_request(&mut stream).await;
        let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /destination http/1.1\r\n"));
        assert!(!request.contains("referer:"));
        assert!(!request.contains("private-value"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .await
            .unwrap();
    });
    assert_eq!(
        client()
            .get(&url, 32, Duration::from_secs(1))
            .await
            .unwrap()
            .body,
        b"{}"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn https_verifies_the_server_certificate() {
    use tokio_rustls::{TlsAcceptor, rustls};

    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()]).unwrap();
    let root = reqwest::Certificate::from_der(cert.der()).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("https://{}/status", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        assert!(
            acceptor.accept(stream).await.is_err(),
            "untrusted certificate must abort TLS"
        );
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(stream).await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
            assert!(request.len() <= 8192);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .await
            .unwrap();
        stream.shutdown().await.unwrap();
    });
    assert!(matches!(
        client().get(&url, 32, Duration::from_secs(2)).await,
        Err(PollError {
            kind: PollErrorKind::Transport,
            ..
        })
    ));
    let trusted = PollingClient {
        client: PollingClient::builder()
            .no_proxy()
            .add_root_certificate(root)
            .build()
            .unwrap(),
    };
    assert_eq!(
        trusted
            .get(&url, 32, Duration::from_secs(2))
            .await
            .unwrap()
            .body,
        b"{}"
    );
    server.await.unwrap();
}

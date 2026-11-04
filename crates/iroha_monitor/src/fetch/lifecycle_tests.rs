//! Fetcher cancellation and concurrent polling regressions against actual sockets.

use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

async fn request_headers(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(stream.read_u8().await.unwrap());
        assert!(bytes.len() < 8192);
    }
    String::from_utf8(bytes).unwrap()
}

#[tokio::test]
async fn polls_status_and_metrics_concurrently() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut connections = Vec::new();
        // Neither endpoint responds until both requests have arrived.
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = request_headers(&mut stream).await;
            connections.push((stream, request));
        }
        for (mut stream, request) in connections {
            let body = if request.starts_with("GET /status ") {
                "{\"blocks\":7}"
            } else {
                "block_gas_used 42\n"
            };
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    let client = PollingClient::new().unwrap();
    let update = tokio::time::timeout(Duration::from_secs(1), fetch_once(&client, 3, &endpoint))
        .await
        .unwrap();
    assert_eq!(update.index, 3);
    assert_eq!(update.snapshot.status.unwrap().blocks, Some(7));
    assert_eq!(update.snapshot.metrics.gas_used, Some(42));
    assert!(update.snapshot.notices.is_empty());
    server.await.unwrap();
}

#[tokio::test]
async fn dropping_fetcher_cancels_both_stalled_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (started_tx, started_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut connections = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            request_headers(&mut stream).await;
            connections.push(stream);
        }
        started_tx.send(()).unwrap();
        for mut stream in connections {
            let mut byte = [0];
            let read = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut byte))
                .await
                .unwrap();
            assert!(
                matches!(read, Ok(0) | Err(_)),
                "fetcher drop releases requests"
            );
        }
    });
    let fetcher = PeerFetcher::new(vec![endpoint], Duration::from_secs(60)).unwrap();
    tokio::time::timeout(Duration::from_secs(1), started_rx)
        .await
        .unwrap()
        .unwrap();
    drop(fetcher);
    server.await.unwrap();
}

#[tokio::test]
async fn http_failures_keep_endpoint_specific_notice_kinds() {
    for (status, metrics, expected_status, expected_metrics) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 131073\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 524289\r\n\r\n",
            NoticeKind::StatusPayloadTooLarge,
            NoticeKind::MetricsPayloadTooLarge,
        ),
        (
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
            NoticeKind::StatusFetchFailed,
            NoticeKind::MetricsFetchFailed,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nbad",
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nbad",
            NoticeKind::StatusParseFailed,
            NoticeKind::MetricsParseFailed,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = request_headers(&mut stream).await;
                let response = if request.starts_with("GET /status ") {
                    status
                } else {
                    metrics
                };
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let result = fetch_once(&PollingClient::new().unwrap(), 0, &endpoint).await;
        assert_eq!(
            result
                .snapshot
                .notices
                .iter()
                .map(|notice| notice.kind)
                .collect::<Vec<_>>(),
            [expected_status, expected_metrics]
        );
        assert!(result.snapshot.status.is_none());
        assert_eq!(result.snapshot.metrics, MetricsSnapshot::default());
        server.await.unwrap();
    }
}

//! Opt-in loopback benchmark of the shipping polling client, without a baseline dependency.
//!
//! Run from the repository root on an otherwise idle machine:
//! ```text
//! scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_monitor --bin iroha_monitor fetch::http::benchmark::loopback_polling -- --exact --ignored --nocapture
//! ```
//! Each sample performs 25 polls per peer against separate loopback listeners. A poll
//! concurrently requests status and metrics; each response has 5 ms of server delay.
//! Five samples report elapsed time, p50/p95 poll latency, and exact request/connection
//! counts. Client construction is outside the timer. Timing is diagnostic, not a CI
//! threshold; compare identical builds and machine load when measuring changes.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

use super::super::{
    METRICS_BODY_LIMIT, METRICS_HTTP_TIMEOUT, STATUS_BODY_LIMIT, STATUS_HTTP_TIMEOUT,
};
use super::{Duration, Instant, PollingClient};

struct LoopbackPeers {
    endpoints: Vec<String>,
    listeners: Vec<JoinHandle<()>>,
    connections: Arc<AtomicUsize>,
    requests: Arc<AtomicUsize>,
}

impl Drop for LoopbackPeers {
    fn drop(&mut self) {
        for listener in &self.listeners {
            // Dropping each listener's JoinSet also aborts its connection tasks.
            listener.abort();
        }
    }
}

async fn serve_connection(stream: TcpStream, requests: Arc<AtomicUsize>) {
    let mut stream = BufReader::new(stream);
    loop {
        let mut line = String::new();
        if stream.read_line(&mut line).await.unwrap_or(0) == 0 {
            return;
        }
        let status = line.starts_with("GET /status ");
        loop {
            line.clear();
            if stream.read_line(&mut line).await.unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
        }
        requests.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(5)).await;
        let body = if status {
            "{\"alias\":\"test\",\"blocks\":123,\"txs_approved\":456}"
        } else {
            "block_gas_used 42\nblock_fee_total_units 7\n"
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{body}",
            body.len()
        );
        if stream
            .get_mut()
            .write_all(response.as_bytes())
            .await
            .is_err()
        {
            return;
        }
    }
}

async fn start_peers(count: usize) -> LoopbackPeers {
    let mut peers = LoopbackPeers {
        endpoints: Vec::with_capacity(count),
        listeners: Vec::with_capacity(count),
        connections: Arc::new(AtomicUsize::new(0)),
        requests: Arc::new(AtomicUsize::new(0)),
    };
    for _ in 0..count {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        peers
            .endpoints
            .push(format!("http://{}", listener.local_addr().unwrap()));
        let connections = peers.connections.clone();
        let requests = peers.requests.clone();
        peers.listeners.push(tokio::spawn(async move {
            let mut handlers = JoinSet::new();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                connections.fetch_add(1, Ordering::Relaxed);
                handlers.spawn(serve_connection(stream, requests.clone()));
            }
        }));
    }
    peers
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "opt-in loopback performance measurement; excludes ordinary test gates"]
async fn loopback_polling() {
    const ROUNDS: usize = 25;
    for peer_count in [1, 4, 16] {
        for sample in 1..=5 {
            let peers = start_peers(peer_count).await;
            let client = PollingClient {
                client: PollingClient::builder().no_proxy().build().unwrap(),
            };
            let mut latencies = Vec::with_capacity(ROUNDS * peer_count);
            let start = Instant::now();
            for _ in 0..ROUNDS {
                let mut polls = JoinSet::new();
                for endpoint in &peers.endpoints {
                    let endpoint = endpoint.clone();
                    let client = client.clone();
                    polls.spawn(async move {
                        let start = Instant::now();
                        let status = format!("{endpoint}/status");
                        let metrics = format!("{endpoint}/metrics");
                        let (status, metrics) = tokio::join!(
                            client.get(&status, STATUS_BODY_LIMIT, STATUS_HTTP_TIMEOUT),
                            client.get(&metrics, METRICS_BODY_LIMIT, METRICS_HTTP_TIMEOUT),
                        );
                        assert!(!status.unwrap().truncated);
                        assert!(!metrics.unwrap().truncated);
                        start.elapsed().as_micros()
                    });
                }
                while let Some(result) = polls.join_next().await {
                    latencies.push(result.unwrap());
                }
            }
            let elapsed = start.elapsed();
            latencies.sort_unstable();
            let connections = peers.connections.load(Ordering::Relaxed);
            let requests = peers.requests.load(Ordering::Relaxed);
            assert_eq!(requests, ROUNDS * peer_count * 2);
            assert_eq!(
                connections,
                peer_count * 2,
                "completed bodies reuse each peer's two connections"
            );
            println!(
                "sample={sample} peers={peer_count} polls={} elapsed_us={} p50_us={} p95_us={} connections={connections} requests={requests}",
                ROUNDS * peer_count,
                elapsed.as_micros(),
                latencies[latencies.len() / 2],
                latencies[latencies.len() * 95 / 100],
            );
        }
    }
}

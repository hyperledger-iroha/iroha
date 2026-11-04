//! SSE smoke test: verify that `/v1/events/sse` streams trigger and data events.
use eyre::{Result, WrapErr as _, eyre};
use integration_tests::sandbox;
use iroha::data_model::prelude::*;
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::ALICE_ID;
use norito::json::{self, Value as JsonValue};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// One deadline covers connection, headers and every event in a scenario.
const SSE_TIMEOUT: Duration = Duration::from_secs(300);
const SSE_MAX_FRAME_BYTES: usize = 1024 * 1024;

enum SseMessage {
    Ready,
    Event(JsonValue),
    Failed(eyre::Report),
}

struct SseReader {
    rx: Receiver<SseMessage>,
    deadline: Instant,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl SseReader {
    fn receive(&self) -> Result<SseMessage> {
        let now = Instant::now();
        if now >= self.deadline {
            return Err(eyre!("SSE scenario deadline elapsed"));
        }
        let remaining = self.deadline.saturating_duration_since(now);
        match self.rx.recv_timeout(remaining) {
            Ok(SseMessage::Failed(error)) => Err(error),
            Ok(message) => Ok(message),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(eyre!("SSE scenario deadline elapsed")),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(eyre!("SSE reader stopped before the expected event"))
            }
        }
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| eyre!("SSE reader worker panicked"))?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        self.stop()
    }
}

impl Drop for SseReader {
    fn drop(&mut self) {
        // Cancellation drops the HTTP stream and joins this reader's exact worker,
        // including when a scenario returns early or unwinds.
        let _ = self.stop();
    }
}

fn parse_sse_data_line(line: &str) -> Option<&str> {
    let line = line.trim_end_matches(['\r', '\n']);
    let rest = line.strip_prefix("data:")?;
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    if rest.is_empty() { None } else { Some(rest) }
}

#[derive(Default)]
struct SseDecoder {
    line: Vec<u8>,
    data: String,
    event: String,
}

impl SseDecoder {
    fn push(&mut self, bytes: &[u8], tx: &Sender<SseMessage>) -> Result<()> {
        for &byte in bytes {
            if self.line.len() + self.data.len() + self.event.len() >= SSE_MAX_FRAME_BYTES {
                return Err(eyre!("SSE frame exceeds the fixture byte limit"));
            }
            if byte != b'\n' {
                self.line.push(byte);
                continue;
            }
            let line = std::str::from_utf8(&self.line).wrap_err("SSE line is not UTF-8")?;
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                if !self.data.is_empty() {
                    let value = json::from_str(self.data.trim_end_matches('\n'))
                        .wrap_err("SSE event contains malformed JSON")?;
                    if self.event == "stream_error" {
                        return Err(eyre!("SSE server refused the stream: {value:?}"));
                    }
                    tx.send(SseMessage::Event(value))
                        .map_err(|_| eyre!("SSE receiver was dropped"))?;
                }
                self.data.clear();
                self.event.clear();
            } else if let Some(data) = parse_sse_data_line(line) {
                self.data.push_str(data);
                self.data.push('\n');
            } else if let Some(event) = line.strip_prefix("event:") {
                self.event = event.trim_start_matches(' ').to_owned();
            }
            self.line.clear();
        }
        Ok(())
    }
}

fn wait_for_sse_ready(reader: &SseReader) -> Result<()> {
    match reader.receive()? {
        SseMessage::Ready => Ok(()),
        _ => Err(eyre!("SSE reader emitted an event before HTTP admission")),
    }
}
#[test]
fn sse_smoke_scenarios() -> Result<()> {
    use iroha::data_model::events::time::{ExecutionTime, TimeEventFilter};
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_min_peers(4),
        stringify!(sse_smoke_scenarios),
    )?
    else {
        return Ok(());
    };
    let peer = network.peer();
    let client = peer.client();
    // Scenario 1: Execute trigger event + data event.
    {
        let rx = spawn_sse_reader(client.client().clone(), SSE_TIMEOUT)?;
        wait_for_sse_ready(&rx)?;
        let trigger_id: TriggerId = "sse_smoke_trigger_exec".parse()?;
        let asset_id = AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "rose".parse()?,
            ),
            ALICE_ID.clone(),
        );
        let asset_id_literal = asset_id.canonical_literal();
        let register = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![InstructionBox::from(Mint::asset_quantity(1_u32, asset_id))],
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(ALICE_ID.clone()),
            )
            .expect("trigger action fixture satisfies validation invariants"),
        ));
        if sandbox::handle_result(
            client.submit(
                register,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            ),
            stringify!(sse_emits_execute_trigger_event),
        )?
        .is_none()
        {
            return Ok(());
        }
        if sandbox::handle_result(
            client.submit(
                ExecuteTrigger::new(trigger_id),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            ),
            stringify!(sse_emits_execute_trigger_event),
        )?
        .is_none()
        {
            return Ok(());
        }
        let result: Result<()> = (|| {
            let exec_evt = wait_for_sse(&rx, |val| summary_contains(val, "ExecuteTriggerEvent"))?;
            assert_eq!(exec_evt["category"].as_str(), Some("Other"));
            let data_evt = wait_for_sse(&rx, |val| {
                val["category"].as_str() == Some("Data")
                    && summary_contains(val, "Asset(Added")
                    && summary_contains(val, &asset_id_literal)
            })?;
            assert_eq!(data_evt["category"].as_str(), Some("Data"));
            Ok(())
        })();
        if sandbox::handle_result(result, stringify!(sse_emits_execute_trigger_event))?.is_none() {
            return Ok(());
        }
        rx.finish()?;
    }
    // Scenario 2: Time trigger + metadata event.
    {
        let rx = spawn_sse_reader(client.client().clone(), SSE_TIMEOUT)?;
        wait_for_sse_ready(&rx)?;
        let key: Name = "sse_tick".parse()?;
        let time_trigger = Trigger::new(
            "sse_time_trigger_event".parse()?,
            Action::new(
                vec![InstructionBox::from(SetKeyValue::account(
                    ALICE_ID.clone(),
                    key.clone(),
                    Json::from(norito::json!("tick")),
                ))],
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                TimeEventFilter::new(ExecutionTime::PreCommit),
            )
            .expect("trigger action fixture satisfies validation invariants"),
        );
        if sandbox::handle_result(
            client.submit(
                Register::trigger(time_trigger),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            ),
            stringify!(sse_captures_time_trigger_and_metadata_events),
        )?
        .is_none()
        {
            return Ok(());
        }
        if sandbox::handle_result(
            client.submit(
                Log::new(Level::INFO, "trigger tick".to_string()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            ),
            stringify!(sse_captures_time_trigger_and_metadata_events),
        )?
        .is_none()
        {
            return Ok(());
        }
        let result: Result<()> = (|| {
            let time_evt = wait_for_sse(&rx, |val| summary_contains(val, "TimeEvent"))?;
            assert_eq!(time_evt["category"].as_str(), Some("Other"));
            let meta_evt = wait_for_sse(&rx, |val| {
                val["category"].as_str() == Some("Data")
                    && summary_contains(val, "MetadataInserted")
                    && summary_contains(val, key.as_ref())
            })?;
            assert_eq!(meta_evt["category"].as_str(), Some("Data"));
            Ok(())
        })();
        if sandbox::handle_result(
            result,
            stringify!(sse_captures_time_trigger_and_metadata_events),
        )?
        .is_none()
        {
            return Ok(());
        }
        rx.finish()?;
    }
    Ok(())
}
fn spawn_sse_reader(client: iroha::client::Client, timeout: Duration) -> Result<SseReader> {
    let deadline = Instant::now() + timeout;
    let (tx, rx) = mpsc::channel();
    let (shutdown, cancelled) = tokio::sync::oneshot::channel();
    let worker = std::thread::Builder::new()
        .name("iroha-integration-sse".to_owned())
        .spawn(move || {
            let result: Result<()> = (|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async {
                    tokio::select! {
                        biased;
                        _ = cancelled => Ok(()),
                        result = tokio::time::timeout_at(
                            tokio::time::Instant::from_std(deadline),
                            read_sse(&client, &tx),
                        ) => result.map_err(|_| eyre!("SSE scenario deadline elapsed"))?,
                    }
                })
            })();
            if let Err(error) = result {
                let _ = tx.send(SseMessage::Failed(error));
            }
        })?;
    Ok(SseReader {
        rx,
        deadline,
        shutdown: Some(shutdown),
        worker: Some(worker),
    })
}

fn signed_sse_request(
    http: &reqwest::Client,
    client: &iroha::client::Client,
) -> Result<reqwest::Request> {
    use iroha::client::{
        canonical_network_request_signature_message, canonical_request_account_header_value,
        canonical_request_signature_header_value, canonical_request_timestamp_header_value,
    };
    static NONCE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let url = client.endpoint().join("v1/events/sse")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let nonce = format!(
        "sse-{}-{timestamp}-{}",
        std::process::id(),
        NONCE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let message = canonical_network_request_signature_message(
        client.network_id(),
        &iroha::http::Method::GET,
        &url,
        &[],
        timestamp,
        &nonce,
    )?;
    let signature = iroha_crypto::Signature::try_new(client.key_pair().private_key(), &message)?;
    let mut request = http.get(url);
    for (name, value) in client.headers() {
        if !matches!(
            name.to_ascii_lowercase().as_str(),
            "accept"
                | "x-iroha-account"
                | "x-iroha-signature"
                | "x-iroha-timestamp-ms"
                | "x-iroha-nonce"
                | "x-iroha-witness"
        ) {
            request = request.header(name.as_str(), value.as_str());
        }
    }
    Ok(request
        .header("Accept", "text/event-stream")
        .header(
            "x-iroha-account",
            canonical_request_account_header_value(client.account())?,
        )
        .header(
            "x-iroha-signature",
            canonical_request_signature_header_value(&signature)?,
        )
        .header(
            "x-iroha-timestamp-ms",
            canonical_request_timestamp_header_value(timestamp)?,
        )
        .header("x-iroha-nonce", nonce)
        .build()?)
}

async fn read_sse(client: &iroha::client::Client, tx: &Sender<SseMessage>) -> Result<()> {
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut response = http.execute(signed_sse_request(&http, client)?).await?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(eyre!("SSE request refused with HTTP {}", response.status()));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next());
    if !content_type.is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream")) {
        return Err(eyre!(
            "SSE response is missing text/event-stream Content-Type"
        ));
    }
    tx.send(SseMessage::Ready)
        .map_err(|_| eyre!("SSE receiver was dropped"))?;
    let mut decoder = SseDecoder::default();
    while let Some(chunk) = response.chunk().await? {
        decoder.push(&chunk, tx)?;
    }
    Err(eyre!(
        "SSE stream closed before cancellation; live streams cannot replay missing events"
    ))
}

fn wait_for_sse<F>(reader: &SseReader, predicate: F) -> Result<JsonValue>
where
    F: Fn(&JsonValue) -> bool,
{
    loop {
        if let SseMessage::Event(value) = reader.receive()?
            && predicate(&value)
        {
            return Ok(value);
        }
    }
}
fn summary_contains(val: &JsonValue, needle: &str) -> bool {
    val.get("summary")
        .and_then(JsonValue::as_str)
        .is_some_and(|s| s.contains(needle))
}

#[test]
fn parse_sse_data_line_accepts_optional_space() {
    assert_eq!(
        parse_sse_data_line("data: {\"ok\":true}\r\n"),
        Some("{\"ok\":true}")
    );
    assert_eq!(
        parse_sse_data_line("data:{\"ok\":true}\n"),
        Some("{\"ok\":true}")
    );
    assert_eq!(parse_sse_data_line("data:\r\n"), None);
    assert_eq!(parse_sse_data_line(": comment\n"), None);
}

#[test]
fn sse_decoder_handles_split_frames_and_refuses_bad_or_terminal_payloads() -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut decoder = SseDecoder::default();
    for chunk in [
        b": keepalive\r\ndata: {\r\nda".as_slice(),
        b"ta: \"ok\": true\r\ndata: }\r\n\r\ndata:{\"noop\":true}\n\n".as_slice(),
    ] {
        decoder.push(chunk, &tx)?;
    }
    for field in ["ok", "noop"] {
        let SseMessage::Event(value) = rx.try_recv()? else {
            panic!("expected an event")
        };
        assert_eq!(value[field].as_bool(), Some(true));
    }
    assert!(rx.try_recv().is_err());
    assert!(
        SseDecoder::default()
            .push(b"data: {broken}\n\n", &tx)
            .is_err()
    );
    assert!(SseDecoder::default().push(b"data: \xff\n\n", &tx).is_err());
    let error = SseDecoder::default()
        .push(
            b"event: stream_error\ndata: {\"code\":\"stream_authorization_revoked\"}\n\n",
            &tx,
        )
        .unwrap_err();
    assert!(error.to_string().contains("stream_authorization_revoked"));
    assert!(
        SseDecoder::default()
            .push(&vec![b'x'; SSE_MAX_FRAME_BYTES + 1], &tx)
            .is_err()
    );
    Ok(())
}

fn fixture_client(endpoint: reqwest::Url) -> iroha::client::Client {
    let config = iroha::config::Config {
        chain: "sse-fixture".into(),
        network_id: iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x91; 32])),
        ),
        key_pair: iroha_test_samples::ALICE_KEYPAIR.clone(),
        account: ALICE_ID.clone(),
        account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
        http_transport_policy: iroha::config::HttpTransportPolicy::Ordinary,
        torii_api_url: endpoint,
        torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        basic_auth: None,
        api_token: None,
        transaction_add_nonce: false,
        transaction_ttl: Duration::from_secs(5),
        transaction_status_timeout: Duration::from_secs(10),
        sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
        sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::default(),
        sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::default(),
    };
    iroha::client::Client::builder(config)
        .build()
        .expect("valid SSE fixture configuration")
}

#[test]
fn sse_request_signs_exact_network_target_and_freshness() -> Result<()> {
    use base64::Engine as _;
    let client = fixture_client("http://127.0.0.1:8080/".parse()?);
    let http = reqwest::Client::new();
    let request = signed_sse_request(&http, &client)?;
    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(request.url().path(), "/v1/events/sse");
    assert_eq!(request.url().query(), None);
    assert_eq!(request.headers()["accept"], "text/event-stream");
    assert_eq!(
        request.headers()["x-iroha-account"],
        iroha::client::canonical_request_account_header_value(client.account())?
    );
    let timestamp = request.headers()["x-iroha-timestamp-ms"]
        .to_str()?
        .parse()?;
    let nonce = request.headers()["x-iroha-nonce"].to_str()?;
    let signature = iroha_crypto::Signature::try_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(request.headers()["x-iroha-signature"].to_str()?)?,
    )?;
    let message = iroha::client::canonical_network_request_signature_message(
        client.network_id(),
        &iroha::http::Method::GET,
        request.url(),
        &[],
        timestamp,
        nonce,
    )?;
    signature.verify(client.key_pair().public_key(), &message)?;
    let changed_target = client.endpoint().join("v1/events")?;
    let wrong_message = iroha::client::canonical_network_request_signature_message(
        client.network_id(),
        &iroha::http::Method::GET,
        &changed_target,
        &[],
        timestamp,
        nonce,
    )?;
    assert!(
        signature
            .verify(client.key_pair().public_key(), &wrong_message)
            .is_err()
    );
    let second = signed_sse_request(&http, &client)?;
    assert_ne!(
        request.headers()["x-iroha-nonce"],
        second.headers()["x-iroha-nonce"]
    );
    Ok(())
}

fn serve_sse_response(
    response: &'static str,
    wait_for_close: bool,
) -> Result<(iroha::client::Client, JoinHandle<Result<String>>)> {
    use std::{
        io::{BufRead as _, BufReader, Read as _, Write as _},
        net::TcpListener,
    };
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let client = fixture_client(format!("http://{}/", listener.local_addr()?).parse()?);
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Err(eyre!("SSE request ended before headers"));
            }
            request.push_str(&line);
            if request.len() > 16 * 1024 {
                return Err(eyre!("SSE request headers too large"));
            }
            if line == "\r\n" {
                break;
            }
        }
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        if wait_for_close {
            let mut byte = [0_u8];
            assert_eq!(
                reader.read(&mut byte)?,
                0,
                "reader cancellation closes the HTTP stream"
            );
        }
        Ok(request)
    });
    Ok((client, server))
}

#[test]
fn sse_reader_connects_and_reads_data_lines() -> Result<()> {
    let (client, server) = serve_sse_response(
        concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n",
            "14\r\ndata:{\"noop\":true}\n\n\r\n",
            "13\r\ndata: {\"ok\":true}\n\n\r\n",
        ),
        true,
    )?;
    let reader = spawn_sse_reader(client, Duration::from_secs(10))?;
    wait_for_sse_ready(&reader)?;
    for field in ["noop", "ok"] {
        let payload = wait_for_sse(&reader, |value| value[field].as_bool() == Some(true))?;
        assert_eq!(payload[field].as_bool(), Some(true));
    }
    reader.finish()?;
    let request = server
        .join()
        .map_err(|_| eyre!("SSE fixture server panicked"))??;
    assert!(request.starts_with("GET /v1/events/sse HTTP/1.1\r\n"));
    for header in [
        "x-iroha-account:",
        "x-iroha-signature:",
        "x-iroha-timestamp-ms:",
        "x-iroha-nonce:",
    ] {
        assert!(request.to_ascii_lowercase().contains(header));
    }
    Ok(())
}

#[test]
fn sse_reader_refuses_http_errors_and_invalid_media_before_ready() -> Result<()> {
    for (response, expected) in [
        (
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
            "HTTP 401",
        ),
        (
            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
            "HTTP 403",
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
            "Content-Type",
        ),
    ] {
        let (client, server) = serve_sse_response(response, false)?;
        let reader = spawn_sse_reader(client, Duration::from_secs(10))?;
        let error = wait_for_sse_ready(&reader).unwrap_err();
        assert!(error.to_string().contains(expected), "{error:?}");
        reader.finish()?;
        server
            .join()
            .map_err(|_| eyre!("SSE fixture server panicked"))??;
    }
    Ok(())
}

#[test]
fn sse_reader_honors_original_deadline_and_joins_on_drop() -> Result<()> {
    const RESPONSE: &str =
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";
    let (client, server) = serve_sse_response(RESPONSE, true)?;
    let reader = spawn_sse_reader(client, Duration::from_secs(2))?;
    wait_for_sse_ready(&reader)?;
    let original_deadline = reader.deadline;
    let error = wait_for_sse(&reader, |_| true).unwrap_err();
    assert!(error.to_string().contains("deadline"), "{error:?}");
    assert_eq!(reader.deadline, original_deadline);
    drop(reader);
    server
        .join()
        .map_err(|_| eyre!("SSE fixture server panicked"))??;

    let (client, server) = serve_sse_response(RESPONSE, true)?;
    let reader = spawn_sse_reader(client, Duration::from_secs(10))?;
    wait_for_sse_ready(&reader)?;
    drop(reader);
    server
        .join()
        .map_err(|_| eyre!("SSE fixture server panicked"))??;
    Ok(())
}

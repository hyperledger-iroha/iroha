//! Bounded credential-free native parent-frame retrieval regressions.

use super::*;
use iroha::http::{HttpTransport, StatusCode, TransportFuture, TransportRequest};
use iroha_data_model::sumeragi_finality::{
    SumeragiFinalityProof, test_fixtures::NativeFinalityFixture,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Debug)]
struct Transport {
    status: StatusCode,
    genesis: Vec<u8>,
    successor: Vec<u8>,
    requests: Mutex<Vec<TransportRequest>>,
}

impl HttpTransport for Transport {
    fn send_blocking(
        &self,
        request: TransportRequest,
    ) -> color_eyre::eyre::Result<iroha::http::Response<Vec<u8>>> {
        let bytes = match request.url.path() {
            "/v1/bridge/finality/1" => self.genesis.clone(),
            "/v1/bridge/finality/2" => self.successor.clone(),
            _ => panic!("unexpected native parent route"),
        };
        self.requests.lock().unwrap().push(request);
        let mut response = iroha::http::Response::new(bytes);
        *response.status_mut() = self.status;
        response
            .headers_mut()
            .insert("content-type", "application/x-norito".parse().unwrap());
        Ok(response)
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn transport(
    status: StatusCode,
    genesis: Vec<u8>,
    successor: Vec<u8>,
) -> (CheckpointTransport, Arc<Transport>) {
    let raw = Arc::new(Transport {
        status,
        genesis,
        successor,
        requests: Mutex::new(Vec::new()),
    });
    (
        CheckpointTransport::with_client(PublicHttpClient::with_transport(raw.clone())),
        raw,
    )
}

fn encoded(proof: &SumeragiFinalityProof) -> Vec<u8> {
    norito::encode_canonical(proof).unwrap()
}

#[test]
fn amx_reader_fetches_only_exact_native_genesis_and_successor_without_credentials() {
    let _ = CheckpointTransport::new();
    let fixture = NativeFinalityFixture::new();
    let checkpoint = fixture.checkpoint();
    let bootstrap = crate::bootstrap::tests::authenticated(
        crate::bootstrap::tests::metadata(&checkpoint),
        &checkpoint,
    );
    let (reader, raw) = transport(
        StatusCode::OK,
        encoded(fixture.genesis_proof()),
        encoded(fixture.latest()),
    );
    let (genesis, successor) = reader.fetch_amx_pair(&bootstrap, deadline()).unwrap();
    assert_eq!(genesis, fixture.genesis_proof().block_wire);
    assert_eq!(successor, fixture.latest().block_wire);
    let requests = raw.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(
            request.url.as_str(),
            format!("https://torii.example/v1/bridge/finality/{}", index + 1)
        );
        assert_eq!(request.method, iroha::http::Method::GET);
        assert!(
            !request
                .headers
                .iter()
                .any(|(name, _)| name == "authorization")
        );
        assert!(request.body.is_empty());
        assert_eq!(request.max_response_bytes, MAX_PUBLIC_READ_BYTES);
        assert!(
            request
                .timeout
                .is_some_and(|value| value <= Duration::from_secs(5))
        );
    }
}

#[test]
fn amx_reader_rejects_wrong_height_malformed_frames_and_http_refusal() {
    let fixture = NativeFinalityFixture::new();
    let checkpoint = fixture.checkpoint();
    let bootstrap = crate::bootstrap::tests::authenticated(
        crate::bootstrap::tests::metadata(&checkpoint),
        &checkpoint,
    );
    let mut tampered = fixture.genesis_proof().clone();
    tampered.block_wire[0] ^= 1;
    for genesis in [
        encoded(fixture.latest()),
        b"not a canonical proof".to_vec(),
        encoded(&tampered),
    ] {
        let (reader, raw) = transport(StatusCode::OK, genesis, encoded(fixture.latest()));
        assert!(reader.fetch_amx_pair(&bootstrap, deadline()).is_err());
        assert_eq!(raw.requests.lock().unwrap().len(), 1);
    }
    let (reader, raw) = transport(
        StatusCode::NOT_FOUND,
        encoded(fixture.genesis_proof()),
        encoded(fixture.latest()),
    );
    assert!(matches!(
        reader.fetch_amx_pair(&bootstrap, deadline()),
        Err(CheckpointReadError::Http(PublicHttpError::Status(404)))
    ));
    assert_eq!(raw.requests.lock().unwrap().len(), 1);
    let (reader, raw) = transport(
        StatusCode::OK,
        encoded(fixture.genesis_proof()),
        encoded(fixture.genesis_proof()),
    );
    assert!(reader.fetch_amx_pair(&bootstrap, deadline()).is_err());
    assert_eq!(raw.requests.lock().unwrap().len(), 2);
}

#[test]
fn amx_reader_preserves_the_original_deadline_before_dispatch() {
    let fixture = NativeFinalityFixture::new();
    let checkpoint = fixture.checkpoint();
    let bootstrap = crate::bootstrap::tests::authenticated(
        crate::bootstrap::tests::metadata(&checkpoint),
        &checkpoint,
    );
    let (reader, raw) = transport(
        StatusCode::OK,
        encoded(fixture.genesis_proof()),
        encoded(fixture.latest()),
    );
    assert!(matches!(
        reader.fetch_amx_pair(&bootstrap, Instant::now()),
        Err(CheckpointReadError::Http(PublicHttpError::Deadline))
    ));
    assert!(raw.requests.lock().unwrap().is_empty());
}

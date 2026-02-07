#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Shared fixtures used across Torii integration/unit tests.
//!
//! Some helpers are gated by telemetry and may be unused when those tests are
//! disabled; allow the definitions to stay available across feature sets.

use std::sync::{Arc, LazyLock, Mutex};

use axum::{body::Body, http::Request};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_core::state::World;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::peer::PeerId;
use iroha_telemetry::metrics::Metrics;
use iroha_test_samples::ALICE_ID;
use rand::RngCore as _;

static SHARED_METRICS: LazyLock<Mutex<Arc<Metrics>>> =
    LazyLock::new(|| Mutex::new(Arc::new(Metrics::default())));

/// Canonical literals for a single well-known account used in tx query tests.
#[allow(dead_code)]
pub struct AccountLiterals {
    /// IH58 canonical literal.
    pub canonical: String,
    /// Compressed literal.
    pub compressed: String,
    /// Raw public-key literal with domain suffix.
    pub raw_public_key: String,
}

/// Singleton fixture so all tx query tests share the same literals.
#[allow(dead_code)]
pub static TX_QUERY_ACCOUNT: LazyLock<AccountLiterals> = LazyLock::new(|| {
    let account = ALICE_ID.clone();
    let domain_label = account.domain().to_string();
    let compressed = account
        .to_account_address()
        .and_then(|addr| addr.to_compressed_sora())
        .expect("compressed literal should encode");
    let raw_public_key = format!("{}@{}", account.signatory(), domain_label);
    AccountLiterals {
        canonical: account.to_string(),
        compressed,
        raw_public_key,
    }
});

/// Ensure duplicate metric registrations panic inside tests so suites do not silently reuse registries.
#[allow(dead_code)]
pub fn enable_duplicate_metric_panic() {
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("IROHA_METRICS_PANIC_ON_DUPLICATE", "1");
    }
}

/// Shared metrics registry for tests to avoid duplicate Prometheus descriptor warnings.
#[allow(dead_code)]
pub fn shared_metrics() -> Arc<Metrics> {
    enable_duplicate_metric_panic();
    SHARED_METRICS
        .lock()
        .expect("shared metrics mutex poisoned")
        .clone()
}

/// Reset the shared metrics registry to a fresh instance for suites that need a clean slate.
#[allow(dead_code)]
pub fn reset_shared_metrics() -> Arc<Metrics> {
    enable_duplicate_metric_panic();
    let mut guard = SHARED_METRICS
        .lock()
        .expect("shared metrics mutex poisoned");
    let metrics = Arc::new(Metrics::default());
    *guard = metrics.clone();
    metrics
}

/// Seed the world with the given peer IDs using the test-only mutator.
#[allow(dead_code)]
pub fn seed_peers<I>(world: &mut World, peer_ids: I)
where
    I: IntoIterator<Item = PeerId>,
{
    let mut world_block = world.block();
    let peers = world_block.peers_mut_for_testing().get_mut();
    for peer_id in peer_ids {
        let _ = peers.push(peer_id);
    }
    world_block.commit();
}

/// Seed the world with a single peer ID.
#[allow(dead_code)]
pub fn seed_peer(world: &mut World, peer_id: PeerId) {
    seed_peers(world, [peer_id]);
}

/// Attach operator signature headers to a request targeting operator-only endpoints.
///
/// Operator endpoints are internet-reachable by design but must be authenticated with a
/// request signature bound to (method, path, query, body, timestamp, nonce).
#[allow(dead_code)]
pub fn operator_signed_request(
    key_pair: &KeyPair,
    mut request: Request<Body>,
    body_bytes: &[u8],
) -> Request<Body> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);

    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);

    let mut msg =
        iroha_torii::canonical_request_message(request.method(), request.uri(), body_bytes);
    msg.extend_from_slice(b"\n");
    msg.extend_from_slice(ts_ms.to_string().as_bytes());
    msg.extend_from_slice(b"\n");
    msg.extend_from_slice(nonce.as_bytes());

    let signature = Signature::new(key_pair.private_key(), &msg);

    let headers = request.headers_mut();
    headers.insert(
        "x-iroha-operator-public-key",
        key_pair
            .public_key()
            .to_string()
            .parse()
            .expect("operator public key header"),
    );
    headers.insert(
        "x-iroha-operator-timestamp-ms",
        ts_ms
            .to_string()
            .parse()
            .expect("operator timestamp header"),
    );
    headers.insert(
        "x-iroha-operator-nonce",
        nonce.parse().expect("operator nonce header"),
    );
    headers.insert(
        "x-iroha-operator-signature",
        BASE64_STANDARD
            .encode(signature.payload())
            .parse()
            .expect("operator signature header"),
    );

    request
}

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Shared fixtures used across Torii integration/unit tests.
//!
//! Some helpers are gated by telemetry and may be unused when those tests are
//! disabled; allow the definitions to stay available across feature sets.

use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicU64, Ordering},
};

use axum::{body::Body, http::Request};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_core::state::World;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{account::AccountId, peer::PeerId};
use iroha_telemetry::metrics::Metrics;
use iroha_test_samples::ALICE_ID;

static SHARED_METRICS: LazyLock<Mutex<Arc<Metrics>>> =
    LazyLock::new(|| Mutex::new(Arc::new(Metrics::default())));
static OPERATOR_NONCE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Canonical literals for a single well-known account used in tx query tests.
#[allow(dead_code)]
pub struct AccountLiterals {
    /// I105 canonical literal.
    pub canonical: String,
    /// Compressed literal.
    pub compressed: String,
}

/// Singleton fixture so all tx query tests share the same literals.
#[allow(dead_code)]
pub static TX_QUERY_ACCOUNT: LazyLock<AccountLiterals> = LazyLock::new(|| {
    let account = ALICE_ID.clone();
    let compressed = account
        .to_account_address()
        .and_then(|addr| addr.to_i105())
        .expect("compressed literal should encode");
    AccountLiterals {
        canonical: account.to_string(),
        compressed,
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

    let nonce_counter = OPERATOR_NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut nonce_bytes = [0_u8; 12];
    nonce_bytes[..8].copy_from_slice(&nonce_counter.to_le_bytes());
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);

    let mut msg =
        iroha_torii::canonical_request_message(request.method(), request.uri(), body_bytes);
    msg.extend_from_slice(b"\n");
    msg.extend_from_slice(ts_ms.to_string().as_bytes());
    msg.extend_from_slice(b"\n");
    msg.extend_from_slice(nonce.as_bytes());

    let signature =
        Signature::try_new(key_pair.private_key(), &msg).expect("operator request signature");

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

/// Attach app-canonical signature headers to a request targeting app-authenticated endpoints.
#[allow(dead_code)]
pub fn app_signed_request(
    account_id: &AccountId,
    key_pair: &KeyPair,
    mut request: Request<Body>,
    body_bytes: &[u8],
) -> Request<Body> {
    use std::{
        sync::LazyLock,
        time::{SystemTime, UNIX_EPOCH},
    };

    static APP_NONCE_SEQ: LazyLock<std::sync::atomic::AtomicU64> =
        LazyLock::new(|| std::sync::atomic::AtomicU64::new(0));

    let ts_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);

    let nonce_seq = APP_NONCE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = format!(
        "itest-{ts_ms}-{nonce_seq}-{}-{}",
        request.method().as_str(),
        request.uri().path()
    );
    let msg = iroha_torii::canonical_request_signature_message(
        request.method(),
        request.uri(),
        body_bytes,
        ts_ms,
        &nonce,
    );
    let signature =
        Signature::try_new(key_pair.private_key(), &msg).expect("app canonical request signature");

    let headers = request.headers_mut();
    headers.insert(
        iroha_torii::HEADER_ACCOUNT,
        account_id.to_string().parse().expect("account header"),
    );
    headers.insert(
        iroha_torii::HEADER_SIGNATURE,
        iroha_torii::signature_header_value(&signature)
            .parse()
            .expect("app signature header"),
    );
    headers.insert(
        iroha_torii::HEADER_TIMESTAMP_MS,
        ts_ms.to_string().parse().expect("timestamp header"),
    );
    headers.insert(
        iroha_torii::HEADER_NONCE,
        nonce.parse().expect("nonce header"),
    );

    request
}

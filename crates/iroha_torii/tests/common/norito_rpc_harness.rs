//! Shared harness for Norito-RPC ingress/capability tests to avoid ad-hoc runtimes.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::connect_info::ConnectInfo,
    http::{Request, header::CONTENT_TYPE},
};
use iroha_config::parameters::actual::{Queue as QueueConfig, Root as ActualRoot};
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::Log,
    query::{QueryRequest, SingularQueryBox, runtime::prelude::FindAbiVersion},
    transaction::{
        SignedTransaction, TransactionBuilder, TransactionEntrypoint, signed::TransactionSignature,
    },
};
use iroha_logger::Level;
use iroha_torii::{OnlinePeersProvider, Torii};
use iroha_torii_shared::uri;
use iroha_version::codec::EncodeVersioned;
use tower::ServiceExt as _;

#[allow(dead_code)]
const NORITO_MIME: &str = "application/x-norito";

fn checked_norito_rpc_ed25519_key_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked Norito RPC Ed25519 fixture keypair")
}

#[test]
fn norito_rpc_fixture_uses_checked_ed25519_key_generation() {
    let key_pair = checked_norito_rpc_ed25519_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture Norito RPC public key has a valid algorithm");

    assert_eq!(algorithm, Algorithm::Ed25519);
}

/// Return a loopback peer address for Axum handlers that require `ConnectInfo`.
#[must_use]
pub fn loopback_connect_info() -> ConnectInfo<std::net::SocketAddr> {
    ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
}

/// Shared Norito-RPC harness used by integration tests to avoid ad-hoc runtimes.
pub struct NoritoRpcHarness {
    /// Application router ready for HTTP testing.
    pub app: axum::Router,
    #[allow(dead_code)]
    /// Effective configuration used to initialise the harness.
    pub cfg: ActualRoot,
}

impl NoritoRpcHarness {
    /// Construct a harness from a preconfigured `ActualRoot`.
    pub fn with_config(cfg: ActualRoot) -> Self {
        // Use the lightweight Kiso mock to avoid spinning up the full actor in tests.
        let kiso = KisoHandle::mock(&cfg);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));
        let queue_cfg = QueueConfig::default();
        let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
        let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
        let _ = peers_tx;

        let torii = Torii::new_with_handle(
            cfg.common.chain.clone(),
            kiso,
            cfg.torii.clone(),
            queue.clone(),
            tokio::sync::broadcast::channel(1).0,
            LiveQueryStore::start_test(),
            kura,
            state,
            cfg.common.key_pair.clone(),
            OnlinePeersProvider::new(peers_rx),
            None,
            iroha_torii::MaybeTelemetry::disabled(),
        );

        Self {
            app: torii.api_router_for_tests(),
            cfg,
        }
    }

    /// Construct a harness by applying a small configuration delta to the minimal defaults.
    pub fn new<F>(mut configure: F) -> Self
    where
        F: FnMut(&mut ActualRoot),
    {
        let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
        configure(&mut cfg);
        Self::with_config(cfg)
    }

    /// Post a Norito-encoded transaction to the RPC endpoint.
    #[allow(dead_code)]
    pub async fn post_transaction(
        &self,
        set_content_type: bool,
        extra_headers: &[(&str, &str)],
    ) -> axum::response::Response {
        let mut builder = Request::builder().method("POST").uri(uri::TRANSACTION);

        if set_content_type {
            builder = builder.header(CONTENT_TYPE, NORITO_MIME);
        }
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }

        let mut req = builder
            .body(Body::from(sample_transaction_bytes()))
            .expect("request");
        req.extensions_mut().insert(loopback_connect_info());

        self.app.clone().oneshot(req).await.expect("response")
    }
}

/// Construct a signed transaction payload suitable for Norito-RPC ingress tests.
#[allow(dead_code)]
pub fn sample_signed_transaction() -> SignedTransaction {
    let chain_id: ChainId = ChainId::from("test-chain");
    let key_pair = checked_norito_rpc_ed25519_key_fixture();
    let account = AccountId::of(key_pair.public_key().clone());
    TransactionBuilder::new(chain_id, account)
        .with_instructions([Log::new(Level::INFO, "norito-rpc test".to_owned())])
        .sign(key_pair.private_key())
}

/// Construct a versioned external transaction payload suitable for public `/v1/pipeline/transactions` tests.
#[allow(dead_code)]
pub fn sample_transaction_bytes() -> Vec<u8> {
    sample_signed_transaction().encode_versioned()
}

/// Construct a bare signed-transaction payload used to assert legacy ingress rejection.
#[allow(dead_code)]
pub fn sample_bare_transaction_bytes() -> Vec<u8> {
    norito::to_bytes(&sample_signed_transaction()).expect("encode bare signed transaction")
}

/// Construct a versioned external transaction with a valid wire shape but invalid signature.
#[allow(dead_code)]
pub fn sample_invalid_signature_transaction_bytes() -> Vec<u8> {
    let mut tx = sample_signed_transaction();
    let mut signature = tx.signature().0.payload().to_vec();
    let last = signature
        .last_mut()
        .expect("sample transaction signature payload is non-empty");
    *last ^= 0xFF;
    tx.set_signature(TransactionSignature(SignatureOf::from_signature(
        Signature::from_bytes(&signature),
    )));
    tx.encode_versioned()
}

/// Construct a versioned internal entrypoint payload for negative public-ingress tests.
#[allow(dead_code)]
pub fn sample_transaction_entrypoint_bytes() -> Vec<u8> {
    TransactionEntrypoint::External(sample_signed_transaction()).encode_versioned()
}

/// Construct a signed query payload suitable for public `/v1/query` tests.
#[allow(dead_code)]
pub fn sample_signed_query() -> iroha_data_model::query::SignedQuery {
    let key_pair = checked_norito_rpc_ed25519_key_fixture();
    let account = AccountId::of(key_pair.public_key().clone());
    QueryRequest::Singular(SingularQueryBox::FindAbiVersion(FindAbiVersion))
        .with_authority(account)
        .sign(&key_pair)
}

/// Construct a versioned signed-query payload suitable for public `/v1/query` tests.
#[allow(dead_code)]
pub fn sample_query_bytes() -> Vec<u8> {
    sample_signed_query().encode_versioned()
}

/// Construct a versioned signed query with a valid wire shape but invalid signature.
#[allow(dead_code)]
pub fn sample_invalid_signature_query_bytes() -> Vec<u8> {
    let signed = sample_signed_query();
    let mut signature = signed.signature.0.payload().to_vec();
    let last = signature
        .last_mut()
        .expect("sample query signature payload is non-empty");
    *last ^= 0xFF;

    iroha_data_model::query::SignedQuery {
        signature: iroha_data_model::query::QuerySignature(SignatureOf::from_signature(
            Signature::from_bytes(&signature),
        )),
        payload: signed.payload,
    }
    .encode_versioned()
}

/// Construct a bare signed-query payload used to assert legacy ingress rejection.
#[allow(dead_code)]
pub fn sample_bare_query_bytes() -> Vec<u8> {
    norito::to_bytes(&sample_signed_query()).expect("encode bare signed query")
}

#[test]
fn sample_signed_query_roundtrips_as_a_versioned_singular_request() {
    use iroha_version::codec::DecodeVersioned as _;

    let signed = sample_signed_query();
    assert!(matches!(
        signed.request(),
        QueryRequest::Singular(SingularQueryBox::FindAbiVersion(_))
    ));

    let bytes = signed.encode_versioned();
    let decoded = iroha_data_model::query::SignedQuery::decode_all_versioned(&bytes)
        .expect("decode versioned signed query");

    assert!(matches!(
        decoded.request(),
        QueryRequest::Singular(SingularQueryBox::FindAbiVersion(_))
    ));
    assert_eq!(decoded.authority(), signed.authority());
    assert_eq!(decoded.encode_versioned(), bytes);
}

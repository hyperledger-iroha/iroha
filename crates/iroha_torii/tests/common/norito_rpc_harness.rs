//! Shared harness for Norito-RPC ingress/capability tests to avoid ad-hoc runtimes.
use axum::{
    body::Body,
    extract::connect_info::ConnectInfo,
    http::{Request, header::CONTENT_TYPE},
};
use iroha_config::parameters::actual::{Queue as QueueConfig, Root as ActualRoot};
use iroha_core::{
    kiso::KisoHandle,
    query::store::LiveQueryStore,
    queue::Queue,
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    NetworkId, Registrable,
    account::{Account, AccountId},
    isi::Log,
    query::{QueryRequest, SignedQuery, SingularQueryBox, runtime::prelude::FindAbiVersion},
    transaction::{
        SignedTransaction, TransactionBuilder, TransactionEntrypoint, signed::TransactionSignature,
    },
};
use iroha_logger::Level;
use iroha_torii::{OnlinePeersProvider, Torii, test_utils::TestDataDirGuard};
use iroha_version::codec::EncodeVersioned;
use std::{
    num::{NonZeroU32, NonZeroU64},
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
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
///
/// The configuration and chain are boxed so futures that own the harness stay small.
pub struct NoritoRpcHarness {
    /// Application router ready for HTTP testing.
    pub app: iroha_torii::TestApiRouterRuntime,
    #[allow(dead_code)]
    /// Effective configuration used to initialise the harness.
    pub cfg: Box<ActualRoot>,
    /// Exact genesis-lineage identity shared by the state and Torii admission.
    pub network_id: NetworkId,
    /// Signing key of the account the original genesis registers for sample requests.
    pub client_key_pair: KeyPair,
    /// Original signed genesis whose native execution gives the router its root scope.
    chain: Box<CertifiedTestChain>,
    /// Exclusive temporary Torii data directory, released after the router and chain.
    data_dir: TestDataDirGuard,
}
impl NoritoRpcHarness {
    /// Construct a harness from a configuration rooted under `data_dir`.
    ///
    /// Torii serves the State of an original signed genesis executed natively, as a node does
    /// after startup; the configured genesis identity is rebound to that chain.
    fn with_config(mut cfg: ActualRoot, data_dir: TestDataDirGuard) -> Self {
        let client_key_pair = checked_norito_rpc_ed25519_key_fixture();
        let client_id = AccountId::of(client_key_pair.public_key().clone());
        let world = World::with([], [Account::new(client_id.clone()).build(&client_id)], []);
        let mut chain_config = TestChainConfig::new(world, unix_now_ms());
        chain_config.chain_id = cfg.common.chain.clone();
        cfg.genesis.public_key = chain_config.genesis_key.public_key().clone();
        let chain =
            Box::new(CertifiedTestChain::start(chain_config).expect("original signed genesis"));
        assert_eq!(
            chain.state().authenticated_root_scope(),
            Some(chain.root_scope()),
            "native genesis execution establishes the listener root scope"
        );
        cfg.genesis.expected_hash = chain.genesis().hash();
        let network_id = chain.network_id();
        // Torii ingress executes locally only for a member of the route's signed committee.
        let local_peer_id = chain.validators()[0].0.clone();
        // Use the lightweight Kiso mock to avoid spinning up the full actor in tests.
        let kiso = KisoHandle::mock(&cfg);
        let queue_cfg = QueueConfig::default();
        let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
        let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
        let _ = peers_tx;
        let torii = Torii::new_with_handle(
            cfg.common.chain.clone(),
            network_id,
            kiso,
            cfg.torii.clone(),
            queue.clone(),
            tokio::sync::broadcast::channel(1).0,
            LiveQueryStore::start_test(),
            chain.kura().clone(),
            chain.state().clone(),
            cfg.common.key_pair.clone(),
            OnlinePeersProvider::new(peers_rx),
            None,
            iroha_torii::test_utils::with_private_settlement_test_signers(
                iroha_torii::ToriiRuntimeDeps::new(
                    build_identity_test_fixture::build_identity(),
                    iroha_torii::MaybeTelemetry::disabled(),
                ),
                &iroha_torii::test_utils::private_settlement_test_node_key(),
            ),
        )
        .expect("valid Torii Norito-RPC fixture")
        .with_local_peer_id(local_peer_id);
        Self {
            app: torii
                .api_router_for_tests()
                .expect("test Torii router initializes"),
            cfg: Box::new(cfg),
            network_id,
            client_key_pair,
            chain,
            data_dir,
        }
    }
    /// Construct a harness by applying a small configuration delta to the minimal defaults.
    ///
    /// Durable Torii state resolves under the harness's own data directory, never the process
    /// working directory shared by concurrent harnesses and test processes.
    pub fn new<F>(mut configure: F) -> Self
    where
        F: FnMut(&mut ActualRoot),
    {
        let data_dir = TestDataDirGuard::new();
        let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
        configure(&mut cfg);
        Self::with_config(cfg, data_dir)
    }
    /// Signal shutdown and join every worker retained by the test router.
    pub async fn shutdown(self) {
        let Self {
            app,
            chain,
            data_dir,
            ..
        } = self;
        app.shutdown().await;
        drop(chain);
        drop(data_dir);
    }
    /// Authenticate an operator request with the node key against the network this harness runs.
    #[allow(dead_code)]
    pub fn operator_signed_request(
        &self,
        mut request: Request<Body>,
        body_bytes: &[u8],
    ) -> Request<Body> {
        let headers = iroha_torii::operator_signed_request_headers(
            &self.cfg.common.key_pair,
            &self.network_id,
            request.method(),
            request.uri(),
            body_bytes,
        )
        .expect("operator signature bound to the harness genesis");
        request.headers_mut().extend(headers);
        request
    }
    /// Post a Norito-encoded transaction to the RPC endpoint.
    #[allow(dead_code)]
    pub async fn post_transaction(
        &self,
        set_content_type: bool,
        extra_headers: &[(&str, &str)],
    ) -> axum::response::Response {
        let mut builder = Request::builder()
            .method("POST")
            .uri(iroha_torii_shared::route_catalog::pipeline::TRANSACTION.path());
        if set_content_type {
            builder = builder.header(CONTENT_TYPE, NORITO_MIME);
        }
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }
        let mut req = builder
            .body(Body::from(self.sample_transaction_bytes()))
            .expect("request");
        req.extensions_mut().insert(loopback_connect_info());
        self.app.clone().oneshot(req).await.expect("response")
    }
    fn sample_signed_transaction(&self) -> SignedTransaction {
        signed_transaction(self.network_id, &self.client_key_pair)
    }
    /// Construct a versioned external transaction payload suitable for public `/v1/pipeline/transactions` tests.
    #[allow(dead_code)]
    pub fn sample_transaction_bytes(&self) -> Vec<u8> {
        self.sample_signed_transaction().encode_versioned()
    }
    /// Construct a bare signed-transaction payload used to assert legacy ingress rejection.
    #[allow(dead_code)]
    pub fn sample_bare_transaction_bytes(&self) -> Vec<u8> {
        norito::to_bytes(&self.sample_signed_transaction()).expect("encode bare signed transaction")
    }
    /// Construct a versioned external transaction with a valid wire shape but invalid signature.
    #[allow(dead_code)]
    pub fn sample_invalid_signature_transaction_bytes(&self) -> Vec<u8> {
        let mut tx = self.sample_signed_transaction();
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
    pub fn sample_transaction_entrypoint_bytes(&self) -> Vec<u8> {
        TransactionEntrypoint::External(self.sample_signed_transaction()).encode_versioned()
    }
    fn sample_signed_query(&self) -> SignedQuery {
        signed_query(self.network_id, &self.client_key_pair)
    }
    /// Construct a versioned signed-query payload suitable for public `/v1/query` tests.
    #[allow(dead_code)]
    pub fn sample_query_bytes(&self) -> Vec<u8> {
        self.sample_signed_query().encode_versioned()
    }
    /// Construct a versioned signed query with a valid wire shape but invalid signature.
    #[allow(dead_code)]
    pub fn sample_invalid_signature_query_bytes(&self) -> Vec<u8> {
        let signed = self.sample_signed_query();
        let mut signature = signed.signature.0.payload().to_vec();
        let last = signature
            .last_mut()
            .expect("sample query signature payload is non-empty");
        *last ^= 0xFF;
        SignedQuery {
            signature: iroha_data_model::query::QuerySignature(SignatureOf::from_signature(
                Signature::from_bytes(&signature),
            )),
            payload: signed.payload,
        }
        .encode_versioned()
    }
    /// Construct a bare signed-query payload used to assert legacy ingress rejection.
    #[allow(dead_code)]
    pub fn sample_bare_query_bytes(&self) -> Vec<u8> {
        norito::to_bytes(&self.sample_signed_query()).expect("encode bare signed query")
    }
}
fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock follows Unix epoch")
        .as_millis()
        .try_into()
        .expect("Unix time in milliseconds fits u64")
}
/// Construct a `Log` transaction signed by `key_pair` for `network_id`.
fn signed_transaction(network_id: NetworkId, key_pair: &KeyPair) -> SignedTransaction {
    // Samples stay distinct when one client signs several within a millisecond.
    static NEXT_NONCE: AtomicU32 = AtomicU32::new(1);
    let mut builder = TransactionBuilder::new(
        network_id,
        AccountId::of(key_pair.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "norito-rpc test".to_owned())]);
    builder.set_nonce(
        NonZeroU32::new(NEXT_NONCE.fetch_add(1, Ordering::Relaxed))
            .expect("nonzero transaction nonce"),
    );
    builder.sign(key_pair.private_key())
}
/// Construct a `FindAbiVersion` query signed by `key_pair` for `network_id`.
fn signed_query(network_id: NetworkId, key_pair: &KeyPair) -> SignedQuery {
    static NEXT_NONCE: AtomicU64 = AtomicU64::new(1);
    let mut nonce = [0_u8; 32];
    nonce[24..].copy_from_slice(&NEXT_NONCE.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    QueryRequest::Singular(SingularQueryBox::FindAbiVersion(FindAbiVersion))
        .with_authority(
            network_id,
            AccountId::of(key_pair.public_key().clone()),
            unix_now_ms(),
            NonZeroU64::new(100_000).expect("nonzero query TTL"),
            nonce,
        )
        .sign(key_pair)
}
#[test]
fn sample_signed_query_roundtrips_as_a_versioned_singular_request() {
    use iroha_version::codec::DecodeVersioned as _;
    let signed = signed_query(
        iroha_torii::test_utils::signed_query_network_id(),
        &checked_norito_rpc_ed25519_key_fixture(),
    );
    assert!(matches!(
        signed.request(),
        QueryRequest::Singular(SingularQueryBox::FindAbiVersion(_))
    ));
    let bytes = signed.encode_versioned();
    let decoded = SignedQuery::decode_all_versioned(&bytes).expect("decode versioned signed query");
    assert!(matches!(
        decoded.request(),
        QueryRequest::Singular(SingularQueryBox::FindAbiVersion(_))
    ));
    assert_eq!(decoded.authority(), signed.authority());
    assert_eq!(decoded.encode_versioned(), bytes);
}

#[path = "../../src/build_identity_test_fixture.rs"]
mod build_identity_test_fixture;

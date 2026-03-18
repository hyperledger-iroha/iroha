//! Shared harness for Norito-RPC ingress/capability tests to avoid ad-hoc runtimes.

use std::sync::Arc;

use axum::{
    body::Body,
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
use iroha_crypto::KeyPair;
use iroha_data_model::{ChainId, account::AccountId, isi::Log, transaction::TransactionBuilder};
use iroha_logger::Level;
use iroha_torii::{OnlinePeersProvider, Torii};
use iroha_torii_shared::uri;
use iroha_version::codec::EncodeVersioned;
use tower::ServiceExt as _;

#[allow(dead_code)]
const NORITO_MIME: &str = "application/x-norito";

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

        let req = builder
            .body(Body::from(sample_transaction_bytes()))
            .expect("request");

        self.app.clone().oneshot(req).await.expect("response")
    }
}

/// Construct a signed transaction payload suitable for Norito-RPC ingress tests.
#[allow(dead_code)]
pub fn sample_transaction_bytes() -> Vec<u8> {
    let chain_id: ChainId = ChainId::from("test-chain");
    let key_pair = KeyPair::random();
    let account = AccountId::of(key_pair.public_key().clone());
    TransactionBuilder::new(chain_id, account)
        .with_instructions([Log::new(Level::INFO, "norito-rpc test".to_owned())])
        .sign(key_pair.private_key())
        .encode_versioned()
}

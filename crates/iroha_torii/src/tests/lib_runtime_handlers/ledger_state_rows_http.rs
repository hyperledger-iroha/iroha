// Production signed POST and independently verified reads under a single certified tip.
struct PoolProofFixture {
    chain: iroha_core::sumeragi::test_chain::CertifiedTestChain,
    fixture: RuntimeApiRouterFixture,
    asset: iroha_data_model::asset::AssetDefinitionId,
}
fn pool_proof_fixture(keyed: bool) -> PoolProofFixture {
    use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::{
        asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition},
        isi::privacy::BootstrapPrivacyIronwoodPoolV1,
        privacy::{PrivacyIronwoodPoolBootstrapV1, PrivacyPoolIdV1},
    };
    use iroha_executor_data_model::permission::query::CanReadAllLedgerData;
    let payer = iroha_test_samples::ALICE_ID.clone();
    let reserve = iroha_test_samples::BOB_ID.clone();
    let asset = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("ironwood", "universal").unwrap(),
        "http-cut".parse().unwrap(),
    );
    let mut world = World::with(
        [iroha_data_model::domain::Domain::new(
            iroha_model_base::domain::DomainId::try_new("ironwood", "universal").unwrap(),
        )
        .build(&payer)],
        [
            Account::new(payer.clone()).build(&payer),
            Account::new(reserve.clone()).build(&payer),
        ],
        [
            AssetDefinition::numeric(asset.clone(), "HTTP cut", AssetBalancePolicy::Global, None)
                .build(&payer),
        ],
    );
    iroha_core::runtime::ironwood::install_for_test(&mut world);
    if keyed {
        iroha_core::runtime::keyed_state::install_for_test(&mut world);
    }
    let mut config = TestChainConfig::new(world, 1_000);
    config.chain_id = ChainId::from("ironwood-http-cut");
    config.genesis_instructions = vec![
        iroha_data_model::isi::Grant::account_permission(CanReadAllLedgerData, payer.clone())
            .into(),
        BootstrapPrivacyIronwoodPoolV1::new(
            PrivacyIronwoodPoolBootstrapV1::new(
                PrivacyPoolIdV1::new([0x71; 32]),
                asset.clone(),
                AssetBalanceScope::Global,
                reserve,
            )
            .unwrap(),
        )
        .into(),
    ];
    let mut chain = CertifiedTestChain::start(config).unwrap();
    chain.commit(Vec::new());
    let fixture = RuntimeApiRouterFixture::with_runtime(
        "ironwood-http-cut",
        Arc::clone(chain.kura()),
        Arc::clone(chain.state()),
        ToriiRuntimeDeps::new(
            crate::build_identity_test_fixture::build_identity(),
            routing::MaybeTelemetry::disabled(),
        ),
    );
    PoolProofFixture {
        chain,
        fixture,
        asset,
    }
}

impl PoolProofFixture {
    fn verified(&self) -> iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock {
        use iroha_data_model::sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier};
        let validators = self
            .chain
            .validators()
            .iter()
            .map(|(peer, pop)| FinalityValidator {
                public_key: peer.public_key().clone(),
                proof_of_possession: pop.clone(),
            })
            .collect();
        let mut verifier =
            SumeragiFinalityVerifier::new(self.chain.genesis(), "ironwood-http-cut", validators)
                .unwrap();
        let verify = |height| {
            iroha_core::sumeragi::finality::build_proof(&self.chain.state().view(), height).unwrap()
        };
        verifier.verify(&verify(1)).unwrap();
        verifier.verify(&verify(2)).unwrap()
    }
    async fn post(&self, path: &str, body: String, expected: StatusCode) -> axum::body::Bytes {
        let uri: axum::http::Uri = path.parse().unwrap();
        let headers = signed_network_app_headers(
            &self.chain.network_id(),
            &iroha_test_samples::ALICE_ID,
            &iroha_test_samples::ALICE_KEYPAIR,
            &HttpMethod::POST,
            &uri,
            body.as_bytes(),
        );
        let mut request = Request::builder()
            .method(HttpMethod::POST)
            .uri(uri)
            .header(axum::http::header::ACCEPT, "application/x-norito")
            .body(Body::from(body))
            .unwrap();
        request.headers_mut().extend(headers);
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        let response = self.fixture.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        if status == StatusCode::OK {
            assert_eq!(
                response.headers()[axum::http::header::CACHE_CONTROL],
                "private, no-store"
            );
        }
        let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(status, expected, "{}", String::from_utf8_lossy(&bytes));
        bytes
    }
    async fn rows(
        &self,
        keys: &[iroha_data_model::sumeragi_finality::keyed_state::StateKeyV1],
    ) -> (iroha_data_model::sumeragi_finality::KeyedStateRowsV1, usize) {
        let body = Self::request(keys);
        let bytes = self
            .post("/v1/ledger/state-proof/2", body, StatusCode::OK)
            .await;
        (
            norito::decode_canonical_for_admission(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .unwrap(),
            bytes.len(),
        )
    }
    fn request(keys: &[iroha_data_model::sumeragi_finality::keyed_state::StateKeyV1]) -> String {
        let rows = keys.iter().map(|key| norito::json!({"table":(key.table.clone()),"key_hex":(hex::encode(&key.key))})).collect::<Vec<_>>();
        norito::json::to_json(&norito::json!({"rows":rows})).unwrap()
    }
    async fn payment_rows(
        &self,
        tip: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
        schema: &iroha_core_privacy::ironwood_state_rows::IronwoodStateSchemaV1,
    ) -> usize {
        use iroha_core_privacy::ironwood_state_rows::{IronwoodPoolContextV1, ironwood_pool_rows};
        let pool = iroha_data_model::privacy::PrivacyPoolIdV1::new([0x71; 32]);
        let (first, first_bytes) = self.rows(&ironwood_pool_rows(pool).unwrap()).await;
        let context = IronwoodPoolContextV1::verify(
            tip,
            schema,
            pool,
            &self.asset,
            &iroha_test_samples::ALICE_ID,
            &first,
        )
        .unwrap();
        let (remaining, rest_bytes) = self.rows(&context.remaining_rows().unwrap()).await;
        let (payer, reserve) = context.verify_balances(&remaining).unwrap();
        assert!(payer.is_none() && reserve.is_none());
        first_bytes + rest_bytes
    }
    fn expected_schema(&self) -> iroha_core_privacy::ironwood_state_rows::IronwoodStateSchemaV1 {
        use iroha_core::state::WorldReadOnly;
        let schema = iroha_data_model::sumeragi_finality::SelectedStateSchemaV1::from_selection(
            self.chain.state().view().world().runtime_selection(),
            &iroha_core::runtime::release_catalog().unwrap().definitions,
        )
        .unwrap();
        let digest = schema.digest();
        iroha_core_privacy::ironwood_state_rows::IronwoodStateSchemaV1::from_selected(
            schema, digest,
        )
        .unwrap()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ledger_state_rows_verifies_ironwood_against_a_certified_tip_and_refuses_a_stale_height() {
    let mut f = pool_proof_fixture(true);
    let tip = f.verified();
    let schema = f.expected_schema();
    assert!(f.payment_rows(&tip, &schema).await > 0);
    f.chain.commit(Vec::new());
    use iroha_core_privacy::ironwood_state_rows::ironwood_pool_rows;
    let keys =
        ironwood_pool_rows(iroha_data_model::privacy::PrivacyPoolIdV1::new([0x71; 32])).unwrap();
    f.post(
        "/v1/ledger/state-proof/2",
        PoolProofFixture::request(&keys),
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    f.fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ledger_state_rows_does_not_prove_from_an_uncertified_local_tree_on_an_lthash_chain() {
    let f = pool_proof_fixture(false);
    let body = r#"{"rows":[{"table":"world.assets","key_hex":"00"}]}"#.to_owned();
    f.post(
        "/v1/ledger/state-proof/2",
        body,
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    f.fixture.shutdown().await;
}

/// Compare the two signed requests, transport bytes, decoding and application verification
/// against the measured full-World reference on this same fixture. One warmup, seven samples.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "performance measurement on an otherwise idle host"]
async fn measure_ironwood_payment_context_http() {
    use std::time::Instant;
    let fixture = pool_proof_fixture(true);
    let tip = fixture.verified();
    let schema = fixture.expected_schema();
    let mut elapsed_ms = Vec::new();
    for iteration in 0..8 {
        let start = Instant::now();
        let bytes = fixture.payment_rows(&tip, &schema).await;
        let ms = start.elapsed().as_secs_f64() * 1000.;
        if iteration != 0 {
            elapsed_ms.push(ms);
        }
        println!("sample {iteration}: {ms:.3} ms, {bytes} bytes");
    }
    elapsed_ms.sort_by(f64::total_cmp);
    println!("median selected rows {:.3} ms", elapsed_ms[3]);
    fixture.fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ledger_state_rows_http_authenticates_once_and_rejects_replayed_nonce() {
    let PoolProofFixture { chain, fixture, .. } = pool_proof_fixture(true);
    let payer = iroha_test_samples::ALICE_ID.clone();
    let keys = iroha_core_privacy::ironwood_state_rows::ironwood_pool_rows(
        iroha_data_model::privacy::PrivacyPoolIdV1::new([0x71; 32]),
    )
    .unwrap();
    let body = PoolProofFixture::request(&keys);
    let uri: axum::http::Uri = "/v1/ledger/state-proof/2".parse().unwrap();
    let headers = signed_network_app_headers(
        &chain.network_id(),
        &payer,
        &iroha_test_samples::ALICE_KEYPAIR,
        &HttpMethod::POST,
        &uri,
        body.as_bytes(),
    );
    for expected in [StatusCode::OK, StatusCode::UNAUTHORIZED] {
        let mut request = Request::builder()
            .method(HttpMethod::POST)
            .uri(uri.clone())
            .header(axum::http::header::ACCEPT, "application/x-norito")
            .body(Body::from(body.clone()))
            .unwrap();
        request.headers_mut().extend(headers.clone());
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        let response = fixture.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(status, expected, "{}", String::from_utf8_lossy(&bytes));
    }
    fixture.shutdown().await;
}

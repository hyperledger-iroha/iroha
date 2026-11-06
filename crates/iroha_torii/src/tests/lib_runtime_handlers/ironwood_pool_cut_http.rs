// Ironwood cut authentication through the production HTTP router and middleware stack.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ironwood_pool_cut_http_authenticates_once_and_rejects_replayed_nonce() {
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
    let mut config = TestChainConfig::new(world, 1_000);
    config.chain_id = ChainId::from("ironwood-http-cut");
    config.genesis_instructions = vec![
        iroha_data_model::isi::Grant::account_permission(CanReadAllLedgerData, payer.clone())
            .into(),
        BootstrapPrivacyIronwoodPoolV1::new(
            PrivacyIronwoodPoolBootstrapV1::new(
                PrivacyPoolIdV1::new([0x71; 32]),
                asset,
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
    let body = norito::json::to_json(&norito::json!({"pool_id_hex": (hex::encode([0x71;32])), "payer_account_id": (payer.to_string())})).unwrap();
    let uri: axum::http::Uri = "/v1/privacy/ironwood/pool-cut/2".parse().unwrap();
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
        if status == StatusCode::OK {
            let proof: iroha_core_privacy::ironwood_pool_cut::IronwoodPoolCutProofV1 =
                norito::decode_canonical_for_admission(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
                .unwrap();
            assert!(proof.payer_balance.is_none());
            assert!(proof.reserve_balance.is_none());
        }
    }
    fixture.shutdown().await;
}

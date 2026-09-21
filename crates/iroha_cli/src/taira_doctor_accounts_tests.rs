mod doctor_account_tests {
    //! Wallet discovery is part of both scoped doctor contracts, using SDK semantics.
    use super::*;
    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"doctor wallet discovery fixture",
        )))
    }
    pub(super) fn capability_response() -> MockResponse {
        let capabilities =
            iroha_torii_shared::account_capabilities::AccountCapabilitiesV1::from_admission(
                network(),
                DEFAULT_CHAIN_DISCRIMINANT,
                &[Algorithm::Ed25519],
            )
            .unwrap();
        MockResponse::json(200, json::to_value(&capabilities).unwrap())
    }
    pub(super) fn faucet_response() -> MockResponse {
        let _profile = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
        let policy = iroha_torii_shared::account_faucet_policy::AccountFaucetAdvertisement {
            schema_version: 1,
            network_id: network(),
            network_prefix: DEFAULT_CHAIN_DISCRIMINANT,
            authority: iroha_test_samples::ALICE_ID.clone(),
            asset_definition_id: DEFAULT_GAS_ASSET_ID.parse().unwrap(),
            amount: Quantity::from(100_u32),
        };
        MockResponse::json(200, json::to_value(&policy).unwrap())
    }
    #[test]
    fn doctor_wallet_discovery_is_unsigned_and_mandatory_in_both_scopes() {
        for scope in [DoctorScope::Basic, DoctorScope::Full] {
            let server = spawn_mock_http(16, |request| doctor_mock_response(request, None));
            let report = run_doctor(&server.base_url, scope).unwrap();
            let requests = finish_mock(server);
            assert_eq!(report_status(&report), Some("ok"));
            for path in ["/v1/accounts/capabilities", "/v1/accounts/faucet/policy"] {
                let request = requests
                    .iter()
                    .find(|request| path_only(&request.path) == path)
                    .unwrap();
                assert_eq!(request.method, "GET");
                for header in [
                    "authorization",
                    "x-iroha-account",
                    "x-iroha-signature",
                    "x-iroha-onboarding-token",
                ] {
                    assert!(request.header_values(header).is_empty());
                }
            }
            for name in ["account_capabilities", "account_faucet_policy"] {
                assert!(
                    doctor_expected_checks(scope)
                        .iter()
                        .any(|check| check.0 == name)
                );
                assert!(
                    report["checks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|check| check["name"].as_str() == Some(name)
                            && check["ok"].as_bool() == Some(true))
                );
            }
        }
    }
    #[test]
    fn doctor_missing_faucet_policy_is_a_concrete_failure() {
        let server = spawn_mock_http(16, |request| {
            if path_only(&request.path) == "/v1/accounts/faucet/policy" {
                MockResponse::text(404, "missing")
            } else {
                doctor_mock_response(request, None)
            }
        });
        let report = run_doctor(&server.base_url, DoctorScope::Basic).unwrap();
        finish_mock(server);
        assert_eq!(report_status(&report), Some("fail"));
        let check = report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"].as_str() == Some("account_faucet_policy"))
            .unwrap();
        assert_eq!(check["http_status"].as_u64(), Some(404));
        assert_eq!(check["ok"].as_bool(), Some(false));
    }
    #[test]
    fn doctor_rejects_unusable_signing_policy_and_foreign_faucet_identity() {
        for capabilities in [true, false] {
            let server = spawn_mock_http(16, move |request| {
                let mut response = doctor_mock_response(request, None);
                let target = if capabilities {
                    "/v1/accounts/capabilities"
                } else {
                    "/v1/accounts/faucet/policy"
                };
                if path_only(&request.path) == target {
                    let mut body: Value = json::from_slice(&response.body).unwrap();
                    if capabilities {
                        body.as_object_mut()
                            .unwrap()
                            .insert("default_signing".to_owned(), Value::from("secp256k1"));
                    } else {
                        body.as_object_mut()
                            .unwrap()
                            .insert("network_prefix".to_owned(), Value::from(999_u64));
                    }
                    response.body = json::to_vec(&body).unwrap();
                }
                response
            });
            let report = run_doctor(&server.base_url, DoctorScope::Basic).unwrap();
            let requests = finish_mock(server);
            assert_eq!(report_status(&report), Some("fail"));
            if capabilities {
                assert!(
                    !requests
                        .iter()
                        .any(|request| path_only(&request.path) == "/v1/accounts/faucet/policy")
                );
            }
            assert!(report["checks"].as_array().unwrap().iter().any(
                |check| check["name"].as_str() == Some("account_faucet_policy")
                    && check["ok"].as_bool() == Some(false)
            ));
        }
    }
    #[test]
    fn doctor_rejects_another_product_profile_or_a_non_xor_faucet() {
        for wrong_profile in [true, false] {
            let server = spawn_mock_http(16, move |request| {
                let mut response = doctor_mock_response(request, None);
                let path = path_only(&request.path);
                if wrong_profile && path == "/v1/accounts/capabilities" {
                    let mut body: Value = json::from_slice(&response.body).unwrap();
                    body.as_object_mut()
                        .unwrap()
                        .insert("network_prefix".to_owned(), Value::from(753_u64));
                    response.body = json::to_vec(&body).unwrap();
                } else if !wrong_profile && path == "/v1/accounts/faucet/policy" {
                    let other = AssetDefinitionId::derive_from_components(
                        iroha_model_base::domain::DomainId::try_new("assets", "universal").unwrap(),
                        "other".parse().unwrap(),
                    );
                    let mut body: Value = json::from_slice(&response.body).unwrap();
                    body.as_object_mut().unwrap().insert(
                        "asset_definition_id".to_owned(),
                        Value::from(other.to_string()),
                    );
                    response.body = json::to_vec(&body).unwrap();
                }
                response
            });
            let report = run_doctor(&server.base_url, DoctorScope::Basic).unwrap();
            finish_mock(server);
            assert_eq!(report_status(&report), Some("fail"));
            let name = if wrong_profile {
                "account_capabilities"
            } else {
                "account_faucet_policy"
            };
            let check = report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|check| check["name"].as_str() == Some(name))
                .unwrap();
            assert_eq!(check["http_status"].as_u64(), Some(200));
            assert_eq!(check["ok"].as_bool(), Some(false));
            assert!(
                check["detail"]
                    .as_str()
                    .unwrap()
                    .contains(if wrong_profile {
                        "profile 369"
                    } else {
                        "canonical XOR"
                    })
            );
        }
    }
}

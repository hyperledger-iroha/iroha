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
            iroha_torii_shared::account_capabilities::AccountCapabilitiesV1::from_policy(
                network(),
                DEFAULT_CHAIN_DISCRIMINANT,
                &iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1::new(vec![
                    Algorithm::Ed25519,
                ])
                .unwrap(),
            );
        MockResponse::json(200, &json::to_value(&capabilities).unwrap())
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
        MockResponse::json(200, &json::to_value(&policy).unwrap())
    }
    #[test]
    fn doctor_wallet_discovery_is_unsigned_and_mandatory_in_both_scopes() {
        for scope in [DoctorScope::Basic, DoctorScope::Full] {
            let server = spawn_mock_http(16, |request| doctor_mock_response(request, None));
            let report = run_doctor(&server.base_url, scope).unwrap();
            let requests = finish_mock(server);
            assert_eq!(report_status(&report), Some("ok"));
            assert!(
                !requests
                    .iter()
                    .any(|request| path_only(&request.path) == "/v1/sccp/governance/readiness")
            );
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
                            .insert("allowed_signing".to_owned(), Value::Array(Vec::new()));
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
                let check = doctor_check(&report, "account_capabilities");
                assert_eq!(check["ok"].as_bool(), Some(false));
                assert_eq!(
                    check["detail"].as_str(),
                    Some(
                        "account_capabilities: network does not advertise a canonical account signing policy"
                    )
                );
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
    fn doctor_check<'a>(report: &'a Value, name: &str) -> &'a Value {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("doctor check {name}"))
    }
    fn parliament_readiness() -> Value {
        use iroha_sccp::api::{SccpGovernanceBodySizesV1, SccpGovernanceReadinessV1};
        json::to_value(&SccpGovernanceReadinessV1 {
            committed_height: 42,
            eligible_citizens: None,
            body_sizes: SccpGovernanceBodySizesV1 {
                rules_committee: 5,
                agenda_council: 5,
                interest_panel: 5,
                review_panel: 5,
                coordination_council: 5,
                policy_jury: 17,
                confirmation_jury: 0,
                oversight_committee: 5,
                mpc_committee: 5,
                fma_committee: 5,
                alternates: 3,
            },
            active_beacon_session: Some([0x51; 32]),
            beacon_matches_current_roster: true,
            parliament_binding_ballot_available: false,
            reconciliation_available: false,
            attempt_latency_ms: None,
            active_attempts: Vec::new(),
        })
        .unwrap()
    }
    fn run_parliament_response(response: MockResponse) -> (Result<()>, Value, Vec<MockRequest>) {
        let server = spawn_mock_http(32, move |request| {
            if path_only(&request.path) == "/v1/sccp/governance/readiness" {
                MockResponse {
                    status: response.status,
                    content_type: response.content_type,
                    headers: response.headers.clone(),
                    body: response.body.clone(),
                }
            } else {
                doctor_mock_response(request, None)
            }
        });
        let mut output = WriterReportOutput {
            write: Vec::new(),
            output_format: CliOutputFormat::Json,
        };
        let result = Doctor {
            public_root: server.base_url.clone(),
            scope: DoctorScope::Basic,
            parliament: true,
            json: true,
        }
        .run_with_output(&mut output);
        let requests = finish_mock(server);
        let report = json::from_slice(&output.write).expect("doctor must emit its failure report");
        (result, report, requests)
    }
    #[test]
    fn doctor_parliament_reads_live_body_sizes_and_beacon_without_a_signer() {
        let (result, report, requests) =
            run_parliament_response(MockResponse::json(200, &parliament_readiness()));
        result.unwrap();
        assert_eq!(report_status(&report), Some("ok"));
        let requests: Vec<_> = requests
            .iter()
            .filter(|request| path_only(&request.path) == "/v1/sccp/governance/readiness")
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        for header in [
            "authorization",
            "x-iroha-account",
            "x-iroha-signature",
            "x-iroha-onboarding-token",
        ] {
            assert!(requests[0].header_values(header).is_empty());
        }
        let body_sizes = doctor_check(&report, "parliament_body_sizes");
        assert_eq!(body_sizes["ok"].as_bool(), Some(true));
        let detail = body_sizes["detail"].as_str().unwrap();
        assert!(detail.contains("42"));
        assert!(detail.contains("\"policy_jury\":17"));
        assert!(detail.contains("not a seating verdict"));
        let beacon = doctor_check(&report, "parliament_global_beacon_session");
        assert_eq!(beacon["ok"].as_bool(), Some(true));
        assert!(
            beacon["detail"]
                .as_str()
                .unwrap()
                .contains(&hex::encode([0x51; 32]))
        );
        let warnings = report["warnings"].as_array().unwrap();
        for name in [
            "parliament_eligible_citizens",
            "parliament_live_profile",
            "parliament_attempt_progress",
        ] {
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.as_str().unwrap().starts_with(name))
            );
        }
        assert!(warnings.iter().any(|warning| {
            let warning = warning.as_str().unwrap();
            warning.starts_with("parliament_launch: unavailable")
                && warning.contains("binding_ballot_available=false")
                && warning.contains("reconciliation_available=false")
        }));
    }
    #[test]
    fn doctor_parliament_missing_or_mismatched_beacon_fails_the_command_and_report() {
        for (session, matches) in [
            (Value::Null, false),
            (Value::Null, true),
            (json::to_value(&[0x51_u8; 32]).unwrap(), false),
        ] {
            let mut body = parliament_readiness();
            body.as_object_mut()
                .unwrap()
                .insert("active_beacon_session".into(), session);
            body.as_object_mut()
                .unwrap()
                .insert("beacon_matches_current_roster".into(), Value::from(matches));
            let (result, report, _) = run_parliament_response(MockResponse::json(200, &body));
            assert!(result.is_err());
            assert_eq!(report_status(&report), Some("fail"));
            assert_eq!(
                doctor_check(&report, "parliament_global_beacon_session")["ok"].as_bool(),
                Some(false)
            );
            assert!(
                report["failures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|failure| failure
                        .as_str()
                        .unwrap()
                        .contains("parliament_global_beacon_session"))
            );
        }
    }
    #[test]
    fn doctor_parliament_reports_each_closed_launch_gate_without_claiming_authorization() {
        for (ballot, reconciliation) in [(false, true), (true, false), (true, true)] {
            let mut body = parliament_readiness();
            body.as_object_mut().unwrap().insert(
                "parliament_binding_ballot_available".into(),
                Value::from(ballot),
            );
            body.as_object_mut().unwrap().insert(
                "reconciliation_available".into(),
                Value::from(reconciliation),
            );
            let (result, report, _) = run_parliament_response(MockResponse::json(200, &body));
            result.unwrap();
            let warnings = report["warnings"].as_array().unwrap();
            assert_eq!(
                warnings.iter().any(|warning| warning
                    .as_str()
                    .unwrap()
                    .starts_with("parliament_launch: unavailable")),
                !ballot || !reconciliation
            );
            assert!(warnings.iter().any(|warning| {
                warning
                    .as_str()
                    .unwrap()
                    .contains("diagnostics do not authorize launch")
            }));
        }
    }
    #[test]
    fn doctor_parliament_rejects_unavailable_and_malformed_readiness() {
        let mut missing_census = parliament_readiness();
        missing_census
            .as_object_mut()
            .unwrap()
            .remove("eligible_citizens");
        let mut wrong_body_size = parliament_readiness();
        wrong_body_size
            .get_mut("body_sizes")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("policy_jury".into(), Value::from("9"));
        for response in [
            MockResponse::text(503, "unavailable"),
            MockResponse::text(404, "missing"),
            MockResponse::text(200, "not JSON"),
            MockResponse::json(200, &missing_census),
            MockResponse::json(200, &wrong_body_size),
        ] {
            let status = response.status;
            let (result, report, _) = run_parliament_response(response);
            assert!(result.is_err());
            assert_eq!(report_status(&report), Some("fail"));
            let check = doctor_check(&report, "parliament_readiness");
            assert_eq!(check["ok"].as_bool(), Some(false));
            assert_eq!(check["http_status"].as_u64(), Some(u64::from(status)));
            assert!(!report["failures"].as_array().unwrap().is_empty());
            assert!(
                !report["checks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|check| check["name"].as_str() == Some("parliament_body_sizes"))
            );
        }
    }
    #[test]
    fn doctor_parliament_transport_failure_still_emits_a_failed_report() {
        let mut response = MockResponse::json(200, &parliament_readiness());
        // The mock writes its real Content-Length first; this conflicting header makes the
        // HTTP transport reject the response before JSON decoding.
        response.headers.push(("Content-Length", "1".into()));
        let (result, report, _) = run_parliament_response(response);
        assert!(result.is_err());
        assert_eq!(report_status(&report), Some("fail"));
        let check = doctor_check(&report, "parliament_readiness");
        assert_eq!(check["ok"].as_bool(), Some(false));
        assert_eq!(check["http_status"].as_u64(), Some(0));
        assert!(!report["failures"].as_array().unwrap().is_empty());
    }
    #[test]
    fn doctor_parliament_rejects_oversized_readiness() {
        let response = MockResponse {
            status: 200,
            content_type: "application/json",
            headers: Vec::new(),
            body: vec![b' '; 16 * 1024 * 1024 + 1],
        };
        let (result, report, _) = run_parliament_response(response);
        assert!(result.is_err());
        assert_eq!(report_status(&report), Some("fail"));
        let check = doctor_check(&report, "parliament_readiness");
        assert_eq!(check["http_status"].as_u64(), Some(200));
        assert!(check["detail"].as_str().unwrap().contains("byte limit"));
    }
    #[test]
    fn doctor_checks_the_canonical_bond_against_the_live_faucet() {
        let seated = run_doctor_with_faucet_amount(None);
        assert_eq!(report_status(&seated), Some("ok"));
        // Both Parliament checks judge the compiled canonical profile, and their exact detail
        // says the live values are not verified, so they cannot pass for live evidence.
        let reach = doctor_check(&seated, "canonical_bond_faucet_reach");
        assert_eq!(reach["http_status"].as_u64(), Some(200));
        assert_eq!(reach["ok"].as_bool(), Some(true));
        assert_eq!(
            reach["detail"].as_str(),
            Some(doctor_accounts::CANONICAL_BOND_FAUCET_REACH_DETAIL)
        );
        assert!(
            doctor_accounts::CANONICAL_BOND_FAUCET_REACH_DETAIL
                .contains("the live bond is not verified")
        );
        let profile = doctor_check(&seated, "canonical_profile_seating");
        assert_eq!(profile["http_status"].as_u64(), Some(0));
        assert_eq!(profile["ok"].as_bool(), Some(true));
        assert_eq!(
            profile["detail"].as_str(),
            Some(doctor_accounts::CANONICAL_PROFILE_SEATING_DETAIL)
        );
        assert!(
            doctor_accounts::CANONICAL_PROFILE_SEATING_DETAIL
                .contains("the live profile is not verified")
        );
        for name in ["parliament_bond_faucet_reach", "parliament_seating_profile"] {
            assert!(
                !seated["checks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|check| check["name"].as_str() == Some(name)),
                "{name} would read as live evidence"
            );
        }
        // The public-reset contract carries the same exact details.
        let expected = doctor_accounts::expected();
        assert_eq!(
            expected
                .iter()
                .map(|(name, _, _)| *name)
                .collect::<Vec<_>>(),
            [
                "account_capabilities",
                "account_faucet_policy",
                "canonical_bond_faucet_reach",
                "canonical_profile_seating",
            ]
        );
        assert_eq!(
            expected[2].2.as_deref(),
            Some(doctor_accounts::CANONICAL_BOND_FAUCET_REACH_DETAIL)
        );
        assert_eq!(
            expected[3].2.as_deref(),
            Some(doctor_accounts::CANONICAL_PROFILE_SEATING_DETAIL)
        );

        // One XOR above 1 000 000 / 40 puts the bond within 40 faucet claims.
        let generous = run_doctor_with_faucet_amount(Some("25001"));
        assert_eq!(report_status(&generous), Some("fail"));
        let reach = doctor_check(&generous, "canonical_bond_faucet_reach");
        assert_eq!(reach["ok"].as_bool(), Some(false));
        let detail = reach["detail"].as_str().unwrap();
        assert!(
            detail.contains("citizenship_bond_amount=1000000"),
            "{detail}"
        );
        assert!(
            generous["failures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|failure| failure
                    .as_str()
                    .is_some_and(|failure| failure.starts_with("canonical_bond_faucet_reach")))
        );
    }
    fn run_doctor_with_faucet_amount(amount: Option<&'static str>) -> Value {
        let server = spawn_mock_http(16, move |request| {
            let mut response = doctor_mock_response(request, None);
            if let Some(amount) = amount
                && path_only(&request.path) == "/v1/accounts/faucet/policy"
            {
                let mut body: Value = json::from_slice(&response.body).unwrap();
                body.as_object_mut()
                    .unwrap()
                    .insert("amount".to_owned(), Value::from(amount));
                response.body = json::to_vec(&body).unwrap();
            }
            response
        });
        let report = run_doctor(&server.base_url, DoctorScope::Basic).unwrap();
        finish_mock(server);
        report
    }
    #[test]
    fn doctor_does_not_double_count_an_unverifiable_bond_reach() {
        let server = spawn_mock_http(16, |request| {
            if path_only(&request.path) == "/v1/accounts/faucet/policy" {
                MockResponse::text(503, "unavailable")
            } else {
                doctor_mock_response(request, None)
            }
        });
        let report = run_doctor(&server.base_url, DoctorScope::Basic).unwrap();
        finish_mock(server);
        let reach = doctor_check(&report, "canonical_bond_faucet_reach");
        assert_eq!(reach["http_status"].as_u64(), Some(0));
        assert_eq!(reach["ok"].as_bool(), Some(false));
        assert!(
            reach["detail"]
                .as_str()
                .unwrap()
                .contains("cannot be verified")
        );
        let failures = report["failures"].as_array().unwrap();
        assert!(!failures.iter().any(|failure| {
            failure
                .as_str()
                .is_some_and(|failure| failure.starts_with("canonical_"))
        }));
        assert_eq!(
            doctor_check(&report, "canonical_profile_seating")["ok"].as_bool(),
            Some(true)
        );
    }
    #[test]
    fn doctor_parliament_warnings_name_every_unverifiable_requirement() {
        let warnings = doctor_accounts::parliament_warnings().unwrap();
        assert_eq!(warnings.len(), 3);
        for name in [
            "parliament_eligible_citizens",
            "parliament_live_profile",
            "parliament_attempt_progress",
        ] {
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.starts_with(name) && warning.contains("not verified")),
                "{name}: {warnings:?}"
            );
        }
        assert!(warnings[0].contains("at least 9 citizens bonded at 1000000 XOR"));
        assert!(
            warnings[1].contains("/v1/gov/capabilities"),
            "{}",
            warnings[1]
        );
        // The report stays within the public-reset qualification bound for warnings.
        assert!(warnings.iter().all(|warning| warning.len() <= 1_024));
    }
}

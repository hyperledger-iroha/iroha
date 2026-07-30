    #[tokio::test]
    async fn or_multi_field_with_ties_and_sorting() {
        use iroha_data_model::prelude as dm;
        // State
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));

        // Accounts and chain
        let chain_id: dm::ChainId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
        let kp_b = checked_smoke_keypair(
            0x6F,
            iroha_crypto::Algorithm::Ed25519,
            "derive multi-field account transaction filter fixture key",
        );
        let dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let acc_b = dm::AccountId::new(kp_b.public_key().clone());
        // Pre-register the domain and the successful authority (account B).
        {
            let leader0 = checked_smoke_keypair(
                0x70,
                iroha_crypto::Algorithm::BlsNormal,
                "derive multi-field preregistration block leader fixture key",
            );
            let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
            let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
                .chain(0, state.view().latest_block().as_deref())
                .sign(leader0.private_key())
                .unpack(|_| {});
            let mut st_block0 = state.block(unverified0.header());
            let mut stx0 = st_block0.transaction();
            let exec_id = dm::AccountId::new(kp_b.public_key().clone());
            dm::Register::domain(dm::Domain::new(dom.clone()))
                .execute(exec_id.account(), &mut stx0)
                .ok();
            dm::Register::account(dm::Account::new(acc_b.account().clone()))
                .execute(exec_id.account(), &mut stx0)
                .ok();
            stx0.apply();
            let valid0 = unverified0
                .clone()
                .validate_and_record_transactions(&mut st_block0)
                .unpack(|_| {});
            let committed0 = valid0.commit_unchecked().unpack(|_| {});
            crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
        }
        let (_max_clock_drift, _tx_limits) = {
            let v = state.view();
            let p = v.world().parameters();
            (p.sumeragi().max_clock_drift(), p.transaction())
        };

        // tx_success: authority == B, deterministic success
        let mut success_builder = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_b.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        success_builder.set_creation_time(core::time::Duration::from_millis(1500));
        let signed_success = success_builder
            .with_instructions::<dm::InstructionBox>([log_instruction()])
            .sign(kp_b.private_key());
        let tx_success = AcceptedTransaction::new_unchecked(Cow::Owned(signed_success));
        // tx_fail_c: failure due to missing account
        let mut fail_c_builder = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_b.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        fail_c_builder.set_creation_time(core::time::Duration::from_millis(1500));
        let fail_inst_c = dm::Register::domain(dm::Domain::new(dom.clone()));
        let signed_c = fail_c_builder
            .with_instructions::<dm::InstructionBox>([fail_inst_c.into()])
            .sign(kp_b.private_key());
        let tx_fail_c = AcceptedTransaction::new_unchecked(Cow::Owned(signed_c));

        // tx_fail_d: failure due to missing account (unregister non-existent)
        let mut unregister_missing_builder = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_b.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        unregister_missing_builder.set_creation_time(core::time::Duration::from_millis(1500));
        let fail_inst_d = dm::Unregister::domain(DomainId::try_new("void", "universal").unwrap());
        let signed_d = unregister_missing_builder
            .with_instructions::<dm::InstructionBox>([fail_inst_d.into()])
            .sign(kp_b.private_key());
        let tx_fail_d = AcceptedTransaction::new_unchecked(Cow::Owned(signed_d));

        // tx_fail_e: another failure case (duplicate domain register)
        let mut duplicate_domain_builder = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_b.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        duplicate_domain_builder.set_creation_time(core::time::Duration::from_millis(1500));
        let fail_inst_e = dm::Register::domain(dm::Domain::new(dom.clone()));
        let signed_e = duplicate_domain_builder
            .with_instructions::<dm::InstructionBox>([fail_inst_e.into()])
            .sign(kp_b.private_key());
        let tx_fail_e = AcceptedTransaction::new_unchecked(Cow::Owned(signed_e));

        // Commit block with all four
        let leader = checked_smoke_keypair(
            0x71,
            iroha_crypto::Algorithm::BlsNormal,
            "derive multi-field transaction block leader fixture key",
        );
        let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
        let unverified = BlockBuilder::new(vec![tx_success, tx_fail_c, tx_fail_d, tx_fail_e])
            .chain(0, state.view().latest_block().as_deref())
            .sign(leader.private_key())
            .unpack(|_| {});
        let mut st_block = state.block(unverified.header());
        let valid: ValidBlock = unverified
            .validate_and_record_transactions(&mut st_block)
            .unpack(|_| {});
        let committed = valid.clone().commit_unchecked().unpack(|_| {});
        crate::test_utils::finalize_committed_block(&state, st_block, committed);

        // Filter: (result_ok == true) OR (result_ok == false) → all transactions, used to
        // exercise multi-key sorting (result flag + entrypoint hash).
        // Sort: result_ok desc, then entrypoint_hash asc (all timestamps identical)
        let expr = crate::filter::FilterExpr::And(vec![
            crate::filter::FilterExpr::Or(vec![
                crate::filter::FilterExpr::Eq(
                    crate::filter::FieldPath("result_ok".into()),
                    norito::json::Value::Bool(true),
                ),
                crate::filter::FilterExpr::Eq(
                    crate::filter::FieldPath("result_ok".into()),
                    norito::json::Value::Bool(false),
                ),
            ]),
            crate::filter::FilterExpr::Gte(
                crate::filter::FieldPath("timestamp_ms".into()),
                norito::json::Value::from(1500u64),
            ),
        ]);
        let env = crate::filter::QueryEnvelope {
            query: None,
            filter: Some(expr),
            select: None,
            aggregate: None,
            sort: vec![
                crate::filter::SortKey {
                    key: crate::filter::FieldPath("result_ok".into()),
                    order: crate::filter::Order::Desc,
                },
                crate::filter::SortKey {
                    key: crate::filter::FieldPath("entrypoint_hash".into()),
                    order: crate::filter::Order::Asc,
                },
            ],
            pagination: crate::filter::Pagination {
                limit: Some(10),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        };

        let resp = handle_v1_account_transactions(
            state.clone(),
            axum::extract::Path(acc_b.account().to_string()),
            crate::utils::extractors::NoritoJson(env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
        let items = v["items"].as_array().unwrap();
        // All four should match and appear in stable order.
        assert_eq!(items.len(), 4);
        // Ensure `result_ok` flags never flip back to true after the first false entry.
        let mut seen_false = false;
        let mut prev_hash: Option<&str> = None;
        for item in items {
            let ok_flag = item["result_ok"].as_bool().unwrap_or(false);
            if !ok_flag {
                let hash = item["entrypoint_hash"].as_str().unwrap();
                if let Some(prev) = prev_hash {
                    assert!(prev <= hash);
                }
                prev_hash = Some(hash);
                seen_false = true;
            } else {
                assert!(
                    !seen_false,
                    "result_ok entries must be grouped ahead of failures"
                );
            }
        }
    }

    #[tokio::test]
    async fn stable_ordering_with_multiple_keys() {
        use iroha_data_model::prelude as dm;
        // State
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));

        // Accounts and chain
        let chain_id: dm::ChainId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
        let kp_a = checked_smoke_keypair(
            0x72,
            iroha_crypto::Algorithm::Ed25519,
            "derive stable-ordering account transaction fixture key",
        );
        let dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let acc_a = dm::AccountId::new(kp_a.public_key().clone());

        // Pre-register domain and accounts so A exists; B and C will attempt invalid ops later
        {
            let leader0 = checked_smoke_keypair(
                0x73,
                iroha_crypto::Algorithm::BlsNormal,
                "derive stable-ordering preregistration block leader fixture key",
            );
            let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
            let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
                .chain(0, state.view().latest_block().as_deref())
                .sign(leader0.private_key())
                .unpack(|_| {});
            let mut st_block0 = state.block(unverified0.header());
            let mut stx0 = st_block0.transaction();
            let exec_id = dm::AccountId::new(kp_a.public_key().clone());
            dm::Register::domain(dm::Domain::new(dom.clone()))
                .execute(exec_id.account(), &mut stx0)
                .ok();
            dm::Register::account(dm::Account::new(acc_a.account().clone()))
                .execute(exec_id.account(), &mut stx0)
                .ok();
            stx0.apply();
            let valid0 = unverified0
                .clone()
                .validate_and_record_transactions(&mut st_block0)
                .unpack(|_| {});
            let committed0 = valid0.commit_unchecked().unpack(|_| {});
            crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
        }

        let (_max_clock_drift, _tx_limits) = {
            let v = state.view();
            let p = v.world().parameters();
            (p.sumeragi().max_clock_drift(), p.transaction())
        };

        // A_true@1000
        let mut b1 = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_a.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b1.set_creation_time(core::time::Duration::from_millis(1000));
        let signed_a = b1
            .with_instructions::<dm::InstructionBox>([log_instruction()])
            .sign(kp_a.private_key());
        let entry_hash_a = format!("{}", signed_a.hash_as_entrypoint());
        let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(signed_a));
        // A_false@2000 (fail)
        let mut b2 = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_a.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b2.set_creation_time(core::time::Duration::from_millis(2000));
        let signed_b = b2
            .with_instructions::<dm::InstructionBox>([dm::Unregister::domain(
                DomainId::try_new("nope", "universal").unwrap(),
            )
            .into()])
            .sign(kp_a.private_key());
        let entry_hash_b = format!("{}", signed_b.hash_as_entrypoint());
        let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(signed_b));
        // A_false@2000 (fail)
        let mut b3 = dm::TransactionBuilder::new(
            chain_id.clone(),
            acc_a.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b3.set_creation_time(core::time::Duration::from_millis(2000));
        let signed_c = b3
            .with_instructions::<dm::InstructionBox>([dm::Unregister::domain(
                DomainId::try_new("nada", "universal").unwrap(),
            )
            .into()])
            .sign(kp_a.private_key());
        let entry_hash_c = format!("{}", signed_c.hash_as_entrypoint());
        let tx3 = AcceptedTransaction::new_unchecked(Cow::Owned(signed_c));

        // Commit block
        let leader = checked_smoke_keypair(
            0x74,
            iroha_crypto::Algorithm::BlsNormal,
            "derive stable-ordering transaction block leader fixture key",
        );
        let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
        let unverified = BlockBuilder::new(vec![tx1, tx2, tx3])
            .chain(0, state.view().latest_block().as_deref())
            .sign(leader.private_key())
            .unpack(|_| {});
        let mut st_block = state.block(unverified.header());
        let valid: ValidBlock = unverified
            .validate_and_record_transactions(&mut st_block)
            .unpack(|_| {});
        let committed = valid.clone().commit_unchecked().unpack(|_| {});
        crate::test_utils::finalize_committed_block(&state, st_block, committed);

        // Sort by 4 keys: result_ok desc, timestamp_ms asc, entrypoint_hash asc, authority asc
        let env = crate::filter::QueryEnvelope {
            query: None,
            filter: Some(crate::filter::FilterExpr::Gte(
                crate::filter::FieldPath("timestamp_ms".into()),
                norito::json::Value::from(1000u64),
            )),
            select: None,
            aggregate: None,
            sort: vec![
                crate::filter::SortKey {
                    key: crate::filter::FieldPath("result_ok".into()),
                    order: crate::filter::Order::Desc,
                },
                crate::filter::SortKey {
                    key: crate::filter::FieldPath("timestamp_ms".into()),
                    order: crate::filter::Order::Asc,
                },
                crate::filter::SortKey {
                    key: crate::filter::FieldPath("entrypoint_hash".into()),
                    order: crate::filter::Order::Asc,
                },
                crate::filter::SortKey {
                    key: crate::filter::FieldPath("authority".into()),
                    order: crate::filter::Order::Asc,
                },
            ],
            pagination: crate::filter::Pagination {
                limit: Some(10),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        };
        let resp = handle_v1_account_transactions(
            state.clone(),
            axum::extract::Path(acc_a.account().to_string()),
            crate::utils::extractors::NoritoJson(env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        let bools: Vec<bool> = items
            .iter()
            .map(|it| it["result_ok"].as_bool().unwrap())
            .collect();
        let timestamps: Vec<u64> = items
            .iter()
            .map(|it| it["timestamp_ms"].as_u64().unwrap())
            .collect();
        // Ensure result_ok is sorted descending and timestamp asc within ties
        assert!(bools.windows(2).all(|w| w[0] >= w[1]));
        assert_eq!(bools.first(), Some(&true));
        assert_eq!(bools.last(), Some(&false));
        assert!(timestamps.windows(2).all(|w| w[0] <= w[1]));
        // Verify the returned hashes match the expected set {A,B,C}
        let mut actual_hashes: Vec<String> = items
            .iter()
            .map(|it| it["entrypoint_hash"].as_str().unwrap().to_owned())
            .collect();
        actual_hashes.sort();
        let mut expected_hashes = vec![entry_hash_a, entry_hash_b.clone(), entry_hash_c.clone()];
        expected_hashes.sort();
        assert_eq!(actual_hashes, expected_hashes);
    }

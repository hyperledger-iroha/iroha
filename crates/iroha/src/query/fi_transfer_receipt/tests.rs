//! Candidate transport tests; these do not authenticate ledger execution.
use super::*;
use iroha_data_model::{
    asset::fi_link::{AssetFiLinkAttestationV1, AssetFiLinkV1, AssetFiV1},
    isi::asset_fi::TransferToLinkedFiV1,
    nexus::AxtAssetIncarnationV1,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn fixture() -> (FindAssetFiTransferReceiptV1, AssetFiTransferReceiptV1) {
    let account = |seed| {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    };
    let asset = "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().unwrap();
    let wallet = account(41);
    let fi = account(42);
    let incarnation =
        AxtAssetIncarnationV1::try_from_bytes(*iroha_crypto::Hash::new(b"FI SDK receipt").as_ref())
            .unwrap();
    let instruction = TransferToLinkedFiV1 {
        asset_definition_id: asset,
        account_id: wallet.clone(),
        amount: 5_u32.into(),
        expected_link_revision: 1,
        expected_policy_revision: 1,
    };
    let receipt = AssetFiTransferReceiptV1 {
        asset_incarnation: incarnation,
        network: crate::client::test_network_id(),
        execution_call_hash: iroha_crypto::Hash::new(b"execution"),
        instruction_hash: iroha_crypto::Hash::new(b"instruction"),
        authority: wallet.clone(),
        controller: None,
        fi: AssetFiV1 {
            asset_definition_id: instruction.asset_definition_id.clone(),
            fi_account_id: fi.clone(),
            revision: 1,
            enabled: true,
        },
        attestation: AssetFiLinkAttestationV1 {
            asset_definition_id: instruction.asset_definition_id.clone(),
            account_id: wallet.clone(),
            fi_account_id: fi.clone(),
            reference_digest: [9; 32],
            revision: 1,
            enabled: true,
        },
        link: AssetFiLinkV1 {
            asset_definition_id: instruction.asset_definition_id.clone(),
            account_id: wallet.clone(),
            fi_account_id: fi,
            reference_digest: [9; 32],
            fi_revision: 1,
            attestation_revision: 1,
            revision: 1,
            enabled: true,
            controller: None,
            waterfall: false,
            reverse_waterfall: false,
        },
        instruction,
        uid: [7; 32],
        uid_policy_revision: 1,
        uid_policy_hash: iroha_crypto::Hash::new(b"policy"),
        execution_height: 10,
        execution_time_ms: 1000,
        transcript_index: 2,
    };
    let query = FindAssetFiTransferReceiptV1 {
        asset_definition_id: receipt.instruction.asset_definition_id.clone(),
        expected_incarnation: incarnation,
        account_id: wallet,
        execution_call_hash: receipt.execution_call_hash,
        transcript_index: receipt.transcript_index,
    };
    (query, receipt)
}
fn response(
    rows: Vec<AssetFiTransferReceiptV1>,
    more: bool,
    cursor: Option<ForwardCursor>,
) -> http::Response<Vec<u8>> {
    encoded_response(QueryResponse::Iterable(QueryOutput::new_bounded(
        QueryOutputBatchBox::AssetFiTransferReceiptV1(rows).into(),
        more,
        cursor,
    )))
}
fn encoded_response(value: QueryResponse) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, APPLICATION_NORITO)
        .body(norito::encode_canonical(&value).unwrap())
        .unwrap()
}
fn decode(
    reply: &http::Response<Vec<u8>>,
    query: &FindAssetFiTransferReceiptV1,
) -> Result<Option<AssetFiTransferReceiptV1>, QueryError> {
    decode_receipt_response(
        reply,
        query,
        crate::client::test_network_id(),
        MAX_RESPONSE_BYTES,
    )
}
#[test]
fn complete_typed_absence_and_one_candidate_are_preserved() {
    let (query, receipt) = fixture();
    assert_eq!(
        decode(&response(vec![], false, None), &query).unwrap(),
        None
    );
    assert_eq!(
        decode(&response(vec![receipt.clone()], false, None), &query).unwrap(),
        Some(receipt)
    );
}
#[test]
fn each_changed_network_or_historical_selector_is_refused() {
    let (query, receipt) = fixture();
    for field in 0..6 {
        let mut changed = receipt.clone();
        match field {
            0 => {
                changed.network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"other network"),
                ))
            }
            1 => {
                changed.instruction.asset_definition_id =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        iroha_model_base::domain::DomainId::try_new("changed", "universal")
                            .unwrap(),
                        "different".parse().unwrap(),
                    )
            }
            2 => changed.instruction.account_id = receipt.fi.fi_account_id.clone(),
            3 => {
                changed.asset_incarnation = AxtAssetIncarnationV1::try_from_bytes(
                    *iroha_crypto::Hash::new(b"other incarnation").as_ref(),
                )
                .unwrap()
            }
            4 => changed.execution_call_hash = iroha_crypto::Hash::new(b"other execution"),
            _ => changed.transcript_index += 1,
        }
        assert!(
            decode(&response(vec![changed], false, None), &query).is_err(),
            "changed field {field}"
        );
    }
}
#[test]
fn duplicate_partial_cursor_and_wrong_item_are_refused_even_when_empty() {
    let (query, receipt) = fixture();
    for rows in [vec![], vec![receipt.clone()]] {
        for reply in [
            response(rows.clone(), true, None),
            encoded_response(QueryResponse::Iterable(QueryOutput {
                batch: QueryOutputBatchBox::AssetFiTransferReceiptV1(rows.clone()).into(),
                remaining_items: Some(1),
                has_more: false,
                continue_cursor: None,
            })),
            response(
                rows,
                false,
                Some(ForwardCursor {
                    query: "a".repeat(64),
                    cursor: NonZeroU64::new(1).unwrap(),
                    gas_budget: None,
                }),
            ),
        ] {
            assert!(decode(&reply, &query).is_err());
        }
    }
    assert!(
        decode(
            &response(vec![receipt.clone(), receipt], false, None),
            &query
        )
        .is_err()
    );
    for rows in [vec![], vec![query.account_id.clone()]] {
        assert!(
            decode(
                &encoded_response(QueryResponse::Iterable(QueryOutput::new_bounded(
                    QueryOutputBatchBox::AccountId(rows).into(),
                    false,
                    None
                ))),
                &query
            )
            .is_err()
        );
    }
}
#[test]
fn byte_cap_media_canonical_encoding_and_http_failures_are_enforced() {
    let (query, receipt) = fixture();
    let mut reply = response(vec![receipt.clone()], false, None);
    assert!(
        decode_receipt_response(&reply, &query, receipt.network, reply.body().len() - 1).is_err()
    );
    reply
        .headers_mut()
        .insert(CONTENT_TYPE, "application/json".parse().unwrap());
    assert!(decode(&reply, &query).is_err());
    let mut reply = response(vec![receipt], false, None);
    reply.body_mut().push(0);
    assert!(decode(&reply, &query).is_err());
    for status in [
        StatusCode::NOT_FOUND,
        StatusCode::FORBIDDEN,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let mut reply = response(vec![], false, None);
        *reply.status_mut() = status;
        assert!(
            decode(&reply, &query).is_err(),
            "HTTP errors must never become absence"
        );
    }
}

#[test]
fn client_signs_one_exact_ephemeral_query_and_never_retries() {
    const TEST_RESPONSE_BYTES: usize = 16 * 1024;
    use iroha_version::codec::DecodeVersioned as _;
    let (query, original) = fixture();
    let expected_query = query.clone();
    let expected_original = original.clone();
    let sent = Arc::new(AtomicUsize::new(0));
    let count = sent.clone();
    let key = KeyPair::try_from_seed(vec![41; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
    let account = AccountId::new(key.public_key().clone());
    let expected_account = account.clone();
    let transport = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
        let ordinal = count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(snapshot.max_response_bytes, TEST_RESPONSE_BYTES);
        assert_eq!(snapshot.url.path(), torii_routes::pipeline::QUERY.path());
        assert_eq!(
            snapshot.url.query(),
            Some("cursor_mode=ephemeral&count_mode=bounded")
        );
        let signed =
            iroha_data_model::query::SignedQuery::decode_all_versioned(&snapshot.body).unwrap();
        assert_eq!(signed.payload.authority, expected_account);
        assert_ne!(signed.payload.nonce, [0; 32]);
        let QueryRequest::Start(request) = signed.payload.request else {
            panic!("start query");
        };
        assert_eq!(
            request.item,
            iroha_data_model::query::QueryItemKind::AssetFiTransferReceiptV1
        );
        assert_eq!(request.query_payload, expected_query.dyn_encode());
        assert_eq!(request.params.fetch_size.fetch_size, NonZeroU64::new(1));
        if ordinal == 0 {
            Ok(response(vec![original.clone()], false, None))
        } else {
            Ok(http::Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(vec![])
                .unwrap())
        }
    }));
    let config = crate::config::Config {
        chain: iroha_model_base::chain::ChainId::from("fi-receipt-sdk-test"),
        network_id: crate::client::test_network_id(),
        account,
        key_pair: key,
        account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
        torii_api_url: Url::parse("http://mock.local/").unwrap(),
        torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        basic_auth: None,
        api_token: None,
        transaction_add_nonce: false,
        transaction_ttl: Duration::from_secs(5),
        transaction_status_timeout: Duration::from_secs(10),
        sorafs_alias_cache: crate::client::default_alias_policy(),
        sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::GuardPq,
        http_transport_policy: crate::config::HttpTransportPolicy::Ordinary,
        sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::Canary,
    };
    let client = Client::builder(config)
        .build()
        .unwrap()
        .with_test_http_transport(transport);
    *client.data_model_compatibility.lock().unwrap() =
        crate::client::DataModelCompatibility::SubmitCompatible;
    assert_eq!(
        client
            .get_asset_fi_transfer_receipt(query.clone(), TEST_RESPONSE_BYTES)
            .unwrap(),
        Some(expected_original)
    );
    assert!(
        client
            .get_asset_fi_transfer_receipt(query.clone(), TEST_RESPONSE_BYTES)
            .is_err()
    );
    assert_eq!(
        sent.load(Ordering::SeqCst),
        2,
        "one send for each explicitly requested read"
    );
    assert!(client.get_asset_fi_transfer_receipt(query, 0).is_err());
    assert_eq!(
        sent.load(Ordering::SeqCst),
        2,
        "invalid bounds never dispatch"
    );
}

mod accepted_receipt_regressions {
    //! Candidate transport tests; receipt observation never grants execution authority.
    use super::super::*;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        asset::fi_link::{AssetFiLinkAttestationV1, AssetFiLinkV1, AssetFiV1},
        isi::{InstructionBox, asset_fi::TransferToLinkedFiV1},
        nexus::AxtAssetIncarnationV1,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn fixture() -> (FindAssetFiTransferReceiptV1, AssetFiTransferReceiptV1) {
        let account = |seed| {
            AccountId::new(
                KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
            )
        };
        let instruction = TransferToLinkedFiV1 {
            asset_definition_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().unwrap(),
            account_id: account(41),
            amount: 5_u32.into(),
            expected_link_revision: 3,
            expected_policy_revision: 4,
        };
        let fi = AssetFiV1 {
            asset_definition_id: instruction.asset_definition_id.clone(),
            fi_account_id: account(42),
            revision: 1,
            enabled: true,
        };
        let attestation = AssetFiLinkAttestationV1 {
            asset_definition_id: instruction.asset_definition_id.clone(),
            account_id: instruction.account_id.clone(),
            fi_account_id: fi.fi_account_id.clone(),
            reference_digest: [9; 32],
            revision: 2,
            enabled: true,
        };
        let link = AssetFiLinkV1 {
            asset_definition_id: instruction.asset_definition_id.clone(),
            account_id: instruction.account_id.clone(),
            fi_account_id: fi.fi_account_id.clone(),
            reference_digest: attestation.reference_digest,
            fi_revision: fi.revision,
            attestation_revision: attestation.revision,
            revision: instruction.expected_link_revision,
            enabled: true,
            controller: None,
            waterfall: false,
            reverse_waterfall: false,
        };
        let receipt = AssetFiTransferReceiptV1 {
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"FI SDK historical occurrence").as_ref(),
            )
            .unwrap(),
            network: crate::client::test_network_id(),
            execution_call_hash: Hash::new(b"original call"),
            instruction_hash: Hash::new(
                norito::encode_canonical(&InstructionBox::from(instruction.clone())).unwrap(),
            ),
            authority: instruction.account_id.clone(),
            instruction,
            controller: None,
            fi,
            attestation,
            link,
            uid: [7; 32],
            uid_policy_revision: 4,
            uid_policy_hash: Hash::new(b"historical policy"),
            execution_height: 10,
            execution_time_ms: 1234,
            transcript_index: 2,
        };
        let query = FindAssetFiTransferReceiptV1::new(
            receipt.instruction.asset_definition_id.clone(),
            receipt.asset_incarnation,
            receipt.instruction.account_id.clone(),
            receipt.execution_call_hash,
            receipt.transcript_index,
        );
        (query, receipt)
    }

    fn encoded_response(value: QueryResponse) -> http::Response<Vec<u8>> {
        http::Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, APPLICATION_NORITO)
            .body(norito::encode_canonical(&value).unwrap())
            .unwrap()
    }

    fn response(
        rows: Vec<AssetFiTransferReceiptV1>,
        more: bool,
        cursor: Option<ForwardCursor>,
    ) -> http::Response<Vec<u8>> {
        encoded_response(QueryResponse::Iterable(QueryOutput::new_bounded(
            QueryOutputBatchBox::AssetFiTransferReceiptV1(rows).into(),
            more,
            cursor,
        )))
    }

    #[test]
    fn complete_zero_or_one_rows_preserve_exact_historical_observation() {
        let (query, receipt) = fixture();
        let network = crate::client::test_network_id();
        for rows in [vec![], vec![receipt.clone()]] {
            let expected = rows.first().cloned();
            for reply in [
                response(rows.clone(), false, None),
                encoded_response(QueryResponse::Iterable(QueryOutput::new(
                    QueryOutputBatchBox::AssetFiTransferReceiptV1(rows.clone()).into(),
                    0,
                    None,
                ))),
            ] {
                assert_eq!(
                    decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn every_occurrence_selector_and_network_substitution_is_refused() {
        let (query, receipt) = fixture();
        let network = crate::client::test_network_id();
        for field in 0..6 {
            let mut changed = receipt.clone();
            match field {
                0 => {
                    changed.instruction.asset_definition_id =
                        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                            iroha_model_base::domain::DomainId::try_new("changed", "universal")
                                .unwrap(),
                            "different".parse().unwrap(),
                        )
                }
                1 => changed.instruction.account_id = receipt.fi.fi_account_id.clone(),
                2 => {
                    changed.asset_incarnation = AxtAssetIncarnationV1::try_from_bytes(
                        *Hash::new(b"other incarnation").as_ref(),
                    )
                    .unwrap()
                }
                3 => changed.execution_call_hash = Hash::new(b"other call"),
                4 => changed.transcript_index += 1,
                _ => {
                    changed.network = NetworkId::from_genesis_hash(HashOf::<
                        iroha_data_model::block::BlockHeader,
                    >::from_untyped_unchecked(
                        Hash::new(b"other network")
                    ))
                }
            }
            assert!(
                decode_receipt_response(
                    &response(vec![changed], false, None),
                    &query,
                    network,
                    MAX_RESPONSE_BYTES
                )
                .is_err(),
                "changed selector {field}"
            );
        }
    }

    #[test]
    fn incomplete_empty_duplicate_wrong_type_and_extra_columns_are_refused() {
        let (query, receipt) = fixture();
        let network = crate::client::test_network_id();
        for rows in [vec![], vec![receipt.clone()]] {
            for reply in [
                response(rows.clone(), true, None),
                response(
                    rows.clone(),
                    false,
                    Some(ForwardCursor {
                        query: "a".repeat(64),
                        cursor: NonZeroU64::new(1).unwrap(),
                        gas_budget: None,
                    }),
                ),
                encoded_response(QueryResponse::Iterable(QueryOutput {
                    batch: QueryOutputBatchBox::AssetFiTransferReceiptV1(rows.clone()).into(),
                    remaining_items: Some(1),
                    has_more: false,
                    continue_cursor: None,
                })),
                encoded_response(QueryResponse::Iterable(QueryOutput::new_bounded(
                    QueryOutputBatchBoxTuple::new(vec![
                        QueryOutputBatchBox::AssetFiTransferReceiptV1(rows.clone()),
                        QueryOutputBatchBox::AssetFiTransferReceiptV1(rows.clone()),
                    ])
                    .unwrap(),
                    false,
                    None,
                ))),
            ] {
                assert!(
                    decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).is_err()
                );
            }
        }
        for reply in [
            response(vec![receipt.clone(), receipt.clone()], false, None),
            encoded_response(QueryResponse::Iterable(QueryOutput::new_bounded(
                QueryOutputBatchBox::AccountId(vec![]).into(),
                false,
                None,
            ))),
            encoded_response(QueryResponse::Singular(SingularQueryOutputBox::AccountIds(
                vec![],
            ))),
        ] {
            assert!(decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).is_err());
        }
    }

    #[test]
    fn byte_cap_media_and_noncanonical_frames_cannot_become_absence() {
        let (query, receipt) = fixture();
        let network = crate::client::test_network_id();
        for rows in [vec![], vec![receipt]] {
            let mut reply = response(rows.clone(), false, None);
            assert!(
                decode_receipt_response(&reply, &query, network, reply.body().len() - 1).is_err()
            );
            reply
                .headers_mut()
                .insert(CONTENT_TYPE, "application/json".parse().unwrap());
            assert!(decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).is_err());
            let mut reply = response(rows.clone(), false, None);
            reply
                .headers_mut()
                .append(CONTENT_TYPE, APPLICATION_NORITO.parse().unwrap());
            assert!(decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).is_err());
            let mut reply = response(rows.clone(), false, None);
            reply.body_mut().push(0);
            assert!(decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).is_err());
            let mut reply = response(rows, false, None);
            let truncated_len = reply.body().len() / 2;
            reply.body_mut().truncate(truncated_len);
            assert!(decode_receipt_response(&reply, &query, network, MAX_RESPONSE_BYTES).is_err());
        }
    }

    #[test]
    fn every_http_failure_remains_error_even_with_a_valid_empty_body() {
        let (query, _) = fixture();
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let mut reply = response(vec![], false, None);
            *reply.status_mut() = status;
            assert!(
                decode_receipt_response(
                    &reply,
                    &query,
                    crate::client::test_network_id(),
                    MAX_RESPONSE_BYTES
                )
                .is_err()
            );
        }
    }

    #[test]
    fn client_signs_one_exact_ephemeral_query_and_never_retries() {
        const TEST_RESPONSE_BYTES: usize = 16 * 1024;
        use iroha_version::codec::DecodeVersioned as _;
        let (query, original) = fixture();
        let expected_query = query.clone();
        let expected_original = original.clone();
        let sent = Arc::new(AtomicUsize::new(0));
        let count = sent.clone();
        let key = KeyPair::try_from_seed(vec![41; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        let account = AccountId::new(key.public_key().clone());
        let expected_account = account.clone();
        let transport = DefaultHttpTransport::mock(Arc::new(move |snapshot| {
            let ordinal = count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(snapshot.max_response_bytes, TEST_RESPONSE_BYTES);
            assert_eq!(snapshot.url.path(), torii_routes::pipeline::QUERY.path());
            assert_eq!(
                snapshot.url.query(),
                Some("cursor_mode=ephemeral&count_mode=bounded")
            );
            let signed =
                iroha_data_model::query::SignedQuery::decode_all_versioned(&snapshot.body).unwrap();
            assert_eq!(signed.payload.authority, expected_account);
            assert_ne!(signed.payload.nonce, [0; 32]);
            assert_eq!(signed.payload.network_id, crate::client::test_network_id());
            let QueryRequest::Start(request) = signed.payload.request else {
                panic!("start query");
            };
            assert_eq!(
                request.item,
                iroha_data_model::query::QueryItemKind::AssetFiTransferReceiptV1
            );
            assert_eq!(request.query_payload, expected_query.dyn_encode());
            assert_eq!(request.params.fetch_size.fetch_size, NonZeroU64::new(1));
            assert_eq!(request.params.pagination.offset_value(), 0);
            assert_eq!(request.params.pagination.limit_value(), None);
            assert_eq!(request.params.sorting, Default::default());
            assert_eq!(
                request.predicate_bytes,
                CompoundPredicate::<AssetFiTransferReceiptV1>::PASS.encode()
            );
            assert_eq!(
                request.selector_bytes,
                SelectorTuple::<AssetFiTransferReceiptV1>::default().encode()
            );
            if ordinal == 0 {
                Ok(response(vec![original.clone()], false, None))
            } else {
                Ok(http::Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(vec![])
                    .unwrap())
            }
        }));
        let config = crate::config::Config {
            chain: iroha_model_base::chain::ChainId::from("fi-receipt-sdk-test"),
            network_id: crate::client::test_network_id(),
            account,
            key_pair: key,
            account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
            torii_api_url: Url::parse("http://mock.local/").unwrap(),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            basic_auth: None,
            api_token: None,
            transaction_add_nonce: false,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(10),
            sorafs_alias_cache: crate::client::default_alias_policy(),
            sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::GuardPq,
            http_transport_policy: crate::config::HttpTransportPolicy::Ordinary,
            sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::Canary,
        };
        let client = Client::builder(config)
            .build()
            .unwrap()
            .with_test_http_transport(transport);
        *client.data_model_compatibility.lock().unwrap() =
            crate::client::DataModelCompatibility::SubmitCompatible;
        assert_eq!(
            client
                .get_asset_fi_transfer_receipt(query.clone(), TEST_RESPONSE_BYTES)
                .unwrap(),
            Some(expected_original)
        );
        assert!(
            client
                .get_asset_fi_transfer_receipt(query.clone(), TEST_RESPONSE_BYTES)
                .is_err()
        );
        assert_eq!(
            sent.load(Ordering::SeqCst),
            2,
            "one send for each explicitly requested read"
        );
        assert!(
            client
                .get_asset_fi_transfer_receipt(query.clone(), 0)
                .is_err()
        );
        assert!(
            client
                .get_asset_fi_transfer_receipt(query, MAX_RESPONSE_BYTES + 1)
                .is_err()
        );
        assert_eq!(
            sent.load(Ordering::SeqCst),
            2,
            "invalid bounds never dispatch"
        );
    }
}

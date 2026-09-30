fn billing_runtime_test_handle(slot: IrohaRuntimeProviderSlotV1) -> &'static str {
    match slot {
        IrohaRuntimeProviderSlotV1::BillingFinalizedQuery => SERVER_TEST_BILLING_QUERY_HANDLE,
        IrohaRuntimeProviderSlotV1::BillingJournalVerifier => SERVER_TEST_BILLING_VERIFIER_HANDLE,
        IrohaRuntimeProviderSlotV1::BillingStatementSigner => SERVER_TEST_BILLING_SIGNER_HANDLE,
        IrohaRuntimeProviderSlotV1::BillingStatementPublisher => {
            SERVER_TEST_BILLING_PUBLISHER_HANDLE
        }
        IrohaRuntimeProviderSlotV1::BillingAcknowledgementAuthority => {
            SERVER_TEST_BILLING_ACKNOWLEDGEMENT_HANDLE
        }
        IrohaRuntimeProviderSlotV1::BillingEpochWitnessStore => {
            SERVER_TEST_BILLING_EPOCH_STORE_HANDLE
        }
        _ => panic!("slot is not a hedging/billing runtime provider"),
    }
}
fn billing_runtime_test_binding(slot: IrohaRuntimeProviderSlotV1) -> ProviderBindingWireV1 {
    let catalog = IrohaRuntimeProviderBindingsV1::qualified_for_test(
        "server-test-chain",
        slot,
        billing_runtime_test_handle(slot),
        7,
        TEST_POLICY_DIGEST,
    );
    ProviderBindingWireV1::try_from_binding(catalog.iter().next().expect("one billing binding"))
        .expect("project billing test binding")
}
fn billing_operation_request(
    slot: IrohaRuntimeProviderSlotV1,
    request_id: u64,
    operation: u16,
    payload: Vec<u8>,
) -> OperationRequestV1 {
    make_operation_request(
        TEST_SESSION_ID,
        request_id,
        billing_runtime_test_binding(slot),
        [0xB1; 32],
        operation,
        payload,
    )
    .expect("build billing broker operation")
}
#[test]
fn billing_journal_commitment_rejects_same_label_different_genesis() {
    let display_label_a = "server-test-chain";
    let display_label_b = "server-test-chain";
    let local_network = server_test_network_id();
    let foreign_network = test_network_id(0x16);
    assert_eq!(display_label_a, display_label_b);
    assert_ne!(local_network, foreign_network);
    let commitment = sorafs_node::hedging_billing_service::HedgingBillingJournalCommitmentV1 {
        version:
            sorafs_node::hedging_billing_service::HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
        network_id: local_network,
        finalized_cursor: sorafs_node::hedging_billing_service::HedgingBillingFinalizedCursorV1 {
            height: 7,
            block_hash: [0x17; 32],
            finalized_at_unix: 1_800_000_000,
        },
        journal_next_sequence: 2,
        journal_root: [0x18; 32],
    };
    assert!(validate_billing_journal_commitment(commitment, local_network).is_ok());
    assert!(matches!(
        validate_billing_journal_commitment(commitment, foreign_network),
        Err(BrokerError::Rejected)
    ));
}
#[test]
fn billing_verifier_requires_exact_network_identity() {
    use sorafs_node::hedging_billing_service::{
        HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1, HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
        HedgingBillingFinalizedCursorV1, HedgingBillingFinalizedEventPageV1,
        HedgingBillingJournalCommitmentV1,
    };

    let network_id = server_test_network_id();
    // Construct the complete wire record explicitly: a chain-label field cannot
    // reappear without making this initializer incomplete.
    let verify = BillingVerifyPageRequestWireV1 {
        network_id,
        previous: None,
        page: HedgingBillingFinalizedEventPageV1 {
            version: HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1,
            network_id,
            start_sequence: 1,
            next_sequence: 1,
            journal_commitment: HedgingBillingJournalCommitmentV1 {
                version: HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
                network_id,
                finalized_cursor: HedgingBillingFinalizedCursorV1 {
                    height: 7,
                    block_hash: [0x17; 32],
                    finalized_at_unix: 1_800_000_000,
                },
                journal_next_sequence: 1,
                journal_root: [0x18; 32],
            },
            // The broker checks bounded proof shape; the verifier authenticates it.
            append_proof: vec![0xA5],
            inclusion_proof: vec![0xB6],
            events: Vec::new(),
        },
    };
    let bytes = encode_canonical(&verify, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        .expect("encode exact billing verifier wire");
    let decoded: BillingVerifyPageRequestWireV1 =
        decode_canonical(&bytes, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
            .expect("decode exact billing verifier wire");
    assert_eq!(decoded.network_id, network_id);
    assert_eq!(decoded.page.network_id, network_id);
    let request = billing_operation_request(
        IrohaRuntimeProviderSlotV1::BillingJournalVerifier,
        1,
        OPERATION_BILLING_VERIFY_PAGE_V1,
        bytes,
    );
    assert_eq!(
        validate_operation_request_for_session(&request, "server-test-chain", &network_id),
        Ok(())
    );
    assert_eq!(
        validate_operation_request_for_session(&request, "another-display-label", &network_id),
        Ok(()),
        "a display label is not network authority"
    );
    assert_eq!(
        validate_operation_request_for_session(
            &request,
            "server-test-chain",
            &test_network_id(0x16),
        ),
        Err(BrokerError::Rejected),
        "the same display label cannot authorize another genesis"
    );
}

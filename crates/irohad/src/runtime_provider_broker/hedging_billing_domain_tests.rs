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
fn billing_wire_retires_label_chain_identity() {
    let source = include_str!("../runtime_provider_broker.rs");
    let start = source
        .find("struct BillingVerifyPageRequestWireV1")
        .expect("billing verifier wire start");
    let end = source[start..]
        .find("struct BillingSignDigestRequestWireV1")
        .map(|offset| start + offset)
        .expect("billing verifier wire end");
    let verifier_wire = &source[start..end];
    assert!(verifier_wire.contains("network_id: iroha_data_model::NetworkId"));
    assert!(!verifier_wire.contains("chain_id"));
    assert!(!source.contains("HEDGING_BILLING_CHAIN_ID_MAX_BYTES_V1"));
}

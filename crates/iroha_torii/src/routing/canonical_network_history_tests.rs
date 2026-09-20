//! Structural display projections retain typed joins and sealed outer identity.
//! Exact storage/finality and byte admission are exercised by Core's reader tests.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::{builder::BlockBuilder, execution_output::*, output_budget::ExecutionOutputLimits},
    events::{
        time::{TimeEvent, TimeInterval},
        trigger_completed::TriggerCompletedOutcome,
    },
    transaction::{
        FeePaymentIntent, TransactionBuilder,
        signed::{ExecutionStep, SealedTransactionReveal},
    },
    trigger::DataTriggerStep,
};

fn carrier() -> SignedBlock {
    let key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).unwrap();
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert(
        "contract_address".parse().unwrap(),
        iroha_primitives::json::Json::new("contract"),
    );
    metadata.insert(
        "contract_entrypoint".parse().unwrap(),
        iroha_primitives::json::Json::new("submit"),
    );
    let network = iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::new(b"routing query fixture"),
    ));
    let mut tx = TransactionBuilder::new(
        network,
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(vec![], None),
    );
    tx.set_creation_time(Duration::from_millis(900));
    let signed = tx.with_metadata(metadata).sign(key.private_key());
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(2).unwrap(),
        Some(HashOf::from_untyped_unchecked(Hash::new(b"routing parent"))),
        None,
        1000,
        0,
    );
    let mut builder = BlockBuilder::new(header);
    builder.push_sealed_transaction_reveal(SealedTransactionReveal::new(
        Hash::new(b"routing commitment"),
        signed,
        [0x36; 32],
    ));
    let mut block = builder.build_with_signature(0, key.private_key());
    let id: iroha_data_model::trigger::TriggerId = "clock".parse().unwrap();
    let rows = vec![
        ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index: 0,
            result: TransactionResult::new(Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted("business rejection".into()),
                ),
            )),
            completions: vec![],
        }),
        ExecutionOutputV1::Time(TimeExecutionOutputV1 {
            invocation: TimeInvocationV1 {
                schedule_index: 0,
                event: TimeEvent {
                    interval: TimeInterval {
                        since_ms: 999,
                        length_ms: 1,
                    },
                },
                trigger: TriggerUseV1 {
                    trigger_id: id.clone(),
                    registered_at_height: 1,
                    action_hash: Hash::new(b"structural clock action"),
                },
            },
            result: TransactionResult::new(Ok(vec![DataTriggerStep {
                id: id.clone(),
                instructions: ExecutionStep(iroha_primitives::const_vec::ConstVec::new_empty()),
            }])),
            failure_root: None,
            completions: vec![InvocationCompletionV1 {
                callback_index: 0,
                trigger_id: id,
                outcome: TriggerCompletedOutcome::Success,
            }],
        }),
    ];
    block
        .set_execution_outputs(
            rows,
            1,
            Default::default(),
            vec![],
            Default::default(),
            Default::default(),
            vec![],
            &ExecutionOutputLimits {
                max_outputs: 4,
                max_output_bytes: 65536,
                max_total_output_bytes: 262144,
                max_executed_wire_bytes: 1048576,
            },
        )
        .unwrap();
    block.validate_output_merkle_cache().unwrap();
    block
}

#[test]
fn borrowed_history_projection_matches_real_typed_proof_dto_and_sealed_identity() {
    let block = carrier();
    let source = block.network_entrypoint_at(0).unwrap();
    let (_, output) = block.network_output_at(0).unwrap();
    let borrowed = BorrowedNetworkTransaction {
        entrypoint: source,
        entrypoint_hash: source.hash(),
        result: &output.result,
        block_hash: block.hash(),
    };
    let owned = iroha_data_model::query::CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: source.hash(),
        entrypoint_proof: block.network_input_proof(0).unwrap(),
        entrypoint: source.clone(),
        output_hash: HashOf::new(&block.execution_outputs()[0]),
        output_proof: block.output_proof(0).unwrap(),
        output: block.execution_outputs()[0].clone(),
    };
    assert!(owned.verify_inclusion_in_block(&block));
    assert_eq!(
        tx_field_value(&borrowed, "entrypoint_kind"),
        Some("sealed_reveal".into())
    );
    for field in [
        "authority",
        "timestamp_ms",
        "entrypoint_hash",
        "result_ok",
        "metadata.contract_address",
    ] {
        assert_eq!(
            tx_field_value(&borrowed, field),
            tx_field_value(&owned, field)
        );
    }
    let a = contract_activity_projection_from_tx(2, &borrowed).unwrap();
    let b = contract_activity_projection_from_tx(2, &owned).unwrap();
    assert_eq!(a.entrypoint_hash, b.entrypoint_hash);
    assert_eq!(a.contract_address, "contract");
    assert!(!a.result_ok);
    let event = contract_event_projection_from_tx(2, &borrowed).unwrap();
    assert_eq!(event.tx_hash_hex, source.hash().to_string());
    assert_eq!(event.block_hash_hex, block.hash().to_string());
    assert!(!event.result_ok);
    let calls = external_signed_transaction_results(&block).collect::<Vec<_>>();
    assert_eq!(block.execution_outputs().len(), 2);
    assert_eq!(
        calls.len(),
        1,
        "the internal Time output has no Network transaction position"
    );
    assert_eq!(calls[0].0, 0);
    assert_eq!(calls[0].1, source.hash());
    assert_ne!(calls[0].1, calls[0].2.hash_as_entrypoint());
    assert!(calls[0].3.is_err());
    assert!(external_signed_transaction_result_at(&block, 1).is_none());
}

#[test]
fn history_range_missing_canonical_prefix_returns_no_projected_rows() {
    let state = CoreState::new_for_testing(
        iroha_core::state::World::default(),
        Kura::blank_kura_for_testing(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    let mut visited = 0;
    let result = project_finalized_network_range(
        &state,
        1,
        1,
        false,
        HashOf::from_untyped_unchecked(Hash::new(b"missing tip")),
        |_, _| {
            visited += 1;
            vec![()]
        },
    );
    assert!(result.is_err());
    assert_eq!(visited, 0);
    assert!(
        project_finalized_network_range(
            &state,
            1,
            0,
            true,
            HashOf::from_untyped_unchecked(Hash::new(b"unused empty tip")),
            |_, _| vec![()]
        )
        .unwrap()
        .is_empty()
    );
    check_history_retention(2, 2).unwrap();
    assert!(check_history_retention(3, 2).is_err());
}

#[test]
fn history_cache_anchor_requires_the_captured_tip_identity() {
    let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"captured cache tip"));
    assert_eq!(history_cache_anchor(0, &None).unwrap(), None);
    assert!(history_cache_anchor(0, &Some(hash.to_string())).is_err());
    assert!(history_cache_anchor(1, &None).is_err());
    assert!(history_cache_anchor(1, &Some("bad".into())).is_err());
    assert_eq!(
        history_cache_anchor(1, &Some(hash.to_string())).unwrap(),
        Some(hash)
    );
    let state = CoreState::new_for_testing(
        iroha_core::state::World::default(),
        Kura::blank_kura_for_testing(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    assert!(require_history_anchor(&state, 1, hash).is_err());
}

#[test]
fn visibility_refusal_is_sticky_until_consuming_finish() {
    let state = Arc::new(CoreState::new_for_testing(
        iroha_core::state::World::default(),
        Kura::blank_kura_for_testing(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    ));
    let restricted = DataspaceReadVisibility::new(BTreeSet::new(), false);
    let source = HashOf::from_untyped_unchecked(Hash::new(b"unavailable source"));
    let owner = HistoryVisibilityReads::new(Arc::clone(&state));
    assert!(!owner.allows(&restricted, 1, None, source));
    assert!(!owner.allows(&restricted, 1, None, source));
    assert!(owner.reads.lock().unwrap().blocks.is_empty());
    // A later globally visible row cannot erase the earlier admission refusal.
    assert!(owner.allows(&DataspaceReadVisibility::all_for_tests(), 1, None, source));
    assert!(owner.finish().is_err());
    HistoryVisibilityReads::new(state).finish().unwrap();
}

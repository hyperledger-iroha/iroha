//! FX registry publication retains original admission and transaction rollback.

use super::*;
use crate::{
    pipeline_gas_policy::RetainedStateParameters, test_allocations::refuse_one_layout_during,
};

#[test]
fn fx_registry_preparation_refusal_keeps_parameters_and_events_unchanged() {
    let (state, policy) = fx_corridor_state(10, 1_000, 76, true);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    configure_fx_catalog(&mut tx, &policy);
    let before = tx.world.parameters.get().clone();
    let outputs = (
        tx.world.external_event_buf.len(),
        tx.world.internal_event_buf.len(),
    );
    let instruction = SetFxCorridorPolicy { policy };
    let (result, refused) = refuse_one_layout_during(
        RetainedStateParameters::graph_allocation_layout_for_testing(),
        || instruction.execute(&ALICE_ID, &mut tx),
    );
    assert!(refused, "exercise the original parameter graph allocation");
    result.expect_err("registry publication must stop at original resource refusal");
    assert!(tx.is_execution_deferred());
    assert!(before.same_version(tx.world.parameters.get()));
    assert_eq!(
        (
            tx.world.external_event_buf.len(),
            tx.world.internal_event_buf.len()
        ),
        outputs,
    );
}

#[test]
fn fx_usage_preparation_refusal_precedes_balance_transcript_and_receipt_effects() {
    let (state, policy) = fx_corridor_state(10, 1_000, 76, true);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new(b"fx-preparation-refusal"));
    configure_fx_catalog(&mut tx, &policy);
    SetFxCorridorPolicy {
        policy: policy.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    fund_fx_corridor(&mut tx, &policy, 1_000);
    let before = tx.world.parameters.get().clone();
    let outputs = (
        tx.world.external_event_buf.len(),
        tx.world.internal_event_buf.len(),
    );
    let transcripts = tx.pending_transfer_transcript_count_for_testing();
    let assets: Vec<_> = tx
        .world
        .assets
        .iter()
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect();
    let instruction = fx_settlement(&policy, "fx_preparation_refusal", 10);
    let settlement_id = instruction.settlement_id.clone();
    let (result, refused) = refuse_one_layout_during(
        RetainedStateParameters::graph_allocation_layout_for_testing(),
        || instruction.execute(&ALICE_ID, &mut tx),
    );
    assert!(refused, "exercise the original parameter graph allocation");
    result.expect_err("usage publication must be prepared before payment effects");
    assert!(tx.is_execution_deferred());
    assert!(before.same_version(tx.world.parameters.get()));
    assert_eq!(
        tx.world
            .assets
            .iter()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect::<Vec<_>>(),
        assets,
    );
    assert_eq!(
        (
            tx.world.external_event_buf.len(),
            tx.world.internal_event_buf.len()
        ),
        outputs,
    );
    assert!(tx.world.settlement_receipts.get(&settlement_id).is_none());
    assert_eq!(
        tx.pending_transfer_transcript_count_for_testing(),
        transcripts
    );
}

#[test]
fn fx_usage_preparation_is_abandoned_with_failed_transfer_transaction() {
    let (state, policy) = fx_corridor_state(10, 1_000, 76, true);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    {
        let mut setup = block.transaction_for_fastpq_testing(Hash::new(b"fx-rollback-setup"));
        configure_fx_catalog(&mut setup, &policy);
        SetFxCorridorPolicy {
            policy: policy.clone(),
        }
        .execute(&ALICE_ID, &mut setup)
        .unwrap();
        fund_fx_corridor(&mut setup, &policy, 1_000);
        setup.apply();
    }
    let before = block.world.parameters.get().clone();
    let instruction = fx_settlement(&policy, "fx_parameter_rollback", 10);
    let settlement_id = instruction.settlement_id.clone();
    {
        // Deliberately omit the transaction call hash required by transfer transcripts.
        let mut tx = block.transaction();
        configure_fx_catalog(&mut tx, &policy);
        let pool = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let reserved_before = pool.reserved_bytes();
        let error = instruction
            .execute(&ALICE_ID, &mut tx)
            .expect_err("transfer must reject absent transcript identity");
        assert!(
            matches!(error,
                InstructionExecutionError::InvariantViolation(ref message)
                    if message.as_ref() == "native FX transfer requires a transaction call_hash before balance or transcript mutation"
            ),
            "the actual transfer owner must be reached after successful parameter preparation"
        );
        assert!(
            !tx.is_execution_deferred(),
            "missing call identity is the deterministic rejection under test"
        );
        assert_eq!(
            pool.reserved_bytes(),
            reserved_before,
            "the abandoned prepared graph refunds its original pool"
        );
        assert!(
            before.same_version(tx.world.parameters.get()),
            "a failed transfer cannot install the prepared registry"
        );
        assert!(tx.world.settlement_receipts.get(&settlement_id).is_none());
    }
    let retry = block.transaction();
    assert!(before.same_version(retry.world.parameters.get()));
    assert!(
        retry
            .world
            .settlement_receipts
            .get(&settlement_id)
            .is_none()
    );
}

#[test]
fn fx_configuration_event_follows_the_unchanged_transfer_event_sequence() {
    use iroha_data_model::events::data::prelude::{ConfigurationEvent, DomainEvent};
    let (state, policy) = fx_corridor_state(10, 1_000, 76, true);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new(b"fx-event-order"));
    configure_fx_catalog(&mut tx, &policy);
    SetFxCorridorPolicy {
        policy: policy.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    fund_fx_corridor(&mut tx, &policy, 1_000);
    let before = (
        tx.world.external_event_buf.len(),
        tx.world.internal_event_buf.len(),
    );
    fx_settlement(&policy, "fx_event_order", 10)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let internal = &tx.world.internal_event_buf[before.1..];
    let external = &tx.world.external_event_buf[before.0..];
    assert_eq!(internal.len(), external.len());
    let tags: Vec<_> = internal
        .iter()
        .map(|event| {
            let asset = match event.as_ref() {
                DataEvent::Asset(asset) => Some(asset),
                DataEvent::Domain(DomainEvent::Asset(scoped)) => Some(&scoped.event),
                DataEvent::Configuration(ConfigurationEvent::Changed(_)) => return "configuration",
                _ => return "other",
            };
            match asset.unwrap() {
                AssetEvent::Created(_) => "created",
                AssetEvent::Removed(_) => "removed",
                AssetEvent::Added(_) => "added",
                AssetEvent::Transferred(_) => "transferred",
                _ => "other asset",
            }
        })
        .collect();
    assert_eq!(
        &tags[tags.len() - 7..],
        [
            "removed",
            "added",
            "transferred",
            "removed",
            "added",
            "transferred",
            "configuration"
        ],
    );
    assert!(tags[..tags.len() - 7].iter().all(|tag| *tag == "created"));
    for (external, internal) in external.iter().zip(internal) {
        let iroha_data_model::events::EventBox::Data(external) = external else {
            panic!("native FX emits paired data events");
        };
        assert_eq!(external.as_ref(), internal.as_ref());
    }
}

#[test]
fn fx_output_preparation_refusal_keeps_original_cause_and_rolls_back_dirty_prefix() {
    use iroha_data_model::events::EventBox;
    use std::alloc::Layout;

    // A real funding prefix makes the sticky apply barrier observable even though
    // the new FX parameter/output admission correctly precedes its transfer pair.
    for attempt_refused_apply in [false, true] {
        let (state, policy) = fx_corridor_state(10, 2_000, 76, true);
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        let identity = Hash::prehashed([0xF3; Hash::LENGTH]);
        {
            let mut setup = block.transaction_for_fastpq_testing(identity);
            configure_fx_catalog(&mut setup, &policy);
            SetFxCorridorPolicy {
                policy: policy.clone(),
            }
            .execute(&ALICE_ID, &mut setup)
            .unwrap();
            fund_fx_corridor(&mut setup, &policy, 1_000);
            setup.apply();
        }
        let instruction = fx_settlement(&policy, "fx_output_preparation_refusal", 10);
        let source_id = AssetId::with_scope(
            policy.source_asset_definition_id.clone(),
            ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(policy.source_dataspace),
        );
        let receiver_id = AssetId::with_scope(
            policy.destination_asset_definition_id.clone(),
            BOB_ID.clone(),
            AssetBalanceScope::Dataspace(policy.destination_dataspace),
        );
        let initial_parameters = block.world.parameters.get().clone();
        let initial_events = block.world.pending_external_events().to_vec();
        let initial_balances = block
            .world
            .assets
            .iter()
            .map(|(id, value)| (id.clone(), value.as_ref().clone()))
            .collect::<Vec<_>>();
        let initial_receipts = block
            .world
            .settlement_receipts
            .iter()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect::<Vec<_>>();
        let (
            prefix_external,
            prefix_internal,
            complete_external,
            complete_internal,
            complete_transcripts,
        ) = {
            let mut baseline = block.transaction_for_fastpq_testing(identity);
            configure_fx_catalog(&mut baseline, &policy);
            assert!(baseline.world.external_event_buf.is_empty());
            assert!(baseline.world.internal_event_buf.is_empty());
            fund_fx_corridor(&mut baseline, &policy, 1);
            let prefix = (
                baseline.world.external_event_buf.len(),
                baseline.world.internal_event_buf.len(),
            );
            assert!(prefix.0 > 0 && prefix.1 > 0);
            instruction
                .clone()
                .execute(&ALICE_ID, &mut baseline)
                .unwrap();
            assert_eq!(
                asset_balance_or_zero(&baseline, &source_id),
                Quantity::zero()
            );
            assert_eq!(
                asset_balance_or_zero(&baseline, &receiver_id),
                Quantity::from(760_u32)
            );
            (
                prefix.0,
                prefix.1,
                baseline.world.external_event_buf.len(),
                baseline.world.internal_event_buf.len(),
                baseline.pending_transfer_transcript_count_for_testing(),
            )
            // Discard this actual successful baseline without changing the block.
        };
        let transfer_events = complete_external - prefix_external - 1;
        assert!((6..=10).contains(&transfer_events));
        assert_eq!(complete_internal - prefix_internal - 1, transfer_events);
        let mut refused = block.transaction_for_fastpq_testing(identity);
        configure_fx_catalog(&mut refused, &policy);
        fund_fx_corridor(&mut refused, &policy, 1);
        let prefix_balances = refused
            .world
            .assets
            .iter()
            .map(|(id, value)| (id.clone(), value.as_ref().clone()))
            .collect::<Vec<_>>();
        assert_ne!(
            prefix_balances, initial_balances,
            "actual funding dirties this overlay"
        );
        let prefix_external_values = refused.world.external_event_buf.clone();
        let prefix_internal_values = refused.world.internal_event_buf.clone();
        let prefix_transcripts = refused.pending_transfer_transcript_count_for_testing();
        assert_eq!(prefix_external_values.len(), prefix_external);
        assert_eq!(prefix_internal_values.len(), prefix_internal);
        assert!(initial_parameters.same_version(refused.world.parameters.get()));
        assert_eq!(
            asset_balance_or_zero(&refused, &source_id),
            Quantity::from(10_u32)
        );
        assert_eq!(
            asset_balance_or_zero(&refused, &receiver_id),
            Quantity::zero()
        );

        // Leave room for the measured transfer sequence, but not the parameter
        // notification. NativeFxPair reserves the replacement BEFORE that pair.
        // Its checked double-capacity request covers the canonical ten-event bound.
        let capacity = prefix_external + transfer_events;
        let mut output = Vec::with_capacity(capacity);
        assert_eq!(output.capacity(), capacity);
        output.append(&mut refused.world.external_event_buf);
        refused.world.external_event_buf = output;
        let refused_layout = Layout::array::<EventBox>(capacity * 2).unwrap();
        assert!(capacity * 2 >= prefix_external + 11);
        let actual_input = instruction.clone();
        let (result, observed) = refuse_one_layout_during(refused_layout, || {
            actual_input.execute(&ALICE_ID, &mut refused)
        });
        assert!(
            observed,
            "reach the actual fallible output preparation allocation"
        );
        result.expect_err("original output refusal stops before the transfer pair");
        assert!(refused.is_execution_deferred());
        assert_eq!(refused.world.external_event_buf, prefix_external_values);
        assert_eq!(refused.world.internal_event_buf, prefix_internal_values);
        assert_eq!(
            refused.pending_transfer_transcript_count_for_testing(),
            prefix_transcripts
        );
        assert_eq!(
            refused
                .world
                .assets
                .iter()
                .map(|(id, value)| (id.clone(), value.as_ref().clone()))
                .collect::<Vec<_>>(),
            prefix_balances
        );
        assert_eq!(
            asset_balance_or_zero(&refused, &source_id),
            Quantity::from(10_u32)
        );
        assert_eq!(
            asset_balance_or_zero(&refused, &receiver_id),
            Quantity::zero()
        );
        assert!(initial_parameters.same_version(refused.world.parameters.get()));
        assert!(
            fx_policy_registry(&refused)
                .unwrap()
                .usage(&policy.policy_id)
                .is_none()
        );
        assert!(
            refused
                .world
                .settlement_receipts
                .get(&instruction.settlement_id)
                .is_none()
        );
        let original = refused
            .take_execution_deferral()
            .expect("one original refusal owner");
        assert!(
            original.allocation_refusal().is_none(),
            "physical refusal is not a fabricated pool wait"
        );
        assert_eq!(
            original
                .allocator_refusal()
                .unwrap()
                .vec_reserve_error()
                .unwrap()
                .1,
            refused_layout
        );
        assert!(refused.take_execution_deferral().is_none());
        assert!(
            refused.is_execution_deferred(),
            "transport cannot authorize dirty-prefix publication"
        );
        if attempt_refused_apply {
            refused.apply();
        } else {
            drop(refused);
        }
        assert!(initial_parameters.same_version(block.world.parameters.get()));
        assert_eq!(
            block.world.pending_external_events(),
            initial_events.as_slice()
        );
        assert_eq!(
            block
                .world
                .assets
                .iter()
                .map(|(id, value)| (id.clone(), value.as_ref().clone()))
                .collect::<Vec<_>>(),
            initial_balances
        );
        assert_eq!(
            block
                .world
                .settlement_receipts
                .iter()
                .map(|(id, value)| (id.clone(), value.clone()))
                .collect::<Vec<_>>(),
            initial_receipts
        );
        assert_eq!(
            block
                .world
                .assets
                .get(&source_id)
                .map(|value| value.as_ref().clone()),
            Some(Quantity::from(10_u32))
        );
        assert_eq!(
            block
                .world
                .assets
                .get(&receiver_id)
                .map(|value| value.as_ref().clone())
                .unwrap_or_else(Quantity::zero),
            Quantity::zero()
        );
        let unchanged_registry = FxCorridorPolicyRegistry::from_custom_parameter(
            block
                .world
                .parameters
                .get()
                .custom()
                .get(&FxCorridorPolicyRegistry::parameter_id())
                .unwrap(),
        )
        .unwrap()
        .unwrap();
        assert!(unchanged_registry.usage(&policy.policy_id).is_none());
        assert!(
            block
                .world
                .settlement_receipts
                .get(&instruction.settlement_id)
                .is_none()
        );
        if attempt_refused_apply {
            // Attempting to publish the deferred overlay poisons its carrier.
            // Keep the rollback observations above read-only and prove that a
            // fresh child cannot erase this sticky publication refusal.
            let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                drop(block.transaction());
            }))
            .expect_err("refused publication must retain its poisoned carrier");
            assert_eq!(
                error.downcast_ref::<&str>().copied(),
                Some("completed execution retains immutable original State")
            );
        } else {
            let mut retry = block.transaction_for_fastpq_testing(identity);
            configure_fx_catalog(&mut retry, &policy);
            fund_fx_corridor(&mut retry, &policy, 1);
            assert_eq!(
                retry
                    .world
                    .assets
                    .iter()
                    .map(|(id, value)| (id.clone(), value.as_ref().clone()))
                    .collect::<Vec<_>>(),
                prefix_balances
            );
            instruction.clone().execute(&ALICE_ID, &mut retry).unwrap();
            assert!(!retry.is_execution_deferred());
            assert_eq!(asset_balance_or_zero(&retry, &source_id), Quantity::zero());
            assert_eq!(
                asset_balance_or_zero(&retry, &receiver_id),
                Quantity::from(760_u32)
            );
            assert!(
                fx_policy_registry(&retry)
                    .unwrap()
                    .usage(&policy.policy_id)
                    .is_some()
            );
            assert!(
                retry
                    .world
                    .settlement_receipts
                    .get(&instruction.settlement_id)
                    .is_some()
            );
            assert_eq!(retry.world.external_event_buf.len(), complete_external);
            assert_eq!(retry.world.internal_event_buf.len(), complete_internal);
            assert_eq!(
                retry.pending_transfer_transcript_count_for_testing(),
                complete_transcripts
            );
        }
    }
}

#[test]
fn fx_usage_preparation_original_pool_refusal_refunds_and_retries_identical_input() {
    use iroha_allocation::{AllocationBudget, AllocationRefusal};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };
    #[derive(Default)]
    struct Wakes(AtomicUsize);
    impl Wake for Wakes {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (state, policy) = fx_corridor_state(10, 1_000, 76, true);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let identity = Hash::new(b"fx-original-pool-refusal");
    {
        let mut setup = block.transaction_for_fastpq_testing(identity);
        configure_fx_catalog(&mut setup, &policy);
        SetFxCorridorPolicy {
            policy: policy.clone(),
        }
        .execute(&ALICE_ID, &mut setup)
        .unwrap();
        fund_fx_corridor(&mut setup, &policy, 1_000);
        setup.apply();
    }
    let initial_parameters = block.world.parameters.get().clone();
    let initial_events = block.world.pending_external_events().to_vec();
    let initial_balances = block
        .world
        .assets
        .iter()
        .map(|(id, value)| (id.clone(), value.as_ref().clone()))
        .collect::<Vec<_>>();
    let instruction = fx_settlement(&policy, "fx_original_pool_refusal", 10);
    let mut refused = block.transaction_for_fastpq_testing(identity);
    configure_fx_catalog(&mut refused, &policy);
    // Resolve the actual original FX input before occupying capacity so a
    // precondition's unrelated allocation cannot impersonate registry admission.
    let validated =
        validate_fx_settlement_preconditions(&ALICE_ID, &mut refused, &instruction).unwrap();
    let mut registry = fx_policy_registry(&refused).unwrap();
    registry
        .usage
        .insert(policy.policy_id.clone(), validated.next_usage);
    let next = Parameter::Custom(registry.into_custom_parameter());
    let original_pool = refused
        .world
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    let mut registration = crate::unit_test_support::release_registration(&original_pool);
    assert!(registration.belongs_to(&original_pool));
    let reserved_before = original_pool.reserved_bytes();
    let limit = original_pool.limit_bytes();
    assert!(limit > reserved_before);
    let occupied = original_pool
        .try_reserve_bytes(limit - reserved_before)
        .unwrap();
    assert_eq!(original_pool.reserved_bytes(), limit);
    let foreign_pool = AllocationBudget::new(1);
    let foreign_occupied = foreign_pool.try_reserve_bytes(1).unwrap();
    let transcripts = refused.pending_transfer_transcript_count_for_testing();
    let error = match refused.prepare_fx_usage_parameter_change(&next) {
        Err(original) => original,
        Ok(_) => panic!("actual usage preparation must use its occupied original pool"),
    };
    let _instruction_error = refused.parameter_change_instruction_error(error);
    assert!(refused.is_execution_deferred());
    assert!(initial_parameters.same_version(refused.world.parameters.get()));
    assert_eq!(
        refused
            .world
            .assets
            .iter()
            .map(|(id, value)| (id.clone(), value.as_ref().clone()))
            .collect::<Vec<_>>(),
        initial_balances
    );
    assert!(refused.world.external_event_buf.is_empty());
    assert!(refused.world.internal_event_buf.is_empty());
    assert_eq!(
        refused.pending_transfer_transcript_count_for_testing(),
        transcripts
    );
    assert!(
        fx_policy_registry(&refused)
            .unwrap()
            .usage(&policy.policy_id)
            .is_none()
    );
    assert!(
        refused
            .world
            .settlement_receipts
            .get(&instruction.settlement_id)
            .is_none()
    );
    assert_eq!(
        original_pool.reserved_bytes(),
        limit,
        "no leaked partial preparation charge"
    );
    let original = refused
        .take_execution_deferral()
        .expect("original capacity owner");
    let Some(AllocationRefusal::Capacity {
        requested_bytes,
        reserved_bytes,
        limit_bytes,
        release,
    }) = original.allocation_refusal()
    else {
        panic!("preserve the actual original capacity refusal");
    };
    assert!(*requested_bytes > 0);
    assert_eq!(*reserved_bytes, limit);
    assert_eq!(*limit_bytes, limit);
    assert!(original.allocator_refusal().is_none());
    assert!(refused.take_execution_deferral().is_none());
    assert!(refused.is_execution_deferred());
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wakes));
    let mut context = Context::from_waker(&waker);
    assert_eq!(registration.poll_wait(release, &mut context), Poll::Pending);
    drop(foreign_occupied);
    assert_eq!(
        wakes.0.load(Ordering::SeqCst),
        0,
        "foreign pool release cannot satisfy this refusal"
    );
    assert_eq!(registration.poll_wait(release, &mut context), Poll::Pending);
    drop(occupied);
    // Credits are reusable immediately, but callbacks cannot run while the
    // enclosing World still owns its physical writers.
    assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
    assert_eq!(registration.poll_wait(release, &mut context), Poll::Pending);
    assert_eq!(original_pool.reserved_bytes(), reserved_before);
    drop(refused);
    assert!(initial_parameters.same_version(block.world.parameters.get()));
    assert_eq!(
        block.world.pending_external_events(),
        initial_events.as_slice()
    );
    assert_eq!(
        block
            .world
            .assets
            .iter()
            .map(|(id, value)| (id.clone(), value.as_ref().clone()))
            .collect::<Vec<_>>(),
        initial_balances
    );
    let mut retry = block.transaction_for_fastpq_testing(identity);
    configure_fx_catalog(&mut retry, &policy);
    assert!(original_pool.same_pool(retry.world.sccp_committee_generations.allocation_budget()));
    let reserved_before_retry = original_pool.reserved_bytes();
    let prepared = retry.prepare_fx_usage_parameter_change(&next).unwrap();
    assert!(initial_parameters.same_version(retry.world.parameters.get()));
    drop(prepared);
    assert_eq!(original_pool.reserved_bytes(), reserved_before_retry);
    instruction.clone().execute(&ALICE_ID, &mut retry).unwrap();
    assert!(!retry.is_execution_deferred());
    assert!(
        fx_policy_registry(&retry)
            .unwrap()
            .usage(&policy.policy_id)
            .is_some()
    );
    assert!(
        retry
            .world
            .settlement_receipts
            .get(&instruction.settlement_id)
            .is_some()
    );
    let source = AssetId::with_scope(
        policy.source_asset_definition_id.clone(),
        ALICE_ID.clone(),
        AssetBalanceScope::Dataspace(policy.source_dataspace),
    );
    let receiver = AssetId::with_scope(
        policy.destination_asset_definition_id.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Dataspace(policy.destination_dataspace),
    );
    assert_eq!(asset_balance_or_zero(&retry, &source), Quantity::zero());
    assert_eq!(
        asset_balance_or_zero(&retry, &receiver),
        Quantity::from(760_u32)
    );
    drop(retry);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
    drop(block);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        registration.poll_wait(release, &mut context),
        Poll::Ready(())
    );
}

fn assert_fx_prepared_event_window(preceding_events: usize) {
    use iroha_data_model::{
        asset::{AssetTransferControlWindow, AssetTransferLimit},
        events::data::prelude::{AccountEvent, ConfigurationEvent, DomainEvent},
    };
    assert!(matches!(preceding_events, 6 | 10));
    let minimum = preceding_events == 6;
    let funding = if minimum { 2_000 } else { 1_000 };
    let (state, policy) = fx_corridor_state(if minimum { 20 } else { 10 }, funding, 76, true);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new(b"fx-event-window"));
    configure_fx_catalog(&mut tx, &policy);
    SetFxCorridorPolicy {
        policy: policy.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    fund_fx_corridor(&mut tx, &policy, funding);
    let escrow = fx_corridor_escrow_account(&tx, &policy);
    if minimum {
        // An actual earlier settlement creates both destinations. The next pair
        // therefore emits neither creation nor transfer-control metadata events.
        fx_settlement(&policy, "fx_event_window_warmup", 10)
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
    } else {
        // Both destinations remain absent. Real outbound Day controls on both
        // sources produce one metadata update per leg when its usage advances.
        for (account, definition) in [
            (ALICE_ID.clone(), policy.source_asset_definition_id.clone()),
            (
                escrow.clone(),
                policy.destination_asset_definition_id.clone(),
            ),
        ] {
            let mut control = AssetTransferControlRecord::new(definition);
            control.limits.push(AssetTransferLimit {
                window: AssetTransferControlWindow::Day,
                cap_amount: Some(Quantity::from(10_000_u32)),
            });
            crate::smartcontracts::isi::asset::isi::update_control_record(
                &mut tx, &account, control,
            )
            .unwrap();
        }
    }
    let source_destination = AssetId::with_scope(
        policy.source_asset_definition_id.clone(),
        policy.owner.clone(),
        AssetBalanceScope::Dataspace(policy.source_dataspace),
    );
    let recipient = AssetId::with_scope(
        policy.destination_asset_definition_id.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Dataspace(policy.destination_dataspace),
    );
    assert_eq!(tx.world.assets.get(&source_destination).is_some(), minimum);
    assert_eq!(tx.world.assets.get(&recipient).is_some(), minimum);
    let before_parameters = tx.world.parameters.get().clone();
    let offsets = (
        tx.world.external_event_buf.len(),
        tx.world.internal_event_buf.len(),
    );
    let instruction = fx_settlement(&policy, "fx_event_window_checked", 10);
    let settlement_id = instruction.settlement_id.clone();
    instruction.execute(&ALICE_ID, &mut tx).unwrap();
    assert!(!tx.is_execution_deferred());
    assert!(!before_parameters.same_version(tx.world.parameters.get()));
    let internal = &tx.world.internal_event_buf[offsets.1..];
    let external = &tx.world.external_event_buf[offsets.0..];
    assert_eq!(internal.len(), preceding_events + 1);
    assert_eq!(external.len(), internal.len());
    for (external, internal) in external.iter().zip(internal) {
        let iroha_data_model::events::EventBox::Data(external) = external else {
            panic!("the actual FX pair and parameter publication emit paired data events");
        };
        assert_eq!(external.as_ref(), internal.as_ref());
    }
    let tags = internal
        .iter()
        .map(|event| {
            let asset = match event.as_ref() {
                DataEvent::Asset(asset) => asset,
                DataEvent::Domain(DomainEvent::Asset(scoped)) => &scoped.event,
                DataEvent::Account(AccountEvent::MetadataInserted(change)) => {
                    assert_eq!(change.key.as_ref(), "asset_transfer_controls");
                    return "control";
                }
                DataEvent::Configuration(ConfigurationEvent::Changed(_)) => return "configuration",
                other => panic!("unexpected event in actual FX publication: {other:?}"),
            };
            match asset {
                AssetEvent::Created(_) => "created",
                AssetEvent::Removed(_) => "removed",
                AssetEvent::Added(_) => "added",
                AssetEvent::Transferred(_) => "transferred",
                other => panic!("unexpected FX asset event: {other:?}"),
            }
        })
        .collect::<Vec<_>>();
    if minimum {
        assert_eq!(
            tags,
            [
                "removed",
                "added",
                "transferred",
                "removed",
                "added",
                "transferred",
                "configuration"
            ]
        );
    } else {
        assert_eq!(
            tags,
            [
                "created",
                "control",
                "created",
                "control",
                "removed",
                "added",
                "transferred",
                "removed",
                "added",
                "transferred",
                "configuration"
            ]
        );
        for (account_id, definition, spent) in [
            (
                ALICE_ID.clone(),
                policy.source_asset_definition_id.clone(),
                10_u32,
            ),
            (
                escrow,
                policy.destination_asset_definition_id.clone(),
                760_u32,
            ),
        ] {
            let account = tx.world.account(&account_id).unwrap();
            let store = crate::smartcontracts::isi::asset::isi::load_asset_transfer_control_store_from_account(&account_id, account.metadata()).unwrap();
            let control = store.find(&definition).unwrap();
            assert_eq!(control.usages.len(), 1);
            assert_eq!(control.usages[0].window, AssetTransferControlWindow::Day);
            assert_eq!(control.usages[0].spent_amount, Quantity::from(spent));
        }
    }
    let source = AssetId::with_scope(
        policy.source_asset_definition_id.clone(),
        ALICE_ID.clone(),
        AssetBalanceScope::Dataspace(policy.source_dataspace),
    );
    assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::zero());
    assert_eq!(
        asset_balance_or_zero(&tx, &recipient),
        Quantity::from(if minimum { 1_520_u32 } else { 760_u32 })
    );
    let registry = fx_policy_registry(&tx).unwrap();
    let usage = registry.usage(&policy.policy_id).unwrap();
    assert_eq!(usage.settlements, if minimum { 2 } else { 1 });
    assert_eq!(
        usage.source_amount,
        Quantity::from(if minimum { 20_u32 } else { 10_u32 })
    );
    assert_eq!(
        usage.destination_amount,
        Quantity::from(if minimum { 1_520_u32 } else { 760_u32 })
    );
    assert!(tx.world.settlement_receipts.get(&settlement_id).is_some());
}

#[test]
fn fx_prepared_publication_accepts_actual_six_event_minimum() {
    assert_fx_prepared_event_window(6);
}

#[test]
fn fx_prepared_publication_accepts_actual_ten_event_maximum() {
    assert_fx_prepared_event_window(10);
}

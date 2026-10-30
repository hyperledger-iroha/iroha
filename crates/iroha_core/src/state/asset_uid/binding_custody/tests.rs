//! Real transaction binding admission must fund graph bytes after both controls.
use super::*;
use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};
use iroha_data_model::asset::uid::AssetUidDailyWindowV1;

#[test]
fn sccp_binding_graph_capacity_preserves_original_refusal_before_uid_escape() {
    let _fixture_retirement_pin = crossbeam_epoch::pin();
    let state = blank_state();
    let mut block = state.block(header(2));
    let account = authority(1);
    let definition = iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id();
    let binding = AssetUidBindingV1 {
        asset_definition_id: definition.clone(),
        account_id: account.clone(),
        uid: [7; 32],
    };
    let policy = AssetUidPolicyV1 {
        asset_definition_id: definition.clone(),
        uid: binding.uid,
        revision: 1,
        accounts: BTreeSet::from([account.clone()]),
        holding_limit: None,
        daily_window: AssetUidDailyWindowV1::UtcDay,
        daily_spend_limit: None,
        daily_outgoing_limit: None,
    };
    let binding_path = binding_key(&definition, &account);
    let binding_bytes = encode(&binding).unwrap();
    block
        .world
        .smart_contract_state
        .insert(binding_path.clone(), binding_bytes.clone());
    block.world.smart_contract_state.insert(
        policy_key(&definition, &binding.uid),
        encode(&policy).unwrap(),
    );
    let tx = block.transaction();
    let budget = tx
        .world
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    let baseline = budget.reserved_bytes();
    let controls = norito::core::PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum::<usize>();
    let key = account
        .try_signatory()
        .unwrap()
        .retained_allocation_layout();
    // Both physical controls fit. Only the subsequent real account key is refused.
    let held = budget
        .try_reserve_bytes(budget.limit_bytes() - baseline - controls)
        .unwrap();
    let expected = budget.try_reserve_bytes(controls + key.size()).unwrap_err();
    let iroha_allocation::AllocationRefusal::Capacity {
        release: expected_release,
        ..
    } = expected
    else {
        unreachable!()
    };
    let result = member_policy_with_owner(&*tx.world, &definition, &account, Some(&tx.world));
    assert!(
        result.is_err(),
        "HC975 transaction binding must refuse its actual account graph on the original pool before returning the UID"
    );
    let reason = tx
        .borrow_execution_deferral()
        .expect("real graph refusal defers original transaction");
    let Some(iroha_allocation::AllocationRefusal::Capacity {
        requested_bytes,
        reserved_bytes,
        limit_bytes,
        release,
    }) = reason.allocation_refusal()
    else {
        panic!("original graph capacity refusal was replaced")
    };
    assert_eq!(
        *requested_bytes,
        key.size(),
        "the leaf key, not either counter control, must be refused"
    );
    assert_eq!(*reserved_bytes, budget.limit_bytes());
    assert_eq!(*limit_bytes, budget.limit_bytes());
    assert_eq!(
        release, &expected_release,
        "retain the original pre-probe release observation"
    );
    assert!(reason.allocator_refusal().is_none());
    assert_eq!(budget.reserved_bytes(), baseline + held.remaining_bytes());
    assert_eq!(
        tx.world.smart_contract_state.get(&binding_path),
        Some(&binding_bytes)
    );
    drop(reason);
    drop(held);
    drop(tx);
    let tx = block.transaction();
    let retry_baseline = budget.reserved_bytes();
    assert_eq!(
        member_policy_with_owner(&*tx.world, &definition, &account, Some(&tx.world)).unwrap(),
        Some(policy)
    );
    assert!(!tx.is_execution_deferred());
    assert_eq!(budget.reserved_bytes(), retry_baseline);
    assert_eq!(
        binding_uid_with_owner(&*tx.world, &definition, &account, Some(&tx.world)).unwrap(),
        Some(binding.uid)
    );
}

#[test]
fn transaction_binding_malformed_header_precedes_graph_and_control_admission() {
    let _fixture_retirement_pin = crossbeam_epoch::pin();
    let state = blank_state();
    let mut block = state.block(header(2));
    let account = authority(1);
    let definition = iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id();
    block
        .world
        .smart_contract_state
        .insert(binding_key(&definition, &account), vec![0; 4]);
    let tx = block.transaction();
    let budget = tx.world.sccp_committee_generations.allocation_budget();
    let held = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    assert!(binding_uid_with_owner(&*tx.world, &definition, &account, Some(&tx.world)).is_err());
    assert!(!tx.is_execution_deferred());
    drop(held);
}

#[test]
fn transaction_and_readonly_binding_share_canonical_key_rejection_order() {
    let state = blank_state();
    let mut block = state.block(header(2));
    let account = authority(1);
    let definition = iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id();
    for (bound_account, uid) in [
        (account.clone(), [7; 32]),
        (authority(2), [7; 32]),
        (account.clone(), [0; 32]),
    ] {
        let value = AssetUidBindingV1 {
            asset_definition_id: definition.clone(),
            account_id: bound_account,
            uid,
        };
        block
            .world
            .smart_contract_state
            .insert(binding_key(&definition, &account), encode(&value).unwrap());
        let tx = block.transaction();
        let readonly =
            binding(&*tx.world, &definition, &account).map(|value| value.map(|value| value.uid));
        let owned = binding_uid_with_owner(&*tx.world, &definition, &account, Some(&tx.world));
        assert_eq!(
            readonly.map_err(|error| error.to_string()),
            owned.map_err(|error| error.to_string())
        );
        assert!(!tx.is_execution_deferred());
    }
}

#[test]
fn actual_binding_key_allocator_refusal_preserves_original_physical_cause() {
    let value = AssetUidBindingV1 {
        asset_definition_id: iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id(),
        account_id: authority(1),
        uid: [7; 32],
    };
    let bytes = encode(&value).unwrap();
    let pool = iroha_allocation::AllocationBudget::new(1 << 20);
    let layout = value
        .account_id
        .try_signatory()
        .unwrap()
        .retained_allocation_layout();
    let (result, refused) =
        crate::test_allocations::refuse_layout_occurrence_during(layout, 0, || {
            decode_asset_uid_binding_with_owned_controls(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
                &pool,
            )
        });
    assert!(refused);
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("actual binding key must refuse"),
    };
    let ExecutionAttemptError::Deferred(reason) = decode_attempt(error) else {
        panic!("actual graph refusal cannot reject the binding")
    };
    assert!(reason.allocation_refusal().is_none());
    assert!(reason.decode_refusal().is_none());
    assert!(matches!(reason.allocator_refusal().unwrap().buffer_error(),
        Some(iroha_allocation::ChargedBufferError::Allocator { requested_bytes }) if *requested_bytes == layout.size()));
    assert_eq!(pool.reserved_bytes(), 0);
}

use core::num::NonZeroU64;

use iroha_test_samples::ALICE_ID;

use super::*;

fn test_state_with_account() -> (State, AccountId) {
    let kura = crate::kura::Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let mut world = World::default();
    let account_id = (*ALICE_ID).clone();
    world.accounts.insert(
        account_id.clone(),
        AccountValue::new(iroha_data_model::account::AccountDetails::default()),
    );
    let state = State::new_for_testing(world, Arc::clone(&kura), query);
    (state, account_id)
}

#[test]
fn state_block_accounts_snapshot_is_cached() {
    let (state, account_id) = test_state_with_account();
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = state.block(header);
    let first = block.accounts_snapshot();
    let second = block.accounts_snapshot();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.len(), 1);
    assert_eq!(&first[0], &account_id);
}

#[test]
fn state_view_accounts_snapshot_is_cached() {
    let (state, account_id) = test_state_with_account();
    let view = state.view();
    let first = view.accounts_snapshot();
    let second = view.accounts_snapshot();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.len(), 1);
    assert_eq!(&first[0], &account_id);
}

//! Exact account identity fixtures shared by committed and frozen owner controls.

use super::*;
use crate::test_allocations::allocations_during;
use iroha_data_model::account::AccountDetails;
use iroha_model_base::metadata::Metadata;
use iroha_test_samples::{ALICE_ID, BOB_ID};

/// Stored fixture UAID, independent of the account controller hash.
pub(in crate::state) fn uaid() -> UniversalAccountId {
    UniversalAccountId::from_hash(Hash::new(b"checked identity uaid"))
}
/// Fixed-size stored opaque fixture identifier.
pub(in crate::state) fn opaque(index: u8) -> OpaqueAccountId {
    OpaqueAccountId::from_hash(Hash::new([index; 32]))
}
/// Canonical stored account details, with implicit accounts kept representable.
pub(in crate::state) fn details(
    uaid: Option<UniversalAccountId>,
    ids: Vec<OpaqueAccountId>,
) -> AccountValue {
    AccountValue::new(AccountDetails::new(Metadata::default(), None, uaid, ids))
}
/// Original explicit and implicit account fixture with exact derived indexes.
pub(in crate::state) fn fixture() -> World {
    let mut world = World::default();
    world
        .accounts
        .insert(ALICE_ID.clone(), details(Some(uaid()), vec![opaque(1)]));
    world.accounts.insert(BOB_ID.clone(), details(None, vec![]));
    crate::state::account_identity_restore::rebuild(&mut world).unwrap();
    world
}
/// One account, one stored UAID and one opaque with no undo image.
pub(in crate::state) fn single() -> World {
    let mut world = World::default();
    world
        .accounts
        .insert(ALICE_ID.clone(), details(Some(uaid()), vec![opaque(1)]));
    crate::state::account_identity_restore::rebuild(&mut world).unwrap();
    world
}
/// Require the actual borrowed relation/currentness operation to allocate nothing.
pub(in crate::state) fn without_allocations<T>(run: impl FnOnce() -> T) -> T {
    let mut value = None;
    assert_eq!(allocations_during(|| value = Some(run())), 0);
    value.unwrap()
}
/// Derive the exact monotone local work boundary of a valid original relation.
pub(in crate::state) fn exact_work(
    accounts: &impl RawStorageImages<AccountId, AccountValue>,
    uaids: &impl RawStorageImages<UniversalAccountId, AccountId>,
    opaques: &impl RawStorageImages<OpaqueAccountId, UniversalAccountId>,
) -> u64 {
    let mut low = 0;
    let mut high = 16_777_216;
    validate_original_account_identities(accounts, uaids, opaques, high).unwrap();
    while low < high {
        let allowance = low + (high - low) / 2;
        match validate_original_account_identities(accounts, uaids, opaques, allowance) {
            Ok(()) => high = allowance,
            Err(IdentityOwnershipError::WorkLimit) => low = allowance + 1,
            Err(error) => panic!("valid original identity relation: {error}"),
        }
    }
    low
}
/// Inspect one committed relation using its exact original native readers.
pub(in crate::state) fn exact_world_work(world: &World) -> u64 {
    let accounts = world.accounts.try_committed_view_nonblocking().unwrap();
    let uaids = world
        .uaid_accounts
        .try_committed_view_nonblocking()
        .unwrap();
    let opaques = world.opaque_uaids.try_committed_view_nonblocking().unwrap();
    exact_work(&accounts, &uaids, &opaques)
}

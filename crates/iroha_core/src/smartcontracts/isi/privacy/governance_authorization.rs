fn invalid_privacy_parameter(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}
fn has_exact_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    required: &Permission,
) -> bool {
    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.contains(required))
    {
        return true;
    }
    state_transaction
        .world
        .account_roles
        .iter()
        .filter_map(|(role_key, ())| {
            if &role_key.account == authority {
                state_transaction.world.roles.get(&role_key.id)
            } else {
                None
            }
        })
        .any(|role| role.permissions().any(|permission| permission == required))
}
pub(super) fn ensure_privacy_governance(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // The signed initial genesis block is the chain's root governance act.  It
    // must be able to seed immutable privacy profiles and policies before any
    // account permission exists.  The committed-history guard prevents a
    // height-one-shaped header replay from regaining this bootstrap authority.
    if crate::executor::is_initial_genesis_context(state_transaction) {
        return Ok(());
    }
    let required: Permission = CanEnactGovernance.into();
    if !has_exact_permission(state_transaction, authority, &required) {
        return Err(Error::InvariantViolation(
            "not permitted: CanEnactGovernance".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod governance_authorization_tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use iroha_test_samples::ALICE_ID;

    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};

    fn state() -> crate::state::State {
        crate::state::State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn header(height: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        )
    }

    #[test]
    fn initial_genesis_is_the_privacy_governance_root() {
        let state = state();
        let mut block = state.block(header(1));
        let transaction = block.transaction();

        ensure_privacy_governance(&ALICE_ID, &transaction)
            .expect("initial signed genesis may seed privacy governance");
    }

    #[test]
    fn permissionless_post_genesis_authority_is_rejected() {
        let state = state();
        let mut block = state.block(header(2));
        let transaction = block.transaction();

        let error = ensure_privacy_governance(&ALICE_ID, &transaction)
            .expect_err("post-genesis privacy governance requires the exact permission");
        assert!(matches!(
            error,
            Error::InvariantViolation(ref message)
                if message.as_ref() == "not permitted: CanEnactGovernance"
        ));
    }

    #[test]
    fn replayed_genesis_header_does_not_restore_privacy_governance() {
        let state = state();
        let mut block = state.block(header(1));
        block
            .block_hashes
            .push_for_tests(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"privacy-genesis-replay-guard",
            )));
        let transaction = block.transaction();

        assert!(!crate::executor::is_initial_genesis_context(&transaction));
        let error = ensure_privacy_governance(&ALICE_ID, &transaction)
            .expect_err("committed history must disable privacy genesis authority");
        assert!(matches!(
            error,
            Error::InvariantViolation(ref message)
                if message.as_ref() == "not permitted: CanEnactGovernance"
        ));
    }
}

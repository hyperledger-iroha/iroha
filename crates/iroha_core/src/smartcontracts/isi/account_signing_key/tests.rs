//! Native execution of off-chain signing key installs, rotations and revocations.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, MultisigMember, MultisigPolicy},
    isi::{InstructionBox, error::InstructionExecutionError},
    prelude::BlockHeader,
    runtime::lifecycle::RuntimeRejectionV1,
};
use std::num::NonZeroU64;

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key")
}

fn account(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}

fn multisig() -> AccountId {
    AccountId::new_multisig(
        MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(key(21).public_key().clone(), 1).unwrap(),
                MultisigMember::new(key(22).public_key().clone(), 1).unwrap(),
            ],
        )
        .unwrap(),
    )
}

/// Registered accounts: `account(1)`, `account(2)`, `account(3)` and the 2-of-2 [`multisig`].
fn state() -> State {
    let owner = account(1);
    let accounts = [account(1), account(2), account(3), multisig()]
        .into_iter()
        .map(|id| Account::new(id).build(&owner));
    State::new_for_testing(
        World::with([], accounts, []),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0)
}

fn install(account: &AccountId, seed: u8) -> SetAccountSigningKeyV1 {
    SetAccountSigningKeyV1::install(account.clone(), key(seed).public_key().clone())
}

fn refusal(result: Result<(), Error>) -> String {
    match result {
        Err(Error::InvariantViolation(reason)) => reason.into(),
        other => panic!("expected an invariant refusal, got {other:?}"),
    }
}

fn reservation(tx: &StateTransaction<'_, '_>, seed: u8) -> Option<AccountSigningKeyReservationV1> {
    tx.world
        .account_signing_key_reservations
        .get(&account(seed))
        .cloned()
}

#[test]
fn install_rotate_and_revoke_raise_the_generation_and_keep_old_keys_reserved() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let alice = account(1);

    install(&alice, 11).execute(&alice, &mut tx).unwrap();
    assert_eq!(
        account_signing_key(&tx.world, &alice),
        Some(AccountSigningKeyV1 {
            public_key: key(11).public_key().clone(),
            generation: 1,
        })
    );

    install(&alice, 12).execute(&alice, &mut tx).unwrap();
    assert_eq!(
        account_signing_key(&tx.world, &alice),
        Some(AccountSigningKeyV1 {
            public_key: key(12).public_key().clone(),
            generation: 2,
        })
    );
    assert_eq!(
        reservation(&tx, 11),
        Some(AccountSigningKeyReservationV1 {
            account: alice.clone(),
            generation: 1,
            active: false,
        })
    );
    assert_eq!(
        reservation(&tx, 12),
        Some(AccountSigningKeyReservationV1 {
            account: alice.clone(),
            generation: 2,
            active: true,
        })
    );

    SetAccountSigningKeyV1::revoke(alice.clone())
        .execute(&alice, &mut tx)
        .unwrap();
    assert_eq!(account_signing_key(&tx.world, &alice), None);
    assert_eq!(
        tx.world.account_signing_keys.get(&alice),
        Some(&AccountSigningKeyRecordV1 {
            generation: 3,
            public_key: None,
        })
    );
    assert_eq!(reservation(&tx, 12).map(|r| r.active), Some(false));

    for retired in [11, 12] {
        assert!(
            refusal(install(&alice, retired).execute(&alice, &mut tx))
                .contains("cannot be installed again"),
            "a retired key stays reserved"
        );
    }
    install(&alice, 13).execute(&alice, &mut tx).unwrap();
    assert_eq!(
        account_signing_key(&tx.world, &alice).map(|key| key.generation),
        Some(4)
    );
}

#[test]
fn only_the_account_itself_changes_its_signing_key() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let (alice, bob) = (account(1), account(2));
    assert!(
        refusal(install(&alice, 11).execute(&bob, &mut tx)).contains("only the account itself")
    );
    install(&alice, 11).execute(&alice, &mut tx).unwrap();
    assert!(
        refusal(SetAccountSigningKeyV1::revoke(alice.clone()).execute(&bob, &mut tx))
            .contains("only the account itself")
    );
    assert_eq!(
        account_signing_key(&tx.world, &alice).map(|key| key.generation),
        Some(1)
    );
}

#[test]
fn a_controller_key_never_becomes_a_signing_key() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let alice = account(1);
    assert!(
        refusal(install(&alice, 1).execute(&alice, &mut tx))
            .contains("member of the account's controller"),
        "the account's own transaction key"
    );
    assert!(
        refusal(install(&alice, 3).execute(&alice, &mut tx)).contains("controls an account"),
        "another registered account's transaction key"
    );
    let shared = multisig();
    for member in [21, 22] {
        assert!(
            refusal(install(&shared, member).execute(&shared, &mut tx))
                .contains("member of the account's controller"),
            "a member of the account's multisig policy"
        );
    }
    install(&shared, 23).execute(&shared, &mut tx).unwrap();
    assert_eq!(
        account_signing_key(&tx.world, &shared).map(|key| key.public_key),
        Some(key(23).public_key().clone())
    );
    assert!(tx.world.account_signing_keys.get(&alice).is_none());
}

#[test]
fn a_key_is_the_signing_key_of_one_account_only() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let (alice, bob) = (account(1), account(2));
    install(&alice, 11).execute(&alice, &mut tx).unwrap();
    assert!(
        refusal(install(&bob, 11).execute(&bob, &mut tx)).contains("cannot be installed again")
    );
    assert!(
        refusal(install(&alice, 11).execute(&alice, &mut tx)).contains("cannot be installed again"),
        "reinstalling the active key is a refusal, not a silent generation bump"
    );
    assert_eq!(account_signing_key(&tx.world, &bob), None);
}

#[test]
fn revoking_nothing_and_unregistered_accounts_are_refused() {
    let state = state();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let alice = account(1);
    assert!(
        refusal(SetAccountSigningKeyV1::revoke(alice.clone()).execute(&alice, &mut tx))
            .contains("no off-chain signing key to revoke")
    );
    let stranger = account(9);
    assert!(
        refusal(install(&stranger, 11).execute(&stranger, &mut tx))
            .contains("needs a registered account")
    );
    assert!(
        tx.world
            .account_signing_key_reservations
            .iter()
            .next()
            .is_none()
    );
}

#[test]
fn the_instruction_resolves_only_once_the_addition_activates() {
    use crate::runtime::catalog::additions::{ACCOUNT_SIGNING_KEYS, SORAFS_REPAIR};
    let instruction = InstructionBox::from(install(&account(1), 11));
    let baseline = World::new();
    assert_eq!(
        crate::runtime::resolve(
            &instruction,
            baseline.runtime_selection.view().get(),
            1,
            || None
        ),
        Err(InstructionExecutionError::Runtime(
            RuntimeRejectionV1::DefinitionInactive {
                wire_id: SetAccountSigningKeyV1::WIRE_ID.to_owned(),
            }
        ))
    );
    let activated = World::new();
    crate::runtime::activate_additions_for_testing(&activated, ACCOUNT_SIGNING_KEYS.module)
        .expect("activate the account signing key addition");
    assert!(
        activated
            .runtime_selection
            .view()
            .get()
            .bindings
            .iter()
            .all(|binding| !SORAFS_REPAIR
                .instructions
                .contains(&binding.wire_id.as_str())),
        "activating account signing keys must not activate the Taira-only repair"
    );
    assert!(matches!(
        crate::runtime::resolve(
            &instruction,
            activated.runtime_selection.view().get(),
            1,
            || None
        ),
        Ok(Some(_))
    ));
}

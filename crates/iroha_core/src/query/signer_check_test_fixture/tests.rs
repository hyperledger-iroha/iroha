//! Public test facade retains exact native outcomes and its own real finalized parent chain.
use super::*;
use crate::query::{
    signer_custody_history::{AccountPurpose, CustodyPurpose, ReceiptPurpose, control_head_key},
    signer_finality::verify_signer_finality_v1,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Level, Registrable,
    account::{Account, AccountId},
    isi::{InstructionBox, Log, Register, sorafs::MutateSorafsFinalPromotionAccountCustody},
    sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyActionV1,
    transaction::{FeePaymentIntent, TransactionBuilder, TransactionEntrypoint},
};
use std::{
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};

fn key() -> KeyPair {
    KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::Ed25519).unwrap()
}

#[test]
fn final_promotion_setup_executes_scoped_grants_without_native_history() {
    use crate::state::WorldReadOnly;
    use iroha_executor_data_model::permission::sorafs::{
        CanCheckSorafsFinalPromotion, CanCheckSorafsFinalPromotionAccountCustody,
        CanManageSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionCustody,
        CanOperateSorafsFinalPromotion,
    };
    let account = |seed| {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    };
    let (manager, operator, observer) = (account(1), account(2), account(3));
    let fixture = NativeCheckTestFixtureV1::with_final_promotion_accounts(
        "promotion-primary",
        manager.clone(),
        operator.clone(),
        observer.clone(),
    );
    assert!(fixture.finalized_floor().is_none());
    let view = fixture.state().view();
    assert_eq!(view.height(), 0);
    assert!(view.block_hashes().is_empty());
    assert!(fixture.finalized.is_empty());
    // State startup seeds the three SNS namespace policies even for an empty World.
    // Registration and grants must not create custody, Check or operation history.
    let keys = view
        .world
        .smart_contract_state
        .iter()
        .map(|(key, _)| key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_keys = [
        crate::sns::SnsNamespace::AccountAlias,
        crate::sns::SnsNamespace::Domain,
        crate::sns::SnsNamespace::Dataspace,
    ]
    .map(|namespace| crate::sns::policy_storage_key(namespace.suffix_id()))
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys, expected_keys);
    for namespace in [ReceiptPurpose::NAMESPACE, AccountPurpose::NAMESPACE] {
        assert!(
            keys.iter().all(|key| !key.as_ref().starts_with(namespace)),
            "setup must not seed any row in {namespace}"
        );
    }
    assert_eq!(view.world.accounts.iter().count(), 3);
    assert_eq!(view.world.account_permissions.iter().count(), 3);
    let expected: [(AccountId, Vec<iroha_data_model::permission::Permission>); 3] = [
        (
            manager,
            vec![
                CanManageSorafsFinalPromotionCustody {
                    deployment_id: "promotion-primary".into(),
                }
                .into(),
                CanManageSorafsFinalPromotionAccountCustody {
                    deployment_id: "promotion-primary".into(),
                }
                .into(),
            ],
        ),
        (
            operator,
            vec![
                CanOperateSorafsFinalPromotion {
                    deployment_id: "promotion-primary".into(),
                }
                .into(),
            ],
        ),
        (
            observer,
            vec![
                CanCheckSorafsFinalPromotion {
                    deployment_id: "promotion-primary".into(),
                }
                .into(),
                CanCheckSorafsFinalPromotionAccountCustody {
                    deployment_id: "promotion-primary".into(),
                }
                .into(),
            ],
        ),
    ];
    for (account, grants) in expected {
        assert!(view.world.accounts.get(&account).is_some());
        assert_eq!(
            view.world.account_permissions.get(&account).unwrap().len(),
            grants.len()
        );
        for permission in grants {
            assert!(
                view.world
                    .account_contains_inherent_permission(&account, &permission)
            );
        }
    }
}
fn world() -> World {
    let authority = AccountId::new(key().public_key().clone());
    let (id, account) = Account::new(authority.clone())
        .build(&authority)
        .into_key_value();
    let mut world = World::new();
    world.accounts.insert(id, account);
    world
}
fn signed(fixture: &NativeCheckTestFixtureV1, instruction: InstructionBox) -> SignedTransaction {
    let mut builder = TransactionBuilder::new(
        *fixture.state().network_id_ref(),
        AccountId::new(key().public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(1_000));
    builder
        .with_instructions([instruction])
        .try_sign(key().private_key())
        .unwrap()
}

#[test]
fn test_facade_retains_exact_executed_results_membership_and_real_finalized_parents() {
    let mut fixture = NativeCheckTestFixtureV1::new(world());
    assert!(fixture.finalized_floor().is_none());
    let state = Arc::clone(fixture.state());
    let success = signed(
        &fixture,
        Log::new(Level::INFO, "native fixture".into()).into(),
    );
    let failure = signed(
        &fixture,
        MutateSorafsFinalPromotionAccountCustody {
            deployment_id: "promotion-primary".into(),
            expected_control_revision: 0,
            expected_control_digest: [0; 32],
            action: FinalPromotionAccountCustodyActionV1::Configure(Vec::new()),
        }
        .into(),
    );
    let transactions = [success, failure];
    assert_eq!(fixture.commit(1_000, transactions.to_vec()), [true, false]);
    assert!(Arc::ptr_eq(&state, fixture.state()));
    let first = fixture.finalized_floor().unwrap();
    assert_eq!(first.0, 1);
    let block = state
        .block_by_height(NonZeroUsize::new(1).unwrap())
        .unwrap();
    assert_eq!(block.committed_fragment_count(), Some(1));
    block.validate_output_merkle_cache().unwrap();
    for (index, signed) in transactions.into_iter().enumerate() {
        assert!(state.has_committed_entrypoint(signed.hash_as_entrypoint()));
        assert_eq!(
            block.network_entrypoint_at(index),
            Some(&TransactionEntrypoint::External(signed))
        );
        let (_, output) = block.network_output_at(index.try_into().unwrap()).unwrap();
        assert_eq!(output.input_index as usize, index);
        assert_eq!(output.result.is_ok(), index == 0);
        assert!(output.completions.is_empty());
    }
    let finalized = &fixture.finalized[0];
    assert_eq!(
        finalized
            .proof()
            .finality_artifact
            .height_context
            .roster
            .len(),
        4
    );
    assert_eq!(
        finalized.proof().finality_artifact.commit_qc.signers.len(),
        3
    );
    finalized.proof().finality_artifact.verify().unwrap();
    verify_signer_finality_v1(&state.view(), first.0, first.1).unwrap();
    assert!(fixture.commit(2_000, Vec::new()).is_empty());
    let second = fixture.finalized_floor().unwrap();
    assert_eq!(second.0, 2);
    assert_ne!(second.1, first.1);
    assert_eq!(
        fixture.finalized[1].block().header().prev_block_hash(),
        Some(block.hash())
    );
    verify_signer_finality_v1(&state.view(), second.0, second.1).unwrap();
}

#[test]
fn test_facade_rejects_unsupported_execution_before_any_native_publication() {
    let mut fixture = NativeCheckTestFixtureV1::new(world());
    let other = AccountId::new(
        KeyPair::try_from_seed(vec![0xA2; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let unsupported = signed(
        &fixture,
        Register::account(Account::new(other.clone())).into(),
    );
    let before = signed(
        &fixture,
        Log::new(Level::INFO, "first source".into()).into(),
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            fixture.commit(1_000, vec![before.clone(), unsupported]);
        }))
        .is_err()
    );
    assert!(fixture.finalized_floor().is_none());
    assert_eq!(fixture.state().committed_height(), 0);
    assert!(
        !fixture
            .state()
            .has_committed_entrypoint(before.hash_as_entrypoint())
    );
    assert!(fixture.state().view().world.accounts.get(&other).is_none());
    assert_eq!(fixture.commit(1_000, vec![before]), [true]);
}

#[test]
fn test_facade_rejects_preseeded_history_and_foreign_network_before_publication() {
    let mut seeded = world();
    seeded.smart_contract_state.insert(
        control_head_key::<AccountPurpose>("promotion-primary").unwrap(),
        vec![1],
    );
    assert!(catch_unwind(AssertUnwindSafe(|| NativeCheckTestFixtureV1::new(seeded))).is_err());
    let mut fixture = NativeCheckTestFixtureV1::new(world());
    let original = signed(
        &fixture,
        Log::new(Level::INFO, "network fixture".into()).into(),
    );
    let mut payload = original.payload().clone();
    payload.domain = iroha_data_model::transaction::TransactionDomain::Genesis;
    let foreign = TransactionBuilder::from_genesis_payload(payload)
        .unwrap()
        .try_sign(key().private_key())
        .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| fixture.commit(1_000, vec![foreign]))).is_err());
    assert!(fixture.finalized_floor().is_none());
    assert_eq!(fixture.state().committed_height(), 0);
    assert_eq!(fixture.commit(1_000, vec![original]), [true]);
}

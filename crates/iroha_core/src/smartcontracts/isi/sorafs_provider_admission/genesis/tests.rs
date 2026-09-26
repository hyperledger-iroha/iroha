//! Actual genesis-domain signatures, derived network identity and durable native admission.
use super::*;
use crate::{
    kura::Kura,
    query::{signer_check::fixture as execution, store::LiveQueryStore},
    smartcontracts::isi::sorafs_provider_admission::test_fixture::{
        NOW, ProviderAdmissionTestFixtureV1 as Fixture, key,
    },
    state::{State, World},
};
use iroha_data_model::{
    IntoKeyValue, NetworkId, Registrable,
    account::Account,
    block::{BlockHeader, builder::BlockBuilder},
    isi::InstructionBox,
    sorafs::provider_admission::governance::{
        InitialProviderAdmissionCouncilV1, InitialProviderAdmissionV1,
    },
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use std::{sync::Arc, time::Duration};

fn initializer() -> InitializeSorafsProviderAdmissionV1 {
    let fixture = Fixture::new();
    InitializeSorafsProviderAdmissionV1 {
        council: InitialProviderAdmissionCouncilV1 {
            policy_id: fixture.policy.policy_id,
            trusted_signers: fixture.policy.trusted_signers.clone(),
            signature_threshold: 1,
        },
        providers: vec![InitialProviderAdmissionV1 {
            owner: AccountId::new(key(1).public_key().clone()),
            material: norito::encode_canonical(&ProviderAdmissionGenesisMaterialV1 {
                proposal: fixture.envelope.proposal.clone(),
                advert_body: fixture.envelope.advert_body.clone(),
                issued_at: NOW,
                retention_epoch: NOW + 3600,
            })
            .unwrap(),
        }],
    }
}
fn signed(
    instructions: Vec<InstructionBox>,
    network: Option<NetworkId>,
    now: u64,
) -> SignedTransaction {
    let owner = AccountId::new(key(1).public_key().clone());
    let fee = FeePaymentIntent::authority(Vec::new(), None);
    let mut builder = match network {
        Some(network) => TransactionBuilder::new(network, owner, fee),
        None => TransactionBuilder::new_genesis(owner, fee),
    };
    builder.set_creation_time(Duration::from_millis(now));
    builder
        .with_instructions(instructions)
        .try_sign(key(1).private_key())
        .unwrap()
}
fn state_for_genesis(
    transaction: &SignedTransaction,
    existing_owner: Option<AccountId>,
) -> Arc<State> {
    let mut builder = BlockBuilder::new(BlockHeader::new(
        1.try_into().unwrap(),
        None,
        None,
        NOW * 1000,
        0,
    ));
    builder.push_transaction(transaction.clone());
    let signed = builder
        .try_build_with_signature(0, key(0xfe).private_key())
        .unwrap();
    let network = NetworkId::from_genesis_hash(signed.hash());
    let owner = AccountId::new(key(1).public_key().clone());
    let mut world = World::new();
    let (id, account) = Account::new(owner.clone()).build(&owner).into_key_value();
    world.accounts.insert(id, account);
    if let Some(owner) = existing_owner {
        world
            .provider_owners
            .insert(Fixture::new().provider(), owner);
    }
    Arc::new(State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "genesis-admission-test".parse().unwrap(),
        network,
    ))
}

#[test]
fn signed_genesis_templates_bind_the_actual_genesis_hash_and_canonical_journal() {
    for existing in [false, true] {
        let initialize = initializer();
        let owner = initialize.providers[0].owner.clone();
        let transaction = signed(vec![initialize.clone().into()], None, NOW * 1000);
        assert!(transaction.network_id().is_none());
        let state = state_for_genesis(&transaction, existing.then_some(owner));
        let mut blocks = Vec::new();
        assert_eq!(
            execution::commit_genesis_admission(
                &state,
                &mut blocks,
                NOW * 1000,
                vec![transaction],
                true
            ),
            [true]
        );
        assert_eq!(
            state.network_id_ref().as_bytes(),
            blocks[0].block().hash().as_ref()
        );
        let provider = Fixture::new().provider();
        let actual = native::read_finalized_provider_admission_v1(&state.view(), provider, NOW)
            .unwrap()
            .unwrap();
        assert!(actual.is_genesis_material());
        assert!(!actual.is_council_verified());
        assert!(actual.envelope().council_signatures.is_empty());
        assert_eq!(
            actual.envelope().network_id,
            *state.network_id_ref().as_bytes()
        );
        assert!(
            sorafs_manifest::provider_admission::verify_envelope_untrusted_signers(
                actual.envelope()
            )
            .is_err()
        );
        assert_eq!(
            native::retained_provider_count_v1(&state.view()).unwrap(),
            1
        );
        let original = native::read_head(state.view().world(), Some(provider)).unwrap();
        let replay = signed(
            vec![initialize.into()],
            Some(*state.network_id_ref()),
            (NOW + 1) * 1000,
        );
        assert_eq!(
            execution::commit_genesis_admission(
                &state,
                &mut blocks,
                (NOW + 1) * 1000,
                vec![replay],
                true
            ),
            [false]
        );
        assert_eq!(
            native::read_head(state.view().world(), Some(provider)).unwrap(),
            original
        );
    }
}
#[test]
fn finalized_genesis_admission_expiry_rejects_a_lagging_local_clock() {
    let mut initialize = initializer();
    let mut material: ProviderAdmissionGenesisMaterialV1 =
        decode_frame(&initialize.providers[0].material).unwrap();
    material.retention_epoch = NOW + 2;
    initialize.providers[0].material = norito::encode_canonical(&material).unwrap();
    let transaction = signed(vec![initialize.into()], None, NOW * 1000);
    let state = state_for_genesis(&transaction, None);
    let mut blocks = Vec::new();
    assert_eq!(
        execution::commit_genesis_admission(
            &state,
            &mut blocks,
            NOW * 1000,
            vec![transaction],
            true
        ),
        [true]
    );
    let provider = Fixture::new().provider();
    assert!(
        native::read_finalized_provider_admission_v1(&state.view(), provider, NOW - 1).is_err()
    );
    for (offset, valid) in [(1, true), (2, false)] {
        assert!(
            execution::commit_genesis_admission(
                &state,
                &mut blocks,
                (NOW + offset) * 1000,
                Vec::new(),
                true
            )
            .is_empty()
        );
        let result = native::read_finalized_provider_admission_v1(&state.view(), provider, NOW);
        if valid {
            assert!(result.unwrap().is_some());
        } else {
            assert!(
                result.is_err(),
                "signed-genesis material cannot outlive finalized expiry"
            );
        }
    }
}

#[test]
fn applied_genesis_without_its_exact_durable_qc_cannot_admit() {
    let transaction = signed(vec![initializer().into()], None, NOW * 1000);
    let state = state_for_genesis(&transaction, None);
    let mut blocks = Vec::new();
    assert_eq!(
        execution::commit_genesis_admission(
            &state,
            &mut blocks,
            NOW * 1000,
            vec![transaction],
            false
        ),
        [true]
    );
    assert!(
        native::read_finalized_provider_admission_v1(&state.view(), Fixture::new().provider(), NOW)
            .is_err()
    );
    state
        .kura()
        .store_v2_finality_artifact(&blocks[0].proof().finality_artifact)
        .unwrap();
    assert!(
        native::read_finalized_provider_admission_v1(&state.view(), Fixture::new().provider(), NOW)
            .unwrap()
            .is_some()
    );
}
#[test]
fn malformed_templates_and_owner_conflicts_leave_no_genesis_journal() {
    for malformed in 0..6 {
        let mut initialize = initializer();
        let mut existing = None;
        match malformed {
            0 => initialize.providers.push(initialize.providers[0].clone()),
            1 => initialize.providers[0].owner = AccountId::new(key(2).public_key().clone()),
            2 => initialize.council.signature_threshold = 2,
            3 => initialize.providers[0].material.push(0),
            4 => existing = Some(AccountId::new(key(2).public_key().clone())),
            5 => {
                let mut material: ProviderAdmissionGenesisMaterialV1 =
                    decode_frame(&initialize.providers[0].material).unwrap();
                material.retention_epoch = NOW;
                initialize.providers[0].material = norito::encode_canonical(&material).unwrap();
            }
            _ => unreachable!(),
        }
        let transaction = signed(vec![initialize.into()], None, NOW * 1000);
        let state = state_for_genesis(&transaction, existing);
        assert_eq!(
            execution::commit_genesis_admission(
                &state,
                &mut Vec::new(),
                NOW * 1000,
                vec![transaction],
                true
            ),
            [false]
        );
        assert!(
            native::read_head(state.view().world(), None)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn second_initializer_in_one_genesis_transaction_rolls_back_the_first() {
    let initialize = initializer();
    let transaction = signed(
        vec![initialize.clone().into(), initialize.into()],
        None,
        NOW * 1000,
    );
    let state = state_for_genesis(&transaction, None);
    assert_eq!(
        execution::commit_genesis_admission(
            &state,
            &mut Vec::new(),
            NOW * 1000,
            vec![transaction],
            true
        ),
        [false]
    );
    assert!(
        native::read_head(state.view().world(), None)
            .unwrap()
            .is_none()
    );
}
#[test]
fn indirect_or_network_scoped_execution_cannot_acquire_genesis_authority() {
    let initialize = initializer();
    let transaction = signed(vec![initialize.clone().into()], None, NOW * 1000);
    let state = state_for_genesis(&transaction, None);
    let mut block = state.block(BlockHeader::new(
        1.try_into().unwrap(),
        None,
        None,
        NOW * 1000,
        0,
    ));
    let mut tx = block.transaction();
    tx.current_network_entrypoint_hash = Some(transaction.hash_as_entrypoint());
    tx.current_tx_hash = Some(transaction.hash());
    tx.tx_call_hash = Some(iroha_crypto::Hash::from(transaction.hash_as_entrypoint()));
    tx.current_entrypoint_index = Some(0);
    let boxed = initialize.clone().into();
    assert!(
        !crate::executor::Executor::direct_sorafs_admission_initialization(
            &tx,
            &transaction,
            &boxed,
            0,
            false
        )
    );
    assert!(
        !crate::executor::Executor::direct_sorafs_admission_initialization(
            &tx,
            &transaction,
            &boxed,
            1,
            true
        )
    );
    assert!(
        crate::executor::Executor::Initial
            .execute_instruction(&mut tx, transaction.authority(), boxed)
            .is_err()
    );
    assert!(native::read_head(tx.world(), None).unwrap().is_none());
    drop(tx);
    drop(block);
    let ordinary = signed(
        vec![initialize.into()],
        Some(*state.network_id_ref()),
        NOW * 1000,
    );
    let state = state_for_genesis(&ordinary, None);
    assert_eq!(
        execution::commit_genesis_admission(
            &state,
            &mut Vec::new(),
            NOW * 1000,
            vec![ordinary],
            true
        ),
        [false]
    );
}

//! Actual TxOverlay admission, executor authorization and execution of atomic settlements.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateBlock, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::Account,
    asset::{
        Asset, AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId, AssetId,
    },
    domain::Domain,
    isi::{
        AtomicSettlementMovement, AtomicSettlementMovements, Grant, Instruction, SettleAtomic,
        SettlementDetails,
    },
    permission::Permission,
};
use iroha_executor_data_model::permission::settlement::CanExecuteSettlement;
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
use iroha_primitives::numeric::Quantity;
use nonzero_ext::nonzero;

fn owner(index: u16) -> AccountId {
    let mut seed = vec![0xB7; 32];
    seed[..2].copy_from_slice(&index.to_le_bytes());
    AccountId::new(
        KeyPair::try_from_seed(seed, Algorithm::Ed25519)
            .expect("deterministic owner")
            .public_key()
            .clone(),
    )
}

fn fixture(count: usize, final_scope_mismatch: bool) -> (State, SettleAtomic, AccountId) {
    let sponsor = owner(u16::MAX);
    let domain_id = DomainId::try_new("atomic_overlay", "universal").expect("domain");
    let definition =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "cash".parse().expect("name"));
    let mut movements = (0..count)
        .map(|index| AtomicSettlementMovement {
            source: AssetId::with_scope(
                definition.clone(),
                owner(index as u16),
                AssetBalanceScope::Global,
            ),
            recipient: sponsor.clone(),
            quantity: Quantity::from(index as u64 + 42),
        })
        .collect::<Vec<_>>();
    movements.sort_by(|a, b| (&a.source, &a.recipient).cmp(&(&b.source, &b.recipient)));
    if final_scope_mismatch {
        let last = movements.last_mut().expect("at least three payments");
        last.source = AssetId::with_scope(
            definition.clone(),
            last.source.account().clone(),
            AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
        );
        // AssetId orders account first. The final signed movement remains last,
        // so the failure follows preparation of every earlier valid movement.
        assert!(
            movements
                .windows(2)
                .all(|pair| (&pair[0].source, &pair[0].recipient)
                    < (&pair[1].source, &pair[1].recipient))
        );
    }
    let accounts = std::iter::once(Account::new(sponsor.clone()).build(&sponsor))
        .chain(
            movements
                .iter()
                .map(|movement| Account::new(movement.source.account().clone()).build(&sponsor)),
        )
        .collect::<Vec<_>>();
    let assets = movements
        .iter()
        .map(|movement| Asset::new(movement.source.clone(), Quantity::from(1000_u32)))
        .collect::<Vec<_>>();
    let world = World::with_assets(
        [Domain::new(domain_id).build(&sponsor)],
        accounts,
        [AssetDefinition::numeric(
            definition,
            "cash".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&sponsor)],
        assets,
        [],
    );
    let state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let instruction = SettleAtomic::new(
        state.network_id.clone(),
        "overlay_atomic_business".parse().expect("id"),
        AtomicSettlementMovements::try_from(movements).expect("canonical full vector"),
        nonzero!(100_u64),
        Metadata::default(),
    );
    (state, instruction, sponsor)
}

fn permission(instruction: &SettleAtomic, source: &AssetId) -> Permission {
    CanExecuteSettlement {
        debited_asset: source.clone(),
        settlement_id: instruction.settlement_id().clone(),
        intent_hash: instruction.intent_hash().expect("full intent"),
    }
    .into()
}

fn grant_consents(
    block: &mut StateBlock<'_>,
    instruction: &SettleAtomic,
    sponsor: &AccountId,
    omit: Option<usize>,
) {
    let intent_hash = instruction
        .intent_hash()
        .expect("full intent once per grant set");
    for (index, movement) in instruction.movements().as_slice().iter().enumerate() {
        if Some(index) == omit {
            continue;
        }
        let mut grant_tx = block.transaction();
        assert!(matches!(
            grant_tx.world.executor.clone(),
            crate::executor::Executor::Initial
        ));
        assert!(
            !crate::executor::is_initial_genesis_context(&grant_tx),
            "actual issuer policy must execute without genesis exceptions"
        );
        grant_tx.tx_call_hash = Some(Hash::new(index.to_le_bytes()));
        let permission: Permission = CanExecuteSettlement {
            debited_asset: movement.source.clone(),
            settlement_id: instruction.settlement_id().clone(),
            intent_hash,
        }
        .into();
        TxOverlay::from_instructions(vec![
            Grant::account_permission(permission, sponsor.clone()).into(),
        ])
        .apply(&mut grant_tx, movement.source.account())
        .expect("runtime executor accepts exact owner-issued consent");
        assert_eq!(grant_tx.pending_transfer_transcript_count_for_testing(), 0);
        grant_tx.apply();
    }
}

fn overlay(instruction: SettleAtomic, direct: bool) -> TxOverlay {
    let boxed = if direct {
        Box::new(instruction).into_instruction_box()
    } else {
        SettlementInstructionBox::Atomic(instruction).into()
    };
    assert_eq!(boxed.as_any().is::<SettleAtomic>(), direct);
    assert_eq!(boxed.as_any().is::<SettlementInstructionBox>(), !direct);
    TxOverlay::from_instructions(vec![boxed])
}

fn observable(state_tx: &StateTransaction<'_, '_>) -> (Vec<Vec<u8>>, usize) {
    macro_rules! capture {
        ($field:ident) => {
            norito::encode_canonical(
                &state_tx
                    .world
                    .$field
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<Vec<_>>(),
            )
            .expect("canonical observable state")
        };
    }
    let assets = state_tx
        .world
        .assets
        .iter()
        .map(|(key, value)| (key.clone(), value.as_ref().clone()))
        .collect::<Vec<_>>();
    let events = state_tx
        .world
        .internal_event_buf
        .iter()
        .map(|event| norito::encode_canonical(event.as_ref()).expect("event bytes"))
        .collect::<Vec<_>>();
    (
        vec![
            norito::encode_canonical(&assets).expect("balances"),
            capture!(accounts),
            capture!(account_permissions),
            capture!(asset_definition_assets),
            capture!(assets_by_account),
            capture!(assets_by_domain),
            capture!(asset_definition_nonzero_holders),
            capture!(settlement_receipts),
            norito::encode_canonical(&events).expect("event inventory"),
        ],
        state_tx.pending_transfer_transcript_count_for_testing(),
    )
}

#[test]
fn atomic_overlay_direct_and_boxed_execute_exact_owner_consents() {
    for direct in [false, true] {
        for count in [3, 255] {
            let (state, instruction, sponsor) = fixture(count, false);
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
            grant_consents(&mut block, &instruction, &sponsor, None);
            let mut state_tx = block.transaction();
            state_tx.tx_call_hash = Some(Hash::new(b"atomic-overlay-carrier"));
            assert_eq!(state_tx.pending_transfer_transcript_count_for_testing(), 0);
            let event_count = state_tx.world.internal_event_buf.len();
            overlay(instruction.clone(), direct)
                .apply(&mut state_tx, &sponsor)
                .expect("actual TxOverlay admission and Initial execution");
            assert_eq!(state_tx.pending_transfer_transcript_count_for_testing(), 1);
            assert!(state_tx.world.internal_event_buf.len() > event_count);
            let mut incoming = Quantity::zero();
            for movement in instruction.movements().as_slice() {
                incoming = incoming
                    .checked_add(&movement.quantity)
                    .expect("bounded sum");
                assert_eq!(
                    state_tx
                        .world
                        .assets
                        .get(&movement.source)
                        .expect("source remains")
                        .as_ref(),
                    &Quantity::from(1000_u32)
                        .checked_sub(&movement.quantity)
                        .expect("prefunded")
                );
            }
            let destination = instruction.movements().as_slice()[0].destination();
            assert_eq!(
                state_tx
                    .world
                    .assets
                    .get(&destination)
                    .expect("all credits")
                    .as_ref(),
                &incoming
            );
            let receipt = state_tx
                .world
                .settlement_receipts
                .get(instruction.settlement_id())
                .expect("one full receipt");
            assert_eq!(receipt.authority, sponsor);
            let SettlementDetails::Atomic(iroha_data_model::isi::AtomicSettlementDetails {
                movements,
                intent_hash,
            }) = &receipt.details
            else {
                panic!("atomic receipt");
            };
            assert_eq!(
                movements,
                &instruction.movements().resolve().expect("exact scopes")
            );
            assert_eq!(
                *intent_hash,
                instruction.intent_hash().expect("full intent")
            );
        }
    }
}

#[test]
fn atomic_overlay_final_missing_consent_rejects_before_any_movement() {
    for direct in [false, true] {
        let (state, instruction, sponsor) = fixture(255, false);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        grant_consents(&mut block, &instruction, &sponsor, Some(254));
        let mut state_tx = block.transaction();
        state_tx.tx_call_hash = Some(Hash::new(b"missing-final-consent"));
        let before = observable(&state_tx);
        assert_eq!(
            before.1, 0,
            "empty transcript inventory is exact, not a same-count approximation"
        );
        let error = overlay(instruction, direct)
            .apply(&mut state_tx, &sponsor)
            .expect_err("last owner consent is mandatory at actual overlay admission");
        assert!(
            matches!(&error, ValidationFail::InstructionFailed(iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message)) if message.contains("whole-intent consent")),
            "{error}"
        );
        assert_eq!(observable(&state_tx), before);
        assert!(state_tx.sccp_ivm_proved_execution_binding.is_none());
    }
}

#[test]
fn atomic_overlay_final_scope_policy_mismatch_rejects_without_partial_execution() {
    for direct in [false, true] {
        let (state, instruction, sponsor) = fixture(255, true);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        grant_consents(&mut block, &instruction, &sponsor, None);
        let mut state_tx = block.transaction();
        state_tx.tx_call_hash = Some(Hash::new(b"final-scope-policy"));
        let before = observable(&state_tx);
        assert_eq!(before.1, 0);
        let error = overlay(instruction, direct)
            .apply(&mut state_tx, &sponsor)
            .expect_err("final exact signed bucket violates the live definition policy");
        assert!(
            matches!(&error, ValidationFail::InstructionFailed(iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message)) if message.contains("global asset definition requires the global public balance scope")),
            "{error}"
        );
        assert_eq!(observable(&state_tx), before);
        assert!(state_tx.sccp_ivm_proved_execution_binding.is_none());
    }
}

#[test]
fn atomic_overlay_nonowner_cannot_issue_the_final_owner_consent() {
    let (state, instruction, sponsor) = fixture(3, false);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut state_tx = block.transaction();
    assert!(!crate::executor::is_initial_genesis_context(&state_tx));
    let source = &instruction.movements().as_slice()[2].source;
    let grant = TxOverlay::from_instructions(vec![
        Grant::account_permission(permission(&instruction, source), sponsor.clone()).into(),
    ]);
    let before = observable(&state_tx);
    assert_eq!(before.1, 0);
    let error = grant
        .apply(&mut state_tx, &sponsor)
        .expect_err("carrier cannot grant itself a foreign owner's exact consent");
    assert!(matches!(error, ValidationFail::NotPermitted(_)));
    assert_eq!(observable(&state_tx), before);
}

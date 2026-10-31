//! Executor migration cannot invalidate live escrow settlement authority.

use super::*;
use crate::executor::Executor;
use crate::smartcontracts::Execute;
use iroha_data_model::escrow::{AssetEscrowKind, AssetEscrowRecord, AssetEscrowStatus, EscrowId};

fn escrow(status: AssetEscrowStatus) -> AssetEscrowRecord {
    let live = crate::runtime::obligations::is_live(status);
    AssetEscrowRecord {
        id: EscrowId(Hash::prehashed([0x91; 32])),
        seller: ALICE_ID.clone(),
        buyer: Some(BOB_ID.clone()),
        asset_definition: iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .unwrap(),
        amount: Quantity::one(),
        custody: ALICE_ID.clone(),
        status,
        kind: if matches!(
            status,
            AssetEscrowStatus::Locked | AssetEscrowStatus::DrawnDown | AssetEscrowStatus::Expired
        ) {
            AssetEscrowKind::Lock
        } else {
            AssetEscrowKind::Marketplace
        },
        remaining_amount: if live {
            Quantity::one()
        } else {
            Quantity::zero()
        },
        release_authority: None,
        expires_at_ms: None,
        evidence_hashes: Vec::new(),
        conditions: Vec::new(),
        created_at_ms: 1,
        accepted_at_ms: None,
        payment_sent_at_ms: None,
        disputed_at_ms: None,
        closed_at_ms: (!live).then_some(2),
        resolution: None,
    }
}

fn with_escrow(status: AssetEscrowStatus, run: impl FnOnce(&mut StateTransaction<'_, '_>)) {
    let mut world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    world.account_permissions.insert(
        ALICE_ID.clone(),
        BTreeSet::from([
            executor_permission::executor::CanUpgradeExecutor.into(),
            Permission::new("original_settlement_authority".into(), Json::new(())),
        ]),
    );
    let record = escrow(status);
    world.asset_escrows.insert(record.id, record);
    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
        ChainId::from("executor-live-custody"),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 3, 0));
    let mut tx = block.transaction();
    run(&mut tx);
}

fn replacement() -> data_model_executor::Executor {
    data_model_executor::Executor::new(IvmBytecode::from_compiled(generate_migration_program(&Ok(
        initial_executor_data_model_fallback(),
    ))))
}

#[test]
fn executor_upgrade_refuses_every_live_custody_state_before_migration() {
    for status in [
        AssetEscrowStatus::Open,
        AssetEscrowStatus::Accepted,
        AssetEscrowStatus::PaymentSent,
        AssetEscrowStatus::Disputed,
        AssetEscrowStatus::Locked,
    ] {
        with_escrow(status, |tx| {
            let permissions = tx.world.account_permissions.get(&ALICE_ID).unwrap().clone();
            let model = tx.world.executor_data_model.get().clone();
            let records: Vec<_> = tx
                .world
                .asset_escrows
                .iter()
                .map(|(id, value)| (*id, value.clone()))
                .collect();
            let before = tx.world.executor.runtime_consent_hash();
            let error = iroha_data_model::isi::Upgrade::new(replacement())
                .execute(&ALICE_ID, tx)
                .unwrap_err();
            assert!(
                matches!(error, InstructionExecutionError::InvariantViolation(ref reason) if reason.contains("live obligations")),
                "{status:?}: {error:?}"
            );
            assert_eq!(tx.world.executor.runtime_consent_hash(), before);
            assert_eq!(tx.world.executor_data_model.get(), &model);
            assert_eq!(
                tx.world.account_permissions.get(&ALICE_ID),
                Some(&permissions)
            );
            assert_eq!(
                tx.world
                    .asset_escrows
                    .iter()
                    .map(|(id, value)| (*id, value.clone()))
                    .collect::<Vec<_>>(),
                records
            );
            assert!(!tx.is_execution_deferred());
        });
    }
}

#[test]
fn executor_upgrade_runs_only_after_every_custody_obligation_has_drained() {
    for status in [
        AssetEscrowStatus::Released,
        AssetEscrowStatus::Cancelled,
        AssetEscrowStatus::Resolved,
        AssetEscrowStatus::DrawnDown,
        AssetEscrowStatus::Expired,
    ] {
        with_escrow(status, |tx| {
            iroha_data_model::isi::Upgrade::new(replacement())
                .execute(&ALICE_ID, tx)
                .expect("drained custody permits authorized migration");
            assert!(matches!(tx.world.executor.get(), Executor::UserProvided(_)));
            assert_eq!(
                tx.world
                    .asset_escrows
                    .get(&EscrowId(Hash::prehashed([0x91; 32])))
                    .unwrap()
                    .status,
                status
            );
        });
    }
}

#[test]
fn direct_and_identical_code_executor_migrations_cannot_bypass_live_custody() {
    with_escrow(AssetEscrowStatus::Locked, |tx| {
        let raw = replacement();
        let mut executor = Executor::UserProvided(LoadedExecutor::load(raw.clone()).unwrap());
        let before = executor.runtime_consent_hash();
        let model = tx.world.executor_data_model.get().clone();
        assert!(matches!(
            executor.migrate(raw, tx, &ALICE_ID),
            Err(ExecutorMigrationError::LiveCustody)
        ));
        assert_eq!(executor.runtime_consent_hash(), before);
        assert_eq!(tx.world.executor_data_model.get(), &model);
        let invalid = data_model_executor::Executor::new(IvmBytecode::from_compiled(Vec::new()));
        assert!(
            matches!(
                executor.migrate(invalid, tx, &ALICE_ID),
                Err(ExecutorMigrationError::LiveCustody)
            ),
            "custody is checked before loading untrusted replacement code"
        );
    });
}

#[test]
fn a_live_amx_participation_blocks_executor_migration() {
    use iroha_data_model::sumeragi_amx::{AmxBeginV1, AmxGlobalEntryV1, SumeragiAmxState};
    use iroha_model_base::topology::DataSpaceId;
    with_escrow(AssetEscrowStatus::Released, |tx| {
        *tx.world.sumeragi_amx.get_mut() = SumeragiAmxState {
            dataspaces: Vec::new(),
            transactions: vec![AmxGlobalEntryV1 {
                begin: AmxBeginV1 {
                    tx: [0x92; 32],
                    participants: vec![DataSpaceId::new(41), DataSpaceId::new(42)],
                    deadline: 10,
                    expires_at_ms: 1_000,
                },
                begun_at: 1,
                begun_time_ms: 1,
                original_cadence_ms: 100,
                yes: Vec::new(),
                decided: None,
            }],
        };
        let raw = replacement();
        let mut executor = Executor::UserProvided(LoadedExecutor::load(raw.clone()).unwrap());
        assert!(
            matches!(
                executor.migrate(raw, tx, &ALICE_ID),
                Err(ExecutorMigrationError::LiveCustody)
            ),
            "an undecided AMX transaction keeps the authorization it was admitted under"
        );
    });
}

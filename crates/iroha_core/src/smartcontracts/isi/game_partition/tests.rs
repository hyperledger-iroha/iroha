//! Actual native State, signed foreign finality, atomic rollback and restart-custody tests.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::decode_framed_signed_block,
    isi::{Grant, InstructionBox, Log},
};
use iroha_logger::Level;

fn world_id() -> Hash {
    Hash::new(b"native-independent-game-partition-test")
}
fn key() -> KeyPair {
    KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519)
}
fn config(label: &str) -> TestChainConfig {
    let mut config = TestChainConfig::new(World::new(), 1000);
    config.chain_id = label.parse().expect("canonical fixture chain label");
    config.clock_bootstrap_in_genesis = true;
    config.genesis_instructions = vec![
        Grant::account_permission(
            iroha_executor_data_model::permission::parameter::CanSetParameters,
            AccountId::new(config.genesis_key.public_key().clone()),
        )
        .into(),
        InstallGamePartitionV1 {
            world_id: world_id(),
            kernel_hash: ivm::game_unit_step::kernel_hash(),
        }
        .into(),
    ];
    config
}
fn commit(chain: &mut CertifiedTestChain, isi: impl Into<InstructionBox>) -> bool {
    let when = 2000 + chain.height() * 1000;
    let tx = chain.sign(&key(), vec![isi.into()], when - 1);
    let results = chain.commit_at(when, vec![tx]);
    if results != vec![true] {
        eprintln!(
            "native game instruction rejected at height {}: {:?}",
            chain.height(),
            chain
                .committed(chain.height())
                .block()
                .network_output_at(0)
                .map(|(_, output)| &output.result)
        );
    }
    results == vec![true]
}
fn start(label: &str) -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(config(label)).unwrap();
    assert!(commit(
        &mut chain,
        Log::new(Level::INFO, "authenticate original genesis".to_string())
    ));
    chain
}
fn registration(source: &CertifiedTestChain, label: &str) -> RegisterGamePartitionPeerV1 {
    RegisterGamePartitionPeerV1 {
        world_id: world_id(),
        chain_id: label.parse().expect("canonical fixture chain label"),
        network_id: source.network_id(),
        genesis: source.committed(1).block().encode_wire().unwrap(),
        successor: source.committed(2).block().encode_wire().unwrap(),
        endpoints: vec!["http://127.0.0.1:12345".into()],
    }
}
fn snapshot(chain: &CertifiedTestChain) -> GamePartitionStateV1 {
    game_partition_snapshot(&chain.state().view(), world_id())
        .unwrap()
        .unwrap()
}
fn seed(chain: &mut CertifiedTestChain, nonce: u64) -> Hash {
    assert!(commit(
        chain,
        SeedGameUnitV1 {
            world_id: world_id(),
            nonce,
            owner: AccountId::new(key().public_key().clone()),
            hp: 21,
            x: -4,
            work_remaining: 9
        }
    ));
    game_unit_id(world_id(), chain.instance().0, nonce)
}
fn paired() -> (CertifiedTestChain, CertifiedTestChain) {
    let mut a = start("game-native-root-a");
    let mut b = start("game-native-root-b");
    assert!(commit(&mut a, registration(&b, "game-native-root-b")));
    assert!(commit(&mut b, registration(&a, "game-native-root-a")));
    (a, b)
}
#[test]
fn game_partition_certified_export_import_replay_and_direct_ivm() {
    let (mut a, mut b) = paired();
    let id = seed(&mut a, 7);
    assert!(commit(
        &mut a,
        AdvanceGameUnitV1 {
            world_id: world_id(),
            unit_id: id
        }
    ));
    assert_eq!(
        (
            snapshot(&a).units[0].x,
            snapshot(&a).units[0].work_remaining
        ),
        (-3, 8)
    );
    assert!(commit(
        &mut a,
        ExportGameUnitV1 {
            world_id: world_id(),
            unit_id: id,
            destination: b.instance().0
        }
    ));
    assert!(snapshot(&a).units.is_empty());
    let record = snapshot(&a).exports[0].clone();
    let proof = game_partition_export_proof(&a.state().view(), world_id(), record.receipt_id())
        .unwrap()
        .unwrap();
    assert!(validate_game_partition_import(&b.state().view(), &proof).unwrap());
    assert!(commit(
        &mut b,
        ImportGameUnitV1 {
            proof: proof.clone()
        }
    ));
    assert_eq!(snapshot(&b).units[0], record.unit);
    assert!(!validate_game_partition_import(&b.state().view(), &proof).unwrap());
    assert!(commit(
        &mut b,
        ImportGameUnitV1 {
            proof: proof.clone()
        }
    ));
    assert_eq!(snapshot(&b).units.len(), 1);
    assert!(!commit(
        &mut a,
        AdvanceGameUnitV1 {
            world_id: world_id(),
            unit_id: id
        }
    ));
    // Re-export, return and replay old receipt: no resurrection at B.
    assert!(commit(
        &mut b,
        ExportGameUnitV1 {
            world_id: world_id(),
            unit_id: id,
            destination: a.instance().0
        }
    ));
    assert!(commit(
        &mut b,
        ImportGameUnitV1 {
            proof: proof.clone()
        }
    ));
    assert!(snapshot(&b).units.is_empty());
    let back = snapshot(&b).exports[0].clone();
    let back_proof = game_partition_export_proof(&b.state().view(), world_id(), back.receipt_id())
        .unwrap()
        .unwrap();
    assert!(commit(&mut a, ImportGameUnitV1 { proof: back_proof }));
    assert_eq!(snapshot(&a).units[0].sequence, 2);
    assert_eq!(snapshot(&a).units[0].work_remaining, 8);
    // Genuine signed-prefix replay retains source tombstones, active units and proof discovery.
    let mut restart = CertifiedTestChain::start(config("game-native-root-a")).unwrap();
    restart.replay_from(&a).unwrap();
    assert_eq!(snapshot(&restart), snapshot(&a));
    assert_eq!(
        game_partition_export_proof(&restart.state().view(), world_id(), record.receipt_id())
            .unwrap()
            .unwrap(),
        proof
    );
}
#[test]
fn game_partition_rejects_tampered_receipts_and_failed_batch_rolls_back_export() {
    let (mut a, mut b) = paired();
    let id = seed(&mut a, 8);
    let before = snapshot(&a);
    let when = 2000 + a.height() * 1000;
    let tx = a.sign(
        &key(),
        vec![
            ExportGameUnitV1 {
                world_id: world_id(),
                unit_id: id,
                destination: b.instance().0,
            }
            .into(),
            AdvanceGameUnitV1 {
                world_id: world_id(),
                unit_id: id,
            }
            .into(),
        ],
        when - 1,
    );
    assert_eq!(a.commit_at(when, vec![tx]), vec![false]);
    assert_eq!(snapshot(&a), before);
    let (_, rolled_back_receipt) = export_candidate(
        before.clone(),
        id,
        b.instance().0,
        &AccountId::new(key().public_key().clone()),
        a.height(),
    )
    .unwrap();
    assert!(
        crate::query::native_receipts::game_partition_export_proof(
            &a.state().view(),
            &rolled_back_receipt
        )
        .unwrap()
        .is_none(),
        "rejected transaction must leave no certified export write"
    );
    assert!(commit(
        &mut a,
        ExportGameUnitV1 {
            world_id: world_id(),
            unit_id: id,
            destination: b.instance().0
        }
    ));
    let record = snapshot(&a).exports[0].clone();
    let proof = game_partition_export_proof(&a.state().view(), world_id(), record.receipt_id())
        .unwrap()
        .unwrap();
    for mutation in 0..11 {
        let mut bad = proof.clone();
        match mutation {
            0 => bad.record.unit.hp += 1,
            1 => bad.record.unit.x += 1,
            2 => bad.record.unit.work_remaining += 1,
            3 => bad.record.destination = [9; 32],
            4 => bad.record.kernel_hash = [9; 32],
            5 => bad.record.unit.sequence += 1,
            6 => bad.record.source_height += 1,
            7 => bad.block.commit_qc[0] ^= 1,
            8 => {
                bad.record.unit.owner = AccountId::new(
                    KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                )
            }
            9 => bad.record.unit.origin = [8; 32],
            _ => bad.record.unit.unit_id = Hash::new(b"substituted-unit"),
        }
        assert!(
            validate_game_partition_import(&b.state().view(), &bad).is_err(),
            "mutation {mutation}"
        );
        assert!(!commit(&mut b, ImportGameUnitV1 { proof: bad }));
        assert!(snapshot(&b).units.is_empty());
    }
    assert!(commit(&mut b, ImportGameUnitV1 { proof }));
}
#[test]
fn game_partition_registration_cannot_substitute_genesis_or_reseed_tombstone() {
    let (mut a, b) = paired();
    let id = seed(&mut a, 9);
    let snapshot_before = snapshot(&a);
    assert!(!commit(&mut a, registration(&b, "foreign-chain-label")));
    assert_eq!(snapshot(&a), snapshot_before);
    assert!(commit(
        &mut a,
        ExportGameUnitV1 {
            world_id: world_id(),
            unit_id: id,
            destination: b.instance().0
        }
    ));
    assert!(!commit(
        &mut a,
        SeedGameUnitV1 {
            world_id: world_id(),
            nonce: 9,
            owner: AccountId::new(key().public_key().clone()),
            hp: 21,
            x: -4,
            work_remaining: 9
        }
    ));
    assert!(snapshot(&a).units.is_empty());
    let source = decode_framed_signed_block(&b.genesis().encode_wire().unwrap()).unwrap();
    assert_eq!(source.hash(), b.genesis().hash());
}

#[test]
fn game_partition_caught_export_rejection_has_no_state_or_witness_effect() {
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};
    let (mut source, destination) = paired();
    let id = seed(&mut source, 10);
    let mut original = snapshot(&source);
    // Force a checked arithmetic rejection after the candidate removed its unit.
    // The caller deliberately catches the error and commits its outer transaction.
    original.units[0].sequence = u64::MAX;
    original.known_units[0].sequence = u64::MAX;
    original.validate().unwrap();
    let state = blank_state();
    let mut block = state.block(header(2));
    {
        let mut tx = block.transaction();
        tx.world
            .game_partitions
            .insert(world_id(), original.clone());
        tx.apply();
    }
    let _capture = crate::exec_witness::exec_witness_guard();
    crate::exec_witness::start_block();
    {
        let mut tx = block.transaction();
        let rejected = ExportGameUnitV1 {
            world_id: world_id(),
            unit_id: id,
            destination: destination.instance().0,
        }
        .execute(&AccountId::new(key().public_key().clone()), &mut tx);
        assert!(rejected.is_err());
        tx.apply();
    }
    assert_eq!(
        block.world.game_partitions.get(&world_id()),
        Some(&original)
    );
    assert!(
        crate::exec_witness::snapshot_exec_witness()
            .writes
            .is_empty()
    );
}

#[test]
fn game_partition_time_export_retains_certified_receipt_and_imports() {
    use iroha_data_model::{
        events::time::{ExecutionTime, Schedule, TimeEventFilter},
        isi::Register,
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    use std::time::Duration;

    let (mut a, mut b) = paired();
    let id = seed(&mut a, 71);
    let registration_time = 2_000 + a.height() * 1_000;
    let scheduled_time = registration_time + 500;
    let action = Action::new(
        vec![InstructionBox::from(ExportGameUnitV1 {
            world_id: world_id(),
            unit_id: id,
            destination: b.instance().0,
        })],
        Repeats::Exactly(1),
        AccountId::new(key().public_key().clone()),
        TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
            Duration::from_millis(scheduled_time),
        ))),
    )
    .unwrap();
    assert!(commit(
        &mut a,
        Register::trigger(Trigger::new(
            "game_certified_time_export".parse().unwrap(),
            action,
        ))
    ));
    assert!(
        snapshot(&a).exports.is_empty(),
        "registration precedes the scheduled interval"
    );
    assert!(commit(
        &mut a,
        Log::new(Level::INFO, "ordinary work carries the Time phase".into())
    ));
    let source = snapshot(&a);
    assert!(
        source.units.is_empty(),
        "Time callback removed source custody"
    );
    assert_eq!(source.exports.len(), 1, "one successful scheduled export");
    let record = &source.exports[0];
    assert_eq!(record.source_height, a.height());
    let proof = game_partition_export_proof(&a.state().view(), world_id(), record.receipt_id())
        .unwrap()
        .expect("Time output retained its original archived ordinary-write receipt");
    assert_eq!(&proof.record, record);
    assert!(validate_game_partition_import(&b.state().view(), &proof).unwrap());
    assert!(commit(&mut b, ImportGameUnitV1 { proof }));
    assert_eq!(snapshot(&b).units, vec![record.unit.clone()]);
    assert!(
        snapshot(&a).units.is_empty(),
        "foreign import cannot restore source custody"
    );
}

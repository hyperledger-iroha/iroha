//! Paid independent-root intent, reservation, export, import, cancellation and final ACK controls.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    isi::{Grant, InstructionBox, Log},
    runtime::definition::{RestrictionEntryV1, RestrictionV1},
};
use iroha_logger::Level;

fn world() -> Hash {
    Hash::new(b"captured-game-world")
}
fn key() -> KeyPair {
    KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519)
}
fn owner() -> AccountId {
    AccountId::new(key().public_key().clone())
}
fn start(label: &str) -> CertifiedTestChain {
    let world_state = World::new();
    crate::runtime::activate_additions_for_testing(
        &world_state,
        crate::runtime::catalog::additions::NATIVE_GAME_CAPTURE.module,
    )
    .unwrap();
    let mut config = TestChainConfig::new(world_state, 1000);
    config.chain_id = label.parse().unwrap();
    config.clock_bootstrap_in_genesis = true;
    config.genesis_instructions = vec![
        Grant::account_permission(
            iroha_executor_data_model::permission::parameter::CanSetParameters,
            AccountId::new(config.genesis_key.public_key().clone()),
        )
        .into(),
        InstallGamePartitionV1 {
            world_id: world(),
            kernel_hash: ivm::game_unit_step::kernel_hash(),
        }
        .into(),
    ];
    let mut chain = CertifiedTestChain::start(config).unwrap();
    assert!(commit(
        &mut chain,
        Log::new(Level::INFO, "certify original genesis".into())
    ));
    chain
}
fn commit(chain: &mut CertifiedTestChain, instruction: impl Into<InstructionBox>) -> bool {
    let instruction = instruction.into();
    let bytes = norito::encode_canonical(&instruction).unwrap();
    let decoded: InstructionBox = norito::decode_from_bytes(&bytes).unwrap();
    assert_eq!(instruction, decoded);
    let json = norito::json::to_json(&instruction).unwrap();
    let decoded: InstructionBox = norito::json::from_str(&json).unwrap();
    assert_eq!(instruction, decoded);
    let when = 2000 + chain.height() * 1000;
    let tx = chain.sign(&key(), vec![instruction], when - 1);
    let result = chain.commit_at(when, vec![tx]);
    if result != vec![true] {
        eprintln!(
            "captured game rejected at {}: {:?}",
            chain.height(),
            chain
                .committed(chain.height())
                .block()
                .network_output_at(0)
                .map(|(_, output)| &output.result)
        );
    }
    result == vec![true]
}
fn registration(peer: &CertifiedTestChain, label: &str) -> RegisterGamePartitionPeerV1 {
    RegisterGamePartitionPeerV1 {
        world_id: world(),
        chain_id: label.parse().unwrap(),
        network_id: peer.network_id(),
        genesis: peer.committed(1).block().encode_wire().unwrap(),
        successor: peer.committed(2).block().encode_wire().unwrap(),
        endpoints: vec!["http://127.0.0.1:12345".into()],
    }
}
fn pair() -> (CertifiedTestChain, CertifiedTestChain) {
    let mut source = start("captured-game-source");
    let mut destination = start("captured-game-destination");
    assert!(commit(
        &mut source,
        registration(&destination, "captured-game-destination")
    ));
    assert!(commit(
        &mut destination,
        registration(&source, "captured-game-source")
    ));
    (source, destination)
}
fn partition(chain: &CertifiedTestChain) -> GamePartitionStateV1 {
    game_partition_snapshot(&chain.state().view(), world())
        .unwrap()
        .unwrap()
}
fn transfer(chain: &CertifiedTestChain, id: Hash) -> NativeGameTransferV1 {
    super::super::native_game_transfer_snapshot(&chain.state().view(), world(), id)
        .unwrap()
        .unwrap()
}
fn seed(chain: &mut CertifiedTestChain, nonce: u64) -> Hash {
    assert!(commit(
        chain,
        SeedGameUnitV1 {
            world_id: world(),
            nonce,
            owner: owner(),
            hp: 21,
            x: -4,
            work_remaining: 9
        }
    ));
    game_unit_id(world(), chain.instance().0, nonce)
}
fn request(
    source: &mut CertifiedTestChain,
    destination: &CertifiedTestChain,
    unit: Hash,
    nonce: u64,
) -> GameUnitExportIntentProofV1 {
    assert!(commit(
        source,
        RequestGameUnitExportV1 {
            world_id: world(),
            unit_id: unit,
            destination: destination.instance().0,
            nonce
        }
    ));
    let view = source.state().view();
    let id = *view
        .world()
        .runtime_game_transfers()
        .iter()
        .find(|(_, row)| row.intent.unit.unit_id == unit && row.intent.nonce == nonce)
        .unwrap()
        .0;
    super::super::game_unit_export_intent_proof(&view, world(), id)
        .unwrap()
        .unwrap()
}
fn reserve(
    destination: &mut CertifiedTestChain,
    intent: GameUnitExportIntentProofV1,
) -> GameUnitReservationProofV1 {
    let id = intent.record.transfer_id();
    assert!(commit(
        destination,
        ReserveGameUnitImportV1 { proof: intent }
    ));
    super::super::game_unit_reservation_proof(&destination.state().view(), world(), id)
        .unwrap()
        .unwrap()
}
fn export(
    source: &mut CertifiedTestChain,
    reservation: GameUnitReservationProofV1,
) -> ReservedGameUnitExportProofV1 {
    let id = reservation.record.transfer_id();
    assert!(commit(
        source,
        ExportReservedGameUnitV1 { proof: reservation }
    ));
    super::super::reserved_game_unit_export_proof(&source.state().view(), world(), id)
        .unwrap()
        .unwrap()
}

#[test]
fn captured_game_paid_two_root_transfer_requires_exact_destination_ack_and_keeps_tombstones() {
    let (mut source, mut destination) = pair();
    let unit = seed(&mut source, 1);
    assert!(!commit(
        &mut source,
        ExportGameUnitV1 {
            world_id: world(),
            unit_id: unit,
            destination: destination.instance().0
        }
    ));
    let intent = request(&mut source, &destination, unit, 7);
    let id = intent.record.transfer_id();
    let original = partition(&source).units[0].clone();
    assert!(!commit(
        &mut source,
        AdvanceGameUnitV1 {
            world_id: world(),
            unit_id: unit
        }
    ));
    assert_eq!(partition(&source).units, vec![original]);
    let reservation = reserve(&mut destination, intent.clone());
    assert!(commit(
        &mut destination,
        ReserveGameUnitImportV1 { proof: intent }
    ));
    let exported = export(&mut source, reservation.clone());
    assert!(commit(
        &mut source,
        ExportReservedGameUnitV1 { proof: reservation }
    ));
    assert!(partition(&source).units.is_empty());
    assert!(transfer(&source, id).capture.is_some());
    assert!(transfer(&destination, id).capture.is_some());
    assert!(!commit(
        &mut source,
        CancelGameUnitExportRequestV1 { transfer_id: id }
    ));
    assert!(commit(
        &mut destination,
        ImportReservedGameUnitV1 {
            proof: exported.clone()
        }
    ));
    assert!(commit(
        &mut destination,
        ImportReservedGameUnitV1 {
            proof: exported.clone()
        }
    ));
    let ack = super::super::game_unit_import_ack_proof(&destination.state().view(), world(), id)
        .unwrap()
        .unwrap();
    assert_eq!(
        partition(&destination).units,
        vec![exported.record.export.unit.clone()]
    );
    assert!(transfer(&destination, id).capture.is_none());
    assert!(transfer(&source, id).capture.is_some());
    assert!(commit(
        &mut source,
        AcknowledgeGameUnitImportV1 { proof: ack.clone() }
    ));
    assert!(commit(
        &mut source,
        AcknowledgeGameUnitImportV1 { proof: ack }
    ));
    assert!(transfer(&source, id).capture.is_none());
    assert_eq!(
        partition(&source).exports,
        vec![exported.record.export.clone()]
    );
    assert_eq!(
        partition(&destination).imported,
        vec![exported.record.export.receipt_id()]
    );
    for chain in [&source, &destination] {
        custody::validate_persistence(chain.state().view().world()).unwrap();
    }
}

#[test]
fn captured_game_cancel_is_certified_preexport_only_and_never_reuses_request_identity() {
    let (mut source, mut destination) = pair();
    let unit = seed(&mut source, 2);
    let intent = request(&mut source, &destination, unit, 13);
    let id = intent.record.transfer_id();
    let reservation = reserve(&mut destination, intent);
    let before = partition(&source);
    assert!(commit(
        &mut source,
        CancelGameUnitExportRequestV1 { transfer_id: id }
    ));
    assert_eq!(partition(&source), before);
    assert!(transfer(&source, id).capture.is_none());
    assert!(transfer(&destination, id).capture.is_some());
    let cancellation =
        super::super::game_unit_export_cancel_proof(&source.state().view(), world(), id)
            .unwrap()
            .unwrap();
    let mut changed = cancellation.clone();
    changed.record.source_height += 1;
    assert!(!commit(
        &mut destination,
        CancelReservedGameUnitImportV1 { proof: changed }
    ));
    assert!(transfer(&destination, id).capture.is_some());
    assert!(commit(
        &mut destination,
        CancelReservedGameUnitImportV1 {
            proof: cancellation.clone()
        }
    ));
    assert!(commit(
        &mut destination,
        CancelReservedGameUnitImportV1 {
            proof: cancellation
        }
    ));
    assert!(transfer(&destination, id).capture.is_none());
    assert!(!commit(
        &mut source,
        ExportReservedGameUnitV1 { proof: reservation }
    ));
    assert!(!commit(
        &mut source,
        RequestGameUnitExportV1 {
            world_id: world(),
            unit_id: unit,
            destination: destination.instance().0,
            nonce: 13
        }
    ));
    assert!(commit(
        &mut source,
        AdvanceGameUnitV1 {
            world_id: world(),
            unit_id: unit
        }
    ));
    let next = request(&mut source, &destination, unit, 14);
    assert_ne!(next.record.transfer_id(), id);
    assert!(partition(&destination).units.is_empty());
    custody::validate_persistence(source.state().view().world()).unwrap();
    custody::validate_persistence(destination.state().view().world()).unwrap();
}

#[test]
fn captured_game_substituted_reservation_export_and_ack_cannot_release_original_custody() {
    let (mut source, mut destination) = pair();
    let unit = seed(&mut source, 3);
    let intent = request(&mut source, &destination, unit, 17);
    let id = intent.record.transfer_id();
    let reservation = reserve(&mut destination, intent);
    let source_before = partition(&source);
    let mut changed = reservation.clone();
    changed.record.intent.unit.hp += 1;
    assert!(!commit(
        &mut source,
        ExportReservedGameUnitV1 { proof: changed }
    ));
    assert_eq!(partition(&source), source_before);
    let exported = export(&mut source, reservation);
    let destination_before = partition(&destination);
    let mut changed = exported.clone();
    changed.record.export.unit.work_remaining += 1;
    assert!(!commit(
        &mut destination,
        ImportReservedGameUnitV1 { proof: changed }
    ));
    assert_eq!(partition(&destination), destination_before);
    assert!(transfer(&destination, id).capture.is_some());
    assert!(commit(
        &mut destination,
        ImportReservedGameUnitV1 { proof: exported }
    ));
    let ack = super::super::game_unit_import_ack_proof(&destination.state().view(), world(), id)
        .unwrap()
        .unwrap();
    let mut changed = ack.clone();
    changed.record.export_hash = Hash::new(b"substituted original export");
    assert!(!commit(
        &mut source,
        AcknowledgeGameUnitImportV1 { proof: changed }
    ));
    assert!(transfer(&source, id).capture.is_some());
    let mut changed = ack.clone();
    changed.block.commit_qc[0] ^= 1;
    assert!(!commit(
        &mut source,
        AcknowledgeGameUnitImportV1 { proof: changed }
    ));
    assert!(transfer(&source, id).capture.is_some());
    assert!(commit(
        &mut source,
        AcknowledgeGameUnitImportV1 { proof: ack }
    ));
}

#[test]
fn captured_game_source_export_is_continue_but_cancellation_is_complete() {
    let (mut source, mut destination) = pair();
    let unit = seed(&mut source, 4);
    let intent = request(&mut source, &destination, unit, 19);
    let id = intent.record.transfer_id();
    let reservation = reserve(&mut destination, intent);
    let height = source.height() + 1;
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(height).unwrap(),
        None,
        None,
        100_000,
        0,
    );
    let mut block = source.state().block(header);
    let mut tx = block.transaction();
    let export: InstructionBox = ExportReservedGameUnitV1 { proof: reservation }.into();
    let cancel: InstructionBox = CancelGameUnitExportRequestV1 { transfer_id: id }.into();
    for instruction in [&export, &cancel] {
        let wire = iroha_data_model::isi::instruction_wire_id(instruction).unwrap();
        let definition = tx
            .world
            .runtime_selection
            .canonical()
            .bindings
            .iter()
            .find(|b| b.wire_id == wire)
            .unwrap()
            .interface;
        tx.world
            .runtime_selection
            .get_mut()
            .restrictions
            .push(RestrictionEntryV1 {
                definition,
                restriction: RestrictionV1::CompletionOnly(height),
            });
    }
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    assert!(
        crate::runtime::execution::with_native_obligation(&mut tx, &export, |tx| {
            assert_eq!(
                tx.native_execution.as_ref().unwrap().step,
                crate::runtime::obligations::Step::Continue
            );
            crate::runtime::resolve_executing(tx, &export)
                .map(|_| ())
                .map_err(iroha_data_model::ValidationFail::InstructionFailed)
        })
        .is_err()
    );
    assert!(
        crate::runtime::execution::with_native_obligation(&mut tx, &cancel, |tx| {
            assert_eq!(
                tx.native_execution.as_ref().unwrap().step,
                crate::runtime::obligations::Step::Complete
            );
            crate::runtime::resolve_executing(tx, &cancel)
                .map(|_| ())
                .map_err(iroha_data_model::ValidationFail::InstructionFailed)
        })
        .is_ok()
    );
    assert!(tx.native_execution.is_none());
}

#[test]
fn captured_game_original_codec_refusal_never_writes_or_funds_a_request() {
    let (mut source, destination) = pair();
    let unit = seed(&mut source, 5);
    let before = partition(&source);
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(source.height() + 1).unwrap(),
        None,
        None,
        100_000,
        0,
    );
    let mut block = source.state().block(header);
    let _witness = crate::exec_witness::exec_witness_guard();
    crate::exec_witness::start_block();
    let mut tx = block.transaction();
    let request = RequestGameUnitExportV1 {
        world_id: world(),
        unit_id: unit,
        destination: destination.instance().0,
        nonce: 23,
    };
    let result = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || request.execute(&owner(), &mut tx),
    );
    assert!(result.is_err());
    let original = tx
        .require_storage_admission()
        .expect_err("original codec refusal is sticky");
    assert!(matches!(
        &original,
        crate::state::StateStorageAdmissionError::AmxDecode(_)
    ));
    assert_eq!(tx.world.game_partitions.get(&world()), Some(&before));
    assert_eq!(tx.world.runtime_game_transfers.iter().count(), 0);
    assert!(
        crate::exec_witness::snapshot_exec_witness()
            .writes
            .is_empty()
    );
    drop(tx);
    assert_eq!(block.require_storage_admission(), Err(original));
    drop(block);
    assert!(commit(&mut source, request));
}

#[test]
fn captured_game_restore_requires_original_lock_reservation_capacity_and_tombstone() {
    let (mut source, mut destination) = pair();
    let unit = seed(&mut source, 6);
    let intent = request(&mut source, &destination, unit, 29);
    let id = intent.record.transfer_id();
    let reservation = reserve(&mut destination, intent);
    let header = |chain: &CertifiedTestChain| {
        BlockHeader::new(
            std::num::NonZeroU64::new(chain.height() + 1).unwrap(),
            None,
            None,
            100_000,
            0,
        )
    };
    {
        let mut block = source.state().block(header(&source));
        let mut tx = block.transaction();
        custody::validate_persistence(tx.world()).unwrap();
        let original_row = transfer(&source, id);
        let mut changed_row = original_row.clone();
        changed_row.capture.as_mut().unwrap().terminal.pop();
        tx.world.runtime_game_transfers.insert(id, changed_row);
        assert!(custody::validate_persistence(tx.world()).is_err());
        tx.world.runtime_game_transfers.insert(id, original_row);
        let mut changed = partition(&source);
        changed.units[0].hp += 1;
        tx.world.game_partitions.insert(world(), changed);
        assert!(custody::validate_persistence(tx.world()).is_err());
    }
    {
        let view = destination.state().view();
        custody::validate_persistence(view.world()).unwrap();
        let mut full = partition(&destination);
        // Reservation owns its eventual imported-receipt slot before source removal.
        full.imported = (0..GAME_PARTITION_MAX_RECEIPTS)
            .map(|index| Hash::new(index.to_le_bytes()))
            .collect();
        assert!(custody::check_partition_capacity(view.world(), &full).is_err());
        // The same reserved slot is consumed, rather than counted twice, at import.
        custody::check_capacity_after(view.world(), &full, Some(id)).unwrap();
    }
    export(&mut source, reservation);
    {
        let mut block = source.state().block(header(&source));
        let mut tx = block.transaction();
        custody::validate_persistence(tx.world()).unwrap();
        let mut changed = partition(&source);
        changed.exports.clear();
        tx.world.game_partitions.insert(world(), changed);
        assert!(custody::validate_persistence(tx.world()).is_err());
    }
}

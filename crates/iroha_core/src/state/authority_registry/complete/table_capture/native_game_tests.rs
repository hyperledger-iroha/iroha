//! Populated native game transfer rows bind their exact typed contents to paired table proofs.
//! These storage fixtures establish persistence coverage, not protocol admission or finality.

use super::{native_test_support::*, *};
use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
use iroha_crypto::Hash;
use iroha_data_model::{
    game_partition::GameUnitV1,
    runtime::native_game::{
        GameUnitExportCancelV1, GameUnitExportIntentV1, NativeGameRoleV1, NativeGameTransferV1,
    },
};

fn cancelled_transfer() -> (Hash, NativeGameTransferV1) {
    let intent = GameUnitExportIntentV1 {
        world_id: Hash::prehashed([1; 32]),
        source: [2; 32],
        destination: [3; 32],
        kernel_hash: [4; 32],
        unit: GameUnitV1 {
            unit_id: Hash::prehashed([5; 32]),
            origin: [2; 32],
            owner: iroha_test_samples::ALICE_ID.clone(),
            hp: 100,
            x: 7,
            sequence: 1,
            work_remaining: 10,
        },
        nonce: 1,
        source_height: 2,
    };
    let record = NativeGameTransferV1 {
        role: NativeGameRoleV1::Source,
        cancellation: Some(GameUnitExportCancelV1 {
            intent: intent.clone(),
            source_height: 3,
        }),
        intent,
        reservation: None,
        export: None,
        acknowledgement: None,
        capture: None,
    };
    record.validate().unwrap();
    (record.intent.transfer_id(), record)
}

capture_controls!(
    native_game_transfer_capture_binds_tombstone_and_original_identity,
    runtime_game_transfers,
    capture_runtime_game_transfers_once,
    cancelled_transfer(),
    |record: &mut NativeGameTransferV1| {
        record.intent.nonce += 1;
    }
);

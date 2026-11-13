//! Permanent retirement rows remain authenticated State after handler code is removed.
//! These fixtures establish exact persistence coverage, not governance or finality.

use super::{native_test_support::*, *};
use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    runtime::{
        checkpoint::StartupFloorV1,
        definition::DefinitionId,
        retirement::{
            RuntimeRetirementReceiptV1, RuntimeRetirementRootV1, RuntimeRetirementStateV1,
        },
    },
};

fn retired_definition() -> (DefinitionId, RuntimeRetirementStateV1) {
    let definition = DefinitionId([0x7a; 32]);
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"root")));
    let row = RuntimeRetirementStateV1 {
        history_started: 2,
        last_required: Some(3),
        governed_excluded_at: Some(4),
        receipt: Some(RuntimeRetirementReceiptV1 {
            root: RuntimeRetirementRootV1 {
                network,
                scope: SumeragiRootScope::Global,
                instance: [0x71; 32],
            },
            definition,
            history_started: 2,
            last_required: Some(3),
            excluded_at: 4,
            startup_floor: StartupFloorV1 {
                network,
                height: 4,
                block_hash: Hash::new(b"floor").into(),
                committed_ms: 4_000,
            },
            receipt_height: 5,
        }),
    };
    row.validate(definition).unwrap();
    (definition, row)
}

capture_controls!(
    runtime_retirement_capture_binds_permanent_exclusion_and_floor,
    runtime_retirements,
    capture_runtime_retirements_once,
    retired_definition(),
    |record: &mut RuntimeRetirementStateV1| {
        record.receipt.as_mut().unwrap().startup_floor.block_hash[0] ^= 1;
    }
);

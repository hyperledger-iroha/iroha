//! Every atomic movement participates in fee-asset effect classification.
use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    asset::AssetId,
    block::BlockHeader,
    isi::{AtomicSettlementMovement, AtomicSettlementMovements},
};
use std::num::NonZeroU64;

fn instruction() -> SettleAtomic {
    let account = |seed| {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture key")
                .public_key()
                .clone(),
        )
    };
    let payer = account(1);
    let recipient = account(2);
    let mut movements = (0..255)
        .map(|index| {
            let definition = AssetDefinitionId::from_uuid_bytes([
                index, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
            ])
            .expect("UUIDv4 fixture");
            AtomicSettlementMovement {
                source: AssetId::new(definition, payer.clone()),
                recipient: recipient.clone(),
                quantity: Quantity::from(42_u32),
            }
        })
        .collect::<Vec<_>>();
    movements.sort_by(|a, b| (&a.source, &a.recipient).cmp(&(&b.source, &b.recipient)));
    SettleAtomic::new(
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"fee-fixture",
        ))),
        "fee_atomic".parse().expect("id"),
        AtomicSettlementMovements::try_from(movements).expect("canonical"),
        NonZeroU64::new(500).expect("expiry"),
        Metadata::default(),
    )
}

#[test]
fn fee_asset_in_first_middle_or_last_atomic_movement_fails_closed() {
    let value = instruction();
    let boxed: InstructionBox = value.clone().into();
    for index in [0, 127, 254] {
        let fee = value.movements.as_slice()[index].source.definition();
        assert_eq!(
            native_fee_asset_movement_wire_id(&boxed, fee),
            Some(SettleAtomic::WIRE_ID)
        );
        assert_eq!(
            native_instruction_ds_effect_disposition(&boxed, fee),
            NativeInstructionDsEffectDisposition::RejectKnownDsCapable(SettleAtomic::WIRE_ID)
        );
    }
}

#[test]
fn atomic_batch_with_no_fee_asset_has_no_policy_asset_effect() {
    let boxed: InstructionBox = instruction().into();
    let unrelated = AssetDefinitionId::from_uuid_bytes([
        255, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
    ])
    .expect("distinct UUIDv4");
    assert_eq!(native_fee_asset_movement_wire_id(&boxed, &unrelated), None);
    assert_eq!(
        native_instruction_ds_effect_disposition(&boxed, &unrelated),
        NativeInstructionDsEffectDisposition::AuditedNoDsEffect
    );
}

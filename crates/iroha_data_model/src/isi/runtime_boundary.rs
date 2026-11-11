//! Ratify the adoption anchor of a root and raise its startup floor
//! (`crate::runtime::checkpoint`, `specs/live_runtime_upgrades.md` §7.3, §13).
use super::*;
use crate::runtime::checkpoint::{
    RuntimeAdoptionAnchorV1, SignedStartupFloorAttestationV1, StartupFloorV1,
};
isi! {
 /// Ratify the adoption anchor of the root: the adoption record with one statement from every
 /// validator of the current committee.
 ///
 /// The statements are the authority, so any account relays them. The kernel checks what
 /// committed State decides: the root holds no boundary yet, and the record names this network
 /// and the block the chain committed at its height. Each validator checked the rest against
 /// its own store before it signed. A ratified anchor never changes.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::runtime_boundary::RatifyRuntimeAdoptionAnchorV1")]
 pub struct RatifyRuntimeAdoptionAnchorV1 {
  /// The adoption record and every validator's statement of it.
  pub anchor: RuntimeAdoptionAnchorV1,
 }
}
impl crate::seal::Instruction for RatifyRuntimeAdoptionAnchorV1 {}
impl RatifyRuntimeAdoptionAnchorV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.boundary.anchor.v1";
}
isi! {
 /// Raise the startup floor of the root to `floor`, with one statement from every validator of
 /// the current committee.
 ///
 /// The statements are the authority, so any account relays them. The floor comes into force
 /// the committed checkpoint policy's notice after the block that executes this instruction,
 /// and until then the root raises no other floor. The floor lies above the root's latest floor
 /// and its adoption anchor, and its checkpoint is at least the policy's recovery window old
 /// when it comes into force.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::runtime_boundary::RaiseRuntimeStartupFloorV1")]
 pub struct RaiseRuntimeStartupFloorV1 {
  /// The checkpoint the floor rises to.
  pub floor: StartupFloorV1,
  /// One statement of the floor from every member of the current committee.
  pub attestations: Vec<SignedStartupFloorAttestationV1>,
 }
}
impl crate::seal::Instruction for RaiseRuntimeStartupFloorV1 {}
impl RaiseRuntimeStartupFloorV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.runtime.boundary.floor.v1";
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    use super::*;
    use crate::{
        NetworkId,
        block::BlockHeader,
        runtime::{
            checkpoint::RuntimeStartupFloorV1, definition::SelectionId, lifecycle::AdoptionRecordV1,
        },
    };

    #[test]
    fn boundary_instructions_roundtrip_through_their_wire_ids() {
        let key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([7; 32]),
        ));
        let record = AdoptionRecordV1 {
            network,
            height: 5524,
            block_hash: [1; 32],
            predecessor_world_root: [2; 32],
            imported_state_root: [3; 32],
            committee: [4; 32],
            selection: SelectionId([5; 32]),
            authorization: [0; 32],
        };
        let ratify = RatifyRuntimeAdoptionAnchorV1 {
            anchor: RuntimeAdoptionAnchorV1 {
                attestations: vec![
                    RuntimeAdoptionAnchorV1::statement(&record, key.public_key().clone())
                        .unwrap()
                        .sign(key.private_key())
                        .unwrap(),
                ],
                record,
            },
        };
        let floor = StartupFloorV1 {
            network,
            height: 6000,
            block_hash: [6; 32],
            committed_ms: 24_000_000,
        };
        let raise = RaiseRuntimeStartupFloorV1 {
            attestations: vec![
                RuntimeStartupFloorV1::statement(&floor, key.public_key().clone())
                    .unwrap()
                    .sign(key.private_key())
                    .unwrap(),
            ],
            floor,
        };
        for (instruction, wire_id) in [
            (
                InstructionBox::from(ratify.clone()),
                RatifyRuntimeAdoptionAnchorV1::WIRE_ID,
            ),
            (
                InstructionBox::from(raise.clone()),
                RaiseRuntimeStartupFloorV1::WIRE_ID,
            ),
        ] {
            assert_eq!(instruction_wire_id(&instruction), Some(wire_id));
            let bytes = norito::encode_canonical(&instruction).unwrap();
            assert_eq!(
                norito::decode_canonical::<InstructionBox>(&bytes).unwrap(),
                instruction
            );
        }
        let json = norito::json::to_json(&ratify).unwrap();
        assert_eq!(
            norito::json::from_str::<RatifyRuntimeAdoptionAnchorV1>(&json).unwrap(),
            ratify
        );
        let json = norito::json::to_json(&raise).unwrap();
        assert_eq!(
            norito::json::from_str::<RaiseRuntimeStartupFloorV1>(&json).unwrap(),
            raise
        );
    }
}

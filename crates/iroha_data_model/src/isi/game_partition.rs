//! Native independent-root unit transport instructions; no operator relay authority.
use super::*;
isi! {
 /// Native bounded install transition.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::InstallGamePartitionV1")]
 #[derive(Copy)]
 pub struct InstallGamePartitionV1 {
  /// Shared application identity.
  pub world_id: iroha_crypto::Hash,
  /// Exact supported IVM artifact, authorized by governance.
  pub kernel_hash: [u8;32],
 }
}
impl crate::seal::Instruction for InstallGamePartitionV1 {}
impl InstallGamePartitionV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.install.v1";
}
isi! {
 /// Native bounded register peer transition.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::RegisterGamePartitionPeerV1")]
 pub struct RegisterGamePartitionPeerV1 {
  /// Local application identity.
  pub world_id: iroha_crypto::Hash,
  /// Governance-authenticated foreign chain label.
  pub chain_id: iroha_model_base::chain::ChainId,
  /// Governance-authenticated signed-genesis network hash.
  pub network_id: crate::NetworkId,
  /// Complete canonical signed foreign genesis.
  pub genesis: Vec<u8>,
  /// Real H2 successor authenticating genesis execution.
  pub successor: Vec<u8>,
  /// At most four governance-approved public read endpoints.
  pub endpoints: Vec<String>,
 }
}
impl crate::seal::Instruction for RegisterGamePartitionPeerV1 {}
impl RegisterGamePartitionPeerV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.register_peer.v1";
}
isi! {
 /// Native bounded seed transition.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::SeedGameUnitV1")]
 pub struct SeedGameUnitV1 {
  /// Installed application identity.
  pub world_id: iroha_crypto::Hash,
  /// Unique source seed nonce.
  pub nonce: u64,
  /// Unit controller.
  pub owner: crate::account::AccountId,
  /// Initial hit points.
  pub hp: u32,
  /// Initial signed position.
  pub x: i32,
  /// Finite nonrefillable IVM advance allowance.
  pub work_remaining: u32,
 }
}
impl crate::seal::Instruction for SeedGameUnitV1 {}
impl SeedGameUnitV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.seed.v1";
}
isi! {
 /// Native bounded advance transition.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::AdvanceGameUnitV1")]
 #[derive(Copy)]
 pub struct AdvanceGameUnitV1 {
  /// Installed application identity.
  pub world_id: iroha_crypto::Hash,
  /// Active unit to execute once.
  pub unit_id: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for AdvanceGameUnitV1 {}
impl AdvanceGameUnitV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.advance.v1";
}
isi! {
 /// Native bounded export transition.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::ExportGameUnitV1")]
 #[derive(Copy)]
 pub struct ExportGameUnitV1 {
  /// Installed application identity.
  pub world_id: iroha_crypto::Hash,
  /// Active unit to remove atomically.
  pub unit_id: iroha_crypto::Hash,
  /// Registered sole destination instance.
  pub destination: [u8;32],
 }
}
impl crate::seal::Instruction for ExportGameUnitV1 {}
impl ExportGameUnitV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.export.v1";
}
isi! {
 /// Native bounded import transition.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::ImportGameUnitV1")]
 pub struct ImportGameUnitV1 {
  /// Public proof authorizing this exact import; carrier pays ordinary fees.
  pub proof: crate::game_partition::GameUnitImportProofV1,
 }
}
impl crate::seal::Instruction for ImportGameUnitV1 {}
impl ImportGameUnitV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.import.v1";
}

isi! {
 /// Owner-authorized source request locking one unit before remote destination admission.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::RequestGameUnitExportV1")]
 #[derive(Copy)]
 pub struct RequestGameUnitExportV1 {
  /// Registered local application world.
  pub world_id: iroha_crypto::Hash,
  /// Original active source unit to lock.
  pub unit_id: iroha_crypto::Hash,
  /// Registered sole destination for this transfer.
  pub destination: [u8;32],
  /// Fresh owner-chosen identity; canceled requests remain permanently distinct.
  pub nonce: u64,
 }
}
impl crate::seal::Instruction for RequestGameUnitExportV1 {}
impl RequestGameUnitExportV1 {
    /// Stable selected-protocol wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.request_export.v1";
}
isi! {
 /// Destination reservation authorized by a certified source request and unit lock.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::ReserveGameUnitImportV1")]
 pub struct ReserveGameUnitImportV1 {
  /// Authentic source request, binding its locked original unit and sole destination.
  pub proof: crate::runtime::native_game::GameUnitExportIntentProofV1,
 }
}
impl crate::seal::Instruction for ReserveGameUnitImportV1 {}
impl ReserveGameUnitImportV1 {
    /// Stable wire identity, activated only by the independent native game capture addition.
    pub const WIRE_ID: &'static str = "iroha.game_partition.reserve_import.v1";
}
isi! {
 /// Irreversible source export after authentic destination acceptance.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::ExportReservedGameUnitV1")]
 pub struct ExportReservedGameUnitV1 {
  /// Destination's exact certified reservation and original completion guarantee.
  pub proof: crate::runtime::native_game::GameUnitReservationProofV1,
 }
}
impl crate::seal::Instruction for ExportReservedGameUnitV1 {}
impl ExportReservedGameUnitV1 {
    /// Stable selected-protocol wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.export_reserved.v1";
}
isi! {
 /// Complete reserved destination custody using authentic source removal.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::ImportReservedGameUnitV1")]
 pub struct ImportReservedGameUnitV1 {
  /// Exact source export bound to this root's retained reservation.
  pub proof: crate::runtime::native_game::ReservedGameUnitExportProofV1,
 }
}
impl crate::seal::Instruction for ImportReservedGameUnitV1 {}
impl ImportReservedGameUnitV1 {
    /// Stable selected-protocol wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.import_reserved.v1";
}
isi! {
 /// Final source ACK of certified destination import; never recreates a unit.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::AcknowledgeGameUnitImportV1")]
 pub struct AcknowledgeGameUnitImportV1 {
  /// Exact destination import evidence; source tombstones remain permanent.
  pub proof: crate::runtime::native_game::GameUnitImportAckProofV1,
 }
}
impl crate::seal::Instruction for AcknowledgeGameUnitImportV1 {}
impl AcknowledgeGameUnitImportV1 {
    /// Stable selected-protocol wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.acknowledge_import.v1";
}
isi! {
 /// Owner cancellation before the source has exported its original locked unit.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::CancelGameUnitExportRequestV1")]
 #[derive(Copy)]
 pub struct CancelGameUnitExportRequestV1 {
  /// Original admitted request that has not exported.
  pub transfer_id: iroha_crypto::Hash,
 }
}
impl crate::seal::Instruction for CancelGameUnitExportRequestV1 {}
impl CancelGameUnitExportRequestV1 {
    /// Stable selected-protocol wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.cancel_export_request.v1";
}
isi! {
 /// Release destination reservation only after certified permanent source cancellation.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::game_partition::CancelReservedGameUnitImportV1")]
 pub struct CancelReservedGameUnitImportV1 {
  /// Authentic source proof that this request was never exported and cannot export later.
  pub proof: crate::runtime::native_game::GameUnitExportCancelProofV1,
 }
}
impl crate::seal::Instruction for CancelReservedGameUnitImportV1 {}
impl CancelReservedGameUnitImportV1 {
    /// Stable selected-protocol wire identity.
    pub const WIRE_ID: &'static str = "iroha.game_partition.cancel_import_reservation.v1";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game_partition::*,
        sumeragi_amx::{AmxCertifiedBlockV1, AmxWriteProofV1},
    };
    #[test]
    fn game_partition_native_instructions_roundtrip_registry() {
        let pair = iroha_crypto::KeyPair::random();
        let world = iroha_crypto::Hash::new(b"wire-world");
        let owner = crate::account::AccountId::new(pair.public_key().clone());
        let unit = GameUnitV1 {
            unit_id: game_unit_id(world, [1; 32], 1),
            origin: [1; 32],
            owner: owner.clone(),
            hp: 8,
            x: -1,
            sequence: 1,
            work_remaining: 3,
        };
        let proof = GameUnitImportProofV1 {
            record: GameUnitExportV1 {
                world_id: world,
                source: [1; 32],
                destination: [2; 32],
                kernel_hash: [3; 32],
                unit: unit.clone(),
                source_height: 2,
            },
            block: AmxCertifiedBlockV1 {
                consensus_header: vec![1],
                commit_qc: vec![2],
                result_preimage: vec![3],
            },
            path: AmxWriteProofV1 {
                present: [0; 32],
                siblings: vec![],
            },
        };
        let instructions: Vec<InstructionBox> = vec![
            InstallGamePartitionV1 {
                world_id: world,
                kernel_hash: [3; 32],
            }
            .into(),
            RegisterGamePartitionPeerV1 {
                world_id: world,
                chain_id: "peer".into(),
                network_id: crate::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                    crate::block::BlockHeader,
                >::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"genesis"),
                )),
                genesis: vec![1],
                successor: vec![2],
                endpoints: vec!["http://localhost".into()],
            }
            .into(),
            SeedGameUnitV1 {
                world_id: world,
                nonce: 1,
                owner,
                hp: 8,
                x: -1,
                work_remaining: 3,
            }
            .into(),
            AdvanceGameUnitV1 {
                world_id: world,
                unit_id: unit.unit_id,
            }
            .into(),
            ExportGameUnitV1 {
                world_id: world,
                unit_id: unit.unit_id,
                destination: [2; 32],
            }
            .into(),
            ImportGameUnitV1 { proof }.into(),
        ];
        for original in instructions {
            let encoded = norito::encode_canonical(&original).unwrap();
            let decoded: InstructionBox = norito::decode_canonical(&encoded).unwrap();
            assert_eq!(decoded, original);
        }
    }
}

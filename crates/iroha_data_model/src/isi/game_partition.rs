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

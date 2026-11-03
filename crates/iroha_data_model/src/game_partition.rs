//! Bounded independent-root game-unit transport. These records are native State, not
//! client checkpoints. Export is irreversible; only a certified source write admits import.
use crate::{
    account::AccountId,
    sumeragi_amx::{AmxCertifiedBlockV1, AmxForeignInstanceV1, AmxWriteProofV1},
};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Maximum installed game worlds per root in the finite transport prototype.
pub const GAME_PARTITION_MAX_WORLDS: usize = 16;
/// Maximum foreign roots registered to one local world.
pub const GAME_PARTITION_MAX_PEERS: usize = 2;
/// Maximum simultaneously active units in one local world.
pub const GAME_PARTITION_MAX_UNITS: usize = 64;
/// Permanent per-world receipt and unit-identity bound; admission rejects before mutation.
pub const GAME_PARTITION_MAX_RECEIPTS: usize = 256;
/// Maximum direct IVM advances granted to one seeded unit, never replenished on import.
pub const GAME_PARTITION_MAX_WORK: u32 = 4096;
/// Bound on the encoded certified import graph, independent of network frame limits.
pub const GAME_PARTITION_MAX_PROOF_BYTES: usize = 1024 * 1024;

macro_rules! record {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$f:meta])* pub $field:ident: $ty:ty,)* }) => {
        #[derive(Debug,Clone,PartialEq,Eq,Encode,Decode,norito::NoritoSchema,IntoSchema,crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
        $(#[$m])*
        #[norito(deny_unknown_fields)]
        pub struct $name { $($(#[$f])* pub $field:$ty,)* }
    };
}
record! {
    /// One uniquely identified unit; every transfer preserves its original identity and budget.
    #[derive(PartialOrd,Ord)]
    #[norito_schema(name = "iroha_data_model::game_partition::GameUnitV1")]
    pub struct GameUnitV1 {
        /// Domain-separated identity created only by the original source.
        pub unit_id: Hash,
        /// Original signed-root consensus instance.
        pub origin: [u8;32],
        /// Controller of movement and export.
        pub owner: AccountId,
        /// Exact live hit points.
        pub hp: u32,
        /// Signed bounded position for the narrow direct-IVM prototype.
        pub x: i32,
        /// Transfer sequence; increases once at export.
        pub sequence: u64,
        /// Finite remaining direct-IVM advance allowance, preserved across roots.
        pub work_remaining: u32,
    }
}
record! {
    /// Immutable certified source-export statement.
    #[derive(PartialOrd,Ord)]
    #[norito_schema(name = "iroha_data_model::game_partition::GameUnitExportV1")]
    pub struct GameUnitExportV1 {
        /// Application world identity, shared only by explicitly registered peers.
        pub world_id: Hash,
        /// Root that removed the active unit.
        pub source: [u8;32],
        /// Sole root authorized to activate it.
        pub destination: [u8;32],
        /// Exact governed executable semantics.
        pub kernel_hash: [u8;32],
        /// Full post-export state, including incremented transfer sequence.
        pub unit: GameUnitV1,
        /// Height of the ordinary-write receipt carrier.
        pub source_height: u64,
    }
}
record! {
    /// Public proof-authorized import; the submitter is not a trusted relay.
    #[derive(PartialOrd,Ord)]
    #[norito_schema(name = "iroha_data_model::game_partition::GameUnitImportProofV1")]
    pub struct GameUnitImportProofV1 {
        /// Exact source transition.
        pub record: GameUnitExportV1,
        /// Source CommitQC and authenticated result preimage.
        pub block: AmxCertifiedBlockV1,
        /// Path of the complete record under the certified ordinary-write root.
        pub path: AmxWriteProofV1,
    }
}
record! {
    /// Immutable governance-installed peer and its independently authenticated committee.
    #[norito_schema(name = "iroha_data_model::game_partition::GamePartitionPeerV1")]
    pub struct GamePartitionPeerV1 {
        /// Foreign source verifier; scope is Global in this two-root prototype.
        pub tracker: AmxForeignInstanceV1,
        /// Hash of the retained authenticated signed genesis source.
        pub genesis_hash: Hash,
        /// Hash of the H2 source authenticating genesis execution.
        pub successor_hash: Hash,
        /// Bounded public read endpoints. Responses remain untrusted until proof verification.
        pub endpoints: Vec<String>,
    }
}
record! {
    /// Permanent identity watermark, retained when an active unit leaves this root.
    #[derive(Copy)]
    #[norito_schema(name = "iroha_data_model::game_partition::GameUnitWatermarkV1")]
    pub struct GameUnitWatermarkV1 {
        /// Globally stable unit identity.
        pub unit_id: Hash,
        /// Greatest sequence accepted or exported here.
        pub sequence: u64,
    }
}
record! {
    /// Bounded authoritative native State of one game partition.
    #[norito_schema(name = "iroha_data_model::game_partition::GamePartitionStateV1")]
    pub struct GamePartitionStateV1 {
        /// Immutable application identity.
        pub world_id: Hash,
        /// Actual executing signed-root identity, never caller selected.
        pub local_instance: [u8;32],
        /// Immutable approved narrow IVM artifact.
        pub kernel_hash: [u8;32],
        /// Governance-authorized installing account. No mutation bypass is granted.
        pub controller: AccountId,
        /// Independently authenticated immutable peer registrations.
        pub peers: Vec<GamePartitionPeerV1>,
        /// Active units, sorted by identifier.
        pub units: Vec<GameUnitV1>,
        /// Permanent source tombstones and durable native delivery discovery.
        pub exports: Vec<GameUnitExportV1>,
        /// Exact imported receipt identities; retries do not resurrect subsequently exported units.
        pub imported: Vec<Hash>,
        /// Permanent local identity/sequence watermarks.
        pub known_units: Vec<GameUnitWatermarkV1>,
    }
}

impl GameUnitExportV1 {
    /// Domain-separated immutable receipt identity.
    pub fn receipt_id(&self) -> Hash {
        Hash::new_from_chunks(&[
            b"iroha/game-unit-export/v1",
            self.world_id.as_ref(),
            &self.source,
            &self.destination,
            self.unit.unit_id.as_ref(),
            &self.unit.sequence.to_le_bytes(),
        ])
    }
    /// Reserved ordinary-write key authenticated by native finality.
    pub fn witness_key(&self) -> Vec<u8> {
        let mut key = b"iroha/game-partition/export/v1/".to_vec();
        key.extend_from_slice(self.receipt_id().as_ref());
        key
    }
}
/// Deterministic identity of a seeded unit. A foreign root cannot seed the same origin.
pub fn game_unit_id(world: Hash, origin: [u8; 32], nonce: u64) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha/game-unit/v1",
        world.as_ref(),
        &origin,
        &nonce.to_le_bytes(),
    ])
}
impl GamePartitionStateV1 {
    /// Check finite retention and canonical ordering after native mutation or snapshot decode.
    ///
    /// # Errors
    /// Rejects inconsistent local custody, watermarks, peer identities or retention bounds.
    #[expect(
        clippy::suspicious_operation_groupings,
        reason = "an export's `source` is the instance it left, which must be this partition's `local_instance`"
    )]
    pub fn validate(&self) -> Result<(), String> {
        use std::collections::BTreeSet;
        if self.peers.len() > GAME_PARTITION_MAX_PEERS
            || self.units.len() > GAME_PARTITION_MAX_UNITS
            || self.exports.len() > GAME_PARTITION_MAX_RECEIPTS
            || self.imported.len() > GAME_PARTITION_MAX_RECEIPTS
            || self.known_units.len() > GAME_PARTITION_MAX_RECEIPTS
        {
            return Err("game partition retention bound".into());
        }
        let mut peers = BTreeSet::new();
        for peer in &self.peers {
            peer.tracker.validate().map_err(|e| e.to_string())?;
            if peer.tracker.instance == self.local_instance
                || !peers.insert(peer.tracker.instance)
                || peer.tracker.result_scope
                    != crate::sumeragi_finality::CertifiedResultScope::TairaGlobal
                || peer.endpoints.is_empty()
                || peer.endpoints.len() > 4
                || peer.endpoints.iter().any(|s| {
                    s.len() > 512 || !(s.starts_with("http://") || s.starts_with("https://"))
                })
            {
                return Err("invalid game partition peer".into());
            }
        }
        if !self.units.windows(2).all(|w| w[0].unit_id < w[1].unit_id)
            || !self
                .known_units
                .windows(2)
                .all(|w| w[0].unit_id < w[1].unit_id)
        {
            return Err("noncanonical game unit order".into());
        }
        for unit in &self.units {
            if unit.work_remaining > GAME_PARTITION_MAX_WORK
                || self
                    .known_units
                    .iter()
                    .find(|w| w.unit_id == unit.unit_id)
                    .map(|w| w.sequence)
                    != Some(unit.sequence)
            {
                return Err("invalid game unit watermark or work budget".into());
            }
        }
        let mut receipts = BTreeSet::new();
        for receipt in &self.exports {
            if receipt.world_id != self.world_id
                || receipt.source != self.local_instance
                || receipt.kernel_hash != self.kernel_hash
                || receipt.unit.sequence == 0
                || receipt.unit.work_remaining > GAME_PARTITION_MAX_WORK
                || receipt.source_height == 0
                || self.units.iter().any(|u| {
                    u.unit_id == receipt.unit.unit_id && u.sequence <= receipt.unit.sequence
                })
                || !peers.contains(&receipt.destination)
                || !receipts.insert(receipt.receipt_id())
                || self
                    .known_units
                    .iter()
                    .find(|w| w.unit_id == receipt.unit.unit_id)
                    .is_none_or(|w| w.sequence < receipt.unit.sequence)
            {
                return Err("invalid game export custody".into());
            }
        }
        if self.imported.iter().copied().collect::<BTreeSet<_>>().len() != self.imported.len() {
            return Err("duplicate game import receipt".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_and_receipt_roundtrip_bind_every_state_field() {
        let key = iroha_crypto::KeyPair::random();
        let unit = GameUnitV1 {
            unit_id: game_unit_id(Hash::new(b"world"), [1; 32], 7),
            origin: [1; 32],
            owner: AccountId::new(key.public_key().clone()),
            hp: 9,
            x: -7,
            sequence: 1,
            work_remaining: 17,
        };
        let receipt = GameUnitExportV1 {
            world_id: Hash::new(b"world"),
            source: [1; 32],
            destination: [2; 32],
            kernel_hash: [3; 32],
            unit,
            source_height: 3,
        };
        let wire = norito::encode_canonical(&receipt).unwrap();
        assert_eq!(
            norito::decode_canonical::<GameUnitExportV1>(&wire).unwrap(),
            receipt
        );
        let mut changed = receipt.clone();
        changed.unit.hp -= 1;
        assert_ne!(norito::encode_canonical(&changed).unwrap(), wire);
        assert_eq!(changed.witness_key(), receipt.witness_key());
        changed.unit.sequence += 1;
        assert_ne!(changed.witness_key(), receipt.witness_key());
        assert_ne!(
            game_unit_id(receipt.world_id, [1; 32], 7),
            game_unit_id(receipt.world_id, [2; 32], 7)
        );
    }
}

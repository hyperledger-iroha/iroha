//! Decode-compatible wire declarations for retired Sumeragi v1 block sync.
//!
//! These types remain in [`crate::NetworkMessage`] so archival tooling can
//! identify historical envelopes. There is deliberately no synchronizer,
//! request tracker, QC recovery path, roster heuristic, or network handler in
//! this module. Production ingress rejects every `NetworkMessage::BlockSync`;
//! Sumeragi v2 synchronizes only through its context-bound certified body and
//! CommitQC services.
use std::collections::BTreeSet;
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    consensus::{Qc, ValidatorSetCheckpoint},
    peer::PeerId,
};
use norito::codec::{Decode, Encode};
use crate::sumeragi::stake_snapshot::CommitStakeSnapshot;
/// Retired v1 block-sync message declarations.
pub mod message {
    use super::*;
    /// Historical roster hints carried beside a v1 block-sync payload.
    #[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
    pub struct RosterMetadata {
        /// Optional v1 commit certificate.
        pub commit_qc: Option<Qc>,
        /// Optional v1 validator checkpoint.
        pub validator_checkpoint: Option<ValidatorSetCheckpoint>,
        /// Optional v1 stake snapshot aligned to the validator set.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub stake_snapshot: Option<CommitStakeSnapshot>,
    }
    /// Historical request for blocks following a known prefix.
    #[derive(Clone, Debug, Decode, Encode)]
    pub struct GetBlocksAfter {
        /// Requesting peer identifier.
        pub peer_id: PeerId,
        /// Hash of the second-to-latest known block.
        pub prev_hash: Option<HashOf<BlockHeader>>,
        /// Hash of the latest known block.
        pub latest_hash: Option<HashOf<BlockHeader>>,
        /// Block hashes already held by the requester.
        pub seen_blocks: BTreeSet<HashOf<BlockHeader>>,
    }
    /// Historical response containing v1 blocks and auxiliary certificates.
    #[derive(Clone, Debug, Decode, Encode)]
    pub struct ShareBlocks {
        /// Responding peer identifier.
        pub peer_id: PeerId,
        /// Canonical blocks in the response.
        pub blocks: Vec<SignedBlock>,
        /// Optional v1 QCs aligned with `blocks`.
        pub qcs: Vec<Option<Qc>>,
        /// Optional v1 roster metadata aligned with `blocks`.
        pub rosters: Vec<RosterMetadata>,
    }
    /// Historical Sumeragi v1 block-sync envelope.
    #[derive(Clone, Debug, Decode)]
    pub enum Message {
        /// Request blocks following a known prefix.
        GetBlocksAfter(GetBlocksAfter),
        /// Return a batch of blocks and v1 auxiliary evidence.
        ShareBlocks(ShareBlocks),
    }
    impl norito::core::NoritoSerialize for Message {
        fn serialize(
            &self,
            _writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            Err(norito::core::Error::Message(
                "refusing to emit decode-only Sumeragi v1 block-sync message".to_owned(),
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use iroha_p2p::ClassifyTopic;
    use norito::codec::{Decode, Encode};
    use super::message::{GetBlocksAfter, Message};
    #[derive(Encode)]
    enum ArchivedMessage {
        GetBlocksAfter(GetBlocksAfter),
    }
    #[test]
    fn block_sync_envelope_is_archival_decode_only() {
        let peer_id = iroha_data_model::peer::PeerId::new(
            KeyPair::try_random()
                .expect("generate checked archival block-sync peer")
                .public_key()
                .clone(),
        );
        let archived = ArchivedMessage::GetBlocksAfter(GetBlocksAfter {
            peer_id,
            prev_hash: None,
            latest_hash: None,
            seen_blocks: std::collections::BTreeSet::new(),
        })
        .encode();
        let decoded = Message::decode(&mut archived.as_slice())
            .expect("historical v1 block-sync fixture must remain decodable");
        assert!(matches!(decoded, Message::GetBlocksAfter(_)));
        let network = crate::NetworkMessage::BlockSync(Box::new(decoded));
        assert!(!network.is_outbound_allowed());
        assert!(
            norito::core::to_bytes(&network).is_err(),
            "decoded v1 block sync must not be serializable for live networking"
        );
    }
}

//! Compare a borrowed consensus message with its original admitted Norito bytes.
//!
//! The ingress occurrence already retains the canonical bytes. Re-encoding into
//! another `Vec` solely to compare them duplicates an unfunded message-sized
//! buffer. This writer checks the canonical serialization as it is produced.

use std::io::{self, Write};

use super::message::BlockMessage;

struct ExactMessageBytesWriter<'a> {
    expected: &'a [u8],
    consumed: usize,
    rejected: bool,
}

impl Write for ExactMessageBytesWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(end) = self.consumed.checked_add(bytes.len()) else {
            self.rejected = true;
            return Err(io::Error::other("consensus message exceeds admitted bytes"));
        };
        if self.expected.get(self.consumed..end) != Some(bytes) {
            self.rejected = true;
            return Err(io::Error::other(
                "consensus message differs from admitted bytes",
            ));
        }
        self.consumed = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn matches(message: &BlockMessage, expected: &[u8]) -> bool {
    let mut writer = ExactMessageBytesWriter {
        expected,
        consumed: 0,
        rejected: false,
    };
    // Global V2 ingress retains the inner consensus bytes; lane-local and
    // auxiliary ingress retain the complete BlockMessage enum frame.
    let encoded = match message {
        BlockMessage::V2(inner) => norito::codec::encode_adaptive_into(inner, &mut writer),
        _ => norito::codec::encode_adaptive_into(message, &mut writer),
    };
    encoded.is_ok() && !writer.rejected && writer.consumed == expected.len()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::block::consensus_v2 as wire;
    use iroha_model_base::peer::PeerId;
    use norito::codec::Encode as _;

    use super::*;

    fn message() -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"ingress-exact-bytes")),
                index: 4,
                bytes: vec![0xA5; 8],
                sender: 0,
                signature: vec![0x5A],
            }),
        ))
    }

    #[test]
    fn original_bytes_match_and_any_changed_or_short_frame_rejects() {
        let message = message();
        let BlockMessage::V2(inner) = &message else {
            unreachable!("fixture uses global V2 consensus bytes")
        };
        let original = inner.encode();
        assert!(matches(&message, &original));
        assert!(!matches(&message, &message.encode()));
        for index in [0, original.len() / 2, original.len() - 1] {
            let mut changed = original.clone();
            changed[index] ^= 1;
            assert!(!matches(&message, &changed));
        }
        assert!(!matches(&message, &original[..original.len() - 1]));
        let mut extended = original;
        extended.push(0);
        assert!(!matches(&message, &extended));
    }

    #[test]
    fn lane_local_request_matches_its_outer_block_message_frame() {
        let requester = PeerId::from(
            KeyPair::try_from_seed(vec![7; 32], iroha_crypto::Algorithm::BlsNormal)
                .expect("deterministic requester")
                .public_key()
                .clone(),
        );
        let message = BlockMessage::LaneHistoricalRecoveryRequest(Box::new(
            super::super::message::LaneHistoricalRecoveryRequestV1 {
                version: super::super::message::LANE_HISTORICAL_RECOVERY_VERSION_V1,
                requester,
                certificate: None,
                signer_pops: BTreeMap::new(),
                kind: super::super::message::LaneHistoricalRecoveryKindV1::CanonicalBlock {
                    finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"lane-local-finality",
                    )),
                },
            },
        ));
        let original = message.encode();
        assert!(matches(&message, &original));
        assert!(!matches(&message, &original[4..]));
    }
}

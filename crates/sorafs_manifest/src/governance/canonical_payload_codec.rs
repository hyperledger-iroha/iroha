//! Canonical frame projections and borrowed construction for Governance DAG payloads.

use super::{
    GovernanceDagBlockCidPayloadViewV1, GovernanceDagBlockSignaturePayloadViewV1,
    GovernanceDagBlockSignaturePayloadViewWireV1, GovernanceDagBlockV1,
    GovernanceDagHeadSignaturePayloadViewV1, GovernanceDagHeadSignaturePayloadViewWireV1,
    GovernanceDagHeadV1, GovernanceLogNodeCidPayloadViewV1, GovernanceLogNodeV1,
    GovernanceLogSignaturePayloadViewV1, GovernanceLogSignaturePayloadViewWireV1, borrowed_norito,
};

impl norito::core::SerializePayload for GovernanceLogNodeCidPayloadViewV1<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl norito::core::SerializePayload for GovernanceDagBlockCidPayloadViewV1<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<'a> From<&'a GovernanceDagBlockV1> for GovernanceDagBlockSignaturePayloadViewV1<'a> {
    fn from(block: &'a GovernanceDagBlockV1) -> Self {
        Self(GovernanceDagBlockSignaturePayloadViewWireV1 {
            version: block.version,
            block_cid: borrowed_norito::Vec(&block.block_cid),
            prev_block_cid: borrowed_norito::Option(block.prev_block_cid.as_deref()),
            sequence: block.sequence,
            timestamp: block.timestamp,
            publisher_peer_id: borrowed_norito::Vec(&block.publisher_peer_id),
            node: borrowed_norito::Value(&block.node),
        })
    }
}

impl norito::core::SerializePayload for GovernanceDagBlockSignaturePayloadViewV1<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<'a> From<&'a GovernanceDagHeadV1> for GovernanceDagHeadSignaturePayloadViewV1<'a> {
    fn from(head: &'a GovernanceDagHeadV1) -> Self {
        Self(GovernanceDagHeadSignaturePayloadViewWireV1 {
            version: head.version,
            head_block_cid: borrowed_norito::Vec(&head.head_block_cid),
            block_count: head.block_count,
            generated_at: head.generated_at,
            publisher_peer_id: borrowed_norito::Vec(&head.publisher_peer_id),
            checkpoint_cid: borrowed_norito::Option(head.checkpoint_cid.as_deref()),
        })
    }
}

impl norito::core::SerializePayload for GovernanceDagHeadSignaturePayloadViewV1<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<'a> From<&'a GovernanceLogNodeV1> for GovernanceLogSignaturePayloadViewV1<'a> {
    fn from(node: &'a GovernanceLogNodeV1) -> Self {
        Self(GovernanceLogSignaturePayloadViewWireV1 {
            version: node.version,
            node_cid: borrowed_norito::Vec(&node.node_cid),
            prev_cid: borrowed_norito::Option(node.prev_cid.as_deref()),
            timestamp: node.timestamp,
            publisher_peer_id: borrowed_norito::Vec(&node.publisher_peer_id),
            submission_provenance: node
                .submission_provenance
                .as_ref()
                .map(borrowed_norito::Value),
            payload: borrowed_norito::Value(&node.payload),
        })
    }
}

impl norito::core::SerializePayload for GovernanceLogSignaturePayloadViewV1<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

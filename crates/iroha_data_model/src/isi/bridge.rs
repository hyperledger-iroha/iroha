//! Bridge proof ingestion instructions.

use super::*;

isi! {
    /// Submit a bridge proof artifact for verification and registry retention.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitBridgeProof {
        /// Bridge proof payload (ICS or transparent ZK).
        pub proof: crate::bridge::BridgeProof,
    }
}

impl crate::seal::Instruction for SubmitBridgeProof {}

impl SubmitBridgeProof {
    /// Construct a new submission wrapping the provided proof.
    pub fn new(proof: crate::bridge::BridgeProof) -> Self {
        Self { proof }
    }
}

isi! {
    /// Record a bridge receipt and emit a typed bridge event.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordBridgeReceipt {
        /// Bridge receipt payload to record.
        pub receipt: crate::bridge::BridgeReceipt,
    }
}

impl crate::seal::Instruction for RecordBridgeReceipt {}

impl RecordBridgeReceipt {
    /// Construct a new record instruction for the provided receipt.
    pub fn new(receipt: crate::bridge::BridgeReceipt) -> Self {
        Self { receipt }
    }
}

isi! {
    /// Record an SCCP message payload for block-level commitment anchoring.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordSccpMessage {
        /// Canonical SCCP payload bytes.
        pub payload_bytes: Vec<u8>,
    }
}

impl crate::seal::Instruction for RecordSccpMessage {}

impl RecordSccpMessage {
    /// Construct a new SCCP message record instruction for the provided payload bytes.
    pub fn new(payload_bytes: Vec<u8>) -> Self {
        Self { payload_bytes }
    }
}

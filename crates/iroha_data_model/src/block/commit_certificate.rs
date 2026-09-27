//! Sumeragi finality proof stored with a committed block.
//!
//! A committed block is persisted and served as one canonical `SignedBlockWire` frame that
//! carries its result and, from the first consensus height on, a [`CommitCertificate`]: the
//! Sumeragi core header of the block, its `CommitQC` and the preimage of the certified execution
//! result `R` (`specs/sumeragi.md` §3.2, §3.4, §4.1). The data model keeps the three parts as
//! opaque canonical Norito bytes so it does not depend on the consensus core; `iroha_core`
//! decodes and verifies them, and Torii exposes a decoded view.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::vec::Vec;

/// Finality proof of one committed block.
///
/// All three fields are untrusted bytes until `iroha_core` has decoded them with the consensus
/// frame limit and verified the `CommitQC` against the committee of the block's height. Genesis
/// has no certificate (its authenticator is the genesis signature).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::commit_certificate::CommitCertificate")]
pub struct CommitCertificate {
    /// Canonical Norito encoding of the Sumeragi core block header
    /// (`iroha_sumeragi::message::BlockHeader`, §3.2) whose block hash the `CommitQC` certifies.
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    pub consensus_header: Vec<u8>,
    /// Canonical Norito encoding of the block's `CommitQC` (`iroha_sumeragi::message::Qc`, §3.4).
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    pub commit_qc: Vec<u8>,
    /// Preimage of the certified execution result: the canonical Norito encoding of the node's
    /// `ExecutionResultCommitment`, with `R = H("iroha/sumeragi/result/v1" ‖ result_preimage)`
    /// (§4.1).
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    pub result_preimage: Vec<u8>,
}

impl CommitCertificate {
    /// Assemble a certificate from the canonical encodings of its three parts.
    #[must_use]
    pub fn new(consensus_header: Vec<u8>, commit_qc: Vec<u8>, result_preimage: Vec<u8>) -> Self {
        Self {
            consensus_header,
            commit_qc,
            result_preimage,
        }
    }

    /// Total number of opaque payload bytes carried by the certificate.
    #[must_use]
    pub fn payload_len(&self) -> usize {
        self.consensus_header
            .len()
            .saturating_add(self.commit_qc.len())
            .saturating_add(self.result_preimage.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::DecodeAll as _;

    fn sample() -> CommitCertificate {
        CommitCertificate::new(vec![1, 2, 3], vec![4, 5], vec![6; 40])
    }

    #[test]
    fn new_keeps_parts_in_order() {
        let cert = sample();
        assert_eq!(cert.consensus_header, vec![1, 2, 3]);
        assert_eq!(cert.commit_qc, vec![4, 5]);
        assert_eq!(cert.result_preimage, vec![6; 40]);
    }

    #[test]
    fn payload_len_sums_all_parts() {
        assert_eq!(sample().payload_len(), 45);
        assert_eq!(
            CommitCertificate::new(Vec::new(), Vec::new(), Vec::new()).payload_len(),
            0
        );
    }

    #[test]
    fn codec_round_trip() {
        let cert = sample();
        let bytes = cert.encode();
        let decoded = CommitCertificate::decode_all(&mut bytes.as_slice()).expect("decode");
        assert_eq!(decoded, cert);
        let framed = norito::to_bytes(&cert).expect("framed encode");
        let decoded: CommitCertificate = norito::decode_from_bytes(&framed).expect("framed decode");
        assert_eq!(decoded, cert);
    }

    #[test]
    fn json_round_trip_uses_base64() {
        let cert = sample();
        let json = norito::json::to_json(&cert).expect("json");
        assert_eq!(
            json,
            r#"{"consensus_header":"AQID","commit_qc":"BAU=","result_preimage":"BgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBg=="}"#
        );
        let parsed: CommitCertificate = norito::json::from_str(&json).expect("parse");
        assert_eq!(parsed, cert);
        assert!(
            norito::json::from_str::<CommitCertificate>(
                r#"{"consensus_header":"","commit_qc":"","result_preimage":"","extra":1}"#
            )
            .is_err()
        );
    }
}

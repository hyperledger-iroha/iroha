use super::*;
use crate::{
    account::AccountId,
    metadata::Metadata,
    nexus::{DataSpaceId, LaneRelayEnvelope, ProofBlob},
};

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Set or clear emergency validators used for lane relay quorum recovery.
    ///
    /// This instruction is disabled by default and requires
    /// `nexus.lane_relay_emergency.enabled = true`. When enabled, the transaction authority
    /// must be a multisig account meeting the configured threshold/member minimums
    /// (defaults to 3-of-5).
    pub struct SetLaneRelayEmergencyValidators {
        /// Dataspace whose validator pool is being overridden.
        pub dataspace_id: DataSpaceId,
        /// Validators added to the pool when quorum is at risk.
        pub validators: Vec<AccountId>,
        /// Optional block height (inclusive) after which the override expires.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub expires_at_height: Option<u64>,
        /// Optional metadata describing the override decision.
        #[norito(default)]
        pub metadata: Metadata,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Persist a verified private-source lane relay so contracts can consume it by reference.
    pub struct RegisterVerifiedLaneRelay {
        /// Canonical lane relay envelope being registered.
        pub envelope: LaneRelayEnvelope,
        /// FASTPQ/AXT proof blob used to verify the relay payload.
        pub proof_blob: ProofBlob,
    }
}

impl PartialOrd for RegisterVerifiedLaneRelay {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegisterVerifiedLaneRelay {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl crate::seal::Instruction for SetLaneRelayEmergencyValidators {}
impl crate::seal::Instruction for RegisterVerifiedLaneRelay {}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use norito::codec::Encode;

    use super::*;
    use crate::{
        block::{BlockHeader, consensus::LaneBlockCommitment},
        nexus::LaneId,
    };

    fn sample_commitment(height: u64) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::new(3),
            dataspace_id: DataSpaceId::new(2),
            tx_count: 1,
            total_local_micro: 10,
            total_xor_due_micro: 5,
            total_xor_after_haircut_micro: 4,
            total_xor_variance_micro: 1,
            swap_metadata: None,
            receipts: Vec::new(),
        }
    }

    fn sample_header(height: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        )
    }

    fn sample_envelope(height: u64) -> LaneRelayEnvelope {
        LaneRelayEnvelope::new(
            sample_header(height),
            None,
            None,
            sample_commitment(height),
            0,
        )
        .expect("valid lane relay envelope")
    }

    #[test]
    fn register_verified_lane_relay_order_uses_canonical_encoding() {
        let left = RegisterVerifiedLaneRelay {
            envelope: sample_envelope(5),
            proof_blob: ProofBlob {
                payload: vec![0x01],
                expiry_slot: None,
            },
        };
        let right = RegisterVerifiedLaneRelay {
            envelope: sample_envelope(5),
            proof_blob: ProofBlob {
                payload: vec![0x02],
                expiry_slot: None,
            },
        };

        assert_eq!(
            left.partial_cmp(&right),
            Some(left.encode().cmp(&right.encode()))
        );
        assert_eq!(left.cmp(&right), left.encode().cmp(&right.encode()));
    }
}

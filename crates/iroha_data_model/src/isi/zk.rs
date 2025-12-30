//! ZK-related instructions.
//!
//! This module defines instructions for submitting proofs for verification
//! against on-chain verifying keys.

use iroha_crypto::Hash;

use super::*;
use crate::{
    asset::definition::ConfidentialPolicyMode, confidential::ConfidentialEncryptedPayload,
};

isi! {
    /// Verify a zero-knowledge proof against a verifying key.
    ///
    /// This instruction records verification result in WSV. Backends and
    /// cryptographic verification are provided by `iroha_core` under feature
    /// flags; this data model type acts as a transport envelope.
    pub struct VerifyProof {
        /// Proof attachment containing the proof and either a VK reference
        /// or an inline verifying key.
        pub attachment: crate::proof::ProofAttachment,
    }
}

impl crate::seal::Instruction for VerifyProof {}

impl VerifyProof {
    /// Construct a new `VerifyProof` instruction from a proof attachment.
    pub fn new(attachment: crate::proof::ProofAttachment) -> Self {
        Self { attachment }
    }
}

isi! {
    /// Prune proof registry entries according to the on-chain retention policy.
    ///
    /// When `backend` is `None`, all backends are considered; otherwise only
    /// the matching backend is pruned. Retention limits (cap/grace/batch) come
    /// from the `zk` configuration, keeping pruning deterministic.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct PruneProofs {
        /// Optional backend label to restrict pruning scope (e.g., `halo2/ipa`).
        pub backend: Option<String>,
    }
}

impl crate::seal::Instruction for PruneProofs {}

impl PruneProofs {
    /// Construct a new prune request.
    pub fn new(backend: Option<String>) -> Self {
        Self { backend }
    }
}

// --- ZK Assets ---

/// Shielded asset mode.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Decode,
    norito::codec::Encode,
    iroha_schema::IntoSchema,
)]
pub enum ZkAssetMode {
    /// Only shielded ledger (no public account balances).
    ZkNative,
    /// Hybrid: public balances plus shielded ledger; allows shield/unshield when policy permits.
    Hybrid,
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ZkAssetMode {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ZkAssetMode::ZkNative => "ZkNative",
            ZkAssetMode::Hybrid => "Hybrid",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ZkAssetMode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "ZkNative" => Ok(ZkAssetMode::ZkNative),
            "Hybrid" => Ok(ZkAssetMode::Hybrid),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}

isi! {
    /// Register a ZK-capable asset definition with policy and verifying keys.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterZkAsset {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Asset mode.
        pub mode: ZkAssetMode,
        /// Allow shielding from public to shielded.
        pub allow_shield: bool,
        /// Allow unshielding from shielded to public.
        pub allow_unshield: bool,
        /// Verifying key for shielded transfers.
        pub vk_transfer: Option<crate::proof::VerifyingKeyId>,
        /// Verifying key for unshield proofs.
        pub vk_unshield: Option<crate::proof::VerifyingKeyId>,
        /// Optional verifying key for shield proofs.
        pub vk_shield: Option<crate::proof::VerifyingKeyId>,
    }
}

impl crate::seal::Instruction for RegisterZkAsset {}
impl RegisterZkAsset {
    /// Construct a new `RegisterZkAsset` instruction.
    pub fn new(
        asset: AssetDefinitionId,
        mode: ZkAssetMode,
        allow_shield: bool,
        allow_unshield: bool,
        vk_transfer: Option<crate::proof::VerifyingKeyId>,
        vk_unshield: Option<crate::proof::VerifyingKeyId>,
        vk_shield: Option<crate::proof::VerifyingKeyId>,
    ) -> Self {
        Self {
            asset,
            mode,
            allow_shield,
            allow_unshield,
            vk_transfer,
            vk_unshield,
            vk_shield,
        }
    }
}

isi! {
    /// Schedule a confidential policy transition for an asset definition.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ScheduleConfidentialPolicyTransition {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Policy mode that becomes active at `effective_height`.
        pub new_mode: ConfidentialPolicyMode,
        /// Block height at which the transition must be applied.
        pub effective_height: u64,
        /// Deterministic identifier for the transition (governance audit/replay).
        pub transition_id: Hash,
        /// Optional conversion window length (in blocks) prior to finalizing the transition.
        pub conversion_window: Option<u64>,
    }
}

impl crate::seal::Instruction for ScheduleConfidentialPolicyTransition {}
impl ScheduleConfidentialPolicyTransition {
    /// Construct a new `ScheduleConfidentialPolicyTransition` instruction.
    pub fn new(
        asset: AssetDefinitionId,
        new_mode: ConfidentialPolicyMode,
        effective_height: u64,
        transition_id: Hash,
        conversion_window: Option<u64>,
    ) -> Self {
        Self {
            asset,
            new_mode,
            effective_height,
            transition_id,
            conversion_window,
        }
    }
}

isi! {
    /// Cancel a pending confidential policy transition for an asset definition.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct CancelConfidentialPolicyTransition {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Identifier of the transition to cancel.
        pub transition_id: Hash,
    }
}

impl crate::seal::Instruction for CancelConfidentialPolicyTransition {}
impl CancelConfidentialPolicyTransition {
    /// Construct a new `CancelConfidentialPolicyTransition` instruction.
    pub fn new(asset: AssetDefinitionId, transition_id: Hash) -> Self {
        Self {
            asset,
            transition_id,
        }
    }
}

isi! {
    /// Shield public funds into the asset's shielded ledger by appending a note commitment.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct Shield {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Account to debit.
        pub from: AccountId,
        /// Public amount to debit.
        pub amount: u128,
        /// Output note commitment (opaque 32 bytes under the asset's note scheme).
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Encrypted recipient payload (versioned envelope).
        pub enc_payload: ConfidentialEncryptedPayload,
    }
}

impl crate::seal::Instruction for Shield {}
impl Shield {
    /// Construct a new Shield instruction.
    pub fn new(
        asset: AssetDefinitionId,
        from: AccountId,
        amount: u128,
        note_commitment: [u8; 32],
        enc_payload: impl Into<ConfidentialEncryptedPayload>,
    ) -> Self {
        Self {
            asset,
            from,
            amount,
            note_commitment,
            enc_payload: enc_payload.into(),
        }
    }
}

isi! {
    /// Private-to-private transfer within a shielded ledger.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ZkTransfer {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Spent nullifiers.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub inputs: Vec<[u8; 32]>,
        /// Output note commitments.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub outputs: Vec<[u8; 32]>,
        /// Proof attachment for the transfer.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

impl crate::seal::Instruction for ZkTransfer {}
impl ZkTransfer {
    /// Construct a new `ZkTransfer` instruction.
    pub fn new(
        asset: AssetDefinitionId,
        inputs: Vec<[u8; 32]>,
        outputs: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            asset,
            inputs,
            outputs,
            proof,
            root_hint,
        }
    }
}

isi! {
    /// Unshield private funds into a public account balance.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct Unshield {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Recipient account to credit.
        pub to: AccountId,
        /// Public amount to credit.
        pub public_amount: u128,
        /// Spent nullifiers.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub inputs: Vec<[u8; 32]>,
        /// Proof attachment for the unshield.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

impl crate::seal::Instruction for Unshield {}
impl Unshield {
    /// Construct a new Unshield instruction.
    pub fn new(
        asset: AssetDefinitionId,
        to: AccountId,
        public_amount: u128,
        inputs: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            asset,
            to,
            public_amount,
            inputs,
            proof,
            root_hint,
        }
    }
}

// --- ZK Voting ---

isi! {
    /// Create an anonymous election.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct CreateElection {
        /// Unique election id.
        pub election_id: String,
        /// Number of options (K).
        pub options: u32,
        /// Merkle root of eligible voters.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub eligible_root: [u8; 32],
        /// Start timestamp (ms since epoch).
        pub start_ts: u64,
        /// End timestamp (ms since epoch).
        pub end_ts: u64,
        /// Verifying key for ballot proofs.
        pub vk_ballot: crate::proof::VerifyingKeyId,
        /// Verifying key for tally proofs.
        pub vk_tally: crate::proof::VerifyingKeyId,
        /// Domain separation tag for ballot nullifiers.
        pub domain_tag: String,
    }
}

impl crate::seal::Instruction for CreateElection {}

isi! {
    /// Submit a private ballot for an election.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitBallot {
        /// Election id.
        pub election_id: String,
        /// Encrypted ballot payload (opaque bytes).
        pub ciphertext: Vec<u8>,
        /// ZK proof of eligibility and well-formed vote.
        pub ballot_proof: crate::proof::ProofAttachment,
        /// Unique ballot nullifier to prevent double-voting.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub nullifier: [u8; 32],
    }
}

impl crate::seal::Instruction for SubmitBallot {}

isi! {
    /// Finalize an election by verifying the tally proof and recording the result.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct FinalizeElection {
        /// Election id.
        pub election_id: String,
        /// Public tally per option.
        pub tally: Vec<u64>,
        /// ZK proof that `tally` is consistent with submitted ballots.
        pub tally_proof: crate::proof::ProofAttachment,
    }
}

impl crate::seal::Instruction for FinalizeElection {}

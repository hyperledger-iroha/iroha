//! FASTPQ-specific data structures shared between the host and prover.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;

use crate::{account::AccountId, asset::id::AssetDefinitionId};

/// Metadata key storing Norito-encoded [`TransferTranscript`] collections for
/// FASTPQ gadgets.
pub const TRANSFER_TRANSCRIPTS_METADATA_KEY: &str = "transfer_transcripts";

/// Transcript describing one or more deterministic asset transfers within a transaction.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferTranscript {
    /// Hash of the transaction entrypoint (`hash_as_entrypoint`) that emitted this transcript.
    pub batch_hash: Hash,
    /// Grouped transfer deltas covered by the transcript.
    pub deltas: Vec<TransferDeltaTranscript>,
    /// Host-side digest of the authority set (signers, quorum, etc.).
    pub authority_digest: Hash,
    /// Optional Poseidon digest of the preimage `(from, to, asset, amount, batch_hash)`.
    ///
    /// Present for single-delta transcripts; omitted for multi-delta batches.
    pub poseidon_preimage_digest: Option<Hash>,
}

/// Per-transfer delta describing the balance change for the sender and receiver.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferDeltaTranscript {
    /// Source account.
    pub from_account: AccountId,
    /// Destination account.
    pub to_account: AccountId,
    /// Asset definition being transferred.
    pub asset_definition: AssetDefinitionId,
    /// Amount being transferred.
    pub amount: Numeric,
    /// Sender balance before the transfer.
    pub from_balance_before: Numeric,
    /// Sender balance after the transfer.
    pub from_balance_after: Numeric,
    /// Receiver balance before the transfer.
    pub to_balance_before: Numeric,
    /// Receiver balance after the transfer.
    pub to_balance_after: Numeric,
    /// Placeholder for the sender's Merkle proof.
    ///
    /// Populated with the sender's sparse Merkle proof once the witness plumbing lands.
    pub from_merkle_proof: Option<Vec<u8>>,
    /// Placeholder for the receiver's Merkle proof.
    ///
    /// Populated with the receiver's sparse Merkle proof once the witness plumbing lands.
    pub to_merkle_proof: Option<Vec<u8>>,
}

impl TransferDeltaTranscript {
    /// Attach sparse Merkle proofs for sender and receiver accounts.
    #[must_use]
    pub fn with_merkle_proofs(mut self, from_proof: Vec<u8>, to_proof: Vec<u8>) -> Self {
        self.from_merkle_proof = Some(from_proof);
        self.to_merkle_proof = Some(to_proof);
        self
    }
}

/// Canonical FASTPQ transition batch recorded in execution witnesses.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct FastpqTransitionBatch {
    /// Parameter set name (e.g., `fastpq-lane-balanced`).
    pub parameter: String,
    /// Public inputs committed by the prover and replayed by the verifier.
    pub public_inputs: FastpqPublicInputs,
    /// Ordered transitions the prover must replay.
    pub transitions: Vec<FastpqStateTransition>,
    /// Arbitrary metadata (e.g., entry hash, transcript count).
    pub metadata: BTreeMap<String, Vec<u8>>,
}

/// Canonical FASTPQ state transition.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct FastpqStateTransition {
    /// Schema-qualified logical key (asset/account path).
    pub key: Vec<u8>,
    /// Pre-state value prior to executing the transition.
    pub pre_value: Vec<u8>,
    /// Post-state value after executing the transition.
    pub post_value: Vec<u8>,
    /// Operation selector describing the transition semantics.
    pub operation: FastpqOperationKind,
}

/// FASTPQ operation selector recorded in batches.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
#[norito(tag = "kind", content = "payload")]
pub enum FastpqOperationKind {
    /// Asset transfer between two existing accounts.
    Transfer,
    /// Asset mint increasing the circulating supply.
    Mint,
    /// Asset burn decreasing the circulating supply.
    Burn,
    /// Grant a permission to a role.
    RoleGrant(FastpqRolePermissionDelta),
    /// Revoke a permission from a role.
    RoleRevoke(FastpqRolePermissionDelta),
    /// Metadata mutation (domains, accounts, assets, etc.).
    MetaSet,
}

/// Public inputs committed by the FASTPQ prover.
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct FastpqPublicInputs {
    /// Data-space identifier (little-endian UUID bytes).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Permission table commitment for this slot.
    pub perm_root: [u8; 32],
    /// Transaction set hash recorded by the scheduler.
    pub tx_set_hash: [u8; 32],
}

/// Role permission delta descriptor used in FASTPQ transcripts.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct FastpqRolePermissionDelta {
    /// Canonical role identifier (little-endian bytes).
    pub role_id: Vec<u8>,
    /// Canonical permission identifier (little-endian bytes).
    pub permission_id: Vec<u8>,
    /// Epoch at which the change becomes effective (little-endian u64).
    pub epoch: u64,
}

/// Bundle of transcripts keyed by the transaction entrypoint hash.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferTranscriptBundle {
    /// Entry hash (`hash_as_entrypoint`) associated with the transcripts.
    pub entry_hash: Hash,
    /// Recorded transcripts for the entry.
    pub transcripts: Vec<TransferTranscript>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::{account::AccountId, asset::id::AssetDefinitionId, domain::DomainId, name::Name};

    const SIGNATORY: &str =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";

    fn account(label: &str) -> AccountId {
        let _ = label;
        AccountId::new(
            "wonderland".parse().expect("domain id"),
            SIGNATORY.parse().expect("valid public key"),
        )
    }

    fn asset(label: &str) -> AssetDefinitionId {
        let name = Name::from_str(label).expect("valid asset name");
        let domain = DomainId::from_str("wonderland").expect("valid domain id");
        AssetDefinitionId::new(domain, name)
    }

    #[test]
    fn transfer_delta_transcript_attaches_merkle_proofs() {
        let delta = TransferDeltaTranscript {
            from_account: account("alice"),
            to_account: account("bob"),
            asset_definition: asset("xor"),
            amount: Numeric::new(10, 0),
            from_balance_before: Numeric::new(100, 0),
            from_balance_after: Numeric::new(90, 0),
            to_balance_before: Numeric::new(50, 0),
            to_balance_after: Numeric::new(60, 0),
            from_merkle_proof: None,
            to_merkle_proof: None,
        };
        let updated = delta.with_merkle_proofs(vec![1, 2, 3], vec![4, 5, 6]);
        assert_eq!(updated.from_merkle_proof.as_deref(), Some(&[1, 2, 3][..]));
        assert_eq!(updated.to_merkle_proof.as_deref(), Some(&[4, 5, 6][..]));
    }
}

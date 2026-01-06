//! Governance instructions (sketch)
//!
//! Minimal instruction shells to start wiring governance flows described in
//! `gov.md`. Execution is not implemented here; these types exist to anchor
//! serialization, registry, and CLI/endpoint integration.

use std::{string::String, vec::Vec};

#[cfg(feature = "governance")]
use crate::governance::types::ParliamentBody;
#[cfg(not(feature = "governance"))]
type ParliamentBody = ();

use norito::codec::{Decode, Encode};

#[cfg(not(feature = "governance"))]
pub use self::at_window_placeholder::AtWindow;
#[cfg(feature = "governance")]
pub use crate::governance::types::AtWindow;
use crate::{prelude::*, smart_contract::manifest::ManifestProvenance};

#[cfg(not(feature = "governance"))]
mod at_window_placeholder {
    use super::*;

    #[derive(
        Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Inclusive governance enactment window expressed in block heights.
    pub struct AtWindow {
        /// Lower bound (inclusive) of the enactment window.
        pub lower: u64,
        /// Upper bound (inclusive) of the enactment window.
        pub upper: u64,
    }
}

/// Voting mode for a referendum
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
pub enum VotingMode {
    /// Zero-knowledge voting flow (default ballot type).
    Zk,
    /// Plain-text quadratic voting flow.
    Plain,
}

/// Council derivation method
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
)]
pub enum CouncilDerivationKind {
    /// Derived from verified VRF proofs
    Vrf,
    /// Derived from deterministic fallback (no proofs available)
    #[default]
    Fallback,
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for CouncilDerivationKind {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            CouncilDerivationKind::Vrf => "Vrf",
            CouncilDerivationKind::Fallback => "Fallback",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for CouncilDerivationKind {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Vrf" => Ok(CouncilDerivationKind::Vrf),
            "Fallback" => Ok(CouncilDerivationKind::Fallback),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}

/// Propose deployment of an IVM bytecode (`.to`) by hash
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeDeployContract {
    /// Namespace for governance gating
    pub namespace: String,
    /// Contract identifier within the namespace
    pub contract_id: String,
    /// Blake2b-32 hash of the compiled `.to` bytecode slated for deployment (lowercase hex).
    pub code_hash_hex: String,
    /// Blake2b-32 hash of the ABI surface expected by the host (lowercase hex).
    pub abi_hash_hex: String,
    /// ABI version (e.g., "1") supplied by the proposer.
    pub abi_version: String,
    /// Optional enactment window override (inclusive)
    pub window: Option<AtWindow>,
    /// Optional voting mode for the referendum created by this proposal (default Zk)
    pub mode: Option<VotingMode>,
    /// Optional manifest provenance to attest the contract manifest on enactment.
    pub manifest_provenance: Option<ManifestProvenance>,
}

impl crate::seal::Instruction for ProposeDeployContract {}

/// Cast a ZK ballot (default voting mode)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct CastZkBallot {
    /// Election/referendum identifier (opaque)
    pub election_id: String,
    /// Base64-encoded proof bytes (envelope routing determines backend)
    pub proof_b64: String,
    /// Opaque JSON of public inputs as a UTF-8 string (for Norito)
    pub public_inputs_json: String,
}

impl crate::seal::Instruction for CastZkBallot {}

#[cfg(feature = "zk-ballot")]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
/// Sketch of a ZK ballot proof envelope.
///
/// Opaque container for the ballot proof and minimal public context.
/// Threaded under a feature for prototyping; not used by execution yet.
pub struct BallotProof {
    /// Proof backend tag (e.g., "halo2/ipa" or "halo2/pasta/tiny-add").
    pub backend: iroha_schema::Ident,
    /// Opaque proof envelope bytes (ZK1 or H2* container).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub envelope_bytes: Vec<u8>,
    /// Optional eligibility root hint (32-byte) to bind verification to a known root.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub root_hint: Option<[u8; 32]>,
    /// Optional owner account id (when the circuit commits to it in public inputs).
    pub owner: Option<crate::account::AccountId>,
    /// Optional nullifier hint (32-byte) derived from the proof's commitment.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub nullifier: Option<[u8; 32]>,
}

/// Cast a non‑ZK quadratic ballot (optional mode)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct CastPlainBallot {
    /// Identifier of the referendum this ballot targets.
    pub referendum_id: String,
    /// Account submitting the ballot.
    pub owner: AccountId,
    /// Quadratic voting credit amount committed by the ballot.
    pub amount: u128,
    /// Duration of the lock in blocks.
    pub duration_blocks: u64,
    /// 0=Aye, 1=Nay, 2=Abstain
    pub direction: u8,
}

impl crate::seal::Instruction for CastPlainBallot {}

/// Enact an approved referendum (host validates certificate separately)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct EnactReferendum {
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    /// Identifier of the referendum to enact.
    pub referendum_id: [u8; 32],
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    /// Blake2b-32 hash of the referendum preimage (proposal payload).
    pub preimage_hash: [u8; 32],
    /// Window describing when enactment is valid.
    pub at_window: AtWindow,
}

impl crate::seal::Instruction for EnactReferendum {}

/// Finalize a referendum: compute tally and emit Approved/Rejected events
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FinalizeReferendum {
    /// Identifier of the referendum to finalize.
    pub referendum_id: String,
    /// Deterministic proposal id this referendum governs
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub proposal_id: [u8; 32],
}

impl crate::seal::Instruction for FinalizeReferendum {}

/// Record a council approval for a governance proposal.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema,
)]
pub struct ApproveGovernanceProposal {
    /// Parliament body granting the approval (defaults to Agenda Council).
    #[norito(default)]
    pub body: ParliamentBody,
    /// Deterministic proposal id (Blake2b-32) being approved by the council.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub proposal_id: [u8; 32],
}

impl crate::seal::Instruction for ApproveGovernanceProposal {}

/// Persist a council membership for an epoch.
///
/// This instruction records `members` for `epoch` in the WSV. It includes a
/// `candidates_count` for auditability and a `derived_by` flag to indicate
/// whether members were derived from verified VRF proofs or via a deterministic
/// fallback.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct PersistCouncilForEpoch {
    /// Epoch index
    pub epoch: u64,
    /// Council members in deterministic order
    pub members: Vec<crate::account::AccountId>,
    /// Alternates that can replace members who decline or are ineligible.
    #[norito(default)]
    pub alternates: Vec<crate::account::AccountId>,
    /// Number of candidates whose VRF proofs verified successfully.
    #[norito(default)]
    pub verified: u32,
    /// Total number of candidates considered
    pub candidates_count: u32,
    /// Derivation method
    pub derived_by: CouncilDerivationKind,
}

impl crate::seal::Instruction for PersistCouncilForEpoch {}

/// Discipline event recorded for a citizen assigned to a governance role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "event", content = "value", rename_all = "kebab-case")
)]
pub enum CitizenServiceEvent {
    /// Citizen declined the assignment.
    Decline,
    /// Citizen failed to appear for the assignment.
    NoShow,
    /// Citizen committed misconduct during the assignment.
    Misconduct,
}

impl core::cmp::PartialOrd for CitizenServiceEvent {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::cmp::Ord for CitizenServiceEvent {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

/// Record a citizen service discipline event (decline, no-show, misconduct) for a role/epoch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordCitizenServiceOutcome {
    /// Citizen account receiving the record.
    pub owner: AccountId,
    /// Epoch index associated with the assignment.
    pub epoch: u64,
    /// Governance role label (e.g., "council", "`policy_jury`").
    pub role: String,
    /// Recorded event kind.
    pub event: CitizenServiceEvent,
}

impl crate::seal::Instruction for RecordCitizenServiceOutcome {}

/// Bond the configured citizenship amount to join the citizen registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct RegisterCitizen {
    /// Account receiving citizenship.
    pub owner: AccountId,
    /// Amount to bond (must meet or exceed the configured floor).
    pub amount: u128,
}

impl crate::seal::Instruction for RegisterCitizen {}

/// Unbond and remove a citizen from the registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct UnregisterCitizen {
    /// Account to remove from the registry.
    pub owner: AccountId,
}

impl crate::seal::Instruction for UnregisterCitizen {}

/// Slash a governance bond lock for a referendum.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct SlashGovernanceLock {
    /// Identifier of the referendum whose lock is being slashed.
    pub referendum_id: String,
    /// Account whose bond lock will be reduced.
    pub owner: AccountId,
    /// Amount (smallest units) to slash from the lock.
    pub amount: u128,
    /// Human-readable reason recorded with the slash event.
    pub reason: String,
}

impl crate::seal::Instruction for SlashGovernanceLock {}

/// Restitute a previously slashed governance bond lock.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct RestituteGovernanceLock {
    /// Identifier of the referendum whose lock is being restored.
    pub referendum_id: String,
    /// Account receiving the restitution.
    pub owner: AccountId,
    /// Amount (smallest units) to restore to the lock.
    pub amount: u128,
    /// Human-readable reason recorded with the restitution event.
    pub reason: String,
}

impl crate::seal::Instruction for RestituteGovernanceLock {}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "governance"))]
    use norito::core::DecodeFromSlice;

    use super::*;
    #[test]
    fn encode_roundtrip_basic() {
        let p = ProposeDeployContract {
            namespace: "apps".into(),
            contract_id: "my.contract".into(),
            code_hash_hex: "aa".repeat(32),
            abi_hash_hex: "bb".repeat(32),
            abi_version: "1".into(),
            window: Some(AtWindow {
                lower: 10,
                upper: 20,
            }),
            mode: Some(VotingMode::Zk),
            manifest_provenance: None,
        };
        let enc = norito::codec::Encode::encode(&p);
        let mut cur = enc.as_slice();
        let dec = ProposeDeployContract::decode(&mut cur).unwrap();
        assert_eq!(p, dec);
    }

    #[test]
    fn at_window_roundtrip() {
        let win = AtWindow { lower: 1, upper: 2 };
        let enc = norito::codec::Encode::encode(&win);
        let mut cur = enc.as_slice();
        let dec = AtWindow::decode(&mut cur).unwrap();
        assert_eq!(win, dec);
    }

    #[test]
    fn approve_proposal_roundtrip() {
        let ins = ApproveGovernanceProposal {
            body: ParliamentBody::AgendaCouncil,
            proposal_id: [0xAA; 32],
        };
        let enc = norito::codec::Encode::encode(&ins);
        let mut cur = enc.as_slice();
        let dec = ApproveGovernanceProposal::decode(&mut cur).unwrap();
        assert_eq!(ins, dec);
    }

    #[cfg(not(feature = "governance"))]
    #[test]
    fn at_window_decodes_from_slice_via_norito() {
        let window = AtWindow {
            lower: 10,
            upper: 42,
        };
        let bytes = norito::codec::Encode::encode(&window);
        let (decoded, used) = <AtWindow as DecodeFromSlice>::decode_from_slice(&bytes)
            .expect("decode_from_slice should succeed");
        assert_eq!(decoded, window);
        assert_eq!(used, bytes.len());
    }
}

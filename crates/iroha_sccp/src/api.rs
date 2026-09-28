//! SCCP v1 Torii read-API records (`specs/sccp.md` §6).
//!
//! These are the wire types every Taira peer serves from committed state and that wallets, the
//! CLI and the SDKs decode, so Torii and the wallet agree on one layout. Each record is Norito
//! (binary, bare or headered) and Norito JSON, with a stable schema name
//! `iroha_sccp::api::<Type>`. JSON follows the data-model conventions of the records Torii
//! serves next to them: fixed-size byte arrays (digests, addresses, signatures, Merkle
//! siblings) are hex strings, and the variable-length transfer payload is padded standard
//! base64, exactly as in `SccpOutboundMessageRecordV1`.
//!
//! Proof material:
//!
//! - [`SccpRosterViewV1`]: one roster generation as destinations see it (§3.7, `RosterV1` of
//!   §5.2.2) with its digest.
//! - [`SccpSignatureSetV1`]: a §3.8 signature set, one 65-byte signature per set bitmap bit in
//!   ascending bit order.
//! - [`SccpHistoryProofV1`]: a historical SCCP block and its history path (§3.5).
//! - [`SccpMessageProofBundleV1`] (`GET /v1/sccp/messages/{id}/proof`) and
//!   [`SccpControlProofBundleV1`] (`GET /v1/sccp/controls/{network}/{revision}/{nonce}/proof`):
//!   everything one `finalizeFromTaira*`/`voidExpired*` or `applyControl*` call needs.
//! - [`SccpRotationStepV1`] and [`SccpRotationChainV1`] (`GET /v1/sccp/rosters/rotations`): the
//!   ordered catch-up chain for `rotateRosters`, with the first unattested handoff when the chain
//!   is broken.
//!
//! Nothing here is trusted by its reader: wallets recompute every digest, roster hash, signature
//! and Merkle path locally (`iroha_sccp_wallet::pure`). The conversions below map the records
//! onto the contract-visible `v1` structures that the verifiers and ABI encoders take.
//!
//! - [`SccpCapabilitiesV1`] (`GET /v1/sccp/capabilities`): identity, parameters and
//!   attestation health.
//!
//! Routes that serve stored data-model records (registry, outbound records, rosters, bridge
//! keys) use those records directly.
//!
//! TODO(ws35): add the remaining §6 read-API records (attestation views, history, governance).

use iroha_data_model::{bridge::SccpNetworkV1, sccp::attestation::SccpAttestationStatementV1};
use norito::codec::{Decode, Encode};

use crate::v1::{
    constants::{
        MAX_BLOCK_PATH, MAX_HISTORY_PATH, MAX_PAYLOAD_BYTES, MAX_ROSTER_MEMBERS,
        MAX_ROTATIONS_PER_CALL, SIGNATURE_BYTES,
    },
    eip712::AttestationFieldsV1,
    proof::{ControlProofV1, HistoryBlockV1, HistoryProofV1, MessageProofV1},
    roster::{RosterError, RosterV1},
    signature::SignatureSetV1,
};

/// A record exceeds a protocol bound before any cryptographic check (§3.4, §3.5, §3.7, §3.8,
/// §5.1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiShapeError {
    /// More than 31 roster members.
    TooManyMembers,
    /// More than 31 signatures, or a signature count that differs from the bitmap popcount.
    BadSignatureCount,
    /// A block path longer than 9 siblings.
    BlockPathTooLong,
    /// A history path longer than 32 siblings.
    HistoryPathTooLong,
    /// A transfer payload longer than 4096 bytes.
    PayloadTooLong,
    /// More than 16 rotation steps in one chain page.
    TooManyRotations,
}

impl core::fmt::Display for ApiShapeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::TooManyMembers => "a roster has at most 31 members",
            Self::BadSignatureCount => {
                "a signature set has one signature per set bitmap bit and at most 31"
            }
            Self::BlockPathTooLong => "a block path has at most 9 siblings",
            Self::HistoryPathTooLong => "a history path has at most 32 siblings",
            Self::PayloadTooLong => "a transfer payload has at most 4096 bytes",
            Self::TooManyRotations => "a rotation chain page has at most 16 steps",
        })
    }
}

impl std::error::Error for ApiShapeError {}

// ---------------------------------------------------------------------------------------------
// Attestation statement <-> contract-visible fields
// ---------------------------------------------------------------------------------------------

impl From<SccpAttestationStatementV1> for AttestationFieldsV1 {
    fn from(statement: SccpAttestationStatementV1) -> Self {
        Self {
            height: statement.height,
            epoch: statement.epoch,
            timestamp_ms: statement.timestamp_ms,
            block_hash: statement.block_hash,
            sccp_root: statement.sccp_root,
            message_count: statement.message_count,
            history_root: statement.history_root,
            history_size: statement.history_size,
            roster_digest: statement.roster_digest,
            next_roster_digest: statement.next_roster_digest,
        }
    }
}

impl From<AttestationFieldsV1> for SccpAttestationStatementV1 {
    fn from(fields: AttestationFieldsV1) -> Self {
        Self {
            height: fields.height,
            epoch: fields.epoch,
            timestamp_ms: fields.timestamp_ms,
            block_hash: fields.block_hash,
            sccp_root: fields.sccp_root,
            message_count: fields.message_count,
            history_root: fields.history_root,
            history_size: fields.history_size,
            roster_digest: fields.roster_digest,
            next_roster_digest: fields.next_roster_digest,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Roster view
// ---------------------------------------------------------------------------------------------

/// One roster generation as destinations receive it (§3.7), with its claimed digest.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpRosterViewV1")]
pub struct SccpRosterViewV1 {
    /// Generation number, at least 1.
    pub generation: u64,
    /// Start of validity (Taira time of the generation's rotation block).
    pub valid_from_ms: u64,
    /// End of validity.
    pub valid_until_ms: u64,
    /// `t = ⌊2n/3⌋ + 1`.
    pub threshold: u8,
    /// Bridge-key addresses in §3.7 order: zero (keyless) slots first, then strictly ascending.
    pub members: Vec<[u8; 20]>,
    /// §3.7 roster digest.
    pub digest: [u8; 32],
}

impl SccpRosterViewV1 {
    /// The view of `roster` under the given Taira `NetworkId`, with its threshold and digest.
    ///
    /// # Errors
    ///
    /// Returns the [`RosterError`] of a roster that breaks a §3.7 rule.
    pub fn from_roster(
        roster: &RosterV1,
        taira_network_id: &[u8; 32],
    ) -> Result<Self, RosterError> {
        let digest = roster.digest(taira_network_id)?;
        let threshold = u8::try_from(roster.threshold()).map_err(|_| RosterError::BadSize)?;
        Ok(Self {
            generation: roster.generation,
            valid_from_ms: roster.valid_from_ms,
            valid_until_ms: roster.valid_until_ms,
            threshold,
            members: roster.members.clone(),
            digest,
        })
    }

    /// The contract-visible roster, with the `n`, generation, ordering and threshold checks a
    /// destination performs while hashing (the claimed [`Self::digest`] is not consulted).
    ///
    /// # Errors
    ///
    /// Returns the first violated [`RosterError`].
    pub fn to_roster(&self) -> Result<RosterV1, RosterError> {
        let roster = RosterV1 {
            generation: self.generation,
            valid_from_ms: self.valid_from_ms,
            valid_until_ms: self.valid_until_ms,
            members: self.members.clone(),
        };
        roster.validate()?;
        if usize::from(self.threshold) != roster.threshold() {
            return Err(RosterError::BadThreshold);
        }
        Ok(roster)
    }

    /// Reject a member list longer than any roster (bounds work before hashing).
    ///
    /// # Errors
    ///
    /// Returns [`ApiShapeError::TooManyMembers`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        if self.members.len() > MAX_ROSTER_MEMBERS {
            return Err(ApiShapeError::TooManyMembers);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Signature set
// ---------------------------------------------------------------------------------------------

/// A §3.8 signature set: bit `i` names roster member `i`; one 65-byte `r ‖ s ‖ v` signature
/// per set bit, in ascending bit order.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpSignatureSetV1")]
pub struct SccpSignatureSetV1 {
    /// Bit `i` set iff roster member `i` signed.
    pub signer_bitmap: u32,
    /// One signature per set bit, in ascending bit order.
    pub signatures: Vec<[u8; 65]>,
}

impl SccpSignatureSetV1 {
    /// Number of set bitmap bits.
    #[must_use]
    pub fn popcount(&self) -> u32 {
        self.signer_bitmap.count_ones()
    }

    /// Reject more than 31 signatures or a count that differs from the bitmap popcount.
    ///
    /// # Errors
    ///
    /// Returns [`ApiShapeError::BadSignatureCount`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        if self.signatures.len() > MAX_ROSTER_MEMBERS
            || self.signatures.len() != self.popcount() as usize
        {
            return Err(ApiShapeError::BadSignatureCount);
        }
        Ok(())
    }

    /// The contract-visible set with the signatures concatenated (`SignaturesV1`).
    #[must_use]
    pub fn to_signature_set(&self) -> SignatureSetV1 {
        SignatureSetV1 {
            signer_bitmap: self.signer_bitmap,
            signatures: self.signatures.iter().flatten().copied().collect(),
        }
    }
}

impl TryFrom<&SignatureSetV1> for SccpSignatureSetV1 {
    type Error = ApiShapeError;

    fn try_from(set: &SignatureSetV1) -> Result<Self, Self::Error> {
        if !set.signatures.len().is_multiple_of(SIGNATURE_BYTES) {
            return Err(ApiShapeError::BadSignatureCount);
        }
        let signatures = set
            .signatures
            .chunks_exact(SIGNATURE_BYTES)
            .map(|chunk| {
                let mut signature = [0_u8; 65];
                signature.copy_from_slice(chunk);
                signature
            })
            .collect();
        let view = Self {
            signer_bitmap: set.signer_bitmap,
            signatures,
        };
        view.check_shape()?;
        Ok(view)
    }
}

// ---------------------------------------------------------------------------------------------
// History proof
// ---------------------------------------------------------------------------------------------

/// A historical SCCP-bearing block and the path of its history leaf (§3.5, `HistoryProofV1`
/// of §5.2.2).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpHistoryProofV1")]
pub struct SccpHistoryProofV1 {
    /// Height of the historical block.
    pub height: u64,
    /// Its commitment root.
    pub sccp_root: [u8; 32],
    /// Its leaf count (transfers and controls).
    pub message_count: u32,
    /// Index of its history leaf.
    pub leaf_index: u64,
    /// History path (at most 32 siblings).
    pub path: Vec<[u8; 32]>,
}

impl SccpHistoryProofV1 {
    /// Reject a path longer than 32 siblings.
    ///
    /// # Errors
    ///
    /// Returns [`ApiShapeError::HistoryPathTooLong`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        if self.path.len() > MAX_HISTORY_PATH {
            return Err(ApiShapeError::HistoryPathTooLong);
        }
        Ok(())
    }
}

impl From<&SccpHistoryProofV1> for HistoryProofV1 {
    fn from(proof: &SccpHistoryProofV1) -> Self {
        Self {
            block: HistoryBlockV1 {
                height: proof.height,
                sccp_root: proof.sccp_root,
                message_count: proof.message_count,
            },
            leaf_index: proof.leaf_index,
            path: proof.path.clone(),
        }
    }
}

impl From<&HistoryProofV1> for SccpHistoryProofV1 {
    fn from(proof: &HistoryProofV1) -> Self {
        Self {
            height: proof.block.height,
            sccp_root: proof.block.sccp_root,
            message_count: proof.block.message_count,
            leaf_index: proof.leaf_index,
            path: proof.path.clone(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Message proof bundle
// ---------------------------------------------------------------------------------------------

/// `GET /v1/sccp/messages/{message_id}/proof`: everything one `finalizeFromTaira*` or
/// `voidExpired*` call needs (§5.1.3, §5.1.8, §6).
///
/// Without [`Self::history`] the message is proven against the attested block itself (direct
/// mode, [`Self::message_count`] equals the statement's); with it, the message is proven
/// against the historical block and that block against the statement's history root.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpMessageProofBundleV1")]
pub struct SccpMessageProofBundleV1 {
    /// `message_id` (§3.3).
    pub message_id: [u8; 32],
    /// The §3.2 transfer payload.
    #[norito(json = "crate::api::base64_json")]
    pub payload: Vec<u8>,
    /// Destination-time mint deadline copied from the payload.
    pub deadline_ms: u64,
    /// Commitment index of the transfer leaf in its block.
    pub leaf_index: u32,
    /// Leaf count of the block that holds the message.
    pub message_count: u32,
    /// Block path of the transfer leaf (at most 9 siblings).
    pub path: Vec<[u8; 32]>,
    /// The attested statement.
    pub statement: SccpAttestationStatementV1,
    /// EIP-712 digest of the statement (§3.6).
    pub digest: [u8; 32],
    /// The roster generation that signed the statement.
    pub roster: SccpRosterViewV1,
    /// At least `t` signatures of that roster, ascending.
    pub signatures: SccpSignatureSetV1,
    /// History proof of an older block (historical mode), or `None` (direct mode).
    #[norito(required)]
    pub history: Option<SccpHistoryProofV1>,
}

impl SccpMessageProofBundleV1 {
    /// Whether the bundle proves the message through the history root.
    #[must_use]
    pub fn is_historical(&self) -> bool {
        self.history.is_some()
    }

    /// The contract-visible attestation of the statement.
    #[must_use]
    pub fn attestation(&self) -> AttestationFieldsV1 {
        self.statement.into()
    }

    /// `MessageProofV1 { payload, leafIndex, path }` (§5.2.2).
    #[must_use]
    pub fn message_proof(&self) -> MessageProofV1 {
        MessageProofV1 {
            payload: self.payload.clone(),
            leaf_index: self.leaf_index,
            path: self.path.clone(),
        }
    }

    /// `HistoryProofV1` of historical mode.
    #[must_use]
    pub fn history_proof(&self) -> Option<HistoryProofV1> {
        self.history.as_ref().map(HistoryProofV1::from)
    }

    /// Reject oversized fields before any hashing or signature recovery.
    ///
    /// # Errors
    ///
    /// Returns the first violated [`ApiShapeError`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        if self.payload.len() > MAX_PAYLOAD_BYTES {
            return Err(ApiShapeError::PayloadTooLong);
        }
        if self.path.len() > MAX_BLOCK_PATH {
            return Err(ApiShapeError::BlockPathTooLong);
        }
        self.roster.check_shape()?;
        self.signatures.check_shape()?;
        if let Some(history) = &self.history {
            history.check_shape()?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Control proof bundle
// ---------------------------------------------------------------------------------------------

/// `GET /v1/sccp/controls/{network}/{revision}/{control_nonce}/proof`: everything one
/// `applyControl*` call (or TON `sccp_apply_control`) needs (§4.14.6, §5.1.6, §6).
///
/// The bundle carries the control's fields, never its leaf: a verifier computes the control
/// leaf from the destination's own immutables and these fields.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpControlProofBundleV1")]
pub struct SccpControlProofBundleV1 {
    /// Target network of the control.
    pub network: SccpNetworkV1,
    /// Route revision whose deployment the control commands.
    pub revision: u32,
    /// Control nonce (at least 1, strictly increasing per `(network, revision)`).
    pub control_nonce: u64,
    /// `true` pauses minting, `false` resumes it.
    pub paused: bool,
    /// Commitment index of the control leaf in its block.
    pub leaf_index: u32,
    /// Leaf count of the block that holds the control.
    pub message_count: u32,
    /// Block path of the control leaf (at most 9 siblings).
    pub path: Vec<[u8; 32]>,
    /// The attested statement.
    pub statement: SccpAttestationStatementV1,
    /// EIP-712 digest of the statement (§3.6).
    pub digest: [u8; 32],
    /// The roster generation that signed the statement.
    pub roster: SccpRosterViewV1,
    /// At least `t` signatures of that roster, ascending.
    pub signatures: SccpSignatureSetV1,
    /// History proof of an older block (historical mode), or `None` (direct mode).
    #[norito(required)]
    pub history: Option<SccpHistoryProofV1>,
}

impl SccpControlProofBundleV1 {
    /// Whether the bundle proves the control through the history root.
    #[must_use]
    pub fn is_historical(&self) -> bool {
        self.history.is_some()
    }

    /// The contract-visible attestation of the statement.
    #[must_use]
    pub fn attestation(&self) -> AttestationFieldsV1 {
        self.statement.into()
    }

    /// `ControlProofV1 { controlNonce, paused, leafIndex, path }` (§5.2.2).
    #[must_use]
    pub fn control_proof(&self) -> ControlProofV1 {
        ControlProofV1 {
            control_nonce: self.control_nonce,
            paused: self.paused,
            leaf_index: self.leaf_index,
            path: self.path.clone(),
        }
    }

    /// `HistoryProofV1` of historical mode.
    #[must_use]
    pub fn history_proof(&self) -> Option<HistoryProofV1> {
        self.history.as_ref().map(HistoryProofV1::from)
    }

    /// Reject oversized fields before any hashing or signature recovery.
    ///
    /// # Errors
    ///
    /// Returns the first violated [`ApiShapeError`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        if self.path.len() > MAX_BLOCK_PATH {
            return Err(ApiShapeError::BlockPathTooLong);
        }
        self.roster.check_shape()?;
        self.signatures.check_shape()?;
        if let Some(history) = &self.history {
            history.check_shape()?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Rotation chain
// ---------------------------------------------------------------------------------------------

/// One attested handoff: the rotation statement signed by the outgoing generation, and both
/// generations (`RotationV1` of §5.2.2 plus the statement digest).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpRotationStepV1")]
pub struct SccpRotationStepV1 {
    /// The rotation statement (`next_roster_digest ≠ 0`).
    pub statement: SccpAttestationStatementV1,
    /// EIP-712 digest of the statement.
    pub digest: [u8; 32],
    /// At least `t` signatures of the outgoing generation.
    pub signatures: SccpSignatureSetV1,
    /// The outgoing generation.
    pub current_roster: SccpRosterViewV1,
    /// The successor generation.
    pub next_roster: SccpRosterViewV1,
}

impl SccpRotationStepV1 {
    /// The contract-visible attestation of the statement.
    #[must_use]
    pub fn attestation(&self) -> AttestationFieldsV1 {
        self.statement.into()
    }

    /// Reject oversized rosters or signature sets.
    ///
    /// # Errors
    ///
    /// Returns the first violated [`ApiShapeError`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        self.current_roster.check_shape()?;
        self.next_roster.check_shape()?;
        self.signatures.check_shape()
    }
}

/// `GET /v1/sccp/rosters/rotations?after_generation=G`: the ordered catch-up chain from
/// generation `G`, ready for one `rotateRosters` call (§5.1.5, §6).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpRotationChainV1")]
pub struct SccpRotationChainV1 {
    /// Consecutive handoffs `G → G+1 → …`, at most 16 per page.
    pub steps: Vec<SccpRotationStepV1>,
    /// Rotation height of the first handoff that is not attested yet, when the chain is broken
    /// there (§4.3.3).
    #[norito(required)]
    pub first_unattested_handoff: Option<u64>,
}

impl SccpRotationChainV1 {
    /// Reject pages with more than 16 steps or oversized steps.
    ///
    /// # Errors
    ///
    /// Returns the first violated [`ApiShapeError`].
    pub fn check_shape(&self) -> Result<(), ApiShapeError> {
        if self.steps.len() > MAX_ROTATIONS_PER_CALL {
            return Err(ApiShapeError::TooManyRotations);
        }
        self.steps
            .iter()
            .try_for_each(SccpRotationStepV1::check_shape)
    }
}

// ---------------------------------------------------------------------------------------------
// JSON helper: padded standard base64 for byte strings
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------------------------

/// One member of the current generation with its liveness (`GET /v1/sccp/capabilities`).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpMemberLivenessV1")]
pub struct SccpMemberLivenessV1 {
    /// Member index in the generation.
    pub index: u8,
    /// Bridge-key address; zero for a keyless slot.
    pub address: [u8; 20],
    /// Last height this address signed, if any.
    #[norito(required)]
    pub last_signed_height: Option<u64>,
}

/// One open SCCP governance proposal (`GET /v1/sccp/governance/proposals`, §4.14.5 item 4).
///
/// The Parliament driver creates attempts for admissible proposals without one, oldest first.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpGovernanceProposalStatusV1")]
pub struct SccpGovernanceProposalStatusV1 {
    /// Proposal content id (the key of every attempt).
    pub content_id: iroha_data_model::governance::types::ProposalContentId,
    /// Taira height at which the proposal was submitted.
    pub created_height: u64,
    /// The proposal.
    pub proposal: iroha_data_model::sccp::governance::SccpGovernanceProposalV1,
    /// A new attempt would pass the SCCP preflight: every base revision is current and every
    /// registered destination word is unused.
    pub admissible: bool,
    /// The newest Parliament attempt, if any.
    #[norito(required)]
    pub latest_attempt: Option<iroha_data_model::governance::types::GovernanceAttemptV1>,
}

/// A rotation subject whose handoff is not attested yet (§4.3.3).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpPendingHandoffV1")]
pub struct SccpPendingHandoffV1 {
    /// Rotation height.
    pub height: u64,
    /// Outgoing generation.
    pub generation: u64,
    /// Whether it stayed unattested for `attestation_stall_ms` of Taira time.
    pub stalled: bool,
}

/// `GET /v1/sccp/capabilities`: the Taira identity, parameters and attestation health a wallet
/// reads before any flow (§6, §7.1).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Decode,
    Encode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::api::SccpCapabilitiesV1")]
pub struct SccpCapabilitiesV1 {
    /// Live Taira `NetworkId` (the EIP-712 salt).
    pub network_id: [u8; 32],
    /// EIP-712 domain separator of attestations (§3.6).
    pub domain_separator: [u8; 32],
    /// SCCP parameters; `None` when SCCP does not exist on this network.
    #[norito(required)]
    pub parameters: Option<iroha_data_model::sccp::params::SccpParametersV1>,
    /// Latest committed Taira height.
    pub committed_height: u64,
    /// Highest attested subject height, if any.
    #[norito(required)]
    pub latest_attested_height: Option<u64>,
    /// Current roster generation (0 before the first).
    pub current_generation: u64,
    /// Members of the current generation.
    pub members: Vec<SccpMemberLivenessV1>,
    /// Unattested rotation subjects, oldest first.
    pub pending_handoffs: Vec<SccpPendingHandoffV1>,
}

/// Norito JSON field helper for `Vec<u8>` as padded standard base64 (RFC 4648 §4), the
/// data-model convention for opaque byte strings. Decoding accepts only the canonical
/// encoding: padded, standard alphabet, zero unused bits.
pub mod base64_json {
    use norito::json::{self, BoundedJsonError, JsonWriteSink, Parser};

    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// Write `bytes` as one base64 JSON string.
    pub fn serialize(bytes: &[u8], out: &mut String) {
        json::write_base64_json(bytes, out);
    }

    /// Write `bytes` as one base64 JSON string through a checked sink.
    ///
    /// # Errors
    ///
    /// Returns the sink's [`BoundedJsonError`].
    pub fn serialize_bounded(
        bytes: &[u8],
        out: &mut dyn JsonWriteSink,
    ) -> Result<(), BoundedJsonError> {
        json::write_base64_json_to(bytes, out)
    }

    /// Read one canonical base64 JSON string.
    ///
    /// # Errors
    ///
    /// Returns a JSON error for a non-string or a non-canonical encoding.
    pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<u8>, json::Error> {
        let text = parser.parse_string()?;
        decode(&text)
            .ok_or_else(|| json::Error::Message("expected canonical padded base64".to_owned()))
    }

    fn sextet(byte: u8) -> Option<u8> {
        ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .and_then(|index| u8::try_from(index).ok())
    }

    /// Strictly decode padded standard base64; `None` for any non-canonical input.
    #[must_use]
    pub fn decode(text: &str) -> Option<Vec<u8>> {
        let bytes = text.as_bytes();
        if !bytes.len().is_multiple_of(4) {
            return None;
        }
        let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
        let quads = bytes.len() / 4;
        for (index, quad) in bytes.chunks_exact(4).enumerate() {
            let last = index + 1 == quads;
            let padding = quad.iter().rev().take_while(|byte| **byte == b'=').count();
            if padding > 2 || (padding > 0 && !last) {
                return None;
            }
            let mut values = [0_u8; 4];
            for (slot, byte) in values.iter_mut().zip(&quad[..4 - padding]) {
                *slot = sextet(*byte)?;
            }
            let word = (u32::from(values[0]) << 18)
                | (u32::from(values[1]) << 12)
                | (u32::from(values[2]) << 6)
                | u32::from(values[3]);
            let [_, first, second, third] = word.to_be_bytes();
            match padding {
                0 => out.extend_from_slice(&[first, second, third]),
                1 => {
                    if third != 0 {
                        return None;
                    }
                    out.extend_from_slice(&[first, second]);
                }
                _ => {
                    if second != 0 || third != 0 {
                        return None;
                    }
                    out.push(first);
                }
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use norito::{NoritoSchema as _, codec::DecodeAll, core::DecodeFromSlice, json::Value};

    use super::*;
    use crate::v1::{
        hashes::keccak256,
        signature::{address_of_secret, sign_digest},
    };

    const TAIRA: [u8; 32] = [0x11; 32];

    fn secret(index: u8) -> [u8; 32] {
        keccak256(&[b"SCCP/API/TEST/KEY", &[index]])
    }

    fn roster(generation: u64) -> RosterV1 {
        let mut members: Vec<[u8; 20]> = (0..3)
            .map(|index| address_of_secret(&secret(index)).expect("valid secret"))
            .collect();
        members.push([0; 20]);
        members.sort_unstable();
        RosterV1 {
            generation,
            valid_from_ms: 1_800_000_000_000,
            valid_until_ms: 1_800_000_000_000 + 14 * 86_400_000,
            members,
        }
    }

    fn statement(roster: &RosterV1, next: [u8; 32]) -> SccpAttestationStatementV1 {
        SccpAttestationStatementV1 {
            height: 100,
            epoch: 3,
            timestamp_ms: 1_800_000_100_000,
            block_hash: [0xb1; 32],
            sccp_root: [0xc2; 32],
            message_count: 3,
            history_root: [0xd3; 32],
            history_size: 2,
            roster_digest: roster.digest(&TAIRA).expect("valid roster"),
            next_roster_digest: next,
        }
    }

    fn signatures(roster: &RosterV1, digest: &[u8; 32]) -> SccpSignatureSetV1 {
        let entries: Vec<(usize, [u8; 65])> = (0..3)
            .map(|key| {
                let address = address_of_secret(&secret(key)).expect("valid secret");
                let index = roster
                    .members
                    .iter()
                    .position(|member| *member == address)
                    .expect("member");
                (index, sign_digest(&secret(key), digest).expect("signature"))
            })
            .collect();
        let set = SignatureSetV1::from_signers(roster.n(), &entries).expect("set");
        SccpSignatureSetV1::try_from(&set).expect("view")
    }

    fn history() -> SccpHistoryProofV1 {
        SccpHistoryProofV1 {
            height: 50,
            sccp_root: [0xe4; 32],
            message_count: 1,
            leaf_index: 0,
            path: vec![[0xf5; 32]],
        }
    }

    fn message_bundle(history: Option<SccpHistoryProofV1>) -> SccpMessageProofBundleV1 {
        let roster = roster(7);
        let statement = statement(&roster, [0; 32]);
        let digest = AttestationFieldsV1::from(statement).digest(&TAIRA);
        SccpMessageProofBundleV1 {
            message_id: [0x0a; 32],
            payload: vec![0x02, 0x01, 0xaa, 0xbb],
            deadline_ms: 1_800_086_400_000,
            leaf_index: 1,
            message_count: 3,
            path: vec![[0x01; 32], [0x02; 32]],
            statement,
            digest,
            signatures: signatures(&roster, &digest),
            roster: SccpRosterViewV1::from_roster(&roster, &TAIRA).expect("view"),
            history,
        }
    }

    fn control_bundle(history: Option<SccpHistoryProofV1>) -> SccpControlProofBundleV1 {
        let bundle = message_bundle(history.clone());
        SccpControlProofBundleV1 {
            network: SccpNetworkV1::EthereumMainnet,
            revision: 1,
            control_nonce: 2,
            paused: true,
            leaf_index: 2,
            message_count: 3,
            path: vec![[0x03; 32]],
            statement: bundle.statement,
            digest: bundle.digest,
            roster: bundle.roster,
            signatures: bundle.signatures,
            history,
        }
    }

    fn rotation_chain(first_unattested_handoff: Option<u64>) -> SccpRotationChainV1 {
        let current = roster(7);
        let next = roster(8);
        let next_view = SccpRosterViewV1::from_roster(&next, &TAIRA).expect("view");
        let statement = statement(&current, next_view.digest);
        let digest = AttestationFieldsV1::from(statement).digest(&TAIRA);
        SccpRotationChainV1 {
            steps: vec![SccpRotationStepV1 {
                statement,
                digest,
                signatures: signatures(&current, &digest),
                current_roster: SccpRosterViewV1::from_roster(&current, &TAIRA).expect("view"),
                next_roster: next_view,
            }],
            first_unattested_handoff,
        }
    }

    /// Bare, headered, zero-copy slice and JSON roundtrips.
    fn roundtrip<T>(value: &T)
    where
        T: norito::codec::Encode
            + DecodeAll
            + norito::NoritoSerialize
            + for<'a> norito::NoritoDeserialize<'a>
            + for<'a> DecodeFromSlice<'a>
            + norito::json::JsonSerialize
            + norito::json::JsonDeserialize
            + PartialEq
            + core::fmt::Debug,
    {
        let encoded = value.encode();
        assert_eq!(
            &T::decode_all(&mut encoded.as_slice()).expect("bare decode"),
            value
        );
        let (decoded, used) = T::decode_from_slice(&encoded).expect("slice decode");
        assert_eq!(used, encoded.len());
        assert_eq!(&decoded, value);
        let framed = norito::to_bytes(value).expect("frame");
        assert_eq!(
            &norito::decode_from_bytes::<T>(&framed).expect("frame decode"),
            value
        );
        let json = norito::json::to_json(value).expect("JSON");
        assert_eq!(
            &norito::json::from_json::<T>(&json).expect("JSON decode"),
            value,
            "{json}"
        );
    }

    fn assert_rejects_unknown_field<T>(value: &T)
    where
        T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
    {
        let mut json = norito::json::to_value(value).expect("JSON value");
        json.as_object_mut()
            .expect("object")
            .insert("adversarial".to_owned(), Value::Null);
        let hostile = norito::json::to_json(&json).expect("JSON");
        assert!(norito::json::from_json::<T>(&hostile).is_err(), "{hostile}");
    }

    fn assert_requires_field<T>(value: &T, field: &str)
    where
        T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
    {
        let mut json = norito::json::to_value(value).expect("JSON value");
        json.as_object_mut()
            .expect("object")
            .remove(field)
            .expect("field present");
        let missing = norito::json::to_json(&json).expect("JSON");
        assert!(norito::json::from_json::<T>(&missing).is_err(), "{missing}");
    }

    #[test]
    fn api_shape_errors_display_their_bounds() {
        assert_eq!(
            ApiShapeError::TooManyMembers.to_string(),
            "a roster has at most 31 members"
        );
        assert_eq!(
            ApiShapeError::TooManyRotations.to_string(),
            "a rotation chain page has at most 16 steps"
        );
        let boxed: Box<dyn std::error::Error> = Box::new(ApiShapeError::PayloadTooLong);
        assert!(boxed.to_string().contains("4096"));
    }

    #[test]
    fn api_schema_names_are_stable() {
        assert_eq!(
            SccpRosterViewV1::nominal_name(),
            "iroha_sccp::api::SccpRosterViewV1"
        );
        assert_eq!(
            SccpSignatureSetV1::nominal_name(),
            "iroha_sccp::api::SccpSignatureSetV1"
        );
        assert_eq!(
            SccpHistoryProofV1::nominal_name(),
            "iroha_sccp::api::SccpHistoryProofV1"
        );
        assert_eq!(
            SccpMessageProofBundleV1::nominal_name(),
            "iroha_sccp::api::SccpMessageProofBundleV1"
        );
        assert_eq!(
            SccpControlProofBundleV1::nominal_name(),
            "iroha_sccp::api::SccpControlProofBundleV1"
        );
        assert_eq!(
            SccpRotationStepV1::nominal_name(),
            "iroha_sccp::api::SccpRotationStepV1"
        );
        assert_eq!(
            SccpRotationChainV1::nominal_name(),
            "iroha_sccp::api::SccpRotationChainV1"
        );
    }

    #[test]
    fn api_records_roundtrip_in_every_codec() {
        roundtrip(&message_bundle(None));
        roundtrip(&message_bundle(Some(history())));
        roundtrip(&control_bundle(None));
        roundtrip(&control_bundle(Some(history())));
        roundtrip(&rotation_chain(None));
        roundtrip(&rotation_chain(Some(151)));
        roundtrip(&SccpRotationChainV1::default());
        roundtrip(&history());
        roundtrip(&message_bundle(None).roster);
        roundtrip(&message_bundle(None).signatures);
    }

    #[test]
    fn api_json_objects_are_closed_and_complete() {
        assert_rejects_unknown_field(&message_bundle(None));
        assert_rejects_unknown_field(&control_bundle(Some(history())));
        assert_rejects_unknown_field(&rotation_chain(None));
        assert_rejects_unknown_field(&history());
        assert_rejects_unknown_field(&message_bundle(None).roster);
        assert_rejects_unknown_field(&message_bundle(None).signatures);
        assert_requires_field(&message_bundle(None), "history");
        assert_requires_field(&message_bundle(None), "payload");
        assert_requires_field(&control_bundle(None), "history");
        assert_requires_field(&control_bundle(None), "control_nonce");
        assert_requires_field(&rotation_chain(None), "first_unattested_handoff");
    }

    #[test]
    fn api_json_uses_base64_payload_and_hex_arrays() {
        let bundle = message_bundle(None);
        let json = norito::json::to_value(&bundle).expect("JSON value");
        assert_eq!(
            json.get("payload").and_then(Value::as_str),
            Some("AgGquw==")
        );
        let digest = json.get("digest").and_then(Value::as_str).expect("digest");
        assert_eq!(digest.len(), 64);
        assert!(json.get("history").expect("history key").is_null());
        let mut hostile = json.clone();
        hostile
            .as_object_mut()
            .expect("object")
            .insert("payload".to_owned(), Value::from("AgGquw"));
        let text = norito::json::to_json(&hostile).expect("JSON");
        assert!(norito::json::from_json::<SccpMessageProofBundleV1>(&text).is_err());
    }

    #[test]
    fn base64_decoder_is_strict_and_canonical() {
        assert_eq!(base64_json::decode(""), Some(Vec::new()));
        assert_eq!(base64_json::decode("AA=="), Some(vec![0]));
        assert_eq!(base64_json::decode("AAE="), Some(vec![0, 1]));
        assert_eq!(base64_json::decode("AAEC"), Some(vec![0, 1, 2]));
        assert_eq!(
            base64_json::decode("AAECA/8="),
            Some(vec![0, 1, 2, 3, 0xff])
        );
        assert_eq!(base64_json::decode("AAECAw"), None, "unpadded");
        assert_eq!(base64_json::decode("AB=="), None, "nonzero unused bits");
        assert_eq!(base64_json::decode("AAF="), None, "nonzero unused bits");
        assert_eq!(base64_json::decode("A==="), None, "three padding chars");
        assert_eq!(
            base64_json::decode("AA==AAAA"),
            None,
            "padding before the end"
        );
        assert_eq!(base64_json::decode("AA-_"), None, "URL-safe alphabet");
        assert_eq!(base64_json::decode("AA A"), None, "whitespace");
        let mut out = String::new();
        base64_json::serialize(&[0, 1, 2, 3, 0xff], &mut out);
        assert_eq!(out, "\"AAECA/8=\"");
        let mut parser = norito::json::Parser::new(&out);
        assert_eq!(
            base64_json::deserialize(&mut parser).expect("decodes"),
            vec![0, 1, 2, 3, 0xff]
        );
    }

    #[test]
    fn statement_and_attestation_fields_convert_both_ways() {
        let roster = roster(7);
        let statement = statement(&roster, [0x5d; 32]);
        let fields = AttestationFieldsV1::from(statement);
        assert_eq!(fields.height, 100);
        assert_eq!(fields.epoch, 3);
        assert_eq!(fields.timestamp_ms, 1_800_000_100_000);
        assert_eq!(fields.block_hash, [0xb1; 32]);
        assert_eq!(fields.sccp_root, [0xc2; 32]);
        assert_eq!(fields.message_count, 3);
        assert_eq!(fields.history_root, [0xd3; 32]);
        assert_eq!(fields.history_size, 2);
        assert_eq!(fields.roster_digest, statement.roster_digest);
        assert_eq!(fields.next_roster_digest, [0x5d; 32]);
        assert_eq!(SccpAttestationStatementV1::from(fields), statement);
    }

    #[test]
    fn roster_view_converts_and_checks_the_threshold() {
        let roster = roster(7);
        let view = SccpRosterViewV1::from_roster(&roster, &TAIRA).expect("view");
        assert_eq!(view.threshold, 3);
        assert_eq!(view.digest, roster.digest(&TAIRA).expect("digest"));
        assert_eq!(view.to_roster().expect("roster"), roster);
        let mut bad_threshold = view.clone();
        bad_threshold.threshold = 4;
        assert_eq!(bad_threshold.to_roster(), Err(RosterError::BadThreshold));
        let mut unordered = view.clone();
        unordered.members.reverse();
        assert_eq!(unordered.to_roster(), Err(RosterError::BadOrder));
        let mut too_small = view.clone();
        too_small.members.truncate(3);
        assert_eq!(too_small.to_roster(), Err(RosterError::BadSize));
        let mut too_many = view.clone();
        too_many.members = vec![[1; 20]; 32];
        assert_eq!(too_many.check_shape(), Err(ApiShapeError::TooManyMembers));
        assert_eq!(view.check_shape(), Ok(()));
        let empty = RosterV1 {
            members: Vec::new(),
            ..roster
        };
        assert_eq!(
            SccpRosterViewV1::from_roster(&empty, &TAIRA),
            Err(RosterError::BadSize)
        );
    }

    #[test]
    fn signature_set_view_converts_and_checks_counts() {
        let bundle = message_bundle(None);
        let set = bundle.signatures.to_signature_set();
        assert_eq!(set.signatures.len(), 3 * SIGNATURE_BYTES);
        assert_eq!(set.signer_bitmap, bundle.signatures.signer_bitmap);
        assert_eq!(
            SccpSignatureSetV1::try_from(&set).expect("view"),
            bundle.signatures
        );
        assert_eq!(bundle.signatures.popcount(), 3);
        assert_eq!(bundle.signatures.check_shape(), Ok(()));
        let ragged = SignatureSetV1 {
            signer_bitmap: 1,
            signatures: vec![0; 64],
        };
        assert_eq!(
            SccpSignatureSetV1::try_from(&ragged),
            Err(ApiShapeError::BadSignatureCount)
        );
        let mut mismatched = bundle.signatures.clone();
        mismatched.signatures.pop();
        assert_eq!(
            mismatched.check_shape(),
            Err(ApiShapeError::BadSignatureCount)
        );
    }

    #[test]
    fn history_proof_converts_both_ways() {
        let proof = history();
        let contract = HistoryProofV1::from(&proof);
        assert_eq!(contract.block.height, 50);
        assert_eq!(contract.block.sccp_root, [0xe4; 32]);
        assert_eq!(contract.block.message_count, 1);
        assert_eq!(contract.leaf_index, 0);
        assert_eq!(contract.path, vec![[0xf5; 32]]);
        assert_eq!(SccpHistoryProofV1::from(&contract), proof);
        let mut long = proof;
        long.path = vec![[0; 32]; MAX_HISTORY_PATH + 1];
        assert_eq!(long.check_shape(), Err(ApiShapeError::HistoryPathTooLong));
    }

    #[test]
    fn bundle_accessors_expose_contract_structures() {
        let direct = message_bundle(None);
        assert!(!direct.is_historical());
        assert_eq!(direct.history_proof(), None);
        assert_eq!(direct.attestation().digest(&TAIRA), direct.digest);
        let proof = direct.message_proof();
        assert_eq!(proof.payload, direct.payload);
        assert_eq!(proof.leaf_index, 1);
        assert_eq!(proof.path, direct.path);
        assert_eq!(direct.check_shape(), Ok(()));

        let historical = message_bundle(Some(history()));
        assert!(historical.is_historical());
        assert_eq!(
            historical.history_proof().expect("history").block.height,
            50
        );

        let mut oversized = direct.clone();
        oversized.payload = vec![0; MAX_PAYLOAD_BYTES + 1];
        assert_eq!(oversized.check_shape(), Err(ApiShapeError::PayloadTooLong));
        let mut long_path = direct;
        long_path.path = vec![[0; 32]; MAX_BLOCK_PATH + 1];
        assert_eq!(
            long_path.check_shape(),
            Err(ApiShapeError::BlockPathTooLong)
        );

        let control = control_bundle(Some(history()));
        assert!(control.is_historical());
        assert_eq!(control.attestation().digest(&TAIRA), control.digest);
        let control_proof = control.control_proof();
        assert_eq!(control_proof.control_nonce, 2);
        assert!(control_proof.paused);
        assert_eq!(control_proof.leaf_index, 2);
        assert_eq!(control.history_proof().expect("history").leaf_index, 0);
        assert_eq!(control.check_shape(), Ok(()));
        let mut long_control = control;
        long_control.path = vec![[0; 32]; MAX_BLOCK_PATH + 1];
        assert_eq!(
            long_control.check_shape(),
            Err(ApiShapeError::BlockPathTooLong)
        );
        assert!(!control_bundle(None).is_historical());
    }

    #[test]
    fn rotation_chain_shape_is_bounded() {
        let chain = rotation_chain(None);
        assert_eq!(chain.check_shape(), Ok(()));
        let step = &chain.steps[0];
        assert_eq!(step.attestation().digest(&TAIRA), step.digest);
        assert!(step.attestation().is_rotation());
        let long = SccpRotationChainV1 {
            steps: vec![step.clone(); MAX_ROTATIONS_PER_CALL + 1],
            first_unattested_handoff: None,
        };
        assert_eq!(long.check_shape(), Err(ApiShapeError::TooManyRotations));
        let mut bad_step = step.clone();
        bad_step.signatures.signer_bitmap = 0;
        assert_eq!(
            bad_step.check_shape(),
            Err(ApiShapeError::BadSignatureCount)
        );
    }
}

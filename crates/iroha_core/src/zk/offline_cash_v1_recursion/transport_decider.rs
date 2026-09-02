//! Fail-closed checkpoint contract for the compact Offline Cash V1 transport decider.
//!
//! The current recursive circuits are intentionally not admitted as transport
//! proofs: their authenticated Halo2 transcript profiles exceed the V1 wire
//! slots.  This module fixes the state machine and proof-size budget for the
//! replacement `k = 16`, one-column staged verifier.  Checkpoints are internal
//! prover state and grant no monetary authority by themselves.
//!
//! TODO: constrain every transition below in the paired one-column Halo2
//! transport-decider circuits before exposing a completed checkpoint as a
//! payment proof. Every non-genesis step must recursively verify the
//! opposite-parity predecessor proof against the release-pinned outer VK; the
//! verified predecessor's public checkpoint and pair commitments are the only
//! permitted chain inputs. The circuits must derive checkpoint and pair
//! commitments with the release-pinned field-native hash rather than accepting
//! host-provided digests.

// This pre-admission contract intentionally has no production call site until
// the TODO above is complete. Keeping it compiled prevents the circuit-facing
// state contract from silently drifting while it remains non-authoritative.
#![allow(dead_code)]

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, OFFLINE_CASH_RECURSION_IPA_K_V1,
    OfflineCashPastaParityV1,
};

const SCHEDULE_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:transport-decider-schedule\0";
const TRANSCRIPT_ITEM_BYTES: usize = 32;
const K16_IPA_OPENING_ITEMS: usize = OFFLINE_CASH_RECURSION_IPA_K_V1 as usize * 2 + 5;

/// Maximum non-opening transcript inventory which fits one V1 parity slot at `k = 16`.
pub(super) const OFFLINE_CASH_TRANSPORT_DECIDER_NON_OPENING_ITEMS_MAX_V1: usize =
    OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 / TRANSCRIPT_ITEM_BYTES - K16_IPA_OPENING_ITEMS;

/// A circuit family and parity pinned into a staged verifier checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashTransportInnerRoleV1 {
    /// Aggregate-state Eq/Fp proof.
    AggregateStateEq,
    /// Aggregate-state Ep/Fq proof.
    AggregateStateEp,
    /// Commit-wrapper Eq/Fp proof.
    CommitWrapperEq,
    /// Commit-wrapper Ep/Fq proof.
    CommitWrapperEp,
    /// Mint-authorization Eq/Fp proof.
    MintAuthorizationEq,
    /// Mint-authorization Ep/Fq proof.
    MintAuthorizationEp,
    /// Mint-credit Eq/Fp proof.
    MintCreditEq,
    /// Mint-credit Ep/Fq proof.
    MintCreditEp,
    /// Platform-credential Eq/Fp proof.
    PlatformCredentialEq,
    /// Platform-credential Ep/Fq proof.
    PlatformCredentialEp,
    /// GuardBundle Eq/Fp proof.
    GuardBundleEq,
    /// GuardBundle Ep/Fq proof.
    GuardBundleEp,
}

impl OfflineCashTransportInnerRoleV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::AggregateStateEq => 0,
            Self::AggregateStateEp => 1,
            Self::CommitWrapperEq => 2,
            Self::CommitWrapperEp => 3,
            Self::MintAuthorizationEq => 4,
            Self::MintAuthorizationEp => 5,
            Self::MintCreditEq => 6,
            Self::MintCreditEp => 7,
            Self::PlatformCredentialEq => 8,
            Self::PlatformCredentialEp => 9,
            Self::GuardBundleEq => 10,
            Self::GuardBundleEp => 11,
        }
    }

    const fn parity(self) -> OfflineCashPastaParityV1 {
        match self {
            Self::AggregateStateEq
            | Self::CommitWrapperEq
            | Self::MintAuthorizationEq
            | Self::MintCreditEq
            | Self::PlatformCredentialEq
            | Self::GuardBundleEq => OfflineCashPastaParityV1::Eq,
            Self::AggregateStateEp
            | Self::CommitWrapperEp
            | Self::MintAuthorizationEp
            | Self::MintCreditEp
            | Self::PlatformCredentialEp
            | Self::GuardBundleEp => OfflineCashPastaParityV1::Ep,
        }
    }

    const fn family(self) -> u8 {
        self.tag() / 2
    }
}

/// Deterministic phase of the resumable transport verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashTransportDeciderStageV1 {
    /// Parse the exact authenticated inner proof and update its transcript.
    Transcript,
    /// Evaluate the scalar verifier and prepare tagged curve equations.
    ScalarAlgebra,
    /// Enforce the opposite-parity deferred MSM equation in bounded slices.
    ReciprocalMsm,
    /// Bind the completed paired result to its public transport statement.
    Finalize,
}

impl OfflineCashTransportDeciderStageV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Transcript => 0,
            Self::ScalarAlgebra => 1,
            Self::ReciprocalMsm => 2,
            Self::Finalize => 3,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::Transcript => Some(Self::ScalarAlgebra),
            Self::ScalarAlgebra => Some(Self::ReciprocalMsm),
            Self::ReciprocalMsm => Some(Self::Finalize),
            Self::Finalize => None,
        }
    }
}

/// Exact ordinary-proof transcript profile of one transport-decider parity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashTransportDeciderProfileV1 {
    /// Witness commitments across all phases.
    pub(super) witness_commitments: usize,
    /// Quotient commitments.
    pub(super) quotient_commitments: usize,
    /// Scalar evaluations.
    pub(super) evaluations: usize,
    /// Distinct BGH19 rotation sets.
    pub(super) bgh19_rotation_sets: usize,
}

/// Feasibility target for the compiled one-column outer transcript profile.
///
/// This is not an achieved proof profile. Artifact generation must derive the
/// real profile from the compiled decider VK and reject any mismatch or slot
/// overflow before the circuit can be admitted by a release.
pub(super) const OFFLINE_CASH_TRANSPORT_DECIDER_PROFILE_V1: OfflineCashTransportDeciderProfileV1 =
    OfflineCashTransportDeciderProfileV1 {
        witness_commitments: 6,
        quotient_commitments: 4,
        evaluations: 17,
        bgh19_rotation_sets: 4,
    };

/// Raw proof bytes predicted by the staged-decider feasibility target.
pub(super) const OFFLINE_CASH_TRANSPORT_DECIDER_TARGET_PROOF_BYTES_V1: usize = 2_176;

impl OfflineCashTransportDeciderProfileV1 {
    /// Return the exact canonical proof length after validating the V1 slot budget.
    pub(super) fn proof_bytes(self) -> Result<usize, OfflineCashTransportDeciderErrorV1> {
        let non_opening = self
            .witness_commitments
            .checked_add(self.quotient_commitments)
            .and_then(|count| count.checked_add(self.evaluations))
            .and_then(|count| count.checked_add(self.bgh19_rotation_sets))
            .ok_or(OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)?;
        if non_opening > OFFLINE_CASH_TRANSPORT_DECIDER_NON_OPENING_ITEMS_MAX_V1 {
            return Err(OfflineCashTransportDeciderErrorV1::ProofProfileTooWide {
                non_opening,
                maximum: OFFLINE_CASH_TRANSPORT_DECIDER_NON_OPENING_ITEMS_MAX_V1,
            });
        }
        let bytes = non_opening
            .checked_add(K16_IPA_OPENING_ITEMS)
            .and_then(|count| count.checked_mul(TRANSCRIPT_ITEM_BYTES))
            .ok_or(OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)?;
        if bytes > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 {
            return Err(OfflineCashTransportDeciderErrorV1::ProofSlotExceeded {
                actual: bytes,
                maximum: OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
            });
        }
        Ok(bytes)
    }
}

/// Release-pinned fixed checkpoint schedule for one transport-decider proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashTransportDeciderPlanV1 {
    /// Exact authenticated release digest which owns this schedule.
    pub(super) release_digest: [u8; 32],
    /// Compiled outer Eq protocol digest.
    pub(super) outer_eq_protocol_digest: [u8; 32],
    /// Compiled outer Ep protocol digest.
    pub(super) outer_ep_protocol_digest: [u8; 32],
    /// Compiled outer Eq verifying-key digest.
    pub(super) outer_eq_verifying_key_digest: [u8; 32],
    /// Compiled outer Ep verifying-key digest.
    pub(super) outer_ep_verifying_key_digest: [u8; 32],
    /// Exact k16 delayed-history layout digest.
    pub(super) history_layout_digest: [u8; 32],
    /// Exact Eq role compiled into the decider release.
    pub(super) eq_inner_role: OfflineCashTransportInnerRoleV1,
    /// Exact Ep role compiled into the decider release.
    pub(super) ep_inner_role: OfflineCashTransportInnerRoleV1,
    /// Exact authenticated Eq compiled-protocol digest.
    pub(super) eq_inner_protocol_digest: [u8; 32],
    /// Exact authenticated Ep compiled-protocol digest.
    pub(super) ep_inner_protocol_digest: [u8; 32],
    /// Exact authenticated Eq verifying-key digest.
    pub(super) eq_inner_verifying_key_digest: [u8; 32],
    /// Exact authenticated Ep verifying-key digest.
    pub(super) ep_inner_verifying_key_digest: [u8; 32],
    /// Canonical successful Eq equation-decision digest.
    pub(super) eq_accept_decision_digest: [u8; 32],
    /// Canonical successful Ep equation-decision digest.
    pub(super) ep_accept_decision_digest: [u8; 32],
    /// Exact authenticated Eq inner-proof bytes consumed by the parser.
    pub(super) eq_inner_proof_bytes: u64,
    /// Exact authenticated Ep inner-proof bytes consumed by the parser.
    pub(super) ep_inner_proof_bytes: u64,
    /// Bounded transcript/parser slices.
    pub(super) transcript_slices: u32,
    /// Bounded scalar-verifier slices.
    pub(super) scalar_slices: u32,
    /// Bounded reciprocal-MSM slices.
    pub(super) reciprocal_msm_slices: u32,
    /// Authenticated finalization slices; V1 requires exactly one.
    pub(super) finalize_slices: u32,
    /// Compiled outer proof profile.
    pub(super) proof_profile: OfflineCashTransportDeciderProfileV1,
}

impl OfflineCashTransportDeciderPlanV1 {
    /// Validate the schedule and derive its domain-separated commitment.
    pub(super) fn digest(self) -> Result<[u8; 32], OfflineCashTransportDeciderErrorV1> {
        if self.eq_inner_role.parity() != OfflineCashPastaParityV1::Eq
            || self.ep_inner_role.parity() != OfflineCashPastaParityV1::Ep
            || self.eq_inner_role.family() != self.ep_inner_role.family()
            || [
                self.release_digest,
                self.outer_eq_protocol_digest,
                self.outer_ep_protocol_digest,
                self.outer_eq_verifying_key_digest,
                self.outer_ep_verifying_key_digest,
                self.history_layout_digest,
                self.eq_inner_protocol_digest,
                self.ep_inner_protocol_digest,
                self.eq_inner_verifying_key_digest,
                self.ep_inner_verifying_key_digest,
                self.eq_accept_decision_digest,
                self.ep_accept_decision_digest,
            ]
            .contains(&[0; 32])
            || self.outer_eq_protocol_digest == self.outer_ep_protocol_digest
            || self.outer_eq_verifying_key_digest == self.outer_ep_verifying_key_digest
            || self.eq_inner_protocol_digest == self.ep_inner_protocol_digest
            || self.eq_inner_verifying_key_digest == self.ep_inner_verifying_key_digest
            || self.eq_accept_decision_digest == self.ep_accept_decision_digest
            || self.eq_inner_proof_bytes == 0
            || self.ep_inner_proof_bytes == 0
            || self.eq_inner_proof_bytes % TRANSCRIPT_ITEM_BYTES as u64 != 0
            || self.ep_inner_proof_bytes % TRANSCRIPT_ITEM_BYTES as u64 != 0
            || self.transcript_slices == 0
            || u64::from(self.transcript_slices)
                > self.eq_inner_proof_bytes / TRANSCRIPT_ITEM_BYTES as u64
            || u64::from(self.transcript_slices)
                > self.ep_inner_proof_bytes / TRANSCRIPT_ITEM_BYTES as u64
            || self.scalar_slices == 0
            || self.reciprocal_msm_slices == 0
            || self.finalize_slices != 1
            || self.proof_profile != OFFLINE_CASH_TRANSPORT_DECIDER_PROFILE_V1
        {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidSchedule);
        }
        self.proof_profile.proof_bytes()?;
        let mut hash = Sha256::new();
        hash.update(SCHEDULE_DOMAIN_V1);
        hash.update(OFFLINE_CASH_RECURSION_IPA_K_V1.to_le_bytes());
        hash.update(self.release_digest);
        hash.update(self.outer_eq_protocol_digest);
        hash.update(self.outer_ep_protocol_digest);
        hash.update(self.outer_eq_verifying_key_digest);
        hash.update(self.outer_ep_verifying_key_digest);
        hash.update(self.history_layout_digest);
        hash.update([self.eq_inner_role.tag()]);
        hash.update([self.ep_inner_role.tag()]);
        hash.update(self.eq_inner_protocol_digest);
        hash.update(self.ep_inner_protocol_digest);
        hash.update(self.eq_inner_verifying_key_digest);
        hash.update(self.ep_inner_verifying_key_digest);
        hash.update(self.eq_accept_decision_digest);
        hash.update(self.ep_accept_decision_digest);
        hash.update(self.eq_inner_proof_bytes.to_le_bytes());
        hash.update(self.ep_inner_proof_bytes.to_le_bytes());
        hash.update(self.transcript_slices.to_le_bytes());
        hash.update(self.scalar_slices.to_le_bytes());
        hash.update(self.reciprocal_msm_slices.to_le_bytes());
        hash.update(self.finalize_slices.to_le_bytes());
        for count in [
            self.proof_profile.witness_commitments,
            self.proof_profile.quotient_commitments,
            self.proof_profile.evaluations,
            self.proof_profile.bgh19_rotation_sets,
        ] {
            hash.update(
                u64::try_from(count)
                    .map_err(|_| OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)?
                    .to_le_bytes(),
            );
        }
        Ok(hash.finalize().into())
    }

    const fn slices(self, stage: OfflineCashTransportDeciderStageV1) -> u32 {
        match stage {
            OfflineCashTransportDeciderStageV1::Transcript => self.transcript_slices,
            OfflineCashTransportDeciderStageV1::ScalarAlgebra => self.scalar_slices,
            OfflineCashTransportDeciderStageV1::ReciprocalMsm => self.reciprocal_msm_slices,
            OfflineCashTransportDeciderStageV1::Finalize => self.finalize_slices,
        }
    }

    const fn inner_proof_bytes(self, parity: OfflineCashPastaParityV1) -> u64 {
        match parity {
            OfflineCashPastaParityV1::Eq => self.eq_inner_proof_bytes,
            OfflineCashPastaParityV1::Ep => self.ep_inner_proof_bytes,
        }
    }

    const fn inner_role(self, parity: OfflineCashPastaParityV1) -> OfflineCashTransportInnerRoleV1 {
        match parity {
            OfflineCashPastaParityV1::Eq => self.eq_inner_role,
            OfflineCashPastaParityV1::Ep => self.ep_inner_role,
        }
    }

    const fn inner_protocol_digest(self, parity: OfflineCashPastaParityV1) -> [u8; 32] {
        match parity {
            OfflineCashPastaParityV1::Eq => self.eq_inner_protocol_digest,
            OfflineCashPastaParityV1::Ep => self.ep_inner_protocol_digest,
        }
    }

    const fn inner_verifying_key_digest(self, parity: OfflineCashPastaParityV1) -> [u8; 32] {
        match parity {
            OfflineCashPastaParityV1::Eq => self.eq_inner_verifying_key_digest,
            OfflineCashPastaParityV1::Ep => self.ep_inner_verifying_key_digest,
        }
    }

    fn transcript_cursor(
        self,
        parity: OfflineCashPastaParityV1,
        slice_index: u32,
    ) -> Result<u64, OfflineCashTransportDeciderErrorV1> {
        if slice_index > self.transcript_slices {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidSchedule);
        }
        let proof_items = self.inner_proof_bytes(parity) / TRANSCRIPT_ITEM_BYTES as u64;
        let consumed_items = u128::from(proof_items)
            .checked_mul(u128::from(slice_index))
            .ok_or(OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)?
            / u128::from(self.transcript_slices);
        u64::try_from(
            consumed_items
                .checked_mul(TRANSCRIPT_ITEM_BYTES as u128)
                .ok_or(OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)?,
        )
        .map_err(|_| OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)
    }
}

/// Internal authenticated state carried between fixed-size verifier steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashTransportDeciderCheckpointV1 {
    /// Exact release/profile digest.
    pub(super) release_digest: [u8; 32],
    /// Exact fixed checkpoint schedule digest.
    pub(super) schedule_digest: [u8; 32],
    /// Circuit family and parity of the inner proof.
    pub(super) inner_role: OfflineCashTransportInnerRoleV1,
    /// Exact authenticated inner compiled-protocol digest.
    pub(super) inner_protocol_digest: [u8; 32],
    /// Exact authenticated inner verifying-key digest.
    pub(super) inner_verifying_key_digest: [u8; 32],
    /// Public semantic/state output shared by both parities.
    pub(super) semantic_digest: [u8; 32],
    /// Exact Eq inner-proof digest, exposed identically by both outer parities.
    pub(super) eq_inner_proof_digest: [u8; 32],
    /// Exact Ep inner-proof digest, exposed identically by both outer parities.
    pub(super) ep_inner_proof_digest: [u8; 32],
    /// Authenticated Eq proof-chunk root used by the staged parser.
    pub(super) eq_inner_chunk_root: [u8; 32],
    /// Authenticated Ep proof-chunk root used by the staged parser.
    pub(super) ep_inner_chunk_root: [u8; 32],
    /// Eq delayed-history output exposed identically by both outer parities.
    pub(super) eq_history_digest: [u8; 32],
    /// Ep delayed-history output exposed identically by both outer parities.
    pub(super) ep_history_digest: [u8; 32],
    /// Eq deferred-equation audit shared by both parities.
    pub(super) eq_audit_digest: [u8; 32],
    /// Ep deferred-equation audit shared by both parities.
    pub(super) ep_audit_digest: [u8; 32],
    /// Exact parser byte cursor.
    pub(super) parser_cursor: u64,
    /// Commitment to the resumable transcript sponge state.
    pub(super) transcript_state_digest: [u8; 32],
    /// Commitment to the resumable scalar-verifier state.
    pub(super) scalar_state_digest: [u8; 32],
    /// Commitment to the resumable deferred-MSM state.
    pub(super) msm_state_digest: [u8; 32],
    /// Eq equation-decision output; zero until the final constrained slice.
    pub(super) eq_decision_digest: [u8; 32],
    /// Ep equation-decision output; zero until the final constrained slice.
    pub(super) ep_decision_digest: [u8; 32],
    /// Current fixed verifier phase.
    pub(super) stage: OfflineCashTransportDeciderStageV1,
    /// Zero-based slice to process next, or `slice_count` only when complete.
    pub(super) slice_index: u32,
    /// Release-pinned number of slices in the current phase.
    pub(super) slice_count: u32,
    /// True only after the single finalization slice has been constrained.
    pub(super) complete: bool,
    /// Monotonic verifier step, beginning at zero for canonical genesis.
    pub(super) step_index: u64,
    /// Circuit-produced commitment to the predecessor checkpoint; zero only at genesis.
    pub(super) predecessor_checkpoint_commitment: [u8; 32],
    /// Circuit-produced field-native commitment to this checkpoint.
    pub(super) checkpoint_commitment: [u8; 32],
    /// Circuit-produced commitment to the opposite-parity checkpoint at this step.
    pub(super) counterpart_checkpoint_commitment: [u8; 32],
    /// Common pair binding exposed by the predecessor proof pair; zero only at genesis.
    pub(super) predecessor_pair_binding_commitment: [u8; 32],
    /// Circuit-produced common commitment which prevents Eq/Ep pair splicing.
    pub(super) pair_binding_commitment: [u8; 32],
}

impl OfflineCashTransportDeciderCheckpointV1 {
    /// Validate canonical structure and release-pinned bindings.
    ///
    /// This does not authenticate circuit-derived commitments and therefore
    /// must never be used as monetary admission by itself.
    pub(super) fn validate(
        &self,
        plan: OfflineCashTransportDeciderPlanV1,
    ) -> Result<(), OfflineCashTransportDeciderErrorV1> {
        if self.schedule_digest != plan.digest()? {
            return Err(OfflineCashTransportDeciderErrorV1::ScheduleSubstitution);
        }
        let parity = self.parity();
        if self.inner_role != plan.inner_role(parity) {
            return Err(OfflineCashTransportDeciderErrorV1::RoleParityMismatch);
        }
        if self.inner_protocol_digest != plan.inner_protocol_digest(parity)
            || self.inner_verifying_key_digest != plan.inner_verifying_key_digest(parity)
        {
            return Err(OfflineCashTransportDeciderErrorV1::InnerArtifactSubstitution);
        }
        if self.release_digest != plan.release_digest {
            return Err(OfflineCashTransportDeciderErrorV1::ReleaseSubstitution);
        }
        if [
            self.release_digest,
            self.inner_protocol_digest,
            self.inner_verifying_key_digest,
            self.semantic_digest,
            self.eq_inner_proof_digest,
            self.ep_inner_proof_digest,
            self.eq_inner_chunk_root,
            self.ep_inner_chunk_root,
            self.eq_history_digest,
            self.ep_history_digest,
            self.eq_audit_digest,
            self.ep_audit_digest,
            self.transcript_state_digest,
            self.scalar_state_digest,
            self.msm_state_digest,
            self.checkpoint_commitment,
            self.counterpart_checkpoint_commitment,
            self.pair_binding_commitment,
        ]
        .contains(&[0; 32])
        {
            return Err(OfflineCashTransportDeciderErrorV1::ZeroBinding);
        }
        if self.eq_inner_proof_digest == self.ep_inner_proof_digest
            || self.eq_inner_chunk_root == self.ep_inner_chunk_root
            || self.eq_history_digest == self.ep_history_digest
            || self.eq_audit_digest == self.ep_audit_digest
        {
            return Err(OfflineCashTransportDeciderErrorV1::ParityBindingAlias);
        }
        let expected_count = plan.slices(self.stage);
        if self.slice_count != expected_count || self.slice_count == 0 {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidSchedule);
        }
        let canonical_completion = self.stage == OfflineCashTransportDeciderStageV1::Finalize
            && self.slice_index == self.slice_count;
        if self.complete != canonical_completion
            || (!self.complete && self.slice_index >= self.slice_count)
        {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidCompletionState);
        }
        let expected_proof_bytes = plan.inner_proof_bytes(self.parity());
        let canonical_cursor = match self.stage {
            OfflineCashTransportDeciderStageV1::Transcript => {
                self.parser_cursor == plan.transcript_cursor(self.parity(), self.slice_index)?
            }
            OfflineCashTransportDeciderStageV1::ScalarAlgebra
            | OfflineCashTransportDeciderStageV1::ReciprocalMsm
            | OfflineCashTransportDeciderStageV1::Finalize => {
                self.parser_cursor == expected_proof_bytes
            }
        };
        if !canonical_cursor {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidParserCursor);
        }
        let canonical_chain_link = if self.step_index == 0 {
            self.predecessor_checkpoint_commitment == [0; 32]
                && self.predecessor_pair_binding_commitment == [0; 32]
        } else {
            self.predecessor_checkpoint_commitment != [0; 32]
                && self.predecessor_pair_binding_commitment != [0; 32]
        };
        if !canonical_chain_link
            || self.checkpoint_commitment == self.counterpart_checkpoint_commitment
            || self.pair_binding_commitment == self.checkpoint_commitment
            || self.pair_binding_commitment == self.counterpart_checkpoint_commitment
        {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidCheckpointChain);
        }
        let decisions_are_zero =
            self.eq_decision_digest == [0; 32] && self.ep_decision_digest == [0; 32];
        let decisions_are_complete =
            self.eq_decision_digest != [0; 32] && self.ep_decision_digest != [0; 32];
        if (!self.complete && !decisions_are_zero)
            || (self.complete
                && (!decisions_are_complete
                    || self.eq_decision_digest != plan.eq_accept_decision_digest
                    || self.ep_decision_digest != plan.ep_accept_decision_digest))
        {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidDecisionState);
        }
        Ok(())
    }

    /// Require the sole canonical starting position for a proof-specific checkpoint chain.
    ///
    /// This is structural admission only. The future Halo2 circuit must derive
    /// the initial transcript state from its release-pinned protocol, VK,
    /// public instances, proof digest, and authenticated chunk root.
    pub(super) fn validate_genesis(
        &self,
        plan: OfflineCashTransportDeciderPlanV1,
    ) -> Result<(), OfflineCashTransportDeciderErrorV1> {
        self.validate(plan)?;
        if self.step_index != 0
            || self.predecessor_checkpoint_commitment != [0; 32]
            || self.predecessor_pair_binding_commitment != [0; 32]
            || self.stage != OfflineCashTransportDeciderStageV1::Transcript
            || self.slice_index != 0
            || self.parser_cursor != 0
            || self.complete
        {
            return Err(OfflineCashTransportDeciderErrorV1::InvalidGenesis);
        }
        Ok(())
    }

    /// Return the role-derived parity; parity is never witness-selected independently.
    pub(super) const fn parity(&self) -> OfflineCashPastaParityV1 {
        self.inner_role.parity()
    }
}

/// Structurally reject skips, reordering, substitution, and premature completion.
///
/// Monetary admission additionally requires the recursively verified decider
/// proof; this host check never substitutes for it.
pub(super) fn validate_transport_decider_successor_v1(
    plan: OfflineCashTransportDeciderPlanV1,
    predecessor: &OfflineCashTransportDeciderCheckpointV1,
    successor: &OfflineCashTransportDeciderCheckpointV1,
) -> Result<(), OfflineCashTransportDeciderErrorV1> {
    predecessor.validate(plan)?;
    successor.validate(plan)?;
    if predecessor.complete {
        return Err(OfflineCashTransportDeciderErrorV1::AlreadyComplete);
    }
    if predecessor.release_digest != successor.release_digest
        || predecessor.schedule_digest != successor.schedule_digest
        || predecessor.inner_role != successor.inner_role
        || predecessor.inner_protocol_digest != successor.inner_protocol_digest
        || predecessor.inner_verifying_key_digest != successor.inner_verifying_key_digest
        || predecessor.semantic_digest != successor.semantic_digest
        || predecessor.eq_inner_proof_digest != successor.eq_inner_proof_digest
        || predecessor.ep_inner_proof_digest != successor.ep_inner_proof_digest
        || predecessor.eq_inner_chunk_root != successor.eq_inner_chunk_root
        || predecessor.ep_inner_chunk_root != successor.ep_inner_chunk_root
        || predecessor.eq_history_digest != successor.eq_history_digest
        || predecessor.ep_history_digest != successor.ep_history_digest
        || predecessor.eq_audit_digest != successor.eq_audit_digest
        || predecessor.ep_audit_digest != successor.ep_audit_digest
    {
        return Err(OfflineCashTransportDeciderErrorV1::BindingSubstitution);
    }
    if successor.step_index
        != predecessor
            .step_index
            .checked_add(1)
            .ok_or(OfflineCashTransportDeciderErrorV1::ArithmeticOverflow)?
        || successor.predecessor_checkpoint_commitment != predecessor.checkpoint_commitment
        || successor.predecessor_pair_binding_commitment != predecessor.pair_binding_commitment
        || successor.checkpoint_commitment == predecessor.checkpoint_commitment
        || successor.counterpart_checkpoint_commitment
            == predecessor.counterpart_checkpoint_commitment
        || successor.pair_binding_commitment == predecessor.pair_binding_commitment
    {
        return Err(OfflineCashTransportDeciderErrorV1::InvalidCheckpointChain);
    }

    let last_slice = predecessor.slice_index + 1 == predecessor.slice_count;
    let (expected_stage, expected_index, expected_count, expected_complete) = if last_slice {
        match predecessor.stage.next() {
            Some(stage) => (stage, 0, plan.slices(stage), false),
            None => (
                OfflineCashTransportDeciderStageV1::Finalize,
                predecessor.slice_count,
                predecessor.slice_count,
                true,
            ),
        }
    } else {
        (
            predecessor.stage,
            predecessor.slice_index + 1,
            predecessor.slice_count,
            false,
        )
    };
    if successor.stage != expected_stage
        || successor.slice_index != expected_index
        || successor.slice_count != expected_count
        || successor.complete != expected_complete
    {
        return Err(OfflineCashTransportDeciderErrorV1::StageSkipOrReorder);
    }

    let parser_changed = predecessor.parser_cursor != successor.parser_cursor;
    let transcript_changed =
        predecessor.transcript_state_digest != successor.transcript_state_digest;
    let scalar_changed = predecessor.scalar_state_digest != successor.scalar_state_digest;
    let msm_changed = predecessor.msm_state_digest != successor.msm_state_digest;
    let decisions_changed = predecessor.eq_decision_digest != successor.eq_decision_digest
        && predecessor.ep_decision_digest != successor.ep_decision_digest;
    let valid_progress = match predecessor.stage {
        OfflineCashTransportDeciderStageV1::Transcript => {
            successor.parser_cursor > predecessor.parser_cursor
                && (if last_slice {
                    successor.parser_cursor == plan.inner_proof_bytes(predecessor.parity())
                } else {
                    successor.parser_cursor < plan.inner_proof_bytes(predecessor.parity())
                })
                && transcript_changed
                && !scalar_changed
                && !msm_changed
                && !decisions_changed
        }
        OfflineCashTransportDeciderStageV1::ScalarAlgebra => {
            !parser_changed
                && !transcript_changed
                && scalar_changed
                && !msm_changed
                && !decisions_changed
        }
        OfflineCashTransportDeciderStageV1::ReciprocalMsm => {
            !parser_changed
                && !transcript_changed
                && !scalar_changed
                && msm_changed
                && !decisions_changed
        }
        OfflineCashTransportDeciderStageV1::Finalize => {
            !parser_changed
                && !transcript_changed
                && !scalar_changed
                && !msm_changed
                && decisions_changed
        }
    };
    if !valid_progress {
        return Err(OfflineCashTransportDeciderErrorV1::InvalidStageProgress);
    }
    Ok(())
}

/// Structurally require an Eq/Ep pair to expose the same common outputs.
///
/// Monetary admission additionally requires both release-pinned recursive
/// proofs; equality of host-provided commitments grants no authority.
pub(super) fn validate_transport_decider_pair_v1(
    plan: OfflineCashTransportDeciderPlanV1,
    eq: &OfflineCashTransportDeciderCheckpointV1,
    ep: &OfflineCashTransportDeciderCheckpointV1,
) -> Result<(), OfflineCashTransportDeciderErrorV1> {
    eq.validate(plan)?;
    ep.validate(plan)?;
    if eq.parity() != OfflineCashPastaParityV1::Eq
        || ep.parity() != OfflineCashPastaParityV1::Ep
        || eq.inner_role.family() != ep.inner_role.family()
        || eq.release_digest != ep.release_digest
        || eq.schedule_digest != ep.schedule_digest
        || eq.semantic_digest != ep.semantic_digest
        || eq.eq_inner_proof_digest != ep.eq_inner_proof_digest
        || eq.ep_inner_proof_digest != ep.ep_inner_proof_digest
        || eq.eq_inner_chunk_root != ep.eq_inner_chunk_root
        || eq.ep_inner_chunk_root != ep.ep_inner_chunk_root
        || eq.eq_history_digest != ep.eq_history_digest
        || eq.ep_history_digest != ep.ep_history_digest
        || eq.eq_audit_digest != ep.eq_audit_digest
        || eq.ep_audit_digest != ep.ep_audit_digest
        || eq.stage != ep.stage
        || eq.slice_index != ep.slice_index
        || eq.slice_count != ep.slice_count
        || eq.complete != ep.complete
        || eq.step_index != ep.step_index
        || eq.eq_decision_digest != ep.eq_decision_digest
        || eq.ep_decision_digest != ep.ep_decision_digest
        || eq.counterpart_checkpoint_commitment != ep.checkpoint_commitment
        || ep.counterpart_checkpoint_commitment != eq.checkpoint_commitment
        || eq.predecessor_pair_binding_commitment != ep.predecessor_pair_binding_commitment
        || eq.pair_binding_commitment != ep.pair_binding_commitment
    {
        return Err(OfflineCashTransportDeciderErrorV1::PairBindingMismatch);
    }
    Ok(())
}

/// Structurally bind one complete paired step to the exact preceding paired step.
///
/// This check is deliberately non-authoritative: the future Eq/Ep circuits must
/// recursively verify the opposite-parity predecessor proofs and constrain all
/// commitments checked here. Host validation alone never admits value.
pub(super) fn validate_transport_decider_pair_successor_v1(
    plan: OfflineCashTransportDeciderPlanV1,
    predecessor_eq: &OfflineCashTransportDeciderCheckpointV1,
    predecessor_ep: &OfflineCashTransportDeciderCheckpointV1,
    successor_eq: &OfflineCashTransportDeciderCheckpointV1,
    successor_ep: &OfflineCashTransportDeciderCheckpointV1,
) -> Result<(), OfflineCashTransportDeciderErrorV1> {
    validate_transport_decider_pair_v1(plan, predecessor_eq, predecessor_ep)?;
    validate_transport_decider_pair_v1(plan, successor_eq, successor_ep)?;
    if successor_eq.predecessor_checkpoint_commitment != predecessor_eq.checkpoint_commitment
        || successor_ep.predecessor_checkpoint_commitment != predecessor_ep.checkpoint_commitment
        || successor_eq.predecessor_pair_binding_commitment
            != predecessor_eq.pair_binding_commitment
        || successor_ep.predecessor_pair_binding_commitment
            != predecessor_ep.pair_binding_commitment
    {
        return Err(OfflineCashTransportDeciderErrorV1::PairChainSplice);
    }
    validate_transport_decider_successor_v1(plan, predecessor_eq, successor_eq)?;
    validate_transport_decider_successor_v1(plan, predecessor_ep, successor_ep)?;
    Ok(())
}

/// Fail-closed transport-decider checkpoint validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum OfflineCashTransportDeciderErrorV1 {
    /// A checked transcript-size computation overflowed.
    #[error("transport-decider proof-profile arithmetic overflow")]
    ArithmeticOverflow,
    /// The compiled proof contains too many non-opening transcript items.
    #[error("transport-decider profile has {non_opening} non-opening items; maximum is {maximum}")]
    ProofProfileTooWide {
        /// Actual item count.
        non_opening: usize,
        /// Maximum slot-compatible item count.
        maximum: usize,
    },
    /// The exact proof length exceeds the unchanged V1 parity slot.
    #[error("transport-decider proof is {actual} bytes; maximum is {maximum}")]
    ProofSlotExceeded {
        /// Actual proof bytes.
        actual: usize,
        /// Maximum proof bytes.
        maximum: usize,
    },
    /// A phase is empty or finalization is not exactly one slice.
    #[error("invalid transport-decider checkpoint schedule")]
    InvalidSchedule,
    /// A checkpoint substituted a different authenticated schedule.
    #[error("transport-decider schedule substitution")]
    ScheduleSubstitution,
    /// A checkpoint substituted a release outside the fixed schedule.
    #[error("transport-decider release substitution")]
    ReleaseSubstitution,
    /// The inner role does not select the checkpoint parity.
    #[error("transport-decider role/parity mismatch")]
    RoleParityMismatch,
    /// A checkpoint substituted a protocol or VK outside its release-pinned role.
    #[error("transport-decider inner protocol or verifying-key substitution")]
    InnerArtifactSubstitution,
    /// A mandatory binding or state commitment is zero.
    #[error("transport-decider checkpoint contains a zero binding")]
    ZeroBinding,
    /// Distinct Eq and Ep proof roles were collapsed onto the same digest.
    #[error("transport-decider Eq/Ep binding alias")]
    ParityBindingAlias,
    /// Completion, phase, and cursor fields are not canonical.
    #[error("invalid transport-decider completion state")]
    InvalidCompletionState,
    /// The parser cursor does not exactly match the authenticated proof length at phase exit.
    #[error("invalid transport-decider parser cursor")]
    InvalidParserCursor,
    /// A checkpoint does not have the unique structural genesis position.
    #[error("invalid transport-decider genesis checkpoint")]
    InvalidGenesis,
    /// A checkpoint does not link to the exact preceding constrained state.
    #[error("invalid transport-decider checkpoint chain")]
    InvalidCheckpointChain,
    /// Equation decisions appeared before finalization or were absent afterwards.
    #[error("invalid transport-decider equation-decision state")]
    InvalidDecisionState,
    /// Common Eq/Ep outputs do not have their exact pair binding.
    #[error("transport-decider Eq/Ep pair binding mismatch")]
    PairBindingMismatch,
    /// A paired successor does not descend from the exact preceding proof pair.
    #[error("transport-decider paired checkpoint-chain splice")]
    PairChainSplice,
    /// An immutable release, protocol, VK, semantic, history, or audit binding changed.
    #[error("transport-decider immutable binding substitution")]
    BindingSubstitution,
    /// A successor skipped or reordered a fixed verifier slice.
    #[error("transport-decider stage skip or reorder")]
    StageSkipOrReorder,
    /// A stage modified the wrong resumable state component.
    #[error("transport-decider stage made invalid progress")]
    InvalidStageProgress,
    /// No successor is permitted after finalization.
    #[error("transport-decider checkpoint is already complete")]
    AlreadyComplete,
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn digest(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn plan() -> OfflineCashTransportDeciderPlanV1 {
        OfflineCashTransportDeciderPlanV1 {
            release_digest: digest(1),
            outer_eq_protocol_digest: digest(2),
            outer_ep_protocol_digest: digest(3),
            outer_eq_verifying_key_digest: digest(4),
            outer_ep_verifying_key_digest: digest(5),
            history_layout_digest: digest(6),
            eq_inner_role: OfflineCashTransportInnerRoleV1::AggregateStateEq,
            ep_inner_role: OfflineCashTransportInnerRoleV1::AggregateStateEp,
            eq_inner_protocol_digest: digest(20),
            ep_inner_protocol_digest: digest(21),
            eq_inner_verifying_key_digest: digest(22),
            ep_inner_verifying_key_digest: digest(23),
            eq_accept_decision_digest: digest(24),
            ep_accept_decision_digest: digest(25),
            eq_inner_proof_bytes: 20_128,
            ep_inner_proof_bytes: 20_128,
            transcript_slices: 2,
            scalar_slices: 2,
            reciprocal_msm_slices: 2,
            finalize_slices: 1,
            proof_profile: OFFLINE_CASH_TRANSPORT_DECIDER_PROFILE_V1,
        }
    }

    fn checkpoint(
        role: OfflineCashTransportInnerRoleV1,
    ) -> OfflineCashTransportDeciderCheckpointV1 {
        let plan = plan();
        OfflineCashTransportDeciderCheckpointV1 {
            release_digest: plan.release_digest,
            schedule_digest: plan.digest().unwrap(),
            inner_role: role,
            inner_protocol_digest: plan.inner_protocol_digest(role.parity()),
            inner_verifying_key_digest: plan.inner_verifying_key_digest(role.parity()),
            semantic_digest: digest(40),
            eq_inner_proof_digest: digest(41),
            ep_inner_proof_digest: digest(42),
            eq_inner_chunk_root: digest(43),
            ep_inner_chunk_root: digest(44),
            eq_history_digest: digest(45),
            ep_history_digest: digest(46),
            eq_audit_digest: digest(47),
            ep_audit_digest: digest(48),
            parser_cursor: 0,
            transcript_state_digest: digest(49),
            scalar_state_digest: digest(50),
            msm_state_digest: digest(51),
            eq_decision_digest: [0; 32],
            ep_decision_digest: [0; 32],
            stage: OfflineCashTransportDeciderStageV1::Transcript,
            slice_index: 0,
            slice_count: plan.transcript_slices,
            complete: false,
            step_index: 0,
            predecessor_checkpoint_commitment: [0; 32],
            checkpoint_commitment: match role.parity() {
                OfflineCashPastaParityV1::Eq => digest(52),
                OfflineCashPastaParityV1::Ep => digest(53),
            },
            counterpart_checkpoint_commitment: match role.parity() {
                OfflineCashPastaParityV1::Eq => digest(53),
                OfflineCashPastaParityV1::Ep => digest(52),
            },
            predecessor_pair_binding_commitment: [0; 32],
            pair_binding_commitment: digest(54),
        }
    }

    fn link_successor(
        predecessor: &OfflineCashTransportDeciderCheckpointV1,
        successor: &mut OfflineCashTransportDeciderCheckpointV1,
        tag: u8,
    ) {
        successor.step_index = predecessor.step_index + 1;
        successor.predecessor_checkpoint_commitment = predecessor.checkpoint_commitment;
        successor.predecessor_pair_binding_commitment = predecessor.pair_binding_commitment;
        successor.checkpoint_commitment = digest(tag);
        successor.counterpart_checkpoint_commitment = digest(tag.wrapping_add(1));
        successor.pair_binding_commitment = digest(tag.wrapping_add(128));
    }

    #[test]
    fn theoretical_one_column_profile_fits_unchanged_slot() {
        let plan = plan();
        let profile = plan.proof_profile;
        assert_eq!(
            profile.proof_bytes().unwrap(),
            OFFLINE_CASH_TRANSPORT_DECIDER_TARGET_PROOF_BYTES_V1
        );
        assert!(profile.proof_bytes().unwrap() <= OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1);
        assert_eq!(OFFLINE_CASH_TRANSPORT_DECIDER_NON_OPENING_ITEMS_MAX_V1, 40);
        assert!(
            OfflineCashTransportDeciderProfileV1 {
                witness_commitments: 7,
                quotient_commitments: 8,
                evaluations: 22,
                bgh19_rotation_sets: 4,
            }
            .proof_bytes()
            .is_err()
        );

        let first_cursor = plan
            .transcript_cursor(OfflineCashPastaParityV1::Eq, 1)
            .unwrap();
        assert_eq!(first_cursor % TRANSCRIPT_ITEM_BYTES as u64, 0);
        assert_eq!(
            plan.transcript_cursor(OfflineCashPastaParityV1::Eq, plan.transcript_slices)
                .unwrap(),
            plan.eq_inner_proof_bytes
        );

        let mut too_many_slices = plan;
        too_many_slices.transcript_slices = 630;
        assert_eq!(
            too_many_slices.digest(),
            Err(OfflineCashTransportDeciderErrorV1::InvalidSchedule)
        );

        let mut unpinned_profile = plan;
        unpinned_profile.proof_profile.witness_commitments = 5;
        assert_eq!(
            unpinned_profile.digest(),
            Err(OfflineCashTransportDeciderErrorV1::InvalidSchedule)
        );
    }

    #[test]
    fn schedule_rejects_skip_substitution_and_premature_completion() {
        let current = checkpoint(OfflineCashTransportInnerRoleV1::AggregateStateEq);
        current.validate_genesis(plan()).unwrap();
        let mut valid = current;
        valid.parser_cursor = plan()
            .transcript_cursor(OfflineCashPastaParityV1::Eq, 1)
            .unwrap();
        valid.transcript_state_digest = digest(60);
        valid.slice_index = 1;
        link_successor(&current, &mut valid, 61);
        assert!(validate_transport_decider_successor_v1(plan(), &current, &valid).is_ok());

        let mut skipped = valid;
        skipped.stage = OfflineCashTransportDeciderStageV1::ReciprocalMsm;
        skipped.parser_cursor = plan().eq_inner_proof_bytes;
        skipped.slice_index = 0;
        skipped.slice_count = plan().reciprocal_msm_slices;
        link_successor(&current, &mut skipped, 62);
        assert_eq!(
            validate_transport_decider_successor_v1(plan(), &current, &skipped),
            Err(OfflineCashTransportDeciderErrorV1::StageSkipOrReorder)
        );

        let mut substituted = valid;
        substituted.inner_protocol_digest = digest(99);
        assert_eq!(
            validate_transport_decider_successor_v1(plan(), &current, &substituted),
            Err(OfflineCashTransportDeciderErrorV1::InnerArtifactSubstitution)
        );

        let mut premature = current;
        premature.complete = true;
        assert_eq!(
            premature.validate(plan()),
            Err(OfflineCashTransportDeciderErrorV1::InvalidCompletionState)
        );

        let mut unlinked = valid;
        unlinked.predecessor_checkpoint_commitment = digest(99);
        assert_eq!(
            validate_transport_decider_successor_v1(plan(), &current, &unlinked),
            Err(OfflineCashTransportDeciderErrorV1::InvalidCheckpointChain)
        );
    }

    #[test]
    fn pair_validation_rejects_cross_family_and_spliced_outputs() {
        let eq = checkpoint(OfflineCashTransportInnerRoleV1::AggregateStateEq);
        let ep = checkpoint(OfflineCashTransportInnerRoleV1::AggregateStateEp);
        assert!(validate_transport_decider_pair_v1(plan(), &eq, &ep).is_ok());

        let wrong_family = checkpoint(OfflineCashTransportInnerRoleV1::CommitWrapperEp);
        assert_eq!(
            validate_transport_decider_pair_v1(plan(), &eq, &wrong_family),
            Err(OfflineCashTransportDeciderErrorV1::RoleParityMismatch)
        );

        let mut spliced = ep;
        spliced.ep_history_digest = digest(88);
        assert_eq!(
            validate_transport_decider_pair_v1(plan(), &eq, &spliced),
            Err(OfflineCashTransportDeciderErrorV1::PairBindingMismatch)
        );

        let mut aliased = ep;
        aliased.ep_inner_proof_digest = aliased.eq_inner_proof_digest;
        assert_eq!(
            aliased.validate(plan()),
            Err(OfflineCashTransportDeciderErrorV1::ParityBindingAlias)
        );

        let mut substituted_release = eq;
        substituted_release.release_digest = digest(89);
        assert_eq!(
            substituted_release.validate(plan()),
            Err(OfflineCashTransportDeciderErrorV1::ReleaseSubstitution)
        );
    }

    #[test]
    fn paired_successor_binds_both_predecessor_proofs_and_pair_history() {
        let plan = plan();
        let predecessor_eq = checkpoint(OfflineCashTransportInnerRoleV1::AggregateStateEq);
        let predecessor_ep = checkpoint(OfflineCashTransportInnerRoleV1::AggregateStateEp);
        let mut successor_eq = predecessor_eq;
        successor_eq.parser_cursor = plan
            .transcript_cursor(OfflineCashPastaParityV1::Eq, 1)
            .unwrap();
        successor_eq.transcript_state_digest = digest(71);
        successor_eq.slice_index = 1;
        link_successor(&predecessor_eq, &mut successor_eq, 72);
        let mut successor_ep = predecessor_ep;
        successor_ep.parser_cursor = plan
            .transcript_cursor(OfflineCashPastaParityV1::Ep, 1)
            .unwrap();
        successor_ep.transcript_state_digest = digest(73);
        successor_ep.slice_index = 1;
        link_successor(&predecessor_ep, &mut successor_ep, 73);
        successor_ep.counterpart_checkpoint_commitment = successor_eq.checkpoint_commitment;
        successor_ep.pair_binding_commitment = successor_eq.pair_binding_commitment;

        validate_transport_decider_pair_successor_v1(
            plan,
            &predecessor_eq,
            &predecessor_ep,
            &successor_eq,
            &successor_ep,
        )
        .unwrap();

        let mut spliced_eq = successor_eq;
        let mut spliced_ep = successor_ep;
        spliced_eq.predecessor_pair_binding_commitment = digest(99);
        spliced_ep.predecessor_pair_binding_commitment = digest(99);
        assert_eq!(
            validate_transport_decider_pair_successor_v1(
                plan,
                &predecessor_eq,
                &predecessor_ep,
                &spliced_eq,
                &spliced_ep,
            ),
            Err(OfflineCashTransportDeciderErrorV1::PairChainSplice)
        );
    }

    #[test]
    fn exact_schedule_reaches_one_terminal_state_and_cannot_advance_again() {
        let plan = plan();
        let mut current = checkpoint(OfflineCashTransportInnerRoleV1::AggregateStateEq);
        let initial_commitment = current.checkpoint_commitment;
        current.validate_genesis(plan).unwrap();

        let mut next = current;
        next.parser_cursor = plan
            .transcript_cursor(OfflineCashPastaParityV1::Eq, 1)
            .unwrap();
        next.transcript_state_digest = digest(60);
        next.slice_index = 1;
        link_successor(&current, &mut next, 61);
        validate_transport_decider_successor_v1(plan, &current, &next).unwrap();
        current = next;

        next = current;
        next.parser_cursor = plan.eq_inner_proof_bytes;
        next.transcript_state_digest = digest(62);
        next.stage = OfflineCashTransportDeciderStageV1::ScalarAlgebra;
        next.slice_index = 0;
        next.slice_count = plan.scalar_slices;
        link_successor(&current, &mut next, 63);
        validate_transport_decider_successor_v1(plan, &current, &next).unwrap();
        current = next;

        for (tag, scalar_digest) in [(64, digest(64)), (65, digest(65))] {
            next = current;
            next.scalar_state_digest = scalar_digest;
            if current.slice_index + 1 == current.slice_count {
                next.stage = OfflineCashTransportDeciderStageV1::ReciprocalMsm;
                next.slice_index = 0;
                next.slice_count = plan.reciprocal_msm_slices;
            } else {
                next.slice_index += 1;
            }
            link_successor(&current, &mut next, tag);
            validate_transport_decider_successor_v1(plan, &current, &next).unwrap();
            current = next;
        }

        for (tag, msm_digest) in [(66, digest(66)), (67, digest(67))] {
            next = current;
            next.msm_state_digest = msm_digest;
            if current.slice_index + 1 == current.slice_count {
                next.stage = OfflineCashTransportDeciderStageV1::Finalize;
                next.slice_index = 0;
                next.slice_count = plan.finalize_slices;
            } else {
                next.slice_index += 1;
            }
            link_successor(&current, &mut next, tag);
            validate_transport_decider_successor_v1(plan, &current, &next).unwrap();
            current = next;
        }

        next = current;
        next.slice_index = next.slice_count;
        next.complete = true;
        next.eq_decision_digest = plan.eq_accept_decision_digest;
        next.ep_decision_digest = plan.ep_accept_decision_digest;
        link_successor(&current, &mut next, 70);
        validate_transport_decider_successor_v1(plan, &current, &next).unwrap();
        assert!(next.complete);
        assert_ne!(initial_commitment, next.checkpoint_commitment);
        assert_eq!(
            validate_transport_decider_successor_v1(plan, &next, &next),
            Err(OfflineCashTransportDeciderErrorV1::AlreadyComplete)
        );
    }
}

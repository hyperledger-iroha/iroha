//! Atomic verifier boundary for the replacement RNS-native composite proof.
//!
//! The boundary consumes one canonical envelope, one move-only authenticated
//! source-snapshot owner, and the move-only result of the canonical transcript.
//! The source layout and structural receipt are derived only from that retained
//! owner. It exposes no downstream verifier trait and no partial stage receipt:
//! all four sections must pass private first-party adapters before a
//! non-authorizing candidate receipt can be minted.
//!
//! None of the existing complete proof kernels can currently satisfy that
//! contract for the 40-limb replacement profile.  The replacement qPCS adapter
//! now authenticates every qPCS tree, checks all queried opening/batch equations,
//! all eighteen FRI folds, and the terminal-degree equation. The RLWE/source
//! linkage is still unavailable. The terminal adapter now checks the complete
//! 1,536-row cross-basis representation-equality kernel. A separate private
//! source-to-terminal mapping prerequisite exists, but this boundary lacks the
//! exact public-artifact inventory needed to construct its preceding RLWE/source
//! preflight and that preflight proves no RLWE equality. The 40-limb
//! cross-field prerequisites are not joined under one authenticated
//! source/terminal owner, the global-lookup module does not verify a proof, and
//! the authenticated zero-padding commitment inventory is not yet linked to
//! the source, lookup, or terminal materialization. Production therefore fails
//! closed with an explicit unavailable stage. Success here, once those adapters are replaced,
//! will still be proof verification only and can never grant readiness or
//! release authority.

#![allow(
    clippy::large_types_passed_by_value,
    reason = "atomic verification deliberately consumes fixed-capacity proof and transcript owners"
)]

use super::{
    rns_native_profile::{
        zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_qpcs_fri_complete::authenticate_rns_native_qpcs_fri_complete_v1,
    rns_native_section_codec::{
        CompositeSectionSetErrorV1, ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1,
        ZkAmsMkheRnsNativeTerminalBridgeSectionV1, ZkAmsMkheRnsNativeZeroPaddingSectionV1,
        validate_composite_section_set_exact_v1,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReceiptV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_terminal_cross_basis::authenticate_rns_native_terminal_cross_basis_kernel_v1,
    rns_native_transcript::{
        ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1, ZkAmsMkheRnsNativeChallengeSeedsV1,
    },
    rns_native_wire::{
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1,
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1,
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1, ZkAmsMkheRnsNativeProofEnvelopeV1,
        ZkAmsMkheRnsNativeProofSectionDescriptorV1, ZkAmsMkheRnsNativeProofSectionKindV1,
    },
    rns_native_zero_padding_commitment::authenticate_rns_native_zero_padding_commitments_v1,
};
use crate::vega::sponge::Keccak256;

const VERIFICATION_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-composite-verification-context";
const CANDIDATE_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-composite-candidate-receipt";
const SECTION_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-proof-section";
const WHOLE_PROOF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-composite-proof-envelope";
const RNS_NATIVE_PROOF_ENVELOPE_TAG_V1: [u8; 4] = *b"ZANP";
const MAX_BOUND_DIGESTS_V1: usize = 18 + ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1;

/// Atomic composite-verifier schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_COMPOSITE_VERIFICATION_VERSION_V1: u8 = 1;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 == 4);
    assert!(MAX_BOUND_DIGESTS_V1 == 46);
};

/// Canonical stage order at the atomic verification boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheRnsNativeVerificationStageV1 {
    /// Terminal Hyrax proof plus the source mapping and cross-basis bridge.
    TerminalHyraxBpBridge = 1,
    /// Two RNS equations and the complete qPCS proof.
    RnsRelationQpcs = 2,
    /// Cross-field relations and the committed global lookup.
    CrossFieldGlobalLookup = 3,
    /// Proof that every governed padding lane is zero.
    ZeroPadding = 4,
}

impl ZkAmsMkheRnsNativeVerificationStageV1 {
    const fn index(self) -> usize {
        self as usize - 1
    }

    const fn section_kind(self) -> ZkAmsMkheRnsNativeProofSectionKindV1 {
        match self {
            Self::TerminalHyraxBpBridge => {
                ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge
            }
            Self::RnsRelationQpcs => ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs,
            Self::CrossFieldGlobalLookup => {
                ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup
            }
            Self::ZeroPadding => ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding,
        }
    }
}

const VERIFICATION_STAGE_ORDER_V1: [ZkAmsMkheRnsNativeVerificationStageV1;
    ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1] = [
    ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
    ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
    ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup,
    ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
];

/// Failure at the atomic replacement-proof verification boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheRnsNativeCompositeVerificationErrorV1 {
    /// The retained source owner did not yield a valid layout/receipt pair.
    InvalidSourceContext,
    /// Envelope identities did not match the exact source/profile context.
    InvalidEnvelopeContext,
    /// The move-only transcript result was zero, duplicated, or otherwise invalid.
    InvalidTranscript,
    /// A section descriptor, byte length, digest, or whole-envelope digest was invalid.
    InvalidSection(ZkAmsMkheRnsNativeVerificationStageV1),
    /// No sound first-party verifier exists for this replacement-profile stage.
    StageUnavailable(ZkAmsMkheRnsNativeVerificationStageV1),
    /// A first-party verifier rejected this stage.
    StageRejected(ZkAmsMkheRnsNativeVerificationStageV1),
}

impl core::fmt::Display for ZkAmsMkheRnsNativeCompositeVerificationErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSourceContext => {
                formatter.write_str("invalid RNS-native composite source context")
            }
            Self::InvalidEnvelopeContext => {
                formatter.write_str("invalid RNS-native composite envelope context")
            }
            Self::InvalidTranscript => {
                formatter.write_str("invalid RNS-native composite transcript")
            }
            Self::InvalidSection(stage) => {
                write!(
                    formatter,
                    "invalid RNS-native composite section at {stage:?}"
                )
            }
            Self::StageUnavailable(stage) => {
                write!(
                    formatter,
                    "RNS-native composite verifier unavailable at {stage:?}"
                )
            }
            Self::StageRejected(stage) => {
                write!(
                    formatter,
                    "RNS-native composite verification rejected at {stage:?}"
                )
            }
        }
    }
}

impl std::error::Error for ZkAmsMkheRnsNativeCompositeVerificationErrorV1 {}

/// Move-only proof-verification candidate emitted only after all four stages.
///
/// This receipt records atomic proof verification and nothing more.  It is not
/// a readiness certificate, release capability, or authorization token, and
/// intentionally implements neither `Clone` nor `Copy`.
#[derive(Debug, PartialEq, Eq)]
#[allow(
    missing_copy_implementations,
    reason = "the non-authorizing candidate receipt must remain a move-only terminal result"
)]
pub struct ZkAmsMkheRnsNativeCompositeCandidateReceiptV1 {
    version: u8,
    profile_manifest_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    governed_roster_digest: [u8; 32],
    public_ciphertext_digest: [u8; 32],
    transcript_digest: [u8; 32],
    proof_digest: [u8; 32],
    section_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    candidate_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeCompositeCandidateReceiptV1 {
    /// Canonical non-authorizing profile-manifest identity.
    #[must_use]
    pub const fn profile_manifest_digest(&self) -> [u8; 32] {
        self.profile_manifest_digest
    }

    /// Canonical 40-limb proof-topology identity.
    #[must_use]
    pub const fn topology_digest(&self) -> [u8; 32] {
        self.topology_digest
    }

    /// Non-authorizing release-candidate identity.
    #[must_use]
    pub const fn release_candidate_digest(&self) -> [u8; 32] {
        self.release_candidate_digest
    }

    /// Exact statement identity.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }

    /// Exact operational/replay context identity.
    #[must_use]
    pub const fn operational_context_digest(&self) -> [u8; 32] {
        self.operational_context_digest
    }

    /// Exact confidential-source binding.
    #[must_use]
    pub const fn source_binding_digest(&self) -> [u8; 32] {
        self.source_binding_digest
    }

    /// Validated structural source-receipt identity.
    #[must_use]
    pub const fn source_receipt_digest(&self) -> [u8; 32] {
        self.source_receipt_digest
    }

    /// Exact governed verifier-roster identity.
    #[must_use]
    pub const fn governed_roster_digest(&self) -> [u8; 32] {
        self.governed_roster_digest
    }

    /// Exact public ciphertext/statement-material identity.
    #[must_use]
    pub const fn public_ciphertext_digest(&self) -> [u8; 32] {
        self.public_ciphertext_digest
    }

    /// Fully ratcheted canonical transcript identity.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Complete canonical envelope identity.
    #[must_use]
    pub const fn proof_digest(&self) -> [u8; 32] {
        self.proof_digest
    }

    /// Four section digests in canonical verification order.
    #[must_use]
    pub const fn section_digests(
        &self,
    ) -> &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1] {
        &self.section_digests
    }

    /// Digest of this complete non-authorizing candidate receipt.
    #[must_use]
    pub const fn candidate_digest(&self) -> [u8; 32] {
        self.candidate_digest
    }
}

/// Atomically verify one replacement composite proof.
///
/// The envelope, source snapshot, and transcript are consumed even on failure.
/// No partial stage result escapes this boundary. At present a valid cross-basis
/// kernel still returns
/// [`ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable`]
/// because the private source-mapping prerequisite cannot be constructed from
/// the public facts retained by this boundary and no RLWE-equality verifier is
/// integrated.
///
/// # Errors
///
/// Rejects mismatched source/envelope/transcript contexts, malformed section
/// structure, any unavailable proof stage, or any first-party stage failure.
pub fn verify_zk_ams_mkhe_rns_native_composite_v1<S>(
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    source_snapshot: S,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<
    ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
    ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    verify_with_first_party_authority_v1(
        envelope,
        source_snapshot,
        transcript,
        FirstPartyStageAuthorityV1::Production,
    )
}

fn verify_with_first_party_authority_v1<S>(
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    source_snapshot: S,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    authority: FirstPartyStageAuthorityV1,
) -> Result<
    ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
    ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let result = (|| {
        let source_layout = source_snapshot.layout();
        source_layout
            .validate()
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;
        let source_receipt = source_snapshot
            .structural_receipt()
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;
        source_receipt
            .validate(source_layout)
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;

        ContextCheckedV1::new(
            envelope,
            source_layout,
            source_receipt,
            transcript,
            authority,
        )?
        .verify_terminal_bridge_v1()?
        .verify_rns_relation_qpcs_v1()?
        .verify_cross_field_global_lookup_v1()?
        .verify_zero_padding_v1()?
        .finish_v1()
    })();
    // Retain the actual snapshot owner until the complete atomic result has
    // been materialized; no detached layout/receipt pair can outlive it here.
    drop(source_snapshot);
    result
}

struct CandidateAxesV1 {
    profile_manifest_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    governed_roster_digest: [u8; 32],
    public_ciphertext_digest: [u8; 32],
    transcript_digest: [u8; 32],
    proof_digest: [u8; 32],
    section_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    context_digest: [u8; 32],
}

struct ContextCheckedV1 {
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    axes: CandidateAxesV1,
    authority: FirstPartyStageAuthorityV1,
}

impl ContextCheckedV1 {
    fn new(
        envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
        source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
        transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
        authority: FirstPartyStageAuthorityV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        let axes = validate_context_v1(&envelope, source_layout, source_receipt, &transcript)?;
        Ok(Self {
            envelope,
            transcript,
            axes,
            authority,
        })
    }

    fn verify_stage_v1(
        &self,
        stage: ZkAmsMkheRnsNativeVerificationStageV1,
    ) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        let kind = stage.section_kind();
        let descriptor = self.envelope.descriptors()[stage.index()];
        let section = self.envelope.section(kind);
        self.authority
            .verify_v1(stage, &self.axes, &self.transcript, descriptor, section)
    }

    fn verify_terminal_bridge_v1(
        self,
    ) -> Result<TerminalBridgeCheckedV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        self.verify_stage_v1(ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge)?;
        Ok(TerminalBridgeCheckedV1(self))
    }
}

struct TerminalBridgeCheckedV1(ContextCheckedV1);

impl TerminalBridgeCheckedV1 {
    fn verify_rns_relation_qpcs_v1(
        self,
    ) -> Result<RnsRelationQpcsCheckedV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        self.0
            .verify_stage_v1(ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs)?;
        Ok(RnsRelationQpcsCheckedV1(self.0))
    }
}

struct RnsRelationQpcsCheckedV1(ContextCheckedV1);

impl RnsRelationQpcsCheckedV1 {
    fn verify_cross_field_global_lookup_v1(
        self,
    ) -> Result<CrossFieldGlobalLookupCheckedV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1>
    {
        self.0
            .verify_stage_v1(ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup)?;
        Ok(CrossFieldGlobalLookupCheckedV1(self.0))
    }
}

struct CrossFieldGlobalLookupCheckedV1(ContextCheckedV1);

impl CrossFieldGlobalLookupCheckedV1 {
    fn verify_zero_padding_v1(
        self,
    ) -> Result<ZeroPaddingCheckedV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        self.0
            .verify_stage_v1(ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding)?;
        Ok(ZeroPaddingCheckedV1(self.0))
    }
}

struct ZeroPaddingCheckedV1(ContextCheckedV1);

impl ZeroPaddingCheckedV1 {
    fn finish_v1(
        self,
    ) -> Result<
        ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
    > {
        let axes = self.0.axes;
        let mut receipt = ZkAmsMkheRnsNativeCompositeCandidateReceiptV1 {
            version: ZK_AMS_MKHE_RNS_NATIVE_COMPOSITE_VERIFICATION_VERSION_V1,
            profile_manifest_digest: axes.profile_manifest_digest,
            topology_digest: axes.topology_digest,
            release_candidate_digest: axes.release_candidate_digest,
            statement_digest: axes.statement_digest,
            operational_context_digest: axes.operational_context_digest,
            source_binding_digest: axes.source_binding_digest,
            source_receipt_digest: axes.source_receipt_digest,
            governed_roster_digest: axes.governed_roster_digest,
            public_ciphertext_digest: axes.public_ciphertext_digest,
            transcript_digest: axes.transcript_digest,
            proof_digest: axes.proof_digest,
            section_digests: axes.section_digests,
            candidate_digest: [0; 32],
        };
        receipt.candidate_digest = candidate_receipt_digest_v1(&receipt);
        if receipt.candidate_digest == [0; 32]
            || receipt.section_digests.contains(&receipt.candidate_digest)
        {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
        }
        Ok(receipt)
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "the large exact-fixture authority exists only in test builds"
)]
enum FirstPartyStageAuthorityV1 {
    Production,
    #[cfg(test)]
    ExactFixture(Box<ExactFixtureStageAuthorityV1>),
}

impl FirstPartyStageAuthorityV1 {
    fn verify_v1(
        &self,
        stage: ZkAmsMkheRnsNativeVerificationStageV1,
        axes: &CandidateAxesV1,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
        section: &[u8],
    ) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        match self {
            Self::Production => {
                verify_production_stage_v1(stage, axes, transcript, descriptor, section)
            }
            #[cfg(test)]
            Self::ExactFixture(authority) => {
                authority.verify_v1(stage, axes, transcript, descriptor, section)
            }
        }
    }
}

fn verify_production_stage_v1(
    stage: ZkAmsMkheRnsNativeVerificationStageV1,
    axes: &CandidateAxesV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
    section: &[u8],
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    match stage {
        ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge => {
            verify_terminal_hyrax_bp_bridge_production_v1(axes, transcript, descriptor, section)
        }
        ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs => {
            verify_rns_relation_qpcs_production_v1(axes, transcript, descriptor, section)
        }
        ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup => {
            verify_cross_field_global_lookup_production_v1(axes, transcript, descriptor, section)
        }
        ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding => {
            verify_zero_padding_production_v1(axes, transcript, descriptor, section)
        }
    }
}

fn verify_terminal_hyrax_bp_bridge_production_v1(
    _axes: &CandidateAxesV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    _descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
    section: &[u8],
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    let typed = ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
        section, transcript,
    )
    .map_err(|_| {
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
            ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
        )
    })?;
    let _cross_basis =
        authenticate_rns_native_terminal_cross_basis_kernel_v1(transcript, typed.proof()).map_err(
            |_| {
                ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                    ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
                )
            },
        )?;
    // The kernel proves only representation equality for the detached ordered
    // point rows. A private source-to-Hyrax mapping prerequisite exists, but
    // this boundary cannot construct its preceding RLWE/source stage from the
    // public facts it owns. The atomic stage may not pass and no partial token
    // escapes.
    Err(
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(
            ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
        ),
    )
}

fn verify_rns_relation_qpcs_production_v1(
    _axes: &CandidateAxesV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    _descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
    section: &[u8],
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    let typed = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
        section, transcript,
    )
    .map_err(|_| {
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
        )
    })?;
    let _qpcs_fri = authenticate_rns_native_qpcs_fri_complete_v1(
        transcript,
        typed.equation_commitment_digests(),
        typed.limb_commitment_digests(),
        typed.query_opening_digests(),
        typed.proof(),
    )
    .map_err(|_| {
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
        )
    })?;
    // Initial, quotient, and all eighteen FRI tree memberships, every queried
    // opening/batch/fold equation, and the derived terminal-degree equation are
    // now checked. The retained RLWE/source residual still requires its own
    // verifier before this atomic stage may pass.
    Err(
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(
            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
        ),
    )
}

fn verify_cross_field_global_lookup_production_v1(
    _axes: &CandidateAxesV1,
    _transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    _descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
    _section: &[u8],
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    // The 40-limb cross-field prerequisites are not joined under one
    // authenticated source/terminal owner and explicitly lack an instantiated
    // global-lookup verifier and production authority.
    Err(
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(
            ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup,
        ),
    )
}

fn verify_zero_padding_production_v1(
    _axes: &CandidateAxesV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    _descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
    section: &[u8],
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    let typed =
        ZkAmsMkheRnsNativeZeroPaddingSectionV1::from_canonical_bytes_exact_v1(section, transcript)
            .map_err(|_| {
                ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                    ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
                )
            })?;
    let _padding = authenticate_rns_native_zero_padding_commitments_v1(
        transcript,
        typed.limb_padding_digests(),
        typed.proof(),
    )
    .map_err(|_| {
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
            ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
        )
    })?;
    // The committed padding inventory is authenticated as zero. Its private
    // source/terminal linkage prerequisite is not reachable from this
    // boundary, and no global-lookup verifier proves that these are the actual
    // governed padding lanes. No partial prerequisite escapes this adapter.
    Err(
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(
            ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
        ),
    )
}

fn validate_context_v1(
    envelope: &ZkAmsMkheRnsNativeProofEnvelopeV1,
    source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<CandidateAxesV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    source_layout
        .validate()
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;
    source_receipt
        .validate(source_layout)
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;

    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?;
    manifest
        .validate()
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?;
    let topology = zk_ams_mkhe_rns_native_topology_v1()
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?;
    topology
        .validate()
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?;
    let release_candidate = zk_ams_mkhe_rns_native_release_candidate_digest_v1()
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?;

    if envelope.profile_manifest_digest() != manifest.manifest_digest
        || source_layout.profile_digest() != manifest.profile_digest
        || envelope.topology_digest() != topology.topology_digest
        || source_layout.topology_digest() != topology.topology_digest
        || envelope.release_candidate_digest() != release_candidate
        || source_layout.release_candidate_digest() != release_candidate
        || envelope.statement_digest() != source_layout.statement_digest()
        || envelope.operational_context_digest() != source_layout.operational_context_digest()
        || envelope.source_receipt_digest() != source_receipt.receipt_digest
    {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext);
    }
    if transcript.profile_manifest_digest() != manifest.manifest_digest
        || transcript.profile_digest() != manifest.profile_digest
        || transcript.topology_digest() != topology.topology_digest
        || transcript.release_candidate_digest() != release_candidate
        || transcript.statement_digest() != source_layout.statement_digest()
        || transcript.operational_context_digest() != source_layout.operational_context_digest()
        || transcript.source_binding_digest() != source_layout.source_binding_digest()
        || transcript.main_snapshot_digest() != source_receipt.main_snapshot_digest
        || transcript.nonce_snapshot_digest() != source_receipt.nonce_snapshot_digest
        || transcript.source_receipt_digest() != source_receipt.receipt_digest
    {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
    }

    let mut section_digests = [[0_u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1];
    let mut total_wire_bytes = ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1;
    for (index, stage) in VERIFICATION_STAGE_ORDER_V1.into_iter().enumerate() {
        let kind = stage.section_kind();
        if ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1[index] != kind {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage));
        }
        let descriptor = envelope.descriptors()[index];
        let section = envelope.section(kind);
        let encoded_bytes = usize::try_from(descriptor.encoded_bytes())
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage))?;
        if descriptor.kind() != kind
            || descriptor.max_bytes() != kind.max_bytes()
            || section.is_empty()
            || encoded_bytes != section.len()
            || section.len()
                > usize::try_from(kind.max_bytes()).map_err(|_| {
                    ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage)
                })?
            || descriptor.section_digest() == [0; 32]
            || descriptor.section_digest() != section_digest_v1(kind, section)?
        {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage));
        }
        total_wire_bytes = total_wire_bytes
            .checked_add(section.len())
            .ok_or(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage))?;
        section_digests[index] = descriptor.section_digest();
    }
    if u32::try_from(total_wire_bytes).ok() != Some(envelope.total_wire_bytes())
        || envelope.proof_digest() == [0; 32]
        || whole_proof_digest_v1(envelope) != envelope.proof_digest()
    {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext);
    }
    validate_typed_sections_v1(envelope, transcript, &section_digests)?;

    let challenge_seeds = transcript.ordered_challenge_seeds();
    let mut digests = DigestRegistryV1::new();
    for digest in [
        manifest.manifest_digest,
        manifest.profile_digest,
        topology.topology_digest,
        release_candidate,
        source_layout.statement_digest(),
        source_layout.operational_context_digest(),
        source_layout.source_binding_digest(),
        source_receipt.main_snapshot_digest,
        source_receipt.nonce_snapshot_digest,
        source_receipt.receipt_digest,
        transcript.governed_roster_digest(),
        transcript.public_ciphertext_digest(),
        transcript.transcript_digest(),
        envelope.proof_digest(),
    ] {
        digests.insert(digest)?;
    }
    for digest in section_digests {
        digests.insert(digest)?;
    }
    for seed in challenge_seeds {
        digests.insert(seed)?;
    }

    let mut axes = CandidateAxesV1 {
        profile_manifest_digest: manifest.manifest_digest,
        topology_digest: topology.topology_digest,
        release_candidate_digest: release_candidate,
        statement_digest: source_layout.statement_digest(),
        operational_context_digest: source_layout.operational_context_digest(),
        source_binding_digest: source_layout.source_binding_digest(),
        source_receipt_digest: source_receipt.receipt_digest,
        governed_roster_digest: transcript.governed_roster_digest(),
        public_ciphertext_digest: transcript.public_ciphertext_digest(),
        transcript_digest: transcript.transcript_digest(),
        proof_digest: envelope.proof_digest(),
        section_digests,
        context_digest: [0; 32],
    };
    axes.context_digest = verification_context_digest_v1(&axes);
    if axes.context_digest == [0; 32]
        || digests.digests[..digests.len].contains(&axes.context_digest)
    {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
    }
    Ok(axes)
}

fn validate_typed_sections_v1(
    envelope: &ZkAmsMkheRnsNativeProofEnvelopeV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    section_digests: &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    validate_composite_section_set_exact_v1(
        envelope.section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge),
        envelope.section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs),
        envelope.section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup),
        envelope.section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding),
        transcript,
        section_digests,
        envelope.proof_digest(),
    )
    .map_err(|error| match error {
        CompositeSectionSetErrorV1::Section(kind) => {
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage_from_kind_v1(kind))
        }
        CompositeSectionSetErrorV1::CrossSectionAlias => {
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript
        }
    })
}

fn section_digest_v1(
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    bytes: &[u8],
) -> Result<[u8; 32], ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    let length = u64::try_from(bytes.len()).map_err(|_| {
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(stage_from_kind_v1(kind))
    })?;
    let mut hash = Keccak256::new();
    hash.update(SECTION_DIGEST_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1, kind as u8]);
    hash.update(&kind.max_bytes().to_be_bytes());
    hash.update(&length.to_be_bytes());
    hash.update(bytes);
    Ok(hash.finalize())
}

fn whole_proof_digest_v1(envelope: &ZkAmsMkheRnsNativeProofEnvelopeV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(WHOLE_PROOF_DOMAIN_V1);
    hash.update(&RNS_NATIVE_PROOF_ENVELOPE_TAG_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1]);
    hash.update(&envelope.profile_manifest_digest());
    hash.update(&envelope.topology_digest());
    hash.update(&envelope.release_candidate_digest());
    hash.update(&envelope.statement_digest());
    hash.update(&envelope.operational_context_digest());
    hash.update(&envelope.source_receipt_digest());
    hash.update(&[4]);
    hash.update(&envelope.total_wire_bytes().to_be_bytes());
    for descriptor in envelope.descriptors() {
        hash.update(&[descriptor.kind() as u8]);
        hash.update(&descriptor.max_bytes().to_be_bytes());
        hash.update(&descriptor.encoded_bytes().to_be_bytes());
        hash.update(&descriptor.section_digest());
    }
    for kind in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1 {
        hash.update(envelope.section(kind));
    }
    hash.finalize()
}

fn verification_context_digest_v1(axes: &CandidateAxesV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(VERIFICATION_CONTEXT_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_COMPOSITE_VERIFICATION_VERSION_V1]);
    for digest in [
        axes.profile_manifest_digest,
        axes.topology_digest,
        axes.release_candidate_digest,
        axes.statement_digest,
        axes.operational_context_digest,
        axes.source_binding_digest,
        axes.source_receipt_digest,
        axes.governed_roster_digest,
        axes.public_ciphertext_digest,
        axes.transcript_digest,
        axes.proof_digest,
    ] {
        hash.update(&digest);
    }
    for digest in axes.section_digests {
        hash.update(&digest);
    }
    hash.finalize()
}

fn candidate_receipt_digest_v1(
    receipt: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CANDIDATE_RECEIPT_DOMAIN_V1);
    hash.update(&[receipt.version]);
    for digest in [
        receipt.profile_manifest_digest,
        receipt.topology_digest,
        receipt.release_candidate_digest,
        receipt.statement_digest,
        receipt.operational_context_digest,
        receipt.source_binding_digest,
        receipt.source_receipt_digest,
        receipt.governed_roster_digest,
        receipt.public_ciphertext_digest,
        receipt.transcript_digest,
        receipt.proof_digest,
    ] {
        hash.update(&digest);
    }
    for digest in receipt.section_digests {
        hash.update(&digest);
    }
    hash.finalize()
}

fn stage_from_kind_v1(
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
) -> ZkAmsMkheRnsNativeVerificationStageV1 {
    match kind {
        ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge => {
            ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge
        }
        ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs => {
            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs
        }
        ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup => {
            ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup
        }
        ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding => {
            ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding
        }
    }
}

struct DigestRegistryV1 {
    digests: [[u8; 32]; MAX_BOUND_DIGESTS_V1],
    len: usize,
}

impl DigestRegistryV1 {
    const fn new() -> Self {
        Self {
            digests: [[0; 32]; MAX_BOUND_DIGESTS_V1],
            len: 0,
        }
    }

    fn insert(
        &mut self,
        digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        if digest == [0; 32] || self.digests[..self.len].contains(&digest) {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
        }
        let destination = self
            .digests
            .get_mut(self.len)
            .ok_or(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)?;
        *destination = digest;
        self.len += 1;
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct ExactFixtureStageExpectationV1 {
    stage: ZkAmsMkheRnsNativeVerificationStageV1,
    section_digest: [u8; 32],
    encoded_bytes: u32,
}

#[cfg(test)]
struct ExactFixtureStageAuthorityV1 {
    context_digest: [u8; 32],
    transcript_digest: [u8; 32],
    proof_digest: [u8; 32],
    expectations: [ExactFixtureStageExpectationV1; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    reject_stage: Option<ZkAmsMkheRnsNativeVerificationStageV1>,
}

#[cfg(test)]
impl ExactFixtureStageAuthorityV1 {
    fn new(
        envelope: &ZkAmsMkheRnsNativeProofEnvelopeV1,
        source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        let axes = validate_context_v1(envelope, source_layout, source_receipt, transcript)?;
        let expectations = core::array::from_fn(|index| {
            let descriptor = envelope.descriptors()[index];
            ExactFixtureStageExpectationV1 {
                stage: VERIFICATION_STAGE_ORDER_V1[index],
                section_digest: descriptor.section_digest(),
                encoded_bytes: descriptor.encoded_bytes(),
            }
        });
        Ok(Self {
            context_digest: axes.context_digest,
            transcript_digest: axes.transcript_digest,
            proof_digest: axes.proof_digest,
            expectations,
            reject_stage: None,
        })
    }

    fn reject(mut self, stage: ZkAmsMkheRnsNativeVerificationStageV1) -> Self {
        self.reject_stage = Some(stage);
        self
    }

    fn verify_v1(
        &self,
        stage: ZkAmsMkheRnsNativeVerificationStageV1,
        axes: &CandidateAxesV1,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
        section: &[u8],
    ) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        let expected = self.expectations[stage.index()];
        if self.reject_stage == Some(stage)
            || expected.stage != stage
            || expected.section_digest != descriptor.section_digest()
            || expected.encoded_bytes != descriptor.encoded_bytes()
            || usize::try_from(expected.encoded_bytes).ok() != Some(section.len())
            || self.context_digest != axes.context_digest
            || self.transcript_digest != transcript.transcript_digest()
            || self.transcript_digest != axes.transcript_digest
            || self.proof_digest != axes.proof_digest
        {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(stage));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "rns_native_composite_verifier_tests.rs"]
mod tests;

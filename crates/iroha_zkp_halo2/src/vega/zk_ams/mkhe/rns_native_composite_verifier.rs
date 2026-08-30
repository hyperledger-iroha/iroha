//! Atomic verifier boundary for the replacement RNS-native composite proof.
//!
//! The boundary consumes one move-only transport minted only after bounded
//! canonical decoding and verifier derivation of its source/transcript context
//! and exact 43-pair commitment root. It exposes no downstream verifier trait
//! and no partial stage receipt: all four sections must pass private first-party
//! adapters before a non-authorizing candidate receipt can be minted.
//!
//! None of the existing complete proof kernels can currently satisfy that
//! contract for the 40-limb replacement profile.  The replacement qPCS adapter
//! now authenticates every qPCS tree, checks all queried opening/batch equations,
//! all eighteen FRI folds, and the terminal-degree equation. The RLWE/source
//! linkage is still unavailable. The terminal adapter now checks the complete
//! 1,536-row cross-basis representation-equality kernel, but source mapping,
//! terminal materialization, and packing remain unavailable. The cross-field
//! implementation is fixed to the retired 38-limb shape,
//! the global-lookup module does not verify a proof, and the authenticated
//! zero-padding commitment inventory is not yet linked to the source, lookup,
//! or terminal materialization. Production therefore fails closed with an
//! explicit unavailable stage. Success here, once those adapters are replaced,
//! will still be proof verification only and can never grant readiness or
//! release authority.

#![allow(
    clippy::large_types_passed_by_value,
    reason = "atomic verification deliberately consumes fixed-capacity proof and transcript owners"
)]

use super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1, ZkAmsMkheRnsNativeFamilyV1,
        zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_qpcs_fri_complete::authenticate_rns_native_qpcs_fri_complete_v1,
    rns_native_section_codec::{
        CompositeSectionSetErrorV1, ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1,
        ZkAmsMkheRnsNativeTerminalBridgeSectionV1, ZkAmsMkheRnsNativeZeroPaddingSectionV1,
        validate_composite_section_set_exact_v1,
    },
    rns_native_source::{ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReceiptV1},
    rns_native_terminal_cross_basis::authenticate_rns_native_terminal_cross_basis_kernel_v1,
    rns_native_transcript::{
        ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1, ZkAmsMkheRnsNativeChallengeSeedsV1,
    },
    rns_native_wire::{
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1,
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
const OPENING_COMMITMENT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-verifier-opening-commitment-root";
const CANONICAL_WIRE_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-verifier-canonical-wire-binding";
const VERIFIER_AUTHENTICATED_TRANSPORT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-verifier-authenticated-transport";
const CANDIDATE_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-composite-candidate-receipt";
const ALGEBRAIC_RECEIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-algebraic-receipt";
const SECTION_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-proof-section";
const WHOLE_PROOF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-composite-proof-envelope";
const RNS_NATIVE_PROOF_ENVELOPE_TAG_V1: [u8; 4] = *b"ZANP";
const MAX_BOUND_DIGESTS_V1: usize = 22 + ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1;

/// Atomic composite-verifier schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_COMPOSITE_VERIFICATION_VERSION_V1: u8 = 1;
/// Verifier-authenticated canonical transport schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_VERIFIER_TRANSPORT_VERSION_V1: u8 = 1;
/// Opaque algebraic-receipt schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_ALGEBRAIC_RECEIPT_VERSION_V1: u8 = 1;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 == 4);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 == 43);
    assert!(MAX_BOUND_DIGESTS_V1 == 50);
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
    /// The independently retained source layout/receipt pair did not validate.
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

/// Leaf-private construction seal for one authenticated verifier transport.
struct VerifierAuthenticatedTransportSealV1;

/// Move-only canonical proof transport authenticated by the composite verifier.
///
/// The sole constructor first performs the envelope's bounded exact preflight,
/// then derives every context axis from the validated source layout/receipt and
/// final transcript. It also authenticates the exact ordered 43-pair source/
/// Hyrax commitment transport carried by the terminal section. This value is
/// byte/context evidence only: it cannot authorize release or stand in for any
/// unavailable algebraic stage.
///
/// The type has private fields and deliberately implements no clone, copy,
/// default, codec, or serialization surface. It retains only public proof
/// material and public digests; confidential source chunks are never accepted
/// or serialized here.
#[must_use = "dropping the transport discards its single verifier invocation"]
pub struct ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1 {
    _seal: VerifierAuthenticatedTransportSealV1,
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    axes: CandidateAxesV1,
}

impl core::fmt::Debug for ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
            .field("canonical_wire_bytes", &self.axes.canonical_wire_bytes)
            .field(
                "verifier_transport_digest",
                &hex::encode(self.axes.verifier_transport_digest),
            )
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1 {
    /// Authenticate one exact bounded canonical proof transport.
    ///
    /// The source layout and structural receipt are independently revalidated
    /// before decoding. The decoder allocates section owners only after its
    /// allocation-free length/cap/context preflight. The final verifier pass
    /// then derives the context digest, canonical wire binding, and ordered
    /// opening-commitment root rather than accepting any of them from the
    /// caller.
    ///
    /// # Errors
    ///
    /// Rejects every source mismatch, non-canonical or truncated wire, changed
    /// context, commitment substitution, section splice, or transcript replay.
    pub fn authenticate_canonical_exact_v1(
        bytes: &[u8],
        source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
        transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        source_layout
            .validate()
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;
        source_receipt
            .validate(source_layout)
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)?;
        let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::from_canonical_bytes_exact_v1(
            bytes,
            source_layout,
            source_receipt,
        )
        .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?;
        let axes = validate_context_v1(&envelope, source_layout, source_receipt, &transcript)?;
        if u32::try_from(bytes.len()).ok() != Some(axes.canonical_wire_bytes) {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext);
        }
        let transport = Self {
            _seal: VerifierAuthenticatedTransportSealV1,
            envelope,
            source_layout,
            source_receipt,
            transcript,
            axes,
        };
        transport.validate_v1()?;
        Ok(transport)
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        let rebuilt = validate_context_v1(
            &self.envelope,
            self.source_layout,
            self.source_receipt,
            &self.transcript,
        )?;
        if rebuilt != self.axes {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
        }
        Ok(())
    }

    fn into_verified_parts_v1(
        self,
    ) -> Result<
        (
            ZkAmsMkheRnsNativeProofEnvelopeV1,
            ZkAmsMkheRnsNativeChallengeSeedsV1,
            CandidateAxesV1,
        ),
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
    > {
        self.validate_v1()?;
        Ok((self.envelope, self.transcript, self.axes))
    }

    /// Exact bounded canonical wire length authenticated by the verifier.
    #[must_use]
    pub const fn canonical_wire_bytes(&self) -> u32 {
        self.axes.canonical_wire_bytes
    }

    /// Domain-separated identity of the sole canonical wire representation.
    #[must_use]
    pub const fn canonical_wire_digest(&self) -> [u8; 32] {
        self.axes.canonical_wire_digest
    }

    /// Verifier-derived root of all 43 ordered source/Hyrax commitment pairs.
    #[must_use]
    pub const fn opening_commitment_root(&self) -> [u8; 32] {
        self.axes.opening_commitment_root
    }

    /// Verifier-derived identity of every authenticated public context axis.
    #[must_use]
    pub const fn verifier_context_digest(&self) -> [u8; 32] {
        self.axes.verifier_context_digest
    }

    /// Complete canonical wire/context/commitment transport identity.
    #[must_use]
    pub const fn verifier_transport_digest(&self) -> [u8; 32] {
        self.axes.verifier_transport_digest
    }
}

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
    canonical_wire_bytes: u32,
    canonical_wire_digest: [u8; 32],
    opening_commitment_root: [u8; 32],
    verifier_context_digest: [u8; 32],
    verifier_transport_digest: [u8; 32],
    candidate_digest: [u8; 32],
    verification_seal: CompositeAlgebraicVerificationSealV1,
}

/// Module-private provenance retained from the terminal typestate transition.
///
/// Neither the public envelope nor a caller-nominated digest can construct or
/// update this seal. It commits the exact context established before the four
/// stage checks and the candidate digest established after them.
#[derive(Debug, PartialEq, Eq)]
struct CompositeAlgebraicVerificationSealV1 {
    verifier_context_digest: [u8; 32],
    verifier_transport_digest: [u8; 32],
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

    /// Exact canonical wire length consumed by transport authentication.
    #[must_use]
    pub const fn canonical_wire_bytes(&self) -> u32 {
        self.canonical_wire_bytes
    }

    /// Domain-separated canonical wire identity.
    #[must_use]
    pub const fn canonical_wire_digest(&self) -> [u8; 32] {
        self.canonical_wire_digest
    }

    /// Root of the exact ordered 43-pair commitment transport.
    #[must_use]
    pub const fn opening_commitment_root(&self) -> [u8; 32] {
        self.opening_commitment_root
    }

    /// Verifier-derived public context identity.
    #[must_use]
    pub const fn verifier_context_digest(&self) -> [u8; 32] {
        self.verifier_context_digest
    }

    /// Complete verifier-authenticated transport identity.
    #[must_use]
    pub const fn verifier_transport_digest(&self) -> [u8; 32] {
        self.verifier_transport_digest
    }

    /// Digest of this complete non-authorizing candidate receipt.
    #[must_use]
    pub const fn candidate_digest(&self) -> [u8; 32] {
        self.candidate_digest
    }

    /// Consume the atomic four-stage result into its only algebraic receipt.
    ///
    /// This conversion revalidates every bound axis and both receipt digests.
    /// It accepts no caller-supplied digest, boolean, verifier callback, or
    /// decoded proof component. The returned receipt remains proof-verification
    /// evidence only; terminal and decryption consumers require separate
    /// move-only receipt-enforcement handoffs.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript`]
    /// if the move-only candidate was corrupted after verification.
    fn into_algebraic_receipt_v1(
        self,
    ) -> Result<ZkAmsMkheRnsNativeAlgebraicReceiptV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1>
    {
        ZkAmsMkheRnsNativeAlgebraicReceiptV1::from_verified_composite_v1(self)
    }
}

/// Move-only receipt proving that all four RNS-native algebraic stages passed.
///
/// Construction is sealed behind the consumed atomic composite candidate. The
/// receipt has no public fields, codec, default, clone, or copy surface, so
/// canonical proof bytes and digest shells cannot manufacture this capability.
/// It authorizes only the completed RNS-Link verification result; it is not a
/// release/readiness certificate. Terminal and split-decryption consumers can
/// only obtain their own move-only uses by consuming this receipt.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "dropping the algebraic receipt discards the verified RNS-Link capability"]
pub struct ZkAmsMkheRnsNativeAlgebraicReceiptV1 {
    composite: ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
    receipt_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeAlgebraicReceiptV1 {
    fn from_verified_composite_v1(
        composite: ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        validate_composite_candidate_integrity_v1(&composite)?;
        let receipt_digest = algebraic_receipt_digest_v1(&composite);
        let receipt = Self {
            composite,
            receipt_digest,
        };
        receipt.validate_v1()?;
        Ok(receipt)
    }

    /// Revalidate the sealed composite candidate and algebraic receipt digest.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript`]
    /// if any in-memory binding or digest no longer matches.
    pub fn validate_v1(&self) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        validate_composite_candidate_integrity_v1(&self.composite)?;
        let expected = algebraic_receipt_digest_v1(&self.composite);
        if self.receipt_digest == [0; 32]
            || self.receipt_digest != expected
            || self.receipt_digest == self.composite.candidate_digest
            || self
                .composite
                .section_digests
                .contains(&self.receipt_digest)
        {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
        }
        Ok(())
    }

    /// Canonical 40-limb proof-profile identity.
    #[must_use]
    pub const fn profile_manifest_digest(&self) -> [u8; 32] {
        self.composite.profile_manifest_digest
    }

    /// Canonical 40-limb proof-topology identity.
    #[must_use]
    pub const fn topology_digest(&self) -> [u8; 32] {
        self.composite.topology_digest
    }

    /// Exact release-candidate identity.
    #[must_use]
    pub const fn release_candidate_digest(&self) -> [u8; 32] {
        self.composite.release_candidate_digest
    }

    /// Exact statement identity.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.composite.statement_digest
    }

    /// Exact operational/replay context identity.
    #[must_use]
    pub const fn operational_context_digest(&self) -> [u8; 32] {
        self.composite.operational_context_digest
    }

    /// Exact confidential-source binding.
    #[must_use]
    pub const fn source_binding_digest(&self) -> [u8; 32] {
        self.composite.source_binding_digest
    }

    /// Independently validated structural source-receipt identity.
    #[must_use]
    pub const fn source_receipt_digest(&self) -> [u8; 32] {
        self.composite.source_receipt_digest
    }

    /// Exact governed verifier-roster identity.
    #[must_use]
    pub const fn governed_roster_digest(&self) -> [u8; 32] {
        self.composite.governed_roster_digest
    }

    /// Exact public ciphertext/statement-material identity.
    #[must_use]
    pub const fn public_ciphertext_digest(&self) -> [u8; 32] {
        self.composite.public_ciphertext_digest
    }

    /// Fully ratcheted canonical transcript identity.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.composite.transcript_digest
    }

    /// Complete canonical envelope identity.
    #[must_use]
    pub const fn proof_digest(&self) -> [u8; 32] {
        self.composite.proof_digest
    }

    /// Four algebraically verified section identities in canonical order.
    #[must_use]
    pub const fn section_digests(
        &self,
    ) -> &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1] {
        &self.composite.section_digests
    }

    /// Exact canonical wire length authenticated before algebraic verification.
    #[must_use]
    pub const fn canonical_wire_bytes(&self) -> u32 {
        self.composite.canonical_wire_bytes
    }

    /// Domain-separated canonical wire identity.
    #[must_use]
    pub const fn canonical_wire_digest(&self) -> [u8; 32] {
        self.composite.canonical_wire_digest
    }

    /// Root of all 43 authenticated source/Hyrax commitment pairs.
    #[must_use]
    pub const fn opening_commitment_root(&self) -> [u8; 32] {
        self.composite.opening_commitment_root
    }

    /// Verifier-derived public context identity.
    #[must_use]
    pub const fn verifier_context_digest(&self) -> [u8; 32] {
        self.composite.verifier_context_digest
    }

    /// Complete verifier-authenticated transport identity.
    #[must_use]
    pub const fn verifier_transport_digest(&self) -> [u8; 32] {
        self.composite.verifier_transport_digest
    }

    /// Digest of the consumed atomic composite candidate.
    #[must_use]
    pub const fn composite_candidate_digest(&self) -> [u8; 32] {
        self.composite.candidate_digest
    }

    /// Domain-separated digest of this opaque algebraic receipt.
    #[must_use]
    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }
}

/// Atomically verify one replacement composite proof.
///
/// The verifier-authenticated transport is consumed even on failure. No raw
/// envelope, caller-nominated context digest, or detached commitment set can
/// enter this boundary. No partial stage result escapes it. At present a valid cross-basis kernel
/// still returns
/// [`ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable`]
/// because its source mapping, terminal materialization, and packing seals are
/// not implemented.
///
/// # Errors
///
/// Rejects mismatched source/envelope/transcript contexts, malformed section
/// structure, any unavailable proof stage, or any first-party stage failure.
pub fn verify_zk_ams_mkhe_rns_native_composite_v1(
    transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
) -> Result<
    ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
    ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
> {
    verify_with_first_party_authority_v1(transport, FirstPartyStageAuthorityV1::Production)
}

/// Atomically verify one replacement proof and mint its algebraic receipt.
///
/// This is the sole production entry point returning the opaque RNS-Link
/// algebraic capability. It delegates all proof work to the exact four-stage
/// composite verifier and then consumes its integrity-checked terminal result.
/// Existing unavailable stage adapters therefore continue to fail closed.
///
/// # Errors
///
/// Returns the exact composite verification failure, including any explicitly
/// unavailable production proof stage, or rejects a corrupted terminal result.
pub fn verify_zk_ams_mkhe_rns_native_algebraic_v1(
    transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
) -> Result<ZkAmsMkheRnsNativeAlgebraicReceiptV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    verify_algebraic_with_first_party_authority_v1(
        transport,
        FirstPartyStageAuthorityV1::Production,
    )
}

fn verify_algebraic_with_first_party_authority_v1(
    transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
    authority: FirstPartyStageAuthorityV1,
) -> Result<ZkAmsMkheRnsNativeAlgebraicReceiptV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    // Minting is inseparable from re-running the exact atomic stage chain over
    // the consumed proof/context owners. No public API accepts a prebuilt
    // candidate, a stage mask, booleans, or caller-nominated receipt digests.
    verify_with_first_party_authority_v1(transport, authority)?.into_algebraic_receipt_v1()
}

fn verify_with_first_party_authority_v1(
    transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
    authority: FirstPartyStageAuthorityV1,
) -> Result<
    ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
    ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
> {
    ContextCheckedV1::new(transport, authority)?
        .verify_terminal_bridge_v1()?
        .verify_rns_relation_qpcs_v1()?
        .verify_cross_field_global_lookup_v1()?
        .verify_zero_padding_v1()?
        .finish_v1()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    canonical_wire_bytes: u32,
    canonical_wire_digest: [u8; 32],
    opening_commitment_root: [u8; 32],
    verifier_context_digest: [u8; 32],
    verifier_transport_digest: [u8; 32],
}

struct ContextCheckedV1 {
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    axes: CandidateAxesV1,
    authority: FirstPartyStageAuthorityV1,
}

impl ContextCheckedV1 {
    fn new(
        transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
        authority: FirstPartyStageAuthorityV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        let (envelope, transcript, axes) = transport.into_verified_parts_v1()?;
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
            canonical_wire_bytes: axes.canonical_wire_bytes,
            canonical_wire_digest: axes.canonical_wire_digest,
            opening_commitment_root: axes.opening_commitment_root,
            verifier_context_digest: axes.verifier_context_digest,
            verifier_transport_digest: axes.verifier_transport_digest,
            candidate_digest: [0; 32],
            verification_seal: CompositeAlgebraicVerificationSealV1 {
                verifier_context_digest: axes.verifier_context_digest,
                verifier_transport_digest: axes.verifier_transport_digest,
                candidate_digest: [0; 32],
            },
        };
        receipt.candidate_digest = candidate_receipt_digest_v1(&receipt);
        receipt.verification_seal.candidate_digest = receipt.candidate_digest;
        validate_composite_candidate_integrity_v1(&receipt)?;
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
    // point rows. The source-to-Hyrax mapping, terminal materialization, and
    // production source/packing seals remain absent, so the atomic stage may
    // not pass and no partial token escapes.
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
    // The cross-field prerequisite is fixed to 38 limbs and explicitly lacks
    // an instantiated global-lookup verifier and production authority.
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
    // The committed padding inventory is authenticated as zero, but no
    // source/global-lookup/terminal owner yet proves that these are the actual
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
    let opening_commitment_root = opening_commitment_root_v1(transcript)?;
    let canonical_wire_bytes = envelope.total_wire_bytes();
    if canonical_wire_bytes
        < u32::try_from(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1)
            .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)?
        || canonical_wire_bytes > ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1
    {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext);
    }

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
        canonical_wire_bytes,
        canonical_wire_digest: [0; 32],
        opening_commitment_root,
        verifier_context_digest: [0; 32],
        verifier_transport_digest: [0; 32],
    };
    digests.insert(axes.opening_commitment_root)?;
    axes.canonical_wire_digest = canonical_wire_binding_digest_v1(&axes);
    digests.insert(axes.canonical_wire_digest)?;
    axes.verifier_context_digest = verification_context_digest_v1(&axes);
    digests.insert(axes.verifier_context_digest)?;
    axes.verifier_transport_digest = verifier_authenticated_transport_digest_v1(&axes);
    digests.insert(axes.verifier_transport_digest)?;
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

fn opening_commitment_root_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<[u8; 32], ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    const OPENING_DIGESTS_V1: usize = 2 * ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize;
    let mut seen = [[0_u8; 32]; OPENING_DIGESTS_V1];
    let mut seen_len = 0_usize;
    let mut hash = Keccak256::new();
    hash.update(OPENING_COMMITMENT_ROOT_DOMAIN_V1);
    hash.update(&[
        ZK_AMS_MKHE_RNS_NATIVE_VERIFIER_TRANSPORT_VERSION_V1,
        ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
    ]);
    for (ordinal, opening) in transcript.opening_commitments().iter().copied().enumerate() {
        let expected = expected_opening_role_v1(ordinal)
            .ok_or(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)?;
        let source = opening.source_commitment_digest();
        let hyrax = opening.hyrax_commitment_digest();
        if (opening.family(), opening.family_index()) != expected
            || source == [0; 32]
            || hyrax == [0; 32]
            || source == hyrax
        {
            return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
        }
        for digest in [source, hyrax] {
            if seen[..seen_len].contains(&digest) {
                return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
            }
            seen[seen_len] = digest;
            seen_len += 1;
        }
        hash.update(
            &u16::try_from(ordinal)
                .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)?
                .to_be_bytes(),
        );
        hash.update(&[opening.family() as u8, opening.family_index()]);
        hash.update(&source);
        hash.update(&hyrax);
    }
    if seen_len != OPENING_DIGESTS_V1 {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
    }
    let root = hash.finalize();
    if root == [0; 32] || seen.contains(&root) {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
    }
    Ok(root)
}

fn expected_opening_role_v1(ordinal: usize) -> Option<(ZkAmsMkheRnsNativeFamilyV1, u8)> {
    match ordinal {
        0 => Some((ZkAmsMkheRnsNativeFamilyV1::X, 0)),
        1..=16 => Some((
            ZkAmsMkheRnsNativeFamilyV1::U,
            u8::try_from(ordinal - 1).ok()?,
        )),
        17..=32 => Some((
            ZkAmsMkheRnsNativeFamilyV1::E,
            u8::try_from(ordinal - 17).ok()?,
        )),
        33 => Some((ZkAmsMkheRnsNativeFamilyV1::RE, 0)),
        34..=41 => Some((
            ZkAmsMkheRnsNativeFamilyV1::W,
            u8::try_from(ordinal - 34).ok()?,
        )),
        42 => Some((ZkAmsMkheRnsNativeFamilyV1::RW, 0)),
        _ => None,
    }
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
    hash.update(&axes.opening_commitment_root);
    hash.finalize()
}

fn canonical_wire_binding_digest_v1(axes: &CandidateAxesV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CANONICAL_WIRE_BINDING_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_VERIFIER_TRANSPORT_VERSION_V1]);
    hash.update(&axes.canonical_wire_bytes.to_be_bytes());
    for digest in [
        axes.profile_manifest_digest,
        axes.topology_digest,
        axes.release_candidate_digest,
        axes.statement_digest,
        axes.operational_context_digest,
        axes.source_receipt_digest,
        axes.proof_digest,
    ] {
        hash.update(&digest);
    }
    for digest in axes.section_digests {
        hash.update(&digest);
    }
    hash.finalize()
}

fn verifier_authenticated_transport_digest_v1(axes: &CandidateAxesV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(VERIFIER_AUTHENTICATED_TRANSPORT_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_VERIFIER_TRANSPORT_VERSION_V1]);
    hash.update(&axes.canonical_wire_bytes.to_be_bytes());
    for digest in [
        axes.canonical_wire_digest,
        axes.verifier_context_digest,
        axes.opening_commitment_root,
        axes.transcript_digest,
        axes.proof_digest,
    ] {
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
    hash.update(&receipt.canonical_wire_bytes.to_be_bytes());
    for digest in [
        receipt.canonical_wire_digest,
        receipt.opening_commitment_root,
        receipt.verifier_context_digest,
        receipt.verifier_transport_digest,
    ] {
        hash.update(&digest);
    }
    hash.finalize()
}

fn validate_composite_candidate_integrity_v1(
    candidate: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
) -> Result<(), ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    if candidate.version != ZK_AMS_MKHE_RNS_NATIVE_COMPOSITE_VERIFICATION_VERSION_V1
        || candidate.candidate_digest == [0; 32]
        || candidate.candidate_digest != candidate_receipt_digest_v1(candidate)
        || candidate.canonical_wire_bytes
            < u32::try_from(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1)
                .map_err(|_| ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)?
        || candidate.canonical_wire_bytes > ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1
        || candidate.canonical_wire_digest != canonical_wire_digest_from_candidate_v1(candidate)
        || candidate.verifier_context_digest
            != verification_context_digest_from_candidate_v1(candidate)
        || candidate.verifier_transport_digest
            != verifier_transport_digest_from_candidate_v1(candidate)
        || candidate.verification_seal.verifier_context_digest == [0; 32]
        || candidate.verification_seal.verifier_context_digest != candidate.verifier_context_digest
        || candidate.verification_seal.verifier_transport_digest == [0; 32]
        || candidate.verification_seal.verifier_transport_digest
            != candidate.verifier_transport_digest
        || candidate.verification_seal.candidate_digest != candidate.candidate_digest
    {
        return Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript);
    }
    let mut digests = DigestRegistryV1::new();
    for digest in [
        candidate.profile_manifest_digest,
        candidate.topology_digest,
        candidate.release_candidate_digest,
        candidate.statement_digest,
        candidate.operational_context_digest,
        candidate.source_binding_digest,
        candidate.source_receipt_digest,
        candidate.governed_roster_digest,
        candidate.public_ciphertext_digest,
        candidate.transcript_digest,
        candidate.proof_digest,
    ] {
        digests.insert(digest)?;
    }
    for digest in candidate.section_digests {
        digests.insert(digest)?;
    }
    for digest in [
        candidate.canonical_wire_digest,
        candidate.opening_commitment_root,
        candidate.verifier_context_digest,
        candidate.verifier_transport_digest,
    ] {
        digests.insert(digest)?;
    }
    digests.insert(candidate.candidate_digest)
}

fn verification_context_digest_from_candidate_v1(
    candidate: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
) -> [u8; 32] {
    verification_context_digest_v1(&candidate_axes_v1(candidate))
}

fn canonical_wire_digest_from_candidate_v1(
    candidate: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
) -> [u8; 32] {
    canonical_wire_binding_digest_v1(&candidate_axes_v1(candidate))
}

fn verifier_transport_digest_from_candidate_v1(
    candidate: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
) -> [u8; 32] {
    verifier_authenticated_transport_digest_v1(&candidate_axes_v1(candidate))
}

fn candidate_axes_v1(candidate: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1) -> CandidateAxesV1 {
    CandidateAxesV1 {
        profile_manifest_digest: candidate.profile_manifest_digest,
        topology_digest: candidate.topology_digest,
        release_candidate_digest: candidate.release_candidate_digest,
        statement_digest: candidate.statement_digest,
        operational_context_digest: candidate.operational_context_digest,
        source_binding_digest: candidate.source_binding_digest,
        source_receipt_digest: candidate.source_receipt_digest,
        governed_roster_digest: candidate.governed_roster_digest,
        public_ciphertext_digest: candidate.public_ciphertext_digest,
        transcript_digest: candidate.transcript_digest,
        proof_digest: candidate.proof_digest,
        section_digests: candidate.section_digests,
        canonical_wire_bytes: candidate.canonical_wire_bytes,
        canonical_wire_digest: candidate.canonical_wire_digest,
        opening_commitment_root: candidate.opening_commitment_root,
        verifier_context_digest: candidate.verifier_context_digest,
        verifier_transport_digest: candidate.verifier_transport_digest,
    }
}

fn algebraic_receipt_digest_v1(
    candidate: &ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(ALGEBRAIC_RECEIPT_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_ALGEBRAIC_RECEIPT_VERSION_V1]);
    for digest in [
        candidate.profile_manifest_digest,
        candidate.topology_digest,
        candidate.release_candidate_digest,
        candidate.statement_digest,
        candidate.operational_context_digest,
        candidate.source_binding_digest,
        candidate.source_receipt_digest,
        candidate.governed_roster_digest,
        candidate.public_ciphertext_digest,
        candidate.transcript_digest,
        candidate.proof_digest,
        candidate.candidate_digest,
    ] {
        hash.update(&digest);
    }
    for digest in candidate.section_digests {
        hash.update(&digest);
    }
    hash.update(&candidate.canonical_wire_bytes.to_be_bytes());
    for digest in [
        candidate.canonical_wire_digest,
        candidate.opening_commitment_root,
        candidate.verifier_context_digest,
        candidate.verifier_transport_digest,
    ] {
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
    verifier_context_digest: [u8; 32],
    verifier_transport_digest: [u8; 32],
    transcript_digest: [u8; 32],
    proof_digest: [u8; 32],
    expectations: [ExactFixtureStageExpectationV1; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    reject_stage: Option<ZkAmsMkheRnsNativeVerificationStageV1>,
}

#[cfg(test)]
impl ExactFixtureStageAuthorityV1 {
    fn new(
        transport: &ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
        transport.validate_v1()?;
        let envelope = &transport.envelope;
        let axes = transport.axes;
        let expectations = core::array::from_fn(|index| {
            let descriptor = envelope.descriptors()[index];
            ExactFixtureStageExpectationV1 {
                stage: VERIFICATION_STAGE_ORDER_V1[index],
                section_digest: descriptor.section_digest(),
                encoded_bytes: descriptor.encoded_bytes(),
            }
        });
        Ok(Self {
            verifier_context_digest: axes.verifier_context_digest,
            verifier_transport_digest: axes.verifier_transport_digest,
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
            || self.verifier_context_digest != axes.verifier_context_digest
            || self.verifier_transport_digest != axes.verifier_transport_digest
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

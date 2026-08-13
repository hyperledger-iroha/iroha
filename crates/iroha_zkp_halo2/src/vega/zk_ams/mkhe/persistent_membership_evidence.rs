//! Canonical eight-chunk membership evidence for one persistent MKHE secret.
//!
//! This container binds eight ordered transparent T256 generalized-
//! Bulletproofs to the complete collective-public-key source context.  Each
//! proof certifies exactly 16,384 coefficients in `{-1, 0, 1}`, so the set
//! covers one 131,072-coefficient release polynomial without a partial-chunk
//! or variable-shape path.
//!
//! Parsing this evidence does not create a verified witness binding.  In
//! particular, membership of a committed vector does not establish that the
//! same vector was used in a CPK share relation.  That separate relation proof
//! remains fail closed at its state-owned minting boundary.
#[cfg(test)]
use super::exact_eight_chunk_membership::{
    commitment_set_digest as exact_commitment_set_digest,
    proof_set_digest as exact_proof_set_digest,
    verifier_transcript_set_digest as exact_verifier_transcript_set_digest,
};
use super::{
    ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    exact_eight_chunk_membership::{
        ExactEightChunkMembershipContextV1, ExactEightChunkMembershipErrorV1,
        ExactEightChunkMembershipEvidenceV1, PersistentSecretMembershipRoleV1,
        VerifiedExactEightChunkMembershipV1,
    },
};
#[cfg(test)]
use crate::vega::{
    bulletproof_t256::{
        prove_zk_ams_t256_membership_chunk_v1, verify_zk_ams_t256_membership_chunk_v1,
    },
    sponge::Keccak256,
};
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZkAmsT256MembershipBoundV1,
            ZkAmsT256MembershipErrorV1, ZkAmsT256MembershipProofV1,
        },
    },
};
use thiserror::Error;
const PERSISTENT_MEMBERSHIP_MAGIC_V1: [u8; 4] = *b"ZPME";
const PERSISTENT_MEMBERSHIP_VERSION_V1: u8 = 1;
const PERSISTENT_MEMBERSHIP_BOUND_V1: ZkAmsT256MembershipBoundV1 = ZkAmsT256MembershipBoundV1::One;
/// Exact number of ordered proofs for one release-ring persistent secret.
pub(super) const ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1: usize = 8;
/// Exact coefficient count covered by the complete persistent proof set.
pub(super) const ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_COEFFICIENTS_V1: usize =
    ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 * ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1;
/// Membership evidence alone never certifies linkage to the CPK share relation.
pub(super) const ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CPK_RELATION_LINKED_V1: bool = false;
const BOUND_ONE_PROOF_BYTES_V1: usize = 1_447;
const MEMBERSHIP_CHUNK_WIRE_HEADER_BYTES_V1: usize = 47;
const MEMBERSHIP_CHUNK_WIRE_BYTES_V1: usize =
    MEMBERSHIP_CHUNK_WIRE_HEADER_BYTES_V1 + BOUND_ONE_PROOF_BYTES_V1;
const OFFSET_BOUND_V1: usize = 5;
const OFFSET_CHUNK_COUNT_V1: usize = OFFSET_BOUND_V1 + 1;
const OFFSET_COEFFICIENT_COUNT_V1: usize = OFFSET_CHUNK_COUNT_V1 + 1;
const OFFSET_GENERATOR_BASIS_DIGEST_V1: usize = OFFSET_COEFFICIENT_COUNT_V1 + 4;
const OFFSET_PROFILE_DIGEST_V1: usize = OFFSET_GENERATOR_BASIS_DIGEST_V1 + 32;
const OFFSET_ROSTER_DIGEST_V1: usize = OFFSET_PROFILE_DIGEST_V1 + 32;
const OFFSET_KEY_MATERIAL_DIGEST_V1: usize = OFFSET_ROSTER_DIGEST_V1 + 32;
const OFFSET_EPOCH_V1: usize = OFFSET_KEY_MATERIAL_DIGEST_V1 + 32;
const OFFSET_CPK_TRANSCRIPT_DIGEST_V1: usize = OFFSET_EPOCH_V1 + 8;
const OFFSET_PARTY_V1: usize = OFFSET_CPK_TRANSCRIPT_DIGEST_V1 + 32;
const OFFSET_SHARE_STATEMENT_DIGEST_V1: usize = OFFSET_PARTY_V1 + 32;
const OFFSET_COMMITMENT_SET_DIGEST_V1: usize = OFFSET_SHARE_STATEMENT_DIGEST_V1 + 32;
const OFFSET_PROOF_SET_DIGEST_V1: usize = OFFSET_COMMITMENT_SET_DIGEST_V1 + 32;
const OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1: usize = OFFSET_PROOF_SET_DIGEST_V1 + 32;
const PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1: usize = OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1 + 32;
/// Exact canonical wire length of one persistent membership evidence set.
pub(super) const ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_WIRE_BYTES_V1: usize =
    PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1
        + ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * MEMBERSHIP_CHUNK_WIRE_BYTES_V1;
const _: () = {
    assert!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_COEFFICIENTS_V1 == 131_072);
    assert!(BOUND_ONE_PROOF_BYTES_V1 == 1_447);
    assert!(MEMBERSHIP_CHUNK_WIRE_BYTES_V1 == 1_494);
    assert!(PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1 == 339);
    assert!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
    assert!(!ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CPK_RELATION_LINKED_V1);
};
/// Stable failures for persistent T256 membership evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum ZkAmsMkhePersistentMembershipErrorV1 {
    /// One or more source-context axes are zero or inconsistent with the roster.
    #[error("invalid ZK-AMS persistent-membership context")]
    Context,
    /// The outer or inner proof-set shape is not the exact release shape.
    #[error("invalid ZK-AMS persistent-membership proof-set shape")]
    Shape,
    /// The wire is truncated, extended, malformed, or non-canonical.
    #[error("invalid canonical ZK-AMS persistent-membership wire encoding")]
    WireEncoding,
    /// A stored ordered-set digest does not recompute from the evidence.
    #[error("invalid ZK-AMS persistent-membership ordered-set digest")]
    DigestMismatch,
    /// The encoded generator basis is not the pinned production basis.
    #[error("invalid ZK-AMS persistent-membership generator basis")]
    GeneratorBasis,
    /// One T256 coefficient-membership proof failed.
    #[error(transparent)]
    Membership(#[from] ZkAmsT256MembershipErrorV1),
}
impl From<ExactEightChunkMembershipErrorV1> for ZkAmsMkhePersistentMembershipErrorV1 {
    fn from(error: ExactEightChunkMembershipErrorV1) -> Self {
        match error {
            ExactEightChunkMembershipErrorV1::Context => Self::Context,
            ExactEightChunkMembershipErrorV1::Shape => Self::Shape,
            ExactEightChunkMembershipErrorV1::WireEncoding => Self::WireEncoding,
            ExactEightChunkMembershipErrorV1::DigestMismatch => Self::DigestMismatch,
            ExactEightChunkMembershipErrorV1::GeneratorBasis => Self::GeneratorBasis,
            ExactEightChunkMembershipErrorV1::Membership(error) => Self::Membership(error),
        }
    }
}
/// Complete public context bound into all eight persistent-membership proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkhePersistentMembershipContextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    party: ZkAmsMkhePartyIdV1,
    share_statement_digest: [u8; 32],
}
impl ZkAmsMkhePersistentMembershipContextV1 {
    /// Construct the exact context for one governed roster participant.
    pub(super) fn from_governed_roster(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        party_index: usize,
        cpk_transcript_digest: [u8; 32],
        share_statement_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        roster
            .validate()
            .map_err(|_| ZkAmsMkhePersistentMembershipErrorV1::Context)?;
        let participant = roster
            .participants()
            .get(party_index)
            .ok_or(ZkAmsMkhePersistentMembershipErrorV1::Context)?;
        Self::new(
            roster.profile_digest(),
            roster.roster_digest(),
            roster.key_material_digest(),
            roster.epoch(),
            cpk_transcript_digest,
            participant.party(),
            share_statement_digest,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn new(
        profile_digest: [u8; 32],
        roster_digest: [u8; 32],
        key_material_digest: [u8; 32],
        epoch: u64,
        cpk_transcript_digest: [u8; 32],
        party: ZkAmsMkhePartyIdV1,
        share_statement_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        let context = Self {
            profile_digest,
            roster_digest,
            key_material_digest,
            epoch,
            cpk_transcript_digest,
            party,
            share_statement_digest,
        };
        context.validate()?;
        Ok(context)
    }
    /// Construct the role-specific context expected by the CPK relation adapter.
    ///
    /// The complete statement digest binds the statement-only security,
    /// public-`a`, participant-index, and party-`b` axes in addition to the
    /// repeated source axes carried directly by this frame.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_relation_axes(
        profile_digest: [u8; 32],
        roster_digest: [u8; 32],
        key_material_digest: [u8; 32],
        epoch: u64,
        cpk_transcript_digest: [u8; 32],
        party: ZkAmsMkhePartyIdV1,
        share_statement_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        Self::new(
            profile_digest,
            roster_digest,
            key_material_digest,
            epoch,
            cpk_transcript_digest,
            party,
            share_statement_digest,
        )
    }
    fn validate(self) -> Result<(), ZkAmsMkhePersistentMembershipErrorV1> {
        self.to_exact().map(|_| ())
    }
    fn to_exact(
        self,
    ) -> Result<
        ExactEightChunkMembershipContextV1<PersistentSecretMembershipRoleV1>,
        ZkAmsMkhePersistentMembershipErrorV1,
    > {
        ExactEightChunkMembershipContextV1::new(
            self.profile_digest,
            self.roster_digest,
            self.key_material_digest,
            self.epoch,
            self.cpk_transcript_digest,
            self.party,
            self.share_statement_digest,
        )
        .map_err(Into::into)
    }
    fn from_exact(
        context: ExactEightChunkMembershipContextV1<PersistentSecretMembershipRoleV1>,
    ) -> Self {
        Self {
            profile_digest: context.profile_digest(),
            roster_digest: context.roster_digest(),
            key_material_digest: context.key_material_digest(),
            epoch: context.epoch(),
            cpk_transcript_digest: context.cpk_transcript_digest(),
            party: context.party(),
            share_statement_digest: context.share_statement_digest(),
        }
    }
    /// Frozen release-profile digest.
    pub(super) const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }
    /// Exact governed-roster digest.
    pub(super) const fn roster_digest(self) -> [u8; 32] {
        self.roster_digest
    }
    /// Ordered roster-key-material digest.
    pub(super) const fn key_material_digest(self) -> [u8; 32] {
        self.key_material_digest
    }
    /// Nonzero governed key epoch.
    pub(super) const fn epoch(self) -> u64 {
        self.epoch
    }
    /// Collective-public-key transcript digest used as source context.
    pub(super) const fn cpk_transcript_digest(self) -> [u8; 32] {
        self.cpk_transcript_digest
    }
    /// Participant whose persistent secret is committed.
    pub(super) const fn party(self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }
    /// Digest of the exact CPK-share statement associated with this evidence.
    pub(super) const fn share_statement_digest(self) -> [u8; 32] {
        self.share_statement_digest
    }
    /// Digest absorbed by every chunk proof and every ordered-set root.
    pub(super) fn context_digest(self) -> [u8; 32] {
        self.to_exact()
            .expect("validated persistent-membership context")
            .context_digest()
    }
}
/// Canonical public evidence for one persistent bound-one secret polynomial.
///
/// This type deliberately has no conversion to
/// `VerifiedPersistentWitnessBindingV1`.  [`Self::verify`] establishes only
/// coefficient membership and context-bound transcript integrity, not the CPK
/// relation which must consume the same commitment set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkhePersistentMembershipEvidenceV1 {
    context: ZkAmsMkhePersistentMembershipContextV1,
    generator_basis_digest: [u8; 32],
    chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    commitment_set_digest: [u8; 32],
    proof_set_digest: [u8; 32],
    verifier_transcript_digest: [u8; 32],
}
impl ZkAmsMkhePersistentMembershipEvidenceV1 {
    fn from_exact(
        evidence: ExactEightChunkMembershipEvidenceV1<PersistentSecretMembershipRoleV1>,
    ) -> Self {
        Self {
            context: ZkAmsMkhePersistentMembershipContextV1::from_exact(evidence.context()),
            generator_basis_digest: evidence.generator_basis_digest(),
            chunks: evidence.chunks().clone(),
            commitment_set_digest: evidence.commitment_set_digest(),
            proof_set_digest: evidence.proof_set_digest(),
            verifier_transcript_digest: evidence.verifier_transcript_digest(),
        }
    }
    fn to_exact(
        &self,
    ) -> Result<
        ExactEightChunkMembershipEvidenceV1<PersistentSecretMembershipRoleV1>,
        ZkAmsMkhePersistentMembershipErrorV1,
    > {
        ExactEightChunkMembershipEvidenceV1::from_structural_parts(
            self.context.to_exact()?,
            self.generator_basis_digest,
            self.chunks.clone(),
            self.commitment_set_digest,
            self.proof_set_digest,
            self.verifier_transcript_digest,
        )
        .map_err(Into::into)
    }
    /// Prove and locally verify all eight production-shape bound-one chunks.
    ///
    /// Blindings are borrowed from state and each local per-call copy is
    /// explicitly cleared before error propagation.  A future state-owned
    /// relation prover must preserve the originals separately.  This method
    /// never mints a reusable binding capability.
    pub(super) fn prove<R: ProofRandomSource>(
        context: ZkAmsMkhePersistentMembershipContextV1,
        coefficients: &[i8],
        blindings: &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
        random: &mut R,
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        ExactEightChunkMembershipEvidenceV1::prove(
            context.to_exact()?,
            coefficients,
            blindings,
            random,
        )
        .map(Self::from_exact)
        .map_err(Into::into)
    }
    /// Verify and assemble an exact ordered set of externally supplied chunks.
    pub(super) fn from_proof_chunks_verified(
        context: ZkAmsMkhePersistentMembershipContextV1,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        ExactEightChunkMembershipEvidenceV1::from_proof_chunks_verified(context.to_exact()?, chunks)
            .map(Self::from_exact)
            .map_err(Into::into)
    }
    #[cfg(test)]
    fn assemble(
        context: ZkAmsMkhePersistentMembershipContextV1,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
        transcript_digests: [[u8; 32]; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        ExactEightChunkMembershipEvidenceV1::assemble_for_test(
            context.to_exact()?,
            chunks,
            transcript_digests,
        )
        .map(Self::from_exact)
        .map_err(Into::into)
    }
    /// Strictly decode the exact 12,291-byte canonical evidence layout.
    ///
    /// Decoding validates shape plus the public commitment/proof roots.  The
    /// verifier-transcript root can only be recomputed by executing the eight
    /// proofs, so callers must invoke [`Self::verify`] before trusting it.
    pub(super) fn from_wire_bytes_exact(
        bytes: &[u8],
    ) -> Result<Self, ZkAmsMkhePersistentMembershipErrorV1> {
        ExactEightChunkMembershipEvidenceV1::from_wire_bytes_exact(bytes)
            .map(Self::from_exact)
            .map_err(Into::into)
    }
    /// Encode the fixed-layout canonical representation after rechecking roots.
    pub(super) fn to_wire_bytes(&self) -> Result<Vec<u8>, ZkAmsMkhePersistentMembershipErrorV1> {
        self.to_exact()?.to_wire_bytes().map_err(Into::into)
    }
    /// Replay every production proof and self-recompute all three ordered roots.
    pub(super) fn verify(&self) -> Result<(), ZkAmsMkhePersistentMembershipErrorV1> {
        self.to_exact()?.verify().map_err(Into::into)
    }
    /// Consume this evidence and return a move-only membership-only receipt.
    ///
    /// The receipt has no conversion into active persistent-witness lineage;
    /// only the future complete CPK relation verifier may consume it.
    pub(super) fn into_verified(
        self,
    ) -> Result<ZkAmsMkheVerifiedPersistentMembershipV1, ZkAmsMkhePersistentMembershipErrorV1> {
        self.to_exact()?
            .into_verified()
            .map(|inner| ZkAmsMkheVerifiedPersistentMembershipV1 { inner })
            .map_err(Into::into)
    }
    #[cfg(test)]
    fn verify_with<F>(
        &self,
        mut verify_chunk: F,
    ) -> Result<(), ZkAmsMkhePersistentMembershipErrorV1>
    where
        F: FnMut(
            [u8; 32],
            u16,
            &ZkAmsT256MembershipProofV1,
        ) -> Result<[u8; 32], ZkAmsMkhePersistentMembershipErrorV1>,
    {
        self.to_exact()?
            .verify_with_for_test(|context_digest, ordinal, chunk| {
                verify_chunk(context_digest, ordinal, chunk).map_err(|error| match error {
                    ZkAmsMkhePersistentMembershipErrorV1::Context => {
                        ExactEightChunkMembershipErrorV1::Context
                    }
                    ZkAmsMkhePersistentMembershipErrorV1::Shape => {
                        ExactEightChunkMembershipErrorV1::Shape
                    }
                    ZkAmsMkhePersistentMembershipErrorV1::WireEncoding => {
                        ExactEightChunkMembershipErrorV1::WireEncoding
                    }
                    ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch => {
                        ExactEightChunkMembershipErrorV1::DigestMismatch
                    }
                    ZkAmsMkhePersistentMembershipErrorV1::GeneratorBasis => {
                        ExactEightChunkMembershipErrorV1::GeneratorBasis
                    }
                    ZkAmsMkhePersistentMembershipErrorV1::Membership(error) => {
                        ExactEightChunkMembershipErrorV1::Membership(error)
                    }
                })
            })
            .map_err(Into::into)
    }
    fn validate_structural_digests(&self) -> Result<(), ZkAmsMkhePersistentMembershipErrorV1> {
        self.to_exact().map(|_| ())
    }
    /// Complete source context carried by this evidence.
    pub(super) const fn context(&self) -> ZkAmsMkhePersistentMembershipContextV1 {
        self.context
    }
    /// Ordered production membership chunks.
    pub(super) const fn chunks(
        &self,
    ) -> &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        &self.chunks
    }
    /// Ordered commitment points certified by the eight membership proofs.
    pub(super) fn commitments(&self) -> [Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        core::array::from_fn(|index| self.chunks[index].commitment())
    }
    /// Pinned full T256 generator-basis digest.
    pub(super) const fn generator_basis_digest(&self) -> [u8; 32] {
        self.generator_basis_digest
    }
    /// Stable digest of the eight ordered commitment points.
    ///
    /// This intentionally mirrors the context-independent commitment identity
    /// consumed by the persistent-binding graph.  The proof and verifier-
    /// transcript roots bind that identity to [`Self::context`].
    pub(super) const fn commitment_set_digest(&self) -> [u8; 32] {
        self.commitment_set_digest
    }
    /// Context-bound digest of the eight exact canonical chunk wires.
    pub(super) const fn proof_set_digest(&self) -> [u8; 32] {
        self.proof_set_digest
    }
    /// Context-bound root of the eight verifier transcript digests.
    pub(super) const fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verifier_transcript_digest
    }
}
/// Move-only verification receipt for bound-one persistent membership.
///
/// This receipt proves only coefficient membership and context integrity.  It
/// intentionally has no `From` path to the active exact-binding graph and no
/// relation/admission constructor.
pub(super) struct ZkAmsMkheVerifiedPersistentMembershipV1 {
    inner: VerifiedExactEightChunkMembershipV1<PersistentSecretMembershipRoleV1>,
}
impl ZkAmsMkheVerifiedPersistentMembershipV1 {
    pub(super) fn context(&self) -> ZkAmsMkhePersistentMembershipContextV1 {
        ZkAmsMkhePersistentMembershipContextV1::from_exact(self.inner.context())
    }
    pub(super) const fn commitments(
        &self,
    ) -> &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        self.inner.commitments()
    }
    pub(super) const fn generator_basis_digest(&self) -> [u8; 32] {
        self.inner.generator_basis_digest()
    }
    pub(super) const fn commitment_set_digest(&self) -> [u8; 32] {
        self.inner.commitment_set_digest()
    }
    pub(super) const fn proof_set_digest(&self) -> [u8; 32] {
        self.inner.proof_set_digest()
    }
    pub(super) const fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.inner.verifier_transcript_digest()
    }
}
impl core::fmt::Debug for ZkAmsMkheVerifiedPersistentMembershipV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheVerifiedPersistentMembershipV1")
            .field(
                "commitment_set_digest",
                &hex::encode(self.commitment_set_digest()),
            )
            .field(
                "verifier_transcript_digest",
                &hex::encode(self.verifier_transcript_digest()),
            )
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
fn commitment_set_digest(
    generator_basis_digest: [u8; 32],
    chunks: &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[u8; 32], ZkAmsMkhePersistentMembershipErrorV1> {
    exact_commitment_set_digest::<PersistentSecretMembershipRoleV1>(generator_basis_digest, chunks)
        .map_err(Into::into)
}
#[cfg(test)]
fn proof_set_digest(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    chunks: &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[u8; 32], ZkAmsMkhePersistentMembershipErrorV1> {
    exact_proof_set_digest::<PersistentSecretMembershipRoleV1>(
        context_digest,
        generator_basis_digest,
        chunks,
    )
    .map_err(Into::into)
}
#[cfg(test)]
fn verifier_transcript_set_digest(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    transcript_digests: &[[u8; 32]; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> [u8; 32] {
    exact_verifier_transcript_set_digest::<PersistentSecretMembershipRoleV1>(
        context_digest,
        generator_basis_digest,
        transcript_digests,
    )
}
// TODO: Keep the CPK-relation linkage and `VerifiedPersistentWitnessBindingV1`
// mint closed until the state-owned relation proof consumes these exact
// commitment points together with their retained, zeroized blindings.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        generalized_bulletproof::GeneralizedBulletproofErrorV1,
        vega::{derive_t256_generators_v1, sponge::keccak256},
    };
    const INNER_COMMITMENT_OFFSET_V1: usize = 12;
    const INNER_PROOF_OFFSET_V1: usize = 47;
    const RELEASE_EVIDENCE_KAT_DOMAIN_V1: &[u8] =
        b"iroha.zk-ams.v1.mkhe.persistent-membership.release-evidence-kat";
    const RELEASE_EVIDENCE_KAT_RANDOM_DOMAIN_V1: &[u8] =
        b"iroha.zk-ams.v1.mkhe.persistent-membership.release-evidence-kat.random";
    const RELEASE_EVIDENCE_KAT_PROOF_BYTES_V1: usize =
        ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * BOUND_ONE_PROOF_BYTES_V1;
    const RELEASE_EVIDENCE_KAT_CHUNK_WIRE_BYTES_V1: usize =
        ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * MEMBERSHIP_CHUNK_WIRE_BYTES_V1;
    // Baseline from before padded vector-commitment coordinates were
    // zero-constrained.  A hardened release proof must never reproduce it.
    const PRE_ZERO_TAIL_RELEASE_EVIDENCE_KAT_DIGEST_V1: [u8; 32] = [
        0xa5, 0x18, 0xb2, 0x46, 0xbc, 0xb3, 0x8b, 0xa6, 0x3c, 0xcd, 0x7f, 0x8e, 0x8c, 0xe0, 0x6a,
        0x67, 0x9d, 0x08, 0xa0, 0xca, 0x82, 0x87, 0x86, 0x60, 0x45, 0x22, 0x68, 0xd1, 0x49, 0x41,
        0x20, 0x30,
    ];
    // First canonical release vector after every padded vector-commitment
    // coordinate was constrained to zero.
    const POST_ZERO_TAIL_RELEASE_EVIDENCE_KAT_DIGEST_V1: [u8; 32] = [
        0x7b, 0x38, 0xbf, 0x21, 0x92, 0xfe, 0xd1, 0x3d, 0x3c, 0xfe, 0x61, 0x08, 0x9c, 0xfc, 0xac,
        0x78, 0xec, 0x38, 0x4e, 0x7b, 0x15, 0x18, 0xba, 0x43, 0xb0, 0xdd, 0xe5, 0x89, 0xc1, 0x3a,
        0xfd, 0x07,
    ];
    struct ReleaseKatRandom {
        seed: [u8; 32],
        next_block: u64,
        max_blocks: u64,
    }
    impl ReleaseKatRandom {
        fn new(label: &[u8], max_blocks: u64) -> Self {
            Self {
                seed: keccak256(label),
                next_block: 0,
                max_blocks,
            }
        }
    }
    impl ProofRandomSource for ReleaseKatRandom {
        fn fill_bytes(
            &mut self,
            destination: &mut [u8],
        ) -> Result<(), GeneralizedBulletproofErrorV1> {
            let mut written = 0;
            while written < destination.len() {
                if self.next_block >= self.max_blocks {
                    return Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable);
                }
                let mut frame = [0_u8; 40];
                frame[..32].copy_from_slice(&self.seed);
                frame[32..].copy_from_slice(&self.next_block.to_be_bytes());
                let block = keccak256(&frame);
                self.next_block = self
                    .next_block
                    .checked_add(1)
                    .ok_or(GeneralizedBulletproofErrorV1::RandomnessUnavailable)?;
                let take = (destination.len() - written).min(block.len());
                destination[written..written + take].copy_from_slice(&block[..take]);
                written += take;
            }
            Ok(())
        }
    }
    fn release_kat_coefficients() -> Vec<i8> {
        (0..ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_COEFFICIENTS_V1)
            .map(|index| {
                let chunk = index / ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
                match (index + 2 * chunk) % 3 {
                    0 => -1,
                    1 => 0,
                    2 => 1,
                    _ => unreachable!(),
                }
            })
            .collect()
    }
    fn release_kat_blindings() -> [Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        [
            Scalar::from_u64(0x101),
            Scalar::from_u64(0x211),
            Scalar::from_u64(0x307),
            Scalar::from_u64(0x401),
            Scalar::from_u64(0x503),
            Scalar::from_u64(0x601),
            Scalar::from_u64(0x709),
            Scalar::from_u64(0x809),
        ]
    }
    fn release_evidence_kat_digest(
        context: ZkAmsMkhePersistentMembershipContextV1,
        coefficients: &[i8],
        blindings: &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
        random_seed: [u8; 32],
        evidence: &ZkAmsMkhePersistentMembershipEvidenceV1,
        wire: &[u8],
    ) -> [u8; 32] {
        let mut coefficient_hash = Keccak256::new();
        coefficient_hash.update(RELEASE_EVIDENCE_KAT_DOMAIN_V1);
        coefficient_hash.update(b".coefficients");
        coefficient_hash.update(
            &u32::try_from(coefficients.len())
                .expect("release coefficient count fits u32")
                .to_be_bytes(),
        );
        for coefficient in coefficients {
            coefficient_hash.update(&coefficient.to_be_bytes());
        }
        let mut blinding_hash = Keccak256::new();
        blinding_hash.update(RELEASE_EVIDENCE_KAT_DOMAIN_V1);
        blinding_hash.update(b".blindings");
        blinding_hash.update(&[ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 as u8]);
        for (index, blinding) in blindings.iter().enumerate() {
            blinding_hash.update(
                &u16::try_from(index)
                    .expect("release blinding index fits u16")
                    .to_be_bytes(),
            );
            blinding_hash.update(&blinding.to_be_bytes());
        }
        let mut hash = Keccak256::new();
        hash.update(RELEASE_EVIDENCE_KAT_DOMAIN_V1);
        hash.update(&PERSISTENT_MEMBERSHIP_MAGIC_V1);
        hash.update(&[
            PERSISTENT_MEMBERSHIP_VERSION_V1,
            PERSISTENT_MEMBERSHIP_BOUND_V1 as u8,
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 as u8,
        ]);
        for dimension in [
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_COEFFICIENTS_V1,
            ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1,
            BOUND_ONE_PROOF_BYTES_V1,
            RELEASE_EVIDENCE_KAT_PROOF_BYTES_V1,
            MEMBERSHIP_CHUNK_WIRE_BYTES_V1,
            RELEASE_EVIDENCE_KAT_CHUNK_WIRE_BYTES_V1,
            PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1,
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_WIRE_BYTES_V1,
        ] {
            hash.update(
                &u32::try_from(dimension)
                    .expect("release evidence KAT dimension fits u32")
                    .to_be_bytes(),
            );
        }
        hash.update(&context.context_digest());
        hash.update(&random_seed);
        hash.update(&coefficient_hash.finalize());
        hash.update(&blinding_hash.finalize());
        hash.update(&evidence.generator_basis_digest());
        hash.update(&evidence.commitment_set_digest());
        hash.update(&evidence.proof_set_digest());
        hash.update(&evidence.verifier_transcript_digest());
        hash.update(
            &u32::try_from(wire.len())
                .expect("release evidence wire length fits u32")
                .to_be_bytes(),
        );
        hash.update(wire);
        hash.finalize()
    }
    fn context(seed: &[u8]) -> ZkAmsMkhePersistentMembershipContextV1 {
        let digest = |label: &[u8]| {
            let mut frame = Vec::new();
            frame.extend_from_slice(seed);
            frame.extend_from_slice(label);
            keccak256(&frame)
        };
        ZkAmsMkhePersistentMembershipContextV1::new(
            digest(b"profile"),
            digest(b"roster"),
            digest(b"key-material"),
            7,
            digest(b"cpk-transcript"),
            ZkAmsMkhePartyIdV1::new(digest(b"party")).expect("nonzero party"),
            digest(b"share-statement"),
        )
        .expect("canonical test context")
    }
    fn fake_chunks(
        context: ZkAmsMkhePersistentMembershipContextV1,
        seed: u8,
    ) -> [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        let points = derive_t256_generators_v1(
            b"iroha.zk-ams.v1.mkhe.persistent-membership.test-points",
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 + usize::from(seed),
        )
        .expect("test points");
        let context_digest = context.context_digest();
        core::array::from_fn(|index| {
            let mut proof = vec![seed.wrapping_add(index as u8); BOUND_ONE_PROOF_BYTES_V1];
            proof[..32].copy_from_slice(&context_digest);
            proof[32..34].copy_from_slice(&(index as u16).to_be_bytes());
            let mut wire = Vec::with_capacity(MEMBERSHIP_CHUNK_WIRE_BYTES_V1);
            wire.extend_from_slice(b"ZMBP");
            wire.push(1);
            wire.push(PERSISTENT_MEMBERSHIP_BOUND_V1 as u8);
            wire.extend_from_slice(&(index as u16).to_be_bytes());
            wire.extend_from_slice(
                &u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
                    .expect("fixed count")
                    .to_be_bytes(),
            );
            wire.extend_from_slice(
                &points[usize::from(seed) + index]
                    .to_non_identity_wire_bytes()
                    .expect("test point"),
            );
            wire.extend_from_slice(
                &u16::try_from(proof.len())
                    .expect("fixed proof length")
                    .to_be_bytes(),
            );
            wire.extend_from_slice(&proof);
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wire).expect("synthetic chunk")
        })
    }
    fn fake_verify(
        context_digest: [u8; 32],
        ordinal: u16,
        chunk: &ZkAmsT256MembershipProofV1,
    ) -> Result<[u8; 32], ZkAmsMkhePersistentMembershipErrorV1> {
        if chunk.proof_bytes().get(..32) != Some(context_digest.as_slice())
            || chunk.proof_bytes().get(32..34) != Some(ordinal.to_be_bytes().as_slice())
        {
            return Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch);
        }
        let mut hash = Keccak256::new();
        hash.update(b"iroha.zk-ams.v1.mkhe.persistent-membership.test-transcript");
        hash.update(&context_digest);
        hash.update(&ordinal.to_be_bytes());
        hash.update(&chunk.to_wire_bytes());
        Ok(hash.finalize())
    }
    fn fake_evidence(seed: &[u8], proof_seed: u8) -> ZkAmsMkhePersistentMembershipEvidenceV1 {
        let context = context(seed);
        let chunks = fake_chunks(context, proof_seed);
        let transcripts = core::array::from_fn(|index| {
            fake_verify(context.context_digest(), index as u16, &chunks[index])
                .expect("fake transcript")
        });
        ZkAmsMkhePersistentMembershipEvidenceV1::assemble(context, chunks, transcripts)
            .expect("synthetic evidence")
    }
    fn chunk_wire_range(index: usize) -> core::ops::Range<usize> {
        let start = PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1 + index * MEMBERSHIP_CHUNK_WIRE_BYTES_V1;
        start..start + MEMBERSHIP_CHUNK_WIRE_BYTES_V1
    }
    #[test]
    fn canonical_wire_has_exact_release_shape_and_roundtrips() {
        let evidence = fake_evidence(b"canonical-evidence", 0);
        let wire = evidence.to_wire_bytes().expect("wire");
        assert_eq!(wire.len(), ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_WIRE_BYTES_V1);
        assert_eq!(evidence.chunks().len(), 8);
        assert!(
            evidence
                .chunks()
                .iter()
                .all(|chunk| chunk.proof_bytes().len() == 1_447)
        );
        let decoded =
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&wire).expect("decode");
        assert_eq!(decoded, evidence);
        assert_eq!(decoded.to_wire_bytes().expect("re-encode"), wire);
        assert!(decoded.verify_with(fake_verify).is_ok());
        assert!(!ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CPK_RELATION_LINKED_V1);
    }
    #[test]
    fn every_truncation_and_trailing_bytes_are_rejected_before_parsing() {
        let wire = fake_evidence(b"length-adversary", 0)
            .to_wire_bytes()
            .expect("wire");
        for end in 0..wire.len() {
            assert_eq!(
                ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&wire[..end]),
                Err(ZkAmsMkhePersistentMembershipErrorV1::WireEncoding),
                "truncation at {end} was accepted"
            );
        }
        for trailing_len in [1, 2, 32, 1_494] {
            let mut trailing = wire.clone();
            trailing.resize(wire.len() + trailing_len, 0);
            assert_eq!(
                ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&trailing),
                Err(ZkAmsMkhePersistentMembershipErrorV1::WireEncoding)
            );
        }
    }
    #[test]
    fn reordered_duplicated_and_spliced_chunks_are_rejected() {
        let first = fake_evidence(b"set-first", 0);
        let second = fake_evidence(b"set-second", 8);
        let wire = first.to_wire_bytes().expect("first wire");
        let second_wire = second.to_wire_bytes().expect("second wire");
        let mut reordered = wire.clone();
        let first_chunk = reordered[chunk_wire_range(0)].to_vec();
        let second_chunk = reordered[chunk_wire_range(1)].to_vec();
        reordered[chunk_wire_range(0)].copy_from_slice(&second_chunk);
        reordered[chunk_wire_range(1)].copy_from_slice(&first_chunk);
        assert!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&reordered).is_err()
        );
        let mut duplicated = wire.clone();
        duplicated[chunk_wire_range(1)].copy_from_slice(&first_chunk);
        assert!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&duplicated).is_err()
        );
        let mut spliced = wire.clone();
        spliced[chunk_wire_range(3)].copy_from_slice(&second_wire[chunk_wire_range(3)]);
        assert_eq!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&spliced),
            Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
        );
        let mut context_splice = first.clone();
        context_splice.context = second.context;
        context_splice.commitment_set_digest = commitment_set_digest(
            context_splice.generator_basis_digest,
            &context_splice.chunks,
        )
        .expect("commitment root");
        context_splice.proof_set_digest = proof_set_digest(
            context_splice.context.context_digest(),
            context_splice.generator_basis_digest,
            &context_splice.chunks,
        )
        .expect("proof root");
        assert!(context_splice.verify_with(fake_verify).is_err());
    }
    #[test]
    fn every_context_axis_is_bound_into_structural_and_transcript_roots() {
        let evidence = fake_evidence(b"axis-binding", 0);
        for axis in 0..7 {
            let mut changed = evidence.clone();
            match axis {
                0 => changed.context.profile_digest[0] ^= 1,
                1 => changed.context.roster_digest[0] ^= 1,
                2 => changed.context.key_material_digest[0] ^= 1,
                3 => changed.context.epoch += 1,
                4 => changed.context.cpk_transcript_digest[0] ^= 1,
                5 => {
                    let mut party = changed.context.party.to_bytes();
                    party[0] ^= 1;
                    changed.context.party = ZkAmsMkhePartyIdV1::new(party).expect("party");
                }
                6 => changed.context.share_statement_digest[0] ^= 1,
                _ => unreachable!(),
            }
            assert_eq!(
                changed.validate_structural_digests(),
                Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch),
                "axis {axis} was not structurally bound"
            );
            changed.commitment_set_digest =
                commitment_set_digest(changed.generator_basis_digest, &changed.chunks)
                    .expect("commitment root");
            changed.proof_set_digest = proof_set_digest(
                changed.context.context_digest(),
                changed.generator_basis_digest,
                &changed.chunks,
            )
            .expect("proof root");
            assert!(
                changed.verify_with(fake_verify).is_err(),
                "axis {axis} was not transcript bound"
            );
        }
    }
    #[test]
    fn commitment_proof_and_digest_mutations_are_rejected() {
        let evidence = fake_evidence(b"byte-mutations", 0);
        let wire = evidence.to_wire_bytes().expect("wire");
        let mut commitment = wire.clone();
        let source = PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1
            + MEMBERSHIP_CHUNK_WIRE_BYTES_V1
            + INNER_COMMITMENT_OFFSET_V1;
        let destination = PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1 + INNER_COMMITMENT_OFFSET_V1;
        let replacement = commitment[source..source + 33].to_vec();
        commitment[destination..destination + 33].copy_from_slice(&replacement);
        assert_eq!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&commitment),
            Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
        );
        for index in [0, 31, BOUND_ONE_PROOF_BYTES_V1 - 1] {
            let mut proof = wire.clone();
            proof[PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1 + INNER_PROOF_OFFSET_V1 + index] ^= 1;
            assert_eq!(
                ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&proof),
                Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
            );
        }
        // Even after an attacker recomputes both public structural roots, the
        // retained verifier-transcript root detects a changed commitment or
        // proof before this evidence can pass verification.
        for mutate_commitment in [false, true] {
            let mut changed = evidence.clone();
            let mut chunk_wire = changed.chunks[0].to_wire_bytes();
            if mutate_commitment {
                let replacement = evidence.chunks[1]
                    .commitment()
                    .to_non_identity_wire_bytes()
                    .expect("replacement commitment");
                chunk_wire[INNER_COMMITMENT_OFFSET_V1..INNER_COMMITMENT_OFFSET_V1 + 33]
                    .copy_from_slice(&replacement);
            } else {
                chunk_wire[INNER_PROOF_OFFSET_V1 + 64] ^= 1;
            }
            changed.chunks[0] = ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&chunk_wire)
                .expect("structurally valid changed chunk");
            changed.commitment_set_digest =
                commitment_set_digest(changed.generator_basis_digest, &changed.chunks)
                    .expect("changed commitment root");
            changed.proof_set_digest = proof_set_digest(
                changed.context.context_digest(),
                changed.generator_basis_digest,
                &changed.chunks,
            )
            .expect("changed proof root");
            assert_eq!(
                changed.verify_with(fake_verify),
                Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
            );
        }
        for offset in [
            OFFSET_COMMITMENT_SET_DIGEST_V1,
            OFFSET_PROOF_SET_DIGEST_V1,
            OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1,
        ] {
            let mut digest = wire.clone();
            digest[offset] ^= 1;
            let decoded = ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&digest);
            if offset == OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1 {
                let decoded = decoded.expect("transcript root requires proof replay");
                assert_eq!(
                    decoded.verify_with(fake_verify),
                    Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
                );
            } else {
                assert_eq!(
                    decoded,
                    Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
                );
            }
        }
    }
    #[test]
    fn noncanonical_outer_and_inner_shape_fields_are_rejected() {
        let wire = fake_evidence(b"shape-mutations", 0)
            .to_wire_bytes()
            .expect("wire");
        for (offset, value) in [(4, 2), (OFFSET_BOUND_V1, 2), (OFFSET_CHUNK_COUNT_V1, 7)] {
            let mut changed = wire.clone();
            changed[offset] = value;
            assert!(
                ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&changed).is_err()
            );
        }
        let mut coefficient_count = wire.clone();
        coefficient_count[OFFSET_COEFFICIENT_COUNT_V1..OFFSET_COEFFICIENT_COUNT_V1 + 4]
            .copy_from_slice(&16_383_u32.to_be_bytes());
        assert!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&coefficient_count)
                .is_err()
        );
        let mut basis = wire.clone();
        basis[OFFSET_GENERATOR_BASIS_DIGEST_V1] ^= 1;
        assert_eq!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&basis),
            Err(ZkAmsMkhePersistentMembershipErrorV1::GeneratorBasis)
        );
        let first_chunk = PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1;
        for offset in [5, 6, 7, 8, 9, 10, 11, 45, 46] {
            let mut changed = wire.clone();
            changed[first_chunk + offset] ^= 1;
            assert!(
                ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&changed).is_err(),
                "inner shape byte {offset} was accepted"
            );
        }
    }
    #[test]
    fn zero_context_axes_are_rejected() {
        let valid = context(b"zero-axis");
        for axis in 0..7 {
            let result = ZkAmsMkhePersistentMembershipContextV1::new(
                if axis == 0 {
                    [0; 32]
                } else {
                    valid.profile_digest
                },
                if axis == 1 {
                    [0; 32]
                } else {
                    valid.roster_digest
                },
                if axis == 2 {
                    [0; 32]
                } else {
                    valid.key_material_digest
                },
                if axis == 3 { 0 } else { valid.epoch },
                if axis == 4 {
                    [0; 32]
                } else {
                    valid.cpk_transcript_digest
                },
                if axis == 5 {
                    ZkAmsMkhePartyIdV1([0; 32])
                } else {
                    valid.party
                },
                if axis == 6 {
                    [0; 32]
                } else {
                    valid.share_statement_digest
                },
            );
            assert_eq!(
                result,
                Err(ZkAmsMkhePersistentMembershipErrorV1::Context),
                "zero axis {axis} was accepted"
            );
        }
    }
    #[test]
    #[ignore = "resource smoke: proves one real 16384-coefficient production T256 chunk"]
    fn release_parameter_single_chunk_membership_resource_smoke() {
        let coefficients = release_kat_coefficients();
        let first_chunk = &coefficients[..ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1];
        assert!(first_chunk.contains(&-1));
        assert!(first_chunk.contains(&0));
        assert!(first_chunk.contains(&1));
        let context = context(b"real-release-parameter-membership-evidence-kat");
        let context_digest = context.context_digest();
        let mut blindings = release_kat_blindings();
        let mut random = ReleaseKatRandom::new(RELEASE_EVIDENCE_KAT_RANDOM_DOMAIN_V1, 1_u64 << 20);
        let (proof, prover_transcript_digest) = prove_zk_ams_t256_membership_chunk_v1(
            context_digest,
            0,
            PERSISTENT_MEMBERSHIP_BOUND_V1,
            first_chunk,
            &blindings[0],
            &mut random,
        )
        .expect("one real release-shape membership chunk proves");
        assert_eq!(proof.proof_bytes().len(), BOUND_ONE_PROOF_BYTES_V1);
        assert_eq!(proof.to_wire_bytes().len(), MEMBERSHIP_CHUNK_WIRE_BYTES_V1);
        assert!(!proof.commitment().is_identity());
        let verifier_transcript_digest = verify_zk_ams_t256_membership_chunk_v1(
            context_digest,
            0,
            PERSISTENT_MEMBERSHIP_BOUND_V1,
            &proof,
        )
        .expect("one real release-shape membership chunk verifies");
        assert_eq!(prover_transcript_digest, verifier_transcript_digest);
        for blinding in &mut blindings {
            blinding.clear_secret();
        }
        assert!(blindings.iter().all(|blinding| blinding.is_zero()));
    }
    #[test]
    #[ignore = "real 131072-coefficient production T256 membership KAT; run explicitly"]
    fn release_parameter_eight_chunk_membership_evidence_kat() {
        assert_eq!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1, 8);
        assert_eq!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_COEFFICIENTS_V1, 131_072);
        assert_eq!(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, 16_384);
        assert_eq!(BOUND_ONE_PROOF_BYTES_V1, 1_447);
        assert_eq!(RELEASE_EVIDENCE_KAT_PROOF_BYTES_V1, 11_576);
        assert_eq!(MEMBERSHIP_CHUNK_WIRE_BYTES_V1, 1_494);
        assert_eq!(RELEASE_EVIDENCE_KAT_CHUNK_WIRE_BYTES_V1, 11_952);
        assert_eq!(PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1, 339);
        assert_eq!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_WIRE_BYTES_V1, 12_291);
        let coefficients = release_kat_coefficients();
        assert_eq!(coefficients.len(), 131_072);
        assert!(
            coefficients
                .iter()
                .all(|coefficient| (-1..=1).contains(coefficient))
        );
        for chunk in coefficients.chunks_exact(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1) {
            assert_eq!(chunk.len(), 16_384);
            assert!(chunk.contains(&-1));
            assert!(chunk.contains(&0));
            assert!(chunk.contains(&1));
        }
        let mut blindings = release_kat_blindings();
        assert_eq!(blindings.len(), 8);
        assert!(blindings.iter().all(|blinding| !blinding.is_zero()));
        let mut unavailable = ReleaseKatRandom::new(
            b"iroha.zk-ams.v1.mkhe.persistent-membership.release-kat.unavailable",
            0,
        );
        let mut unavailable_byte = [0_u8; 1];
        assert_eq!(
            unavailable.fill_bytes(&mut unavailable_byte),
            Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
        );
        let context = context(b"real-release-parameter-membership-evidence-kat");
        let random_seed = keccak256(RELEASE_EVIDENCE_KAT_RANDOM_DOMAIN_V1);
        let mut random = ReleaseKatRandom::new(RELEASE_EVIDENCE_KAT_RANDOM_DOMAIN_V1, 1_u64 << 20);
        let evidence = ZkAmsMkhePersistentMembershipEvidenceV1::prove(
            context,
            &coefficients,
            &blindings,
            &mut random,
        )
        .expect("real release-shape membership evidence proves");
        evidence
            .verify()
            .expect("real release-shape membership evidence verifies");
        assert_eq!(evidence.context(), context);
        assert_eq!(evidence.chunks().len(), 8);
        assert!(
            evidence
                .commitments()
                .iter()
                .all(|point| !point.is_identity())
        );
        let proof_lengths: [usize; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] =
            core::array::from_fn(|index| evidence.chunks()[index].proof_bytes().len());
        assert_eq!(proof_lengths, [1_447; 8]);
        assert_eq!(proof_lengths.iter().sum::<usize>(), 11_576);
        let chunk_wire_lengths: [usize; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] =
            core::array::from_fn(|index| evidence.chunks()[index].to_wire_bytes().len());
        assert_eq!(chunk_wire_lengths, [1_494; 8]);
        assert_eq!(chunk_wire_lengths.iter().sum::<usize>(), 11_952);
        let wire = evidence
            .to_wire_bytes()
            .expect("canonical release evidence wire");
        assert_eq!(wire.len(), 12_291);
        let decoded = ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&wire)
            .expect("exact release evidence decode");
        assert_eq!(decoded, evidence);
        assert_eq!(
            decoded.to_wire_bytes().expect("canonical re-encoding"),
            wire
        );
        decoded
            .verify()
            .expect("round-tripped release evidence verifies");
        let mut wrong_context = wire.clone();
        wrong_context[OFFSET_CPK_TRANSCRIPT_DIGEST_V1] ^= 1;
        assert_eq!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&wrong_context),
            Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
        );
        let mut wrong_proof = wire.clone();
        wrong_proof[PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1
            + INNER_PROOF_OFFSET_V1
            + BOUND_ONE_PROOF_BYTES_V1 / 2] ^= 1;
        assert_eq!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&wrong_proof),
            Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
        );
        let mut wrong_commitment = wire.clone();
        let first_commitment = PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1 + INNER_COMMITMENT_OFFSET_V1;
        let second_commitment = PERSISTENT_MEMBERSHIP_HEADER_BYTES_V1
            + MEMBERSHIP_CHUNK_WIRE_BYTES_V1
            + INNER_COMMITMENT_OFFSET_V1;
        let replacement = wrong_commitment[second_commitment..second_commitment + 33].to_vec();
        wrong_commitment[first_commitment..first_commitment + 33].copy_from_slice(&replacement);
        assert_eq!(
            ZkAmsMkhePersistentMembershipEvidenceV1::from_wire_bytes_exact(&wrong_commitment),
            Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)
        );
        let kat_digest = release_evidence_kat_digest(
            context,
            &coefficients,
            &blindings,
            random_seed,
            &evidence,
            &wire,
        );
        for blinding in &mut blindings {
            blinding.clear_secret();
        }
        assert!(blindings.iter().all(|blinding| blinding.is_zero()));
        assert!(!ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CPK_RELATION_LINKED_V1);
        assert_ne!(
            kat_digest, PRE_ZERO_TAIL_RELEASE_EVIDENCE_KAT_DIGEST_V1,
            "hardened membership evidence reproduced the pre-zero-tail digest"
        );
        assert_eq!(
            kat_digest, POST_ZERO_TAIL_RELEASE_EVIDENCE_KAT_DIGEST_V1,
            "post-zero-tail release membership KAT digest changed"
        );
    }
}

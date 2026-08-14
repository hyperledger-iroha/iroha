//! Exact membership and retained-opening bootstrap for direct RKG ephemerals.
//!
//! One party-local `u_i` is committed in eight canonical T256 chunks under a
//! role which is disjoint from persistent CPK secrets and CPK public errors.
//! The public 12,291-byte evidence is bound to the complete direct ceremony
//! context and the same party's already verified secret-lineage identity.
//! Only replay of all eight exact membership proofs can mint the move-only
//! source consumed by the active exact-binding graph.
//!
//! The retained opening owns all 131,072 scalar coordinates and all eight
//! commitment blindings. It never returns either borrow. A purpose-checked
//! closure is the only way a future RKG round can use them, and construction
//! recomputes every commitment before retaining the opening.

use super::{
    MKHE_VERSION_V1, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        VerifiedPersistentWitnessBindingSetV1, VerifiedPersistentWitnessBindingV1,
        mint_rkg_ephemeral_binding_from_verified_membership_v1,
    },
    direct_collective_eval_ceremony::{
        ZkAmsMkheDirectCeremonyContextV1, ZkAmsMkheDirectCeremonyRoundV1,
    },
    exact_eight_chunk_membership::{
        ExactEightChunkMembershipContextV1, ExactEightChunkMembershipErrorV1,
        ExactEightChunkMembershipEvidenceV1, RkgEphemeralMembershipRoleV1,
        VerifiedExactEightChunkMembershipV1, ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1,
        ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1,
        ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1,
    },
};
#[cfg(test)]
use crate::vega::bulletproof_t256::ZkAmsT256MembershipProofV1;
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZeroizingT256ScalarCopyV1,
            ZeroizingT256ScalarVecV1, ZkAmsT256MembershipBoundV1,
            commit_zk_ams_t256_membership_chunk_v1,
        },
        sponge::Keccak256,
    },
};
use thiserror::Error;
const RKG_EPHEMERAL_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-ephemeral-membership.statement";
const RKG_EPHEMERAL_VERIFIED_SOURCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-ephemeral-membership.verified-source";
const _: () = {
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
};
/// Stable wrapper failures before the active binding capability is minted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum ZkAmsMkheDirectRkgEphemeralMembershipErrorV1 {
    /// The governed/direct context is missing, stale, or inconsistent.
    #[error("invalid direct RKG-ephemeral membership context")]
    Context,
    /// The exact role-separated membership engine rejected the evidence.
    #[error(transparent)]
    ExactMembership(#[from] ExactEightChunkMembershipErrorV1),
}
/// Canonical wrapper context for one party-local RKG `u_i` source.
///
/// The exact membership frame carries the common profile/roster/key/epoch,
/// CPK-transcript, and party axes directly. Its statement digest binds the
/// remaining direct-context, ordinal, digit, record, and secret-lineage axes
/// in the fixed order implemented by [`rkg_ephemeral_statement_digest_v1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectRkgEphemeralMembershipContextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    direct_context_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    evaluated_key_ordinal: u8,
    digit_index: u8,
    record_index: u32,
    secret_lineage_identity_digest: [u8; 32],
    statement_digest: [u8; 32],
}
impl ZkAmsMkheDirectRkgEphemeralMembershipContextV1 {
    /// Derive every axis from the governed roster, opaque secret-binding set,
    /// and validated direct relinearization context.
    pub(super) fn from_verified_binding_set(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
        direct_context: &ZkAmsMkheDirectCeremonyContextV1,
        party_index: usize,
        record_index: u32,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        direct_context.validate_rkg_ephemeral_membership_axes(roster, bindings)?;
        let participant = roster
            .participants()
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
        if record_index == 0 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut context = Self {
            profile_digest: direct_context.profile_digest(),
            roster_digest: direct_context.roster_digest(),
            key_material_digest: direct_context.key_material_digest(),
            epoch: direct_context.epoch(),
            cpk_transcript_digest: direct_context.transcript_digest(),
            direct_context_digest: direct_context.digest(),
            party_index: u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party: participant.party(),
            evaluated_key_ordinal: direct_context.evaluated_key_ordinal(),
            digit_index: direct_context.digit_index(),
            record_index,
            secret_lineage_identity_digest: bindings.identity_digests()[party_index],
            statement_digest: [0; 32],
        };
        context.statement_digest = rkg_ephemeral_statement_digest_v1(context);
        context
            .validate()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        Ok(context)
    }
    fn validate(self) -> Result<(), ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        if self.profile_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.cpk_transcript_digest == [0; 32]
            || self.direct_context_digest == [0; 32]
            || self.party.to_bytes() == [0; 32]
            || self.evaluated_key_ordinal != 0
            || self.record_index == 0
            || self.secret_lineage_identity_digest == [0; 32]
            || self.statement_digest == [0; 32]
            || self.statement_digest != rkg_ephemeral_statement_digest_v1(self)
        {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        Ok(())
    }
    fn to_exact(
        self,
    ) -> Result<
        ExactEightChunkMembershipContextV1<RkgEphemeralMembershipRoleV1>,
        ZkAmsMkheDirectRkgEphemeralMembershipErrorV1,
    > {
        self.validate()?;
        ExactEightChunkMembershipContextV1::new(
            self.profile_digest,
            self.roster_digest,
            self.key_material_digest,
            self.epoch,
            self.cpk_transcript_digest,
            self.party,
            self.statement_digest,
        )
        .map_err(Into::into)
    }
    pub(super) const fn statement_digest(self) -> [u8; 32] {
        self.statement_digest
    }
    pub(super) const fn direct_context_digest(self) -> [u8; 32] {
        self.direct_context_digest
    }
    pub(super) const fn party_index(self) -> usize {
        self.party_index as usize
    }
    pub(super) const fn record_index(self) -> u32 {
        self.record_index
    }
}
fn rkg_ephemeral_statement_digest_v1(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RKG_EPHEMERAL_STATEMENT_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&context.profile_digest);
    hash.update(&context.roster_digest);
    hash.update(&context.key_material_digest);
    hash.update(&context.epoch.to_be_bytes());
    hash.update(&context.cpk_transcript_digest);
    hash.update(&context.direct_context_digest);
    hash.update(&[context.party_index]);
    hash.update(&context.party.to_bytes());
    hash.update(&[context.evaluated_key_ordinal]);
    hash.update(&[context.digit_index]);
    hash.update(&context.record_index.to_be_bytes());
    hash.update(&context.secret_lineage_identity_digest);
    hash.finalize()
}
/// Canonical public evidence for one direct RKG-ephemeral opening.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1 {
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    inner: ExactEightChunkMembershipEvidenceV1<RkgEphemeralMembershipRoleV1>,
}
impl ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1 {
    /// Prove and locally verify all eight bound-one chunks.
    pub(super) fn prove<Random: ProofRandomSource>(
        context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        coefficients: &[i8],
        blindings: &[Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        random: &mut Random,
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        context.validate()?;
        let inner = ExactEightChunkMembershipEvidenceV1::prove(
            context.to_exact()?,
            coefficients,
            blindings,
            random,
        )?;
        Ok(Self { context, inner })
    }
    /// Decode exactly 12,291 bytes at a verifier-derived wrapper context.
    ///
    /// There is no context-free decoder: the omitted direct axes are carried
    /// by the expected statement digest and must be known before parsing.
    pub(super) fn from_wire_bytes_exact(
        expected_context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        bytes: &[u8],
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        expected_context.validate()?;
        if bytes.len() != ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1 {
            return Err(ExactEightChunkMembershipErrorV1::WireEncoding.into());
        }
        let inner = ExactEightChunkMembershipEvidenceV1::from_wire_bytes_exact(bytes)?;
        if inner.context() != expected_context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        Ok(Self {
            context: expected_context,
            inner,
        })
    }
    /// Encode the unique role-separated exact-membership wire.
    pub(super) fn to_wire_bytes(
        &self,
    ) -> Result<Vec<u8>, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        self.context.validate()?;
        if self.inner.context() != self.context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        self.inner.to_wire_bytes().map_err(Into::into)
    }
    /// Replay all eight proofs and mint the sole move-only wrapper source.
    pub(super) fn into_verified(
        self,
    ) -> Result<VerifiedRkgEphemeralMembershipSourceV1, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1>
    {
        self.context.validate()?;
        if self.inner.context() != self.context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        let verified = self.inner.into_verified()?;
        VerifiedRkgEphemeralMembershipSourceV1::from_exact_verifier(self.context, verified)
    }
    #[cfg(test)]
    pub(super) fn assemble_for_test(
        context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        transcript_digests: [[u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        let inner = ExactEightChunkMembershipEvidenceV1::assemble_for_test(
            context.to_exact()?,
            chunks,
            transcript_digests,
        )?;
        Ok(Self { context, inner })
    }
    #[cfg(test)]
    pub(super) fn into_verified_with_for_test<F>(
        self,
        verify_chunk: F,
    ) -> Result<VerifiedRkgEphemeralMembershipSourceV1, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1>
    where
        F: FnMut(
            [u8; 32],
            u16,
            &ZkAmsT256MembershipProofV1,
        ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        self.context.validate()?;
        if self.inner.context() != self.context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        let verified = self.inner.into_verified_with_for_test(verify_chunk)?;
        VerifiedRkgEphemeralMembershipSourceV1::from_exact_verifier(self.context, verified)
    }
}
/// Move-only exact-verifier source for one RKG-ephemeral binding.
///
/// This type has no decoder, public constructor, or `Clone` implementation.
/// Its only constructor consumes the move-only result of all eight exact
/// membership verifications.
pub(super) struct VerifiedRkgEphemeralMembershipSourceV1 {
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    verified: VerifiedExactEightChunkMembershipV1<RkgEphemeralMembershipRoleV1>,
    source_verification_digest: [u8; 32],
}
impl VerifiedRkgEphemeralMembershipSourceV1 {
    fn from_exact_verifier(
        context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        verified: VerifiedExactEightChunkMembershipV1<RkgEphemeralMembershipRoleV1>,
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        context.validate()?;
        if verified.context() != context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        let mut source = Self {
            context,
            verified,
            source_verification_digest: [0; 32],
        };
        source.source_verification_digest = verified_source_digest_v1(&source)?;
        source
            .validate_against(context)
            .map_err(|_| ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context)?;
        Ok(source)
    }
    pub(super) fn validate_against(
        &self,
        expected_context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        expected_context
            .validate()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if self.context != expected_context
            || self.verified.context()
                != expected_context
                    .to_exact()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            || self.source_verification_digest == [0; 32]
            || self.source_verification_digest
                != verified_source_digest_v1(self)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    pub(super) const fn generator_basis_digest(&self) -> [u8; 32] {
        self.verified.generator_basis_digest()
    }
    pub(super) const fn commitments(&self) -> &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        self.verified.commitments()
    }
    pub(super) const fn commitment_set_digest(&self) -> [u8; 32] {
        self.verified.commitment_set_digest()
    }
    pub(super) const fn membership_proof_digest(&self) -> [u8; 32] {
        self.verified.proof_set_digest()
    }
    pub(super) const fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verified.verifier_transcript_digest()
    }
    pub(super) const fn source_verification_digest(&self) -> [u8; 32] {
        self.source_verification_digest
    }
}
impl core::fmt::Debug for VerifiedRkgEphemeralMembershipSourceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("VerifiedRkgEphemeralMembershipSourceV1")
            .field("context", &self.context)
            .field(
                "commitment_set_digest",
                &hex::encode(self.commitment_set_digest()),
            )
            .field(
                "source_verification_digest",
                &hex::encode(self.source_verification_digest),
            )
            .finish_non_exhaustive()
    }
}
fn verified_source_digest_v1(
    source: &VerifiedRkgEphemeralMembershipSourceV1,
) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
    source
        .context
        .validate()
        .map_err(|_| ExactEightChunkMembershipErrorV1::Context)?;
    let exact_context = source.context.to_exact().map_err(|error| match error {
        ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::ExactMembership(error) => error,
        ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context => {
            ExactEightChunkMembershipErrorV1::Context
        }
    })?;
    if source.verified.context() != exact_context {
        return Err(ExactEightChunkMembershipErrorV1::Context);
    }
    let mut hash = Keccak256::new();
    hash.update(RKG_EPHEMERAL_VERIFIED_SOURCE_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&source.context.statement_digest());
    hash.update(&exact_context.context_digest());
    hash.update(&source.verified.generator_basis_digest());
    hash.update(&source.verified.commitment_set_digest());
    hash.update(&source.verified.proof_set_digest());
    hash.update(&source.verified.verifier_transcript_digest());
    Ok(hash.finalize())
}
/// Move-only owner of one exact RKG-ephemeral opening and its compact binding.
pub(super) struct RetainedRkgEphemeralOpeningV1 {
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    binding: VerifiedPersistentWitnessBindingV1,
    u: ZeroizingT256ScalarVecV1,
    blindings: [ZeroizingT256ScalarCopyV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
}
impl RetainedRkgEphemeralOpeningV1 {
    /// Verify, bind, and retain one complete opening.
    ///
    /// The returned second value is an explicit fork of compact public
    /// binding metadata only. Neither the 131,072 coordinates nor any
    /// blinding is duplicated by that fork.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_verified_membership(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
        direct_context: &ZkAmsMkheDirectCeremonyContextV1,
        party_index: usize,
        record_index: u32,
        source: VerifiedRkgEphemeralMembershipSourceV1,
        u: ZeroizingT256ScalarVecV1,
        blindings: [ZeroizingT256ScalarCopyV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<(Self, VerifiedPersistentWitnessBindingV1), ZkAmsMkheErrorV1> {
        let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            roster,
            bindings,
            direct_context,
            party_index,
            record_index,
        )?;
        source.validate_against(context)?;
        let binding = mint_rkg_ephemeral_binding_from_verified_membership_v1(
            roster,
            bindings,
            direct_context,
            party_index,
            record_index,
            source,
        )?;
        let (binding, verifier_binding) = binding.fork_for_state_and_verifier_v1();
        bindings.validate_rkg_ephemeral_binding_for_direct_context(
            roster,
            direct_context,
            party_index,
            &binding,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
        )?;
        bindings.validate_rkg_ephemeral_binding_for_direct_context(
            roster,
            direct_context,
            party_index,
            &binding,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
        )?;
        verify_retained_opening_commitments_v1(&binding, &u, &blindings)?;
        Ok((
            Self {
                context,
                binding,
                u,
                blindings,
            },
            verifier_binding,
        ))
    }
    /// Borrow the retained opening only inside one authorized RKG-round call.
    ///
    /// `RkgNormalize` and `Galois` are rejected before the closure runs. The
    /// closure cannot return a borrow tied to these arguments, so neither
    /// secret slice escapes this boundary.
    pub(super) fn with_borrowed_opening_for_round<T, F>(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
        direct_context: &ZkAmsMkheDirectCeremonyContextV1,
        round: ZkAmsMkheDirectCeremonyRoundV1,
        use_opening: F,
    ) -> Result<T, ZkAmsMkheErrorV1>
    where
        F: FnOnce(&[Scalar], [&Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1]) -> T,
    {
        let expected = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            roster,
            bindings,
            direct_context,
            self.context.party_index(),
            self.context.record_index(),
        )?;
        if expected != self.context || expected.direct_context_digest() != direct_context.digest() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        bindings.validate_rkg_ephemeral_binding_for_direct_context(
            roster,
            direct_context,
            self.context.party_index(),
            &self.binding,
            round,
        )?;
        let blindings = core::array::from_fn(|index| self.blindings[index].as_ref());
        verify_retained_opening_commitments_v1(&self.binding, &self.u, &self.blindings)?;
        Ok(use_opening(self.u.as_slice(), blindings))
    }
}
impl core::fmt::Debug for RetainedRkgEphemeralOpeningV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RetainedRkgEphemeralOpeningV1")
            .field("context", &self.context)
            .field("u", &"[REDACTED; 131072]")
            .field("blindings", &"[REDACTED; 8]")
            .finish_non_exhaustive()
    }
}
fn verify_retained_opening_commitments_v1(
    binding: &VerifiedPersistentWitnessBindingV1,
    u: &ZeroizingT256ScalarVecV1,
    blindings: &[ZeroizingT256ScalarCopyV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    if u.len() != ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for (index, chunk) in u
        .as_slice()
        .chunks_exact(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
        .enumerate()
    {
        let coefficients = ZeroizingRkgEphemeralCoefficientChunkV1::from_scalars(chunk)?;
        let commitment = commit_zk_ams_t256_membership_chunk_v1(
            ZkAmsT256MembershipBoundV1::One,
            coefficients.as_slice(),
            blindings[index].as_ref(),
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if binding.commitments()[index] != commitment {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}
struct ZeroizingRkgEphemeralCoefficientChunkV1(Vec<i8>);
impl ZeroizingRkgEphemeralCoefficientChunkV1 {
    fn from_scalars(values: &[Scalar]) -> Result<Self, ZkAmsMkheErrorV1> {
        if values.len() != ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut coefficients = Self(Vec::with_capacity(values.len()));
        for value in values {
            let coefficient = if value == &Scalar::zero() {
                0
            } else if value == &Scalar::one() {
                1
            } else if value == &-Scalar::one() {
                -1
            } else {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            };
            coefficients.0.push(coefficient);
        }
        Ok(coefficients)
    }
    fn as_slice(&self) -> &[i8] {
        &self.0
    }
}
impl Drop for ZeroizingRkgEphemeralCoefficientChunkV1 {
    fn drop(&mut self) {
        let coefficients = core::hint::black_box(&mut self.0);
        coefficients.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *coefficients);
    }
}
#[cfg(test)]
#[path = "direct_rkg_ephemeral_membership_tests.rs"]
mod tests;

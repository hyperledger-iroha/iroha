//! Move-only state owner for one unverified persistent CPK opening.
//!
//! This is material for producing public membership evidence, not authority
//! that the opening satisfies the CPK relation.  Only the complete CPK
//! verifier may mint `VerifiedPersistentWitnessBindingV1`.

use super::super::{
    SecretPolynomial, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active_exact_binding::VerifiedPersistentWitnessBindingV1,
    exact_eight_chunk_membership::ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1,
    manifest::ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
};
use super::{
    PersistentOpeningCommitmentWireV1, PersistentSecretCommitmentBlindingsV1,
    ZeroizingT256MembershipCoefficientsV1, commit_persistent_secret_opening_v1,
    encode_persistent_opening_commitments_v1,
};
use crate::vega::VegaT256PointV1 as Point;

/// Every public axis that identifies the sole state-owned CPK opening.
pub(super) struct PersistentDirectOpeningAxesV1 {
    pub(super) profile_digest: [u8; 32],
    pub(super) security_certificate_digest: [u8; 32],
    pub(super) roster_digest: [u8; 32],
    pub(super) key_material_digest: [u8; 32],
    pub(super) epoch: u64,
    pub(super) cpk_transcript_digest: [u8; 32],
    pub(super) party_index: u8,
    pub(super) party: ZkAmsMkhePartyIdV1,
    pub(super) public_share_digest: [u8; 32],
}

impl PersistentDirectOpeningAxesV1 {
    pub(super) fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if [
            self.profile_digest,
            self.security_certificate_digest,
            self.roster_digest,
            self.key_material_digest,
            self.cpk_transcript_digest,
            self.party.to_bytes(),
            self.public_share_digest,
        ]
        .contains(&[0; 32])
            || self.epoch == 0
            || usize::from(self.party_index) >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}

/// Sole persistent owner of the secret, original blindings, binding slot, and
/// canonical public commitments for one collective-party state.
///
/// This type is intentionally neither cloneable nor serializable.  Its
/// retained point encodings are public comparison material; they confer no
/// verified relation authority.
pub(super) struct PersistentDirectOpeningOwnerV1 {
    pub(super) axes: PersistentDirectOpeningAxesV1,
    pub(super) verified_binding: Option<VerifiedPersistentWitnessBindingV1>,
    pub(super) blindings: PersistentSecretCommitmentBlindingsV1,
    pub(super) secret: SecretPolynomial,
    pub(super) retained_commitment_wire: PersistentOpeningCommitmentWireV1,
}

impl PersistentDirectOpeningOwnerV1 {
    pub(super) fn new_unverified(
        axes: PersistentDirectOpeningAxesV1,
        secret: SecretPolynomial,
        blindings: PersistentSecretCommitmentBlindingsV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        axes.validate()?;
        let coefficients = ZeroizingT256MembershipCoefficientsV1::from_ternary_secret(&secret)?;
        let commitments =
            commit_persistent_secret_opening_v1(coefficients.as_slice(), blindings.as_array())?;
        let retained_commitment_wire = encode_persistent_opening_commitments_v1(&commitments)?;
        Ok(Self {
            axes,
            verified_binding: None,
            blindings,
            secret,
            retained_commitment_wire,
        })
    }
}

/// Post-CPK guard which preserves the state-owned opening through a proof.
pub(super) struct PostCpkPersistentDirectOpeningGuardV1<'a> {
    pub(super) owner: &'a mut PersistentDirectOpeningOwnerV1,
    pub(super) coefficients: ZeroizingT256MembershipCoefficientsV1,
    public_error: &'a SecretPolynomial,
    creation_mask_digit_burn: (&'a u64, u64),
}

impl<'a> PostCpkPersistentDirectOpeningGuardV1<'a> {
    pub(super) fn new(
        owner: &'a mut PersistentDirectOpeningOwnerV1,
        public_error: &'a SecretPolynomial,
        creation_mask_digit_burn: (&'a u64, u64),
        expected_identity: [u8; 32],
        expected_commitments: [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let binding = owner
            .verified_binding
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::ReleaseUnavailable)?;
        if binding.identity_digest() != expected_identity
            || binding.commitments() != &expected_commitments
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let coefficients =
            ZeroizingT256MembershipCoefficientsV1::from_ternary_secret(&owner.secret)?;
        let commitments = commit_persistent_secret_opening_v1(
            coefficients.as_slice(),
            owner.blindings.as_array(),
        )?;
        if commitments != expected_commitments
            || encode_persistent_opening_commitments_v1(&commitments)?
                != owner.retained_commitment_wire
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            owner,
            coefficients,
            public_error,
            creation_mask_digit_burn,
        })
    }

    pub(in crate::vega::zk_ams::mkhe::collective) fn into_compacted_post_seal_v1(
        self,
    ) -> impl Sized + 'a {
        let retained = (self.owner, self.public_error, self.creation_mask_digit_burn);
        drop(self.coefficients);
        retained
    }
}

impl core::fmt::Debug for PersistentDirectOpeningOwnerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PersistentDirectOpeningOwnerV1")
            .field(
                "persistent_secret_binding_verified",
                &self.verified_binding.is_some(),
            )
            .field(
                "retained_public_commitment_count",
                &self.retained_commitment_wire.len(),
            )
            .field("blindings", &"[REDACTED]")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
#[path = "persistent_direct_opening_v1_tests.rs"]
mod tests;

//! Commitment-only economic Sybil resistance for `SoraFS` services.
//!
//! Citizen bonds are deliberately not proof of personhood. They make parallel
//! identities economically costly while keeping the bond serial and
//! authorization material hidden behind commitments.

use crate::asset::AssetDefinitionId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Sole first-release citizen-bond record version.
pub const SORAFS_CITIZEN_BOND_VERSION_V1: u16 = 1;
/// Minimum active citizen-bond population fixed by a membership snapshot.
pub const SORAFS_CITIZEN_BOND_SNAPSHOT_MIN_V1: u64 = 1_024;
/// Domain for citizen-bond snapshot commitments.
pub const SORAFS_CITIZEN_BOND_SNAPSHOT_DOMAIN_V1: &[u8] = b"sorafs.citizen-bond.snapshot.v1";
/// Delayed-exit payload for a commitment-only citizen bond.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsCitizenBondExitPendingV1 {
    /// Finalized height at which exit was requested.
    pub requested_at_height: u64,
    /// First finalized height at which the locked value may be released.
    pub unlock_height: u64,
}

/// Lifecycle of one commitment-only citizen bond.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "state",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum SorafsCitizenBondStateV1 {
    /// Bond participates in the current citizen membership root.
    Active,
    /// Exit was requested and remains locked through `unlock_height`.
    ExitPending(SorafsCitizenBondExitPendingV1),
}

/// Consensus record for one economically backed anonymous citizenship leaf.
///
/// No account identifier, issuer, broker, personhood attribute, or revocation
/// handle is present. The serial commitment is immutable for the bond's whole
/// lifetime. Authorization can rotate by compare-and-set while its revision
/// increases, and the policy root is frozen at admission.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsCitizenBondV1 {
    /// Schema version; must be [`SORAFS_CITIZEN_BOND_VERSION_V1`].
    pub version: u16,
    /// Immutable hidden bond serial commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub serial_commitment: [u8; 32],
    /// Rotatable hidden authorization-key commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub authorization_commitment: [u8; 32],
    /// Monotonic authorization revision, beginning at one.
    pub authorization_revision: u64,
    /// Immutable commitment to the locked economic value.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub locked_value_commitment: [u8; 32],
    /// Asset in which the bond is locked.
    pub bond_asset: AssetDefinitionId,
    /// Public economic cost in the bond asset's atomic units.
    pub bond_atomic_units: u128,
    /// Governance policy root frozen for this bond until exit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub frozen_policy_root: [u8; 32],
    /// Finalized height at which the bond entered the membership tree.
    pub bonded_at_height: u64,
    /// Immutable exit delay selected by the frozen policy.
    pub exit_delay_blocks: u64,
    /// Current bond lifecycle.
    pub state: SorafsCitizenBondStateV1,
}

impl SorafsCitizenBondV1 {
    /// Validate a self-contained citizen-bond record.
    ///
    /// # Errors
    ///
    /// Returns an error for inert or overlapping commitments, zero economic
    /// value, an invalid revision/height/delay, or an inconsistent pending exit.
    pub fn validate(&self) -> Result<(), SorafsCitizenBondErrorV1> {
        if self.version != SORAFS_CITIZEN_BOND_VERSION_V1 {
            return Err(SorafsCitizenBondErrorV1::UnsupportedVersion(self.version));
        }
        let commitments = [
            self.serial_commitment,
            self.authorization_commitment,
            self.locked_value_commitment,
            self.frozen_policy_root,
        ];
        if commitments.iter().any(|value| *value == [0; 32]) {
            return Err(SorafsCitizenBondErrorV1::InertCommitment);
        }
        for (index, commitment) in commitments.iter().enumerate() {
            if commitments[..index].contains(commitment) {
                return Err(SorafsCitizenBondErrorV1::OverlappingCommitments);
            }
        }
        if self.authorization_revision == 0
            || self.bond_atomic_units == 0
            || self.bonded_at_height == 0
            || self.exit_delay_blocks == 0
        {
            return Err(SorafsCitizenBondErrorV1::InvalidScalar);
        }
        if let SorafsCitizenBondStateV1::ExitPending(SorafsCitizenBondExitPendingV1 {
            requested_at_height,
            unlock_height,
        }) = self.state
        {
            if requested_at_height < self.bonded_at_height
                || unlock_height
                    != requested_at_height
                        .checked_add(self.exit_delay_blocks)
                        .ok_or(SorafsCitizenBondErrorV1::HeightOverflow)?
            {
                return Err(SorafsCitizenBondErrorV1::InvalidExitWindow);
            }
        }
        Ok(())
    }

    /// Rotate only the authorization commitment by exact compare-and-set.
    ///
    /// # Errors
    ///
    /// Returns an error unless the bond is active and both the expected
    /// commitment and revision match the current record exactly.
    pub fn rotate_authorization(
        &self,
        expected_authorization_commitment: [u8; 32],
        expected_revision: u64,
        next_authorization_commitment: [u8; 32],
    ) -> Result<Self, SorafsCitizenBondErrorV1> {
        self.validate()?;
        if self.state != SorafsCitizenBondStateV1::Active {
            return Err(SorafsCitizenBondErrorV1::NotActive);
        }
        if expected_authorization_commitment != self.authorization_commitment
            || expected_revision != self.authorization_revision
        {
            return Err(SorafsCitizenBondErrorV1::CompareAndSet);
        }
        if next_authorization_commitment == [0; 32]
            || next_authorization_commitment == self.authorization_commitment
            || next_authorization_commitment == self.serial_commitment
            || next_authorization_commitment == self.locked_value_commitment
            || next_authorization_commitment == self.frozen_policy_root
        {
            return Err(SorafsCitizenBondErrorV1::InvalidNextAuthorization);
        }
        let mut next = self.clone();
        next.authorization_commitment = next_authorization_commitment;
        next.authorization_revision = next
            .authorization_revision
            .checked_add(1)
            .ok_or(SorafsCitizenBondErrorV1::RevisionOverflow)?;
        Ok(next)
    }

    /// Enter the immutable delayed-exit window.
    ///
    /// # Errors
    ///
    /// Returns an error unless the bond is active and height arithmetic is safe.
    pub fn request_exit(&self, finalized_height: u64) -> Result<Self, SorafsCitizenBondErrorV1> {
        self.validate()?;
        if self.state != SorafsCitizenBondStateV1::Active {
            return Err(SorafsCitizenBondErrorV1::NotActive);
        }
        let unlock_height = finalized_height
            .checked_add(self.exit_delay_blocks)
            .ok_or(SorafsCitizenBondErrorV1::HeightOverflow)?;
        let mut next = self.clone();
        next.state = SorafsCitizenBondStateV1::ExitPending(SorafsCitizenBondExitPendingV1 {
            requested_at_height: finalized_height,
            unlock_height,
        });
        next.validate()?;
        Ok(next)
    }
}

/// Citizen-bond validation or transition failure.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SorafsCitizenBondErrorV1 {
    /// Unsupported record version.
    #[error("unsupported SoraFS citizen-bond version {0}")]
    UnsupportedVersion(u16),
    /// One required commitment is all zeroes.
    #[error("citizen-bond commitments must be non-zero")]
    InertCommitment,
    /// Domain-separated commitments were incorrectly reused.
    #[error("citizen-bond commitments must be pairwise distinct")]
    OverlappingCommitments,
    /// A required revision, amount, height, or delay is zero.
    #[error("citizen-bond scalar fields must be non-zero")]
    InvalidScalar,
    /// Pending-exit heights do not match the frozen delay.
    #[error("citizen-bond exit window is inconsistent")]
    InvalidExitWindow,
    /// Only an active bond may rotate or request exit.
    #[error("citizen bond is not active")]
    NotActive,
    /// Compare-and-set input did not match current state.
    #[error("citizen-bond authorization compare-and-set failed")]
    CompareAndSet,
    /// Replacement authorization commitment is inert or aliases existing material.
    #[error("invalid next citizen-bond authorization commitment")]
    InvalidNextAuthorization,
    /// Authorization revision overflowed.
    #[error("citizen-bond authorization revision overflow")]
    RevisionOverflow,
    /// Finalized-height arithmetic overflowed.
    #[error("citizen-bond height overflow")]
    HeightOverflow,
    /// A membership snapshot contains an inert root or height.
    #[error("citizen-bond snapshot is inert")]
    InvalidSnapshot,
    /// A membership snapshot does not meet the anonymity-set minimum.
    #[error("citizen-bond snapshot does not meet the minimum active bond count")]
    SnapshotTooSmall,
}

/// Frozen public membership snapshot used by anonymous candidacy proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsCitizenBondSnapshotV1 {
    /// Frozen governance policy root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub frozen_policy_root: [u8; 32],
    /// Root of active citizen-bond serial commitments.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub active_membership_root: [u8; 32],
    /// Finalized height at which this root was fixed.
    pub finalized_height: u64,
    /// Number of active leaves committed by the root.
    pub active_bond_count: u64,
}

impl SorafsCitizenBondSnapshotV1 {
    /// Validate non-inert roots and the public minimum anonymity set.
    ///
    /// # Errors
    ///
    /// Returns an error for an inert root/height or fewer than 1,024 active bonds.
    pub fn validate(&self) -> Result<(), SorafsCitizenBondErrorV1> {
        if self.frozen_policy_root == [0; 32]
            || self.active_membership_root == [0; 32]
            || self.finalized_height == 0
        {
            return Err(SorafsCitizenBondErrorV1::InvalidSnapshot);
        }
        if self.active_bond_count < SORAFS_CITIZEN_BOND_SNAPSHOT_MIN_V1 {
            return Err(SorafsCitizenBondErrorV1::SnapshotTooSmall);
        }
        Ok(())
    }

    /// Compute the domain-separated frozen snapshot digest.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        hash_parts(
            SORAFS_CITIZEN_BOND_SNAPSHOT_DOMAIN_V1,
            &[
                &self.frozen_policy_root,
                &self.active_membership_root,
                &self.finalized_height.to_le_bytes(),
                &self.active_bond_count.to_le_bytes(),
            ],
        )
    }
}

fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&(parts.len() as u64).to_le_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_rejects_too_few_bonds() {
        let snapshot = SorafsCitizenBondSnapshotV1 {
            frozen_policy_root: [1; 32],
            active_membership_root: [2; 32],
            finalized_height: 1,
            active_bond_count: SORAFS_CITIZEN_BOND_SNAPSHOT_MIN_V1 - 1,
        };
        assert_eq!(
            snapshot.validate(),
            Err(SorafsCitizenBondErrorV1::SnapshotTooSmall)
        );
    }
}

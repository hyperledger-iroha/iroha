//! Commitment-only economic Sybil resistance for anonymous `SoraFS` services.
//!
//! Citizen bonds are deliberately not proof of personhood. They make parallel
//! identities economically costly while keeping the bond serial and
//! authorization material hidden behind commitments. Anonymous service notes
//! reuse the sole Kagemusha commitment/nullifier lifecycle and are the only
//! collateral that an anonymous moderation juror can lose.

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    offline::{KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Sole first-release citizen-bond record version.
pub const SORAFS_CITIZEN_BOND_VERSION_V1: u16 = 1;
/// Sole first-release anonymous service-note policy version.
pub const SORAFS_ANONYMOUS_SERVICE_NOTE_POLICY_VERSION_V1: u16 = 1;
/// Sole first-release anonymous juror-candidacy version.
pub const SORAFS_ANONYMOUS_JUROR_CANDIDACY_VERSION_V1: u16 = 1;
/// Minimum finalized-block age of a service note used for candidacy.
pub const SORAFS_ANONYMOUS_SERVICE_NOTE_MIN_AGE_BLOCKS_V1: u64 = 300;
/// Minimum active citizen-bond population fixed by a candidacy snapshot.
pub const SORAFS_ANONYMOUS_CITIZEN_SET_MIN_V1: u64 = 1_024;
/// Maximum mandatory lattice-to-STARK bridge proof bytes.
pub const SORAFS_ANONYMOUS_CANDIDACY_PROOF_MAX_BYTES_V1: usize = 512 * 1024;
/// Domain for citizen-bond snapshot commitments.
pub const SORAFS_CITIZEN_BOND_SNAPSHOT_DOMAIN_V1: &[u8] = b"sorafs.citizen-bond.snapshot.v1";
/// Domain for anonymous juror action identifiers.
pub const SORAFS_ANONYMOUS_JUROR_ACTION_DOMAIN_V1: &[u8] = b"sorafs.anonymous-juror.action.v1";
/// Domain for mandatory lattice-to-STARK bridge proof digests.
pub const SORAFS_ANONYMOUS_CANDIDACY_PROOF_DOMAIN_V1: &[u8] =
    b"sorafs.anonymous-juror.bridge-proof.v1";
/// Domain for anonymous service-note escrow identifiers.
pub const SORAFS_ANONYMOUS_SERVICE_ESCROW_DOMAIN_V1: &[u8] =
    b"sorafs.anonymous-service-note.escrow.v1";

/// Digest exact mandatory lattice-to-STARK bridge proof bytes.
#[must_use]
pub fn sorafs_anonymous_candidacy_proof_digest_v1(proof: &[u8]) -> [u8; 32] {
    hash_parts(SORAFS_ANONYMOUS_CANDIDACY_PROOF_DOMAIN_V1, &[proof])
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
    ExitPending {
        /// Finalized height at which exit was requested.
        requested_at_height: u64,
        /// First finalized height at which the locked value may be released.
        unlock_height: u64,
    },
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
        if let SorafsCitizenBondStateV1::ExitPending {
            requested_at_height,
            unlock_height,
        } = self.state
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
        next.state = SorafsCitizenBondStateV1::ExitPending {
            requested_at_height: finalized_height,
            unlock_height,
        };
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
    pub fn validate(&self) -> Result<(), SorafsAnonymousCandidacyErrorV1> {
        if self.frozen_policy_root == [0; 32]
            || self.active_membership_root == [0; 32]
            || self.finalized_height == 0
        {
            return Err(SorafsAnonymousCandidacyErrorV1::InvalidCitizenSnapshot);
        }
        if self.active_bond_count < SORAFS_ANONYMOUS_CITIZEN_SET_MIN_V1 {
            return Err(SorafsAnonymousCandidacyErrorV1::CitizenSetTooSmall {
                found: self.active_bond_count,
            });
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

/// Sole fixed-denomination service-note policy used by anonymous applications.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsAnonymousServiceNotePolicyV1 {
    /// Schema version; must be the sole V1 policy version.
    pub version: u16,
    /// Exact network of every acceptable Kagemusha note.
    pub network_id: NetworkId,
    /// Exact service asset.
    pub asset: AssetDefinitionId,
    /// One and only accepted note denomination.
    pub denomination: KagemushaScaledAmountV2,
    /// Public sink receiving a slashed anonymous note.
    pub penalty_sink: AccountId,
    /// Immutable policy root bound by candidacy proofs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub policy_root: [u8; 32],
}

/// Public Kagemusha note metadata retained for anonymous service admission.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsAnonymousServiceNoteV1 {
    /// Kagemusha commitment/nullifier descriptor; no alternate lifecycle exists.
    pub kagemusha_note: KagemushaSpendableNoteDescriptorV2,
    /// Finalized height at which the note entered the commitment tree.
    pub created_at_finalized_height: u64,
}

impl SorafsAnonymousServiceNoteV1 {
    /// Validate the Kagemusha binding and exact fixed denomination.
    ///
    /// # Errors
    ///
    /// Returns an error if the note is inert, belongs to another network/asset,
    /// or differs from the single denomination fixed by policy.
    pub fn validate_against(
        &self,
        policy: &SorafsAnonymousServiceNotePolicyV1,
    ) -> Result<(), SorafsAnonymousCandidacyErrorV1> {
        if policy.version != SORAFS_ANONYMOUS_SERVICE_NOTE_POLICY_VERSION_V1
            || policy.policy_root == [0; 32]
            || policy.denomination.atomic_units == 0
            || self.created_at_finalized_height == 0
            || self.kagemusha_note.validate_public_binding().is_err()
        {
            return Err(SorafsAnonymousCandidacyErrorV1::InvalidServiceNote);
        }
        if self.kagemusha_note.network_id != policy.network_id
            || self.kagemusha_note.asset != policy.asset
            || self.kagemusha_note.amount != policy.denomination
        {
            return Err(SorafsAnonymousCandidacyErrorV1::WrongDenomination);
        }
        Ok(())
    }
}

/// Typed public statement proven by one anonymous moderation candidate.
///
/// The bridge proof must establish membership in `citizen_snapshot`, ownership
/// and unspentness of `service_note`, and equality of every public binding. No
/// `AccountId`, bond serial, authorization key, or credential issuer is part of
/// this statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsAnonymousJurorCandidacyV1 {
    /// Schema version; must be the sole V1 candidacy version.
    pub version: u16,
    /// Digest of the exact moderation case and round.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub case_digest: [u8; 32],
    /// Frozen citizen-bond membership snapshot.
    pub citizen_snapshot: SorafsCitizenBondSnapshotV1,
    /// Call-scoped citizen-membership nullifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub citizen_nullifier: [u8; 32],
    /// Call-scoped candidate identity used by sortition and ballot state.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub juror_tag: [u8; 32],
    /// Ephemeral holder key authorising this case only.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub session_public_key: [u8; 32],
    /// Aged fixed-denomination Kagemusha service note.
    pub service_note: SorafsAnonymousServiceNoteV1,
    /// Commitment-tree root authenticating the service note.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub service_note_root: [u8; 32],
    /// Call-scoped confidential fee tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub fee_tag: [u8; 32],
    /// Last finalized height at which the candidacy is valid.
    pub expiry_finalized_height: u64,
    /// Replay-proof digest of all preceding candidacy fields.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub action_digest: [u8; 32],
    /// Domain-separated digest of the exact bridge proof bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub bridge_proof_digest: [u8; 32],
    /// Exact mandatory lattice-to-STARK bridge proof; never persisted after verification.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub bridge_proof: Vec<u8>,
}

/// Payload-free ledger record retained after bridge-proof verification.
///
/// The proof bytes, bond serial, account identity, and authorization material
/// are intentionally absent. The associated service-note nullifier is retained
/// only as permanent spentness state and cannot be enumerated through a public
/// collection endpoint.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsAnonymousJurorCandidacyRecordV1 {
    /// Exact case and round digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub case_digest: [u8; 32],
    /// Call-scoped anonymous juror tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub juror_tag: [u8; 32],
    /// Ephemeral session public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub session_public_key: [u8; 32],
    /// Call-scoped citizen membership nullifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub citizen_nullifier: [u8; 32],
    /// Reserved Kagemusha note commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub service_note_commitment: [u8; 32],
    /// Reserved Kagemusha note nullifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub service_note_nullifier: [u8; 32],
    /// Confidential settlement fee tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub fee_tag: [u8; 32],
    /// Frozen citizen-bond snapshot digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub citizen_snapshot_digest: [u8; 32],
    /// Replay-proof action digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub action_digest: [u8; 32],
    /// Associated service-note escrow id.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub escrow_id: [u8; 32],
    /// Consensus finalized height at admission.
    pub registered_at_finalized_height: u64,
    /// Exclusive candidacy expiry height.
    pub expiry_finalized_height: u64,
}

impl SorafsAnonymousJurorCandidacyRecordV1 {
    /// Project the payload-free record committed by consensus after verification.
    #[must_use]
    pub fn from_verified(
        candidacy: &SorafsAnonymousJurorCandidacyV1,
        escrow_id: [u8; 32],
        registered_at_finalized_height: u64,
    ) -> Self {
        Self {
            case_digest: candidacy.case_digest,
            juror_tag: candidacy.juror_tag,
            session_public_key: candidacy.session_public_key,
            citizen_nullifier: candidacy.citizen_nullifier,
            service_note_commitment: candidacy.service_note.kagemusha_note.note_commitment,
            service_note_nullifier: candidacy.service_note.kagemusha_note.spend_nullifier,
            fee_tag: candidacy.fee_tag,
            citizen_snapshot_digest: candidacy.citizen_snapshot.digest(),
            action_digest: candidacy.action_digest,
            escrow_id,
            registered_at_finalized_height,
            expiry_finalized_height: candidacy.expiry_finalized_height,
        }
    }
}

/// Chain facts supplied by the shared WSV verifier at candidacy admission.
///
/// This is deliberately not serializable and cannot become a caller-authored
/// assertion on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SorafsAnonymousCandidacyLedgerContextV1 {
    /// Exact case expected at this admission site.
    pub expected_case_digest: [u8; 32],
    /// Current finalized chain height.
    pub current_finalized_height: u64,
    /// Current frozen citizen membership root.
    pub expected_citizen_membership_root: [u8; 32],
    /// Current Kagemusha service-note tree root.
    pub expected_service_note_root: [u8; 32],
    /// Whether the permanent spentness set already contains the note nullifier.
    pub service_note_is_spent: bool,
}

impl SorafsAnonymousJurorCandidacyV1 {
    /// Derive the only accepted action digest for this candidacy.
    #[must_use]
    pub fn expected_action_digest(&self) -> [u8; 32] {
        let snapshot_digest = self.citizen_snapshot.digest();
        hash_parts(
            SORAFS_ANONYMOUS_JUROR_ACTION_DOMAIN_V1,
            &[
                &self.version.to_le_bytes(),
                &self.case_digest,
                &snapshot_digest,
                &self.citizen_nullifier,
                &self.juror_tag,
                &self.session_public_key,
                &self.service_note.kagemusha_note.note_commitment,
                &self.service_note.kagemusha_note.spend_nullifier,
                &self.service_note.created_at_finalized_height.to_le_bytes(),
                &self.service_note_root,
                &self.fee_tag,
                &self.expiry_finalized_height.to_le_bytes(),
            ],
        )
    }

    /// Validate every public binding against consensus-derived chain facts.
    ///
    /// The caller must additionally verify the bridge proof against the exact
    /// statement and atomically reserve the note nullifier.
    ///
    /// # Errors
    ///
    /// Returns an error for substitutions, replay, insufficient anonymity,
    /// an immature/spent/wrong-denomination note, or malformed bridge proof.
    pub fn validate_against(
        &self,
        policy: &SorafsAnonymousServiceNotePolicyV1,
        context: SorafsAnonymousCandidacyLedgerContextV1,
    ) -> Result<(), SorafsAnonymousCandidacyErrorV1> {
        if self.version != SORAFS_ANONYMOUS_JUROR_CANDIDACY_VERSION_V1 {
            return Err(SorafsAnonymousCandidacyErrorV1::UnsupportedVersion(
                self.version,
            ));
        }
        self.citizen_snapshot.validate()?;
        self.service_note.validate_against(policy)?;
        if self.case_digest != context.expected_case_digest
            || self.citizen_snapshot.active_membership_root
                != context.expected_citizen_membership_root
            || self.citizen_snapshot.frozen_policy_root != policy.policy_root
            || self.service_note_root != context.expected_service_note_root
        {
            return Err(SorafsAnonymousCandidacyErrorV1::ChainSubstitution);
        }
        if context.service_note_is_spent {
            return Err(SorafsAnonymousCandidacyErrorV1::ServiceNoteSpent);
        }
        let note_age = context
            .current_finalized_height
            .checked_sub(self.service_note.created_at_finalized_height)
            .ok_or(SorafsAnonymousCandidacyErrorV1::ServiceNoteFromFuture)?;
        if note_age < SORAFS_ANONYMOUS_SERVICE_NOTE_MIN_AGE_BLOCKS_V1 {
            return Err(SorafsAnonymousCandidacyErrorV1::ServiceNoteTooYoung { found: note_age });
        }
        if self.expiry_finalized_height <= context.current_finalized_height {
            return Err(SorafsAnonymousCandidacyErrorV1::Expired);
        }
        for value in [
            self.case_digest,
            self.citizen_nullifier,
            self.juror_tag,
            self.session_public_key,
            self.service_note_root,
            self.fee_tag,
            self.action_digest,
            self.bridge_proof_digest,
        ] {
            if value == [0; 32] {
                return Err(SorafsAnonymousCandidacyErrorV1::InertBinding);
            }
        }
        if self.action_digest != self.expected_action_digest() {
            return Err(SorafsAnonymousCandidacyErrorV1::ActionDigest);
        }
        if self.bridge_proof.is_empty()
            || self.bridge_proof.len() > SORAFS_ANONYMOUS_CANDIDACY_PROOF_MAX_BYTES_V1
            || self.bridge_proof_digest
                != sorafs_anonymous_candidacy_proof_digest_v1(&self.bridge_proof)
        {
            return Err(SorafsAnonymousCandidacyErrorV1::BridgeProof);
        }
        Ok(())
    }
}

/// Anonymous candidacy validation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SorafsAnonymousCandidacyErrorV1 {
    /// Unsupported candidacy version.
    #[error("unsupported anonymous SoraFS juror candidacy version {0}")]
    UnsupportedVersion(u16),
    /// Citizen snapshot has inert roots or height.
    #[error("invalid frozen citizen-bond snapshot")]
    InvalidCitizenSnapshot,
    /// Snapshot has fewer than the required 1,024 active bonds.
    #[error("anonymous citizen set is too small: found {found}, require at least 1024")]
    CitizenSetTooSmall {
        /// Active bond count fixed by the root.
        found: u64,
    },
    /// Service note or policy is structurally invalid.
    #[error("invalid anonymous Kagemusha service note")]
    InvalidServiceNote,
    /// Note does not match the sole fixed denomination.
    #[error("anonymous service note has the wrong denomination")]
    WrongDenomination,
    /// A chain-, case-, policy-, or root-binding substitution was attempted.
    #[error("anonymous candidacy chain binding mismatch")]
    ChainSubstitution,
    /// Permanent spentness state already contains the service-note nullifier.
    #[error("anonymous service note is already spent or reserved")]
    ServiceNoteSpent,
    /// Note creation height is later than the current finalized height.
    #[error("anonymous service note creation height is in the future")]
    ServiceNoteFromFuture,
    /// Service note is younger than 300 finalized blocks.
    #[error("anonymous service note is too young: {found} finalized blocks")]
    ServiceNoteTooYoung {
        /// Observed finalized-block age.
        found: u64,
    },
    /// Candidacy has expired.
    #[error("anonymous candidacy has expired")]
    Expired,
    /// A required public binding is inert.
    #[error("anonymous candidacy contains an inert public binding")]
    InertBinding,
    /// Replay-proof action digest does not match the statement.
    #[error("anonymous candidacy action digest mismatch")]
    ActionDigest,
    /// Bridge proof is empty, oversized, or digest-mismatched.
    #[error("invalid anonymous candidacy lattice-to-STARK bridge proof")]
    BridgeProof,
}

/// Sole anonymous service-note escrow lifecycle.
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
pub enum SorafsAnonymousServiceEscrowStateV1 {
    /// Note nullifier is reserved while the juror obligation is live.
    Reserved,
    /// Obligation completed; a fresh refund-note commitment was emitted.
    Refunded {
        /// Fresh Kagemusha output commitment replacing the reserved note.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        refund_note_commitment: [u8; 32],
        /// Finalized height of the refund.
        finalized_height: u64,
    },
    /// Governance adjudication transferred only the note value to the penalty sink.
    Slashed {
        /// Signed misconduct evidence digest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        evidence_digest: [u8; 32],
        /// Governance adjudication digest authorising the slash.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        adjudication_digest: [u8; 32],
        /// Finalized height of the slash.
        finalized_height: u64,
    },
}

/// Identity-free escrow record for one reserved anonymous service note.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct SorafsAnonymousServiceEscrowV1 {
    /// Deterministic reservation identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub escrow_id: [u8; 32],
    /// Exact case bound by the candidacy.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub case_digest: [u8; 32],
    /// Replay-proof candidacy action.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub action_digest: [u8; 32],
    /// Anonymous call-scoped juror tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub juror_tag: [u8; 32],
    /// Reserved Kagemusha note commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub note_commitment: [u8; 32],
    /// Permanently consumed/reserved Kagemusha note nullifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub note_nullifier: [u8; 32],
    /// Confidential settlement fee tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub fee_tag: [u8; 32],
    /// Public penalty sink fixed by service-note policy.
    pub penalty_sink: AccountId,
    /// Finalized height of atomic reservation.
    pub reserved_at_height: u64,
    /// Current terminal lifecycle state.
    pub state: SorafsAnonymousServiceEscrowStateV1,
}

impl SorafsAnonymousServiceEscrowV1 {
    /// Construct the sole reserved escrow from an admitted candidacy.
    #[must_use]
    pub fn reserve(
        candidacy: &SorafsAnonymousJurorCandidacyV1,
        policy: &SorafsAnonymousServiceNotePolicyV1,
        reserved_at_height: u64,
    ) -> Self {
        let escrow_id = hash_parts(
            SORAFS_ANONYMOUS_SERVICE_ESCROW_DOMAIN_V1,
            &[
                &candidacy.case_digest,
                &candidacy.action_digest,
                &candidacy.service_note.kagemusha_note.note_commitment,
                &candidacy.service_note.kagemusha_note.spend_nullifier,
            ],
        );
        Self {
            escrow_id,
            case_digest: candidacy.case_digest,
            action_digest: candidacy.action_digest,
            juror_tag: candidacy.juror_tag,
            note_commitment: candidacy.service_note.kagemusha_note.note_commitment,
            note_nullifier: candidacy.service_note.kagemusha_note.spend_nullifier,
            fee_tag: candidacy.fee_tag,
            penalty_sink: policy.penalty_sink.clone(),
            reserved_at_height,
            state: SorafsAnonymousServiceEscrowStateV1::Reserved,
        }
    }

    /// Transition `Reserved` to the only successful terminal state.
    ///
    /// # Errors
    ///
    /// Returns an error for an inert replacement note or any second transition.
    pub fn refund(
        &self,
        refund_note_commitment: [u8; 32],
        finalized_height: u64,
    ) -> Result<Self, SorafsAnonymousEscrowErrorV1> {
        self.require_reserved(finalized_height)?;
        if refund_note_commitment == [0; 32]
            || refund_note_commitment == self.note_commitment
            || refund_note_commitment == self.note_nullifier
        {
            return Err(SorafsAnonymousEscrowErrorV1::InvalidTerminalBinding);
        }
        let mut next = self.clone();
        next.state = SorafsAnonymousServiceEscrowStateV1::Refunded {
            refund_note_commitment,
            finalized_height,
        };
        Ok(next)
    }

    /// Transition `Reserved` to a governance-adjudicated note-only slash.
    ///
    /// Packet loss, health failure, or identity opening cannot be represented by
    /// this transition. Core must verify signed misconduct evidence and the
    /// adjudication before transferring the note to `penalty_sink`.
    ///
    /// # Errors
    ///
    /// Returns an error for inert evidence/adjudication or a second transition.
    pub fn slash(
        &self,
        evidence_digest: [u8; 32],
        adjudication_digest: [u8; 32],
        finalized_height: u64,
    ) -> Result<Self, SorafsAnonymousEscrowErrorV1> {
        self.require_reserved(finalized_height)?;
        if evidence_digest == [0; 32]
            || adjudication_digest == [0; 32]
            || evidence_digest == adjudication_digest
        {
            return Err(SorafsAnonymousEscrowErrorV1::InvalidTerminalBinding);
        }
        let mut next = self.clone();
        next.state = SorafsAnonymousServiceEscrowStateV1::Slashed {
            evidence_digest,
            adjudication_digest,
            finalized_height,
        };
        Ok(next)
    }

    fn require_reserved(&self, finalized_height: u64) -> Result<(), SorafsAnonymousEscrowErrorV1> {
        if self.state != SorafsAnonymousServiceEscrowStateV1::Reserved {
            return Err(SorafsAnonymousEscrowErrorV1::AlreadyTerminal);
        }
        if finalized_height < self.reserved_at_height {
            return Err(SorafsAnonymousEscrowErrorV1::InvalidFinalizedHeight);
        }
        Ok(())
    }
}

/// Anonymous service-note escrow transition failure.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SorafsAnonymousEscrowErrorV1 {
    /// Terminal state cannot transition again.
    #[error("anonymous service-note escrow is already terminal")]
    AlreadyTerminal,
    /// Terminal transition predates reservation.
    #[error("anonymous service-note escrow terminal height predates reservation")]
    InvalidFinalizedHeight,
    /// Refund or slash binding is inert or ambiguous.
    #[error("invalid anonymous service-note escrow terminal binding")]
    InvalidTerminalBinding,
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
    use crate::{
        account::AccountId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::DomainId,
        offline::{KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("sorafs", "universal").expect("domain id"),
            "service".parse().expect("asset name"),
        )
    }

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"sorafs-anonymous-service-test-network",
        )))
    }

    fn policy() -> SorafsAnonymousServiceNotePolicyV1 {
        SorafsAnonymousServiceNotePolicyV1 {
            version: 1,
            network_id: network(),
            asset: asset(),
            denomination: KagemushaScaledAmountV2 {
                atomic_units: 10_000,
                scale: 2,
            },
            penalty_sink: account(9),
            policy_root: [0x44; 32],
        }
    }

    fn candidacy(current_height: u64) -> SorafsAnonymousJurorCandidacyV1 {
        let policy = policy();
        let proof = vec![0xA5; 128];
        let mut value = SorafsAnonymousJurorCandidacyV1 {
            version: 1,
            case_digest: [0x10; 32],
            citizen_snapshot: SorafsCitizenBondSnapshotV1 {
                frozen_policy_root: policy.policy_root,
                active_membership_root: [0x20; 32],
                finalized_height: current_height - 10,
                active_bond_count: 1_024,
            },
            citizen_nullifier: [0x21; 32],
            juror_tag: [0x22; 32],
            session_public_key: [0x23; 32],
            service_note: SorafsAnonymousServiceNoteV1 {
                kagemusha_note: KagemushaSpendableNoteDescriptorV2 {
                    network_id: policy.network_id.clone(),
                    asset: policy.asset.clone(),
                    note_commitment: [0x30; 32],
                    spend_nullifier: [0x31; 32],
                    amount: policy.denomination,
                },
                created_at_finalized_height: current_height - 300,
            },
            service_note_root: [0x32; 32],
            fee_tag: [0x33; 32],
            expiry_finalized_height: current_height + 20,
            action_digest: [0; 32],
            bridge_proof_digest: sorafs_anonymous_candidacy_proof_digest_v1(&proof),
            bridge_proof: proof,
        };
        value.action_digest = value.expected_action_digest();
        value
    }

    fn context(current_height: u64) -> SorafsAnonymousCandidacyLedgerContextV1 {
        SorafsAnonymousCandidacyLedgerContextV1 {
            expected_case_digest: [0x10; 32],
            current_finalized_height: current_height,
            expected_citizen_membership_root: [0x20; 32],
            expected_service_note_root: [0x32; 32],
            service_note_is_spent: false,
        }
    }

    #[test]
    fn authorization_rotation_preserves_immutable_bond_fields() {
        let bond = SorafsCitizenBondV1 {
            version: 1,
            serial_commitment: [1; 32],
            authorization_commitment: [2; 32],
            authorization_revision: 1,
            locked_value_commitment: [3; 32],
            bond_asset: asset(),
            bond_atomic_units: 1_000,
            frozen_policy_root: [4; 32],
            bonded_at_height: 100,
            exit_delay_blocks: 300,
            state: SorafsCitizenBondStateV1::Active,
        };
        let rotated = bond
            .rotate_authorization([2; 32], 1, [5; 32])
            .expect("rotation");
        assert_eq!(rotated.serial_commitment, bond.serial_commitment);
        assert_eq!(rotated.frozen_policy_root, bond.frozen_policy_root);
        assert_eq!(rotated.authorization_revision, 2);
        assert_eq!(rotated.authorization_commitment, [5; 32]);
    }

    #[test]
    fn exit_delay_is_frozen_and_exact() {
        let bond = SorafsCitizenBondV1 {
            version: 1,
            serial_commitment: [1; 32],
            authorization_commitment: [2; 32],
            authorization_revision: 1,
            locked_value_commitment: [3; 32],
            bond_asset: asset(),
            bond_atomic_units: 1_000,
            frozen_policy_root: [4; 32],
            bonded_at_height: 100,
            exit_delay_blocks: 300,
            state: SorafsCitizenBondStateV1::Active,
        };
        assert_eq!(
            bond.request_exit(500).expect("exit").state,
            SorafsCitizenBondStateV1::ExitPending {
                requested_at_height: 500,
                unlock_height: 800,
            }
        );
    }

    #[test]
    fn candidacy_requires_age_anonymity_and_unspent_note() {
        let height = 10_000;
        let value = candidacy(height);
        value
            .validate_against(&policy(), context(height))
            .expect("valid candidacy");

        let mut young = value.clone();
        young.service_note.created_at_finalized_height = height - 299;
        young.action_digest = young.expected_action_digest();
        assert_eq!(
            young.validate_against(&policy(), context(height)),
            Err(SorafsAnonymousCandidacyErrorV1::ServiceNoteTooYoung { found: 299 })
        );

        let mut small = value.clone();
        small.citizen_snapshot.active_bond_count = 1_023;
        small.action_digest = small.expected_action_digest();
        assert_eq!(
            small.validate_against(&policy(), context(height)),
            Err(SorafsAnonymousCandidacyErrorV1::CitizenSetTooSmall { found: 1_023 })
        );

        let mut spent = context(height);
        spent.service_note_is_spent = true;
        assert_eq!(
            value.validate_against(&policy(), spent),
            Err(SorafsAnonymousCandidacyErrorV1::ServiceNoteSpent)
        );
    }

    #[test]
    fn candidacy_substitution_breaks_action_digest() {
        let height = 10_000;
        let mut value = candidacy(height);
        value.session_public_key[0] ^= 1;
        assert_eq!(
            value.validate_against(&policy(), context(height)),
            Err(SorafsAnonymousCandidacyErrorV1::ActionDigest)
        );
    }

    #[test]
    fn escrow_has_only_refund_or_adjudicated_note_slash() {
        let height = 10_000;
        let value = candidacy(height);
        let reserved = SorafsAnonymousServiceEscrowV1::reserve(&value, &policy(), height);
        let refunded = reserved.refund([0x61; 32], height + 1).expect("refund");
        assert_eq!(
            refunded.slash([0x62; 32], [0x63; 32], height + 2),
            Err(SorafsAnonymousEscrowErrorV1::AlreadyTerminal)
        );

        let slashed = reserved
            .slash([0x62; 32], [0x63; 32], height + 1)
            .expect("slash");
        assert_eq!(slashed.penalty_sink, policy().penalty_sink);
        assert_eq!(
            slashed.refund([0x64; 32], height + 2),
            Err(SorafsAnonymousEscrowErrorV1::AlreadyTerminal)
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_rejects_retired_issuer_and_revocation_fields() {
        let value = candidacy(10_000);
        let json = norito::json::to_json(&value).expect("json");
        let decoded: SorafsAnonymousJurorCandidacyV1 =
            norito::json::from_str(&json).expect("roundtrip");
        assert_eq!(decoded, value);

        let old = format!(
            "{{\"issuer_policy_digest\":\"{}\",\"revocation_root\":\"{}\"}}",
            "00".repeat(32),
            "00".repeat(32)
        );
        assert!(norito::json::from_str::<SorafsAnonymousJurorCandidacyV1>(&old).is_err());
    }
}

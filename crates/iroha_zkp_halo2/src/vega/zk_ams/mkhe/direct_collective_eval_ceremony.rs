//! Fail-closed direct-collective evaluated-key ceremony.
//!
//! The evaluated-key generator must never collect the eight RLWE secrets in
//! one process.  This module pins the replacement algebra, transcript identity,
//! canonical limb-stream admission, contribution ordering, and deterministic
//! noise/resource bounds.  It deliberately does **not** accept a release
//! contribution yet: the existing sparse-challenge linear proof has no pinned
//! exact extractor and no reduction for the structured module-SIS instances
//! used here.  Only an independently reviewed proof adapter may construct the
//! verified contribution type consumed by the coordinator.
//!
//! For one gadget digit `g^d`, parties publish
//! `H0_i = -a*u_i + g^d*s_i + p*e0_i` and
//! `H1_i = a*s_i + p*e1_i`.  With `Hj = sum_i Hj_i`, party `i` publishes
//! `K_i = H0*s_i + H1*(u_i-s_i) + p*e2_i`.  Therefore
//! `(sum_i K_i) + H1*S = g^d*S^2 + p*(E0*S + E1*U + E2)`.
//! To retain the compact seeded-final-`a` key format, a third round derives
//! `A` from the context and publishes `N_i=(H1-A)*s_i+p*e3_i`. Thus the final
//! key `(B=K+sum_i N_i,A)` decrypts to the same target plus `p*E3`.
//! A Galois contribution is
//! `b_i = -a_d*s_i + g^d*sigma_k(s_i) + p*e_i`, hence
//! `sum_i b_i + a_d*S = g^d*sigma_k(S) + p*sum_i e_i`.

use super::{
    ArtifactAuthentication, BgvProfile, MKHE_VERSION_V1, MaskedRelaxedRandomSourceV1,
    ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::{ZkAmsMkheActivePartySecretV1, ZkAmsMkheGovernedActiveRosterV1},
    active_exact_binding::{
        PersistentDirectRelationV1, PersistentWitnessConsumerV1,
        VerifiedDirectRelationProofReceiptV1, VerifiedPersistentWitnessBindingSetV1,
    },
    manifest::{
        ZK_AMS_MKHE_CIPHERTEXT_MODULUS_BITS_V1, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        ZK_AMS_MKHE_SPARSE_MAP_FAN_IN_CEILING_V1, ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1,
        release_profile_v1,
    },
    packing::{
        ZK_AMS_T256_GALOIS_KEY_COUNT_V1, validate_zk_ams_t256_galois_key_schedule_v1,
        zk_ams_t256_galois_key_schedule_v1,
    },
    phase23_max_composed_rotation_key_switch_count,
    wire::derive_wire_length_certificate_v1,
};
use crate::vega::sponge::{Keccak256, keccak256};

const DIRECT_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-eval-context";
const DIRECT_COMMON_A_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-rkg-common-a";
const DIRECT_TARGET_A_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-galois-target-a";
const DIRECT_FINAL_A_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-rkg-final-a";
const DIRECT_INITIAL_ROUND_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-initial-round";
const DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-polynomial-stream";
const DIRECT_CONTRIBUTION_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-contribution";
const DIRECT_RELATION_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-relation-statement";
const DIRECT_CONTRIBUTION_AUTH_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-contribution-auth";
const DIRECT_ORDERED_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-ordered-set";
const DIRECT_ORDERED_EVIDENCE_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-ordered-evidence-set";
const DIRECT_ADMITTED_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-admitted-set";
const DIRECT_ADMISSION_HISTORY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-admission-history";
const DIRECT_AGGREGATE_ROUND_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-aggregate-round";
const DIRECT_COMPLETION_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-completion";
const DIRECT_PROOF_AUDIT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-proof-audit";
const DIRECT_RESOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-resource";
const DIRECT_NOISE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-collective-noise";
const DIRECT_EVALUATED_KEY_SET_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-evaluated-key-set-admission";
const DIRECT_NOISE_INTEGRATION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-noise-integration";

const ACTIVE_SPARSE_CHALLENGE_WEIGHT_V1: u8 = 60;
const RESIDUE_BYTES_V1: usize = core::mem::size_of::<u64>();

/// Exact evaluated-key target of one direct-collective ceremony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheDirectEvaluatedKeyTargetV1 {
    /// One digit of the collective relinearization key.
    Relinearization,
    /// One digit of the frozen Galois-key schedule entry.
    Galois {
        /// Zero-based position in the frozen 31-entry schedule.
        schedule_index: u8,
    },
}

impl ZkAmsMkheDirectEvaluatedKeyTargetV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Relinearization => 1,
            Self::Galois { .. } => 2,
        }
    }
}

/// Canonical contribution round in the direct ceremony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheDirectCeremonyRoundV1 {
    /// Jointly proved `H0_i,H1_i` publication.
    RkgRoundOne = 1,
    /// Jointly proved `K_i` publication bound to the complete first round.
    RkgRoundTwo = 2,
    /// Public-`A` normalization `N_i=(H1-A)*s_i+p*e3_i`.
    RkgNormalize = 3,
    /// Automorphism-linked `b_i` publication.
    Galois = 4,
}

impl ZkAmsMkheDirectCeremonyRoundV1 {
    const fn tag(self) -> u8 {
        self as u8
    }
}

/// Polynomial role carried by one canonical limb stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheDirectPolynomialRoleV1 {
    /// Party-local first-round `H0_i`.
    RkgH0 = 1,
    /// Party-local first-round `H1_i`.
    RkgH1 = 2,
    /// Party-local second-round `K_i`.
    RkgK = 3,
    /// Party-local Galois-key constant component `b_i`.
    GaloisB = 4,
    /// Party-local public-`A` normalization component `N_i`.
    RkgNormalization = 5,
}

impl ZkAmsMkheDirectPolynomialRoleV1 {
    const fn tag(self) -> u8 {
        self as u8
    }

    const fn belongs_to(self, round: ZkAmsMkheDirectCeremonyRoundV1) -> bool {
        matches!(
            (self, round),
            (
                Self::RkgH0 | Self::RkgH1,
                ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne
            ) | (Self::RkgK, ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo)
                | (
                    Self::RkgNormalization,
                    ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize
                )
                | (Self::GaloisB, ZkAmsMkheDirectCeremonyRoundV1::Galois)
        )
    }
}

/// Complete immutable identity of one evaluated-key digit ceremony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectCeremonyContextV1 {
    version: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    secret_lineage_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    secret_lineage_root: [u8; 32],
    target: ZkAmsMkheDirectEvaluatedKeyTargetV1,
    evaluated_key_ordinal: u8,
    digit_index: u8,
    galois_exponent: u32,
    binding_digest: [u8; 32],
    common_a_seed: [u8; 32],
    target_a_seed: [u8; 32],
    final_a_seed: [u8; 32],
    initial_round_digest: [u8; 32],
    context_digest: [u8; 32],
}

impl ZkAmsMkheDirectCeremonyContextV1 {
    /// Build one release-profile context without accepting any contribution.
    ///
    /// This raw-digest constructor exists only for adversarial unit fixtures: a
    /// digest does not prove persistent-witness membership.  Production uses
    /// `from_verified_binding_set`, which consumes the opaque ordered set whose
    /// secret role mask covers CPK, both RKG rounds, normalization, every
    /// Galois key, and decryption.
    #[cfg(test)]
    pub(super) fn new(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        collective_public_key_digest: [u8; 32],
        secret_lineage_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        target: ZkAmsMkheDirectEvaluatedKeyTargetV1,
        digit_index: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        let profile = release_profile_v1();
        profile.validate()?;
        if transcript_digest == [0; 32]
            || collective_public_key_digest == [0; 32]
            || secret_lineage_digests.contains(&[0; 32])
            || digit_index >= profile.gadget_digits
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let (evaluated_key_ordinal, galois_exponent) = match target {
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => (0, 0),
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index } => {
                let schedule = zk_ams_t256_galois_key_schedule_v1()?;
                validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
                let index = usize::from(schedule_index);
                if index >= ZK_AMS_T256_GALOIS_KEY_COUNT_V1 {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let exponent = schedule
                    .entries
                    .get(index)
                    .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .exponent;
                (
                    schedule_index
                        .checked_add(1)
                        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    exponent,
                )
            }
        };
        let digit_index =
            u8::try_from(digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let secret_lineage_root = direct_secret_lineage_root(roster, &secret_lineage_digests)?;
        let binding_digest = direct_context_binding_digest(
            roster.profile_digest(),
            roster.roster_digest(),
            roster.key_material_digest(),
            roster.epoch(),
            transcript_digest,
            collective_public_key_digest,
            secret_lineage_root,
            target,
            evaluated_key_ordinal,
            digit_index,
            galois_exponent,
        );
        let common_a_seed = derive_a_seed(DIRECT_COMMON_A_DOMAIN_V1, binding_digest);
        let target_a_seed = derive_a_seed(DIRECT_TARGET_A_DOMAIN_V1, binding_digest);
        let final_a_seed = derive_a_seed(DIRECT_FINAL_A_DOMAIN_V1, binding_digest);
        let initial_round_digest = hash_domain_parts(
            DIRECT_INITIAL_ROUND_DOMAIN_V1,
            &[
                &binding_digest,
                &common_a_seed,
                &target_a_seed,
                &final_a_seed,
            ],
        );
        let context_digest = hash_domain_parts(
            DIRECT_CONTEXT_DOMAIN_V1,
            &[
                &binding_digest,
                &common_a_seed,
                &target_a_seed,
                &final_a_seed,
                &initial_round_digest,
            ],
        );
        let value = Self {
            version: MKHE_VERSION_V1,
            profile_digest: roster.profile_digest(),
            roster_digest: roster.roster_digest(),
            key_material_digest: roster.key_material_digest(),
            epoch: roster.epoch(),
            transcript_digest,
            collective_public_key_digest,
            secret_lineage_digests,
            secret_lineage_root,
            target,
            evaluated_key_ordinal,
            digit_index,
            galois_exponent,
            binding_digest,
            common_a_seed,
            target_a_seed,
            final_a_seed,
            initial_round_digest,
            context_digest,
        };
        value.validate(roster)?;
        Ok(value)
    }

    /// Mint one context from the complete opaque CPK secret-binding set.
    ///
    /// Callers cannot provide lineage leaves or a root.  Both are taken from
    /// the exact-membership verifier capability, and the CPK transcript/key
    /// axes are inherited from that same sealed set.
    pub(super) fn from_verified_binding_set(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
        target: ZkAmsMkheDirectEvaluatedKeyTargetV1,
        digit_index: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let consumer = match target {
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => {
                PersistentWitnessConsumerV1::RkgRoundOne
            }
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { .. } => {
                PersistentWitnessConsumerV1::Galois
            }
        };
        bindings.validate_for_consumer(roster, consumer)?;
        let value = Self::new_unchecked_inputs(
            roster,
            bindings.cpk_transcript_digest(),
            bindings.collective_public_key_digest(),
            *bindings.identity_digests(),
            Some(bindings.set_root()),
            target,
            digit_index,
        )?;
        value.validate(roster)?;
        Ok(value)
    }

    /// Revalidate the exact relinearization context used by an RKG-ephemeral
    /// membership source against the opaque CPK secret-binding set.
    ///
    /// This deliberately returns no lineage leaf or set internals.  It is the
    /// narrow authority check used by the sibling membership wrapper before
    /// binding a party-local `u_i` opening to one evaluated-key digit.
    pub(super) fn validate_rkg_ephemeral_membership_axes(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate(roster)?;
        bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
        if self.profile_digest != roster.profile_digest()
            || self.roster_digest != roster.roster_digest()
            || self.key_material_digest != roster.key_material_digest()
            || self.epoch != roster.epoch()
            || self.transcript_digest != bindings.cpk_transcript_digest()
            || self.collective_public_key_digest != bindings.collective_public_key_digest()
            || self.secret_lineage_digests != *bindings.identity_digests()
            || self.secret_lineage_root != bindings.set_root()
            || self.target != ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization
            || self.evaluated_key_ordinal != 0
            || self.galois_exponent != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    /// Frozen profile digest.
    #[must_use]
    pub const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }

    /// Exact governed roster digest.
    #[must_use]
    pub const fn roster_digest(self) -> [u8; 32] {
        self.roster_digest
    }

    /// Exact authentication-key material digest of the roster.
    #[must_use]
    pub const fn key_material_digest(self) -> [u8; 32] {
        self.key_material_digest
    }

    /// Governed nonzero secret epoch.
    #[must_use]
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Parent collective-public-key transcript digest.
    #[must_use]
    pub const fn transcript_digest(self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Digest of the already verified collective public key.
    #[must_use]
    pub const fn collective_public_key_digest(self) -> [u8; 32] {
        self.collective_public_key_digest
    }

    /// Root of the eight proof-layer secret-lineage commitments.
    ///
    /// The exact proof layer must link each leaf to the same `s_i` used by the
    /// governed CPK share, every evaluated-key contribution, and decryption.
    #[must_use]
    pub const fn secret_lineage_root(self) -> [u8; 32] {
        self.secret_lineage_root
    }

    /// Bound evaluated-key target.
    #[must_use]
    pub const fn target(self) -> ZkAmsMkheDirectEvaluatedKeyTargetV1 {
        self.target
    }

    /// Exact evaluated-key ordinal (relinearization zero, then Galois order).
    #[must_use]
    pub const fn evaluated_key_ordinal(self) -> u8 {
        self.evaluated_key_ordinal
    }

    /// Exact hybrid-RNS gadget digit.
    #[must_use]
    pub const fn digit_index(self) -> u8 {
        self.digit_index
    }

    /// Exact frozen odd Galois exponent, or zero for relinearization.
    #[must_use]
    pub const fn galois_exponent(self) -> u32 {
        self.galois_exponent
    }

    /// Seed for deterministic rejection-sampled RKG common `a`.
    #[must_use]
    pub const fn common_a_seed(self) -> [u8; 32] {
        self.common_a_seed
    }

    /// Seed for deterministic rejection-sampled Galois target `a_d`.
    #[must_use]
    pub const fn target_a_seed(self) -> [u8; 32] {
        self.target_a_seed
    }

    /// Seed for the final public `A` in the compact relinearization key.
    #[must_use]
    pub const fn final_a_seed(self) -> [u8; 32] {
        self.final_a_seed
    }

    /// Required `prior_round_digest` for the first contribution set.
    #[must_use]
    pub const fn initial_round_digest(self) -> [u8; 32] {
        self.initial_round_digest
    }

    /// Consensus identity of every context field and both `a` derivations.
    #[must_use]
    pub const fn digest(self) -> [u8; 32] {
        self.context_digest
    }

    fn validate(self, roster: &ZkAmsMkheGovernedActiveRosterV1) -> Result<(), ZkAmsMkheErrorV1> {
        roster.validate()?;
        let rebuilt = Self::new_unchecked_inputs(
            roster,
            self.transcript_digest,
            self.collective_public_key_digest,
            self.secret_lineage_digests,
            Some(self.secret_lineage_root),
            self.target,
            usize::from(self.digit_index),
        )?;
        if self != rebuilt {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn new_unchecked_inputs(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        collective_public_key_digest: [u8; 32],
        secret_lineage_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        sealed_secret_lineage_root: Option<[u8; 32]>,
        target: ZkAmsMkheDirectEvaluatedKeyTargetV1,
        digit_index: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Keep validation non-recursive while forcing the same canonical
        // constructor path for every derived field.
        let profile = release_profile_v1();
        if transcript_digest == [0; 32]
            || collective_public_key_digest == [0; 32]
            || secret_lineage_digests.contains(&[0; 32])
            || digit_index >= profile.gadget_digits
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let (evaluated_key_ordinal, galois_exponent) = match target {
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => (0, 0),
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index } => {
                let schedule = zk_ams_t256_galois_key_schedule_v1()?;
                validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
                let entry = schedule
                    .entries
                    .get(usize::from(schedule_index))
                    .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
                (
                    schedule_index
                        .checked_add(1)
                        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    entry.exponent,
                )
            }
        };
        let digit_index =
            u8::try_from(digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let secret_lineage_root = match sealed_secret_lineage_root {
            Some(root) if root != [0; 32] => root,
            Some(_) => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            None => direct_secret_lineage_root(roster, &secret_lineage_digests)?,
        };
        let binding_digest = direct_context_binding_digest(
            roster.profile_digest(),
            roster.roster_digest(),
            roster.key_material_digest(),
            roster.epoch(),
            transcript_digest,
            collective_public_key_digest,
            secret_lineage_root,
            target,
            evaluated_key_ordinal,
            digit_index,
            galois_exponent,
        );
        let common_a_seed = derive_a_seed(DIRECT_COMMON_A_DOMAIN_V1, binding_digest);
        let target_a_seed = derive_a_seed(DIRECT_TARGET_A_DOMAIN_V1, binding_digest);
        let final_a_seed = derive_a_seed(DIRECT_FINAL_A_DOMAIN_V1, binding_digest);
        let initial_round_digest = hash_domain_parts(
            DIRECT_INITIAL_ROUND_DOMAIN_V1,
            &[
                &binding_digest,
                &common_a_seed,
                &target_a_seed,
                &final_a_seed,
            ],
        );
        let context_digest = hash_domain_parts(
            DIRECT_CONTEXT_DOMAIN_V1,
            &[
                &binding_digest,
                &common_a_seed,
                &target_a_seed,
                &final_a_seed,
                &initial_round_digest,
            ],
        );
        Ok(Self {
            version: MKHE_VERSION_V1,
            profile_digest: roster.profile_digest(),
            roster_digest: roster.roster_digest(),
            key_material_digest: roster.key_material_digest(),
            epoch: roster.epoch(),
            transcript_digest,
            collective_public_key_digest,
            secret_lineage_digests,
            secret_lineage_root,
            target,
            evaluated_key_ordinal,
            digit_index,
            galois_exponent,
            binding_digest,
            common_a_seed,
            target_a_seed,
            final_a_seed,
            initial_round_digest,
            context_digest,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn direct_context_binding_digest(
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    secret_lineage_root: [u8; 32],
    target: ZkAmsMkheDirectEvaluatedKeyTargetV1,
    evaluated_key_ordinal: u8,
    digit_index: u8,
    galois_exponent: u32,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(DIRECT_CONTEXT_DOMAIN_V1);
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&profile_digest);
    frame.extend_from_slice(&roster_digest);
    frame.extend_from_slice(&key_material_digest);
    frame.extend_from_slice(&epoch.to_be_bytes());
    frame.extend_from_slice(&transcript_digest);
    frame.extend_from_slice(&collective_public_key_digest);
    frame.extend_from_slice(&secret_lineage_root);
    frame.push(target.tag());
    match target {
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => frame.push(u8::MAX),
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index } => {
            frame.push(schedule_index);
        }
    }
    frame.push(evaluated_key_ordinal);
    frame.push(digit_index);
    frame.extend_from_slice(&galois_exponent.to_be_bytes());
    keccak256(&frame)
}

fn direct_secret_lineage_root(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    lineage_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if lineage_digests.contains(&[0; 32]) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.direct-collective-secret-lineage-root");
    hash.update(&roster.profile_digest());
    hash.update(&roster.roster_digest());
    hash.update(&roster.key_material_digest());
    hash.update(&roster.epoch().to_be_bytes());
    for (index, (participant, digest)) in roster
        .participants()
        .iter()
        .zip(lineage_digests)
        .enumerate()
    {
        hash.update(&[index as u8]);
        hash.update(&participant.party().to_bytes());
        hash.update(digest);
    }
    Ok(hash.finalize())
}

fn derive_a_seed(domain: &[u8], binding_digest: [u8; 32]) -> [u8; 32] {
    let seed = hash_domain_parts(
        domain,
        &[&binding_digest, b"shake256-u64-le-rejection-sampling"],
    );
    debug_assert_ne!(seed, [0; 32]);
    seed
}

fn hash_domain_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(domain);
    for part in parts {
        hash.update(
            &u32::try_from(part.len())
                .expect("direct-ceremony digest inputs are statically bounded")
                .to_be_bytes(),
        );
        hash.update(part);
    }
    hash.finalize()
}

/// Audit result for the retired sparse-challenge proof as a direct-ceremony proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectProofAuditV1 {
    /// Exact weight of the audited release challenge family.
    pub sparse_challenge_weight: u8,
    /// Whether every challenge has even Hamming weight.
    pub challenge_weight_is_even: bool,
    /// Whether every distinct challenge difference is proven a ring unit.
    pub challenge_difference_unit_guaranteed: bool,
    /// Whether an exact bounded witness extractor is pinned.
    pub exact_extractor_pinned: bool,
    /// Whether a reduction for every structured ceremony matrix is pinned.
    pub structured_module_sis_reduction_pinned: bool,
    /// Whether concrete hardness estimates for those structured instances are pinned.
    pub structured_module_sis_estimate_pinned: bool,
    /// Whether the proof may admit a release contribution.
    pub release_proof_admission_enabled: bool,
    /// Digest of the audit facts and all relevant domains.
    pub audit_digest: [u8; 32],
}

impl ZkAmsMkheDirectProofAuditV1 {
    /// Recompute the fail-closed audit result.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        if self != derive_direct_proof_audit_v1()
            || !self.challenge_weight_is_even
            || self.challenge_difference_unit_guaranteed
            || self.exact_extractor_pinned
            || self.structured_module_sis_reduction_pinned
            || self.structured_module_sis_estimate_pinned
            || self.release_proof_admission_enabled
            || self.audit_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return the machine-checkable fail-closed direct-ceremony proof audit.
pub fn zk_ams_mkhe_direct_proof_audit_v1() -> Result<ZkAmsMkheDirectProofAuditV1, ZkAmsMkheErrorV1>
{
    let value = derive_direct_proof_audit_v1();
    value.validate()?;
    Ok(value)
}

fn derive_direct_proof_audit_v1() -> ZkAmsMkheDirectProofAuditV1 {
    let mut value = ZkAmsMkheDirectProofAuditV1 {
        sparse_challenge_weight: ACTIVE_SPARSE_CHALLENGE_WEIGHT_V1,
        challenge_weight_is_even: ACTIVE_SPARSE_CHALLENGE_WEIGHT_V1.is_multiple_of(2),
        challenge_difference_unit_guaranteed: false,
        exact_extractor_pinned: false,
        structured_module_sis_reduction_pinned: false,
        structured_module_sis_estimate_pinned: false,
        release_proof_admission_enabled: false,
        audit_digest: [0; 32],
    };
    let flags = [
        value.challenge_weight_is_even.into(),
        value.challenge_difference_unit_guaranteed.into(),
        value.exact_extractor_pinned.into(),
        value.structured_module_sis_reduction_pinned.into(),
        value.structured_module_sis_estimate_pinned.into(),
        value.release_proof_admission_enabled.into(),
    ];
    value.audit_digest = hash_domain_parts(
        DIRECT_PROOF_AUDIT_DOMAIN_V1,
        &[
            &[value.sparse_challenge_weight],
            &flags,
            b"forking-yields-A-delta-z-equals-delta-c-times-target",
            b"exact-extraction-requires-justified-division-or-an-exact-proof",
            DIRECT_CONTRIBUTION_DOMAIN_V1,
        ],
    );
    value
}

/// Exact deterministic bounds for the direct-collective evaluated-key algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectNoiseCertificateV1 {
    /// Release ring degree.
    pub ring_degree: u32,
    /// Exact governed party count.
    pub roster_size: u8,
    /// Maximum absolute sampled error coefficient.
    pub error_eta: u8,
    /// Bound on each aggregate `E0`, `E1`, `E2`, `E3`, or Galois error.
    pub aggregate_error_bound: u64,
    /// Bound on `S` and `U` coefficients.
    pub aggregate_secret_bound: u64,
    /// Bound on `E0*S + E1*U + E2 + E3` after public-`A` normalization.
    pub relinearization_intrinsic_error_bound: u64,
    /// Bound on the intrinsic error of one Galois digit.
    pub galois_intrinsic_error_bound: u64,
    /// Strict bit length of the relinearization bound.
    pub relinearization_intrinsic_error_bits: u8,
    /// Digest of the exact algebra and numeric bounds.
    pub certificate_digest: [u8; 32],
}

impl ZkAmsMkheDirectNoiseCertificateV1 {
    /// Recompute every algebraic worst-case bound.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        if self != derive_direct_noise_certificate(&release_profile_v1())?
            || self.aggregate_error_bound == 0
            || self.aggregate_secret_bound == 0
            || self.relinearization_intrinsic_error_bound == 0
            || self.galois_intrinsic_error_bound == 0
            || self.certificate_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return exact release-profile direct-ceremony noise accounting.
pub fn zk_ams_mkhe_direct_noise_certificate_v1()
-> Result<ZkAmsMkheDirectNoiseCertificateV1, ZkAmsMkheErrorV1> {
    let value = derive_direct_noise_certificate(&release_profile_v1())?;
    value.validate()?;
    Ok(value)
}

fn derive_direct_noise_certificate(
    profile: &BgvProfile,
) -> Result<ZkAmsMkheDirectNoiseCertificateV1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    let degree =
        u64::try_from(profile.ring_degree).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let parties = u64::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let eta = u64::from(profile.error_eta);
    let aggregate_error_bound = parties
        .checked_mul(eta)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let aggregate_secret_bound = parties;
    // ||f*g||_inf <= N ||f||_inf ||g||_inf in Z[X]/(X^N+1).
    let product_bound = degree
        .checked_mul(aggregate_error_bound)
        .and_then(|value| value.checked_mul(aggregate_secret_bound))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let relinearization_intrinsic_error_bound = product_bound
        .checked_mul(2)
        .and_then(|value| {
            aggregate_error_bound
                .checked_mul(2)
                .and_then(|linear| value.checked_add(linear))
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let relinearization_intrinsic_error_bits =
        u8::try_from(u64::BITS - relinearization_intrinsic_error_bound.leading_zeros())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut value = ZkAmsMkheDirectNoiseCertificateV1 {
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        roster_size: u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        error_eta: profile.error_eta,
        aggregate_error_bound,
        aggregate_secret_bound,
        relinearization_intrinsic_error_bound,
        galois_intrinsic_error_bound: aggregate_error_bound,
        relinearization_intrinsic_error_bits,
        certificate_digest: [0; 32],
    };
    let mut frame = Vec::with_capacity(192);
    frame.extend_from_slice(DIRECT_NOISE_DOMAIN_V1);
    frame.extend_from_slice(&profile.digest()?);
    frame.extend_from_slice(&value.ring_degree.to_be_bytes());
    frame.push(value.roster_size);
    frame.push(value.error_eta);
    frame.extend_from_slice(&value.aggregate_error_bound.to_be_bytes());
    frame.extend_from_slice(&value.aggregate_secret_bound.to_be_bytes());
    frame.extend_from_slice(&value.relinearization_intrinsic_error_bound.to_be_bytes());
    frame.extend_from_slice(&value.galois_intrinsic_error_bound.to_be_bytes());
    frame.push(value.relinearization_intrinsic_error_bits);
    frame.extend_from_slice(
        b"K+H1*S=g^d*S^2+p*(E0*S+E1*U+E2);B=K+sum((H1-A)*s_i+p*e3_i);B+A*S=g^d*S^2+p*(E0*S+E1*U+E2+E3);G+a_d*S=g^d*sigma_k(S)+p*E",
    );
    value.certificate_digest = keccak256(&frame);
    Ok(value)
}

/// Opaque admission of the complete 32-key, 38-digit direct evaluated-key set.
///
/// There is deliberately no public constructor or decoder.  A production
/// constructor may be added only after every one of the 1,216 digit ceremonies
/// has an exact proof admission, canonical aggregate stream, and ordered-set
/// replay.  A per-round digest, signature, or one completed digit is not this
/// token and cannot disable the old evaluated-key CKS accounting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectEvaluatedKeySetAdmissionV1 {
    version: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    secret_lineage_root: [u8; 32],
    evaluated_key_count: u8,
    gadget_digit_count: u8,
    ordered_complete_key_set_digest: [u8; 32],
    exact_proof_admission_digest: [u8; 32],
    direct_algebra_digest: [u8; 32],
    resource_certificate_digest: [u8; 32],
    admission_digest: [u8; 32],
}

impl ZkAmsMkheDirectEvaluatedKeySetAdmissionV1 {
    /// Consensus identity of the complete admitted evaluated-key set.
    #[must_use]
    pub const fn digest(self) -> [u8; 32] {
        self.admission_digest
    }

    fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let expected_key_count = ZK_AMS_T256_GALOIS_KEY_COUNT_V1
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        let noise = zk_ams_mkhe_direct_noise_certificate_v1()?;
        let resources = zk_ams_mkhe_direct_resource_certificate_v1()?;
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.collective_public_key_digest == [0; 32]
            || self.secret_lineage_root == [0; 32]
            || usize::from(self.evaluated_key_count) != expected_key_count
            || usize::from(self.gadget_digit_count) != profile.gadget_digits
            || self.ordered_complete_key_set_digest == [0; 32]
            || self.exact_proof_admission_digest == [0; 32]
            || self.direct_algebra_digest != noise.certificate_digest
            || self.resource_certificate_digest != resources.certificate_digest
            || self.admission_digest == [0; 32]
            || self.admission_digest != direct_evaluated_key_set_admission_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}

fn direct_evaluated_key_set_admission_digest(
    admission: ZkAmsMkheDirectEvaluatedKeySetAdmissionV1,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(384);
    frame.push(admission.version);
    frame.extend_from_slice(&admission.profile_digest);
    frame.extend_from_slice(&admission.roster_digest);
    frame.extend_from_slice(&admission.key_material_digest);
    frame.extend_from_slice(&admission.epoch.to_be_bytes());
    frame.extend_from_slice(&admission.transcript_digest);
    frame.extend_from_slice(&admission.collective_public_key_digest);
    frame.extend_from_slice(&admission.secret_lineage_root);
    frame.push(admission.evaluated_key_count);
    frame.push(admission.gadget_digit_count);
    frame.extend_from_slice(&admission.ordered_complete_key_set_digest);
    frame.extend_from_slice(&admission.exact_proof_admission_digest);
    frame.extend_from_slice(&admission.direct_algebra_digest);
    frame.extend_from_slice(&admission.resource_certificate_digest);
    hash_domain_parts(DIRECT_EVALUATED_KEY_SET_ADMISSION_DOMAIN_V1, &[&frame])
}

/// Conditional integration of direct-key intrinsic noise with the retired
/// evaluated-key CKS correction.
///
/// The 494/2,287-bit schedule remains applicable until the complete opaque
/// direct-key admission token exists.  With that token it is inapplicable to
/// evaluated-key generation because the keys use the direct algebra certified
/// here.  This fact does not remove ingress CKS accounting, nor does it claim a
/// recomputed Phase-II/III schedule or close any release gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectNoiseIntegrationCertificateV1 {
    /// Digest of the exact direct RKG/Galois algebra and numeric bounds.
    pub direct_algebra_digest: [u8; 32],
    /// Complete direct evaluated-key-set admission, or zero when absent.
    pub direct_evaluated_key_set_admission_digest: [u8; 32],
    /// Direct normalized relinearization intrinsic coefficient bound.
    pub direct_relinearization_intrinsic_error_bound: u64,
    /// Strict bit length of that relinearization bound.
    pub direct_relinearization_intrinsic_error_bits: u8,
    /// Direct Galois intrinsic coefficient bound.
    pub direct_galois_intrinsic_error_bound: u64,
    /// Direct intrinsic quotient after multiplication by plaintext modulus `p`.
    pub direct_p_scaled_relinearization_residual_bits: u16,
    /// One 38-digit hybrid switch using a direct admitted key.
    pub direct_hybrid_key_switch_residual_bits: u16,
    /// Unchanged independent-key ingress plus its distinct CKS smudging.
    pub ingress_cks_residual_bits: u16,
    /// Eight-switch canonical composed-rotation residual.
    pub direct_composed_rotation_residual_bits: u16,
    /// Residual after one release sparse packed map.
    pub direct_mapped_fresh_residual_bits: u16,
    /// Residual after the eight-input linear accumulator.
    pub direct_linear_accumulator_residual_bits: u16,
    /// Residual after one encrypted cross product and relinearization.
    pub direct_cross_product_residual_bits: u16,
    /// Residual after the four-product Equation-(6) cross term.
    pub direct_equation_six_residual_bits: u16,
    /// Final level-one accumulator residual before public decryption smudging.
    pub direct_level_one_residual_bits: u16,
    /// Equation-(7) encrypted commitment-path residual.
    pub direct_encrypted_commitment_residual_bits: u16,
    /// Per-party statistically hiding terminal-decryption quotient width.
    pub direct_decryption_smudge_quotient_bits: u16,
    /// Public all-eight-share final residual.
    pub direct_public_decryption_final_residual_bits: u16,
    /// Correctness margin for public all-eight-share decoding.
    pub direct_public_decryption_margin_bits: u16,
    /// Internal-only MPC decode residual when no public smudging is exposed.
    pub direct_internal_decode_residual_bits: u16,
    /// Correctness margin for internal-only MPC decode.
    pub direct_internal_decode_margin_bits: u16,
    /// Old audited evaluated-key switch residual width.
    pub old_corrected_switch_residual_bits: u16,
    /// Old audited level-one residual width using centralized/CKS evaluated keys.
    pub old_corrected_level_one_residual_bits: u16,
    /// Old audited downstream final residual width.
    pub old_corrected_final_residual_bits: u16,
    /// Whether the complete opaque direct-key admission token was supplied.
    pub direct_evaluated_key_set_admitted: bool,
    /// Whether the old evaluated-key CKS-smudge blocker is eliminated.
    pub old_evaluated_key_cks_smudge_blocker_eliminated: bool,
    /// Whether the old 494/2,287-bit schedule still applies to evaluated keys.
    pub old_cks_494_2287_schedule_applies_to_evaluated_keys: bool,
    /// Ingress CKS remains a separate construction and accounting obligation.
    pub ingress_cks_noise_accounting_remains_distinct: bool,
    /// Whether a complete Phase-II/III schedule using direct keys is pinned.
    pub direct_phase23_noise_schedule_pinned: bool,
    /// Whether that schedule is applicable to the admitted key set.
    pub direct_phase23_noise_schedule_applies: bool,
    /// Always false until direct proof, streaming, KAT, and downstream gates close.
    pub release_gate: bool,
    /// Digest of every integration fact above.
    pub certificate_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectPhase23NoiseScheduleV1 {
    p_scaled_relinearization: u16,
    hybrid_key_switch: u16,
    ingress_cks: u16,
    composed_rotation: u16,
    mapped_fresh: u16,
    linear_accumulator: u16,
    cross_product: u16,
    equation_six: u16,
    level_one: u16,
    encrypted_commitment: u16,
    decryption_smudge_quotient: u16,
    public_final: u16,
    public_margin: u16,
    internal_final: u16,
    internal_margin: u16,
}

fn direct_bound_add(left: u16, right: u16) -> Result<u16, ZkAmsMkheErrorV1> {
    left.max(right)
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)
}

fn direct_ceil_log2(value: usize) -> Result<u16, ZkAmsMkheErrorV1> {
    if value == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    u16::try_from(usize::BITS - (value - 1).leading_zeros())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)
}

fn direct_bound_sum(value: u16, count: usize) -> Result<u16, ZkAmsMkheErrorV1> {
    value
        .checked_add(direct_ceil_log2(count)?)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)
}

fn direct_bound_polynomial_mul(
    left: u16,
    right: u16,
    log_ring_degree: u16,
) -> Result<u16, ZkAmsMkheErrorV1> {
    left.checked_add(right)
        .and_then(|value| value.checked_add(log_ring_degree))
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)
}

fn derive_direct_phase23_noise_schedule_v1()
-> Result<DirectPhase23NoiseScheduleV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    let direct = zk_ams_mkhe_direct_noise_certificate_v1()?;
    let log_ring_degree = u16::try_from(profile.ring_degree.trailing_zeros())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let party_count = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1;
    let hybrid_limb_count = profile.moduli.len();
    let max_batch_size =
        usize::from(super::phase23::zk_ams_phase23_equation_certificate_v1().max_batch_size);
    let max_switches = phase23_max_composed_rotation_key_switch_count(profile.ring_degree / 2)?;
    let centered_capacity = ZK_AMS_MKHE_CIPHERTEXT_MODULUS_BITS_V1
        .checked_sub(1)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let plaintext = 255_u16;
    let plaintext_modulus = 256_u16;
    let ternary = 1_u16;
    let sampled_error = direct_ceil_log2(usize::from(profile.error_eta) + 1)?;

    // Independently rederive the unchanged ingress CKS schedule.  This is not
    // imported from noise.rs, so changing either implementation is detected by
    // tests and certificate validation rather than silently self-confirming.
    let sampled_times_ternary =
        direct_bound_polynomial_mul(sampled_error, ternary, log_ring_degree)?;
    let independent_fresh_quotient = direct_bound_sum(sampled_times_ternary.max(sampled_error), 3)?;
    let independent_fresh_residual = independent_fresh_quotient
        .checked_add(plaintext_modulus)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let cks_smudge_quotient = independent_fresh_quotient
        .checked_add(ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1)
        .and_then(|value| value.checked_add(1))
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let aggregate_cks_smudge = direct_bound_sum(cks_smudge_quotient, party_count)?;
    let cks_smudge_residual = aggregate_cks_smudge
        .checked_add(plaintext_modulus)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let ingress_cks = direct_bound_add(independent_fresh_residual, cks_smudge_residual)?;

    let p_scaled_relinearization = u16::from(direct.relinearization_intrinsic_error_bits)
        .checked_add(plaintext_modulus)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let hybrid_key_switch = direct_bound_sum(
        direct_bound_polynomial_mul(60, p_scaled_relinearization, log_ring_degree)?,
        hybrid_limb_count,
    )?;
    let composed_rotation = direct_bound_add(
        ingress_cks,
        direct_bound_sum(hybrid_key_switch, max_switches)?,
    )?;
    let mapped_fresh = direct_bound_sum(
        direct_bound_polynomial_mul(composed_rotation, plaintext, log_ring_degree)?,
        ZK_AMS_MKHE_SPARSE_MAP_FAN_IN_CEILING_V1,
    )?;
    let phase23_fresh_operand = mapped_fresh.max(ingress_cks);
    let linear_accumulator = direct_bound_sum(
        direct_bound_polynomial_mul(mapped_fresh, plaintext, log_ring_degree)?,
        max_batch_size,
    )?;
    let plaintext_product_quotient = direct_bound_add(
        direct_bound_polynomial_mul(plaintext, plaintext, log_ring_degree)?,
        plaintext,
    )?;
    let first_mixed =
        direct_bound_polynomial_mul(plaintext, phase23_fresh_operand, log_ring_degree)?;
    let second_mixed = direct_bound_polynomial_mul(plaintext, linear_accumulator, log_ring_degree)?;
    let residual_product =
        direct_bound_polynomial_mul(linear_accumulator, phase23_fresh_operand, log_ring_degree)?;
    let cross_product = direct_bound_add(
        direct_bound_sum(
            plaintext_product_quotient
                .max(first_mixed)
                .max(second_mixed)
                .max(residual_product),
            4,
        )?,
        hybrid_key_switch,
    )?;
    let equation_six = direct_bound_sum(cross_product, 4)?;
    let challenge_times_cross =
        direct_bound_polynomial_mul(equation_six, plaintext, log_ring_degree)?;
    let fresh_residual_product = direct_bound_polynomial_mul(
        phase23_fresh_operand,
        phase23_fresh_operand,
        log_ring_degree,
    )?;
    let fresh_product = direct_bound_add(
        direct_bound_sum(
            plaintext_product_quotient
                .max(first_mixed)
                .max(fresh_residual_product),
            4,
        )?,
        hybrid_key_switch,
    )?;
    let challenge_squared_times_fresh =
        direct_bound_polynomial_mul(fresh_product, plaintext, log_ring_degree)?;
    let level_one = direct_bound_sum(
        direct_bound_add(challenge_times_cross, challenge_squared_times_fresh)?,
        max_batch_size,
    )?;
    let encrypted_commitment = direct_bound_sum(
        direct_bound_polynomial_mul(
            direct_bound_add(linear_accumulator, hybrid_key_switch)?,
            plaintext,
            log_ring_degree,
        )?,
        ZK_AMS_MKHE_SPARSE_MAP_FAN_IN_CEILING_V1,
    )?;
    let evaluated_residual = level_one.max(encrypted_commitment);
    let evaluated_quotient = evaluated_residual
        .checked_sub(255)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let decryption_smudge_quotient = evaluated_quotient
        .checked_add(ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1)
        .and_then(|value| value.checked_add(1))
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let aggregate_decryption_smudge = direct_bound_sum(decryption_smudge_quotient, party_count)?;
    let public_final = direct_bound_add(
        evaluated_residual,
        aggregate_decryption_smudge
            .checked_add(plaintext_modulus)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?,
    )?;
    let public_margin = centered_capacity
        .checked_sub(public_final)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let internal_final = evaluated_residual;
    let internal_margin = centered_capacity
        .checked_sub(internal_final)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;

    Ok(DirectPhase23NoiseScheduleV1 {
        p_scaled_relinearization,
        hybrid_key_switch,
        ingress_cks,
        composed_rotation,
        mapped_fresh,
        linear_accumulator,
        cross_product,
        equation_six,
        level_one,
        encrypted_commitment,
        decryption_smudge_quotient,
        public_final,
        public_margin,
        internal_final,
        internal_margin,
    })
}

impl ZkAmsMkheDirectNoiseIntegrationCertificateV1 {
    /// Recheck all conditional facts against both source certificates.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let noise = zk_ams_mkhe_direct_noise_certificate_v1()?;
        let phase23 =
            super::phase23_mask_proof::zk_ams_phase23_mask_proof_audit_v1(&release_profile_v1())?;
        let schedule = derive_direct_phase23_noise_schedule_v1()?;
        let admission_present = self.direct_evaluated_key_set_admission_digest != [0; 32];
        if self.direct_algebra_digest != noise.certificate_digest
            || self.direct_relinearization_intrinsic_error_bound
                != noise.relinearization_intrinsic_error_bound
            || self.direct_relinearization_intrinsic_error_bits
                != noise.relinearization_intrinsic_error_bits
            || self.direct_galois_intrinsic_error_bound != noise.galois_intrinsic_error_bound
            || self.direct_p_scaled_relinearization_residual_bits
                != schedule.p_scaled_relinearization
            || self.direct_hybrid_key_switch_residual_bits != schedule.hybrid_key_switch
            || self.ingress_cks_residual_bits != schedule.ingress_cks
            || self.direct_composed_rotation_residual_bits != schedule.composed_rotation
            || self.direct_mapped_fresh_residual_bits != schedule.mapped_fresh
            || self.direct_linear_accumulator_residual_bits != schedule.linear_accumulator
            || self.direct_cross_product_residual_bits != schedule.cross_product
            || self.direct_equation_six_residual_bits != schedule.equation_six
            || self.direct_level_one_residual_bits != schedule.level_one
            || self.direct_encrypted_commitment_residual_bits != schedule.encrypted_commitment
            || self.direct_decryption_smudge_quotient_bits != schedule.decryption_smudge_quotient
            || self.direct_public_decryption_final_residual_bits != schedule.public_final
            || self.direct_public_decryption_margin_bits != schedule.public_margin
            || self.direct_internal_decode_residual_bits != schedule.internal_final
            || self.direct_internal_decode_margin_bits != schedule.internal_margin
            || self.old_corrected_switch_residual_bits != phase23.corrected_switch_residual_bits
            || self.old_corrected_level_one_residual_bits
                != phase23.corrected_level_one_residual_bits
            || self.old_corrected_final_residual_bits != phase23.corrected_final_residual_bits
            || self.direct_evaluated_key_set_admitted != admission_present
            || self.old_evaluated_key_cks_smudge_blocker_eliminated != admission_present
            || self.old_cks_494_2287_schedule_applies_to_evaluated_keys == admission_present
            || !self.ingress_cks_noise_accounting_remains_distinct
            || !self.direct_phase23_noise_schedule_pinned
            || self.direct_phase23_noise_schedule_applies != admission_present
            || self.release_gate
            || self.certificate_digest == [0; 32]
            || self.certificate_digest != direct_noise_integration_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return the current fail-closed integration fact without a direct-key token.
pub fn zk_ams_mkhe_direct_noise_integration_certificate_v1()
-> Result<ZkAmsMkheDirectNoiseIntegrationCertificateV1, ZkAmsMkheErrorV1> {
    derive_direct_noise_integration_certificate(None)
}

/// Return the conditional integration fact for an opaque complete direct-key set.
pub fn zk_ams_mkhe_direct_noise_integration_for_admitted_keys_v1(
    admission: &ZkAmsMkheDirectEvaluatedKeySetAdmissionV1,
) -> Result<ZkAmsMkheDirectNoiseIntegrationCertificateV1, ZkAmsMkheErrorV1> {
    admission.validate()?;
    derive_direct_noise_integration_certificate(Some(admission.admission_digest))
}

fn derive_direct_noise_integration_certificate(
    admission_digest: Option<[u8; 32]>,
) -> Result<ZkAmsMkheDirectNoiseIntegrationCertificateV1, ZkAmsMkheErrorV1> {
    let noise = zk_ams_mkhe_direct_noise_certificate_v1()?;
    let phase23 =
        super::phase23_mask_proof::zk_ams_phase23_mask_proof_audit_v1(&release_profile_v1())?;
    let schedule = derive_direct_phase23_noise_schedule_v1()?;
    let admission_digest = admission_digest.unwrap_or([0; 32]);
    let admitted = admission_digest != [0; 32];
    let mut value = ZkAmsMkheDirectNoiseIntegrationCertificateV1 {
        direct_algebra_digest: noise.certificate_digest,
        direct_evaluated_key_set_admission_digest: admission_digest,
        direct_relinearization_intrinsic_error_bound: noise.relinearization_intrinsic_error_bound,
        direct_relinearization_intrinsic_error_bits: noise.relinearization_intrinsic_error_bits,
        direct_galois_intrinsic_error_bound: noise.galois_intrinsic_error_bound,
        direct_p_scaled_relinearization_residual_bits: schedule.p_scaled_relinearization,
        direct_hybrid_key_switch_residual_bits: schedule.hybrid_key_switch,
        ingress_cks_residual_bits: schedule.ingress_cks,
        direct_composed_rotation_residual_bits: schedule.composed_rotation,
        direct_mapped_fresh_residual_bits: schedule.mapped_fresh,
        direct_linear_accumulator_residual_bits: schedule.linear_accumulator,
        direct_cross_product_residual_bits: schedule.cross_product,
        direct_equation_six_residual_bits: schedule.equation_six,
        direct_level_one_residual_bits: schedule.level_one,
        direct_encrypted_commitment_residual_bits: schedule.encrypted_commitment,
        direct_decryption_smudge_quotient_bits: schedule.decryption_smudge_quotient,
        direct_public_decryption_final_residual_bits: schedule.public_final,
        direct_public_decryption_margin_bits: schedule.public_margin,
        direct_internal_decode_residual_bits: schedule.internal_final,
        direct_internal_decode_margin_bits: schedule.internal_margin,
        old_corrected_switch_residual_bits: phase23.corrected_switch_residual_bits,
        old_corrected_level_one_residual_bits: phase23.corrected_level_one_residual_bits,
        old_corrected_final_residual_bits: phase23.corrected_final_residual_bits,
        direct_evaluated_key_set_admitted: admitted,
        old_evaluated_key_cks_smudge_blocker_eliminated: admitted,
        old_cks_494_2287_schedule_applies_to_evaluated_keys: !admitted,
        ingress_cks_noise_accounting_remains_distinct: true,
        direct_phase23_noise_schedule_pinned: true,
        direct_phase23_noise_schedule_applies: admitted,
        release_gate: false,
        certificate_digest: [0; 32],
    };
    value.certificate_digest = direct_noise_integration_digest(value);
    value.validate()?;
    Ok(value)
}

fn direct_noise_integration_digest(
    value: ZkAmsMkheDirectNoiseIntegrationCertificateV1,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(&value.direct_algebra_digest);
    frame.extend_from_slice(&value.direct_evaluated_key_set_admission_digest);
    frame.extend_from_slice(
        &value
            .direct_relinearization_intrinsic_error_bound
            .to_be_bytes(),
    );
    frame.push(value.direct_relinearization_intrinsic_error_bits);
    frame.extend_from_slice(&value.direct_galois_intrinsic_error_bound.to_be_bytes());
    for bits in [
        value.direct_p_scaled_relinearization_residual_bits,
        value.direct_hybrid_key_switch_residual_bits,
        value.ingress_cks_residual_bits,
        value.direct_composed_rotation_residual_bits,
        value.direct_mapped_fresh_residual_bits,
        value.direct_linear_accumulator_residual_bits,
        value.direct_cross_product_residual_bits,
        value.direct_equation_six_residual_bits,
        value.direct_level_one_residual_bits,
        value.direct_encrypted_commitment_residual_bits,
        value.direct_decryption_smudge_quotient_bits,
        value.direct_public_decryption_final_residual_bits,
        value.direct_public_decryption_margin_bits,
        value.direct_internal_decode_residual_bits,
        value.direct_internal_decode_margin_bits,
    ] {
        frame.extend_from_slice(&bits.to_be_bytes());
    }
    frame.extend_from_slice(&value.old_corrected_switch_residual_bits.to_be_bytes());
    frame.extend_from_slice(&value.old_corrected_level_one_residual_bits.to_be_bytes());
    frame.extend_from_slice(&value.old_corrected_final_residual_bits.to_be_bytes());
    for flag in [
        value.direct_evaluated_key_set_admitted,
        value.old_evaluated_key_cks_smudge_blocker_eliminated,
        value.old_cks_494_2287_schedule_applies_to_evaluated_keys,
        value.ingress_cks_noise_accounting_remains_distinct,
        value.direct_phase23_noise_schedule_pinned,
        value.direct_phase23_noise_schedule_applies,
        value.release_gate,
    ] {
        frame.push(flag.into());
    }
    hash_domain_parts(DIRECT_NOISE_INTEGRATION_DOMAIN_V1, &[&frame])
}

/// Exact byte/work accounting and open release gates for the direct ceremony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectResourceCertificateV1 {
    /// Release ring degree.
    pub ring_degree: u32,
    /// Release RNS limb count and streamed chunks per polynomial.
    pub rns_limb_count: u8,
    /// Exact residues in one limb-aligned stream chunk.
    pub residues_per_chunk: u32,
    /// Exact bytes in one limb-aligned stream chunk.
    pub chunk_bytes: u64,
    /// Exact canonical raw residue bytes in one polynomial.
    pub polynomial_bytes: u64,
    /// Raw bytes in the two first-round polynomials of one party/digit.
    pub rkg_round_one_party_payload_bytes: u64,
    /// Maximum bytes left for proof/auth metadata beside one polynomial record.
    pub one_polynomial_record_headroom_bytes: u64,
    /// Exact ring multiplications performed by one party for one RKG digit.
    pub rkg_party_digit_ring_multiplications: u8,
    /// Exact ring multiplications performed by all parties over all 32 keys.
    pub full_ceremony_ring_multiplications: u64,
    /// Exact canonical bytes of the normalized seeded-`A`, stored-`B` key.
    pub normalized_evaluated_key_wire_bytes: u64,
    /// Raw polynomial bytes alone for a two-stored-polynomial key.
    ///
    /// This is a strict lower bound because it excludes every header, proof,
    /// manifest, and digest byte.
    pub two_stored_polynomial_payload_lower_bound_bytes: u64,
    /// Whether one polynomial fits the governed round ceiling.
    pub one_polynomial_record_fits_round: bool,
    /// Whether `H0_i,H1_i` may incorrectly be put in one round record.
    pub combined_rkg_round_one_record_fits_round: bool,
    /// Whether canonical limb-stream validation is implemented.
    pub canonical_limb_streaming_implemented: bool,
    /// Whether the normalized compact key fits the frozen 2 GiB key ceiling.
    pub normalized_evaluated_key_fits_ceiling: bool,
    /// Whether even the raw payload of a two-stored-polynomial key fits.
    pub two_stored_polynomial_payload_fits_ceiling: bool,
    /// Whether the final proof wire and its peak workspace are certified.
    pub proof_streaming_certified: bool,
    /// Whether a release-size malicious-party KAT is pinned.
    pub release_kat_pinned: bool,
    /// Whether direct-ceremony activation is allowed.
    pub release_gate: bool,
    /// Digest of all counts, ceilings, and gate values.
    pub certificate_digest: [u8; 32],
}

impl ZkAmsMkheDirectResourceCertificateV1 {
    /// Recompute exact release byte/work accounting.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        if self != derive_direct_resource_certificate(&release_profile_v1())?
            || !self.one_polynomial_record_fits_round
            || self.combined_rkg_round_one_record_fits_round
            || !self.canonical_limb_streaming_implemented
            || !self.normalized_evaluated_key_fits_ceiling
            || self.two_stored_polynomial_payload_fits_ceiling
            || self.proof_streaming_certified
            || self.release_kat_pinned
            || self.release_gate
            || self.certificate_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return exact release-profile direct-ceremony resource accounting.
pub fn zk_ams_mkhe_direct_resource_certificate_v1()
-> Result<ZkAmsMkheDirectResourceCertificateV1, ZkAmsMkheErrorV1> {
    let value = derive_direct_resource_certificate(&release_profile_v1())?;
    value.validate()?;
    Ok(value)
}

fn derive_direct_resource_certificate(
    profile: &BgvProfile,
) -> Result<ZkAmsMkheDirectResourceCertificateV1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    let chunk_bytes = profile
        .ring_degree
        .checked_mul(RESIDUE_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let polynomial_bytes = chunk_bytes
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let round_one_bytes = polynomial_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let one_polynomial_record_headroom_bytes = profile
        .max_round_bytes
        .checked_sub(polynomial_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let galois_key_count = u64::try_from(ZK_AMS_T256_GALOIS_KEY_COUNT_V1)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let digits =
        u64::try_from(profile.gadget_digits).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let parties = u64::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    // Per RKG party/digit: a*u, a*s, H0*s, H1*(u-s), (H1-A)*s.
    let rkg_multiplications = 5_u64;
    // Per Galois party/digit: a_d*s; automorphism is a permutation.
    let full_ceremony_ring_multiplications = digits
        .checked_mul(parties)
        .and_then(|value| value.checked_mul(rkg_multiplications))
        .and_then(|value| {
            galois_key_count
                .checked_mul(digits)
                .and_then(|galois| galois.checked_mul(parties))
                .and_then(|galois| value.checked_add(galois))
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let wire = derive_wire_length_certificate_v1(profile)?;
    let normalized_evaluated_key_wire_bytes =
        u64::try_from(wire.seeded_collective_relinearization_key_wire_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let two_stored_polynomial_payload_lower_bound_bytes =
        u64::try_from(wire.rns_polynomial_wire_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .checked_mul(digits)
            .and_then(|value| value.checked_mul(2))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let max_evaluated_key_bytes = u64::try_from(profile.max_evaluated_key_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut value = ZkAmsMkheDirectResourceCertificateV1 {
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        rns_limb_count: u8::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        residues_per_chunk: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        chunk_bytes: u64::try_from(chunk_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        polynomial_bytes: u64::try_from(polynomial_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        rkg_round_one_party_payload_bytes: u64::try_from(round_one_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        one_polynomial_record_headroom_bytes: u64::try_from(one_polynomial_record_headroom_bytes)
            .map_err(|_| {
            ZkAmsMkheErrorV1::ResourceCeilingExceeded
        })?,
        rkg_party_digit_ring_multiplications: u8::try_from(rkg_multiplications)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        full_ceremony_ring_multiplications,
        normalized_evaluated_key_wire_bytes,
        two_stored_polynomial_payload_lower_bound_bytes,
        one_polynomial_record_fits_round: polynomial_bytes <= profile.max_round_bytes,
        combined_rkg_round_one_record_fits_round: round_one_bytes <= profile.max_round_bytes,
        canonical_limb_streaming_implemented: true,
        normalized_evaluated_key_fits_ceiling: normalized_evaluated_key_wire_bytes
            <= max_evaluated_key_bytes,
        two_stored_polynomial_payload_fits_ceiling: two_stored_polynomial_payload_lower_bound_bytes
            <= max_evaluated_key_bytes,
        proof_streaming_certified: false,
        release_kat_pinned: false,
        release_gate: false,
        certificate_digest: [0; 32],
    };
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(DIRECT_RESOURCE_DOMAIN_V1);
    frame.extend_from_slice(&profile.digest()?);
    frame.extend_from_slice(&value.ring_degree.to_be_bytes());
    frame.push(value.rns_limb_count);
    frame.extend_from_slice(&value.residues_per_chunk.to_be_bytes());
    frame.extend_from_slice(&value.chunk_bytes.to_be_bytes());
    frame.extend_from_slice(&value.polynomial_bytes.to_be_bytes());
    frame.extend_from_slice(&value.rkg_round_one_party_payload_bytes.to_be_bytes());
    frame.extend_from_slice(&value.one_polynomial_record_headroom_bytes.to_be_bytes());
    frame.push(value.rkg_party_digit_ring_multiplications);
    frame.extend_from_slice(&value.full_ceremony_ring_multiplications.to_be_bytes());
    frame.extend_from_slice(&value.normalized_evaluated_key_wire_bytes.to_be_bytes());
    frame.extend_from_slice(
        &value
            .two_stored_polynomial_payload_lower_bound_bytes
            .to_be_bytes(),
    );
    for flag in [
        value.one_polynomial_record_fits_round,
        value.combined_rkg_round_one_record_fits_round,
        value.canonical_limb_streaming_implemented,
        value.normalized_evaluated_key_fits_ceiling,
        value.two_stored_polynomial_payload_fits_ceiling,
        value.proof_streaming_certified,
        value.release_kat_pinned,
        value.release_gate,
    ] {
        frame.push(flag.into());
    }
    value.certificate_digest = keccak256(&frame);
    Ok(value)
}

/// Content receipt produced after one polynomial has been streamed canonically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectPolynomialStreamReceiptV1 {
    context_digest: [u8; 32],
    round: ZkAmsMkheDirectCeremonyRoundV1,
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    role: ZkAmsMkheDirectPolynomialRoleV1,
    polynomial_digest: [u8; 32],
    canonical_bytes: u64,
}

impl ZkAmsMkheDirectPolynomialStreamReceiptV1 {
    /// Exact canonical polynomial digest.
    #[must_use]
    pub const fn polynomial_digest(self) -> [u8; 32] {
        self.polynomial_digest
    }

    /// Exact canonical raw residue bytes consumed.
    #[must_use]
    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }
}

/// Allocation-bounded canonical admission of one polynomial, one RNS limb at a time.
pub struct ZkAmsMkheDirectPolynomialStreamV1 {
    context_digest: [u8; 32],
    round: ZkAmsMkheDirectCeremonyRoundV1,
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    role: ZkAmsMkheDirectPolynomialRoleV1,
    expected_digest: [u8; 32],
    next_limb: usize,
    canonical_bytes: usize,
    hash: Keccak256,
}

impl core::fmt::Debug for ZkAmsMkheDirectPolynomialStreamV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectPolynomialStreamV1")
            .field("context_digest", &hex::encode(self.context_digest))
            .field("round", &self.round)
            .field("party_index", &self.party_index)
            .field("party", &self.party)
            .field("role", &self.role)
            .field("next_limb", &self.next_limb)
            .field("canonical_bytes", &self.canonical_bytes)
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDirectPolynomialStreamV1 {
    /// Begin one exact release-profile polynomial stream.
    pub fn begin(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        context: ZkAmsMkheDirectCeremonyContextV1,
        round: ZkAmsMkheDirectCeremonyRoundV1,
        party_index: usize,
        role: ZkAmsMkheDirectPolynomialRoleV1,
        expected_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        context.validate(roster)?;
        if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || !role.belongs_to(round)
            || expected_digest == [0; 32]
            || matches!(
                (context.target, round),
                (
                    ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
                    ZkAmsMkheDirectCeremonyRoundV1::Galois
                ) | (
                    ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { .. },
                    ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne
                        | ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo
                        | ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize
                )
            )
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let party = roster.participants()[party_index].party();
        let profile = release_profile_v1();
        let mut hash = Keccak256::new();
        hash.update(DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1);
        hash.update(&context.context_digest);
        hash.update(&[round.tag(), role.tag()]);
        hash.update(
            &u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?
                .to_be_bytes(),
        );
        hash.update(&party.to_bytes());
        hash.update(
            &u32::try_from(profile.ring_degree)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                .to_be_bytes(),
        );
        hash.update(
            &u8::try_from(profile.moduli.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                .to_be_bytes(),
        );
        Ok(Self {
            context_digest: context.context_digest,
            round,
            party_index: u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party,
            role,
            expected_digest,
            next_limb: 0,
            canonical_bytes: 0,
            hash,
        })
    }

    /// Admit exactly the next complete canonical RNS limb.
    ///
    /// Length, order, and every residue are checked before the hash state or
    /// byte counters change, so a rejected chunk is transactional.
    pub fn admit_limb(
        &mut self,
        limb_index: usize,
        residues: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let modulus = profile
            .moduli
            .get(limb_index)
            .copied()
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        if limb_index != self.next_limb
            || residues.len() != profile.ring_degree
            || residues.iter().any(|residue| *residue >= modulus)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let chunk_bytes = residues
            .len()
            .checked_mul(RESIDUE_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let next_bytes = self
            .canonical_bytes
            .checked_add(chunk_bytes)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if next_bytes > profile.max_round_bytes {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        self.hash.update(
            &u8::try_from(limb_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
                .to_be_bytes(),
        );
        self.hash.update(&modulus.to_be_bytes());
        for residue in residues {
            self.hash.update(&residue.to_be_bytes());
        }
        self.next_limb += 1;
        self.canonical_bytes = next_bytes;
        Ok(())
    }

    /// Finish only after all limbs were received and the expected digest matches.
    pub fn finish(self) -> Result<ZkAmsMkheDirectPolynomialStreamReceiptV1, ZkAmsMkheErrorV1> {
        let resources = zk_ams_mkhe_direct_resource_certificate_v1()?;
        if self.next_limb != usize::from(resources.rns_limb_count)
            || u64::try_from(self.canonical_bytes).ok() != Some(resources.polynomial_bytes)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let polynomial_digest = self.hash.finalize();
        if polynomial_digest != self.expected_digest {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        Ok(ZkAmsMkheDirectPolynomialStreamReceiptV1 {
            context_digest: self.context_digest,
            round: self.round,
            party_index: self.party_index,
            party: self.party,
            role: self.role,
            polynomial_digest,
            canonical_bytes: resources.polynomial_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectContributionPayloadV1 {
    RkgRoundOne {
        h0: ZkAmsMkheDirectPolynomialStreamReceiptV1,
        h1: ZkAmsMkheDirectPolynomialStreamReceiptV1,
    },
    RkgRoundTwo {
        k: ZkAmsMkheDirectPolynomialStreamReceiptV1,
    },
    RkgNormalize {
        normalization: ZkAmsMkheDirectPolynomialStreamReceiptV1,
    },
    Galois {
        b: ZkAmsMkheDirectPolynomialStreamReceiptV1,
    },
}

/// Authenticated contribution which can only be constructed by a proof adapter.
///
/// There is intentionally no public constructor or decoder in V1.  This type
/// becomes reachable by the coordinator only after the direct proof gate is
/// closed; raw polynomial digests and signatures are not a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectVerifiedContributionV1 {
    version: u8,
    context_digest: [u8; 32],
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    secret_lineage_digest: [u8; 32],
    rkg_ephemeral_lineage_digest: [u8; 32],
    payload: DirectContributionPayloadV1,
    relation_use_digest: [u8; 32],
    proof_digest: [u8; 32],
    evidence_set_digest: [u8; 32],
    authentication: ArtifactAuthentication,
    contribution_digest: [u8; 32],
}

impl ZkAmsMkheDirectVerifiedContributionV1 {
    /// Bound roster position.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }

    /// Bound contributor identity.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Bound ceremony round.
    #[must_use]
    pub const fn round(&self) -> ZkAmsMkheDirectCeremonyRoundV1 {
        self.round
    }

    /// Digest of the exact prior aggregate round.
    #[must_use]
    pub const fn prior_round_digest(&self) -> [u8; 32] {
        self.prior_round_digest
    }

    /// Persistent proof-layer commitment to this party's `s_i`.
    #[must_use]
    pub const fn secret_lineage_digest(&self) -> [u8; 32] {
        self.secret_lineage_digest
    }

    /// Proof-layer commitment to `u_i`, or zero for a Galois contribution.
    #[must_use]
    pub const fn rkg_ephemeral_lineage_digest(&self) -> [u8; 32] {
        self.rkg_ephemeral_lineage_digest
    }

    /// Digest of the consumed, actual-commitment relation-use capability.
    #[must_use]
    pub const fn relation_use_digest(&self) -> [u8; 32] {
        self.relation_use_digest
    }

    /// Digest of the complete contribution including proof identity and signature.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.contribution_digest
    }

    /// Digest of the exact canonical proof-evidence stream for this party slot.
    #[must_use]
    pub const fn evidence_set_digest(&self) -> [u8; 32] {
        self.evidence_set_digest
    }
}

fn persistent_relation_for_round(
    round: ZkAmsMkheDirectCeremonyRoundV1,
) -> PersistentDirectRelationV1 {
    match round {
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne => PersistentDirectRelationV1::RkgRoundOne,
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo => PersistentDirectRelationV1::RkgRoundTwo,
        ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize => PersistentDirectRelationV1::RkgNormalize,
        ZkAmsMkheDirectCeremonyRoundV1::Galois => PersistentDirectRelationV1::Galois,
    }
}

fn direct_relation_contribution_statement_digest(
    context: ZkAmsMkheDirectCeremonyContextV1,
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    payload: DirectContributionPayloadV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if prior_round_digest == [0; 32]
        || usize::from(party_index) >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(DIRECT_RELATION_STATEMENT_DOMAIN_V1);
    hash.update(&context.context_digest);
    hash.update(&[round.tag()]);
    hash.update(&prior_round_digest);
    hash.update(&[party_index]);
    hash.update(&party.to_bytes());
    hash.update(&[
        context.target.tag(),
        context.evaluated_key_ordinal,
        context.digit_index,
    ]);
    hash.update(&context.galois_exponent.to_be_bytes());
    hash.update(&context.common_a_seed);
    hash.update(&context.target_a_seed);
    hash.update(&context.final_a_seed);
    match payload {
        DirectContributionPayloadV1::RkgRoundOne { h0, h1 }
            if round == ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne =>
        {
            hash.update(&[1]);
            hash.update(&h0.polynomial_digest);
            hash.update(&h1.polynomial_digest);
        }
        DirectContributionPayloadV1::RkgRoundTwo { k }
            if round == ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo =>
        {
            hash.update(&[2]);
            hash.update(&k.polynomial_digest);
        }
        DirectContributionPayloadV1::RkgNormalize { normalization }
            if round == ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize =>
        {
            hash.update(&[3]);
            hash.update(&normalization.polynomial_digest);
        }
        DirectContributionPayloadV1::Galois { b }
            if round == ZkAmsMkheDirectCeremonyRoundV1::Galois =>
        {
            hash.update(&[4]);
            hash.update(&b.polynomial_digest);
        }
        _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
    }
    Ok(hash.finalize())
}

/// Sole constructor used after the exact proof verifier has consumed an
/// actual-commitment relation capability.  It is unreachable while that
/// verifier remains fail-closed.
#[allow(dead_code, clippy::too_many_arguments)]
fn mint_verified_contribution_from_exact_receipt<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    payload: DirectContributionPayloadV1,
    receipt: VerifiedDirectRelationProofReceiptV1,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheDirectVerifiedContributionV1, ZkAmsMkheErrorV1> {
    context.validate(roster)?;
    receipt.validate()?;
    let party_index = usize::from(receipt.party_index());
    let participant = roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    let expected_statement = direct_relation_contribution_statement_digest(
        context,
        round,
        prior_round_digest,
        receipt.party_index(),
        receipt.party(),
        payload,
    )?;
    if receipt.relation() != persistent_relation_for_round(round)
        || receipt.context_digest() != context.context_digest
        || receipt.prior_round_digest() != prior_round_digest
        || receipt.evaluated_key_ordinal() != context.evaluated_key_ordinal
        || receipt.digit_index() != context.digit_index
        || receipt.galois_exponent() != context.galois_exponent
        || receipt.party() != participant.party()
        || receipt.party() != party_secret.party()?
        || receipt.secret_identity_digest() != context.secret_lineage_digests[party_index]
        || receipt.contribution_statement_digest() != expected_statement
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let ephemeral = receipt.ephemeral_identity_digest();
    match round {
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne
        | ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo
            if ephemeral == [0; 32] =>
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize | ZkAmsMkheDirectCeremonyRoundV1::Galois
            if ephemeral != [0; 32] =>
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        _ => {}
    }
    let mut contribution = ZkAmsMkheDirectVerifiedContributionV1 {
        version: MKHE_VERSION_V1,
        context_digest: context.context_digest,
        round,
        prior_round_digest,
        party_index: receipt.party_index(),
        party: receipt.party(),
        secret_lineage_digest: receipt.secret_identity_digest(),
        rkg_ephemeral_lineage_digest: ephemeral,
        payload,
        relation_use_digest: receipt.relation_use_digest(),
        proof_digest: receipt.proof_digest(),
        evidence_set_digest: receipt.evidence_set_digest(),
        authentication: ArtifactAuthentication {
            version: 0,
            party: receipt.party(),
            public_key: [0; 33],
            signature: [0; 65],
        },
        contribution_digest: [0; 32],
    };
    validate_contribution_payload(context, &contribution)?;
    let statement = direct_contribution_auth_statement(&contribution)?;
    contribution.authentication = party_secret.authenticate_artifact(
        DIRECT_CONTRIBUTION_AUTH_DOMAIN_V1,
        statement,
        random,
    )?;
    contribution.contribution_digest = direct_contribution_digest(&contribution)?;
    validate_verified_contribution(roster, context, round, prior_round_digest, &contribution)?;
    Ok(contribution)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectCoordinatorPhaseV1 {
    RkgRoundOne,
    RkgRoundTwo,
    RkgNormalize,
    Galois,
    Complete,
}

/// Coordinator state that admits only the unforgeable verified type above.
///
/// The coordinator owns no party secret and stores only eight fixed-size
/// contribution digests.  Advancing across an aggregate-polynomial boundary is
/// kept private until the proof-carrying streaming aggregator is implemented.
#[derive(Clone)]
pub struct ZkAmsMkheDirectCoordinatorV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    phase: DirectCoordinatorPhaseV1,
    prior_round_digest: [u8; 32],
    slots: [Option<[u8; 32]>; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    round_one_secret_lineage: [Option<[u8; 32]>; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    round_one_ephemeral_lineage: [Option<[u8; 32]>; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    active_admission_digest: Option<[u8; 32]>,
    completed_admission_digests: [Option<[u8; 32]>; 3],
    completed_round_digest: Option<[u8; 32]>,
}

impl core::fmt::Debug for ZkAmsMkheDirectCoordinatorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectCoordinatorV1")
            .field("context_digest", &hex::encode(self.context.context_digest))
            .field("phase", &self.phase)
            .field("prior_round_digest", &hex::encode(self.prior_round_digest))
            .field(
                "occupied_slots",
                &self.slots.iter().filter(|slot| slot.is_some()).count(),
            )
            .field(
                "active_admission_digest",
                &self.active_admission_digest.map(hex::encode),
            )
            .field(
                "completed_round_digest",
                &self.completed_round_digest.map(hex::encode),
            )
            .finish()
    }
}

impl ZkAmsMkheDirectCoordinatorV1 {
    /// Start a coordinator without allocating any polynomial or accepting evidence.
    pub fn new(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        context: ZkAmsMkheDirectCeremonyContextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        context.validate(roster)?;
        let phase = match context.target {
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => {
                DirectCoordinatorPhaseV1::RkgRoundOne
            }
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { .. } => DirectCoordinatorPhaseV1::Galois,
        };
        Ok(Self {
            context,
            phase,
            prior_round_digest: context.initial_round_digest,
            slots: [None; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
            round_one_secret_lineage: [None; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
            round_one_ephemeral_lineage: [None; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
            active_admission_digest: None,
            completed_admission_digests: [None; 3],
            completed_round_digest: None,
        })
    }

    fn active_round(&self) -> Result<ZkAmsMkheDirectCeremonyRoundV1, ZkAmsMkheErrorV1> {
        match self.phase {
            DirectCoordinatorPhaseV1::RkgRoundOne => {
                Ok(ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne)
            }
            DirectCoordinatorPhaseV1::RkgRoundTwo => {
                Ok(ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo)
            }
            DirectCoordinatorPhaseV1::RkgNormalize => {
                Ok(ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize)
            }
            DirectCoordinatorPhaseV1::Galois => Ok(ZkAmsMkheDirectCeremonyRoundV1::Galois),
            DirectCoordinatorPhaseV1::Complete => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        }
    }

    /// Admit one proof-verified and authenticated contribution transactionally.
    pub fn admit(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        contribution: &ZkAmsMkheDirectVerifiedContributionV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.context.validate(roster)?;
        let expected_round = self.active_round()?;
        validate_verified_contribution(
            roster,
            self.context,
            expected_round,
            self.prior_round_digest,
            contribution,
        )?;
        let index = usize::from(contribution.party_index);
        if self.slots[index].is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        match self.phase {
            DirectCoordinatorPhaseV1::RkgRoundOne => {
                if self.round_one_secret_lineage[index].is_some()
                    || self.round_one_ephemeral_lineage[index].is_some()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                self.round_one_secret_lineage[index] = Some(contribution.secret_lineage_digest);
                self.round_one_ephemeral_lineage[index] =
                    Some(contribution.rkg_ephemeral_lineage_digest);
            }
            DirectCoordinatorPhaseV1::RkgRoundTwo => {
                if self.round_one_secret_lineage[index] != Some(contribution.secret_lineage_digest)
                    || self.round_one_ephemeral_lineage[index]
                        != Some(contribution.rkg_ephemeral_lineage_digest)
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            }
            DirectCoordinatorPhaseV1::RkgNormalize => {
                if self.round_one_secret_lineage[index] != Some(contribution.secret_lineage_digest)
                    || contribution.rkg_ephemeral_lineage_digest != [0; 32]
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            }
            DirectCoordinatorPhaseV1::Galois => {}
            DirectCoordinatorPhaseV1::Complete => unreachable!(),
        }
        self.slots[index] = Some(contribution.contribution_digest);
        Ok(())
    }

    /// Return the exact ordered set digest once all eight roster slots exist.
    pub fn ordered_contribution_set_digest(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.context.validate(roster)?;
        if self.phase == DirectCoordinatorPhaseV1::Complete {
            return self
                .completed_round_digest
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let round = self.active_round()?;
        ordered_contribution_set_digest(
            roster,
            self.context,
            round,
            self.prior_round_digest,
            &self.slots,
        )
    }

    #[cfg(test)]
    fn advance_after_verified_aggregate(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        aggregate_polynomial_digests: &[[u8; 32]],
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let set_digest = self.ordered_contribution_set_digest(roster)?;
        let active_admission_digest = self
            .active_admission_digest
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let expected_polynomials = match self.phase {
            DirectCoordinatorPhaseV1::RkgRoundOne => 2,
            DirectCoordinatorPhaseV1::RkgRoundTwo
            | DirectCoordinatorPhaseV1::RkgNormalize
            | DirectCoordinatorPhaseV1::Galois => 1,
            DirectCoordinatorPhaseV1::Complete => {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        };
        if aggregate_polynomial_digests.len() != expected_polynomials
            || aggregate_polynomial_digests.contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut hash = Keccak256::new();
        hash.update(DIRECT_AGGREGATE_ROUND_DOMAIN_V1);
        hash.update(&self.context.context_digest);
        hash.update(&set_digest);
        hash.update(&active_admission_digest);
        hash.update(&self.prior_round_digest);
        hash.update(
            &u8::try_from(aggregate_polynomial_digests.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
        for digest in aggregate_polynomial_digests {
            hash.update(digest);
        }
        let aggregate_round_digest = hash.finalize();
        let history_index = match self.phase {
            DirectCoordinatorPhaseV1::RkgRoundOne | DirectCoordinatorPhaseV1::Galois => 0,
            DirectCoordinatorPhaseV1::RkgRoundTwo => 1,
            DirectCoordinatorPhaseV1::RkgNormalize => 2,
            DirectCoordinatorPhaseV1::Complete => unreachable!(),
        };
        let mut next_history = self.completed_admission_digests;
        if next_history[history_index].is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        next_history[history_index] = Some(active_admission_digest);
        let completion = if matches!(
            self.phase,
            DirectCoordinatorPhaseV1::RkgNormalize | DirectCoordinatorPhaseV1::Galois
        ) {
            let history_digest = direct_admission_history_digest(self.context, &next_history)?;
            Some(hash_domain_parts(
                DIRECT_COMPLETION_DOMAIN_V1,
                &[
                    &self.context.context_digest,
                    &aggregate_round_digest,
                    &set_digest,
                    &active_admission_digest,
                    &history_digest,
                ],
            ))
        } else {
            None
        };
        self.completed_admission_digests = next_history;
        self.active_admission_digest = None;
        match self.phase {
            DirectCoordinatorPhaseV1::RkgRoundOne => {
                self.phase = DirectCoordinatorPhaseV1::RkgRoundTwo;
                self.prior_round_digest = aggregate_round_digest;
                self.slots = [None; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
            }
            DirectCoordinatorPhaseV1::RkgRoundTwo => {
                self.phase = DirectCoordinatorPhaseV1::RkgNormalize;
                self.prior_round_digest = aggregate_round_digest;
                self.slots = [None; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
            }
            DirectCoordinatorPhaseV1::RkgNormalize | DirectCoordinatorPhaseV1::Galois => {
                self.phase = DirectCoordinatorPhaseV1::Complete;
                self.completed_round_digest = completion;
            }
            DirectCoordinatorPhaseV1::Complete => unreachable!(),
        }
        Ok(aggregate_round_digest)
    }
}

fn direct_admission_history_digest(
    context: ZkAmsMkheDirectCeremonyContextV1,
    history: &[Option<[u8; 32]>; 3],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    match context.target {
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization
            if history.iter().all(Option::is_some) => {}
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { .. }
            if history[0].is_some() && history[1].is_none() && history[2].is_none() => {}
        _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
    }
    let mut hash = Keccak256::new();
    hash.update(DIRECT_ADMISSION_HISTORY_DOMAIN_V1);
    hash.update(&context.context_digest);
    hash.update(&[context.target.tag()]);
    for (index, digest) in history.iter().enumerate() {
        hash.update(&[index as u8, u8::from(digest.is_some())]);
        hash.update(&digest.unwrap_or([0; 32]));
    }
    Ok(hash.finalize())
}

/// Bounded-memory source of proof-verified public contributions.
///
/// The provider is read twice in canonical roster order.  Returning a
/// [`ZkAmsMkheDirectVerifiedContributionV1`] requires a proof adapter inside
/// this crate; the public API intentionally offers no constructor or decoder
/// that can manufacture that token from caller-supplied digests.
pub trait ZkAmsMkheDirectVerifiedContributionProviderV1 {
    /// Exact number of available contribution records.
    fn contribution_count(&mut self) -> Result<usize, ZkAmsMkheErrorV1>;

    /// Read one proof-verified record at its canonical roster position.
    fn read_verified_contribution(
        &mut self,
        index: usize,
    ) -> Result<ZkAmsMkheDirectVerifiedContributionV1, ZkAmsMkheErrorV1>;
}

/// Admission manifest for one complete ordered direct-contribution set.
///
/// This small object binds both deterministic `a` seeds, the complete context,
/// the exact prior round, all eight authenticated contribution digests, all
/// eight proof-evidence stream digests, and a second provider replay.  It is not
/// an evaluated key and cannot by itself create a runtime key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectAdmittedContributionSetV1 {
    context_digest: [u8; 32],
    common_a_seed: [u8; 32],
    target_a_seed: [u8; 32],
    final_a_seed: [u8; 32],
    secret_lineage_root: [u8; 32],
    ordered_ephemeral_lineage_set_digest: [u8; 32],
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    ordered_contribution_set_digest: [u8; 32],
    ordered_evidence_set_digest: [u8; 32],
    provider_replay_digest: [u8; 32],
    admission_digest: [u8; 32],
}

impl ZkAmsMkheDirectAdmittedContributionSetV1 {
    /// Complete direct-ceremony context digest.
    #[must_use]
    pub const fn context_digest(self) -> [u8; 32] {
        self.context_digest
    }

    /// Deterministic rejection-sampling seed for RKG common `a`.
    #[must_use]
    pub const fn common_a_seed(self) -> [u8; 32] {
        self.common_a_seed
    }

    /// Deterministic rejection-sampling seed for Galois target `a_d`.
    #[must_use]
    pub const fn target_a_seed(self) -> [u8; 32] {
        self.target_a_seed
    }

    /// Seed of the normalized compact relinearization-key `A` polynomial.
    #[must_use]
    pub const fn final_a_seed(self) -> [u8; 32] {
        self.final_a_seed
    }

    /// Root of the eight persistent `s_i` proof-lineage commitments.
    #[must_use]
    pub const fn secret_lineage_root(self) -> [u8; 32] {
        self.secret_lineage_root
    }

    /// Ordered digest of the eight RKG `u_i` commitments (or Galois zero markers).
    #[must_use]
    pub const fn ordered_ephemeral_lineage_set_digest(self) -> [u8; 32] {
        self.ordered_ephemeral_lineage_set_digest
    }

    /// Exact admitted contribution round.
    #[must_use]
    pub const fn round(self) -> ZkAmsMkheDirectCeremonyRoundV1 {
        self.round
    }

    /// Digest of the exact aggregate state preceding this round.
    #[must_use]
    pub const fn prior_round_digest(self) -> [u8; 32] {
        self.prior_round_digest
    }

    /// Digest of all eight authenticated contributions in roster order.
    #[must_use]
    pub const fn ordered_contribution_set_digest(self) -> [u8; 32] {
        self.ordered_contribution_set_digest
    }

    /// Digest of all eight proof-evidence streams in roster order.
    #[must_use]
    pub const fn ordered_evidence_set_digest(self) -> [u8; 32] {
        self.ordered_evidence_set_digest
    }

    /// Digest of the exact second provider replay.
    #[must_use]
    pub const fn provider_replay_digest(self) -> [u8; 32] {
        self.provider_replay_digest
    }

    /// Mandatory identity to bind into aggregate-round and key manifests.
    #[must_use]
    pub const fn admission_digest(self) -> [u8; 32] {
        self.admission_digest
    }
}

/// Admit and replay the complete ordered contribution set for the active round.
///
/// V1 fails before provider I/O because the exact direct proof gate is open.
/// Once that gate is closed, the already implemented inner state machine reads
/// exactly eight records twice, checks order/authentication/context on both
/// passes, retains only fixed-size digests, and commits coordinator state only
/// after both passes succeed.  The same path applies to RKG rounds one and two,
/// public-`A` normalization, and Galois contributions.
pub fn admit_zk_ams_mkhe_direct_contribution_set_v1<P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    coordinator: &mut ZkAmsMkheDirectCoordinatorV1,
    provider: &mut P,
) -> Result<ZkAmsMkheDirectAdmittedContributionSetV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectVerifiedContributionProviderV1,
{
    let audit = zk_ams_mkhe_direct_proof_audit_v1()?;
    if !audit.release_proof_admission_enabled {
        return Err(ZkAmsMkheErrorV1::ReleaseUnavailable);
    }
    admit_ordered_set_inner(roster, coordinator, provider)
}

fn admit_ordered_set_inner<P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    coordinator: &mut ZkAmsMkheDirectCoordinatorV1,
    provider: &mut P,
) -> Result<ZkAmsMkheDirectAdmittedContributionSetV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectVerifiedContributionProviderV1,
{
    let context = coordinator.context;
    context.validate(roster)?;
    let round = coordinator.active_round()?;
    let prior_round_digest = coordinator.prior_round_digest;
    if coordinator.slots.iter().any(Option::is_some)
        || coordinator.active_admission_digest.is_some()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    if provider.contribution_count()? != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    // Stage every mutation.  Omission, replay substitution, authentication
    // failure, or provider error leaves the caller's coordinator byte-for-byte
    // unchanged.
    let mut staged = coordinator.clone();
    let mut contribution_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    let mut evidence_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    let mut ephemeral_lineage_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    for index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let contribution = provider.read_verified_contribution(index)?;
        if usize::from(contribution.party_index) != index {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        staged.admit(roster, &contribution)?;
        contribution_digests[index] = contribution.contribution_digest;
        evidence_digests[index] = contribution.evidence_set_digest;
        ephemeral_lineage_digests[index] = contribution.rkg_ephemeral_lineage_digest;
    }
    let ordered_contribution_set_digest = staged.ordered_contribution_set_digest(roster)?;
    let ordered_evidence_set_digest = ordered_evidence_set_digest(
        roster,
        context,
        round,
        prior_round_digest,
        &evidence_digests,
    )?;
    let ordered_ephemeral_lineage_set_digest = ordered_lineage_set_digest(
        roster,
        context,
        round,
        prior_round_digest,
        &ephemeral_lineage_digests,
    );

    let mut replay = Keccak256::new();
    replay.update(DIRECT_ADMITTED_SET_DOMAIN_V1);
    replay.update(&context.context_digest);
    replay.update(&ordered_contribution_set_digest);
    replay.update(&ordered_evidence_set_digest);
    for index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let contribution = provider.read_verified_contribution(index)?;
        if usize::from(contribution.party_index) != index
            || contribution.contribution_digest != contribution_digests[index]
            || contribution.evidence_set_digest != evidence_digests[index]
            || contribution.rkg_ephemeral_lineage_digest != ephemeral_lineage_digests[index]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_verified_contribution(roster, context, round, prior_round_digest, &contribution)?;
        replay.update(&[index as u8]);
        replay.update(&contribution.contribution_digest);
        replay.update(&contribution.evidence_set_digest);
    }
    let provider_replay_digest = replay.finalize();
    let admission_digest = hash_domain_parts(
        DIRECT_ADMITTED_SET_DOMAIN_V1,
        &[
            &context.context_digest,
            &context.common_a_seed,
            &context.target_a_seed,
            &context.final_a_seed,
            &context.secret_lineage_root,
            &ordered_ephemeral_lineage_set_digest,
            &[round.tag()],
            &prior_round_digest,
            &ordered_contribution_set_digest,
            &ordered_evidence_set_digest,
            &provider_replay_digest,
        ],
    );
    let admitted = ZkAmsMkheDirectAdmittedContributionSetV1 {
        context_digest: context.context_digest,
        common_a_seed: context.common_a_seed,
        target_a_seed: context.target_a_seed,
        final_a_seed: context.final_a_seed,
        secret_lineage_root: context.secret_lineage_root,
        ordered_ephemeral_lineage_set_digest,
        round,
        prior_round_digest,
        ordered_contribution_set_digest,
        ordered_evidence_set_digest,
        provider_replay_digest,
        admission_digest,
    };
    staged.active_admission_digest = Some(admission_digest);
    *coordinator = staged;
    Ok(admitted)
}

fn ordered_lineage_set_digest(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    lineage_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.direct-collective-ordered-u-lineage-set");
    hash.update(&context.context_digest);
    hash.update(&context.secret_lineage_root);
    hash.update(&[round.tag()]);
    hash.update(&prior_round_digest);
    for (index, (participant, digest)) in roster
        .participants()
        .iter()
        .zip(lineage_digests)
        .enumerate()
    {
        hash.update(&[index as u8]);
        hash.update(&participant.party().to_bytes());
        hash.update(digest);
    }
    hash.finalize()
}

fn ordered_evidence_set_digest(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    evidence_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if evidence_digests.contains(&[0; 32]) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(DIRECT_ORDERED_EVIDENCE_SET_DOMAIN_V1);
    hash.update(&context.context_digest);
    hash.update(&context.common_a_seed);
    hash.update(&context.target_a_seed);
    hash.update(&context.final_a_seed);
    hash.update(&[round.tag()]);
    hash.update(&prior_round_digest);
    for (index, (participant, digest)) in roster
        .participants()
        .iter()
        .zip(evidence_digests)
        .enumerate()
    {
        hash.update(&[index as u8]);
        hash.update(&participant.party().to_bytes());
        hash.update(digest);
    }
    Ok(hash.finalize())
}

fn ordered_contribution_set_digest(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    slots: &[Option<[u8; 32]>; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if prior_round_digest == [0; 32] || slots.iter().any(Option::is_none) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(DIRECT_ORDERED_SET_DOMAIN_V1);
    hash.update(&context.context_digest);
    hash.update(&[round.tag()]);
    hash.update(&prior_round_digest);
    hash.update(
        &u8::try_from(slots.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?
            .to_be_bytes(),
    );
    for (index, (participant, digest)) in roster.participants().iter().zip(slots).enumerate() {
        hash.update(
            &u8::try_from(index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?
                .to_be_bytes(),
        );
        hash.update(&participant.party().to_bytes());
        hash.update(&digest.ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?);
    }
    Ok(hash.finalize())
}

fn validate_verified_contribution(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    expected_round: ZkAmsMkheDirectCeremonyRoundV1,
    expected_prior_round_digest: [u8; 32],
    contribution: &ZkAmsMkheDirectVerifiedContributionV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let index = usize::from(contribution.party_index);
    let participant = roster
        .participants()
        .get(index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    if contribution.version != MKHE_VERSION_V1
        || contribution.context_digest != context.context_digest
        || contribution.round != expected_round
        || contribution.prior_round_digest != expected_prior_round_digest
        || contribution.party != participant.party()
        || contribution.secret_lineage_digest != context.secret_lineage_digests[index]
        || contribution.relation_use_digest == [0; 32]
        || contribution.proof_digest == [0; 32]
        || contribution.evidence_set_digest == [0; 32]
        || contribution.authentication.party != contribution.party
        || contribution.authentication.public_key != participant.authentication_public_key()
        || contribution.contribution_digest == [0; 32]
        || contribution.contribution_digest != direct_contribution_digest(contribution)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    match contribution.round {
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne
        | ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo
            if contribution.rkg_ephemeral_lineage_digest == [0; 32] =>
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize | ZkAmsMkheDirectCeremonyRoundV1::Galois
            if contribution.rkg_ephemeral_lineage_digest != [0; 32] =>
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        _ => {}
    }
    validate_contribution_payload(context, contribution)?;
    let statement = direct_contribution_auth_statement(contribution)?;
    contribution
        .authentication
        .verify(DIRECT_CONTRIBUTION_AUTH_DOMAIN_V1, statement)
}

fn validate_contribution_payload(
    context: ZkAmsMkheDirectCeremonyContextV1,
    contribution: &ZkAmsMkheDirectVerifiedContributionV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected = |receipt: ZkAmsMkheDirectPolynomialStreamReceiptV1,
                    role: ZkAmsMkheDirectPolynomialRoleV1| {
        let resources = zk_ams_mkhe_direct_resource_certificate_v1()?;
        if receipt.context_digest != context.context_digest
            || receipt.round != contribution.round
            || receipt.party_index != contribution.party_index
            || receipt.party != contribution.party
            || receipt.role != role
            || receipt.polynomial_digest == [0; 32]
            || receipt.canonical_bytes != resources.polynomial_bytes
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    };
    match contribution.payload {
        DirectContributionPayloadV1::RkgRoundOne { h0, h1 }
            if contribution.round == ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne =>
        {
            expected(h0, ZkAmsMkheDirectPolynomialRoleV1::RkgH0)?;
            expected(h1, ZkAmsMkheDirectPolynomialRoleV1::RkgH1)
        }
        DirectContributionPayloadV1::RkgRoundTwo { k }
            if contribution.round == ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo =>
        {
            expected(k, ZkAmsMkheDirectPolynomialRoleV1::RkgK)
        }
        DirectContributionPayloadV1::RkgNormalize { normalization }
            if contribution.round == ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize =>
        {
            expected(
                normalization,
                ZkAmsMkheDirectPolynomialRoleV1::RkgNormalization,
            )
        }
        DirectContributionPayloadV1::Galois { b }
            if contribution.round == ZkAmsMkheDirectCeremonyRoundV1::Galois =>
        {
            expected(b, ZkAmsMkheDirectPolynomialRoleV1::GaloisB)
        }
        _ => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
    }
}

fn direct_contribution_auth_statement(
    contribution: &ZkAmsMkheDirectVerifiedContributionV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut frame = direct_contribution_frame(contribution)?;
    frame.extend_from_slice(&contribution.proof_digest);
    frame.extend_from_slice(&contribution.evidence_set_digest);
    Ok(hash_domain_parts(DIRECT_CONTRIBUTION_DOMAIN_V1, &[&frame]))
}

fn direct_contribution_digest(
    contribution: &ZkAmsMkheDirectVerifiedContributionV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let statement = direct_contribution_auth_statement(contribution)?;
    Ok(hash_domain_parts(
        DIRECT_CONTRIBUTION_DOMAIN_V1,
        &[
            &statement,
            &contribution.authentication.public_key,
            &contribution.authentication.signature,
        ],
    ))
}

fn direct_contribution_frame(
    contribution: &ZkAmsMkheDirectVerifiedContributionV1,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    if contribution.context_digest == [0; 32]
        || contribution.prior_round_digest == [0; 32]
        || contribution.relation_use_digest == [0; 32]
        || contribution.proof_digest == [0; 32]
        || contribution.evidence_set_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut frame = Vec::with_capacity(320);
    frame.push(contribution.version);
    frame.extend_from_slice(&contribution.context_digest);
    frame.push(contribution.round.tag());
    frame.extend_from_slice(&contribution.prior_round_digest);
    frame.push(contribution.party_index);
    frame.extend_from_slice(&contribution.party.to_bytes());
    frame.extend_from_slice(&contribution.secret_lineage_digest);
    frame.extend_from_slice(&contribution.rkg_ephemeral_lineage_digest);
    frame.extend_from_slice(&contribution.relation_use_digest);
    match contribution.payload {
        DirectContributionPayloadV1::RkgRoundOne { h0, h1 } => {
            frame.push(1);
            frame.extend_from_slice(&h0.polynomial_digest);
            frame.extend_from_slice(&h1.polynomial_digest);
        }
        DirectContributionPayloadV1::RkgRoundTwo { k } => {
            frame.push(2);
            frame.extend_from_slice(&k.polynomial_digest);
        }
        DirectContributionPayloadV1::RkgNormalize { normalization } => {
            frame.push(3);
            frame.extend_from_slice(&normalization.polynomial_digest);
        }
        DirectContributionPayloadV1::Galois { b } => {
            frame.push(4);
            frame.extend_from_slice(&b.polynomial_digest);
        }
    }
    Ok(frame)
}

#[cfg(test)]
fn authenticate_test_verified_contribution<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    round: ZkAmsMkheDirectCeremonyRoundV1,
    prior_round_digest: [u8; 32],
    party_index: usize,
    payload: DirectContributionPayloadV1,
    proof_digest: [u8; 32],
    secret_lineage_digest: [u8; 32],
    rkg_ephemeral_lineage_digest: [u8; 32],
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheDirectVerifiedContributionV1, ZkAmsMkheErrorV1> {
    // Test-only stand-in. Production deliberately has no corresponding entry
    // point until a reviewed proof adapter returns an unforgeable token.
    let participant = roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    if participant.party() != party_secret.party()? || proof_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut contribution = ZkAmsMkheDirectVerifiedContributionV1 {
        version: MKHE_VERSION_V1,
        context_digest: context.context_digest,
        round,
        prior_round_digest,
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        party: participant.party(),
        secret_lineage_digest,
        rkg_ephemeral_lineage_digest,
        payload,
        relation_use_digest: hash_domain_parts(
            DIRECT_RELATION_STATEMENT_DOMAIN_V1,
            &[&context.context_digest, &proof_digest],
        ),
        proof_digest,
        evidence_set_digest: hash_domain_parts(
            DIRECT_ORDERED_EVIDENCE_SET_DOMAIN_V1,
            &[&context.context_digest, &proof_digest],
        ),
        authentication: ArtifactAuthentication {
            version: 0,
            party: participant.party(),
            public_key: [0; 33],
            signature: [0; 65],
        },
        contribution_digest: [0; 32],
    };
    let statement = direct_contribution_auth_statement(&contribution)?;
    contribution.authentication = party_secret.authenticate_artifact(
        DIRECT_CONTRIBUTION_AUTH_DOMAIN_V1,
        statement,
        random,
    )?;
    contribution.contribution_digest = direct_contribution_digest(&contribution)?;
    // Do not validate the payload here.  Adversarial tests deliberately need
    // correctly authenticated malformed tokens to prove that coordinator
    // admission, rather than the test fixture, rejects them transactionally.
    Ok(contribution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{MaskedRelaxedRandomErrorV1, sponge::keccak256};

    struct KatRandom {
        label: Vec<u8>,
        counter: u64,
    }

    impl KatRandom {
        fn new(label: &[u8]) -> Self {
            Self {
                label: label.to_vec(),
                counter: 0,
            }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for KatRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            let mut written = 0;
            while written < destination.len() {
                let mut frame = self.label.clone();
                frame.extend_from_slice(&self.counter.to_be_bytes());
                let block = keccak256(&frame);
                let take = (destination.len() - written).min(block.len());
                destination[written..written + take].copy_from_slice(&block[..take]);
                self.counter = self.counter.wrapping_add(1);
                written += take;
            }
            Ok(())
        }
    }

    fn roster_fixture(
        label: &[u8],
    ) -> (
        ZkAmsMkheGovernedActiveRosterV1,
        Vec<ZkAmsMkheActivePartySecretV1>,
        KatRandom,
    ) {
        let mut random = KatRandom::new(label);
        let mut secrets = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|_| ZkAmsMkheActivePartySecretV1::generate(&mut random).unwrap())
            .collect::<Vec<_>>();
        secrets.sort_by_key(|secret| secret.party().unwrap());
        let references: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            core::array::from_fn(|index| &secrets[index]);
        let roster = ZkAmsMkheGovernedActiveRosterV1::new(41, references, &mut random).unwrap();
        (roster, secrets, random)
    }

    fn context_fixture(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        target: ZkAmsMkheDirectEvaluatedKeyTargetV1,
    ) -> ZkAmsMkheDirectCeremonyContextV1 {
        let secret_lineage =
            core::array::from_fn(|index| keccak256(format!("secret-lineage-{index}").as_bytes()));
        ZkAmsMkheDirectCeremonyContextV1::new(
            roster,
            keccak256(b"direct-ceremony-parent-transcript"),
            keccak256(b"direct-ceremony-collective-public-key"),
            secret_lineage,
            target,
            3,
        )
        .unwrap()
    }

    fn test_complete_evaluated_key_set_admission(
        context: ZkAmsMkheDirectCeremonyContextV1,
    ) -> ZkAmsMkheDirectEvaluatedKeySetAdmissionV1 {
        let profile = release_profile_v1();
        let noise = zk_ams_mkhe_direct_noise_certificate_v1().unwrap();
        let resources = zk_ams_mkhe_direct_resource_certificate_v1().unwrap();
        let mut admission = ZkAmsMkheDirectEvaluatedKeySetAdmissionV1 {
            version: MKHE_VERSION_V1,
            profile_digest: profile.digest().unwrap(),
            roster_digest: context.roster_digest,
            key_material_digest: context.key_material_digest,
            epoch: context.epoch,
            transcript_digest: context.transcript_digest,
            collective_public_key_digest: context.collective_public_key_digest,
            secret_lineage_root: context.secret_lineage_root,
            evaluated_key_count: (ZK_AMS_T256_GALOIS_KEY_COUNT_V1 + 1) as u8,
            gadget_digit_count: profile.gadget_digits as u8,
            ordered_complete_key_set_digest: keccak256(b"test-complete-1216-digit-key-set"),
            exact_proof_admission_digest: keccak256(b"test-exact-proof-admission"),
            direct_algebra_digest: noise.certificate_digest,
            resource_certificate_digest: resources.certificate_digest,
            admission_digest: [0; 32],
        };
        admission.admission_digest = direct_evaluated_key_set_admission_digest(admission);
        admission.validate().unwrap();
        admission
    }

    #[test]
    fn proof_audit_is_fail_closed_and_even_weight_entropy_is_not_an_extractor() {
        let audit = zk_ams_mkhe_direct_proof_audit_v1().unwrap();
        assert_eq!(audit.sparse_challenge_weight, 60);
        assert!(audit.challenge_weight_is_even);
        assert!(!audit.challenge_difference_unit_guaranteed);
        assert!(!audit.exact_extractor_pinned);
        assert!(!audit.structured_module_sis_reduction_pinned);
        assert!(!audit.release_proof_admission_enabled);

        // In R = F_17[X]/(X^4+1), both challenges below have exact even
        // weight two. Their difference evaluates to zero at the ring root 2,
        // hence is a zero divisor and cannot be divided out by an extractor.
        // This is deliberately test-only algebra, not a production attack API.
        let challenge_left = [-1_i64, -1, 0, 0];
        let challenge_right = [1_i64, 0, -1, 0];
        let difference =
            core::array::from_fn::<_, 4, _>(|index| challenge_left[index] - challenge_right[index]);
        let evaluate = |polynomial: &[i64; 4], point: i64| {
            polynomial
                .iter()
                .rev()
                .fold(0_i64, |accumulator, coefficient| {
                    (accumulator * point + coefficient).rem_euclid(17)
                })
        };
        assert_eq!((2_i64.pow(4) + 1) % 17, 0);
        assert_eq!(evaluate(&difference, 2), 0);
        assert_ne!(difference, [0; 4]);
    }

    #[test]
    fn release_noise_and_resource_accounting_are_exact_and_fail_closed() {
        let noise = zk_ams_mkhe_direct_noise_certificate_v1().unwrap();
        assert_eq!(noise.aggregate_error_bound, 16);
        assert_eq!(noise.aggregate_secret_bound, 8);
        assert_eq!(noise.relinearization_intrinsic_error_bound, 33_554_464);
        assert_eq!(noise.galois_intrinsic_error_bound, 16);
        assert_eq!(noise.relinearization_intrinsic_error_bits, 26);

        let resources = zk_ams_mkhe_direct_resource_certificate_v1().unwrap();
        assert_eq!(resources.rns_limb_count, 38);
        assert_eq!(resources.chunk_bytes, 1_048_576);
        assert_eq!(resources.polynomial_bytes, 39_845_888);
        assert_eq!(resources.rkg_round_one_party_payload_bytes, 79_691_776);
        assert_eq!(resources.one_polynomial_record_headroom_bytes, 27_262_976);
        assert_eq!(resources.full_ceremony_ring_multiplications, 10_944);
        assert_eq!(resources.normalized_evaluated_key_wire_bytes, 1_514_144_113);
        assert_eq!(
            resources.two_stored_polynomial_payload_lower_bound_bytes,
            3_028_287_792
        );
        assert!(resources.one_polynomial_record_fits_round);
        assert!(!resources.combined_rkg_round_one_record_fits_round);
        assert!(resources.normalized_evaluated_key_fits_ceiling);
        assert!(!resources.two_stored_polynomial_payload_fits_ceiling);
        assert!(!resources.proof_streaming_certified);
        assert!(!resources.release_gate);
    }

    #[test]
    fn old_cks_noise_schedule_is_inapplicable_only_with_complete_direct_key_admission() {
        let without_admission = zk_ams_mkhe_direct_noise_integration_certificate_v1().unwrap();
        assert_eq!(
            without_admission.direct_relinearization_intrinsic_error_bound,
            33_554_464
        );
        assert_eq!(
            without_admission.direct_relinearization_intrinsic_error_bits,
            26
        );
        assert_eq!(without_admission.direct_galois_intrinsic_error_bound, 16);
        assert_eq!(
            without_admission.direct_p_scaled_relinearization_residual_bits,
            282
        );
        assert_eq!(
            without_admission.direct_hybrid_key_switch_residual_bits,
            365
        );
        assert_eq!(without_admission.ingress_cks_residual_bits, 411);
        assert_eq!(
            without_admission.direct_composed_rotation_residual_bits,
            412
        );
        assert_eq!(without_admission.direct_mapped_fresh_residual_bits, 704);
        assert_eq!(
            without_admission.direct_linear_accumulator_residual_bits,
            979
        );
        assert_eq!(without_admission.direct_cross_product_residual_bits, 1_703);
        assert_eq!(without_admission.direct_equation_six_residual_bits, 1_705);
        assert_eq!(without_admission.direct_level_one_residual_bits, 1_981);
        assert_eq!(
            without_admission.direct_encrypted_commitment_residual_bits,
            1_272
        );
        assert_eq!(
            without_admission.direct_decryption_smudge_quotient_bits,
            1_855
        );
        assert_eq!(
            without_admission.direct_public_decryption_final_residual_bits,
            2_115
        );
        assert_eq!(without_admission.direct_public_decryption_margin_bits, 164);
        assert_eq!(
            without_admission.direct_internal_decode_residual_bits,
            1_981
        );
        assert_eq!(without_admission.direct_internal_decode_margin_bits, 298);
        assert_eq!(without_admission.old_corrected_switch_residual_bits, 494);
        assert_eq!(
            without_admission.old_corrected_level_one_residual_bits,
            2_153
        );
        assert_eq!(without_admission.old_corrected_final_residual_bits, 2_287);
        assert!(!without_admission.direct_evaluated_key_set_admitted);
        assert!(!without_admission.old_evaluated_key_cks_smudge_blocker_eliminated);
        assert!(without_admission.old_cks_494_2287_schedule_applies_to_evaluated_keys);
        assert!(without_admission.ingress_cks_noise_accounting_remains_distinct);
        assert!(without_admission.direct_phase23_noise_schedule_pinned);
        assert!(!without_admission.direct_phase23_noise_schedule_applies);
        assert!(!without_admission.release_gate);

        let (roster, _, _) = roster_fixture(b"direct-noise-integration");
        let context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        let admission = test_complete_evaluated_key_set_admission(context);
        let with_admission =
            zk_ams_mkhe_direct_noise_integration_for_admitted_keys_v1(&admission).unwrap();
        assert!(with_admission.direct_evaluated_key_set_admitted);
        assert!(with_admission.old_evaluated_key_cks_smudge_blocker_eliminated);
        assert!(!with_admission.old_cks_494_2287_schedule_applies_to_evaluated_keys);
        assert!(with_admission.ingress_cks_noise_accounting_remains_distinct);
        assert_eq!(
            with_admission.direct_algebra_digest,
            without_admission.direct_algebra_digest
        );
        assert_ne!(
            with_admission.direct_evaluated_key_set_admission_digest,
            [0; 32]
        );
        assert_ne!(
            with_admission.certificate_digest,
            without_admission.certificate_digest
        );
        assert!(with_admission.direct_phase23_noise_schedule_pinned);
        assert!(with_admission.direct_phase23_noise_schedule_applies);
        assert!(!with_admission.release_gate);

        let mut substituted = admission;
        substituted.ordered_complete_key_set_digest[0] ^= 1;
        assert!(zk_ams_mkhe_direct_noise_integration_for_admitted_keys_v1(&substituted).is_err());
    }

    #[test]
    fn context_binds_every_coordinate_and_all_three_a_derivations() {
        let (roster, _, _) = roster_fixture(b"direct-context");
        let base = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        assert_eq!(base.evaluated_key_ordinal(), 0);
        assert_eq!(base.galois_exponent(), 0);
        assert_ne!(base.common_a_seed(), base.target_a_seed());
        assert_ne!(base.common_a_seed(), base.final_a_seed());
        assert_ne!(base.target_a_seed(), base.final_a_seed());

        let galois = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index: 0 },
        );
        assert_eq!(galois.evaluated_key_ordinal(), 1);
        assert_ne!(galois.galois_exponent(), 0);
        assert_ne!(galois.digest(), base.digest());
        assert_ne!(galois.common_a_seed(), base.common_a_seed());

        assert!(
            ZkAmsMkheDirectCeremonyContextV1::new(
                &roster,
                [0; 32],
                keccak256(b"key"),
                core::array::from_fn(|index| {
                    keccak256(format!("secret-lineage-{index}").as_bytes())
                }),
                ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
                0,
            )
            .is_err()
        );
        let mut lineages =
            core::array::from_fn(|index| keccak256(format!("secret-lineage-{index}").as_bytes()));
        lineages[5] = [0; 32];
        assert!(
            ZkAmsMkheDirectCeremonyContextV1::new(
                &roster,
                keccak256(b"transcript"),
                keccak256(b"key"),
                lineages,
                ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
                0,
            )
            .is_err()
        );
        assert!(
            ZkAmsMkheDirectCeremonyContextV1::new(
                &roster,
                keccak256(b"transcript"),
                keccak256(b"key"),
                core::array::from_fn(|index| {
                    keccak256(format!("secret-lineage-{index}").as_bytes())
                }),
                ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index: 31 },
                0,
            )
            .is_err()
        );
    }

    fn stream_digest(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        context: ZkAmsMkheDirectCeremonyContextV1,
        round: ZkAmsMkheDirectCeremonyRoundV1,
        party_index: usize,
        role: ZkAmsMkheDirectPolynomialRoleV1,
        limbs: &[Vec<u64>],
    ) -> [u8; 32] {
        let mut hash = Keccak256::new();
        hash.update(DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1);
        hash.update(&context.context_digest);
        hash.update(&[round.tag(), role.tag()]);
        hash.update(&[party_index as u8]);
        hash.update(&roster.participants()[party_index].party().to_bytes());
        hash.update(&(release_profile_v1().ring_degree as u32).to_be_bytes());
        hash.update(&[release_profile_v1().moduli.len() as u8]);
        for (index, residues) in limbs.iter().enumerate() {
            hash.update(&[index as u8]);
            hash.update(&release_profile_v1().moduli[index].to_be_bytes());
            for residue in residues {
                hash.update(&residue.to_be_bytes());
            }
        }
        hash.finalize()
    }

    #[test]
    fn canonical_limb_stream_is_transactional_and_rejects_all_shape_splices() {
        let (roster, _, _) = roster_fixture(b"direct-stream");
        let context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        let profile = release_profile_v1();
        let limbs = profile
            .moduli
            .iter()
            .enumerate()
            .map(|(limb, modulus)| {
                (0..profile.ring_degree)
                    .map(|coefficient| (limb as u64 + coefficient as u64) % modulus)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let expected = stream_digest(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            0,
            ZkAmsMkheDirectPolynomialRoleV1::RkgK,
            &limbs,
        );
        let mut stream = ZkAmsMkheDirectPolynomialStreamV1::begin(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            0,
            ZkAmsMkheDirectPolynomialRoleV1::RkgK,
            expected,
        )
        .unwrap();
        assert!(stream.admit_limb(1, &limbs[1]).is_err());
        assert_eq!(stream.next_limb, 0);
        assert_eq!(stream.canonical_bytes, 0);
        assert!(
            stream
                .admit_limb(0, &limbs[0][..profile.ring_degree - 1])
                .is_err()
        );
        assert_eq!(stream.next_limb, 0);
        let mut noncanonical = limbs[0].clone();
        noncanonical[17] = profile.moduli[0];
        assert!(stream.admit_limb(0, &noncanonical).is_err());
        assert_eq!(stream.next_limb, 0);
        for (index, limb) in limbs.iter().enumerate() {
            stream.admit_limb(index, limb).unwrap();
        }
        assert_eq!(stream.finish().unwrap().polynomial_digest(), expected);

        let wrong = ZkAmsMkheDirectPolynomialStreamV1::begin(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            0,
            ZkAmsMkheDirectPolynomialRoleV1::RkgK,
            keccak256(b"wrong-polynomial"),
        )
        .unwrap();
        assert!(wrong.finish().is_err());
        assert!(
            ZkAmsMkheDirectPolynomialStreamV1::begin(
                &roster,
                context,
                ZkAmsMkheDirectCeremonyRoundV1::Galois,
                0,
                ZkAmsMkheDirectPolynomialRoleV1::GaloisB,
                expected,
            )
            .is_err()
        );
    }

    fn fake_receipt(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        context: ZkAmsMkheDirectCeremonyContextV1,
        round: ZkAmsMkheDirectCeremonyRoundV1,
        party_index: usize,
        role: ZkAmsMkheDirectPolynomialRoleV1,
        label: &[u8],
    ) -> ZkAmsMkheDirectPolynomialStreamReceiptV1 {
        ZkAmsMkheDirectPolynomialStreamReceiptV1 {
            context_digest: context.context_digest,
            round,
            party_index: party_index as u8,
            party: roster.participants()[party_index].party(),
            role,
            polynomial_digest: keccak256(label),
            canonical_bytes: zk_ams_mkhe_direct_resource_certificate_v1()
                .unwrap()
                .polynomial_bytes,
        }
    }

    fn test_ephemeral_lineage(
        context: ZkAmsMkheDirectCeremonyContextV1,
        party_index: usize,
    ) -> [u8; 32] {
        hash_domain_parts(
            b"iroha.zk-ams.v1.mkhe.direct-test-ephemeral-lineage",
            &[&context.context_digest, &[party_index as u8]],
        )
    }

    fn make_round_one_contributions(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        secrets: &[ZkAmsMkheActivePartySecretV1],
        context: ZkAmsMkheDirectCeremonyContextV1,
        random: &mut KatRandom,
    ) -> Vec<ZkAmsMkheDirectVerifiedContributionV1> {
        (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|party_index| {
                let payload = DirectContributionPayloadV1::RkgRoundOne {
                    h0: fake_receipt(
                        roster,
                        context,
                        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
                        party_index,
                        ZkAmsMkheDirectPolynomialRoleV1::RkgH0,
                        format!("h0-{party_index}").as_bytes(),
                    ),
                    h1: fake_receipt(
                        roster,
                        context,
                        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
                        party_index,
                        ZkAmsMkheDirectPolynomialRoleV1::RkgH1,
                        format!("h1-{party_index}").as_bytes(),
                    ),
                };
                authenticate_test_verified_contribution(
                    roster,
                    context,
                    ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
                    context.initial_round_digest,
                    party_index,
                    payload,
                    keccak256(format!("proof-{party_index}").as_bytes()),
                    context.secret_lineage_digests[party_index],
                    test_ephemeral_lineage(context, party_index),
                    &secrets[party_index],
                    random,
                )
                .unwrap()
            })
            .collect()
    }

    fn make_round_two_contributions(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        secrets: &[ZkAmsMkheActivePartySecretV1],
        context: ZkAmsMkheDirectCeremonyContextV1,
        prior_round_digest: [u8; 32],
        random: &mut KatRandom,
    ) -> Vec<ZkAmsMkheDirectVerifiedContributionV1> {
        (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|party_index| {
                authenticate_test_verified_contribution(
                    roster,
                    context,
                    ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
                    prior_round_digest,
                    party_index,
                    DirectContributionPayloadV1::RkgRoundTwo {
                        k: fake_receipt(
                            roster,
                            context,
                            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
                            party_index,
                            ZkAmsMkheDirectPolynomialRoleV1::RkgK,
                            format!("k-{party_index}").as_bytes(),
                        ),
                    },
                    keccak256(format!("proof-k-{party_index}").as_bytes()),
                    context.secret_lineage_digests[party_index],
                    test_ephemeral_lineage(context, party_index),
                    &secrets[party_index],
                    random,
                )
                .unwrap()
            })
            .collect()
    }

    fn make_normalization_contributions(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        secrets: &[ZkAmsMkheActivePartySecretV1],
        context: ZkAmsMkheDirectCeremonyContextV1,
        prior_round_digest: [u8; 32],
        random: &mut KatRandom,
    ) -> Vec<ZkAmsMkheDirectVerifiedContributionV1> {
        (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|party_index| {
                authenticate_test_verified_contribution(
                    roster,
                    context,
                    ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
                    prior_round_digest,
                    party_index,
                    DirectContributionPayloadV1::RkgNormalize {
                        normalization: fake_receipt(
                            roster,
                            context,
                            ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
                            party_index,
                            ZkAmsMkheDirectPolynomialRoleV1::RkgNormalization,
                            format!("normalization-{party_index}").as_bytes(),
                        ),
                    },
                    keccak256(format!("proof-normalization-{party_index}").as_bytes()),
                    context.secret_lineage_digests[party_index],
                    [0; 32],
                    &secrets[party_index],
                    random,
                )
                .unwrap()
            })
            .collect()
    }

    fn make_galois_contributions(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        secrets: &[ZkAmsMkheActivePartySecretV1],
        context: ZkAmsMkheDirectCeremonyContextV1,
        random: &mut KatRandom,
    ) -> Vec<ZkAmsMkheDirectVerifiedContributionV1> {
        (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|party_index| {
                authenticate_test_verified_contribution(
                    roster,
                    context,
                    ZkAmsMkheDirectCeremonyRoundV1::Galois,
                    context.initial_round_digest,
                    party_index,
                    DirectContributionPayloadV1::Galois {
                        b: fake_receipt(
                            roster,
                            context,
                            ZkAmsMkheDirectCeremonyRoundV1::Galois,
                            party_index,
                            ZkAmsMkheDirectPolynomialRoleV1::GaloisB,
                            format!("galois-b-{party_index}").as_bytes(),
                        ),
                    },
                    keccak256(format!("proof-galois-{party_index}").as_bytes()),
                    context.secret_lineage_digests[party_index],
                    [0; 32],
                    &secrets[party_index],
                    random,
                )
                .unwrap()
            })
            .collect()
    }

    #[derive(Clone)]
    struct VecContributionProvider {
        records: Vec<ZkAmsMkheDirectVerifiedContributionV1>,
        count: usize,
        reads: usize,
        replay_substitution: Option<(usize, ZkAmsMkheDirectVerifiedContributionV1)>,
    }

    impl VecContributionProvider {
        fn new(records: Vec<ZkAmsMkheDirectVerifiedContributionV1>) -> Self {
            let count = records.len();
            Self {
                records,
                count,
                reads: 0,
                replay_substitution: None,
            }
        }
    }

    impl ZkAmsMkheDirectVerifiedContributionProviderV1 for VecContributionProvider {
        fn contribution_count(&mut self) -> Result<usize, ZkAmsMkheErrorV1> {
            Ok(self.count)
        }

        fn read_verified_contribution(
            &mut self,
            index: usize,
        ) -> Result<ZkAmsMkheDirectVerifiedContributionV1, ZkAmsMkheErrorV1> {
            let replay = self.reads >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1;
            self.reads += 1;
            if replay
                && let Some((substitution_index, contribution)) = &self.replay_substitution
                && *substitution_index == index
            {
                return Ok(contribution.clone());
            }
            self.records
                .get(index)
                .cloned()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        }
    }

    #[test]
    fn ordered_provider_replay_is_bounded_and_release_admission_is_fail_closed() {
        let (roster, secrets, mut random) = roster_fixture(b"direct-provider");
        let context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        let records = make_round_one_contributions(&roster, &secrets, context, &mut random);

        let mut unavailable_coordinator =
            ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
        let mut unavailable = VecContributionProvider::new(records.clone());
        assert_eq!(
            admit_zk_ams_mkhe_direct_contribution_set_v1(
                &roster,
                &mut unavailable_coordinator,
                &mut unavailable,
            ),
            Err(ZkAmsMkheErrorV1::ReleaseUnavailable)
        );
        assert_eq!(unavailable.reads, 0);
        assert!(unavailable_coordinator.slots.iter().all(Option::is_none));

        let mut coordinator = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
        let mut provider = VecContributionProvider::new(records.clone());
        let admitted = admit_ordered_set_inner(&roster, &mut coordinator, &mut provider).unwrap();
        assert_eq!(provider.reads, 16);
        assert!(coordinator.slots.iter().all(Option::is_some));
        assert_eq!(admitted.context_digest(), context.digest());
        assert_eq!(admitted.common_a_seed(), context.common_a_seed());
        assert_eq!(admitted.target_a_seed(), context.target_a_seed());
        assert_eq!(admitted.final_a_seed(), context.final_a_seed());
        assert_eq!(
            admitted.secret_lineage_root(),
            context.secret_lineage_root()
        );
        assert_ne!(admitted.ordered_contribution_set_digest(), [0; 32]);
        assert_ne!(admitted.ordered_evidence_set_digest(), [0; 32]);
        assert_ne!(admitted.provider_replay_digest(), [0; 32]);
        assert_ne!(admitted.admission_digest(), [0; 32]);

        let reject = |mut source: VecContributionProvider| {
            let mut candidate = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
            assert!(admit_ordered_set_inner(&roster, &mut candidate, &mut source).is_err());
            assert!(candidate.slots.iter().all(Option::is_none));
            source.reads
        };

        let omitted = VecContributionProvider::new(records[..7].to_vec());
        assert_eq!(reject(omitted), 0);

        let mut reordered_records = records.clone();
        reordered_records.swap(0, 1);
        let reordered = VecContributionProvider::new(reordered_records);
        assert_eq!(reject(reordered), 1);

        let mut duplicated_records = records.clone();
        duplicated_records[1] = duplicated_records[0].clone();
        let duplicated = VecContributionProvider::new(duplicated_records);
        assert_eq!(reject(duplicated), 2);

        let mut proof_splice_records = records.clone();
        proof_splice_records[2].proof_digest = records[3].proof_digest;
        let proof_splice = VecContributionProvider::new(proof_splice_records);
        assert_eq!(reject(proof_splice), 3);

        let mut relation_use_splice_records = records.clone();
        relation_use_splice_records[3].relation_use_digest = records[4].relation_use_digest;
        let relation_use_splice = VecContributionProvider::new(relation_use_splice_records);
        assert_eq!(reject(relation_use_splice), 4);

        let mut evidence_splice_records = records.clone();
        evidence_splice_records[6].evidence_set_digest = records[7].evidence_set_digest;
        let evidence_splice = VecContributionProvider::new(evidence_splice_records);
        assert_eq!(reject(evidence_splice), 7);

        let mut replay_substitution = VecContributionProvider::new(records.clone());
        replay_substitution.replay_substitution = Some((4, records[5].clone()));
        assert_eq!(reject(replay_substitution), 13);

        let context_mutations: [fn(&mut ZkAmsMkheDirectCeremonyContextV1); 7] = [
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| context.common_a_seed[0] ^= 1,
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| context.target_a_seed[0] ^= 1,
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| context.final_a_seed[0] ^= 1,
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| {
                context.collective_public_key_digest[0] ^= 1;
            },
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| context.transcript_digest[0] ^= 1,
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| context.secret_lineage_root[0] ^= 1,
            |context: &mut ZkAmsMkheDirectCeremonyContextV1| context.epoch ^= 1,
        ];
        for mutate in context_mutations {
            let mut changed = context;
            mutate(&mut changed);
            let mut candidate = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
            candidate.context = changed;
            let mut source = VecContributionProvider::new(records.clone());
            assert!(admit_ordered_set_inner(&roster, &mut candidate, &mut source).is_err());
            assert_eq!(source.reads, 0);
            assert!(candidate.slots.iter().all(Option::is_none));
        }

        let cross_purpose_context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index: 0 },
        );
        let mut cross_purpose_coordinator =
            ZkAmsMkheDirectCoordinatorV1::new(&roster, cross_purpose_context).unwrap();
        let mut cross_purpose = VecContributionProvider::new(records.clone());
        assert!(
            admit_ordered_set_inner(&roster, &mut cross_purpose_coordinator, &mut cross_purpose,)
                .is_err()
        );
        assert_eq!(cross_purpose.reads, 1);

        let references: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            core::array::from_fn(|index| &secrets[index]);
        let next_epoch_roster =
            ZkAmsMkheGovernedActiveRosterV1::new(42, references, &mut random).unwrap();
        let next_epoch_context = context_fixture(
            &next_epoch_roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        let mut next_epoch_coordinator =
            ZkAmsMkheDirectCoordinatorV1::new(&next_epoch_roster, next_epoch_context).unwrap();
        let mut cross_epoch = VecContributionProvider::new(records);
        assert!(
            admit_ordered_set_inner(
                &next_epoch_roster,
                &mut next_epoch_coordinator,
                &mut cross_epoch,
            )
            .is_err()
        );
        assert_eq!(cross_epoch.reads, 1);
    }

    #[test]
    fn coordinator_binds_order_prior_round_replay_and_authentication() {
        let (roster, secrets, mut random) = roster_fixture(b"direct-state-machine");
        let context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        let mut coordinator = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
        let round_one = make_round_one_contributions(&roster, &secrets, context, &mut random);
        assert!(
            coordinator
                .ordered_contribution_set_digest(&roster)
                .is_err()
        );
        for contribution in round_one.iter().rev() {
            coordinator.admit(&roster, contribution).unwrap();
        }
        let ordered = coordinator
            .ordered_contribution_set_digest(&roster)
            .unwrap();
        assert_ne!(ordered, [0; 32]);
        let snapshot = coordinator.slots;
        assert!(coordinator.admit(&roster, &round_one[0]).is_err());
        assert_eq!(coordinator.slots, snapshot);
        assert!(
            coordinator
                .advance_after_verified_aggregate(
                    &roster,
                    &[keccak256(b"aggregate-h0"), keccak256(b"aggregate-h1")],
                )
                .is_err()
        );
        assert_eq!(coordinator.slots, snapshot);

        coordinator = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
        let mut round_one_provider = VecContributionProvider::new(round_one.clone());
        admit_ordered_set_inner(&roster, &mut coordinator, &mut round_one_provider).unwrap();
        let aggregate_round = coordinator
            .advance_after_verified_aggregate(
                &roster,
                &[keccak256(b"aggregate-h0"), keccak256(b"aggregate-h1")],
            )
            .unwrap();
        assert_ne!(aggregate_round, context.initial_round_digest);
        assert!(
            coordinator
                .ordered_contribution_set_digest(&roster)
                .is_err()
        );
        assert!(coordinator.admit(&roster, &round_one[0]).is_err());

        let payload = DirectContributionPayloadV1::RkgRoundTwo {
            k: fake_receipt(
                &roster,
                context,
                ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
                0,
                ZkAmsMkheDirectPolynomialRoleV1::RkgK,
                b"k-0",
            ),
        };
        let mut round_two = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            aggregate_round,
            0,
            payload,
            keccak256(b"proof-k-0"),
            context.secret_lineage_digests[0],
            test_ephemeral_lineage(context, 0),
            &secrets[0],
            &mut random,
        )
        .unwrap();
        let authentic = round_two.clone();
        round_two.prior_round_digest = context.initial_round_digest;
        assert!(coordinator.admit(&roster, &round_two).is_err());
        round_two = authentic.clone();
        round_two.party_index = 1;
        assert!(coordinator.admit(&roster, &round_two).is_err());
        round_two = authentic.clone();
        round_two.proof_digest[0] ^= 1;
        assert!(coordinator.admit(&roster, &round_two).is_err());
        coordinator.admit(&roster, &authentic).unwrap();
    }

    #[test]
    fn normalized_rkg_requires_complete_same_secret_and_ephemeral_lineage() {
        let (roster, secrets, mut random) = roster_fixture(b"direct-normalization-state-machine");
        let context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        );
        let mut coordinator = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();

        let round_one = make_round_one_contributions(&roster, &secrets, context, &mut random);
        let mut round_one_provider = VecContributionProvider::new(round_one);
        let round_one_admitted =
            admit_ordered_set_inner(&roster, &mut coordinator, &mut round_one_provider).unwrap();
        assert_eq!(
            round_one_admitted.round(),
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne
        );
        let round_one_aggregate = coordinator
            .advance_after_verified_aggregate(
                &roster,
                &[
                    keccak256(b"normalization-h0"),
                    keccak256(b"normalization-h1"),
                ],
            )
            .unwrap();

        let round_two_payload = |label: &[u8]| DirectContributionPayloadV1::RkgRoundTwo {
            k: fake_receipt(
                &roster,
                context,
                ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
                0,
                ZkAmsMkheDirectPolynomialRoleV1::RkgK,
                label,
            ),
        };
        let wrong_ephemeral = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            round_one_aggregate,
            0,
            round_two_payload(b"wrong-u-k"),
            keccak256(b"wrong-u-proof"),
            context.secret_lineage_digests[0],
            keccak256(b"substituted-u-lineage"),
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(coordinator.admit(&roster, &wrong_ephemeral).is_err());
        assert!(coordinator.slots[0].is_none());

        let wrong_secret = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
            round_one_aggregate,
            0,
            round_two_payload(b"wrong-secret-k"),
            keccak256(b"wrong-secret-proof"),
            keccak256(b"substituted-secret-lineage"),
            test_ephemeral_lineage(context, 0),
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(coordinator.admit(&roster, &wrong_secret).is_err());
        assert!(coordinator.slots[0].is_none());

        let round_two = make_round_two_contributions(
            &roster,
            &secrets,
            context,
            round_one_aggregate,
            &mut random,
        );
        let mut omitted_round_two = VecContributionProvider::new(round_two[..7].to_vec());
        assert!(
            admit_ordered_set_inner(&roster, &mut coordinator, &mut omitted_round_two).is_err()
        );
        assert_eq!(omitted_round_two.reads, 0);
        assert!(coordinator.slots.iter().all(Option::is_none));
        let mut round_two_provider = VecContributionProvider::new(round_two.clone());
        let round_two_admitted =
            admit_ordered_set_inner(&roster, &mut coordinator, &mut round_two_provider).unwrap();
        assert_eq!(round_two_provider.reads, 16);
        assert_eq!(
            round_two_admitted.round(),
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo
        );
        let ordered_round_two = round_two_admitted.ordered_contribution_set_digest();
        assert_ne!(ordered_round_two, [0; 32]);
        assert!(coordinator.admit(&roster, &round_two[0]).is_err());
        let round_two_aggregate = coordinator
            .advance_after_verified_aggregate(&roster, &[keccak256(b"normalization-k")])
            .unwrap();

        let normalization_payload =
            |role, label: &[u8]| DirectContributionPayloadV1::RkgNormalize {
                normalization: fake_receipt(
                    &roster,
                    context,
                    ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
                    0,
                    role,
                    label,
                ),
            };
        let wrong_normalization_secret = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
            round_two_aggregate,
            0,
            normalization_payload(
                ZkAmsMkheDirectPolynomialRoleV1::RkgNormalization,
                b"wrong-normalization-secret",
            ),
            keccak256(b"wrong-normalization-secret-proof"),
            keccak256(b"substituted-normalization-secret-lineage"),
            [0; 32],
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(
            coordinator
                .admit(&roster, &wrong_normalization_secret)
                .is_err()
        );

        let nonzero_normalization_u = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
            round_two_aggregate,
            0,
            normalization_payload(
                ZkAmsMkheDirectPolynomialRoleV1::RkgNormalization,
                b"nonzero-normalization-u",
            ),
            keccak256(b"nonzero-normalization-u-proof"),
            context.secret_lineage_digests[0],
            test_ephemeral_lineage(context, 0),
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(
            coordinator
                .admit(&roster, &nonzero_normalization_u)
                .is_err()
        );

        let wrong_normalization_role = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
            round_two_aggregate,
            0,
            normalization_payload(
                ZkAmsMkheDirectPolynomialRoleV1::RkgK,
                b"wrong-normalization-role",
            ),
            keccak256(b"wrong-normalization-role-proof"),
            context.secret_lineage_digests[0],
            [0; 32],
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(
            coordinator
                .admit(&roster, &wrong_normalization_role)
                .is_err()
        );
        assert!(coordinator.slots.iter().all(Option::is_none));

        let normalization = make_normalization_contributions(
            &roster,
            &secrets,
            context,
            round_two_aggregate,
            &mut random,
        );
        let mut omitted_normalization = VecContributionProvider::new(normalization[..7].to_vec());
        assert!(
            admit_ordered_set_inner(&roster, &mut coordinator, &mut omitted_normalization).is_err()
        );
        assert_eq!(omitted_normalization.reads, 0);
        assert!(coordinator.slots.iter().all(Option::is_none));

        let mut reordered_normalization_records = normalization.clone();
        reordered_normalization_records.swap(0, 1);
        let mut reordered_normalization =
            VecContributionProvider::new(reordered_normalization_records);
        assert!(
            admit_ordered_set_inner(&roster, &mut coordinator, &mut reordered_normalization)
                .is_err()
        );
        assert_eq!(reordered_normalization.reads, 1);
        assert!(coordinator.slots.iter().all(Option::is_none));

        let mut substituted_normalization = VecContributionProvider::new(normalization.clone());
        substituted_normalization.replay_substitution = Some((3, normalization[4].clone()));
        assert!(
            admit_ordered_set_inner(&roster, &mut coordinator, &mut substituted_normalization)
                .is_err()
        );
        assert_eq!(substituted_normalization.reads, 12);
        assert!(coordinator.slots.iter().all(Option::is_none));

        let mut normalization_provider = VecContributionProvider::new(normalization.clone());
        let normalization_admitted =
            admit_ordered_set_inner(&roster, &mut coordinator, &mut normalization_provider)
                .unwrap();
        assert_eq!(normalization_provider.reads, 16);
        assert_eq!(
            normalization_admitted.round(),
            ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize
        );
        let normalization_set_digest = normalization_admitted.ordered_contribution_set_digest();
        assert_ne!(normalization_set_digest, ordered_round_two);
        let full_slots = coordinator.slots;
        assert!(coordinator.admit(&roster, &normalization[0]).is_err());
        assert_eq!(coordinator.slots, full_slots);
        assert!(
            coordinator
                .advance_after_verified_aggregate(&roster, &[])
                .is_err()
        );
        assert!(
            coordinator
                .advance_after_verified_aggregate(
                    &roster,
                    &[
                        keccak256(b"final-b"),
                        keccak256(b"unexpected-second-output")
                    ],
                )
                .is_err()
        );
        assert_eq!(coordinator.slots, full_slots);

        let final_aggregate = coordinator
            .advance_after_verified_aggregate(&roster, &[keccak256(b"final-b")])
            .unwrap();
        assert_ne!(final_aggregate, round_two_aggregate);
        let completion = coordinator
            .ordered_contribution_set_digest(&roster)
            .unwrap();
        assert_ne!(completion, final_aggregate);
        assert!(coordinator.admit(&roster, &normalization[0]).is_err());
    }

    #[test]
    fn galois_round_requires_zero_u_same_secret_and_canonical_provider_replay() {
        let (roster, secrets, mut random) = roster_fixture(b"direct-galois-state-machine");
        let context = context_fixture(
            &roster,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index: 7 },
        );
        let mut coordinator = ZkAmsMkheDirectCoordinatorV1::new(&roster, context).unwrap();
        let contributions = make_galois_contributions(&roster, &secrets, context, &mut random);

        let payload = |label: &[u8]| DirectContributionPayloadV1::Galois {
            b: fake_receipt(
                &roster,
                context,
                ZkAmsMkheDirectCeremonyRoundV1::Galois,
                0,
                ZkAmsMkheDirectPolynomialRoleV1::GaloisB,
                label,
            ),
        };
        let nonzero_u = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::Galois,
            context.initial_round_digest,
            0,
            payload(b"galois-nonzero-u"),
            keccak256(b"galois-nonzero-u-proof"),
            context.secret_lineage_digests[0],
            test_ephemeral_lineage(context, 0),
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(coordinator.admit(&roster, &nonzero_u).is_err());

        let wrong_secret = authenticate_test_verified_contribution(
            &roster,
            context,
            ZkAmsMkheDirectCeremonyRoundV1::Galois,
            context.initial_round_digest,
            0,
            payload(b"galois-wrong-secret"),
            keccak256(b"galois-wrong-secret-proof"),
            keccak256(b"galois-substituted-secret-lineage"),
            [0; 32],
            &secrets[0],
            &mut random,
        )
        .unwrap();
        assert!(coordinator.admit(&roster, &wrong_secret).is_err());
        assert!(coordinator.slots.iter().all(Option::is_none));

        let mut reordered_records = contributions.clone();
        reordered_records.swap(2, 3);
        let mut reordered = VecContributionProvider::new(reordered_records);
        assert!(admit_ordered_set_inner(&roster, &mut coordinator, &mut reordered).is_err());
        assert_eq!(reordered.reads, 3);
        assert!(coordinator.slots.iter().all(Option::is_none));

        let mut provider = VecContributionProvider::new(contributions.clone());
        let admitted = admit_ordered_set_inner(&roster, &mut coordinator, &mut provider).unwrap();
        assert_eq!(provider.reads, 16);
        assert_eq!(admitted.round(), ZkAmsMkheDirectCeremonyRoundV1::Galois);
        assert_eq!(admitted.target_a_seed(), context.target_a_seed());
        assert_eq!(admitted.ordered_ephemeral_lineage_set_digest(), {
            ordered_lineage_set_digest(
                &roster,
                context,
                ZkAmsMkheDirectCeremonyRoundV1::Galois,
                context.initial_round_digest,
                &[[0; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
            )
        });
        assert!(coordinator.admit(&roster, &contributions[0]).is_err());
        let aggregate = coordinator
            .advance_after_verified_aggregate(&roster, &[keccak256(b"galois-aggregate-b")])
            .unwrap();
        let completion = coordinator
            .ordered_contribution_set_digest(&roster)
            .unwrap();
        assert_ne!(aggregate, completion);
        assert!(coordinator.admit(&roster, &contributions[0]).is_err());
    }

    fn add_poly(left: &[i64], right: &[i64], modulus: i64) -> Vec<i64> {
        left.iter()
            .zip(right)
            .map(|(left, right)| (left + right).rem_euclid(modulus))
            .collect()
    }

    fn sub_poly(left: &[i64], right: &[i64], modulus: i64) -> Vec<i64> {
        left.iter()
            .zip(right)
            .map(|(left, right)| (left - right).rem_euclid(modulus))
            .collect()
    }

    fn scale_poly(value: &[i64], scalar: i64, modulus: i64) -> Vec<i64> {
        value
            .iter()
            .map(|coefficient| (coefficient * scalar).rem_euclid(modulus))
            .collect()
    }

    fn negacyclic_mul(left: &[i64], right: &[i64], modulus: i64) -> Vec<i64> {
        let mut output = vec![0_i64; left.len()];
        for (left_index, left_coefficient) in left.iter().enumerate() {
            for (right_index, right_coefficient) in right.iter().enumerate() {
                let destination = left_index + right_index;
                let (destination, sign) = if destination >= left.len() {
                    (destination - left.len(), -1)
                } else {
                    (destination, 1)
                };
                output[destination] = (output[destination]
                    + sign * left_coefficient * right_coefficient)
                    .rem_euclid(modulus);
            }
        }
        output
    }

    fn automorphism(value: &[i64], exponent: usize, modulus: i64) -> Vec<i64> {
        let twice_degree = value.len() * 2;
        let mut output = vec![0_i64; value.len()];
        for (index, coefficient) in value.iter().enumerate() {
            let destination = index * exponent % twice_degree;
            if destination >= value.len() {
                output[destination - value.len()] = (-coefficient).rem_euclid(modulus);
            } else {
                output[destination] = coefficient.rem_euclid(modulus);
            }
        }
        output
    }

    #[test]
    fn tiny_direct_collective_rkg_and_galois_algebra_are_exact() {
        const N: usize = 8;
        const PARTIES: usize = 3;
        const Q: i64 = 257;
        const P: i64 = 17;
        const GADGET: i64 = 16;
        let a = [7, 29, 3, 41, 5, 11, 13, 19];
        let secrets = [
            [1, 0, -1, 0, 1, 0, 0, -1],
            [0, 1, 0, -1, 0, 1, -1, 0],
            [-1, 0, 1, 1, 0, 0, 1, 0],
        ];
        let ephemeral = [
            [0, 1, 0, 0, -1, 1, 0, 0],
            [1, 0, -1, 0, 0, 0, 1, 0],
            [0, -1, 0, 1, 0, 1, 0, -1],
        ];
        let e0 = [[1, 0, -1, 0, 1, 0, -1, 0]; PARTIES];
        let e1 = [[0, 1, 0, -1, 0, 1, 0, -1]; PARTIES];
        let e2 = [[1, -1, 0, 0, 1, -1, 0, 0]; PARTIES];
        let mut h0 = vec![0; N];
        let mut h1 = vec![0; N];
        for party in 0..PARTIES {
            let h0_i = add_poly(
                &sub_poly(
                    &scale_poly(&secrets[party], GADGET, Q),
                    &negacyclic_mul(&a, &ephemeral[party], Q),
                    Q,
                ),
                &scale_poly(&e0[party], P, Q),
                Q,
            );
            let h1_i = add_poly(
                &negacyclic_mul(&a, &secrets[party], Q),
                &scale_poly(&e1[party], P, Q),
                Q,
            );
            h0 = add_poly(&h0, &h0_i, Q);
            h1 = add_poly(&h1, &h1_i, Q);
        }
        let mut k = vec![0; N];
        for party in 0..PARTIES {
            let u_minus_s = sub_poly(&ephemeral[party], &secrets[party], Q);
            let k_i = add_poly(
                &add_poly(
                    &negacyclic_mul(&h0, &secrets[party], Q),
                    &negacyclic_mul(&h1, &u_minus_s, Q),
                    Q,
                ),
                &scale_poly(&e2[party], P, Q),
                Q,
            );
            k = add_poly(&k, &k_i, Q);
        }
        let sum = |values: &[[i64; N]; PARTIES]| {
            values.iter().fold(vec![0; N], |accumulator, value| {
                add_poly(&accumulator, value, Q)
            })
        };
        let s = sum(&secrets);
        let u = sum(&ephemeral);
        let aggregate_e0 = sum(&e0);
        let aggregate_e1 = sum(&e1);
        let aggregate_e2 = sum(&e2);
        let intrinsic_error = add_poly(
            &add_poly(
                &negacyclic_mul(&aggregate_e0, &s, Q),
                &negacyclic_mul(&aggregate_e1, &u, Q),
                Q,
            ),
            &aggregate_e2,
            Q,
        );
        let decrypted = add_poly(&k, &negacyclic_mul(&h1, &s, Q), Q);
        let expected = add_poly(
            &scale_poly(&negacyclic_mul(&s, &s, Q), GADGET, Q),
            &scale_poly(&intrinsic_error, P, Q),
            Q,
        );
        assert_eq!(decrypted, expected);

        let final_a = [23, 2, 31, 17, 43, 5, 47, 7];
        let e3 = [[-1, 0, 1, 0, -1, 0, 1, 0]; PARTIES];
        let h1_minus_final_a = sub_poly(&h1, &final_a, Q);
        let mut normalization = vec![0; N];
        for party in 0..PARTIES {
            let contribution = add_poly(
                &negacyclic_mul(&h1_minus_final_a, &secrets[party], Q),
                &scale_poly(&e3[party], P, Q),
                Q,
            );
            normalization = add_poly(&normalization, &contribution, Q);
        }
        let final_b = add_poly(&k, &normalization, Q);
        let normalized_decryption = add_poly(&final_b, &negacyclic_mul(&final_a, &s, Q), Q);
        let normalized_expected = add_poly(&expected, &scale_poly(&sum(&e3), P, Q), Q);
        assert_eq!(normalized_decryption, normalized_expected);

        let exponent = 3;
        let galois_errors = [[1, 0, -1, 0, 0, 1, 0, -1]; PARTIES];
        let mut b = vec![0; N];
        for party in 0..PARTIES {
            let b_i = add_poly(
                &sub_poly(
                    &scale_poly(&automorphism(&secrets[party], exponent, Q), GADGET, Q),
                    &negacyclic_mul(&a, &secrets[party], Q),
                    Q,
                ),
                &scale_poly(&galois_errors[party], P, Q),
                Q,
            );
            b = add_poly(&b, &b_i, Q);
        }
        let galois_decrypted = add_poly(&b, &negacyclic_mul(&a, &s, Q), Q);
        let galois_expected = add_poly(
            &scale_poly(&automorphism(&s, exponent, Q), GADGET, Q),
            &scale_poly(&sum(&galois_errors), P, Q),
            Q,
        );
        assert_eq!(galois_decrypted, galois_expected);
    }
}

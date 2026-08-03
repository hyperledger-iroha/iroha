//! Governed active-party ceremonies for the first-release ZK-AMS MKHE profile.
//!
//! The active-party boundary is deliberately stricter than a collection of
//! signed byte strings.  A secret epoch fixes exactly eight authentication
//! party IDs in one canonical order, every authentication key proves possession
//! against a separate whole-roster key-material certificate before the roster
//! can be admitted, and every later contribution is
//! bound to that roster, epoch, protocol transcript, round, record index, and
//! payload.  Aggregation either returns an exact ordered receipt or stable
//! identifiable-abort evidence naming the first offending position.
//!
//! This module closes roster governance, rogue-authentication-key resistance,
//! and deterministic active-round accounting. Schnorr authentication is never
//! treated as a polynomial proof. RKG uses the narrow-coefficient lattice proof
//! below; statistically smudged CKS uses a distinct fixed-width wide-coefficient
//! proof because its release witness bound cannot fit in `i64`. The readiness
//! gate remains closed until both families are wired to canonical records and
//! release KATs.

use super::{
    ArtifactAuthentication, AuthenticationSecret, MKHE_VERSION_V1, Scalar, ZkAmsMkheErrorV1,
    ZkAmsMkhePartyIdV1, auth_generator,
    manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    packing::{
        ZK_AMS_T256_GALOIS_KEY_COUNT_V1, validate_zk_ams_t256_galois_key_schedule_v1,
        zk_ams_t256_galois_key_schedule_v1,
    },
};
use crate::vega::{
    MaskedRelaxedRandomSourceV1, VegaT256PointV1,
    sponge::{Keccak256, keccak256, shake256},
};

const ACTIVE_ROSTER_KEY_MATERIAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.active-governed-key-material";
const ROSTER_POP_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.active-roster-key-pop";
const ACTIVE_CONTRIBUTION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.active-contribution-authentication";
const ACTIVE_ROUND_RECEIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.active-round-receipt";
const ACTIVE_ABORT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.identifiable-abort";
const ACTIVE_COLLECTIVE_KEY_MATERIAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.governed-collective-key-material";
const ACTIVE_COLLECTIVE_PUBLIC_A_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.active-collective-public-a";
#[cfg(test)]
const ROSTER_POP_BYTES_V1: usize = 65;
const RANDOM_REJECTION_ATTEMPTS_V1: usize = 128;
const RKG_LINEAR_PROOF_MAX_WITNESSES_V1: usize = 8;
const RKG_LINEAR_PROOF_MAX_OUTPUTS_V1: usize = 4;
const RKG_LINEAR_PROOF_MASK_SLACK_FACTOR_V1: i64 = 1 << 24;
const RKG_LINEAR_PROOF_RELEASE_CHALLENGE_WEIGHT_V1: usize = 60;
const RKG_LINEAR_PROOF_RANDOM_HEALTH_RETRIES_V1: usize = 16;
const RKG_LINEAR_PROOF_FIAT_SHAMIR_BITS_V1: u16 = 256;
const RKG_LINEAR_PROOF_CHALLENGE_MIN_ENTROPY_BITS_V1: u16 = 256;
const RKG_LINEAR_PROOF_TRANSCRIPT_BINDING_BITS_V1: u16 = 128;
const RKG_LINEAR_PROOF_SOUNDNESS_BITS_V1: u16 = 128;
const RKG_LINEAR_PROOF_CHALLENGE_SPACE_LOWER_BOUND_BITS_V1: u16 = 720;
const RKG_LINEAR_PROOF_RETRY_EXHAUSTION_BITS_V1: u16 = 512;
const RKG_LINEAR_PROOF_SIGNED_COEFFICIENT_BYTES_V1: u8 = 8;
const RKG_LINEAR_PROOF_WIRE_TAG_V1: [u8; 4] = *b"ZARP";
const RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_V1: usize = 4 + 1 + 32 + 1 + 4;
const RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_U32_V1: u32 = 4 + 1 + 32 + 1 + 4;
const ACTIVE_RKG_EVIDENCE_TAG_V1: [u8; 4] = *b"ZARE";
const ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1: usize = 4 + 1 + 32 + 1 + 4;
const ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1: usize =
    1 + 32 + 32 + 8 + 32 + 1 + 4 + 32 + 32 + 1 + 32 + 33 + 65;

/// One proof of possession for a governed T256 authentication key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRosterKeyProofV1 {
    commitment: [u8; 33],
    response: [u8; 32],
}

impl ZkAmsMkheRosterKeyProofV1 {
    /// Canonical nonidentity Schnorr commitment.
    #[must_use]
    pub const fn commitment(self) -> [u8; 33] {
        self.commitment
    }

    /// Canonical T256 Schnorr response.
    #[must_use]
    pub const fn response(self) -> [u8; 32] {
        self.response
    }

    #[cfg(test)]
    fn signature_bytes(self) -> [u8; ROSTER_POP_BYTES_V1] {
        let mut bytes = [0_u8; ROSTER_POP_BYTES_V1];
        bytes[..33].copy_from_slice(&self.commitment);
        bytes[33..].copy_from_slice(&self.response);
        bytes
    }
}

/// Secret authentication state for one governed active party.
///
/// The scalar is generated from a caller-supplied cryptographic random source,
/// is never cloneable, is redacted from debug output, and is cleared on drop by
/// the underlying secret type.
pub struct ZkAmsMkheActivePartySecretV1 {
    authentication: AuthenticationSecret,
}

impl ZkAmsMkheActivePartySecretV1 {
    /// Generate one fresh nonzero T256 authentication secret.
    pub fn generate<R: MaskedRelaxedRandomSourceV1>(
        random: &mut R,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Ok(Self {
            authentication: AuthenticationSecret::generate(random)?,
        })
    }

    /// Authentication-key-derived party identifier.
    pub fn party(&self) -> Result<ZkAmsMkhePartyIdV1, ZkAmsMkheErrorV1> {
        self.authentication.party_id()
    }

    /// Canonical nonidentity T256 authentication public key.
    pub fn public_key(&self) -> Result<[u8; 33], ZkAmsMkheErrorV1> {
        self.authentication.public_key()
    }

    /// Authenticate an internal protocol artifact without exposing the scalar.
    pub(super) fn authenticate_artifact<R: MaskedRelaxedRandomSourceV1>(
        &self,
        domain: &[u8],
        statement_digest: [u8; 32],
        random: &mut R,
    ) -> Result<ArtifactAuthentication, ZkAmsMkheErrorV1> {
        ArtifactAuthentication::sign(domain, statement_digest, &self.authentication, random)
    }
}

impl core::fmt::Debug for ZkAmsMkheActivePartySecretV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsMkheActivePartySecretV1([REDACTED])")
    }
}

/// One authentication-key-bound member of a governed MKHE roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheGovernedParticipantV1 {
    party: ZkAmsMkhePartyIdV1,
    authentication_public_key: [u8; 33],
    key_proof: ZkAmsMkheRosterKeyProofV1,
}

impl ZkAmsMkheGovernedParticipantV1 {
    /// Authentication-key-derived participant identifier.
    #[must_use]
    pub const fn party(self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Canonical nonidentity T256 authentication public key.
    #[must_use]
    pub const fn authentication_public_key(self) -> [u8; 33] {
        self.authentication_public_key
    }

    /// Proof of possession bound to the complete ordered roster.
    #[must_use]
    pub const fn key_proof(self) -> ZkAmsMkheRosterKeyProofV1 {
        self.key_proof
    }
}

/// The sole governed roster form: exactly eight ordered keys in one nonzero epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheGovernedActiveRosterV1 {
    version: u8,
    profile_digest: [u8; 32],
    epoch: u64,
    participants: [ZkAmsMkheGovernedParticipantV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
}

impl ZkAmsMkheGovernedActiveRosterV1 {
    /// Build and verify an exact eight-party release roster.
    ///
    /// The caller supplies the intended canonical order. This constructor does
    /// not sort keys, because silently sorting governance input would hide an
    /// operator ordering error and produce a different consensus identity.
    pub fn new<R: MaskedRelaxedRandomSourceV1>(
        epoch: u64,
        parties: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        random: &mut R,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        assemble_governed_active_roster(epoch, parties.map(|party| &party.authentication), random)
    }

    /// Frozen release-profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    /// Nonzero governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Exact ordered authentication-key roster.
    #[must_use]
    pub const fn participants(
        &self,
    ) -> &[ZkAmsMkheGovernedParticipantV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.participants
    }

    /// Consensus digest of the release profile, epoch, and ordered party IDs.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Digest of the exact ordered authentication keys certified by the PoPs.
    #[must_use]
    pub const fn key_material_digest(&self) -> [u8; 32] {
        self.key_material_digest
    }

    /// Convert to the canonical wire roster without changing its consensus identity.
    pub fn to_wire_roster(&self) -> Result<super::ZkAmsMkheGovernedRosterWireV1, ZkAmsMkheErrorV1> {
        self.validate()?;
        let parties = self.participants.map(|participant| participant.party);
        let wire =
            super::ZkAmsMkheGovernedRosterWireV1::new(self.profile_digest, self.epoch, parties)?;
        if wire.roster_digest() != self.roster_digest {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        Ok(wire)
    }

    /// Revalidate the complete roster and every proof of possession.
    pub fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected_profile = release_profile_v1().digest()?;
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != expected_profile
            || self.epoch == 0
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let keys = self
            .participants
            .iter()
            .map(|participant| participant.authentication_public_key)
            .collect::<Vec<_>>();
        let (parties, roster_digest, key_material_digest) =
            active_roster_identity(self.profile_digest, self.epoch, keys.as_slice(), true)?;
        if roster_digest != self.roster_digest
            || key_material_digest != self.key_material_digest
            || self
                .participants
                .iter()
                .zip(parties)
                .any(|(participant, party)| participant.party != party)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        for (index, participant) in self.participants.iter().enumerate() {
            verify_roster_key_proof(
                self.profile_digest,
                self.epoch,
                self.roster_digest,
                self.key_material_digest,
                index,
                *participant,
            )?;
        }
        Ok(())
    }

    fn participant(
        &self,
        index: usize,
    ) -> Result<ZkAmsMkheGovernedParticipantV1, ZkAmsMkheErrorV1> {
        self.participants
            .get(index)
            .copied()
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)
    }

    fn index_of(&self, party: ZkAmsMkhePartyIdV1) -> Option<usize> {
        self.participants
            .binary_search_by_key(&party, |participant| participant.party)
            .ok()
    }
}

fn assemble_governed_active_roster<R: MaskedRelaxedRandomSourceV1>(
    epoch: u64,
    authentication_secrets: [&AuthenticationSecret; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    random: &mut R,
) -> Result<ZkAmsMkheGovernedActiveRosterV1, ZkAmsMkheErrorV1> {
    let profile_digest = release_profile_v1().digest()?;
    if epoch == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let keys = authentication_secrets
        .iter()
        .map(|secret| secret.public_key())
        .collect::<Result<Vec<_>, _>>()?;
    let (parties, roster_digest, key_material_digest) =
        active_roster_identity(profile_digest, epoch, &keys, true)?;
    let mut participants = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    for (index, ((secret, public_key), party)) in authentication_secrets
        .iter()
        .zip(keys)
        .zip(parties)
        .enumerate()
    {
        let key_proof = prove_roster_key_possession(
            profile_digest,
            epoch,
            roster_digest,
            key_material_digest,
            index,
            party,
            public_key,
            secret,
            random,
        )?;
        participants.push(ZkAmsMkheGovernedParticipantV1 {
            party,
            authentication_public_key: public_key,
            key_proof,
        });
    }
    let roster = ZkAmsMkheGovernedActiveRosterV1 {
        version: MKHE_VERSION_V1,
        profile_digest,
        epoch,
        participants: participants
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        roster_digest,
        key_material_digest,
    };
    roster.validate()?;
    Ok(roster)
}

fn active_roster_identity(
    profile_digest: [u8; 32],
    epoch: u64,
    keys: &[[u8; 33]],
    require_release_profile: bool,
) -> Result<(Vec<ZkAmsMkhePartyIdV1>, [u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
    if profile_digest == [0; 32]
        || epoch == 0
        || keys.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || (require_release_profile && profile_digest != release_profile_v1().digest()?)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let mut parties = Vec::with_capacity(keys.len());
    for key in keys {
        VegaT256PointV1::from_non_identity_wire_bytes_exact(key)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
        parties.push(ZkAmsMkhePartyIdV1::from_authentication_key(key)?);
    }
    if parties.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let roster_digest = super::wire::governed_roster_digest(profile_digest, epoch, &parties);
    let mut frame = Vec::with_capacity(128 + keys.len() * 69);
    frame.extend_from_slice(ACTIVE_ROSTER_KEY_MATERIAL_DOMAIN_V1);
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&profile_digest);
    frame.extend_from_slice(&epoch.to_be_bytes());
    frame.extend_from_slice(&roster_digest);
    frame.push(u8::try_from(keys.len()).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?);
    for (index, (party, key)) in parties.iter().zip(keys).enumerate() {
        frame.extend_from_slice(
            &u32::try_from(index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?
                .to_be_bytes(),
        );
        frame.extend_from_slice(&party.to_bytes());
        frame.extend_from_slice(key);
    }
    Ok((parties, roster_digest, keccak256(&frame)))
}

#[allow(clippy::too_many_arguments)]
fn prove_roster_key_possession<R: MaskedRelaxedRandomSourceV1>(
    profile_digest: [u8; 32],
    epoch: u64,
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    index: usize,
    party: ZkAmsMkhePartyIdV1,
    public_key: [u8; 33],
    secret: &AuthenticationSecret,
    random: &mut R,
) -> Result<ZkAmsMkheRosterKeyProofV1, ZkAmsMkheErrorV1> {
    if roster_digest == [0; 32]
        || key_material_digest == [0; 32]
        || index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || secret.party_id()? != party
        || secret.public_key()? != public_key
    {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let nonce = sample_nonzero_scalar(random)?;
    let commitment = auth_generator()?
        .mul_scalar(nonce)
        .to_non_identity_wire_bytes()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
    let challenge = roster_pop_challenge(
        profile_digest,
        epoch,
        roster_digest,
        key_material_digest,
        index,
        party,
        public_key,
        commitment,
    )?;
    let proof = ZkAmsMkheRosterKeyProofV1 {
        commitment,
        response: (nonce + challenge * secret.scalar()?).to_be_bytes(),
    };
    verify_roster_key_proof(
        profile_digest,
        epoch,
        roster_digest,
        key_material_digest,
        index,
        ZkAmsMkheGovernedParticipantV1 {
            party,
            authentication_public_key: public_key,
            key_proof: proof,
        },
    )?;
    Ok(proof)
}

fn verify_roster_key_proof(
    profile_digest: [u8; 32],
    epoch: u64,
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    index: usize,
    participant: ZkAmsMkheGovernedParticipantV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if profile_digest == [0; 32]
        || epoch == 0
        || roster_digest == [0; 32]
        || key_material_digest == [0; 32]
        || index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || participant.party
            != ZkAmsMkhePartyIdV1::from_authentication_key(&participant.authentication_public_key)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let public_key =
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&participant.authentication_public_key)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
    let commitment =
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&participant.key_proof.commitment)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
    let response = Scalar::from_be_bytes_exact(participant.key_proof.response)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
    let challenge = roster_pop_challenge(
        profile_digest,
        epoch,
        roster_digest,
        key_material_digest,
        index,
        participant.party,
        participant.authentication_public_key,
        participant.key_proof.commitment,
    )?;
    if auth_generator()?.mul_scalar(response) != commitment + public_key.mul_scalar(challenge) {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn roster_pop_challenge(
    profile_digest: [u8; 32],
    epoch: u64,
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    index: usize,
    party: ZkAmsMkhePartyIdV1,
    public_key: [u8; 33],
    commitment: [u8; 33],
) -> Result<Scalar, ZkAmsMkheErrorV1> {
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(ROSTER_POP_DOMAIN_V1);
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&profile_digest);
    frame.extend_from_slice(&epoch.to_be_bytes());
    frame.extend_from_slice(&roster_digest);
    frame.extend_from_slice(&key_material_digest);
    frame.extend_from_slice(
        &u32::try_from(index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(&party.to_bytes());
    frame.extend_from_slice(&public_key);
    frame.extend_from_slice(&commitment);
    scalar_challenge(&frame)
}

fn scalar_challenge(frame: &[u8]) -> Result<Scalar, ZkAmsMkheErrorV1> {
    for counter in 0..RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut challenge_frame = Vec::with_capacity(frame.len() + 4);
        challenge_frame.extend_from_slice(frame);
        challenge_frame.extend_from_slice(
            &u32::try_from(counter)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?
                .to_be_bytes(),
        );
        let uniform: [u8; 64] = shake256(&challenge_frame, 64)
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
        let challenge = Scalar::from_uniform_le_bytes(uniform);
        if !challenge.is_zero() {
            return Ok(challenge);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidAuthentication)
}

fn sample_nonzero_scalar<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<Scalar, ZkAmsMkheErrorV1> {
    for _ in 0..RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut uniform = [0_u8; 64];
        random
            .fill_bytes(&mut uniform)
            .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
        let scalar = Scalar::from_uniform_le_bytes(uniform);
        uniform.fill(0);
        if !scalar.is_zero() {
            return Ok(scalar);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

/// Exact active-party ceremony round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheActiveRoundV1 {
    /// Governed collective public-key contribution.
    CollectivePublicKey = 1,
    /// Collective key-switch contribution.
    Cks = 2,
    /// First collective relinearization-key round.
    RkgRoundOne = 3,
    /// Second collective relinearization-key round.
    RkgRoundTwo = 4,
    /// Automorphism-linked source encryption for one collective Galois-key digit.
    GaloisSource = 5,
}

impl ZkAmsMkheActiveRoundV1 {
    fn tag(self) -> u8 {
        self as u8
    }

    fn decode(tag: u8) -> Result<Self, ZkAmsMkheErrorV1> {
        match tag {
            1 => Ok(Self::CollectivePublicKey),
            2 => Ok(Self::Cks),
            3 => Ok(Self::RkgRoundOne),
            4 => Ok(Self::RkgRoundTwo),
            5 => Ok(Self::GaloisSource),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

/// One exactly bound and authenticated active-party contribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheActiveContributionV1 {
    version: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    contribution_index: u32,
    party: ZkAmsMkhePartyIdV1,
    payload_digest: [u8; 32],
    authentication: ArtifactAuthentication,
}

impl ZkAmsMkheActiveContributionV1 {
    /// Bound ceremony round.
    #[must_use]
    pub const fn round(&self) -> ZkAmsMkheActiveRoundV1 {
        self.round
    }

    /// Canonical roster position.
    #[must_use]
    pub const fn contribution_index(&self) -> u32 {
        self.contribution_index
    }

    /// Authentication-key-derived contributor.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Digest of the exact canonical contribution payload and its proof.
    #[must_use]
    pub const fn payload_digest(&self) -> [u8; 32] {
        self.payload_digest
    }

    /// Consensus digest of every contribution field including authentication.
    pub fn digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let statement = active_contribution_statement_digest(self)?;
        let mut frame = Vec::with_capacity(256);
        frame.extend_from_slice(ACTIVE_CONTRIBUTION_DOMAIN_V1);
        frame.extend_from_slice(&statement);
        frame.extend_from_slice(&self.authentication.public_key);
        frame.extend_from_slice(&self.authentication.signature);
        Ok(keccak256(&frame))
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_active_contribution<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    contribution_index: usize,
    payload_digest: [u8; 32],
    authentication_secret: &AuthenticationSecret,
    random: &mut R,
) -> Result<ZkAmsMkheActiveContributionV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    if transcript_digest == [0; 32]
        || payload_digest == [0; 32]
        || contribution_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let participant = roster.participant(contribution_index)?;
    if authentication_secret.party_id()? != participant.party
        || authentication_secret.public_key()? != participant.authentication_public_key
    {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let mut contribution = ZkAmsMkheActiveContributionV1 {
        version: MKHE_VERSION_V1,
        profile_digest: roster.profile_digest,
        roster_digest: roster.roster_digest,
        epoch: roster.epoch,
        transcript_digest,
        round,
        contribution_index: u32::try_from(contribution_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        party: participant.party,
        payload_digest,
        authentication: ArtifactAuthentication {
            version: 0,
            party: participant.party,
            public_key: [0; 33],
            signature: [0; 65],
        },
    };
    let statement = active_contribution_statement_digest(&contribution)?;
    contribution.authentication = ArtifactAuthentication::sign(
        ACTIVE_CONTRIBUTION_DOMAIN_V1,
        statement,
        authentication_secret,
        random,
    )?;
    validate_active_contribution(
        roster,
        transcript_digest,
        round,
        contribution_index,
        &contribution,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
    Ok(contribution)
}

fn active_contribution_statement_digest(
    contribution: &ZkAmsMkheActiveContributionV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if contribution.version != MKHE_VERSION_V1
        || contribution.profile_digest == [0; 32]
        || contribution.roster_digest == [0; 32]
        || contribution.epoch == 0
        || contribution.transcript_digest == [0; 32]
        || contribution.payload_digest == [0; 32]
        || usize::try_from(contribution.contribution_index)
            .map_or(true, |index| index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(ACTIVE_CONTRIBUTION_DOMAIN_V1);
    frame.push(contribution.version);
    frame.extend_from_slice(&contribution.profile_digest);
    frame.extend_from_slice(&contribution.roster_digest);
    frame.extend_from_slice(&contribution.epoch.to_be_bytes());
    frame.extend_from_slice(&contribution.transcript_digest);
    frame.push(contribution.round.tag());
    frame.extend_from_slice(&contribution.contribution_index.to_be_bytes());
    frame.extend_from_slice(&contribution.party.to_bytes());
    frame.extend_from_slice(&contribution.payload_digest);
    Ok(keccak256(&frame))
}

fn validate_active_contribution(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    expected_index: usize,
    contribution: &ZkAmsMkheActiveContributionV1,
) -> Result<(), ZkAmsMkheAbortReasonV1> {
    if contribution.version != MKHE_VERSION_V1 {
        return Err(ZkAmsMkheAbortReasonV1::InvalidVersion);
    }
    if contribution.profile_digest != roster.profile_digest {
        return Err(ZkAmsMkheAbortReasonV1::SplicedProfile);
    }
    if contribution.roster_digest != roster.roster_digest {
        return Err(ZkAmsMkheAbortReasonV1::SplicedRoster);
    }
    if contribution.epoch != roster.epoch {
        return Err(ZkAmsMkheAbortReasonV1::SplicedEpoch);
    }
    if contribution.transcript_digest != transcript_digest {
        return Err(ZkAmsMkheAbortReasonV1::SplicedTranscript);
    }
    if contribution.round != round {
        return Err(ZkAmsMkheAbortReasonV1::SplicedRound);
    }
    if usize::try_from(contribution.contribution_index).ok() != Some(expected_index) {
        return Err(ZkAmsMkheAbortReasonV1::IndexMismatch);
    }
    if contribution.payload_digest == [0; 32] {
        return Err(ZkAmsMkheAbortReasonV1::InvalidPayload);
    }
    let participant = roster
        .participant(expected_index)
        .map_err(|_| ZkAmsMkheAbortReasonV1::UnexpectedContributor)?;
    if contribution.party != participant.party {
        return Err(ZkAmsMkheAbortReasonV1::ReorderedContributor);
    }
    if contribution.authentication.party != participant.party
        || contribution.authentication.public_key != participant.authentication_public_key
    {
        return Err(ZkAmsMkheAbortReasonV1::SplicedAuthenticationKey);
    }
    let statement = active_contribution_statement_digest(contribution)
        .map_err(|_| ZkAmsMkheAbortReasonV1::InvalidPayload)?;
    contribution
        .authentication
        .verify(ACTIVE_CONTRIBUTION_DOMAIN_V1, statement)
        .map_err(|_| ZkAmsMkheAbortReasonV1::InvalidAuthentication)
}

/// Stable first-failure reason carried by identifiable-abort evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheAbortReasonV1 {
    /// A required roster position was absent.
    MissingContributor = 1,
    /// A contributor already seen earlier in the same round reappeared.
    DuplicateContributor = 2,
    /// A valid roster member occupied a different member's position.
    ReorderedContributor = 3,
    /// A party outside the governed roster contributed.
    UnexpectedContributor = 4,
    /// The contribution carried a noncanonical position.
    IndexMismatch = 5,
    /// The contribution came from another profile.
    SplicedProfile = 6,
    /// The contribution came from another roster.
    SplicedRoster = 7,
    /// The contribution came from another secret epoch.
    SplicedEpoch = 8,
    /// The contribution came from another transcript.
    SplicedTranscript = 9,
    /// The contribution came from another active round.
    SplicedRound = 10,
    /// The authentication key did not match the governed roster key.
    SplicedAuthenticationKey = 11,
    /// The payload digest was absent or malformed.
    InvalidPayload = 12,
    /// The contribution authentication proof did not verify.
    InvalidAuthentication = 13,
    /// More than eight contributions were supplied.
    ExcessContributor = 14,
    /// The active record used an unsupported version.
    InvalidVersion = 15,
}

/// Deterministic evidence for the first invalid active-round position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheIdentifiableAbortV1 {
    round: ZkAmsMkheActiveRoundV1,
    expected_index: u32,
    expected_party: ZkAmsMkhePartyIdV1,
    observed_index: Option<u32>,
    observed_party: Option<ZkAmsMkhePartyIdV1>,
    observed_contribution_digest: [u8; 32],
    reason: ZkAmsMkheAbortReasonV1,
    evidence_digest: [u8; 32],
}

impl ZkAmsMkheIdentifiableAbortV1 {
    /// Failed round.
    #[must_use]
    pub const fn round(self) -> ZkAmsMkheActiveRoundV1 {
        self.round
    }

    /// First expected roster position that failed.
    #[must_use]
    pub const fn expected_index(self) -> u32 {
        self.expected_index
    }

    /// Governed party expected at the failed position.
    #[must_use]
    pub const fn expected_party(self) -> ZkAmsMkhePartyIdV1 {
        self.expected_party
    }

    /// Observed record index, or `None` when the record was missing.
    #[must_use]
    pub const fn observed_index(self) -> Option<u32> {
        self.observed_index
    }

    /// Observed party, or `None` when the record was missing.
    #[must_use]
    pub const fn observed_party(self) -> Option<ZkAmsMkhePartyIdV1> {
        self.observed_party
    }

    /// Digest of the observed contribution, or zero for a missing record.
    #[must_use]
    pub const fn observed_contribution_digest(self) -> [u8; 32] {
        self.observed_contribution_digest
    }

    /// Stable failure classification.
    #[must_use]
    pub const fn reason(self) -> ZkAmsMkheAbortReasonV1 {
        self.reason
    }

    /// Consensus digest of all evidence fields.
    #[must_use]
    pub const fn evidence_digest(self) -> [u8; 32] {
        self.evidence_digest
    }
}

/// Exact receipt for one complete eight-party active round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheActiveRoundReceiptV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    contribution_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    receipt_digest: [u8; 32],
}

impl ZkAmsMkheActiveRoundReceiptV1 {
    /// Completed round.
    #[must_use]
    pub const fn round(self) -> ZkAmsMkheActiveRoundV1 {
        self.round
    }

    /// Exact ordered contribution digests.
    #[must_use]
    pub const fn contribution_digests(&self) -> &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.contribution_digests
    }

    /// Consensus digest of the complete ordered round.
    #[must_use]
    pub const fn receipt_digest(self) -> [u8; 32] {
        self.receipt_digest
    }
}

/// Verify and collect exactly one contribution from every roster party in order.
pub fn zk_ams_mkhe_collect_active_round_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    contributions: &[ZkAmsMkheActiveContributionV1],
) -> Result<ZkAmsMkheActiveRoundReceiptV1, ZkAmsMkheIdentifiableAbortV1> {
    if roster.validate().is_err() || transcript_digest == [0; 32] {
        let expected = roster.participants[0];
        return Err(identifiable_abort(
            roster,
            round,
            0,
            expected.party,
            contributions.first(),
            ZkAmsMkheAbortReasonV1::SplicedRoster,
        ));
    }
    let mut seen = [false; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    let mut digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    for expected_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let expected = roster.participants[expected_index];
        let Some(contribution) = contributions.get(expected_index) else {
            return Err(identifiable_abort(
                roster,
                round,
                expected_index,
                expected.party,
                None,
                ZkAmsMkheAbortReasonV1::MissingContributor,
            ));
        };
        if let Some(observed_index) = roster.index_of(contribution.party) {
            if seen[observed_index] {
                return Err(identifiable_abort(
                    roster,
                    round,
                    expected_index,
                    expected.party,
                    Some(contribution),
                    ZkAmsMkheAbortReasonV1::DuplicateContributor,
                ));
            }
            if observed_index != expected_index {
                return Err(identifiable_abort(
                    roster,
                    round,
                    expected_index,
                    expected.party,
                    Some(contribution),
                    ZkAmsMkheAbortReasonV1::ReorderedContributor,
                ));
            }
            seen[observed_index] = true;
        } else {
            return Err(identifiable_abort(
                roster,
                round,
                expected_index,
                expected.party,
                Some(contribution),
                ZkAmsMkheAbortReasonV1::UnexpectedContributor,
            ));
        }
        if let Err(reason) = validate_active_contribution(
            roster,
            transcript_digest,
            round,
            expected_index,
            contribution,
        ) {
            return Err(identifiable_abort(
                roster,
                round,
                expected_index,
                expected.party,
                Some(contribution),
                reason,
            ));
        }
        digests[expected_index] = contribution
            .digest()
            .unwrap_or_else(|_| active_contribution_fallback_digest(contribution));
    }
    if let Some(excess) = contributions.get(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1) {
        return Err(identifiable_abort(
            roster,
            round,
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 - 1,
            roster.participants[ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 - 1].party,
            Some(excess),
            ZkAmsMkheAbortReasonV1::ExcessContributor,
        ));
    }
    let receipt_digest = active_round_receipt_digest(
        roster.profile_digest,
        roster.roster_digest,
        roster.epoch,
        transcript_digest,
        round,
        &digests,
    );
    Ok(ZkAmsMkheActiveRoundReceiptV1 {
        profile_digest: roster.profile_digest,
        roster_digest: roster.roster_digest,
        epoch: roster.epoch,
        transcript_digest,
        round,
        contribution_digests: digests,
        receipt_digest,
    })
}

fn active_round_receipt_digest(
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    contribution_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(384);
    frame.extend_from_slice(ACTIVE_ROUND_RECEIPT_DOMAIN_V1);
    frame.extend_from_slice(&profile_digest);
    frame.extend_from_slice(&roster_digest);
    frame.extend_from_slice(&epoch.to_be_bytes());
    frame.extend_from_slice(&transcript_digest);
    frame.push(round.tag());
    frame.push(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 as u8);
    for (index, digest) in contribution_digests.iter().enumerate() {
        frame.extend_from_slice(&(index as u32).to_be_bytes());
        frame.extend_from_slice(digest);
    }
    keccak256(&frame)
}

fn identifiable_abort(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    round: ZkAmsMkheActiveRoundV1,
    expected_index: usize,
    expected_party: ZkAmsMkhePartyIdV1,
    observed: Option<&ZkAmsMkheActiveContributionV1>,
    reason: ZkAmsMkheAbortReasonV1,
) -> ZkAmsMkheIdentifiableAbortV1 {
    let observed_index = observed.map(|value| value.contribution_index);
    let observed_party = observed.map(|value| value.party);
    let observed_contribution_digest = observed
        .map(|value| {
            value
                .digest()
                .unwrap_or_else(|_| active_contribution_fallback_digest(value))
        })
        .unwrap_or([0; 32]);
    let expected_index = u32::try_from(expected_index).unwrap_or(u32::MAX);
    let mut frame = Vec::with_capacity(320);
    frame.extend_from_slice(ACTIVE_ABORT_DOMAIN_V1);
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&roster.profile_digest);
    frame.extend_from_slice(&roster.roster_digest);
    frame.extend_from_slice(&roster.epoch.to_be_bytes());
    frame.push(round.tag());
    frame.extend_from_slice(&expected_index.to_be_bytes());
    frame.extend_from_slice(&expected_party.to_bytes());
    append_optional_u32(&mut frame, observed_index);
    append_optional_party(&mut frame, observed_party);
    frame.extend_from_slice(&observed_contribution_digest);
    frame.push(reason as u8);
    ZkAmsMkheIdentifiableAbortV1 {
        round,
        expected_index,
        expected_party,
        observed_index,
        observed_party,
        observed_contribution_digest,
        reason,
        evidence_digest: keccak256(&frame),
    }
}

fn append_optional_u32(frame: &mut Vec<u8>, value: Option<u32>) {
    frame.push(value.is_some().into());
    frame.extend_from_slice(&value.unwrap_or_default().to_be_bytes());
}

fn append_optional_party(frame: &mut Vec<u8>, value: Option<ZkAmsMkhePartyIdV1>) {
    frame.push(value.is_some().into());
    frame.extend_from_slice(&value.map_or([0; 32], ZkAmsMkhePartyIdV1::to_bytes));
}

fn active_contribution_fallback_digest(contribution: &ZkAmsMkheActiveContributionV1) -> [u8; 32] {
    let mut frame = Vec::with_capacity(320);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.invalid-active-contribution-evidence");
    frame.push(contribution.version);
    frame.extend_from_slice(&contribution.profile_digest);
    frame.extend_from_slice(&contribution.roster_digest);
    frame.extend_from_slice(&contribution.epoch.to_be_bytes());
    frame.extend_from_slice(&contribution.transcript_digest);
    frame.push(contribution.round.tag());
    frame.extend_from_slice(&contribution.contribution_index.to_be_bytes());
    frame.extend_from_slice(&contribution.party.to_bytes());
    frame.extend_from_slice(&contribution.payload_digest);
    frame.push(contribution.authentication.version);
    frame.extend_from_slice(&contribution.authentication.party.to_bytes());
    frame.extend_from_slice(&contribution.authentication.public_key);
    frame.extend_from_slice(&contribution.authentication.signature);
    keccak256(&frame)
}

/// Identity of the complete governed collective-key ceremony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheGovernedCollectiveKeyMaterialIdentityV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    public_key_receipt_digest: [u8; 32],
    cks_receipt_digest: [u8; 32],
    rkg_round_one_receipt_digest: [u8; 32],
    rkg_round_two_receipt_digest: [u8; 32],
    material_digest: [u8; 32],
}

impl ZkAmsMkheGovernedCollectiveKeyMaterialIdentityV1 {
    /// Construct material identity only from four complete, same-roster receipts.
    pub fn from_receipts(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        public_key: ZkAmsMkheActiveRoundReceiptV1,
        cks: ZkAmsMkheActiveRoundReceiptV1,
        rkg_round_one: ZkAmsMkheActiveRoundReceiptV1,
        rkg_round_two: ZkAmsMkheActiveRoundReceiptV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        let receipts = [public_key, cks, rkg_round_one, rkg_round_two];
        let expected_rounds = [
            ZkAmsMkheActiveRoundV1::CollectivePublicKey,
            ZkAmsMkheActiveRoundV1::Cks,
            ZkAmsMkheActiveRoundV1::RkgRoundOne,
            ZkAmsMkheActiveRoundV1::RkgRoundTwo,
        ];
        for (receipt, expected_round) in receipts.iter().zip(expected_rounds) {
            if receipt.profile_digest != roster.profile_digest
                || receipt.roster_digest != roster.roster_digest
                || receipt.epoch != roster.epoch
                || receipt.round != expected_round
                || receipt.receipt_digest == [0; 32]
                || receipt.receipt_digest
                    != active_round_receipt_digest(
                        receipt.profile_digest,
                        receipt.roster_digest,
                        receipt.epoch,
                        receipt.transcript_digest,
                        receipt.round,
                        &receipt.contribution_digests,
                    )
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        let mut frame = Vec::with_capacity(256);
        frame.extend_from_slice(ACTIVE_COLLECTIVE_KEY_MATERIAL_DOMAIN_V1);
        frame.push(MKHE_VERSION_V1);
        frame.extend_from_slice(&roster.profile_digest);
        frame.extend_from_slice(&roster.roster_digest);
        frame.extend_from_slice(&roster.epoch.to_be_bytes());
        for receipt in receipts {
            frame.push(receipt.round.tag());
            frame.extend_from_slice(&receipt.transcript_digest);
            frame.extend_from_slice(&receipt.receipt_digest);
        }
        Ok(Self {
            profile_digest: roster.profile_digest,
            roster_digest: roster.roster_digest,
            epoch: roster.epoch,
            public_key_receipt_digest: public_key.receipt_digest,
            cks_receipt_digest: cks.receipt_digest,
            rkg_round_one_receipt_digest: rkg_round_one.receipt_digest,
            rkg_round_two_receipt_digest: rkg_round_two.receipt_digest,
            material_digest: keccak256(&frame),
        })
    }

    /// Consensus digest of all four complete ordered contribution rounds.
    #[must_use]
    pub const fn material_digest(self) -> [u8; 32] {
        self.material_digest
    }

    /// Frozen release-profile digest.
    #[must_use]
    pub const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }

    /// Governed roster digest.
    #[must_use]
    pub const fn roster_digest(self) -> [u8; 32] {
        self.roster_digest
    }

    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Complete collective-public-key round receipt.
    #[must_use]
    pub const fn public_key_receipt_digest(self) -> [u8; 32] {
        self.public_key_receipt_digest
    }

    /// Complete CKS round receipt.
    #[must_use]
    pub const fn cks_receipt_digest(self) -> [u8; 32] {
        self.cks_receipt_digest
    }

    /// Complete RKG round-one receipt.
    #[must_use]
    pub const fn rkg_round_one_receipt_digest(self) -> [u8; 32] {
        self.rkg_round_one_receipt_digest
    }

    /// Complete RKG round-two receipt.
    #[must_use]
    pub const fn rkg_round_two_receipt_digest(self) -> [u8; 32] {
        self.rkg_round_two_receipt_digest
    }
}

/// Machine-checkable security parameters of the release RKG linear proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheActiveRkgLinearProofSecurityV1 {
    /// Frozen release ring degree.
    pub ring_degree: u32,
    /// Maximum bounded witness polynomials in one streamed proof.
    pub max_witness_polynomials: u8,
    /// Exact number of nonzero coefficients in the sparse challenge.
    pub challenge_weight: u8,
    /// Conservative lower bound for `log2(2^w * C(N,w))`.
    pub challenge_space_lower_bound_bits: u16,
    /// Random-oracle Fiat--Shamir output width.
    pub fiat_shamir_bits: u16,
    /// Minimum challenge-seed entropy in the random-oracle model.
    pub challenge_min_entropy_bits: u16,
    /// Collision-security level of the Keccak-256 transcript binding.
    pub transcript_binding_bits: u16,
    /// Overall claimed proof soundness after taking the weakest bound.
    pub soundness_bits: u16,
    /// Largest signed coefficient admitted by the released witness families.
    pub max_witness_coefficient: i64,
    /// Maximum absolute coefficient added by challenge multiplication.
    pub challenge_response_slack: i64,
    /// Exact uniform mask interval bound.
    pub mask_coefficient_bound: i64,
    /// Exact accepted signed response interval bound.
    pub response_coefficient_bound: i64,
    /// Number of response coordinates covered by the rejection union bound.
    pub max_response_coordinates: u64,
    /// Denominator in the per-attempt rejection union bound (`<= 1/denominator`).
    pub rejection_probability_denominator: u32,
    /// Hard Fiat--Shamir-with-aborts retry ceiling.
    pub retry_ceiling: u16,
    /// Conservative retry-exhaustion bound in bits.
    pub retry_exhaustion_bits: u16,
    /// Fixed big-endian two's-complement bytes per signed response coefficient.
    pub signed_coefficient_bytes: u8,
    /// Exact largest canonical proof encoding carried by this family.
    pub max_proof_bytes: u32,
    /// Smallest release RNS modulus, used to rule out signed-lift ambiguity.
    pub minimum_rns_modulus: u64,
    /// Digest of every active proof domain and exact numeric parameter above.
    pub parameter_digest: [u8; 32],
}

impl ZkAmsMkheActiveRkgLinearProofSecurityV1 {
    /// Recompute every arithmetic and domain-separation invariant.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = derive_active_rkg_linear_proof_security_v1()?;
        if self != expected
            || self.challenge_space_lower_bound_bits < self.fiat_shamir_bits
            || self.fiat_shamir_bits < 256
            || self.challenge_min_entropy_bits != self.fiat_shamir_bits
            || self.transcript_binding_bits < 128
            || self.soundness_bits
                != self
                    .challenge_min_entropy_bits
                    .min(self.transcript_binding_bits)
            || self.response_coefficient_bound <= 0
            || self.response_coefficient_bound
                >= (i64::try_from(self.minimum_rns_modulus)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                    - 1)
                    / 2
            || self.retry_exhaustion_bits < 256
            || self.signed_coefficient_bytes != 8
            || usize::try_from(self.max_proof_bytes).ok()
                != Some(linear_proof_wire_bytes(
                    usize::from(self.max_witness_polynomials),
                    usize::try_from(self.ring_degree)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                )?)
            || usize::try_from(self.max_proof_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                > super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
            || self.parameter_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return the exact release security certificate for active RKG linear proofs.
pub fn zk_ams_mkhe_active_rkg_linear_proof_security_v1()
-> Result<ZkAmsMkheActiveRkgLinearProofSecurityV1, ZkAmsMkheErrorV1> {
    let certificate = derive_active_rkg_linear_proof_security_v1()?;
    certificate.validate()?;
    Ok(certificate)
}

fn derive_active_rkg_linear_proof_security_v1()
-> Result<ZkAmsMkheActiveRkgLinearProofSecurityV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    let challenge_weight = linear_challenge_weight(profile.ring_degree)?;
    // `C(N,w) >= (N/w)^w`; adding one independent sign bit per
    // nonzero coefficient gives this deterministic conservative lower bound.
    let challenge_space_lower_bound_bits = u16::try_from(
        (profile.ring_degree / challenge_weight)
            .ilog2()
            .checked_add(1)
            .and_then(|bits| bits.checked_mul(challenge_weight as u32))
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    if profile.ring_degree != 1 << 17
        || challenge_weight != RKG_LINEAR_PROOF_RELEASE_CHALLENGE_WEIGHT_V1
        || challenge_space_lower_bound_bits != RKG_LINEAR_PROOF_CHALLENGE_SPACE_LOWER_BOUND_BITS_V1
        || RKG_LINEAR_PROOF_MASK_SLACK_FACTOR_V1 != 1 << 24
        || RANDOM_REJECTION_ATTEMPTS_V1 != 128
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let max_witness_coefficient = i64::from(profile.error_eta);
    let (mask_bound, response_bound) =
        linear_response_parameters(max_witness_coefficient, challenge_weight)?;
    let minimum_rns_modulus = profile
        .moduli
        .iter()
        .copied()
        .min()
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let minimum_rns_modulus_i64 =
        i64::try_from(minimum_rns_modulus).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let max_response_coordinates = u64::try_from(profile.ring_degree)
        .ok()
        .and_then(|degree| degree.checked_mul(RKG_LINEAR_PROOF_MAX_WITNESSES_V1 as u64))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let rejection_probability_denominator = u32::try_from(
        u64::try_from(RKG_LINEAR_PROOF_MASK_SLACK_FACTOR_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .checked_div(max_response_coordinates)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    if rejection_probability_denominator < 16 || response_bound >= (minimum_rns_modulus_i64 - 1) / 2
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let mut certificate = ZkAmsMkheActiveRkgLinearProofSecurityV1 {
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        max_witness_polynomials: u8::try_from(RKG_LINEAR_PROOF_MAX_WITNESSES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        challenge_weight: u8::try_from(challenge_weight)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        challenge_space_lower_bound_bits,
        fiat_shamir_bits: RKG_LINEAR_PROOF_FIAT_SHAMIR_BITS_V1,
        challenge_min_entropy_bits: RKG_LINEAR_PROOF_CHALLENGE_MIN_ENTROPY_BITS_V1,
        transcript_binding_bits: RKG_LINEAR_PROOF_TRANSCRIPT_BINDING_BITS_V1,
        soundness_bits: RKG_LINEAR_PROOF_SOUNDNESS_BITS_V1,
        max_witness_coefficient,
        challenge_response_slack: max_witness_coefficient
            .checked_mul(challenge_weight as i64)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        mask_coefficient_bound: mask_bound,
        response_coefficient_bound: response_bound,
        max_response_coordinates,
        rejection_probability_denominator,
        retry_ceiling: u16::try_from(RANDOM_REJECTION_ATTEMPTS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        retry_exhaustion_bits: RKG_LINEAR_PROOF_RETRY_EXHAUSTION_BITS_V1,
        signed_coefficient_bytes: RKG_LINEAR_PROOF_SIGNED_COEFFICIENT_BYTES_V1,
        max_proof_bytes: u32::try_from(linear_proof_wire_bytes(
            RKG_LINEAR_PROOF_MAX_WITNESSES_V1,
            profile.ring_degree,
        )?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        minimum_rns_modulus,
        parameter_digest: [0; 32],
    };
    certificate.parameter_digest = active_rkg_linear_proof_parameter_digest(certificate);
    Ok(certificate)
}

fn active_rkg_linear_proof_parameter_digest(
    certificate: ZkAmsMkheActiveRkgLinearProofSecurityV1,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(512);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.active-rkg-linear-proof-security");
    for domain in [
        super::wire::GOVERNED_ROSTER_DOMAIN_V1,
        ACTIVE_ROSTER_KEY_MATERIAL_DOMAIN_V1,
        ROSTER_POP_DOMAIN_V1,
        ACTIVE_CONTRIBUTION_DOMAIN_V1,
        ACTIVE_ROUND_RECEIPT_DOMAIN_V1,
        ACTIVE_ABORT_DOMAIN_V1,
        ACTIVE_COLLECTIVE_KEY_MATERIAL_DOMAIN_V1,
        ACTIVE_COLLECTIVE_PUBLIC_A_DOMAIN_V1,
        b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-context",
        b"iroha.zk-ams.v1.mkhe.rkg-linear-relation-statement",
        b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-fiat-shamir",
        b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-sparse-challenge",
        b"iroha.zk-ams.v1.mkhe.rkg-linear-relation-proof",
    ] {
        frame.extend_from_slice(&(domain.len() as u32).to_be_bytes());
        frame.extend_from_slice(domain);
    }
    frame.extend_from_slice(&certificate.ring_degree.to_be_bytes());
    frame.push(certificate.max_witness_polynomials);
    frame.push(certificate.challenge_weight);
    frame.extend_from_slice(&certificate.challenge_space_lower_bound_bits.to_be_bytes());
    frame.extend_from_slice(&certificate.fiat_shamir_bits.to_be_bytes());
    frame.extend_from_slice(&certificate.challenge_min_entropy_bits.to_be_bytes());
    frame.extend_from_slice(&certificate.transcript_binding_bits.to_be_bytes());
    frame.extend_from_slice(&certificate.soundness_bits.to_be_bytes());
    frame.extend_from_slice(&certificate.max_witness_coefficient.to_be_bytes());
    frame.extend_from_slice(&certificate.challenge_response_slack.to_be_bytes());
    frame.extend_from_slice(&certificate.mask_coefficient_bound.to_be_bytes());
    frame.extend_from_slice(&certificate.response_coefficient_bound.to_be_bytes());
    frame.extend_from_slice(&certificate.max_response_coordinates.to_be_bytes());
    frame.extend_from_slice(&certificate.rejection_probability_denominator.to_be_bytes());
    frame.extend_from_slice(&certificate.retry_ceiling.to_be_bytes());
    frame.extend_from_slice(&certificate.retry_exhaustion_bits.to_be_bytes());
    frame.push(certificate.signed_coefficient_bytes);
    frame.extend_from_slice(&certificate.max_proof_bytes.to_be_bytes());
    frame.extend_from_slice(&certificate.minimum_rns_modulus.to_be_bytes());
    frame.extend_from_slice(&RKG_LINEAR_PROOF_WIRE_TAG_V1);
    frame.extend_from_slice(&RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_U32_V1.to_be_bytes());
    frame.extend_from_slice(b"signed-i64-big-endian-twos-complement");
    keccak256(&frame)
}

/// Borrowed public statement linking one party's bounded secret to its
/// collective-public-key share.
#[derive(Clone, Copy, Debug)]
pub struct ZkAmsMkheActiveCollectivePublicKeyStatementV1<'a> {
    public_a: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: &'a super::ZkAmsMkheRnsPolynomialWireV1,
}

impl<'a> ZkAmsMkheActiveCollectivePublicKeyStatementV1<'a> {
    /// Construct the exact release statement `b_i = -a*s_i + t*e_i`.
    pub fn new(
        public_a: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        party_public_b: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        public_a.encoded_len()?;
        party_public_b.encoded_len()?;
        Ok(Self {
            public_a,
            party_public_b,
        })
    }

    /// Common public `a` polynomial.
    #[must_use]
    pub const fn public_a(&self) -> &'a super::ZkAmsMkheRnsPolynomialWireV1 {
        self.public_a
    }

    /// This party's public `b_i` contribution.
    #[must_use]
    pub const fn party_public_b(&self) -> &'a super::ZkAmsMkheRnsPolynomialWireV1 {
        self.party_public_b
    }
}

/// Borrowed bounded witnesses for one collective-public-key share.
#[derive(Clone, Copy)]
pub struct ZkAmsMkheActiveCollectivePublicKeyWitnessV1<'a> {
    secret: &'a [i64],
    public_error: &'a [i64],
}

impl core::fmt::Debug for ZkAmsMkheActiveCollectivePublicKeyWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsMkheActiveCollectivePublicKeyWitnessV1([REDACTED])")
    }
}

impl<'a> ZkAmsMkheActiveCollectivePublicKeyWitnessV1<'a> {
    /// Construct witnesses with exact release dimensions and coefficient bounds.
    pub fn new(secret: &'a [i64], public_error: &'a [i64]) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_release_narrow_witness(secret, 1)?;
        validate_release_narrow_witness(public_error, 2)?;
        Ok(Self {
            secret,
            public_error,
        })
    }
}

/// Borrowed public statement for one streamed RKG round-one digit.
#[derive(Clone, Copy, Debug)]
pub struct ZkAmsMkheActiveRkgRoundOneStatementV1<'a> {
    public_key: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'a>,
    common_a: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    h0: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    h1: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    left: ZkAmsMkhePartyIdV1,
    right: ZkAmsMkhePartyIdV1,
    digit_index: u32,
}

impl<'a> ZkAmsMkheActiveRkgRoundOneStatementV1<'a> {
    /// Construct the exact two-equation RKG round-one statement for one digit.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        public_key: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'a>,
        common_a: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        h0: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        h1: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        left: ZkAmsMkhePartyIdV1,
        right: ZkAmsMkhePartyIdV1,
        digit_index: u32,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        if left > right
            || usize::try_from(digit_index)
                .ok()
                .is_none_or(|digit| digit >= profile.gadget_digits)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        common_a.encoded_len()?;
        h0.encoded_len()?;
        h1.encoded_len()?;
        Ok(Self {
            public_key,
            common_a,
            h0,
            h1,
            left,
            right,
            digit_index,
        })
    }

    /// Canonical hybrid-RNS digit index.
    #[must_use]
    pub const fn digit_index(self) -> u32 {
        self.digit_index
    }
}

/// Borrowed bounded witnesses for one streamed RKG round-one digit.
#[derive(Clone, Copy)]
pub struct ZkAmsMkheActiveRkgRoundOneWitnessV1<'a> {
    secret: &'a [i64],
    public_error: &'a [i64],
    ephemeral: &'a [i64],
    error_zero: &'a [i64],
    error_one: &'a [i64],
}

impl core::fmt::Debug for ZkAmsMkheActiveRkgRoundOneWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsMkheActiveRkgRoundOneWitnessV1([REDACTED])")
    }
}

impl<'a> ZkAmsMkheActiveRkgRoundOneWitnessV1<'a> {
    /// Construct all five witnesses with exact release dimensions and bounds.
    pub fn new(
        secret: &'a [i64],
        public_error: &'a [i64],
        ephemeral: &'a [i64],
        error_zero: &'a [i64],
        error_one: &'a [i64],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_release_narrow_witness(secret, 1)?;
        validate_release_narrow_witness(public_error, 2)?;
        validate_release_narrow_witness(ephemeral, 1)?;
        validate_release_narrow_witness(error_zero, 2)?;
        validate_release_narrow_witness(error_one, 2)?;
        Ok(Self {
            secret,
            public_error,
            ephemeral,
            error_zero,
            error_one,
        })
    }
}

/// Borrowed public statement for one streamed RKG round-two digit.
#[derive(Clone, Copy, Debug)]
pub struct ZkAmsMkheActiveRkgRoundTwoStatementV1<'a> {
    round_one: ZkAmsMkheActiveRkgRoundOneStatementV1<'a>,
    aggregate_h0: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    aggregate_h1: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    k0: &'a super::ZkAmsMkheRnsPolynomialWireV1,
}

impl<'a> ZkAmsMkheActiveRkgRoundTwoStatementV1<'a> {
    /// Construct the exact round-two statement, including this party's
    /// round-one equations to equality-link the ephemeral witness.
    pub fn new(
        round_one: ZkAmsMkheActiveRkgRoundOneStatementV1<'a>,
        aggregate_h0: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        aggregate_h1: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        k0: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        aggregate_h0.encoded_len()?;
        aggregate_h1.encoded_len()?;
        k0.encoded_len()?;
        Ok(Self {
            round_one,
            aggregate_h0,
            aggregate_h1,
            k0,
        })
    }

    /// Canonical hybrid-RNS digit index inherited from round one.
    #[must_use]
    pub const fn digit_index(self) -> u32 {
        self.round_one.digit_index
    }
}

/// Borrowed bounded witnesses for one streamed RKG round-two digit.
#[derive(Clone, Copy)]
pub struct ZkAmsMkheActiveRkgRoundTwoWitnessV1<'a> {
    round_one: ZkAmsMkheActiveRkgRoundOneWitnessV1<'a>,
    error_two: &'a [i64],
}

impl core::fmt::Debug for ZkAmsMkheActiveRkgRoundTwoWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsMkheActiveRkgRoundTwoWitnessV1([REDACTED])")
    }
}

impl<'a> ZkAmsMkheActiveRkgRoundTwoWitnessV1<'a> {
    /// Construct the round-two witness while retaining every round-one opening.
    pub fn new(
        round_one: ZkAmsMkheActiveRkgRoundOneWitnessV1<'a>,
        error_two: &'a [i64],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_release_narrow_witness(error_two, 2)?;
        Ok(Self {
            round_one,
            error_two,
        })
    }
}

/// Borrowed exact relation for one automorphism-linked Galois-source digit.
#[derive(Clone, Copy, Debug)]
pub(super) struct ZkAmsMkheActiveGaloisSourceStatementV1<'a> {
    public_key: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'a>,
    source_constant: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    source_linear: &'a super::ZkAmsMkheRnsPolynomialWireV1,
    schedule_index: u8,
    exponent: u32,
    digit_index: u32,
}

impl<'a> ZkAmsMkheActiveGaloisSourceStatementV1<'a> {
    /// Bind a verified public share and exact source ciphertext to the frozen schedule.
    pub(super) fn new(
        public_key: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'a>,
        source_constant: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        source_linear: &'a super::ZkAmsMkheRnsPolynomialWireV1,
        schedule_index: usize,
        exponent: u32,
        digit_index: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_galois_source_coordinate(schedule_index, exponent, digit_index)?;
        source_constant.encoded_len()?;
        source_linear.encoded_len()?;
        Ok(Self {
            public_key,
            source_constant,
            source_linear,
            schedule_index: u8::try_from(schedule_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            exponent,
            digit_index: u32::try_from(digit_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        })
    }
}

fn validate_galois_source_coordinate(
    schedule_index: usize,
    exponent: u32,
    digit_index: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let schedule = zk_ams_t256_galois_key_schedule_v1()?;
    validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
    if schedule_index >= ZK_AMS_T256_GALOIS_KEY_COUNT_V1
        || schedule
            .entries
            .get(schedule_index)
            .is_none_or(|entry| entry.exponent != exponent)
        || digit_index >= profile.gadget_digits
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

/// Borrowed bounded witness for one automorphism-linked Galois-source digit.
#[derive(Clone, Copy)]
pub(super) struct ZkAmsMkheActiveGaloisSourceWitnessV1<'a> {
    secret: &'a [i64],
    public_error: &'a [i64],
    ephemeral: &'a [i64],
    error_zero: &'a [i64],
    error_one: &'a [i64],
}

impl core::fmt::Debug for ZkAmsMkheActiveGaloisSourceWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkAmsMkheActiveGaloisSourceWitnessV1([REDACTED])")
    }
}

impl<'a> ZkAmsMkheActiveGaloisSourceWitnessV1<'a> {
    /// Validate exact release dimensions and every narrow coefficient bound.
    pub(super) fn new(
        secret: &'a [i64],
        public_error: &'a [i64],
        ephemeral: &'a [i64],
        error_zero: &'a [i64],
        error_one: &'a [i64],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_release_narrow_witness(secret, 1)?;
        validate_release_narrow_witness(public_error, 2)?;
        validate_release_narrow_witness(ephemeral, 1)?;
        validate_release_narrow_witness(error_zero, 2)?;
        validate_release_narrow_witness(error_one, 2)?;
        Ok(Self {
            secret,
            public_error,
            ephemeral,
            error_zero,
            error_one,
        })
    }
}

/// Authenticated canonical narrow-coefficient proof for one collective-key or
/// streamed RKG record.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkAmsMkheActiveRkgProofV1 {
    statement_digest: [u8; 32],
    witness_polynomials: u8,
    proof_bytes: Vec<u8>,
    contribution: ZkAmsMkheActiveContributionV1,
}

impl core::fmt::Debug for ZkAmsMkheActiveRkgProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheActiveRkgProofV1")
            .field("statement_digest", &hex::encode(self.statement_digest))
            .field("witness_polynomials", &self.witness_polynomials)
            .field("proof_bytes_len", &self.proof_bytes.len())
            .field("contribution", &self.contribution)
            .finish()
    }
}

impl ZkAmsMkheActiveRkgProofV1 {
    /// Digest of the complete exact algebraic statement.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }

    /// Exact number of bounded witness polynomials.
    #[must_use]
    pub const fn witness_polynomials(&self) -> u8 {
        self.witness_polynomials
    }

    /// Canonical fixed-width Fiat--Shamir-with-aborts proof bytes.
    #[must_use]
    pub fn proof_bytes(&self) -> &[u8] {
        &self.proof_bytes
    }

    /// Authenticated active-round contribution whose payload is this proof.
    #[must_use]
    pub const fn contribution(&self) -> &ZkAmsMkheActiveContributionV1 {
        &self.contribution
    }

    /// Exact bytes in the complete durable proof/authentication record.
    pub fn evidence_encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        self.validate_evidence()
    }

    /// Visit the complete durable proof/authentication record without first
    /// materializing a second release-sized byte vector.
    ///
    /// The algebraic public statement is intentionally carried by the enclosing
    /// evaluated-key evidence record; this stream preserves every proof and
    /// authenticated-contribution byte needed to replay it.
    pub fn write_evidence_chunks(
        &self,
        mut write: impl FnMut(&[u8]) -> Result<(), ZkAmsMkheErrorV1>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_evidence()?;
        write(&ACTIVE_RKG_EVIDENCE_TAG_V1)?;
        write(&[MKHE_VERSION_V1])?;
        write(&self.statement_digest)?;
        write(&[self.witness_polynomials])?;
        write(
            &u32::try_from(self.proof_bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        )?;
        write(&self.proof_bytes)?;
        let contribution = &self.contribution;
        write(&[contribution.version])?;
        write(&contribution.profile_digest)?;
        write(&contribution.roster_digest)?;
        write(&contribution.epoch.to_be_bytes())?;
        write(&contribution.transcript_digest)?;
        write(&[contribution.round.tag()])?;
        write(&contribution.contribution_index.to_be_bytes())?;
        write(&contribution.party.to_bytes())?;
        write(&contribution.payload_digest)?;
        write(&[contribution.authentication.version])?;
        write(&contribution.authentication.party.to_bytes())?;
        write(&contribution.authentication.public_key)?;
        write(&contribution.authentication.signature)
    }

    /// Encode the complete durable audit record, including authentication bytes.
    pub fn encode_evidence(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let total = self.evidence_encoded_len()?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.write_evidence_chunks(|chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        })?;
        if bytes.len() != total {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    fn validate_evidence(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        if self.statement_digest == [0; 32]
            || self.witness_polynomials == 0
            || usize::from(self.witness_polynomials) > RKG_LINEAR_PROOF_MAX_WITNESSES_V1
            || self.proof_bytes.len()
                != linear_proof_wire_bytes(
                    usize::from(self.witness_polynomials),
                    profile.ring_degree,
                )?
            || self.contribution.authentication.version != MKHE_VERSION_V1
            || self.contribution.authentication.party != self.contribution.party
            || ZkAmsMkhePartyIdV1::from_authentication_key(
                &self.contribution.authentication.public_key,
            )? != self.contribution.party
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let contribution_statement = active_contribution_statement_digest(&self.contribution)?;
        self.contribution
            .authentication
            .verify(ACTIVE_CONTRIBUTION_DOMAIN_V1, contribution_statement)?;
        ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1
            .checked_add(self.proof_bytes.len())
            .and_then(|value| value.checked_add(ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }

    /// Decode one exact durable audit record with complete length preflight.
    pub fn decode_evidence_exact(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len()
            < ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1 + ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1
            || bytes.get(..4) != Some(ACTIVE_RKG_EVIDENCE_TAG_V1.as_slice())
            || bytes.get(4).copied() != Some(MKHE_VERSION_V1)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let witness_polynomials = bytes
            .get(37)
            .copied()
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if witness_polynomials == 0
            || usize::from(witness_polynomials) > RKG_LINEAR_PROOF_MAX_WITNESSES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let proof_len = u32::from_be_bytes(
            bytes
                .get(38..42)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        );
        let proof_len =
            usize::try_from(proof_len).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let profile = release_profile_v1();
        if proof_len
            != linear_proof_wire_bytes(usize::from(witness_polynomials), profile.ring_degree)?
            || ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1
                .checked_add(proof_len)
                .and_then(|value| value.checked_add(ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1))
                != Some(bytes.len())
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let statement_digest = bytes
            .get(5..37)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let mut cursor = ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1;
        let proof_bytes = bytes
            .get(cursor..cursor + proof_len)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .to_vec();
        LinearRelationProofV1::decode_wire_exact(
            &proof_bytes,
            usize::from(witness_polynomials),
            profile.ring_degree,
        )?;
        cursor += proof_len;
        let contribution = decode_active_evidence_contribution_exact(
            bytes
                .get(cursor..)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        )?;
        let value = Self {
            statement_digest,
            witness_polynomials,
            proof_bytes,
            contribution,
        };
        if value.statement_digest == [0; 32] || value.evidence_encoded_len()? != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(value)
    }

    /// Decode one exact framed proof record from a reader without retaining a
    /// second encoded copy. The caller supplies the enclosing record's trusted
    /// length and owns any desired EOF check.
    pub fn decode_evidence_from_reader<R: std::io::Read>(
        reader: &mut R,
        encoded_len: u64,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let encoded_len =
            usize::try_from(encoded_len).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let minimum = ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1
            .checked_add(ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if encoded_len < minimum {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut header = [0_u8; ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1];
        read_active_evidence_io_exact(reader, &mut header)?;
        if header[..4] != ACTIVE_RKG_EVIDENCE_TAG_V1 || header[4] != MKHE_VERSION_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let statement_digest: [u8; 32] = header[5..37]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let witness_polynomials = header[37];
        if statement_digest == [0; 32]
            || witness_polynomials == 0
            || usize::from(witness_polynomials) > RKG_LINEAR_PROOF_MAX_WITNESSES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let proof_len = usize::try_from(u32::from_be_bytes(
            header[38..42]
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        ))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let profile = release_profile_v1();
        if proof_len
            != linear_proof_wire_bytes(usize::from(witness_polynomials), profile.ring_degree)?
            || minimum.checked_add(proof_len) != Some(encoded_len)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut proof_bytes = Vec::new();
        proof_bytes
            .try_reserve_exact(proof_len)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        proof_bytes.resize(proof_len, 0);
        read_active_evidence_io_exact(reader, &mut proof_bytes)?;
        LinearRelationProofV1::decode_wire_exact(
            &proof_bytes,
            usize::from(witness_polynomials),
            profile.ring_degree,
        )?;
        let mut contribution_bytes = [0_u8; ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1];
        read_active_evidence_io_exact(reader, &mut contribution_bytes)?;
        let contribution = decode_active_evidence_contribution_exact(&contribution_bytes)?;
        let value = Self {
            statement_digest,
            witness_polynomials,
            proof_bytes,
            contribution,
        };
        if value.evidence_encoded_len()? != encoded_len {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(value)
    }
}

fn decode_active_evidence_contribution_exact(
    bytes: &[u8],
) -> Result<ZkAmsMkheActiveContributionV1, ZkAmsMkheErrorV1> {
    if bytes.len() != ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut cursor = 0;
    let contribution_version = read_active_evidence_u8(bytes, &mut cursor)?;
    let profile_digest = read_active_evidence_array(bytes, &mut cursor)?;
    let roster_digest = read_active_evidence_array(bytes, &mut cursor)?;
    let epoch = u64::from_be_bytes(read_active_evidence_array(bytes, &mut cursor)?);
    let transcript_digest = read_active_evidence_array(bytes, &mut cursor)?;
    let round = ZkAmsMkheActiveRoundV1::decode(read_active_evidence_u8(bytes, &mut cursor)?)?;
    let contribution_index = u32::from_be_bytes(read_active_evidence_array(bytes, &mut cursor)?);
    let party = ZkAmsMkhePartyIdV1::new(read_active_evidence_array(bytes, &mut cursor)?)?;
    let payload_digest = read_active_evidence_array(bytes, &mut cursor)?;
    let authentication_version = read_active_evidence_u8(bytes, &mut cursor)?;
    let authentication_party =
        ZkAmsMkhePartyIdV1::new(read_active_evidence_array(bytes, &mut cursor)?)?;
    let authentication_public_key = read_active_evidence_array(bytes, &mut cursor)?;
    let authentication_signature = read_active_evidence_array(bytes, &mut cursor)?;
    if cursor != bytes.len()
        || contribution_version != MKHE_VERSION_V1
        || authentication_version != MKHE_VERSION_V1
        || authentication_party != party
        || ZkAmsMkhePartyIdV1::from_authentication_key(&authentication_public_key)? != party
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let contribution = ZkAmsMkheActiveContributionV1 {
        version: contribution_version,
        profile_digest,
        roster_digest,
        epoch,
        transcript_digest,
        round,
        contribution_index,
        party,
        payload_digest,
        authentication: ArtifactAuthentication {
            version: authentication_version,
            party: authentication_party,
            public_key: authentication_public_key,
            signature: authentication_signature,
        },
    };
    let statement = active_contribution_statement_digest(&contribution)?;
    contribution
        .authentication
        .verify(ACTIVE_CONTRIBUTION_DOMAIN_V1, statement)?;
    Ok(contribution)
}

fn read_active_evidence_io_exact(
    reader: &mut impl std::io::Read,
    bytes: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    reader
        .read_exact(bytes)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}

fn read_active_evidence_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(N)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor = end;
    Ok(value)
}

fn read_active_evidence_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, ZkAmsMkheErrorV1> {
    let value = bytes
        .get(*cursor)
        .copied()
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor = cursor
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    Ok(value)
}

/// Derive the sole uniformly sampled collective-public-key `a` polynomial for
/// a governed roster and protocol transcript.
pub fn zk_ams_mkhe_active_collective_public_a_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
) -> Result<super::ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let polynomial = derive_active_collective_public_a(&profile, roster, transcript_digest)?;
    super::ZkAmsMkheRnsPolynomialWireV1::new(polynomial.coefficients)
}

/// Prove and authenticate one bounded collective-public-key share relation.
#[allow(clippy::too_many_arguments)]
pub fn prove_zk_ams_mkhe_active_collective_public_key_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'_>,
    witness: ZkAmsMkheActiveCollectivePublicKeyWitnessV1<'_>,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheActiveRkgProofV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    validate_collective_public_a(&profile, roster, transcript_digest, statement.public_a)?;
    let context = active_linear_context(
        &profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        party_index,
        u32::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        0,
    )?;
    let relation = collective_public_key_relation(&profile, statement)?;
    let witnesses = vec![
        secret_polynomial_exact(&profile, witness.secret, 1)?,
        secret_polynomial_exact(&profile, witness.public_error, i64::from(profile.error_eta))?,
    ];
    prove_authenticated_active_relation(
        &profile,
        roster,
        context,
        &relation,
        witnesses,
        &party_secret.authentication,
        random,
    )
}

/// Verify one authenticated collective-public-key share proof against an
/// independently trusted roster position and transcript.
pub fn verify_zk_ams_mkhe_active_collective_public_key_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'_>,
    proof: &ZkAmsMkheActiveRkgProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    validate_collective_public_a(&profile, roster, transcript_digest, statement.public_a)?;
    let context = active_linear_context(
        &profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        party_index,
        u32::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        0,
    )?;
    let relation = collective_public_key_relation(&profile, statement)?;
    verify_authenticated_active_relation(&profile, roster, context, &relation, proof)
}

/// Prove and authenticate one exact streamed RKG round-one pair/digit record.
#[allow(clippy::too_many_arguments)]
pub fn prove_zk_ams_mkhe_active_rkg_round_one_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveRkgRoundOneStatementV1<'_>,
    witness: ZkAmsMkheActiveRkgRoundOneWitnessV1<'_>,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheActiveRkgProofV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let context = rkg_linear_context(
        &profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        party_index,
        statement,
    )?;
    let party = context.party;
    let relation = rkg_round_one_relation(&profile, statement, party)?;
    let witnesses = round_one_witness_polynomials(&profile, witness)?;
    prove_authenticated_active_relation(
        &profile,
        roster,
        context,
        &relation,
        witnesses,
        &party_secret.authentication,
        random,
    )
}

/// Verify one exact streamed RKG round-one pair/digit proof.
pub fn verify_zk_ams_mkhe_active_rkg_round_one_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveRkgRoundOneStatementV1<'_>,
    proof: &ZkAmsMkheActiveRkgProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let context = rkg_linear_context(
        &profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        party_index,
        statement,
    )?;
    let relation = rkg_round_one_relation(&profile, statement, context.party)?;
    verify_authenticated_active_relation(&profile, roster, context, &relation, proof)
}

/// Prove and authenticate one exact streamed RKG round-two pair/digit record.
#[allow(clippy::too_many_arguments)]
pub fn prove_zk_ams_mkhe_active_rkg_round_two_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveRkgRoundTwoStatementV1<'_>,
    witness: ZkAmsMkheActiveRkgRoundTwoWitnessV1<'_>,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheActiveRkgProofV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let context = rkg_linear_context(
        &profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::RkgRoundTwo,
        party_index,
        statement.round_one,
    )?;
    let relation = rkg_round_two_relation(&profile, statement, context.party)?;
    let mut witnesses = round_one_witness_polynomials(&profile, witness.round_one)?;
    witnesses.push(secret_polynomial_exact(
        &profile,
        witness.error_two,
        i64::from(profile.error_eta),
    )?);
    prove_authenticated_active_relation(
        &profile,
        roster,
        context,
        &relation,
        witnesses,
        &party_secret.authentication,
        random,
    )
}

/// Verify one exact streamed RKG round-two pair/digit proof.
pub fn verify_zk_ams_mkhe_active_rkg_round_two_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveRkgRoundTwoStatementV1<'_>,
    proof: &ZkAmsMkheActiveRkgProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let context = rkg_linear_context(
        &profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::RkgRoundTwo,
        party_index,
        statement.round_one,
    )?;
    let relation = rkg_round_two_relation(&profile, statement, context.party)?;
    verify_authenticated_active_relation(&profile, roster, context, &relation, proof)
}

/// Prove and authenticate one exact automorphism-linked Galois-source digit.
pub(super) fn prove_zk_ams_mkhe_active_galois_source_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveGaloisSourceStatementV1<'_>,
    witness: ZkAmsMkheActiveGaloisSourceWitnessV1<'_>,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheActiveRkgProofV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let context =
        galois_source_linear_context(&profile, roster, transcript_digest, party_index, statement)?;
    let relation = galois_source_relation(&profile, statement)?;
    let witnesses = vec![
        secret_polynomial_exact(&profile, witness.secret, 1)?,
        secret_polynomial_exact(&profile, witness.public_error, i64::from(profile.error_eta))?,
        secret_polynomial_exact(&profile, witness.ephemeral, 1)?,
        secret_polynomial_exact(&profile, witness.error_zero, i64::from(profile.error_eta))?,
        secret_polynomial_exact(&profile, witness.error_one, i64::from(profile.error_eta))?,
    ];
    prove_authenticated_active_relation(
        &profile,
        roster,
        context,
        &relation,
        witnesses,
        &party_secret.authentication,
        random,
    )
}

/// Verify one automorphism-linked Galois-source digit against trusted context.
pub(super) fn verify_zk_ams_mkhe_active_galois_source_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveGaloisSourceStatementV1<'_>,
    proof: &ZkAmsMkheActiveRkgProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let context =
        galois_source_linear_context(&profile, roster, transcript_digest, party_index, statement)?;
    let relation = galois_source_relation(&profile, statement)?;
    verify_authenticated_active_relation(&profile, roster, context, &relation, proof)
}

fn derive_active_collective_public_a(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
) -> Result<super::RnsPolynomial, ZkAmsMkheErrorV1> {
    roster.validate()?;
    if transcript_digest == [0; 32] || roster.profile_digest != profile.digest()? {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut context = Vec::with_capacity(74);
    context.push(MKHE_VERSION_V1);
    context.extend_from_slice(&roster.roster_digest);
    context.extend_from_slice(&roster.epoch.to_be_bytes());
    context.extend_from_slice(&transcript_digest);
    super::derive_uniform_rns_from_context(profile, ACTIVE_COLLECTIVE_PUBLIC_A_DOMAIN_V1, &context)
}

fn validate_collective_public_a(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    public_a: &super::ZkAmsMkheRnsPolynomialWireV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let provided = release_wire_polynomial(profile, public_a)?;
    if provided != derive_active_collective_public_a(profile, roster, transcript_digest)? {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn active_linear_context(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    party_index: usize,
    record_index: u32,
    relation_index: u32,
) -> Result<LinearProofContextV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    let participant = roster.participant(party_index)?;
    let context = LinearProofContextV1 {
        profile_digest: profile.digest()?,
        roster_digest: roster.roster_digest,
        epoch: roster.epoch,
        transcript_digest,
        round,
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        party: participant.party,
        record_index,
        relation_index,
    };
    context.validate(profile)?;
    Ok(context)
}

fn rkg_linear_context(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    party_index: usize,
    statement: ZkAmsMkheActiveRkgRoundOneStatementV1<'_>,
) -> Result<LinearProofContextV1, ZkAmsMkheErrorV1> {
    if !matches!(
        round,
        ZkAmsMkheActiveRoundV1::RkgRoundOne | ZkAmsMkheActiveRoundV1::RkgRoundTwo
    ) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_collective_public_a(
        profile,
        roster,
        transcript_digest,
        statement.public_key.public_a,
    )?;
    let pair_index = canonical_rkg_pair_index(roster, statement.left, statement.right)?;
    let digit =
        usize::try_from(statement.digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if digit >= profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let party_set = active_party_set(roster)?;
    let provided_common_a = release_wire_polynomial(profile, statement.common_a)?;
    let expected_common_a = super::derive_rkg_common_a(
        profile,
        &party_set,
        transcript_digest,
        statement.left,
        statement.right,
        digit,
    )?;
    if provided_common_a != expected_common_a {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let gadget_digits =
        u32::try_from(profile.gadget_digits).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let record_index = pair_index
        .checked_mul(gadget_digits)
        .and_then(|base| base.checked_add(statement.digit_index))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    active_linear_context(
        profile,
        roster,
        transcript_digest,
        round,
        party_index,
        record_index,
        pair_index,
    )
}

fn galois_source_linear_context(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: ZkAmsMkheActiveGaloisSourceStatementV1<'_>,
) -> Result<LinearProofContextV1, ZkAmsMkheErrorV1> {
    validate_collective_public_a(
        profile,
        roster,
        transcript_digest,
        statement.public_key.public_a,
    )?;
    let schedule = zk_ams_t256_galois_key_schedule_v1()?;
    validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
    let schedule_index = usize::from(statement.schedule_index);
    let digit_index =
        usize::try_from(statement.digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if schedule
        .entries
        .get(schedule_index)
        .is_none_or(|entry| entry.exponent != statement.exponent)
        || digit_index >= profile.gadget_digits
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let record_index = schedule_index
        .checked_mul(profile.gadget_digits)
        .and_then(|base| base.checked_add(digit_index))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    active_linear_context(
        profile,
        roster,
        transcript_digest,
        ZkAmsMkheActiveRoundV1::GaloisSource,
        party_index,
        record_index,
        statement.exponent,
    )
}

fn active_party_set(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
) -> Result<super::PartySet, ZkAmsMkheErrorV1> {
    roster.validate()?;
    super::PartySet::new(
        roster
            .participants
            .iter()
            .map(|participant| participant.party)
            .collect(),
    )
}

fn canonical_rkg_pair_index(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    left: ZkAmsMkhePartyIdV1,
    right: ZkAmsMkhePartyIdV1,
) -> Result<u32, ZkAmsMkheErrorV1> {
    roster.validate()?;
    let left_index = roster
        .index_of(left)
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let right_index = roster
        .index_of(right)
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if left_index > right_index {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut pair_index = 0_u32;
    for row in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        for column in row..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            if row == left_index && column == right_index {
                return Ok(pair_index);
            }
            pair_index = pair_index
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
}

fn release_wire_polynomial(
    profile: &super::BgvProfile,
    polynomial: &super::ZkAmsMkheRnsPolynomialWireV1,
) -> Result<super::RnsPolynomial, ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    super::RnsPolynomial::from_flat(profile, polynomial.residues().to_vec())
}

fn ring_one(profile: &super::BgvProfile) -> Result<super::RnsPolynomial, ZkAmsMkheErrorV1> {
    let mut coefficients = vec![0_u64; profile.ring_degree * profile.moduli.len()];
    for limb in 0..profile.moduli.len() {
        coefficients[limb * profile.ring_degree] = 1;
    }
    super::RnsPolynomial::from_flat(profile, coefficients)
}

fn push_nonzero_term(
    terms: &mut Vec<LinearRelationTermV1>,
    witness_index: usize,
    multiplier: super::RnsPolynomial,
) {
    if multiplier
        .coefficients
        .iter()
        .any(|coefficient| *coefficient != 0)
    {
        terms.push(LinearRelationTermV1 {
            witness_index,
            multiplier,
            witness_automorphism_exponent: 1,
        });
    }
}

fn collective_public_key_relation(
    profile: &super::BgvProfile,
    statement: ZkAmsMkheActiveCollectivePublicKeyStatementV1<'_>,
) -> Result<LinearRelationStatementV1, ZkAmsMkheErrorV1> {
    let public_a = release_wire_polynomial(profile, statement.public_a)?;
    let public_b = release_wire_polynomial(profile, statement.party_public_b)?;
    let plaintext_multiplier = ring_one(profile)?.scale_plaintext_modulus(profile)?;
    let relation = LinearRelationStatementV1 {
        witness_bounds: vec![1, i64::from(profile.error_eta)],
        witness_challenge_automorphism_exponents: vec![1, 1],
        outputs: vec![LinearRelationOutputV1 {
            target: public_b,
            challenge_automorphism_exponent: 1,
            terms: vec![
                LinearRelationTermV1 {
                    witness_index: 0,
                    multiplier: public_a.negate(profile)?,
                    witness_automorphism_exponent: 1,
                },
                LinearRelationTermV1 {
                    witness_index: 1,
                    multiplier: plaintext_multiplier,
                    witness_automorphism_exponent: 1,
                },
            ],
        }],
    };
    relation.validate(profile)?;
    Ok(relation)
}

fn rkg_round_one_relation(
    profile: &super::BgvProfile,
    statement: ZkAmsMkheActiveRkgRoundOneStatementV1<'_>,
    party: ZkAmsMkhePartyIdV1,
) -> Result<LinearRelationStatementV1, ZkAmsMkheErrorV1> {
    let mut relation = collective_public_key_relation(profile, statement.public_key)?;
    relation.witness_bounds.extend([
        1,
        i64::from(profile.error_eta),
        i64::from(profile.error_eta),
    ]);
    relation
        .witness_challenge_automorphism_exponents
        .extend([1, 1, 1]);
    let common_a = release_wire_polynomial(profile, statement.common_a)?;
    let plaintext_multiplier = ring_one(profile)?.scale_plaintext_modulus(profile)?;
    let gadget_multiplier = ring_one(profile)?.scale_gadget(
        usize::try_from(statement.digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        profile,
    )?;
    let mut h0_terms = Vec::with_capacity(3);
    if party == statement.left {
        push_nonzero_term(&mut h0_terms, 0, gadget_multiplier);
    }
    push_nonzero_term(&mut h0_terms, 2, common_a.negate(profile)?);
    push_nonzero_term(&mut h0_terms, 3, plaintext_multiplier.clone());
    let mut h1_terms = Vec::with_capacity(2);
    if party == statement.right {
        push_nonzero_term(&mut h1_terms, 0, common_a);
    }
    push_nonzero_term(&mut h1_terms, 4, plaintext_multiplier);
    relation.outputs.extend([
        LinearRelationOutputV1 {
            target: release_wire_polynomial(profile, statement.h0)?,
            challenge_automorphism_exponent: 1,
            terms: h0_terms,
        },
        LinearRelationOutputV1 {
            target: release_wire_polynomial(profile, statement.h1)?,
            challenge_automorphism_exponent: 1,
            terms: h1_terms,
        },
    ]);
    relation.validate(profile)?;
    Ok(relation)
}

fn rkg_round_two_relation(
    profile: &super::BgvProfile,
    statement: ZkAmsMkheActiveRkgRoundTwoStatementV1<'_>,
    party: ZkAmsMkhePartyIdV1,
) -> Result<LinearRelationStatementV1, ZkAmsMkheErrorV1> {
    let mut relation = rkg_round_one_relation(profile, statement.round_one, party)?;
    relation.witness_bounds.push(i64::from(profile.error_eta));
    relation.witness_challenge_automorphism_exponents.push(1);
    let aggregate_h0 = release_wire_polynomial(profile, statement.aggregate_h0)?;
    let aggregate_h1 = release_wire_polynomial(profile, statement.aggregate_h1)?;
    let mut secret_multiplier = aggregate_h1.negate(profile)?;
    if party == statement.round_one.right {
        secret_multiplier = secret_multiplier.add(&aggregate_h0, profile)?;
    }
    let plaintext_multiplier = ring_one(profile)?.scale_plaintext_modulus(profile)?;
    let mut k0_terms = Vec::with_capacity(3);
    push_nonzero_term(&mut k0_terms, 0, secret_multiplier);
    push_nonzero_term(&mut k0_terms, 2, aggregate_h1);
    push_nonzero_term(&mut k0_terms, 5, plaintext_multiplier);
    relation.outputs.push(LinearRelationOutputV1 {
        target: release_wire_polynomial(profile, statement.k0)?,
        challenge_automorphism_exponent: 1,
        terms: k0_terms,
    });
    relation.validate(profile)?;
    Ok(relation)
}

fn galois_source_relation(
    profile: &super::BgvProfile,
    statement: ZkAmsMkheActiveGaloisSourceStatementV1<'_>,
) -> Result<LinearRelationStatementV1, ZkAmsMkheErrorV1> {
    let exponent =
        usize::try_from(statement.exponent).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut relation = collective_public_key_relation(profile, statement.public_key)?;
    relation.witness_bounds.extend([
        1,
        i64::from(profile.error_eta),
        i64::from(profile.error_eta),
    ]);
    relation
        .witness_challenge_automorphism_exponents
        .extend([exponent, exponent, exponent]);
    let public_a = release_wire_polynomial(profile, statement.public_key.public_a)?;
    let public_b = release_wire_polynomial(profile, statement.public_key.party_public_b)?;
    let plaintext_multiplier = ring_one(profile)?.scale_plaintext_modulus(profile)?;
    let gadget_multiplier = ring_one(profile)?.scale_gadget(
        usize::try_from(statement.digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        profile,
    )?;
    relation.outputs.extend([
        LinearRelationOutputV1 {
            target: release_wire_polynomial(profile, statement.source_constant)?,
            challenge_automorphism_exponent: exponent,
            terms: vec![
                LinearRelationTermV1 {
                    witness_index: 0,
                    multiplier: gadget_multiplier,
                    witness_automorphism_exponent: exponent,
                },
                LinearRelationTermV1 {
                    witness_index: 2,
                    multiplier: public_b,
                    witness_automorphism_exponent: 1,
                },
                LinearRelationTermV1 {
                    witness_index: 3,
                    multiplier: plaintext_multiplier.clone(),
                    witness_automorphism_exponent: 1,
                },
            ],
        },
        LinearRelationOutputV1 {
            target: release_wire_polynomial(profile, statement.source_linear)?,
            challenge_automorphism_exponent: exponent,
            terms: vec![
                LinearRelationTermV1 {
                    witness_index: 2,
                    multiplier: public_a,
                    witness_automorphism_exponent: 1,
                },
                LinearRelationTermV1 {
                    witness_index: 4,
                    multiplier: plaintext_multiplier,
                    witness_automorphism_exponent: 1,
                },
            ],
        },
    ]);
    relation.validate(profile)?;
    Ok(relation)
}

fn secret_polynomial_exact(
    profile: &super::BgvProfile,
    coefficients: &[i64],
    bound: i64,
) -> Result<super::SecretPolynomial, ZkAmsMkheErrorV1> {
    if coefficients.len() != profile.ring_degree
        || bound <= 0
        || coefficients
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > bound as u64)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(super::SecretPolynomial {
        coefficients: coefficients.to_vec(),
    })
}

fn round_one_witness_polynomials(
    profile: &super::BgvProfile,
    witness: ZkAmsMkheActiveRkgRoundOneWitnessV1<'_>,
) -> Result<Vec<super::SecretPolynomial>, ZkAmsMkheErrorV1> {
    Ok(vec![
        secret_polynomial_exact(profile, witness.secret, 1)?,
        secret_polynomial_exact(profile, witness.public_error, i64::from(profile.error_eta))?,
        secret_polynomial_exact(profile, witness.ephemeral, 1)?,
        secret_polynomial_exact(profile, witness.error_zero, i64::from(profile.error_eta))?,
        secret_polynomial_exact(profile, witness.error_one, i64::from(profile.error_eta))?,
    ])
}

#[allow(clippy::too_many_arguments)]
fn prove_authenticated_active_relation<R: MaskedRelaxedRandomSourceV1>(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: LinearProofContextV1,
    statement: &LinearRelationStatementV1,
    witnesses: Vec<super::SecretPolynomial>,
    authentication_secret: &AuthenticationSecret,
    random: &mut R,
) -> Result<ZkAmsMkheActiveRkgProofV1, ZkAmsMkheErrorV1> {
    let witness_refs = witnesses.iter().collect::<Vec<_>>();
    let proof = prove_linear_relation_v1(profile, context, statement, &witness_refs, random)?;
    let statement_digest = statement.digest(profile)?;
    let proof_bytes = proof.encode_wire()?;
    let contribution = authenticate_verified_linear_contribution(
        profile,
        roster,
        context,
        statement,
        &proof,
        authentication_secret,
        random,
    )?;
    Ok(ZkAmsMkheActiveRkgProofV1 {
        statement_digest,
        witness_polynomials: u8::try_from(witnesses.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        proof_bytes,
        contribution,
    })
}

fn verify_authenticated_active_relation(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: LinearProofContextV1,
    statement: &LinearRelationStatementV1,
    proof: &ZkAmsMkheActiveRkgProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    context.validate(profile)?;
    statement.validate(profile)?;
    if proof.statement_digest != statement.digest(profile)?
        || usize::from(proof.witness_polynomials) != statement.witness_bounds.len()
        || proof.proof_bytes.is_empty()
        || proof.proof_bytes.len() > super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let decoded = LinearRelationProofV1::decode_wire_exact(
        &proof.proof_bytes,
        statement.witness_bounds.len(),
        profile.ring_degree,
    )?;
    verify_linear_relation_proof(profile, context, statement, &decoded)?;
    let payload_digest = decoded.digest(profile, context, statement)?;
    if proof.contribution.payload_digest != payload_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_active_contribution(
        roster,
        context.transcript_digest,
        context.round,
        usize::from(context.party_index),
        &proof.contribution,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)
}

fn validate_release_narrow_witness(values: &[i64], bound: i64) -> Result<(), ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    if values.len() != profile.ring_degree
        || values
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > bound as u64)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

/// Exact binding for one streamed collective-public-key or RKG relation proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LinearProofContextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    round: ZkAmsMkheActiveRoundV1,
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    record_index: u32,
    relation_index: u32,
}

impl LinearProofContextV1 {
    fn validate(&self, profile: &super::BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        if self.profile_digest != profile.digest()?
            || self.roster_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || usize::from(self.party_index) >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || !matches!(
                self.round,
                ZkAmsMkheActiveRoundV1::CollectivePublicKey
                    | ZkAmsMkheActiveRoundV1::RkgRoundOne
                    | ZkAmsMkheActiveRoundV1::RkgRoundTwo
                    | ZkAmsMkheActiveRoundV1::GaloisSource
            )
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LinearRelationTermV1 {
    witness_index: usize,
    multiplier: super::RnsPolynomial,
    witness_automorphism_exponent: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LinearRelationOutputV1 {
    target: super::RnsPolynomial,
    challenge_automorphism_exponent: usize,
    terms: Vec<LinearRelationTermV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LinearRelationStatementV1 {
    witness_bounds: Vec<i64>,
    witness_challenge_automorphism_exponents: Vec<usize>,
    outputs: Vec<LinearRelationOutputV1>,
}

impl LinearRelationStatementV1 {
    fn validate(&self, profile: &super::BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        profile.validate()?;
        if self.witness_bounds.is_empty()
            || self.witness_bounds.len() > RKG_LINEAR_PROOF_MAX_WITNESSES_V1
            || self.witness_challenge_automorphism_exponents.len() != self.witness_bounds.len()
            || self.outputs.is_empty()
            || self.outputs.len() > RKG_LINEAR_PROOF_MAX_OUTPUTS_V1
            || self
                .witness_bounds
                .iter()
                .any(|bound| *bound <= 0 || *bound > i64::from(profile.error_eta))
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let twice_degree = profile
            .ring_degree
            .checked_mul(2)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        if self
            .witness_challenge_automorphism_exponents
            .iter()
            .any(|exponent| *exponent == 0 || *exponent >= twice_degree || exponent % 2 == 0)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let challenge_weight = linear_challenge_weight(profile.ring_degree)?;
        let minimum_modulus = profile
            .moduli
            .iter()
            .copied()
            .min()
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        let minimum_modulus =
            i64::try_from(minimum_modulus).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
        for bound in &self.witness_bounds {
            let (mask_bound, response_bound) =
                linear_response_parameters(*bound, challenge_weight)?;
            if mask_bound >= (minimum_modulus - 1) / 2
                || response_bound >= (minimum_modulus - 1) / 2
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        let mut used_witnesses = vec![false; self.witness_bounds.len()];
        for output in &self.outputs {
            output.target.validate(profile)?;
            if output.challenge_automorphism_exponent == 0
                || output.challenge_automorphism_exponent >= twice_degree
                || output.challenge_automorphism_exponent % 2 == 0
                || output.terms.is_empty()
                || output.terms.len() > self.witness_bounds.len()
                || output
                    .terms
                    .windows(2)
                    .any(|pair| pair[0].witness_index >= pair[1].witness_index)
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            for term in &output.terms {
                if term.witness_index >= self.witness_bounds.len() {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let witness_challenge_exponent = self
                    .witness_challenge_automorphism_exponents
                    .get(term.witness_index)
                    .copied()
                    .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
                if term.witness_automorphism_exponent == 0
                    || term.witness_automorphism_exponent >= twice_degree
                    || term.witness_automorphism_exponent % 2 == 0
                    || term
                        .witness_automorphism_exponent
                        .checked_mul(witness_challenge_exponent)
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                        % twice_degree
                        != output.challenge_automorphism_exponent
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                term.multiplier.validate(profile)?;
                if term.multiplier == super::RnsPolynomial::zero(profile) {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                used_witnesses[term.witness_index] = true;
            }
        }
        if used_witnesses.iter().any(|used| !used) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let response_bytes = self
            .witness_bounds
            .len()
            .checked_mul(profile.ring_degree)
            .and_then(|count| count.checked_mul(core::mem::size_of::<i64>()))
            .and_then(|bytes| bytes.checked_add(RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_V1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if response_bytes > profile.max_round_bytes {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(())
    }

    fn digest(&self, profile: &super::BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.validate(profile)?;
        let mut hash = Keccak256::new();
        hash.update(b"iroha.zk-ams.v1.mkhe.rkg-linear-relation-statement");
        hash.update(&profile.digest()?);
        hash.update(
            &u32::try_from(self.witness_bounds.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
        for (index, bound) in self.witness_bounds.iter().enumerate() {
            hash.update(&(index as u32).to_be_bytes());
            hash.update(&bound.to_be_bytes());
            hash.update(
                &u32::try_from(self.witness_challenge_automorphism_exponents[index])
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .to_be_bytes(),
            );
        }
        hash.update(
            &u32::try_from(self.outputs.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
        for (output_index, output) in self.outputs.iter().enumerate() {
            hash.update(&(output_index as u32).to_be_bytes());
            hash.update(
                &u32::try_from(output.challenge_automorphism_exponent)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .to_be_bytes(),
            );
            update_rns_hash(&mut hash, profile, &output.target)?;
            hash.update(
                &u32::try_from(output.terms.len())
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .to_be_bytes(),
            );
            for term in &output.terms {
                hash.update(
                    &u32::try_from(term.witness_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                        .to_be_bytes(),
                );
                hash.update(
                    &u32::try_from(term.witness_automorphism_exponent)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                        .to_be_bytes(),
                );
                update_rns_hash(&mut hash, profile, &term.multiplier)?;
            }
        }
        Ok(hash.finalize())
    }
}

/// Fiat--Shamir-with-aborts proof of one or more exact linear RNS relations.
///
/// This is a lattice proof, not a signature and not a digest-as-proof. The
/// verifier reconstructs the masked ring commitments from the bounded response
/// polynomials and the sparse challenge, then re-derives the challenge seed.
/// Soundness is the standard special-soundness reduction to the corresponding
/// module-SIS relation. Uniform-box aborts keep every accepted response inside
/// a witness-independent common interval.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LinearRelationProofV1 {
    challenge_seed: [u8; 32],
    responses: Vec<Vec<i64>>,
}

impl LinearRelationProofV1 {
    fn encode_wire(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        if self.challenge_seed == [0; 32]
            || self.responses.is_empty()
            || self.responses.len() > RKG_LINEAR_PROOF_MAX_WITNESSES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let ring_degree = self
            .responses
            .first()
            .map(Vec::len)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if ring_degree == 0
            || !ring_degree.is_power_of_two()
            || self
                .responses
                .iter()
                .any(|response| response.len() != ring_degree)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let length = linear_proof_wire_bytes(self.responses.len(), ring_degree)?;
        if length > super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::WireTooLarge);
        }
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(&RKG_LINEAR_PROOF_WIRE_TAG_V1);
        bytes.push(MKHE_VERSION_V1);
        bytes.extend_from_slice(&self.challenge_seed);
        bytes.push(
            u8::try_from(self.responses.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        );
        bytes.extend_from_slice(
            &u32::try_from(ring_degree)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        for response in &self.responses {
            for coefficient in response {
                bytes.extend_from_slice(&coefficient.to_be_bytes());
            }
        }
        if bytes.len() != length {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    fn decode_wire_exact(
        bytes: &[u8],
        expected_witnesses: usize,
        expected_ring_degree: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if expected_witnesses == 0
            || expected_witnesses > RKG_LINEAR_PROOF_MAX_WITNESSES_V1
            || expected_ring_degree == 0
            || !expected_ring_degree.is_power_of_two()
            || bytes.len() != linear_proof_wire_bytes(expected_witnesses, expected_ring_degree)?
            || bytes.len() > super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut cursor = 0;
        if bytes.get(cursor..cursor + 4) != Some(RKG_LINEAR_PROOF_WIRE_TAG_V1.as_slice()) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        cursor += 4;
        if bytes.get(cursor).copied() != Some(MKHE_VERSION_V1) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        cursor += 1;
        let challenge_seed: [u8; 32] = bytes
            .get(cursor..cursor + 32)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        cursor += 32;
        if challenge_seed == [0; 32]
            || bytes.get(cursor).copied()
                != Some(
                    u8::try_from(expected_witnesses)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                )
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        cursor += 1;
        let ring_degree = u32::from_be_bytes(
            bytes
                .get(cursor..cursor + 4)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        );
        cursor += 4;
        if usize::try_from(ring_degree).ok() != Some(expected_ring_degree) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut responses = Vec::with_capacity(expected_witnesses);
        for _ in 0..expected_witnesses {
            let mut response = Vec::with_capacity(expected_ring_degree);
            for _ in 0..expected_ring_degree {
                let coefficient = i64::from_be_bytes(
                    bytes
                        .get(
                            cursor
                                ..cursor
                                    + usize::from(RKG_LINEAR_PROOF_SIGNED_COEFFICIENT_BYTES_V1),
                        )
                        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                );
                cursor += usize::from(RKG_LINEAR_PROOF_SIGNED_COEFFICIENT_BYTES_V1);
                response.push(coefficient);
            }
            responses.push(response);
        }
        if cursor != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            challenge_seed,
            responses,
        })
    }

    fn digest(
        &self,
        profile: &super::BgvProfile,
        context: LinearProofContextV1,
        statement: &LinearRelationStatementV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        verify_linear_relation_proof(profile, context, statement, self)?;
        let mut hash = Keccak256::new();
        hash.update(b"iroha.zk-ams.v1.mkhe.rkg-linear-relation-proof");
        hash.update(&linear_context_digest(profile, context)?);
        hash.update(&statement.digest(profile)?);
        hash.update(&self.encode_wire()?);
        Ok(hash.finalize())
    }
}

fn linear_proof_wire_bytes(
    witness_count: usize,
    ring_degree: usize,
) -> Result<usize, ZkAmsMkheErrorV1> {
    witness_count
        .checked_mul(ring_degree)
        .and_then(|count| {
            count.checked_mul(usize::from(RKG_LINEAR_PROOF_SIGNED_COEFFICIENT_BYTES_V1))
        })
        .and_then(|bytes| bytes.checked_add(RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn prove_linear_relation_v1<R: MaskedRelaxedRandomSourceV1>(
    profile: &super::BgvProfile,
    context: LinearProofContextV1,
    statement: &LinearRelationStatementV1,
    witnesses: &[&super::SecretPolynomial],
    random: &mut R,
) -> Result<LinearRelationProofV1, ZkAmsMkheErrorV1> {
    context.validate(profile)?;
    statement.validate(profile)?;
    validate_linear_witnesses(profile, statement, witnesses)?;
    let witness_rns = witnesses
        .iter()
        .map(|witness| witness.as_rns(profile))
        .collect::<Result<Vec<_>, _>>()?;
    if apply_linear_relation(profile, statement, &witness_rns)?
        != statement
            .outputs
            .iter()
            .map(|output| output.target.clone())
            .collect::<Vec<_>>()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_linear_random_health(random)?;
    let challenge_weight = linear_challenge_weight(profile.ring_degree)?;
    for _ in 0..RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut masks = statement
            .witness_bounds
            .iter()
            .map(|bound| {
                let (mask_bound, _) = linear_response_parameters(*bound, challenge_weight)?;
                sample_signed_mask(profile.ring_degree, mask_bound, random)
            })
            .collect::<Result<Vec<_>, ZkAmsMkheErrorV1>>()?;
        let mask_rns = masks
            .iter()
            .map(|mask| super::RnsPolynomial::from_signed(profile, mask))
            .collect::<Result<Vec<_>, _>>()?;
        let commitments = apply_linear_relation(profile, statement, &mask_rns)?;
        let challenge_seed =
            linear_commitment_challenge_seed(profile, context, statement, &commitments)?;
        if challenge_seed == [0; 32] {
            for mask in &mut masks {
                mask.fill(0);
            }
            continue;
        }
        let challenge = derive_sparse_challenge(profile.ring_degree, challenge_seed)?;
        let mut responses = Vec::with_capacity(witnesses.len());
        let mut accepted = true;
        for (((mask, witness), bound), challenge_exponent) in masks
            .iter()
            .zip(witnesses)
            .zip(&statement.witness_bounds)
            .zip(&statement.witness_challenge_automorphism_exponents)
        {
            let witness_challenge = automorphism_signed(&challenge, *challenge_exponent)?;
            let folded = sparse_negacyclic_mul_signed(&witness_challenge, &witness.coefficients)?;
            let (_, response_limit) = linear_response_parameters(*bound, challenge_weight)?;
            let mut response = Vec::with_capacity(profile.ring_degree);
            for (mask, folded) in mask.iter().copied().zip(folded) {
                let value = mask
                    .checked_add(folded)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                if value.unsigned_abs() > response_limit as u64 {
                    accepted = false;
                }
                response.push(value);
            }
            responses.push(response);
        }
        for mask in &mut masks {
            mask.fill(0);
        }
        if !accepted {
            continue;
        }
        let proof = LinearRelationProofV1 {
            challenge_seed,
            responses,
        };
        verify_linear_relation_proof(profile, context, statement, &proof)?;
        return Ok(proof);
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn verify_linear_relation_proof(
    profile: &super::BgvProfile,
    context: LinearProofContextV1,
    statement: &LinearRelationStatementV1,
    proof: &LinearRelationProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    context.validate(profile)?;
    statement.validate(profile)?;
    if proof.challenge_seed == [0; 32]
        || proof.responses.len() != statement.witness_bounds.len()
        || proof
            .responses
            .iter()
            .any(|response| response.len() != profile.ring_degree)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let challenge_weight = linear_challenge_weight(profile.ring_degree)?;
    for (response, bound) in proof.responses.iter().zip(&statement.witness_bounds) {
        validate_linear_response_coefficients(
            response,
            profile.ring_degree,
            *bound,
            challenge_weight,
        )?;
    }
    let response_rns = proof
        .responses
        .iter()
        .map(|response| super::RnsPolynomial::from_signed(profile, response))
        .collect::<Result<Vec<_>, _>>()?;
    let applied = apply_linear_relation(profile, statement, &response_rns)?;
    let challenge = derive_sparse_challenge(profile.ring_degree, proof.challenge_seed)?;
    let commitments = applied
        .into_iter()
        .zip(&statement.outputs)
        .map(|(response, output)| {
            let output_challenge =
                automorphism_signed(&challenge, output.challenge_automorphism_exponent)?;
            let output_challenge_rns =
                super::RnsPolynomial::from_signed(profile, &output_challenge)?;
            response.sub(&output.target.mul(&output_challenge_rns, profile)?, profile)
        })
        .collect::<Result<Vec<_>, ZkAmsMkheErrorV1>>()?;
    let expected = linear_commitment_challenge_seed(profile, context, statement, &commitments)?;
    if expected != proof.challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_linear_response_coefficients(
    response: &[i64],
    ring_degree: usize,
    witness_bound: i64,
    challenge_weight: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    let (_, response_limit) = linear_response_parameters(witness_bound, challenge_weight)?;
    if response.len() != ring_degree
        || response
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > response_limit as u64)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_verified_linear_contribution<R: MaskedRelaxedRandomSourceV1>(
    profile: &super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    context: LinearProofContextV1,
    statement: &LinearRelationStatementV1,
    proof: &LinearRelationProofV1,
    authentication_secret: &AuthenticationSecret,
    random: &mut R,
) -> Result<ZkAmsMkheActiveContributionV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    context.validate(profile)?;
    let party_index = usize::from(context.party_index);
    let participant = roster.participant(party_index)?;
    if context.profile_digest != roster.profile_digest
        || context.roster_digest != roster.roster_digest
        || context.epoch != roster.epoch
        || context.party != participant.party
        || authentication_secret.party_id()? != participant.party
        || authentication_secret.public_key()? != participant.authentication_public_key
    {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let proof_digest = proof.digest(profile, context, statement)?;
    authenticate_active_contribution(
        roster,
        context.transcript_digest,
        context.round,
        party_index,
        proof_digest,
        authentication_secret,
        random,
    )
}

fn validate_linear_witnesses(
    profile: &super::BgvProfile,
    statement: &LinearRelationStatementV1,
    witnesses: &[&super::SecretPolynomial],
) -> Result<(), ZkAmsMkheErrorV1> {
    if witnesses.len() != statement.witness_bounds.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for (witness, bound) in witnesses.iter().zip(&statement.witness_bounds) {
        if witness.coefficients.len() != profile.ring_degree
            || witness
                .coefficients
                .iter()
                .any(|coefficient| coefficient.unsigned_abs() > *bound as u64)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}

fn apply_linear_relation(
    profile: &super::BgvProfile,
    statement: &LinearRelationStatementV1,
    witnesses: &[super::RnsPolynomial],
) -> Result<Vec<super::RnsPolynomial>, ZkAmsMkheErrorV1> {
    if witnesses.len() != statement.witness_bounds.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    statement
        .outputs
        .iter()
        .map(|output| {
            let mut value = super::RnsPolynomial::zero(profile);
            for term in &output.terms {
                let witness = witnesses
                    .get(term.witness_index)
                    .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .automorphism(term.witness_automorphism_exponent, profile)?;
                value = value.add(&term.multiplier.mul(&witness, profile)?, profile)?;
            }
            Ok(value)
        })
        .collect()
}

fn linear_response_parameters(
    witness_bound: i64,
    challenge_weight: usize,
) -> Result<(i64, i64), ZkAmsMkheErrorV1> {
    let challenge_weight =
        i64::try_from(challenge_weight).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let challenge_slack = witness_bound
        .checked_mul(challenge_weight)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mask_bound = challenge_slack
        .checked_mul(RKG_LINEAR_PROOF_MASK_SLACK_FACTOR_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let response_limit = mask_bound
        .checked_sub(challenge_slack)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if witness_bound <= 0 || challenge_slack <= 0 || response_limit <= 0 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok((mask_bound, response_limit))
}

fn linear_challenge_weight(ring_degree: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    if ring_degree < 2 || !ring_degree.is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(
        if ring_degree >= RKG_LINEAR_PROOF_RELEASE_CHALLENGE_WEIGHT_V1 {
            RKG_LINEAR_PROOF_RELEASE_CHALLENGE_WEIGHT_V1
        } else {
            (ring_degree / 2).max(1)
        },
    )
}

fn sample_signed_mask<R: MaskedRelaxedRandomSourceV1>(
    count: usize,
    bound: i64,
    random: &mut R,
) -> Result<Vec<i64>, ZkAmsMkheErrorV1> {
    let width = u64::try_from(bound)
        .ok()
        .and_then(|bound| bound.checked_mul(2))
        .and_then(|width| width.checked_add(1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    (0..count)
        .map(|_| {
            let sample = super::sample_below(width, random)?;
            i64::try_from(sample)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .checked_sub(bound)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        })
        .collect()
}

fn validate_linear_random_health<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut first = [0_u8; 32];
    random
        .fill_bytes(&mut first)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
    if first == [0; 32] {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    for _ in 0..RKG_LINEAR_PROOF_RANDOM_HEALTH_RETRIES_V1 {
        let mut next = [0_u8; 32];
        random
            .fill_bytes(&mut next)
            .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
        if next != [0; 32] && next != first {
            first.fill(0);
            next.fill(0);
            return Ok(());
        }
        next.fill(0);
    }
    first.fill(0);
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SparseChallengeTermV1 {
    position: u32,
    sign: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SparseChallengeV1 {
    terms: Vec<SparseChallengeTermV1>,
}

impl SparseChallengeV1 {
    fn new(
        ring_degree: usize,
        terms: Vec<SparseChallengeTermV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if terms.len() != linear_challenge_weight(ring_degree)?
            || terms.iter().any(|term| {
                usize::try_from(term.position).map_or(true, |position| position >= ring_degree)
                    || ![-1, 1].contains(&term.sign)
            })
            || terms
                .windows(2)
                .any(|pair| pair[0].position >= pair[1].position)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self { terms })
    }

    fn to_dense(&self, ring_degree: usize) -> Result<Vec<i64>, ZkAmsMkheErrorV1> {
        let mut dense = vec![0_i64; ring_degree];
        for term in &self.terms {
            let position =
                usize::try_from(term.position).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            let coefficient = dense
                .get_mut(position)
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if *coefficient != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            *coefficient = i64::from(term.sign);
        }
        Ok(dense)
    }
}

fn derive_sparse_challenge(
    ring_degree: usize,
    challenge_seed: [u8; 32],
) -> Result<Vec<i64>, ZkAmsMkheErrorV1> {
    if challenge_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let weight = linear_challenge_weight(ring_degree)?;
    let mut frame = Vec::with_capacity(96);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-sparse-challenge");
    frame.extend_from_slice(&challenge_seed);
    frame.extend_from_slice(
        &u32::try_from(ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(
        &u32::try_from(weight)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    let stream = shake256(
        &frame,
        weight
            .checked_mul(RANDOM_REJECTION_ATTEMPTS_V1)
            .and_then(|bytes| bytes.checked_mul(8))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    );
    let position_mask =
        u64::try_from(ring_degree - 1).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut selected_positions = vec![false; ring_degree];
    let mut terms = Vec::with_capacity(weight);
    let mut selected = 0;
    for chunk in stream.chunks_exact(8) {
        let candidate = u64::from_le_bytes(
            chunk
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        );
        // The governed degree is a power of two. Low position bits are
        // therefore exactly uniform and independent of the high sign bit;
        // no modulo-reduction bias or sign-skewing rejection zone exists.
        let position = usize::try_from(candidate & position_mask)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
        if selected_positions[position] {
            continue;
        }
        selected_positions[position] = true;
        terms.push(SparseChallengeTermV1 {
            position: u32::try_from(position).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
            sign: if candidate >> 63 == 0 { -1 } else { 1 },
        });
        selected += 1;
        if selected == weight {
            terms.sort_by_key(|term| term.position);
            return SparseChallengeV1::new(ring_degree, terms)?.to_dense(ring_degree);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
}

fn sparse_negacyclic_mul_signed(
    sparse: &[i64],
    dense: &[i64],
) -> Result<Vec<i64>, ZkAmsMkheErrorV1> {
    if sparse.len() != dense.len() || sparse.is_empty() || !sparse.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let degree = sparse.len();
    let mut output = vec![0_i64; degree];
    for (shift, sign) in sparse.iter().copied().enumerate() {
        if sign == 0 {
            continue;
        }
        if ![-1, 1].contains(&sign) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        for (index, coefficient) in dense.iter().copied().enumerate() {
            let destination = index + shift;
            let (destination, wrap_sign) = if destination >= degree {
                (destination - degree, -1_i64)
            } else {
                (destination, 1_i64)
            };
            let term = coefficient
                .checked_mul(sign)
                .and_then(|value| value.checked_mul(wrap_sign))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            output[destination] = output[destination]
                .checked_add(term)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
    }
    Ok(output)
}

fn automorphism_signed(
    coefficients: &[i64],
    exponent: usize,
) -> Result<Vec<i64>, ZkAmsMkheErrorV1> {
    if coefficients.is_empty() || !coefficients.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let twice_degree = coefficients
        .len()
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    if exponent == 0 || exponent >= twice_degree || exponent % 2 == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut output = vec![0_i64; coefficients.len()];
    for (index, coefficient) in coefficients.iter().copied().enumerate() {
        let mapped = index
            .checked_mul(exponent)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            % twice_degree;
        let (destination, sign) = if mapped >= coefficients.len() {
            (mapped - coefficients.len(), -1_i64)
        } else {
            (mapped, 1_i64)
        };
        output[destination] = coefficient
            .checked_mul(sign)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    Ok(output)
}

fn linear_context_digest(
    profile: &super::BgvProfile,
    context: LinearProofContextV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    context.validate(profile)?;
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-context");
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&context.profile_digest);
    frame.extend_from_slice(&context.roster_digest);
    frame.extend_from_slice(&context.epoch.to_be_bytes());
    frame.extend_from_slice(&context.transcript_digest);
    frame.push(context.round.tag());
    frame.push(context.party_index);
    frame.extend_from_slice(&context.party.to_bytes());
    frame.extend_from_slice(&context.record_index.to_be_bytes());
    frame.extend_from_slice(&context.relation_index.to_be_bytes());
    Ok(keccak256(&frame))
}

fn linear_commitment_challenge_seed(
    profile: &super::BgvProfile,
    context: LinearProofContextV1,
    statement: &LinearRelationStatementV1,
    commitments: &[super::RnsPolynomial],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if commitments.len() != statement.outputs.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-fiat-shamir");
    hash.update(&linear_context_digest(profile, context)?);
    hash.update(&statement.digest(profile)?);
    hash.update(
        &u32::try_from(commitments.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    for (index, commitment) in commitments.iter().enumerate() {
        hash.update(&(index as u32).to_be_bytes());
        update_rns_hash(&mut hash, profile, commitment)?;
    }
    Ok(hash.finalize())
}

fn update_rns_hash(
    hash: &mut Keccak256,
    profile: &super::BgvProfile,
    polynomial: &super::RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    hash.update(
        &u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    hash.update(
        &u32::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    for (limb, values) in polynomial
        .coefficients
        .chunks_exact(profile.ring_degree)
        .enumerate()
    {
        hash.update(&(limb as u32).to_be_bytes());
        hash.update(&profile.moduli[limb].to_be_bytes());
        for value in values {
            hash.update(&value.to_be_bytes());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("active_tests.rs");
}

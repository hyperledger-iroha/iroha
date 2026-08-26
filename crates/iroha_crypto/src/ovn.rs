//! Reference Ristretto255 Open Vote Network ballot primitives.
//!
//! The v1 protocol freezes an ordered survivor roster before voting. Every
//! participant registers three independent public keys (one per choice) with
//! Schnorr proofs of possession. A ballot masks each one-hot choice component
//! with the corresponding OVN pairwise-cancelling key and proves, in one
//! Fiat--Shamir OR proof, knowledge of all three registered secrets and that
//! exactly one component contains the basepoint.
//!
//! This module proves neither late-dropout recovery nor linkage between a TLE
//! envelope and a ballot commitment. It is retained as a compact reference and
//! known-answer-test surface; Parliament admission must use
//! [`crate::timed_ovn`] and must not combine this module with a standalone TLE
//! envelope as a fallback. Public aggregates reveal counts; this module does
//! not claim anonymity, receipt freeness, coercion resistance, or privacy for
//! tiny electorates whose aggregate itself identifies a vote.

use std::{collections::HashSet, vec::Vec};

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::Identity as _,
};
use rand_core::{OsRng, TryCryptoRng};
use sha2::{Digest as _, Sha256, Sha512};
use thiserror::Error;
use zeroize::Zeroizing;

/// Fixed protocol version for native OVN ballots.
pub const OVN_PROTOCOL_VERSION_V1: u16 = 1;
/// Exact number of choices: Aye, Nay, and Abstain.
pub const OVN_CHOICE_COUNT_V1: usize = 3;
/// Maximum frozen roster accepted by the bounded tally decoder.
pub const OVN_MAX_PARTICIPANTS_V1: usize = 1_000;

const SESSION_DOMAIN_V1: &[u8] = b"iroha.parliament.ovn.session.v1\0";
const SESSION_DIGEST_DOMAIN_V1: &[u8] = b"iroha.parliament.ovn.session-digest.v1\0";
const POP_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.parliament.ovn.pop.v1\0";
const ROSTER_ROOT_DOMAIN_V1: &[u8] = b"iroha.parliament.ovn.roster-root.v1\0";
const SURVIVOR_ROOT_DOMAIN_V1: &[u8] = b"iroha.parliament.ovn.survivor-root.v1\0";
const BALLOT_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.parliament.ovn.ballot-or-proof.v1\0";
const REGISTRATION_MAGIC_V1: &[u8; 8] = b"IOVNREG1";
const BALLOT_MAGIC_V1: &[u8; 8] = b"IOVNBAL1";
const OPTION_TAGS_V1: [u8; OVN_CHOICE_COUNT_V1] = [0, 1, 2];
const REGISTRATION_WIRE_BYTES_V1: usize = 8 + 32 + 32 + OVN_CHOICE_COUNT_V1 * 32 * 3;
const BALLOT_WIRE_BYTES_V1: usize = 8
    + 32 * 4
    + 2
    + OVN_CHOICE_COUNT_V1 * 32
    + OVN_CHOICE_COUNT_V1 * 32
    + OVN_CHOICE_COUNT_V1 * OVN_CHOICE_COUNT_V1 * 32;

/// Closed three-choice Parliament ballot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OvnChoiceV1 {
    /// Approve the proposal.
    Aye = 0,
    /// Reject the proposal.
    Nay = 1,
    /// Count toward turnout without choosing Aye or Nay.
    Abstain = 2,
}

impl OvnChoiceV1 {
    const fn index(self) -> usize {
        self as usize
    }
}

/// Errors returned by native OVN admission, proof verification, and tallying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OvnError {
    /// A required protocol digest was an all-zero placeholder.
    #[error("OVN binding digest must be non-zero")]
    ZeroBinding,
    /// The roster is empty or exceeds the fixed v1 bound.
    #[error("OVN roster size is outside the v1 bound")]
    InvalidRosterSize,
    /// Registrations were not supplied in strictly increasing participant order.
    #[error("OVN registrations are not in canonical participant order")]
    NonCanonicalRoster,
    /// A participant identity appeared more than once.
    #[error("OVN roster contains a duplicate participant")]
    DuplicateParticipant,
    /// A public registration key appeared more than once.
    #[error("OVN roster contains a duplicate registration key")]
    DuplicateRegistrationKey,
    /// A survivor was absent from the registration roster or supplied out of order.
    #[error("OVN survivor list is not a canonical roster subsequence")]
    NonCanonicalSurvivorSet,
    /// A point encoding was malformed, noncanonical, or not a Ristretto point.
    #[error("invalid canonical Ristretto255 point")]
    InvalidPoint,
    /// A public key, proof commitment, or derived masking key was the identity.
    #[error("OVN public protocol point must be non-identity")]
    IdentityPoint,
    /// A scalar response or challenge was not canonically encoded.
    #[error("invalid canonical Ristretto255 scalar")]
    InvalidScalar,
    /// A Schnorr registration proof failed.
    #[error("OVN registration proof of possession failed")]
    InvalidProofOfPossession,
    /// A ballot one-hot OR proof failed.
    #[error("OVN ballot one-hot proof failed")]
    InvalidBallotProof,
    /// An object was replayed under another session, roster, survivor set, or seat.
    #[error("OVN transcript binding mismatch")]
    BindingMismatch,
    /// The secret does not match the participant registration.
    #[error("OVN registration secret does not match the frozen public registration")]
    SecretMismatch,
    /// The participant is not in the frozen survivor roster.
    #[error("OVN participant is not in the frozen survivor roster")]
    UnknownParticipant,
    /// The ballot corpus omitted, duplicated, or reordered a survivor.
    #[error("OVN ballot corpus must contain every survivor exactly once in roster order")]
    NonCanonicalBallotCorpus,
    /// Fallible cryptographic randomness failed.
    #[error("OVN CSPRNG failed")]
    RandomnessUnavailable,
    /// Random material reduced to an inert zero scalar.
    #[error("OVN CSPRNG returned inert scalar material")]
    InertRandomness,
    /// Wire bytes were truncated, oversized, had a wrong tag, or contained trailing data.
    #[error("invalid canonical OVN wire encoding")]
    InvalidEncoding,
    /// Pairwise masks did not cancel to a bounded choice count.
    #[error("OVN aggregate does not decode within the frozen survivor bound")]
    MaskCancellationFailed,
    /// Decoded choice counts did not sum to the accepted survivor count.
    #[error("OVN decoded counts do not equal the accepted ballot count")]
    InvalidTally,
}

/// Complete immutable binding for one OVN ballot attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OvnSessionV1 {
    network_id: [u8; 32],
    proposal_content_id: [u8; 32],
    governance_attempt_id: [u8; 32],
    body_instance_id: [u8; 32],
    ballot_attempt_id: [u8; 32],
    parameter_hash: [u8; 32],
}

impl OvnSessionV1 {
    /// Construct one fully bound OVN session.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError::ZeroBinding`] for any inert digest placeholder.
    pub fn new(
        network_id: [u8; 32],
        proposal_content_id: [u8; 32],
        governance_attempt_id: [u8; 32],
        body_instance_id: [u8; 32],
        ballot_attempt_id: [u8; 32],
        parameter_hash: [u8; 32],
    ) -> Result<Self, OvnError> {
        if [
            network_id,
            proposal_content_id,
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            parameter_hash,
        ]
        .iter()
        .any(|binding| is_zero(binding))
        {
            return Err(OvnError::ZeroBinding);
        }
        Ok(Self {
            network_id,
            proposal_content_id,
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            parameter_hash,
        })
    }

    /// Return the ballot-attempt identifier.
    #[must_use]
    pub const fn ballot_attempt_id(&self) -> &[u8; 32] {
        &self.ballot_attempt_id
    }

    /// Return the deterministic digest of the complete typed session.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(SESSION_DIGEST_DOMAIN_V1);
        hasher.update(self.canonical_bytes());
        hasher.finalize().into()
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SESSION_DOMAIN_V1.len() + 2 + 32 * 6);
        bytes.extend_from_slice(SESSION_DOMAIN_V1);
        bytes.extend_from_slice(&OVN_PROTOCOL_VERSION_V1.to_be_bytes());
        bytes.extend_from_slice(&self.network_id);
        bytes.extend_from_slice(&self.proposal_content_id);
        bytes.extend_from_slice(&self.governance_attempt_id);
        bytes.extend_from_slice(&self.body_instance_id);
        bytes.extend_from_slice(&self.ballot_attempt_id);
        bytes.extend_from_slice(&self.parameter_hash);
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SchnorrPopV1 {
    commitment: [u8; 32],
    response: [u8; 32],
}

/// Three independent OVN public keys and proofs of possession for one participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvnRegistrationV1 {
    session_digest: [u8; 32],
    participant_hash: [u8; 32],
    public_keys: [[u8; 32]; OVN_CHOICE_COUNT_V1],
    proofs: [SchnorrPopV1; OVN_CHOICE_COUNT_V1],
}

impl OvnRegistrationV1 {
    /// Decode and verify one fully consuming fixed-width registration.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] for a malformed encoding, wrong session, invalid
    /// point, identity point, or failed proof of possession.
    pub fn from_bytes(session: &OvnSessionV1, bytes: &[u8]) -> Result<Self, OvnError> {
        if bytes.len() != REGISTRATION_WIRE_BYTES_V1 {
            return Err(OvnError::InvalidEncoding);
        }
        let mut cursor = 0_usize;
        if take::<8>(bytes, &mut cursor)? != *REGISTRATION_MAGIC_V1 {
            return Err(OvnError::InvalidEncoding);
        }
        let session_digest = take::<32>(bytes, &mut cursor)?;
        let participant_hash = take::<32>(bytes, &mut cursor)?;
        let mut public_keys = [[0_u8; 32]; OVN_CHOICE_COUNT_V1];
        let mut proofs = [SchnorrPopV1 {
            commitment: [0; 32],
            response: [0; 32],
        }; OVN_CHOICE_COUNT_V1];
        for option in 0..OVN_CHOICE_COUNT_V1 {
            public_keys[option] = take::<32>(bytes, &mut cursor)?;
            proofs[option] = SchnorrPopV1 {
                commitment: take::<32>(bytes, &mut cursor)?,
                response: take::<32>(bytes, &mut cursor)?,
            };
        }
        if cursor != bytes.len() {
            return Err(OvnError::InvalidEncoding);
        }
        let registration = Self {
            session_digest,
            participant_hash,
            public_keys,
            proofs,
        };
        registration.verify(session)?;
        Ok(registration)
    }

    /// Encode the canonical fixed-width registration.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(REGISTRATION_WIRE_BYTES_V1);
        bytes.extend_from_slice(REGISTRATION_MAGIC_V1);
        bytes.extend_from_slice(&self.session_digest);
        bytes.extend_from_slice(&self.participant_hash);
        for option in 0..OVN_CHOICE_COUNT_V1 {
            bytes.extend_from_slice(&self.public_keys[option]);
            bytes.extend_from_slice(&self.proofs[option].commitment);
            bytes.extend_from_slice(&self.proofs[option].response);
        }
        bytes
    }

    /// Return the canonical participant identity hash.
    #[must_use]
    pub const fn participant_hash(&self) -> &[u8; 32] {
        &self.participant_hash
    }

    /// Return the three canonical Ristretto255 public-key encodings.
    #[must_use]
    pub const fn public_keys(&self) -> &[[u8; 32]; OVN_CHOICE_COUNT_V1] {
        &self.public_keys
    }

    fn verify(&self, session: &OvnSessionV1) -> Result<(), OvnError> {
        if self.session_digest != session.digest() || is_zero(&self.participant_hash) {
            return Err(OvnError::BindingMismatch);
        }
        for option in 0..OVN_CHOICE_COUNT_V1 {
            let public_key = decode_nonidentity_point(&self.public_keys[option])?;
            let commitment = decode_nonidentity_point(&self.proofs[option].commitment)?;
            let response = decode_scalar(&self.proofs[option].response)?;
            let challenge = pop_challenge(
                session,
                &self.participant_hash,
                option,
                &self.public_keys[option],
                &self.proofs[option].commitment,
            );
            if response * RISTRETTO_BASEPOINT_POINT != commitment + challenge * public_key {
                return Err(OvnError::InvalidProofOfPossession);
            }
        }
        Ok(())
    }
}

/// Non-cloneable, zeroizing owner of one participant's three registration secrets.
///
/// The type has no serialization, byte-export, or `Debug` implementation.
pub struct OvnRegistrationSecretV1 {
    session_digest: [u8; 32],
    participant_hash: [u8; 32],
    scalar_bytes: Zeroizing<[[u8; 32]; OVN_CHOICE_COUNT_V1]>,
}

impl OvnRegistrationSecretV1 {
    /// Generate secrets and corresponding proofs using the operating-system CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] when a binding or fallible randomness operation fails.
    pub fn generate(
        session: &OvnSessionV1,
        participant_hash: [u8; 32],
    ) -> Result<(Self, OvnRegistrationV1), OvnError> {
        Self::generate_with_rng(session, participant_hash, &mut OsRng)
    }

    /// Generate secrets and proofs using an explicit cryptographic RNG.
    ///
    /// This entry point supports deterministic interoperability tests. A live
    /// caller must never reuse seeded RNG state across participants or attempts.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] when a binding or fallible randomness operation fails.
    pub fn generate_with_rng<R: TryCryptoRng + ?Sized>(
        session: &OvnSessionV1,
        participant_hash: [u8; 32],
        rng: &mut R,
    ) -> Result<(Self, OvnRegistrationV1), OvnError> {
        if is_zero(&participant_hash) {
            return Err(OvnError::ZeroBinding);
        }
        let mut scalar_bytes = Zeroizing::new([[0_u8; 32]; OVN_CHOICE_COUNT_V1]);
        let mut public_keys = [[0_u8; 32]; OVN_CHOICE_COUNT_V1];
        let mut proofs = [SchnorrPopV1 {
            commitment: [0; 32],
            response: [0; 32],
        }; OVN_CHOICE_COUNT_V1];
        for option in 0..OVN_CHOICE_COUNT_V1 {
            let secret = random_nonzero_scalar(rng)?;
            let nonce = random_nonzero_scalar(rng)?;
            let public_key = (secret * RISTRETTO_BASEPOINT_POINT).compress().to_bytes();
            let commitment = (nonce * RISTRETTO_BASEPOINT_POINT).compress().to_bytes();
            let challenge =
                pop_challenge(session, &participant_hash, option, &public_key, &commitment);
            scalar_bytes[option] = secret.to_bytes();
            public_keys[option] = public_key;
            proofs[option] = SchnorrPopV1 {
                commitment,
                response: (nonce + challenge * secret).to_bytes(),
            };
        }
        let secret = Self {
            session_digest: session.digest(),
            participant_hash,
            scalar_bytes,
        };
        let registration = OvnRegistrationV1 {
            session_digest: session.digest(),
            participant_hash,
            public_keys,
            proofs,
        };
        registration.verify(session)?;
        Ok((secret, registration))
    }

    fn scalars(&self) -> Result<[Scalar; OVN_CHOICE_COUNT_V1], OvnError> {
        let mut scalars = [Scalar::ZERO; OVN_CHOICE_COUNT_V1];
        for (scalar, bytes) in scalars.iter_mut().zip(self.scalar_bytes.iter()) {
            *scalar = decode_scalar(bytes).map_err(|_| OvnError::SecretMismatch)?;
            if *scalar == Scalar::ZERO {
                return Err(OvnError::SecretMismatch);
            }
        }
        Ok(scalars)
    }
}

/// Canonically ordered, proof-validated OVN registration roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OvnRosterV1 {
    session: OvnSessionV1,
    registrations: Vec<OvnRegistrationV1>,
    roster_root: [u8; 32],
}

impl OvnRosterV1 {
    /// Freeze one complete registration roster without sorting remote input.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] unless the registrations are valid, unique, and
    /// already strictly ordered by canonical participant hash.
    pub fn new(
        session: OvnSessionV1,
        registrations: Vec<OvnRegistrationV1>,
    ) -> Result<Self, OvnError> {
        if registrations.is_empty() || registrations.len() > OVN_MAX_PARTICIPANTS_V1 {
            return Err(OvnError::InvalidRosterSize);
        }
        let mut previous_participant: Option<[u8; 32]> = None;
        let mut participants = HashSet::with_capacity(registrations.len());
        let mut public_keys = HashSet::with_capacity(registrations.len() * OVN_CHOICE_COUNT_V1);
        for registration in &registrations {
            registration.verify(&session)?;
            if let Some(previous) = previous_participant {
                if registration.participant_hash == previous {
                    return Err(OvnError::DuplicateParticipant);
                }
                if registration.participant_hash < previous {
                    return Err(OvnError::NonCanonicalRoster);
                }
            }
            if !participants.insert(registration.participant_hash) {
                return Err(OvnError::DuplicateParticipant);
            }
            for public_key in registration.public_keys {
                if !public_keys.insert(public_key) {
                    return Err(OvnError::DuplicateRegistrationKey);
                }
            }
            previous_participant = Some(registration.participant_hash);
        }
        let roster_root = roster_root(&session, &registrations);
        Ok(Self {
            session,
            registrations,
            roster_root,
        })
    }

    /// Return the immutable session.
    #[must_use]
    pub const fn session(&self) -> &OvnSessionV1 {
        &self.session
    }

    /// Return the canonical registration-roster root.
    #[must_use]
    pub const fn roster_root(&self) -> &[u8; 32] {
        &self.roster_root
    }

    /// Return the ordered validated registrations.
    #[must_use]
    pub fn registrations(&self) -> &[OvnRegistrationV1] {
        &self.registrations
    }
}

/// Frozen survivor subset used for mask derivation and ballot admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OvnSurvivorRosterV1 {
    session: OvnSessionV1,
    roster_root: [u8; 32],
    survivor_root: [u8; 32],
    registrations: Vec<OvnRegistrationV1>,
}

impl OvnSurvivorRosterV1 {
    /// Freeze a nonempty canonical subsequence of the registration roster.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError::NonCanonicalSurvivorSet`] for unknown, duplicate,
    /// or reordered survivor identities.
    pub fn new(roster: &OvnRosterV1, survivor_ids: &[[u8; 32]]) -> Result<Self, OvnError> {
        if survivor_ids.is_empty() || survivor_ids.len() > roster.registrations.len() {
            return Err(OvnError::InvalidRosterSize);
        }
        let mut registrations = Vec::with_capacity(survivor_ids.len());
        let mut roster_cursor = 0_usize;
        for survivor_id in survivor_ids {
            if is_zero(survivor_id) {
                return Err(OvnError::NonCanonicalSurvivorSet);
            }
            while roster_cursor < roster.registrations.len()
                && roster.registrations[roster_cursor].participant_hash < *survivor_id
            {
                roster_cursor += 1;
            }
            let registration = roster
                .registrations
                .get(roster_cursor)
                .filter(|registration| registration.participant_hash == *survivor_id)
                .ok_or(OvnError::NonCanonicalSurvivorSet)?;
            registrations.push(registration.clone());
            roster_cursor += 1;
        }
        let survivor_root = survivor_root(&roster.session, &roster.roster_root, &registrations);
        Ok(Self {
            session: roster.session,
            roster_root: roster.roster_root,
            survivor_root,
            registrations,
        })
    }

    /// Return the canonical original registration-roster root.
    #[must_use]
    pub const fn roster_root(&self) -> &[u8; 32] {
        &self.roster_root
    }

    /// Return the canonical frozen survivor root.
    #[must_use]
    pub const fn survivor_root(&self) -> &[u8; 32] {
        &self.survivor_root
    }

    /// Return the ordered survivor registrations.
    #[must_use]
    pub fn registrations(&self) -> &[OvnRegistrationV1] {
        &self.registrations
    }

    /// Derive the three survivor-root-bound OVN masking keys for one seat.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] for an unknown participant or a degenerate identity
    /// masking key. A degenerate survivor set must be redrawn before voting.
    pub fn masking_keys(&self, participant_hash: &[u8; 32]) -> Result<OvnMaskingKeysV1, OvnError> {
        let index = self
            .registrations
            .binary_search_by_key(participant_hash, |registration| {
                registration.participant_hash
            })
            .map_err(|_| OvnError::UnknownParticipant)?;
        let mut masking_points = core::array::from_fn(|_| RistrettoPoint::identity());
        for option in 0..OVN_CHOICE_COUNT_V1 {
            let mut masking_key = RistrettoPoint::identity();
            for registration in &self.registrations[..index] {
                masking_key += decode_nonidentity_point(&registration.public_keys[option])?;
            }
            for registration in &self.registrations[index + 1..] {
                masking_key -= decode_nonidentity_point(&registration.public_keys[option])?;
            }
            if masking_key == RistrettoPoint::identity() {
                return Err(OvnError::IdentityPoint);
            }
            masking_points[option] = masking_key;
        }
        Ok(OvnMaskingKeysV1 {
            session_digest: self.session.digest(),
            roster_root: self.roster_root,
            survivor_root: self.survivor_root,
            participant_hash: *participant_hash,
            index: u16::try_from(index).map_err(|_| OvnError::InvalidRosterSize)?,
            points: masking_points.map(|point| point.compress().to_bytes()),
        })
    }

    fn registration_at(&self, index: usize) -> Option<&OvnRegistrationV1> {
        self.registrations.get(index)
    }
}

/// Three derived OVN masking keys bound to one frozen survivor seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvnMaskingKeysV1 {
    session_digest: [u8; 32],
    roster_root: [u8; 32],
    survivor_root: [u8; 32],
    participant_hash: [u8; 32],
    index: u16,
    points: [[u8; 32]; OVN_CHOICE_COUNT_V1],
}

impl OvnMaskingKeysV1 {
    /// Return the zero-based canonical survivor index.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Return the three canonical masking-key encodings.
    #[must_use]
    pub const fn points(&self) -> &[[u8; 32]; OVN_CHOICE_COUNT_V1] {
        &self.points
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BallotOrProofV1 {
    challenges: [[u8; 32]; OVN_CHOICE_COUNT_V1],
    responses: [[[u8; 32]; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1],
}

/// One survivor-bound masked three-choice ballot and native one-hot proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvnMaskedBallotV1 {
    session_digest: [u8; 32],
    roster_root: [u8; 32],
    survivor_root: [u8; 32],
    participant_hash: [u8; 32],
    index: u16,
    points: [[u8; 32]; OVN_CHOICE_COUNT_V1],
    proof: BallotOrProofV1,
}

impl OvnMaskedBallotV1 {
    /// Decode and verify one fully consuming survivor-bound ballot.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] for malformed canonical encodings, a binding
    /// mismatch, or an invalid one-hot OR proof.
    pub fn from_bytes(survivors: &OvnSurvivorRosterV1, bytes: &[u8]) -> Result<Self, OvnError> {
        if bytes.len() != BALLOT_WIRE_BYTES_V1 {
            return Err(OvnError::InvalidEncoding);
        }
        let mut cursor = 0_usize;
        if take::<8>(bytes, &mut cursor)? != *BALLOT_MAGIC_V1 {
            return Err(OvnError::InvalidEncoding);
        }
        let session_digest = take::<32>(bytes, &mut cursor)?;
        let roster_root = take::<32>(bytes, &mut cursor)?;
        let survivor_root = take::<32>(bytes, &mut cursor)?;
        let participant_hash = take::<32>(bytes, &mut cursor)?;
        let index = u16::from_be_bytes(take::<2>(bytes, &mut cursor)?);
        let mut points = [[0_u8; 32]; OVN_CHOICE_COUNT_V1];
        for point in &mut points {
            *point = take::<32>(bytes, &mut cursor)?;
            decode_point(point)?;
        }
        let mut challenges = [[0_u8; 32]; OVN_CHOICE_COUNT_V1];
        for challenge in &mut challenges {
            *challenge = take::<32>(bytes, &mut cursor)?;
            decode_scalar(challenge)?;
        }
        let mut responses = [[[0_u8; 32]; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1];
        for branch in &mut responses {
            for response in branch {
                *response = take::<32>(bytes, &mut cursor)?;
                decode_scalar(response)?;
            }
        }
        if cursor != bytes.len() {
            return Err(OvnError::InvalidEncoding);
        }
        let ballot = Self {
            session_digest,
            roster_root,
            survivor_root,
            participant_hash,
            index,
            points,
            proof: BallotOrProofV1 {
                challenges,
                responses,
            },
        };
        ballot.verify(survivors)?;
        Ok(ballot)
    }

    /// Encode the canonical fixed-width ballot and proof.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BALLOT_WIRE_BYTES_V1);
        bytes.extend_from_slice(BALLOT_MAGIC_V1);
        bytes.extend_from_slice(&self.session_digest);
        bytes.extend_from_slice(&self.roster_root);
        bytes.extend_from_slice(&self.survivor_root);
        bytes.extend_from_slice(&self.participant_hash);
        bytes.extend_from_slice(&self.index.to_be_bytes());
        for point in self.points {
            bytes.extend_from_slice(&point);
        }
        for challenge in self.proof.challenges {
            bytes.extend_from_slice(&challenge);
        }
        for branch in self.proof.responses {
            for response in branch {
                bytes.extend_from_slice(&response);
            }
        }
        bytes
    }

    /// Return the canonical participant identity hash.
    #[must_use]
    pub const fn participant_hash(&self) -> &[u8; 32] {
        &self.participant_hash
    }

    /// Return the zero-based canonical survivor index.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Return the three masked ballot points.
    #[must_use]
    pub const fn points(&self) -> &[[u8; 32]; OVN_CHOICE_COUNT_V1] {
        &self.points
    }

    /// Verify all bindings and the native three-branch one-hot OR proof.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] for a wrong seat/root/session, malformed point or
    /// scalar, or invalid proof equation.
    pub fn verify(&self, survivors: &OvnSurvivorRosterV1) -> Result<(), OvnError> {
        let index = usize::from(self.index);
        let registration = survivors
            .registration_at(index)
            .filter(|registration| registration.participant_hash == self.participant_hash)
            .ok_or(OvnError::BindingMismatch)?;
        if self.session_digest != survivors.session.digest()
            || self.roster_root != survivors.roster_root
            || self.survivor_root != survivors.survivor_root
        {
            return Err(OvnError::BindingMismatch);
        }
        let masks = survivors.masking_keys(&self.participant_hash)?;
        if masks.index != self.index
            || masks.session_digest != self.session_digest
            || masks.roster_root != self.roster_root
            || masks.survivor_root != self.survivor_root
            || masks.participant_hash != self.participant_hash
        {
            return Err(OvnError::BindingMismatch);
        }
        let public_keys = decode_nonidentity_points(&registration.public_keys)?;
        let mask_points = decode_nonidentity_points(&masks.points)?;
        let ballot_points = decode_points(&self.points)?;
        let challenges = decode_scalars(&self.proof.challenges)?;
        let mut responses = [[Scalar::ZERO; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1];
        for (decoded, encoded) in responses.iter_mut().zip(self.proof.responses) {
            for (response, bytes) in decoded.iter_mut().zip(encoded) {
                *response = decode_scalar(&bytes)?;
            }
        }
        let (commitments_g, commitments_mask) = reconstruct_or_commitments(
            &public_keys,
            &mask_points,
            &ballot_points,
            &challenges,
            &responses,
        );
        let expected = ballot_challenge(
            &survivors.session,
            &survivors.roster_root,
            &survivors.survivor_root,
            &self.participant_hash,
            self.index,
            &registration.public_keys,
            &masks.points,
            &self.points,
            &commitments_g,
            &commitments_mask,
        );
        if challenges.into_iter().sum::<Scalar>() != expected {
            return Err(OvnError::InvalidBallotProof);
        }
        Ok(())
    }
}

impl OvnRegistrationSecretV1 {
    /// Cast a choice with fresh operating-system proof randomness.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] for a secret/binding mismatch, degenerate survivor
    /// set, or fallible randomness failure.
    pub fn cast_ballot(
        &self,
        survivors: &OvnSurvivorRosterV1,
        choice: OvnChoiceV1,
    ) -> Result<OvnMaskedBallotV1, OvnError> {
        self.cast_ballot_with_rng(survivors, choice, &mut OsRng)
    }

    /// Cast a choice using an explicit cryptographic proof RNG.
    ///
    /// This entry point supports deterministic interoperability tests. Live
    /// callers must use fresh, non-repeating CSPRNG state for every ballot.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] for a secret/binding mismatch, degenerate survivor
    /// set, or fallible randomness failure.
    pub fn cast_ballot_with_rng<R: TryCryptoRng + ?Sized>(
        &self,
        survivors: &OvnSurvivorRosterV1,
        choice: OvnChoiceV1,
        rng: &mut R,
    ) -> Result<OvnMaskedBallotV1, OvnError> {
        if self.session_digest != survivors.session.digest() {
            return Err(OvnError::BindingMismatch);
        }
        let masks = survivors.masking_keys(&self.participant_hash)?;
        if masks.participant_hash != self.participant_hash {
            return Err(OvnError::BindingMismatch);
        }
        let registration = survivors
            .registration_at(usize::from(masks.index))
            .filter(|registration| registration.participant_hash == self.participant_hash)
            .ok_or(OvnError::UnknownParticipant)?;
        let secrets = self.scalars()?;
        let public_keys = decode_nonidentity_points(&registration.public_keys)?;
        for option in 0..OVN_CHOICE_COUNT_V1 {
            if secrets[option] * RISTRETTO_BASEPOINT_POINT != public_keys[option] {
                return Err(OvnError::SecretMismatch);
            }
        }
        let mask_points = decode_nonidentity_points(&masks.points)?;
        let ballot_points = core::array::from_fn(|option| {
            let vote = if option == choice.index() {
                RISTRETTO_BASEPOINT_POINT
            } else {
                RistrettoPoint::identity()
            };
            secrets[option] * mask_points[option] + vote
        });
        let encoded_ballots = ballot_points.map(|point| point.compress().to_bytes());

        let true_branch = choice.index();
        let mut challenges = [Scalar::ZERO; OVN_CHOICE_COUNT_V1];
        let mut responses = [[Scalar::ZERO; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1];
        let mut true_nonces = [Scalar::ZERO; OVN_CHOICE_COUNT_V1];
        let mut commitments_g =
            core::array::from_fn(|_| core::array::from_fn(|_| RistrettoPoint::identity()));
        let mut commitments_mask =
            core::array::from_fn(|_| core::array::from_fn(|_| RistrettoPoint::identity()));
        for branch in 0..OVN_CHOICE_COUNT_V1 {
            if branch == true_branch {
                for option in 0..OVN_CHOICE_COUNT_V1 {
                    let nonce = random_nonzero_scalar(rng)?;
                    true_nonces[option] = nonce;
                    commitments_g[branch][option] = nonce * RISTRETTO_BASEPOINT_POINT;
                    commitments_mask[branch][option] = nonce * mask_points[option];
                }
            } else {
                challenges[branch] = random_nonzero_scalar(rng)?;
                for option in 0..OVN_CHOICE_COUNT_V1 {
                    responses[branch][option] = random_nonzero_scalar(rng)?;
                    let statement = ballot_points[option]
                        - if option == branch {
                            RISTRETTO_BASEPOINT_POINT
                        } else {
                            RistrettoPoint::identity()
                        };
                    commitments_g[branch][option] = responses[branch][option]
                        * RISTRETTO_BASEPOINT_POINT
                        - challenges[branch] * public_keys[option];
                    commitments_mask[branch][option] = responses[branch][option]
                        * mask_points[option]
                        - challenges[branch] * statement;
                }
            }
        }
        let global_challenge = ballot_challenge(
            &survivors.session,
            &survivors.roster_root,
            &survivors.survivor_root,
            &self.participant_hash,
            masks.index,
            &registration.public_keys,
            &masks.points,
            &encoded_ballots,
            &commitments_g,
            &commitments_mask,
        );
        let simulated_sum = challenges
            .iter()
            .enumerate()
            .filter(|(branch, _)| *branch != true_branch)
            .map(|(_, challenge)| *challenge)
            .sum::<Scalar>();
        challenges[true_branch] = global_challenge - simulated_sum;
        for option in 0..OVN_CHOICE_COUNT_V1 {
            responses[true_branch][option] =
                true_nonces[option] + challenges[true_branch] * secrets[option];
        }
        let ballot = OvnMaskedBallotV1 {
            session_digest: survivors.session.digest(),
            roster_root: survivors.roster_root,
            survivor_root: survivors.survivor_root,
            participant_hash: self.participant_hash,
            index: masks.index,
            points: encoded_ballots,
            proof: BallotOrProofV1 {
                challenges: challenges.map(|scalar| scalar.to_bytes()),
                responses: responses.map(|branch| branch.map(|scalar| scalar.to_bytes())),
            },
        };
        ballot.verify(survivors)?;
        Ok(ballot)
    }
}

/// Public aggregate of every canonical survivor ballot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvnAggregateV1 {
    session_digest: [u8; 32],
    roster_root: [u8; 32],
    survivor_root: [u8; 32],
    accepted_ballots: u16,
    points: [[u8; 32]; OVN_CHOICE_COUNT_V1],
}

impl OvnAggregateV1 {
    /// Decode the three aggregate points with a bounded deterministic search.
    ///
    /// # Errors
    ///
    /// Returns [`OvnError`] unless every point is a count in
    /// `0..=accepted_ballots` and the three counts sum to the corpus size.
    pub fn tally(&self) -> Result<OvnTallyV1, OvnError> {
        if usize::from(self.accepted_ballots) > OVN_MAX_PARTICIPANTS_V1 {
            return Err(OvnError::InvalidTally);
        }
        let mut counts = [0_u16; OVN_CHOICE_COUNT_V1];
        for (count, encoded) in counts.iter_mut().zip(self.points) {
            let point = decode_point(&encoded)?;
            *count = bounded_discrete_log(&point, self.accepted_ballots)
                .ok_or(OvnError::MaskCancellationFailed)?;
        }
        let total = counts
            .iter()
            .try_fold(0_u16, |sum, count| sum.checked_add(*count));
        if total != Some(self.accepted_ballots) {
            return Err(OvnError::InvalidTally);
        }
        Ok(OvnTallyV1 {
            aye: counts[OvnChoiceV1::Aye.index()],
            nay: counts[OvnChoiceV1::Nay.index()],
            abstain: counts[OvnChoiceV1::Abstain.index()],
        })
    }

    /// Return the accepted canonical ballot count.
    #[must_use]
    pub const fn accepted_ballots(&self) -> u16 {
        self.accepted_ballots
    }

    /// Return the OVN session digest bound to the aggregate.
    #[must_use]
    pub const fn session_digest(&self) -> &[u8; 32] {
        &self.session_digest
    }

    /// Return the original registration-roster root bound to the aggregate.
    #[must_use]
    pub const fn roster_root(&self) -> &[u8; 32] {
        &self.roster_root
    }

    /// Return the frozen survivor root bound to the aggregate.
    #[must_use]
    pub const fn survivor_root(&self) -> &[u8; 32] {
        &self.survivor_root
    }

    /// Return the three canonical aggregate points.
    #[must_use]
    pub const fn points(&self) -> &[[u8; 32]; OVN_CHOICE_COUNT_V1] {
        &self.points
    }
}

/// Decoded Aye/Nay/Abstain counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OvnTallyV1 {
    /// Number of Aye ballots.
    pub aye: u16,
    /// Number of Nay ballots.
    pub nay: u16,
    /// Number of Abstain ballots.
    pub abstain: u16,
}

/// Verify and aggregate exactly one ballot per survivor in canonical order.
///
/// # Errors
///
/// Returns [`OvnError`] for an incomplete, duplicate, reordered, replayed, or
/// invalid ballot corpus.
pub fn aggregate_ballots_v1(
    survivors: &OvnSurvivorRosterV1,
    ballots: &[OvnMaskedBallotV1],
) -> Result<OvnAggregateV1, OvnError> {
    if ballots.len() != survivors.registrations.len() {
        return Err(OvnError::NonCanonicalBallotCorpus);
    }
    let mut aggregate = core::array::from_fn(|_| RistrettoPoint::identity());
    for (index, (ballot, registration)) in ballots.iter().zip(&survivors.registrations).enumerate()
    {
        if usize::from(ballot.index) != index
            || ballot.participant_hash != registration.participant_hash
        {
            return Err(OvnError::NonCanonicalBallotCorpus);
        }
        ballot.verify(survivors)?;
        for (sum, encoded) in aggregate.iter_mut().zip(ballot.points) {
            *sum += decode_point(&encoded)?;
        }
    }
    Ok(OvnAggregateV1 {
        session_digest: survivors.session.digest(),
        roster_root: survivors.roster_root,
        survivor_root: survivors.survivor_root,
        accepted_ballots: u16::try_from(ballots.len()).map_err(|_| OvnError::InvalidRosterSize)?,
        points: aggregate.map(|point| point.compress().to_bytes()),
    })
}

fn reconstruct_or_commitments(
    public_keys: &[RistrettoPoint; OVN_CHOICE_COUNT_V1],
    masks: &[RistrettoPoint; OVN_CHOICE_COUNT_V1],
    ballots: &[RistrettoPoint; OVN_CHOICE_COUNT_V1],
    challenges: &[Scalar; OVN_CHOICE_COUNT_V1],
    responses: &[[Scalar; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1],
) -> (
    [[RistrettoPoint; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1],
    [[RistrettoPoint; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1],
) {
    let commitments_g = core::array::from_fn(|branch| {
        core::array::from_fn(|option| {
            responses[branch][option] * RISTRETTO_BASEPOINT_POINT
                - challenges[branch] * public_keys[option]
        })
    });
    let commitments_mask = core::array::from_fn(|branch| {
        core::array::from_fn(|option| {
            let statement = ballots[option]
                - if option == branch {
                    RISTRETTO_BASEPOINT_POINT
                } else {
                    RistrettoPoint::identity()
                };
            responses[branch][option] * masks[option] - challenges[branch] * statement
        })
    });
    (commitments_g, commitments_mask)
}

#[allow(clippy::too_many_arguments)]
fn ballot_challenge(
    session: &OvnSessionV1,
    roster_root: &[u8; 32],
    survivor_root: &[u8; 32],
    participant_hash: &[u8; 32],
    index: u16,
    public_keys: &[[u8; 32]; OVN_CHOICE_COUNT_V1],
    masks: &[[u8; 32]; OVN_CHOICE_COUNT_V1],
    ballots: &[[u8; 32]; OVN_CHOICE_COUNT_V1],
    commitments_g: &[[RistrettoPoint; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1],
    commitments_mask: &[[RistrettoPoint; OVN_CHOICE_COUNT_V1]; OVN_CHOICE_COUNT_V1],
) -> Scalar {
    let mut hasher = Sha512::new();
    hasher.update(BALLOT_CHALLENGE_DOMAIN_V1);
    hasher.update(session.canonical_bytes());
    hasher.update(roster_root);
    hasher.update(survivor_root);
    hasher.update(participant_hash);
    hasher.update(index.to_be_bytes());
    for option in 0..OVN_CHOICE_COUNT_V1 {
        hasher.update([OPTION_TAGS_V1[option]]);
        hasher.update(public_keys[option]);
        hasher.update(masks[option]);
        hasher.update(ballots[option]);
    }
    for branch in 0..OVN_CHOICE_COUNT_V1 {
        for option in 0..OVN_CHOICE_COUNT_V1 {
            hasher.update(commitments_g[branch][option].compress().as_bytes());
            hasher.update(commitments_mask[branch][option].compress().as_bytes());
        }
    }
    scalar_from_sha512(hasher)
}

fn pop_challenge(
    session: &OvnSessionV1,
    participant_hash: &[u8; 32],
    option: usize,
    public_key: &[u8; 32],
    commitment: &[u8; 32],
) -> Scalar {
    let mut hasher = Sha512::new();
    hasher.update(POP_CHALLENGE_DOMAIN_V1);
    hasher.update(session.canonical_bytes());
    hasher.update(participant_hash);
    hasher.update([OPTION_TAGS_V1[option]]);
    hasher.update(public_key);
    hasher.update(commitment);
    scalar_from_sha512(hasher)
}

fn scalar_from_sha512(hasher: Sha512) -> Scalar {
    let wide: [u8; 64] = hasher.finalize().into();
    Scalar::from_bytes_mod_order_wide(&wide)
}

fn random_nonzero_scalar<R: TryCryptoRng + ?Sized>(rng: &mut R) -> Result<Scalar, OvnError> {
    let mut wide = Zeroizing::new([0_u8; 64]);
    rng.try_fill_bytes(wide.as_mut())
        .map_err(|_| OvnError::RandomnessUnavailable)?;
    if is_zero(wide.as_ref()) {
        return Err(OvnError::InertRandomness);
    }
    let scalar = Scalar::from_bytes_mod_order_wide(&wide);
    if scalar == Scalar::ZERO {
        return Err(OvnError::InertRandomness);
    }
    Ok(scalar)
}

fn roster_root(session: &OvnSessionV1, registrations: &[OvnRegistrationV1]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ROSTER_ROOT_DOMAIN_V1);
    hasher.update(session.canonical_bytes());
    hasher.update((registrations.len() as u64).to_be_bytes());
    for registration in registrations {
        hasher.update(registration.to_bytes());
    }
    hasher.finalize().into()
}

fn survivor_root(
    session: &OvnSessionV1,
    roster_root: &[u8; 32],
    registrations: &[OvnRegistrationV1],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SURVIVOR_ROOT_DOMAIN_V1);
    hasher.update(session.canonical_bytes());
    hasher.update(roster_root);
    hasher.update((registrations.len() as u64).to_be_bytes());
    for registration in registrations {
        hasher.update(registration.participant_hash);
        for public_key in registration.public_keys {
            hasher.update(public_key);
        }
    }
    hasher.finalize().into()
}

fn bounded_discrete_log(point: &RistrettoPoint, max: u16) -> Option<u16> {
    let mut candidate = RistrettoPoint::identity();
    for count in 0..=max {
        if candidate == *point {
            return Some(count);
        }
        candidate += RISTRETTO_BASEPOINT_POINT;
    }
    None
}

fn decode_point(bytes: &[u8; 32]) -> Result<RistrettoPoint, OvnError> {
    let point = CompressedRistretto(*bytes)
        .decompress()
        .ok_or(OvnError::InvalidPoint)?;
    if point.compress().to_bytes() != *bytes {
        return Err(OvnError::InvalidPoint);
    }
    Ok(point)
}

fn decode_nonidentity_point(bytes: &[u8; 32]) -> Result<RistrettoPoint, OvnError> {
    let point = decode_point(bytes)?;
    if point == RistrettoPoint::identity() {
        return Err(OvnError::IdentityPoint);
    }
    Ok(point)
}

fn decode_scalar(bytes: &[u8; 32]) -> Result<Scalar, OvnError> {
    Scalar::from_canonical_bytes(*bytes)
        .into_option()
        .ok_or(OvnError::InvalidScalar)
}

fn decode_points(
    encoded: &[[u8; 32]; OVN_CHOICE_COUNT_V1],
) -> Result<[RistrettoPoint; OVN_CHOICE_COUNT_V1], OvnError> {
    Ok([
        decode_point(&encoded[0])?,
        decode_point(&encoded[1])?,
        decode_point(&encoded[2])?,
    ])
}

fn decode_nonidentity_points(
    encoded: &[[u8; 32]; OVN_CHOICE_COUNT_V1],
) -> Result<[RistrettoPoint; OVN_CHOICE_COUNT_V1], OvnError> {
    Ok([
        decode_nonidentity_point(&encoded[0])?,
        decode_nonidentity_point(&encoded[1])?,
        decode_nonidentity_point(&encoded[2])?,
    ])
}

fn decode_scalars(
    encoded: &[[u8; 32]; OVN_CHOICE_COUNT_V1],
) -> Result<[Scalar; OVN_CHOICE_COUNT_V1], OvnError> {
    Ok([
        decode_scalar(&encoded[0])?,
        decode_scalar(&encoded[1])?,
        decode_scalar(&encoded[2])?,
    ])
}

fn take<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], OvnError> {
    let end = cursor.checked_add(N).ok_or(OvnError::InvalidEncoding)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(OvnError::InvalidEncoding)?
        .try_into()
        .map_err(|_| OvnError::InvalidEncoding)?;
    *cursor = end;
    Ok(value)
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng as _;

    use super::*;

    fn binding(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn session(last: u8) -> OvnSessionV1 {
        OvnSessionV1::new(
            binding(1),
            binding(2),
            binding(3),
            binding(4),
            binding(last),
            binding(6),
        )
        .expect("session")
    }

    fn fixture() -> (
        OvnSessionV1,
        OvnRosterV1,
        OvnSurvivorRosterV1,
        Vec<OvnRegistrationSecretV1>,
    ) {
        let session = session(5);
        let mut secrets = Vec::new();
        let mut registrations = Vec::new();
        for participant in 10_u8..14 {
            let mut rng = ChaCha20Rng::from_seed([participant; 32]);
            let (secret, registration) = OvnRegistrationSecretV1::generate_with_rng(
                &session,
                binding(participant),
                &mut rng,
            )
            .expect("registration");
            secrets.push(secret);
            registrations.push(registration);
        }
        let roster = OvnRosterV1::new(session, registrations).expect("roster");
        let survivor_ids = [binding(10), binding(11), binding(12), binding(13)];
        let survivors = OvnSurvivorRosterV1::new(&roster, &survivor_ids).expect("survivors");
        (session, roster, survivors, secrets)
    }

    #[test]
    fn registration_wire_and_pop_fail_closed() {
        let session = session(5);
        let (_, registration) = OvnRegistrationSecretV1::generate_with_rng(
            &session,
            binding(10),
            &mut ChaCha20Rng::from_seed([10; 32]),
        )
        .expect("registration");
        let encoded = registration.to_bytes();
        assert_eq!(
            OvnRegistrationV1::from_bytes(&session, &encoded),
            Ok(registration.clone())
        );
        assert_eq!(
            OvnRegistrationV1::from_bytes(&session, &encoded[..encoded.len() - 1]),
            Err(OvnError::InvalidEncoding)
        );

        let mut identity = encoded.clone();
        identity[72..104].fill(0);
        assert_eq!(
            OvnRegistrationV1::from_bytes(&session, &identity),
            Err(OvnError::IdentityPoint)
        );

        let mut forged = encoded;
        forged[136] ^= 1;
        assert!(matches!(
            OvnRegistrationV1::from_bytes(&session, &forged),
            Err(OvnError::InvalidProofOfPossession | OvnError::InvalidScalar)
        ));
    }

    #[test]
    fn roster_rejects_duplicates_and_reordering() {
        let (_, _, _, _) = fixture();
        let session = session(5);
        let mut registrations = Vec::new();
        for participant in [10_u8, 11] {
            let (_, registration) = OvnRegistrationSecretV1::generate_with_rng(
                &session,
                binding(participant),
                &mut ChaCha20Rng::from_seed([participant; 32]),
            )
            .expect("registration");
            registrations.push(registration);
        }
        assert_eq!(
            OvnRosterV1::new(
                session,
                vec![registrations[0].clone(), registrations[0].clone()]
            ),
            Err(OvnError::DuplicateParticipant)
        );
        registrations.reverse();
        assert_eq!(
            OvnRosterV1::new(session, registrations),
            Err(OvnError::NonCanonicalRoster)
        );
    }

    #[test]
    fn survivor_list_rejects_unknown_duplicate_and_reordered_ids() {
        let (_, roster, _, _) = fixture();
        assert_eq!(
            OvnSurvivorRosterV1::new(&roster, &[binding(10), binding(10)]),
            Err(OvnError::NonCanonicalSurvivorSet)
        );
        assert_eq!(
            OvnSurvivorRosterV1::new(&roster, &[binding(11), binding(10)]),
            Err(OvnError::NonCanonicalSurvivorSet)
        );
        assert_eq!(
            OvnSurvivorRosterV1::new(&roster, &[binding(99)]),
            Err(OvnError::NonCanonicalSurvivorSet)
        );
    }

    #[test]
    fn three_choice_proofs_cancel_and_tally() {
        let (_, _, survivors, secrets) = fixture();
        let choices = [
            OvnChoiceV1::Aye,
            OvnChoiceV1::Nay,
            OvnChoiceV1::Abstain,
            OvnChoiceV1::Aye,
        ];
        let ballots = secrets
            .iter()
            .zip(choices)
            .enumerate()
            .map(|(index, (secret, choice))| {
                secret
                    .cast_ballot_with_rng(
                        &survivors,
                        choice,
                        &mut ChaCha20Rng::from_seed([40 + index as u8; 32]),
                    )
                    .expect("ballot")
            })
            .collect::<Vec<_>>();
        for ballot in &ballots {
            ballot.verify(&survivors).expect("verify ballot");
            assert_eq!(
                OvnMaskedBallotV1::from_bytes(&survivors, &ballot.to_bytes()),
                Ok(ballot.clone())
            );
        }
        let aggregate = aggregate_ballots_v1(&survivors, &ballots).expect("aggregate");
        assert_eq!(
            aggregate.tally(),
            Ok(OvnTallyV1 {
                aye: 2,
                nay: 1,
                abstain: 1,
            })
        );
    }

    #[test]
    fn ballot_replay_non_one_hot_and_corpus_reordering_fail() {
        let (_, _, survivors, secrets) = fixture();
        let ballot = secrets[0]
            .cast_ballot_with_rng(
                &survivors,
                OvnChoiceV1::Aye,
                &mut ChaCha20Rng::from_seed([50; 32]),
            )
            .expect("ballot");
        let mut forged = ballot.clone();
        let second = decode_point(&forged.points[1]).expect("point") + RISTRETTO_BASEPOINT_POINT;
        forged.points[1] = second.compress().to_bytes();
        assert_eq!(forged.verify(&survivors), Err(OvnError::InvalidBallotProof));

        let other_session = session(55);
        let mut other_regs = Vec::new();
        for participant in 10_u8..14 {
            let (_, registration) = OvnRegistrationSecretV1::generate_with_rng(
                &other_session,
                binding(participant),
                &mut ChaCha20Rng::from_seed([participant + 20; 32]),
            )
            .expect("registration");
            other_regs.push(registration);
        }
        let other_roster = OvnRosterV1::new(other_session, other_regs).expect("roster");
        let other_survivors = OvnSurvivorRosterV1::new(
            &other_roster,
            &[binding(10), binding(11), binding(12), binding(13)],
        )
        .expect("survivors");
        assert_eq!(
            OvnMaskedBallotV1::from_bytes(&other_survivors, &ballot.to_bytes()),
            Err(OvnError::BindingMismatch)
        );

        let mut ballots = Vec::new();
        for (index, secret) in secrets.iter().enumerate() {
            ballots.push(
                secret
                    .cast_ballot_with_rng(
                        &survivors,
                        OvnChoiceV1::Abstain,
                        &mut ChaCha20Rng::from_seed([60 + index as u8; 32]),
                    )
                    .expect("ballot"),
            );
        }
        ballots.swap(0, 1);
        assert_eq!(
            aggregate_ballots_v1(&survivors, &ballots),
            Err(OvnError::NonCanonicalBallotCorpus)
        );
    }

    #[test]
    fn malformed_ballot_and_wrong_survivor_root_fail() {
        let (_, roster, survivors, secrets) = fixture();
        let ballot = secrets[0]
            .cast_ballot_with_rng(
                &survivors,
                OvnChoiceV1::Nay,
                &mut ChaCha20Rng::from_seed([70; 32]),
            )
            .expect("ballot");
        let mut malformed = ballot.to_bytes();
        malformed[138..170].fill(0xff);
        assert_eq!(
            OvnMaskedBallotV1::from_bytes(&survivors, &malformed),
            Err(OvnError::InvalidPoint)
        );
        let subset = OvnSurvivorRosterV1::new(&roster, &[binding(10), binding(11), binding(12)])
            .expect("subset");
        assert_eq!(
            OvnMaskedBallotV1::from_bytes(&subset, &ballot.to_bytes()),
            Err(OvnError::BindingMismatch)
        );
    }

    #[test]
    fn bounded_decoder_rejects_count_above_limit() {
        let point = Scalar::from(1_001_u64) * RISTRETTO_BASEPOINT_POINT;
        assert_eq!(bounded_discrete_log(&point, 1_000), None);
    }

    #[test]
    fn zero_rng_and_single_survivor_fail_closed() {
        #[derive(Debug)]
        struct ZeroRng;
        impl rand_core::RngCore for ZeroRng {
            fn next_u32(&mut self) -> u32 {
                0
            }
            fn next_u64(&mut self) -> u64 {
                0
            }
            fn fill_bytes(&mut self, destination: &mut [u8]) {
                destination.fill(0);
            }
        }
        impl rand_core::CryptoRng for ZeroRng {}
        assert!(matches!(
            OvnRegistrationSecretV1::generate_with_rng(&session(5), binding(10), &mut ZeroRng),
            Err(OvnError::InertRandomness)
        ));

        let (_, roster, _, _) = fixture();
        let single = OvnSurvivorRosterV1::new(&roster, &[binding(10)]).expect("single survivor");
        assert_eq!(
            single.masking_keys(&binding(10)),
            Err(OvnError::IdentityPoint)
        );
    }
}

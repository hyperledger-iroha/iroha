//! Canonical bounded Norito-profile wire for collective ZK-AMS artifacts.
//!
//! These records use one fixed first-release layout: big-endian integers,
//! explicit fixed-width type tags, and `u32` collection lengths.  Decoders
//! preflight every declared count and the complete byte length before any
//! attacker-sized allocation.  This manual profile is intentional: generic
//! collection decoding cannot enforce the different polynomial, digit, proof,
//! and sample ceilings early enough for release-sized (`N = 131_072`) inputs.
//!
//! This module implements the canonical representation/predecode obligation.
//! Decryption shares deliberately do not reuse its combined-contribution
//! frame: their release transport is the authenticated split manifest plus
//! separately addressed polynomial and native-proof objects implemented by
//! `decryption`.

use std::sync::Arc;

use super::{
    BgvProfile, MKHE_VERSION_V1, Scalar, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    checked_rns_polynomial_bytes,
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_release_manifest_v1,
    },
};
use crate::vega::{
    VegaT256PointV1,
    sponge::{Keccak256, keccak256},
};

const ROSTER_TAG_V1: [u8; 4] = *b"ZAGR";
pub(super) const GOVERNED_ROSTER_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.governed-roster";
const CIPHERTEXT_TAG_V1: [u8; 4] = *b"ZACT";
#[cfg(test)]
const SEEDED_RKG_KEY_TAG_V1: [u8; 4] = *b"ZARK";
const CKS_CONTRIBUTION_TAG_V1: [u8; 4] = *b"ZACK";
const PROOF_ENVELOPE_TAG_V1: [u8; 4] = *b"ZAPE";

const AUTHENTICATION_WIRE_BYTES: usize = 1 + 32 + 33 + 65;
const COMMON_BINDING_WIRE_BYTES: usize = 4 + 1 + 32 + 32 + 8 + 32 + 4 + 1;
const ROSTER_WIRE_BYTES: usize = 4 + 1 + 32 + 8 + 1 + ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 * 32;
const CIPHERTEXT_HEADER_WIRE_BYTES: usize = COMMON_BINDING_WIRE_BYTES + 8;
const SEEDED_RKG_KEY_HEADER_WIRE_BYTES: usize = COMMON_BINDING_WIRE_BYTES + 32 + 32 + 1;
const CONTRIBUTION_HEADER_WIRE_BYTES: usize =
    COMMON_BINDING_WIRE_BYTES + 32 + AUTHENTICATION_WIRE_BYTES + 4;
const PROOF_ENVELOPE_HEADER_WIRE_BYTES: usize = COMMON_BINDING_WIRE_BYTES + 1 + 32 + 4;

/// Absolute allocation ceiling for one opaque canonical proof payload.
///
/// The enclosing CKS/RKG record applies its stricter governed round ceiling as
/// well. Proof-system decoders must independently enforce
/// their own exact canonical layout after this transport layer has preflighted
/// the byte string.
pub const ZK_AMS_MKHE_MAX_PROOF_BYTES_V1: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
struct WireDimensions<'a> {
    ring_degree: usize,
    moduli: &'a [u64],
    #[cfg(test)]
    gadget_digits: usize,
    roster_size: usize,
    max_samples: u64,
    max_ciphertext_bytes: usize,
    #[cfg(test)]
    max_evaluated_key_bytes: usize,
    max_round_bytes: usize,
}

impl<'a> WireDimensions<'a> {
    fn coefficient_count(self) -> Result<usize, ZkAmsMkheErrorV1> {
        self.ring_degree
            .checked_mul(self.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }

    fn polynomial_wire_bytes(self) -> Result<usize, ZkAmsMkheErrorV1> {
        self.coefficient_count()?
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }
}

/// Exact lengths of the first-release canonical collective wire records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheWireLengthCertificateV1 {
    pub(super) governed_roster_wire_bytes: usize,
    pub(super) rns_polynomial_wire_bytes: usize,
    pub(super) compact_collective_ciphertext_wire_bytes: usize,
    pub(super) multiplication_triple_wire_bytes: usize,
    pub(super) seeded_collective_relinearization_key_wire_bytes: usize,
    pub(super) streamed_contribution_base_wire_bytes: usize,
    pub(super) proof_envelope_header_wire_bytes: usize,
}

pub(super) fn derive_wire_length_certificate_v1(
    profile: &BgvProfile,
) -> Result<ZkAmsMkheWireLengthCertificateV1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    let polynomial = checked_rns_polynomial_bytes(profile)?;
    let ciphertext = CIPHERTEXT_HEADER_WIRE_BYTES
        .checked_add(
            polynomial
                .checked_mul(2)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let triple = CIPHERTEXT_HEADER_WIRE_BYTES
        .checked_add(
            polynomial
                .checked_mul(3)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let seeded_key = profile
        .gadget_digits
        .checked_mul(
            polynomial
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|bytes| bytes.checked_add(SEEDED_RKG_KEY_HEADER_WIRE_BYTES))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let contribution = CONTRIBUTION_HEADER_WIRE_BYTES
        .checked_add(polynomial)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok(ZkAmsMkheWireLengthCertificateV1 {
        governed_roster_wire_bytes: ROSTER_WIRE_BYTES,
        rns_polynomial_wire_bytes: polynomial,
        compact_collective_ciphertext_wire_bytes: ciphertext,
        multiplication_triple_wire_bytes: triple,
        seeded_collective_relinearization_key_wire_bytes: seeded_key,
        streamed_contribution_base_wire_bytes: contribution,
        proof_envelope_header_wire_bytes: PROOF_ENVELOPE_HEADER_WIRE_BYTES,
    })
}

fn release_dimensions() -> Result<WireDimensions<'static>, ZkAmsMkheErrorV1> {
    // `release_profile_v1` only references static modulus/root arrays.  Build
    // the dimensions directly so no reference to the local profile escapes.
    let profile = release_profile_v1();
    let manifest = zk_ams_mkhe_release_manifest_v1()?;
    profile.validate()?;
    Ok(WireDimensions {
        ring_degree: profile.ring_degree,
        moduli: profile.moduli,
        #[cfg(test)]
        gadget_digits: profile.gadget_digits,
        roster_size: ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        max_samples: manifest.max_samples_per_secret_epoch,
        max_ciphertext_bytes: profile.max_ciphertext_bytes,
        #[cfg(test)]
        max_evaluated_key_bytes: profile.max_evaluated_key_bytes,
        max_round_bytes: profile.max_round_bytes,
    })
}

/// Exact profile/roster/epoch/transcript/index/level binding of one wire record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheWireBindingV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    record_index: u32,
    level: u8,
}

impl ZkAmsMkheWireBindingV1 {
    /// Construct a binding under one previously validated governed roster.
    pub fn new(
        roster: &ZkAmsMkheGovernedRosterWireV1,
        transcript_digest: [u8; 32],
        record_index: u32,
        level: u8,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        let binding = Self {
            profile_digest: roster.profile_digest,
            roster_digest: roster.roster_digest,
            epoch: roster.epoch,
            transcript_digest,
            record_index,
            level,
        };
        binding.validate(dimensions)?;
        Ok(binding)
    }

    fn validate(self, dimensions: WireDimensions<'_>) -> Result<(), ZkAmsMkheErrorV1> {
        if self.profile_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || u64::from(self.record_index) >= dimensions.max_samples
            || self.level > 1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    /// Exact frozen profile digest.
    #[must_use]
    pub const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }

    /// Digest of the exact ordered governed roster.
    #[must_use]
    pub const fn roster_digest(self) -> [u8; 32] {
        self.roster_digest
    }

    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Exact protocol transcript digest.
    #[must_use]
    pub const fn transcript_digest(self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Canonical zero-based record index.
    #[must_use]
    pub const fn record_index(self) -> u32 {
        self.record_index
    }

    /// BGV level (`0` or `1`).
    #[must_use]
    pub const fn level(self) -> u8 {
        self.level
    }
}

/// Exact eight-party governed release roster and secret epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheGovernedRosterWireV1 {
    profile_digest: [u8; 32],
    epoch: u64,
    parties: [ZkAmsMkhePartyIdV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    roster_digest: [u8; 32],
}

impl ZkAmsMkheGovernedRosterWireV1 {
    /// Construct the sole fixed-size roster form; parties must be strictly ordered.
    pub fn new(
        profile_digest: [u8; 32],
        epoch: u64,
        parties: [ZkAmsMkhePartyIdV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let expected_profile = release_profile_v1().digest()?;
        if profile_digest != expected_profile
            || epoch == 0
            || parties.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let roster_digest = governed_roster_digest(profile_digest, epoch, &parties);
        Ok(Self {
            profile_digest,
            epoch,
            parties,
            roster_digest,
        })
    }

    /// Encode the exact fixed-width roster record.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let mut encoder = WireEncoder::new(ROSTER_WIRE_BYTES)?;
        encoder.bytes(&ROSTER_TAG_V1);
        encoder.u8(MKHE_VERSION_V1);
        encoder.bytes(&self.profile_digest);
        encoder.u64(self.epoch);
        encoder
            .u8(u8::try_from(self.parties.len()).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?);
        for party in self.parties {
            encoder.bytes(&party.to_bytes());
        }
        encoder.finish_exact(ROSTER_WIRE_BYTES)
    }

    /// Decode exactly one roster under independently trusted profile and epoch.
    pub fn decode_exact(
        bytes: &[u8],
        expected_profile_digest: [u8; 32],
        expected_epoch: u64,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != ROSTER_WIRE_BYTES {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut decoder = WireDecoder::new(bytes);
        decoder.expect_bytes(&ROSTER_TAG_V1)?;
        decoder.expect_u8(MKHE_VERSION_V1)?;
        let profile_digest = decoder.array()?;
        let epoch = decoder.u64()?;
        if profile_digest != expected_profile_digest || epoch != expected_epoch {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        decoder.expect_u8(
            u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        )?;
        let mut parties = [ZkAmsMkhePartyIdV1::new([1; 32])?; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
        for party in &mut parties {
            *party = ZkAmsMkhePartyIdV1::new(decoder.array()?)?;
        }
        decoder.finish()?;
        Self::new(profile_digest, epoch, parties)
    }

    /// Exact frozen profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    /// Exact governed epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Strictly ordered roster members.
    #[must_use]
    pub const fn parties(&self) -> &[ZkAmsMkhePartyIdV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.parties
    }

    /// Consensus digest of profile, epoch, and exact ordered parties.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
}

pub(super) fn governed_roster_digest(
    profile_digest: [u8; 32],
    epoch: u64,
    parties: &[ZkAmsMkhePartyIdV1],
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(96 + parties.len() * 32);
    frame.extend_from_slice(GOVERNED_ROSTER_DOMAIN_V1);
    frame.extend_from_slice(&profile_digest);
    frame.extend_from_slice(&epoch.to_be_bytes());
    frame.push(u8::try_from(parties.len()).unwrap_or(u8::MAX));
    for party in parties {
        frame.extend_from_slice(&party.to_bytes());
    }
    keccak256(&frame)
}

/// Canonical limb-major RNS residue vector for the frozen release profile.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkAmsMkheRnsPolynomialWireV1 {
    residues: Arc<Vec<u64>>,
}

impl core::fmt::Debug for ZkAmsMkheRnsPolynomialWireV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheRnsPolynomialWireV1")
            .field("residue_count", &self.residues.len())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheRnsPolynomialWireV1 {
    /// Construct a release polynomial from exact canonical limb-major residues.
    pub fn new(residues: Vec<u64>) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_with_dimensions(residues, release_dimensions()?)
    }

    fn new_with_dimensions(
        residues: Vec<u64>,
        dimensions: WireDimensions<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_residues(&residues, dimensions)?;
        // Keep the caller's Vec allocation as the sole canonical backing.
        // `Arc<[u64]>` would require copying this release-sized table into a
        // new dynamically sized Arc allocation.
        Ok(Self {
            residues: Arc::new(residues),
        })
    }

    /// Borrow the exact limb-major residues.
    #[must_use]
    pub fn residues(&self) -> &[u64] {
        self.residues.as_slice()
    }

    pub(super) fn shared_residues(&self) -> Arc<Vec<u64>> {
        Arc::clone(&self.residues)
    }

    /// Exact encoded bytes under the release dimensions.
    pub fn encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        self.validate(release_dimensions()?)?;
        release_dimensions()?.polynomial_wire_bytes()
    }

    fn validate(&self, dimensions: WireDimensions<'_>) -> Result<(), ZkAmsMkheErrorV1> {
        validate_residues(self.residues.as_slice(), dimensions)
    }
}

fn validate_residues(
    residues: &[u64],
    dimensions: WireDimensions<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if residues.len() != dimensions.coefficient_count()? {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (limb, values) in residues.chunks_exact(dimensions.ring_degree).enumerate() {
        if values.iter().any(|value| *value >= dimensions.moduli[limb]) {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    Ok(())
}

/// Canonical authentication material carried by contribution/share records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheAuthenticationWireV1 {
    party: ZkAmsMkhePartyIdV1,
    public_key: [u8; 33],
    signature: [u8; 65],
}

impl ZkAmsMkheAuthenticationWireV1 {
    /// Construct canonical authentication bytes and bind the party to its key.
    pub fn new(
        party: ZkAmsMkhePartyIdV1,
        public_key: [u8; 33],
        signature: [u8; 65],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            party,
            public_key,
            signature,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.party != ZkAmsMkhePartyIdV1::from_authentication_key(&self.public_key)? {
            return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
        }
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&self.public_key)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&self.signature[..33])
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
        let response: [u8; 32] = self.signature[33..]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
        Scalar::from_be_bytes_exact(response)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
        Ok(())
    }

    /// Authentication-key-derived participant identifier.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Canonical nonidentity T256 public key.
    #[must_use]
    pub const fn public_key(&self) -> [u8; 33] {
        self.public_key
    }

    /// Canonical commitment plus scalar response.
    #[must_use]
    pub const fn signature(&self) -> [u8; 65] {
        self.signature
    }
}

/// Exact contribution-proof family carried by a proof envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheProofKindV1 {
    /// Collective-key-switch contribution proof.
    CksContribution = 1,
    /// RKG digit/contribution proof.
    RkgContribution = 2,
}

impl TryFrom<u8> for ZkAmsMkheProofKindV1 {
    type Error = ZkAmsMkheErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::CksContribution),
            2 => Ok(Self::RkgContribution),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

/// Canonical kind-tagged envelope for one proof system's exact byte encoding.
///
/// The transport does not invent a common curve-point/scalar shape: the active
/// RKG and CKS proof systems have different native transcripts and enforce
/// their canonical encodings in their own decoders. Decryption proofs use
/// their standalone native `ZADP` encoding instead.
#[cfg_attr(test, derive(Clone))]
#[derive(PartialEq, Eq)]
pub struct ZkAmsMkheProofEnvelopeWireV1 {
    binding: ZkAmsMkheWireBindingV1,
    kind: ZkAmsMkheProofKindV1,
    statement_digest: [u8; 32],
    proof_bytes: Vec<u8>,
}

impl core::fmt::Debug for ZkAmsMkheProofEnvelopeWireV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheProofEnvelopeWireV1")
            .field("binding", &self.binding)
            .field("kind", &self.kind)
            .field("statement_digest", &hex::encode(self.statement_digest))
            .field("proof_bytes_len", &self.proof_bytes.len())
            .field("proof_bytes", &"<redacted>")
            .finish()
    }
}

impl Drop for ZkAmsMkheProofEnvelopeWireV1 {
    fn drop(&mut self) {
        let proof_bytes = core::hint::black_box(&mut self.proof_bytes);
        proof_bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        PROOF_ENVELOPE_ZEROIZED_DROP_COUNT_V1.with(|count| {
            debug_assert!(proof_bytes.iter().all(|byte| *byte == 0));
            count.set(count.get().saturating_add(1));
        });
        let _ = core::hint::black_box(&mut *proof_bytes);
    }
}

#[cfg(test)]
std::thread_local! {
    static PROOF_ENVELOPE_ZEROIZED_DROP_COUNT_V1: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn proof_envelope_zeroized_drop_count_v1() -> usize {
    PROOF_ENVELOPE_ZEROIZED_DROP_COUNT_V1.with(core::cell::Cell::get)
}

impl ZkAmsMkheProofEnvelopeWireV1 {
    /// Construct one exact proof envelope and validate all canonical fields.
    pub fn new(
        binding: ZkAmsMkheWireBindingV1,
        kind: ZkAmsMkheProofKindV1,
        statement_digest: [u8; 32],
        proof_bytes: Vec<u8>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            binding,
            kind,
            statement_digest,
            proof_bytes,
        };
        value.validate(release_dimensions()?)?;
        Ok(value)
    }

    /// Encode the sole canonical proof-envelope layout.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        self.validate(dimensions)?;
        let length = proof_envelope_wire_bytes(self.proof_bytes.len())?;
        let mut encoder = WireEncoder::new(length)?;
        self.write_to(&mut encoder)?;
        encoder.finish_exact(length)
    }

    /// Decode one proof envelope under an independently trusted exact binding and kind.
    pub fn decode_exact(
        bytes: &[u8],
        expected_binding: ZkAmsMkheWireBindingV1,
        expected_kind: ZkAmsMkheProofKindV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        preflight_proof_envelope(bytes, expected_binding, expected_kind, dimensions)?;
        decode_proof_envelope(bytes, expected_binding, expected_kind, dimensions)
    }

    /// Exact artifact binding.
    #[must_use]
    pub const fn binding(&self) -> ZkAmsMkheWireBindingV1 {
        self.binding
    }

    /// Proof family.
    #[must_use]
    pub const fn kind(&self) -> ZkAmsMkheProofKindV1 {
        self.kind
    }

    /// Digest of the exact statement proved by this envelope.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }

    /// Exact canonical bytes of the proof-system-specific proof.
    #[must_use]
    pub fn proof_bytes(&self) -> &[u8] {
        &self.proof_bytes
    }

    fn validate(&self, dimensions: WireDimensions<'_>) -> Result<(), ZkAmsMkheErrorV1> {
        self.binding.validate(dimensions)?;
        if self.statement_digest == [0; 32]
            || self.proof_bytes.is_empty()
            || self.proof_bytes.len() > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        proof_envelope_wire_bytes(self.proof_bytes.len())?;
        Ok(())
    }

    fn write_to(&self, encoder: &mut WireEncoder) -> Result<(), ZkAmsMkheErrorV1> {
        write_binding(encoder, PROOF_ENVELOPE_TAG_V1, self.binding);
        encoder.u8(self.kind as u8);
        encoder.bytes(&self.statement_digest);
        encoder.u32(as_u32(self.proof_bytes.len())?);
        encoder.bytes(&self.proof_bytes);
        Ok(())
    }
}

/// Compact collective ciphertext containing exactly two RNS polynomials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveCiphertextWireV1 {
    binding: ZkAmsMkheWireBindingV1,
    sample_index: u64,
    constant: ZkAmsMkheRnsPolynomialWireV1,
    linear: ZkAmsMkheRnsPolynomialWireV1,
}

impl ZkAmsMkheCollectiveCiphertextWireV1 {
    /// Construct one exact two-polynomial collective ciphertext.
    pub fn new(
        binding: ZkAmsMkheWireBindingV1,
        sample_index: u64,
        constant: ZkAmsMkheRnsPolynomialWireV1,
        linear: ZkAmsMkheRnsPolynomialWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        let value = Self {
            binding,
            sample_index,
            constant,
            linear,
        };
        value.validate(dimensions)?;
        Ok(value)
    }

    /// Encode the sole canonical compact ciphertext layout.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        self.validate(dimensions)?;
        let length = collective_ciphertext_wire_bytes(dimensions)?;
        let mut encoder = WireEncoder::new(length)?;
        write_binding(&mut encoder, CIPHERTEXT_TAG_V1, self.binding);
        encoder.u64(self.sample_index);
        write_polynomial(&mut encoder, &self.constant)?;
        write_polynomial(&mut encoder, &self.linear)?;
        encoder.finish_exact(length)
    }

    /// Decode exactly under a trusted binding and epoch sample index.
    pub fn decode_exact(
        bytes: &[u8],
        expected_binding: ZkAmsMkheWireBindingV1,
        expected_sample_index: u64,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        preflight_ciphertext(bytes, expected_binding, expected_sample_index, dimensions)?;
        let mut decoder = WireDecoder::new(bytes);
        read_binding(
            &mut decoder,
            CIPHERTEXT_TAG_V1,
            expected_binding,
            dimensions,
        )?;
        decoder.expect_u64(expected_sample_index)?;
        let constant = read_polynomial(&mut decoder, dimensions)?;
        let linear = read_polynomial(&mut decoder, dimensions)?;
        decoder.finish()?;
        Self::new(expected_binding, expected_sample_index, constant, linear)
    }

    /// Exact artifact binding.
    #[must_use]
    pub const fn binding(&self) -> ZkAmsMkheWireBindingV1 {
        self.binding
    }

    /// Zero-based RLWE sample index within the governed secret epoch.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }

    /// Constant ciphertext polynomial.
    #[must_use]
    pub const fn constant(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.constant
    }

    /// Collective-secret ciphertext polynomial.
    #[must_use]
    pub const fn linear(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.linear
    }

    /// Clone immutable polynomial owners while replacing only public binding
    /// metadata in hostile tests. The release-sized residue tables remain
    /// shared; validation still applies every canonical wire constraint.
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "hostile binding mutation seam retained for wire reference tests"
    )]
    pub(super) fn with_binding_and_sample_for_test(
        &self,
        binding: ZkAmsMkheWireBindingV1,
        sample_index: u64,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        let value = Self {
            binding,
            sample_index,
            constant: self.constant.clone(),
            linear: self.linear.clone(),
        };
        value.validate(dimensions)?;
        Ok(value)
    }

    fn validate(&self, dimensions: WireDimensions<'_>) -> Result<(), ZkAmsMkheErrorV1> {
        self.binding.validate(dimensions)?;
        if self.sample_index >= dimensions.max_samples {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.constant.validate(dimensions)?;
        self.linear.validate(dimensions)?;
        if collective_ciphertext_wire_bytes(dimensions)? > dimensions.max_ciphertext_bytes {
            return Err(ZkAmsMkheErrorV1::WireTooLarge);
        }
        Ok(())
    }
}

/// One proof-bound collective-key-switch contribution.
#[cfg_attr(test, derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCksContributionWireV1 {
    binding: ZkAmsMkheWireBindingV1,
    source_ciphertext_digest: [u8; 32],
    authentication: ZkAmsMkheAuthenticationWireV1,
    contribution: ZkAmsMkheRnsPolynomialWireV1,
    proof: ZkAmsMkheProofEnvelopeWireV1,
}

impl ZkAmsMkheCksContributionWireV1 {
    /// Construct a CKS contribution whose proof statement binds every record field.
    pub fn new(
        roster: &ZkAmsMkheGovernedRosterWireV1,
        binding: ZkAmsMkheWireBindingV1,
        source_ciphertext_digest: [u8; 32],
        authentication: ZkAmsMkheAuthenticationWireV1,
        contribution: ZkAmsMkheRnsPolynomialWireV1,
        proof: ZkAmsMkheProofEnvelopeWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            binding,
            source_ciphertext_digest,
            authentication,
            contribution,
            proof,
        };
        value.validate(release_dimensions()?)?;
        if expected_roster_party(roster, binding)? != value.authentication.party {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(value)
    }

    /// Encode one exact CKS contribution.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let dimensions = release_dimensions()?;
        self.validate(dimensions)?;
        let proof_len = proof_envelope_wire_bytes(self.proof.proof_bytes.len())?;
        let length = contribution_wire_bytes(dimensions, proof_len, dimensions.max_round_bytes)?;
        let mut encoder = WireEncoder::new(length)?;
        write_binding(&mut encoder, CKS_CONTRIBUTION_TAG_V1, self.binding);
        encoder.bytes(&self.source_ciphertext_digest);
        write_authentication(&mut encoder, &self.authentication);
        write_polynomial(&mut encoder, &self.contribution)?;
        encoder.u32(as_u32(proof_len)?);
        self.proof.write_to(&mut encoder)?;
        encoder.finish_exact(length)
    }

    /// Decode under trusted binding, source ciphertext, and roster party.
    pub fn decode_exact(
        bytes: &[u8],
        expected_roster: &ZkAmsMkheGovernedRosterWireV1,
        expected_binding: ZkAmsMkheWireBindingV1,
        expected_source_ciphertext_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let expected_party = expected_roster_party(expected_roster, expected_binding)?;
        decode_contribution(
            bytes,
            CKS_CONTRIBUTION_TAG_V1,
            expected_binding,
            expected_source_ciphertext_digest,
            expected_party,
            ZkAmsMkheProofKindV1::CksContribution,
        )
        .and_then(|decoded| {
            Self::new(
                expected_roster,
                decoded.binding,
                decoded.subject_digest,
                decoded.authentication,
                decoded.polynomial,
                decoded.proof,
            )
        })
    }

    /// Exact contribution binding.
    #[must_use]
    pub const fn binding(&self) -> ZkAmsMkheWireBindingV1 {
        self.binding
    }

    /// Digest of the exact independent-owner input ciphertext.
    #[must_use]
    pub const fn source_ciphertext_digest(&self) -> [u8; 32] {
        self.source_ciphertext_digest
    }

    /// Authenticated roster party.
    #[must_use]
    pub const fn authentication(&self) -> &ZkAmsMkheAuthenticationWireV1 {
        &self.authentication
    }

    /// CKS contribution polynomial.
    #[must_use]
    pub const fn contribution(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.contribution
    }

    /// Proof envelope bound to this contribution statement.
    #[must_use]
    pub const fn proof(&self) -> &ZkAmsMkheProofEnvelopeWireV1 {
        &self.proof
    }

    fn validate(&self, dimensions: WireDimensions<'_>) -> Result<(), ZkAmsMkheErrorV1> {
        validate_contribution_parts(
            self.binding,
            self.source_ciphertext_digest,
            &self.authentication,
            &self.contribution,
            &self.proof,
            ZkAmsMkheProofKindV1::CksContribution,
            dimensions,
            dimensions.max_round_bytes,
            CKS_STATEMENT_DOMAIN_V1,
        )
    }
}

const CKS_STATEMENT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-wire-statement";

fn expected_roster_party(
    roster: &ZkAmsMkheGovernedRosterWireV1,
    binding: ZkAmsMkheWireBindingV1,
) -> Result<ZkAmsMkhePartyIdV1, ZkAmsMkheErrorV1> {
    if binding.profile_digest != roster.profile_digest
        || binding.roster_digest != roster.roster_digest
        || binding.epoch != roster.epoch
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    roster
        .parties
        .get(
            usize::try_from(binding.record_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        )
        .copied()
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
}

/// Derive the exact statement digest required by a CKS proof envelope.
pub fn zk_ams_mkhe_cks_statement_digest_v1(
    binding: ZkAmsMkheWireBindingV1,
    source_ciphertext_digest: [u8; 32],
    party: ZkAmsMkhePartyIdV1,
    contribution: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    contribution_statement_digest(
        CKS_STATEMENT_DOMAIN_V1,
        binding,
        source_ciphertext_digest,
        party,
        contribution,
        release_dimensions()?,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "validation keeps every independently authenticated wire component explicit"
)]
fn validate_contribution_parts(
    binding: ZkAmsMkheWireBindingV1,
    subject_digest: [u8; 32],
    authentication: &ZkAmsMkheAuthenticationWireV1,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
    proof: &ZkAmsMkheProofEnvelopeWireV1,
    expected_kind: ZkAmsMkheProofKindV1,
    dimensions: WireDimensions<'_>,
    ceiling: usize,
    statement_domain: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    binding.validate(dimensions)?;
    if subject_digest == [0; 32]
        || usize::try_from(binding.record_index)
            .map_or(true, |index| index >= dimensions.roster_size)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    authentication.validate()?;
    polynomial.validate(dimensions)?;
    proof.validate(dimensions)?;
    if proof.binding != binding || proof.kind != expected_kind {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let expected_statement = contribution_statement_digest(
        statement_domain,
        binding,
        subject_digest,
        authentication.party,
        polynomial,
        dimensions,
    )?;
    if proof.statement_digest != expected_statement {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let proof_len = proof_envelope_wire_bytes(proof.proof_bytes.len())?;
    contribution_wire_bytes(dimensions, proof_len, ceiling)?;
    Ok(())
}

fn contribution_statement_digest(
    domain: &[u8],
    binding: ZkAmsMkheWireBindingV1,
    subject_digest: [u8; 32],
    party: ZkAmsMkhePartyIdV1,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
    dimensions: WireDimensions<'_>,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    binding.validate(dimensions)?;
    if subject_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    polynomial.validate(dimensions)?;
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(domain);
    append_binding_frame(&mut frame, binding);
    frame.extend_from_slice(&subject_digest);
    frame.extend_from_slice(&party.to_bytes());
    frame.extend_from_slice(&polynomial_digest(polynomial)?);
    Ok(keccak256(&frame))
}

fn polynomial_digest(
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-polynomial");
    hash.update(&as_u32(polynomial.residues.len())?.to_be_bytes());
    for residue in polynomial.residues.iter() {
        hash.update(&residue.to_be_bytes());
    }
    Ok(hash.finalize())
}

fn append_binding_frame(frame: &mut Vec<u8>, binding: ZkAmsMkheWireBindingV1) {
    frame.extend_from_slice(&binding.profile_digest);
    frame.extend_from_slice(&binding.roster_digest);
    frame.extend_from_slice(&binding.epoch.to_be_bytes());
    frame.extend_from_slice(&binding.transcript_digest);
    frame.extend_from_slice(&binding.record_index.to_be_bytes());
    frame.push(binding.level);
}

struct DecodedContribution {
    binding: ZkAmsMkheWireBindingV1,
    subject_digest: [u8; 32],
    authentication: ZkAmsMkheAuthenticationWireV1,
    polynomial: ZkAmsMkheRnsPolynomialWireV1,
    proof: ZkAmsMkheProofEnvelopeWireV1,
}

fn decode_contribution(
    bytes: &[u8],
    tag: [u8; 4],
    expected_binding: ZkAmsMkheWireBindingV1,
    expected_subject_digest: [u8; 32],
    expected_party: ZkAmsMkhePartyIdV1,
    expected_kind: ZkAmsMkheProofKindV1,
) -> Result<DecodedContribution, ZkAmsMkheErrorV1> {
    let dimensions = release_dimensions()?;
    preflight_contribution(
        bytes,
        tag,
        expected_binding,
        expected_subject_digest,
        expected_party,
        expected_kind,
        dimensions,
        dimensions.max_round_bytes,
    )?;
    let mut decoder = WireDecoder::new(bytes);
    read_binding(&mut decoder, tag, expected_binding, dimensions)?;
    let subject_digest = decoder.array()?;
    let authentication = read_authentication(&mut decoder)?;
    let polynomial = read_polynomial(&mut decoder, dimensions)?;
    let proof_len =
        usize::try_from(decoder.u32()?).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let proof_bytes = decoder.take(proof_len)?;
    let proof = decode_proof_envelope(proof_bytes, expected_binding, expected_kind, dimensions)?;
    decoder.finish()?;
    if subject_digest != expected_subject_digest || authentication.party != expected_party {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(DecodedContribution {
        binding: expected_binding,
        subject_digest,
        authentication,
        polynomial,
        proof,
    })
}

fn preflight_ciphertext(
    bytes: &[u8],
    expected_binding: ZkAmsMkheWireBindingV1,
    expected_sample_index: u64,
    dimensions: WireDimensions<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected_len = collective_ciphertext_wire_bytes(dimensions)?;
    if bytes.len() != expected_len || bytes.len() > dimensions.max_ciphertext_bytes {
        return Err(if bytes.len() > dimensions.max_ciphertext_bytes {
            ZkAmsMkheErrorV1::WireTooLarge
        } else {
            ZkAmsMkheErrorV1::InvalidWireEncoding
        });
    }
    let mut decoder = WireDecoder::new(bytes);
    read_binding(
        &mut decoder,
        CIPHERTEXT_TAG_V1,
        expected_binding,
        dimensions,
    )?;
    decoder.expect_u64(expected_sample_index)?;
    if expected_sample_index >= dimensions.max_samples {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    skip_polynomial(&mut decoder, dimensions)?;
    skip_polynomial(&mut decoder, dimensions)?;
    decoder.finish()
}

#[cfg(test)]
fn preflight_seeded_rkg_key(
    bytes: &[u8],
    expected_binding: ZkAmsMkheWireBindingV1,
    dimensions: WireDimensions<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected_len = seeded_rkg_key_wire_bytes(dimensions)?;
    if bytes.len() != expected_len || bytes.len() > dimensions.max_evaluated_key_bytes {
        return Err(if bytes.len() > dimensions.max_evaluated_key_bytes {
            ZkAmsMkheErrorV1::WireTooLarge
        } else {
            ZkAmsMkheErrorV1::InvalidWireEncoding
        });
    }
    let mut decoder = WireDecoder::new(bytes);
    read_binding(
        &mut decoder,
        SEEDED_RKG_KEY_TAG_V1,
        expected_binding,
        dimensions,
    )?;
    if decoder.array::<32>()? == [0; 32] || decoder.array::<32>()? == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    decoder.expect_u8(as_u8(dimensions.gadget_digits)?)?;
    for index in 0..dimensions.gadget_digits {
        decoder.expect_u8(as_u8(index)?)?;
        skip_polynomial(&mut decoder, dimensions)?;
    }
    decoder.finish()
}

#[allow(clippy::too_many_arguments)]
fn preflight_contribution(
    bytes: &[u8],
    tag: [u8; 4],
    expected_binding: ZkAmsMkheWireBindingV1,
    expected_subject_digest: [u8; 32],
    expected_party: ZkAmsMkhePartyIdV1,
    expected_kind: ZkAmsMkheProofKindV1,
    dimensions: WireDimensions<'_>,
    ceiling: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() > ceiling {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let mut decoder = WireDecoder::new(bytes);
    read_binding(&mut decoder, tag, expected_binding, dimensions)?;
    if decoder.array::<32>()? != expected_subject_digest || expected_subject_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let authentication = read_authentication(&mut decoder)?;
    if authentication.party != expected_party {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    skip_polynomial(&mut decoder, dimensions)?;
    let proof_len =
        usize::try_from(decoder.u32()?).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if proof_len > max_proof_envelope_bytes(dimensions)? || proof_len > decoder.remaining() {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let proof_bytes = decoder.take(proof_len)?;
    preflight_proof_envelope(proof_bytes, expected_binding, expected_kind, dimensions)?;
    decoder.finish()?;
    contribution_wire_bytes(dimensions, proof_len, ceiling).and_then(|expected| {
        if expected == bytes.len() {
            Ok(())
        } else {
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        }
    })
}

fn preflight_proof_envelope(
    bytes: &[u8],
    expected_binding: ZkAmsMkheWireBindingV1,
    expected_kind: ZkAmsMkheProofKindV1,
    dimensions: WireDimensions<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() > proof_envelope_wire_bytes(ZK_AMS_MKHE_MAX_PROOF_BYTES_V1)? {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let mut decoder = WireDecoder::new(bytes);
    read_binding(
        &mut decoder,
        PROOF_ENVELOPE_TAG_V1,
        expected_binding,
        dimensions,
    )?;
    if ZkAmsMkheProofKindV1::try_from(decoder.u8()?)? != expected_kind
        || decoder.array::<32>()? == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let proof_len =
        usize::try_from(decoder.u32()?).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if proof_len == 0 || proof_len > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let expected_len = proof_envelope_wire_bytes(proof_len)?;
    if expected_len != bytes.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    decoder.skip(proof_len)?;
    decoder.finish()
}

fn decode_proof_envelope(
    bytes: &[u8],
    expected_binding: ZkAmsMkheWireBindingV1,
    expected_kind: ZkAmsMkheProofKindV1,
    dimensions: WireDimensions<'_>,
) -> Result<ZkAmsMkheProofEnvelopeWireV1, ZkAmsMkheErrorV1> {
    preflight_proof_envelope(bytes, expected_binding, expected_kind, dimensions)?;
    let mut decoder = WireDecoder::new(bytes);
    read_binding(
        &mut decoder,
        PROOF_ENVELOPE_TAG_V1,
        expected_binding,
        dimensions,
    )?;
    let kind = ZkAmsMkheProofKindV1::try_from(decoder.u8()?)?;
    let statement_digest = decoder.array()?;
    let proof_len = decoder.u32()? as usize;
    let encoded_proof = decoder.take(proof_len)?;
    let mut proof_bytes = Vec::new();
    proof_bytes
        .try_reserve_exact(proof_len)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    proof_bytes.extend_from_slice(encoded_proof);
    decoder.finish()?;
    let proof = ZkAmsMkheProofEnvelopeWireV1 {
        binding: expected_binding,
        kind,
        statement_digest,
        proof_bytes,
    };
    proof.validate(dimensions)?;
    Ok(proof)
}

fn write_binding(encoder: &mut WireEncoder, tag: [u8; 4], binding: ZkAmsMkheWireBindingV1) {
    encoder.bytes(&tag);
    encoder.u8(MKHE_VERSION_V1);
    encoder.bytes(&binding.profile_digest);
    encoder.bytes(&binding.roster_digest);
    encoder.u64(binding.epoch);
    encoder.bytes(&binding.transcript_digest);
    encoder.u32(binding.record_index);
    encoder.u8(binding.level);
}

fn read_binding(
    decoder: &mut WireDecoder<'_>,
    tag: [u8; 4],
    expected: ZkAmsMkheWireBindingV1,
    dimensions: WireDimensions<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    expected.validate(dimensions)?;
    decoder.expect_bytes(&tag)?;
    decoder.expect_u8(MKHE_VERSION_V1)?;
    if decoder.array::<32>()? != expected.profile_digest
        || decoder.array::<32>()? != expected.roster_digest
        || decoder.u64()? != expected.epoch
        || decoder.array::<32>()? != expected.transcript_digest
        || decoder.u32()? != expected.record_index
        || decoder.u8()? != expected.level
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn write_authentication(encoder: &mut WireEncoder, authentication: &ZkAmsMkheAuthenticationWireV1) {
    encoder.u8(MKHE_VERSION_V1);
    encoder.bytes(&authentication.party.to_bytes());
    encoder.bytes(&authentication.public_key);
    encoder.bytes(&authentication.signature);
}

fn read_authentication(
    decoder: &mut WireDecoder<'_>,
) -> Result<ZkAmsMkheAuthenticationWireV1, ZkAmsMkheErrorV1> {
    decoder.expect_u8(MKHE_VERSION_V1)?;
    let party = ZkAmsMkhePartyIdV1::new(decoder.array()?)?;
    let public_key = decoder.array()?;
    let signature = decoder.array()?;
    ZkAmsMkheAuthenticationWireV1::new(party, public_key, signature)
}

fn write_polynomial(
    encoder: &mut WireEncoder,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    encoder.u32(as_u32(polynomial.residues.len())?);
    for residue in polynomial.residues.iter() {
        encoder.u64(*residue);
    }
    Ok(())
}

fn skip_polynomial(
    decoder: &mut WireDecoder<'_>,
    dimensions: WireDimensions<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    decoder.expect_u32(as_u32(dimensions.coefficient_count()?)?)?;
    decoder.skip(
        dimensions
            .coefficient_count()?
            .checked_mul(8)
            .ok_or(ZkAmsMkheErrorV1::WireTooLarge)?,
    )
}

fn read_polynomial(
    decoder: &mut WireDecoder<'_>,
    dimensions: WireDimensions<'_>,
) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
    let count = dimensions.coefficient_count()?;
    decoder.expect_u32(as_u32(count)?)?;
    let mut residues = Vec::new();
    residues
        .try_reserve_exact(count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for index in 0..count {
        let residue = decoder.u64()?;
        let limb = index / dimensions.ring_degree;
        if residue >= dimensions.moduli[limb] {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        residues.push(residue);
    }
    ZkAmsMkheRnsPolynomialWireV1::new_with_dimensions(residues, dimensions)
}

fn collective_ciphertext_wire_bytes(
    dimensions: WireDimensions<'_>,
) -> Result<usize, ZkAmsMkheErrorV1> {
    CIPHERTEXT_HEADER_WIRE_BYTES
        .checked_add(
            dimensions
                .polynomial_wire_bytes()?
                .checked_mul(2)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

#[cfg(test)]
fn seeded_rkg_key_wire_bytes(dimensions: WireDimensions<'_>) -> Result<usize, ZkAmsMkheErrorV1> {
    dimensions
        .polynomial_wire_bytes()?
        .checked_add(1)
        .and_then(|digit| digit.checked_mul(dimensions.gadget_digits))
        .and_then(|digits| digits.checked_add(SEEDED_RKG_KEY_HEADER_WIRE_BYTES))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn contribution_wire_bytes(
    dimensions: WireDimensions<'_>,
    proof_len: usize,
    ceiling: usize,
) -> Result<usize, ZkAmsMkheErrorV1> {
    let length = CONTRIBUTION_HEADER_WIRE_BYTES
        .checked_add(dimensions.polynomial_wire_bytes()?)
        .and_then(|base| base.checked_add(proof_len))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if length > ceiling {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    Ok(length)
}

fn max_proof_envelope_bytes(dimensions: WireDimensions<'_>) -> Result<usize, ZkAmsMkheErrorV1> {
    let base = CONTRIBUTION_HEADER_WIRE_BYTES
        .checked_add(dimensions.polynomial_wire_bytes()?)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    dimensions
        .max_round_bytes
        .checked_sub(base)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn proof_envelope_wire_bytes(proof_len: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    PROOF_ENVELOPE_HEADER_WIRE_BYTES
        .checked_add(proof_len)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

#[cfg(test)]
fn as_u8(value: usize) -> Result<u8, ZkAmsMkheErrorV1> {
    u8::try_from(value).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}

fn as_u32(value: usize) -> Result<u32, ZkAmsMkheErrorV1> {
    u32::try_from(value).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}

struct WireEncoder {
    output: Vec<u8>,
}

impl WireEncoder {
    fn new(capacity: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self { output })
    }

    fn u8(&mut self, value: u8) {
        self.output.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.output.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.output.extend_from_slice(&value.to_be_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.output.extend_from_slice(value);
    }

    fn finish_exact(self, expected: usize) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        if self.output.len() != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(self.output)
    }
}

struct WireDecoder<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> WireDecoder<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, cursor: 0 }
    }

    fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.cursor)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ZkAmsMkheErrorV1> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let bytes = self
            .input
            .get(self.cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        self.cursor = end;
        Ok(bytes)
    }

    fn skip(&mut self, length: usize) -> Result<(), ZkAmsMkheErrorV1> {
        self.take(length).map(|_| ())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ZkAmsMkheErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
    }

    fn u8(&mut self) -> Result<u8, ZkAmsMkheErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, ZkAmsMkheErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ZkAmsMkheErrorV1> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn expect_bytes(&mut self, expected: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn expect_u8(&mut self, expected: u8) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u8()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn expect_u32(&mut self, expected: u32) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u32()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn expect_u64(&mut self, expected: u64) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u64()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn finish(self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.cursor != self.input.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
}

// Compile-time guards keep the certificate commentary and actual headers tied.
const _: [(); 114] = [(); COMMON_BINDING_WIRE_BYTES];
const _: [(); 302] = [(); ROSTER_WIRE_BYTES];
const _: [(); 122] = [(); CIPHERTEXT_HEADER_WIRE_BYTES];
const _: [(); 179] = [(); SEEDED_RKG_KEY_HEADER_WIRE_BYTES];
const _: [(); 281] = [(); CONTRIBUTION_HEADER_WIRE_BYTES];
const _: [(); 151] = [(); PROOF_ENVELOPE_HEADER_WIRE_BYTES];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, derive_t256_generators_v1};

    const TEST_MODULI: [u64; 2] = [17, 97];

    fn dimensions() -> WireDimensions<'static> {
        WireDimensions {
            ring_degree: 4,
            moduli: &TEST_MODULI,
            gadget_digits: 2,
            roster_size: 8,
            max_samples: 32,
            max_ciphertext_bytes: 4_096,
            max_evaluated_key_bytes: 4_096,
            max_round_bytes: 8_192,
        }
    }

    fn roster() -> ZkAmsMkheGovernedRosterWireV1 {
        let profile_digest = release_profile_v1().digest().unwrap();
        let parties = core::array::from_fn(|index| {
            let mut bytes = [0_u8; 32];
            bytes[31] = u8::try_from(index + 1).unwrap();
            ZkAmsMkhePartyIdV1::new(bytes).unwrap()
        });
        ZkAmsMkheGovernedRosterWireV1::new(profile_digest, 7, parties).unwrap()
    }

    fn binding(index: u32, level: u8) -> ZkAmsMkheWireBindingV1 {
        ZkAmsMkheWireBindingV1::new(&roster(), [0x55; 32], index, level).unwrap()
    }

    fn toy_polynomial() -> ZkAmsMkheRnsPolynomialWireV1 {
        ZkAmsMkheRnsPolynomialWireV1::new_with_dimensions(
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            dimensions(),
        )
        .unwrap()
    }

    #[test]
    fn polynomial_wire_consumes_vec_and_clones_one_shared_backing() {
        let residues = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let original_allocation = residues.as_ptr();
        let polynomial =
            ZkAmsMkheRnsPolynomialWireV1::new_with_dimensions(residues, dimensions()).unwrap();
        assert_eq!(polynomial.residues().as_ptr(), original_allocation);

        let cloned = polynomial.clone();
        assert!(Arc::ptr_eq(&polynomial.residues, &cloned.residues));
        assert_eq!(cloned.residues().as_ptr(), original_allocation);

        let ciphertext = ZkAmsMkheCollectiveCiphertextWireV1 {
            binding: binding(0, 0),
            sample_index: 0,
            constant: polynomial.clone(),
            linear: cloned,
        };
        let rendered = format!("{ciphertext:?}");
        assert!(rendered.len() < 256);
        assert!(!rendered.contains("[1, 2, 3"));
        assert!(rendered.contains("residue_count: 8"));
    }

    #[test]
    fn polynomial_wire_source_forbids_full_residue_debug() {
        let source = include_str!("wire.rs");
        let declaration = source
            .split("/// Canonical limb-major RNS residue vector")
            .nth(1)
            .expect("polynomial declaration")
            .split("impl ZkAmsMkheRnsPolynomialWireV1")
            .next()
            .expect("polynomial implementation boundary");
        assert!(!declaration.contains("derive(Clone, Debug"));
        assert!(declaration.contains("impl core::fmt::Debug"));
        assert!(declaration.contains("residue_count"));
        assert!(!declaration.contains(".field(\"residues\""));
    }

    fn proof_fields() -> ([u8; 33], [u8; 32]) {
        let point = derive_t256_generators_v1(b"zk-ams-wire-test", 1)
            .unwrap()
            .remove(0)
            .to_non_identity_wire_bytes()
            .unwrap();
        (point, Scalar::from_u64(9).to_be_bytes())
    }

    fn authentication() -> ZkAmsMkheAuthenticationWireV1 {
        let (public_key, response) = proof_fields();
        let commitment = derive_t256_generators_v1(b"zk-ams-wire-auth-test", 1)
            .unwrap()
            .remove(0)
            .to_non_identity_wire_bytes()
            .unwrap();
        let mut signature = [0_u8; 65];
        signature[..33].copy_from_slice(&commitment);
        signature[33..].copy_from_slice(&response);
        let party = ZkAmsMkhePartyIdV1::from_authentication_key(&public_key).unwrap();
        ZkAmsMkheAuthenticationWireV1::new(party, public_key, signature).unwrap()
    }

    fn proof_with_dimensions(
        binding: ZkAmsMkheWireBindingV1,
        kind: ZkAmsMkheProofKindV1,
        statement_digest: [u8; 32],
    ) -> ZkAmsMkheProofEnvelopeWireV1 {
        let proof = ZkAmsMkheProofEnvelopeWireV1 {
            binding,
            kind,
            statement_digest,
            proof_bytes: vec![1, 2, 3],
        };
        proof.validate(dimensions()).unwrap();
        proof
    }

    #[test]
    fn proof_envelope_redacts_and_zeroizes_success_error_and_unwind() {
        let binding = binding(1, 0);
        let before_success = proof_envelope_zeroized_drop_count_v1();
        let proof =
            proof_with_dimensions(binding, ZkAmsMkheProofKindV1::RkgContribution, [0x31; 32]);
        let debug = format!("{proof:?}");
        assert!(debug.contains("proof_bytes_len: 3"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("[1, 2, 3]"));
        drop(proof);
        assert_eq!(proof_envelope_zeroized_drop_count_v1(), before_success + 1);

        let before_error = proof_envelope_zeroized_drop_count_v1();
        let result = (|| -> Result<(), ZkAmsMkheErrorV1> {
            let _proof =
                proof_with_dimensions(binding, ZkAmsMkheProofKindV1::CksContribution, [0x32; 32]);
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        })();
        assert_eq!(result, Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
        assert_eq!(proof_envelope_zeroized_drop_count_v1(), before_error + 1);

        let before_unwind = proof_envelope_zeroized_drop_count_v1();
        let unwind = std::panic::catch_unwind(|| {
            let _proof =
                proof_with_dimensions(binding, ZkAmsMkheProofKindV1::CksContribution, [0x33; 32]);
            panic!("injected proof-envelope unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(proof_envelope_zeroized_drop_count_v1(), before_unwind + 1);

        let source = include_str!("wire.rs");
        let declaration = source
            .split("/// Canonical kind-tagged envelope")
            .nth(1)
            .expect("proof-envelope declaration")
            .split("impl ZkAmsMkheProofEnvelopeWireV1")
            .next()
            .expect("proof-envelope implementation boundary");
        assert!(!declaration.contains("derive(Clone, Debug"));
        assert!(declaration.contains("impl core::fmt::Debug"));
        assert!(declaration.contains("impl Drop"));
        assert!(declaration.contains("proof_bytes_len"));
        assert!(declaration.contains("<redacted>"));
        assert!(source.contains(concat!(
            "#[cfg_attr(test, derive(Clone))]\n",
            "#[derive(Debug, PartialEq, Eq)]\n",
            "pub struct ZkAmsMkheCksContributionWireV1"
        )));
    }

    fn encode_proof_for_dimensions(
        proof: &ZkAmsMkheProofEnvelopeWireV1,
    ) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let length = proof_envelope_wire_bytes(proof.proof_bytes.len())?;
        let mut encoder = WireEncoder::new(length)?;
        proof.write_to(&mut encoder)?;
        encoder.finish_exact(length)
    }

    fn encode_ciphertext_for_dimensions(
        binding: ZkAmsMkheWireBindingV1,
        sample_index: u64,
    ) -> Vec<u8> {
        let dimensions = dimensions();
        let polynomial = toy_polynomial();
        let length = collective_ciphertext_wire_bytes(dimensions).unwrap();
        let mut encoder = WireEncoder::new(length).unwrap();
        write_binding(&mut encoder, CIPHERTEXT_TAG_V1, binding);
        encoder.u64(sample_index);
        write_polynomial(&mut encoder, &polynomial).unwrap();
        write_polynomial(&mut encoder, &polynomial).unwrap();
        encoder.finish_exact(length).unwrap()
    }

    fn encode_seeded_rkg_for_dimensions(binding: ZkAmsMkheWireBindingV1) -> Vec<u8> {
        let dimensions = dimensions();
        let polynomial = toy_polynomial();
        let length = seeded_rkg_key_wire_bytes(dimensions).unwrap();
        let mut encoder = WireEncoder::new(length).unwrap();
        write_binding(&mut encoder, SEEDED_RKG_KEY_TAG_V1, binding);
        encoder.bytes(&[1; 32]);
        encoder.bytes(&[2; 32]);
        encoder.u8(as_u8(dimensions.gadget_digits).unwrap());
        for index in 0..dimensions.gadget_digits {
            encoder.u8(as_u8(index).unwrap());
            write_polynomial(&mut encoder, &polynomial).unwrap();
        }
        encoder.finish_exact(length).unwrap()
    }

    fn encode_contribution_for_dimensions(
        tag: [u8; 4],
        binding: ZkAmsMkheWireBindingV1,
        subject_digest: [u8; 32],
        proof_kind: ZkAmsMkheProofKindV1,
        statement_domain: &[u8],
    ) -> (Vec<u8>, ZkAmsMkhePartyIdV1) {
        let dimensions = dimensions();
        let polynomial = toy_polynomial();
        let authentication = authentication();
        let statement_digest = contribution_statement_digest(
            statement_domain,
            binding,
            subject_digest,
            authentication.party,
            &polynomial,
            dimensions,
        )
        .unwrap();
        let proof = proof_with_dimensions(binding, proof_kind, statement_digest);
        let proof_len = proof_envelope_wire_bytes(proof.proof_bytes.len()).unwrap();
        let length =
            contribution_wire_bytes(dimensions, proof_len, dimensions.max_round_bytes).unwrap();
        let mut encoder = WireEncoder::new(length).unwrap();
        write_binding(&mut encoder, tag, binding);
        encoder.bytes(&subject_digest);
        write_authentication(&mut encoder, &authentication);
        write_polynomial(&mut encoder, &polynomial).unwrap();
        encoder.u32(as_u32(proof_len).unwrap());
        proof.write_to(&mut encoder).unwrap();
        (encoder.finish_exact(length).unwrap(), authentication.party)
    }

    #[test]
    fn release_length_certificate_is_exact_and_matches_resource_formulas() {
        let profile = release_profile_v1();
        let lengths = derive_wire_length_certificate_v1(&profile).unwrap();
        assert_eq!(lengths.governed_roster_wire_bytes, 302);
        assert_eq!(lengths.rns_polynomial_wire_bytes, 39_845_892);
        assert_eq!(lengths.compact_collective_ciphertext_wire_bytes, 79_691_906);
        assert_eq!(lengths.multiplication_triple_wire_bytes, 119_537_798);
        assert_eq!(
            lengths.seeded_collective_relinearization_key_wire_bytes,
            1_514_144_113
        );
        assert_eq!(lengths.streamed_contribution_base_wire_bytes, 39_846_173);
        assert_eq!(lengths.proof_envelope_header_wire_bytes, 151);
    }

    #[test]
    fn roster_wire_rejects_reordering_duplicates_cross_epoch_and_trailing_bytes() {
        let roster = roster();
        let encoded = roster.encode().unwrap();
        assert_eq!(
            ZkAmsMkheGovernedRosterWireV1::decode_exact(
                &encoded,
                roster.profile_digest,
                roster.epoch,
            )
            .unwrap(),
            roster
        );
        let parties_offset = 4 + 1 + 32 + 8 + 1;
        let mut reordered = encoded.clone();
        let first = reordered[parties_offset..parties_offset + 32].to_vec();
        let second = reordered[parties_offset + 32..parties_offset + 64].to_vec();
        reordered[parties_offset..parties_offset + 32].copy_from_slice(&second);
        reordered[parties_offset + 32..parties_offset + 64].copy_from_slice(&first);
        assert!(
            ZkAmsMkheGovernedRosterWireV1::decode_exact(
                &reordered,
                roster.profile_digest,
                roster.epoch,
            )
            .is_err()
        );
        let mut duplicate = encoded.clone();
        let first = duplicate[parties_offset..parties_offset + 32].to_vec();
        duplicate[parties_offset + 32..parties_offset + 64].copy_from_slice(&first);
        assert!(
            ZkAmsMkheGovernedRosterWireV1::decode_exact(
                &duplicate,
                roster.profile_digest,
                roster.epoch,
            )
            .is_err()
        );
        assert!(
            ZkAmsMkheGovernedRosterWireV1::decode_exact(
                &encoded,
                roster.profile_digest,
                roster.epoch + 1,
            )
            .is_err()
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert!(
            ZkAmsMkheGovernedRosterWireV1::decode_exact(
                &trailing,
                roster.profile_digest,
                roster.epoch,
            )
            .is_err()
        );
    }

    #[test]
    fn ciphertext_preflight_rejects_every_binding_substitution_and_bad_shape() {
        let binding = binding(3, 1);
        let canonical = encode_ciphertext_for_dimensions(binding, 5);
        preflight_ciphertext(&canonical, binding, 5, dimensions()).unwrap();
        for changed in [
            ZkAmsMkheWireBindingV1 {
                profile_digest: [9; 32],
                ..binding
            },
            ZkAmsMkheWireBindingV1 {
                roster_digest: [9; 32],
                ..binding
            },
            ZkAmsMkheWireBindingV1 {
                epoch: binding.epoch + 1,
                ..binding
            },
            ZkAmsMkheWireBindingV1 {
                transcript_digest: [9; 32],
                ..binding
            },
            ZkAmsMkheWireBindingV1 {
                record_index: binding.record_index + 1,
                ..binding
            },
            ZkAmsMkheWireBindingV1 {
                level: 0,
                ..binding
            },
        ] {
            assert!(preflight_ciphertext(&canonical, changed, 5, dimensions()).is_err());
        }
        assert!(preflight_ciphertext(&canonical, binding, 6, dimensions()).is_err());
        for end in 0..canonical.len() {
            assert!(preflight_ciphertext(&canonical[..end], binding, 5, dimensions()).is_err());
        }
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(preflight_ciphertext(&trailing, binding, 5, dimensions()).is_err());
        let mut wrong_count = canonical;
        let count_offset = CIPHERTEXT_HEADER_WIRE_BYTES;
        wrong_count[count_offset..count_offset + 4].copy_from_slice(&7_u32.to_be_bytes());
        assert!(preflight_ciphertext(&wrong_count, binding, 5, dimensions()).is_err());
    }

    #[test]
    fn polynomial_decode_rejects_noncanonical_residue_before_returning_value() {
        let dimensions = dimensions();
        let polynomial = toy_polynomial();
        let mut encoder = WireEncoder::new(dimensions.polynomial_wire_bytes().unwrap()).unwrap();
        write_polynomial(&mut encoder, &polynomial).unwrap();
        let canonical = encoder
            .finish_exact(dimensions.polynomial_wire_bytes().unwrap())
            .unwrap();
        let mut decoder = WireDecoder::new(&canonical);
        assert_eq!(
            read_polynomial(&mut decoder, dimensions).unwrap(),
            polynomial
        );
        let mut noncanonical = canonical;
        noncanonical[4..12].copy_from_slice(&TEST_MODULI[0].to_be_bytes());
        let mut decoder = WireDecoder::new(&noncanonical);
        assert_eq!(
            read_polynomial(&mut decoder, dimensions),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
    }

    #[test]
    fn proof_preflight_rejects_lengths_zero_statements_and_cross_domain_splicing() {
        let binding = binding(2, 0);
        let proof =
            proof_with_dimensions(binding, ZkAmsMkheProofKindV1::CksContribution, [0x44; 32]);
        let canonical = encode_proof_for_dimensions(&proof).unwrap();
        preflight_proof_envelope(
            &canonical,
            binding,
            ZkAmsMkheProofKindV1::CksContribution,
            dimensions(),
        )
        .unwrap();
        assert!(
            preflight_proof_envelope(
                &canonical,
                binding,
                ZkAmsMkheProofKindV1::RkgContribution,
                dimensions(),
            )
            .is_err()
        );
        let length_offset = COMMON_BINDING_WIRE_BYTES + 1 + 32;
        for forged_length in [0_u32, u32::MAX] {
            let mut forged = canonical.clone();
            forged[length_offset..length_offset + 4].copy_from_slice(&forged_length.to_be_bytes());
            assert!(
                preflight_proof_envelope(
                    &forged,
                    binding,
                    ZkAmsMkheProofKindV1::CksContribution,
                    dimensions(),
                )
                .is_err()
            );
        }
        let mut zero_statement = canonical.clone();
        zero_statement[COMMON_BINDING_WIRE_BYTES + 1..COMMON_BINDING_WIRE_BYTES + 1 + 32].fill(0);
        assert!(
            preflight_proof_envelope(
                &zero_statement,
                binding,
                ZkAmsMkheProofKindV1::CksContribution,
                dimensions(),
            )
            .is_err()
        );
        let decoded = decode_proof_envelope(
            &canonical,
            binding,
            ZkAmsMkheProofKindV1::CksContribution,
            dimensions(),
        )
        .unwrap();
        assert_eq!(decoded.proof_bytes(), &[1, 2, 3]);
    }

    #[test]
    fn seeded_rkg_preflight_rejects_duplicate_reordered_and_wrong_digit_counts() {
        let dimensions = dimensions();
        let binding = binding(0, 1);
        let polynomial = toy_polynomial();
        let length = seeded_rkg_key_wire_bytes(dimensions).unwrap();
        let mut encoder = WireEncoder::new(length).unwrap();
        write_binding(&mut encoder, SEEDED_RKG_KEY_TAG_V1, binding);
        encoder.bytes(&[1; 32]);
        encoder.bytes(&[2; 32]);
        encoder.u8(2);
        for index in 0..2 {
            encoder.u8(index);
            write_polynomial(&mut encoder, &polynomial).unwrap();
        }
        let canonical = encoder.finish_exact(length).unwrap();
        preflight_seeded_rkg_key(&canonical, binding, dimensions).unwrap();
        let digit_count_offset = COMMON_BINDING_WIRE_BYTES + 64;
        let first_index_offset = digit_count_offset + 1;
        let second_index_offset =
            first_index_offset + 1 + dimensions.polynomial_wire_bytes().unwrap();
        let mut duplicate = canonical.clone();
        duplicate[second_index_offset] = 0;
        assert!(preflight_seeded_rkg_key(&duplicate, binding, dimensions).is_err());
        let mut reordered = canonical.clone();
        reordered[first_index_offset] = 1;
        reordered[second_index_offset] = 0;
        assert!(preflight_seeded_rkg_key(&reordered, binding, dimensions).is_err());
        let mut excessive = canonical;
        excessive[digit_count_offset] = 3;
        assert!(preflight_seeded_rkg_key(&excessive, binding, dimensions).is_err());
    }

    #[test]
    fn seeded_rkg_aggregate_owner_is_absent_from_production() {
        let source = include_str!("wire.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("production source prefix");
        assert!(!production.contains("pub struct ZkAmsMkheSeededRkgKeyWireV1"));
        assert!(!production.contains("impl ZkAmsMkheSeededRkgKeyWireV1"));
        assert!(production.contains("#[cfg(test)]\nfn preflight_seeded_rkg_key"));
        assert!(production.contains("#[cfg(test)]\nfn seeded_rkg_key_wire_bytes"));
    }

    #[test]
    fn every_artifact_tag_and_version_byte_is_exact_and_cross_type_rejected() {
        let roster = roster();
        let roster_bytes = roster.encode().unwrap();
        for offset in 0..5 {
            let mut forged = roster_bytes.clone();
            forged[offset] ^= 0x80;
            assert!(
                ZkAmsMkheGovernedRosterWireV1::decode_exact(
                    &forged,
                    roster.profile_digest,
                    roster.epoch,
                )
                .is_err(),
                "roster tag/version byte {offset}"
            );
        }

        let binding = binding(1, 0);
        let ciphertext = encode_ciphertext_for_dimensions(binding, 3);
        let seeded_key = encode_seeded_rkg_for_dimensions(binding);
        let proof = encode_proof_for_dimensions(&proof_with_dimensions(
            binding,
            ZkAmsMkheProofKindV1::RkgContribution,
            [0x31; 32],
        ))
        .unwrap();
        for (name, canonical, mut verify) in [
            (
                "ciphertext",
                ciphertext.clone(),
                Box::new(move |bytes: &[u8]| preflight_ciphertext(bytes, binding, 3, dimensions()))
                    as Box<dyn FnMut(&[u8]) -> Result<(), ZkAmsMkheErrorV1>>,
            ),
            (
                "seeded-rkg",
                seeded_key.clone(),
                Box::new(move |bytes: &[u8]| {
                    preflight_seeded_rkg_key(bytes, binding, dimensions())
                }),
            ),
            (
                "proof",
                proof.clone(),
                Box::new(move |bytes: &[u8]| {
                    preflight_proof_envelope(
                        bytes,
                        binding,
                        ZkAmsMkheProofKindV1::RkgContribution,
                        dimensions(),
                    )
                }),
            ),
        ] {
            verify(&canonical).unwrap();
            for offset in 0..5 {
                let mut forged = canonical.clone();
                forged[offset] ^= 0x80;
                assert!(verify(&forged).is_err(), "{name} tag/version byte {offset}");
            }
        }

        let (cks, cks_party) = encode_contribution_for_dimensions(
            CKS_CONTRIBUTION_TAG_V1,
            binding,
            [0x41; 32],
            ZkAmsMkheProofKindV1::CksContribution,
            CKS_STATEMENT_DOMAIN_V1,
        );
        for offset in 0..5 {
            let mut forged = cks.clone();
            forged[offset] ^= 0x80;
            assert!(
                preflight_contribution(
                    &forged,
                    CKS_CONTRIBUTION_TAG_V1,
                    binding,
                    [0x41; 32],
                    cks_party,
                    ZkAmsMkheProofKindV1::CksContribution,
                    dimensions(),
                    dimensions().max_round_bytes,
                )
                .is_err(),
                "cks tag/version byte {offset}"
            );
        }
    }

    #[test]
    fn zero_nonrelease_bindings_and_exact_sample_boundaries_fail_closed() {
        let roster = roster();
        assert_eq!(
            ZkAmsMkheGovernedRosterWireV1::new([0; 32], roster.epoch, roster.parties),
            Err(ZkAmsMkheErrorV1::InvalidPartySet)
        );
        assert_eq!(
            ZkAmsMkheGovernedRosterWireV1::new([9; 32], roster.epoch, roster.parties),
            Err(ZkAmsMkheErrorV1::InvalidPartySet)
        );
        assert_eq!(
            ZkAmsMkheGovernedRosterWireV1::new(roster.profile_digest, 0, roster.parties,),
            Err(ZkAmsMkheErrorV1::InvalidPartySet)
        );
        let canonical = binding(0, 0);
        for invalid in [
            ZkAmsMkheWireBindingV1 {
                profile_digest: [0; 32],
                ..canonical
            },
            ZkAmsMkheWireBindingV1 {
                roster_digest: [0; 32],
                ..canonical
            },
            ZkAmsMkheWireBindingV1 {
                epoch: 0,
                ..canonical
            },
            ZkAmsMkheWireBindingV1 {
                transcript_digest: [0; 32],
                ..canonical
            },
            ZkAmsMkheWireBindingV1 {
                record_index: 32,
                ..canonical
            },
            ZkAmsMkheWireBindingV1 {
                level: 2,
                ..canonical
            },
        ] {
            assert_eq!(
                invalid.validate(dimensions()),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
            );
        }
        let last_record = ZkAmsMkheWireBindingV1 {
            record_index: 31,
            ..canonical
        };
        last_record.validate(dimensions()).unwrap();
        let last_sample = encode_ciphertext_for_dimensions(last_record, 31);
        preflight_ciphertext(&last_sample, last_record, 31, dimensions()).unwrap();
        let excessive_sample = encode_ciphertext_for_dimensions(last_record, 32);
        assert!(preflight_ciphertext(&excessive_sample, last_record, 32, dimensions()).is_err());
    }

    #[test]
    fn zero_rkg_digests_and_contribution_subjects_are_rejected_preallocation() {
        let binding = binding(0, 1);
        let canonical = encode_seeded_rkg_for_dimensions(binding);
        for offset in [COMMON_BINDING_WIRE_BYTES, COMMON_BINDING_WIRE_BYTES + 32] {
            let mut forged = canonical.clone();
            forged[offset..offset + 32].fill(0);
            assert!(preflight_seeded_rkg_key(&forged, binding, dimensions()).is_err());
        }
        let (contribution, party) = encode_contribution_for_dimensions(
            CKS_CONTRIBUTION_TAG_V1,
            binding,
            [0x61; 32],
            ZkAmsMkheProofKindV1::CksContribution,
            CKS_STATEMENT_DOMAIN_V1,
        );
        let mut zero_subject = contribution;
        zero_subject[COMMON_BINDING_WIRE_BYTES..COMMON_BINDING_WIRE_BYTES + 32].fill(0);
        assert!(
            preflight_contribution(
                &zero_subject,
                CKS_CONTRIBUTION_TAG_V1,
                binding,
                [0; 32],
                party,
                ZkAmsMkheProofKindV1::CksContribution,
                dimensions(),
                dimensions().max_round_bytes,
            )
            .is_err()
        );
    }

    #[test]
    fn authentication_rejects_party_key_mismatch_and_noncanonical_response() {
        let authentication = authentication();
        let mut encoder = WireEncoder::new(AUTHENTICATION_WIRE_BYTES).unwrap();
        write_authentication(&mut encoder, &authentication);
        let canonical = encoder.finish_exact(AUTHENTICATION_WIRE_BYTES).unwrap();
        let mut decoder = WireDecoder::new(&canonical);
        assert_eq!(read_authentication(&mut decoder).unwrap(), authentication);

        let mut wrong_party = canonical.clone();
        wrong_party[1 + 31] ^= 1;
        let mut decoder = WireDecoder::new(&wrong_party);
        assert!(read_authentication(&mut decoder).is_err());

        let mut wrong_key = canonical.clone();
        let other_key = derive_t256_generators_v1(b"zk-ams-wire-other-auth-key", 1)
            .unwrap()
            .remove(0)
            .to_non_identity_wire_bytes()
            .unwrap();
        wrong_key[1 + 32..1 + 32 + 33].copy_from_slice(&other_key);
        let mut decoder = WireDecoder::new(&wrong_key);
        assert!(read_authentication(&mut decoder).is_err());

        let mut noncanonical_response = canonical;
        let response_offset = 1 + 32 + 33 + 33;
        noncanonical_response[response_offset..response_offset + 32]
            .copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
        let mut decoder = WireDecoder::new(&noncanonical_response);
        assert!(read_authentication(&mut decoder).is_err());
    }

    #[test]
    fn proof_payload_length_truncation_and_trailing_bytes_are_exact() {
        let binding = binding(3, 1);
        let proof =
            proof_with_dimensions(binding, ZkAmsMkheProofKindV1::RkgContribution, [0x71; 32]);
        let canonical = encode_proof_for_dimensions(&proof).unwrap();
        let proof_length_offset = COMMON_BINDING_WIRE_BYTES + 1 + 32;
        for length in [0_u32, 4, u32::MAX] {
            let mut forged = canonical.clone();
            forged[proof_length_offset..proof_length_offset + 4]
                .copy_from_slice(&length.to_be_bytes());
            assert!(
                preflight_proof_envelope(
                    &forged,
                    binding,
                    ZkAmsMkheProofKindV1::RkgContribution,
                    dimensions(),
                )
                .is_err()
            );
        }
        for end in 0..canonical.len() {
            assert!(
                preflight_proof_envelope(
                    &canonical[..end],
                    binding,
                    ZkAmsMkheProofKindV1::RkgContribution,
                    dimensions(),
                )
                .is_err()
            );
        }
        let mut trailing = canonical;
        trailing.push(0);
        assert!(
            preflight_proof_envelope(
                &trailing,
                binding,
                ZkAmsMkheProofKindV1::RkgContribution,
                dimensions(),
            )
            .is_err()
        );
    }

    #[test]
    fn standalone_proof_cap_is_independent_from_enclosing_round_ceiling() {
        let dimensions = dimensions();
        let binding = binding(3, 1);
        let embedded_ceiling = max_proof_envelope_bytes(dimensions).unwrap();
        let payload_len = embedded_ceiling
            .checked_sub(PROOF_ENVELOPE_HEADER_WIRE_BYTES)
            .and_then(|value| value.checked_add(1))
            .unwrap();
        let proof = ZkAmsMkheProofEnvelopeWireV1 {
            binding,
            kind: ZkAmsMkheProofKindV1::RkgContribution,
            statement_digest: [0x72; 32],
            proof_bytes: vec![0xa5; payload_len],
        };
        proof.validate(dimensions).unwrap();
        let encoded = encode_proof_for_dimensions(&proof).unwrap();
        assert_eq!(encoded.len(), embedded_ceiling + 1);
        preflight_proof_envelope(
            &encoded,
            binding,
            ZkAmsMkheProofKindV1::RkgContribution,
            dimensions,
        )
        .unwrap();
        assert_eq!(
            contribution_wire_bytes(dimensions, encoded.len(), dimensions.max_round_bytes),
            Err(ZkAmsMkheErrorV1::WireTooLarge)
        );
    }

    #[test]
    fn contribution_proof_lengths_and_spliced_records_fail_before_polynomial_allocation() {
        let binding = binding(4, 0);
        let subject = [0x81; 32];
        let (canonical, party) = encode_contribution_for_dimensions(
            CKS_CONTRIBUTION_TAG_V1,
            binding,
            subject,
            ZkAmsMkheProofKindV1::CksContribution,
            CKS_STATEMENT_DOMAIN_V1,
        );
        preflight_contribution(
            &canonical,
            CKS_CONTRIBUTION_TAG_V1,
            binding,
            subject,
            party,
            ZkAmsMkheProofKindV1::CksContribution,
            dimensions(),
            dimensions().max_round_bytes,
        )
        .unwrap();
        let proof_length_offset =
            CONTRIBUTION_HEADER_WIRE_BYTES + dimensions().polynomial_wire_bytes().unwrap() - 4;
        let max = max_proof_envelope_bytes(dimensions()).unwrap();
        for length in [0, max, max + 1, u32::MAX as usize] {
            let mut forged = canonical.clone();
            forged[proof_length_offset..proof_length_offset + 4]
                .copy_from_slice(&u32::try_from(length).unwrap().to_be_bytes());
            assert!(
                preflight_contribution(
                    &forged,
                    CKS_CONTRIBUTION_TAG_V1,
                    binding,
                    subject,
                    party,
                    ZkAmsMkheProofKindV1::CksContribution,
                    dimensions(),
                    dimensions().max_round_bytes,
                )
                .is_err(),
                "embedded proof length {length}"
            );
        }

        let other_binding = ZkAmsMkheWireBindingV1 {
            transcript_digest: [0x82; 32],
            ..binding
        };
        assert!(
            preflight_contribution(
                &canonical,
                CKS_CONTRIBUTION_TAG_V1,
                other_binding,
                subject,
                party,
                ZkAmsMkheProofKindV1::CksContribution,
                dimensions(),
                dimensions().max_round_bytes,
            )
            .is_err()
        );
        let wrong_party = roster().parties[0];
        assert_ne!(wrong_party, party);
        assert!(
            preflight_contribution(
                &canonical,
                CKS_CONTRIBUTION_TAG_V1,
                binding,
                subject,
                wrong_party,
                ZkAmsMkheProofKindV1::CksContribution,
                dimensions(),
                dimensions().max_round_bytes,
            )
            .is_err()
        );
    }

    #[test]
    fn every_wire_ceiling_rejects_one_byte_over_its_governed_limit() {
        let binding = binding(0, 0);
        let ciphertext = encode_ciphertext_for_dimensions(binding, 0);
        let ciphertext_dimensions = WireDimensions {
            max_ciphertext_bytes: ciphertext.len() - 1,
            ..dimensions()
        };
        assert_eq!(
            preflight_ciphertext(&ciphertext, binding, 0, ciphertext_dimensions),
            Err(ZkAmsMkheErrorV1::WireTooLarge)
        );

        let key = encode_seeded_rkg_for_dimensions(binding);
        let key_dimensions = WireDimensions {
            max_evaluated_key_bytes: key.len() - 1,
            ..dimensions()
        };
        assert_eq!(
            preflight_seeded_rkg_key(&key, binding, key_dimensions),
            Err(ZkAmsMkheErrorV1::WireTooLarge)
        );

        let subject = [0x91; 32];
        let (contribution, party) = encode_contribution_for_dimensions(
            CKS_CONTRIBUTION_TAG_V1,
            binding,
            subject,
            ZkAmsMkheProofKindV1::CksContribution,
            CKS_STATEMENT_DOMAIN_V1,
        );
        assert_eq!(
            preflight_contribution(
                &contribution,
                CKS_CONTRIBUTION_TAG_V1,
                binding,
                subject,
                party,
                ZkAmsMkheProofKindV1::CksContribution,
                dimensions(),
                contribution.len() - 1,
            ),
            Err(ZkAmsMkheErrorV1::WireTooLarge)
        );
    }

    #[test]
    fn static_codec_kats_do_not_impersonate_release_parameter_wire_evidence() {
        let readiness = super::super::manifest::zk_ams_mkhe_readiness_v1().unwrap();
        assert!(!readiness.wire_gate);
        assert!(!readiness.malicious_party_gate);
        assert!(!readiness.decryption_share_gate);
        assert!(!readiness.release_kat_gate);
    }
}

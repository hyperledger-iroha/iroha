//! Exact compact collective RNS-BGV key and ciphertext core.
//!
//! This module owns the sole native two-polynomial collective ciphertext used
//! by encryption, evaluation, canonical wire conversion, and full-roster
//! decryption.  Secret RLWE coefficients never cross its public API boundary.

use super::{
    BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1, MaskedRelaxedRandomSourceV1,
    RnsPolynomial, SecretPolynomial, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::{
        ZkAmsMkheActiveCollectivePublicKeyStatementV1, ZkAmsMkheActiveCollectivePublicKeyWitnessV1,
        ZkAmsMkheActivePartySecretV1, ZkAmsMkheActiveRkgProofV1, ZkAmsMkheGovernedActiveRosterV1,
        prove_zk_ams_mkhe_active_collective_public_key_v1,
        verify_zk_ams_mkhe_active_collective_public_key_v1,
        zk_ams_mkhe_active_collective_public_a_v1,
    },
    checked_coefficient_work, checked_ring_multiplication_work,
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_release_manifest_v1,
        zk_ams_mkhe_security_certificate_v1,
    },
    packing::{ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1, packed_plaintext_to_rns_v1},
    wire::{
        ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheGovernedRosterWireV1,
        ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheWireBindingV1, governed_roster_digest,
    },
};
use crate::vega::sponge::{Keccak256, keccak256};

const COLLECTIVE_CIPHERTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.compact-collective-ciphertext";
const COLLECTIVE_PARTY_SHARE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-public-key-share";
const COLLECTIVE_PUBLIC_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-public-key";
const COLLECTIVE_ENCRYPTION_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-encryption";
const COLLECTIVE_ADD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-add";
const COLLECTIVE_SUB_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-sub";
const COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-plaintext-mul";
const COLLECTIVE_AUTOMORPHISM_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-automorphism";
const COLLECTIVE_MULTIPLY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-multiply";
const COLLECTIVE_LEVEL_ONE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-level-one";

struct ZeroizingRns(RnsPolynomial);

impl Drop for ZeroizingRns {
    fn drop(&mut self) {
        self.0.coefficients.fill(0);
    }
}

/// Opaque RLWE state owned by one exact governed party and secret epoch.
///
/// The ternary secret and centered-binomial public-key error are generated
/// internally, are redacted from debug output, and are zeroized by their
/// underlying secret containers on drop.
pub struct ZkAmsMkheCollectivePartyStateV1 {
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    public_share_digest: [u8; 32],
    secret: SecretPolynomial,
    public_error: SecretPolynomial,
}

impl core::fmt::Debug for ZkAmsMkheCollectivePartyStateV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectivePartyStateV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field(
                "security_certificate_digest",
                &hex::encode(self.security_certificate_digest),
            )
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field("party_index", &self.party_index)
            .field("party", &self.party)
            .field(
                "public_share_digest",
                &hex::encode(self.public_share_digest),
            )
            .field("secret", &"[REDACTED]")
            .field("public_error", &"[REDACTED]")
            .finish()
    }
}

impl ZkAmsMkheCollectivePartyStateV1 {
    pub(super) const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    pub(super) const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Authentication-key-derived governed party identifier.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Exact zero-based position in the governed eight-party roster.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }

    /// Digest of the matching verified public share.
    #[must_use]
    pub const fn public_share_digest(&self) -> [u8; 32] {
        self.public_share_digest
    }

    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Exact collective-key transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    pub(super) const fn secret(&self) -> &SecretPolynomial {
        &self.secret
    }

    pub(super) const fn public_error(&self) -> &SecretPolynomial {
        &self.public_error
    }

    pub(super) const fn profile_digest_internal(&self) -> [u8; 32] {
        self.profile_digest
    }

    pub(super) const fn security_certificate_digest_internal(&self) -> [u8; 32] {
        self.security_certificate_digest
    }

    pub(super) const fn roster_digest_internal(&self) -> [u8; 32] {
        self.roster_digest
    }

    pub(super) const fn key_material_digest_internal(&self) -> [u8; 32] {
        self.key_material_digest
    }
}

/// One proof-carrying share of the exact eight-party collective public key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectivePublicKeyShareV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    public_a: ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: ZkAmsMkheRnsPolynomialWireV1,
    proof: ZkAmsMkheActiveRkgProofV1,
    digest: [u8; 32],
}

impl ZkAmsMkheCollectivePublicKeyShareV1 {
    /// Exact governed contributor.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Exact governed roster position.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }

    /// Common deterministic public `a` polynomial.
    #[must_use]
    pub const fn public_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.public_a
    }

    /// This party's `b_i = -a*s_i + t*e_i` contribution.
    #[must_use]
    pub const fn party_public_b(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.party_public_b
    }

    /// Native bounded-relation and authentication proof.
    #[must_use]
    pub const fn proof(&self) -> &ZkAmsMkheActiveRkgProofV1 {
        &self.proof
    }

    /// Consensus digest of the complete share and proof.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// Verified aggregate of all eight collective-public-key shares.
pub struct ZkAmsMkheCollectivePublicKeyV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    parties: super::PartySet,
    public_a: RnsPolynomial,
    collective_public_b: RnsPolynomial,
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheCollectivePublicKeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectivePublicKeyV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field("share_digests", &self.share_digests.map(hex::encode))
            .field("digest", &hex::encode(self.digest))
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCollectivePublicKeyV1 {
    /// Frozen release profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    /// Frozen estimator certificate digest.
    #[must_use]
    pub const fn security_certificate_digest(&self) -> [u8; 32] {
        self.security_certificate_digest
    }

    /// Exact ordered governed roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Exact collective-key ceremony transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Consensus identity of the aggregate public key and all eight shares.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Canonical release wire form of common `a`.
    pub fn public_a_wire(&self) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
        self.validate(&release_profile_v1())?;
        ZkAmsMkheRnsPolynomialWireV1::new(self.public_a.coefficients.clone())
    }

    /// Canonical release wire form of aggregate `b = sum_i b_i`.
    pub fn collective_public_b_wire(
        &self,
    ) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
        self.validate(&release_profile_v1())?;
        ZkAmsMkheRnsPolynomialWireV1::new(self.collective_public_b.coefficients.clone())
    }

    pub(super) const fn parties(&self) -> &super::PartySet {
        &self.parties
    }

    pub(super) const fn key_material_digest_internal(&self) -> [u8; 32] {
        self.key_material_digest
    }

    pub(super) const fn share_digests_internal(
        &self,
    ) -> &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.share_digests
    }

    pub(super) fn validate(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.security_certificate_digest == [0; 32]
            || self.roster_digest
                != governed_roster_digest(self.profile_digest, self.epoch, &self.parties.parties)
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.share_digests.iter().any(|digest| *digest == [0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.public_a.validate(profile)?;
        self.collective_public_b.validate(profile)?;
        if self.public_a == RnsPolynomial::zero(profile)
            || self.collective_public_b == RnsPolynomial::zero(profile)
            || self.digest == [0; 32]
            || self.digest != collective_public_key_digest(self, profile)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}

/// Generate opaque RLWE state and its proof-carrying public-key share for one
/// exact governed roster position.
///
/// The caller supplies only the governed authentication secret and a
/// cryptographic random source. Raw RLWE secret/error arrays are neither an
/// input nor an output of this boundary.
pub fn generate_zk_ams_mkhe_collective_party_state_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<
    (
        ZkAmsMkheCollectivePartyStateV1,
        ZkAmsMkheCollectivePublicKeyShareV1,
    ),
    ZkAmsMkheErrorV1,
> {
    // Complete all attacker-controlled scalar checks before allocating any
    // ring-sized secret or polynomial storage.
    roster.validate()?;
    if transcript_digest == [0; 32]
        || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || roster.participants()[party_index].party() != party_secret.party()?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    profile.validate()?;
    let security_certificate_digest = release_security_certificate_digest()?;
    let public_a = zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?;
    let public_a_native = RnsPolynomial::from_flat(&profile, public_a.residues().to_vec())?;
    let secret = sample_nonzero_ternary(&profile, random)?;
    let public_error = SecretPolynomial::sample_error(&profile, random)?;
    let secret_rns = ZeroizingRns(secret.as_rns(&profile)?);
    let scaled_error = scaled_public_error(&profile, &public_error)?;
    let party_public_b_native = public_a_native
        .mul(&secret_rns.0, &profile)?
        .negate(&profile)?
        .add(&scaled_error, &profile)?;
    let party_public_b = ZkAmsMkheRnsPolynomialWireV1::new(party_public_b_native.coefficients)?;
    let statement = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(&public_a, &party_public_b)?;
    let witness = ZkAmsMkheActiveCollectivePublicKeyWitnessV1::new(
        &secret.coefficients,
        &public_error.coefficients,
    )?;
    let proof = prove_zk_ams_mkhe_active_collective_public_key_v1(
        roster,
        transcript_digest,
        party_index,
        statement,
        witness,
        party_secret,
        random,
    )?;
    let mut share = ZkAmsMkheCollectivePublicKeyShareV1 {
        version: MKHE_VERSION_V1,
        profile_digest: roster.profile_digest(),
        security_certificate_digest,
        roster_digest: roster.roster_digest(),
        key_material_digest: roster.key_material_digest(),
        epoch: roster.epoch(),
        transcript_digest,
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        party: party_secret.party()?,
        public_a,
        party_public_b,
        proof,
        digest: [0; 32],
    };
    share.digest = collective_public_key_share_digest(&share)?;
    validate_collective_public_key_share(roster, transcript_digest, party_index, &share)?;
    let state = ZkAmsMkheCollectivePartyStateV1 {
        profile_digest: share.profile_digest,
        security_certificate_digest,
        roster_digest: share.roster_digest,
        key_material_digest: share.key_material_digest,
        epoch: share.epoch,
        transcript_digest,
        party_index: share.party_index,
        party: share.party,
        public_share_digest: share.digest,
        secret,
        public_error,
    };
    Ok((state, share))
}

/// Verify and aggregate exactly all eight ordered collective-public-key shares.
///
/// Missing, duplicate, reordered, cross-roster, cross-epoch, cross-transcript,
/// and proof-spliced shares are rejected before the aggregate key is returned.
pub fn aggregate_zk_ams_mkhe_collective_public_key_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<ZkAmsMkheCollectivePublicKeyV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    if transcript_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    profile.validate()?;
    // Verification is deliberately complete before allocating the aggregate
    // output polynomial, so malformed proof sets cannot trigger that work.
    for (party_index, share) in shares.iter().enumerate() {
        validate_collective_public_key_share(roster, transcript_digest, party_index, share)?;
    }
    let expected_public_a = zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?;
    if shares
        .iter()
        .any(|share| share.public_a != expected_public_a)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    checked_coefficient_work(&profile, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?;
    let mut aggregate_b = RnsPolynomial::zero(&profile);
    for share in shares {
        let party_b = RnsPolynomial::from_flat(&profile, share.party_public_b.residues().to_vec())?;
        aggregate_b = aggregate_b.add(&party_b, &profile)?;
    }
    let parties = super::PartySet::new(
        roster
            .participants()
            .iter()
            .map(|participant| participant.party())
            .collect(),
    )?;
    let mut key = ZkAmsMkheCollectivePublicKeyV1 {
        version: MKHE_VERSION_V1,
        profile_digest: roster.profile_digest(),
        security_certificate_digest: release_security_certificate_digest()?,
        roster_digest: roster.roster_digest(),
        key_material_digest: roster.key_material_digest(),
        epoch: roster.epoch(),
        transcript_digest,
        parties,
        public_a: RnsPolynomial::from_flat(&profile, expected_public_a.residues().to_vec())?,
        collective_public_b: aggregate_b,
        share_digests: shares.map(ZkAmsMkheCollectivePublicKeyShareV1::digest),
        digest: [0; 32],
    };
    key.digest = collective_public_key_digest(&key, &profile)?;
    key.validate(&profile)?;
    Ok(key)
}

fn validate_collective_public_key_share(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    let security_certificate_digest = release_security_certificate_digest()?;
    let expected_party = roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
        .party();
    if share.version != MKHE_VERSION_V1
        || share.profile_digest != roster.profile_digest()
        || share.security_certificate_digest != security_certificate_digest
        || share.roster_digest != roster.roster_digest()
        || share.key_material_digest != roster.key_material_digest()
        || share.epoch != roster.epoch()
        || share.transcript_digest != transcript_digest
        || usize::from(share.party_index) != party_index
        || share.party != expected_party
        || share.digest == [0; 32]
        || share.digest != collective_public_key_share_digest(share)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let expected_public_a = zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?;
    if share.public_a != expected_public_a {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let statement =
        ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(&share.public_a, &share.party_public_b)?;
    verify_zk_ams_mkhe_active_collective_public_key_v1(
        roster,
        transcript_digest,
        party_index,
        statement,
        &share.proof,
    )
}

fn release_security_certificate_digest() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let certificate = zk_ams_mkhe_security_certificate_v1()?;
    let digest = certificate.certificate_digest();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(digest)
}

fn collective_public_key_share_digest(
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    share.public_a.encoded_len()?;
    share.party_public_b.encoded_len()?;
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PARTY_SHARE_DOMAIN_V1);
    hash.update(&[share.version]);
    hash.update(&share.profile_digest);
    hash.update(&share.security_certificate_digest);
    hash.update(&share.roster_digest);
    hash.update(&share.key_material_digest);
    hash.update(&share.epoch.to_be_bytes());
    hash.update(&share.transcript_digest);
    hash.update(&[share.party_index]);
    hash.update(&share.party.to_bytes());
    update_wire_polynomial_hash(&mut hash, &share.public_a)?;
    update_wire_polynomial_hash(&mut hash, &share.party_public_b)?;
    hash.update(&share.proof.statement_digest());
    hash.update(&[share.proof.witness_polynomials()]);
    hash.update(&share.proof.contribution().digest()?);
    Ok(hash.finalize())
}

fn collective_public_key_digest(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    profile: &BgvProfile,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    key.public_a.validate(profile)?;
    key.collective_public_b.validate(profile)?;
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PUBLIC_KEY_DOMAIN_V1);
    hash.update(&[key.version]);
    hash.update(&key.profile_digest);
    hash.update(&key.security_certificate_digest);
    hash.update(&key.roster_digest);
    hash.update(&key.key_material_digest);
    hash.update(&key.epoch.to_be_bytes());
    hash.update(&key.transcript_digest);
    for party in &key.parties.parties {
        hash.update(&party.to_bytes());
    }
    update_rns_hash(&mut hash, profile, &key.public_a)?;
    update_rns_hash(&mut hash, profile, &key.collective_public_b)?;
    for share_digest in &key.share_digests {
        hash.update(share_digest);
    }
    Ok(hash.finalize())
}

fn update_wire_polynomial_hash(
    hash: &mut Keccak256,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    hash.update(
        &u32::try_from(polynomial.residues().len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    for residue in polynomial.residues() {
        hash.update(&residue.to_be_bytes());
    }
    Ok(())
}

/// Exact native collective ciphertext containing only `(c_0, c_1)`.
///
/// The transcript digest is the artifact-lineage commitment.  Constructors
/// for encryption and evaluation derive it from the frozen security
/// certificate, collective-key digest, exact input identity, and operation;
/// callers never provide a replacement lineage digest to those APIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveCiphertextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    sample_index: u64,
    level: u8,
    constant: RnsPolynomial,
    linear: RnsPolynomial,
    // Native evaluation capability. It is deliberately absent after decoding
    // an untrusted wire record; decryption remains possible, but homomorphic
    // evaluation requires an exact verified collective key.
    evaluation_key_digest: Option<[u8; 32]>,
    digest: [u8; 32],
}

impl ZkAmsMkheCollectiveCiphertextV1 {
    /// Decode the exact release wire representation under its governed roster.
    ///
    /// Wire dimensions and every binding axis are checked before residue
    /// storage is copied into the native representation.
    pub fn from_release_wire(
        roster: &ZkAmsMkheGovernedRosterWireV1,
        wire: &ZkAmsMkheCollectiveCiphertextWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let binding = wire.binding();
        if binding.profile_digest() != roster.profile_digest()
            || binding.roster_digest() != roster.roster_digest()
            || binding.epoch() != roster.epoch()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        // `encoded_len` performs the release preflight without allocating.
        wire.constant().encoded_len()?;
        wire.linear().encoded_len()?;
        let parties = super::PartySet::new(roster.parties().to_vec())?;
        Self::new(
            &profile,
            &parties,
            roster.epoch(),
            binding.transcript_digest(),
            wire.sample_index(),
            binding.level(),
            RnsPolynomial::from_flat(&profile, wire.constant().residues().to_vec())?,
            RnsPolynomial::from_flat(&profile, wire.linear().residues().to_vec())?,
        )
    }

    /// Convert to the sole canonical release wire representation.
    pub fn to_release_wire(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        record_index: u32,
    ) -> Result<ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        if roster.profile_digest() != self.profile_digest
            || roster.roster_digest() != self.roster_digest
            || roster.epoch() != self.epoch
            || self.sample_index >= zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let parties = super::PartySet::new(roster.parties().to_vec())?;
        self.validate(&profile, &parties)?;
        let binding =
            ZkAmsMkheWireBindingV1::new(roster, self.transcript_digest, record_index, self.level)?;
        ZkAmsMkheCollectiveCiphertextWireV1::new(
            binding,
            self.sample_index,
            ZkAmsMkheRnsPolynomialWireV1::new(self.constant.coefficients.clone())?,
            ZkAmsMkheRnsPolynomialWireV1::new(self.linear.coefficients.clone())?,
        )
    }

    /// Frozen profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    /// Exact ordered governed-roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Nonzero governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Exact security/key/input/operation lineage digest.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Zero-based RLWE sample identity for fresh encryption, or the canonical
    /// minimum origin index for an evaluated result. The transcript commits
    /// the complete ordered operand lineage.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }

    /// BGV ciphertext level (`0` or `1`).
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    /// Consensus digest of every native field and both exact polynomials.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Verified native evaluation-key identity, absent for wire-only records.
    #[must_use]
    pub const fn evaluation_key_digest(&self) -> Option<[u8; 32]> {
        self.evaluation_key_digest
    }

    pub(super) fn new(
        profile: &BgvProfile,
        parties: &super::PartySet,
        epoch: u64,
        transcript_digest: [u8; 32],
        sample_index: u64,
        level: u8,
        constant: RnsPolynomial,
        linear: RnsPolynomial,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_with_key(
            profile,
            parties,
            epoch,
            transcript_digest,
            sample_index,
            level,
            constant,
            linear,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_with_key(
        profile: &BgvProfile,
        parties: &super::PartySet,
        epoch: u64,
        transcript_digest: [u8; 32],
        sample_index: u64,
        level: u8,
        constant: RnsPolynomial,
        linear: RnsPolynomial,
        evaluation_key_digest: Option<[u8; 32]>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        if parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || epoch == 0
            || transcript_digest == [0; 32]
            || level > 1
            || evaluation_key_digest.is_some_and(|digest| digest == [0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        constant.validate(profile)?;
        linear.validate(profile)?;
        let profile_digest = profile.digest()?;
        let roster_digest = governed_roster_digest(profile_digest, epoch, &parties.parties);
        let mut value = Self {
            profile_digest,
            roster_digest,
            epoch,
            transcript_digest,
            sample_index,
            level,
            constant,
            linear,
            evaluation_key_digest,
            digest: [0; 32],
        };
        value.digest = value.compute_digest(profile)?;
        value.validate(profile, parties)?;
        Ok(value)
    }

    pub(super) fn validate(
        &self,
        profile: &BgvProfile,
        parties: &super::PartySet,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.profile_digest != profile.digest()?
            || self.roster_digest
                != governed_roster_digest(self.profile_digest, self.epoch, &parties.parties)
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.level > 1
            || self
                .evaluation_key_digest
                .is_some_and(|digest| digest == [0; 32])
            || parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.constant.validate(profile)?;
        self.linear.validate(profile)?;
        if self.digest == [0; 32] || self.digest != self.compute_digest(profile)? {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }

    pub(super) const fn constant(&self) -> &RnsPolynomial {
        &self.constant
    }

    pub(super) const fn linear(&self) -> &RnsPolynomial {
        &self.linear
    }

    fn compute_digest(&self, profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut hash = Keccak256::new();
        hash.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
        hash.update(&self.profile_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&[self.level]);
        update_rns_hash(&mut hash, profile, &self.constant)?;
        update_rns_hash(&mut hash, profile, &self.linear)?;
        Ok(hash.finalize())
    }
}

/// Encrypt one exact canonical T256 packed-plaintext chunk under the verified
/// all-eight collective public key.
pub fn encrypt_zk_ams_mkhe_collective_packed_v1<R: MaskedRelaxedRandomSourceV1>(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    layout: ZkAmsT256PackingLayoutV1,
    plaintext: &ZkAmsT256PackedPlaintextV1,
    sample_index: u64,
    random: &mut R,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    key.validate(&profile)?;
    let manifest = zk_ams_mkhe_release_manifest_v1()?;
    if sample_index >= manifest.max_samples_per_secret_epoch
        || layout.profile_digest != key.profile_digest
        || plaintext.profile_digest != key.profile_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    // Canonical layout/digest/padding checks happen inside this conversion,
    // before secret randomness or ciphertext-sized output is allocated.
    let message = packed_plaintext_to_rns_v1(layout, plaintext)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_ENCRYPTION_DOMAIN_V1,
        key,
        &[],
        &[
            layout.digest.as_slice(),
            plaintext.digest.as_slice(),
            &plaintext.chunk_index.to_be_bytes(),
            &sample_index.to_be_bytes(),
        ],
    );
    encrypt_collective_native(
        &profile,
        key,
        &message,
        transcript_digest,
        sample_index,
        random,
    )
}

impl ZkAmsMkheCollectiveCiphertextV1 {
    /// Add two same-key compact ciphertexts and derive exact combined lineage.
    pub fn add(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_ADD_DOMAIN_V1, RnsPolynomial::add)
    }

    /// Subtract two same-key compact ciphertexts and derive exact combined lineage.
    pub fn sub(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_SUB_DOMAIN_V1, RnsPolynomial::sub)
    }

    /// Multiply both components by one exact canonical packed plaintext.
    pub fn mul_plaintext(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        validate_compact_for_key(self, key, &profile)?;
        if layout.profile_digest != key.profile_digest
            || plaintext.profile_digest != key.profile_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let multiplier = packed_plaintext_to_rns_v1(layout, plaintext)?;
        compact_plaintext_mul_with_profile(
            &profile,
            self,
            key,
            &multiplier,
            &[layout.digest.as_slice(), plaintext.digest.as_slice()],
        )
    }

    /// Apply the exact raw Galois automorphism to both components.
    ///
    /// The result is deliberately bound to the automorphed secret-key domain;
    /// it cannot be evaluated as an original-key ciphertext until a verified
    /// Galois key switch restores that domain.
    pub fn automorphism(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        exponent: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        validate_compact_for_key(self, key, &profile)?;
        let exponent_bytes = u64::try_from(exponent)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?
            .to_be_bytes();
        compact_automorphism_with_profile(&profile, self, key, exponent, exponent_bytes)
    }

    /// Multiply two level-zero ciphertexts into the exact unrelinearized
    /// `(d_0, d_1, d_2)` level-one form.
    pub fn multiply(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        multiply_with_profile(&profile, self, key, rhs)
    }

    fn binary(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
        domain: &[u8],
        operation: fn(
            &RnsPolynomial,
            &RnsPolynomial,
            &BgvProfile,
        ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        compact_binary_with_profile(&profile, self, key, rhs, domain, operation)
    }
}

fn compact_binary_with_profile(
    profile: &BgvProfile,
    left: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    right: &ZkAmsMkheCollectiveCiphertextV1,
    domain: &[u8],
    operation: fn(
        &RnsPolynomial,
        &RnsPolynomial,
        &BgvProfile,
    ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(left, key, profile)?;
    validate_compact_for_key(right, key, profile)?;
    if left.level != right.level {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_coefficient_work(profile, 2)?;
    let transcript_digest =
        collective_lineage_digest(domain, key, &[left.digest, right.digest], &[]);
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        left.epoch,
        transcript_digest,
        left.sample_index.min(right.sample_index),
        left.level,
        operation(&left.constant, &right.constant, profile)?,
        operation(&left.linear, &right.linear, profile)?,
        Some(key.digest),
    )
}

fn multiply_with_profile(
    profile: &BgvProfile,
    left: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    right: &ZkAmsMkheCollectiveCiphertextV1,
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(left, key, profile)?;
    validate_compact_for_key(right, key, profile)?;
    if left.level != 0 || right.level != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_ring_multiplication_work(profile, 4)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_MULTIPLY_DOMAIN_V1,
        key,
        &[left.digest, right.digest],
        &[],
    );
    let linear = left
        .constant
        .mul(&right.linear, profile)?
        .add(&left.linear.mul(&right.constant, profile)?, profile)?;
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        left.sample_index.min(right.sample_index),
        left.constant.mul(&right.constant, profile)?,
        linear,
        left.linear.mul(&right.linear, profile)?,
        key.digest,
    )
}

fn compact_plaintext_mul_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    multiplier: &RnsPolynomial,
    input_identity: &[&[u8]],
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(ciphertext, key, profile)?;
    multiplier.validate(profile)?;
    checked_ring_multiplication_work(profile, 2)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        input_identity,
    );
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        ciphertext.epoch,
        transcript_digest,
        ciphertext.sample_index,
        ciphertext.level,
        ciphertext.constant.mul(multiplier, profile)?,
        ciphertext.linear.mul(multiplier, profile)?,
        Some(key.digest),
    )
}

fn compact_automorphism_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    exponent: usize,
    exponent_bytes: [u8; 8],
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(ciphertext, key, profile)?;
    // Validate the exponent before deriving the new key-domain identity.
    let constant = ciphertext.constant.automorphism(exponent, profile)?;
    let linear = ciphertext.linear.automorphism(exponent, profile)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        &[&exponent_bytes],
    );
    let transformed_key_digest = keccak256(
        &[
            COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
            key.digest.as_slice(),
            &exponent_bytes,
        ]
        .concat(),
    );
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        ciphertext.epoch,
        transcript_digest,
        ciphertext.sample_index,
        ciphertext.level,
        constant,
        linear,
        Some(transformed_key_digest),
    )
}

/// Exact unrelinearized three-polynomial level-one collective ciphertext.
pub struct ZkAmsMkheCollectiveLevelOneV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    sample_index: u64,
    evaluation_key_digest: [u8; 32],
    constant: RnsPolynomial,
    linear: RnsPolynomial,
    quadratic: RnsPolynomial,
    digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheCollectiveLevelOneV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveLevelOneV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field("sample_index", &self.sample_index)
            .field(
                "evaluation_key_digest",
                &hex::encode(self.evaluation_key_digest),
            )
            .field("digest", &hex::encode(self.digest))
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCollectiveLevelOneV1 {
    /// Frozen profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    /// Exact ordered governed-roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Security/key/input/operation lineage digest.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Canonical minimum origin-sample index; complete origin identity is in
    /// the transcript lineage.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }

    /// Exact evaluation-key domain required by this ciphertext.
    #[must_use]
    pub const fn evaluation_key_digest(&self) -> [u8; 32] {
        self.evaluation_key_digest
    }

    /// Consensus digest of all context fields and exactly three polynomials.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Add two same-domain level-one ciphertexts.
    pub fn add(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_ADD_DOMAIN_V1, RnsPolynomial::add)
    }

    /// Subtract two same-domain level-one ciphertexts.
    pub fn sub(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_SUB_DOMAIN_V1, RnsPolynomial::sub)
    }

    /// Multiply all three level-one components by a canonical packed plaintext.
    pub fn mul_plaintext(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_for_key(key, &profile)?;
        if layout.profile_digest != key.profile_digest
            || plaintext.profile_digest != key.profile_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let multiplier = packed_plaintext_to_rns_v1(layout, plaintext)?;
        level_one_plaintext_mul_with_profile(
            &profile,
            self,
            key,
            &multiplier,
            &[layout.digest.as_slice(), plaintext.digest.as_slice()],
        )
    }

    /// Apply the raw automorphism to all three components and move to the
    /// corresponding automorphed secret-key domain.
    pub fn automorphism(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        exponent: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_for_key(key, &profile)?;
        let exponent_bytes = u64::try_from(exponent)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?
            .to_be_bytes();
        level_one_automorphism_with_profile(&profile, self, key, exponent, exponent_bytes)
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        transcript_digest: [u8; 32],
        sample_index: u64,
        constant: RnsPolynomial,
        linear: RnsPolynomial,
        quadratic: RnsPolynomial,
        evaluation_key_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        key.validate(profile)?;
        if transcript_digest == [0; 32] || evaluation_key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        constant.validate(profile)?;
        linear.validate(profile)?;
        quadratic.validate(profile)?;
        let mut value = Self {
            version: MKHE_VERSION_V1,
            profile_digest: key.profile_digest,
            security_certificate_digest: key.security_certificate_digest,
            roster_digest: key.roster_digest,
            epoch: key.epoch,
            transcript_digest,
            sample_index,
            evaluation_key_digest,
            constant,
            linear,
            quadratic,
            digest: [0; 32],
        };
        value.digest = value.compute_digest(profile)?;
        value.validate(profile)?;
        Ok(value)
    }

    fn validate(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.security_certificate_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.evaluation_key_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.constant.validate(profile)?;
        self.linear.validate(profile)?;
        self.quadratic.validate(profile)?;
        if self.digest == [0; 32] || self.digest != self.compute_digest(profile)? {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }

    pub(super) fn validate_for_key(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        profile: &BgvProfile,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        key.validate(profile)?;
        self.validate(profile)?;
        if self.profile_digest != key.profile_digest
            || self.security_certificate_digest != key.security_certificate_digest
            || self.roster_digest != key.roster_digest
            || self.epoch != key.epoch
            || self.evaluation_key_digest != key.digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }

    pub(super) const fn constant(&self) -> &RnsPolynomial {
        &self.constant
    }

    pub(super) const fn linear(&self) -> &RnsPolynomial {
        &self.linear
    }

    pub(super) const fn quadratic(&self) -> &RnsPolynomial {
        &self.quadratic
    }

    fn binary(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
        domain: &[u8],
        operation: fn(
            &RnsPolynomial,
            &RnsPolynomial,
            &BgvProfile,
        ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        level_one_binary_with_profile(&profile, self, key, rhs, domain, operation)
    }

    fn compute_digest(&self, profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut hash = Keccak256::new();
        hash.update(COLLECTIVE_LEVEL_ONE_DOMAIN_V1);
        hash.update(&[self.version]);
        hash.update(&self.profile_digest);
        hash.update(&self.security_certificate_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&self.evaluation_key_digest);
        hash.update(&[1]);
        update_rns_hash(&mut hash, profile, &self.constant)?;
        update_rns_hash(&mut hash, profile, &self.linear)?;
        update_rns_hash(&mut hash, profile, &self.quadratic)?;
        Ok(hash.finalize())
    }
}

fn level_one_binary_with_profile(
    profile: &BgvProfile,
    left: &ZkAmsMkheCollectiveLevelOneV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    right: &ZkAmsMkheCollectiveLevelOneV1,
    domain: &[u8],
    operation: fn(
        &RnsPolynomial,
        &RnsPolynomial,
        &BgvProfile,
    ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    left.validate_for_key(key, profile)?;
    right.validate_for_key(key, profile)?;
    checked_coefficient_work(profile, 3)?;
    let transcript_digest =
        collective_lineage_digest(domain, key, &[left.digest, right.digest], &[]);
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        left.sample_index.min(right.sample_index),
        operation(&left.constant, &right.constant, profile)?,
        operation(&left.linear, &right.linear, profile)?,
        operation(&left.quadratic, &right.quadratic, profile)?,
        key.digest,
    )
}

fn level_one_plaintext_mul_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    multiplier: &RnsPolynomial,
    input_identity: &[&[u8]],
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    ciphertext.validate_for_key(key, profile)?;
    multiplier.validate(profile)?;
    checked_ring_multiplication_work(profile, 3)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        input_identity,
    );
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        ciphertext.sample_index,
        ciphertext.constant.mul(multiplier, profile)?,
        ciphertext.linear.mul(multiplier, profile)?,
        ciphertext.quadratic.mul(multiplier, profile)?,
        key.digest,
    )
}

fn level_one_automorphism_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    exponent: usize,
    exponent_bytes: [u8; 8],
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    ciphertext.validate_for_key(key, profile)?;
    let constant = ciphertext.constant.automorphism(exponent, profile)?;
    let linear = ciphertext.linear.automorphism(exponent, profile)?;
    let quadratic = ciphertext.quadratic.automorphism(exponent, profile)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        &[&exponent_bytes],
    );
    let transformed_key_digest = keccak256(
        &[
            COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
            key.digest.as_slice(),
            &exponent_bytes,
        ]
        .concat(),
    );
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        ciphertext.sample_index,
        constant,
        linear,
        quadratic,
        transformed_key_digest,
    )
}

pub(super) fn validate_compact_for_key(
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    key.validate(profile)?;
    ciphertext.validate(profile, &key.parties)?;
    if ciphertext.profile_digest != key.profile_digest
        || ciphertext.roster_digest != key.roster_digest
        || ciphertext.epoch != key.epoch
        || ciphertext.evaluation_key_digest != Some(key.digest)
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    Ok(())
}

fn encrypt_collective_native<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    message: &RnsPolynomial,
    transcript_digest: [u8; 32],
    sample_index: u64,
    random: &mut R,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    key.validate(profile)?;
    message.validate(profile)?;
    if transcript_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_ring_multiplication_work(profile, 2)?;
    let ephemeral = sample_nonzero_ternary(profile, random)?;
    let error_zero = SecretPolynomial::sample_error(profile, random)?;
    let error_one = SecretPolynomial::sample_error(profile, random)?;
    let ephemeral_rns = ZeroizingRns(ephemeral.as_rns(profile)?);
    let scaled_error_zero = scaled_public_error(profile, &error_zero)?;
    let scaled_error_one = scaled_public_error(profile, &error_one)?;
    let constant = key
        .collective_public_b
        .mul(&ephemeral_rns.0, profile)?
        .add(&scaled_error_zero, profile)?
        .add(message, profile)?;
    let linear = key
        .public_a
        .mul(&ephemeral_rns.0, profile)?
        .add(&scaled_error_one, profile)?;
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        key.epoch,
        transcript_digest,
        sample_index,
        0,
        constant,
        linear,
        Some(key.digest),
    )
}

fn collective_lineage_digest(
    domain: &[u8],
    key: &ZkAmsMkheCollectivePublicKeyV1,
    operand_digests: &[[u8; 32]],
    supplemental: &[&[u8]],
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(
        256 + operand_digests.len() * 32 + supplemental.iter().map(|v| v.len()).sum::<usize>(),
    );
    frame.extend_from_slice(domain);
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&key.profile_digest);
    frame.extend_from_slice(&key.security_certificate_digest);
    frame.extend_from_slice(&key.roster_digest);
    frame.extend_from_slice(&key.key_material_digest);
    frame.extend_from_slice(&key.epoch.to_be_bytes());
    frame.extend_from_slice(&key.transcript_digest);
    frame.extend_from_slice(&key.digest);
    frame.extend_from_slice(&(operand_digests.len() as u32).to_be_bytes());
    for digest in operand_digests {
        frame.extend_from_slice(digest);
    }
    frame.extend_from_slice(&(supplemental.len() as u32).to_be_bytes());
    for value in supplemental {
        frame.extend_from_slice(&(value.len() as u32).to_be_bytes());
        frame.extend_from_slice(value);
    }
    keccak256(&frame)
}

fn scaled_public_error(
    profile: &BgvProfile,
    error: &SecretPolynomial,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    let raw = ZeroizingRns(error.as_rns(profile)?);
    let scaled = raw.0.scale_plaintext_modulus(profile)?;
    Ok(scaled)
}

fn sample_nonzero_ternary<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let candidate = SecretPolynomial::sample_ternary(profile, random)?;
        if candidate
            .coefficients
            .iter()
            .any(|coefficient| *coefficient != 0)
        {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn update_rns_hash(
    hash: &mut Keccak256,
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    hash.update(
        &u32::try_from(polynomial.coefficients.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    for coefficient in &polynomial.coefficients {
        hash.update(&coefficient.to_be_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{MaskedRelaxedRandomErrorV1, sponge::shake256};

    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];

    fn test_profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0x61; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: super::super::PlaintextModulus::Tiny(17),
            error_eta: 2,
            hybrid_rns_decomposition: false,
            gadget_base_log: 8,
            gadget_digits: 8,
            max_ciphertext_bytes: 1 << 20,
            max_evaluated_key_bytes: 16 << 20,
            max_round_bytes: 16 << 20,
            max_share_bytes: 4 << 20,
            max_workspace_bytes: 16 << 20,
            max_work_units: 1 << 20,
        }
    }

    struct KatRandom {
        state: [u8; 32],
        counter: u64,
    }

    impl KatRandom {
        fn new(label: &[u8]) -> Self {
            Self {
                state: keccak256(label),
                counter: 0,
            }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for KatRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            let mut written = 0;
            while written < destination.len() {
                let mut frame = Vec::with_capacity(40);
                frame.extend_from_slice(&self.state);
                frame.extend_from_slice(&self.counter.to_be_bytes());
                let block = shake256(&frame, 64);
                let take = (destination.len() - written).min(block.len());
                destination[written..written + take].copy_from_slice(&block[..take]);
                self.state = keccak256(&block);
                self.counter = self.counter.wrapping_add(1);
                written += take;
            }
            Ok(())
        }
    }

    struct ConstantRandom(u8);

    impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(self.0);
            Ok(())
        }
    }

    fn test_parties() -> super::super::PartySet {
        super::super::PartySet::new(
            (1_u8..=ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 as u8)
                .map(|tag| {
                    let mut bytes = [0_u8; 32];
                    bytes[31] = tag;
                    ZkAmsMkhePartyIdV1::new(bytes).unwrap()
                })
                .collect(),
        )
        .unwrap()
    }

    fn test_key(label: u8) -> (ZkAmsMkheCollectivePublicKeyV1, SecretPolynomial) {
        let profile = test_profile();
        profile.validate().unwrap();
        let parties = test_parties();
        let aggregate_secret = SecretPolynomial {
            coefficients: vec![8, 0, 0, 0, 0, 0, 0, 0],
        };
        let public_a = RnsPolynomial::from_unsigned(&profile, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let collective_public_b = public_a
            .mul(&aggregate_secret.as_rns(&profile).unwrap(), &profile)
            .unwrap()
            .negate(&profile)
            .unwrap();
        let epoch = 19;
        let mut key = ZkAmsMkheCollectivePublicKeyV1 {
            version: MKHE_VERSION_V1,
            profile_digest: profile.digest().unwrap(),
            security_certificate_digest: [0x22; 32],
            roster_digest: governed_roster_digest(
                profile.digest().unwrap(),
                epoch,
                &parties.parties,
            ),
            key_material_digest: [label; 32],
            epoch,
            transcript_digest: [label.wrapping_add(1); 32],
            parties,
            public_a,
            collective_public_b,
            share_digests: core::array::from_fn(|index| [index as u8 + 1; 32]),
            digest: [0; 32],
        };
        key.digest = collective_public_key_digest(&key, &profile).unwrap();
        key.validate(&profile).unwrap();
        (key, aggregate_secret)
    }

    fn encrypt_test(
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        values: &[u64; 8],
        sample_index: u64,
        label: &[u8],
    ) -> ZkAmsMkheCollectiveCiphertextV1 {
        let message = RnsPolynomial::from_test_plaintext(profile, values).unwrap();
        encrypt_collective_native(
            profile,
            key,
            &message,
            keccak256(label),
            sample_index,
            &mut KatRandom::new(label),
        )
        .unwrap()
    }

    fn decrypt_compact(
        profile: &BgvProfile,
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
        secret: &SecretPolynomial,
    ) -> Vec<u64> {
        let value = ciphertext
            .constant
            .add(
                &ciphertext
                    .linear
                    .mul(&secret.as_rns(profile).unwrap(), profile)
                    .unwrap(),
                profile,
            )
            .unwrap();
        super::super::reduce_test_polynomial(profile, &value).unwrap()
    }

    fn decrypt_level_one(
        profile: &BgvProfile,
        ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
        secret: &SecretPolynomial,
    ) -> Vec<u64> {
        let secret = secret.as_rns(profile).unwrap();
        let secret_square = secret.mul(&secret, profile).unwrap();
        let value = ciphertext
            .constant
            .add(&ciphertext.linear.mul(&secret, profile).unwrap(), profile)
            .unwrap()
            .add(
                &ciphertext.quadratic.mul(&secret_square, profile).unwrap(),
                profile,
            )
            .unwrap();
        super::super::reduce_test_polynomial(profile, &value).unwrap()
    }

    fn negacyclic_plaintext_product(left: &[u64; 8], right: &[u64; 8]) -> Vec<u64> {
        let mut output = [0_i128; 8];
        for (left_index, left_value) in left.iter().copied().enumerate() {
            for (right_index, right_value) in right.iter().copied().enumerate() {
                let product = i128::from(left_value) * i128::from(right_value);
                let index = left_index + right_index;
                if index < 8 {
                    output[index] += product;
                } else {
                    output[index - 8] -= product;
                }
            }
        }
        output
            .into_iter()
            .map(|value| value.rem_euclid(17) as u64)
            .collect()
    }

    #[test]
    fn tiny_collective_algebra_matches_plaintext_oracle() {
        let profile = test_profile();
        let (key, secret) = test_key(0x31);
        let left_values = [1, 2, 3, 4, 5, 6, 7, 8];
        let right_values = [8, 0, 2, 0, 4, 0, 6, 0];
        let left = encrypt_test(&profile, &key, &left_values, 11, b"collective-left");
        let right = encrypt_test(&profile, &key, &right_values, 17, b"collective-right");
        assert_eq!(decrypt_compact(&profile, &left, &secret), left_values);
        assert_eq!(decrypt_compact(&profile, &right, &secret), right_values);

        let sum = compact_binary_with_profile(
            &profile,
            &left,
            &key,
            &right,
            COLLECTIVE_ADD_DOMAIN_V1,
            RnsPolynomial::add,
        )
        .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &sum, &secret),
            left_values
                .iter()
                .zip(right_values)
                .map(|(left, right)| (*left + right) % 17)
                .collect::<Vec<_>>()
        );
        assert_eq!(sum.sample_index(), 11);
        assert_ne!(sum.transcript_digest(), left.transcript_digest());

        let difference = compact_binary_with_profile(
            &profile,
            &left,
            &key,
            &right,
            COLLECTIVE_SUB_DOMAIN_V1,
            RnsPolynomial::sub,
        )
        .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &difference, &secret),
            left_values
                .iter()
                .zip(right_values)
                .map(|(left, right)| (17 + *left - right) % 17)
                .collect::<Vec<_>>()
        );
        assert_ne!(sum.digest(), difference.digest());

        let expected_product = negacyclic_plaintext_product(&left_values, &right_values);
        let product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &product, &secret),
            expected_product
        );
        assert_eq!(product.evaluation_key_digest(), key.digest());

        let plaintext_multiplier =
            RnsPolynomial::from_test_plaintext(&profile, &right_values).unwrap();
        let scaled = compact_plaintext_mul_with_profile(
            &profile,
            &left,
            &key,
            &plaintext_multiplier,
            &[b"canonical-test-plaintext"],
        )
        .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &scaled, &secret),
            expected_product
        );

        let doubled_product = level_one_binary_with_profile(
            &profile,
            &product,
            &key,
            &product,
            COLLECTIVE_ADD_DOMAIN_V1,
            RnsPolynomial::add,
        )
        .unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &doubled_product, &secret),
            expected_product
                .iter()
                .map(|value| (2 * value) % 17)
                .collect::<Vec<_>>()
        );
        let zero_product = level_one_binary_with_profile(
            &profile,
            &product,
            &key,
            &product,
            COLLECTIVE_SUB_DOMAIN_V1,
            RnsPolynomial::sub,
        )
        .unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &zero_product, &secret),
            vec![0; 8]
        );

        let scaled_product = level_one_plaintext_mul_with_profile(
            &profile,
            &product,
            &key,
            &plaintext_multiplier,
            &[b"canonical-level-one-test-plaintext"],
        )
        .unwrap();
        let expected_product_array: [u64; 8] = expected_product.clone().try_into().unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &scaled_product, &secret),
            negacyclic_plaintext_product(&expected_product_array, &right_values)
        );

        let transformed_product =
            level_one_automorphism_with_profile(&profile, &product, &key, 3, 3_u64.to_be_bytes())
                .unwrap();
        let transformed_secret = secret.automorphism(3, &profile).unwrap();
        let expected_transformed =
            RnsPolynomial::from_test_plaintext(&profile, &expected_product_array)
                .unwrap()
                .automorphism(3, &profile)
                .unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &transformed_product, &transformed_secret),
            super::super::reduce_test_polynomial(&profile, &expected_transformed).unwrap()
        );
    }

    #[test]
    fn raw_automorphism_moves_to_exact_automorphed_key_domain() {
        let profile = test_profile();
        let (key, secret) = test_key(0x41);
        let values = [1, 2, 4, 8, 3, 6, 12, 7];
        let ciphertext = encrypt_test(&profile, &key, &values, 5, b"collective-auto");
        let exponent = 3;
        let transformed = compact_automorphism_with_profile(
            &profile,
            &ciphertext,
            &key,
            exponent,
            (exponent as u64).to_be_bytes(),
        )
        .unwrap();
        assert_ne!(transformed.evaluation_key_digest(), Some(key.digest()));
        assert_eq!(
            validate_compact_for_key(&transformed, &key, &profile),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
        let transformed_secret = secret.automorphism(exponent, &profile).unwrap();
        let expected = RnsPolynomial::from_test_plaintext(&profile, &values)
            .unwrap()
            .automorphism(exponent, &profile)
            .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &transformed, &transformed_secret),
            super::super::reduce_test_polynomial(&profile, &expected).unwrap()
        );
        for invalid in [0, 2, 16, usize::MAX] {
            assert!(
                compact_automorphism_with_profile(
                    &profile,
                    &ciphertext,
                    &key,
                    invalid,
                    u64::try_from(invalid).unwrap_or(u64::MAX).to_be_bytes(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn cross_key_unbound_and_tampered_ciphertexts_fail_closed() {
        let profile = test_profile();
        let (key, _) = test_key(0x51);
        let (other_key, _) = test_key(0x52);
        let values = [1, 0, 0, 0, 0, 0, 0, 0];
        let ciphertext = encrypt_test(&profile, &key, &values, 3, b"collective-binding");
        assert_eq!(
            compact_binary_with_profile(
                &profile,
                &ciphertext,
                &other_key,
                &ciphertext,
                COLLECTIVE_ADD_DOMAIN_V1,
                RnsPolynomial::add,
            ),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );

        let unbound = ZkAmsMkheCollectiveCiphertextV1::new(
            &profile,
            &key.parties,
            key.epoch,
            [0x71; 32],
            3,
            0,
            ciphertext.constant.clone(),
            ciphertext.linear.clone(),
        )
        .unwrap();
        assert_eq!(
            compact_binary_with_profile(
                &profile,
                &unbound,
                &key,
                &ciphertext,
                COLLECTIVE_ADD_DOMAIN_V1,
                RnsPolynomial::add,
            ),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );

        for axis in 0..4 {
            let mut tampered = ciphertext.clone();
            match axis {
                0 => tampered.profile_digest[0] ^= 1,
                1 => tampered.roster_digest[0] ^= 1,
                2 => tampered.epoch ^= 1,
                _ => tampered.transcript_digest[0] ^= 1,
            }
            assert!(tampered.validate(&profile, &key.parties).is_err());
        }
        let mut tampered_component = ciphertext.clone();
        tampered_component.constant.coefficients[0] = TEST_MODULI[0];
        assert_eq!(
            tampered_component.validate(&profile, &key.parties),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
    }

    #[test]
    fn level_one_component_and_digest_tampering_is_rejected() {
        let profile = test_profile();
        let (key, _) = test_key(0x61);
        let values = [1, 2, 3, 4, 5, 6, 7, 8];
        let left = encrypt_test(&profile, &key, &values, 1, b"level-one-left");
        let right = encrypt_test(&profile, &key, &values, 2, b"level-one-right");
        let mut product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
        product.quadratic.coefficients[0] ^= 1;
        assert_eq!(
            product.validate(&profile),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
        let mut product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
        product.evaluation_key_digest[0] ^= 1;
        assert_eq!(
            product.validate(&profile),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
    }

    #[test]
    fn deterministic_zero_ternary_rng_exhausts_without_emitting_secret_or_ciphertext() {
        let profile = test_profile();
        let mut zero_ternary = ConstantRandom(0x55);
        assert!(matches!(
            sample_nonzero_ternary(&profile, &mut zero_ternary),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));

        let (key, _) = test_key(0x71);
        let message = RnsPolynomial::from_test_plaintext(&profile, &[0; 8]).unwrap();
        let mut zero_ternary = ConstantRandom(0x55);
        assert_eq!(
            encrypt_collective_native(&profile, &key, &message, [0x72; 32], 0, &mut zero_ternary,),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
    }

    #[test]
    fn opaque_party_state_debug_and_api_do_not_expose_rlwe_coefficients() {
        let state = ZkAmsMkheCollectivePartyStateV1 {
            profile_digest: [1; 32],
            security_certificate_digest: [2; 32],
            roster_digest: [3; 32],
            key_material_digest: [4; 32],
            epoch: 1,
            transcript_digest: [5; 32],
            party_index: 0,
            party: test_parties().parties[0],
            public_share_digest: [6; 32],
            secret: SecretPolynomial {
                coefficients: vec![1, -1, 0],
            },
            public_error: SecretPolynomial {
                coefficients: vec![2, -2, 0],
            },
        };
        let debug = format!("{state:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("-1"));
        assert!(!debug.contains("-2"));
        assert_eq!(state.secret().coefficients.len(), 3);
        assert_eq!(state.public_error().coefficients.len(), 3);
    }
}

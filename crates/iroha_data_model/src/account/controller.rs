//! Account controller policies (single key and multisignature).
#![allow(clippy::useless_let_if_seq)]

use core::fmt;
use std::vec::Vec;

use blake2::{
    Blake2bMac,
    digest::{Mac, consts::U32},
};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::curve::CurveId;

/// Controller responsible for authorising account actions.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "payload", no_fast_from_json)]
pub enum AccountController {
    /// Single public key controls the account.
    Single(PublicKey),
    /// Multisignature policy controls the account.
    Multisig(MultisigPolicy),
}

impl AccountController {
    /// Construct a single-signature controller.
    #[must_use]
    pub fn single(signatory: PublicKey) -> Self {
        Self::Single(signatory)
    }

    /// Construct a multisignature controller.
    #[must_use]
    pub fn multisig(policy: MultisigPolicy) -> Self {
        Self::Multisig(policy)
    }

    /// Borrow the single-signature public key when present.
    #[must_use]
    pub fn single_signatory(&self) -> Option<&PublicKey> {
        match self {
            Self::Single(key) => Some(key),
            Self::Multisig(_) => None,
        }
    }

    /// Borrow the single-signature public key, panicking if the controller is not single-key.
    #[must_use]
    pub fn expect_single_signatory(&self) -> &PublicKey {
        self.single_signatory()
            .expect("account controller must be single-key")
    }

    /// Borrow the multisignature policy when present.
    #[must_use]
    pub fn multisig_policy(&self) -> Option<&MultisigPolicy> {
        match self {
            Self::Single(_) => None,
            Self::Multisig(policy) => Some(policy),
        }
    }
}

impl fmt::Display for AccountController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(_) => write!(f, "single"),
            Self::Multisig(policy) => write!(
                f,
                "multisig(threshold={}, members={})",
                policy.threshold(),
                policy.members().len()
            ),
        }
    }
}

/// Multisignature authorisation policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct MultisigPolicy {
    version: u8,
    threshold: u16,
    members: Vec<MultisigMember>,
}

impl MultisigPolicy {
    /// Current policy version for newly created policies.
    pub const CURRENT_VERSION: u8 = 1;

    /// Construct a new multisignature policy.
    ///
    /// # Errors
    ///
    /// Returns [`MultisigPolicyError`] if the supplied configuration is invalid.
    pub fn new(threshold: u16, members: Vec<MultisigMember>) -> Result<Self, MultisigPolicyError> {
        Self::validate(Self::CURRENT_VERSION, threshold, members)
    }

    /// Construct a policy from serialized components.
    ///
    /// # Errors
    ///
    /// Returns [`MultisigPolicyError`] if the provided version is unsupported or
    /// the contents fail validation.
    pub fn from_serialized(
        version: u8,
        threshold: u16,
        members: Vec<MultisigMember>,
    ) -> Result<Self, MultisigPolicyError> {
        Self::validate(version, threshold, members)
    }

    fn validate(
        version: u8,
        threshold: u16,
        members: Vec<MultisigMember>,
    ) -> Result<Self, MultisigPolicyError> {
        if version != Self::CURRENT_VERSION {
            return Err(MultisigPolicyError::UnsupportedVersion(version));
        }
        if members.is_empty() {
            return Err(MultisigPolicyError::EmptyMembers);
        }
        if threshold == 0 {
            return Err(MultisigPolicyError::ZeroThreshold);
        }

        for member in &members {
            if member.weight() == 0 {
                return Err(MultisigPolicyError::MemberWeightZero);
            }
        }

        let mut keyed: Vec<(Vec<u8>, MultisigMember)> = members
            .into_iter()
            .map(|member| (member.canonical_sort_key(), member))
            .collect();
        keyed.sort_by(|left, right| left.0.cmp(&right.0));

        let mut deduped = Vec::with_capacity(keyed.len());
        let mut previous_key: Option<Vec<u8>> = None;
        for (key, member) in keyed {
            if previous_key.as_ref().is_some_and(|prev| prev == &key) {
                return Err(MultisigPolicyError::DuplicateMember);
            }
            previous_key = Some(key);
            deduped.push(member);
        }
        let members = deduped;

        let total_weight = members
            .iter()
            .map(|member| u32::from(member.weight()))
            .sum::<u32>();

        if u32::from(threshold) > total_weight {
            return Err(MultisigPolicyError::ThresholdExceedsTotal {
                threshold,
                total_weight,
            });
        }

        Ok(Self {
            version,
            threshold,
            members,
        })
    }

    /// Policy version.
    #[must_use]
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Number of approval weight required.
    #[must_use]
    pub fn threshold(&self) -> u16 {
        self.threshold
    }

    /// Borrow the members participating in the policy.
    #[must_use]
    pub fn members(&self) -> &[MultisigMember] {
        &self.members
    }

    /// Compute the aggregate weight across all members.
    #[must_use]
    pub fn total_weight(&self) -> u32 {
        self.members
            .iter()
            .map(|member| u32::from(member.weight()))
            .sum()
    }

    /// Encode the policy into the CTAP2-style canonical CBOR map used for
    /// multisignature fixtures.
    #[must_use]
    pub fn encode_ctap2(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(128);
        cbor_write_map_len(&mut buffer, CTAP2_POLICY_FIELD_COUNT);
        cbor_write_unsigned(&mut buffer, u64::from(CTAP2_POLICY_KEY_VERSION));
        cbor_write_unsigned(&mut buffer, u64::from(self.version()));
        cbor_write_unsigned(&mut buffer, u64::from(CTAP2_POLICY_KEY_THRESHOLD));
        cbor_write_unsigned(&mut buffer, u64::from(self.threshold()));
        cbor_write_unsigned(&mut buffer, u64::from(CTAP2_POLICY_KEY_MEMBERS));
        cbor_write_array_len(&mut buffer, self.members().len());

        for member in self.members() {
            cbor_write_map_len(&mut buffer, CTAP2_MEMBER_FIELD_COUNT);
            cbor_write_unsigned(&mut buffer, u64::from(CTAP2_MEMBER_KEY_CURVE));
            let (algorithm, payload) = member.public_key().to_bytes();
            let curve_id = CurveId::try_from_algorithm(algorithm)
                .expect("multisig members validate curve support at construction");
            cbor_write_unsigned(&mut buffer, u64::from(curve_id.as_u8()));
            cbor_write_unsigned(&mut buffer, u64::from(CTAP2_MEMBER_KEY_WEIGHT));
            cbor_write_unsigned(&mut buffer, u64::from(member.weight()));
            cbor_write_unsigned(&mut buffer, u64::from(CTAP2_MEMBER_KEY_KEY_BYTES));
            cbor_write_bytes(&mut buffer, payload);
        }

        buffer
    }

    /// Compute the canonical Blake2b-256 digest (personalised) over the
    /// CTAP2-encoded policy.
    #[must_use]
    pub fn digest_blake2b256(&self) -> [u8; CTAP2_POLICY_DIGEST_LEN] {
        let encoded = self.encode_ctap2();
        let mut hasher =
            Blake2bMac::<U32>::new_with_salt_and_personal(&[], &[], CTAP2_POLICY_PERSONALISATION)
                .expect("personalised Blake2b parameters must be valid");
        Mac::update(&mut hasher, &encoded);
        let mut output = [0u8; CTAP2_POLICY_DIGEST_LEN];
        let tag = hasher.finalize().into_bytes();
        output.copy_from_slice(&tag);
        output
    }
}

/// Participant in a multisignature policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct MultisigMember {
    public_key: PublicKey,
    weight: u16,
}

impl MultisigMember {
    /// Construct a new multisignature member.
    ///
    /// # Errors
    ///
    /// Returns [`MultisigPolicyError::MemberWeightZero`] if `weight` is zero.
    pub fn new(public_key: PublicKey, weight: u16) -> Result<Self, MultisigPolicyError> {
        if weight == 0 {
            return Err(MultisigPolicyError::MemberWeightZero);
        }
        let algorithm = public_key.algorithm();
        CurveId::try_from_algorithm(algorithm)
            .map_err(|_| MultisigPolicyError::UnsupportedCurve(algorithm))?;
        Ok(Self { public_key, weight })
    }

    /// Borrow the member public key.
    #[must_use]
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Weight contributed by this member.
    #[must_use]
    pub fn weight(&self) -> u16 {
        self.weight
    }

    /// Signing algorithm used by the member.
    #[must_use]
    pub fn algorithm(&self) -> Algorithm {
        self.public_key.algorithm()
    }

    fn canonical_sort_key(&self) -> Vec<u8> {
        let (algorithm, payload) = self.public_key.to_bytes();
        let mut key = Vec::with_capacity(algorithm.as_static_str().len() + 1 + payload.len());
        key.extend_from_slice(algorithm.as_static_str().as_bytes());
        key.push(0);
        key.extend_from_slice(payload);
        key
    }
}

impl TryFrom<(&PublicKey, u16)> for MultisigMember {
    type Error = MultisigPolicyError;

    fn try_from(value: (&PublicKey, u16)) -> Result<Self, Self::Error> {
        Self::new(value.0.clone(), value.1)
    }
}

impl TryFrom<(PublicKey, u16)> for MultisigMember {
    type Error = MultisigPolicyError;

    fn try_from(value: (PublicKey, u16)) -> Result<Self, Self::Error> {
        Self::new(value.0, value.1)
    }
}

/// Errors raised while constructing multisignature policies.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MultisigPolicyError {
    /// Multisignature policies require at least one member.
    #[error("multisig policy requires at least one member")]
    EmptyMembers,
    /// Threshold cannot be zero.
    #[error("multisig threshold must be at least 1")]
    ZeroThreshold,
    /// Member weight must be positive.
    #[error("multisig member weight must be at least 1")]
    MemberWeightZero,
    /// Multisig member uses an unsupported signing curve.
    #[error("multisig member uses unsupported curve: {0}")]
    UnsupportedCurve(Algorithm),
    /// Duplicate member encountered.
    #[error("duplicate multisig member public key")]
    DuplicateMember,
    /// Threshold exceeds the aggregate member weight.
    #[error("multisig threshold {threshold} exceeds total member weight {total_weight}")]
    ThresholdExceedsTotal {
        /// Required approval weight.
        threshold: u16,
        /// Aggregate weight contributed by members.
        total_weight: u32,
    },
    /// Policy version is not supported.
    #[error("unsupported multisig policy version: {0}")]
    UnsupportedVersion(u8),
}

const CTAP2_POLICY_FIELD_COUNT: usize = 3;
const CTAP2_MEMBER_FIELD_COUNT: usize = 3;

const CTAP2_POLICY_KEY_VERSION: u8 = 0x01;
const CTAP2_POLICY_KEY_THRESHOLD: u8 = 0x02;
const CTAP2_POLICY_KEY_MEMBERS: u8 = 0x03;

const CTAP2_MEMBER_KEY_CURVE: u8 = 0x01;
const CTAP2_MEMBER_KEY_WEIGHT: u8 = 0x02;
const CTAP2_MEMBER_KEY_KEY_BYTES: u8 = 0x03;

const CTAP2_POLICY_DIGEST_LEN: usize = 32;
const CTAP2_POLICY_PERSONALISATION: &[u8] = b"iroha-ms-policy";

fn cbor_write_unsigned(buffer: &mut Vec<u8>, value: u64) {
    match value {
        0..=23 => buffer.push(u8::try_from(value).expect("value fits u8")),
        24..=0xFF => {
            buffer.push(0x18);
            buffer.push(u8::try_from(value).expect("value fits u8"));
        }
        0x100..=0xFFFF => {
            buffer.push(0x19);
            let truncated = u16::try_from(value).expect("value fits u16");
            buffer.extend_from_slice(&truncated.to_be_bytes());
        }
        0x1_0000..=0xFFFF_FFFF => {
            buffer.push(0x1A);
            let truncated = u32::try_from(value).expect("value fits u32");
            buffer.extend_from_slice(&truncated.to_be_bytes());
        }
        _ => {
            buffer.push(0x1B);
            buffer.extend_from_slice(&value.to_be_bytes());
        }
    }
}

fn cbor_write_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) {
    cbor_write_len(buffer, 0b010, bytes.len());
    buffer.extend_from_slice(bytes);
}

fn cbor_write_map_len(buffer: &mut Vec<u8>, len: usize) {
    cbor_write_len(buffer, 0b101, len);
}

fn cbor_write_array_len(buffer: &mut Vec<u8>, len: usize) {
    cbor_write_len(buffer, 0b100, len);
}

fn cbor_write_len(buffer: &mut Vec<u8>, major: u8, len: usize) {
    debug_assert!(major <= 0b111, "major type must fit in 3 bits");
    let base = major << 5;
    let value = len as u64;
    match value {
        0..=23 => buffer.push(base | u8::try_from(value).expect("length fits u8")),
        24..=0xFF => {
            buffer.push(base | 24);
            buffer.push(u8::try_from(value).expect("length fits u8"));
        }
        0x100..=0xFFFF => {
            buffer.push(base | 25);
            let truncated = u16::try_from(value).expect("length fits u16");
            buffer.extend_from_slice(&truncated.to_be_bytes());
        }
        0x1_0000..=0xFFFF_FFFF => {
            buffer.push(base | 26);
            let truncated = u32::try_from(value).expect("length fits u32");
            buffer.extend_from_slice(&truncated.to_be_bytes());
        }
        _ => {
            buffer.push(base | 27);
            buffer.extend_from_slice(&value.to_be_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;

    use super::*;

    #[test]
    fn multisig_members_require_positive_weight() {
        let key = KeyPair::random().public_key().clone();
        assert_eq!(
            MultisigMember::new(key, 0).unwrap_err(),
            MultisigPolicyError::MemberWeightZero
        );
    }

    #[test]
    fn multisig_members_accept_supported_curve() {
        let (public_key, _) = KeyPair::random_with_algorithm(Algorithm::Secp256k1).into_parts();
        let member = MultisigMember::new(public_key.clone(), 1).expect("member must be valid");
        assert_eq!(member.public_key(), &public_key);
    }

    #[test]
    fn multisig_policy_enforces_threshold() {
        let member_keys: Vec<MultisigMember> = (0..3)
            .map(|_| {
                MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member")
            })
            .collect();
        let err = MultisigPolicy::new(4, member_keys.clone()).unwrap_err();
        assert_eq!(
            err,
            MultisigPolicyError::ThresholdExceedsTotal {
                threshold: 4,
                total_weight: 3
            }
        );

        let policy = MultisigPolicy::new(2, member_keys).expect("policy");
        assert_eq!(policy.threshold(), 2);
        assert_eq!(policy.total_weight(), 3);
        assert_eq!(policy.version(), MultisigPolicy::CURRENT_VERSION);
    }

    #[test]
    fn multisig_policy_rejects_duplicates() {
        let key = KeyPair::random().public_key().clone();
        let members = vec![
            MultisigMember::new(key.clone(), 1).expect("member"),
            MultisigMember::new(key, 1).expect("member"),
        ];
        assert_eq!(
            MultisigPolicy::new(1, members).unwrap_err(),
            MultisigPolicyError::DuplicateMember
        );
    }

    #[test]
    fn multisig_policy_serialized_version_check() {
        let member =
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member");
        assert_eq!(
            MultisigPolicy::from_serialized(2, 1, vec![member.clone()]).unwrap_err(),
            MultisigPolicyError::UnsupportedVersion(2)
        );

        let ok = MultisigPolicy::from_serialized(1, 1, vec![member]).expect("policy");
        assert_eq!(ok.version(), 1);
    }

    #[test]
    fn multisig_policy_ctap2_encoding_and_digest_are_stable() {
        let members: Vec<MultisigMember> = (0..3)
            .map(|seed| {
                let seed_byte = u8::try_from(seed).expect("seed fits u8");
                let bytes = vec![seed_byte; 32];
                let (public_key, _) = KeyPair::from_seed(bytes, Algorithm::Ed25519).into_parts();
                MultisigMember::new(public_key, seed + 1).expect("member")
            })
            .collect();
        let policy = MultisigPolicy::new(3, members).expect("policy");

        let encoded = policy.encode_ctap2();
        assert!(
            !encoded.is_empty(),
            "CTAP2 encoding must produce a non-empty payload"
        );
        assert_eq!(
            encoded[0], 0xA3,
            "CTAP2 encoding should begin with a 3-field map header"
        );

        let digest = policy.digest_blake2b256();
        assert_ne!(
            digest, [0u8; CTAP2_POLICY_DIGEST_LEN],
            "digest must not be zeroed"
        );
        assert_eq!(
            digest,
            policy.digest_blake2b256(),
            "digest computation must be deterministic"
        );
    }
}

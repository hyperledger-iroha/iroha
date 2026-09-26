//! Canonical provider-admission council policy and exact signed-claim validation.
//!
//! This value is a candidate for governed State; constructing or decoding it does not enact it.
//! Core must read the active policy from the same transaction State as the provider admission head,
//! enforce its transition, and bind its result to finalized State/Kura before issuing a grant.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::{
    DeserializePayload, SerializePayload,
    codec::{Decode, Encode},
    core::{self as ncore, DecodeFromSlice},
};
use sorafs_manifest::provider_admission::{
    ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeV1,
};
use thiserror::Error;

pub mod governance;

/// Sole first-release policy layout.
pub const PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1: u16 = 1;
/// Maximum distinct strong council signers retained in one policy.
pub const PROVIDER_ADMISSION_COUNCIL_MAX_SIGNERS_V1: usize = 32;
/// Domain separator for the canonical governed council-policy digest.
pub const PROVIDER_ADMISSION_COUNCIL_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.provider-admission.council-policy.v1\0";

/// Bounded, predecessor-linked council policy whose digest is claimed by signed admission events.
///
/// Native governance must enact this value before it can authorize an event. A caller-supplied
/// policy, even one with valid signatures, is not a finalized State/Kura trust root.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::ProviderAdmissionCouncilPolicyV1"
)]
#[norito(deny_unknown_fields)]
pub struct ProviderAdmissionCouncilPolicyV1 {
    /// Exact genesis-derived network identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub network_id: [u8; 32],
    /// Nonzero, network-scoped council policy identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub policy_id: [u8; 32],
    /// Sole first-release layout version.
    pub version: u16,
    /// Monotonic revision beginning at one.
    pub revision: u64,
    /// Exact previous policy digest; absent only for revision one.
    #[norito(json = "crate::json_helpers::fixed_bytes::option")]
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Strictly increasing canonical strong Ed25519 public keys.
    #[norito(json = "crate::sorafs::provider_admission::bounded_council_signers_json")]
    pub trusted_signers: Vec<[u8; 32]>,
    /// Distinct trusted signatures required for one admission transition.
    pub signature_threshold: u8,
    /// Whether admission transitions under this policy are suspended.
    pub paused: bool,
}

// The JSON helper streams signers and each 32-byte key with fixed storage.
// It rejects the 33rd entry before growing the outer vector or parsing its body.
mod bounded_council_signers_json {
    use super::PROVIDER_ADMISSION_COUNCIL_MAX_SIGNERS_V1;
    use norito::json::{self, JsonDeserialize, Parser, SeqVisitor};

    struct Signer([u8; 32]);

    impl JsonDeserialize for Signer {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
            let mut sequence = SeqVisitor::new(parser)?;
            let mut bytes = [0_u8; 32];
            for byte in &mut bytes {
                *byte = sequence.next_element::<u8>()?.ok_or_else(|| {
                    json::Error::Message("expected exactly 32 signer-key bytes".into())
                })?;
            }
            if !sequence.is_finished() {
                return Err(json::Error::Message(
                    "expected exactly 32 signer-key bytes".into(),
                ));
            }
            sequence.finish()?;
            Ok(Self(bytes))
        }
    }

    pub(super) fn serialize(value: &[[u8; 32]], out: &mut String) {
        crate::json_helpers::fixed_bytes::vec::serialize(value, out);
    }

    pub(super) fn serialize_bounded(
        value: &[[u8; 32]],
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        crate::json_helpers::fixed_bytes::vec::serialize_bounded(value, out)
    }

    pub(super) fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<[u8; 32]>, json::Error> {
        let mut sequence = SeqVisitor::new(parser)?;
        let mut signers = Vec::new();
        while !sequence.is_finished() {
            if signers.len() == PROVIDER_ADMISSION_COUNCIL_MAX_SIGNERS_V1 {
                return Err(json::Error::Message(
                    "provider admission council signer count exceeds 32".into(),
                ));
            }
            let signer = sequence
                .next_element::<Signer>()?
                .ok_or_else(|| json::Error::Message("expected a council signer key".into()))?;
            signers.push(signer.0);
        }
        sequence.finish()?;
        Ok(signers)
    }
}

// Norito's packed hybrid derive classifies self-delimiting fields by their last
// syntactic type segment. Keep this private segment named Vec so the bounded
// decoder leaves the public policy's wire layout and schema unchanged.
mod bounded_council_signers {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct Vec<T>(pub(super) std::vec::Vec<T>);

    impl SerializePayload for Vec<[u8; 32]> {
        fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
            self.0.serialize(writer)
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            self.0.encoded_len_exact()
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }
    }

    impl<'de> DeserializePayload<'de> for Vec<[u8; 32]> {
        fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
            Self::try_deserialize(archived).expect("bounded council signer archive")
        }

        fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
            let bytes = ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
            let (value, used) = Self::decode_from_slice(bytes)?;
            // Packed self-delimiting fields may be followed by other fields in
            // this context. Report the exact prefix without requiring bytes.len().
            ncore::note_payload_access(bytes, used);
            Ok(value)
        }
    }

    impl<'a> DecodeFromSlice<'a> for Vec<[u8; 32]> {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
            let (count, _) = ncore::inspect_seq_len_slice(bytes)?;
            if count > PROVIDER_ADMISSION_COUNCIL_MAX_SIGNERS_V1 {
                return Err(ncore::Error::Message(
                    "provider admission council signer count exceeds 32".into(),
                ));
            }
            let (signers, used) =
                <std::vec::Vec<[u8; 32]> as DecodeFromSlice>::decode_from_slice(bytes)?;
            Ok((Self(signers), used))
        }
    }
}

// The wire projection changes only the signer field's decoder. Its field
// order and serialized payload match the public policy under every layout.
#[derive(Encode, Decode)]
#[norito(decode_from_slice)]
struct ProviderAdmissionCouncilPolicyDecodedV1 {
    network_id: [u8; 32],
    policy_id: [u8; 32],
    version: u16,
    revision: u64,
    predecessor_policy_digest: Option<[u8; 32]>,
    trusted_signers: bounded_council_signers::Vec<[u8; 32]>,
    signature_threshold: u8,
    paused: bool,
}

impl From<ProviderAdmissionCouncilPolicyDecodedV1> for ProviderAdmissionCouncilPolicyV1 {
    fn from(value: ProviderAdmissionCouncilPolicyDecodedV1) -> Self {
        Self {
            network_id: value.network_id,
            policy_id: value.policy_id,
            version: value.version,
            revision: value.revision,
            predecessor_policy_digest: value.predecessor_policy_digest,
            trusted_signers: value.trusted_signers.0,
            signature_threshold: value.signature_threshold,
            paused: value.paused,
        }
    }
}

impl<'de> DeserializePayload<'de> for ProviderAdmissionCouncilPolicyV1 {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("bounded provider admission council policy archive")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = ProviderAdmissionCouncilPolicyDecodedV1::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(value.into())
    }
}

impl<'a> DecodeFromSlice<'a> for ProviderAdmissionCouncilPolicyV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (value, used) = ProviderAdmissionCouncilPolicyDecodedV1::decode_from_slice(bytes)?;
        Ok((value.into(), used))
    }
}

impl ProviderAdmissionCouncilPolicyV1 {
    /// Validate exact shape, bounded canonical key order, and strong key material.
    ///
    /// # Errors
    ///
    /// Refuses zero identity, invalid lineage, weak/duplicate/unsorted keys, and impossible quorum.
    pub fn validate(&self) -> Result<(), ProviderAdmissionCouncilPolicyValidationErrorV1> {
        if self.version != PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1 {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::UnsupportedVersion);
        }
        if self.network_id == [0; 32] || self.policy_id == [0; 32] {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::ZeroIdentity);
        }
        match (self.revision, self.predecessor_policy_digest) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::UnexpectedPredecessor);
            }
            (2.., Some(digest)) if digest != [0; 32] => {}
            _ => return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::InvalidLineage),
        }
        if self.trusted_signers.is_empty()
            || self.trusted_signers.len() > PROVIDER_ADMISSION_COUNCIL_MAX_SIGNERS_V1
        {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::SignerCount);
        }
        if self.signature_threshold == 0
            || usize::from(self.signature_threshold) > self.trusted_signers.len()
        {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::Threshold);
        }
        for (index, signer) in self.trusted_signers.iter().enumerate() {
            if index > 0 && self.trusted_signers[index - 1] >= *signer {
                return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::SignerOrder);
            }
            if iroha_crypto::ed25519_parse_public_key(signer).is_err() {
                return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::InvalidSigner);
            }
        }
        Ok(())
    }

    /// Compute the canonical domain-separated digest after validating the complete policy.
    ///
    /// # Errors
    ///
    /// Refuses invalid policy material or a canonical Norito encoding failure.
    pub fn canonical_digest(
        &self,
    ) -> Result<[u8; 32], ProviderAdmissionCouncilPolicyValidationErrorV1> {
        self.validate()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PROVIDER_ADMISSION_COUNCIL_POLICY_DIGEST_DOMAIN_V1);
        norito::core::write_canonical_to_writer(self, &mut Blake3Writer(&mut hasher))
            .map_err(|_| ProviderAdmissionCouncilPolicyValidationErrorV1::CanonicalEncoding)?;
        Ok(*hasher.finalize().as_bytes())
    }

    /// Verify the immediate, same-network and same-identity successor policy.
    ///
    /// # Errors
    ///
    /// Refuses skipped revisions, predecessor substitution, or network/identity rotation.
    pub fn validate_successor(
        &self,
        previous: &Self,
    ) -> Result<(), ProviderAdmissionCouncilPolicyValidationErrorV1> {
        self.validate()?;
        previous.validate()?;
        if self.network_id != previous.network_id || self.policy_id != previous.policy_id {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::IdentityChanged);
        }
        if previous.revision.checked_add(1) != Some(self.revision)
            || self.predecessor_policy_digest != Some(previous.canonical_digest()?)
        {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::PredecessorMismatch);
        }
        Ok(())
    }

    /// Verify one signed envelope's policy claims, quorum, network, and deterministic lifetime.
    ///
    /// This only validates supplied values. Core must obtain this policy from governed State and
    /// check the exact current provider head/tombstone before this result can affect authority.
    /// `now_unix_secs` must be the deterministic committing block time in consensus execution.
    ///
    /// # Errors
    ///
    /// Refuses a paused policy, stale or substituted claims, invalid quorum, or expired envelope.
    pub fn verify_envelope_policy_claim(
        &self,
        envelope: &ProviderAdmissionEnvelopeV1,
        now_unix_secs: u64,
    ) -> Result<(), ProviderAdmissionCouncilPolicyValidationErrorV1> {
        self.validate()?;
        if self.paused {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::Paused);
        }
        if envelope.network_id != self.network_id || envelope.policy_id != self.policy_id {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::ClaimMismatch);
        }
        // The signed policy revision names this governed row's revision.
        if envelope.policy_revision != self.revision {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::ClaimMismatch);
        }
        if envelope.policy_digest != self.canonical_digest()? {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::ClaimMismatch);
        }
        if now_unix_secs < envelope.issued_at || now_unix_secs >= envelope.retention_epoch {
            return Err(ProviderAdmissionCouncilPolicyValidationErrorV1::Expired);
        }
        let policy = ProviderAdmissionCouncilPolicy::new(
            self.trusted_signers.iter().copied(),
            usize::from(self.signature_threshold),
        )
        .map_err(|_| ProviderAdmissionCouncilPolicyValidationErrorV1::InvalidSigner)?;
        sorafs_manifest::provider_admission::verify_envelope(envelope, &policy)
            .map_err(|_| ProviderAdmissionCouncilPolicyValidationErrorV1::InvalidEnvelope)?;
        Ok(())
    }
}

struct Blake3Writer<'a>(&'a mut blake3::Hasher);
impl std::io::Write for Blake3Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Fail-closed council-policy and signed-claim validation errors.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ProviderAdmissionCouncilPolicyValidationErrorV1 {
    /// A retired or unknown layout was supplied.
    #[error("unsupported provider admission council policy version")]
    UnsupportedVersion,
    /// The network or council identity is zero.
    #[error("provider admission council identity must be nonzero")]
    ZeroIdentity,
    /// The first policy revision has a predecessor.
    #[error("initial provider admission council policy must have no predecessor")]
    UnexpectedPredecessor,
    /// Revision or predecessor digest is invalid.
    #[error("provider admission council policy requires a one-based revision and predecessor")]
    InvalidLineage,
    /// The signer vector violates its hard ceiling.
    #[error("provider admission council signer count is outside its bound")]
    SignerCount,
    /// Quorum is zero or exceeds the distinct signer count.
    #[error("provider admission council signature threshold is unsatisfiable")]
    Threshold,
    /// Keys are not strictly increasing.
    #[error("provider admission council signer order is not canonical")]
    SignerOrder,
    /// A signer is malformed, inert, or weak.
    #[error("provider admission council signer must be a strong Ed25519 key")]
    InvalidSigner,
    /// The canonical policy cannot be encoded.
    #[error("provider admission council policy encoding failed")]
    CanonicalEncoding,
    /// A successor changed immutable scope.
    #[error("provider admission council successor changed its network or identity")]
    IdentityChanged,
    /// A successor skipped a revision or substituted its predecessor.
    #[error("provider admission council predecessor does not match")]
    PredecessorMismatch,
    /// The governed policy is paused.
    #[error("provider admission council policy is paused")]
    Paused,
    /// An envelope claims a different council policy or network.
    #[error("provider admission envelope council claim does not match")]
    ClaimMismatch,
    /// The deterministic execution time is outside the envelope lifetime.
    #[error("provider admission envelope is not current at execution")]
    Expired,
    /// The envelope, signature set, or claimed fields fail validation.
    #[error("provider admission envelope failed council verification")]
    InvalidEnvelope,
}

#[cfg(test)]
mod tests;

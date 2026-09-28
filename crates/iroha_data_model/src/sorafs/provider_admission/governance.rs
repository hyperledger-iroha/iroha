//! Parliament effects carrying the sole canonical signed provider-admission frames.

use super::ProviderAdmissionCouncilPolicyV1;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, sorafs::capacity::ProviderId};
use sorafs_manifest::{
    ProviderAdmissionEnvelopeV1, ProviderAdmissionRenewalV1, ProviderAdmissionRevocationV1,
};

/// Maximum canonical admission material carried by one Parliament effect.
pub const PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1: usize = 1024 * 1024;
/// Maximum retained transitions per provider, including a reserved terminal revocation slot.
pub const PROVIDER_ADMISSION_MAX_REVISIONS_V1: u64 = 1024;
/// Maximum distinct provider identities, including terminal tombstones.
pub const PROVIDER_ADMISSION_MAX_PROVIDERS_V1: u64 = 4096;
/// Maximum retained non-emergency history bytes; revocations have a separate reserved allowance.
pub const PROVIDER_ADMISSION_HISTORY_MAX_BYTES_V1: u64 = 128 * 1024 * 1024;
/// Maximum canonical terminal revocation frame.
pub const PROVIDER_ADMISSION_MAX_REVOCATION_BYTES_V1: usize = 16 * 1024;
/// Maximum provider entries in the sole signed-genesis admission initialization.
pub const PROVIDER_ADMISSION_GENESIS_MAX_PROVIDERS_V1: usize = 64;
/// Maximum complete canonical signed-genesis initialization instruction.
pub const PROVIDER_ADMISSION_GENESIS_MAX_BYTES_V1: usize = 4 * 1024 * 1024;

/// Network-independent council template bound to the actual network only during signed genesis.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::governance::InitialProviderAdmissionCouncilV1"
)]
#[norito(deny_unknown_fields)]
pub struct InitialProviderAdmissionCouncilV1 {
    /// Stable nonzero council identity.
    pub policy_id: [u8; 32],
    /// Strictly sorted distinct strong Ed25519 council keys.
    pub trusted_signers: Vec<[u8; 32]>,
    /// Required distinct council signatures for subsequent native governance effects.
    pub signature_threshold: u8,
}
impl InitialProviderAdmissionCouncilV1 {
    /// Derive the sole revision-one policy for an already-derived genesis network identity.
    ///
    /// # Errors
    /// Rejects a council that cannot form a valid revision-one policy.
    pub fn bind(
        &self,
        network_id: [u8; 32],
    ) -> Result<
        ProviderAdmissionCouncilPolicyV1,
        super::ProviderAdmissionCouncilPolicyValidationErrorV1,
    > {
        let policy = ProviderAdmissionCouncilPolicyV1 {
            network_id,
            policy_id: self.policy_id,
            version: 1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: self.trusted_signers.clone(),
            signature_threshold: self.signature_threshold,
            paused: false,
        };
        policy.validate()?;
        Ok(policy)
    }
}
/// Exact initial owner and network-independent material authenticated by signed genesis.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::governance::InitialProviderAdmissionV1"
)]
#[norito(deny_unknown_fields)]
pub struct InitialProviderAdmissionV1 {
    /// Registered universal account owning this provider at genesis.
    pub owner: crate::account::AccountId,
    /// Complete canonical `ProviderAdmissionGenesisMaterialV1` frame; never a council envelope.
    pub material: Vec<u8>,
}

/// Closed Parliament effect; byte fields contain canonical Norito frames, never alternate layouts.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::governance::ProviderAdmissionGovernanceActionV1"
)]
#[norito(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProviderAdmissionGovernanceActionV1 {
    /// Enact the exact initial or predecessor-linked council policy.
    ConfigureCouncil(Vec<u8>),
    /// Admit a provider with no retained admission history.
    Admit(Vec<u8>),
    /// Apply a signed successor to the exact current active envelope.
    Renew(Vec<u8>),
    /// Permanently tombstone the exact current admission event.
    Revoke(Vec<u8>),
}

/// Malformed, oversized, noncanonical, or internally inconsistent admission effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid canonical provider admission effect")]
pub struct InvalidProviderAdmissionEffectV1;

/// Decode one bounded exact canonical frame.
///
/// # Errors
/// Rejects empty, oversized or noncanonical frames.
pub fn decode_frame<T>(bytes: &[u8]) -> Result<T, InvalidProviderAdmissionEffectV1>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1 {
        return Err(InvalidProviderAdmissionEffectV1);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            65536,
            PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1,
            65536,
            4 * PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1,
            64,
        ),
    )
    .map_err(|_| InvalidProviderAdmissionEffectV1)
}

impl ProviderAdmissionGovernanceActionV1 {
    /// Validate the complete canonical material and return its provider, or `None` for council policy.
    ///
    /// # Errors
    /// Rejects incomplete or noncanonical admission material.
    pub fn provider_id(&self) -> Result<Option<ProviderId>, InvalidProviderAdmissionEffectV1> {
        let id = match self {
            Self::ConfigureCouncil(bytes) => {
                decode_frame::<ProviderAdmissionCouncilPolicyV1>(bytes)?
                    .validate()
                    .map_err(|_| InvalidProviderAdmissionEffectV1)?;
                return Ok(None);
            }
            Self::Admit(bytes) => {
                let envelope: ProviderAdmissionEnvelopeV1 = decode_frame(bytes)?;
                envelope
                    .validate()
                    .map_err(|_| InvalidProviderAdmissionEffectV1)?;
                if envelope.admission_revision != 1 {
                    return Err(InvalidProviderAdmissionEffectV1);
                }
                envelope.proposal.provider_id
            }
            Self::Renew(bytes) => {
                let renewal: ProviderAdmissionRenewalV1 = decode_frame(bytes)?;
                renewal
                    .envelope
                    .validate()
                    .map_err(|_| InvalidProviderAdmissionEffectV1)?;
                if renewal.provider_id != renewal.envelope.proposal.provider_id {
                    return Err(InvalidProviderAdmissionEffectV1);
                }
                *renewal.provider_id()
            }
            Self::Revoke(bytes) => {
                if bytes.len() > PROVIDER_ADMISSION_MAX_REVOCATION_BYTES_V1 {
                    return Err(InvalidProviderAdmissionEffectV1);
                }
                let revocation: ProviderAdmissionRevocationV1 = decode_frame(bytes)?;
                sorafs_manifest::provider_admission::verify_revocation_signatures_untrusted_signers(&revocation)
                    .map_err(|_| InvalidProviderAdmissionEffectV1)?;
                revocation.provider_id
            }
        };
        if id == [0; 32] {
            return Err(InvalidProviderAdmissionEffectV1);
        }
        Ok(Some(ProviderId::new(id)))
    }
}

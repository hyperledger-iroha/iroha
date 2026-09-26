//! Parliament effects carrying the sole canonical signed provider-admission frames.

use super::ProviderAdmissionCouncilPolicyV1;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, sorafs::capacity::ProviderId};
use sorafs_manifest::{ProviderAdmissionEnvelopeV1, ProviderAdmissionRenewalV1, ProviderAdmissionRevocationV1};

/// Maximum canonical admission material carried by one Parliament effect.
pub const PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1: usize = 1024 * 1024;
/// Maximum retained transitions per provider, including a reserved terminal revocation slot.
pub const PROVIDER_ADMISSION_MAX_REVISIONS_V1: u64 = 1024;
/// Maximum distinct provider identities, including terminal tombstones.
pub const PROVIDER_ADMISSION_MAX_PROVIDERS_V1: u64 = 4096;

/// Closed Parliament effect; byte fields contain canonical Norito frames, never alternate layouts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, norito::codec::Encode,
    norito::codec::Decode, iroha_schema::IntoSchema, DeriveJsonSerialize,
    DeriveJsonDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::provider_admission::governance::ProviderAdmissionGovernanceActionV1")]
#[norito(tag = "action", content = "value", rename_all = "snake_case", deny_unknown_fields)]
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
pub fn decode_frame<T>(bytes: &[u8]) -> Result<T, InvalidProviderAdmissionEffectV1>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1 {
        return Err(InvalidProviderAdmissionEffectV1);
    }
    norito::decode_canonical_with_limits(bytes, norito::DecodeLimits::new(
        65536, PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1, 65536,
        4 * PROVIDER_ADMISSION_MAX_FRAME_BYTES_V1, 64,
    )).map_err(|_| InvalidProviderAdmissionEffectV1)
}

impl ProviderAdmissionGovernanceActionV1 {
    /// Validate the complete canonical material and return its provider, or `None` for council policy.
    pub fn provider_id(&self) -> Result<Option<ProviderId>, InvalidProviderAdmissionEffectV1> {
        let id = match self {
            Self::ConfigureCouncil(bytes) => {
                decode_frame::<ProviderAdmissionCouncilPolicyV1>(bytes)?.validate()
                    .map_err(|_| InvalidProviderAdmissionEffectV1)?;
                return Ok(None);
            }
            Self::Admit(bytes) => {
                let envelope: ProviderAdmissionEnvelopeV1 = decode_frame(bytes)?;
                envelope.validate().map_err(|_| InvalidProviderAdmissionEffectV1)?;
                if envelope.admission_revision != 1 { return Err(InvalidProviderAdmissionEffectV1); }
                envelope.proposal.provider_id
            }
            Self::Renew(bytes) => {
                let renewal: ProviderAdmissionRenewalV1 = decode_frame(bytes)?;
                renewal.envelope.validate().map_err(|_| InvalidProviderAdmissionEffectV1)?;
                *renewal.provider_id()
            }
            Self::Revoke(bytes) => {
                let revocation: ProviderAdmissionRevocationV1 = decode_frame(bytes)?;
                revocation.validate().map_err(|_| InvalidProviderAdmissionEffectV1)?;
                revocation.provider_id
            }
        };
        if id == [0; 32] { return Err(InvalidProviderAdmissionEffectV1); }
        Ok(Some(ProviderId::new(id)))
    }
}

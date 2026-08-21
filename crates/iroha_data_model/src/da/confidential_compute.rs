//! Confidential-compute lane policy.
//!
//! These types describe the confidentiality posture for computation lanes that must keep payloads
//! off the public DA surface while still advertising deterministic availability and proof
//! parameters. Policies are carried directly by the typed lane catalog and enforced during DA
//! validation.
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{collections::BTreeSet, fmt, num::NonZeroU32, str::FromStr};
use thiserror::Error;
/// Confidential-compute protection mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum ConfidentialComputeMechanism {
    /// Payload is encrypted (e.g., envelope-encrypted with a rotation key).
    Encryption,
    /// Payload is split across shares (e.g., SMPC/secret sharing).
    SecretSharing,
}
impl ConfidentialComputeMechanism {
    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Encryption => "encryption",
            Self::SecretSharing => "secret_sharing",
        }
    }
}
impl fmt::Display for ConfidentialComputeMechanism {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
impl FromStr for ConfidentialComputeMechanism {
    type Err = ConfidentialComputeMechanismParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "encryption" => Ok(Self::Encryption),
            "secret_sharing" => Ok(Self::SecretSharing),
            _ => Err(ConfidentialComputeMechanismParseError(value.to_owned())),
        }
    }
}
/// Error returned for a non-canonical confidential-compute mechanism label.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid confidential-compute mechanism `{0}`")]
pub struct ConfidentialComputeMechanismParseError(pub String);

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ConfidentialComputeMechanism {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }

    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_str(), out)
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ConfidentialComputeMechanism {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser
            .parse_string()?
            .parse()
            .map_err(|error: ConfidentialComputeMechanismParseError| {
                norito::json::Error::Message(error.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::ConfidentialComputeMechanism;

    #[test]
    fn metadata_mechanism_labels_are_canonical() {
        assert_eq!(
            "encryption".parse::<ConfidentialComputeMechanism>(),
            Ok(ConfidentialComputeMechanism::Encryption)
        );
        assert_eq!(
            "secret_sharing".parse::<ConfidentialComputeMechanism>(),
            Ok(ConfidentialComputeMechanism::SecretSharing)
        );
        for retired_alias in ["encrypt", "aes-gcm", "secret-sharing", "smpc"] {
            assert_eq!(
                retired_alias.parse::<ConfidentialComputeMechanism>(),
                Err(super::ConfidentialComputeMechanismParseError(
                    retired_alias.to_owned()
                )),
                "non-canonical mechanism alias `{retired_alias}` must fail closed"
            );
        }
    }
}
/// Lane-level confidentiality policy committed directly by the lane catalog.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[norito(deny_unknown_fields)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ConfidentialComputePolicy {
    /// Selected confidentiality mechanism.
    pub mechanism: ConfidentialComputeMechanism,
    /// Key rotation or share version expected for the lane.
    pub key_version: NonZeroU32,
    /// Allowed audiences (roles, operators, or labels) permitted to fetch the payload.
    pub allowed_audiences: BTreeSet<String>,
}
impl ConfidentialComputePolicy {
    /// Construct a new policy.
    #[must_use]
    pub fn new(
        mechanism: ConfidentialComputeMechanism,
        key_version: NonZeroU32,
        allowed_audiences: BTreeSet<String>,
    ) -> Self {
        Self {
            mechanism,
            key_version,
            allowed_audiences,
        }
    }
}

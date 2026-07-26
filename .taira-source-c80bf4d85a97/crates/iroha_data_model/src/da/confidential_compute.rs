//! Confidential-compute lane policy metadata.
//!
//! These types describe the confidentiality posture for computation lanes that
//! must keep payloads off the public DA surface while still advertising
//! deterministic availability and proof parameters. Policies are derived from
//! lane metadata and enforced during DA validation.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Confidential-compute protection mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum ConfidentialComputeMechanism {
    /// Payload is encrypted (e.g., envelope-encrypted with a rotation key).
    Encryption,
    /// Payload is split across shares (e.g., SMPC/secret sharing).
    SecretSharing,
}

impl ConfidentialComputeMechanism {
    /// Parse a mechanism string from lane metadata.
    #[must_use]
    pub fn from_metadata_value(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "encryption" | "encrypt" | "aes-gcm" | "aes_gcm" => Some(Self::Encryption),
            "secret_sharing" | "secret-sharing" | "smpc" => Some(Self::SecretSharing),
            _ => None,
        }
    }

    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Encryption => "encryption",
            Self::SecretSharing => "secret_sharing",
        }
    }
}

/// Lane-level confidentiality policy derived from metadata.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ConfidentialComputePolicy {
    /// Selected confidentiality mechanism.
    pub mechanism: ConfidentialComputeMechanism,
    /// Key rotation or share version expected for the lane.
    pub key_version: u32,
    /// Allowed audiences (roles, operators, or labels) permitted to fetch the payload.
    pub allowed_audiences: Vec<String>,
}

impl ConfidentialComputePolicy {
    /// Construct a new policy.
    #[must_use]
    pub fn new(
        mechanism: ConfidentialComputeMechanism,
        key_version: u32,
        allowed_audiences: Vec<String>,
    ) -> Self {
        Self {
            mechanism,
            key_version,
            allowed_audiences,
        }
    }
}

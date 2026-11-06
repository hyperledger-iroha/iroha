//! Canonical bounded payload and receipt for the one-time Taira SoraFS repair.
//!
//! The payload is hash-bound by `runtime::sorafs_repair::SorafsRepairPlanV1` and all four
//! scheduled validators. It executes only under the separately selected repair definition.
//! It is not a threshold-key lifecycle action; that selected definition remains unchanged.
//! See `specs/sorafs/taira_repair_v1.md`.

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId, account::AccountId,
    permission::Permission,
};

/// Maximum initial providers; equal to the signed-genesis initializer bound.
pub const SORAFS_BOOTSTRAP_MAX_PROVIDERS_V1: usize =
    super::provider_admission::governance::PROVIDER_ADMISSION_GENESIS_MAX_PROVIDERS_V1;
/// Maximum first permission grants.
pub const SORAFS_BOOTSTRAP_MAX_GRANTS_V1: usize = 1024;
/// Maximum initial gateway compliance operators.
pub const SORAFS_BOOTSTRAP_MAX_COMPLIANCE_OPERATORS_V1: usize = 16;
/// Maximum canonical payload bytes; within the typed repair certificate's bound.
pub const SORAFS_BOOTSTRAP_MAX_PAYLOAD_BYTES_V1: usize =
    super::provider_admission::governance::PROVIDER_ADMISSION_GENESIS_MAX_BYTES_V1;
/// Role registered for the listed gateway compliance operators.
pub const SORAFS_BOOTSTRAP_COMPLIANCE_OPERATOR_ROLE_V1: &str = "sorafs_gateway_compliance_operator";
/// Unit `SoraFS` permissions whose first holder a bootstrap may establish.
pub const SORAFS_BOOTSTRAP_NETWORK_PERMISSIONS_V1: [&str; 12] = [
    "CanBindSorafsAlias",
    "CanDeclareSorafsCapacity",
    "CanFileSorafsCapacityDispute",
    "CanIssueSorafsReplicationOrder",
    "CanManageSorafsProofOutcomePolicy",
    "CanManageSorafsReputationJournalPolicy",
    "CanRecordSorafsReputationJournal",
    "CanResolveSorafsCapacityDispute",
    "CanSetSorafsPricing",
    "CanSetSorafsReservePolicy",
    "CanSubmitSorafsTelemetry",
    "CanUpsertSorafsProviderCredit",
];
/// Provider-scoped worker permissions a bootstrap may establish for its own providers.
pub const SORAFS_BOOTSTRAP_PROVIDER_PERMISSIONS_V1: [&str; 3] = [
    "CanCompleteSorafsReplicationOrder",
    "CanOperateSorafsRepair",
    "CanRecordSorafsProofOutcome",
];
/// Native State path of the sole bootstrap record.
pub const SORAFS_BOOTSTRAP_RECORD_PATH_V1: &str = "sorafs/post_genesis_bootstrap/record";

const PAYLOAD_DOMAIN_V1: &[u8] = b"iroha.sorafs.post-genesis-bootstrap.payload.v1\0";
const SESSION_DOMAIN_V1: &[u8] = b"iroha.sorafs.post-genesis-bootstrap.session.v1\0";

/// One initial provider: its registered owner and council-signed admission envelope.
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
#[norito_schema(name = "iroha_data_model::sorafs::bootstrap::SorafsBootstrapProviderV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsBootstrapProviderV1 {
    /// Registered universal account bound as the provider owner.
    pub owner: AccountId,
    /// Canonical revision-one `ProviderAdmissionEnvelopeV1` frame signed by the payload council.
    pub envelope: Vec<u8>,
}

/// First grant of one allowlisted `SoraFS` permission.
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
#[norito_schema(name = "iroha_data_model::sorafs::bootstrap::SorafsBootstrapGrantV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsBootstrapGrantV1 {
    /// Registered account receiving the permission.
    pub account: AccountId,
    /// Exact permission from [`SORAFS_BOOTSTRAP_NETWORK_PERMISSIONS_V1`] or
    /// [`SORAFS_BOOTSTRAP_PROVIDER_PERMISSIONS_V1`].
    pub permission: Permission,
}

/// Closed content a `SoraFS`-initializing genesis would have carried.
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
#[norito_schema(name = "iroha_data_model::sorafs::bootstrap::SorafsBootstrapPayloadV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsBootstrapPayloadV1 {
    /// Canonical revision-one `ProviderAdmissionCouncilPolicyV1` frame for this network.
    pub council: Vec<u8>,
    /// Initial providers, strictly ordered by the provider id in their envelopes.
    pub providers: Vec<SorafsBootstrapProviderV1>,
    /// First grants, strictly ordered and unique.
    pub grants: Vec<SorafsBootstrapGrantV1>,
    /// Initial members of the gateway compliance operator role, strictly ordered and unique.
    pub compliance_operators: Vec<AccountId>,
}

/// Retained proof that the sole bootstrap executed on this chain.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::bootstrap::SorafsBootstrapRecordV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsBootstrapRecordV1 {
    /// Exact genesis-derived network identity.
    pub network_id: [u8; 32],
    /// Executing block height, the certificate's effective height.
    pub height: u64,
    /// Executing block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// The certificate's session identity, [`SorafsBootstrapPayloadV1::session_id`].
    pub session_id: [u8; 32],
    /// Digest of the applied payload.
    pub payload_digest: [u8; 32],
    /// Hash of the exact ordered roster whose quorum signed the certificate.
    pub roster_hash: [u8; 32],
}

/// Closed payload failures detected before any State is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SorafsBootstrapPayloadErrorV1 {
    /// The payload is empty, oversized, unordered, duplicated or exceeds an entry bound.
    #[error("SoraFS bootstrap payload is malformed or exceeds its bounds")]
    Shape,
    /// A grant names a permission outside the closed `SoraFS` allowlist.
    #[error("SoraFS bootstrap grant is not an allowlisted SoraFS permission")]
    Permission,
    /// A canonical encoding failed or the bytes are not canonical.
    #[error("SoraFS bootstrap payload is not canonical")]
    Encoding,
}

fn strictly_increasing<T: Ord>(items: &[T]) -> bool {
    items.windows(2).all(|pair| pair[0] < pair[1])
}

impl SorafsBootstrapPayloadV1 {
    /// Check every State-independent invariant: entry presence, bounds, ordering, uniqueness,
    /// the permission allowlist and the canonical size.
    ///
    /// # Errors
    /// Returns the first violated invariant.
    pub fn validate(&self) -> Result<(), SorafsBootstrapPayloadErrorV1> {
        if self.council.is_empty()
            || self.providers.is_empty()
            || self.providers.len() > SORAFS_BOOTSTRAP_MAX_PROVIDERS_V1
            || self.providers.iter().any(|entry| entry.envelope.is_empty())
            || self.grants.len() > SORAFS_BOOTSTRAP_MAX_GRANTS_V1
            || !strictly_increasing(&self.grants)
            || self.compliance_operators.len() > SORAFS_BOOTSTRAP_MAX_COMPLIANCE_OPERATORS_V1
            || !strictly_increasing(&self.compliance_operators)
        {
            return Err(SorafsBootstrapPayloadErrorV1::Shape);
        }
        if self.grants.iter().any(|grant| {
            !SORAFS_BOOTSTRAP_NETWORK_PERMISSIONS_V1.contains(&grant.permission.name())
                && !SORAFS_BOOTSTRAP_PROVIDER_PERMISSIONS_V1.contains(&grant.permission.name())
        }) {
            return Err(SorafsBootstrapPayloadErrorV1::Permission);
        }
        if norito::canonical_frame_len(self).map_err(|_| SorafsBootstrapPayloadErrorV1::Encoding)?
            > SORAFS_BOOTSTRAP_MAX_PAYLOAD_BYTES_V1
        {
            return Err(SorafsBootstrapPayloadErrorV1::Shape);
        }
        Ok(())
    }

    /// Exact canonical representation of the public bootstrap content.
    ///
    /// # Errors
    /// Returns [`SorafsBootstrapPayloadErrorV1::Encoding`] if the payload does not encode.
    pub fn to_public_state(&self) -> Result<Vec<u8>, SorafsBootstrapPayloadErrorV1> {
        norito::encode_canonical(self).map_err(|_| SorafsBootstrapPayloadErrorV1::Encoding)
    }

    /// Decode the sole canonical representation of the public bootstrap content.
    ///
    /// # Errors
    /// Rejects bytes that do not decode or do not re-encode identically.
    pub fn from_public_state(bytes: &[u8]) -> Result<Self, SorafsBootstrapPayloadErrorV1> {
        let payload: Self = norito::decode_from_bytes(bytes)
            .map_err(|_| SorafsBootstrapPayloadErrorV1::Encoding)?;
        if payload.to_public_state()? != bytes {
            return Err(SorafsBootstrapPayloadErrorV1::Encoding);
        }
        Ok(payload)
    }

    /// Domain-separated digest of the canonical bootstrap payload.
    ///
    /// # Errors
    /// Returns [`SorafsBootstrapPayloadErrorV1::Encoding`] if the payload does not encode.
    pub fn canonical_digest(&self) -> Result<[u8; 32], SorafsBootstrapPayloadErrorV1> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(PAYLOAD_DOMAIN_V1);
        hasher.update(&self.to_public_state()?);
        Ok(*hasher.finalize().as_bytes())
    }

    /// Payload identity bound to one network, retained in the bootstrap receipt.
    ///
    /// # Errors
    /// Returns [`SorafsBootstrapPayloadErrorV1::Encoding`] if the payload does not encode.
    pub fn session_id(
        &self,
        network: &NetworkId,
    ) -> Result<[u8; 32], SorafsBootstrapPayloadErrorV1> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SESSION_DOMAIN_V1);
        hasher.update(network.as_bytes());
        hasher.update(&self.canonical_digest()?);
        Ok(*hasher.finalize().as_bytes())
    }
}

impl SorafsBootstrapRecordV1 {
    /// Decode the sole canonical record layout.
    ///
    /// # Errors
    /// Rejects noncanonical or trailing bytes.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        let record: Self = norito::decode_from_bytes(bytes)?;
        if norito::encode_canonical(&record)? != bytes {
            return Err(norito::core::Error::Message(
                "SoraFS bootstrap record is not canonical".into(),
            ));
        }
        Ok(record)
    }
}

#[cfg(test)]
mod tests;

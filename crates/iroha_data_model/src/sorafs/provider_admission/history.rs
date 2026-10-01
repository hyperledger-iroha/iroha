//! Sole native provider-admission history layout and bounded canonical codec.
//!
//! These records are data. Authority requires their exact current native World
//! preimages authenticated under an independently selected finality root.

use crate::{account::AccountId, sorafs::capacity::ProviderId};
use iroha_model_base::state_path::StatePath;
use std::str::FromStr;

/// Maximum complete retained history frame, including its admission material.
pub const MAX_ADMISSION_HISTORY_BYTES_V1: usize = 2 * 1024 * 1024;

/// Canonical retained policy or provider transition, including terminal tombstones.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::history::AdmissionHistoryRecordV1"
)]
pub struct AdmissionHistoryRecordV1 {
    /// Exact genesis-derived network identity.
    pub network_id: [u8; 32],
    /// Exact signed-genesis initializer, absent on later Parliament effects.
    #[norito(required)]
    pub genesis_origin: Option<GenesisAdmissionOriginV1>,
    /// Monotonic transition revision beginning at one.
    pub revision: u64,
    /// Canonical previous retained history digest, absent only at revision one.
    #[norito(required)]
    pub predecessor: Option<[u8; 32]>,
    /// Original successful execution height.
    pub height: u64,
    /// Original execution timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Exact registered provider owner; absent for council policy records.
    #[norito(required)]
    pub owner: Option<AccountId>,
    /// Terminal provider revocation marker.
    pub revoked: bool,
    /// Complete canonical policy, admission envelope, or revocation frame.
    pub material: Vec<u8>,
}

/// Exact initializer inside the independently authenticated signed genesis.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::history::GenesisAdmissionOriginV1"
)]
pub struct GenesisAdmissionOriginV1 {
    /// Original external genesis entry index.
    pub entrypoint_index: u32,
    /// Canonical initializer instruction digest.
    pub instruction_digest: [u8; 32],
}

/// Closed native admission key space; callers cannot inject arbitrary path suffixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionHistoryPathV1 {
    /// Current canonical retained transition.
    Head,
    /// One immutable transition revision.
    Revision(u64),
    /// Permanent provider identity count, including tombstones.
    ProviderCount,
    /// Retained non-emergency history byte counter.
    HistoryBytes,
    /// Retained terminal revocation byte counter.
    RevocationBytes,
}

/// Canonical native storage path for a council or provider admission record.
#[must_use]
pub fn admission_history_path(
    subject: Option<ProviderId>,
    suffix: AdmissionHistoryPathV1,
) -> StatePath {
    let suffix = match suffix {
        AdmissionHistoryPathV1::Head => "head".into(),
        AdmissionHistoryPathV1::Revision(revision) => format!("history/{revision}"),
        AdmissionHistoryPathV1::ProviderCount => "provider_count".into(),
        AdmissionHistoryPathV1::HistoryBytes => "history_bytes".into(),
        AdmissionHistoryPathV1::RevocationBytes => "revocation_bytes".into(),
    };
    let subject = subject.map_or_else(|| "council".into(), |id| hex::encode(id.as_bytes()));
    StatePath::from_str(&format!("sorafs/provider_admission/{subject}/{suffix}"))
        .expect("native admission callers supply a fixed canonical suffix")
}

/// Encode one native admission history value within its fixed frame bound.
/// # Errors
/// Serialization fails or the canonical frame exceeds the retained record limit.
pub fn encode_history_value<T: norito::core::NoritoSerialize>(
    value: &T,
) -> Result<Vec<u8>, norito::core::Error> {
    if norito::canonical_frame_len(value)? > MAX_ADMISSION_HISTORY_BYTES_V1 {
        return Err(norito::core::Error::Message(
            "admission history frame exceeds its bound".into(),
        ));
    }
    norito::encode_canonical(value)
}

impl AdmissionHistoryRecordV1 {
    /// Decode the sole canonical history layout with a finite allocation budget.
    /// # Errors
    /// Invalid, oversized, noncanonical or resource-exhausting input.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        if bytes.len() > MAX_ADMISSION_HISTORY_BYTES_V1 {
            return Err(norito::core::Error::Message(
                "admission history frame exceeds its bound".into(),
            ));
        }
        norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(
                65536,
                MAX_ADMISSION_HISTORY_BYTES_V1,
                65536,
                8 * 1024 * 1024,
                64,
            ),
        )
    }

    /// Exact predecessor digest used by native admission transitions.
    /// # Errors
    /// Canonical serialization fails or the retained frame is oversized.
    pub fn canonical_digest(&self) -> Result<[u8; 32], norito::core::Error> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"iroha.sorafs.provider-admission.native-history.v1\0");
        hasher.update(&encode_history_value(self)?);
        Ok(*hasher.finalize().as_bytes())
    }
}

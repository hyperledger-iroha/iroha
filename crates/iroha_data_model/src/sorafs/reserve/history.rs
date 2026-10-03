//! Sole canonical native reserve singleton layout and structural policy validation.
//!
//! Decoding these data values proves no inclusion, permission or initial namespace eligibility.
//! Core owns event-journal continuity and native mutation; the proof reader authenticates only
//! the selected singleton and explicitly projected prerequisites at one certified World cut.

use super::ReserveAuthorityPolicyRecordV1;
use crate::permission::Permission;
use iroha_model_base::state_path::StatePath;
use iroha_primitives::json::Json;
use std::{str::FromStr, sync::OnceLock};

/// Maximum canonical original accepted by the native reserve state codec.
pub const STATE_MAX_BYTES: usize = 2 * 1024 * 1024;
/// Existing finite native reserve decode limits, inherited by nested decoding.
pub const STATE_LIMITS: norito::DecodeLimits =
    norito::DecodeLimits::new(4_096, STATE_MAX_BYTES, 32_768, STATE_MAX_BYTES * 2, 64);

/// Return the sole canonical reserve state key.
#[must_use]
pub fn reserve_state_key() -> &'static StatePath {
    static KEY: OnceLock<StatePath> = OnceLock::new();
    KEY.get_or_init(|| {
        StatePath::from_str("sorafs_reserve_state_v1").expect("static state key is valid")
    })
}

/// Exact unit permission enforced by native reserve governance.
///
/// This is the representation of the executor model's unit `CanSetSorafsReservePolicy` token,
/// also used by native transaction permission checks. No delegated or inherited grant is inferred.
#[must_use]
pub fn reserve_policy_permission() -> Permission {
    Permission::new("CanSetSorafsReservePolicy".to_owned(), Json::new(()))
}

/// Exact native event-journal head stored beside the active policy.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::reserve::history::ReserveEventJournalHeadV1")]
pub struct ReserveEventJournalHeadV1 {
    /// Last committed reserve event sequence.
    pub last_sequence: u64,
    /// Native block height that executed the last event.
    pub last_target_block_height: u64,
    /// Native reserve event ordinal within that block.
    pub last_event_index: u32,
}

/// Sole canonical reserve singleton; a data value, not authenticated evidence.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::reserve::history::ReserveStateV1")]
pub struct ReserveStateV1 {
    /// Exact currently activated policy and its native provenance.
    pub policy: ReserveAuthorityPolicyRecordV1,
    /// Exact event head; continuity requires the native event originals separately.
    pub journal_head: ReserveEventJournalHeadV1,
}

/// Validate the existing native policy digest, policy fields and activation timestamp.
///
/// # Errors
/// Invalid policy, failed canonical digest encoding, changed digest or zero activation time.
pub fn validate_policy_record(
    record: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), norito::Error> {
    record.policy.validate().map_err(|error| {
        norito::Error::Message(format!("invalid stored reserve policy: {error}"))
    })?;
    if record.policy.digest()? != record.policy_digest || record.activated_at_unix == 0 {
        return Err(norito::Error::Message(
            "stored reserve policy digest or activation timestamp is invalid".into(),
        ));
    }
    Ok(())
}

impl ReserveStateV1 {
    /// Validate the canonical singleton's native structural constraints.
    ///
    /// This does not authenticate event continuity, namespace emptiness or any state inclusion.
    /// # Errors
    /// Invalid policy, digest, timestamp or zero journal sequence/height.
    pub fn validate(&self) -> Result<(), norito::Error> {
        validate_policy_record(&self.policy)?;
        if self.journal_head.last_sequence == 0 || self.journal_head.last_target_block_height == 0 {
            return Err(norito::Error::Message(
                "stored reserve event journal head is invalid".into(),
            ));
        }
        Ok(())
    }

    /// Decode the sole canonical native layout under the existing finite reserve limits.
    /// # Errors
    /// Oversized, noncanonical, resource-exhausting or structurally invalid original.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::Error> {
        if bytes.is_empty() || bytes.len() > STATE_MAX_BYTES {
            return Err(norito::Error::Message(
                "reserve state frame exceeds its bound".into(),
            ));
        }
        norito::core::with_decode_limits_scope(STATE_LIMITS, || {
            let state: Self = norito::decode_canonical_with_limits(bytes, STATE_LIMITS)?;
            state.validate()?;
            Ok(state)
        })
    }
}

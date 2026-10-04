//! Sole canonical native reserve singleton layout and structural policy validation.
//!
//! Decoding these data values proves no inclusion, permission or initial namespace eligibility.
//! Core owns event-journal continuity and native mutation; the proof reader authenticates only
//! the selected singleton and explicitly projected prerequisites at one certified World cut.

use super::{
    RESERVE_MAX_OPEN_APPEALS_V1, RESERVE_MAX_PENDING_MOVEMENTS_V1, ReserveAuthorityPolicyRecordV1,
    ReserveProviderAccountV1,
};
use crate::permission::Permission;
use crate::sorafs::capacity::ProviderId;
use iroha_model_base::state_path::StatePath;
use iroha_primitives::json::Json;
use std::{str::FromStr, sync::OnceLock};

/// Maximum canonical original accepted by the native reserve state codec.
pub const STATE_MAX_BYTES: usize = 2 * 1024 * 1024;
/// Existing finite native reserve decode limits, inherited by nested decoding.
pub const STATE_LIMITS: norito::DecodeLimits =
    norito::DecodeLimits::new(4_096, STATE_MAX_BYTES, 32_768, STATE_MAX_BYTES * 2, 64);

/// Physical state namespace owned exclusively by native reserve transitions.
pub const RESERVE_STATE_KEY_PREFIX_V1: &str = "sorafs_reserve_";

/// Whether a physical state key belongs to the native reserve namespace.
///
/// The exact trailing underscore is significant. This predicate establishes ownership only;
/// it proves neither that any key exists nor that the namespace is empty at a certified cut.
#[must_use]
pub fn is_reserve_state_key(key: &str) -> bool {
    key.starts_with(RESERVE_STATE_KEY_PREFIX_V1)
}

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

/// Sole physical prefix for the native per-provider reserve partition.
pub const RESERVE_PROVIDER_STATE_KEY_PREFIX_V1: &str = "sorafs_reserve_provider_v1_";

/// Derive the exact existing native provider partition key, with lowercase full-width hex.
#[must_use]
pub fn reserve_provider_key(provider_id: ProviderId) -> StatePath {
    StatePath::from_str(&format!(
        "{RESERVE_PROVIDER_STATE_KEY_PREFIX_V1}{}",
        hex::encode(provider_id.as_bytes())
    ))
    .expect("static prefix plus lowercase hex is a valid state key")
}

/// Validate the native stored-partition structural predicates without decoding or cloning it.
///
/// The expected id is the independently selected physical key. Current policy equality is not
/// a predicate: committed provider projections legitimately lag a governance policy rotation.
/// Provider existence, nonzero selection, owner, finality and custody backing are separate checks.
/// # Errors
/// Wrong physical-key id, malformed counters, timestamps, capacity, digest or principal ceiling.
pub fn validate_provider_record(
    account: &ReserveProviderAccountV1,
    provider_id: ProviderId,
) -> Result<(), norito::Error> {
    if account.terms.provider_id != provider_id
        || account.terms.capacity_gib == 0
        || account.policy_digest == [0; 32]
        || account.revision == 0
        || account.debt_principal > account.credit_cap
        || account.pending_movements > RESERVE_MAX_PENDING_MOVEMENTS_V1
        || account.open_appeals > RESERVE_MAX_OPEN_APPEALS_V1
        || account.rent_charged_through_unix == 0
        || account.interest_accrued_at_unix == 0
        || account.updated_at_unix == 0
        || account.rent_charged_through_unix > account.updated_at_unix
        || account.interest_accrued_at_unix > account.updated_at_unix
    {
        return Err(norito::Error::Message(
            "stored reserve provider account is inconsistent".into(),
        ));
    }
    Ok(())
}

/// Decode the sole canonical provider frame and validate its original native structure.
///
/// Core queries keep their existing measured `decode_state_with_current` owner and invoke the
/// shared borrowed validator afterward. They must not replace that owner with this convenience.
/// # Errors
/// Oversized, malformed, noncanonical, resource-exhausting or structurally invalid originals.
pub fn decode_reserve_provider_frame(
    bytes: &[u8],
    provider_id: ProviderId,
) -> Result<ReserveProviderAccountV1, norito::Error> {
    if bytes.is_empty() || bytes.len() > STATE_MAX_BYTES {
        return Err(norito::Error::Message(
            "reserve provider frame exceeds its bound".into(),
        ));
    }
    norito::core::with_decode_limits_scope(STATE_LIMITS, || {
        let account: ReserveProviderAccountV1 =
            norito::decode_canonical_with_limits(bytes, STATE_LIMITS)?;
        validate_provider_record(&account, provider_id)?;
        Ok(account)
    })
}

#[cfg(test)]
mod namespace_tests {
    use super::*;

    #[test]
    fn reserve_namespace_matches_only_the_native_physical_prefix() {
        for key in [
            RESERVE_STATE_KEY_PREFIX_V1,
            reserve_state_key().as_ref(),
            "sorafs_reserve_provider_v1_00",
            "sorafs_reserve_movement_v1_00",
            "sorafs_reserve_appeal_v1_00",
            "sorafs_reserve_event_v1_0001",
            "sorafs_reserve_unknown_future_record",
        ] {
            assert!(is_reserve_state_key(key), "{key}");
        }
        for key in [
            "sorafs_reserve",
            "sorafs_reserve/user",
            "sorafs_reservex_state_v1",
            "sorafs_reserves_state_v1",
            "sc/contract/sorafs_reserve_state_v1",
            "user/sorafs_reserve_state_v1",
        ] {
            assert!(!is_reserve_state_key(key), "{key}");
        }
    }
}

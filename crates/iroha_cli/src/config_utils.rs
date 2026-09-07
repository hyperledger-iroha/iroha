use iroha_config::parameters::defaults::sorafs::gateway::{
    DEFAULT_ANONYMITY_POLICY, DEFAULT_ROLLOUT_PHASE,
};
use iroha_service_model::soranet::AnonymityPolicy;
use iroha_service_model::soranet::RolloutPhase;
use sorafs_manifest::alias_cache::AliasCachePolicy;
use std::time::Duration;
pub fn default_alias_cache_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(iroha_service_model::sorafs::DEFAULT_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}
pub fn default_anonymity_policy() -> AnonymityPolicy {
    AnonymityPolicy::parse(DEFAULT_ANONYMITY_POLICY).unwrap_or(AnonymityPolicy::GuardPq)
}
pub fn default_rollout_phase() -> RolloutPhase {
    RolloutPhase::parse(DEFAULT_ROLLOUT_PHASE).unwrap_or_default()
}

use std::time::Duration;

use iroha_config::parameters::{
    actual::SorafsRolloutPhase,
    defaults::{
        sorafs::gateway::{DEFAULT_ANONYMITY_POLICY, DEFAULT_ROLLOUT_PHASE},
        torii,
    },
};
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;

pub fn default_alias_cache_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
        Duration::from_secs(torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}

pub fn default_anonymity_policy() -> AnonymityPolicy {
    AnonymityPolicy::parse(DEFAULT_ANONYMITY_POLICY).unwrap_or(AnonymityPolicy::GuardPq)
}

pub fn default_rollout_phase() -> SorafsRolloutPhase {
    SorafsRolloutPhase::parse(DEFAULT_ROLLOUT_PHASE).unwrap_or_default()
}

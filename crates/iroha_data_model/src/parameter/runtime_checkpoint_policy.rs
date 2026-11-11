//! Committed checkpoint policy of the universal root (`specs/live_runtime_upgrades.md` §7.3;
//! custom parameter `runtime_checkpoint_policy_v1`).
//!
//! A checkpoint is the published State after one block, authenticated by the certified parent
//! root of the next. This policy decides, from committed block time alone, at which heights
//! every node produces one, which of them a node keeps at least, how old a checkpoint must be
//! before it becomes the startup floor, and how much notice raising the floor takes. A root
//! without the parameter raises no floor; a malformed committed value never falls back to a
//! default.

use iroha_primitives::json::Json;

use super::{CustomParameter, CustomParameterId};

/// Why a committed `runtime_checkpoint_policy_v1` parameter cannot be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeCheckpointPolicyErrorV1 {
    /// The custom parameter has another identity, an oversized or malformed payload, or a
    /// payload that is not the canonical JSON of its value.
    #[error("runtime checkpoint policy parameter: {0}")]
    CustomParameter(&'static str),
    /// The cadence is zero.
    #[error("the checkpoint cadence must be positive")]
    Cadence,
    /// The spaced retention is finer than the cadence it thins.
    #[error("the retention spacing must be zero or at least the cadence")]
    Spacing,
    /// The recovery window is shorter than the cadence, so no checkpoint could be old enough.
    #[error("the recovery window must be at least the cadence")]
    RecoveryWindow,
}

/// When checkpoints are produced, which are kept, and when the startup floor may rise.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::parameter::runtime_checkpoint_policy::RuntimeCheckpointPolicyV1"
)]
pub struct RuntimeCheckpointPolicyV1 {
    /// Production cadence in committed milliseconds: the first block committed in each cadence
    /// interval is a checkpoint height.
    pub cadence_ms: u64,
    /// Every checkpoint younger than this, in committed milliseconds, is kept.
    pub retain_all_ms: u64,
    /// Of the older checkpoints above the floor, one per this many committed milliseconds is
    /// kept; zero keeps every one.
    pub retain_spacing_ms: u64,
    /// Fewest committed milliseconds between a checkpoint and the height at which it becomes
    /// the startup floor: the oldest start a node can still recover from.
    pub recovery_window_ms: u64,
    /// Committed milliseconds between the block that raises the floor and the time the floor
    /// comes into force.
    pub floor_notice_ms: u64,
}

const HOUR_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * HOUR_MS;

impl Default for RuntimeCheckpointPolicyV1 {
    /// The production values of the design: a checkpoint every committed day, every one kept
    /// back to the floor, a 30-day recovery window and seven days of notice. A network commits
    /// its own values explicitly; this is never a fallback for an uncommitted policy.
    fn default() -> Self {
        Self {
            cadence_ms: DAY_MS,
            retain_all_ms: DAY_MS,
            retain_spacing_ms: 0,
            recovery_window_ms: 30 * DAY_MS,
            floor_notice_ms: 7 * DAY_MS,
        }
    }
}

impl RuntimeCheckpointPolicyV1 {
    /// Largest accepted JSON payload.
    pub const MAX_PAYLOAD_BYTES: usize = 1024;

    /// The sole canonical custom parameter identity.
    #[must_use]
    pub fn parameter_id() -> &'static CustomParameterId {
        static ID: std::sync::OnceLock<CustomParameterId> = std::sync::OnceLock::new();
        ID.get_or_init(|| {
            "runtime_checkpoint_policy_v1"
                .parse()
                .expect("static parameter identity")
        })
    }

    /// Check the policy's own rules.
    ///
    /// # Errors
    /// The first rule the policy violates.
    pub const fn validate(&self) -> Result<(), RuntimeCheckpointPolicyErrorV1> {
        if self.cadence_ms == 0 {
            return Err(RuntimeCheckpointPolicyErrorV1::Cadence);
        }
        if self.retain_spacing_ms != 0 && self.retain_spacing_ms < self.cadence_ms {
            return Err(RuntimeCheckpointPolicyErrorV1::Spacing);
        }
        if self.recovery_window_ms < self.cadence_ms {
            return Err(RuntimeCheckpointPolicyErrorV1::RecoveryWindow);
        }
        Ok(())
    }

    /// Whether the block committed at `committed_ms`, whose parent was committed at
    /// `parent_ms`, is a checkpoint height: the first block of its cadence interval. An idle
    /// chain produces no block and so no checkpoint; its latest one stays the newest.
    #[must_use]
    pub const fn is_checkpoint(&self, parent_ms: u64, committed_ms: u64) -> bool {
        self.cadence_ms != 0 && committed_ms / self.cadence_ms > parent_ms / self.cadence_ms
    }

    /// Whether a checkpoint committed at `committed_ms` is old enough at `now_ms` to become
    /// the startup floor.
    #[must_use]
    pub const fn recoverable(&self, committed_ms: u64, now_ms: u64) -> bool {
        now_ms >= committed_ms && now_ms - committed_ms >= self.recovery_window_ms
    }

    /// The committed time from which a floor raised in the block committed at `raised_ms` is in
    /// force: the notice later. `None` if that time is not representable.
    #[must_use]
    pub const fn floor_effective_ms(&self, raised_ms: u64) -> Option<u64> {
        raised_ms.checked_add(self.floor_notice_ms)
    }

    /// The heights of `checkpoints` a node keeps at least, in order. `checkpoints` lists each
    /// held checkpoint's height and committed time, sorted by height; `floor` is the height of
    /// the startup floor, if one is set, and `now_ms` the committed time of the tip.
    ///
    /// Kept are the floor's own checkpoint, the newest checkpoint, every checkpoint younger
    /// than [`Self::retain_all_ms`], and of the older ones above the floor the first of each
    /// spacing interval. A checkpoint below the floor is never kept. These are minimums: a node
    /// may hold more.
    #[must_use]
    pub fn retained(
        &self,
        checkpoints: &[(u64, u64)],
        floor: Option<u64>,
        now_ms: u64,
    ) -> Vec<u64> {
        let newest = checkpoints.last().map(|(height, _)| *height);
        let mut last_interval = None;
        checkpoints
            .iter()
            .filter(|(height, committed_ms)| {
                if floor.is_some_and(|floor| *height < floor) {
                    return false;
                }
                if Some(*height) == floor || Some(*height) == newest {
                    return true;
                }
                if now_ms.saturating_sub(*committed_ms) < self.retain_all_ms {
                    return true;
                }
                if self.retain_spacing_ms == 0 {
                    return true;
                }
                let interval = committed_ms / self.retain_spacing_ms;
                let first = last_interval != Some(interval);
                last_interval = Some(interval);
                first
            })
            .map(|(height, _)| *height)
            .collect()
    }

    /// Decode and validate the exact committed parameter, without substituting defaults.
    ///
    /// # Errors
    /// Another identity, more than [`Self::MAX_PAYLOAD_BYTES`] of JSON, a malformed or
    /// non-canonical payload, or a value that fails [`Self::validate`].
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Self, RuntimeCheckpointPolicyErrorV1> {
        if parameter.id() != Self::parameter_id() {
            return Err(RuntimeCheckpointPolicyErrorV1::CustomParameter(
                "another parameter identity",
            ));
        }
        let payload = parameter.payload();
        if payload.get().len() > Self::MAX_PAYLOAD_BYTES {
            return Err(RuntimeCheckpointPolicyErrorV1::CustomParameter(
                "payload exceeds 1024 bytes",
            ));
        }
        let value: Self = norito::json::from_str(payload.get())
            .map_err(|_| RuntimeCheckpointPolicyErrorV1::CustomParameter("malformed payload"))?;
        if Json::try_new(&value).ok().as_ref() != Some(payload) {
            return Err(RuntimeCheckpointPolicyErrorV1::CustomParameter(
                "payload is not the canonical JSON of its value",
            ));
        }
        value.validate()?;
        Ok(value)
    }

    /// The committed custom parameter of a valid policy, with its canonical JSON payload.
    ///
    /// # Errors
    /// A policy that fails [`Self::validate`], or a payload that cannot be produced.
    pub fn to_custom_parameter(&self) -> Result<CustomParameter, RuntimeCheckpointPolicyErrorV1> {
        self.validate()?;
        let payload = Json::try_new(self).map_err(|_| {
            RuntimeCheckpointPolicyErrorV1::CustomParameter("payload cannot be serialized")
        })?;
        Ok(CustomParameter::new(Self::parameter_id().clone(), payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Taira's committed values of the design: every 15 minutes, all of the last day, then one
    /// per day; a seven-day recovery window and a day of notice.
    const TAIRA: RuntimeCheckpointPolicyV1 = RuntimeCheckpointPolicyV1 {
        cadence_ms: 15 * 60 * 1000,
        retain_all_ms: DAY_MS,
        retain_spacing_ms: DAY_MS,
        recovery_window_ms: 7 * DAY_MS,
        floor_notice_ms: DAY_MS,
    };

    #[test]
    fn a_valid_policy_round_trips_through_its_parameter() {
        for policy in [RuntimeCheckpointPolicyV1::default(), TAIRA] {
            let parameter = policy.to_custom_parameter().expect("valid policy");
            assert_eq!(
                RuntimeCheckpointPolicyV1::from_custom_parameter(&parameter),
                Ok(policy)
            );
        }
    }

    #[test]
    fn a_malformed_policy_is_refused_and_never_defaulted() {
        let invalid = |policy: RuntimeCheckpointPolicyV1| policy.validate().unwrap_err();
        assert_eq!(
            invalid(RuntimeCheckpointPolicyV1 {
                cadence_ms: 0,
                ..TAIRA
            }),
            RuntimeCheckpointPolicyErrorV1::Cadence
        );
        assert_eq!(
            invalid(RuntimeCheckpointPolicyV1 {
                retain_spacing_ms: TAIRA.cadence_ms - 1,
                ..TAIRA
            }),
            RuntimeCheckpointPolicyErrorV1::Spacing
        );
        assert_eq!(
            invalid(RuntimeCheckpointPolicyV1 {
                recovery_window_ms: TAIRA.cadence_ms - 1,
                ..TAIRA
            }),
            RuntimeCheckpointPolicyErrorV1::RecoveryWindow
        );

        let parameter = TAIRA.to_custom_parameter().unwrap();
        let other = CustomParameter::new(
            "another_policy".parse().unwrap(),
            parameter.payload().clone(),
        );
        assert!(RuntimeCheckpointPolicyV1::from_custom_parameter(&other).is_err());
        let oversized = CustomParameter::new(
            RuntimeCheckpointPolicyV1::parameter_id().clone(),
            Json::new("x".repeat(RuntimeCheckpointPolicyV1::MAX_PAYLOAD_BYTES)),
        );
        assert_eq!(
            RuntimeCheckpointPolicyV1::from_custom_parameter(&oversized),
            Err(RuntimeCheckpointPolicyErrorV1::CustomParameter(
                "payload exceeds 1024 bytes"
            ))
        );
        let another_shape = CustomParameter::new(
            RuntimeCheckpointPolicyV1::parameter_id().clone(),
            Json::new(7_u64),
        );
        assert_eq!(
            RuntimeCheckpointPolicyV1::from_custom_parameter(&another_shape),
            Err(RuntimeCheckpointPolicyErrorV1::CustomParameter(
                "malformed payload"
            ))
        );
    }

    #[test]
    fn the_first_block_of_each_cadence_interval_is_a_checkpoint() {
        let cadence = TAIRA.cadence_ms;
        assert!(!TAIRA.is_checkpoint(cadence, cadence + 1));
        assert!(TAIRA.is_checkpoint(cadence - 1, cadence));
        assert!(
            TAIRA.is_checkpoint(cadence - 1, 5 * cadence + 7),
            "an idle stretch yields one checkpoint at the next block, not one per interval"
        );
        assert!(!TAIRA.is_checkpoint(5 * cadence + 7, 5 * cadence + 8));
    }

    #[test]
    fn the_floor_rises_only_to_a_recoverable_checkpoint_after_notice() {
        let committed = 10 * DAY_MS;
        assert!(!TAIRA.recoverable(committed, committed + 7 * DAY_MS - 1));
        assert!(TAIRA.recoverable(committed, committed + 7 * DAY_MS));
        assert!(!TAIRA.recoverable(committed, committed - 1));
        assert_eq!(
            TAIRA.floor_effective_ms(committed),
            Some(committed + DAY_MS)
        );
        assert_eq!(TAIRA.floor_effective_ms(u64::MAX), None);
    }

    #[test]
    fn retention_keeps_the_recent_the_spaced_the_floor_and_the_newest() {
        let now = 10 * DAY_MS;
        // (height, committed time): three old days with two checkpoints each, then the last day.
        let checkpoints = [
            (10, 6 * DAY_MS + 1),
            (20, 6 * DAY_MS + HOUR_MS),
            (30, 7 * DAY_MS + 1),
            (40, 7 * DAY_MS + HOUR_MS),
            (50, 8 * DAY_MS + 1),
            (60, 8 * DAY_MS + HOUR_MS),
            (70, 9 * DAY_MS + 1),
            (80, 9 * DAY_MS + HOUR_MS),
        ];
        assert_eq!(
            TAIRA.retained(&checkpoints, None, now),
            [10, 30, 50, 70, 80],
            "one per day of the older ones, every one of the last day"
        );
        assert_eq!(
            TAIRA.retained(&checkpoints, Some(40), now),
            [40, 50, 70, 80],
            "nothing below the floor; the floor's own checkpoint is pinned"
        );
        assert_eq!(
            RuntimeCheckpointPolicyV1::default().retained(&checkpoints, Some(20), now),
            [20, 30, 40, 50, 60, 70, 80],
            "zero spacing keeps every checkpoint back to the floor"
        );
        assert_eq!(
            TAIRA.retained(&checkpoints[..2], None, 100 * DAY_MS),
            [10, 20],
            "an idle chain keeps its newest checkpoint however old"
        );
        assert!(TAIRA.retained(&[], Some(5), now).is_empty());
    }
}

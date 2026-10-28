//! Ordinary per-protocol limits and direct canonical native admission.

use iroha_data_model::privacy::{
    PrivacyCompiledProfileSnapshotV1, PrivacyProtocolIdV1, PrivacyProtocolLimitsV1,
    PrivacyProtocolLimitsValidationError, PrivacyProtocolPolicyV1,
    PrivacyProtocolPolicyValidationErrorV1,
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::privacy_profiles::{CompiledPrivacyProfileErrorV1, compiled_privacy_profile_v1};

/// Canonical key for an optional committed resource policy.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::privacy_state::PrivacyProtocolPolicyKeyV1")]
pub struct PrivacyProtocolPolicyKeyV1 {
    protocol_id: PrivacyProtocolIdV1,
}
impl PrivacyProtocolPolicyKeyV1 {
    /// Construct the sole key for the selected protocol.
    #[must_use]
    pub const fn new(protocol_id: PrivacyProtocolIdV1) -> Self {
        Self { protocol_id }
    }
    /// Protocol selected by the storage key.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }
}

/// Move-only selected implementation and ordinary resource limits.
///
/// Keep each admission result owned by its selecting execution path.
/// ```compile_fail
/// use iroha_core_privacy::privacy_state::AdmittedPrivacyProtocolV1;
/// fn duplicate(value: AdmittedPrivacyProtocolV1)
///     -> (AdmittedPrivacyProtocolV1, AdmittedPrivacyProtocolV1) {
///     (value, value)
/// }
/// ```
#[expect(
    missing_copy_implementations,
    reason = "The selected admission result intentionally has one owner, even though its public descriptors are Copy."
)]
pub struct AdmittedPrivacyProtocolV1 {
    profile: PrivacyCompiledProfileSnapshotV1,
    protocol_limits: PrivacyProtocolLimitsV1,
}
impl AdmittedPrivacyProtocolV1 {
    /// Canonical profile selected from the actual native implementation.
    #[must_use]
    pub const fn profile(&self) -> &PrivacyCompiledProfileSnapshotV1 {
        &self.profile
    }
    /// Effective limits selected from committed policy or canonical defaults.
    #[must_use]
    pub const fn protocol_limits(&self) -> &PrivacyProtocolLimitsV1 {
        &self.protocol_limits
    }
}

/// Concrete implementation or committed-policy admission failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyAdmissionErrorV1 {
    /// The selected native construction is absent, invalid or incomplete.
    #[error("privacy implementation is unavailable: {0}")]
    Implementation(#[source] CompiledPrivacyProfileErrorV1),
    /// A policy stored under the selected key identifies another protocol.
    #[error("privacy policy key {selected:?} differs from record {record:?}")]
    PolicyProtocolMismatch {
        /// Selected protocol.
        selected: PrivacyProtocolIdV1,
        /// Stored policy protocol.
        record: PrivacyProtocolIdV1,
    },
    /// The selected committed policy is malformed or has a missed schedule.
    #[error("privacy protocol policy is invalid: {0}")]
    Policy(#[source] PrivacyProtocolPolicyValidationErrorV1),
    /// Effective limits exceed the canonical selected implementation's safety bounds.
    #[error("privacy protocol limits exceed the canonical profile: {0}")]
    Limits(#[source] PrivacyProtocolLimitsValidationError),
}

/// Select the actual native implementation and optional committed resource policy.
///
/// No registration, qualification receipt or protocol lifecycle is consulted. A concrete
/// implementation error is returned before examining policy, so policy cannot enable it.
/// # Errors
/// Returns the original typed implementation failure or the selected policy/limit failure.
pub fn resolve_admitted_privacy_protocol_v1(
    policies: &impl StorageReadOnly<PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1>,
    protocol_id: PrivacyProtocolIdV1,
    committed_height: u64,
) -> Result<AdmittedPrivacyProtocolV1, PrivacyAdmissionErrorV1> {
    let compiled = compiled_privacy_profile_v1(protocol_id)
        .map_err(PrivacyAdmissionErrorV1::Implementation)?;
    let protocol_limits = match policies.get(&PrivacyProtocolPolicyKeyV1::new(protocol_id)) {
        Some(policy) => {
            if policy.protocol_id != protocol_id {
                return Err(PrivacyAdmissionErrorV1::PolicyProtocolMismatch {
                    selected: protocol_id,
                    record: policy.protocol_id,
                });
            }
            policy
                .validate_at_committed_height(committed_height)
                .map_err(PrivacyAdmissionErrorV1::Policy)?;
            policy.current_limits
        }
        None => compiled.protocol_limits,
    };
    protocol_limits
        .validate_with_ceiling(&compiled.protocol_limits)
        .map_err(PrivacyAdmissionErrorV1::Limits)?;
    Ok(AdmittedPrivacyProtocolV1 {
        profile: compiled.into(),
        protocol_limits,
    })
}

/// Deterministic policy restoration or block-start planning failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[expect(
    variant_size_differences,
    reason = "The bounded Copy error retains exact protocol and numeric diagnostics without allocating an error box."
)]
pub enum PrivacyProtocolLimitsPlanErrorV1 {
    /// A block must have a representable predecessor height.
    #[error("privacy protocol policy planning requires a positive incoming block height")]
    InvalidIncomingHeight,
    /// A storage key differs from its policy record.
    #[error("privacy policy key {key_protocol:?} differs from record {record_protocol:?}")]
    KeyProtocolMismatch {
        /// Storage key identity.
        key_protocol: PrivacyProtocolIdV1,
        /// Stored record identity.
        record_protocol: PrivacyProtocolIdV1,
    },
    /// A policy violates canonical bounds or exact scheduling.
    #[error("privacy policy {protocol_id:?} is invalid: {source}")]
    InvalidPolicy {
        /// Invalid policy identity.
        protocol_id: PrivacyProtocolIdV1,
        /// Original structured failure.
        source: PrivacyProtocolPolicyValidationErrorV1,
    },
}

fn validate_key_v1(
    key: PrivacyProtocolPolicyKeyV1,
    policy: &PrivacyProtocolPolicyV1,
) -> Result<(), PrivacyProtocolLimitsPlanErrorV1> {
    if key.protocol_id() != policy.protocol_id {
        return Err(PrivacyProtocolLimitsPlanErrorV1::KeyProtocolMismatch {
            key_protocol: key.protocol_id(),
            record_protocol: policy.protocol_id,
        });
    }
    Ok(())
}

/// Validate optional persisted policies without requiring a local implementation.
/// # Errors
/// Rejects mismatched keys, malformed limits and intrinsically invalid pending updates.
pub fn validate_privacy_protocol_policies_v1(
    policies: &impl StorageReadOnly<PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1>,
) -> Result<(), PrivacyProtocolLimitsPlanErrorV1> {
    for (key, policy) in policies.iter() {
        validate_key_v1(*key, policy)?;
        policy
            .validate()
            .map_err(|source| PrivacyProtocolLimitsPlanErrorV1::InvalidPolicy {
                protocol_id: policy.protocol_id,
                source,
            })?;
    }
    Ok(())
}

/// Validate every optional policy against its exact committed snapshot height.
/// # Errors
/// Rejects malformed policies, future claimed admission and unapplied due updates.
pub fn validate_privacy_protocol_policies_at_committed_height_v1(
    policies: &impl StorageReadOnly<PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1>,
    committed_height: u64,
) -> Result<(), PrivacyProtocolLimitsPlanErrorV1> {
    for (key, policy) in policies.iter() {
        validate_key_v1(*key, policy)?;
        policy
            .validate_at_committed_height(committed_height)
            .map_err(|source| PrivacyProtocolLimitsPlanErrorV1::InvalidPolicy {
                protocol_id: policy.protocol_id,
                source,
            })?;
    }
    Ok(())
}

/// Plan exact-boundary ordinary limit updates after all predecessor records validate.
///
/// This function is read-only. The caller applies the complete plan atomically. No implementation
/// artifact or protocol activation state participates in restoration or update execution.
/// # Errors
/// Rejects a zero incoming height, invalid predecessor policies and missed schedules.
pub fn plan_due_privacy_protocol_limits_v1(
    policies: &impl StorageReadOnly<PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1>,
    incoming_height: u64,
) -> Result<
    Vec<(PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1)>,
    PrivacyProtocolLimitsPlanErrorV1,
> {
    let predecessor = incoming_height
        .checked_sub(1)
        .ok_or(PrivacyProtocolLimitsPlanErrorV1::InvalidIncomingHeight)?;
    validate_privacy_protocol_policies_at_committed_height_v1(policies, predecessor)?;
    let mut updates = Vec::new();
    for (key, policy) in policies.iter() {
        if let Some(pending) = policy.pending_update
            && pending.effective_at_height == incoming_height
        {
            let successor = PrivacyProtocolPolicyV1 {
                protocol_id: policy.protocol_id,
                current_limits: pending.next_limits,
                pending_update: None,
            };
            successor.validate().map_err(|source| {
                PrivacyProtocolLimitsPlanErrorV1::InvalidPolicy {
                    protocol_id: policy.protocol_id,
                    source,
                }
            })?;
            updates.push((*key, successor));
        }
    }
    Ok(updates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::privacy::{PrivacyProtocolLimitsUpdateV1, VeRangeLimitsV1};
    use mv::storage::Storage;

    fn ve_range(count: u32) -> PrivacyProtocolLimitsV1 {
        PrivacyProtocolLimitsV1::VeRangeTransparentRangeV1(VeRangeLimitsV1 {
            max_aggregation_count: count,
        })
    }

    fn scheduled_policy(current: u32, next: u32) -> PrivacyProtocolPolicyV1 {
        PrivacyProtocolPolicyV1 {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            current_limits: ve_range(current),
            pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
                scheduled_at_height: 100,
                effective_at_height: 101,
                next_limits: ve_range(next),
            }),
        }
    }

    #[test]
    fn admission_uses_compiled_defaults_without_registration_at_any_height() {
        let policies = Storage::new();
        let protocol_id = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
        let compiled = compiled_privacy_profile_v1(protocol_id).unwrap();
        for height in [0, 1, 100, u64::MAX] {
            let admitted =
                resolve_admitted_privacy_protocol_v1(&policies.view(), protocol_id, height)
                    .expect("canonical implemented profile requires no activation row");
            assert_eq!(admitted.profile(), &compiled.snapshot());
            assert_eq!(admitted.protocol_limits(), &compiled.protocol_limits);
        }
    }

    #[test]
    fn admission_selects_only_its_ordinary_policy_and_keeps_snapshot_immutable() {
        let protocol_id = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
        let compiled = compiled_privacy_profile_v1(protocol_id).unwrap();
        let mut policies = Storage::new();
        let mut policy = PrivacyProtocolPolicyV1::canonical_default_v1(protocol_id);
        policy.current_limits = ve_range(1);
        policies.insert(PrivacyProtocolPolicyKeyV1::new(protocol_id), policy);
        // A malformed unrelated policy is a restoration error, but cannot gate this selection.
        policies.insert(
            PrivacyProtocolPolicyKeyV1::new(PrivacyProtocolIdV1::IrohaZkX509StarkP256V1),
            policy,
        );
        let admitted =
            resolve_admitted_privacy_protocol_v1(&policies.view(), protocol_id, 10).unwrap();
        assert_eq!(admitted.profile(), &compiled.snapshot());
        assert_eq!(admitted.protocol_limits(), &ve_range(1));
        assert!(validate_privacy_protocol_policies_v1(&policies.view()).is_err());
        policy.protocol_id = PrivacyProtocolIdV1::PqMaspStarkV1;
        policies.insert(PrivacyProtocolPolicyKeyV1::new(protocol_id), policy);
        assert!(matches!(
            resolve_admitted_privacy_protocol_v1(&policies.view(), protocol_id, 10),
            Err(PrivacyAdmissionErrorV1::PolicyProtocolMismatch { .. })
        ));
    }

    #[test]
    fn x509_policy_cannot_replace_missing_joint_security_construction() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
        let key = PrivacyProtocolPolicyKeyV1::new(protocol_id);
        let mut policies = Storage::new();
        policies.insert(
            key,
            PrivacyProtocolPolicyV1::canonical_default_v1(protocol_id),
        );
        validate_privacy_protocol_policies_at_committed_height_v1(&policies.view(), 0).expect(
            "optional ordinary policy is independent from local implementation availability",
        );
        assert!(matches!(
            resolve_admitted_privacy_protocol_v1(&policies.view(), protocol_id, 0),
            Err(PrivacyAdmissionErrorV1::Implementation(
                CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete { protocol_id: actual }
            )) if actual == protocol_id
        ));
        assert_eq!(key.protocol_id(), protocol_id);
        let frame = norito::encode_canonical(&key).unwrap();
        assert_eq!(
            norito::decode_canonical::<PrivacyProtocolPolicyKeyV1>(&frame).unwrap(),
            key
        );
    }

    #[test]
    fn ordinary_policy_updates_apply_both_directions_at_exact_boundary() {
        for (current, next) in [(2, 1), (1, 2)] {
            let policy = scheduled_policy(current, next);
            let key = PrivacyProtocolPolicyKeyV1::new(policy.protocol_id);
            let mut policies = Storage::new();
            policies.insert(key, policy);
            assert!(
                plan_due_privacy_protocol_limits_v1(&policies.view(), 100).is_err(),
                "predecessor99 cannot contain an update admitted at100"
            );
            validate_privacy_protocol_policies_at_committed_height_v1(&policies.view(), 100)
                .unwrap();
            let plan = plan_due_privacy_protocol_limits_v1(&policies.view(), 101).unwrap();
            assert_eq!(
                plan,
                vec![(
                    key,
                    PrivacyProtocolPolicyV1 {
                        protocol_id: policy.protocol_id,
                        current_limits: ve_range(next),
                        pending_update: None,
                    }
                )]
            );
            assert_eq!(
                policies.view().get(&key),
                Some(&policy),
                "planning is read-only"
            );
            assert!(
                plan_due_privacy_protocol_limits_v1(&policies.view(), 102).is_err(),
                "a missed effective boundary cannot be silently replayed"
            );
            policies.insert(key, plan[0].1);
            validate_privacy_protocol_policies_at_committed_height_v1(&policies.view(), 101)
                .unwrap();
            assert!(
                plan_due_privacy_protocol_limits_v1(&policies.view(), 102)
                    .unwrap()
                    .is_empty()
            );
            let json = norito::json::to_json(&policies).unwrap();
            let restored: Storage<PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1> =
                norito::json::from_json(&json).unwrap();
            assert_eq!(restored.view().get(&key), Some(&plan[0].1));
        }
    }

    #[test]
    fn future_schedules_validate_and_invalid_registry_never_partially_plans() {
        let mut policy = scheduled_policy(2, 1);
        policy.pending_update.as_mut().unwrap().effective_at_height = 200;
        let key = PrivacyProtocolPolicyKeyV1::new(policy.protocol_id);
        let mut policies = Storage::new();
        policies.insert(key, policy);
        assert!(
            plan_due_privacy_protocol_limits_v1(&policies.view(), 101)
                .unwrap()
                .is_empty()
        );
        assert_eq!(policies.view().get(&key), Some(&policy));
        policies.insert(
            PrivacyProtocolPolicyKeyV1::new(PrivacyProtocolIdV1::PqMaspStarkV1),
            policy,
        );
        assert!(matches!(
            plan_due_privacy_protocol_limits_v1(&policies.view(), 200),
            Err(PrivacyProtocolLimitsPlanErrorV1::KeyProtocolMismatch { .. })
        ));
        assert_eq!(policies.view().get(&key), Some(&policy));
        assert!(matches!(
            plan_due_privacy_protocol_limits_v1(&policies.view(), 0),
            Err(PrivacyProtocolLimitsPlanErrorV1::InvalidIncomingHeight)
        ));
    }

    #[test]
    fn selected_policy_rejects_invalid_limits_and_future_or_missed_updates() {
        let policy = scheduled_policy(2, 1);
        let key = PrivacyProtocolPolicyKeyV1::new(policy.protocol_id);
        let mut policies = Storage::new();
        policies.insert(key, policy);
        for height in [99, 101, u64::MAX] {
            assert!(matches!(
                resolve_admitted_privacy_protocol_v1(&policies.view(), policy.protocol_id, height),
                Err(PrivacyAdmissionErrorV1::Policy(_))
            ));
        }
        let mut invalid = policy;
        invalid.current_limits = ve_range(0);
        policies.insert(key, invalid);
        assert!(matches!(
            resolve_admitted_privacy_protocol_v1(&policies.view(), policy.protocol_id, 100),
            Err(PrivacyAdmissionErrorV1::Policy(_))
        ));
    }
}

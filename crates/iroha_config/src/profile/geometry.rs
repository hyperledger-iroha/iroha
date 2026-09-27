//! Roster-dependent Sumeragi v2 queue, byte and connection geometry.
//!
//! This is the single derivation shared by compiled profiles ([`super::Profile::derive`]) and
//! Kagami's generated local networks. It admits one exact `3f + 1` roster against the same
//! lifecycle, exact-output and outer-ingress checks that user-configuration parsing applies.

use crate::parameters::{actual, defaults};
use iroha_data_model::block::consensus_v2::{MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size};
use thiserror::Error;

/// Inputs of one roster-dependent Sumeragi v2 ingress geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SumeragiV2IngressInputs {
    /// Exact `3f + 1` validator roster, which is also the signed `NPoS` `max_validators`.
    pub validators: usize,
    /// Serialized reducer command FIFO capacity (`sumeragi.queues.commands`).
    pub queue_commands: usize,
    /// Outer-ingress message and certified-request capacity (`sumeragi.queues.bodies`).
    pub queue_bodies: usize,
    /// Authenticated non-validator fair-ingress lanes
    /// (`sumeragi.queues.authenticated_non_validator_sources`).
    pub authenticated_non_validator_sources: usize,
    /// Committee-class ingress sources, `n + max_external_committee_peers` for a profile and
    /// zero for a generator without the committee class. Each owns a validator-sized
    /// partition.
    pub committee_sources: usize,
    /// Total P2P connection bound (`network.max_total_connections`).
    pub max_total_connections: usize,
    /// Per-source outer-ingress byte partition (`sumeragi.queues.body_source_bytes`).
    pub body_source_bytes: usize,
}

/// Admitted roster-dependent geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SumeragiV2IngressGeometry {
    /// Height-local lifecycle record classes.
    pub lifecycle: actual::SumeragiV2LifecycleCapacityGeometry,
    /// Smallest admissible `sumeragi.queues.bodies` for this roster.
    pub required_bodies: usize,
    /// Aggregate outer-ingress bytes (`sumeragi.queues.body_bytes`):
    /// `(n + committee_sources + authenticated) × body_source_bytes`.
    pub body_bytes: usize,
}

/// Inadmissible roster-dependent geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SumeragiV2GeometryError {
    /// The roster exceeds the protocol maximum.
    #[error("validator roster {validators} exceeds the Sumeragi v2 protocol maximum of {maximum}")]
    RosterAboveMaximum {
        /// Requested roster.
        validators: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// The roster is not `3f + 1` with `f >= 1`.
    #[error(
        "validator roster {validators} is not an exact Sumeragi v2 3f+1 committee in the supported range 4..={maximum}"
    )]
    NotExactCommittee {
        /// Requested roster.
        validators: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// Lifecycle record classes do not fit.
    #[error(transparent)]
    Lifecycle(#[from] actual::SumeragiV2LifecycleCapacityGeometryError),
    /// One maximum fanout does not fit the exact-output owner.
    #[error(transparent)]
    ExactOutput(#[from] actual::SumeragiV2ExactOutputGeometryError),
    /// The body queue cannot hold the protected positions of every ingress owner.
    #[error("Sumeragi v2 body queue capacity {actual} is below the roster minimum {minimum}")]
    BodyQueueTooSmall {
        /// Configured capacity.
        actual: usize,
        /// Required capacity.
        minimum: usize,
    },
    /// Capacity arithmetic overflowed.
    #[error("Sumeragi v2 outer-ingress geometry overflowed")]
    Overflow,
}

/// Admit one exact roster and derive its outer-ingress geometry.
///
/// # Errors
///
/// [`SumeragiV2GeometryError`] when the roster is not an exact `3f + 1` committee or any
/// derived capacity is inadmissible.
pub fn sumeragi_v2_ingress_geometry(
    inputs: SumeragiV2IngressInputs,
) -> Result<SumeragiV2IngressGeometry, SumeragiV2GeometryError> {
    let SumeragiV2IngressInputs {
        validators,
        queue_commands,
        queue_bodies,
        authenticated_non_validator_sources,
        committee_sources,
        max_total_connections,
        body_source_bytes,
    } = inputs;
    if validators > MAX_VALIDATORS_PER_HEIGHT {
        return Err(SumeragiV2GeometryError::RosterAboveMaximum {
            validators,
            maximum: MAX_VALIDATORS_PER_HEIGHT,
        });
    }
    if !is_valid_committee_size(validators) {
        return Err(SumeragiV2GeometryError::NotExactCommittee {
            validators,
            maximum: MAX_VALIDATORS_PER_HEIGHT,
        });
    }
    let effect_work_capacity =
        (queue_commands / defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR).max(1);
    let lifecycle = actual::sumeragi_v2_lifecycle_capacity_geometry(
        validators,
        effect_work_capacity,
        queue_bodies,
        authenticated_non_validator_sources,
    )?;
    let shared_ownership_capacity = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
        effect_work_capacity,
        queue_bodies,
    )?;
    actual::validate_sumeragi_v2_exact_output_geometry(
        shared_ownership_capacity,
        max_total_connections,
    )?;
    let ingress_owners = validators
        .checked_add(committee_sources)
        .ok_or(SumeragiV2GeometryError::Overflow)?;
    let required_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        ingress_owners,
        authenticated_non_validator_sources,
    )
    .ok_or(SumeragiV2GeometryError::Overflow)?;
    if queue_bodies < required_bodies {
        return Err(SumeragiV2GeometryError::BodyQueueTooSmall {
            actual: queue_bodies,
            minimum: required_bodies,
        });
    }
    let body_bytes = actual::sumeragi_v2_body_ingress_required_byte_capacity(
        ingress_owners,
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .ok_or(SumeragiV2GeometryError::Overflow)?;
    Ok(SumeragiV2IngressGeometry {
        lifecycle,
        required_bodies,
        body_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(validators: usize) -> SumeragiV2IngressInputs {
        SumeragiV2IngressInputs {
            validators,
            queue_commands: 4096,
            queue_bodies: 1024,
            authenticated_non_validator_sources: 4,
            committee_sources: validators + 12,
            max_total_connections: validators - 1 + 12 + 4,
            body_source_bytes: 34 * 1024 * 1024,
        }
    }

    #[test]
    fn body_bytes_scale_with_every_ingress_owner() {
        let geometry = sumeragi_v2_ingress_geometry(inputs(4)).unwrap();
        assert_eq!(geometry.body_bytes, (4 + 16 + 4) * 34 * 1024 * 1024);
        assert_eq!(geometry.required_bodies, 5 * (4 + 16) + 3 * 4);
        assert_eq!(geometry.lifecycle.serve, (4 + 4 * 1024) * 2);
    }

    #[test]
    fn a_generator_without_committee_class_keeps_n_plus_h_bytes() {
        let geometry = sumeragi_v2_ingress_geometry(SumeragiV2IngressInputs {
            committee_sources: 0,
            ..inputs(7)
        })
        .unwrap();
        assert_eq!(geometry.body_bytes, (7 + 4) * 34 * 1024 * 1024);
    }

    #[test]
    fn rosters_outside_three_f_plus_one_are_rejected() {
        for validators in [0, 1, 3, 5, 6, 8] {
            assert!(matches!(
                sumeragi_v2_ingress_geometry(
                    SumeragiV2IngressInputs {
                        max_total_connections: 16,
                        ..inputs(4)
                    }
                    .with_validators(validators)
                ),
                Err(SumeragiV2GeometryError::NotExactCommittee { .. })
            ));
        }
        let oversized = MAX_VALIDATORS_PER_HEIGHT + 3;
        let error = sumeragi_v2_ingress_geometry(inputs(4).with_validators(oversized)).unwrap_err();
        assert!(matches!(
            error,
            SumeragiV2GeometryError::RosterAboveMaximum { .. }
        ));
        assert!(
            error
                .to_string()
                .contains("exceeds the Sumeragi v2 protocol maximum")
        );
    }

    #[test]
    fn undersized_queues_and_fanouts_are_rejected() {
        assert!(matches!(
            sumeragi_v2_ingress_geometry(SumeragiV2IngressInputs {
                queue_bodies: 16,
                ..inputs(4)
            }),
            Err(SumeragiV2GeometryError::BodyQueueTooSmall { minimum: 112, .. })
        ));
        assert!(matches!(
            sumeragi_v2_ingress_geometry(SumeragiV2IngressInputs {
                max_total_connections: 0,
                ..inputs(4)
            }),
            Err(SumeragiV2GeometryError::ExactOutput(_))
        ));
    }

    impl SumeragiV2IngressInputs {
        fn with_validators(self, validators: usize) -> Self {
            Self { validators, ..self }
        }
    }
}

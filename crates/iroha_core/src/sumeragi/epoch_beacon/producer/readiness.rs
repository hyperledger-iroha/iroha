//! Generation-bound diagnostics from the sole native pulse custodian.

use super::*;
use iroha_allocation::{AllocationBudget, ChargedShared};
use iroha_data_model::{governance::types::BeaconSessionId, sumeragi::BeaconHorizonStatusV1};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Observation {
    generation: u64,
    height: u64,
    applied: u64,
    horizon: BeaconHorizonStatusV1,
    ready: bool,
}

/// Shared fixed-size report: no key, proof, transcript or mutable signing API escapes.
#[derive(Clone, Debug)]
pub(crate) struct NativeBeaconReadiness(ChargedShared<Mutex<Option<Observation>>>);
impl NativeBeaconReadiness {
    pub(super) fn new(budget: &AllocationBudget) -> Result<Self, String> {
        let mut reservation = budget
            .try_reserve(ChargedShared::<Mutex<Option<Observation>>>::allocation_layout())
            .map_err(|error| error.to_string())?;
        ChargedShared::from_reservation(Mutex::new(None), &mut reservation)
            .map(Self)
            .map_err(|(_, error)| error.to_string())
    }

    /// A concurrent publication or a different core height invalidates the entire observation.
    pub(crate) fn read(
        &self,
        generation: u64,
        height: u64,
        applied: u64,
    ) -> Option<(BeaconHorizonStatusV1, bool)> {
        let observed = *self.0.lock().ok()?;
        observed
            .filter(|value| {
                generation % 2 == 0
                    && value.generation == generation
                    && value.height == height
                    && value.applied == applied
            })
            .map(|value| (value.horizon, value.ready))
    }
}

impl NativeBeaconProducer {
    /// Admit this reporting control once from the original State execution pool.
    pub(crate) fn attach_readiness(
        &mut self,
        budget: &AllocationBudget,
    ) -> Result<NativeBeaconReadiness, String> {
        if self.readiness.is_some() {
            return Err("native beacon readiness owner is already attached".into());
        }
        let reporting = NativeBeaconReadiness::new(budget)?;
        self.readiness = Some(reporting.clone());
        Ok(reporting)
    }

    /// Re-probe the same installed provider on every drive retry, including ordinary heights.
    /// The immutable source view remains held during the probe; this produces no partial.
    pub(crate) fn refresh_readiness(
        &self,
        state: &impl StateReadOnly,
        context: &ApplicationControlContext,
        applied: (u64, Hash32),
        generation: u64,
    ) -> Result<(), NativeBeaconError> {
        let Some(reporting) = &self.readiness else {
            return Ok(());
        };
        // Withdraw the old positive result before any fallible observation/probe.
        *reporting
            .0
            .lock()
            .map_err(|_| NativeBeaconError::Source("readiness lock poisoned".into()))? = None;
        if generation % 2 != 0 {
            return Err(NativeBeaconError::Context);
        }
        self.parent_source(state, context, applied)?;
        let retained = state.world().consensus_schedule();
        let current = &retained
            .ready(context.height)
            .map_err(|error| NativeBeaconError::Source(error.to_string()))?
            .epoch;
        if current.network_id != *state.network_id()
            || schedule::core_epoch(current)
                .map_err(|error| NativeBeaconError::Source(error.to_string()))?
                .id
                != context.epoch
        {
            return Err(NativeBeaconError::Context);
        }
        let world = state.world();
        let root_scope =
            crate::sumeragi::lanes::routing::committed_root_scope(world).ok_or_else(|| {
                NativeBeaconError::Source("native readiness requires immutable root scope".into())
            })?;
        if !super::super::owns_global_control(root_scope, world, current)
            .map_err(NativeBeaconError::Source)?
        {
            *reporting
                .0
                .lock()
                .map_err(|_| NativeBeaconError::Source("readiness lock poisoned".into()))? =
                Some(Observation {
                    generation,
                    height: context.height,
                    applied: applied.0,
                    horizon: BeaconHorizonStatusV1 {
                        epoch_length_blocks: 0,
                        next_required_pulse_height: None,
                        active_session_id: None,
                        session_covers_next_pulse: false,
                        local_provider_ready: false,
                    },
                    ready: true,
                });
            return Ok(());
        }
        let boundary_pulse = (current.mode == ConsensusMode::Npos)
            .then(|| current.authorization.last_height.checked_sub(1))
            .flatten()
            .filter(|height| *height >= context.height);
        let parliament = world
            .parliament_required_beacon_pulse_slots()
            .iter()
            .filter(|((session, height), attempts)| {
                *session == BeaconSessionId::for_network_v1(&current.network_id)
                    && *height >= context.height
                    && *height <= current.authorization.last_height
                    && !attempts.is_empty()
            })
            .map(|((_, height), _)| *height)
            .min();
        let next = boundary_pulse.into_iter().chain(parliament).min();
        let local =
            current
                .committee
                .iter()
                .position(|seat| {
                    seat.validator.public_key().try_to_bytes().ok().is_some_and(
                        |(algorithm, bytes)| {
                            algorithm == iroha_crypto::Algorithm::BlsNormal
                                && self
                                    .local_bls
                                    .as_ref()
                                    .is_some_and(|key| key.as_slice() == bytes)
                        },
                    )
                })
                .and_then(|index| u16::try_from(index + 1).ok());
        let mut horizon = BeaconHorizonStatusV1 {
            epoch_length_blocks: if current.mode == ConsensusMode::Npos {
                current.authorization.last_height - current.authorization.first_height + 1
            } else {
                0
            },
            next_required_pulse_height: next,
            active_session_id: world.active_global_beacon_key_session(),
            session_covers_next_pulse: false,
            local_provider_ready: false,
        };
        if let Some(id) = horizon.active_session_id {
            let record = world.global_beacon_key_sessions().get(&id).ok_or_else(|| {
                NativeBeaconError::Source("active readiness session is absent".into())
            })?;
            // Validate the acquired record once within this probe. Every retry still
            // reacquires its original State view and re-probes the installed custodian.
            let session = record
                .validated_session()
                .map_err(|error| NativeBeaconError::Source(error.to_string()))?;
            let binding = InstalledBeaconEpochBindingV1 {
                session_id: id,
                transcript_hash: record.session.transcript_hash,
            };
            let authenticated = record.session.network_id == current.network_id
                && record.session.adaptive_dkg.finalized_at_height <= applied.0
                && record.session.adaptive_dkg.session.authority_generation
                    == current.authority.generation
                && (current.authorization.beacon == BeaconEpochBindingV1::Bootstrap
                    || current.authorization.beacon == BeaconEpochBindingV1::Installed(binding));
            if authenticated {
                let peers = current
                    .committee
                    .iter()
                    .map(|seat| seat.validator.clone())
                    .collect::<Vec<_>>();
                let roster_hash =
                    authenticated_global_threshold_beacon_roster_hash_v1(&record.session, &peers)
                        .map_err(|error| NativeBeaconError::Source(error.to_string()))?;
                session
                    .validate_binding(&GlobalThresholdBeaconSessionBindingV1 {
                        network_id: current.network_id,
                        session_id: id,
                        roster_hash,
                        transcript_hash: record.session.transcript_hash,
                    })
                    .map_err(|error| NativeBeaconError::Source(error.to_string()))?;
                horizon.session_covers_next_pulse =
                    next.is_some_and(|height| record.is_active_at(height));
                horizon.local_provider_ready = record.is_active_at(context.height)
                    && local
                        .zip(self.signer.as_ref())
                        .is_some_and(|(index, signer)| {
                            signer
                                .attest_partial_signing_capability(&session, index)
                                .is_ok()
                        });
            }
        }
        // A local validator needs custody only when an authenticated demand remains. No-demand
        // permissioned nodes and observers do not invent a mandatory signing obligation.
        let ready = local.is_none()
            || next.is_none()
            || (horizon.session_covers_next_pulse && horizon.local_provider_ready);
        *reporting
            .0
            .lock()
            .map_err(|_| NativeBeaconError::Source("readiness lock poisoned".into()))? =
            Some(Observation {
                generation,
                height: context.height,
                applied: applied.0,
                horizon,
                ready,
            });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporting_shell_retains_original_pool_charge_until_last_reader_drops() {
        let bytes = ChargedShared::<Mutex<Option<Observation>>>::allocation_layout().size();
        let budget = AllocationBudget::new(bytes);
        let report = NativeBeaconReadiness::new(&budget).unwrap();
        assert_eq!(budget.reserved_bytes(), bytes);
        let reader = report.clone();
        assert!(NativeBeaconReadiness::new(&budget).is_err());
        drop(report);
        assert_eq!(budget.reserved_bytes(), bytes);
        drop(reader);
        assert_eq!(budget.reserved_bytes(), 0);
        assert!(NativeBeaconReadiness::new(&budget).is_ok());
    }
}

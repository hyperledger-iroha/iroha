//! Crash-safe startup recovery for local autonomous-lane lifecycle custody.
//!
//! This module is deliberately separate from the live height adapter. Startup first authenticates
//! the immutable Queue/Kura cut, completes every already-signed bootstrap, transfers stale local
//! cursor ownership to the current process generation, and restores only body custody proven by
//! the durable payload. The caller may publish the Queue startup gate only after this function
//! returns its original combined V4/V6 receipt.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, KeyPair, Signature};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

use super::v2_apply::{
    LaneReservationSnapshotPlannerEvidence, recover_pending_autonomous_lifecycle_terminal_outcome,
};
use super::v2_core::{
    IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA, IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY,
    ProductionInFlightFirstReleaseStateProjection,
    check_production_in_flight_first_release_crash_transition,
    check_production_in_flight_first_release_recover_transition,
    check_production_in_flight_first_release_rehydrate_local_kura_custody_transition,
};
use crate::{
    kura::{
        AutonomousLifecycleAttemptBindingV1, AutonomousLifecycleCursorPhaseKindV2,
        AutonomousLifecycleCursorPhaseV2, AutonomousLifecycleCursorRead,
        AutonomousLifecycleCursorUnsignedV2, AutonomousLifecycleCursorV2,
        AutonomousLifecyclePayloadCustodySourceV1,
        AutonomousLifecyclePendingReservationGroupObservation,
        AutonomousLifecyclePendingTerminalOutcomeRecovery,
        AutonomousLifecycleProcessGenerationClaim, AutonomousLifecycleTerminalOutcomeDurableStage,
        AutonomousLifecycleTerminalOutcomeSourceKind, Kura,
    },
    queue::{
        LaneQueueReservationGroupBindingV1, LaneQueueReservationGroupIdentityV1,
        LaneQueueReservationKeyV2, LaneQueueReservationReconciliationSnapshotV1,
        LaneReservationSnapshotLifecycleProjectionV1, LaneReservationStartupReconciliationReceipt,
        Queue, lane_queue_reservation_group_binding_from_ordered_keys,
    },
    state::{State, consensus_lane_dataspace_at_height},
};

#[cfg(test)]
std::thread_local! {
    static DEFERRED_TERMINAL_STAGE_PROOF_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
    static POST_LIFECYCLE_CURSOR_CAS_HOOK: std::cell::RefCell<
        Option<Box<dyn FnMut(&AutonomousLifecycleCursorV2)>>,
    > = std::cell::RefCell::new(None);
}

#[cfg(test)]
pub(crate) fn install_deferred_terminal_stage_proof_hook_for_test(hook: impl FnOnce() + 'static) {
    DEFERRED_TERMINAL_STAGE_PROOF_HOOK.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "deferred terminal test hook already installed"
        );
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
fn run_deferred_terminal_stage_proof_hook_for_test() {
    let hook = DEFERRED_TERMINAL_STAGE_PROOF_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

#[cfg(test)]
pub(crate) fn install_post_lifecycle_cursor_cas_hook_for_test(
    hook: impl FnMut(&AutonomousLifecycleCursorV2) + 'static,
) {
    POST_LIFECYCLE_CURSOR_CAS_HOOK.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "post-lifecycle-cursor-CAS test hook already installed"
        );
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
pub(crate) fn clear_post_lifecycle_cursor_cas_hook_for_test() {
    POST_LIFECYCLE_CURSOR_CAS_HOOK.with(|slot| drop(slot.borrow_mut().take()));
}

#[cfg(test)]
fn run_post_lifecycle_cursor_cas_hook_for_test(cursor: &AutonomousLifecycleCursorV2) {
    POST_LIFECYCLE_CURSOR_CAS_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().as_mut() {
            hook(cursor);
        }
    });
}

/// Exact Queue handoff retained after all local lifecycle recovery is durable.
#[must_use = "the recovered Queue receipt must reach the startup publication gate"]
pub(crate) struct RecoveredAutonomousLifecycleStartup {
    snapshot: LaneQueueReservationReconciliationSnapshotV1,
    receipt: LaneReservationStartupReconciliationReceipt,
    deferred_terminal_recovery: AutonomousLifecycleDeferredTerminalRecoveryHandoff,
    completed_bootstraps: usize,
    recovered_attempts: usize,
}

impl RecoveredAutonomousLifecycleStartup {
    /// Consume the recovery result into the exact Queue cut and its move-only receipt.
    pub(crate) fn into_queue_handoff(
        self,
    ) -> (
        LaneQueueReservationReconciliationSnapshotV1,
        LaneReservationStartupReconciliationReceipt,
        AutonomousLifecycleDeferredTerminalRecoveryHandoff,
    ) {
        (self.snapshot, self.receipt, self.deferred_terminal_recovery)
    }

    /// Number of signed bootstrap intents completed idempotently.
    #[must_use]
    pub(crate) const fn completed_bootstraps(&self) -> usize {
        self.completed_bootstraps
    }

    /// Number of non-terminal attempts transferred or rehydrated for this process generation.
    #[must_use]
    pub(crate) const fn recovered_attempts(&self) -> usize {
        self.recovered_attempts
    }
}

/// Bounded result of joining durable terminal sources with their exact Queue
/// ownership outcomes before ordinary reservation planning begins.
#[derive(Debug, Default)]
pub(crate) struct AutonomousLifecycleTerminalRecoverySummary {
    completed_outcomes: usize,
    finalized_reservations: usize,
    deferred_terminal_recovery: AutonomousLifecycleDeferredTerminalRecoveryHandoff,
}

impl AutonomousLifecycleTerminalRecoverySummary {
    /// Number of Pending Kura outcomes promoted through positive Queue proof.
    #[must_use]
    pub(crate) const fn completed_outcomes(&self) -> usize {
        self.completed_outcomes
    }

    /// Number of live Queue reservation owners consumed while catching up.
    pub(crate) const fn finalized_reservations(&self) -> usize {
        self.finalized_reservations
    }

    /// Number of exact Pending groups deferred behind the immutable Queue replay receipt.
    #[must_use]
    pub(crate) fn deferred_pending_groups(&self) -> usize {
        self.deferred_terminal_recovery.group_count()
    }

    /// Consume the pre-sweep result into the exact Pending bindings which the
    /// ordinary planner must terminalize before Queue publication.
    pub(crate) fn into_deferred_terminal_recovery(
        self,
    ) -> AutonomousLifecycleDeferredTerminalRecoveryHandoff {
        self.deferred_terminal_recovery
    }
}

type AutonomousLifecycleTerminalRecoveryUnitIdentity = (u8, Vec<Hash>);

#[derive(Debug)]
struct AutonomousLifecycleDeferredTerminalRecoveryUnit {
    identity: AutonomousLifecycleTerminalRecoveryUnitIdentity,
    pending_groups: Vec<AutonomousLifecyclePendingReservationGroupObservation>,
    owned_group_hashes: BTreeSet<Hash>,
}

/// Opaque move-only handoff from the receipt-safe pre-sweep to normal startup
/// planning and final application.
///
/// Each unit retains complete Pending carrier grouping, exact source keys, and
/// the subset which had an exact owner in the immutable Queue snapshot. No
/// caller can manufacture an absent carrier member or flatten a multi-lane
/// carrier into independently mutable groups.
#[derive(Debug, Default)]
pub(crate) struct AutonomousLifecycleDeferredTerminalRecoveryHandoff {
    units: Vec<AutonomousLifecycleDeferredTerminalRecoveryUnit>,
}

impl AutonomousLifecycleDeferredTerminalRecoveryHandoff {
    /// Construct the only valid caller-supplied form: no deferred source units.
    #[must_use]
    pub(crate) const fn empty() -> Self {
        Self { units: Vec::new() }
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    #[must_use]
    fn group_count(&self) -> usize {
        self.units
            .iter()
            .map(|unit| unit.pending_groups.len())
            .sum()
    }

    fn bindings_for_route(
        &self,
        lane_id: iroha_data_model::nexus::LaneId,
        dataspace_id: iroha_data_model::nexus::DataSpaceId,
        lane_incarnation: Hash,
    ) -> Vec<LaneQueueReservationGroupBindingV1> {
        self.units
            .iter()
            .flat_map(|unit| unit.pending_groups.iter())
            .map(AutonomousLifecyclePendingReservationGroupObservation::binding)
            .filter(|group| {
                group.identity.lane_id == lane_id
                    && group.identity.dataspace_id == dataspace_id
                    && group.identity.lane_incarnation == lane_incarnation
            })
            .collect()
    }
}

fn lifecycle_error(stage: &str, error: impl core::fmt::Display) -> String {
    format!("autonomous lifecycle startup {stage}: {error}")
}

fn active_lifecycle_routes(
    state: &State,
    context: &wire::HeightContext,
) -> Result<
    Vec<(
        iroha_data_model::nexus::LaneId,
        iroha_data_model::nexus::DataSpaceId,
        Hash,
    )>,
    String,
> {
    let nexus = state.nexus_snapshot();
    let incarnations = state.lane_incarnations_snapshot();
    let mut routes = BTreeSet::new();
    for entry in nexus.lane_config.entries() {
        let Some(dataspace_id) =
            consensus_lane_dataspace_at_height(entry.lane_id, &nexus, context.height)
        else {
            continue;
        };
        if dataspace_id != entry.dataspace_id {
            return Err(format!(
                "active lane {} has divergent State/config dataspace identities",
                entry.lane_id.as_u32()
            ));
        }
        let lane_incarnation = incarnations.get(&entry.lane_id).copied().ok_or_else(|| {
            format!(
                "active lane {} lacks its non-zero State incarnation",
                entry.lane_id.as_u32()
            )
        })?;
        if lane_incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(format!(
                "active lane {} has a zero State incarnation",
                entry.lane_id.as_u32()
            ));
        }
        routes.insert((entry.lane_id, dataspace_id, lane_incarnation));
    }
    if routes.is_empty() {
        return Err("the authenticated active context has no lifecycle storage route".to_owned());
    }
    Ok(routes.into_iter().collect())
}

pub(crate) fn sign_lifecycle_cursor(
    key_pair: &KeyPair,
    local_peer: &PeerId,
    validator_set: &[PeerId],
    sequence: u64,
    previous_cursor_hash: Option<Hash>,
    binding: AutonomousLifecycleAttemptBindingV1,
    phase: AutonomousLifecycleCursorPhaseV2,
) -> Result<AutonomousLifecycleCursorV2, String> {
    let unsigned = AutonomousLifecycleCursorUnsignedV2::new(
        sequence,
        previous_cursor_hash,
        binding,
        phase,
        local_peer.clone(),
    )
    .map_err(|reason| lifecycle_error("cursor construction failed", reason))?;
    let preimage = unsigned
        .signing_preimage()
        .map_err(|error| lifecycle_error("cursor preimage encoding failed", error))?;
    let signature = Signature::try_new(key_pair.private_key(), &preimage)
        .map_err(|error| lifecycle_error("cursor signing failed", error))?;
    let signature = <[u8; 96]>::try_from(signature.payload()).map_err(|_| {
        "autonomous lifecycle startup signer did not produce an exact 96-byte BLS signature"
            .to_owned()
    })?;
    unsigned
        .finalize(signature, validator_set)
        .map_err(|reason| lifecycle_error("cursor signature verification failed", reason))
}

fn compare_and_swap_phase(
    kura: &Kura,
    key_pair: &KeyPair,
    local_peer: &PeerId,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    read: AutonomousLifecycleCursorRead,
    phase: AutonomousLifecycleCursorPhaseV2,
) -> Result<AutonomousLifecycleCursorRead, String> {
    let (current, lease) = read.into_parts();
    let sequence = current
        .as_ref()
        .map_or(Ok(1), |cursor| cursor.sequence().checked_add(1).ok_or(()))
        .map_err(|()| "autonomous lifecycle startup cursor sequence is exhausted".to_owned())?;
    let previous_cursor_hash = current
        .as_ref()
        .map(AutonomousLifecycleCursorV2::cursor_hash);
    let binding = current
        .as_ref()
        .map(|cursor| cursor.binding().clone())
        .ok_or_else(|| {
            "autonomous lifecycle startup cannot advance a cursorless retained payload".to_owned()
        })?;
    let next = sign_lifecycle_cursor(
        key_pair,
        local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        sequence,
        previous_cursor_hash,
        binding,
        phase,
    )?;
    let read = kura
        .compare_and_swap_autonomous_lifecycle_cursor(lease, next)
        .map_err(|error| lifecycle_error("cursor compare-and-swap failed", error))?;
    #[cfg(test)]
    run_post_lifecycle_cursor_cas_hook_for_test(
        read.cursor()
            .expect("successful lifecycle cursor CAS returns its durable cursor"),
    );
    Ok(read)
}

fn prepared_recovery_state(
    cursor: &AutonomousLifecycleCursorV2,
) -> Result<ProductionInFlightFirstReleaseStateProjection, String> {
    let transition = cursor
        .prepared_transition_projection()
        .map_err(|reason| lifecycle_error("prepared cursor validation failed", reason))?
        .ok_or_else(|| {
            "autonomous lifecycle startup expected a Prepared transition projection".to_owned()
        })?;
    match transition.action {
        // The active-attempt inventory proves the exact immutable payload has reached Kura.
        IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA => Ok(transition.after),
        // Crash, Recover, and body rehydration have no separately durable mutation outside the
        // cursor. A Prepared head therefore resolves to its complete before-state.
        IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH
        | IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER
        | IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY => Ok(transition.before),
        _ => Err(format!(
            "unsupported Prepared lifecycle action {} lacks a source-specific startup resolver",
            transition.action
        )),
    }
}

fn cursor_recovery_state(
    cursor: &AutonomousLifecycleCursorV2,
) -> Result<ProductionInFlightFirstReleaseStateProjection, String> {
    match cursor.phase_kind() {
        AutonomousLifecycleCursorPhaseKindV2::Prepared => prepared_recovery_state(cursor),
        AutonomousLifecycleCursorPhaseKindV2::Live
        | AutonomousLifecycleCursorPhaseKindV2::Terminal => cursor
            .before_projection()
            .map_err(|reason| lifecycle_error("stable cursor projection failed", reason)),
        AutonomousLifecycleCursorPhaseKindV2::Crashed => cursor
            .after_projection()
            .map_err(|reason| lifecycle_error("crashed cursor projection failed", reason))?
            .ok_or_else(|| {
                "autonomous lifecycle startup crashed cursor lacks its after-state".to_owned()
            }),
    }
}

fn exact_current_queue_group_matches(
    binding: &AutonomousLifecycleAttemptBindingV1,
    ordered_keys: &[LaneQueueReservationKeyV2],
    current_queue_groups: &BTreeMap<
        LaneQueueReservationGroupIdentityV1,
        Vec<LaneQueueReservationKeyV2>,
    >,
) -> bool {
    let expected = binding.reservation_group_binding();
    let Some(current_keys) = current_queue_groups.get(&expected.identity) else {
        return false;
    };
    usize::try_from(expected.reservation_count).ok() == Some(ordered_keys.len())
        && current_keys.as_slice() == ordered_keys
        && lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).ok()
            == Some(expected)
        && lane_queue_reservation_group_binding_from_ordered_keys(current_keys.iter()).ok()
            == Some(expected)
}

/// A producer cursor is signed against the Queue owner which selected the
/// executable payload. Unlike an observer's replicated Kura custody, that
/// local producer authority cannot be reconstructed from the payload alone.
/// Require the complete current V6 owner set before any Crash/Recover CAS.
fn require_local_producer_queue_owner(
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    cursor: &AutonomousLifecycleCursorV2,
    current_queue_groups: &BTreeMap<
        LaneQueueReservationGroupIdentityV1,
        Vec<LaneQueueReservationKeyV2>,
    >,
) -> Result<(), String> {
    let binding = cursor.binding();
    let (_, local_actor) = binding.local_validator_identity();
    if local_actor != binding.producer_actor_projection() {
        return Ok(());
    }
    if !exact_current_queue_group_matches(binding, &payload.reservation_keys, current_queue_groups)
    {
        return Err(
            "autonomous lifecycle local producer cursor lost its exact current Queue reservation owner"
                .to_owned(),
        );
    }
    Ok(())
}

fn recover_one_attempt(
    kura: &Kura,
    process_generation: &AutonomousLifecycleProcessGenerationClaim,
    key_pair: &KeyPair,
    local_peer: &PeerId,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    binding: &AutonomousLifecycleAttemptBindingV1,
) -> Result<bool, String> {
    let current_generation = process_generation.generation();
    let local_actor = binding.local_validator_identity().1;
    let mut changed = false;
    for _ in 0..8 {
        let read = kura
            .read_autonomous_lifecycle_cursor(payload, binding, process_generation)
            .map_err(|error| lifecycle_error("cursor read failed", error))?;
        let cursor = read.cursor().cloned().ok_or_else(|| {
            "autonomous lifecycle startup inventory retained a cursorless payload after bootstrap completion"
                .to_owned()
        })?;
        if cursor.phase_kind() == AutonomousLifecycleCursorPhaseKindV2::Terminal {
            return Ok(changed);
        }
        let owner_generation = cursor.owner_generation();
        if owner_generation > current_generation {
            return Err(
                "autonomous lifecycle startup cursor belongs to a future process generation"
                    .to_owned(),
            );
        }

        if owner_generation < current_generation {
            let before = cursor_recovery_state(&cursor)?;
            let phase = if before.session.crashed & local_actor != 0 {
                AutonomousLifecycleCursorPhaseV2::observed_crashed(
                    owner_generation,
                    current_generation,
                    before,
                )
            } else {
                let crash = check_production_in_flight_first_release_crash_transition(
                    before,
                    local_actor,
                )
                .ok_or_else(|| {
                    "autonomous lifecycle startup Crash transition failed the production kernel"
                        .to_owned()
                })?
                .into_projection();
                AutonomousLifecycleCursorPhaseV2::crashed(
                    owner_generation,
                    current_generation,
                    crash.before,
                    crash.after,
                )
            }
            .map_err(|reason| lifecycle_error("crash phase construction failed", reason))?;
            let _ = compare_and_swap_phase(kura, key_pair, local_peer, payload, read, phase)?;
            changed = true;
            continue;
        }

        match cursor.phase_kind() {
            AutonomousLifecycleCursorPhaseKindV2::Crashed => {
                let before = cursor_recovery_state(&cursor)?;
                let recover = check_production_in_flight_first_release_recover_transition(
                    before,
                    local_actor,
                )
                .ok_or_else(|| {
                    "autonomous lifecycle startup Recover transition failed the production kernel"
                        .to_owned()
                })?
                .into_projection();
                let phase = AutonomousLifecycleCursorPhaseV2::prepared(current_generation, recover)
                    .map_err(|reason| lifecycle_error("Recover preparation failed", reason))?;
                let _ = compare_and_swap_phase(kura, key_pair, local_peer, payload, read, phase)?;
                changed = true;
            }
            AutonomousLifecycleCursorPhaseKindV2::Prepared => {
                let transition = cursor
                    .prepared_transition_projection()
                    .map_err(|reason| lifecycle_error("prepared cursor validation failed", reason))?
                    .ok_or_else(|| {
                        "autonomous lifecycle startup Prepared cursor lost its transition"
                            .to_owned()
                    })?;
                if !matches!(
                    transition.action,
                    IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA
                        | IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER
                        | IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
                ) {
                    return Err(format!(
                        "cannot complete unsupported Prepared lifecycle action {} during startup",
                        transition.action
                    ));
                }
                let phase =
                    AutonomousLifecycleCursorPhaseV2::live(current_generation, transition.after)
                        .map_err(|reason| {
                            lifecycle_error("Live phase construction failed", reason)
                        })?;
                let _ = compare_and_swap_phase(kura, key_pair, local_peer, payload, read, phase)?;
                changed = true;
            }
            AutonomousLifecycleCursorPhaseKindV2::Live => {
                let before = cursor_recovery_state(&cursor)?;
                if before.session.bodies & local_actor != 0 {
                    return Ok(changed);
                }
                let rehydrate =
                    check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(
                        before,
                        local_actor,
                    )
                    .ok_or_else(|| {
                        "autonomous lifecycle startup RehydrateLocalKuraCustody transition failed the production kernel"
                            .to_owned()
                    })?
                    .into_projection();
                let phase =
                    AutonomousLifecycleCursorPhaseV2::prepared(current_generation, rehydrate)
                        .map_err(|reason| {
                            lifecycle_error("rehydration preparation failed", reason)
                        })?;
                let _ = compare_and_swap_phase(kura, key_pair, local_peer, payload, read, phase)?;
                changed = true;
            }
            AutonomousLifecycleCursorPhaseKindV2::Terminal => return Ok(changed),
        }
    }
    Err("autonomous lifecycle startup exceeded its fixed eight-transition attempt bound".to_owned())
}

fn lifecycle_projection_for_cursor(
    queue: &Queue,
    receipt: &LaneReservationStartupReconciliationReceipt,
    cursor: &AutonomousLifecycleCursorV2,
    ordered_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
) -> Result<LaneReservationSnapshotLifecycleProjectionV1, String> {
    let recovered_state = match cursor.phase_kind() {
        AutonomousLifecycleCursorPhaseKindV2::Prepared => prepared_recovery_state(cursor)?,
        AutonomousLifecycleCursorPhaseKindV2::Crashed => cursor_recovery_state(cursor)?,
        AutonomousLifecycleCursorPhaseKindV2::Live
        | AutonomousLifecycleCursorPhaseKindV2::Terminal => {
            return queue
                .select_lane_reservation_snapshot_lifecycle_projection(
                    receipt,
                    cursor,
                    ordered_keys,
                )
                .map_err(|error| lifecycle_error("Queue cursor-state selection failed", error));
        }
    };
    LaneReservationSnapshotLifecycleProjectionV1::from_authenticated_cursor(
        cursor,
        ordered_keys,
        recovered_state,
    )
    .map_err(|error| lifecycle_error("Queue cursor projection failed", error))
}

fn lifecycle_identity_projection_for_cursor(
    cursor: &AutonomousLifecycleCursorV2,
    ordered_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
) -> Result<LaneReservationSnapshotLifecycleProjectionV1, String> {
    let recovered_state = match cursor.phase_kind() {
        AutonomousLifecycleCursorPhaseKindV2::Prepared => prepared_recovery_state(cursor)?,
        AutonomousLifecycleCursorPhaseKindV2::Crashed => cursor_recovery_state(cursor)?,
        AutonomousLifecycleCursorPhaseKindV2::Live
        | AutonomousLifecycleCursorPhaseKindV2::Terminal => cursor
            .before_projection()
            .map_err(|reason| lifecycle_error("signed identity state is invalid", reason))?,
    };
    LaneReservationSnapshotLifecycleProjectionV1::from_authenticated_cursor(
        cursor,
        ordered_keys,
        recovered_state,
    )
    .map_err(|error| lifecycle_error("Queue cursor identity projection failed", error))
}

fn planner_covered_pending_groups_for_route(
    deferred_terminal_recovery: &AutonomousLifecycleDeferredTerminalRecoveryHandoff,
    lane_id: iroha_data_model::nexus::LaneId,
    dataspace_id: iroha_data_model::nexus::DataSpaceId,
    lane_incarnation: Hash,
) -> Vec<LaneQueueReservationGroupBindingV1> {
    deferred_terminal_recovery.bindings_for_route(lane_id, dataspace_id, lane_incarnation)
}

fn observer_retirement_lifecycle_projections(
    kura: &Kura,
    chain_id_hash: Hash,
    local_peer: &PeerId,
    paired_groups: &BTreeSet<LaneQueueReservationGroupIdentityV1>,
    deferred_terminal_recovery: &AutonomousLifecycleDeferredTerminalRecoveryHandoff,
) -> Result<Vec<LaneReservationSnapshotLifecycleProjectionV1>, String> {
    let routes = paired_groups
        .iter()
        .map(|identity| {
            (
                identity.lane_id,
                identity.dataspace_id,
                identity.lane_incarnation,
            )
        })
        .collect::<BTreeSet<_>>();
    let mut projections = BTreeMap::new();
    for (lane_id, dataspace_id, lane_incarnation) in routes {
        let route_pending_groups = planner_covered_pending_groups_for_route(
            deferred_terminal_recovery,
            lane_id,
            dataspace_id,
            lane_incarnation,
        );
        let attempts = kura
            .read_only_active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
                chain_id_hash,
                local_peer,
                lane_id,
                dataspace_id,
                lane_incarnation,
                &route_pending_groups,
            )
            .map_err(|error| lifecycle_error("observer paired-cursor inventory failed", error))?;
        for attempt in attempts {
            let payload = attempt.executable_payload();
            let identity = LaneQueueReservationGroupIdentityV1::from_key(
                payload.reservation_keys.first().ok_or_else(|| {
                    "observer lifecycle inventory retained a payload without a reservation identity"
                        .to_owned()
                })?,
            );
            if !paired_groups.contains(&identity) {
                continue;
            }
            let cursor = attempt.cursor().ok_or_else(|| {
                "observer retirement pairing lacks its exact signed lifecycle cursor".to_owned()
            })?;
            let projection =
                lifecycle_identity_projection_for_cursor(cursor, payload.reservation_keys.clone())?;
            if projections.insert(identity, projection).is_some() {
                return Err(
                    "observer retirement pairing found duplicate signed cursor coverage".to_owned(),
                );
            }
        }
    }
    let observed = projections.keys().copied().collect::<BTreeSet<_>>();
    if observed.len() != paired_groups.len() || !observed.is_subset(paired_groups) {
        return Err(
            "observer retirement pairing does not cover every exact planner-paired Queue group"
                .to_owned(),
        );
    }
    Ok(projections.into_values().collect())
}

struct AutonomousLifecycleTerminalRecoveryPreflight {
    recovery: AutonomousLifecyclePendingTerminalOutcomeRecovery,
    pending_groups: Vec<AutonomousLifecyclePendingReservationGroupObservation>,
    deferred: bool,
}

fn pending_terminal_recovery_observations(
    recovery: &AutonomousLifecyclePendingTerminalOutcomeRecovery,
    chain_id_hash: Hash,
    active_routes: &BTreeSet<(
        iroha_data_model::nexus::LaneId,
        iroha_data_model::nexus::DataSpaceId,
        Hash,
    )>,
) -> Result<
    (
        Vec<AutonomousLifecyclePendingReservationGroupObservation>,
        AutonomousLifecycleTerminalRecoveryUnitIdentity,
    ),
    String,
> {
    if recovery.chain_id_hash() != chain_id_hash {
        return Err(
            "autonomous lifecycle terminal recovery targets another chain context".to_owned(),
        );
    }
    let route_identities = recovery.route_identities();
    if route_identities.is_empty()
        || route_identities
            .iter()
            .any(|identity| !active_routes.contains(identity))
    {
        return Err(
            "autonomous lifecycle terminal recovery targets an empty or stale State route/incarnation set"
                .to_owned(),
        );
    }
    let pending_groups = recovery.pending_reservation_groups().ok_or_else(|| {
        "autonomous lifecycle terminal recovery has malformed Pending reservation coordinates"
            .to_owned()
    })?;
    if pending_groups.is_empty()
        || pending_groups.len() != recovery.pending_outcome_count()
        || pending_groups.iter().any(|observation| {
            let binding = observation.binding();
            lane_queue_reservation_group_binding_from_ordered_keys(
                observation.ordered_keys().iter(),
            )
            .ok()
                != Some(binding)
                || !active_routes.contains(&(
                    binding.identity.lane_id,
                    binding.identity.dataspace_id,
                    binding.identity.lane_incarnation,
                ))
        })
    {
        return Err(
            "autonomous lifecycle terminal recovery changed its exact Pending Queue groups"
                .to_owned(),
        );
    }
    let kind = match recovery {
        AutonomousLifecyclePendingTerminalOutcomeRecovery::Canonical(_) => 0,
        AutonomousLifecyclePendingTerminalOutcomeRecovery::RetiredRelease { .. } => 1,
    };
    let unit_identity = (
        kind,
        pending_groups
            .iter()
            .map(|observation| observation.binding().reservation_group_hash)
            .collect(),
    );
    Ok((pending_groups, unit_identity))
}

fn pending_terminal_group_has_exact_queue_owner(
    snapshot: &LaneQueueReservationReconciliationSnapshotV1,
    observation: &AutonomousLifecyclePendingReservationGroupObservation,
) -> Result<bool, String> {
    let binding = observation.binding();
    let expected_keys = observation
        .ordered_keys()
        .iter()
        .map(|key| (key.signed_transaction_hash, key))
        .collect::<BTreeMap<_, _>>();
    if expected_keys.len() != observation.ordered_keys().len() {
        return Err(
            "autonomous lifecycle terminal recovery duplicates one source Queue key".to_owned(),
        );
    }

    let mut has_exact_owner = false;
    let mut seen_owner_hashes = BTreeSet::new();
    for phase in &snapshot.ordered_owner_phases {
        let phase_identity = LaneQueueReservationGroupIdentityV1::from_key(&phase.key);
        let expected = expected_keys
            .get(&phase.key.signed_transaction_hash)
            .copied();
        if phase_identity == binding.identity {
            if expected != Some(&phase.key)
                || !seen_owner_hashes.insert(phase.key.signed_transaction_hash)
            {
                return Err(
                    "autonomous lifecycle terminal recovery conflicts with a same-identity Queue owner"
                        .to_owned(),
                );
            }
            has_exact_owner = true;
        } else if expected.is_some() {
            return Err(
                "autonomous lifecycle terminal recovery transaction is owned by another Queue identity"
                    .to_owned(),
            );
        }
    }
    Ok(has_exact_owner)
}

/// Complete only already-empty crash-stranded terminal joins before taking the
/// immutable Queue receipt used by ordinary reservation planning.
///
/// The complete Kura inventory and one atomic Queue snapshot are validated
/// before the first mutation. A canonical carrier is an indivisible recovery
/// unit: if any Pending member retains an exact Queue owner, every Pending
/// member is deferred into the normal planner/apply path. Same-identity key
/// substitutions and cross-unit aliases fail before an empty join is touched.
pub(crate) fn reconcile_pending_autonomous_lifecycle_terminal_outcomes(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    context: &wire::HeightContext,
) -> Result<AutonomousLifecycleTerminalRecoverySummary, String> {
    let initial_queue_quarantine = queue.lane_reservation_startup_reconciliation_pending();
    let initial_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .map_err(|error| lifecycle_error("terminal-outcome Queue snapshot failed", error))?;
    if !initial_snapshot.is_empty() && !initial_queue_quarantine {
        return Err(
            "non-empty Queue startup snapshot was published before terminal-outcome pre-sweep"
                .to_owned(),
        );
    }
    let active_routes = active_lifecycle_routes(state, context)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let chain_id_hash = Hash::prehashed(*context.network_id.as_bytes());
    let recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .map_err(|error| lifecycle_error("terminal-outcome inventory failed", error))?;
    let mut seen_group_hashes = BTreeMap::new();
    let mut seen_group_identities = BTreeMap::new();
    let mut seen_transaction_hashes = BTreeMap::new();
    let mut seen_entrypoint_hashes = BTreeSet::new();
    let mut expected_deferred_units = BTreeSet::new();
    let mut deferred_units = Vec::new();
    let mut preflighted = Vec::with_capacity(recoveries.len());
    for recovery in recoveries {
        let (pending_groups, unit_identity) =
            pending_terminal_recovery_observations(&recovery, chain_id_hash, &active_routes)?;
        let mut owned_group_hashes = BTreeSet::new();
        for observation in &pending_groups {
            let binding = observation.binding();
            if seen_group_hashes
                .insert(binding.reservation_group_hash, binding)
                .is_some()
                || seen_group_identities
                    .insert(binding.identity, binding)
                    .is_some()
            {
                return Err(
                    "autonomous lifecycle terminal recovery aliases one Pending Queue group"
                        .to_owned(),
                );
            }
            for key in observation.ordered_keys() {
                if seen_transaction_hashes
                    .insert(key.signed_transaction_hash, *key)
                    .is_some()
                    || !seen_entrypoint_hashes.insert(key.entrypoint_hash.clone())
                {
                    return Err(
                        "autonomous lifecycle terminal recoveries overlap one transaction or entrypoint owner"
                            .to_owned(),
                    );
                }
            }
            if pending_terminal_group_has_exact_queue_owner(&initial_snapshot, observation)? {
                owned_group_hashes.insert(binding.reservation_group_hash);
            }
        }
        let deferred = !owned_group_hashes.is_empty();
        if deferred {
            if !expected_deferred_units.insert(unit_identity.clone()) {
                return Err(
                    "autonomous lifecycle terminal recovery duplicates one deferred unit"
                        .to_owned(),
                );
            }
            deferred_units.push(AutonomousLifecycleDeferredTerminalRecoveryUnit {
                identity: unit_identity,
                pending_groups: pending_groups.clone(),
                owned_group_hashes,
            });
        }
        preflighted.push(AutonomousLifecycleTerminalRecoveryPreflight {
            recovery,
            pending_groups,
            deferred,
        });
    }

    let mut completed_outcomes = 0_usize;
    let mut finalized_reservations = 0_usize;
    for preflight in preflighted {
        if preflight.deferred {
            continue;
        }
        completed_outcomes = completed_outcomes
            .checked_add(preflight.pending_groups.len())
            .ok_or_else(|| "autonomous lifecycle terminal recovery count overflowed".to_owned())?;
        let finalized = recover_pending_autonomous_lifecycle_terminal_outcome(
            state,
            queue,
            kura,
            preflight.recovery,
        )
        .map_err(|error| lifecycle_error("terminal-outcome application failed", error))?;
        if finalized != 0 {
            return Err(
                "pre-planner autonomous terminal recovery consumed a Queue owner".to_owned(),
            );
        }
        finalized_reservations = finalized_reservations.saturating_add(finalized);
    }
    if queue
        .lane_reservation_reconciliation_snapshot()
        .map_err(|error| lifecycle_error("terminal-outcome Queue readback failed", error))?
        != initial_snapshot
    {
        return Err(
            "autonomous lifecycle terminal recovery changed the immutable Queue snapshot"
                .to_owned(),
        );
    }
    if queue.lane_reservation_startup_reconciliation_pending() != initial_queue_quarantine {
        return Err(
            "autonomous lifecycle terminal recovery changed the Queue owner-quarantine state"
                .to_owned(),
        );
    }
    let remaining = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .map_err(|error| lifecycle_error("terminal-outcome readback failed", error))?;
    let mut observed_deferred_units = BTreeSet::new();
    let mut observed_deferred_groups = BTreeMap::new();
    for recovery in &remaining {
        let (pending_groups, unit_identity) =
            pending_terminal_recovery_observations(recovery, chain_id_hash, &active_routes)?;
        if !observed_deferred_units.insert(unit_identity) {
            return Err(
                "autonomous lifecycle terminal recovery readback changed a deferred Queue unit"
                    .to_owned(),
            );
        }
        let mut unit_has_exact_owner = false;
        for observation in &pending_groups {
            let binding = observation.binding();
            if observed_deferred_groups
                .insert(binding.reservation_group_hash, binding)
                .is_some()
            {
                return Err(
                    "autonomous lifecycle terminal recovery readback duplicated a deferred Queue group"
                        .to_owned(),
                );
            }
            unit_has_exact_owner |=
                pending_terminal_group_has_exact_queue_owner(&initial_snapshot, observation)?;
        }
        if !unit_has_exact_owner {
            return Err(
                "autonomous lifecycle terminal recovery readback lost its deferred Queue owner"
                    .to_owned(),
            );
        }
    }
    let deferred_terminal_recovery = AutonomousLifecycleDeferredTerminalRecoveryHandoff {
        units: deferred_units,
    };
    let expected_deferred_groups = deferred_terminal_recovery
        .units
        .iter()
        .flat_map(|unit| unit.pending_groups.iter())
        .map(|observation| {
            let binding = observation.binding();
            (binding.reservation_group_hash, binding)
        })
        .collect::<BTreeMap<_, _>>();
    if observed_deferred_units != expected_deferred_units
        || observed_deferred_groups != expected_deferred_groups
    {
        return Err(
            "autonomous lifecycle terminal recovery readback differs from the exact deferred set"
                .to_owned(),
        );
    }
    Ok(AutonomousLifecycleTerminalRecoverySummary {
        completed_outcomes,
        finalized_reservations,
        deferred_terminal_recovery,
    })
}

/// Close any planner-covered Pending sources left after the normal Queue
/// actions, without consuming another owner or rebinding the startup receipt.
///
/// The opaque handoff preserves whole-carrier grouping and exact source keys.
/// Every remaining Kura recovery must be a source-revalidated subset of one
/// such unit, and the complete unit must now be Queue-empty before any Kura
/// Pending file is promoted to Complete.
pub(crate) fn complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    expected_chain_id_hash: Hash,
    deferred: AutonomousLifecycleDeferredTerminalRecoveryHandoff,
) -> Result<usize, String> {
    let queue_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .map_err(|error| lifecycle_error("deferred terminal Queue snapshot failed", error))?;
    let mut expected_by_group = BTreeMap::new();
    for (unit_index, unit) in deferred.units.iter().enumerate() {
        let observed_identity = (
            unit.identity.0,
            unit.pending_groups
                .iter()
                .map(|observation| observation.binding().reservation_group_hash)
                .collect::<Vec<_>>(),
        );
        if unit.owned_group_hashes.is_empty() || observed_identity != unit.identity {
            return Err("deferred autonomous terminal handoff is malformed".to_owned());
        }
        for (group_position, observation) in unit.pending_groups.iter().enumerate() {
            let binding = observation.binding();
            if expected_by_group
                .insert(
                    binding.reservation_group_hash,
                    (unit_index, group_position, observation),
                )
                .is_some()
                || pending_terminal_group_has_exact_queue_owner(&queue_snapshot, observation)?
            {
                return Err(
                    "deferred autonomous terminal unit still has a Queue owner after normal actions"
                        .to_owned(),
                );
            }
        }
    }

    let expected_groups = deferred
        .units
        .iter()
        .flat_map(|unit| unit.pending_groups.iter().cloned())
        .collect::<Vec<_>>();
    #[cfg(test)]
    run_deferred_terminal_stage_proof_hook_for_test();
    let durable_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            expected_chain_id_hash,
            &expected_groups,
        )
        .map_err(|error| lifecycle_error("deferred terminal stage proof failed", error))?;
    let mut expected_pending_groups = BTreeSet::new();
    let mut observed_stage_groups = BTreeSet::new();
    for durable_stage in durable_stages {
        let binding = durable_stage.binding();
        let Some((unit_index, _group_position, expected)) = expected_by_group
            .get(&binding.reservation_group_hash)
            .copied()
        else {
            return Err(
                "deferred autonomous terminal stage proof contains an unplanned group".to_owned(),
            );
        };
        let source_kind_matches = matches!(
            (
                deferred.units[unit_index].identity.0,
                durable_stage.source_kind()
            ),
            (
                0,
                AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
            ) | (
                1,
                AutonomousLifecycleTerminalOutcomeSourceKind::RetiredRelease
            )
        );
        if expected.binding() != binding
            || !source_kind_matches
            || !observed_stage_groups.insert(binding.reservation_group_hash)
        {
            return Err(
                "deferred autonomous terminal stage proof differs from its exact handoff"
                    .to_owned(),
            );
        }
        if durable_stage.stage() == AutonomousLifecycleTerminalOutcomeDurableStage::Pending {
            expected_pending_groups.insert(binding.reservation_group_hash);
        }
    }
    if observed_stage_groups.len() != expected_by_group.len() {
        return Err(
            "deferred autonomous terminal stage proof omitted an expected handoff group".to_owned(),
        );
    }

    let recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .map_err(|error| lifecycle_error("deferred terminal inventory failed", error))?;
    let mut preflighted = Vec::with_capacity(recoveries.len());
    let mut observed_groups = BTreeSet::new();
    let mut observed_units = BTreeSet::new();
    for recovery in recoveries {
        if recovery.chain_id_hash() != expected_chain_id_hash {
            return Err(
                "deferred autonomous terminal recovery targets another chain context".to_owned(),
            );
        }
        let pending_groups = recovery.pending_reservation_groups().ok_or_else(|| {
            "deferred autonomous terminal recovery has malformed Pending coordinates".to_owned()
        })?;
        if pending_groups.is_empty() || pending_groups.len() != recovery.pending_outcome_count() {
            return Err(
                "deferred autonomous terminal recovery changed its Pending group count".to_owned(),
            );
        }
        let kind = match &recovery {
            AutonomousLifecyclePendingTerminalOutcomeRecovery::Canonical(_) => 0,
            AutonomousLifecyclePendingTerminalOutcomeRecovery::RetiredRelease { .. } => 1,
        };
        let mut recovery_unit = None;
        let mut previous_group_position = None;
        for observation in &pending_groups {
            let binding = observation.binding();
            let Some((unit_index, group_position, expected)) = expected_by_group
                .get(&binding.reservation_group_hash)
                .copied()
            else {
                return Err(
                    "deferred autonomous terminal inventory contains an unplanned Pending group"
                        .to_owned(),
                );
            };
            if expected != observation
                || deferred.units[unit_index].identity.0 != kind
                || recovery_unit.is_some_and(|existing| existing != unit_index)
                || previous_group_position.is_some_and(|previous| previous >= group_position)
                || !observed_groups.insert(binding.reservation_group_hash)
            {
                return Err(
                    "deferred autonomous terminal recovery differs from its whole-unit handoff"
                        .to_owned(),
                );
            }
            recovery_unit = Some(unit_index);
            previous_group_position = Some(group_position);
        }
        let unit_index = recovery_unit.ok_or_else(|| {
            "deferred autonomous terminal recovery has no exact handoff unit".to_owned()
        })?;
        if !observed_units.insert(unit_index) {
            return Err(
                "deferred autonomous terminal inventory split one whole recovery unit".to_owned(),
            );
        }
        preflighted.push((recovery, pending_groups.len()));
    }
    if observed_groups != expected_pending_groups {
        return Err(
            "deferred autonomous terminal Pending inventory differs from its durable stage proof"
                .to_owned(),
        );
    }

    let mut completed = 0_usize;
    for (recovery, pending_count) in preflighted {
        let finalized =
            recover_pending_autonomous_lifecycle_terminal_outcome(state, queue, kura, recovery)
                .map_err(|error| lifecycle_error("deferred terminal application failed", error))?;
        if finalized != 0 {
            return Err(
                "deferred autonomous terminal completion consumed a post-plan Queue owner"
                    .to_owned(),
            );
        }
        completed = completed
            .checked_add(pending_count)
            .ok_or_else(|| "deferred terminal completion count overflowed".to_owned())?;
    }
    if queue
        .lane_reservation_reconciliation_snapshot()
        .map_err(|error| lifecycle_error("deferred terminal Queue readback failed", error))?
        != queue_snapshot
    {
        return Err(
            "deferred autonomous terminal completion changed the post-plan Queue snapshot"
                .to_owned(),
        );
    }
    let final_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            expected_chain_id_hash,
            &expected_groups,
        )
        .map_err(|error| lifecycle_error("deferred terminal final stage proof failed", error))?;
    let mut final_complete_groups = BTreeSet::new();
    for final_stage in final_stages {
        let binding = final_stage.binding();
        let Some((unit_index, _group_position, expected)) = expected_by_group
            .get(&binding.reservation_group_hash)
            .copied()
        else {
            return Err(
                "deferred autonomous terminal final stage proof contains an unplanned group"
                    .to_owned(),
            );
        };
        let source_kind_matches = matches!(
            (
                deferred.units[unit_index].identity.0,
                final_stage.source_kind()
            ),
            (
                0,
                AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
            ) | (
                1,
                AutonomousLifecycleTerminalOutcomeSourceKind::RetiredRelease
            )
        );
        if expected.binding() != binding
            || !source_kind_matches
            || final_stage.stage() != AutonomousLifecycleTerminalOutcomeDurableStage::Complete
            || !final_complete_groups.insert(binding.reservation_group_hash)
        {
            return Err(
                "deferred autonomous terminal final stage proof is not exact and Complete"
                    .to_owned(),
            );
        }
    }
    if final_complete_groups.len() != expected_by_group.len() {
        return Err(
            "deferred autonomous terminal final stage proof omitted an expected handoff group"
                .to_owned(),
        );
    }
    if !kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .map_err(|error| lifecycle_error("deferred terminal readback failed", error))?
        .is_empty()
    {
        return Err("deferred autonomous terminal completion left a Pending source".to_owned());
    }
    Ok(completed)
}

/// Reconcile every local lifecycle bootstrap and cursor before live lane activation.
///
/// The caller must already have recovered canonical State/Kura and installed both Queue journals,
/// while network lane-work ingress remains closed. A non-empty Queue snapshot must still be
/// quarantined; an empty replay legitimately has no Queue owner-quarantine bit to hold.
pub(crate) fn reconcile_autonomous_lifecycle_startup(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    context: &wire::HeightContext,
    planner_evidence: LaneReservationSnapshotPlannerEvidence,
    deferred_terminal_recovery: AutonomousLifecycleDeferredTerminalRecoveryHandoff,
    process_generation: Option<&AutonomousLifecycleProcessGenerationClaim>,
    local_peer: &PeerId,
    key_pair: &KeyPair,
) -> Result<RecoveredAutonomousLifecycleStartup, String> {
    let initial_queue_quarantine = queue.lane_reservation_startup_reconciliation_pending();
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .map_err(|error| lifecycle_error("Queue snapshot failed", error))?;
    if !snapshot.is_empty() && !initial_queue_quarantine {
        return Err(
            "autonomous lifecycle startup found unquarantined durable Queue owners".to_owned(),
        );
    }
    let receipt = queue
        .bind_lane_reservation_startup_reconciliation_receipt(&snapshot)
        .map_err(|error| lifecycle_error("Queue receipt binding failed", error))?
        .ok_or_else(|| "autonomous lifecycle startup Queue snapshot changed".to_owned())?;
    let planner_covered_groups = planner_evidence
        .covered_group_identities(&snapshot)
        .ok_or_else(|| {
            "autonomous lifecycle startup planner evidence names another Queue snapshot".to_owned()
        })?;
    let planner_paired_groups = planner_evidence
        .paired_lifecycle_group_identities(&snapshot)
        .ok_or_else(|| {
            "autonomous lifecycle startup paired evidence names another Queue snapshot".to_owned()
        })?;
    let exact_planner_group_bindings = planner_evidence
        .exact_group_bindings(&snapshot)
        .ok_or_else(|| {
            "autonomous lifecycle startup exact planner evidence names another Queue snapshot"
                .to_owned()
        })?;
    let mut exact_planner_groups = BTreeMap::new();
    for binding in exact_planner_group_bindings {
        if exact_planner_groups
            .insert(binding.reservation_group_hash, binding)
            .is_some()
        {
            return Err(
                "autonomous lifecycle startup planner aliases one exact group hash".to_owned(),
            );
        }
    }
    let mut seen_pending_hashes = BTreeSet::new();
    let mut seen_pending_identities = BTreeSet::new();
    for unit in &deferred_terminal_recovery.units {
        let observed_identity = (
            unit.identity.0,
            unit.pending_groups
                .iter()
                .map(|observation| observation.binding().reservation_group_hash)
                .collect::<Vec<_>>(),
        );
        if unit.owned_group_hashes.is_empty() || observed_identity != unit.identity {
            return Err(
                "autonomous lifecycle startup received a malformed deferred terminal unit"
                    .to_owned(),
            );
        }
        let mut unit_has_planner_anchor = false;
        for observation in &unit.pending_groups {
            let pending = observation.binding();
            let has_current_owner =
                pending_terminal_group_has_exact_queue_owner(&snapshot, observation)?;
            let marked_owned = unit
                .owned_group_hashes
                .contains(&pending.reservation_group_hash);
            if has_current_owner != marked_owned
                || !seen_pending_hashes.insert(pending.reservation_group_hash)
                || !seen_pending_identities.insert(pending.identity)
                || (marked_owned
                    && exact_planner_groups.get(&pending.reservation_group_hash) != Some(&pending))
            {
                return Err(
                    "autonomous lifecycle startup Pending coverage differs from its exact Queue/planner cut"
                        .to_owned(),
                );
            }
            unit_has_planner_anchor |= marked_owned;
        }
        if !unit_has_planner_anchor {
            return Err(
                "autonomous lifecycle startup deferred a terminal unit without a planner-owned anchor"
                    .to_owned(),
            );
        }
    }
    let mut planner_evidence = Some(planner_evidence);
    if snapshot.is_empty() {
        let planner_groups = planner_evidence
            .take()
            .expect("startup owns exactly one planner evidence batch")
            .into_queue_groups(&snapshot)
            .ok_or_else(|| {
                "autonomous lifecycle startup planner evidence names another Queue snapshot"
                    .to_owned()
            })?;
        if !planner_covered_groups.is_empty()
            || !planner_paired_groups.is_empty()
            || !planner_groups.is_empty()
            || deferred_terminal_recovery.group_count() != 0
        {
            return Err(
                "autonomous lifecycle startup has planner groups for an empty Queue snapshot"
                    .to_owned(),
            );
        }
    }

    let chain_id_hash = Hash::prehashed(*context.network_id.as_bytes());
    let Some(process_generation) = process_generation else {
        if snapshot.is_empty() {
            return Ok(RecoveredAutonomousLifecycleStartup {
                snapshot,
                receipt,
                deferred_terminal_recovery,
                completed_bootstraps: 0,
                recovered_attempts: 0,
            });
        }
        let projections = if planner_paired_groups.is_empty() {
            Vec::new()
        } else {
            if local_peer.public_key() != key_pair.public_key() {
                return Err(
                    "observer lifecycle pairing conflicts with the configured local key identity"
                        .to_owned(),
                );
            }
            observer_retirement_lifecycle_projections(
                kura,
                chain_id_hash,
                local_peer,
                &planner_paired_groups,
                &deferred_terminal_recovery,
            )?
        };
        let recovery = queue
            .authorize_lane_reservation_snapshot_recovery(
                receipt,
                projections,
                planner_evidence.take(),
            )
            .map_err(|error| {
                lifecycle_error(
                    "observer Queue snapshot authorization failed; signed lifecycle custody is required",
                    error,
                )
            })?;
        let receipt = recovery.into_reconciliation_receipt().map_err(|error| {
            lifecycle_error("observer Queue recovery consumption failed", error)
        })?;
        return Ok(RecoveredAutonomousLifecycleStartup {
            snapshot,
            receipt,
            deferred_terminal_recovery,
            completed_bootstraps: 0,
            recovered_attempts: 0,
        });
    };
    if process_generation.local_peer_id() != local_peer
        || process_generation.chain_id_hash() != chain_id_hash
        || local_peer.public_key() != key_pair.public_key()
    {
        return Err(
            "autonomous lifecycle process claim conflicts with the active chain or local key"
                .to_owned(),
        );
    }

    let routes = active_lifecycle_routes(state, context)?;
    let mut current_queue_groups =
        BTreeMap::<LaneQueueReservationGroupIdentityV1, Vec<LaneQueueReservationKeyV2>>::new();
    for phase in &snapshot.ordered_owner_phases {
        current_queue_groups
            .entry(LaneQueueReservationGroupIdentityV1::from_key(&phase.key))
            .or_default()
            .push(phase.key);
    }
    let current_groups = current_queue_groups
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut bootstraps = Vec::new();
    for (lane_id, dataspace_id, lane_incarnation) in &routes {
        bootstraps.extend(
            kura.autonomous_lifecycle_bootstrap_recovery_inventory(
                process_generation,
                *lane_id,
                *dataspace_id,
                *lane_incarnation,
            )
            .map_err(|error| lifecycle_error("bootstrap inventory failed", error))?,
        );
    }
    if bootstraps.iter().any(|authority| {
        seen_pending_identities.contains(&authority.binding().reservation_group_binding().identity)
    }) {
        return Err(
            "autonomous lifecycle bootstrap overlaps a deferred terminal handoff identity"
                .to_owned(),
        );
    }

    let mut projections = BTreeMap::new();
    for authority in &bootstraps {
        let identity = authority.binding().reservation_group_binding().identity;
        if authority.custody_source() == AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue
            && !exact_current_queue_group_matches(
                authority.binding(),
                &authority.executable_payload().reservation_keys,
                &current_queue_groups,
            )
        {
            return Err(
                "ProducerQueue bootstrap lost its exact current durable Queue owner".to_owned(),
            );
        }
        if !current_groups.contains(&identity) {
            continue;
        }
        if planner_covered_groups.contains(&identity) || planner_paired_groups.contains(&identity) {
            return Err(
                "planner-classified Queue group still has an unfinished signed lifecycle bootstrap"
                    .to_owned(),
            );
        }
        let projection = queue
            .select_lane_reservation_snapshot_lifecycle_projection(
                &receipt,
                authority.live_cursor(),
                authority.executable_payload().reservation_keys.clone(),
            )
            .map_err(|error| lifecycle_error("bootstrap Queue projection failed", error))?;
        if projections.insert(identity, projection).is_some() {
            return Err(
                "autonomous lifecycle startup found duplicate bootstrap Queue coverage".to_owned(),
            );
        }
    }
    for (lane_id, dataspace_id, lane_incarnation) in &routes {
        let route_pending_groups = planner_covered_pending_groups_for_route(
            &deferred_terminal_recovery,
            *lane_id,
            *dataspace_id,
            *lane_incarnation,
        );
        for attempt in kura
            .active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
                process_generation,
                *lane_id,
                *dataspace_id,
                *lane_incarnation,
                &route_pending_groups,
            )
            .map_err(|error| lifecycle_error("attempt inventory failed", error))?
        {
            let payload = attempt.executable_payload();
            let identity = LaneQueueReservationGroupIdentityV1::from_key(
                payload.reservation_keys.first().ok_or_else(|| {
                    "autonomous lifecycle retained payload has no reservation identity".to_owned()
                })?,
            );
            if !current_groups.contains(&identity) || projections.contains_key(&identity) {
                continue;
            }
            let planner_paired = planner_paired_groups.contains(&identity);
            if planner_covered_groups.contains(&identity) && !planner_paired {
                continue;
            }
            let cursor = attempt.cursor().ok_or_else(|| {
                "autonomous lifecycle Queue owner has no cursor or signed bootstrap".to_owned()
            })?;
            let projection = if planner_paired {
                lifecycle_identity_projection_for_cursor(cursor, payload.reservation_keys.clone())?
            } else {
                require_local_producer_queue_owner(payload, cursor, &current_queue_groups)?;
                lifecycle_projection_for_cursor(
                    queue,
                    &receipt,
                    cursor,
                    payload.reservation_keys.clone(),
                )?
            };
            projections.insert(identity, projection);
        }
    }

    let mut receipt_slot = Some(receipt);
    let mut recovery_authorization = if snapshot.is_empty() {
        None
    } else {
        Some(
            queue
                .authorize_lane_reservation_snapshot_recovery(
                    receipt_slot
                        .take()
                        .expect("non-empty Queue recovery owns its bound receipt"),
                    projections.into_values().collect(),
                    planner_evidence.take(),
                )
                .map_err(|error| lifecycle_error("Queue snapshot authorization failed", error))?,
        )
    };

    let mut completed_bootstraps = 0_usize;
    for authority in bootstraps {
        let expected_live = authority.live_cursor().clone();
        let permit = if authority.custody_source()
            == AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue
        {
            let recovery = recovery_authorization.as_mut().ok_or_else(|| {
                "ProducerQueue bootstrap has no current durable Queue owner".to_owned()
            })?;
            let queue_authorization = recovery
                .authorize_recovered_producer_queue_lifecycle_bootstrap(queue, authority.binding())
                .map_err(|error| lifecycle_error("ProducerQueue reauthentication failed", error))?;
            kura.authenticate_autonomous_lifecycle_bootstrap_recovery(
                authority,
                queue_authorization,
            )
            .map_err(|error| {
                lifecycle_error("ProducerQueue bootstrap authentication failed", error)
            })?
        } else {
            kura.authenticate_autonomous_lifecycle_bootstrap_recovery_from_durable_custody(
                authority,
            )
            .map_err(|error| {
                lifecycle_error("durable-custody bootstrap authentication failed", error)
            })?
        };
        let completion = kura
            .complete_autonomous_lifecycle_bootstrap(permit)
            .map_err(|error| lifecycle_error("bootstrap completion failed", error))?;
        if completion.cursor() != &expected_live {
            return Err(
                "autonomous lifecycle bootstrap completed with a different Live cursor".to_owned(),
            );
        }
        drop(completion);
        completed_bootstraps = completed_bootstraps.saturating_add(1);
    }

    // Consume the checked action-25 stutters only after every ProducerQueue bootstrap has reached
    // exact Live readback. The receipt remains unpublished while generation takeover proceeds.
    let receipt = if let Some(recovery) = recovery_authorization {
        recovery
            .into_reconciliation_receipt()
            .map_err(|error| lifecycle_error("Queue recovery consumption failed", error))?
    } else {
        receipt_slot
            .take()
            .expect("empty Queue recovery retains exactly one direct receipt")
    };

    let mut recovered_attempts = 0_usize;
    for (lane_id, dataspace_id, lane_incarnation) in routes {
        let route_pending_groups = planner_covered_pending_groups_for_route(
            &deferred_terminal_recovery,
            lane_id,
            dataspace_id,
            lane_incarnation,
        );
        for attempt in kura
            .active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
                process_generation,
                lane_id,
                dataspace_id,
                lane_incarnation,
                &route_pending_groups,
            )
            .map_err(|error| lifecycle_error("post-bootstrap attempt inventory failed", error))?
        {
            let payload = attempt.executable_payload();
            let identity = LaneQueueReservationGroupIdentityV1::from_key(
                payload.reservation_keys.first().ok_or_else(|| {
                    "autonomous lifecycle retained payload has no reservation identity".to_owned()
                })?,
            );
            if seen_pending_identities.contains(&identity)
                || planner_covered_groups.contains(&identity)
                || planner_paired_groups.contains(&identity)
            {
                continue;
            }
            let cursor = attempt.cursor().ok_or_else(|| {
                "autonomous lifecycle payload remained cursorless after bootstrap recovery"
                    .to_owned()
            })?;
            require_local_producer_queue_owner(payload, cursor, &current_queue_groups)?;
            if recover_one_attempt(
                kura,
                process_generation,
                key_pair,
                local_peer,
                payload,
                cursor.binding(),
            )? {
                recovered_attempts = recovered_attempts.saturating_add(1);
            }
        }
    }

    if !queue
        .revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)
        .map_err(|error| lifecycle_error("final Queue receipt revalidation failed", error))?
    {
        return Err("autonomous lifecycle startup Queue receipt changed before handoff".to_owned());
    }
    if queue.lane_reservation_startup_reconciliation_pending() != initial_queue_quarantine {
        return Err(
            "autonomous lifecycle startup changed the Queue owner-quarantine state".to_owned(),
        );
    }
    Ok(RecoveredAutonomousLifecycleStartup {
        snapshot,
        receipt,
        deferred_terminal_recovery,
        completed_bootstraps,
        recovered_attempts,
    })
}

#[cfg(test)]
#[path = "tests/v2_lifecycle_recovery.rs"]
mod tests;

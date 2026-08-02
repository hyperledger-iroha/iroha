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
        AutonomousLifecyclePayloadCustodySourceV1, AutonomousLifecycleProcessGenerationClaim,
        Kura,
    },
    queue::{
        LaneQueueReservationGroupIdentityV1, LaneQueueReservationReconciliationSnapshotV1,
        LaneReservationSnapshotLifecycleProjectionV1, LaneReservationStartupReconciliationReceipt,
        Queue,
    },
    state::{State, consensus_lane_dataspace_at_height},
};

/// Exact Queue handoff retained after all local lifecycle recovery is durable.
#[must_use = "the recovered Queue receipt must reach the startup publication gate"]
pub(crate) struct RecoveredAutonomousLifecycleStartup {
    snapshot: LaneQueueReservationReconciliationSnapshotV1,
    receipt: LaneReservationStartupReconciliationReceipt,
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
    ) {
        (self.snapshot, self.receipt)
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

fn lifecycle_error(stage: &str, error: impl core::fmt::Display) -> String {
    format!("autonomous lifecycle startup {stage}: {error}")
}

fn active_lifecycle_routes(
    state: &State,
    context: &wire::HeightContext,
) -> Result<Vec<(iroha_data_model::nexus::LaneId, iroha_data_model::nexus::DataSpaceId, Hash)>, String>
{
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

fn sign_lifecycle_cursor(
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
    let previous_cursor_hash = current.as_ref().map(AutonomousLifecycleCursorV2::cursor_hash);
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
    kura.compare_and_swap_autonomous_lifecycle_cursor(lease, next)
        .map_err(|error| lifecycle_error("cursor compare-and-swap failed", error))
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
            return Err("autonomous lifecycle startup cursor belongs to a future process generation"
                .to_owned());
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
                let phase = AutonomousLifecycleCursorPhaseV2::prepared(
                    current_generation,
                    recover,
                )
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
                let phase = AutonomousLifecycleCursorPhaseV2::live(
                    current_generation,
                    transition.after,
                )
                .map_err(|reason| lifecycle_error("Live phase construction failed", reason))?;
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
                let phase = AutonomousLifecycleCursorPhaseV2::prepared(
                    current_generation,
                    rehydrate,
                )
                .map_err(|reason| lifecycle_error("rehydration preparation failed", reason))?;
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

/// Reconcile every local lifecycle bootstrap and cursor before live lane activation.
///
/// The caller must already have recovered canonical State/Kura and installed both Queue journals,
/// while the Queue startup publication gate and network lane-work ingress remain closed.
pub(crate) fn reconcile_autonomous_lifecycle_startup(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    context: &wire::HeightContext,
    process_generation: Option<&AutonomousLifecycleProcessGenerationClaim>,
    local_peer: &PeerId,
    key_pair: &KeyPair,
) -> Result<RecoveredAutonomousLifecycleStartup, String> {
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .map_err(|error| lifecycle_error("Queue snapshot failed", error))?;
    let receipt = queue
        .bind_lane_reservation_startup_reconciliation_receipt(&snapshot)
        .map_err(|error| lifecycle_error("Queue receipt binding failed", error))?
        .ok_or_else(|| "autonomous lifecycle startup Queue snapshot changed".to_owned())?;

    let Some(process_generation) = process_generation else {
        if snapshot.is_empty() {
            return Ok(RecoveredAutonomousLifecycleStartup {
                snapshot,
                receipt,
                completed_bootstraps: 0,
                recovered_attempts: 0,
            });
        }
        return Err(
            "observer startup found validator-owned durable Queue lifecycle state".to_owned(),
        );
    };
    if process_generation.local_peer_id() != local_peer
        || process_generation.chain_id_hash()
            != Hash::new(context.chain_id.clone().into_inner().as_bytes())
        || local_peer.public_key() != key_pair.public_key()
    {
        return Err(
            "autonomous lifecycle process claim conflicts with the active chain or local key"
                .to_owned(),
        );
    }

    let routes = active_lifecycle_routes(state, context)?;
    let current_groups = snapshot
        .ordered_owner_phases
        .iter()
        .map(|phase| LaneQueueReservationGroupIdentityV1::from_key(&phase.key))
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

    let mut projections = BTreeMap::new();
    for authority in &bootstraps {
        let identity = authority.binding().reservation_group_binding().identity;
        if !current_groups.contains(&identity) {
            continue;
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
        for attempt in kura
            .active_autonomous_lifecycle_attempt_inventory(
                process_generation,
                *lane_id,
                *dataspace_id,
                *lane_incarnation,
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
            let cursor = attempt.cursor().ok_or_else(|| {
                "autonomous lifecycle Queue owner has no cursor or signed bootstrap".to_owned()
            })?;
            let projection = lifecycle_projection_for_cursor(
                queue,
                &receipt,
                cursor,
                payload.reservation_keys.clone(),
            )?;
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
                .authorize_recovered_producer_queue_lifecycle_bootstrap(
                    queue,
                    authority.binding(),
                )
                .map_err(|error| lifecycle_error("ProducerQueue reauthentication failed", error))?;
            kura.authenticate_autonomous_lifecycle_bootstrap_recovery(
                authority,
                queue_authorization,
            )
            .map_err(|error| lifecycle_error("ProducerQueue bootstrap authentication failed", error))?
        } else {
            kura.authenticate_autonomous_lifecycle_bootstrap_recovery_from_durable_custody(
                authority,
            )
            .map_err(|error| lifecycle_error("durable-custody bootstrap authentication failed", error))?
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
        for attempt in kura
            .active_autonomous_lifecycle_attempt_inventory(
                process_generation,
                lane_id,
                dataspace_id,
                lane_incarnation,
            )
            .map_err(|error| lifecycle_error("post-bootstrap attempt inventory failed", error))?
        {
            let payload = attempt.executable_payload();
            let cursor = attempt.cursor().ok_or_else(|| {
                "autonomous lifecycle payload remained cursorless after bootstrap recovery"
                    .to_owned()
            })?;
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
    Ok(RecoveredAutonomousLifecycleStartup {
        snapshot,
        receipt,
        completed_bootstraps,
        recovered_attempts,
    })
}

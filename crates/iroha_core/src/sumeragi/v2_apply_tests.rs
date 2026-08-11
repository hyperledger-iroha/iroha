//! Test-only extensions and the stable v2 apply test module.

#[derive(Default)]
pub(super) struct FailureInjection {
    pub(super) kura_store: std::sync::atomic::AtomicBool,
    pub(super) wsv_checkpoint: std::sync::atomic::AtomicBool,
    pub(super) provider_ingest_archive_capture: std::sync::atomic::AtomicBool,
    pub(super) reputation_archive_capture: std::sync::atomic::AtomicBool,
}

/// Test-only durable-application crash boundary.
pub(super) enum CrashPoint {
    /// After canonical block persistence.
    KuraStore,
    /// After the staged WSV checkpoint.
    WsvCheckpoint,
    /// After provider-ingest archive capture.
    ProviderIngestArchiveCapture,
    /// After reputation archive capture.
    ReputationArchiveCapture,
}

/// Persist the exact payload, exact execution input, and immutable recovery
/// record in crash-safe order after independently rebuilding every authority.
#[cfg(test)]
pub(crate) fn install_historical_autonomous_lane_recovery(
    state: &State,
    kura: &Kura,
    input: &HistoricalAutonomousReservationInstallV1,
) -> Result<HistoricalAutonomousLaneRecoveryInstallOutcome, V2ReservationLifecycleError> {
    let record = preflight_historical_autonomous_lane_recovery(state, kura, input)?;
    persist_preflighted_historical_autonomous_lane_recovery(kura, &record)
}

/// Persist one record whose complete State authority was already validated.
/// Kura performs its bounded namespace preflight, durable dependency checks,
/// and collision checks at the persistence boundary.
#[cfg(test)]
pub(crate) fn persist_preflighted_historical_autonomous_lane_recovery(
    kura: &Kura,
    record: &HistoricalAutonomousLaneRecoveryRecordV1,
) -> Result<HistoricalAutonomousLaneRecoveryInstallOutcome, V2ReservationLifecycleError> {
    kura.persist_lane_executable_payload(
        &record.payload,
        record.payload.network_id,
        record.payload.epoch,
    )?;
    persist_preflighted_historical_autonomous_lane_recoveries(kura, std::slice::from_ref(record))?
        .pop()
        .ok_or_else(|| {
            invalid_historical_autonomous_recovery(
                &record.installation_input(),
                "single historical recovery persistence produced no outcome",
            )
        })
}

impl AutonomousLaneQueueCarrierCleanupAuthorization {
    #[cfg(test)]
    fn from_projection_for_test(
        reservation_group: LaneQueueReservationGroupBindingV1,
        projection: ProductionInFlightFirstReleaseTransitionProjection,
    ) -> Result<Self, String> {
        Self::from_authenticated(AuthenticatedCarrierApplicationProjection {
            reservation_group,
            projection,
        })
    }
}

impl AutonomousLaneQueueCarrierCleanupAuthorization {
    #[cfg(test)]
    fn accepted_projection_for_test(&self) -> ProductionInFlightFirstReleaseTransitionProjection {
        *self.checked_apply_carrier.accepted_projection()
    }
}

impl V2ApplyService {
    #[cfg(test)]
    fn finish_durable_apply_completion(
        &self,
        evidence: DurableApplicationEvidence,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        let application_trace = evidence
            .application_refinement_projection()
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"native application identity cannot be represented losslessly",
                )
            })?;
        let checked_application = check_production_application_transition(application_trace)
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"durable application does not refine its Decision completion",
                )
            })?;
        self.finish_durable_apply_completion_against(
            evidence,
            checked_application.into_projection(),
        )
    }
}

impl V2ApplyService {
    pub(super) fn inject_test_crash(&self, point: CrashPoint) -> Result<(), V2ApplyError> {
        let (requested, error) = match point {
            CrashPoint::KuraStore => (
                self.test_failures
                    .kura_store
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterKuraStore,
            ),
            CrashPoint::WsvCheckpoint => (
                self.test_failures
                    .wsv_checkpoint
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterWsvCheckpoint,
            ),
            CrashPoint::ProviderIngestArchiveCapture => (
                self.test_failures
                    .provider_ingest_archive_capture
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture,
            ),
            CrashPoint::ReputationArchiveCapture => (
                self.test_failures
                    .reputation_archive_capture
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterReputationArchiveCapture,
            ),
        };
        if requested { Err(error) } else { Ok(()) }
    }

    #[cfg(test)]
    fn fail_after_kura_store_for_test(&self) {
        self.test_failures
            .kura_store
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_wsv_checkpoint_for_test(&self) {
        self.test_failures
            .wsv_checkpoint
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_provider_ingest_archive_capture_for_test(&self) {
        self.test_failures
            .provider_ingest_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_reputation_archive_capture_for_test(&self) {
        self.test_failures
            .reputation_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(super) fn snapshot_mismatch_context(staged: &[u8], committed: &[u8]) -> String {
    let first_difference = staged
        .iter()
        .zip(committed)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| staged.len().min(committed.len()));
    let context_start = first_difference.saturating_sub(256);
    let staged_end = first_difference.saturating_add(768).min(staged.len());
    let committed_end = first_difference.saturating_add(768).min(committed.len());
    format!(
        "first_difference={first_difference}, staged_len={}, committed_len={}, \
         staged_context={:?}, committed_context={:?}",
        staged.len(),
        committed.len(),
        String::from_utf8_lossy(&staged[context_start..staged_end]),
        String::from_utf8_lossy(&committed[context_start..committed_end]),
    )
}

/// Compatibility shim kept inside the test module only for older focused
/// fixtures which deliberately exercise the single-process no-network
/// boundary. Production callers must handle the typed recovery plan.
fn reconcile_lane_reservation_ownership(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    verified_active_context: &VerifiedHeightContext,
) -> Result<LaneReservationReconciliationSummary, V2ReservationLifecycleError> {
    match plan_lane_reservation_ownership(state, queue, kura, verified_active_context, None)? {
        LaneReservationReconciliationPlanning::Ready(plan) => {
            apply_lane_reservation_reconciliation_plan(state, queue, kura, plan)
        }
        LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) => {
            let height = needs.first().map_or(0, |need| need.height);
            Err(V2ReservationLifecycleError::MissingCanonicalBody { height })
        }
        LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) => {
            let install = installs
                .first()
                .expect("historical recovery planning is never empty");
            Err(
                V2ReservationLifecycleError::HistoricalRecoveryInstallationMissing {
                    recovery_id: install.recovery_id,
                    lane_id: install.reservation_group.identity.lane_id,
                },
            )
        }
    }
}

/// Focused preflight harness for synthetic merge entries which deliberately
/// lack a durable Kura carrier. Production cleanup additionally requires the
/// canonical carrier/source-outcome authentication path.
fn finalize_certified_merge_reservations_for_test(
    state: &State,
    queue: &Queue,
    entry: &MergeLedgerEntry,
    applications: Vec<AuthenticatedCarrierApplicationProjection>,
) -> Result<usize, V2ReservationLifecycleError> {
    let groups = crate::state::certified_merge_queue_reservation_groups(entry)?;
    if groups.len() != applications.len() {
        return Err(
            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                detail: "authenticated ApplyCarrier cardinality differs from canonical reservation groups"
                    .to_owned(),
            },
        );
    }
    for (transaction_hash, _) in groups.iter().flatten() {
        if !state.has_committed_transaction(*transaction_hash) {
            return Err(V2ReservationLifecycleError::UncommittedMergeTransaction {
                transaction_hash: *transaction_hash,
            });
        }
    }

    let mut authorized_groups = Vec::with_capacity(groups.len());
    for (group, application) in groups.into_iter().zip(applications) {
        let ordered_keys = group.into_iter().map(|(_, key)| key).collect::<Vec<_>>();
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).map_err(
                |reason| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail: reason.to_owned(),
                },
            )?;
        if reservation_group != application.reservation_group {
            return Err(
                V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail: "authenticated ApplyCarrier names another ordered reservation group"
                        .to_owned(),
                },
            );
        }
        let authorization = application
            .queue_cleanup_authorization()
            .map_err(
                |detail| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization { detail },
            )?;
        authorized_groups.push((ordered_keys, authorization));
    }
    let cleanup = queue.commit_lane_reservation_groups_with_authorization(authorized_groups)?;
    let (finalized_reservations, _terminal_evidence) = cleanup.into_parts();
    Ok(finalized_reservations)
}

fn install_live_lifecycle_cursor_for_apply_test(
    kura: &Kura,
    generation: &crate::kura::AutonomousLifecycleProcessGenerationClaim,
    payload: &LaneExecutablePayloadV1,
    height_context_id: wire::HeightContextId,
    local_peer: &PeerId,
    signer: &KeyPair,
) -> LaneQueueReservationGroupBindingV1 {
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind apply lifecycle reservation group");
    let binding = crate::kura::AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        payload.origin_proposal.descriptor.lane_block_height,
        payload,
        reservation_group,
        local_peer,
    )
    .expect("bind apply lifecycle attempt");
    let validator_count = u8::try_from(binding.validator_set_identity().2)
        .expect("apply lifecycle validator count fits refinement width");
    let validator_mask = if validator_count == 128 {
        u128::MAX
    } else {
        (1_u128 << validator_count) - 1
    };
    let (_, local_actor) = binding.local_validator_identity();
    let producer = binding.producer_actor_projection();
    let live_state = ProductionInFlightFirstReleaseStateProjection {
        validator_count,
        producer,
        producer_selected_owner: producer,
        replicated_carrier_owners: validator_mask & !producer,
        payload_binding_a: producer | local_actor,
        binding_a: canonical_lane_queue_reservation_group_identity_projection(reservation_group),
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: reservation_group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: local_actor,
            ..ProductionInFlightFirstReleaseCarrierProjection::default()
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: producer | local_actor,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v4: true,
            ever_reservation_v5: true,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    let cursor = crate::sumeragi::v2_lifecycle_recovery::sign_lifecycle_cursor(
        signer,
        local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        crate::kura::AutonomousLifecycleCursorPhaseV2::live(generation.generation(), live_state)
            .expect("construct apply lifecycle Live cursor"),
    )
    .expect("sign apply lifecycle Live cursor");
    let (_, lease) = kura
        .read_autonomous_lifecycle_cursor(payload, &binding, generation)
        .expect("read absent apply lifecycle cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(lease, cursor.clone())
            .expect("persist apply lifecycle Live cursor")
            .cursor(),
        Some(&cursor),
        "apply setup must read back the exact durable Live cursor",
    );
    reservation_group
}

include!("tests/v2_apply_unsealed_00.rs");
include!("tests/v2_apply_unsealed_01.rs");
include!("tests/v2_apply_unsealed_02.rs");

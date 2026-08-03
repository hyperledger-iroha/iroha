fn recover_pending_canonical_terminal_outcome(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    recovery: AutonomousLifecyclePendingCanonicalCarrierRecovery,
) -> Result<usize, V2ReservationLifecycleError> {
    let (
        pending_sources,
        complete_groups,
        reference,
        entry,
        carrier_block_height,
        carrier_block_hash,
        expected_chain_hash,
    ) = recovery.consume_for_v2_apply().ok_or_else(|| {
        V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
            detail: "Kura Pending canonical recovery has malformed source coordinates".to_owned(),
        }
    })?;
    let invalid = |detail: &str| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
        detail: detail.to_owned(),
    };
    let authenticated =
        authenticate_committed_canonical_carrier(state, kura, &entry, expected_chain_hash)?;
    if !reference.matches_entry(&entry)
        || authenticated.reference.entry_hash != reference.entry_hash
        || authenticated.carrier_height.get() != usize::try_from(carrier_block_height)?
        || authenticated.carrier_block_hash != carrier_block_hash
        || authenticated.groups.len() != pending_sources.len() + complete_groups.len()
    {
        return Err(invalid(
            "Kura Pending canonical recovery application/group cardinality differs",
        ));
    }

    let mut authenticated_groups = BTreeMap::new();
    for group in authenticated.groups {
        if authenticated_groups
            .insert(group.reservation_group.reservation_group_hash, group)
            .is_some()
        {
            return Err(invalid(
                "Kura Pending canonical recovery duplicates a carrier reservation group",
            ));
        }
    }

    // Complete members are part of the same authenticated carrier set but
    // require no Queue mutation. The carrier preflight already validated every
    // application transition and the full group set; matching this exact group
    // is sufficient. Do not mint a Queue-only move authority merely to drop it.
    for complete in complete_groups {
        let observed = authenticated_groups
            .remove(&complete.reservation_group_hash)
            .ok_or_else(|| invalid("Kura Complete group is absent from its carrier"))?;
        if observed.reservation_group != complete {
            return Err(invalid(
                "Kura Complete group differs from its canonical carrier identity",
            ));
        }
        let complete_authorization =
            observed
                .application
                .queue_cleanup_authorization()
                .map_err(|detail| {
                    V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization { detail }
                })?;
        if complete_authorization
            .consume_for_queue(&complete)
            .is_none()
        {
            return Err(invalid(
                "Kura Complete group lacks its exact authenticated ApplyCarrier projection",
            ));
        }
    }

    let expected_terminal_evidence = pending_sources.len();
    let mut queue_groups = Vec::with_capacity(expected_terminal_evidence);
    for (pending_group, source_authorization) in pending_sources {
        let observed = authenticated_groups
            .remove(&pending_group.reservation_group_hash)
            .ok_or_else(|| invalid("Kura Pending group is absent from its carrier"))?;
        if observed.reservation_group != pending_group {
            return Err(invalid(
                "Kura Pending group differs from its canonical carrier identity",
            ));
        }
        let carrier_authorization =
            observed
                .application
                .queue_cleanup_authorization()
                .map_err(|detail| {
                    V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization { detail }
                })?;
        queue_groups.push((source_authorization, carrier_authorization));
    }
    if !authenticated_groups.is_empty() {
        return Err(invalid(
            "Kura lifecycle source-outcome set does not exactly cover its carrier",
        ));
    }

    let cleanup = queue
        .authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes(
            queue_groups,
        )?;
    let finalized_reservations = cleanup.finalized_reservations();
    let (_, terminal_evidence) = cleanup.into_parts();
    if terminal_evidence.len() != expected_terminal_evidence {
        return Err(invalid(
            "Kura Pending canonical recovery did not produce its exact terminal Queue token set",
        ));
    }
    for evidence in terminal_evidence {
        kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)?;
    }
    kura.release_post_wsv_lane_artifact_budget_reservation(
        &entry,
        carrier_block_height,
        carrier_block_hash,
    )?;
    Ok(finalized_reservations)
}

fn recover_pending_release_terminal_outcome(
    queue: &Queue,
    kura: &Kura,
    barrier: &LaneQueueReservationReleaseBarrierV3,
    finalization: crate::kura::AutonomousLaneQueueReleaseFinalizationAuthorization,
    source_outcome_authorization:
        crate::kura::AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization,
) -> Result<usize, V2ReservationLifecycleError> {
    // Prefer the explicitly read-only proof when restart recovered no durable
    // Queue owner for this exact barrier. Unrelated groups must not force the
    // mutating finalizer after this release was already forgotten.
    let snapshot = queue.lane_reservation_reconciliation_snapshot()?;
    let prepared = snapshot
        .prepared_release_barriers
        .iter()
        .filter(|candidate| *candidate == barrier)
        .count();
    let completed = snapshot
        .completed_releases
        .iter()
        .filter(|candidate| candidate.barrier == *barrier)
        .count();
    let (finalized_reservations, terminal_evidence) = match (prepared, completed) {
        (0, 0) => (
            0,
            queue.authenticate_autonomous_lifecycle_pending_release_queue_terminal_outcome(
                barrier,
                finalization,
                source_outcome_authorization,
            )?,
        ),
        (1, 0) | (0, 1) => {
            let completion = queue
                .finalize_autonomous_lifecycle_pending_release_queue_terminal_outcome(
                    barrier,
                    finalization,
                    source_outcome_authorization,
                )?;
            let finalized_reservations = completion.finalized_reservations();
            let (_, terminal_evidence) = completion.into_parts();
            (finalized_reservations, terminal_evidence)
        }
        _ => {
            return Err(V2ReservationLifecycleError::ReleaseRetirementMismatch {
                retirement_hash: barrier.retirement_hash,
            });
        }
    };
    kura.complete_autonomous_lifecycle_release_terminal_outcome(terminal_evidence)?;
    Ok(finalized_reservations)
}

/// Consume one bounded, source-revalidated Kura Pending outcome and close its
/// exact Queue/Kura terminal join.
pub(crate) fn recover_pending_autonomous_lifecycle_terminal_outcome(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    recovery: AutonomousLifecyclePendingTerminalOutcomeRecovery,
) -> Result<usize, V2ReservationLifecycleError> {
    match recovery {
        AutonomousLifecyclePendingTerminalOutcomeRecovery::Canonical(recovery) => {
            recover_pending_canonical_terminal_outcome(state, queue, kura, recovery)
        }
        AutonomousLifecyclePendingTerminalOutcomeRecovery::RetiredRelease {
            barrier,
            finalization,
            source_outcome_authorization,
        } => recover_pending_release_terminal_outcome(
            queue,
            kura,
            &barrier,
            finalization,
            source_outcome_authorization,
        ),
    }
}

struct ReservationReconciliationGroupInput {
    group: LaneQueueReservationReconciliationGroupV1,
    /// Queue owners which remain after an earlier grouped Commit-prefix or
    /// ForgetCommit-prefix crash. Committed groups replace
    /// `group.ordered_keys` with the complete canonical MergeLedger membership
    /// during read-only preflight and resume against that full identity.
    owned_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
    release_barrier: Option<LaneQueueReservationReleaseBarrierV3>,
    committed: bool,
    commit_authorization: Option<AutonomousLaneQueueCarrierCleanupAuthorization>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ReservationRetainDisposition {
    Current,
    Certified,
    PendingMerge,
    HistoricalRecovery(HistoricalAutonomousReservationInstallV1),
}

enum ReservationReconciliationAction {
    Commit {
        keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
        authorization: AutonomousLaneQueueCarrierCleanupAuthorization,
    },
    Retain {
        keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
        disposition: ReservationRetainDisposition,
    },
    Retire {
        retirement: AutonomousLaneSlotRetirementV1,
        epoch: u64,
        resumed: bool,
        snapshot_release: Option<AutonomousLaneRetirementSnapshotEvidenceV1>,
    },
    DirectRelease {
        keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
        authorization: StrictAbsenceDirectReleaseAuthorization,
    },
}

/// Move-only proof that one exact planner-classified pre-Kura group may return to ordinary FIFO.
#[must_use = "strict-absence direct-release authority must reach the Queue journal sink"]
pub(crate) struct StrictAbsenceDirectReleaseAuthorization {
    reservation_group: LaneQueueReservationGroupBindingV1,
    ordered_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
    checked_release:
        CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>,
}

impl StrictAbsenceDirectReleaseAuthorization {
    fn from_snapshot(
        snapshot: &LaneQueueReservationReconciliationSnapshotV1,
        ordered_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
    ) -> Result<Self, V2ReservationLifecycleError> {
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).map_err(
                |detail| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail: detail.to_owned(),
                },
            )?;
        let before = strictly_absent_lane_reservation_snapshot_recovery_state(
            snapshot,
            reservation_group,
            &ordered_keys,
        )?;
        let mut after = before;
        after.queue.reservation_state =
            super::v2_core::IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED;
        after.release.fifo_restored = true;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: super::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT,
            actor: 0,
            target: 0,
            before,
            after,
        };
        let checked_release = check_production_in_flight_first_release_transition(projection)
            .ok_or_else(
                || V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail: "strict-absence direct release failed the composed transition gate"
                        .to_owned(),
                },
            )?;
        Ok(Self {
            reservation_group,
            ordered_keys,
            checked_release,
        })
    }

    /// Borrow the exact ordered group for Queue's global-FIFO union preflight.
    pub(crate) fn queue_group(
        &self,
    ) -> Option<(
        LaneQueueReservationGroupBindingV1,
        &[crate::queue::LaneQueueReservationKeyV2],
        ProductionInFlightFirstReleaseTransitionProjection,
    )> {
        let recomputed =
            lane_queue_reservation_group_binding_from_ordered_keys(self.ordered_keys.iter())
                .ok()?;
        let projection = *self.checked_release.accepted_projection();
        (recomputed == self.reservation_group
            && projection.action
                == super::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT
            && projection.actor == 0
            && projection.target == 0
            && projection.before.binding_a
                == canonical_lane_queue_reservation_group_identity_projection(
                    self.reservation_group,
                )
            && projection.before.queue.selected_count == self.reservation_group.reservation_count)
            .then_some((
                self.reservation_group,
                self.ordered_keys.as_slice(),
                projection,
            ))
    }

    /// Consume the checked transition immediately beside Queue's durable release append.
    pub(crate) fn consume_for_queue(
        self,
    ) -> Option<ProductionInFlightFirstReleaseTransitionProjection> {
        let expected = self.queue_group()?.2;
        let projection = self.checked_release.into_projection();
        (projection == expected).then_some(projection)
    }
}

/// Read-only startup classification result.
///
/// A recovery outcome contains only immutable, finality-authenticated body
/// identities. A ready plan owns every Queue mutation input and can be
/// applied only after its exact Queue snapshot is revalidated.
pub(crate) enum LaneReservationReconciliationPlanning {
    /// Every dependency is locally durable; mutation may begin under the
    /// process fail-stop operation.
    Ready(LaneReservationReconciliationPlan),
    /// One or more canonical executed bodies must be recovered first.
    RecoverCanonicalBodies(Vec<CanonicalExecutedBlockNeedV1>),
    /// Exact historical autonomous work must cross its durable Kura/State
    /// installation boundary before Queue publication may resume.
    InstallHistoricalAutonomousRecoveries(Vec<HistoricalAutonomousReservationInstallV1>),
}

/// Complete immutable mutation plan for one Queue/Kura/State startup cut.
pub(crate) struct LaneReservationReconciliationPlan {
    snapshot: LaneQueueReservationReconciliationSnapshotV1,
    replay_receipt: LaneReservationStartupReconciliationReceipt,
    actions: Vec<ReservationReconciliationAction>,
    direct_release: Vec<crate::queue::LaneQueueReservationKeyV2>,
    deferred_terminal_recovery: AutonomousLifecycleDeferredTerminalRecoveryHandoff,
    chain_hash: Hash,
    recovered: usize,
}

/// Planner-authenticated source for a Queue snapshot group which legitimately has no active local
/// lifecycle cursor.
pub(crate) enum LaneReservationSnapshotPlannerProjectionKind {
    /// The complete State/Kura/canonical-carrier classifier proved that this reservation never
    /// crossed Kura activation and may take only the checked direct-release branch.
    StrictlyAbsent {
        recovered_state: ProductionInFlightFirstReleaseStateProjection,
    },
    /// Canonical State, finality, MergeLedger, and source-bundle validation proved exactly one
    /// `ApplyCarrier`; Queue may recover only its bounded Commit-cleanup suffix.
    CanonicalCarrier {
        applied_state: ProductionInFlightFirstReleaseStateProjection,
    },
    /// Kura authenticated the exact retirement and claim prefix for a durable Queue release
    /// barrier. Queue must pair this proof with the matching signed lifecycle cursor before using
    /// its post-retirement state.
    RetiredRelease {
        evidence: AutonomousLaneRetirementSnapshotEvidenceV1,
    },
}

/// One group inside an opaque planner-authenticated snapshot recovery batch.
#[must_use = "planner snapshot evidence must be consumed by Queue recovery"]
pub(crate) struct LaneReservationSnapshotPlannerGroupEvidence {
    reservation_group: LaneQueueReservationGroupBindingV1,
    ordered_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
    kind: LaneReservationSnapshotPlannerProjectionKind,
}

impl LaneReservationSnapshotPlannerGroupEvidence {
    /// Return the exact reservation-group identity bound by this planner proof.
    #[must_use]
    pub(crate) const fn reservation_group_identity(&self) -> LaneQueueReservationGroupIdentityV1 {
        self.reservation_group.identity
    }

    /// Return whether this planner group still requires a signed lifecycle identity anchor.
    #[must_use]
    pub(crate) fn requires_lifecycle_pair(&self) -> bool {
        matches!(
            &self.kind,
            LaneReservationSnapshotPlannerProjectionKind::RetiredRelease { .. }
        )
    }

    /// Consume the opaque group into Queue-visible immutable facts.
    pub(crate) fn into_queue_parts(
        self,
    ) -> (
        LaneQueueReservationGroupBindingV1,
        Vec<crate::queue::LaneQueueReservationKeyV2>,
        LaneReservationSnapshotPlannerProjectionKind,
    ) {
        (self.reservation_group, self.ordered_keys, self.kind)
    }
}

/// Snapshot-bound non-lifecycle evidence derived only by the complete immutable startup planner.
#[must_use = "planner snapshot evidence must be consumed by Queue recovery"]
pub(crate) struct LaneReservationSnapshotPlannerEvidence {
    snapshot: LaneQueueReservationReconciliationSnapshotV1,
    groups: Vec<LaneReservationSnapshotPlannerGroupEvidence>,
}

impl LaneReservationSnapshotPlannerEvidence {
    /// Construct deliberately arbitrary planner evidence for Queue's adversarial unit tests.
    ///
    /// Production callers cannot bypass the immutable reconciliation planner: this constructor is
    /// compiled only for tests which must prove that Queue independently rejects stale snapshots,
    /// overlapping custody, and planner categories that disagree with durable owner phases.
    #[cfg(test)]
    pub(crate) fn from_parts_for_test(
        snapshot: LaneQueueReservationReconciliationSnapshotV1,
        groups: Vec<(
            LaneQueueReservationGroupBindingV1,
            Vec<crate::queue::LaneQueueReservationKeyV2>,
            LaneReservationSnapshotPlannerProjectionKind,
        )>,
    ) -> Self {
        Self {
            snapshot,
            groups: groups
                .into_iter()
                .map(|(reservation_group, ordered_keys, kind)| {
                    LaneReservationSnapshotPlannerGroupEvidence {
                        reservation_group,
                        ordered_keys,
                        kind,
                    }
                })
                .collect(),
        }
    }

    /// Borrow terminal reservation-group identities which replace signed cursor coverage.
    ///
    /// The coordinator uses this set only to avoid also minting cursor coverage or rehydrating a
    /// planner-terminal attempt. Queue still consumes the complete opaque groups and rejects any
    /// overlap independently.
    pub(crate) fn covered_group_identities(
        &self,
        expected_snapshot: &LaneQueueReservationReconciliationSnapshotV1,
    ) -> Option<BTreeSet<LaneQueueReservationGroupIdentityV1>> {
        (self.snapshot == *expected_snapshot).then(|| {
            self.groups
                .iter()
                .filter(|group| !group.requires_lifecycle_pair())
                .map(|group| group.reservation_group.identity)
                .collect()
        })
    }

    /// Borrow release-barrier identities which must be paired with signed lifecycle anchors.
    pub(crate) fn paired_lifecycle_group_identities(
        &self,
        expected_snapshot: &LaneQueueReservationReconciliationSnapshotV1,
    ) -> Option<BTreeSet<LaneQueueReservationGroupIdentityV1>> {
        (self.snapshot == *expected_snapshot).then(|| {
            self.groups
                .iter()
                .filter(|group| group.requires_lifecycle_pair())
                .map(|group| group.reservation_group.identity)
                .collect()
        })
    }

    /// Borrow every exact terminal group binding covered by this immutable plan.
    ///
    /// Startup uses these non-authorizing identities only to prove that a
    /// pre-sweep-deferred Kura Pending group will be consumed by the same normal
    /// reconciliation plan before the original Queue receipt is published.
    pub(crate) fn exact_group_bindings(
        &self,
        expected_snapshot: &LaneQueueReservationReconciliationSnapshotV1,
    ) -> Option<Vec<LaneQueueReservationGroupBindingV1>> {
        (self.snapshot == *expected_snapshot).then(|| {
            self.groups
                .iter()
                .map(|group| group.reservation_group)
                .collect()
        })
    }

    /// Consume this batch only for the byte-identical Queue snapshot classified by the planner.
    pub(crate) fn into_queue_groups(
        self,
        expected_snapshot: &LaneQueueReservationReconciliationSnapshotV1,
    ) -> Option<Vec<LaneReservationSnapshotPlannerGroupEvidence>> {
        (self.snapshot == *expected_snapshot).then_some(self.groups)
    }
}

impl LaneReservationReconciliationPlan {
    /// Consume the first immutable plan into snapshot-bound recovery evidence.
    ///
    /// Strict absence and canonical carrier cleanup replace cursor state. A durable release
    /// barrier contributes only its Kura-authenticated post-retirement state and remains paired
    /// with the independently signed lifecycle identity. Retain, certification, pending merge,
    /// and historical recovery remain cursor-only.
    pub(crate) fn startup_snapshot_recovery_evidence(
        self,
    ) -> Result<LaneReservationSnapshotPlannerEvidence, V2ReservationLifecycleError> {
        let LaneReservationReconciliationPlan {
            snapshot,
            actions,
            direct_release,
            ..
        } = self;
        let mut groups = Vec::new();
        let mut identities = BTreeSet::new();
        let mut strictly_absent_hashes = BTreeSet::new();
        for action in actions {
            let evidence = match action {
                ReservationReconciliationAction::Commit {
                    keys,
                    authorization,
                } => {
                    let reservation_group =
                        lane_queue_reservation_group_binding_from_ordered_keys(keys.iter())
                            .map_err(|detail| {
                                V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                    detail: detail.to_owned(),
                                }
                            })?;
                    let applied_state = authorization
                        .snapshot_recovery_applied_state(&reservation_group)
                        .ok_or_else(|| {
                            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                detail: "ApplyCarrier cleanup authority cannot seed exact snapshot recovery"
                                    .to_owned(),
                            }
                        })?;
                    LaneReservationSnapshotPlannerGroupEvidence {
                        reservation_group,
                        ordered_keys: keys,
                        kind: LaneReservationSnapshotPlannerProjectionKind::CanonicalCarrier {
                            applied_state,
                        },
                    }
                }
                ReservationReconciliationAction::DirectRelease {
                    keys,
                    authorization,
                } => {
                    let reservation_group =
                        lane_queue_reservation_group_binding_from_ordered_keys(keys.iter())
                            .map_err(|detail| {
                                V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                    detail: detail.to_owned(),
                                }
                            })?;
                    for key in &keys {
                        if !strictly_absent_hashes.insert(key.signed_transaction_hash) {
                            return Err(
                                V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                    detail: "strict-absence planner groups overlap one Queue owner"
                                        .to_owned(),
                                },
                            );
                        }
                    }
                    let (authorized_group, authorized_keys, release_projection) = authorization
                        .queue_group()
                        .ok_or_else(|| {
                            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                detail: "strict-absence direct-release authority lost its exact group binding"
                                    .to_owned(),
                            }
                        })?;
                    if authorized_group != reservation_group || authorized_keys != keys.as_slice() {
                        return Err(
                            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                detail: "strict-absence action differs from its checked direct-release authority"
                                    .to_owned(),
                            },
                        );
                    }
                    let recovered_state = release_projection.before;
                    if check_production_in_flight_first_release_recover_reservation_snapshot_transition(
                        recovered_state,
                    )
                    .is_none()
                    {
                        return Err(
                            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                detail: "strict-absence snapshot state failed the composed recovery kernel"
                                    .to_owned(),
                            },
                        );
                    }
                    LaneReservationSnapshotPlannerGroupEvidence {
                        reservation_group,
                        ordered_keys: keys,
                        kind: LaneReservationSnapshotPlannerProjectionKind::StrictlyAbsent {
                            recovered_state,
                        },
                    }
                }
                ReservationReconciliationAction::Retire {
                    retirement,
                    resumed,
                    snapshot_release: Some(release_evidence),
                    ..
                } => {
                    let barrier = retirement.queue_release_barrier()?;
                    let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
                        barrier.ordered_keys.iter(),
                    )
                    .map_err(|detail| {
                        V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                            detail: detail.to_owned(),
                        }
                    })?;
                    if !resumed
                        || release_evidence.reservation_group() != reservation_group
                        || release_evidence.retirement_hash() != retirement.digest()?
                    {
                        return Err(
                            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                                detail: "retirement snapshot evidence differs from its exact resumed release barrier"
                                    .to_owned(),
                            },
                        );
                    }
                    LaneReservationSnapshotPlannerGroupEvidence {
                        reservation_group,
                        ordered_keys: barrier.ordered_keys,
                        kind: LaneReservationSnapshotPlannerProjectionKind::RetiredRelease {
                            evidence: release_evidence,
                        },
                    }
                }
                ReservationReconciliationAction::Retain { .. }
                | ReservationReconciliationAction::Retire { .. } => continue,
            };
            if !identities.insert(evidence.reservation_group.identity) {
                return Err(
                    V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                        detail: "planner snapshot evidence duplicates one reservation group"
                            .to_owned(),
                    },
                );
            }
            groups.push(evidence);
        }
        let expected_strictly_absent = direct_release
            .iter()
            .map(|key| key.signed_transaction_hash)
            .collect::<BTreeSet<_>>();
        if strictly_absent_hashes != expected_strictly_absent {
            return Err(
                V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail:
                        "strict-absence planner evidence differs from the global FIFO release set"
                            .to_owned(),
                },
            );
        }
        Ok(LaneReservationSnapshotPlannerEvidence { snapshot, groups })
    }
}

fn exact_pending_merge_for_group(
    group: &LaneQueueReservationReconciliationGroupV1,
    pending_by_transaction: &BTreeMap<
        HashOf<SignedTransaction>,
        (
            HashOf<MergeLedgerEntry>,
            crate::queue::LaneQueueReservationKeyV2,
        ),
    >,
    pending_by_entry: &BTreeMap<
        HashOf<MergeLedgerEntry>,
        Vec<crate::queue::LaneQueueReservationKeyV2>,
    >,
) -> Result<bool, V2ReservationLifecycleError> {
    let mut entry_hash = None;
    let mut matched = 0usize;
    for key in &group.ordered_keys {
        let Some((candidate_hash, pending_key)) =
            pending_by_transaction.get(&key.signed_transaction_hash)
        else {
            continue;
        };
        if pending_key != key || entry_hash.is_some_and(|existing| existing != *candidate_hash) {
            return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                lane_id: group.identity.lane_id,
                proposal_height: group.identity.proposal_height,
            });
        }
        entry_hash = Some(*candidate_hash);
        matched = matched.saturating_add(1);
    }
    if matched == 0 {
        return Ok(false);
    }
    if matched != group.ordered_keys.len() {
        return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
            lane_id: group.identity.lane_id,
            proposal_height: group.identity.proposal_height,
        });
    }
    let entry_hash = entry_hash.expect("a matched pending group has one entry hash");
    let exact_group = pending_by_entry
        .get(&entry_hash)
        .into_iter()
        .flatten()
        .filter(|key| reservation_key_matches_group(key, &group.identity))
        .copied()
        .collect::<Vec<_>>();
    if exact_group != group.ordered_keys {
        return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
            lane_id: group.identity.lane_id,
            proposal_height: group.identity.proposal_height,
        });
    }
    Ok(true)
}

fn exact_committed_carrier_height_for_group(
    group: &LaneQueueReservationReconciliationGroupV1,
    carrier_heights: &[BTreeSet<NonZeroUsize>],
) -> Result<NonZeroUsize, V2ReservationLifecycleError> {
    if carrier_heights.len() != group.ordered_keys.len() {
        return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
            lane_id: group.identity.lane_id,
            proposal_height: group.identity.proposal_height,
        });
    }
    let mut exact_carrier = None;
    for (key, heights) in group.ordered_keys.iter().zip(carrier_heights) {
        let mut heights = heights.iter().copied();
        let Some(height) = heights.next() else {
            return Err(V2ReservationLifecycleError::MissingCommittedBinding {
                transaction_hash: key.signed_transaction_hash,
            });
        };
        if heights.next().is_some() || exact_carrier.is_some_and(|existing| existing != height) {
            return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
                lane_id: group.identity.lane_id,
                proposal_height: group.identity.proposal_height,
            });
        }
        exact_carrier = Some(height);
    }
    exact_carrier.ok_or(V2ReservationLifecycleError::CommittedCarrierMismatch {
        lane_id: group.identity.lane_id,
        proposal_height: group.identity.proposal_height,
    })
}

//! Runner-owned live-height turns for the opaque production lifecycle stack.

use super::*;

fn completion_selection_stops_batch(
    selection: &super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1,
) -> bool {
    matches!(
        selection,
        super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::CertifiedServeCompleted
    )
}

fn ingress_restart_error(output_guard: &ConsensusOutputGuard) -> V2RunnerError {
    output_guard.close_admission_for_restart();
    V2RunnerError::RestartRequired
}

/// Drain one bounded ordinary Completion/Runtime/Ingress batch through the
/// activated lifecycle owner.
///
/// Recovered completion and Decision-Fetch work receives the real borrow-bound
/// runner turn before ordinary work. A pass-through keeps that same borrow
/// alive until the ordinary completion/runtime tail runs, while an ordinary
/// ingress winner is already dequeued and must enter the shared opaque
/// post-dequeue consumer. Special certified-fence and timeout-vote episodes
/// remain separate runner modes because they deliberately bypass the ordinary
/// fair-turn census.
// PendingKura intentionally uses a narrower no-clock decided-lane driver;
// every ordinary height enters this owner-preserving batch here.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(in crate::sumeragi) fn drain_lifecycle_v2_ingress(
    activated: &mut super::super::v2_lifecycle_coordinator::ActivatedProductionLifecycleV1,
    runner: &mut ProductionLifecycleActiveRunnerBorrowV1,
    receiver: &Arc<FairV2Ingress>,
    lane_work: &mut V2LaneWorkAdapter,
    kura: &Kura,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    npos_vrf: &mut V2NposVrfLifecycle,
    limit: usize,
) -> Result<(), V2RunnerError> {
    let (context_id, height, output_guard) =
        activated.with_runner_runtime(runner, |_owner, executor, services, _local_proposal| {
            (
                executor.context().id(),
                executor.context().height,
                services.lifecycle_output_guard(),
            )
        });

    let mut outer_turns = outer_ingress_turns(limit, context_id, height);
    while let Some(current_turn) = outer_turns.next_current() {
        match current_turn.target() {
            LifecycleRunnerRankTarget::Completion => {
                match activated.drive_completion_turn(current_turn, lane_work) {
                    super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionTurnV1::PassThrough(
                        ordinary_turn,
                    ) => {
                        activated.with_runner_runtime(
                            runner,
                            |_owner, executor, services, _local_proposal| {
                                services.drain_completions(executor)?;
                                Ok::<(), V2RunnerError>(())
                            },
                        )?;
                        drop(ordinary_turn);
                    }
                    super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionTurnV1::Selected(
                        selected,
                    ) => {
                        if completion_selection_stops_batch(&selected) {
                            // Settlement makes the adjacent ProducerTurn Ready.
                            // Let run_inner claim it before any Runtime or
                            // Ingress owner can add another Serve to the census.
                            return Ok(());
                        }
                        if selected.restart_required() || output_guard.restart_required() {
                            return Err(V2RunnerError::RestartRequired);
                        }
                    }
                }
            }
            LifecycleRunnerRankTarget::Runtime => {
                let installed_terminal = activated.with_runner_runtime(
                    runner,
                    |_owner, executor, services, _local_proposal| {
                        let was_terminal = executor
                            .local_proposal_directive()?
                            .decided_subject()
                            .is_some();
                        advance_executor(receiver, executor, services, 1)?;
                        let is_terminal = executor
                            .local_proposal_directive()?
                            .decided_subject()
                            .is_some();
                        Ok::<_, V2RunnerError>(!was_terminal && is_terminal)
                    },
                )?;
                if installed_terminal {
                    return Ok(());
                }
            }
            LifecycleRunnerRankTarget::Ingress => {
                match activated.drive_ingress_turn(current_turn) {
                    super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressTurnV1::PassThrough(
                        empty_turn,
                    ) => {
                        drop(empty_turn);
                        break;
                    }
                    super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressTurnV1::Ordinary(
                        ordinary,
                    ) => match activated.consume_prepared_ordinary_ingress_turn(
                        runner,
                        ordinary,
                        lane_work,
                        kura,
                        local_key,
                        block_sync_server,
                        block_sync,
                        block_sync_request,
                        npos_vrf,
                    )? {
                        ProductionPreparedOrdinaryIngressConsumptionV1::Continue => {}
                    },
                    super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressTurnV1::Selected(
                        selected,
                    ) => {
                        use super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1;
                        match selected {
                            ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued
                            | ProductionLifecycleIngressSelectionV1::CertifiedFetchQueued
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeQueued
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeTerminal => {}
                            ProductionLifecycleIngressSelectionV1::CapacityPending
                            | ProductionLifecycleIngressSelectionV1::Retry
                            | ProductionLifecycleIngressSelectionV1::OrdinaryRetained => {
                                return Ok(());
                            }
                            ProductionLifecycleIngressSelectionV1::RestartRequired => {
                                return Err(ingress_restart_error(&output_guard));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_certified_serve_yields_before_the_next_outer_turn() {
        use super::super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1;

        assert!(completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::CertifiedServeCompleted
        ));
        assert!(!completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::RestartRequired
        ));
    }

    #[test]
    fn ingress_restart_closes_output_admission_before_returning_error() {
        let output_guard = ConsensusOutputGuard::isolated();

        assert!(!output_guard.restart_required());
        assert!(matches!(
            ingress_restart_error(&output_guard),
            V2RunnerError::RestartRequired
        ));
        assert!(output_guard.restart_required());
    }
}

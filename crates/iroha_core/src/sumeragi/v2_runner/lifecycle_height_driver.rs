//! Runner-owned live-height turns for the opaque production lifecycle stack.

use super::*;

fn completion_selection_stops_batch(
    selection: &super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1,
) -> bool {
    matches!(
        selection,
        super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted
    )
}

/// Return whether a just-published Validate retained its exact Ready-successor
/// token and must re-enter Completion before any Runtime turn.
fn completion_selection_retries_before_runtime(
    selection: &super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1,
) -> bool {
    matches!(
        selection,
        super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::LifecycleValidatePublished { .. }
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::LifecycleValidateSidecarWoken { .. }
    )
}

fn ingress_selection_retries_before_producer(
    selection: &super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1,
) -> bool {
    matches!(
        selection,
        super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedFetchCapacityPending
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCapacityPending
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeRetry
    )
}

fn ingress_restart_error(output_guard: &ConsensusOutputGuard) -> V2RunnerError {
    output_guard.close_admission_for_restart();
    V2RunnerError::RestartRequired
}

/// Closed proof of whether ProducerTurn planning may run after one ingress batch.
///
/// A non-eligible target is minted only after a typed lifecycle transaction
/// queues asynchronous work. Claimed work retains the coordinator's sole
/// non-Producer lease; terminal replay retains a separate serialized worker
/// barrier without a lease. Pass-through turns preserve the exact target until
/// its typed Completion path advances or releases it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum LifecycleProducerClaimDispositionV1 {
    /// No authenticated in-flight lifecycle lease blocks ProducerTurn planning.
    Eligible,
    /// One exact published/woken Validate successor must resolve before any
    /// ordinary owner can rediscover Apply.
    AwaitingValidateSuccessor {
        /// Exact same-address Validate ordinal retained by the move-only token.
        ordinal: u128,
    },
    /// One exact Validate successor is waiting for the reducer fence to advance.
    AwaitingValidateFence {
        /// Exact same-address Validate ordinal.
        ordinal: u128,
        /// Exact reducer-fence generation which still owns the retry.
        wait: super::super::v2_lifecycle_coordinator::WaitToken,
    },
    /// The exact live Apply barrier is installed while its Ready child awaits
    /// normal global scheduling and bounded worker capacity.
    AwaitingLiveApplyQueue {
        /// Exact terminalized Validate parent.
        parent_ordinal: u128,
        /// Exact protected actor-global Apply child ordinal.
        child_ordinal: u128,
    },
    /// A queued worker or parked worker result must advance through Completion.
    AwaitingCompletion,
    /// A no-lease terminal replay worker must finish before finalization.
    AwaitingReplayCompletion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleProducerClaimTransitionErrorV1 {
    Completion,
    Ingress,
}

impl LifecycleProducerClaimTransitionErrorV1 {
    const fn detail(self) -> &'static str {
        match self {
            Self::Completion => {
                "lifecycle Completion did not match the persisted non-Producer target"
            }
            Self::Ingress => "lifecycle Ingress did not match the persisted non-Producer target",
        }
    }
}

fn producer_claim_transition_error(
    output_guard: &ConsensusOutputGuard,
    error: LifecycleProducerClaimTransitionErrorV1,
) -> V2RunnerError {
    iroha_logger::error!(
        reason = error.detail(),
        "Sumeragi v2 asynchronous lifecycle-owner transition failed closed"
    );
    output_guard.close_admission_for_restart();
    V2RunnerError::Service(error.detail().to_owned())
}

impl LifecycleProducerClaimDispositionV1 {
    /// Initial state before this height has dispatched asynchronous lifecycle work.
    pub(in crate::sumeragi) const fn initial() -> Self {
        Self::Eligible
    }

    /// Return whether the outer height must yield before ProducerTurn planning.
    pub(in crate::sumeragi) const fn requires_yield(self) -> bool {
        !matches!(self, Self::Eligible)
    }

    /// Return whether this state may consume the Ready scheduler branch.
    const fn permits_ready_completion(self) -> bool {
        matches!(self, Self::Eligible | Self::AwaitingLiveApplyQueue { .. })
    }

    /// Return whether an empty physical cut must still select this exact child.
    const fn requires_exact_ready_selection(self) -> bool {
        matches!(self, Self::AwaitingLiveApplyQueue { .. })
    }

    const fn required_ready_ordinal(self) -> Option<u128> {
        match self {
            Self::AwaitingLiveApplyQueue { child_ordinal, .. } => Some(child_ordinal),
            Self::Eligible
            | Self::AwaitingValidateSuccessor { .. }
            | Self::AwaitingValidateFence { .. }
            | Self::AwaitingCompletion
            | Self::AwaitingReplayCompletion => None,
        }
    }

    const fn validate_successor_ordinal(self) -> Option<u128> {
        match self {
            Self::AwaitingValidateSuccessor { ordinal }
            | Self::AwaitingValidateFence { ordinal, .. } => Some(ordinal),
            Self::Eligible
            | Self::AwaitingLiveApplyQueue { .. }
            | Self::AwaitingCompletion
            | Self::AwaitingReplayCompletion => None,
        }
    }

    /// Return whether this target must yield after one Runtime turn, before Ingress.
    const fn blocks_ingress(self) -> bool {
        matches!(
            self,
            Self::AwaitingValidateSuccessor { .. }
                | Self::AwaitingValidateFence { .. }
                | Self::AwaitingCompletion
                | Self::AwaitingReplayCompletion
                | Self::AwaitingLiveApplyQueue { .. }
        )
    }

    fn observe_completion(
        self,
        selected: &super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1,
    ) -> Result<Self, LifecycleProducerClaimTransitionErrorV1> {
        use super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionRecoveredDecisionFetchStoreSettlementV1 as FetchSettlement,
            ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1 as ProposalSettlement,
            ProductionRecoveredLifecycleSignBroadcastSettlementV1 as SignSettlement,
            ProductionRecoveredLifecycleSignCompletionSelectionV1 as SignCompletion,
            ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1 as VoteSettlement,
        };

        if selected.restart_required() {
            return Ok(Self::Eligible);
        }

        match (self, selected) {
            (Self::AwaitingCompletion, Completion::LifecycleValidatePublished { ordinal }) => {
                Ok(Self::AwaitingValidateSuccessor { ordinal: *ordinal })
            }
            (Self::Eligible, Completion::LifecycleValidateSidecarWoken { ordinal }) => {
                Ok(Self::AwaitingValidateSuccessor { ordinal: *ordinal })
            }
            (Self::AwaitingCompletion, Completion::LifecycleValidateSidecarWoken { ordinal }) => {
                Ok(Self::AwaitingValidateSuccessor { ordinal: *ordinal })
            }
            (
                state @ Self::AwaitingValidateSuccessor { ordinal },
                Completion::LifecycleValidateSuccessorCapacityPending {
                    ordinal: pending_ordinal,
                },
            ) if ordinal == *pending_ordinal => Ok(state),
            (
                Self::AwaitingValidateSuccessor { ordinal },
                Completion::LifecycleValidateSuccessorFencePending {
                    ordinal: pending_ordinal,
                    wait,
                },
            ) if ordinal == *pending_ordinal => Ok(Self::AwaitingValidateFence {
                ordinal,
                wait: *wait,
            }),
            (
                state @ Self::AwaitingValidateFence { ordinal, wait },
                Completion::LifecycleValidateSuccessorFencePending {
                    ordinal: pending_ordinal,
                    wait: pending_wait,
                },
            ) if ordinal == *pending_ordinal
                && wait.source() == pending_wait.source()
                && wait.observed_generation() <= pending_wait.observed_generation() =>
            {
                let _ = state;
                Ok(Self::AwaitingValidateFence {
                    ordinal,
                    wait: *pending_wait,
                })
            }
            (
                Self::Eligible
                | Self::AwaitingCompletion
                | Self::AwaitingValidateSuccessor { .. }
                | Self::AwaitingValidateFence { .. },
                Completion::CompletionIoDispatch(Ok(Dispatch::BodyStageAdvanced {
                    parent_ordinal,
                    child_ordinal,
                    child: super::super::v2_lifecycle_coordinator::LifecycleWorkClass::Apply,
                })),
            ) if self
                .validate_successor_ordinal()
                .is_none_or(|ordinal| ordinal == *parent_ordinal) =>
            {
                Ok(Self::AwaitingLiveApplyQueue {
                    parent_ordinal: *parent_ordinal,
                    child_ordinal: *child_ordinal,
                })
            }
            (
                Self::AwaitingCompletion
                | Self::AwaitingValidateSuccessor { .. }
                | Self::AwaitingValidateFence { .. },
                Completion::CompletionIoDispatch(Ok(Dispatch::BodyStageAdvanced {
                    parent_ordinal,
                    child:
                        super::super::v2_lifecycle_coordinator::LifecycleWorkClass::SignVote
                        | super::super::v2_lifecycle_coordinator::LifecycleWorkClass::InvalidBodyReport,
                    ..
                })),
            ) if self
                .validate_successor_ordinal()
                .is_none_or(|ordinal| ordinal == *parent_ordinal) =>
            {
                Ok(Self::Eligible)
            }
            (
                Self::AwaitingCompletion
                | Self::AwaitingValidateSuccessor { .. }
                | Self::AwaitingValidateFence { .. },
                Completion::CompletionIoDispatch(Ok(Dispatch::ValidateNoSuccessor { ordinal })),
            ) if self
                .validate_successor_ordinal()
                .is_none_or(|expected| expected == *ordinal) =>
            {
                Ok(Self::Eligible)
            }
            (
                Self::AwaitingValidateSuccessor { ordinal: expected }
                | Self::AwaitingValidateFence {
                    ordinal: expected, ..
                },
                Completion::CompletionIoDispatch(Ok(Dispatch::ValidateQueued { ordinal })),
            ) if expected == *ordinal => Ok(Self::AwaitingCompletion),
            (
                Self::AwaitingLiveApplyQueue { child_ordinal, .. },
                Completion::CompletionIoDispatch(Ok(Dispatch::ApplyQueued { ordinal })),
            ) if child_ordinal == *ordinal => Ok(Self::AwaitingCompletion),
            (
                state @ Self::AwaitingLiveApplyQueue { child_ordinal, .. },
                Completion::CompletionIoDispatch(Ok(Dispatch::CapacityUnavailable {
                    protected_live_apply_ordinal: Some(protected_ordinal),
                })),
            ) if child_ordinal == *protected_ordinal => Ok(state),
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(
                    Dispatch::ValidateQueued { .. }
                    | Dispatch::ApplyQueued { .. }
                    | Dispatch::SignQueued { .. },
                )),
            ) => Ok(Self::AwaitingCompletion),
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(Dispatch::FetchDispatched { .. })),
            ) => {
                // Dispatch atomically publishes the request and settles the
                // recovered Fetch to its exact external Waiting source. No
                // active lease crosses this asynchronous network wait.
                Ok(Self::Eligible)
            }
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(
                    Dispatch::BodyStageAdvanced { .. }
                    | Dispatch::ReducerFenceWait { .. }
                    | Dispatch::ValidateNoSuccessor { .. },
                )),
            ) => {
                // Ordinary body publication either installs its exact Ready
                // child synchronously or settles the parent on the adapter
                // reducer fence. Neither outcome retains an asynchronous
                // completion owner across the outer producer boundary.
                Ok(Self::Eligible)
            }
            (Self::AwaitingCompletion, Completion::LifecycleDecisionApplyDeferred)
            | (Self::AwaitingCompletion, Completion::LifecycleDecisionApplyRequeued)
            | (Self::AwaitingCompletion, Completion::LifecycleDecisionApplyCompletionDeferred)
            | (Self::Eligible | Self::AwaitingCompletion, Completion::LifecycleValidateDeferred) => {
                Ok(Self::AwaitingCompletion)
            }
            (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(SignCompletion::Broadcast(
                    SignSettlement::Retry,
                )),
            )
            | (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(
                    SignCompletion::ProposalPrepareWal(
                        ProposalSettlement::Retry | ProposalSettlement::CapacityUnavailable,
                    )
                    | SignCompletion::ProposalBroadcastAndSign(
                        ProposalSettlement::Retry | ProposalSettlement::CapacityUnavailable,
                    )
                    | SignCompletion::VoteBroadcastAndSign(VoteSettlement::Retry),
                ),
            )
            | (
                Self::AwaitingCompletion,
                Completion::RecoveredDecisionFetchCompletion(FetchSettlement::Retry(_)),
            ) => Ok(Self::AwaitingCompletion),
            (Self::AwaitingCompletion, Completion::LifecycleDecisionApplyApplied)
            | (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(SignCompletion::Broadcast(
                    SignSettlement::Applied,
                )),
            )
            | (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(
                    SignCompletion::ProposalPrepareWal(ProposalSettlement::Applied)
                    | SignCompletion::ProposalBroadcastAndSign(ProposalSettlement::Applied)
                    | SignCompletion::VoteBroadcastAndSign(VoteSettlement::Applied),
                ),
            )
            | (
                Self::AwaitingCompletion,
                Completion::RecoveredDecisionFetchCompletion(FetchSettlement::Applied),
            )
            | (Self::AwaitingCompletion, Completion::CertifiedFetchBodyPersisted)
            | (Self::AwaitingCompletion, Completion::CertifiedServeClaimedCompleted) => {
                Ok(Self::Eligible)
            }
            (Self::AwaitingCompletion, Completion::CertifiedFetchBodyPersistenceRetry) => {
                Ok(Self::AwaitingCompletion)
            }
            (Self::AwaitingReplayCompletion, Completion::CertifiedServeReplayCompleted) => {
                Ok(Self::Eligible)
            }
            (
                Self::Eligible | Self::AwaitingCompletion,
                Completion::CertifiedServeReplayCompleted,
            ) => Ok(self),
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(Dispatch::CapacityUnavailable { .. })),
            )
            | (Self::Eligible, Completion::RecoveredLifecycleBroadcastRefanout(_)) => Ok(self),
            _ => Err(LifecycleProducerClaimTransitionErrorV1::Completion),
        }
    }

    fn observe_ingress(
        self,
        selected: &super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1,
    ) -> Result<Self, LifecycleProducerClaimTransitionErrorV1> {
        use super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1 as Ingress;

        match (self, selected) {
            (_, Ingress::RestartRequired) => Ok(self),
            (
                Self::Eligible,
                Ingress::CertifiedFetchCapacityPending | Ingress::CertifiedFetchRetry,
            ) => Ok(Self::Eligible),
            (Self::Eligible, Ingress::CertifiedFetchCompetingReady) => Ok(Self::Eligible),
            (Self::Eligible, Ingress::CertifiedFetchQueued) => Ok(Self::AwaitingCompletion),
            (
                Self::Eligible,
                Ingress::RecoveredDecisionFetchCapacityPending
                | Ingress::RecoveredDecisionFetchPreparationRetry,
            ) => Ok(Self::Eligible),
            (Self::Eligible, Ingress::RecoveredDecisionFetchCompetingReady) => Ok(Self::Eligible),
            (Self::Eligible, Ingress::CertifiedServeCompetingReady) => Ok(Self::Eligible),
            (Self::Eligible, Ingress::RecoveredDecisionFetchQueued) => Ok(Self::AwaitingCompletion),
            (
                Self::Eligible,
                Ingress::CertifiedServeCapacityPending | Ingress::CertifiedServeRetry,
            ) => Ok(Self::Eligible),
            (Self::Eligible, Ingress::CertifiedServeQueued) => Ok(Self::AwaitingCompletion),
            (Self::Eligible, Ingress::CertifiedServeReplayQueued) => {
                Ok(Self::AwaitingReplayCompletion)
            }
            (Self::Eligible, Ingress::CertifiedServeTerminal) => Ok(Self::Eligible),
            _ => Err(LifecycleProducerClaimTransitionErrorV1::Ingress),
        }
    }
}

/// Closed result of one bounded Completion/Runtime/Ingress batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct LifecycleV2IngressDrainDispositionV1 {
    producer_claim: LifecycleProducerClaimDispositionV1,
    retry_before_producer: bool,
}

impl LifecycleV2IngressDrainDispositionV1 {
    const fn ready(producer_claim: LifecycleProducerClaimDispositionV1) -> Self {
        Self {
            producer_claim,
            retry_before_producer: false,
        }
    }

    const fn retry_before_producer(producer_claim: LifecycleProducerClaimDispositionV1) -> Self {
        Self {
            producer_claim,
            retry_before_producer: true,
        }
    }

    /// Persist the exact Producer-claim target into the next outer iteration.
    pub(in crate::sumeragi) const fn producer_claim(self) -> LifecycleProducerClaimDispositionV1 {
        self.producer_claim
    }

    /// Return whether this batch must yield before fresh Producer planning.
    pub(in crate::sumeragi) const fn requires_yield(self) -> bool {
        self.retry_before_producer || self.producer_claim.requires_yield()
    }
}

fn recovered_output_drain_disposition(
    settlement: super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1,
    producer_claim: LifecycleProducerClaimDispositionV1,
) -> Option<LifecycleV2IngressDrainDispositionV1> {
    match settlement {
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::Empty
        | super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::Deferred => {
            None
        }
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::SourceRetained => {
            Some(LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                producer_claim,
            ))
        }
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::Completed => {
            Some(LifecycleV2IngressDrainDispositionV1::ready(producer_claim))
        }
    }
}

/// Drain one bounded ordinary Completion/Runtime/Ingress batch through the
/// activated lifecycle owner.
///
/// Lifecycle completion and Decision-Fetch work receives the real borrow-bound
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
    mut producer_claim: LifecycleProducerClaimDispositionV1,
) -> Result<LifecycleV2IngressDrainDispositionV1, V2RunnerError> {
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
        let recovered_output_settlement = activated.with_runner_runtime(
            runner,
            |owner, executor, services, _local_proposal| {
                super::super::v2_lifecycle_coordinator::settle_one_recovered_lifecycle_output(
                    owner, executor, services,
                )
                .map_err(V2RunnerError::from)
            },
        )?;
        if let Some(disposition) =
            recovered_output_drain_disposition(recovered_output_settlement, producer_claim)
        {
            // A preceding completion may have exposed this exact ordinal as
            // the new Ready minimum. Settle it before the turn driver can
            // acquire a registry-backed lease for the cold-only carrier.
            return Ok(disposition);
        }
        match current_turn.target() {
            LifecycleRunnerRankTarget::Completion => {
                use super::super::v2_lifecycle_coordinator::{
                    ProductionLifecycleCompletionPreGateV1 as PreGate,
                    ProductionLifecycleCompletionTurnV1 as CompletionTurn,
                };
                let selected = match activated.drive_completion_pre_gate(current_turn, lane_work) {
                    PreGate::Ordinary(ordinary_turn) => {
                        activated.with_runner_runtime(
                            runner,
                            |_owner, executor, services, _local_proposal| {
                                services
                                    .drain_one_ordinary_completion_after_lifecycle_pass_through(
                                        executor,
                                    )?;
                                Ok::<(), V2RunnerError>(())
                            },
                        )?;
                        drop(ordinary_turn);
                        None
                    }
                    PreGate::Selected(selected) => Some(selected),
                    PreGate::Ready(ready) if producer_claim.permits_ready_completion() => {
                        let completion = match producer_claim.required_ready_ordinal() {
                            Some(ordinal) => activated
                                .drive_ready_completion_turn_requiring_ordinal(ready, ordinal),
                            None => activated.drive_ready_completion_turn(ready),
                        };
                        match completion {
                            CompletionTurn::PassThrough(empty_turn) => {
                                if producer_claim.requires_exact_ready_selection() {
                                    drop(empty_turn);
                                    return Err(producer_claim_transition_error(
                                        &output_guard,
                                        LifecycleProducerClaimTransitionErrorV1::Completion,
                                    ));
                                }
                                // Ready census was empty. Do not generic-drain:
                                // a lifecycle completion may arrive after the
                                // physical pre-gate and must be reclassified.
                                drop(empty_turn);
                                None
                            }
                            CompletionTurn::Selected(selected) => Some(selected),
                        }
                    }
                    PreGate::Ready(ready) => {
                        // The existing non-Producer target owns this turn.
                        // Dropping the opaque empty cursor advances to Runtime
                        // without authorizing a second Ready claim.
                        drop(ready);
                        None
                    }
                };
                if let Some(selected) = selected {
                    producer_claim = producer_claim
                        .observe_completion(&selected)
                        .map_err(|error| producer_claim_transition_error(&output_guard, error))?;
                    if completion_selection_retries_before_runtime(&selected) {
                        // The move-only published-successor token is retained
                        // ahead of physical completion classification. Yield
                        // this iteration so Runtime cannot precede its exact
                        // same-address consumption.
                        return Ok(LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                            producer_claim,
                        ));
                    }
                    if completion_selection_stops_batch(&selected) {
                        // Settlement makes the adjacent ProducerTurn Ready.
                        // Let run_inner claim it before any Runtime or
                        // Ingress owner can add another Serve to the census.
                        return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
                    }
                    if selected.restart_required() || output_guard.restart_required() {
                        return Err(V2RunnerError::RestartRequired);
                    }
                }
            }
            LifecycleRunnerRankTarget::Runtime => {
                let (installed_terminal, lifecycle_yield) = activated.with_runner_runtime(
                    runner,
                    |owner, executor, services, _local_proposal| {
                        let was_terminal = executor
                            .local_proposal_directive()?
                            .decided_subject()
                            .is_some();
                        let lifecycle_yield =
                            advance_executor(receiver, owner, executor, services, 1)?;
                        let is_terminal = executor
                            .local_proposal_directive()?
                            .decided_subject()
                            .is_some();
                        Ok::<_, V2RunnerError>((!was_terminal && is_terminal, lifecycle_yield))
                    },
                )?;
                if installed_terminal || lifecycle_yield {
                    return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
                }
                if producer_claim.blocks_ingress() {
                    return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
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
                    ) => {
                        let consumed = activated.consume_prepared_ordinary_ingress_turn(
                            runner,
                            ordinary,
                            lane_work,
                            kura,
                            local_key,
                            block_sync_server,
                            block_sync,
                            block_sync_request,
                            npos_vrf,
                        );
                        if let Err(error) = consumed {
                            iroha_logger::error!(
                                %error,
                                "Sumeragi v2 ordinary ingress consumption failed closed"
                            );
                            return Err(error);
                        }
                    }
                    super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressTurnV1::Selected(
                        selected,
                    ) => {
                        use super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1;
                        producer_claim = producer_claim
                            .observe_ingress(&selected)
                            .map_err(|error| producer_claim_transition_error(&output_guard, error))?;
                        if ingress_selection_retries_before_producer(&selected) {
                            return Ok(
                                LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                                    producer_claim,
                                ),
                            );
                        }
                        match selected {
                            ProductionLifecycleIngressSelectionV1::CertifiedFetchCompetingReady
                            | ProductionLifecycleIngressSelectionV1::CertifiedFetchQueued
                            | ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCompetingReady
                            | ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeCompetingReady
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeQueued
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeReplayQueued
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeTerminal => {
                                return Ok(LifecycleV2IngressDrainDispositionV1::ready(
                                    producer_claim,
                                ));
                            }
                            ProductionLifecycleIngressSelectionV1::RestartRequired => {
                                return Err(ingress_restart_error(&output_guard));
                            }
                            ProductionLifecycleIngressSelectionV1::CertifiedFetchCapacityPending
                            | ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry
                            | ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCapacityPending
                            | ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending
                            | ProductionLifecycleIngressSelectionV1::CertifiedServeRetry => {
                                unreachable!(
                                    "recovered Fetch retained retries return before the terminal match"
                                )
                            }
                        }
                    }
                }
                if producer_claim.requires_yield() {
                    return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
                }
            }
        }
    }
    Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_certified_serve_yields_before_the_next_outer_turn() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionLifecycleCompletionSelectionV1, RecoveredLifecycleOutputSettlementV1,
        };

        let claim = LifecycleProducerClaimDispositionV1::initial();
        assert_eq!(
            recovered_output_drain_disposition(
                RecoveredLifecycleOutputSettlementV1::Completed,
                claim,
            ),
            Some(LifecycleV2IngressDrainDispositionV1::ready(claim))
        );
        let retained = recovered_output_drain_disposition(
            RecoveredLifecycleOutputSettlementV1::SourceRetained,
            claim,
        )
        .expect("source-retained output requires a timed retry");
        assert!(retained.requires_yield());
        assert_eq!(
            retained,
            LifecycleV2IngressDrainDispositionV1::retry_before_producer(claim)
        );
        assert_eq!(
            recovered_output_drain_disposition(RecoveredLifecycleOutputSettlementV1::Empty, claim),
            None
        );
        assert_eq!(
            recovered_output_drain_disposition(
                RecoveredLifecycleOutputSettlementV1::Deferred,
                claim,
            ),
            None
        );

        assert!(completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted
        ));
        assert!(!completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted
        ));
        assert!(!completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::RestartRequired
        ));
    }

    #[test]
    fn published_validate_token_retries_before_runtime_and_apply_barrier_is_exact() {
        use super::super::super::v2_lifecycle_coordinator::{
            LifecycleWorkClass, ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
        };

        assert!(completion_selection_retries_before_runtime(
            &Completion::LifecycleValidatePublished { ordinal: 7 }
        ));
        assert!(!completion_selection_retries_before_runtime(
            &Completion::CompletionIoDispatch(Ok(Dispatch::BodyStageAdvanced {
                parent_ordinal: 7,
                child_ordinal: 13,
                child: LifecycleWorkClass::Apply,
            }))
        ));
        assert!(!completion_selection_retries_before_runtime(
            &Completion::CompletionIoDispatch(Ok(Dispatch::BodyStageAdvanced {
                parent_ordinal: 7,
                child_ordinal: 13,
                child: LifecycleWorkClass::SignVote,
            }))
        ));
        let claim = LifecycleProducerClaimDispositionV1::AwaitingCompletion
            .observe_completion(&Completion::LifecycleValidatePublished { ordinal: 7 })
            .expect("published Validate retains its asynchronous claim");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSuccessor { ordinal: 7 }
        );
        let claim = claim
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::BodyStageAdvanced {
                    parent_ordinal: 7,
                    child_ordinal: 13,
                    child: LifecycleWorkClass::Apply,
                },
            )))
            .expect("exact Apply child carries an installed retransmit owner");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingLiveApplyQueue {
                parent_ordinal: 7,
                child_ordinal: 13,
            }
        );
        assert!(
            claim
                .observe_completion(&Completion::CompletionIoDispatch(Ok(
                    Dispatch::CapacityUnavailable {
                        protected_live_apply_ordinal: Some(12),
                    },
                )))
                .is_err()
        );
        let claim = claim
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::CapacityUnavailable {
                    protected_live_apply_ordinal: Some(13),
                },
            )))
            .expect("capacity retry names the same protected Apply child");
        let claim = claim
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::ApplyQueued { ordinal: 13 },
            )))
            .expect("the exact child enters its dedicated queue");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingCompletion
        );
    }

    #[test]
    fn recovered_sign_dispatch_yields_until_its_typed_completion_settles() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionRecoveredLifecycleSignBroadcastSettlementV1 as SignSettlement,
            ProductionRecoveredLifecycleSignCompletionSelectionV1 as SignCompletion,
        };

        let claim = LifecycleProducerClaimDispositionV1::initial();
        assert!(!claim.requires_yield());
        let claim = claim
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::SignQueued { ordinal: 1 },
            )))
            .expect("eligible Sign dispatch mints the Completion target");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingCompletion
        );
        let claim = claim
            .observe_completion(&Completion::RecoveredLifecycleSignCompletion(
                SignCompletion::Broadcast(SignSettlement::Applied),
            ))
            .expect("matching Sign completion clears the target");
        assert!(!claim.requires_yield());
    }

    #[test]
    fn terminal_serve_replay_does_not_clear_an_unrelated_in_flight_lease() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
        };

        let claim = LifecycleProducerClaimDispositionV1::initial()
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::SignQueued { ordinal: 1 },
            )))
            .expect("eligible Sign dispatch");
        let claim = claim
            .observe_completion(&Completion::CertifiedServeReplayCompleted)
            .expect("terminal replay completion preserves another active target");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingCompletion
        );
    }

    #[test]
    fn ingress_outcomes_retain_their_exact_next_driver_target() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionLifecycleIngressSelectionV1 as Ingress,
        };

        let eligible = LifecycleProducerClaimDispositionV1::initial();
        let fetch = eligible
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::FetchDispatched { ordinal: 1 },
            )))
            .expect("external-Waiting Fetch dispatch releases its lease");
        assert_eq!(fetch, LifecycleProducerClaimDispositionV1::Eligible);
        assert_eq!(
            eligible.observe_ingress(&Ingress::CertifiedServeCapacityPending),
            Ok(LifecycleProducerClaimDispositionV1::Eligible)
        );
        assert_eq!(
            eligible.observe_ingress(&Ingress::CertifiedServeReplayQueued),
            Ok(LifecycleProducerClaimDispositionV1::AwaitingReplayCompletion)
        );
        assert_eq!(
            fetch.observe_ingress(&Ingress::RecoveredDecisionFetchCapacityPending),
            Ok(LifecycleProducerClaimDispositionV1::Eligible)
        );
        assert_eq!(
            fetch.observe_ingress(&Ingress::RecoveredDecisionFetchPreparationRetry),
            Ok(LifecycleProducerClaimDispositionV1::Eligible)
        );
        assert!(ingress_selection_retries_before_producer(
            &Ingress::RecoveredDecisionFetchCapacityPending
        ));
        assert!(ingress_selection_retries_before_producer(
            &Ingress::RecoveredDecisionFetchPreparationRetry
        ));
        assert!(!ingress_selection_retries_before_producer(
            &Ingress::RecoveredDecisionFetchCompetingReady
        ));
        assert!(!ingress_selection_retries_before_producer(
            &Ingress::CertifiedServeCompetingReady
        ));
        assert!(ingress_selection_retries_before_producer(
            &Ingress::CertifiedServeCapacityPending
        ));
        assert!(ingress_selection_retries_before_producer(
            &Ingress::CertifiedServeRetry
        ));
        let retained_retry = LifecycleV2IngressDrainDispositionV1::retry_before_producer(fetch);
        assert_eq!(
            retained_retry.producer_claim(),
            LifecycleProducerClaimDispositionV1::Eligible
        );
        assert!(retained_retry.requires_yield());
        let competing_ready = LifecycleV2IngressDrainDispositionV1::ready(
            fetch
                .observe_ingress(&Ingress::RecoveredDecisionFetchCompetingReady)
                .expect("competing Ready work leaves the recovered response externally parked"),
        );
        assert_eq!(
            competing_ready.producer_claim(),
            LifecycleProducerClaimDispositionV1::Eligible
        );
        assert!(
            !competing_ready.requires_yield(),
            "a competing Ready Producer must run before the parked Fetch response retries"
        );
        let competing_serve = LifecycleV2IngressDrainDispositionV1::ready(
            eligible
                .observe_ingress(&Ingress::CertifiedServeCompetingReady)
                .expect("competing Ready Producer keeps the Serve request parked"),
        );
        assert_eq!(
            competing_serve.producer_claim(),
            LifecycleProducerClaimDispositionV1::Eligible
        );
        assert!(
            !competing_serve.requires_yield(),
            "a current Serve request cannot starve an authenticated Ready Producer"
        );
        assert_eq!(
            fetch.observe_ingress(&Ingress::RecoveredDecisionFetchQueued),
            Ok(LifecycleProducerClaimDispositionV1::AwaitingCompletion)
        );
        assert_eq!(
            eligible.observe_ingress(&Ingress::CertifiedServeQueued),
            Ok(LifecycleProducerClaimDispositionV1::AwaitingCompletion)
        );
    }

    #[test]
    fn replay_worker_blocks_finalization_until_its_typed_completion() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionLifecycleIngressSelectionV1 as Ingress,
        };

        let claim = LifecycleProducerClaimDispositionV1::initial()
            .observe_ingress(&Ingress::CertifiedServeReplayQueued)
            .expect("eligible terminal replay queues one serialized worker");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingReplayCompletion
        );
        assert!(claim.requires_yield());
        assert_eq!(
            claim.observe_completion(&Completion::CertifiedServeReplayCompleted),
            Ok(LifecycleProducerClaimDispositionV1::Eligible)
        );
    }

    #[test]
    fn mismatched_async_owner_transitions_fail_closed() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionLifecycleIngressSelectionV1 as Ingress,
        };

        let awaiting = LifecycleProducerClaimDispositionV1::initial()
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::SignQueued { ordinal: 1 },
            )))
            .expect("queue the exact incumbent Sign");
        assert_eq!(
            awaiting.observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::SignQueued { ordinal: 2 },
            ))),
            Err(LifecycleProducerClaimTransitionErrorV1::Completion),
            "an active lease cannot authenticate a second Ready dispatch"
        );
        assert_eq!(
            awaiting.observe_ingress(&Ingress::CertifiedServeQueued),
            Err(LifecycleProducerClaimTransitionErrorV1::Ingress),
            "an active lease cannot authenticate a fresh Serve claim"
        );
        assert_eq!(
            LifecycleProducerClaimDispositionV1::initial()
                .observe_completion(&Completion::CertifiedServeClaimedCompleted),
            Err(LifecycleProducerClaimTransitionErrorV1::Completion),
            "a claimed completion cannot release an untracked lease"
        );
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

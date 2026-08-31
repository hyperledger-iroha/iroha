//! Runner-owned live-height turns for the opaque production lifecycle stack.

use super::*;

fn completion_selection_stops_batch(
    selection: &super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1,
) -> bool {
    matches!(
        selection,
        super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied
    )
}

/// Return whether a selected completion retained or exposed exact Ready work
/// and must re-enter Completion before any Runtime turn.
fn completion_selection_retries_before_runtime(
    selection: &super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1,
) -> bool {
    matches!(
        selection,
        super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::LifecycleValidatePublished { .. }
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::LifecycleValidateSidecarWoken { .. }
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::ApplyTerminalDirectBroadcastCompleted
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::ApplyTerminalDirectBroadcastDeferred
            | super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
                super::super::v2_lifecycle_coordinator::ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalPrepareWal(
                    super::super::v2_lifecycle_coordinator::ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied,
                )
                | super::super::v2_lifecycle_coordinator::ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalBroadcastAndSign(
                    super::super::v2_lifecycle_coordinator::ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied,
                ),
            )
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
/// non-Producer lease; terminal replay and a registered Validate sidecar wait
/// retain separate serialized barriers without a lease. Pass-through turns
/// preserve the exact target until its typed Completion path advances or
/// releases it.
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
    /// A registered Validate sidecar wait permits only lane transport and
    /// sealed global pacemaker Progress to run.
    AwaitingValidateSidecar,
    /// A typed Decision Apply owns the terminal barrier until Kura and LedgerV1 settle.
    AwaitingApplyCompletion,
    /// Recovered Apply settled the reducer terminal; only authenticated direct/recovered
    /// Broadcast output handoff and durable rollover may follow.
    ApplyTerminalSettled,
    /// A no-lease terminal replay worker must finish before finalization.
    AwaitingReplayCompletion,
}

/// Sealed authority to let an already-durable local Proposal Sign outrank one
/// retained ordinary Completion head.
///
/// Only an otherwise eligible height can mint this authority. The ordinary
/// completion remains in its exact physical slot; the exception merely lets
/// the local proposal cross Sign and acquire its typed Completion target before
/// another runtime step can reproduce the same ProposalIntent.
pub(in crate::sumeragi) struct LifecycleReadyProposalSignPreemptionPermitV1 {
    _seal: LifecycleReadyProposalSignPreemptionPermitSealV1,
}

struct LifecycleReadyProposalSignPreemptionPermitSealV1;

/// Sealed authority to service only decided-lane recovery after Decision.
///
/// The active runner can mint this token from either of the two Apply
/// dispositions, or from the narrower authoritative repair proof that an
/// `Eligible` executor is terminal and ready for rollover. Both paths keep
/// current-height certified-body service alive without reopening ordinary
/// lifecycle admission.
pub(in crate::sumeragi) struct LifecycleDecidedLaneRecoveryPermitV1 {
    _seal: LifecycleDecidedLaneRecoveryPermitSealV1,
}

struct LifecycleDecidedLaneRecoveryPermitSealV1;

/// Monotonic fence between a terminal executor and finalized ingress closure.
///
/// The fence is minted only from an exact current-height Decision together with
/// either authoritative terminal executor readiness or the typed settled-Apply
/// disposition. It keeps already-owned cold output and Completion work
/// serviceable, but forbids a return to Runtime, fresh Ingress, or Producer
/// planning while the lifecycle finalization census converges.
pub(in crate::sumeragi) struct LifecycleTerminalFinalizationCutV1 {
    _seal: LifecycleTerminalFinalizationCutSealV1,
}

struct LifecycleTerminalFinalizationCutSealV1;

impl LifecycleTerminalFinalizationCutV1 {
    /// Mint the narrower authority needed to reconcile an already-owned Serve
    /// or exact terminal output while the finalization fence remains active.
    pub(in crate::sumeragi) const fn decided_lane_recovery_permit(
        &self,
    ) -> LifecycleDecidedLaneRecoveryPermitV1 {
        LifecycleDecidedLaneRecoveryPermitV1 {
            _seal: LifecycleDecidedLaneRecoveryPermitSealV1,
        }
    }

    /// Mint Broadcast-only Ready authority for an already-owned recovered
    /// output. General Ready dispatch could queue fresh Sign, Fetch, or Apply
    /// runtime work and is therefore unavailable past this cut.
    const fn terminal_ready_broadcast_permit(
        &self,
    ) -> LifecycleApplyTerminalReadyBroadcastPermitV1 {
        LifecycleApplyTerminalReadyBroadcastPermitV1 {
            _seal: LifecycleApplyTerminalReadyBroadcastPermitSealV1,
        }
    }
}

/// Sealed authority to finish only authenticated Broadcast output after Apply.
///
/// The terminal Apply disposition cannot authorize the general Ready dispatcher because fresh
/// Apply, Sign, and Fetch I/O would reopen the settled reducer. This token admits only exact
/// direct-output settlement or the closed recovered-Broadcast refanout transaction.
pub(in crate::sumeragi) struct LifecycleApplyTerminalReadyBroadcastPermitV1 {
    _seal: LifecycleApplyTerminalReadyBroadcastPermitSealV1,
}

struct LifecycleApplyTerminalReadyBroadcastPermitSealV1;

/// Sealed authority to consume only lane-local fair ingress while ordinary ingress is blocked.
///
/// This permit deliberately carries no Decision authority. In particular, it cannot serve or
/// retire global leader-wire traffic and cannot enter the Apply-only exact-output corridor. Apply
/// barriers use `LifecycleDecidedLaneRecoveryPermitV1` instead.
pub(in crate::sumeragi) struct LifecycleBlockedOrdinaryLaneLocalIngressPermitV1 {
    _seal: LifecycleBlockedOrdinaryLaneLocalIngressPermitSealV1,
}

struct LifecycleBlockedOrdinaryLaneLocalIngressPermitSealV1;

/// Sealed authority to admit only global pacemaker Progress while one
/// unprotected Validate sidecar owns the ordinary lifecycle barrier.
///
/// The permit cannot be minted by Apply, generic Completion, or replay
/// barriers, so none of those owners can reopen global fair ingress.
pub(in crate::sumeragi) struct LifecycleValidateSidecarPacemakerEscapePermitV1 {
    _seal: LifecycleValidateSidecarPacemakerEscapePermitSealV1,
}

struct LifecycleValidateSidecarPacemakerEscapePermitSealV1;

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
        !matches!(self, Self::Eligible | Self::ApplyTerminalSettled)
    }

    /// Return whether this state may consume the Ready scheduler branch.
    const fn permits_ready_completion(self) -> bool {
        matches!(self, Self::Eligible | Self::AwaitingLiveApplyQueue { .. })
    }

    /// Mint the narrow ordinary-head preemption authority for an exact Ready
    /// local Proposal Sign.
    pub(in crate::sumeragi) const fn ready_proposal_sign_preemption_permit(
        self,
    ) -> Option<LifecycleReadyProposalSignPreemptionPermitV1> {
        if matches!(self, Self::Eligible) {
            Some(LifecycleReadyProposalSignPreemptionPermitV1 {
                _seal: LifecycleReadyProposalSignPreemptionPermitSealV1,
            })
        } else {
            None
        }
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
            | Self::AwaitingValidateSidecar
            | Self::AwaitingApplyCompletion
            | Self::ApplyTerminalSettled
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
            | Self::AwaitingValidateSidecar
            | Self::AwaitingApplyCompletion
            | Self::ApplyTerminalSettled
            | Self::AwaitingReplayCompletion => None,
        }
    }

    /// Return whether this target must yield before fresh Ingress.
    const fn blocks_ingress(self) -> bool {
        matches!(
            self,
            Self::AwaitingValidateSuccessor { .. }
                | Self::AwaitingValidateFence { .. }
                | Self::AwaitingCompletion
                | Self::AwaitingValidateSidecar
                | Self::AwaitingApplyCompletion
                | Self::ApplyTerminalSettled
                | Self::AwaitingReplayCompletion
                | Self::AwaitingLiveApplyQueue { .. }
        )
    }

    /// Return whether generic Runtime must stop before Completion settles.
    pub(super) const fn blocks_runtime(self) -> bool {
        matches!(
            self,
            Self::AwaitingValidateSidecar
                | Self::AwaitingApplyCompletion
                | Self::ApplyTerminalSettled
        )
    }

    /// Mint authority for the blocked-runtime cut to service only the typed
    /// pacemaker escape.
    ///
    /// A registered Validate sidecar can depend on an unavailable holder and
    /// must not suppress the absolute view deadline. Apply barriers are
    /// already decision-bound and therefore cannot mint this authority.
    pub(super) const fn validate_sidecar_pacemaker_escape_permit(
        self,
    ) -> Option<LifecycleValidateSidecarPacemakerEscapePermitV1> {
        if matches!(self, Self::AwaitingValidateSidecar) {
            Some(LifecycleValidateSidecarPacemakerEscapePermitV1 {
                _seal: LifecycleValidateSidecarPacemakerEscapePermitSealV1,
            })
        } else {
            None
        }
    }

    /// Return whether a Decision Apply crossed its exact terminal settlement.
    pub(super) const fn apply_terminal_settled(self) -> bool {
        matches!(self, Self::ApplyTerminalSettled)
    }

    /// Mint the Broadcast-only Ready authority after exact Apply settlement.
    const fn apply_terminal_ready_broadcast_permit(
        self,
    ) -> Option<LifecycleApplyTerminalReadyBroadcastPermitV1> {
        if self.apply_terminal_settled() {
            Some(LifecycleApplyTerminalReadyBroadcastPermitV1 {
                _seal: LifecycleApplyTerminalReadyBroadcastPermitSealV1,
            })
        } else {
            None
        }
    }

    /// Return whether decided Apply recovery may consume decided-lane fair ingress.
    pub(super) const fn permits_decided_lane_recovery_ingress(self) -> bool {
        matches!(
            self,
            Self::AwaitingApplyCompletion | Self::ApplyTerminalSettled
        )
    }

    /// Return whether the unresolved Apply worker may still consume open fair
    /// ingress. Once Apply is terminal, only the finite prefix captured after
    /// finalized ingress closure may enter decided-lane recovery.
    pub(super) const fn permits_open_decided_lane_recovery_ingress(self) -> bool {
        matches!(self, Self::AwaitingApplyCompletion)
    }

    /// Return whether authoritative terminal executor state may repair a stale
    /// process-local claim and enter decided-lane recovery directly.
    ///
    /// A peer can finish Apply through the ordinary executor path without
    /// carrying either Apply-only process-local disposition into the next
    /// runner turn. Once the executor is terminal and ready for rollover, an
    /// exact current-height body request must be served from Kura instead of
    /// attempting to append fresh lifecycle work after Decision.
    pub(super) const fn permits_terminal_ready_decided_lane_recovery(
        self,
        executor_ready_to_finish: bool,
        decided_subject_present: bool,
    ) -> bool {
        matches!(self, Self::Eligible) && executor_ready_to_finish && decided_subject_present
    }

    /// Mint decided-lane authority from authoritative terminal executor state.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn terminal_ready_decided_lane_recovery_permit(
        self,
        executor_ready_to_finish: bool,
        decided_subject_present: bool,
    ) -> Option<LifecycleDecidedLaneRecoveryPermitV1> {
        if self.permits_terminal_ready_decided_lane_recovery(
            executor_ready_to_finish,
            decided_subject_present,
        ) {
            Some(LifecycleDecidedLaneRecoveryPermitV1 {
                _seal: LifecycleDecidedLaneRecoveryPermitSealV1,
            })
        } else {
            None
        }
    }

    /// Freeze fresh Ingress and Producer ownership after authoritative terminal
    /// state is visible. The returned cut is monotonic for the rest of this
    /// process-height. Completion remains serviceable only through the
    /// Broadcast-only terminal dispatcher and cannot reopen serialized runtime.
    pub(in crate::sumeragi) const fn terminal_finalization_cut(
        self,
        executor_ready_to_finish: bool,
        decided_subject_present: bool,
    ) -> Option<LifecycleTerminalFinalizationCutV1> {
        if decided_subject_present
            && (self.apply_terminal_settled()
                || self.permits_terminal_ready_decided_lane_recovery(
                    executor_ready_to_finish,
                    decided_subject_present,
                ))
        {
            Some(LifecycleTerminalFinalizationCutV1 {
                _seal: LifecycleTerminalFinalizationCutSealV1,
            })
        } else {
            None
        }
    }

    /// Mint the sealed decided-lane recovery authority for either Apply barrier.
    pub(in crate::sumeragi) const fn decided_lane_recovery_permit(
        self,
    ) -> Option<LifecycleDecidedLaneRecoveryPermitV1> {
        if self.permits_decided_lane_recovery_ingress() {
            Some(LifecycleDecidedLaneRecoveryPermitV1 {
                _seal: LifecycleDecidedLaneRecoveryPermitSealV1,
            })
        } else {
            None
        }
    }

    /// Mint lane-local fair-ingress authority while a non-Apply owner blocks ordinary ingress.
    pub(in crate::sumeragi) const fn blocked_ordinary_lane_local_ingress_permit(
        self,
    ) -> Option<LifecycleBlockedOrdinaryLaneLocalIngressPermitV1> {
        if self.blocks_ingress() && !self.permits_decided_lane_recovery_ingress() {
            Some(LifecycleBlockedOrdinaryLaneLocalIngressPermitV1 {
                _seal: LifecycleBlockedOrdinaryLaneLocalIngressPermitSealV1,
            })
        } else {
            None
        }
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
            (
                Self::Eligible | Self::AwaitingCompletion | Self::AwaitingValidateSidecar,
                Completion::LifecycleValidateSidecarWoken { ordinal },
            ) => {
                // Woken retains the immutable registration that installed
                // AwaitingValidateSidecar, so this is the same Validate row
                // advancing back into its exact Ready-successor corridor.
                Ok(Self::AwaitingValidateSuccessor { ordinal: *ordinal })
            }
            (
                Self::AwaitingValidateSidecar,
                Completion::LifecycleValidateSidecarSuperseded { .. },
            ) => Ok(Self::Eligible),
            (
                state @ Self::AwaitingValidateSuccessor { ordinal },
                Completion::LifecycleValidateSuccessorCapacityPending {
                    ordinal: pending_ordinal,
                },
            ) if ordinal == *pending_ordinal => Ok(state),
            (
                state @ Self::AwaitingValidateFence { ordinal, .. },
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
            ) if child_ordinal == *ordinal => Ok(Self::AwaitingApplyCompletion),
            (
                state @ Self::AwaitingLiveApplyQueue { child_ordinal, .. },
                Completion::CompletionIoDispatch(Ok(Dispatch::CapacityUnavailable {
                    protected_live_apply_ordinal: Some(protected_ordinal),
                })),
            ) if child_ordinal == *protected_ordinal => Ok(state),
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(
                    Dispatch::ValidateQueued { .. } | Dispatch::SignQueued { .. },
                )),
            ) => Ok(Self::AwaitingCompletion),
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(Dispatch::ApplyQueued { .. })),
            ) => Ok(Self::AwaitingApplyCompletion),
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
            (Self::AwaitingApplyCompletion, Completion::LifecycleDecisionApplyDeferred)
            | (Self::AwaitingApplyCompletion, Completion::LifecycleDecisionApplyRequeued)
            | (
                Self::AwaitingApplyCompletion,
                Completion::LifecycleDecisionApplyCompletionDeferred,
            ) => Ok(Self::AwaitingApplyCompletion),
            (Self::Eligible | Self::AwaitingCompletion, Completion::LifecycleValidateDeferred) => {
                Ok(Self::AwaitingCompletion)
            }
            (
                Self::Eligible | Self::AwaitingCompletion | Self::AwaitingValidateSidecar,
                Completion::LifecycleValidateSidecarWaiting,
            ) => {
                // Registration moved the exact Validate row to an external
                // sidecar wait and released the coordinator lease. Keep
                // Producer and ordinary lifecycle ingress blocked while the
                // narrowed barrier admits its authenticated lane response and
                // sealed global pacemaker Progress.
                Ok(Self::AwaitingValidateSidecar)
            }
            (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(SignCompletion::Broadcast(
                    SignSettlement::Retry,
                )),
            )
            | (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(SignCompletion::Retry),
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
            (Self::AwaitingApplyCompletion, Completion::LifecycleDecisionApplyApplied) => {
                Ok(Self::ApplyTerminalSettled)
            }
            (
                Self::AwaitingCompletion,
                Completion::RecoveredLifecycleSignCompletion(SignCompletion::Superseded),
            )
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
                Self::Eligible | Self::AwaitingCompletion | Self::AwaitingApplyCompletion,
                Completion::CertifiedServeReplayCompleted,
            ) => Ok(self),
            (
                Self::Eligible,
                Completion::CompletionIoDispatch(Ok(Dispatch::CapacityUnavailable { .. })),
            )
            | (
                Self::Eligible | Self::ApplyTerminalSettled,
                Completion::RecoveredLifecycleBroadcastRefanout(_),
            ) => Ok(self),
            (
                Self::ApplyTerminalSettled,
                Completion::ApplyTerminalDirectBroadcastCompleted
                | Completion::ApplyTerminalDirectBroadcastDeferred,
            ) => Ok(self),
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
    terminal_settlement_stops_runtime: bool,
    advance_executor_yield: Option<AdvanceExecutorYieldV1>,
}

impl LifecycleV2IngressDrainDispositionV1 {
    const fn ready(producer_claim: LifecycleProducerClaimDispositionV1) -> Self {
        Self {
            producer_claim,
            retry_before_producer: false,
            terminal_settlement_stops_runtime: false,
            advance_executor_yield: None,
        }
    }

    const fn after_terminal_settlement(
        producer_claim: LifecycleProducerClaimDispositionV1,
    ) -> Self {
        Self {
            producer_claim,
            retry_before_producer: false,
            terminal_settlement_stops_runtime: true,
            advance_executor_yield: None,
        }
    }

    const fn retry_before_producer(producer_claim: LifecycleProducerClaimDispositionV1) -> Self {
        Self {
            producer_claim,
            retry_before_producer: true,
            terminal_settlement_stops_runtime: false,
            advance_executor_yield: None,
        }
    }

    const fn after_advance_executor_yield(
        producer_claim: LifecycleProducerClaimDispositionV1,
        advance_executor_yield: AdvanceExecutorYieldV1,
    ) -> Self {
        Self {
            producer_claim,
            retry_before_producer: false,
            terminal_settlement_stops_runtime: false,
            advance_executor_yield: Some(advance_executor_yield),
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

    /// Return whether terminal settlement must precede all ordinary runtime service.
    pub(in crate::sumeragi) const fn terminal_settlement_stops_runtime(self) -> bool {
        self.terminal_settlement_stops_runtime
    }

    /// Whether the pre-Ingress runtime turn yielded on serialized executor debt.
    pub(in crate::sumeragi) const fn advance_executor_yield(
        self,
    ) -> Option<AdvanceExecutorYieldV1> {
        self.advance_executor_yield
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
            Some(LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(
                producer_claim,
            ))
        }
    }
}

fn settled_apply_output_drain_disposition(
    settlement: super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1,
    producer_claim: LifecycleProducerClaimDispositionV1,
) -> Option<LifecycleV2IngressDrainDispositionV1> {
    debug_assert!(producer_claim.apply_terminal_settled());
    match settlement {
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::SourceRetained => {
            Some(LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                producer_claim,
            ))
        }
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::Completed => {
            Some(LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(
                producer_claim,
            ))
        }
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::Empty
        | super::super::v2_lifecycle_coordinator::RecoveredLifecycleOutputSettlementV1::Deferred => None,
    }
}

const fn blocked_runtime_drain_disposition(
    producer_claim: LifecycleProducerClaimDispositionV1,
) -> LifecycleV2IngressDrainDispositionV1 {
    if producer_claim.apply_terminal_settled() {
        LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(producer_claim)
    } else {
        LifecycleV2IngressDrainDispositionV1::ready(producer_claim)
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
    npos_beacon: &mut V2GlobalBeaconLifecycle,
    limit: usize,
    mut producer_claim: LifecycleProducerClaimDispositionV1,
    terminal_finalization_cut: Option<&LifecycleTerminalFinalizationCutV1>,
) -> Result<LifecycleV2IngressDrainDispositionV1, V2RunnerError> {
    if terminal_finalization_cut.is_some() || producer_claim.apply_terminal_settled() {
        // Make an expired lower direct-output row visible to the cold-output
        // order check before it can service a higher recovered Broadcast.
        // Waking is itself a bounded state transition, so re-enter Completion
        // on the next outer iteration rather than combining it with service I/O.
        let direct_output_woken = activated.with_runner_runtime(
            runner,
            |owner, executor, services, _local_proposal| {
                let fence = executor.lifecycle_reducer_fence_observation();
                match owner.wake_apply_terminal_direct_broadcast_if_fenced(fence) {
                    Ok(woken) => Ok(woken),
                    Err(error) => {
                        iroha_logger::error!(
                            ?error,
                            "Sumeragi v2 post-Apply direct-output fence wake failed closed"
                        );
                        services
                            .lifecycle_output_guard()
                            .close_admission_for_restart();
                        Err(V2RunnerError::RestartRequired)
                    }
                }
            },
        )?;
        if direct_output_woken {
            return Ok(LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                producer_claim,
            ));
        }
    }
    if producer_claim.apply_terminal_settled() {
        // Applied is already the exact reducer terminal. First drain retained,
        // authenticated cold-open output rows required by the finalization
        // census. If none owns this turn, continue only far enough for the
        // ordinary Completion pre-gate and authenticated Ready classifier to
        // park a recovered signed Broadcast on its exact-output wait. Runtime,
        // Ingress, and Producer remain fenced by `ApplyTerminalSettled`.
        let settlement = activated.with_runner_runtime(
            runner,
            |owner, executor, services, _local_proposal| {
                super::super::v2_lifecycle_coordinator::settle_one_recovered_lifecycle_output(
                    owner, executor, services,
                )
                .map_err(V2RunnerError::from)
            },
        )?;
        if let Some(disposition) =
            settled_apply_output_drain_disposition(settlement, producer_claim)
        {
            return Ok(disposition);
        }
    }
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
        if !producer_claim.blocks_runtime() {
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
        }
        match current_turn.target() {
            LifecycleRunnerRankTarget::Completion => {
                use super::super::v2_lifecycle_coordinator::{
                    ProductionLifecycleCompletionPreGateV1 as PreGate,
                    ProductionLifecycleCompletionTurnV1 as CompletionTurn,
                };
                let proposal_sign_preemption =
                    producer_claim.ready_proposal_sign_preemption_permit();
                let pre_gate = match proposal_sign_preemption.as_ref() {
                    Some(permit) => activated
                        .drive_completion_pre_gate_with_ready_proposal_sign_preemption(
                            current_turn,
                            lane_work,
                            permit,
                        ),
                    None => activated.drive_completion_pre_gate(current_turn, lane_work),
                };
                let selected = match pre_gate {
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
                    PreGate::Ready(ready) if terminal_finalization_cut.is_some() => {
                        let permit = terminal_finalization_cut
                            .expect("the terminal cut remains borrowed for this Completion turn")
                            .terminal_ready_broadcast_permit();
                        match activated.drive_apply_terminal_ready_broadcast_turn(ready, permit) {
                            CompletionTurn::PassThrough(empty_turn) => {
                                drop(empty_turn);
                                None
                            }
                            CompletionTurn::Selected(selected) => Some(selected),
                        }
                    }
                    PreGate::Ready(ready) if producer_claim.apply_terminal_settled() => {
                        let permit = producer_claim
                            .apply_terminal_ready_broadcast_permit()
                            .expect("the terminal Apply disposition mints its sealed Ready permit");
                        match activated.drive_apply_terminal_ready_broadcast_turn(ready, permit) {
                            CompletionTurn::PassThrough(empty_turn) => {
                                // Completion I/O and an empty census retain the exact cursor.
                                // Dropping it advances only to the existing Runtime fence below.
                                drop(empty_turn);
                                None
                            }
                            CompletionTurn::Selected(selected) => Some(selected),
                        }
                    }
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
                        return Ok(
                            LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(
                                producer_claim,
                            ),
                        );
                    }
                    if selected.restart_required() || output_guard.restart_required() {
                        return Err(V2RunnerError::RestartRequired);
                    }
                }
                if terminal_finalization_cut.is_some() {
                    // The sealed cut owns exactly the Completion-ranked turn.
                    // Return before asking the cursor for Runtime or Ingress.
                    return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
                }
            }
            LifecycleRunnerRankTarget::Runtime => {
                debug_assert!(
                    terminal_finalization_cut.is_none(),
                    "terminal finalization cannot acquire a Runtime turn"
                );
                if producer_claim.blocks_runtime() {
                    // A typed Apply or registered Validate sidecar wait can
                    // advance outside this batch. Ordinary Runtime and Ingress
                    // remain inert; run_inner separately admits only the
                    // sidecar claim's sealed pacemaker Progress escape.
                    return Ok(blocked_runtime_drain_disposition(producer_claim));
                }
                let (pre_timeout_cut, pre_timeout_advanced) = activated.with_runner_runtime(
                    runner,
                    |_owner, executor, services, _local_proposal| {
                        let now = Instant::now();
                        let Some(cut) = executor.freeze_pre_timeout_locked_prepare_qc_cut(
                            now,
                            receiver.next_physical_admission_ordinal(),
                        )?
                        else {
                            return Ok::<_, V2RunnerError>((None, false));
                        };
                        match executor
                            .step_pre_timeout_locked_prepare_qc_once(now, &cut, services)?
                        {
                            EffectExecutorStep::Idle => Ok((Some(cut), false)),
                            EffectExecutorStep::Advanced { .. } => {
                                let _ = reconcile_executor_locked_body(executor, services)?;
                                Ok((None, true))
                            }
                        }
                    },
                )?;
                if pre_timeout_advanced {
                    // One exact already-admitted Prepare carrier advanced
                    // ahead of the frozen timeout owner. Re-enter
                    // Completion/Runtime so either the next pre-cut witness or
                    // the resulting WAL fence settles before any Producer or
                    // ordinary timeout turn.
                    return Ok(LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                        producer_claim,
                    ));
                }
                if let Some(pre_timeout_cut) = pre_timeout_cut {
                    use super::super::v2_lifecycle_coordinator::ProductionPreTimeoutLockedPrepareQcIngressTurnV1 as PreTimeoutIngress;
                    match activated
                        .prepare_pre_timeout_locked_prepare_qc_ingress_turn(&pre_timeout_cut)
                    {
                        PreTimeoutIngress::Empty => {}
                        PreTimeoutIngress::RestartRequired => {
                            return Err(ingress_restart_error(&output_guard));
                        }
                        PreTimeoutIngress::ObsoletePredecessor(prepared) => {
                            let consumption = activated.consume_prepared_ordinary_ingress_turn(
                                runner,
                                prepared,
                                lane_work,
                                kura,
                                local_key,
                                block_sync_server,
                                block_sync,
                                block_sync_request,
                                npos_beacon,
                            )?;
                            match consumption {
                                super::ordinary_ingress_consumer::ProductionPreparedOrdinaryIngressConsumptionV1::Continue => {}
                            }
                            // Dequeue retirement strictly lowers the fixed
                            // pre-cut predecessor rank. A fresh outer turn
                            // remints the same timeout owner/cut and cannot see
                            // any later producer append.
                            return Ok(
                                LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                                    producer_claim,
                                ),
                            );
                        }
                        PreTimeoutIngress::ExactPrepareProgress(prepared) => {
                            let consumption = activated.consume_prepared_ordinary_ingress_turn(
                                runner,
                                prepared,
                                lane_work,
                                kura,
                                local_key,
                                block_sync_server,
                                block_sync,
                                block_sync_request,
                                npos_beacon,
                            )?;
                            match consumption {
                                super::ordinary_ingress_consumer::ProductionPreparedOrdinaryIngressConsumptionV1::Continue => {}
                            }
                            activated.with_runner_runtime(
                                runner,
                                |_owner, executor, services, _local_proposal| match executor
                                    .step_pre_timeout_locked_prepare_qc_once(
                                        Instant::now(),
                                        &pre_timeout_cut,
                                        services,
                                    )? {
                                    EffectExecutorStep::Idle => Ok::<_, V2RunnerError>(()),
                                    EffectExecutorStep::Advanced { .. } => {
                                        let _ = reconcile_executor_locked_body(executor, services)?;
                                        Ok(())
                                    }
                                },
                            )?;
                            // Authentication or ordinary admission can retire
                            // a previewed duplicate/version-mismatched carrier
                            // without queueing the runtime command. Whether it
                            // advanced or stuttered, retry the same frozen
                            // physical prefix: its roster-bounded rank strictly
                            // decreased, so this cannot admit a post-cut
                            // carrier or defer the timeout forever.
                            return Ok(
                                LifecycleV2IngressDrainDispositionV1::retry_before_producer(
                                    producer_claim,
                                ),
                            );
                        }
                    }
                }
                let (installed_terminal, executor_slice) = activated.with_runner_runtime(
                    runner,
                    |owner, executor, services, _local_proposal| {
                        let was_terminal = executor
                            .local_proposal_directive()?
                            .decided_subject()
                            .is_some();
                        let executor_slice =
                            advance_executor(receiver, owner, executor, services, 1)?;
                        let is_terminal = executor
                            .local_proposal_directive()?
                            .decided_subject()
                            .is_some();
                        Ok::<_, V2RunnerError>((!was_terminal && is_terminal, executor_slice))
                    },
                )?;
                if installed_terminal {
                    return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
                }
                match executor_slice {
                    AdvanceExecutorSliceOutcomeV1::Idle
                    | AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary => {}
                    AdvanceExecutorSliceOutcomeV1::Yielded(advance_executor_yield) => {
                        return Ok(
                            LifecycleV2IngressDrainDispositionV1::after_advance_executor_yield(
                                producer_claim,
                                advance_executor_yield,
                            ),
                        );
                    }
                }
                if producer_claim.blocks_ingress() {
                    return Ok(LifecycleV2IngressDrainDispositionV1::ready(producer_claim));
                }
            }
            LifecycleRunnerRankTarget::Ingress => {
                debug_assert!(
                    terminal_finalization_cut.is_none(),
                    "terminal finalization must stop before the open Ingress turn"
                );
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
                            npos_beacon,
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
        let completed = recovered_output_drain_disposition(
            RecoveredLifecycleOutputSettlementV1::Completed,
            claim,
        )
        .expect("completed output stops the ordinary runtime suffix");
        assert_eq!(
            completed,
            LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(claim)
        );
        assert!(completed.terminal_settlement_stops_runtime());
        assert!(!completed.requires_yield());
        let retained = recovered_output_drain_disposition(
            RecoveredLifecycleOutputSettlementV1::SourceRetained,
            claim,
        )
        .expect("source-retained output requires a timed retry");
        assert!(retained.requires_yield());
        assert!(!retained.terminal_settlement_stops_runtime());
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
        assert!(completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied
        ));
        assert!(!completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted
        ));
        assert!(!completion_selection_stops_batch(
            &ProductionLifecycleCompletionSelectionV1::RestartRequired
        ));
    }

    #[test]
    fn drain_disposition_preserves_exact_executor_yield_owner() {
        let claim = LifecycleProducerClaimDispositionV1::initial();
        let reason = AdvanceExecutorYieldV1::new(
            AdvanceExecutorYieldCheckpointV1::AfterStep,
            AdvanceExecutorYieldCauseV1::SettledLifecycleOutput,
        );
        let disposition =
            LifecycleV2IngressDrainDispositionV1::after_advance_executor_yield(claim, reason);

        assert_eq!(disposition.advance_executor_yield(), Some(reason));
        assert_eq!(disposition.producer_claim(), claim);
        assert!(!disposition.requires_yield());
    }

    #[test]
    fn only_an_eligible_claim_can_preempt_an_ordinary_head_for_ready_proposal_sign() {
        assert!(
            LifecycleProducerClaimDispositionV1::Eligible
                .ready_proposal_sign_preemption_permit()
                .is_some()
        );
        for claim in [
            LifecycleProducerClaimDispositionV1::AwaitingCompletion,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar,
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion,
            LifecycleProducerClaimDispositionV1::ApplyTerminalSettled,
        ] {
            assert!(
                claim.ready_proposal_sign_preemption_permit().is_none(),
                "{claim:?} must retain its existing non-Producer owner"
            );
        }
    }

    #[test]
    fn terminal_ready_executor_repairs_an_eligible_decided_lane_claim() {
        let eligible = LifecycleProducerClaimDispositionV1::initial();
        assert!(eligible.permits_terminal_ready_decided_lane_recovery(true, true));
        assert!(
            eligible
                .terminal_ready_decided_lane_recovery_permit(true, true)
                .is_some()
        );
        assert!(!eligible.permits_terminal_ready_decided_lane_recovery(false, true));
        assert!(
            eligible
                .terminal_ready_decided_lane_recovery_permit(false, true)
                .is_none()
        );
        assert!(!eligible.permits_terminal_ready_decided_lane_recovery(true, false));
        assert!(
            !LifecycleProducerClaimDispositionV1::AwaitingCompletion
                .permits_terminal_ready_decided_lane_recovery(true, true),
            "an unrelated live owner cannot be bypassed by terminal recovery"
        );
        assert!(
            !LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
                .permits_terminal_ready_decided_lane_recovery(true, true),
            "the existing Apply-only recovery corridor remains authoritative"
        );

        assert!(eligible.terminal_finalization_cut(true, true).is_some());
        assert!(eligible.terminal_finalization_cut(false, true).is_none());
        assert!(eligible.terminal_finalization_cut(true, false).is_none());
        assert!(
            LifecycleProducerClaimDispositionV1::ApplyTerminalSettled
                .terminal_finalization_cut(false, true)
                .is_some(),
            "typed Apply settlement fences admission even before its invariant check"
        );
        assert!(
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
                .permits_open_decided_lane_recovery_ingress(),
            "an unresolved Apply may still recover the carrier needed to finish"
        );
        assert!(
            !LifecycleProducerClaimDispositionV1::ApplyTerminalSettled
                .permits_open_decided_lane_recovery_ingress(),
            "settled Apply must wait for the finite closed-ingress drain"
        );
    }

    #[test]
    fn settled_apply_reenters_ready_completion_for_recovered_broadcast() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1 as Refanout,
            RecoveredLifecycleOutputSettlementV1 as OutputSettlement,
        };

        let claim = LifecycleProducerClaimDispositionV1::ApplyTerminalSettled;
        assert!(
            !claim.permits_ready_completion(),
            "a terminal Apply cannot enter the general Ready I/O dispatcher"
        );
        assert!(
            claim.apply_terminal_ready_broadcast_permit().is_some(),
            "a terminal Apply mints only the sealed Broadcast-output Ready permit"
        );
        assert!(
            LifecycleProducerClaimDispositionV1::Eligible
                .apply_terminal_ready_broadcast_permit()
                .is_none(),
            "an ordinary eligible claim cannot mint terminal output authority"
        );
        assert_eq!(
            settled_apply_output_drain_disposition(OutputSettlement::Empty, claim),
            None,
            "an empty cold-output owner must fall through to the Ready classifier"
        );
        assert_eq!(
            settled_apply_output_drain_disposition(OutputSettlement::Deferred, claim),
            None,
            "a deferred cold-output owner must not hide registry-backed Ready work"
        );

        for outcome in [
            Refanout::CapacityUnavailable,
            Refanout::Refanned { ordinal: 86 },
        ] {
            assert_eq!(
                claim.observe_completion(&Completion::RecoveredLifecycleBroadcastRefanout(Ok(
                    outcome,
                ))),
                Ok(claim),
                "refanout retains the terminal Apply barrier until durable output handoff"
            );
        }
        for direct in [
            Completion::ApplyTerminalDirectBroadcastCompleted,
            Completion::ApplyTerminalDirectBroadcastDeferred,
        ] {
            assert_eq!(
                claim.observe_completion(&direct),
                Ok(claim),
                "direct Broadcast settlement retains the terminal Apply barrier"
            );
            assert!(
                completion_selection_retries_before_runtime(&direct),
                "each direct Broadcast outcome must re-enter Completion before rollover"
            );
        }

        let completed = settled_apply_output_drain_disposition(OutputSettlement::Completed, claim)
            .expect("a completed cold output retains the existing one-turn barrier");
        assert!(completed.terminal_settlement_stops_runtime());
        assert_eq!(completed.producer_claim(), claim);
        let after_ready_refanout = blocked_runtime_drain_disposition(claim);
        assert!(
            after_ready_refanout.terminal_settlement_stops_runtime(),
            "a Ready refanout must not reopen the reducer/runtime suffix"
        );
        assert_eq!(after_ready_refanout.producer_claim(), claim);
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
        assert!(completion_selection_retries_before_runtime(
            &Completion::LifecycleDecisionApplyApplied
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
        assert!(
            claim.requires_yield(),
            "the retained same-address Validate successor must precede ProducerTurn"
        );
        let fenced = claim
            .observe_completion(&Completion::LifecycleValidateSuccessorFencePending {
                ordinal: 7,
                wait: super::super::super::v2_lifecycle_coordinator::wait_token_for_test(
                    super::super::super::v2_lifecycle_coordinator::WaitSource::External(
                        super::super::super::v2_lifecycle_coordinator::LifecycleDigest::new(
                            [11; 32],
                        ),
                    ),
                    3,
                ),
            })
            .expect("the exact reducer-fence retry retains the Validate successor");
        assert!(
            fenced.requires_yield(),
            "a reducer-fenced Validate successor must precede ProducerTurn"
        );
        assert_eq!(
            fenced
                .observe_completion(&Completion::LifecycleValidateSuccessorCapacityPending {
                    ordinal: 7,
                })
                .expect("capacity retry retains the exact reducer-fenced Validate successor"),
            fenced
        );
        assert!(
            fenced
                .observe_completion(&Completion::LifecycleValidateSuccessorCapacityPending {
                    ordinal: 8,
                })
                .is_err(),
            "capacity retry for another ordinal must fail closed"
        );
        let sidecar = LifecycleProducerClaimDispositionV1::AwaitingCompletion
            .observe_completion(&Completion::LifecycleValidateSidecarWaiting)
            .expect("registered Validate sidecar retains the height");
        assert_eq!(
            sidecar,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar
        );
        assert!(sidecar.requires_yield());
        assert_eq!(
            sidecar
                .observe_completion(&Completion::LifecycleValidateSidecarWoken { ordinal: 17 })
                .expect("normal sidecar wake retains its exact Validate successor"),
            LifecycleProducerClaimDispositionV1::AwaitingValidateSuccessor { ordinal: 17 }
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
            claim.requires_yield(),
            "the protected live Apply child must precede ProducerTurn"
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
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
        );
    }

    #[test]
    fn recovered_sign_dispatch_yields_until_its_typed_completion_settles() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
            ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1 as ProposalSettlement,
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
        let retry_claim = claim
            .observe_completion(&Completion::RecoveredLifecycleSignCompletion(
                SignCompletion::Retry,
            ))
            .expect("transient runtime debt retains the exact Sign target");
        assert_eq!(retry_claim, claim);
        let superseded_claim = retry_claim
            .observe_completion(&Completion::RecoveredLifecycleSignCompletion(
                SignCompletion::Superseded,
            ))
            .expect("durable supersession cancellation clears the Sign target");
        assert!(!superseded_claim.requires_yield());
        let broadcast_applied = Completion::RecoveredLifecycleSignCompletion(
            SignCompletion::Broadcast(SignSettlement::Applied),
        );
        let applied_claim = claim
            .observe_completion(&broadcast_applied)
            .expect("matching Sign completion clears the target");
        assert!(!applied_claim.requires_yield());
        assert!(
            !completion_selection_retries_before_runtime(&broadcast_applied),
            "an ordinary signed Broadcast leaves its durable refanout to normal Completion ranking"
        );

        for proposal_applied in [
            Completion::RecoveredLifecycleSignCompletion(SignCompletion::ProposalPrepareWal(
                ProposalSettlement::Applied,
            )),
            Completion::RecoveredLifecycleSignCompletion(SignCompletion::ProposalBroadcastAndSign(
                ProposalSettlement::Applied,
            )),
        ] {
            let applied_claim = claim
                .observe_completion(&proposal_applied)
                .expect("matching Proposal Sign completion clears the target");
            assert_eq!(applied_claim, LifecycleProducerClaimDispositionV1::Eligible);
            assert!(
                completion_selection_retries_before_runtime(&proposal_applied),
                "an applied Proposal Sign must re-enter Completion before Runtime can mint timeout work"
            );
        }
    }

    #[test]
    fn blocked_ordinary_claims_mint_only_lane_local_fair_ingress_authority() {
        for claim in [
            LifecycleProducerClaimDispositionV1::AwaitingValidateSuccessor { ordinal: 1 },
            LifecycleProducerClaimDispositionV1::AwaitingValidateFence {
                ordinal: 1,
                wait: super::super::super::v2_lifecycle_coordinator::wait_token_for_test(
                    super::super::super::v2_lifecycle_coordinator::WaitSource::External(
                        super::super::super::v2_lifecycle_coordinator::LifecycleDigest::new(
                            [12; 32],
                        ),
                    ),
                    4,
                ),
            },
            LifecycleProducerClaimDispositionV1::AwaitingLiveApplyQueue {
                parent_ordinal: 2,
                child_ordinal: 3,
            },
            LifecycleProducerClaimDispositionV1::AwaitingCompletion,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar,
            LifecycleProducerClaimDispositionV1::AwaitingReplayCompletion,
        ] {
            assert!(
                claim.blocked_ordinary_lane_local_ingress_permit().is_some(),
                "a non-Apply ordinary-ingress barrier must preserve independent lane progress"
            );
            assert!(claim.decided_lane_recovery_permit().is_none());
        }

        assert!(
            LifecycleProducerClaimDispositionV1::Eligible
                .blocked_ordinary_lane_local_ingress_permit()
                .is_none()
        );
        for claim in [
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion,
            LifecycleProducerClaimDispositionV1::ApplyTerminalSettled,
        ] {
            assert!(
                claim.blocked_ordinary_lane_local_ingress_permit().is_none(),
                "Apply barriers retain their narrower decided-lane recovery authority"
            );
            assert!(claim.decided_lane_recovery_permit().is_some());
        }
    }

    #[test]
    fn registered_validate_sidecar_wake_advances_the_persisted_barrier() {
        use super::super::super::v2_lifecycle_coordinator::{
            ProductionCompletionDispatchV1 as Dispatch,
            ProductionLifecycleCompletionSelectionV1 as Completion,
        };

        let claim = LifecycleProducerClaimDispositionV1::initial()
            .observe_completion(&Completion::CompletionIoDispatch(Ok(
                Dispatch::ValidateQueued { ordinal: 41 },
            )))
            .expect("queued Validate mints its Completion target")
            .observe_completion(&Completion::LifecycleValidateDeferred)
            .expect("missing sidecar retains the queued Validate target")
            .observe_completion(&Completion::LifecycleValidateSidecarWaiting)
            .expect("registered sidecar wait installs its lane-only barrier");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar
        );
        assert!(claim.blocks_runtime());

        let claim = claim
            .observe_completion(&Completion::LifecycleValidateSidecarWaiting)
            .expect("an unchanged registered wait is an idempotent retry");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar
        );

        let claim = claim
            .observe_completion(&Completion::LifecycleValidateSidecarWoken { ordinal: 41 })
            .expect("the registered wait wakes the same Validate row");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSuccessor { ordinal: 41 }
        );
        let claim = claim
            .observe_completion(&Completion::LifecycleValidateSuccessorCapacityPending {
                ordinal: 41,
            })
            .expect("capacity retry retains the exact woken successor");
        assert_eq!(
            claim,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSuccessor { ordinal: 41 }
        );
        assert_eq!(
            claim.observe_completion(&Completion::LifecycleValidateSuccessorCapacityPending {
                ordinal: 42
            }),
            Err(LifecycleProducerClaimTransitionErrorV1::Completion),
            "a different successor still fails closed"
        );
        assert_eq!(
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
                .observe_completion(&Completion::LifecycleValidateSidecarWoken { ordinal: 41 }),
            Err(LifecycleProducerClaimTransitionErrorV1::Completion),
            "an unrelated non-Producer owner still fails closed"
        );
    }

    #[test]
    fn registered_validate_sidecar_mints_only_the_pacemaker_escape_until_superseded() {
        use super::super::super::v2_lifecycle_coordinator::ProductionLifecycleCompletionSelectionV1 as Completion;

        let sidecar = LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar;
        assert!(sidecar.blocks_runtime());
        assert!(sidecar.validate_sidecar_pacemaker_escape_permit().is_some());
        for claim in [
            LifecycleProducerClaimDispositionV1::Eligible,
            LifecycleProducerClaimDispositionV1::AwaitingValidateSuccessor { ordinal: 1 },
            LifecycleProducerClaimDispositionV1::AwaitingCompletion,
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion,
            LifecycleProducerClaimDispositionV1::ApplyTerminalSettled,
            LifecycleProducerClaimDispositionV1::AwaitingReplayCompletion,
        ] {
            assert!(
                claim.validate_sidecar_pacemaker_escape_permit().is_none(),
                "only the registered missing-sidecar barrier may service the pacemaker"
            );
        }

        assert_eq!(
            sidecar
                .observe_completion(&Completion::LifecycleValidateSidecarSuperseded)
                .expect("durable supersession releases the missing-sidecar barrier"),
            LifecycleProducerClaimDispositionV1::Eligible
        );
        assert_eq!(
            LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
                .observe_completion(&Completion::LifecycleValidateSidecarSuperseded,),
            Err(LifecycleProducerClaimTransitionErrorV1::Completion),
            "supersession cannot clear a Decision-bound Apply barrier"
        );
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

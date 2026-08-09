# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# These are the deductive release modules.  TLC configurations are deliberately
# absent: a finite counterexample search can never satisfy a proof obligation.
RELEASE_PROOF_MODULES = (
    "SumeragiV2QuorumProofs",
    "SumeragiV2VocabularyProofs",
    "SumeragiV2SafetyLemmas",
    "SumeragiV2AgreementLemmas",
    "SumeragiV2ChainEpochProofs",
    "SumeragiV2InductiveProofs",
    "SumeragiV2Proofs",
    "SumeragiV2InstalledTcSelectorProofs",
    "SumeragiV2TimeoutDurability",
    "SumeragiV2TimeoutSigningInvariant",
    "SumeragiV2TimeoutViewInvariant",
    "SumeragiV2TimeoutWireAuthorization",
    "SumeragiV2ChainEpochRefinement",
    "SumeragiV2ChainReceiptAgreementProofs",
    "SumeragiV2SuccessorActivationRefinementProofs",
    "SumeragiV2ChainLivenessProofs",
    "SumeragiV2TemporalLemmas",
    "SumeragiV2LivenessProofs",
    "SumeragiV2ServiceRankLemmas",
    "SumeragiV2FiniteProducerEpisodeProofs",
    "SumeragiV2EffectiveLockAcquisitionProofs",
    "SumeragiV2ReplyRouteOwnershipProofs",
    "SumeragiV2ReplyRoutePipelineProofs",
    "SumeragiV2AsyncNetworkReplyRouteProofs",
    "SumeragiV2ReplyWriterDeadlineProofs",
    "SumeragiV2TypedRolloverHandoffProofs",
    "SumeragiV2AsyncFairnessRefinementProofs",
    "SumeragiV2BeginTimeoutReadyProofs",
    "SumeragiV2RegularCommandFramedReadyProofs",
    "SumeragiV2RegularCommandExecutionReadyProofs",
    "SumeragiV2NonRegularCommandExecutionReadyProofs",
    "SumeragiV2CommandExecutionReadyProofs",
    "SumeragiV2CertifiedRequestHashAuthorityProofs",
    "SumeragiV2DurableDecisionRecoveryProofs",
    "SumeragiV2AsyncNetwork",
    *ASYNC_LIVENESS_PROOF_SHARDS,
    *ASYNC_CAUSAL_EPISODE_PROOF_MODULES,
    "SumeragiV2AsyncFiniteProducerEpisodes",
    *ASYNC_TEMPORAL_CLOSURE_PROOF_MODULES,
    *ADEQUATE_LEADER_CONTINUATION_PROOF_MODULES,
    "SumeragiV2AsyncHistoricalRecoveryLivenessProofs",
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
    "SumeragiV2LockedBodyProposalActionProofs",
    "SumeragiV2TerminalIngressLifecycleProofs",
    "SumeragiTimeoutIngressGuardTest",
)

# Theorem-bearing modules outside the release proof surface must be explicit.
# The former debt shard is intentionally theorem-free; the two scratch modules
# only recheck already-authoritative release bridges. None may be used to
# promote a ledger obligation.
NON_RELEASE_THEOREM_MODULES = {
    ASYNC_LIVENESS_DEBT_SHARD: ASYNC_LIVENESS_DEBT_THEOREMS,
    "SumeragiV2HistoricalLockedBodyRecoveryBridgeScratch": (
        "HistoricalLockedBodyRecoveryProductionRefinementBridgeScratch",
    ),
    "SumeragiV2ProgressWitnessCrossToolScratch": (
        "ScratchRechecksAuthoritativeProgressWitnessCrossToolRefinement",
    ),
}


# These are the only theorem ranges whose strict transcripts may support the
# final nine TLAPS and three cross-tool promotions.  Ledger-facing facade names
# remain explicit, while ``provider_module`` identifies the physical theorem
# declaration that TLAPM must select.  The three cross-tool records name the
# bridge theorem, never the operator-only ledger obligation.
PROMOTION_TLAPS_TARGET_IDS = (
    "post-gst-deadlock-freedom",
    "post-gst-starvation-freedom",
    "timeout-view-liveness",
    "rotating-leader-liveness",
    "locked-body-reproposal",
    "application-liveness",
    "successor-activation-starvation-freedom",
    "genesis-height-successor-handoff",
    "height-liveness",
)
PROMOTION_CROSS_TOOL_TARGET_IDS = (
    "effective-lock-body-acquisition-production-refinement",
    "progress-witness-production-refinement",
    "successor-activation-exact-recovery-production-refinement",
)
PROMOTION_PROOF_TARGET_CONTRACTS = (
    PromotionProofTargetContract(
        "post-gst-deadlock-freedom",
        "tlaps",
        "SumeragiV2AsyncLivenessProofs",
        "SumeragiV2AsyncDeadlockProofs",
        "DeadlockFreedomObligation",
    ),
    PromotionProofTargetContract(
        "post-gst-starvation-freedom",
        "tlaps",
        "SumeragiV2AsyncLivenessProofs",
        "SumeragiV2AsyncDeadlockProofs",
        "StarvationFreedomObligation",
    ),
    PromotionProofTargetContract(
        "timeout-view-liveness",
        "tlaps",
        "SumeragiV2AsyncTemporalClosureProofs",
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureTimeoutViewProgressObligation",
    ),
    PromotionProofTargetContract(
        "rotating-leader-liveness",
        "tlaps",
        "SumeragiV2AsyncTemporalClosureProofs",
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureRotatingLeaderProgressObligation",
    ),
    PromotionProofTargetContract(
        "locked-body-reproposal",
        "tlaps",
        "SumeragiV2AsyncTemporalClosureProofs",
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
    ),
    PromotionProofTargetContract(
        "application-liveness",
        "tlaps",
        "SumeragiV2AsyncTemporalClosureProofs",
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureApplicationCompletionProgressObligation",
    ),
    PromotionProofTargetContract(
        "successor-activation-starvation-freedom",
        "tlaps",
        "SumeragiV2SuccessorActivationRefinementProofs",
        "SumeragiV2SuccessorActivationRefinementProofs",
        "SuccessorActivationStarvationFreedomObligation",
    ),
    PromotionProofTargetContract(
        "genesis-height-successor-handoff",
        "tlaps",
        "SumeragiV2ChainEpochRefinement",
        "SumeragiV2ChainEpochRefinement",
        "GenesisHeightSuccessorHandoffObligation",
    ),
    PromotionProofTargetContract(
        "height-liveness",
        "tlaps",
        "SumeragiV2ChainLivenessProofs",
        "SumeragiV2ChainLivenessProofs",
        "HeightLivenessObligation",
    ),
    PromotionProofTargetContract(
        "effective-lock-body-acquisition-production-refinement",
        "cross_tool",
        "SumeragiV2AsyncLivenessProofs",
        "SumeragiV2AsyncStage4RefinementProofs",
        "EffectiveLockBodyAcquisitionCrossToolRefinement",
    ),
    PromotionProofTargetContract(
        "progress-witness-production-refinement",
        "cross_tool",
        "SumeragiV2AsyncTemporalClosureProofs",
        "SumeragiV2AsyncTemporalClosureProofs",
        "ProgressWitnessCrossToolRefinement",
    ),
    PromotionProofTargetContract(
        "successor-activation-exact-recovery-production-refinement",
        "cross_tool",
        "SumeragiV2ChainEpochRefinement",
        "SumeragiV2ChainEpochRefinement",
        "SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement",
    ),
)

# Reviewed release obligation inventory.  Keeping this independent from the
# checked-in ledger makes removing, adding, reordering, or retargeting an
# obligation an explicit proof-gate change rather than a self-authorizing JSON
# edit.  Requirement prose and proof status remain ledger-owned so proof debt
# can be promoted without rewriting this structural contract.
REQUIRED_PROOF_OBLIGATION_INVENTORY = {
    "dual-quorum-definition": (
        "SumeragiV2QuorumProofs",
        "DualQuorumCarriesBothThresholds",
    ),
    "vocabulary-helper-facts": (
        "SumeragiV2VocabularyProofs",
        "PrepareSignerAvailabilityIncludesDurability / "
        "CrashDoesNotErasePrepareIntents / CrashDoesNotEraseDecisions / "
        "IncompleteFrameIsNotAcknowledged / ContextRecordCarriesFrozenEpoch / "
        "ContextRecordCarriesParent / ContextRecordCarriesParentContext / "
        "EquivalentParentCommitQcsConverge / ForeignParentLineageHasDifferentIdentity",
    ),
    "quorum-honest-intersection": (
        "SumeragiV2Proofs",
        "QuorumIntersectionObligation",
    ),
    "durable-vote-append-kernel": (
        "SumeragiV2SafetyLemmas",
        "DurableVoteAppendPreservesUniqueness / "
        "DurableTimeoutAppendPreservesUniqueness",
    ),
    "same-view-certificate-kernel": (
        "SumeragiV2SafetyLemmas",
        "SameViewCertificateUniqueness",
    ),
    "validity-availability-kernel": (
        "SumeragiV2SafetyLemmas",
        "BackedCertificateIsValidAndAvailable",
    ),
    "lock-transition-kernel": (
        "SumeragiV2SafetyLemmas",
        "CommitPersistenceAdvancesLockMonotonically / "
        "TimeoutInstallationAdvancesLockMonotonically / MonotoneLockUpdatesCompose",
    ),
    "timeout-protection-kernel": (
        "SumeragiV2SafetyLemmas",
        "GroupedTimeoutProtectsCommitQuorum",
    ),
    "timeout-envelope-schema": (
        "SumeragiTimeoutIngressGuardTest",
        "SelectedViewCheckDoesNotEstablishEnvelopeSchema / "
        "FullEnvelopeGuardTypesTimeoutVote / DeliverTimeoutRequiresCanonicalEnvelope / "
        "AsyncSentTimeoutItemCarriesCanonicalEnvelope / "
        "ExecuteCoreTimeoutDeliveryCarriesCanonicalEnvelope / "
        "AsyncTypedTimeoutDeliveryRefinesCoreStep",
    ),
    "timeout-wire-authorization": (
        "SumeragiV2TimeoutWireAuthorization",
        "CoreSpecAtAlwaysStrongTimeoutWireAuthorizationInvariant / "
        "StrongWireInvariantAuthorizesPendingTimeoutSignature / "
        "StrongWireInvariantAuthorizesHonestTimeoutEnvelope",
    ),
    "durable-vote-uniqueness": (
        "SumeragiV2Proofs",
        "DurableVoteUniquenessObligation",
    ),
    "lock-monotonicity": ("SumeragiV2Proofs", "LockMonotonicityObligation"),
    "external-validity": ("SumeragiV2Proofs", "ExternalValidityObligation"),
    "certified-body-availability": (
        "SumeragiV2Proofs",
        "AvailabilityObligation",
    ),
    "certificate-uniqueness": (
        "SumeragiV2Proofs",
        "CertificateUniquenessObligation",
    ),
    "same-round-lock-and-commit-authorization": (
        "SumeragiV2Proofs",
        "SameRoundLockAndCommitAuthorizationObligation",
    ),
    "timeout-protection": ("SumeragiV2Proofs", "TimeoutProtectionObligation"),
    "agreement": ("SumeragiV2Proofs", "AgreementObligation"),
    "no-conflicting-commit-qcs": (
        "SumeragiV2Proofs",
        "NoConflictingCommitCertificatesObligation",
    ),
    "chain-prefix": ("SumeragiV2ChainEpochProofs", "ChainPrefixObligation"),
    "crash-restart": ("SumeragiV2Proofs", "CrashRecoveryObligation"),
    "epoch-boundary": ("SumeragiV2ChainEpochProofs", "EpochBoundaryObligation"),
    "effective-lock-body-acquisition-model": (
        "SumeragiV2EffectiveLockAcquisitionProofs",
        "EffectiveLockAcquisitionModelObligation",
    ),
    "effective-lock-body-acquisition-production-refinement": (
        "SumeragiV2AsyncLivenessProofs",
        "EffectiveLockBodyAcquisitionProductionRefinementObligation",
    ),
    "async-runner-scheduler-preservation": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncRunnerStepPreservesSchedulerType",
    ),
    "async-type-invariant": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncTypeInvariantObligation",
    ),
    "async-progress-ownership-invariant": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
    ),
    "post-decision-timeout-exclusion": (
        "SumeragiV2AsyncLivenessProofs",
        "PostDecisionTimeoutExclusionObligation",
    ),
    "decision-recovery-across-restart": (
        "SumeragiV2AsyncLivenessProofs",
        "DecisionRecoveryAcrossRestartObligation",
    ),
    "async-fair-action-refinement": (
        "SumeragiV2AsyncFairnessRefinementProofs",
        "AsyncFairActionsRefineAsyncNextObligation",
    ),
    "generation-scoped-vote-delivery": (
        "SumeragiV2AsyncLivenessProofs",
        "GenerationScopedVoteDeliveryObligation",
    ),
    "progress-witness-preservation": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ProgressWitnessObligation",
    ),
    "progress-witness-production-refinement": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ProgressWitnessProductionRefinementObligation",
    ),
    "protected-service-rank-stage4-ready-causal": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedStage4RankProgressFromFairScheduler",
    ),
    "protected-service-rank-serve-fifo": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedServeRankProgressFromFairFifo",
    ),
    "protected-service-rank-stage5-consensus-fifo": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedStage5RankProgressFromFairFifo",
    ),
    "protected-service-rank": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedServiceRankProgressObligation",
    ),
    "post-gst-deadlock-freedom": (
        "SumeragiV2AsyncLivenessProofs",
        "DeadlockFreedomObligation",
    ),
    "post-gst-starvation-freedom": (
        "SumeragiV2AsyncLivenessProofs",
        "StarvationFreedomObligation",
    ),
    "timeout-view-liveness": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureTimeoutViewProgressObligation",
    ),
    "rotating-leader-liveness": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureRotatingLeaderProgressObligation",
    ),
    "locked-body-reproposal": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
    ),
    "application-liveness": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureApplicationCompletionProgressObligation",
    ),
    "successor-activation-starvation-freedom": (
        "SumeragiV2SuccessorActivationRefinementProofs",
        "SuccessorActivationStarvationFreedomObligation",
    ),
    "successor-activation-exact-recovery-production-refinement": (
        "SumeragiV2ChainEpochRefinement",
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
    ),
    "genesis-height-successor-handoff": (
        "SumeragiV2ChainEpochRefinement",
        "GenesisHeightSuccessorHandoffObligation",
    ),
    "height-liveness": (
        "SumeragiV2ChainLivenessProofs",
        "HeightLivenessObligation",
    ),
    "cryptography": ("trusted-boundary", "cryptography"),
    "durability-system-call": ("trusted-boundary", "os-fsync"),
    "deterministic-execution": ("trusted-boundary", "deterministic-execution"),
    "post-gst-responsive-quorum": (
        "trusted-boundary",
        "post-gst-responsive-quorum",
    ),
    "network-after-gst": ("trusted-boundary", "post-gst-delivery"),
    "runtime-after-gst": ("trusted-boundary", "post-gst-runtime-service"),
    "transaction-inclusion-fairness": (
        "trusted-boundary",
        "transaction-inclusion-fairness",
    ),
}

# These sixteen declarations are proof/evidence decomposition leaves, not
# independently reviewed release claims.  They remain source-bound below and
# must be consumed by one of the exact 54 top-level obligations rather than
# being promoted into additional ledger rows.
SUPPORT_PROOF_OBLIGATION_INVENTORY = {
    "chain-durable-receipt-agreement": (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedChainSpecEstablishesExactPerSlotReceiptAgreement",
    ),
    "terminal-ingress-process-lifetime-absorbency": (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressProcessLifetimeAbsorbencyObligation",
    ),
    "adequate-leader-exact-closure-residual": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderExactClosureResidualObligation",
    ),
    "exact-decision-off-scheduler-residual-convergence": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ExactDecisionOffSchedulerResidualConvergenceObligation",
    ),
    "historical-recovery-authority-acquisition": (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
    ),
    "historical-recovery-certificate-rank-progress": (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankProgressResidualObligation",
    ),
    "historical-recovery-decision-stage-ownership": (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageOwnershipResidualObligation",
    ),
    "historical-recovery-decision-rank-progress": (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
    ),
    "autoscale-lifecycle-production-refinement": (
        "SumeragiV2AutoscaleLifecycle",
        "AutoscaleLifecycleProductionRefinementObligation",
    ),
    "native-application-evidence-production-refinement": (
        "SumeragiV2NativeApplicationEvidence",
        "NativeApplicationEvidenceProductionRefinementObligation",
    ),
    "autonomous-reservation-carrier-production-refinement": (
        "SumeragiV2AutonomousReservationCarrier",
        "AutonomousReservationCarrierProductionRefinementObligation",
    ),
    "reply-writer-deadline-local-termination": (
        "SumeragiV2ReplyWriterDeadlineProofs",
        "ReplyWriterDeadlineModelObligation",
    ),
    "reply-writer-conditional-responsive-cursor-liveness": (
        "SumeragiV2ReplyWriterDeadlineProofs",
        "ConditionalResponsiveWriterCursorLiveness",
    ),
    "reply-writer-responsive-strong-fairness-to-receipt": (
        "SumeragiV2ReplyWriterDeadlineProofs",
        "ResponsiveStrongFairnessToReceiptResidual",
    ),
    "typed-rollover-handoff-model-safety": (
        "SumeragiV2TypedRolloverHandoffProofs",
        "TypedRolloverSpecAlwaysSafeObligation / "
        "TypedRolloverNextPreservesSafetyObligation",
    ),
    "typed-rollover-handoff-conditional-local-liveness": (
        "SumeragiV2TypedRolloverHandoffProofs",
        "ResponsiveDurableExactOutputRolloverLivenessObligation / "
        "ResponsiveRestartRestoreRolloverLivenessObligation",
    ),
}

# Historical recovery's four reviewed decomposition leaves are deductively
# proved. They remain source-bound support for height liveness and may never
# fall back into proofless-support accounting.
HISTORICAL_RECOVERY_PROOFLESS_SUPPORT_IDS: tuple[str, ...] = ()
HISTORICAL_RECOVERY_PROVED_SUPPORT_IDS = (
    "historical-recovery-authority-acquisition",
    "historical-recovery-certificate-rank-progress",
    "historical-recovery-decision-stage-ownership",
    "historical-recovery-decision-rank-progress",
)
DEDUCTIVELY_PROVED_SUPPORT_IDS = (
    "chain-durable-receipt-agreement",
    "terminal-ingress-process-lifetime-absorbency",
    "adequate-leader-exact-closure-residual",
    "exact-decision-off-scheduler-residual-convergence",
) + HISTORICAL_RECOVERY_PROVED_SUPPORT_IDS + (
    "reply-writer-deadline-local-termination",
    "reply-writer-conditional-responsive-cursor-liveness",
    "reply-writer-responsive-strong-fairness-to-receipt",
    "typed-rollover-handoff-model-safety",
    "typed-rollover-handoff-conditional-local-liveness",
)
# Every reviewed temporal support leaf now has a deductive proof body. Keep the
# empty class explicit so a future proofless compatibility surface cannot be
# folded into a proved consumer without updating the completion contract.
STRICT_PROOFLESS_TEMPORAL_SUPPORT_IDS: tuple[str, ...] = ()
CROSS_TOOL_OPERATOR_SUPPORT_IDS = (
    "autoscale-lifecycle-production-refinement",
    "native-application-evidence-production-refinement",
    "autonomous-reservation-carrier-production-refinement",
)

SUPPORT_PROOF_CONSUMER_BY_ID = {
    "chain-durable-receipt-agreement": "height-liveness",
    "terminal-ingress-process-lifetime-absorbency": (
        "successor-activation-exact-recovery-production-refinement"
    ),
    "adequate-leader-exact-closure-residual": "rotating-leader-liveness",
    "exact-decision-off-scheduler-residual-convergence": "application-liveness",
    "historical-recovery-authority-acquisition": "height-liveness",
    "historical-recovery-certificate-rank-progress": "height-liveness",
    "historical-recovery-decision-stage-ownership": "height-liveness",
    "historical-recovery-decision-rank-progress": "height-liveness",
    "autoscale-lifecycle-production-refinement": "progress-witness-production-refinement",
    "native-application-evidence-production-refinement": (
        "progress-witness-production-refinement"
    ),
    "autonomous-reservation-carrier-production-refinement": (
        "progress-witness-production-refinement"
    ),
    "reply-writer-deadline-local-termination": "progress-witness-production-refinement",
    "reply-writer-conditional-responsive-cursor-liveness": (
        "progress-witness-production-refinement"
    ),
    "reply-writer-responsive-strong-fairness-to-receipt": (
        "progress-witness-production-refinement"
    ),
    "typed-rollover-handoff-model-safety": (
        "successor-activation-exact-recovery-production-refinement"
    ),
    "typed-rollover-handoff-conditional-local-liveness": (
        "successor-activation-exact-recovery-production-refinement"
    ),
}
